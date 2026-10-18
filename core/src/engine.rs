//! Match Engine v1 (marco 3.2): força por setor (GR, defesa, meio, ataque) a partir do
//! onze de [`World::get_club_tactics`], golos esperados de cada lado e golos sorteados
//! por Poisson com um RNG seedado. Sem relato nem marcadores; isso vem no 3.7.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

use crate::{ClubTactics, Result, World};

/// Golos esperados de uma equipa média em casa e fora (média da Série A: ~2,4 por jogo).
const BASE_HOME: f64 = 1.42;
const BASE_AWAY: f64 = 0.92;
/// Quanto a razão ataque/defesa pesa: com 3, ser 10% melhor dá ~33% mais golos.
const K_ATTACK: f64 = 3.0;
/// Peso de quem domina o meio-campo.
const K_MIDFIELD: f64 = 1.5;
/// Nota de uma função sem jogador (plantel incompleto).
const EMPTY_SLOT: f64 = 20.0;

/// Quanto do CA rende na função: inteiro com nota 20, 95% com 15 e, fora da posição,
/// de 62% (nota 1) a 88% (nota 14). Um defesa a médio não é um júnior.
fn fit_factor(fit: i32) -> f64 {
    let fit = fit.clamp(1, 20) as f64;
    if fit >= 15.0 { 1.0 - (20.0 - fit) * 0.01 } else { 0.6 + 0.02 * fit }
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
        let mut sums = [0.0; 4];
        let mut counts = [0usize; 4];
        for slot in &t.starters {
            let pos = slot.role.split(' ').next().unwrap_or("");
            let sector = match pos {
                "GR" => 0,
                "L" | "D" | "AL" => 1,
                "MD" | "M" | "MA" => 2,
                _ => 3,
            };
            let rating = slot.player.as_ref().map_or(EMPTY_SLOT, |p| {
                let ca = p.current_ability.unwrap_or(1) as f64;
                ca * fit_factor(p.fit.unwrap_or(10))
            });
            sums[sector] += rating;
            counts[sector] += 1;
        }
        let avg = |i: usize| if counts[i] == 0 { EMPTY_SLOT } else { sums[i] / counts[i] as f64 };
        TeamStrength { gk: avg(0), defence: avg(1), midfield: avg(2), attack: avg(3) }
    }

    fn attacking(&self) -> f64 {
        0.65 * self.attack + 0.35 * self.midfield
    }

    fn defending(&self) -> f64 {
        0.7 * self.defence + 0.3 * self.gk
    }
}

/// Golos esperados (casa, fora).
pub fn expected_goals(home: &TeamStrength, away: &TeamStrength) -> (f64, f64) {
    let mid_home = home.midfield / (home.midfield + away.midfield);
    let xg = |base: f64, att: &TeamStrength, def: &TeamStrength, mid_share: f64| {
        base * (att.attacking() / def.defending()).powf(K_ATTACK) * (2.0 * mid_share).powf(K_MIDFIELD)
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
fn poisson(lambda: f64, rng: &mut impl RngCore) -> u8 {
    let limit = (-lambda).exp();
    let mut k = 0u8;
    let mut p = uniform(rng);
    while p > limit && k < 15 {
        k += 1;
        p *= uniform(rng);
    }
    k
}

/// Resultado (golos da casa, golos de fora).
pub fn play(home: &TeamStrength, away: &TeamStrength, rng: &mut impl RngCore) -> (u8, u8) {
    let (xh, xa) = expected_goals(home, away);
    (poisson(xh, rng), poisson(xa, rng))
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
    }

    #[test]
    fn same_seed_same_result() {
        let (h, a) = (team(130.0), team(125.0));
        let run = |seed| (0..50).map(|id| play(&h, &a, &mut match_rng(seed, id))).collect::<Vec<_>>();
        assert_eq!(run(7), run(7));
        assert_ne!(run(7), run(8));
    }

    #[test]
    fn poisson_mean_is_lambda() {
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let n = 20_000;
        let mean = (0..n).map(|_| poisson(1.3, &mut rng) as f64).sum::<f64>() / n as f64;
        assert!((mean - 1.3).abs() < 0.05, "{mean}");
    }
}
