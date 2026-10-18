//! Um jogo em curso (Fase 3): o save é uma cópia do `world.sqlite` com as tabelas
//! `game`, `news` e `fixtures`. O estado que muda dia a dia fica em memória e só vai para
//! o arquivo em [`Game::save`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::engine::{self, TeamStrength};
use crate::league::{self, Fixture, Match, SERIE_A, StandingRow};
use crate::person::ClubRef;
use crate::search::fold;
use crate::{Error, Result, World, date};

/// Versão do formato do save; saves de outra versão são recusados até haver migração.
/// 2: tabela `fixtures` (marco 3.2).
pub const SAVE_VERSION: i32 = 2;

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
        tx.commit()?;
        self.unsaved = false;
        Ok(())
    }

    /// Avança um dia, joga as partidas desse dia e devolve as notícias novas.
    pub fn advance_day(&mut self) -> Result<Vec<NewsItem>> {
        self.date = date::add_days(&self.date, 1).ok_or_else(|| Error::Invalid("data do jogo inválida".into()))?;
        self.unsaved = true;

        let today: Vec<usize> =
            (0..self.matches.len()).filter(|&i| self.matches[i].date == self.date && self.matches[i].result.is_none()).collect();
        if today.is_empty() {
            return Ok(Vec::new());
        }
        // A força de cada clube é calculada uma vez por dia (o onze não muda no 3.2)
        let mut strength: HashMap<i32, TeamStrength> = HashMap::new();
        for &i in &today {
            let (id, home, away) = (self.matches[i].id, self.matches[i].home_id, self.matches[i].away_id);
            for club in [home, away] {
                if !strength.contains_key(&club) {
                    strength.insert(club, self.world.team_strength(club)?);
                }
            }
            let result = engine::play(&strength[&home], &strength[&away], &mut engine::match_rng(self.seed, id));
            self.matches[i].result = Some(result);
        }

        let mut fresh = Vec::new();
        let mut next_id = self.news.iter().map(|n| n.id).max().unwrap_or(0) + 1;
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
        assert_eq!(g.info().unwrap().unread_news, 7);
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
