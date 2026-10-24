//! Estado dos jogadores num jogo em curso (marco 3.3): condição física, moral, lesões,
//! cartões e suspensões. Desde o 3.4 o cansaço, os cartões e as lesões acontecem em campo
//! (`live.rs`); aqui fica o que eles deixam depois do jogo e a recuperação dia a dia.
//! Desde o 3.6 os cartões e os castigos contam por competição.

use std::collections::{BTreeMap, HashMap};

use rand_core::RngCore;
use serde::Serialize;
use ts_rs::TS;

use crate::engine::uniform;
use crate::personality;
use crate::tactics::{Availability, SquadState};
use crate::{Result, World, date};

/// Moral com que todos começam (o campo do banco está quase sempre vazio).
pub const START_MORALE: i32 = 12;
/// Condição com que um jogador volta de lesão.
const BACK_FROM_INJURY: i32 = 80;
/// Amarelos que dão um jogo de suspensão.
const YELLOWS_FOR_BAN: i32 = 3;

/// Amarelos e jogos de castigo por cumprir numa competição.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Cards {
    pub yellows: i32,
    pub suspended: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState {
    pub condition: i32,
    pub morale: i32,
    /// Por competição (id da competição → cartões).
    pub cards: BTreeMap<i32, Cards>,
    pub injury: Option<String>,
    /// Data ("AAAA-MM-DD") em que volta a estar disponível.
    pub injury_until: Option<String>,
    /// Jogos na época (conta para a evolução do CA).
    pub apps: i32,
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState { condition: 100, morale: START_MORALE, cards: BTreeMap::new(), injury: None, injury_until: None, apps: 0 }
    }
}

impl PlayerState {
    /// Para um jogo de `comp` (sem competição: castigado em alguma).
    pub fn availability(&self, comp: Option<i32>) -> Availability {
        Availability {
            condition: self.condition,
            morale: self.morale,
            injured: self.injury.is_some(),
            suspended: self.suspended_in(comp) > 0,
        }
    }

    /// Jogos de castigo numa competição (ou em todas).
    pub fn suspended_in(&self, comp: Option<i32>) -> i32 {
        match comp {
            Some(c) => self.cards.get(&c).map_or(0, |k| k.suspended),
            None => self.cards.values().map(|k| k.suspended).sum(),
        }
    }

    pub fn yellows(&self) -> i32 {
        self.cards.values().map(|k| k.yellows).sum()
    }

    /// Nova época das competições em que `starts` é verdade: os amarelos acabam; os
    /// castigos por cumprir ficam.
    pub fn clear_cards(&mut self, starts: impl Fn(i32) -> bool) {
        self.cards.retain(|&comp, k| k.suspended > 0 || !starts(comp));
        for (&comp, k) in self.cards.iter_mut() {
            if starts(comp) {
                k.yellows = 0;
            }
        }
    }
}

/// Clube, Forma Natural (1–20, do banco), que dá a recuperação diária, e Temperamento, que
/// mexe na moral depois dos jogos.
#[derive(Debug, Clone, Copy)]
pub struct Traits {
    pub club_id: i32,
    pub natural_fitness: i32,
    pub temperament: i32,
}

/// Para o perfil do jogador e a aba "Lesões & Castigos".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatus {
    pub condition: i32,
    pub morale: i32,
    /// "Excelente" … "Péssima"
    pub morale_label: String,
    pub yellows: i32,
    pub suspended: i32,
    pub injury: Option<String>,
    pub injury_until: Option<String>,
}

pub fn morale_label(m: i32) -> &'static str {
    match m {
        18.. => "Excelente",
        15..=17 => "Muito Boa",
        12..=14 => "Boa",
        9..=11 => "Razoável",
        6..=8 => "Fraca",
        3..=5 => "Muito Fraca",
        _ => "Péssima",
    }
}

/// O que aconteceu a um jogador num jogo (para as notícias do treinador).
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Injured { player: i32, club: i32, injury: String, days: i32 },
    Suspended { player: i32, club: i32, red: bool },
}

/// O que uma partida deixou num clube (vem de `LiveMatch::sides`).
#[derive(Debug, Clone, Default)]
pub struct Side {
    pub club: i32,
    /// Quem jogou (titulares e quem entrou) e a condição com que acabou.
    pub played: Vec<(i32, i32)>,
    /// Amarelos que contam para a suspensão (o duplo amarelo conta como vermelho).
    pub yellows: Vec<i32>,
    pub sent_off: Vec<i32>,
    pub injured: Vec<i32>,
    pub goals_for: u8,
    pub goals_against: u8,
}

const LIGHT: [&str; 5] = ["Distensão muscular", "Contusão na coxa", "Entorse no tornozelo", "Pancada no joelho", "Dores nas costas"];
const MEDIUM: [&str; 4] = ["Rotura muscular", "Lesão nos isquiotibiais", "Entorse grave no tornozelo", "Lesão na virilha"];
const SERIOUS: [&str; 3] = ["Rotura de ligamentos do joelho", "Fractura do perónio", "Lesão no menisco"];

fn pick<'a>(list: &[&'a str], rng: &mut impl RngCore) -> &'a str {
    list[(uniform(rng) * list.len() as f64) as usize % list.len()]
}

/// Estado de todos os jogadores de um conjunto de clubes.
pub struct Squads {
    pub states: HashMap<i32, PlayerState>,
    pub traits: HashMap<i32, Traits>,
}

impl Squads {
    /// Atributos dos jogadores destes clubes, lidos do banco.
    pub fn load_traits(world: &World, clubs: &[i32]) -> Result<HashMap<i32, Traits>> {
        let mut out = HashMap::new();
        let mut stmt = world.conn.prepare_cached(
            "SELECT p.id, p.club_id, pl.natural_fitness, p.temperament FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ?",
        )?;
        for &club in clubs {
            let rows = stmt.query_map([club], |r| {
                let nf = r.get::<_, Option<i32>>(2)?.unwrap_or(10).clamp(1, 20);
                let temperament = r.get::<_, Option<i32>>(3)?.unwrap_or(10).clamp(1, 20);
                Ok((r.get::<_, i32>(0)?, Traits { club_id: r.get(1)?, natural_fitness: nf, temperament }))
            })?;
            for row in rows {
                let (id, t) = row?;
                out.insert(id, t);
            }
        }
        Ok(out)
    }

    pub fn state(&self, id: i32) -> PlayerState {
        self.states.get(&id).cloned().unwrap_or_default()
    }

    fn state_mut(&mut self, id: i32) -> &mut PlayerState {
        self.states.entry(id).or_default()
    }

    /// Estado de quem é de `club` para um jogo de `comp`, para a escolha da equipa.
    pub fn squad_state(&self, club: i32, comp: Option<i32>) -> SquadState {
        self.traits.iter().filter(|(_, t)| t.club_id == club).map(|(&id, _)| (id, self.state(id).availability(comp))).collect()
    }

    pub fn status(&self, id: i32) -> Option<PlayerStatus> {
        self.traits.contains_key(&id).then(|| {
            let s = self.state(id);
            PlayerStatus {
                condition: s.condition,
                morale: s.morale,
                morale_label: morale_label(s.morale).into(),
                yellows: s.yellows(),
                suspended: s.suspended_in(None),
                injury: s.injury,
                injury_until: s.injury_until,
            }
        })
    }

    /// Um dia passa: recuperação física, fim das lesões e a moral a voltar ao normal
    /// (à segunda-feira). Devolve quem ficou recuperado de lesão hoje.
    pub fn new_day(&mut self, today: &str) -> Vec<i32> {
        let monday = date::weekday(today) == Some(1);
        let mut healed = Vec::new();
        let ids: Vec<i32> = self.traits.keys().copied().collect();
        for id in ids {
            let nf = self.traits[&id].natural_fitness;
            let s = self.state_mut(id);
            if let Some(until) = &s.injury_until {
                if until.as_str() <= today {
                    s.injury = None;
                    s.injury_until = None;
                    s.condition = s.condition.max(BACK_FROM_INJURY);
                    healed.push(id);
                }
                continue;
            }
            // 4% (Forma Natural 1) a 9% (20) por dia
            s.condition = (s.condition + 4 + nf / 4).min(100);
            if monday && s.morale != START_MORALE {
                s.morale += (START_MORALE - s.morale).signum();
            }
        }
        healed.sort_unstable();
        healed
    }

    /// Depois de uma partida de `comp`: suspensões cumpridas, condição com que acabaram, moral,
    /// lesões (a duração é sorteada aqui, com o RNG da partida) e cartões.
    pub fn after_match(&mut self, sides: [Side; 2], today: &str, comp: i32, rng: &mut impl RngCore) -> Vec<Event> {
        let mut events = Vec::new();
        for side in &sides {
            let played = |id: i32| side.played.iter().any(|p| p.0 == id);
            let mut squad: Vec<i32> = self.traits.iter().filter(|(_, t)| t.club_id == side.club).map(|(&id, _)| id).collect();
            squad.sort_unstable();
            let result = side.goals_for.cmp(&side.goals_against) as i32; // 1, 0, -1
            for &id in &squad {
                let temperament = self.traits[&id].temperament;
                let s = self.state_mut(id);
                // Quem estava suspenso nesta competição cumpriu este jogo
                if let Some(k) = s.cards.get_mut(&comp)
                    && k.suspended > 0
                    && !played(id)
                {
                    k.suspended -= 1;
                }
                let delta = if played(id) { 2 * result } else { result };
                s.morale = (s.morale + personality::morale_swing(delta, temperament)).clamp(1, 20);
            }
            for &(id, condition) in &side.played {
                if self.traits.contains_key(&id) {
                    let s = self.state_mut(id);
                    s.condition = condition.clamp(0, 100);
                    s.apps += 1;
                }
            }
            for &id in &side.injured {
                let roll = uniform(rng);
                let (list, min, max): (&[&str], i32, i32) = if roll < 0.70 {
                    (&LIGHT, 3, 14)
                } else if roll < 0.95 {
                    (&MEDIUM, 15, 45)
                } else {
                    (&SERIOUS, 46, 120)
                };
                let days = min + (uniform(rng) * (max - min + 1) as f64) as i32;
                let injury = pick(list, rng).to_string();
                let s = self.state_mut(id);
                s.injury = Some(injury.clone());
                s.injury_until = date::add_days(today, days);
                events.push(Event::Injured { player: id, club: side.club, injury, days });
            }
            for &id in &side.yellows {
                let k = self.state_mut(id).cards.entry(comp).or_default();
                k.yellows += 1;
                if k.yellows >= YELLOWS_FOR_BAN {
                    k.yellows = 0;
                    k.suspended += 1;
                    events.push(Event::Suspended { player: id, club: side.club, red: false });
                }
            }
            for &id in &side.sent_off {
                self.state_mut(id).cards.entry(comp).or_default().suspended += 1;
                events.push(Event::Suspended { player: id, club: side.club, red: true });
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;

    fn squads(n: i32, club: i32) -> Squads {
        let traits = (0..n).map(|id| (id, Traits { club_id: club, natural_fitness: 12, temperament: 11 })).collect();
        Squads { states: HashMap::new(), traits }
    }

    fn side(club: i32, played: &[i32], gf: u8, ga: u8) -> Side {
        Side { club, played: played.iter().map(|&id| (id, 76)).collect(), goals_for: gf, goals_against: ga, ..Side::default() }
    }

    #[test]
    fn condition_and_morale_after_the_match() {
        let mut sq = squads(20, 1);
        let starters: Vec<i32> = (0..11).collect();
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        sq.after_match([side(1, &starters, 1, 0), side(2, &[], 0, 1)], "2025-08-02", 65, &mut rng);
        assert_eq!(sq.state(0).condition, 76);
        assert_eq!(sq.state(15).condition, 100, "quem não jogou não cansa");
        assert_eq!(sq.state(0).morale, START_MORALE + 2);
        assert_eq!(sq.state(15).morale, START_MORALE + 1);
        for d in 3..7 {
            sq.new_day(&format!("2025-08-0{d}"));
        }
        assert_eq!(sq.state(0).condition, 100, "recupera numa semana");
    }

    #[test]
    fn temperament_swings_morale() {
        let mut sq = squads(20, 1);
        sq.traits.get_mut(&0).unwrap().temperament = 4;
        sq.traits.get_mut(&1).unwrap().temperament = 18;
        let starters: Vec<i32> = (0..11).collect();
        let mut rng = ChaCha8Rng::seed_from_u64(4);
        sq.after_match([side(1, &starters, 0, 2), side(2, &[], 2, 0)], "2025-08-02", 65, &mut rng);
        assert_eq!(sq.state(0).morale, START_MORALE - 3, "cabeça quente");
        assert_eq!(sq.state(1).morale, START_MORALE - 1, "calmo");
        assert_eq!(sq.state(2).morale, START_MORALE - 2);
    }

    #[test]
    fn cards_and_injuries_from_the_match() {
        let mut sq = squads(20, 1);
        let starters: Vec<i32> = (0..11).collect();
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        sq.state_mut(3).cards.insert(65, Cards { yellows: 2, suspended: 0 });
        let ours = Side { yellows: vec![3, 4], sent_off: vec![5], injured: vec![6], ..side(1, &starters, 0, 0) };
        let events = sq.after_match([ours, side(2, &[], 0, 0)], "2025-08-02", 65, &mut rng);
        assert_eq!(sq.state(3).suspended_in(Some(65)), 1, "terceiro amarelo");
        assert_eq!(sq.state(3).yellows(), 0);
        assert_eq!(sq.state(4).yellows(), 1);
        assert_eq!(sq.state(5).suspended_in(Some(65)), 1, "vermelho");
        // O castigo é só nesta competição
        assert!(sq.state(5).availability(Some(68)).can_play());
        assert!(!sq.state(5).availability(Some(65)).can_play());
        assert!(sq.state(6).injury.is_some() && sq.state(6).injury_until.is_some());
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn suspension_is_served_in_the_next_match() {
        let mut sq = squads(20, 1);
        sq.state_mut(5).cards.insert(65, Cards { yellows: 0, suspended: 1 });
        let starters: Vec<i32> = (0..5).chain(6..12).collect();
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        // Um jogo de outra competição não conta
        sq.after_match([side(1, &starters, 0, 1), side(2, &[], 1, 0)], "2025-08-02", 68, &mut rng);
        assert_eq!(sq.state(5).suspended_in(Some(65)), 1);
        sq.after_match([side(1, &starters, 0, 1), side(2, &[], 1, 0)], "2025-08-06", 65, &mut rng);
        assert_eq!(sq.state(5).suspended_in(Some(65)), 0);
        assert!(sq.state(5).availability(Some(65)).can_play());
    }

    #[test]
    fn injury_heals_on_its_date() {
        let mut sq = squads(3, 1);
        let s = sq.state_mut(1);
        s.injury = Some("Entorse no tornozelo".into());
        s.injury_until = Some("2025-08-10".into());
        s.condition = 40;
        assert!(sq.new_day("2025-08-09").is_empty());
        assert_eq!(sq.new_day("2025-08-10"), vec![1]);
        assert_eq!(sq.state(1).condition, BACK_FROM_INJURY);
    }
}
