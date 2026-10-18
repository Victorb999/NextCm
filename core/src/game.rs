//! Um jogo em curso (Fase 3): o save é uma cópia do `world.sqlite` com as tabelas
//! `game`, `news`, `fixtures`, `player_state`, `selection` e `selection_players`. O estado que muda dia a dia fica em memória e só vai para
//! o arquivo em [`Game::save`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::engine::{self, TeamStrength};
use crate::squad::{Event, PlayerState, PlayerStatus, Side, Squads};
use crate::tactics::{ClubTactics, Instructions, Mentality, Passing, Selection, Tackling};
use crate::league::{self, Fixture, Match, SERIE_A, StandingRow};
use crate::person::ClubRef;
use crate::search::fold;
use crate::{Error, Result, World, date};

/// Versão do formato do save; saves de outra versão são recusados até haver migração.
/// 2: tabela `fixtures` (marco 3.2). 3: estado dos jogadores e a escolha da equipa (3.3).
/// 4: posições livres no campo (`selection_players.x/y`) e titulares na ordem do CM.
/// 5: setas de movimento (`selection_players.run_x/run_y`).
pub const SAVE_VERSION: i32 = 5;

/// Quantos dias "Continuar Jogo" avança no máximo quando não acontece nada.
const MAX_IDLE_DAYS: i32 = 42;

/// Dia em que o jogo começa (o CM 01/02 começa a 1 de julho; patch 25-26).
const START_MONTH_DAY: &str = "07-01";

/// Cargo "Treinador" na tabela `jobs`.
const HEAD_COACH: i32 = 5;

const SCHEMA: &str = "
CREATE TABLE game (
    version INTEGER NOT NULL,
    name TEXT NOT NULL,
    date TEXT NOT NULL,
    seed INTEGER NOT NULL,
    manager_id INTEGER NOT NULL,
    club_id INTEGER NOT NULL,
    created TEXT NOT NULL
);
CREATE TABLE news (
    id INTEGER PRIMARY KEY,
    date TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    read INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE fixtures (
    id INTEGER PRIMARY KEY,
    competition_id INTEGER NOT NULL,
    round INTEGER NOT NULL,
    date TEXT NOT NULL,
    home_id INTEGER NOT NULL,
    away_id INTEGER NOT NULL,
    home_goals INTEGER,
    away_goals INTEGER
);
CREATE INDEX fixtures_date ON fixtures (date);
CREATE TABLE player_state (
    person_id INTEGER PRIMARY KEY,
    condition INTEGER NOT NULL,
    morale INTEGER NOT NULL,
    yellows INTEGER NOT NULL,
    suspended INTEGER NOT NULL,
    injury TEXT,
    injury_until TEXT
);
CREATE TABLE selection (
    club_id INTEGER PRIMARY KEY,
    formation INTEGER NOT NULL,
    auto INTEGER NOT NULL,
    mentality TEXT NOT NULL,
    passing TEXT NOT NULL,
    tackling TEXT NOT NULL,
    pressing INTEGER NOT NULL,
    offside_trap INTEGER NOT NULL,
    counter_attack INTEGER NOT NULL,
    tight_marking INTEGER NOT NULL
);
-- slot 0–10: titulares (na ordem das funções, como na lista do CM); 100–108: suplentes.
-- x/y: posição livre no campo, quando o treinador a mexeu.
CREATE TABLE selection_players (
    club_id INTEGER NOT NULL,
    slot INTEGER NOT NULL,
    person_id INTEGER,
    x INTEGER,
    y INTEGER,
    run_x INTEGER,
    run_y INTEGER,
    PRIMARY KEY (club_id, slot)
);
";

/// Dados do ecrã "Novo Jogo".
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NewGame {
    /// Nome do save (vira o nome do arquivo).
    pub save_name: String,
    pub first_name: String,
    pub last_name: String,
    /// "AAAA-MM-DD"
    pub birth_date: Option<String>,
    pub nation_id: Option<i32>,
    pub club_id: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub id: i32,
    pub date: String,
    /// "message" (mensagens), "competition", "injury"
    pub kind: String,
    pub title: String,
    pub body: String,
    pub read: bool,
}

/// O que a coluna esquerda e o menu do treinador precisam.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub name: String,
    pub date: String,
    pub manager_id: i32,
    pub manager_name: String,
    pub club: ClubRef,
    pub unread_news: i32,
    /// Há mudanças desde a última gravação.
    pub unsaved: bool,
}

/// Uma linha da lista "Carregar Jogo".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SaveSummary {
    pub file: String,
    pub name: String,
    pub manager_name: String,
    pub club_name: String,
    pub date: String,
    /// Segundos desde 1970 da última gravação (data do arquivo).
    #[ts(type = "number")]
    pub saved_at: i64,
}

pub struct Game {
    world: World,
    path: PathBuf,
    name: String,
    date: String,
    seed: i64,
    manager_id: i32,
    club_id: i32,
    news: Vec<NewsItem>,
    /// Calendário de todas as competições vivas (no 3.2, só a Série A).
    matches: Vec<Match>,
    /// Clubes das competições vivas, por id.
    clubs: HashMap<i32, ClubRef>,
    /// Nome curto das competições vivas ("Série A").
    competitions: HashMap<i32, String>,
    /// Condição, moral, lesões e cartões dos jogadores dos clubes vivos.
    squads: Squads,
    /// A equipa escolhida pelo treinador; `None` = o adjunto escolhe (instruções normais).
    selection: Option<Selection>,
    unsaved: bool,
}

/// "Nome do save" → nome de arquivo seguro (letras, números, espaço, - e _).
fn file_stem(name: &str) -> Result<String> {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if stem.trim().is_empty() {
        return Err(Error::Invalid("Escolha um nome para o jogo.".into()));
    }
    Ok(stem)
}

impl Game {
    /// Cria o save a partir de `world_path`, põe o treinador no clube e grava.
    pub fn create(world_path: &Path, saves_dir: &Path, new: &NewGame, seed: i64) -> Result<Game> {
        let first = new.first_name.trim();
        let last = new.last_name.trim();
        if first.is_empty() || last.is_empty() {
            return Err(Error::Invalid("Indique o nome e o apelido do treinador.".into()));
        }
        if let Some(b) = &new.birth_date {
            if date::parse(b).is_none() {
                return Err(Error::Invalid("Data de nascimento inválida.".into()));
            }
        }
        std::fs::create_dir_all(saves_dir)?;
        let path = saves_dir.join(format!("{}.sqlite", file_stem(&new.save_name)?));
        if path.exists() {
            return Err(Error::Invalid(format!("Já existe um jogo chamado \"{}\".", new.save_name.trim())));
        }
        std::fs::copy(world_path, &path)?;

        let result = (|| {
            let world = World::open_save(&path)?;
            let start = format!("{}-{START_MONTH_DAY}", world.start_year());
            let club = world.club_ref(Some(new.club_id))?.ok_or_else(|| Error::NotFound(format!("clube {}", new.club_id)))?;

            let tx = world.conn.unchecked_transaction()?;
            tx.execute_batch(SCHEMA)?;
            let manager_id: i32 = tx.query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM people", [], |r| r.get(0))?;
            // O treinador anterior fica sem clube, como no CM
            tx.execute(
                "UPDATE people SET club_id = NULL, club_job = NULL WHERE club_id = ? AND club_job = ?",
                params![new.club_id, HEAD_COACH],
            )?;
            let display = format!("{first} {last}");
            let birth_year = new.birth_date.as_deref().and_then(|b| b.get(0..4)?.parse::<i32>().ok());
            let contract = format!("{}-06-30", world.start_year() + 2);
            tx.execute(
                "INSERT INTO people (id, display_name, first_name, second_name, common_name, search_name, list_name,
                                     birth_date, birth_year, nation_id, international_apps, international_goals,
                                     club_id, club_job, club_joined, contract_expires,
                                     is_player, is_non_player, birth_sort, positions)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, 0, 0, ?10, ?11, ?12, ?13, 0, 0, ?7, 0)",
                params![
                    manager_id, display, first, last, fold(&display), format!("{last}, {first}"),
                    new.birth_date, birth_year, new.nation_id, new.club_id, HEAD_COACH, start, contract
                ],
            )?;
            tx.execute(
                "INSERT INTO game (version, name, date, seed, manager_id, club_id, created) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![SAVE_VERSION, new.save_name.trim(), start, seed, manager_id, new.club_id, start],
            )?;
            tx.commit()?;

            // Calendário das competições vivas, sorteado com a seed do jogo
            let ids = league_club_ids(&world, SERIE_A)?;
            let matches = league::round_robin(SERIE_A, &ids, world.start_year(), 1, &mut ChaCha8Rng::seed_from_u64(seed as u64));
            let clubs = club_refs(&world, &ids)?;
            let competitions = competition_names(&world, &matches)?;
            fix_squad_numbers(&world, &ids, seed)?;
            let squads = Squads { states: HashMap::new(), traits: Squads::load_traits(&world, &ids)? };

            let welcome = welcome_news(&world, &club, &display, &start)?;
            let mut game = Game {
                world,
                path: path.clone(),
                name: new.save_name.trim().into(),
                date: start,
                seed,
                manager_id,
                club_id: new.club_id,
                news: vec![welcome],
                competitions,
                matches,
                clubs,
                squads,
                selection: None,
                unsaved: true,
            };
            if let Some(n) = game.season_news(2) {
                game.news.insert(0, n);
            }
            game.save()?;
            Ok(game)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&path);
        }
        result
    }

    pub fn load(path: &Path) -> Result<Game> {
        let world = World::open_save(path)?;
        let row = world
            .conn
            .query_row("SELECT version, name, date, seed, manager_id, club_id FROM game", [], |r| {
                Ok((r.get::<_, i32>(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })
            .optional()
            .map_err(|_| Error::Invalid("Este arquivo não é um jogo do NextCM.".into()))?
            .ok_or_else(|| Error::Invalid("Este arquivo não é um jogo do NextCM.".into()))?;
        let (version, name, date, seed, manager_id, club_id) = row;
        let mut version = version;
        if version == 3 {
            migrate_v3(&world, club_id)?;
            version = 4;
        }
        if version == 4 {
            world.conn.execute_batch(
                "ALTER TABLE selection_players ADD COLUMN run_x INTEGER;
                 ALTER TABLE selection_players ADD COLUMN run_y INTEGER;
                 UPDATE game SET version = 5;",
            )?;
            version = 5;
        }
        if version != SAVE_VERSION {
            return Err(Error::Invalid(format!(
                "Jogo gravado numa versão diferente do NextCM (formato {version}, esperado {SAVE_VERSION})."
            )));
        }
        let news = world
            .conn
            .prepare("SELECT id, date, kind, title, body, read FROM news ORDER BY date DESC, id DESC")?
            .query_map([], |r| {
                Ok(NewsItem {
                    id: r.get(0)?,
                    date: r.get(1)?,
                    kind: r.get(2)?,
                    title: r.get(3)?,
                    body: r.get(4)?,
                    read: r.get::<_, i32>(5)? != 0,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let matches: Vec<Match> = world
            .conn
            .prepare(
                "SELECT id, competition_id, round, date, home_id, away_id, home_goals, away_goals
                 FROM fixtures ORDER BY id",
            )?
            .query_map([], |r| {
                let goals: (Option<u8>, Option<u8>) = (r.get(6)?, r.get(7)?);
                Ok(Match {
                    id: r.get(0)?,
                    competition_id: r.get(1)?,
                    round: r.get(2)?,
                    date: r.get(3)?,
                    home_id: r.get(4)?,
                    away_id: r.get(5)?,
                    result: goals.0.zip(goals.1),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut ids: Vec<i32> = matches.iter().flat_map(|m| [m.home_id, m.away_id]).collect();
        ids.sort_unstable();
        ids.dedup();
        let clubs = club_refs(&world, &ids)?;
        let competitions = competition_names(&world, &matches)?;
        let states = world
            .conn
            .prepare("SELECT person_id, condition, morale, yellows, suspended, injury, injury_until FROM player_state")?
            .query_map([], |r| {
                Ok((r.get(0)?, PlayerState {
                    condition: r.get(1)?,
                    morale: r.get(2)?,
                    yellows: r.get(3)?,
                    suspended: r.get(4)?,
                    injury: r.get(5)?,
                    injury_until: r.get(6)?,
                }))
            })?
            .collect::<rusqlite::Result<_>>()?;
        fix_squad_numbers(&world, &ids, seed)?;
        let squads = Squads { states, traits: Squads::load_traits(&world, &ids)? };
        let selection = load_selection(&world, club_id)?;
        Ok(Game {
            world,
            path: path.to_path_buf(),
            name,
            date,
            seed,
            manager_id,
            club_id,
            news,
            matches,
            clubs,
            competitions,
            squads,
            selection,
            unsaved: false,
        })
    }

    /// Grava o estado em memória no arquivo do save.
    pub fn save(&mut self) -> Result<()> {
        let tx = self.world.conn.unchecked_transaction()?;
        tx.execute("UPDATE game SET date = ?, seed = ?", params![self.date, self.seed])?;
        tx.execute("DELETE FROM news", [])?;
        {
            let mut stmt = tx.prepare("INSERT INTO news (id, date, kind, title, body, read) VALUES (?, ?, ?, ?, ?, ?)")?;
            for n in &self.news {
                stmt.execute(params![n.id, n.date, n.kind, n.title, n.body, n.read as i32])?;
            }
        }
        tx.execute("DELETE FROM fixtures", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO fixtures (id, competition_id, round, date, home_id, away_id, home_goals, away_goals)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )?;
            for m in &self.matches {
                let (hg, ag) = (m.result.map(|r| r.0), m.result.map(|r| r.1));
                stmt.execute(params![m.id, m.competition_id, m.round, m.date, m.home_id, m.away_id, hg, ag])?;
            }
        }
        tx.execute("DELETE FROM player_state", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO player_state (person_id, condition, morale, yellows, suspended, injury, injury_until)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )?;
            for (id, st) in &self.squads.states {
                if *st != PlayerState::default() {
                    stmt.execute(params![id, st.condition, st.morale, st.yellows, st.suspended, st.injury, st.injury_until])?;
                }
            }
        }
        tx.execute("DELETE FROM selection", [])?;
        tx.execute("DELETE FROM selection_players", [])?;
        if let Some(sel) = &self.selection {
            let i = &sel.instructions;
            let (mentality, passing, tackling) = instruction_keys(i);
            tx.execute(
                "INSERT INTO selection (club_id, formation, auto, mentality, passing, tackling, pressing, offside_trap,
                                        counter_attack, tight_marking) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    self.club_id, sel.formation, sel.auto, mentality, passing, tackling, i.pressing, i.offside_trap, i.counter_attack, i.tight_marking
                ],
            )?;
            let mut stmt = tx.prepare(
                "INSERT INTO selection_players (club_id, slot, person_id, x, y, run_x, run_y) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )?;
            for (slot, pid) in sel.starters.iter().enumerate() {
                let xy = sel.positions.get(slot);
                let run = sel.runs.get(slot).copied().flatten();
                stmt.execute(params![
                    self.club_id, slot as i32, pid, xy.map(|p| p[0]), xy.map(|p| p[1]), run.map(|r| r[0]), run.map(|r| r[1])
                ])?;
            }
            for (i, pid) in sel.bench.iter().enumerate() {
                stmt.execute(params![self.club_id, 100 + i as i32, pid, None::<i32>, None::<i32>, None::<i32>, None::<i32>])?;
            }
        }
        tx.commit()?;
        self.unsaved = false;
        Ok(())
    }

    /// Avança um dia, joga as partidas desse dia e devolve as notícias novas.
    pub fn advance_day(&mut self) -> Result<Vec<NewsItem>> {
        self.date = date::add_days(&self.date, 1).ok_or_else(|| Error::Invalid("data do jogo inválida".into()))?;
        self.unsaved = true;

        let mut fresh = Vec::new();
        let mut next_id = self.news.iter().map(|n| n.id).max().unwrap_or(0) + 1;

        // Recuperação do dia; quem do clube do treinador voltou de lesão vira notícia
        for id in self.squads.new_day(&self.date.clone()) {
            if self.squads.traits.get(&id).is_some_and(|t| t.club_id == self.club_id) {
                let name = self.person_name(id)?;
                fresh.push(self.news_item(
                    next_id,
                    "injury",
                    format!("{name} recuperado"),
                    format!("{name} já está recuperado da lesão e pode voltar a jogar."),
                ));
                next_id += 1;
            }
        }

        let today: Vec<usize> =
            (0..self.matches.len()).filter(|&i| self.matches[i].date == self.date && self.matches[i].result.is_none()).collect();
        for &i in &today {
            let (id, home, away) = (self.matches[i].id, self.matches[i].home_id, self.matches[i].away_id);
            let (home_t, home_changes) = self.lineup(home)?;
            let (away_t, away_changes) = self.lineup(away)?;
            if let Some(changes) = home_changes.or(away_changes) {
                if !changes.is_empty() {
                    fresh.push(self.changes_news(next_id, &changes)?);
                    next_id += 1;
                }
            }
            let mut rng = engine::match_rng(self.seed, id);
            let result = engine::play(&TeamStrength::from_tactics(&home_t), &TeamStrength::from_tactics(&away_t), &mut rng);
            self.matches[i].result = Some(result);

            let starters = |t: &ClubTactics| t.starters.iter().filter_map(|s| s.player.as_ref().map(|p| p.id)).collect::<Vec<_>>();
            let (hs, as_) = (starters(&home_t), starters(&away_t));
            let sides = [
                Side { club: home, starters: &hs, instructions: home_t.instructions, goals_for: result.0, goals_against: result.1 },
                Side { club: away, starters: &as_, instructions: away_t.instructions, goals_for: result.1, goals_against: result.0 },
            ];
            let date = self.date.clone();
            for event in self.squads.after_match(sides, &date, &mut rng) {
                if let Some(n) = self.event_news(next_id, &event)? {
                    fresh.push(n);
                    next_id += 1;
                }
            }
        }
        if today.is_empty() {
            for n in &fresh {
                self.news.insert(0, n.clone());
            }
            fresh.reverse();
            return Ok(fresh);
        }

        for &i in &today {
            let m = &self.matches[i];
            if m.home_id == self.club_id || m.away_id == self.club_id {
                fresh.push(self.result_news(next_id, m));
                next_id += 1;
            }
        }
        // Competições que acabaram hoje: o campeão
        let mut finished: Vec<i32> = today.iter().map(|&i| self.matches[i].competition_id).collect();
        finished.dedup();
        for comp in finished {
            if self.matches.iter().all(|m| m.competition_id != comp || m.result.is_some()) {
                fresh.push(self.champion_news(next_id, comp)?);
                next_id += 1;
            }
        }
        for n in &fresh {
            self.news.insert(0, n.clone());
        }
        fresh.reverse();
        Ok(fresh)
    }

    /// "Continuar Jogo": avança dia a dia até haver notícias (o jogo do treinador, o fim
    /// de uma competição...), até à véspera de um jogo do treinador (para ver o Próximo
    /// Jogo antes de o jogar) ou até [`MAX_IDLE_DAYS`] dias sem nada.
    pub fn advance_to_next_event(&mut self) -> Result<Vec<NewsItem>> {
        for _ in 0..MAX_IDLE_DAYS {
            let news = self.advance_day()?;
            if !news.is_empty() {
                return Ok(news);
            }
            let tomorrow = date::add_days(&self.date, 1).unwrap_or_default();
            if self.matches.iter().any(|m| m.date == tomorrow && (m.home_id == self.club_id || m.away_id == self.club_id)) {
                break;
            }
        }
        Ok(Vec::new())
    }

    /// A equipa de um clube para o jogo de hoje. No clube do treinador, a escolha dele
    /// (com os indisponíveis trocados pelo adjunto, devolvidos em `Some(trocas)`).
    fn lineup(&self, club: i32) -> Result<(ClubTactics, Option<Vec<(Option<i32>, i32)>>)> {
        let state = self.squads.squad_state(club);
        if club != self.club_id {
            return Ok((self.world.auto_tactics(club, Some(&state), Instructions::default())?, None));
        }
        match &self.selection {
            Some(sel) if !sel.auto => {
                let mut sel = sel.clone();
                let changes = self.world.cover_unavailable(club, &mut sel, &state)?;
                Ok((self.world.tactics_from_selection(club, &sel, Some(&state))?, Some(changes)))
            }
            other => {
                let instructions = other.as_ref().map(|s| s.instructions).unwrap_or_default();
                Ok((self.world.auto_tactics(club, Some(&state), instructions)?, None))
            }
        }
    }

    /// Tácticas de um clube agora: o do treinador é editável; os outros, o que o adjunto escolheria.
    pub fn club_tactics(&self, club: i32) -> Result<ClubTactics> {
        let state = self.squads.squad_state(club);
        let mut t = match (&self.selection, club == self.club_id) {
            (Some(sel), true) if !sel.auto => self.world.tactics_from_selection(club, sel, Some(&state))?,
            (sel, true) => self.world.auto_tactics(club, Some(&state), sel.as_ref().map(|s| s.instructions).unwrap_or_default())?,
            _ => self.world.auto_tactics(club, Some(&state), Instructions::default())?,
        };
        t.editable = club == self.club_id;
        Ok(t)
    }

    /// Como ficaria a equipa com esta escolha (sem gravar): para a tela de Tácticas.
    pub fn preview_selection(&self, sel: &Selection) -> Result<ClubTactics> {
        let state = self.squads.squad_state(self.club_id);
        let mut t = if sel.auto {
            self.world.auto_tactics(self.club_id, Some(&state), sel.instructions)?
        } else {
            self.world.tactics_from_selection(self.club_id, sel, Some(&state))?
        };
        t.editable = true;
        Ok(t)
    }

    /// Põe ou tira um jogador da escolha e mostra como fica (sem gravar).
    pub fn toggle_player(&self, sel: &Selection, player: i32) -> Result<ClubTactics> {
        let mut sel = sel.clone();
        self.world.toggle_player(self.club_id, &mut sel, player)?;
        self.preview_selection(&sel)
    }

    /// Grava a escolha do treinador (Ok na tela de Tácticas).
    pub fn set_selection(&mut self, sel: Selection) -> Result<ClubTactics> {
        let t = self.preview_selection(&sel)?;
        self.selection = Some(sel);
        self.unsaved = true;
        Ok(t)
    }

    /// Condição, moral, lesão e castigos de um jogador de um clube vivo.
    pub fn player_status(&self, id: i32) -> Option<PlayerStatus> {
        self.squads.status(id)
    }

    fn person_name(&self, id: i32) -> Result<String> {
        Ok(self.world.conn.query_row("SELECT display_name FROM people WHERE id = ?", [id], |r| r.get(0))?)
    }

    fn news_item(&self, id: i32, kind: &str, title: String, body: String) -> NewsItem {
        NewsItem { id, date: self.date.clone(), kind: kind.into(), title, body, read: false }
    }

    /// Lesões e suspensões dos jogadores do treinador.
    fn event_news(&self, id: i32, event: &Event) -> Result<Option<NewsItem>> {
        Ok(match event {
            Event::Injured { player, club, injury, days } if *club == self.club_id => {
                let name = self.person_name(*player)?;
                let back = self.squads.state(*player).injury_until.map(|d| cm_date(&d)).unwrap_or_default();
                Some(self.news_item(
                    id,
                    "injury",
                    format!("{name} lesionado"),
                    format!(
                        "{name} sofreu uma lesão no jogo de hoje: {}.\n\nO departamento médico prevê cerca de {days} dias de paragem (regresso a {back}).",
                        injury.to_lowercase()
                    ),
                ))
            }
            Event::Suspended { player, club, red } if *club == self.club_id => {
                let name = self.person_name(*player)?;
                let why = if *red { "foi expulso" } else { "viu o terceiro cartão amarelo" };
                Some(self.news_item(
                    id,
                    "injury",
                    format!("{name} suspenso"),
                    format!("{name} {why} e cumpre um jogo de suspensão."),
                ))
            }
            _ => None,
        })
    }

    /// O adjunto mexeu na equipa porque havia indisponíveis.
    fn changes_news(&self, id: i32, changes: &[(Option<i32>, i32)]) -> Result<NewsItem> {
        let mut lines = Vec::new();
        for &(out, inn) in changes {
            let inn = self.person_name(inn)?;
            lines.push(match out {
                Some(out) => {
                    let st = self.squads.state(out);
                    let why = if st.injury.is_some() { "lesionado" } else { "suspenso" };
                    format!("Sai {} ({why}), entra {inn}.", self.person_name(out)?)
                }
                None => format!("Entra {inn} para completar a equipa."),
            });
        }
        Ok(self.news_item(
            id,
            "message",
            "Alterações na equipa".into(),
            format!("O seu adjunto fez alterações na equipa para o jogo de hoje:\n\n{}", lines.join("\n")),
        ))
    }

    fn competition_name(&self, id: i32) -> &str {
        self.competitions.get(&id).map_or("Liga", String::as_str)
    }

    fn club(&self, id: i32) -> ClubRef {
        self.clubs.get(&id).cloned().unwrap_or_else(|| ClubRef {
            id,
            name: format!("clube {id}"),
            short_name: format!("clube {id}"),
            fg: None,
            bg: None,
        })
    }

    fn fixture(&self, m: &Match) -> Fixture {
        Fixture {
            id: m.id,
            competition_id: m.competition_id,
            competition: self.competition_name(m.competition_id).into(),
            round: m.round,
            date: m.date.clone(),
            home: self.club(m.home_id),
            away: self.club(m.away_id),
            home_goals: m.result.map(|r| r.0 as i32),
            away_goals: m.result.map(|r| r.1 as i32),
        }
    }

    /// Calendário de uma competição viva, por jornada (vazio nas outras).
    pub fn fixtures(&self, competition_id: i32) -> Vec<Fixture> {
        self.matches.iter().filter(|m| m.competition_id == competition_id).map(|m| self.fixture(m)).collect()
    }

    /// Partidas de um clube, por data.
    pub fn club_fixtures(&self, club_id: i32) -> Vec<Fixture> {
        let mut out: Vec<&Match> = self.matches.iter().filter(|m| m.home_id == club_id || m.away_id == club_id).collect();
        out.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
        out.into_iter().map(|m| self.fixture(m)).collect()
    }

    /// Classificação de uma competição viva (vazia nas outras).
    pub fn table(&self, competition_id: i32) -> Vec<StandingRow> {
        let matches: Vec<Match> = self.matches.iter().filter(|m| m.competition_id == competition_id).cloned().collect();
        if matches.is_empty() {
            return Vec::new();
        }
        let mut ids: Vec<i32> = matches.iter().flat_map(|m| [m.home_id, m.away_id]).collect();
        ids.sort_unstable();
        ids.dedup();
        let clubs: Vec<ClubRef> = ids.into_iter().map(|id| self.club(id)).collect();
        league::standings(&matches, &clubs)
    }

    /// Notícia do resultado do jogo do treinador, com a posição na tabela.
    fn result_news(&self, id: i32, m: &Match) -> NewsItem {
        let (hg, ag) = m.result.unwrap_or((0, 0));
        let (home, away) = (self.club(m.home_id), self.club(m.away_id));
        let at_home = m.home_id == self.club_id;
        let (ours, theirs, gf, ga) = if at_home { (&home, &away, hg, ag) } else { (&away, &home, ag, hg) };
        let place = if at_home { "em casa" } else { "fora" };
        let what = if gf > ga {
            format!("O {} venceu o {} {place} por {gf}-{ga}.", ours.short_name, theirs.short_name)
        } else if gf == ga {
            format!("O {} empatou {place} com o {}: {gf}-{ga}.", ours.short_name, theirs.short_name)
        } else {
            format!("O {} perdeu {place} com o {} por {gf}-{ga}.", ours.short_name, theirs.short_name)
        };
        let standing = self
            .table(m.competition_id)
            .iter()
            .find(|r| r.club.id == self.club_id)
            .map(|r| format!("\n\nCom este resultado, o {} é {}º classificado com {} pontos.", ours.short_name, r.position, r.points))
            .unwrap_or_default();
        NewsItem {
            id,
            date: self.date.clone(),
            kind: "competition".into(),
            title: format!("{} {hg}-{ag} {}", home.short_name, away.short_name),
            body: format!("{}, {}ª jornada.\n\n{what}{standing}", self.competition_name(m.competition_id), m.round),
            read: false,
        }
    }

    /// Notícia do campeão, com a posição final da equipa do treinador.
    fn champion_news(&self, id: i32, competition_id: i32) -> Result<NewsItem> {
        let table = self.table(competition_id);
        let first = table.first().ok_or_else(|| Error::NotFound(format!("competição {competition_id}")))?;
        let comp = self.competition_name(competition_id);
        let mut body =
            format!("O {} sagrou-se campeão da {comp} {} com {} pontos", first.club.short_name, self.world.start_year(), first.points);
        if let Some(second) = table.get(1) {
            body += &format!(", {} à frente do {}", first.points - second.points, second.club.short_name);
        }
        body += ".";
        if let Some(r) = table.iter().find(|r| r.club.id == self.club_id) {
            body += &format!("\n\nO {} terminou em {}º lugar, com {} pontos.", r.club.short_name, r.position, r.points);
        }
        Ok(NewsItem {
            id,
            date: self.date.clone(),
            kind: "competition".into(),
            title: format!("{} campeão da {comp}", first.club.short_name),
            body,
            read: false,
        })
    }

    /// Notícia do início da época com o primeiro jogo do treinador.
    fn season_news(&self, id: i32) -> Option<NewsItem> {
        let first = self.matches.first()?;
        let last = self.matches.iter().map(|m| m.date.as_str()).max()?;
        let comp = self.competition_name(first.competition_id);
        let mut body = format!("A {comp} começa a {} e termina a {}.", cm_date(&first.date), cm_date(last));
        if let Some(m) = self.matches.iter().find(|m| m.home_id == self.club_id || m.away_id == self.club_id) {
            let us = self.club(self.club_id).short_name;
            body += &if m.home_id == self.club_id {
                format!("\n\nNa 1ª jornada, o {us} recebe o {} a {}.", self.club(m.away_id).short_name, cm_date(&m.date))
            } else {
                format!("\n\nNa 1ª jornada, o {us} visita o {} a {}.", self.club(m.home_id).short_name, cm_date(&m.date))
            };
        }
        Some(NewsItem {
            id,
            date: self.date.clone(),
            kind: "competition".into(),
            title: format!("Calendário da {comp}"),
            body,
            read: false,
        })
    }

    pub fn info(&self) -> Result<GameInfo> {
        let manager_name: String =
            self.world.conn.query_row("SELECT display_name FROM people WHERE id = ?", [self.manager_id], |r| r.get(0))?;
        let club = self
            .world
            .club_ref(Some(self.club_id))?
            .ok_or_else(|| Error::NotFound(format!("clube {}", self.club_id)))?;
        Ok(GameInfo {
            name: self.name.clone(),
            date: self.date.clone(),
            manager_id: self.manager_id,
            manager_name,
            club,
            unread_news: self.news.iter().filter(|n| !n.read).count() as i32,
            unsaved: self.unsaved,
        })
    }

    /// Notícias, da mais recente para a mais antiga.
    pub fn news(&self) -> &[NewsItem] {
        &self.news
    }

    pub fn mark_news_read(&mut self, id: i32) {
        if let Some(n) = self.news.iter_mut().find(|n| n.id == id && !n.read) {
            n.read = true;
            self.unsaved = true;
        }
    }

    /// O mundo deste jogo (consultas da enciclopédia já veem o treinador no clube).
    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn date(&self) -> &str {
        &self.date
    }
}

/// Clubes de uma divisão, por id.
/// Instruções no save, pelas mesmas palavras do JSON da interface.
fn instruction_keys(i: &Instructions) -> (&'static str, &'static str, &'static str) {
    let m = match i.mentality {
        Mentality::Defensive => "defensive",
        Mentality::Normal => "normal",
        Mentality::Attacking => "attacking",
    };
    let p = match i.passing {
        Passing::Mixed => "mixed",
        Passing::Short => "short",
        Passing::Direct => "direct",
        Passing::Long => "long",
    };
    let t = match i.tackling {
        Tackling::Normal => "normal",
        Tackling::Soft => "soft",
        Tackling::Hard => "hard",
    };
    (m, p, t)
}

/// Lê a escolha do treinador gravada no save.
fn load_selection(world: &World, club_id: i32) -> Result<Option<Selection>> {
    let row = world
        .conn
        .query_row(
            "SELECT formation, auto, mentality, passing, tackling, pressing, offside_trap, counter_attack, tight_marking
             FROM selection WHERE club_id = ?",
            [club_id],
            |r| {
                Ok((
                    r.get::<_, i32>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    [r.get::<_, bool>(5)?, r.get(6)?, r.get(7)?, r.get(8)?],
                ))
            },
        )
        .optional()?;
    let Some((formation, auto, mentality, passing, tackling, flags)) = row else { return Ok(None) };
    let instructions = Instructions {
        mentality: match mentality.as_str() {
            "defensive" => Mentality::Defensive,
            "attacking" => Mentality::Attacking,
            _ => Mentality::Normal,
        },
        passing: match passing.as_str() {
            "short" => Passing::Short,
            "direct" => Passing::Direct,
            "long" => Passing::Long,
            _ => Passing::Mixed,
        },
        tackling: match tackling.as_str() {
            "soft" => Tackling::Soft,
            "hard" => Tackling::Hard,
            _ => Tackling::Normal,
        },
        pressing: flags[0],
        offside_trap: flags[1],
        counter_attack: flags[2],
        tight_marking: flags[3],
    };
    let mut starters = vec![None; 11];
    let mut positions = vec![[0, 0]; 11];
    let mut custom = 0;
    let mut bench = Vec::new();
    let mut runs = vec![None; 11];
    let mut stmt =
        world.conn.prepare("SELECT slot, person_id, x, y, run_x, run_y FROM selection_players WHERE club_id = ? ORDER BY slot")?;
    let rows = stmt.query_map([club_id], |r| {
        Ok((
            r.get::<_, i32>(0)?,
            r.get::<_, Option<i32>>(1)?,
            r.get::<_, Option<i32>>(2)?,
            r.get::<_, Option<i32>>(3)?,
            r.get::<_, Option<i32>>(4)?,
            r.get::<_, Option<i32>>(5)?,
        ))
    })?;
    for row in rows {
        let (slot, pid, x, y, rx, ry) = row?;
        match slot {
            0..=10 => {
                starters[slot as usize] = pid;
                runs[slot as usize] = rx.zip(ry).map(|(x, y)| [x, y]);
                if let (Some(x), Some(y)) = (x, y) {
                    positions[slot as usize] = [x, y];
                    custom += 1;
                }
            }
            100.. => bench.extend(pid),
            _ => {}
        }
    }
    let positions = if custom == 11 { positions } else { Vec::new() };
    Ok(Some(Selection { formation, starters, bench, instructions, auto, positions, runs }))
}

/// Save do formato 3 → 4: colunas x/y e os titulares reordenados para a ordem do CM
/// (antes ficavam pela ordem da formação).
fn migrate_v3(world: &World, club_id: i32) -> Result<()> {
    let tx = world.conn.unchecked_transaction()?;
    tx.execute_batch("ALTER TABLE selection_players ADD COLUMN x INTEGER; ALTER TABLE selection_players ADD COLUMN y INTEGER;")?;
    let formation: Option<i32> =
        tx.query_row("SELECT formation FROM selection WHERE club_id = ?", [club_id], |r| r.get(0)).optional()?;
    if let Some(formation) = formation {
        let old: HashMap<i32, Option<i32>> = tx
            .prepare("SELECT slot, person_id FROM selection_players WHERE club_id = ? AND slot <= 10")?
            .query_map([club_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        tx.execute("DELETE FROM selection_players WHERE club_id = ? AND slot <= 10", [club_id])?;
        for (k, from) in crate::tactics::cm_order_permutation(formation).into_iter().enumerate() {
            let pid = old.get(&(from as i32)).copied().flatten();
            tx.execute("INSERT INTO selection_players (club_id, slot, person_id) VALUES (?, ?, ?)", params![club_id, k as i32, pid])?;
        }
    }
    tx.execute("UPDATE game SET version = 4", [])?;
    tx.commit()?;
    Ok(())
}

/// Números de camisola únicos em cada clube vivo. O banco do patch repete números (o
/// Palmeiras tem dois 4 e dois 6) e deixa muitos jogadores sem número: quem tem mais CA
/// fica com o número, e os outros (e quem não tinha) recebem um número livre sorteado
/// com a seed do jogo (guarda-redes preferem 1, 12, 22, 23...).
fn fix_squad_numbers(world: &World, clubs: &[i32], seed: i64) -> Result<()> {
    // Uma transação só: centenas de UPDATEs soltos fariam um commit (e um fsync) cada
    let tx = world.conn.unchecked_transaction()?;
    let mut stmt = tx.prepare(
        "SELECT pl.person_id, pl.squad_number, pl.pos_goalkeeper FROM players pl JOIN people p ON p.id = pl.person_id
         WHERE p.club_id = ? ORDER BY pl.current_ability DESC, pl.person_id",
    )?;
    for &club in clubs {
        let rows: Vec<(i32, Option<i32>, Option<i32>)> =
            stmt.query_map([club], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut used = std::collections::HashSet::new();
        let mut missing = Vec::new();
        for (id, n, gk) in rows {
            match n.filter(|n| (1..=99).contains(n)) {
                Some(n) if used.insert(n) => {}
                _ => missing.push((id, crate::tactics::is_goalkeeper(gk))),
            }
        }
        let mut rng = ChaCha8Rng::seed_from_u64((seed as u64) ^ (club as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        for (id, gk) in missing {
            let preferred: Vec<i32> = if gk { vec![1, 12, 22, 23, 13, 31, 33] } else { Vec::new() };
            let free = match preferred.into_iter().find(|n| !used.contains(n)) {
                Some(n) => n,
                None => {
                    // Um número livre ao acaso entre 2 e 40 (senão até 99)
                    let pool: Vec<i32> = (2..=40).filter(|n| !used.contains(n)).collect();
                    let pool = if pool.is_empty() { (41..=99).filter(|n| !used.contains(n)).collect() } else { pool };
                    let Some(&n) = pool.get((rand_core::RngCore::next_u64(&mut rng) % pool.len().max(1) as u64) as usize) else { continue };
                    n
                }
            };
            used.insert(free);
            tx.execute("UPDATE players SET squad_number = ? WHERE person_id = ?", params![free, id])?;
        }
    }
    drop(stmt);
    tx.commit()?;
    Ok(())
}

fn league_club_ids(world: &World, division_id: i32) -> Result<Vec<i32>> {
    let ids = world
        .conn
        .prepare("SELECT id FROM clubs WHERE division_id = ? ORDER BY id")?
        .query_map([division_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

fn club_refs(world: &World, ids: &[i32]) -> Result<HashMap<i32, ClubRef>> {
    let mut out = HashMap::with_capacity(ids.len());
    for &id in ids {
        if let Some(c) = world.club_ref(Some(id))? {
            out.insert(id, c);
        }
    }
    Ok(out)
}

fn competition_names(world: &World, matches: &[Match]) -> Result<HashMap<i32, String>> {
    let mut out = HashMap::new();
    for m in matches {
        if !out.contains_key(&m.competition_id) {
            let (name, short): (String, String) = world.conn.query_row(
                "SELECT name, short_name FROM competitions WHERE id = ?",
                [m.competition_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            out.insert(m.competition_id, if short.is_empty() { name } else { short });
        }
    }
    Ok(out)
}

/// "2025-08-02" → "2.8.2025", como nas notícias do CM.
fn cm_date(iso: &str) -> String {
    let mut it = iso.split('-').map(|p| p.trim_start_matches('0'));
    let (y, m, d) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
    format!("{d}.{m}.{y}")
}

/// Mensagem de boas-vindas da direção, com a expectativa pela reputação do clube na divisão.
fn welcome_news(world: &World, club: &ClubRef, manager: &str, date: &str) -> Result<NewsItem> {
    let (rank, total, division): (i32, i32, Option<String>) = world.conn.query_row(
        "SELECT (SELECT COUNT(*) FROM clubs o WHERE o.division_id = c.division_id AND o.reputation > c.reputation) + 1,
                (SELECT COUNT(*) FROM clubs o WHERE o.division_id = c.division_id),
                d.name
         FROM clubs c LEFT JOIN competitions d ON d.id = c.division_id WHERE c.id = ?",
        [club.id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let goal = if total < 4 {
        "fazer uma boa época"
    } else if rank <= 3 {
        "lutar pelo título"
    } else if rank <= total / 2 {
        "terminar na primeira metade da tabela"
    } else if rank <= total - 4 {
        "garantir uma posição tranquila a meio da tabela"
    } else {
        "evitar a despromoção"
    };
    let league = division.map(|d| format!(" no {d}")).unwrap_or_default();
    Ok(NewsItem {
        id: 1,
        date: date.into(),
        kind: "message".into(),
        title: format!("Bem-vindo ao {}", club.short_name),
        body: format!(
            "A Direcção do {} dá as boas-vindas a {manager} como novo treinador.\n\n\
             Para esta época, a Direcção espera que a equipa consiga {goal}{league}.",
            club.name
        ),
        read: false,
    })
}

/// Apaga um jogo gravado de `dir` (só pelo nome do arquivo, nunca um caminho qualquer).
pub fn delete_save(dir: &Path, file: &str) -> Result<()> {
    let name = Path::new(file).file_name().ok_or_else(|| Error::Invalid("Arquivo inválido.".into()))?;
    let path = dir.join(name);
    if path.extension().and_then(|e| e.to_str()) != Some("sqlite") || !path.exists() {
        return Err(Error::Invalid("Esse jogo gravado não existe.".into()));
    }
    std::fs::remove_file(&path)?;
    Ok(())
}

/// Jogos gravados em `dir`, do mais recente para o mais antigo. Arquivos que não são
/// saves do NextCM são ignorados.
pub fn list_saves(dir: &Path) -> Result<Vec<SaveSummary>> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(out) };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sqlite") {
            continue;
        }
        let Ok(world) = World::open(&path) else { continue };
        let row = world.conn.query_row(
            "SELECT g.name, p.display_name, c.name, g.date FROM game g
             LEFT JOIN people p ON p.id = g.manager_id LEFT JOIN clubs c ON c.id = g.club_id",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, String>(3)?)),
        );
        let Ok((name, manager, club, date)) = row else { continue };
        let saved_at = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        out.push(SaveSummary {
            file: path.file_name().unwrap().to_string_lossy().into(),
            name,
            manager_name: manager.unwrap_or_default(),
            club_name: club.unwrap_or_default(),
            date,
            saved_at,
        });
    }
    out.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nextcm-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn new_game(club_id: i32) -> NewGame {
        NewGame {
            save_name: "Teste".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: Some("1990-05-10".into()),
            nation_id: None,
            club_id,
        }
    }

    #[test]
    fn file_stem_is_safe() {
        assert_eq!(file_stem(" Minha época/1 ").unwrap(), "Minha época_1");
        assert!(file_stem("   ").is_err());
    }

    #[test]
    fn create_advance_save_and_load() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("game");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 42).unwrap();

        let info = g.info().unwrap();
        assert_eq!(info.date, "2025-07-01");
        assert_eq!(info.manager_name, "Victor Araujo");
        assert_eq!(info.club.short_name, "Palmeiras");
        // Boas-vindas e calendário da Série A
        assert_eq!(info.unread_news, 2);
        assert_eq!(g.news()[0].title, "Calendário da Série A");
        assert!(g.news()[1].body.contains("Victor Araujo"));

        // O treinador está no plantel técnico e o Abel Ferreira saiu
        let club = g.world().get_club(palmeiras).unwrap();
        let coaches: Vec<_> = club.staff.iter().filter(|s| s.job_id == Some(HEAD_COACH)).map(|s| s.list_name.clone()).collect();
        assert_eq!(coaches, ["Araujo, Victor"]);

        // Nome repetido é recusado
        assert!(matches!(Game::create(&world, &dir, &new_game(palmeiras), 1), Err(Error::Invalid(_))));

        for _ in 0..62 {
            g.advance_day().unwrap();
        }
        assert_eq!(g.date(), "2025-09-01");
        // Agosto tem 5 jornadas (2, 9, 16, 23 e 30): uma notícia por jogo do Palmeiras
        let mine = g.club_fixtures(palmeiras);
        assert_eq!(mine.len(), 38);
        assert_eq!(mine.iter().filter(|f| f.home_goals.is_some()).count(), 5);
        // 2 iniciais + 5 resultados (+ lesões/suspensões do Palmeiras, que variam com a seed)
        let results = g.news().iter().filter(|n| n.kind == "competition" && n.title.contains('-')).count();
        assert_eq!(results, 5);
        assert!(g.info().unwrap().unread_news >= 7);
        let ids: Vec<i32> = g.news().iter().map(|n| n.id).collect();
        for id in ids {
            g.mark_news_read(id);
        }
        assert!(g.info().unwrap().unsaved);
        let table = g.table(SERIE_A);
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);

        let g = Game::load(&path).unwrap();
        let info = g.info().unwrap();
        assert_eq!(info.date, "2025-09-01");
        assert_eq!(info.unread_news, 0);
        assert!(!info.unsaved);
        // Resultados e classificação voltam iguais do save
        let reloaded = g.table(SERIE_A);
        assert_eq!(reloaded.len(), 20);
        let key = |t: &[StandingRow]| t.iter().map(|r| (r.club.id, r.points, r.goals_for)).collect::<Vec<_>>();
        assert_eq!(key(&table), key(&reloaded));

        let saves = list_saves(&dir).unwrap();
        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].club_name, "Sociedade Esportiva Palmeiras");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Joga a Série A inteira; devolve (resultados por id da partida, classificação).
    fn full_season(world: &Path, name: &str, seed: i64) -> (Vec<(i32, Option<i32>, Option<i32>)>, Vec<StandingRow>) {
        let dir = temp_dir(name);
        let mut g = Game::create(world, &dir, &new_game(7568), seed).unwrap();
        // Continuar Jogo pára na véspera do 1º jogo do Palmeiras e, a seguir, depois dele
        let first = g.club_fixtures(7568)[0].clone();
        assert!(g.advance_to_next_event().unwrap().is_empty());
        assert_eq!(date::add_days(g.date(), 1).unwrap(), first.date);
        let news = g.advance_to_next_event().unwrap();
        assert_eq!(g.date(), first.date);
        assert_eq!(news.len(), 1);
        assert!(news[0].body.starts_with("Série A, 1ª jornada."), "{}", news[0].body);
        let mut days = 0;
        while g.fixtures(SERIE_A).iter().any(|f| f.home_goals.is_none()) {
            g.advance_to_next_event().unwrap();
            days += 1;
            assert!(days < 400, "a época não acaba");
        }
        let results = g.fixtures(SERIE_A).iter().map(|f| (f.id, f.home_goals, f.away_goals)).collect();
        let table = g.table(SERIE_A);
        assert!(g.news()[0].title.ends_with("campeão da Série A"), "{}", g.news()[0].title);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
        (results, table)
    }

    #[test]
    fn full_season_is_deterministic_and_coherent() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let (results, table) = full_season(&world, "season-a", 99);
        let (again, _) = full_season(&world, "season-b", 99);
        assert_eq!(results, again, "mesma seed, mesmos resultados");

        assert_eq!(results.len(), 380);
        assert_eq!(table.len(), 20);
        assert!(table.iter().all(|r| r.played == 38 && r.won + r.drawn + r.lost == 38));
        // Pontos: 3 por jogo com vencedor, 2 por empate
        let draws = results.iter().filter(|r| r.1 == r.2).count() as i32;
        let points: i32 = table.iter().map(|r| r.points).sum();
        assert_eq!(points, 3 * (380 - draws) + 2 * draws);
        let goals: i32 = results.iter().map(|r| r.1.unwrap() + r.2.unwrap()).sum();
        assert_eq!(goals, table.iter().map(|r| r.goals_for).sum::<i32>());
        assert_eq!(goals, table.iter().map(|r| r.goals_against).sum::<i32>());
        // Números plausíveis: 2,0–3,2 golos por jogo e a tabela ordenada por pontos
        let per_game = goals as f64 / 380.0;
        assert!((2.0..3.2).contains(&per_game), "{per_game:.2} golos por jogo");
        assert!(table.windows(2).all(|w| w[0].points >= w[1].points));
    }

    #[test]
    fn selection_is_saved_and_changes_strength() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("selection");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 7).unwrap();

        // O adjunto escolhe; a tela recebe a táctica editável
        let auto = g.club_tactics(palmeiras).unwrap();
        assert!(auto.editable && auto.auto);
        assert!(!g.club_tactics(3675).unwrap().editable, "Flamengo não é do treinador");

        // Onze de reservas: troca os 11 titulares pelos 11 piores do plantel
        let mut sel = auto.selection();
        sel.auto = false;
        let reserves: Vec<i32> = auto.reserves.iter().rev().map(|p| p.id).collect();
        let mut pool: Vec<i32> = reserves.into_iter().chain(auto.bench.iter().rev().map(|p| p.id)).collect();
        pool.truncate(11);
        sel.starters = pool.iter().map(|&id| Some(id)).collect();
        sel.bench.retain(|id| !pool.contains(id));
        sel.instructions.mentality = Mentality::Attacking;
        let weak = g.set_selection(sel.clone()).unwrap();
        assert!(!weak.auto);
        let (a, w) = (TeamStrength::from_tactics(&auto), TeamStrength::from_tactics(&weak));
        assert!(w.attack + w.midfield + w.defence < 0.9 * (a.attack + a.midfield + a.defence), "{w:?} vs {a:?}");

        // Escolha inválida é recusada
        let mut bad = sel.clone();
        bad.starters[1] = bad.starters[0];
        assert!(matches!(g.set_selection(bad), Err(Error::Invalid(_))));

        // Gravar e carregar mantém a escolha e as instruções
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&path).unwrap();
        assert_eq!(g.selection.as_ref(), Some(&sel));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn assistant_covers_injured_starter() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("cover");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 9).unwrap();
        let mut sel = g.club_tactics(palmeiras).unwrap().selection();
        sel.auto = false;
        g.set_selection(sel.clone()).unwrap();

        // O guarda-redes titular lesiona-se antes do jogo
        let gk = sel.starters[0].unwrap();
        let st = g.squads.states.entry(gk).or_default();
        st.injury = Some("Entorse no tornozelo".into());
        st.injury_until = Some("2025-12-31".into());
        let (t, changes) = g.lineup(palmeiras).unwrap();
        let changes = changes.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, Some(gk));
        let new_gk = t.starters[0].player.as_ref().unwrap();
        assert_ne!(new_gk.id, gk);
        assert!(new_gk.fit.unwrap() >= 15, "entra outro GR");
        // A escolha gravada não muda: o titular volta quando estiver bom
        assert_eq!(g.selection.as_ref().unwrap().starters[0], Some(gk));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn starters_get_tired_and_keep_their_place() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("tired");
        let botafogo = g_club(&world, "Botafogo");
        let mut g = Game::create(&world, &dir, &new_game(botafogo), 11).unwrap();
        let ids = |t: &ClubTactics| t.starters.iter().filter_map(|s| s.player.as_ref().map(|p| p.id)).collect::<Vec<_>>();
        let before = ids(&g.club_tactics(botafogo).unwrap());
        // Até ao dia do 1º jogo do Botafogo
        while g.club_fixtures(botafogo).iter().all(|f| f.home_goals.is_none()) {
            g.advance_day().unwrap();
        }
        let after = g.club_tactics(botafogo).unwrap();
        // Quem jogou (sem lesão/suspensão) cansou; os suplentes não
        for s in &after.starters {
            let p = s.player.as_ref().unwrap();
            if before.contains(&p.id) && !p.injured && !p.suspended {
                assert!(p.condition.unwrap() < 85, "{} jogou e está a {:?}", p.list_name, p.condition);
            }
        }
        for p in &after.bench {
            assert_eq!(p.condition, Some(100), "{} não jogou", p.list_name);
        }
        // O adjunto mantém o onze (só troca lesionados/suspensos)
        let kept = ids(&after).iter().filter(|id| before.contains(id)).count();
        assert!(kept >= 9, "o onze mudou demais: {kept} de 11");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn g_club(world: &Path, short: &str) -> i32 {
        let w = World::open(world).unwrap();
        w.conn.query_row("SELECT id FROM clubs WHERE short_name = ? AND division_id = 65", [short], |r| r.get(0)).unwrap()
    }

    #[test]
    fn toggle_and_unique_numbers() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("toggle");
        let palmeiras = 7568;
        let g = Game::create(&world, &dir, &new_game(palmeiras), 3).unwrap();

        // Números de camisola sem repetidos no plantel
        let club = g.world().get_club(palmeiras).unwrap();
        assert!(club.players.iter().all(|p| p.squad_number.is_some()), "todos com número");
        let mut numbers: Vec<i32> = club.players.iter().filter_map(|p| p.squad_number).collect();
        let total = numbers.len();
        numbers.sort_unstable();
        numbers.dedup();
        assert_eq!(numbers.len(), total, "números repetidos");

        // Tirar o GR deixa a função vazia; pôr de volta um GR reserva ocupa essa função
        let t = g.club_tactics(palmeiras).unwrap();
        let sel = t.selection();
        let gk = sel.starters[0].unwrap();
        let out = g.toggle_player(&sel, gk).unwrap();
        assert!(out.starters[0].player.is_none() && !out.auto);
        let reserve_gk = t.reserves.iter().find(|p| p.position == "GR").unwrap().id;
        let back = g.toggle_player(&out.selection(), reserve_gk).unwrap();
        assert_eq!(back.starters[0].player.as_ref().unwrap().id, reserve_gk);
        // Com o onze completo, quem entra vai para o banco (se houver lugar)
        let mut full = back.selection();
        full.bench.truncate(6);
        let outsider = back.reserves.iter().find(|p| !full.bench.contains(&p.id)).unwrap().id;
        let benched = g.toggle_player(&full, outsider).unwrap();
        assert!(benched.bench.iter().any(|p| p.id == outsider));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn free_positions_change_roles_and_order() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("positions");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 5).unwrap();
        let t = g.club_tactics(palmeiras).unwrap();
        // 4-4-2 na ordem do CM: o último é um avançado (A C)
        assert_eq!(t.starters[10].role, "A C");
        let striker = t.starters[10].player.as_ref().unwrap().id;
        // Arrastar o avançado para o meio-campo: vira médio e a lista reordena
        let mut sel = t.selection();
        sel.positions = t.starters.iter().map(|s| [s.x, s.y]).collect();
        sel.positions[10] = [50, 45];
        sel.auto = false;
        let moved = g.set_selection(sel).unwrap();
        assert!(moved.custom);
        let at = moved.starters.iter().position(|s| s.player.as_ref().is_some_and(|p| p.id == striker)).unwrap();
        assert_eq!(moved.starters[at].role, "M C");
        let roles: Vec<&str> = moved.starters.iter().map(|s| s.role.as_str()).collect();
        assert_eq!(roles.iter().filter(|r| **r == "A C").count(), 1);
        assert!(at < 10, "a lista segue a ordem das funções");
        // Gravar e carregar mantém as posições
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&path).unwrap();
        let again = g.club_tactics(palmeiras).unwrap();
        assert!(again.custom);
        assert_eq!(again.starters[at].role, "M C");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn runs_are_saved_and_count_in_the_engine() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("runs");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 4).unwrap();
        let t = g.club_tactics(palmeiras).unwrap();
        let mut sel = t.selection();
        sel.auto = false;
        // Os dois laterais (2 e 3 na ordem do CM) sobem pelo corredor
        sel.runs = vec![None; 11];
        for i in [1, 2] {
            sel.runs[i] = Some([t.starters[i].x, t.starters[i].y - 30]);
        }
        let with = g.set_selection(sel.clone()).unwrap();
        assert_eq!(with.starters[1].run, sel.runs[1]);
        let (a, b) = (TeamStrength::from_tactics(&t), TeamStrength::from_tactics(&with));
        assert!(b.attack > a.attack && b.defence < a.defence);
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&path).unwrap();
        assert_eq!(g.club_tactics(palmeiras).unwrap().starters[2].run, sel.runs[2]);
        drop(g);
        assert!(delete_save(&dir, "../world.sqlite").is_err());
        delete_save(&dir, path.file_name().unwrap().to_str().unwrap()).unwrap();
        assert!(list_saves(&dir).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_3_saves_are_migrated() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("migrate");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 8).unwrap();
        let mut sel = g.club_tactics(palmeiras).unwrap().selection();
        sel.auto = false;
        let expected = g.set_selection(sel.clone()).unwrap();
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);

        // Volta o arquivo ao formato 3: sem x/y e titulares na ordem antiga da formação
        {
            let w = World::open_save(&path).unwrap();
            let perm = crate::tactics::cm_order_permutation(sel.formation);
            let mut old = vec![None; 11];
            for (k, &from) in perm.iter().enumerate() {
                old[from] = sel.starters[k];
            }
            w.conn.execute("DELETE FROM selection_players WHERE slot <= 10", []).unwrap();
            w.conn
                .execute_batch("ALTER TABLE selection_players DROP COLUMN x; ALTER TABLE selection_players DROP COLUMN y; ALTER TABLE selection_players DROP COLUMN run_x; ALTER TABLE selection_players DROP COLUMN run_y; UPDATE game SET version = 3;")
                .unwrap();
            for (slot, pid) in old.iter().enumerate() {
                w.conn
                    .execute("INSERT INTO selection_players (club_id, slot, person_id) VALUES (?, ?, ?)", params![palmeiras, slot as i32, pid])
                    .unwrap();
            }
        }

        let g = Game::load(&path).unwrap();
        let t = g.club_tactics(palmeiras).unwrap();
        let ids = |t: &ClubTactics| t.starters.iter().map(|s| (s.role.clone(), s.player.as_ref().map(|p| p.id))).collect::<Vec<_>>();
        assert_eq!(ids(&t), ids(&expected), "cada jogador fica na sua função");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn world_file_is_not_a_save() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        // Abrir o mundo original como jogo tem de falhar sem estragar nada
        let dir = temp_dir("notsave");
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join("world.sqlite");
        std::fs::copy(&world, &copy).unwrap();
        assert!(matches!(Game::load(&copy), Err(Error::Invalid(_))));
        assert!(list_saves(&dir).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
