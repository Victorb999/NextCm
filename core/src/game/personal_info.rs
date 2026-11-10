//! Informação pessoal do jogador, como a do FM08 (print de 9/11): os clubes e as pessoas de que
//! gosta e que detesta (as Preferências do CM), a personalidade e a reputação em palavras, se está
//! feliz no clube, os planos para já e as relações no plantel.

use serde::Serialize;
use ts_rs::TS;

use super::Game;
use crate::Result;
use crate::person::{ClubRef, PersonProfile};

/// Uma pessoa numa lista (para a ligação ao perfil).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PersonLink {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PersonalInfo {
    pub favoured_clubs: Vec<ClubRef>,
    pub disliked_clubs: Vec<ClubRef>,
    pub favoured_people: Vec<PersonLink>,
    pub disliked_people: Vec<PersonLink>,
    /// "Determinado", "Profissional", "Temperamental"...
    pub personality: String,
    /// "Mundial", "Continental", "Nacional", "Regional" ou "Local".
    pub reputation: String,
    /// "Está a gostar de jogar no clube", ou o primeiro motivo de insatisfação.
    pub happiness: String,
    /// "Sem planos imediatos para o futuro", "Pediu para ser transferido"...
    pub plans: String,
    /// As boas e as más relações no plantel ("Dá-se bem com X", "Não se dá com Y").
    pub relationships: Vec<String>,
}

/// A personalidade em palavras, pelos atributos mentais (o mais marcante primeiro).
fn personality_label(
    determination: i32,
    professionalism: i32,
    ambition: i32,
    loyalty: i32,
    temperament: i32,
    sportsmanship: i32,
) -> String {
    let mut out: Vec<&str> = Vec::new();
    if professionalism >= 17 {
        out.push("Profissional");
    }
    if determination >= 17 {
        out.push("Determinado");
    }
    if ambition >= 17 {
        out.push("Ambicioso");
    }
    if loyalty >= 17 {
        out.push("Leal");
    }
    if sportsmanship >= 17 {
        out.push("Desportivo");
    }
    if temperament <= 5 {
        out.push("Temperamental");
    }
    if professionalism <= 5 {
        out.push("Pouco profissional");
    }
    if out.is_empty() {
        out.push("Equilibrado");
    }
    out.truncate(2);
    out.join(", ")
}

/// A reputação em palavras (a mundial e a actual do banco, de 0 a 200).
fn reputation_label(world: i32, current: i32) -> &'static str {
    match world.max(current) {
        150.. => "Mundial",
        110.. => "Continental",
        70.. => "Nacional",
        35.. => "Regional",
        _ => "Local",
    }
}

impl Game {
    pub(super) fn fill_personal(&self, p: &mut PersonProfile) -> Result<()> {
        if p.player.is_none() {
            return Ok(());
        }
        let conn = &self.world.conn;
        let prefs: Vec<(String, i32, bool)> = conn
            .prepare_cached("SELECT kind, target_id, liked != 0 FROM preferences WHERE person_id = ? ORDER BY rowid")?
            .query_map([p.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut info = PersonalInfo::default();
        for (kind, target, liked) in prefs {
            if kind == "club" {
                let club = self.club(target);
                if liked { info.favoured_clubs.push(club) } else { info.disliked_clubs.push(club) }
            } else {
                let name = self.person_name(target)?;
                let link = PersonLink { id: target, name };
                if liked { info.favoured_people.push(link) } else { info.disliked_people.push(link) }
            }
        }
        let (det, pro, amb, loy, tem, spo, world, current, club): (i32, i32, i32, i32, i32, i32, i32, i32, Option<i32>) = conn.query_row(
            "SELECT COALESCE(p.determination, 10), COALESCE(p.professionalism, 10), COALESCE(p.ambition, 10), COALESCE(p.loyalty, 10),
                    COALESCE(p.temperament, 10), COALESCE(p.sportsmanship, 10), COALESCE(pl.world_reputation, 0),
                    COALESCE(pl.current_reputation, 0), p.club_id
             FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.id = ?",
            [p.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)),
        )?;
        info.personality = personality_label(det, pro, amb, loy, tem, spo);
        info.reputation = reputation_label(world, current).into();
        // A felicidade e os planos: os motivos (só no plantel do treinador) e a lista
        let reasons = self.unhappy_reasons(p.id)?;
        info.happiness = match reasons.first() {
            Some(r) => format!("{r}."),
            None if club.is_some() => "Está a gostar de jogar no clube".into(),
            None => "Procura clube".into(),
        };
        let (listed, requested): (bool, bool) = conn
            .query_row("SELECT transfer = 1, requested = 1 FROM transfer_list WHERE person_id = ?", [p.id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap_or((false, false));
        let ending = p
            .contract_expires
            .as_deref()
            .and_then(crate::date::parse)
            .zip(crate::date::parse(&self.date))
            .is_some_and(|(e, t)| e - t <= 180);
        info.plans = if requested {
            "Pediu para ser transferido".into()
        } else if listed {
            "Está na lista de transferências".into()
        } else if ending && club.is_some() {
            "O contrato acaba em breve: pensa no futuro".into()
        } else {
            "Sem planos imediatos para o futuro".into()
        };
        // As relações no plantel: de quem gosta e quem detesta entre os colegas
        if let Some(c) = club {
            let mates: Vec<(String, bool)> = conn
                .prepare_cached(
                    "SELECT q.display_name, f.liked != 0 FROM preferences f JOIN people q ON q.id = f.target_id
                     WHERE f.person_id = ? AND f.kind = 'person' AND q.club_id = ? ORDER BY f.liked DESC, q.display_name",
                )?
                .query_map([p.id, c], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            for (name, liked) in mates {
                info.relationships.push(if liked { format!("Dá-se bem com {name}") } else { format!("Não se dá com {name}") });
            }
        }
        p.personal = Some(info);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_from_the_mental_attributes() {
        assert_eq!(personality_label(18, 10, 10, 10, 10, 10), "Determinado");
        assert_eq!(personality_label(18, 18, 10, 10, 10, 10), "Profissional, Determinado");
        assert_eq!(personality_label(10, 10, 10, 10, 3, 10), "Temperamental");
        assert_eq!(personality_label(10, 10, 10, 10, 10, 10), "Equilibrado");
        assert_eq!(reputation_label(200, 200), "Mundial");
        assert_eq!(reputation_label(20, 40), "Regional");
    }
}
