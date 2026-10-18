//! Estado dos jogadores num jogo em curso (marco 3.3): condição física, moral, lesões,
//! cartões e suspensões. As regras estão no ROADMAP (detalhe do 3.3); os números são um
//! primeiro acerto, para calibrar com a CLI `sim`.

use std::collections::HashMap;

use rand_core::RngCore;
use serde::Serialize;
use ts_rs::TS;

use crate::engine::uniform;
use crate::tactics::{Availability, Instructions, SquadState, Tackling};
use crate::{Result, World, date};

/// Moral com que todos começam (o campo do banco está quase sempre vazio).
pub const START_MORALE: i32 = 12;
/// Condição com que um jogador volta de lesão.
const BACK_FROM_INJURY: i32 = 80;
/// Amarelos que dão um jogo de suspensão.
const YELLOWS_FOR_BAN: i32 = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerState {
    pub condition: i32,
    pub morale: i32,
    pub yellows: i32,
    /// Jogos de suspensão por cumprir.
    pub suspended: i32,
    pub injury: Option<String>,
    /// Data ("AAAA-MM-DD") em que volta a estar disponível.
    pub injury_until: Option<String>,
}

impl Default for PlayerState {
    fn default() -> Self {
        PlayerState { condition: 100, morale: START_MORALE, yellows: 0, suspended: 0, injury: None, injury_until: None }
    }
}

impl PlayerState {
    pub fn availability(&self) -> Availability {
        Availability {
            condition: self.condition,
            morale: self.morale,
            injured: self.injury.is_some(),
            suspended: self.suspended > 0,
        }
    }
}

/// Atributos (1–20, do banco) que mexem no estado.
#[derive(Debug, Clone, Copy)]
pub struct Traits {
    pub club_id: i32,
    pub stamina: i32,
    pub natural_fitness: i32,
    pub injury_proneness: i32,
    pub aggression: i32,
    pub dirtiness: i32,
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

/// Um lado de uma partida: o clube, quem jogou e as instruções.
pub struct Side<'a> {
    pub club: i32,
    pub starters: &'a [i32],
    pub instructions: Instructions,
    pub goals_for: u8,
    pub goals_against: u8,
}

const LIGHT: [&str; 5] = ["Distensão muscular", "Contusão na coxa", "Entorse no tornozelo", "Pancada no joelho", "Dores nas costas"];
const MEDIUM: [&str; 4] = ["Rotura muscular", "Lesão nos isquiotibiais", "Entorse grave no tornozelo", "Lesão na virilha"];
const SERIOUS: [&str; 3] = ["Rotura de ligamentos do joelho", "Fractura do perónio", "Lesão no menisco"];

/// Poisson pelo método de Knuth.
fn poisson(lambda: f64, rng: &mut impl RngCore) -> u32 {
    let limit = (-lambda).exp();
    let (mut k, mut p) = (0, uniform(rng));
    while p > limit && k < 12 {
        k += 1;
        p *= uniform(rng);
    }
    k
}

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
            "SELECT p.id, p.club_id, pl.stamina, pl.natural_fitness, pl.injury_proneness, pl.aggression, pl.dirtiness
             FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ?",
        )?;
        for &club in clubs {
            let rows = stmt.query_map([club], |r| {
                let v = |i: usize| r.get::<_, Option<i32>>(i).map(|v| v.unwrap_or(10).clamp(1, 20));
                Ok((r.get::<_, i32>(0)?, Traits {
                    club_id: r.get(1)?,
                    stamina: v(2)?,
                    natural_fitness: v(3)?,
                    injury_proneness: v(4)?,
                    aggression: v(5)?,
                    dirtiness: v(6)?,
                }))
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

    /// Estado de quem é de `club`, para a escolha da equipa.
    pub fn squad_state(&self, club: i32) -> SquadState {
        self.traits
            .iter()
            .filter(|(_, t)| t.club_id == club)
            .map(|(&id, _)| (id, self.state(id).availability()))
            .collect()
    }

    pub fn status(&self, id: i32) -> Option<PlayerStatus> {
        self.traits.contains_key(&id).then(|| {
            let s = self.state(id);
            PlayerStatus {
                condition: s.condition,
                morale: s.morale,
                morale_label: morale_label(s.morale).into(),
                yellows: s.yellows,
                suspended: s.suspended,
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

    /// Depois de uma partida: suspensões cumpridas, cansaço, moral, lesões e cartões.
    /// Sorteios com `rng` (o da partida, depois do resultado).
    pub fn after_match(&mut self, sides: [Side; 2], today: &str, rng: &mut impl RngCore) -> Vec<Event> {
        let mut events = Vec::new();
        for side in &sides {
            // Quem estava suspenso cumpriu este jogo
            let squad: Vec<i32> = self.traits.iter().filter(|(_, t)| t.club_id == side.club).map(|(&id, _)| id).collect();
            for &id in &squad {
                let s = self.state_mut(id);
                if s.suspended > 0 && !side.starters.contains(&id) {
                    s.suspended -= 1;
                }
            }
            let result = side.goals_for.cmp(&side.goals_against) as i32; // 1, 0, -1
            for &id in &squad {
                let played = side.starters.contains(&id);
                let s = self.state_mut(id);
                let delta = if played { 2 * result } else { result };
                s.morale = (s.morale + delta).clamp(1, 20);
            }
            for &id in side.starters {
                let Some(t) = self.traits.get(&id).copied() else { continue };
                // Cansaço: 20% (Resistência 20) a 28% (1), mais 3% com Pressão
                let drain = 28 - (t.stamina - 1) * 8 / 19 + if side.instructions.pressing { 3 } else { 0 };
                let s = self.state_mut(id);
                s.condition = (s.condition - drain).max(0);
                // Lesão: 0,6% (Propensão 1) a 1,8% (20) por titular e jogo
                let chance = 0.006 + 0.012 * (t.injury_proneness - 1) as f64 / 19.0;
                if uniform(rng) < chance {
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
                    s.injury = Some(injury.clone());
                    s.injury_until = date::add_days(today, days);
                    events.push(Event::Injured { player: id, club: side.club, injury, days });
                }
            }
            events.extend(self.cards(side, rng));
        }
        events
    }

    /// Amarelos (~2 por equipa) e vermelhos (~5%), mais com Desarme Duro e jogadores agressivos.
    fn cards(&mut self, side: &Side, rng: &mut impl RngCore) -> Vec<Event> {
        let mut events = Vec::new();
        let players: Vec<(i32, f64)> = side
            .starters
            .iter()
            .filter_map(|id| self.traits.get(id).map(|t| (*id, (t.aggression + t.dirtiness) as f64)))
            .collect();
        if players.is_empty() {
            return events;
        }
        let total: f64 = players.iter().map(|p| p.1).sum();
        // Jogadores mais agressivos que a média (~21) dão mais cartões
        let temper = total / players.len() as f64 / 21.0;
        let mult = match side.instructions.tackling {
            Tackling::Normal => 1.0,
            Tackling::Soft => 0.6,
            Tackling::Hard => 1.4,
        } * temper;
        let choose = |roll: f64| {
            let mut x = roll * total;
            for &(id, w) in &players {
                if x < w {
                    return id;
                }
                x -= w;
            }
            players[players.len() - 1].0
        };
        let mut booked = Vec::new();
        for _ in 0..poisson(1.9 * mult, rng) {
            let id = choose(uniform(rng));
            if booked.contains(&id) {
                continue; // segundo amarelo: no v1 conta como um só
            }
            booked.push(id);
            let s = self.state_mut(id);
            s.yellows += 1;
            if s.yellows >= YELLOWS_FOR_BAN {
                s.yellows = 0;
                s.suspended += 1;
                events.push(Event::Suspended { player: id, club: side.club, red: false });
            }
        }
        if uniform(rng) < 0.05 * mult {
            let id = choose(uniform(rng));
            self.state_mut(id).suspended += 1;
            events.push(Event::Suspended { player: id, club: side.club, red: true });
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
        let traits = (0..n)
            .map(|id| {
                (id, Traits { club_id: club, stamina: 10, natural_fitness: 12, injury_proneness: 9, aggression: 12, dirtiness: 9 })
            })
            .collect();
        Squads { states: HashMap::new(), traits }
    }

    #[test]
    fn condition_drains_and_recovers() {
        let mut sq = squads(20, 1);
        let starters: Vec<i32> = (0..11).collect();
        let empty: [i32; 0] = [];
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let ours = Side { club: 1, starters: &starters, instructions: Instructions::default(), goals_for: 1, goals_against: 0 };
        let theirs = Side { club: 2, starters: &empty, instructions: Instructions::default(), goals_for: 0, goals_against: 1 };
        sq.after_match([ours, theirs], "2025-08-02", &mut rng);
        let c = sq.state(0).condition;
        assert!((70..=80).contains(&c), "{c}");
        assert_eq!(sq.state(15).condition, 100, "quem não jogou não cansa");
        assert_eq!(sq.state(0).morale, START_MORALE + 2);
        assert_eq!(sq.state(15).morale, START_MORALE + 1);
        for d in 3..7 {
            sq.new_day(&format!("2025-08-0{d}"));
        }
        assert_eq!(sq.state(0).condition, 100, "recupera numa semana");
    }

    #[test]
    fn injuries_and_cards_have_plausible_rates() {
        let mut sq = squads(30, 1);
        let starters: Vec<i32> = (0..11).collect();
        let (mut injuries, mut bans) = (0, 0);
        let games = 2000;
        for g in 0..games {
            let mut rng = ChaCha8Rng::seed_from_u64(g);
            let side = Side { club: 1, starters: &starters, instructions: Instructions::default(), goals_for: 0, goals_against: 0 };
            let none: [i32; 0] = [];
            let other = Side { club: 2, starters: &none, instructions: Instructions::default(), goals_for: 0, goals_against: 0 };
            for e in sq.after_match([side, other], "2025-08-02", &mut rng) {
                match e {
                    Event::Injured { .. } => injuries += 1,
                    Event::Suspended { .. } => bans += 1,
                }
            }
            // Ninguém fica lesionado/suspenso para o teste seguinte
            sq.states.clear();
        }
        let per_game = injuries as f64 / games as f64;
        assert!((0.08..0.16).contains(&per_game), "lesões por equipa e jogo: {per_game}");
        assert!(bans > 0);
    }

    #[test]
    fn suspension_is_served_in_the_next_match() {
        let mut sq = squads(20, 1);
        sq.state_mut(5).suspended = 1;
        let starters: Vec<i32> = (0..5).chain(6..12).collect();
        let none: [i32; 0] = [];
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let side = Side { club: 1, starters: &starters, instructions: Instructions::default(), goals_for: 0, goals_against: 1 };
        let other = Side { club: 2, starters: &none, instructions: Instructions::default(), goals_for: 1, goals_against: 0 };
        sq.after_match([side, other], "2025-08-02", &mut rng);
        assert_eq!(sq.state(5).suspended, 0);
        assert!(sq.state(5).availability().can_play());
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
