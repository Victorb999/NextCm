//! Força das equipas (marcos 3.2 e 3.3): força por setor (GR, defesa, meio, ataque) a partir
//! do onze em campo (nota na função, condição e moral de cada um, instruções à equipa) e os
//! golos esperados de cada lado. O jogo lance a lance (marco 3.4) está em `live.rs` e usa
//! estes golos esperados minuto a minuto, para as médias do motor v1 continuarem a valer.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

use crate::tactics::{Instructions, Mentality, Passing, Tackling};
use crate::{ClubTactics, Result, World};

/// Golos esperados de uma equipa média em casa e fora (com o efeito do `LEAD_EASE` de
/// `live.rs` e das diferenças entre equipas, a Série A fica em ~2,6 por jogo).
const BASE_HOME: f64 = 1.46;
const BASE_AWAY: f64 = 0.94;
/// Quanto a razão ataque/defesa pesa: com 2,55, ser 10% melhor dá ~27% mais golos.
const K_ATTACK: f64 = 2.55;
/// Peso de quem domina o meio-campo.
const K_MIDFIELD: f64 = 1.275;
/// Teto da vantagem (em logaritmo, por tanh): no máximo e ≈ 2,7 vezes os golos da base,
/// para a Série A contra um clube de estadual não dar 15 golos.
const MAX_EDGE: f64 = 1.0;
/// Nota de uma função sem jogador (plantel incompleto).
const EMPTY_SLOT: f64 = 20.0;
/// Cada jogador a menos em campo (expulso, lesionado sem substituto) tira 10% a tudo.
const MAN_DOWN: f64 = 0.9;

/// Setor de uma função ("D C" → 1): 0 GR, 1 defesa, 2 meio, 3 ataque.
pub(crate) fn sector(role: &str) -> usize {
    match role.split(' ').next().unwrap_or("") {
        "GR" => 0,
        "L" | "D" | "AL" => 1,
        "MD" | "M" | "MA" => 2,
        _ => 3,
    }
}

/// Uma função ocupada: a função, a altura no campo, a seta e quanto o jogador rende ali.
pub(crate) struct SlotRating<'a> {
    pub role: &'a str,
    pub y: i32,
    pub run: Option<[i32; 2]>,
    pub rating: f64,
}

/// Quanto do CA rende na função: inteiro com nota 20, 95% com 15 e, fora da posição,
/// de 62% (nota 1) a 88% (nota 14). Um defesa a médio não é um júnior.
pub(crate) fn fit_factor(fit: i32) -> f64 {
    let fit = fit.clamp(1, 20) as f64;
    if fit >= 15.0 { 1.0 - (20.0 - fit) * 0.01 } else { 0.6 + 0.02 * fit }
}

/// Condição 0–100 → rendimento de 75% a 100%.
pub(crate) fn condition_factor(condition: f64) -> f64 {
    0.75 + 0.25 * condition.clamp(0.0, 100.0) / 100.0
}

/// Moral 1–20 → rendimento de 95% a 105% (12, a moral inicial, ≈ 100%).
pub(crate) fn morale_factor(morale: i32) -> f64 {
    0.95 + 0.10 * (morale.clamp(1, 20) - 1) as f64 / 19.0
}

/// Força média de cada setor (escala do CA, 1–200).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeamStrength {
    pub gk: f64,
    pub defence: f64,
    pub midfield: f64,
    pub attack: f64,
}

impl TeamStrength {
    /// Cada titular vale o CA ajustado pela nota (1–20) na função onde joga.
    pub fn from_tactics(t: &ClubTactics) -> TeamStrength {
        let slots: Vec<SlotRating> = t
            .starters
            .iter()
            .map(|slot| SlotRating {
                role: &slot.role,
                y: slot.y,
                run: slot.run,
                rating: slot.player.as_ref().map_or(EMPTY_SLOT, |p| {
                    let ca = p.current_ability.unwrap_or(1) as f64;
                    ca * fit_factor(p.fit.unwrap_or(10))
                        * p.condition.map_or(1.0, |c| condition_factor(c as f64))
                        * p.morale.map_or(1.0, morale_factor)
                }),
            })
            .collect();
        TeamStrength::from_slots(&slots, 0, &t.instructions)
    }

    /// Força a partir das funções ocupadas; `missing` jogadores a menos em campo.
    pub(crate) fn from_slots(slots: &[SlotRating], missing: usize, instructions: &Instructions) -> TeamStrength {
        let mut sums = [0.0; 4];
        let mut counts = [0usize; 4];
        for slot in slots {
            let i = sector(slot.role);
            sums[i] += slot.rating;
            counts[i] += 1;
        }
        let avg = |i: usize| if counts[i] == 0 { EMPTY_SLOT } else { sums[i] / counts[i] as f64 };
        let mut s = TeamStrength { gk: avg(0), defence: avg(1), midfield: avg(2), attack: avg(3) };
        // Setas de movimento: cada titular que corre para a frente dá +1% no ataque e tira
        // 0,5% na defesa; quem recua faz o contrário.
        for slot in slots {
            if let Some([_, ry]) = slot.run {
                if ry < slot.y - 4 {
                    s.attack *= 1.01;
                    s.defence *= 0.995;
                } else if ry > slot.y + 4 {
                    s.defence *= 1.01;
                    s.attack *= 0.995;
                }
            }
        }
        let down = MAN_DOWN.powi(missing as i32);
        s.defence *= down;
        s.midfield *= down;
        s.attack *= down;
        s.with(instructions)
    }

    /// Ajustes pequenos das Instruções à Equipa (Desarme e Pressão também mexem nos
    /// cartões e no cansaço, em `live.rs`).
    pub fn with(mut self, i: &Instructions) -> TeamStrength {
        let (att, mid, def) = match i.mentality {
            Mentality::Defensive => (0.95, 1.0, 1.05),
            Mentality::Normal => (1.0, 1.0, 1.0),
            Mentality::Attacking => (1.05, 1.0, 0.95),
        };
        self.attack *= att;
        self.midfield *= mid;
        self.defence *= def;
        match i.passing {
            Passing::Mixed => {}
            Passing::Short => {
                self.midfield *= 1.03;
                self.attack *= 0.98;
            }
            Passing::Direct | Passing::Long => {
                self.attack *= 1.03;
                self.midfield *= 0.97;
            }
        }
        match i.tackling {
            Tackling::Normal => {}
            Tackling::Soft => self.defence *= 0.98,
            Tackling::Hard => self.defence *= 1.02,
        }
        if i.pressing {
            self.midfield *= 1.04;
        }
        if i.offside_trap {
            self.defence *= 1.02;
        }
        if i.counter_attack {
            self.attack *= 1.03;
            self.midfield *= 0.98;
        }
        if i.tight_marking {
            self.defence *= 1.03;
            self.midfield *= 0.98;
        }
        self
    }

    fn attacking(&self) -> f64 {
        0.65 * self.attack + 0.35 * self.midfield
    }

    fn defending(&self) -> f64 {
        0.7 * self.defence + 0.3 * self.gk
    }
}

/// Golos esperados em campo neutro: as duas equipas com a média das bases de casa e fora.
pub fn expected_goals_neutral(home: &TeamStrength, away: &TeamStrength) -> (f64, f64) {
    let (h, a) = expected_goals(home, away);
    let base = (BASE_HOME + BASE_AWAY) / 2.0;
    (h * base / BASE_HOME, a * base / BASE_AWAY)
}

/// Golos esperados (casa, fora): a base vezes a vantagem `(ataque/defesa)^K_ATTACK ×
/// (2 × meio-campo)^K_MIDFIELD`, achatada em logaritmo até `MAX_EDGE`. Diferenças pequenas
/// quase não sentem o teto; as grandes (promovidos, estaduais) deixam de explodir.
pub fn expected_goals(home: &TeamStrength, away: &TeamStrength) -> (f64, f64) {
    let mid_home = home.midfield / (home.midfield + away.midfield);
    let xg = |base: f64, att: &TeamStrength, def: &TeamStrength, mid_share: f64| {
        let edge = K_ATTACK * (att.attacking() / def.defending()).ln() + K_MIDFIELD * (2.0 * mid_share).ln();
        base * (MAX_EDGE * (edge / MAX_EDGE).tanh()).exp()
    };
    (xg(BASE_HOME, home, away, mid_home), xg(BASE_AWAY, away, home, 1.0 - mid_home))
}

/// RNG de um jogo: a seed do jogo misturada com o id da partida, para cada partida
/// ter o seu sorteio independente da ordem em que é jogada.
pub fn match_rng(seed: i64, fixture_id: i32) -> ChaCha8Rng {
    let mix = (seed as u64) ^ (fixture_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    ChaCha8Rng::seed_from_u64(mix)
}

/// Número uniforme em [0, 1).
pub(crate) fn uniform(rng: &mut impl RngCore) -> f64 {
    (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// Poisson pelo método de Knuth (λ pequeno, como golos num jogo).
pub(crate) fn poisson(lambda: f64, rng: &mut impl RngCore) -> u8 {
    let limit = (-lambda).exp();
    let (mut k, mut p) = (0u8, uniform(rng));
    while p > limit && k < 15 {
        k += 1;
        p *= uniform(rng);
    }
    k
}

/// Desempate por penáltis: 5 para cada lado (acaba antes se um já não pode ser apanhado)
/// e, empatados, morte súbita. Cada penálti é golo 3 vezes em 4.
pub(crate) fn shootout(rng: &mut impl RngCore) -> (u8, u8) {
    let mut goals = [0u8; 2];
    let mut taken = [0u8; 2];
    for kick in 0..10 {
        let side = kick % 2;
        taken[side] += 1;
        goals[side] += (uniform(rng) < 0.75) as u8;
        let left = [5 - taken[0], 5 - taken[1]];
        if goals[0] > goals[1] + left[1] || goals[1] > goals[0] + left[0] {
            return (goals[0], goals[1]);
        }
    }
    while goals[0] == goals[1] {
        goals[0] += (uniform(rng) < 0.75) as u8;
        goals[1] += (uniform(rng) < 0.75) as u8;
    }
    (goals[0], goals[1])
}

impl World {
    /// Força do onze que o clube escolheria hoje.
    pub fn team_strength(&self, club_id: i32) -> Result<TeamStrength> {
        Ok(TeamStrength::from_tactics(&self.get_club_tactics(club_id)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team(v: f64) -> TeamStrength {
        TeamStrength { gk: v, defence: v, midfield: v, attack: v }
    }

    #[test]
    fn equal_teams_have_home_advantage() {
        let (h, a) = expected_goals(&team(120.0), &team(120.0));
        assert!((h - BASE_HOME).abs() < 1e-9 && (a - BASE_AWAY).abs() < 1e-9);
        let (h, a) = expected_goals(&team(140.0), &team(120.0));
        assert!(h > 2.0 && a < 0.8, "favorito marca mais: {h:.2} x {a:.2}");
        // Equipas muito desiguais: o teto segura os golos esperados
        let (h, a) = expected_goals(&team(150.0), &team(50.0));
        assert!(h > 3.5 && h < BASE_HOME * MAX_EDGE.exp() && a > 0.3, "{h:.2} x {a:.2}");
    }
}
