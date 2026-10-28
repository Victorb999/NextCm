//! As ligas do jogo, escolhidas ao começar como no CM ("Escolher Campeonato(s)"): de cada
//! nação com regulamento, **Escolhido** (joga-se com tudo: mercado, evolução dos plantéis,
//! juniores, estatísticas dos jogadores, pode-se treinar lá), **Fundo** (as competições jogam-se,
//! com subidas e descidas, mas os plantéis não mudam nem entram no mercado) ou **Fora** (sem
//! calendário: os clubes ficam parados). As competições continentais jogam-se sempre. Sem
//! escolha (saves antigos, CLI, testes), tudo é Escolhido.

use std::collections::HashMap;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::Game;
use crate::regulation::{self, SeasonKind};
use crate::{Error, Result, World};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum LeagueMode {
    #[default]
    Play,
    Background,
    Off,
}

impl LeagueMode {
    fn key(self) -> &'static str {
        match self {
            LeagueMode::Play => "play",
            LeagueMode::Background => "background",
            LeagueMode::Off => "off",
        }
    }

    fn from_key(k: &str) -> LeagueMode {
        match k {
            "background" => LeagueMode::Background,
            "off" => LeagueMode::Off,
            _ => LeagueMode::Play,
        }
    }
}

/// A escolha de uma nação no ecrã "Escolher Campeonato(s)".
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LeagueChoice {
    pub nation_id: i32,
    pub mode: LeagueMode,
}

/// Uma nação da lista, com as divisões que tem no jogo.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LeagueNation {
    pub nation_id: i32,
    pub name: String,
    /// "Série A, Série B..." (as divisões com regulamento).
    pub divisions: Vec<String>,
    /// Época civil (Brasil, Argentina) ou europeia.
    pub calendar: bool,
}

/// A nação de cada competição com regulamento (as continentais não têm).
pub(super) fn comp_nations(world: &World) -> Result<HashMap<i32, i32>> {
    let mut out = HashMap::new();
    for r in regulation::all() {
        let nation: Option<i32> =
            world.conn.query_row("SELECT nation_id FROM competitions WHERE id = ?", [r.competition], |row| row.get(0)).unwrap_or(None);
        if let Some(n) = nation {
            out.insert(r.competition, n);
        }
    }
    Ok(out)
}

/// As nações que se podem escolher (as que têm divisões com regulamento).
pub fn league_nations(world: &World) -> Result<Vec<LeagueNation>> {
    let nations = comp_nations(world)?;
    let mut out: Vec<LeagueNation> = Vec::new();
    for r in regulation::all() {
        let Some(&n) = nations.get(&r.competition) else { continue };
        let comp: String =
            world.conn.query_row("SELECT name FROM competitions WHERE id = ?", [r.competition], |row| row.get(0)).unwrap_or_default();
        let comp = r.name.clone().unwrap_or(comp);
        match out.iter_mut().find(|x| x.nation_id == n) {
            Some(x) => x.divisions.push(comp),
            None => {
                let name: String = world.conn.query_row("SELECT name FROM nations WHERE id = ?", [n], |row| row.get(0)).unwrap_or_default();
                out.push(LeagueNation { nation_id: n, name, divisions: vec![comp], calendar: r.season == SeasonKind::Calendar });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

impl Game {
    /// Grava a escolha das ligas (ao criar o jogo). O clube do treinador tem de ser de uma
    /// nação escolhida.
    pub(super) fn set_league_modes(&mut self, choices: &[LeagueChoice], club_nation: Option<i32>) -> Result<()> {
        if choices.is_empty() {
            return Ok(());
        }
        if !choices.iter().any(|c| c.mode == LeagueMode::Play) {
            return Err(Error::Invalid("Escolha pelo menos uma liga para jogar.".into()));
        }
        if let Some(n) = club_nation
            && self.comp_nations.values().any(|&x| x == n)
            && !choices.iter().any(|c| c.nation_id == n && c.mode == LeagueMode::Play)
        {
            return Err(Error::Invalid("O clube do treinador tem de ser de uma liga escolhida.".into()));
        }
        for c in choices {
            self.world
                .conn
                .execute("INSERT OR REPLACE INTO league_modes (nation_id, mode) VALUES (?, ?)", params![c.nation_id, c.mode.key()])?;
            self.league_modes.insert(c.nation_id, c.mode);
        }
        Ok(())
    }

    /// Lê a escolha gravada.
    pub(super) fn load_league_modes(world: &World) -> Result<HashMap<i32, LeagueMode>> {
        Ok(world
            .conn
            .prepare("SELECT nation_id, mode FROM league_modes")?
            .query_map([], |r| Ok((r.get::<_, i32>(0)?, LeagueMode::from_key(&r.get::<_, String>(1)?))))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Como se joga uma competição (as continentais: sempre).
    pub(super) fn comp_mode(&self, comp: i32) -> LeagueMode {
        self.comp_nations.get(&comp).and_then(|n| self.league_modes.get(n)).copied().unwrap_or_default()
    }

    /// Divisões geridas (regulamento com `managed`, de nações escolhidas), de um tipo de
    /// época ou de todos, para um `IN (...)` do SQL.
    pub(super) fn managed_sql(&self, kind: Option<SeasonKind>) -> String {
        let ids: Vec<String> = regulation::all()
            .iter()
            .filter(|r| r.managed && kind.is_none_or(|k| r.season == k) && self.comp_mode(r.competition) == LeagueMode::Play)
            .map(|r| r.competition.to_string())
            .collect();
        if ids.is_empty() { "-1".into() } else { ids.join(", ") }
    }

    pub(super) fn managed_ids(&self) -> String {
        self.managed_sql(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BRAZIL: i32 = 27;
    const ENGLAND: i32 = 61;
    const SPAIN: i32 = 172;
    const PREMIER: i32 = 7;
    const LA_LIGA: i32 = 52;

    #[test]
    fn chosen_background_and_off_leagues() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-ligas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let choice = |nation_id, mode| LeagueChoice { nation_id, mode };
        let mut new = crate::NewGame {
            save_name: "Ligas".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: vec![choice(BRAZIL, LeagueMode::Background), choice(ENGLAND, LeagueMode::Play)],
        };
        // O clube do treinador tem de ser de uma liga escolhida
        assert!(Game::create(&path, &dir, &new, 3).is_err());
        let _ = std::fs::remove_dir_all(&dir);
        new.leagues = vec![choice(BRAZIL, LeagueMode::Play), choice(ENGLAND, LeagueMode::Background), choice(SPAIN, LeagueMode::Off)];
        let mut g = Game::create(&path, &dir, &new, 3).unwrap();
        assert!(g.matches.iter().any(|m| m.competition_id == PREMIER), "a liga de fundo joga-se");
        assert!(!g.matches.iter().any(|m| m.competition_id == LA_LIGA), "a liga fora do jogo não");
        assert!(!g.managed_ids().split(", ").any(|c| c == PREMIER.to_string()), "a de fundo não é gerida");
        // As ligas de fundo não gravam as estatísticas dos jogadores
        for _ in 0..50 {
            g.advance_day().unwrap();
        }
        let rows = |comp: i32| -> i64 {
            g.world.conn.query_row("SELECT COUNT(*) FROM player_matches WHERE competition = ?", [comp], |r| r.get(0)).unwrap()
        };
        assert!(g.matches.iter().any(|m| m.competition_id == PREMIER && m.result.is_some()));
        assert_eq!(rows(PREMIER), 0);
        assert!(rows(crate::league::SERIE_A) > 0);
        // Ao carregar, a escolha continua
        g.save().unwrap();
        let file = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&file).unwrap();
        assert_eq!(g.comp_mode(LA_LIGA), LeagueMode::Off);
        assert_eq!(g.comp_mode(PREMIER), LeagueMode::Background);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
