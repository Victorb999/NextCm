//! A cidade do clube e a vontade do jogador de se mudar (4.0, itens 20 e 23): a atracção da
//! cidade do estádio (`cities.attraction`, do `city.dat`), que pesa mais em quem vem de fora
//! (o do país e o que fala a mesma língua sentem-na menos), e se o jogador seria titular.

use std::collections::HashMap;

use rusqlite::OptionalExtension;

use super::Game;
use crate::{Result, personality};

/// As línguas do futebol, pelas nações do banco (as que não estão aqui só contam o país).
const LANGUAGES: [(&str, &[&str]); 7] = [
    (
        "pt",
        &[
            "Brazil",
            "Portugal",
            "Angola",
            "Mozambique",
            "Cape Verde Islands",
            "Guinea-Bissau",
            "São Tomé & Principe",
            "Timor-Leste",
            "Macau",
        ],
    ),
    (
        "es",
        &[
            "Spain",
            "Argentina",
            "Uruguay",
            "Paraguay",
            "Chile",
            "Bolivia",
            "Peru",
            "Ecuador",
            "Colombia",
            "Venezuela",
            "Mexico",
            "Costa Rica",
            "Panama",
            "Honduras",
            "El Salvador",
            "Guatemala",
            "Nicaragua",
            "Cuba",
            "Dominican Republic",
            "Puerto Rico",
            "Equatorial Guinea",
        ],
    ),
    (
        "en",
        &[
            "England",
            "Scotland",
            "Wales",
            "Northern Ireland",
            "Republic of Ireland",
            "Great Britain",
            "United States",
            "Canada",
            "Australia",
            "New Zealand",
            "Jamaica",
            "Trinidad & Tobago",
            "South Africa",
            "Nigeria",
            "Ghana",
            "Kenya",
            "Uganda",
            "Zambia",
            "Zimbabwe",
            "Sierra Leone",
            "Liberia",
            "The Gambia",
        ],
    ),
    (
        "fr",
        &[
            "France",
            "Belgium",
            "Switzerland",
            "Luxembourg",
            "Monaco",
            "Senegal",
            "Ivory Coast",
            "Cameroon",
            "Mali",
            "Guinea",
            "Burkina Faso",
            "Gabon",
            "The Congo",
            "Democratic Republic of Congo",
            "Benin",
            "Togo",
            "Niger",
            "Chad",
            "Central African Republic",
            "Madagascar",
            "Haiti",
            "Martinique",
            "Guadeloupe",
            "French Guiana",
            "Réunion",
            "Comoros",
            "Djibouti",
            "Algeria",
            "Morocco",
            "Tunisia",
        ],
    ),
    ("de", &["Germany", "Austria", "Switzerland", "Liechtenstein", "Luxembourg"]),
    ("it", &["Italy", "San Marino", "Switzerland"]),
    ("nl", &["Netherlands", "Belgium", "Surinam", "Curaçao", "Aruba"]),
];

/// Falam uma língua em comum?
fn same_language(a: &str, b: &str) -> bool {
    LANGUAGES.iter().any(|(_, nations)| nations.contains(&a) && nations.contains(&b))
}

/// O peso da cidade pela nação do jogador e a do clube (`names`: os nomes das nações).
pub(super) fn nation_weight(names: &HashMap<i32, String>, player: i32, club: i32) -> f64 {
    match (names.get(&player), names.get(&club)) {
        _ if player == club && player > 0 => personality::CITY_HOME,
        (Some(a), Some(b)) if same_language(a, b) => personality::CITY_LANGUAGE,
        _ => 1.0,
    }
}

impl Game {
    /// Os nomes das nações (para as línguas).
    pub(super) fn nation_names(&self) -> Result<HashMap<i32, String>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT id, name FROM nations")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// A atracção da cidade do estádio do clube (0 sem dados).
    pub(super) fn city_attraction(&self, club: i32) -> Result<i32> {
        Ok(self
            .world
            .conn
            .query_row(
                "SELECT COALESCE(ci.attraction, 0) FROM clubs c JOIN stadiums s ON s.id = c.stadium_id
                 JOIN cities ci ON ci.id = s.city_id WHERE c.id = ?",
                [club],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    /// Quanto a cidade do `club` pesa para o jogador: 1 de fora, menos com a mesma língua e
    /// menos ainda no país dele.
    pub(super) fn city_weight(&self, person: i32, club: i32) -> Result<f64> {
        let (mine, theirs): (Option<String>, Option<String>) = self
            .world
            .conn
            .query_row(
                "SELECT np.name, nc.name FROM people p, clubs c LEFT JOIN nations np ON np.id = p.nation_id
                 LEFT JOIN nations nc ON nc.id = c.nation_id WHERE p.id = ?1 AND c.id = ?2",
                [person, club],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .unwrap_or((None, None));
        Ok(match (mine, theirs) {
            (Some(a), Some(b)) if a == b => personality::CITY_HOME,
            (Some(a), Some(b)) if same_language(&a, &b) => personality::CITY_LANGUAGE,
            _ => 1.0,
        })
    }

    /// O peso do `club` para este jogador: a reputação, as continentais e a cidade.
    pub(super) fn move_pull(&self, person: i32, club: i32) -> Result<i32> {
        let city = personality::city_pull(self.city_attraction(club)?, self.city_weight(person, club)?);
        Ok(self.club_pull(club)? + city)
    }

    /// Salário pedido × isto pela cidade do `club`.
    pub(super) fn city_wage(&self, person: i32, club: i32) -> Result<f64> {
        Ok(personality::city_wage(self.city_attraction(club)?, self.city_weight(person, club)?))
    }

    /// Um jogador com este CA seria dos 11 melhores do plantel do `club` (sem contar com ele)?
    pub(super) fn plays_at(&self, person: i32, ca: i32, club: i32) -> Result<bool> {
        let better: i64 = self.world.conn.query_row(
            "SELECT COUNT(*) FROM people p JOIN players pl ON pl.person_id = p.id
             WHERE p.club_id = ?1 AND p.is_player = 1 AND p.id <> ?2 AND pl.current_ability > ?3",
            [club, person, ca],
            |r| r.get(0),
        )?;
        Ok(better < 11)
    }

    /// A mudança do jogador do `from` para o `to`, como ele a vê.
    pub(super) fn player_move(&self, person: i32, ca: i32, age: i32, ambition: f64, from: i32, to: i32) -> Result<personality::Move> {
        Ok(personality::Move {
            from: self.club_pull(from)?,
            to: self.move_pull(person, to)?,
            ambition,
            age,
            plays_now: self.plays_at(person, ca, from)?,
            plays_there: self.plays_at(person, ca, to)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_group_nations() {
        assert!(same_language("Brazil", "Portugal"));
        assert!(same_language("Argentina", "Spain"));
        assert!(!same_language("Brazil", "Spain"));
        assert!(!same_language("Malta", "Italy"));
    }
}
