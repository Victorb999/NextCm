//! Contratos sem data de fim (pedido de 17/11, o Andrey do Vasco U21): a base do CM traz uns 19
//! mil jogadores com salário e sem data de fim, quase todos de equipas sub-19/sub-21 e de clubes
//! pequenos. Contavam como contratados para sempre ("Contrato até -", valor nulo). Passam a ter
//! um contrato de 1 a 3 épocas do clube, sorteado fixo por jogador, e entram nas renovações como
//! os outros. Corre no jogo novo e ao abrir um save (não muda nada quando já não há nenhum).

use rusqlite::params;

use super::Game;
use super::market::contract_until;
use crate::Result;

/// Épocas de contrato pela idade e por um número fixo do jogador: jovens 2–3, até 28 anos 1–3, depois 1–2.
fn years_for(age: i32, pick: u64) -> i32 {
    match age {
        ..=21 => 2 + (pick % 2) as i32,
        22..=28 => 1 + (pick % 3) as i32,
        _ => 1 + (pick % 2) as i32,
    }
}

impl Game {
    /// Dá a data de fim aos contratos com salário e sem ela. Devolve quantos mudaram.
    pub(super) fn date_open_contracts(&mut self) -> Result<usize> {
        let year: i32 = self.date[0..4].parse().unwrap_or(0);
        let rows: Vec<(i32, Option<i32>, Option<i32>)> = self
            .world
            .conn
            .prepare(
                "SELECT p.id, c.division_id, p.birth_year FROM people p JOIN clubs c ON c.id = p.club_id
                 WHERE p.is_player = 1 AND p.contract_expires IS NULL AND COALESCE(p.wage, 0) > 0
                   AND p.id NOT IN (SELECT person_id FROM retired) AND p.id NOT IN (SELECT person_id FROM ghosts)",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        if rows.is_empty() {
            return Ok(0);
        }
        let mut stmt = self.world.conn.prepare("UPDATE people SET contract_expires = ? WHERE id = ?")?;
        for &(id, division, born) in &rows {
            let age = born.map_or(25, |b| year - b);
            let pick = (self.seed as u64 ^ (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)).rotate_left(17);
            let until = contract_until(self.club_kind(division), &self.date, years_for(age, pick));
            stmt.execute(params![until, id])?;
        }
        drop(stmt);
        self.unsaved = true;
        Ok(rows.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_years_by_age() {
        for pick in 0..12 {
            assert!((2..=3).contains(&years_for(18, pick)));
            assert!((1..=3).contains(&years_for(25, pick)));
            assert!((1..=2).contains(&years_for(33, pick)));
        }
    }
}
