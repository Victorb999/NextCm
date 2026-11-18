//! Pesquisa e mercado como no CM (marco 3.13, passo 3): as etiquetas da coluna Inf da
//! pesquisa e a tela Transferências com os negócios do mundo, mês a mês.

use std::collections::HashSet;

use rusqlite::params;
use serde::Serialize;
use ts_rs::TS;

use super::Game;
use super::market::{EUROPE, leaving_europe};
use crate::search::PersonSummary;
use crate::{Result, TransferItem, personality};

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

    /// Filtro Interessados da pesquisa: um SELECT com os jogadores que recusam vir para o
    /// clube do treinador, pelas recusas de princípio do contrato: os de um clube muito maior
    /// (`step_down`) e os europeus que não saem da Europa (as idades de `leaves_europe`).
    pub fn unwilling_sql(&self) -> Result<Option<String>> {
        if self.sacked() {
            return Ok(None);
        }
        // O peso dos clubes: a reputação e as continentais desta época (`personality::steps_down`)
        let (club, rep) = (self.club_id, self.club_pull(self.club_id)?);
        let steps: String = (1..=20).map(|a| format!(" WHEN {a} THEN {}", personality::step_down(a as f64))).collect();
        let bonus: String = self.continental_bonuses().iter().map(|(c, b)| format!(" WHEN {c} THEN {b}")).collect();
        let bonus = if bonus.is_empty() { "0".to_string() } else { format!("CASE c.id{bonus} ELSE 0 END") };
        let mut sql = format!(
            "SELECT q.id FROM people q JOIN clubs c ON c.id = q.club_id
             WHERE q.club_id <> {club} AND COALESCE(c.reputation, 0) + {bonus} > {rep} + CASE MIN(MAX(COALESCE(q.ambition, 10), 1), 20){steps} END"
        );
        let (to, nation) = self.club_place(club)?;
        if leaving_europe(EUROPE, to) {
            let (year, day) = (&self.date[0..4], &self.date[5..10]);
            sql += &format!(
                " UNION SELECT id FROM (
                    SELECT q.id, q.nation_id AS nation,
                           {year} - CAST(substr(q.birth_sort, 1, 4) AS INTEGER) - (substr(q.birth_sort, 6, 5) > '{day}') AS age,
                           ROW_NUMBER() OVER (PARTITION BY q.club_id ORDER BY pl.current_ability DESC, q.id) AS n
                    FROM people q JOIN players pl ON pl.person_id = q.id
                    JOIN clubs c ON c.id = q.club_id JOIN nations k ON k.id = c.nation_id
                    WHERE q.is_player = 1 AND k.continent_id = {EUROPE})
                  WHERE NOT (age >= 31 OR (nation = {nation} AND age >= 27) OR (n > 11 AND age >= 29))"
            );
        }
        // As Preferências do CM: quem detesta o clube recusa; quem o tem por favorito aceita
        let sql = format!(
            "SELECT id FROM ({sql} UNION SELECT person_id FROM preferences WHERE kind = 'club' AND liked = 0 AND target_id = {club})
             WHERE id NOT IN (SELECT person_id FROM preferences WHERE kind = 'club' AND liked = 1 AND target_id = {club})"
        );
        Ok(Some(sql))
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
        let wanted: HashSet<i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT person_id FROM offers WHERE buyer <> ? AND kind IN ('transfer', 'loan', 'contract')
                 AND status IN ('pending', 'countered', 'agreed', 'raised', 'waiting', 'talks', 'confirm')",
            )?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let expired: HashSet<i32> = self
            .world
            .conn
            .prepare_cached("SELECT id FROM people WHERE club_id IS NOT NULL AND contract_expires IS NOT NULL AND contract_expires < ?")?
            .query_map([&self.date], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
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
            if expired.contains(&p.id) {
                tags.push("Ctr");
            }
            if wanted.contains(&p.id) {
                tags.push("Des");
            }
            if st.is_some_and(|s| s.yellows() >= 2) {
                tags.push("Amr");
            }
            p.tags = tags.into_iter().map(String::from).collect();
        }
        Ok(())
    }

    /// "Transferências": os negócios do mundo num mês ("2025-08"), dos mais caros para os mais
    /// baratos, e quantos houve. `kind` é o Vêr do CM: "transfer", "free", "loan" ou todos.
    pub fn world_transfers(&self, month: &str, page: i64, kind: Option<&str>) -> Result<WorldTransfers> {
        let like = format!("{month}-%");
        let kinds = match kind {
            Some(k @ ("transfer" | "free" | "loan")) => format!("'{k}'"),
            _ => "'transfer', 'free', 'loan'".to_string(),
        };
        let total: i64 = self.world.conn.query_row(
            &format!("SELECT COUNT(*) FROM transfers WHERE date LIKE ? AND kind IN ({kinds})"),
            [&like],
            |r| r.get(0),
        )?;
        let rows: Vec<(String, i32, Option<i32>, Option<i32>, i64, String)> = self
            .world
            .conn
            .prepare_cached(&format!(
                "SELECT date, person_id, from_club, to_club, fee, kind FROM transfers
                 WHERE date LIKE ? AND kind IN ({kinds}) ORDER BY fee DESC, date, id LIMIT ? OFFSET ?"
            ))?
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
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
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
        // Interessados: no Palmeiras, nem o Mbappé (clube muito maior, titular na Europa) nem
        // ninguém dos maiores clubes europeus; os livres e os do próprio plantel ficam
        let mbappe = SearchQuery { text: Some("Mbappe".into()), kind: Some(PersonKind::Players), ..Default::default() };
        assert!(g.world.search_people_on(&mbappe, &g.date).unwrap().total > 0);
        let unwilling = g.unwilling_sql().unwrap();
        let q = SearchQuery { interested: Some(true), unwilling: unwilling.clone(), ..mbappe };
        assert_eq!(g.world.search_people_on(&q, &g.date).unwrap().total, 0);
        let all = SearchQuery { kind: Some(PersonKind::Players), ..Default::default() };
        let total = g.world.search_people_on(&all, &g.date).unwrap().total;
        let q = SearchQuery { interested: Some(true), unwilling, limit: Some(200), ..all };
        let r = g.world.search_people_on(&q, &g.date).unwrap();
        assert!(r.total > 0 && r.total < total);
        assert!(r.items.iter().all(|p| p.club_id != Some(819) && p.club_id != Some(8383)), "nem Arsenal nem Real Madrid");
        // As transferências do mês
        let w = g.world_transfers(&g.date[0..7], 0, None).unwrap();
        assert!(w.total > 0 && w.items.len() as i32 <= w.total);
        // Ninguém com salário fica sem data de fim do contrato (o Andrey do Vasco U21)
        let open: i64 = g
            .world
            .conn
            .query_row(
                "SELECT COUNT(*) FROM people WHERE is_player = 1 AND club_id IS NOT NULL AND contract_expires IS NULL AND COALESCE(wage, 0) > 0
                 AND id NOT IN (SELECT person_id FROM retired) AND id NOT IN (SELECT person_id FROM ghosts)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(open, 0);
        // As seleções em português, com a sub-21 ligada à principal, e a nacionalidade abre a principal
        let name = |id: i32| -> String { g.world.conn.query_row("SELECT name FROM clubs WHERE id = ?", [id], |r| r.get(0)).unwrap() };
        assert_eq!(name(11663), "Itália");
        assert_eq!(name(11894), "Itália Sub-21");
        let senior = g.world.get_club(11663).unwrap();
        assert_eq!(senior.affiliates.iter().map(|c| c.id).collect::<Vec<_>>(), vec![11894]);
        assert_eq!(g.world.get_club(11894).unwrap().parent.map(|c| c.id), Some(11663));
        // A sub-21 mostra os melhores até 21 anos, sem os da principal
        let (mut u21, mut main) = (g.world.get_club(11894).unwrap(), senior);
        g.fill_squad(&mut u21).unwrap();
        g.fill_squad(&mut main).unwrap();
        assert!(u21.players.len() >= 18, "{}", u21.players.len());
        assert!(u21.players.iter().all(|p| p.age.is_none_or(|a| a <= 22)), "idades");
        assert!(u21.players.iter().all(|p| main.players.iter().all(|m| m.id != p.id)));
        let italian: i32 =
            g.world.conn.query_row("SELECT id FROM people WHERE nation_id = 95 AND is_player = 1 LIMIT 1", [], |r| r.get(0)).unwrap();
        assert_eq!(g.world.get_person(italian).unwrap().national_team_id, Some(11663));
        // O aposentado sai da Busca e o perfil mostra quando se aposentou
        let mbappe = SearchQuery { text: Some("Mbappe".into()), kind: Some(PersonKind::Players), ..Default::default() };
        let id = g.world.search_people_on(&mbappe, &g.date).unwrap().items[0].id;
        g.world.conn.execute("INSERT INTO retired (person_id, date) VALUES (?, ?)", rusqlite::params![id, g.date]).unwrap();
        assert!(g.world.search_people_on(&mbappe, &g.date).unwrap().items.iter().all(|p| p.id != id));
        let mut p = g.world.get_person(id).unwrap();
        g.mask_profile(&mut p).unwrap();
        assert_eq!(p.retired.as_deref(), Some(g.date.as_str()));
        assert!(!p.can_contract);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preferences_decide_who_refuses() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let world = crate::World::open(&path).unwrap();
        const RONALDO: i32 = 3522;
        const BARCELONA: i32 = 1283;
        const SPORTING: i32 = 9627;
        // As Preferências do CM: o Ronaldo gosta do Sporting e detesta o Barcelona
        assert_eq!(world.club_preference(RONALDO, SPORTING).unwrap(), Some(true));
        assert_eq!(world.club_preference(RONALDO, BARCELONA).unwrap(), Some(false));
        drop(world);
        let unwilling = |club: i32, name: &str| -> bool {
            let dir = std::env::temp_dir().join(format!("nextcm-pref-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            let new = crate::NewGame {
                save_name: name.into(),
                first_name: "Victor".into(),
                last_name: "Araujo".into(),
                birth_date: None,
                nation_id: None,
                club_id: club,
                leagues: Vec::new(),
                start: Default::default(),
                license: 0,
                real_players: true,
                masking: true,
                experience: 0,
                favourite_club: None,
            };
            let g = Game::create(&path, &dir, &new, 31).unwrap();
            let sql = g.unwilling_sql().unwrap().unwrap();
            let refuses: bool =
                g.world.conn.query_row(&format!("SELECT COUNT(*) > 0 FROM ({sql}) WHERE id = ?"), [RONALDO], |r| r.get(0)).unwrap();
            drop(g);
            let _ = std::fs::remove_dir_all(&dir);
            refuses
        };
        assert!(unwilling(BARCELONA, "barca"), "detesta o Barcelona");
        assert!(!unwilling(SPORTING, "sporting"), "vem para o Sporting");
    }
}
