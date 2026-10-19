//! Um jogo em curso (Fase 3): o save é uma cópia do `world.sqlite` com as tabelas
//! `game`, `news`, `fixtures`, `player_state`, `selection` e `selection_players`. O estado
//! que muda dia a dia fica em memória e só vai para o arquivo em [`Game::save`].
//!
//! Num dia com jogo do treinador (e com `watch` ligado, como no app), [`Game::advance_day`]
//! pára antes de o jogar: os outros jogos do dia já estão decididos, e o do treinador corre
//! aos bocados pela tela da partida (`live_*`) até [`Game::live_finish`] fechar o dia.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::engine;
use crate::live::{LiveMatch, LiveView, TeamStats};
use crate::squad::{Event, PlayerState, PlayerStatus, Squads};
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
    /// O jogo do treinador de hoje está por acabar (tela da partida).
    pub live: bool,
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

/// "Próximo Jogo" do CM: onde, quando, o último confronto, o tempo e as notícias de cada lado.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MatchPreview {
    pub fixture: Fixture,
    /// Estádio e cidade do clube da casa.
    pub venue: Option<String>,
    pub city: Option<String>,
    /// Último jogo entre os dois nesta save (o mais recente).
    pub last_meeting: Option<Fixture>,
    /// Ex.: "Seco, 24°C" (sorteado com a seed e a partida, igual sempre que se abre).
    pub weather: String,
    /// "Normais", "Molhado"...
    pub pitch: String,
    /// Lesionados e suspensos de cada lado.
    pub home_news: Vec<String>,
    pub away_news: Vec<String>,
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
    /// O treinador vê os jogos dele ao vivo (o app); sem isso (CLI, testes) jogam-se de seguida.
    watch: bool,
    /// O dia de jogo à espera de o treinador acabar a partida dele.
    live: Option<LiveDay>,
    /// Estatísticas dos jogos desde que o jogo foi aberto (para a CLI).
    records: Vec<MatchRecord>,
}

/// Os jogos de um dia: os outros já jogados até ao fim, o do treinador a decorrer.
struct LiveDay {
    /// (índice em `matches`, partida)
    games: Vec<(usize, LiveMatch)>,
    /// Posição do jogo do treinador em `games`.
    user: usize,
    /// Notícias do início do dia (recuperações, alterações do adjunto).
    news: Vec<NewsItem>,
    /// Estádio, tempo, árbitro e assistência do jogo do treinador.
    extras: Option<MatchExtras>,
}

#[derive(Debug, Clone)]
struct MatchExtras {
    venue: Option<String>,
    city: Option<String>,
    weather: String,
    referee: String,
    attendance: i32,
}

/// Números de um jogo acabado.
#[derive(Debug, Clone)]
pub struct MatchRecord {
    pub fixture_id: i32,
    pub stats: [TeamStats; 2],
    pub subs: [i32; 2],
    pub injuries: usize,
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
                watch: false,
                live: None,
                records: Vec::new(),
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
            watch: false,
            live: None,
            records: Vec::new(),
        })
    }

    /// Grava o estado em memória no arquivo do save.
    pub fn save(&mut self) -> Result<()> {
        if self.live.is_some() {
            return Err(Error::Invalid("Não é possível gravar durante um jogo.".into()));
        }
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

    /// Avança um dia e joga as partidas desse dia; devolve as notícias novas. Com `watch`
    /// e um jogo do treinador hoje, pára antes dele (ver [`Game::live_view`]); chamar de
    /// novo nesse estado acaba o jogo e fecha o dia, sem mudar a data.
    pub fn advance_day(&mut self) -> Result<Vec<NewsItem>> {
        if self.live.is_some() {
            return self.live_finish();
        }
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
        let mut games = Vec::new();
        let mut user = None;
        for &i in &today {
            let (home, away) = (self.matches[i].home_id, self.matches[i].away_id);
            let (home_t, home_changes) = self.lineup(home)?;
            let (away_t, away_changes) = self.lineup(away)?;
            if let Some(changes) = home_changes.or(away_changes) {
                if !changes.is_empty() {
                    fresh.push(self.changes_news(next_id, &changes)?);
                    next_id += 1;
                }
            }
            let side = if home == self.club_id {
                Some(0)
            } else if away == self.club_id {
                Some(1)
            } else {
                None
            };
            let user_ai = self.selection.as_ref().is_none_or(|s| s.auto);
            let rng = engine::match_rng(self.seed, self.matches[i].id);
            let mut live = LiveMatch::new(&self.world, self.fixture(&self.matches[i]), home_t, away_t, side, user_ai, rng)?;
            if side.is_some() && self.watch {
                user = Some(games.len());
                live = live.detailed(engine::match_rng(self.seed ^ 0x0F1A_7E55, self.matches[i].id));
            } else {
                live.play_until(90);
            }
            games.push((i, live));
        }
        let user_match = user.is_some();
        let extras = match user {
            Some(u) => Some(self.match_extras(games[u].0)?),
            None => None,
        };
        self.live = Some(LiveDay { games, user: user.unwrap_or(usize::MAX), news: fresh, extras });
        if user_match {
            return Ok(Vec::new());
        }
        self.live_finish()
    }

    /// Acaba os jogos do dia (o do treinador até ao fim, se ainda corre), guarda os
    /// resultados e o que deixaram nos jogadores, e entrega as notícias do dia.
    pub fn live_finish(&mut self) -> Result<Vec<NewsItem>> {
        let Some(mut day) = self.live.take() else { return Ok(Vec::new()) };
        let mut fresh = std::mem::take(&mut day.news);
        let mut next_id = self.news.iter().chain(&fresh).map(|n| n.id).max().unwrap_or(0) + 1;
        let date = self.date.clone();
        for (i, live) in &mut day.games {
            live.play_until(90);
            self.matches[*i].result = Some(live.score());
            let sides = live.sides();
            let injuries = sides[0].injured.len() + sides[1].injured.len();
            self.records.push(MatchRecord { fixture_id: live.fixture.id, stats: live.stats(), subs: live.subs_made(), injuries });
            // As durações das lesões continuam o sorteio da partida
            let mut rng = engine::match_rng(self.seed ^ 0x1E5A_0C0D, live.fixture.id);
            for event in self.squads.after_match(sides, &date, &mut rng) {
                if let Some(n) = self.event_news(next_id, &event)? {
                    fresh.push(n);
                    next_id += 1;
                }
            }
        }
        for (i, live) in &day.games {
            let m = &self.matches[*i];
            if m.home_id == self.club_id || m.away_id == self.club_id {
                fresh.push(self.result_news(next_id, m, live));
                next_id += 1;
            }
        }
        // Competições que acabaram hoje: o campeão
        let mut finished: Vec<i32> = day.games.iter().map(|(i, _)| self.matches[*i].competition_id).collect();
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
        self.unsaved = true;
        Ok(fresh)
    }

    /// "Continuar Jogo": avança dia a dia até haver notícias (o jogo do treinador, o fim
    /// de uma competição...), até à véspera de um jogo do treinador (para ver o Próximo
    /// Jogo antes de o jogar) ou até [`MAX_IDLE_DAYS`] dias sem nada.
    pub fn advance_to_next_event(&mut self) -> Result<Vec<NewsItem>> {
        for _ in 0..MAX_IDLE_DAYS {
            let news = self.advance_day()?;
            if !news.is_empty() || self.live.is_some() {
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

    /// Ficha do "Próximo Jogo" para uma partida.
    pub fn match_preview(&self, fixture_id: i32) -> Result<MatchPreview> {
        let m = self
            .matches
            .iter()
            .find(|m| m.id == fixture_id)
            .ok_or_else(|| Error::NotFound(format!("partida {fixture_id}")))?;
        let (venue, city): (Option<String>, Option<String>) = self.world.conn.query_row(
            "SELECT s.name, ci.name FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id
             LEFT JOIN cities ci ON ci.id = s.city_id WHERE c.id = ?",
            [m.home_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let last_meeting = self
            .matches
            .iter()
            .filter(|o| {
                o.result.is_some()
                    && o.date < m.date
                    && ((o.home_id == m.home_id && o.away_id == m.away_id) || (o.home_id == m.away_id && o.away_id == m.home_id))
            })
            .max_by(|a, b| a.date.cmp(&b.date))
            .map(|o| self.fixture(o));

        // Tempo: estação do ano no Brasil e um sorteio fixo por partida
        let mut rng = engine::match_rng(self.seed ^ 0x7E47_4E52, m.id);
        let roll = |rng: &mut ChaCha8Rng| engine::uniform(rng);
        let month: u32 = m.date.get(5..7).and_then(|v| v.parse().ok()).unwrap_or(7);
        let (min, max) = match month {
            12 | 1 | 2 => (24, 34),
            3..=5 => (20, 30),
            6..=8 => (14, 26),
            _ => (18, 29),
        };
        let temp = min + (roll(&mut rng) * (max - min + 1) as f64) as i32;
        let sky = roll(&mut rng);
        let (weather, pitch) = if sky < 0.55 {
            ("Seco", "Normais")
        } else if sky < 0.78 {
            ("Nublado", "Normais")
        } else if sky < 0.95 {
            ("Chuva", "Molhado")
        } else {
            ("Chuva forte", "Encharcado")
        };

        let news = |club: i32| -> Result<Vec<String>> {
            let mut out = Vec::new();
            let mut ids: Vec<i32> =
                self.squads.traits.iter().filter(|(_, t)| t.club_id == club).map(|(&id, _)| id).collect();
            ids.sort_unstable();
            for id in ids {
                let st = self.squads.state(id);
                if let Some(injury) = &st.injury {
                    let back = st.injury_until.as_deref().map(cm_date).unwrap_or_default();
                    out.push(format!("{} lesionado ({}, regresso a {back})", self.person_name(id)?, injury.to_lowercase()));
                } else if st.suspended > 0 {
                    out.push(format!("{} suspenso", self.person_name(id)?));
                }
            }
            Ok(out)
        };
        Ok(MatchPreview {
            fixture: self.fixture(m),
            venue,
            city,
            last_meeting,
            weather: format!("{weather}, {temp}°C"),
            pitch: pitch.into(),
            home_news: news(m.home_id)?,
            away_news: news(m.away_id)?,
        })
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

    /// Notícia do resultado do jogo do treinador, com a posição na tabela e os marcadores.
    fn result_news(&self, id: i32, m: &Match, live: &LiveMatch) -> NewsItem {
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
        let mut scorers = String::new();
        for (side, club) in [(0, &home), (1, &away)] {
            let goals = live.scorers(side);
            if !goals.is_empty() {
                let list: Vec<String> = goals.iter().map(|(n, min)| format!("{n} {min}'")).collect();
                scorers += &format!("\n{}: {}", club.short_name, list.join(", "));
            }
        }
        if !scorers.is_empty() {
            scorers = format!("\n\nMarcadores:{scorers}");
        }
        NewsItem {
            id,
            date: self.date.clone(),
            kind: "competition".into(),
            title: format!("{} {hg}-{ag} {}", home.short_name, away.short_name),
            body: format!("{}, {}ª jornada.\n\n{what}{scorers}{standing}", self.competition_name(m.competition_id), m.round),
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
            live: self.live.is_some(),
        })
    }

    /// Liga/desliga ver os jogos do treinador ao vivo (o app liga; a CLI não).
    pub fn set_watch(&mut self, watch: bool) {
        self.watch = watch;
    }

    /// Números dos jogos acabados desde que o jogo foi aberto.
    pub fn records(&self) -> &[MatchRecord] {
        &self.records
    }

    fn live_day(&self) -> Result<&LiveDay> {
        self.live.as_ref().filter(|d| d.user < d.games.len()).ok_or_else(|| Error::Invalid("Não há nenhum jogo a decorrer.".into()))
    }

    fn live_match_mut(&mut self) -> Result<&mut LiveMatch> {
        self.live_day()?;
        let day = self.live.as_mut().expect("dia de jogo");
        Ok(&mut day.games[day.user].1)
    }

    /// O jogo do treinador como está agora, com os outros jogos do dia no mesmo minuto.
    pub fn live_view(&self) -> Result<LiveView> {
        let day = self.live_day()?;
        let others: Vec<&LiveMatch> = day.games.iter().enumerate().filter(|(k, _)| *k != day.user).map(|(_, g)| &g.1).collect();
        let (index, live) = &day.games[day.user];
        let mut v = live.view(&others);
        if let Some(x) = &day.extras {
            v.venue = x.venue.clone();
            v.city = x.city.clone();
            v.weather = x.weather.clone();
            v.referee = x.referee.clone();
            v.attendance = x.attendance;
        }
        // Classificação com os jogos do dia no minuto em que vai o do treinador
        let comp = self.matches[*index].competition_id;
        let mut matches: Vec<Match> = self.matches.iter().filter(|m| m.competition_id == comp).cloned().collect();
        if v.minute > 0 {
            for (i, g) in &day.games {
                let (h, a) = g.score_at(v.minute as u8);
                if let Some(m) = matches.iter_mut().find(|m| m.id == self.matches[*i].id) {
                    m.result = Some((h as u8, a as u8));
                }
            }
        }
        let mut ids: Vec<i32> = matches.iter().flat_map(|m| [m.home_id, m.away_id]).collect();
        ids.sort_unstable();
        ids.dedup();
        let clubs: Vec<ClubRef> = ids.into_iter().map(|id| self.club(id)).collect();
        v.table = league::standings(&matches, &clubs);
        Ok(v)
    }

    /// Estádio e tempo (os do Próximo Jogo), árbitro e assistência de uma partida.
    fn match_extras(&self, index: usize) -> Result<MatchExtras> {
        let m = &self.matches[index];
        let preview = self.match_preview(m.id)?;
        let mut rng = engine::match_rng(self.seed ^ 0x00A7_7E4D, m.id);
        // Árbitro: um nome e um apelido de pessoas da nação do clube da casa (um nome inventado)
        let nation: Option<i32> = self.world.conn.query_row("SELECT nation_id FROM clubs WHERE id = ?", [m.home_id], |r| r.get(0))?;
        let count: i64 = self.world.conn.query_row(
            "SELECT COUNT(*) FROM people WHERE nation_id IS ? AND first_name <> '' AND second_name <> ''",
            [nation],
            |r| r.get(0),
        )?;
        let mut name_at = |col: &str| -> Result<String> {
            let offset = (rand_core::RngCore::next_u64(&mut rng) % count.max(1) as u64) as i64;
            Ok(self.world.conn.query_row(
                &format!("SELECT {col} FROM people WHERE nation_id IS ? AND first_name <> '' AND second_name <> '' LIMIT 1 OFFSET ?"),
                params![nation, offset],
                |r| r.get::<_, String>(0),
            )?)
        };
        let referee = if count > 0 {
            let (first, last) = (name_at("first_name")?, name_at("second_name")?);
            let first = first.split_whitespace().next().unwrap_or_default().to_string();
            format!("{first} {}", last.split_whitespace().last().unwrap_or_default())
        } else {
            "-".into()
        };
        // Assistência: 55% a 98% da lotação
        let capacity: Option<i32> = self.world.conn.query_row(
            "SELECT s.capacity FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id WHERE c.id = ?",
            [m.home_id],
            |r| r.get(0),
        )?;
        let fill = 0.55 + 0.43 * engine::uniform(&mut rng);
        let attendance = (capacity.unwrap_or(15000).max(1000) as f64 * fill).round() as i32;
        Ok(MatchExtras { venue: preview.venue, city: preview.city, weather: preview.weather, referee, attendance })
    }

    /// Joga até ao minuto `minute` (no máximo 90).
    pub fn live_step(&mut self, minute: i32) -> Result<LiveView> {
        self.live_match_mut()?.play_until(minute.clamp(0, 90) as u8);
        self.live_view()
    }

    /// Substituição na equipa do treinador.
    pub fn live_substitute(&mut self, out: i32, inn: i32) -> Result<LiveView> {
        self.live_match_mut()?.substitute(out, inn)?;
        self.live_view()
    }

    /// Troca as funções de dois jogadores do treinador em campo.
    pub fn live_swap(&mut self, a: i32, b: i32) -> Result<LiveView> {
        self.live_match_mut()?.swap(a, b)?;
        self.live_view()
    }

    /// As Tácticas de um lado do jogo do treinador (0 casa, 1 fora), como a tela as mostra.
    pub fn live_tactics(&self, side: i32) -> Result<ClubTactics> {
        let day = self.live_day()?;
        day.games[day.user].1.tactics(&self.world, side.clamp(0, 1) as usize, None)
    }

    /// Como ficaria a equipa do treinador com esta escolha (sem mexer no jogo).
    pub fn live_preview_tactics(&self, sel: &Selection) -> Result<ClubTactics> {
        let day = self.live_day()?;
        let live = &day.games[day.user].1;
        let side = live.user_side().ok_or_else(|| Error::Invalid("Este jogo não é da sua equipa.".into()))?;
        live.tactics(&self.world, side, Some(sel))
    }

    /// O Ok das Tácticas a meio do jogo: aplica a escolha toda de uma vez.
    pub fn live_set_tactics(&mut self, sel: &Selection) -> Result<LiveView> {
        self.live_day()?;
        let day = self.live.as_mut().expect("dia de jogo");
        day.games[day.user].1.apply_selection(&self.world, sel)?;
        self.live_view()
    }

    /// Instruções à equipa a meio do jogo.
    pub fn live_instructions(&mut self, instructions: Instructions) -> Result<LiveView> {
        self.live_match_mut()?.set_instructions(instructions)?;
        self.live_view()
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
    use crate::engine::TeamStrength;

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
                // Jogou pelo menos uma parte (o adjunto pode tê-lo trocado ao intervalo)
                assert!(p.condition.unwrap() < 92, "{} jogou e está a {:?}", p.list_name, p.condition);
            }
        }
        // Os suplentes não jogaram ou entraram já na segunda parte
        for p in &after.bench {
            assert!(p.condition >= Some(85), "{} está a {:?}", p.list_name, p.condition);
        }
        // O adjunto mantém o onze (só troca lesionados/suspensos)
        let kept = ids(&after).iter().filter(|id| before.contains(id)).count();
        assert!(kept >= 9, "o onze mudou demais: {kept} de 11");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Avança até o jogo do Palmeiras começar (com `watch`) e devolve o jogo.
    /// `manual`: o treinador escolhe a equipa (o adjunto não faz substituições tácticas).
    fn until_kickoff(world: &Path, name: &str, seed: i64, watch: bool, manual: bool) -> (Game, PathBuf) {
        let dir = temp_dir(name);
        let mut g = Game::create(world, &dir, &new_game(7568), seed).unwrap();
        g.set_watch(watch);
        if manual {
            let mut sel = g.club_tactics(7568).unwrap().selection();
            sel.auto = false;
            g.set_selection(sel).unwrap();
        }
        let first = g.club_fixtures(7568)[0].date.clone();
        while g.date() < first.as_str() {
            g.advance_to_next_event().unwrap();
        }
        (g, dir)
    }

    #[test]
    fn watched_match_is_the_same_match() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let (mut quick, d1) = until_kickoff(&world, "live-quick", 5, false, false);
        let (mut g, d2) = until_kickoff(&world, "live-watch", 5, true, false);
        assert!(quick.live.is_none());
        assert!(g.info().unwrap().live, "o jogo do treinador fica por jogar");
        assert!(matches!(g.save(), Err(Error::Invalid(_))), "não grava a meio do jogo");
        // Os resultados do dia só aparecem no fim
        let today = g.date().to_string();
        assert!(g.fixtures(SERIE_A).iter().filter(|f| f.date == today).all(|f| f.home_goals.is_none()));

        let v = g.live_view().unwrap();
        assert_eq!((v.minute, v.home.players.len(), v.away.players.len()), (0, 11, 11));
        assert_eq!(v.others.len(), g.fixtures(SERIE_A).iter().filter(|f| f.date == today).count() - 1);
        let half = g.live_step(45).unwrap();
        assert!(half.half_time && !half.finished);
        assert!(half.events.iter().any(|e| e.kind == "half"));
        let end = g.live_step(200).unwrap();
        assert!(end.finished && end.minute == 90);
        let shots = end.home.stats.shots + end.away.stats.shots;
        assert!(shots > 5, "{shots} remates");
        assert_eq!(end.home.stats.possession + end.away.stats.possession, 100);
        let goals = end.events.iter().filter(|e| e.kind == "goal").count() as i32;
        assert_eq!(goals, end.home_goals + end.away_goals);
        let news = g.live_finish().unwrap();
        assert!(!g.info().unwrap().live);

        // Sem mexer na equipa, é o mesmo jogo que sem ver
        let key = |g: &Game| g.fixtures(SERIE_A).iter().map(|f| (f.id, f.home_goals, f.away_goals)).collect::<Vec<_>>();
        assert_eq!(key(&g), key(&quick));
        assert_eq!(g.date(), quick.date());
        let titles = |n: &[NewsItem]| n.iter().map(|n| n.title.clone()).collect::<Vec<_>>();
        assert_eq!(titles(&news), titles(&quick.news()[..news.len()].iter().rev().cloned().collect::<Vec<_>>()));
        let body = &news.iter().find(|n| n.kind == "competition").unwrap().body;
        if goals > 0 {
            assert!(body.contains("Marcadores:"), "{body}");
        }
        g.save().unwrap();
        quick.save().unwrap();
        drop((g, quick));
        let _ = std::fs::remove_dir_all(&d1);
        let _ = std::fs::remove_dir_all(&d2);
    }

    #[test]
    fn manager_changes_the_team_at_half_time() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let (mut g, dir) = until_kickoff(&world, "live-subs", 8, true, true);
        let v = g.live_step(45).unwrap();
        let side = v.user_side.unwrap();
        let ours = if side == 0 { &v.home } else { &v.away };
        let (out, inn) = (ours.players[10].id, ours.bench.iter().find(|p| p.position != "GR").unwrap().id);
        assert!(g.live_substitute(out, out).is_err(), "quem entra tem de estar no banco");
        let v = g.live_substitute(out, inn).unwrap();
        let ours = if side == 0 { &v.home } else { &v.away };
        assert!(ours.players.iter().any(|p| p.id == inn && p.on_minute == Some(45)));
        assert!(ours.out.iter().any(|p| p.id == out && p.off_minute == Some(45)));
        assert_eq!(ours.subs_left, MAX_SUBS_TEST - 1);
        assert!(g.live_substitute(out, inn).is_err(), "já saiu");
        let mut i = ours.instructions;
        i.mentality = Mentality::Attacking;
        let v = g.live_instructions(i).unwrap();
        assert!(v.events.last().unwrap().kind == "tactics");
        let (a, b) = (ours.players[1].id, ours.players[2].id);
        g.live_swap(a, b).unwrap();
        let v = g.live_step(90).unwrap();
        let ours = if side == 0 { &v.home } else { &v.away };
        let sub = ours.players.iter().chain(&ours.out).find(|p| p.id == inn).unwrap();
        assert!(sub.condition < 100, "{:?}", sub);
        assert!(g.live_substitute(a, b).is_err(), "o jogo acabou");
        g.live_finish().unwrap();
        // Quem entrou jogou (cansou-se um pouco); quem saiu também conta como tendo jogado
        assert!(g.squads.state(inn).condition < 100);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    const MAX_SUBS_TEST: i32 = crate::live::MAX_SUBS;

    #[test]
    fn tactics_screen_during_the_match() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let (mut g, dir) = until_kickoff(&world, "live-tactics", 12, true, true);
        let side = g.live_view().unwrap().user_side.unwrap();
        let t = g.live_tactics(side).unwrap();
        assert!(t.editable && t.reserves.is_empty());
        assert!(!g.live_tactics(1 - side).unwrap().editable, "a do adversário só se vê");

        // Antes do apito: trocar um titular por um suplente não gasta substituições
        let mut sel = t.selection();
        let (starter, sub) = (sel.starters[10].unwrap(), sel.bench[1]);
        sel.starters[10] = Some(sub);
        sel.bench[1] = starter;
        g.live_set_tactics(&sel).unwrap();
        let v = g.live_view().unwrap();
        let ours = if side == 0 { &v.home } else { &v.away };
        assert_eq!(ours.subs_left, MAX_SUBS_TEST);
        assert!(ours.bench.iter().any(|p| p.id == starter), "volta ao banco");

        // Ao intervalo: outra formação (o preview mostra as funções novas) e uma substituição
        g.live_step(45).unwrap();
        let mut sel = g.live_tactics(side).unwrap().selection();
        sel.formation = 4; // 4-3-3
        sel.positions.clear();
        let preview = g.live_preview_tactics(&sel).unwrap();
        assert_eq!(preview.formation, "4-3-3");
        let (out, inn) = (sel.starters[5].unwrap(), sel.bench[0]);
        sel.starters[5] = Some(inn);
        sel.bench.remove(0);
        let v = g.live_set_tactics(&sel).unwrap();
        let ours = if side == 0 { &v.home } else { &v.away };
        assert_eq!(ours.formation, "4-3-3");
        assert_eq!(ours.subs_left, MAX_SUBS_TEST - 1);
        assert!(ours.out.iter().any(|p| p.id == out));
        assert!(v.events.iter().any(|e| e.kind == "tactics"));

        // Quem saiu não volta
        let mut back = g.live_tactics(side).unwrap().selection();
        back.starters[0] = Some(out);
        assert!(g.live_preview_tactics(&back).is_err());
        assert!(g.live_substitute(back.starters[1].unwrap(), out).is_err());

        // Só 5 substituições: com 4 feitas, trocar 2 de uma vez é recusado e 1 passa
        g.live_step(60).unwrap();
        for k in 0..3 {
            let t = g.live_tactics(side).unwrap();
            let sel = t.selection();
            let inn = *sel.bench.iter().find(|&&b| t.bench.iter().any(|p| p.id == b && p.position != "GR")).unwrap();
            g.live_substitute(sel.starters[9 - k].unwrap(), inn).unwrap();
        }
        let t = g.live_tactics(side).unwrap();
        let mut two = t.selection();
        assert!(two.bench.len() >= 2);
        two.starters[2] = Some(two.bench.remove(0));
        two.starters[3] = Some(two.bench.remove(0));
        let err = g.live_preview_tactics(&two).unwrap_err().to_string();
        assert!(err.contains("5 substituições"), "{err}");
        let mut one = t.selection();
        one.starters[2] = Some(one.bench.remove(0));
        g.live_set_tactics(&one).unwrap();
        let v = g.live_view().unwrap();
        assert_eq!(if side == 0 { &v.home } else { &v.away }.subs_left, 0);
        let last = g.live_tactics(side).unwrap().selection();
        assert!(g.live_substitute(last.starters[4].unwrap(), last.bench[0]).is_err(), "a 6ª é recusada");
        let end = g.live_step(90).unwrap();
        assert_eq!(end.events.iter().filter(|e| e.kind == "goal").count() as i32, end.home_goals + end.away_goals);
        g.live_finish().unwrap();
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
    fn match_preview_has_venue_and_is_stable() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("preview");
        let palmeiras = 7568;
        let g = Game::create(&world, &dir, &new_game(palmeiras), 6).unwrap();
        let first = g.club_fixtures(palmeiras)[0].clone();
        let p = g.match_preview(first.id).unwrap();
        assert!(p.venue.is_some() && p.city.is_some());
        assert!(p.last_meeting.is_none(), "1ª jornada: ainda não se defrontaram");
        assert_eq!(p.weather, g.match_preview(first.id).unwrap().weather, "o tempo não muda ao reabrir");
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
