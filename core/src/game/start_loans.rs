//! Empréstimos em curso quando o jogo começa (época 2025/26). O banco do CM 01/02 só guarda
//! o clube com contrato, por isso os empréstimos conhecidos dos clubes grandes vêm de uma
//! lista feita à mão (`core/data/loans_2025.json`). Quem está no clube dono vai para o clube
//! do empréstimo; quem o patch já pôs no clube do empréstimo fica com o dono registado, para
//! voltar no fim. Os que o patch tem noutro clube ficam como estão.

use rusqlite::{OptionalExtension, params};
use serde::Deserialize;

use super::Game;
use crate::{Error, Result};

const LOANS: &str = include_str!("../../data/loans_2025.json");

#[derive(Deserialize)]
struct StartLoan {
    player: i32,
    parent: i32,
    club: i32,
    until: String,
}

impl Game {
    /// Aplica a lista ao criar o jogo; devolve quantos empréstimos ficaram.
    pub(super) fn start_loans(&mut self) -> Result<usize> {
        let loans: Vec<StartLoan> =
            serde_json::from_str(LOANS).map_err(|e| Error::Invalid(format!("Lista de empréstimos inválida: {e}")))?;
        let mut done = 0;
        for l in loans {
            let club: Option<Option<i32>> =
                self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [l.player], |r| r.get(0)).optional()?;
            match club.flatten() {
                Some(c) if c == l.parent => {
                    self.world
                        .conn
                        .execute("UPDATE people SET club_id = ?, club_joined = ? WHERE id = ?", params![l.club, self.date, l.player])?;
                    self.number_after_move(l.player, Some(l.club))?;
                }
                Some(c) if c == l.club => {}
                _ => continue,
            }
            self.world.conn.execute(
                "INSERT OR REPLACE INTO loans (person_id, parent_id, until) VALUES (?, ?, ?)",
                params![l.player, l.parent, l.until],
            )?;
            done += 1;
        }
        Ok(done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_loans_start_at_the_loan_club() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-emprestimos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Empréstimos".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 6271,
        };
        let g = Game::create(&path, &dir, &new, 3).unwrap();
        // Onana (Man Utd) joga no Trabzonspor e volta ao United no fim da época
        let (club, parent): (Option<i32>, i32) = g
            .world
            .conn
            .query_row("SELECT p.club_id, l.parent_id FROM people p JOIN loans l ON l.person_id = p.id WHERE p.id = 26601", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((club, parent), (Some(10294), 6271));
        // Højlund já estava no Napoli: fica emprestado pelo United
        let parent: i32 = g.world.conn.query_row("SELECT parent_id FROM loans WHERE person_id = 63632", [], |r| r.get(0)).unwrap();
        assert_eq!(parent, 6271);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
