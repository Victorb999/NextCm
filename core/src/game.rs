//! Um jogo em curso (Fase 3, marco 3.1): o save é uma cópia do `world.sqlite` com as
//! tabelas `game` e `news`. O estado que muda dia a dia fica em memória e só vai para o
//! arquivo em [`Game::save`].

use std::path::{Path, PathBuf};

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::person::ClubRef;
use crate::search::fold;
use crate::{Error, Result, World, date};

/// Versão do formato do save; saves de outra versão são recusados até haver migração.
pub const SAVE_VERSION: i32 = 1;

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
                unsaved: true,
            };
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
        Ok(Game { world, path: path.to_path_buf(), name, date, seed, manager_id, club_id, news, unsaved: false })
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
        tx.commit()?;
        self.unsaved = false;
        Ok(())
    }

    /// Avança um dia e devolve as notícias novas (no 3.1 ainda não há eventos).
    pub fn advance_day(&mut self) -> Result<Vec<NewsItem>> {
        self.date = date::add_days(&self.date, 1).ok_or_else(|| Error::Invalid("data do jogo inválida".into()))?;
        self.unsaved = true;
        Ok(Vec::new())
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
        assert_eq!(info.unread_news, 1);
        assert!(g.news()[0].body.contains("Victor Araujo"));

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
        let id = g.news()[0].id;
        g.mark_news_read(id);
        assert!(g.info().unwrap().unsaved);
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);

        let g = Game::load(&path).unwrap();
        let info = g.info().unwrap();
        assert_eq!(info.date, "2025-09-01");
        assert_eq!(info.unread_news, 0);
        assert!(!info.unsaved);

        let saves = list_saves(&dir).unwrap();
        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].club_name, "Sociedade Esportiva Palmeiras");
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
