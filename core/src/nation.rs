use rusqlite::OptionalExtension;
use serde::Serialize;
use ts_rs::TS;

use crate::club::CompetitionRef;
use crate::competition::ClubListItem;
use crate::person::ClubRef;
use crate::{Error, Result, World};

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NationListItem {
    pub id: i32,
    pub name: String,
    pub continent_id: Option<i32>,
    pub clubs: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NationProfile {
    pub id: i32,
    pub name: String,
    pub nationality: Option<String>,
    pub code: Option<String>,
    pub continent: Option<String>,
    pub stadium: Option<String>,
    pub national_team: Option<ClubRef>,
    pub competitions: Vec<CompetitionRef>,
    /// Clubes que disputam alguma competição (divisão ou estadual), por reputação.
    pub clubs: Vec<ClubListItem>,
    /// Clubes sem divisão (amadores/sem liga no banco).
    pub other_clubs: i32,
    pub players: i32,
}

impl World {
    /// Nações com pelo menos um clube, para a tela "Nações & Clubes".
    pub fn list_nations_full(&self) -> Result<Vec<NationListItem>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT n.id, n.name, n.continent_id,
                    (SELECT COUNT(*) FROM clubs c WHERE c.nation_id = n.id AND c.is_national_team = 0) AS clubs
             FROM nations n
             WHERE clubs > 0
             ORDER BY n.name",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(NationListItem { id: r.get(0)?, name: r.get(1)?, continent_id: r.get(2)?, clubs: r.get(3)? })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    pub fn get_nation(&self, id: i32) -> Result<NationProfile> {
        let base = self
            .conn
            .prepare_cached(
                "SELECT n.id, n.name, n.nationality, n.code, ct.name, s.name
                 FROM nations n
                 LEFT JOIN continents ct ON ct.id = n.continent_id
                 LEFT JOIN stadiums s ON s.id = n.stadium_id
                 WHERE n.id = ?",
            )?
            .query_row([id], |r| {
                let blank = |s: Option<String>| s.filter(|s| !s.is_empty());
                Ok(NationProfile {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    nationality: blank(r.get(2)?),
                    code: blank(r.get(3)?),
                    continent: r.get(4)?,
                    stadium: r.get(5)?,
                    national_team: None,
                    competitions: Vec::new(),
                    clubs: Vec::new(),
                    other_clubs: 0,
                    players: 0,
                })
            })
            .optional()?;
        let Some(mut nation) = base else {
            return Err(Error::NotFound(format!("nação {id}")));
        };

        let team: Option<i32> = self
            .conn
            .prepare_cached("SELECT id FROM clubs WHERE nation_id = ? AND is_national_team = 1 LIMIT 1")?
            .query_row([id], |r| r.get(0))
            .optional()?;
        nation.national_team = self.club_ref(team)?;

        let mut stmt = self.conn.prepare_cached(
            "SELECT id, name, short_name FROM competitions
             WHERE nation_id = ? AND reputation > 1
             ORDER BY reputation DESC, name",
        )?;
        nation.competitions = stmt
            .query_map([id], |r| {
                let name: String = r.get(1)?;
                let short: String = r.get(2)?;
                Ok(CompetitionRef {
                    id: r.get(0)?,
                    short_name: if short.is_empty() { name.clone() } else { short },
                    name,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        nation.clubs = self.club_list(
            "c.nation_id = ?1 AND c.is_national_team = 0
             AND (c.division_id IS NOT NULL OR c.reserve_division_id IS NOT NULL)",
            id,
        )?;
        nation.other_clubs = self.conn.query_row(
            "SELECT COUNT(*) FROM clubs WHERE nation_id = ? AND is_national_team = 0
             AND division_id IS NULL AND reserve_division_id IS NULL",
            [id],
            |r| r.get(0),
        )?;
        nation.players = self.conn.query_row(
            "SELECT COUNT(*) FROM people WHERE nation_id = ? AND is_player = 1",
            [id],
            |r| r.get(0),
        )?;
        Ok(nation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brazil() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        let brazil = w.list_nations_full().unwrap().into_iter().find(|n| n.name == "Brazil").unwrap();
        let n = w.get_nation(brazil.id).unwrap();
        assert_eq!(n.continent.as_deref(), Some("South America"));
        assert!(n.national_team.is_some());
        assert!(n.competitions.iter().any(|c| c.name == "Campeonato Brasileiro Série A"));
        assert_eq!(n.clubs[0].club.short_name.is_empty(), false);
        assert!(n.clubs.iter().any(|c| c.club.short_name == "Palmeiras"));
    }
}
