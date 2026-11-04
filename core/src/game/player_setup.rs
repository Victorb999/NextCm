//! O resto do `player_setup.cfg` do patch, que o CM aplica ao começar um jogo (os empréstimos
//! estão em `start_loans.rs`). `scripts/player_setup.py` cruza as linhas com a base e grava
//! `core/data/setup_events.json`:
//!
//! - **Lesões**: o jogador começa lesionado, com o nome e uma duração sorteada entre a mínima e
//!   a máxima do tipo (com a seed do jogo).
//! - **Transferências acertadas**: no dia, se ainda está no clube de origem, muda-se para o de
//!   destino com o valor, o salário e os anos da linha (quase todas são juniores que sobem à
//!   equipa principal, como o Primavera do Parma para o Parma).
//! - **Reformas**: no dia, deixa o clube e termina a carreira.
//! - **Castigos** (má conduta, passaporte falso, doping): não joga em clube nenhum até ao dia.
//!
//! Começando em 2026/27 (Europa), as datas vêm um ano depois, como os empréstimos.

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;

use super::{Game, NewsItem};
use crate::engine::uniform;
use crate::{Error, Result, date};

const EVENTS: &str = include_str!("../../data/setup_events.json");

/// As transferências, reformas e castigos marcados (save 42).
pub(super) const SETUP_SCHEMA: &str = "
CREATE TABLE setup_transfers (person_id INTEGER PRIMARY KEY, from_club INTEGER, to_club INTEGER NOT NULL, date TEXT NOT NULL,
                              fee INTEGER NOT NULL, wage INTEGER NOT NULL, years INTEGER NOT NULL);
CREATE TABLE setup_retirements (person_id INTEGER PRIMARY KEY, date TEXT NOT NULL);
CREATE TABLE bans (person_id INTEGER PRIMARY KEY, kind TEXT NOT NULL, until TEXT NOT NULL);
";

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Event {
    Injury { player: i32, injury: String, min: i32, max: i32 },
    Transfer { player: i32, from: Option<i32>, to: i32, date: String, fee: i64, wage: i64, years: i32 },
    Retirement { player: i32, date: String },
    Ban { player: i32, ban: String, date: String },
}

/// O texto de cada castigo.
pub(super) fn ban_text(kind: &str) -> &'static str {
    match kind {
        "passport" => "Castigado por usar um passaporte falso",
        "drugs" => "Castigado por doping",
        _ => "Castigado por má conduta",
    }
}

/// Uma data `years` anos depois ("2026-08-04" + 1 = "2027-08-04").
fn later(day: &str, years: i32) -> String {
    match day.get(..4).and_then(|y| y.parse::<i32>().ok()) {
        Some(y) => format!("{}{}", y + years, &day[4..]),
        None => day.to_string(),
    }
}

impl Game {
    /// Ao criar o jogo: as lesões já e o resto marcado (datas `years_later` anos depois).
    pub(super) fn start_setup(&mut self, years_later: i32) -> Result<()> {
        let events: Vec<Event> =
            serde_json::from_str(EVENTS).map_err(|e| Error::Invalid(format!("Lista do player_setup inválida: {e}")))?;
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed as u64 ^ 0x0005_E70F);
        let today = self.date.clone();
        for e in events {
            match e {
                Event::Injury { player, injury, min, max } => {
                    let days = min + (uniform(&mut rng) * (max - min + 1) as f64) as i32;
                    let Some(until) = date::add_days(&today, days) else { continue };
                    let s = self.squads.states.entry(player).or_default();
                    s.injury = Some(injury.clone());
                    s.injury_until = Some(until);
                    self.world.conn.execute(
                        "INSERT INTO injury_log (person_id, date, injury, kind, days) VALUES (?, ?, ?, 'Antes da época', ?)",
                        params![player, today, injury, days],
                    )?;
                }
                Event::Transfer { player, from, to, date, fee, wage, years } => {
                    self.world.conn.execute(
                        "INSERT OR REPLACE INTO setup_transfers (person_id, from_club, to_club, date, fee, wage, years) VALUES (?, ?, ?, ?, ?, ?, ?)",
                        params![player, from, to, later(&date, years_later), fee, (wage as f64 * crate::finance::WAGE_SCALE) as i64, years],
                    )?;
                }
                Event::Retirement { player, date } => {
                    self.world.conn.execute(
                        "INSERT OR REPLACE INTO setup_retirements (person_id, date) VALUES (?, ?)",
                        params![player, later(&date, years_later)],
                    )?;
                }
                Event::Ban { player, ban, date } => {
                    let until = later(&date, years_later);
                    if until > today {
                        self.world.conn.execute(
                            "INSERT OR REPLACE INTO bans (person_id, kind, until) VALUES (?, ?, ?)",
                            params![player, ban, until],
                        )?;
                    }
                }
            }
        }
        // O que já passou (num arranque mais tarde) acontece já
        let (mut fresh, mut id) = (Vec::new(), 0);
        self.setup_day(&mut fresh, &mut id)?;
        self.unsaved = true;
        Ok(())
    }

    /// Os castigados que ainda não podem jogar.
    pub(super) fn banned_players(&self) -> Vec<i32> {
        let rows = || -> Result<Vec<i32>> {
            Ok(self
                .world
                .conn
                .prepare_cached("SELECT person_id FROM bans WHERE until > ?")?
                .query_map([&self.date], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?)
        };
        rows().unwrap_or_default()
    }

    /// O castigo de um jogador: (motivo, até quando).
    pub fn ban_of(&self, person: i32) -> Result<Option<(String, String)>> {
        Ok(self
            .world
            .conn
            .query_row("SELECT kind, until FROM bans WHERE person_id = ? AND until > ?", params![person, self.date], |r| {
                Ok((r.get::<_, String>(0)?, r.get(1)?))
            })
            .optional()?
            .map(|(k, u)| (ban_text(&k).to_string(), u)))
    }

    /// Cada dia: as transferências acertadas e as reformas cujo dia chegou.
    pub(super) fn setup_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        type Due = (i32, Option<i32>, i32, i64, i64, i32);
        let due: Vec<Due> = self
            .world
            .conn
            .prepare_cached(
                "SELECT person_id, from_club, to_club, fee, wage, years FROM setup_transfers WHERE date <= ? ORDER BY person_id",
            )?
            .query_map([&self.date], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (person, from, to, fee, wage, years) in due {
            self.world.conn.execute("DELETE FROM setup_transfers WHERE person_id = ?", [person])?;
            // Só se ainda está onde estava (e não se reformou)
            let at: Option<Option<i32>> = self
                .world
                .conn
                .query_row("SELECT club_id FROM people WHERE id = ? AND id NOT IN (SELECT person_id FROM retired)", [person], |r| r.get(0))
                .optional()?;
            let Some(at) = at else { continue };
            if at != from || at == Some(to) {
                continue;
            }
            let kind = self.kind_of(to)?;
            self.execute_move(person, from, Some(to), fee, wage, years, if fee > 0 { "transfer" } else { "free" }, kind)?;
            if from == Some(self.club_id) || to == self.club_id {
                let name = self.person_name(person)?;
                let (f, t) = (from.map(|c| self.club(c).short_name).unwrap_or_default(), self.club(to).short_name);
                let body = format!("{name} mudou-se do {f} para o {t}, como estava combinado.");
                fresh.push(self.news_item(*next_id, "transfer", format!("{name} muda-se para o {t}"), body));
                *next_id += 1;
            }
        }
        let retiring: Vec<(i32, Option<i32>)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT r.person_id, p.club_id FROM setup_retirements r JOIN people p ON p.id = r.person_id WHERE r.date <= ? ORDER BY r.person_id",
            )?
            .query_map([&self.date], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (person, club) in retiring {
            self.world.conn.execute("DELETE FROM setup_retirements WHERE person_id = ?", [person])?;
            self.world.conn.execute("INSERT OR IGNORE INTO retired (person_id, date) VALUES (?, ?)", params![person, self.date])?;
            self.world.conn.execute("UPDATE people SET club_id = NULL, club_job = NULL, contract_expires = NULL WHERE id = ?", [person])?;
            if club == Some(self.club_id) {
                let name = self.person_name(person)?;
                fresh.push(self.news_item(
                    *next_id,
                    "message",
                    format!("{name} termina a carreira"),
                    format!("{name} terminou a carreira."),
                ));
                *next_id += 1;
            }
        }
        self.world.conn.execute("DELETE FROM bans WHERE until <= ?", [&self.date])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_setup_events() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-player-setup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Player setup".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: false,
        };
        let mut g = Game::create(&path, &dir, &new, 5).unwrap();
        let events: Vec<Event> = serde_json::from_str(EVENTS).unwrap();
        // Os lesionados do cfg começam lesionados
        let hurt: Vec<i32> =
            events.iter().filter_map(|e| if let Event::Injury { player, .. } = e { Some(*player) } else { None }).collect();
        assert!(hurt.len() > 600);
        assert!(hurt.iter().take(50).all(|id| g.squads.state(*id).injury.is_some()));
        // Os castigados não jogam
        let banned = g.banned_players();
        assert!(!banned.is_empty());
        assert!(g.ban_of(banned[0]).unwrap().is_some());
        // Uma transferência acertada acontece no dia
        let (person, to, day): (i32, i32, String) = g
            .world
            .conn
            .query_row(
                "SELECT t.person_id, t.to_club, t.date FROM setup_transfers t JOIN people p ON p.id = t.person_id
                 WHERE p.club_id IS t.from_club ORDER BY t.date, t.person_id LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        g.date = day.clone();
        let (mut fresh, mut id) = (Vec::new(), 0);
        g.setup_day(&mut fresh, &mut id).unwrap();
        let club: Option<i32> = g.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [person], |r| r.get(0)).unwrap();
        assert_eq!(club, Some(to), "{person} em {day}");
        // E as reformas no dia delas
        let (retiring, day): (i32, String) = g
            .world
            .conn
            .query_row("SELECT person_id, date FROM setup_retirements ORDER BY date LIMIT 1", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        g.date = day;
        g.setup_day(&mut fresh, &mut id).unwrap();
        let retired: bool =
            g.world.conn.query_row("SELECT COUNT(*) > 0 FROM retired WHERE person_id = ?", [retiring], |r| r.get(0)).unwrap();
        assert!(retired);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
