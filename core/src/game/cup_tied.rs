//! Jogador "cup-tied" nas continentais (item 27 do 3.19.1), como no CM: quem jogou a Liga dos
//! Campeões, a Liga Europa, a Libertadores ou a Sul-Americana por um clube não a joga por outro
//! na mesma época. Fica indisponível para o adjunto nesses jogos, e o perfil diz porquê.

use rusqlite::params;

use super::Game;
use super::finances::CONTINENTAL;
use crate::Result;
use crate::regulation::{self, SeasonKind};

impl Game {
    /// O primeiro dia da época da competição em que hoje cai.
    fn competition_season_start(&self, competition: i32) -> String {
        let year = regulation::season_of(competition, &self.date);
        match regulation::season_kind(competition) {
            SeasonKind::European => format!("{year}-07-01"),
            SeasonKind::International => format!("{year}-08-01"),
            SeasonKind::Calendar => format!("{year}-01-01"),
        }
    }

    /// Os jogadores do clube que já jogaram esta continental por outro clube nesta época.
    pub(super) fn cup_tied(&self, club: i32, competition: i32) -> Result<Vec<i32>> {
        if !CONTINENTAL.contains(&competition) {
            return Ok(Vec::new());
        }
        let start = self.competition_season_start(competition);
        Ok(self
            .world
            .conn
            .prepare_cached(
                "SELECT DISTINCT m.person_id FROM player_matches m JOIN people p ON p.id = m.person_id
                 WHERE p.club_id = ?1 AND m.competition = ?2 AND m.date >= ?3 AND m.club <> ?1",
            )?
            .query_map(params![club, competition, start], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// As continentais que o jogador já não pode jogar pelo clube dele nesta época (o perfil).
    pub(super) fn cup_tied_competitions(&self, person: i32) -> Result<Vec<String>> {
        let club: Option<i32> = self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [person], |r| r.get(0))?;
        let Some(club) = club else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        for comp in CONTINENTAL {
            if self.cup_tied(club, comp)?.contains(&person) {
                out.push(self.competition_name(comp).to_string());
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::league::CHAMPIONS_LEAGUE;

    #[test]
    fn played_for_another_club_in_the_same_continental() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-cup-tied-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Cup-tied".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 1350,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
        };
        let mut g = Game::create(&path, &dir, &new, 31).unwrap();
        g.date = "2025-10-01".into();
        let kane: i32 = g.world.conn.query_row("SELECT id FROM people WHERE display_name = 'Harry Kane'", [], |r| r.get(0)).unwrap();
        assert!(g.cup_tied(1350, CHAMPIONS_LEAGUE).unwrap().is_empty());
        // Jogou a Liga dos Campeões por outro clube (o Tottenham) nesta época
        g.world
            .conn
            .execute(
                "INSERT INTO player_matches (person_id, fixture, date, competition, club, rating, goals, assists) VALUES (?, -1, '2025-09-17', ?, 10270, 7.0, 0, 0)",
                params![kane, CHAMPIONS_LEAGUE],
            )
            .unwrap();
        assert_eq!(g.cup_tied(1350, CHAMPIONS_LEAGUE).unwrap(), vec![kane]);
        assert!(g.club_state(1350, Some(CHAMPIONS_LEAGUE)).get(&kane).is_some_and(|a| !a.can_play()));
        assert!(g.club_state(1350, None).get(&kane).is_none_or(|a| a.can_play()), "joga o resto");
        assert_eq!(g.cup_tied_competitions(kane).unwrap().len(), 1);
        // Na época seguinte já pode
        g.date = "2026-07-15".into();
        assert!(g.cup_tied(1350, CHAMPIONS_LEAGUE).unwrap().is_empty());
        // Nas outras competições não conta
        assert!(g.cup_tied(1350, 16).unwrap().is_empty());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
