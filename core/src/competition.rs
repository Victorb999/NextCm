use rusqlite::OptionalExtension;
use serde::Serialize;
use ts_rs::TS;

use crate::person::ClubRef;
use crate::{Error, Result, World};

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ContinentRef {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionListItem {
    pub id: i32,
    pub name: String,
    pub short_name: String,
    /// "club" ou "national"
    pub kind: String,
    pub nation_id: Option<i32>,
    pub nation: Option<String>,
    /// Continente da competição ou, nas nacionais, o da nação.
    pub continent_id: Option<i32>,
    pub reputation: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClubListItem {
    pub club: ClubRef,
    pub nation: Option<String>,
    pub division: Option<String>,
    pub reputation: Option<i32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub year: Option<i32>,
    pub winner: Option<ClubRef>,
    pub runner_up: Option<ClubRef>,
    pub third: Option<ClubRef>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionProfile {
    pub id: i32,
    pub name: String,
    pub short_name: String,
    pub kind: String,
    pub nation_id: Option<i32>,
    pub nation: Option<String>,
    pub continent: Option<String>,
    pub reputation: i32,
    /// Clubes que têm esta competição como divisão ou estadual.
    pub clubs: Vec<ClubListItem>,
    pub history: Vec<HistoryEntry>,
}

impl World {
    pub fn list_continents(&self) -> Result<Vec<ContinentRef>> {
        let mut stmt = self.conn.prepare_cached("SELECT id, name FROM continents ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok(ContinentRef { id: r.get(0)?, name: r.get(1)? }))?.collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Todas as competições relevantes (reputação > 1), para a tela "Competições".
    pub fn list_competitions(&self) -> Result<Vec<CompetitionListItem>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT c.id, c.name, c.short_name, c.kind, c.nation_id, n.name,
                    COALESCE(c.continent_id, n.continent_id), c.reputation
             FROM competitions c
             LEFT JOIN nations n ON n.id = c.nation_id
             WHERE c.reputation > 1
             ORDER BY n.name, c.reputation DESC, c.name",
        )?;
        let rows = stmt
            .query_map([], |r| {
                let name: String = r.get(1)?;
                let short: String = r.get(2)?;
                Ok(CompetitionListItem {
                    id: r.get(0)?,
                    short_name: if short.is_empty() { name.clone() } else { short },
                    name,
                    kind: r.get(3)?,
                    nation_id: r.get(4)?,
                    nation: r.get(5)?,
                    continent_id: r.get(6)?,
                    reputation: r.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }

    /// Clubes de uma consulta `SELECT c.id ... ORDER BY ...` com nação, divisão e reputação.
    pub(crate) fn club_list(&self, sql_where: &str, id: i32) -> Result<Vec<ClubListItem>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT c.id, n.name, d.short_name, c.reputation
             FROM clubs c
             LEFT JOIN nations n ON n.id = c.nation_id
             LEFT JOIN competitions d ON d.id = c.division_id
             WHERE {sql_where}
             ORDER BY c.reputation DESC, c.name"
        ))?;
        let rows: Vec<(i32, Option<String>, Option<String>, Option<i32>)> =
            stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::with_capacity(rows.len());
        for (club_id, nation, division, reputation) in rows {
            if let Some(club) = self.club_ref(Some(club_id))? {
                out.push(ClubListItem { club, nation, division, reputation });
            }
        }
        Ok(out)
    }

    pub fn get_competition(&self, id: i32) -> Result<CompetitionProfile> {
        let base = self
            .conn
            .prepare_cached(
                "SELECT c.id, c.name, c.short_name, c.kind, c.nation_id, n.name, ct.name, c.reputation
                 FROM competitions c
                 LEFT JOIN nations n ON n.id = c.nation_id
                 LEFT JOIN continents ct ON ct.id = COALESCE(c.continent_id, n.continent_id)
                 WHERE c.id = ?",
            )?
            .query_row([id], |r| {
                let name: String = r.get(1)?;
                let short: String = r.get(2)?;
                Ok(CompetitionProfile {
                    id: r.get(0)?,
                    short_name: if short.is_empty() { name.clone() } else { short },
                    name,
                    kind: r.get(3)?,
                    nation_id: r.get(4)?,
                    nation: r.get(5)?,
                    continent: r.get(6)?,
                    reputation: r.get(7)?,
                    clubs: Vec::new(),
                    history: Vec::new(),
                })
            })
            .optional()?;
        let Some(mut comp) = base else {
            return Err(Error::NotFound(format!("competição {id}")));
        };

        comp.clubs = self.club_list("c.is_national_team = 0 AND ?1 IN (c.division_id, c.reserve_division_id)", id)?;

        let rows: Vec<(Option<i32>, Option<i32>, Option<i32>, Option<i32>)> = self
            .conn
            .prepare_cached(
                "SELECT year, winner_id, runner_up_id, third_id FROM competition_history
                 WHERE competition_id = ? ORDER BY year DESC",
            )?
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (year, winner, runner_up, third) in rows {
            comp.history.push(HistoryEntry {
                year,
                winner: self.club_ref(winner)?,
                runner_up: self.club_ref(runner_up)?,
                third: self.club_ref(third)?,
            });
        }
        Ok(comp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> Option<World> {
        let path = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))?;
        Some(World::open(&path).unwrap())
    }

    #[test]
    fn serie_a_and_libertadores() {
        let Some(w) = world() else { return };
        let comps = w.list_competitions().unwrap();
        let serie_a = comps.iter().find(|c| c.name == "Campeonato Brasileiro Série A").unwrap();
        assert_eq!(serie_a.nation.as_deref(), Some("Brazil"));
        assert_eq!(serie_a.continent_id, Some(5));

        let a = w.get_competition(serie_a.id).unwrap();
        assert_eq!(a.clubs.len(), 20);
        assert!(a.clubs.iter().any(|c| c.club.short_name == "Flamengo"));

        let lib = comps.iter().find(|c| c.name == "Copa Libertadores de América").unwrap();
        let lib = w.get_competition(lib.id).unwrap();
        let last = &lib.history[0];
        assert_eq!(last.year, Some(2025));
        assert_eq!(last.winner.as_ref().unwrap().short_name, "Flamengo");
    }
}
