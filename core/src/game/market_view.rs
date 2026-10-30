//! Pesquisa e mercado como no CM (marco 3.13, passo 3): as etiquetas da coluna Inf da
//! pesquisa e a tela Transferências com os negócios do mundo, mês a mês.

use std::collections::HashSet;

use rusqlite::params;
use serde::Serialize;
use ts_rs::TS;

use super::Game;
use crate::search::PersonSummary;
use crate::{Result, TransferItem};

/// Negócios por página na tela Transferências.
const WORLD_PAGE: i64 = 200;

/// Uma página da tela Transferências.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct WorldTransfers {
    pub items: Vec<TransferItem>,
    pub total: i32,
}

impl Game {
    /// Os jogadores lesionados hoje (para o filtro Lesões da pesquisa).
    pub fn injured_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self
            .squads
            .states
            .iter()
            .filter(|(_, s)| s.injury_until.as_deref().is_some_and(|d| d > self.date.as_str()))
            .map(|(&id, _)| id)
            .collect();
        ids.sort_unstable();
        ids
    }

    /// A coluna Inf: Pro (proposta em curso), Les, Sus, Ped (pediu para sair), Lst (lista de
    /// transferências), Emp (emprestado), Amr (dois amarelos ou mais).
    pub fn tag_people(&self, items: &mut [PersonSummary]) -> Result<()> {
        let set = |sql: &str| -> Result<HashSet<i32>> {
            Ok(self.world.conn.prepare_cached(sql)?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
        };
        let offers = set(
            "SELECT person_id FROM offers WHERE kind IN ('transfer', 'loan', 'contract') AND status IN ('pending', 'countered', 'agreed', 'raised', 'waiting')",
        )?;
        let listed = set("SELECT person_id FROM transfer_list WHERE transfer = 1")?;
        let requested = set("SELECT person_id FROM transfer_list WHERE requested = 1")?;
        let loans = set("SELECT person_id FROM loans")?;
        let ghosts = set("SELECT person_id FROM ghosts")?;
        for p in items.iter_mut() {
            let st = self.squads.states.get(&p.id);
            let mut tags = Vec::new();
            if ghosts.contains(&p.id) {
                tags.push("Ench");
            }
            if offers.contains(&p.id) {
                tags.push("Pro");
            }
            if st.is_some_and(|s| s.injury_until.as_deref().is_some_and(|d| d > self.date.as_str())) {
                tags.push("Les");
            }
            if st.is_some_and(|s| s.suspended_in(None) > 0) {
                tags.push("Sus");
            }
            if requested.contains(&p.id) {
                tags.push("Ped");
            } else if listed.contains(&p.id) {
                tags.push("Lst");
            }
            if loans.contains(&p.id) {
                tags.push("Emp");
            }
            if st.is_some_and(|s| s.yellows() >= 2) {
                tags.push("Amr");
            }
            p.tags = tags.into_iter().map(String::from).collect();
        }
        Ok(())
    }

    /// "Transferências": os negócios do mundo num mês ("2025-08"), dos mais caros para os mais
    /// baratos, e quantos houve.
    pub fn world_transfers(&self, month: &str, page: i64) -> Result<WorldTransfers> {
        let like = format!("{month}-%");
        let total: i64 = self.world.conn.query_row(
            "SELECT COUNT(*) FROM transfers WHERE date LIKE ? AND kind IN ('transfer', 'free', 'loan')",
            [&like],
            |r| r.get(0),
        )?;
        let rows: Vec<(String, i32, Option<i32>, Option<i32>, i64, String)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT date, person_id, from_club, to_club, fee, kind FROM transfers
                 WHERE date LIKE ? AND kind IN ('transfer', 'free', 'loan') ORDER BY fee DESC, date, id LIMIT ? OFFSET ?",
            )?
            .query_map(params![like, WORLD_PAGE, page.max(0) * WORLD_PAGE], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let items = rows
            .into_iter()
            .map(|(date, id, from, to, fee, kind)| {
                Ok(TransferItem {
                    date,
                    person_id: id,
                    name: self.person_name(id)?,
                    from: from.map(|c| self.club(c)),
                    to: to.map(|c| self.club(c)),
                    fee,
                    kind,
                })
            })
            .collect::<Result<_>>()?;
        Ok(WorldTransfers { items, total: total as i32 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::{PersonKind, SearchQuery};

    #[test]
    fn tags_filters_and_world_transfers() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-inf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Inf".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
        };
        let mut g = Game::create(&path, &dir, &new, 19).unwrap();
        for _ in 0..20 {
            g.advance_day().unwrap();
        }
        // Os empréstimos do início têm "Emp"
        let q = SearchQuery { text: Some("Onana".into()), kind: Some(PersonKind::Players), ..Default::default() };
        let mut r = g.world.search_people_on(&q, &g.date).unwrap();
        g.tag_people(&mut r.items).unwrap();
        assert!(r.items.iter().any(|p| p.id == 26601 && p.tags.contains(&"Emp".to_string())));
        // Contrato a acabar e sem o próprio plantel
        let q = SearchQuery { kind: Some(PersonKind::Players), expiring: Some(true), exclude_club: Some(7568), ..Default::default() };
        let r = g.world.search_people_on(&q, &g.date).unwrap();
        assert!(r.total > 0 && r.items.iter().all(|p| p.club_id != Some(7568)));
        // As transferências do mês
        let w = g.world_transfers(&g.date[0..7], 0).unwrap();
        assert!(w.total > 0 && w.items.len() as i32 <= w.total);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
