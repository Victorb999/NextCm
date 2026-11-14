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

/// Ir do clube com o peso `from` para o com `to` é descer demais para este jogador. O peso é a
/// reputação mais o bónus das continentais (`Game::club_pull`): do City, na Liga dos Campeões,
/// para o United fora da Europa desce-se; para outro clube da Liga dos Campeões, não.
pub fn steps_down(from: i32, to: i32, ambition: f64) -> bool {
    from > to + step_down(ambition)
}

/// Folga no [`step_down`] de quem vai ser titular no clube novo sem o ser no dele (4.0, item 20):
/// o jovem no banco de um grande aceita um clube menor onde joga; até aos 23 anos, mais ainda.
pub const PLAY_SLACK: i32 = 1500;
pub const YOUNG_SLACK: i32 = 1000;
pub const YOUNG_AGE: i32 = 23;

/// Uma mudança de clube vista pelo jogador: os pesos dos clubes (`Game::club_pull`, com a
/// cidade já somada ao de destino) e se joga num e no outro.
#[derive(Debug, Clone, Copy)]
pub struct Move {
    pub from: i32,
    pub to: i32,
    pub ambition: f64,
    pub age: i32,
    /// É dos 11 melhores do plantel onde está.
    pub plays_now: bool,
    /// Seria dos 11 melhores do plantel novo.
    pub plays_there: bool,
}

/// [`steps_down`] com a folga de quem passa a jogar.
pub fn move_steps_down(m: &Move) -> bool {
    let slack = if m.plays_there && !m.plays_now { PLAY_SLACK + if m.age <= YOUNG_AGE { YOUNG_SLACK } else { 0 } } else { 0 };
    steps_down(m.from, m.to + slack, m.ambition)
}

/// Porque um jogador recusa ir emprestado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoanRefusal {
    /// Desce demais (pela Ambição, com a folga de quem passa a jogar).
    StepsDown,
    /// Vai para um clube menor e nem lá joga.
    Bench,
}

/// A resposta do jogador a um empréstimo (os clubes já acordaram): aceita descer para jogar,
/// não aceita descer para o banco, e um clube do mesmo tamanho ou maior serve sempre.
pub fn loan_refusal(m: &Move) -> Option<LoanRefusal> {
    if move_steps_down(m) {
        Some(LoanRefusal::StepsDown)
    } else if !m.plays_there && m.to < m.from {
        Some(LoanRefusal::Bench)
    } else {
        None
    }
}

/// Atracção da cidade (`cities.attraction`, 0–20 no CM; 0 = sem dados) a partir da qual não
/// pesa, e o peso para quem é de fora: o do país conta 30%, o que fala a mesma língua 60%.
pub const CITY_MID: i32 = 10;
pub const CITY_HOME: f64 = 0.3;
pub const CITY_LANGUAGE: f64 = 0.6;
/// Peso no clube (como a reputação) por ponto de atracção acima ou abaixo do meio.
const CITY_PULL: f64 = 80.0;
/// Salário pedido a mais por ponto de atracção abaixo do meio (só abaixo: uma cidade grande
/// atrai, mas ninguém aceita ganhar menos por ela).
const CITY_WAGE: f64 = 0.025;

/// A atracção de uma cidade, com 0 (sem dados) no meio.
fn attraction(a: i32) -> f64 {
    if a <= 0 { CITY_MID as f64 } else { a.clamp(1, 20) as f64 }
}

/// O que a cidade soma ao peso do clube para um jogador (`weight`: 1 estrangeiro,
/// [`CITY_LANGUAGE`] com a mesma língua, [`CITY_HOME`] do país): Londres +800, Sunderland −80.
pub fn city_pull(a: i32, weight: f64) -> i32 {
    ((attraction(a) - CITY_MID as f64) * CITY_PULL * weight).round() as i32
}

/// Salário pedido × isto numa cidade pouco atraente: até +22,5% (atracção 1) para quem é de fora.
pub fn city_wage(a: i32, weight: f64) -> f64 {
    1.0 + CITY_WAGE * (CITY_MID as f64 - attraction(a)).max(0.0) * weight
}

/// Degraus de função (o 3.º campo de `SQUAD_ROLES`: 0 indispensável a 3 apoio) abaixo do que
/// espera a partir dos quais recusa o contrato (4.0, item 21): o titular não aceita "apoio".
pub const ROLE_REFUSE_STEPS: i32 = 2;
/// Compensa o papel menor: um clube com mais isto de peso (menos 100 por ponto de Ambição
/// acima de 10) ou um salário 50% acima do que ganha hoje.
pub const ROLE_BIG_CLUB: i32 = 2500;
pub const ROLE_WAGE_LURE: f64 = 1.5;

/// Aceita uma função `steps` degraus abaixo da que espera? `gain`: o peso do clube novo menos
/// o do dele (0 numa renovação); `wage_gain`: salário oferecido sobre o que ganha (None se não
/// ganha nada: o livre aceita).
pub fn accepts_role(steps: i32, ambition: f64, gain: i32, wage_gain: Option<f64>) -> bool {
    if steps < ROLE_REFUSE_STEPS {
        return true;
    }
    let big = ROLE_BIG_CLUB - ((ambition.clamp(1.0, 20.0) - 10.0) * 100.0) as i32;
    gain >= big || wage_gain.is_none_or(|w| w >= ROLE_WAGE_LURE)
}

/// O bónus de peso de quem joga as continentais nesta época (somado à reputação do clube).
pub fn continental_pull(competition: i32) -> i32 {
    use crate::league::{CHAMPIONS_LEAGUE, EUROPA_LEAGUE, LIBERTADORES, SUL_AMERICANA};
    match competition {
        CHAMPIONS_LEAGUE => 1500,
        LIBERTADORES => 1000,
        EUROPA_LEAGUE => 700,
        SUL_AMERICANA => 500,
        _ => 0,
    }
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

// ---------------------------------------------------------------- renovações da IA

/// O que pesa na vontade de um jogador de renovar com um clube da IA (3.19.1, item 37).
pub struct RenewalMood {
    pub loyalty: f64,
    pub ambition: f64,
    /// Épocas inteiras no clube.
    pub seasons: i32,
    /// Moral de 1 a 20, quando o jogo a acompanha.
    pub morale: Option<i32>,
    /// Tem queixas em aberto (insatisfação).
    pub unhappy: bool,
    /// CA do jogador menos a média dos 11 melhores do plantel.
    pub above_squad: i32,
    pub age: i32,
}

/// Diferença para os 11 melhores acima da qual o jogador é grande demais para o clube: 25 com
/// Ambição 10, 10 com Ambição 20.
pub fn outgrown_margin(ambition: f64) -> i32 {
    (25.0 - 1.5 * (ambition.clamp(1.0, 20.0) - 10.0)).round() as i32
}

/// Probabilidade de aceitar renovar: 75% de base; a Lealdade dá ±1,5% por ponto à volta de 10;
/// +2% por época no clube (até +10%); a moral baixa tira 15% e as queixas 30%; grande demais
/// para o clube, −25% a −40% (pela Ambição); 32+ anos, +15%. Entre 10% e 98%.
pub fn renewal_stay_chance(m: &RenewalMood) -> f64 {
    let mut p = 0.75 + 0.015 * (m.loyalty.clamp(1.0, 20.0) - 10.0) + 0.02 * m.seasons.clamp(0, 5) as f64;
    if m.unhappy {
        p -= 0.30;
    } else if m.morale.is_some_and(|v| v < 8) {
        p -= 0.15;
    }
    if m.above_squad > outgrown_margin(m.ambition) {
        p -= 0.25 + 0.15 * above(m.ambition, AMBITION_MID);
    }
    if m.age >= 32 {
        p += 0.15;
    }
    p.clamp(0.10, 0.98)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bench_kid() -> Move {
        Move { from: 9000, to: 5500, ambition: 12.0, age: 20, plays_now: false, plays_there: true }
    }

    #[test]
    fn playing_time_eases_stepping_down() {
        // Do banco de um grande para titular num clube 3500 abaixo: o jovem aceita, o de 28 não
        assert!(!move_steps_down(&bench_kid()));
        assert!(move_steps_down(&Move { age: 28, ..bench_kid() }));
        // O titular não ganha folga
        assert!(move_steps_down(&Move { plays_now: true, ..bench_kid() }));
        // Muito abaixo, nem o jovem
        assert!(move_steps_down(&Move { to: 4000, ..bench_kid() }));
    }

    #[test]
    fn loans_need_minutes_or_a_bigger_club() {
        assert_eq!(loan_refusal(&bench_kid()), None);
        assert_eq!(loan_refusal(&Move { from: 6500, plays_there: false, ..bench_kid() }), Some(LoanRefusal::Bench));
        assert_eq!(loan_refusal(&Move { to: 9500, plays_there: false, ..bench_kid() }), None);
        assert_eq!(loan_refusal(&Move { to: 3000, ..bench_kid() }), Some(LoanRefusal::StepsDown));
    }

    #[test]
    fn the_city_weighs_more_on_foreigners() {
        assert_eq!(city_pull(20, 1.0), 800);
        assert_eq!(city_pull(0, 1.0), 0);
        assert!(city_pull(5, CITY_HOME) > city_pull(5, 1.0));
        assert_eq!(city_wage(20, 1.0), 1.0);
        assert!((city_wage(1, 1.0) - 1.225).abs() < 1e-9);
        assert!(city_wage(4, CITY_HOME) < city_wage(4, CITY_LANGUAGE));
    }

    #[test]
    fn a_lesser_role_needs_a_bigger_club_or_money() {
        assert!(accepts_role(1, 10.0, 0, Some(1.0)));
        assert!(!accepts_role(2, 10.0, 0, Some(1.2)));
        assert!(accepts_role(2, 10.0, 0, Some(1.6)));
        assert!(accepts_role(2, 10.0, 2600, Some(1.0)));
        // O ambicioso aceita o banco de um clube grande mais cedo
        assert!(accepts_role(2, 18.0, 1800, Some(1.0)) && !accepts_role(2, 5.0, 1800, Some(1.0)));
        assert!(accepts_role(3, 10.0, 0, None));
    }

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
    fn europe_weighs_in_the_step_down() {
        use crate::league::CHAMPIONS_LEAGUE;
        // O Enzo (Ambição 13) do City (10 000, Liga dos Campeões) para o United (8 500, fora da Europa)
        let city = 10_000 + continental_pull(CHAMPIONS_LEAGUE);
        assert!(steps_down(city, 8_500, 13.0));
        // Para um clube de 9 000 também na Liga dos Campeões, não
        assert!(!steps_down(city, 9_000 + continental_pull(CHAMPIONS_LEAGUE), 13.0));
        assert!(!steps_down(6_000, 5_500, 13.0));
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

    #[test]
    fn renewal_willingness() {
        let base = RenewalMood { loyalty: 10.0, ambition: 10.0, seasons: 0, morale: None, unhappy: false, above_squad: 0, age: 26 };
        assert!((renewal_stay_chance(&base) - 0.75).abs() < 1e-9);
        // O leal da casa fica quase sempre; o veterano também
        let loyal = RenewalMood { loyalty: 20.0, seasons: 8, ..base };
        assert!((renewal_stay_chance(&loyal) - 0.98).abs() < 1e-9);
        assert!(renewal_stay_chance(&RenewalMood { age: 33, ..base }) > 0.89);
        // Infeliz, ou grande demais para o clube (o ambicioso sente-o mais cedo)
        assert!((renewal_stay_chance(&RenewalMood { unhappy: true, morale: Some(3), ..base }) - 0.45).abs() < 1e-9);
        assert!((renewal_stay_chance(&RenewalMood { morale: Some(5), ..base }) - 0.60).abs() < 1e-9);
        assert_eq!(outgrown_margin(10.0), 25);
        assert_eq!(outgrown_margin(20.0), 10);
        let star = RenewalMood { above_squad: 20, ambition: 18.0, ..base };
        assert!(renewal_stay_chance(&star) < 0.5);
        assert_eq!(renewal_stay_chance(&RenewalMood { ambition: 5.0, ..star }), renewal_stay_chance(&base));
        // Nunca abaixo de 10%
        let worst = RenewalMood { loyalty: 1.0, ambition: 20.0, unhappy: true, above_squad: 60, ..base };
        assert!((renewal_stay_chance(&worst) - 0.10).abs() < 1e-9);
    }
}
