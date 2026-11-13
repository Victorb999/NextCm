//! Empréstimos em curso quando o jogo começa (época 2025/26). O banco do CM 01/02 só guarda
//! o clube com contrato: o CM põe os empréstimos ao começar o jogo, pelas linhas "LOAN" do
//! `player_setup.cfg` do patch. `scripts/player_setup.py` cruza-as com a base e grava
//! `core/data/setup_loans.json` (o jogador, o dono, o clube do empréstimo e o fim). Quem está
//! no clube dono vai para o clube do empréstimo; quem o patch já pôs no clube do empréstimo
//! fica com o dono registado, para voltar no fim. Os que o patch tem noutro clube ficam como
//! estão.
//!
//! Começando em 2026/27 (Europa), o jogo usa os plantéis do patch como os dessa época (renova
//! os contratos que acabavam a 30/6/2026), por isso os empréstimos também acabam um ano depois:
//! senão os que o patch tem no clube do empréstimo ficavam lá de vez, sem empréstimo.

use rusqlite::{OptionalExtension, params};
use serde::Deserialize;

use super::Game;
use crate::{Error, Result};

const LOANS: &str = include_str!("../../data/setup_loans.json");

#[derive(Deserialize)]
struct StartLoan {
    player: i32,
    parent: i32,
    club: i32,
    until: String,
}

impl Game {
    /// Aplica a lista ao criar o jogo, com o fim `years_later` anos depois do da lista; devolve
    /// quantos empréstimos ficaram.
    pub(super) fn start_loans(&mut self, years_later: i32) -> Result<usize> {
        let loans: Vec<StartLoan> =
            serde_json::from_str(LOANS).map_err(|e| Error::Invalid(format!("Lista de empréstimos inválida: {e}")))?;
        let mut done = 0;
        for mut l in loans {
            if let Some(year) = l.until.get(..4).and_then(|y| y.parse::<i32>().ok()) {
                l.until = format!("{}{}", year + years_later, &l.until[4..]);
            }
            if l.until <= self.date {
                continue;
            }
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
            club_id: 1350,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let g = Game::create(&path, &dir, &new, 3).unwrap();
        let at = |g: &Game, id: i32| -> (Option<i32>, i32, String) {
            g.world
                .conn
                .query_row(
                    "SELECT p.club_id, l.parent_id, l.until FROM people p JOIN loans l ON l.person_id = p.id WHERE p.id = ?",
                    [id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap()
        };
        // Como no CM: Zaragoza no Espanyol e Boey no Galatasaray, emprestados pelo Bayern até
        // 15/6/2026; Onana no Trabzonspor, pelo United
        let espanyol: i32 = g.world.conn.query_row("SELECT id FROM clubs WHERE name = 'R.C.D. Espanyol'", [], |r| r.get(0)).unwrap();
        assert_eq!(at(&g, 123625), (Some(espanyol), 1350, "2026-06-15".to_string()));
        assert_eq!(at(&g, 36829).1, 1350);
        assert_eq!(at(&g, 26601), (Some(10294), 6271, "2026-06-15".to_string()));
        // O Rashford, sem linha no player_setup.cfg, fica no United
        let rashford: Option<i32> = g.world.conn.query_row("SELECT club_id FROM people WHERE id = 67186", [], |r| r.get(0)).unwrap();
        assert_eq!(rashford, Some(6271));
        assert!(g.world.conn.query_row("SELECT COUNT(*) FROM loans", [], |r| r.get::<_, i64>(0)).unwrap() > 5000);
        // E aparecem no plantel do dono como emprestados (a azul), como no print do Bayern
        let mut bayern = g.world.get_club(1350).unwrap();
        g.fill_squad(&mut bayern).unwrap();
        assert!(bayern.players.iter().any(|p| p.id == 123625 && p.loaned_out));
        // O treinador renova-o e chama-o de volta (o Zaragoza, no Espanyol)
        let v = g.transfer_view(123625).unwrap();
        assert!(v.can_renew && v.can_recall, "emprestado pelo clube");
        assert!(matches!(g.contract_party(123625).unwrap().2, super::super::deals::Party::Renewal));
        let mut g = g;
        assert!(g.recall_loan(123625).unwrap().contains("chamado de volta"));
        assert_eq!(g.world.conn.query_row("SELECT club_id FROM people WHERE id = 123625", [], |r| r.get::<_, i32>(0)).unwrap(), 1350);
        assert!(g.recall_loan(123625).is_err(), "já voltou");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);

        // Começando em 2026/27, os empréstimos continuam, um ano mais tarde
        let dir = std::env::temp_dir().join(format!("nextcm-emprestimos-2026-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let g = Game::create(&path, &dir, &crate::NewGame { start: crate::StartSeason::Europe, ..new }, 3).unwrap();
        assert_eq!(at(&g, 26601), (Some(10294), 6271, "2027-06-15".to_string()));
        // O Zaragoza e o Boey também, no jogo do Bayern a começar em 2026/27
        assert_eq!(at(&g, 123625), (Some(espanyol), 1350, "2027-06-15".to_string()));
        assert_eq!(at(&g, 36829).1, 1350);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
