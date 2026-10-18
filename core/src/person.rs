use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, Row};
use serde::Serialize;
use ts_rs::TS;

use crate::attributes::displayed_attributes;
use crate::search::{NATURAL_POSITION, position_label};
use crate::{Error, Result, World};

/// Atributos por nome de coluna (`finishing`, `determination`...); `null` = não definido no banco.
pub type Attributes = BTreeMap<String, Option<i32>>;

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClubRef {
    pub id: i32,
    pub name: String,
    pub short_name: String,
    /// Cores do uniforme principal ('#rrggbb'), usadas na faixa de título.
    pub fg: Option<String>,
    pub bg: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PlayerDetails {
    pub squad_number: Option<i32>,
    pub current_ability: Option<i32>,
    pub potential_ability: Option<i32>,
    pub home_reputation: Option<i32>,
    pub current_reputation: Option<i32>,
    pub world_reputation: Option<i32>,
    /// Siglas, ex.: "MA/A DEC".
    pub position: String,
    /// Por extenso, ex.: "Médio Atacante/Avançado (Direita/Esquerda/Centro)".
    pub position_long: String,
    /// "Direito", "Esquerdo" ou "Qualquer".
    pub preferred_foot: Option<String>,
    pub morale: Option<i32>,
    /// Atributos 1–20 como o perfil do CM mostra: os 18 relativos já convertidos com o CA
    /// (ver `attributes.rs`), os demais como no banco.
    pub attributes: Attributes,
    /// Valores do banco, sem conversão: técnicos/físicos (tabela players) + mentais (people).
    pub raw_attributes: Attributes,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct StaffDetails {
    pub current_ability: Option<i32>,
    pub potential_ability: Option<i32>,
    pub attributes: Attributes,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CareerEntry {
    pub year: Option<i32>,
    pub club: Option<ClubRef>,
    pub on_loan: bool,
    pub apps: i32,
    pub goals: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PersonProfile {
    pub id: i32,
    pub name: String,
    pub list_name: String,
    pub birth_date: Option<String>,
    pub birth_year: Option<i32>,
    pub age: Option<i32>,
    pub nation: Option<String>,
    pub nationality: Option<String>,
    pub second_nation: Option<String>,
    pub international_apps: i32,
    pub international_goals: i32,
    pub club: Option<ClubRef>,
    pub job: Option<String>,
    pub club_joined: Option<String>,
    pub contract_expires: Option<String>,
    pub wage: Option<i32>,
    pub player: Option<PlayerDetails>,
    pub staff: Option<StaffDetails>,
    pub career: Vec<CareerEntry>,
}

const MENTAL: [&str; 8] = [
    "adaptability", "ambition", "determination", "loyalty",
    "pressure", "professionalism", "sportsmanship", "temperament",
];

/// Posições por extenso, como no CM 01/02 em português.
fn position_long(p: &[Option<i32>; 12]) -> String {
    let natural = |i: usize| p[i].unwrap_or(0) >= NATURAL_POSITION;
    if natural(0) {
        return "Guarda-Redes".into();
    }
    let roles: Vec<&str> = [
        (1, "Líbero"), (2, "Defesa"), (7, "Ala"), (3, "Médio Defensivo"),
        (4, "Médio"), (5, "Médio Atacante"), (6, "Avançado"),
    ]
    .into_iter()
    .filter(|(i, _)| natural(*i))
    .map(|(_, label)| label)
    .collect();
    let sides: Vec<&str> = [(8, "Direita"), (9, "Esquerda"), (10, "Centro")]
        .into_iter()
        .filter(|(i, _)| natural(*i))
        .map(|(_, label)| label)
        .collect();
    match (roles.is_empty(), sides.is_empty()) {
        (true, _) => String::new(),
        (false, true) => roles.join("/"),
        (false, false) => format!("{} ({})", roles.join("/"), sides.join("/")),
    }
}

fn preferred_foot(left: Option<i32>, right: Option<i32>) -> Option<String> {
    let (l, r) = (left?, right?);
    Some(
        if l >= NATURAL_POSITION && r >= NATURAL_POSITION {
            "Qualquer"
        } else if r >= l {
            "Direito"
        } else {
            "Esquerdo"
        }
        .into(),
    )
}

/// Lê todas as colunas de `row`, menos as listadas em `skip`, como atributos.
fn attributes_from(row: &Row, skip: &[&str]) -> rusqlite::Result<Attributes> {
    let mut out = Attributes::new();
    for (i, name) in row.as_ref().column_names().iter().enumerate() {
        if !skip.contains(name) {
            out.insert((*name).to_string(), row.get(i)?);
        }
    }
    Ok(out)
}

impl World {
    fn club_ref(&self, id: Option<i32>) -> Result<Option<ClubRef>> {
        let Some(id) = id else { return Ok(None) };
        Ok(self
            .conn
            .prepare_cached("SELECT id, name, short_name, home_fg, home_bg FROM clubs WHERE id = ?")?
            .query_row([id], |r| {
                let name: String = r.get(1)?;
                let short: String = r.get(2)?;
                Ok(ClubRef {
                    id: r.get(0)?,
                    short_name: if short.is_empty() { name.clone() } else { short },
                    name,
                    fg: r.get(3)?,
                    bg: r.get(4)?,
                })
            })
            .optional()?)
    }

    /// Idade em 1º de julho do ano inicial do jogo.
    fn age_from(&self, birth_date: Option<&str>, birth_year: Option<i32>) -> Option<i32> {
        match birth_date {
            Some(d) => {
                let year: i32 = d.get(0..4)?.parse().ok()?;
                let after_july = d.get(5..10)? > "07-01";
                Some(self.start_year - year - after_july as i32)
            }
            None => birth_year.map(|y| self.start_year - y),
        }
    }

    pub fn get_person(&self, id: i32) -> Result<PersonProfile> {
        let base = self
            .conn
            .prepare_cached(
                "SELECT p.id, p.display_name, p.list_name, p.birth_date, p.birth_year,
                        n.name, n.nationality, n2.name,
                        p.international_apps, p.international_goals,
                        p.club_id, j.name, p.club_joined, p.contract_expires, p.wage
                 FROM people p
                 LEFT JOIN nations n ON n.id = p.nation_id
                 LEFT JOIN nations n2 ON n2.id = p.second_nation_id
                 LEFT JOIN jobs j ON j.id = p.club_job
                 WHERE p.id = ?",
            )?
            .query_row([id], |r| {
                Ok((
                    PersonProfile {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        list_name: r.get(2)?,
                        birth_date: r.get(3)?,
                        birth_year: r.get(4)?,
                        age: None,
                        nation: r.get(5)?,
                        nationality: r.get(6)?,
                        second_nation: r.get(7)?,
                        international_apps: r.get(8)?,
                        international_goals: r.get(9)?,
                        club: None,
                        job: r.get(11)?,
                        club_joined: r.get(12)?,
                        contract_expires: r.get(13)?,
                        wage: r.get::<_, Option<i32>>(14)?.filter(|w| *w > 0),
                        player: None,
                        staff: None,
                        career: Vec::new(),
                    },
                    r.get::<_, Option<i32>>(10)?,
                ))
            })
            .optional()?;
        let Some((mut person, club_id)) = base else {
            return Err(Error::NotFound(format!("pessoa {id}")));
        };
        person.age = self.age_from(person.birth_date.as_deref(), person.birth_year);
        person.club = self.club_ref(club_id)?;

        let mental: Attributes = self
            .conn
            .prepare_cached(&format!("SELECT {} FROM people WHERE id = ?", MENTAL.join(", ")))?
            .query_row([id], |r| attributes_from(r, &[]))?;

        person.player = self
            .conn
            .prepare_cached("SELECT * FROM players WHERE person_id = ?")?
            .query_row([id], |r| {
                let mut pos = [None; 12];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = r.get(7 + i)?; // pos_* vêm logo após as reputações
                }
                let mut attributes = attributes_from(
                    r,
                    &["person_id", "squad_number", "current_ability", "potential_ability",
                      "home_reputation", "current_reputation", "world_reputation", "morale"],
                )?;
                attributes.retain(|k, _| !k.starts_with("pos_"));
                let foot = preferred_foot(
                    attributes.get("left_foot").copied().flatten(),
                    attributes.get("right_foot").copied().flatten(),
                );
                attributes.extend(mental.clone());
                let current_ability = r.get("current_ability")?;
                Ok(PlayerDetails {
                    squad_number: r.get("squad_number")?,
                    current_ability,
                    potential_ability: r.get("potential_ability")?,
                    home_reputation: r.get("home_reputation")?,
                    current_reputation: r.get("current_reputation")?,
                    world_reputation: r.get("world_reputation")?,
                    position: position_label(&pos),
                    position_long: position_long(&pos),
                    preferred_foot: foot,
                    morale: r.get("morale")?,
                    attributes: displayed_attributes(&attributes, current_ability, pos[0]),
                    raw_attributes: attributes,
                })
            })
            .optional()?;

        person.staff = self
            .conn
            .prepare_cached("SELECT * FROM non_players WHERE person_id = ?")?
            .query_row([id], |r| {
                let mut attributes = attributes_from(
                    r,
                    &["person_id", "current_ability", "potential_ability", "home_reputation",
                      "current_reputation", "world_reputation", "preferred_formation"],
                )?;
                attributes.extend(mental.clone());
                Ok(StaffDetails {
                    current_ability: r.get("current_ability")?,
                    potential_ability: r.get("potential_ability")?,
                    attributes,
                })
            })
            .optional()?;

        let rows: Vec<(Option<i32>, Option<i32>, i32, i32, i32)> = self
            .conn
            .prepare_cached(
                "SELECT year, club_id, on_loan, apps, goals FROM career_history
                 WHERE person_id = ? ORDER BY year DESC, apps DESC",
            )?
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (year, club_id, on_loan, apps, goals) in rows {
            person.career.push(CareerEntry {
                year,
                club: self.club_ref(club_id)?,
                on_loan: on_loan != 0,
                apps,
                goals,
            });
        }

        Ok(person)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_positions() {
        let mut p = [Some(1); 12];
        p[5] = Some(15);
        p[6] = Some(20);
        p[8] = Some(20);
        p[9] = Some(18);
        p[10] = Some(20);
        assert_eq!(position_long(&p), "Médio Atacante/Avançado (Direita/Esquerda/Centro)");
    }

    #[test]
    fn feet() {
        assert_eq!(preferred_foot(Some(20), Some(10)).as_deref(), Some("Esquerdo"));
        assert_eq!(preferred_foot(Some(8), Some(20)).as_deref(), Some("Direito"));
        assert_eq!(preferred_foot(Some(18), Some(17)).as_deref(), Some("Qualquer"));
    }

    #[test]
    fn lamine_yamal_profile() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        let id = w
            .search_people(&crate::SearchQuery { text: Some("lamine yamal".into()), ..Default::default() })
            .unwrap()
            .items[0]
            .id;
        let p = w.get_person(id).unwrap();
        assert_eq!(p.name, "Lamine Yamal");
        assert_eq!(p.birth_date.as_deref(), Some("2007-07-13"));
        assert_eq!(p.age, Some(17));
        let club = p.club.unwrap();
        assert_eq!(club.short_name, "Barcelona");
        assert_eq!(club.bg.as_deref(), Some("#0000a0"));
        let pl = p.player.unwrap();
        assert!(pl.attributes.contains_key("finishing"));
        assert!(pl.attributes.contains_key("determination"));
        assert!(!pl.attributes.keys().any(|k| k.starts_with("pos_")));
        assert!(!p.career.is_empty());
    }

    fn player(w: &World, name: &str) -> PlayerDetails {
        let q = crate::SearchQuery { text: Some(name.into()), ..Default::default() };
        let id = w.search_people(&q).unwrap().items[0].id;
        w.get_person(id).unwrap().player.unwrap()
    }

    /// Atributos convertidos (ver `attributes.rs`) de jogadores conhecidos do patch CLZ 25-26.
    /// No jogo, o Yamal aparece com Finalização 18 e Remates Longe 9: o CM simula ~6 meses
    /// (treino, evolução do CA) antes da 1ª tela, o que a conversão do banco não reproduz.
    #[test]
    fn known_players_attributes() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        let shown = |pl: &PlayerDetails, k: &str| pl.attributes[k].unwrap();
        let raw = |pl: &PlayerDetails, k: &str| pl.raw_attributes[k].unwrap();

        let yamal = player(&w, "lamine yamal");
        assert_eq!((raw(&yamal, "finishing"), raw(&yamal, "long_shots")), (11, 2));
        assert_eq!((shown(&yamal, "finishing"), shown(&yamal, "long_shots")), (16, 8));
        assert!((18 - shown(&yamal, "finishing")).abs() <= 2);
        assert!((9 - shown(&yamal, "long_shots")).abs() <= 2);
        assert_eq!(shown(&yamal, "dribbling"), 20);
        assert_eq!(shown(&yamal, "handling"), 1);
        assert_eq!(shown(&yamal, "pace"), raw(&yamal, "pace")); // físicos não mudam

        let haaland = player(&w, "erling haaland");
        assert_eq!(shown(&haaland, "finishing"), 20);
        assert_eq!(shown(&haaland, "positioning"), 11); // 1 no banco

        // GR: Jogo de Mãos e Reflexos pela fórmula normal; Finalização pela "low" (CA/10).
        let gk = player(&w, "marc-andre ter stegen");
        assert_eq!((shown(&gk, "handling"), shown(&gk, "reflexes")), (17, 20));
        assert_eq!(shown(&gk, "finishing"), 4);
    }
}

