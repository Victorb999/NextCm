use rusqlite::OptionalExtension;
use serde::Serialize;
use ts_rs::TS;

use crate::person::ClubRef;
use crate::search::position_label;
use crate::{Error, Result, World};

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionRef {
    pub id: i32,
    pub name: String,
    pub short_name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SquadPlayer {
    pub id: i32,
    /// "Sobrenome, Nome" ou o nome comum.
    pub list_name: String,
    pub squad_number: Option<i32>,
    pub age: Option<i32>,
    pub nation: Option<String>,
    /// Siglas do CM, ex.: "D DC".
    pub position: String,
    /// Para ordenar por posição como o CM: GR, defesas, médios, avançados.
    pub position_order: i32,
    pub international: bool,
    /// Lugar no plantel pela habilidade (1 = o melhor), para o critério "Habilidade" sem
    /// mostrar o CA, que é interno.
    pub ability_rank: i32,
    #[serde(skip)]
    #[ts(skip)]
    pub current_ability: Option<i32>,
    #[serde(skip)]
    #[ts(skip)]
    pub potential_ability: Option<i32>,
    /// Num jogo em curso: condição (0–100) e indisponibilidade.
    pub condition: Option<i32>,
    pub injured: bool,
    pub suspended: bool,
    /// Na equipa de reservas (só no clube do treinador, num jogo em curso).
    pub reserve: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct StaffMember {
    pub id: i32,
    pub list_name: String,
    pub job: Option<String>,
    pub job_id: Option<i32>,
    pub age: Option<i32>,
    pub nation: Option<String>,
    #[serde(skip)]
    #[ts(skip)]
    pub current_ability: Option<i32>,
}

/// Títulos e vice-campeonatos do clube numa competição (do historial das competições).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Honour {
    pub competition: CompetitionRef,
    /// Anos, do mais recente para o mais antigo.
    pub won: Vec<i32>,
    pub runner_up: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClubProfile {
    pub club: ClubRef,
    pub is_national_team: bool,
    pub nation_id: Option<i32>,
    pub nation: Option<String>,
    pub division: Option<CompetitionRef>,
    /// No patch CLZ: o campeonato estadual.
    pub reserve_division: Option<CompetitionRef>,
    /// Divisão e posição na temporada anterior.
    pub last_division: Option<CompetitionRef>,
    pub last_position: Option<i32>,
    pub stadium: Option<String>,
    pub stadium_capacity: Option<i32>,
    /// Cidade do estádio e lugares sentados.
    pub stadium_city: Option<String>,
    pub stadium_seated: Option<i32>,
    /// "Profissional", "Semi-profissional" ou "Amador".
    pub status: Option<String>,
    pub reputation: Option<i32>,
    pub cash: Option<i32>,
    pub rivals: Vec<ClubRef>,
    pub players: Vec<SquadPlayer>,
    pub staff: Vec<StaffMember>,
    /// Competições com título ou vice, as com mais títulos primeiro.
    pub honours: Vec<Honour>,
}

/// Ordem dos bits de `people.positions` (GR=1, D=2, AL=4, MD=8, M=16, MA=32, A=64):
/// a posição mais defensiva define o lugar na lista.
fn position_order(mask: i32) -> i32 {
    (0..7).find(|b| mask & (1 << b) != 0).unwrap_or(7)
}

impl World {
    fn competition_ref(&self, id: Option<i32>) -> Result<Option<CompetitionRef>> {
        let Some(id) = id else { return Ok(None) };
        Ok(self
            .conn
            .prepare_cached("SELECT id, name, short_name FROM competitions WHERE id = ?")?
            .query_row([id], |r| {
                let name: String = r.get(1)?;
                let short: String = r.get(2)?;
                Ok(CompetitionRef { id: r.get(0)?, short_name: if short.is_empty() { name.clone() } else { short }, name })
            })
            .optional()?)
    }

    pub fn get_club(&self, id: i32) -> Result<ClubProfile> {
        let Some(club) = self.club_ref(Some(id))? else {
            return Err(Error::NotFound(format!("clube {id}")));
        };
        type Row = (
            bool,
            Option<i32>,
            Option<String>,
            Option<i32>,
            Option<i32>,
            Option<String>,
            Option<i32>,
            Option<i32>,
            Option<i32>,
            [Option<i32>; 3],
            Option<i32>,
            Option<i32>,
        );
        let (is_national, nation_id, nation, division, reserve, stadium, capacity, reputation, cash, rivals, last_division, last_position): Row = self
            .conn
            .prepare_cached(
                "SELECT c.is_national_team, c.nation_id, n.name, c.division_id, c.reserve_division_id,
                        s.name, s.capacity, c.reputation, c.cash,
                        c.rival1_id, c.rival2_id, c.rival3_id, c.last_division_id, c.last_position
                 FROM clubs c
                 LEFT JOIN nations n ON n.id = c.nation_id
                 LEFT JOIN stadiums s ON s.id = c.stadium_id
                 WHERE c.id = ?",
            )?
            .query_row([id], |r| {
                Ok((
                    r.get::<_, i32>(0)? != 0,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    [r.get(9)?, r.get(10)?, r.get(11)?],
                    r.get(12)?,
                    r.get(13)?,
                ))
            })?;

        // Seleções: os jogadores ficam ligados por national_team_id, não por club_id.
        let link = if is_national { "p.national_team_id" } else { "p.club_id" };

        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT p.id, p.list_name, pl.squad_number, p.birth_date, p.birth_year, n.name,
                    p.positions, p.international_apps, pl.current_ability, pl.potential_ability,
                    pl.pos_goalkeeper, pl.pos_sweeper, pl.pos_defender, pl.pos_defensive_midfielder,
                    pl.pos_midfielder, pl.pos_attacking_midfielder, pl.pos_attacker, pl.pos_wing_back,
                    pl.pos_right_side, pl.pos_left_side, pl.pos_central, pl.pos_free_role
             FROM people p
             JOIN players pl ON pl.person_id = p.id
             LEFT JOIN nations n ON n.id = p.nation_id
             WHERE {link} = ?"
        ))?;
        let mut players = stmt
            .query_map([id], |r| {
                let mut pos = [None; 12];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = r.get(10 + i)?;
                }
                let birth_date: Option<String> = r.get(3)?;
                Ok(SquadPlayer {
                    id: r.get(0)?,
                    list_name: r.get(1)?,
                    squad_number: r.get(2)?,
                    age: self.age_from(birth_date.as_deref(), r.get(4)?),
                    nation: r.get(5)?,
                    position_order: position_order(r.get(6)?),
                    international: r.get::<_, i32>(7)? > 0,
                    ability_rank: 0,
                    current_ability: r.get(8)?,
                    potential_ability: r.get(9)?,
                    condition: None,
                    injured: false,
                    suspended: false,
                    reserve: false,
                    position: position_label(&pos),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut order: Vec<usize> = (0..players.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(players[i].current_ability.unwrap_or(0)), players[i].id));
        for (rank, i) in order.into_iter().enumerate() {
            players[i].ability_rank = rank as i32 + 1;
        }

        let job_link = if is_national { "p.national_team_job" } else { "p.club_job" };
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT p.id, p.list_name, j.name, j.id, p.birth_date, p.birth_year, n.name, np.current_ability
             FROM people p
             JOIN jobs j ON j.id = {job_link}
             LEFT JOIN nations n ON n.id = p.nation_id
             LEFT JOIN non_players np ON np.person_id = p.id
             WHERE {link} = ? AND j.id != 11
             ORDER BY j.id, p.list_name"
        ))?;
        let staff = stmt
            .query_map([id], |r| {
                let birth_date: Option<String> = r.get(4)?;
                Ok(StaffMember {
                    id: r.get(0)?,
                    list_name: r.get(1)?,
                    job: r.get(2)?,
                    job_id: r.get(3)?,
                    age: self.age_from(birth_date.as_deref(), r.get(5)?),
                    nation: r.get(6)?,
                    current_ability: r.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // Cidade do estádio, lugares sentados e estatuto profissional
        let (stadium_city, seated, professional_status): (Option<String>, Option<i32>, Option<i32>) = self
            .conn
            .prepare_cached(
                "SELECT ci.name, s.seating_capacity, c.professional_status FROM clubs c
                 LEFT JOIN stadiums s ON s.id = c.stadium_id LEFT JOIN cities ci ON ci.id = s.city_id
                 WHERE c.id = ?",
            )?
            .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;

        let mut rival_refs = Vec::new();
        for rival in rivals.into_iter().flatten() {
            rival_refs.extend(self.club_ref(Some(rival))?);
        }

        let mut honours: Vec<Honour> = Vec::new();
        let rows: Vec<(i32, i32, bool)> = self
            .conn
            .prepare_cached(
                "SELECT competition_id, year, winner_id = ?1 FROM competition_history
                 WHERE (winner_id = ?1 OR runner_up_id = ?1) AND year IS NOT NULL
                 ORDER BY year DESC",
            )?
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (comp, year, won) in rows {
            let i = match honours.iter().position(|h| h.competition.id == comp) {
                Some(i) => i,
                None => {
                    let Some(competition) = self.competition_ref(Some(comp))? else { continue };
                    honours.push(Honour { competition, won: Vec::new(), runner_up: Vec::new() });
                    honours.len() - 1
                }
            };
            // O historial do patch repete alguns anos; cada ano conta uma vez
            let years = if won { &mut honours[i].won } else { &mut honours[i].runner_up };
            if !years.contains(&year) {
                years.push(year);
            }
        }
        honours.sort_by(|a, b| b.won.len().cmp(&a.won.len()).then(b.runner_up.len().cmp(&a.runner_up.len())));

        Ok(ClubProfile {
            club,
            is_national_team: is_national,
            nation_id,
            nation,
            division: self.competition_ref(division)?,
            reserve_division: self.competition_ref(reserve)?,
            last_division: self.competition_ref(last_division)?,
            last_position: last_position.filter(|p| *p > 0),
            stadium,
            stadium_capacity: capacity.filter(|c| *c > 0),
            stadium_city,
            stadium_seated: seated.filter(|c| *c > 0),
            status: match professional_status {
                Some(1) => Some("Profissional".into()),
                Some(2) => Some("Semi-profissional".into()),
                Some(3) => Some("Amador".into()),
                _ => None,
            },
            reputation,
            cash,
            rivals: rival_refs,
            players,
            staff,
            honours,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_by_most_defensive_position() {
        assert_eq!(position_order(1), 0); // GR
        assert_eq!(position_order(2 | 64), 1); // D/A -> defesa
        assert_eq!(position_order(64 | 512), 6); // A
        assert_eq!(position_order(0), 7); // sem posição natural
    }

    #[test]
    fn barcelona_squad() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        let yamal =
            w.search_people(&crate::SearchQuery { text: Some("lamine yamal".into()), ..Default::default() }).unwrap().items.remove(0);
        let club = w.get_club(yamal.club_id.unwrap()).unwrap();
        assert_eq!(club.club.short_name, "Barcelona");
        assert_eq!(club.division.as_ref().map(|d| d.short_name.as_str()), Some("La Liga"));
        assert!(club.players.len() >= 20);
        assert!(club.players.iter().any(|p| p.id == yamal.id));
        assert!(club.staff.iter().any(|s| s.job_id == Some(5)), "tem treinador");
    }
}
