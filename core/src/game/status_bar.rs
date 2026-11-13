//! A barra de baixo (marco 4.0, ideia 6 do 3.21): o caminho da tela aberta (Europa › Inglaterra ›
//! Premier › Man Utd) e os resultados do último dia com jogos, como a do FM08.

use rusqlite::OptionalExtension;
use serde::Serialize;
use ts_rs::TS;

use super::Game;
use crate::Result;

/// Um passo do caminho; com `kind` e `id`, abre a tela dele.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PathStep {
    pub label: String,
    /// "nation", "competition", "club" ou "person" (nada: só texto, como o continente).
    pub kind: Option<String>,
    pub id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct StatusBar {
    pub path: Vec<PathStep>,
    /// Os resultados do último dia com jogos ("Série A: Santos 2-1 Bahia"), os do país do
    /// treinador primeiro.
    pub results: Vec<String>,
}

fn continent_pt(name: &str) -> &str {
    match name {
        "Africa" => "África",
        "Asia" => "Ásia",
        "Europe" => "Europa",
        "North America" => "América do Norte",
        "Oceania" => "Oceânia",
        "South America" => "América do Sul",
        other => other,
    }
}

fn step(label: impl Into<String>, kind: Option<&str>, id: Option<i32>) -> PathStep {
    PathStep { label: label.into(), kind: kind.map(str::to_string), id }
}

impl Game {
    /// A barra de baixo para a tela `kind` (a do `Screen` do front) com o `id` dela.
    pub fn status_bar(&self, kind: &str, id: Option<i32>) -> Result<StatusBar> {
        let path = match (kind, id) {
            ("club" | "tactics" | "clubHistory", Some(id)) => self.club_path(id)?,
            ("competition", Some(id)) => self.competition_path(id)?,
            ("nation", Some(id)) => self.nation_path(id)?,
            ("person" | "contract", Some(id)) => self.person_path(id)?,
            _ => self.club_path(self.club_id)?,
        };
        Ok(StatusBar { path, results: self.latest_results() })
    }

    /// Continente › nação.
    fn nation_path(&self, nation: i32) -> Result<Vec<PathStep>> {
        let row: Option<(String, Option<String>)> = self
            .world
            .conn
            .query_row(
                "SELECT n.name, ct.name FROM nations n LEFT JOIN continents ct ON ct.id = n.continent_id WHERE n.id = ?",
                [nation],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((name, continent)) = row else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        if let Some(c) = continent {
            out.push(step(continent_pt(&c), None, None));
        }
        out.push(step(name, Some("nation"), Some(nation)));
        Ok(out)
    }

    /// Continente › nação › competição (as continentais só com o continente).
    fn competition_path(&self, comp: i32) -> Result<Vec<PathStep>> {
        let row: Option<(String, Option<i32>, Option<String>)> = self
            .world
            .conn
            .query_row(
                "SELECT COALESCE(NULLIF(c.short_name, ''), c.name), c.nation_id, ct.name FROM competitions c
                 LEFT JOIN continents ct ON ct.id = c.continent_id WHERE c.id = ?",
                [comp],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((name, nation, continent)) = row else { return Ok(Vec::new()) };
        let mut out = match nation {
            Some(n) => self.nation_path(n)?,
            None => continent.map(|c| vec![step(continent_pt(&c), None, None)]).unwrap_or_default(),
        };
        out.push(step(name, Some("competition"), Some(comp)));
        Ok(out)
    }

    /// Continente › nação › divisão › clube.
    fn club_path(&self, club: i32) -> Result<Vec<PathStep>> {
        let row: Option<(Option<i32>, Option<i32>)> = self
            .world
            .conn
            .query_row("SELECT nation_id, division_id FROM clubs WHERE id = ?", [club], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((nation, division)) = row else { return Ok(Vec::new()) };
        let mut out = match (division, nation) {
            (Some(d), _) => self.competition_path(d)?,
            (None, Some(n)) => self.nation_path(n)?,
            _ => Vec::new(),
        };
        out.push(step(self.club(club).short_name, Some("club"), Some(club)));
        Ok(out)
    }

    /// O caminho do clube do jogador (ou da nação dele, se estiver livre) e o nome.
    fn person_path(&self, person: i32) -> Result<Vec<PathStep>> {
        let row: Option<(Option<i32>, Option<i32>)> = self
            .world
            .conn
            .query_row("SELECT club_id, nation_id FROM people WHERE id = ?", [person], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((club, nation)) = row else { return Ok(Vec::new()) };
        let mut out = match (club, nation) {
            (Some(c), _) => self.club_path(c)?,
            (None, Some(n)) => self.nation_path(n)?,
            _ => Vec::new(),
        };
        out.push(step(self.person_name(person)?, Some("person"), Some(person)));
        Ok(out)
    }

    /// Os resultados do último dia com jogos até hoje (os de hoje, se já houve).
    fn latest_results(&self) -> Vec<String> {
        let last = self.matches.iter().filter(|m| m.result.is_some() && m.date <= self.date).map(|m| m.date.as_str()).max();
        match last {
            Some(day) => {
                let day = day.to_string();
                self.results_where(|m| m.date == day)
            }
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Game, NewGame, World};

    #[test]
    fn path_of_club_person_and_competition() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-barra-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = NewGame {
            save_name: "Barra".into(),
            first_name: "A".into(),
            last_name: "B".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let g = Game::create(&path, &dir, &new, 5).unwrap();
        let labels =
            |kind: &str, id: Option<i32>| -> Vec<String> { g.status_bar(kind, id).unwrap().path.into_iter().map(|p| p.label).collect() };
        let club = labels("club", Some(7568));
        assert_eq!(club.first().map(String::as_str), Some("América do Sul"));
        assert_eq!(club.last().map(String::as_str), Some("Palmeiras"));
        assert_eq!(club.len(), 4, "continente › nação › divisão › clube: {club:?}");
        assert_eq!(labels("search", None), club, "fora das telas com caminho: o clube do treinador");
        let person: i32 = g.world.conn.query_row("SELECT id FROM people WHERE club_id = 7568 LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(labels("person", Some(person)).len(), 5);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
