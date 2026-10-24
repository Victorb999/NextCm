//! Personalidade (marco 3.9): os atributos ocultos que mexem no motor, na moral e no mercado.
//! Aqui ficam só as contas, puras e testadas; quem as usa lê os atributos do banco (1–20,
//! vazio = 10). Os meios das escalas são as médias do banco, para as médias do jogo não mudarem.

use crate::engine::fit_factor;

/// Médias do banco (jogadores): Versatilidade, Desportivismo, Temperamento, Ambição, Lealdade.
const VERSATILITY_MID: f64 = 9.0;
const SPORTSMANSHIP_MID: f64 = 11.8;
const TEMPERAMENT_MID: f64 = 11.0;
const AMBITION_MID: f64 = 12.0;
const LOYALTY_MID: f64 = 12.0;

/// Diferença de reputação acima da qual um jogador de Ambição média não aceita ir para um
/// clube menor; cada ponto de Ambição tira (ou dá) 120.
pub const STEP_DOWN: i32 = 1500;
/// Ambição a partir da qual um jogador fica desiludido quando o clube recusa um clube maior.
pub const AMBITIOUS: i32 = 15;
/// Moral que perde quando isso acontece.
pub const LETDOWN_MORALE: i32 = 3;
/// Rendimento perdido por um estrangeiro acabado de chegar (desaparece com a adaptação).
const ADAPT_LOSS: f64 = 0.06;

/// Um atributo de 1–20 em −1…1 à volta de `mid` (9,5 pontos para cada lado).
fn scale(v: f64, mid: f64) -> f64 {
    ((v.clamp(1.0, 20.0) - mid) / 9.5).clamp(-1.0, 1.0)
}

/// O que sobra de um atributo acima de `mid`, de 0 a 1 (aos 20).
fn above(v: f64, mid: f64) -> f64 {
    ((v.clamp(1.0, 20.0) - mid) / (20.0 - mid)).max(0.0)
}

// ---------------------------------------------------------------- motor

/// Rendimento numa função pela nota (`fit_factor`) e pela Versatilidade: fora da posição
/// (nota < 15) a perda vai de 125% (Versatilidade 1) a 70% (20) da normal.
pub fn versatility_fit(fit: i32, versatility: f64) -> f64 {
    let f = fit_factor(fit);
    if fit >= 15 {
        return f;
    }
    1.0 - (1.0 - f) * (1.0 - 0.3 * scale(versatility, VERSATILITY_MID))
}

/// Dias que um estrangeiro leva a adaptar-se a um clube novo: 30 (Adaptabilidade 20) a 180 (1).
pub fn adaptation_days(adaptability: f64) -> f64 {
    30.0 + 150.0 * (20.0 - adaptability.clamp(1.0, 20.0)) / 19.0
}

/// Rendimento de um estrangeiro `days` dias depois de chegar: −6% no primeiro dia, a subir
/// em linha reta até 100% no fim da adaptação.
pub fn adaptation_factor(days: i32, adaptability: f64) -> f64 {
    let period = adaptation_days(adaptability);
    let left = (1.0 - days.max(0) as f64 / period).max(0.0);
    1.0 - ADAPT_LOSS * left
}

/// Desportivismo: peso de um jogador nas faltas para cartão, de +25% (1) a −25% (20).
pub fn sportsmanship_factor(sportsmanship: f64) -> f64 {
    1.0 - 0.25 * scale(sportsmanship, SPORTSMANSHIP_MID)
}

/// Temperamento: peso nos vermelhos directos (agressões e protestos), de +50% (1) a −47% (20).
pub fn temperament_red_factor(temperament: f64) -> f64 {
    1.0 - 0.5 * scale(temperament, TEMPERAMENT_MID)
}

// ---------------------------------------------------------------- moral

/// Temperamento na moral depois de um jogo: os de cabeça quente (≤ 7) sobem e descem um
/// ponto mais; os calmos (≥ 15) sofrem um ponto menos com as derrotas.
pub fn morale_swing(delta: i32, temperament: i32) -> i32 {
    match temperament {
        _ if delta == 0 => 0,
        ..=7 => delta + delta.signum(),
        15.. if delta < 0 => delta + 1,
        _ => delta,
    }
}

// ---------------------------------------------------------------- mercado

/// Diferença de reputação acima da qual não desce de clube: 2 820 (Ambição 1) a 540 (20).
pub fn step_down(ambition: f64) -> i32 {
    STEP_DOWN + ((AMBITION_MID - ambition.clamp(1.0, 20.0)) * 120.0) as i32
}

/// Salário pedido × isto: um ambicioso que seria dos indispensáveis do clube (peixe grande
/// num lago pequeno) pede até 12% mais.
pub fn ambition_wage(ambition: f64, key: bool) -> f64 {
    if key { 1.0 + 0.12 * above(ambition, AMBITION_MID) } else { 1.0 }
}

/// Renovação: salário pedido × isto, de −10% (Lealdade 20) a +14% (1).
pub fn loyalty_renewal(loyalty: f64) -> f64 {
    1.0 - 0.1 * (loyalty.clamp(1.0, 20.0) - LOYALTY_MID) / (20.0 - LOYALTY_MID)
}

/// Preço pedido pelo clube × isto: um jogador leal custa até 20% mais.
pub fn loyalty_fee(loyalty: f64) -> f64 {
    1.0 + 0.2 * above(loyalty, LOYALTY_MID)
}

/// Salário pedido para trocar de clube × isto: um leal pede até 10% mais para sair.
pub fn loyalty_move_wage(loyalty: f64) -> f64 {
    1.0 + 0.1 * above(loyalty, LOYALTY_MID)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versatility_softens_playing_out_of_position() {
        // Na posição não muda
        assert_eq!(versatility_fit(20, 1.0), 1.0);
        assert_eq!(versatility_fit(16, 20.0), fit_factor(16));
        // Fora dela, o versátil perde menos e o rígido mais
        let (rigid, normal, versatile) = (versatility_fit(8, 1.0), versatility_fit(8, VERSATILITY_MID), versatility_fit(8, 20.0));
        assert!((normal - fit_factor(8)).abs() < 1e-9);
        assert!(rigid < normal && normal < versatile);
        assert!((1.0 - versatile - 0.7 * (1.0 - normal)).abs() < 1e-9);
    }

    #[test]
    fn foreigners_need_time_to_settle() {
        assert!((adaptation_factor(0, 10.0) - 0.94).abs() < 1e-9);
        assert_eq!(adaptation_factor(180, 1.0), 1.0);
        assert_eq!(adaptation_factor(30, 20.0), 1.0);
        // Ao fim de um mês, o adaptável já está em casa e o outro ainda não
        assert!(adaptation_factor(30, 1.0) < adaptation_factor(30, 15.0));
        assert!(adaptation_factor(10, 10.0) < adaptation_factor(60, 10.0));
    }

    #[test]
    fn conduct_weighs_on_cards() {
        assert!(sportsmanship_factor(1.0) > 1.2 && sportsmanship_factor(20.0) < 0.8);
        assert!((sportsmanship_factor(SPORTSMANSHIP_MID) - 1.0).abs() < 1e-9);
        assert!(temperament_red_factor(1.0) > 1.4 && temperament_red_factor(20.0) < 0.55);
    }

    #[test]
    fn temperament_moves_morale() {
        assert_eq!(morale_swing(2, 5), 3);
        assert_eq!(morale_swing(-2, 5), -3);
        assert_eq!(morale_swing(-2, 11), -2);
        assert_eq!(morale_swing(-2, 18), -1);
        assert_eq!(morale_swing(2, 18), 2);
        assert_eq!(morale_swing(0, 3), 0);
    }

    #[test]
    fn ambition_and_loyalty_in_the_market() {
        assert_eq!(step_down(AMBITION_MID), STEP_DOWN);
        assert!(step_down(20.0) < 600 && step_down(1.0) > 2800);
        assert_eq!(ambition_wage(20.0, false), 1.0);
        assert!((ambition_wage(20.0, true) - 1.12).abs() < 1e-9);
        assert_eq!(ambition_wage(8.0, true), 1.0);
        assert!((loyalty_renewal(20.0) - 0.9).abs() < 1e-9 && loyalty_renewal(1.0) > 1.13);
        assert!((loyalty_fee(20.0) - 1.2).abs() < 1e-9 && loyalty_fee(5.0) == 1.0);
        assert!((loyalty_move_wage(20.0) - 1.1).abs() < 1e-9);
    }
}
