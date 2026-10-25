//! Lista Preferencial do treinador: os jogadores que acompanha. Entram sozinhos quando ele faz
//! uma proposta, pede um empréstimo, propõe um contrato ou manda observar; ou à mão, no menu
//! Acção. A pesquisa filtra por ela (`MarketStatus::Shortlist`).

use rusqlite::params;

use super::Game;
use crate::Result;

impl Game {
    /// Junta um jogador à lista (os do plantel do treinador não entram).
    pub(super) fn add_shortlist(&self, person: i32) -> Result<()> {
        let club: Option<i32> = self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [person], |r| r.get(0))?;
        if club != Some(self.club_id) {
            self.world.conn.execute("INSERT OR IGNORE INTO shortlist (person_id, added) VALUES (?, ?)", params![person, self.date])?;
        }
        Ok(())
    }

    /// Adicionar (true) ou Retirar da Lista Preferencial.
    pub fn set_shortlist(&mut self, person: i32, on: bool) -> Result<()> {
        if on {
            self.add_shortlist(person)?;
        } else {
            self.world.conn.execute("DELETE FROM shortlist WHERE person_id = ?", [person])?;
        }
        self.unsaved = true;
        Ok(())
    }

    pub fn in_shortlist(&self, person: i32) -> Result<bool> {
        Ok(self.world.conn.query_row("SELECT COUNT(*) FROM shortlist WHERE person_id = ?", [person], |r| r.get::<_, i64>(0))? > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{MarketStatus, PersonKind, SearchQuery};

    #[test]
    fn shortlist_filters_the_search() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-lista-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Lista".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
        };
        let mut g = Game::create(&path, &dir, &new, 41).unwrap();
        let (mine, other): (i32, i32) = g
            .world
            .conn
            .query_row(
                "SELECT (SELECT id FROM people WHERE club_id = 7568 AND is_player = 1 LIMIT 1),
                        (SELECT id FROM people WHERE club_id <> 7568 AND is_player = 1 LIMIT 1)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        g.set_shortlist(mine, true).unwrap();
        g.set_shortlist(other, true).unwrap();
        assert!(!g.in_shortlist(mine).unwrap(), "os do plantel não entram");
        assert!(g.in_shortlist(other).unwrap());
        let q = SearchQuery { kind: Some(PersonKind::Players), status: Some(MarketStatus::Shortlist), ..Default::default() };
        let r = g.world.search_people_on(&q, &g.date).unwrap();
        assert_eq!(r.items.iter().map(|p| p.id).collect::<Vec<_>>(), vec![other]);
        g.set_shortlist(other, false).unwrap();
        assert_eq!(g.world.search_people_on(&q, &g.date).unwrap().total, 0);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
