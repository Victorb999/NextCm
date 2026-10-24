//! Dinheiro (marco 3.8): valor de mercado e salário pedido de um jogador, e as partes das
//! receitas e despesas de um clube. Funções puras, calibradas com o patch: a folha salarial
//! anual de um clube cresce com a reputação, ≈ e^(10,06 + 0,000929 × reputação) (€1M com
//! 4000, €6M com 6000, €40M com 8000); serve de referência ao salário pedido. As receitas de
//! cada clube partem da folha dele (`REVENUE_OVER_WAGES`).

use serde::Serialize;
use ts_rs::TS;

/// Categorias do livro de contas (a chave no save e o texto da tela de Finanças).
pub const INCOME: [(&str, &str); 7] = [
    ("gate", "Bilheteira"),
    ("tv", "Direitos de TV"),
    ("sponsors", "Patrocínios"),
    ("merchandising", "Merchandising"),
    ("prizes", "Prémios"),
    ("sales", "Venda de jogadores"),
    ("player_fines", "Multas a jogadores"),
];
pub const EXPENSES: [(&str, &str); 8] = [
    ("wages", "Salários dos jogadores"),
    ("staff", "Salários do staff"),
    ("purchases", "Compra de jogadores"),
    ("bonuses", "Prémios de assinatura"),
    ("stadium", "Estádio"),
    ("youth", "Camadas jovens"),
    ("fines", "Multas"),
    ("interest", "Juros"),
];

/// Receitas de uma época normal sobre a folha salarial anual do clube quando entra no jogo.
pub const REVENUE_OVER_WAGES: f64 = 1.3;
/// Receitas mínimas de uma época (clubes do patch sem salários).
pub const MIN_REVENUE: f64 = 300_000.0;

/// Partes das receitas de uma época normal.
pub const TV_SHARE: f64 = 0.40;
pub const SPONSOR_SHARE: f64 = 0.20;
pub const MERCH_SHARE: f64 = 0.10;
pub const GATE_SHARE: f64 = 0.20;
/// Prémios: de cada competição, uma parte das receitas de todos os participantes.
pub const LEAGUE_PRIZES: f64 = 0.07;
pub const CUP_PRIZES: f64 = 0.03;
pub const CONTINENTAL_PRIZES: f64 = 0.06;
/// Despesas fixas: staff sobre a folha dos jogadores, estádio sobre as receitas.
pub const STAFF_SHARE: f64 = 0.12;
pub const STADIUM_SHARE: f64 = 0.02;
/// Juros mensais de um saldo negativo.
pub const MONTHLY_INTEREST: f64 = 0.01;
/// Jogos em casa de referência numa época (para o preço do bilhete).
pub const HOME_GAMES: f64 = 22.0;

/// Folha salarial anual típica de um clube com esta reputação (0–10 000).
pub fn typical_wage_bill(reputation: i32) -> f64 {
    (10.062 + 0.000929 * reputation.clamp(0, 10_000) as f64).exp()
}

/// Receitas de uma época normal de um clube com esta reputação.
pub fn season_revenue(reputation: i32) -> f64 {
    1.25 * typical_wage_bill(reputation)
}

/// Arredonda como o CM mostra: €5K abaixo de 100 mil, €25K abaixo de 1 milhão, €50K abaixo
/// de 10 milhões, €500K acima.
pub fn round_money(v: f64) -> i64 {
    let step = match v {
        x if x < 100_000.0 => 5_000.0,
        x if x < 1_000_000.0 => 25_000.0,
        x if x < 10_000_000.0 => 50_000.0,
        _ => 500_000.0,
    };
    ((v / step).round() * step) as i64
}

/// Valor de mercado. CA pela curva 12 × (CA/10)^5,5 (CA 80 ≈ €1M, 130 ≈ €16M, 180 ≈ €95M na
/// liga mais forte); peso da liga (reputação 1–20, as mais fracas valem ~¼); jovens com
/// potencial acima do CA valem mais e depois dos 28 anos menos; a reputação do jogador
/// (0–200) de 80% a 120%; o último ano de contrato metade; sem contrato, nada.
pub fn market_value(ca: i32, pa: i32, age: i32, player_rep: i32, league_rep: i32, years_left: f64) -> i64 {
    if years_left <= 0.0 {
        return 0;
    }
    let ca = ca.clamp(1, 200) as f64;
    let base = 12.0 * (ca / 10.0).powf(5.5);
    let league = 0.15 + 0.85 * (league_rep.clamp(1, 20) as f64 / 20.0).powf(1.5);
    let gap = (pa.max(0) as f64 - ca).max(0.0);
    let age_factor = match age {
        ..=21 => 1.0 + gap / 60.0,
        22..=23 => 1.0 + gap / 120.0,
        24..=28 => 1.0,
        _ => (1.0 - 0.15 * (age - 28) as f64).max(0.15),
    };
    let rep = 0.8 + 0.4 * player_rep.clamp(0, 200) as f64 / 200.0;
    let contract = if years_left >= 2.0 {
        1.0
    } else if years_left >= 1.0 {
        0.8
    } else {
        0.5
    };
    round_money(base * league * age_factor * rep * contract)
}

/// Salário semanal que um jogador pede: ~2,3% do valor por ano (a mediana do patch),
/// ajustado à riqueza do clube que paga (um clube rico paga mais), e nunca menos de 10%
/// acima do que ganha hoje.
pub fn wage_demand(value: i64, current_wage: i64, club_rep: i32) -> i64 {
    let by_value = value as f64 * 0.023 / 52.0;
    let wealth = (typical_wage_bill(club_rep) / typical_wage_bill(6000)).powf(0.35).clamp(0.3, 2.5);
    let ask = (by_value * wealth).max(current_wage as f64 * 1.1).max(100.0);
    // Arredondado a €100 (abaixo de €10K por semana) ou a €1K
    let step = if ask < 10_000.0 { 100.0 } else { 1_000.0 };
    ((ask / step).round() * step) as i64
}

/// Uma linha do livro de contas na tela de Finanças.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FinanceLine {
    pub label: String,
    #[ts(type = "number")]
    pub amount: i64,
}

/// As contas de um clube numa época.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SeasonBooks {
    /// "2026" ou "2025/26".
    pub season: String,
    pub income: Vec<FinanceLine>,
    pub expenses: Vec<FinanceLine>,
    #[ts(type = "number")]
    pub total_income: i64,
    #[ts(type = "number")]
    pub total_expenses: i64,
}

/// A tela Finanças do clube.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClubFinances {
    pub club_id: i32,
    #[ts(type = "number")]
    pub cash: i64,
    /// Folha salarial semanal dos jogadores.
    #[ts(type = "number")]
    pub wage_bill: i64,
    /// Receitas de uma época normal para a reputação do clube.
    #[ts(type = "number")]
    pub expected_revenue: i64,
    pub this_season: SeasonBooks,
    pub last_season: Option<SeasonBooks>,
}

impl crate::World {
    /// Valor de mercado de um jogador numa data ("AAAA-MM-DD"): pela idade nessa data, pelo
    /// contrato que lhe falta e pela liga do clube dele. `None` se não for jogador.
    pub fn market_value_of(&self, id: i32, today: &str) -> crate::Result<Option<i64>> {
        use rusqlite::OptionalExtension;
        let row: Option<(i32, i32, Option<String>, Option<i32>, i32, Option<String>, i32)> = self
            .conn
            .prepare_cached(
                "SELECT COALESCE(pl.current_ability, 1), COALESCE(pl.potential_ability, 0), p.birth_date, p.birth_year,
                        COALESCE(pl.current_reputation, 0), p.contract_expires, COALESCE(k.reputation, 5)
                 FROM players pl JOIN people p ON p.id = pl.person_id
                 LEFT JOIN clubs c ON c.id = p.club_id LEFT JOIN competitions k ON k.id = c.division_id
                 WHERE pl.person_id = ?",
            )?
            .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))
            .optional()?;
        let Some((ca, pa, birth, birth_year, rep, expires, league)) = row else { return Ok(None) };
        let age = crate::game::age_on(birth.as_deref(), birth_year, today);
        let years = match (expires.as_deref().and_then(crate::date::parse), crate::date::parse(today)) {
            (Some(e), Some(t)) => (e - t) as f64 / 365.25,
            _ => 0.0,
        };
        // PA negativo no banco = "aleatório": conta como um pouco acima do CA
        let pa = if pa > 0 { pa } else { ca + 10 };
        Ok(Some(market_value(ca, pa, age, rep, league, years)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_follow_ability_age_league_and_contract() {
        // Um craque jovem da liga mais forte vale mais de 100 milhões; um titular brasileiro, uns milhões
        let star = market_value(186, 196, 18, 200, 20, 4.0);
        assert!(star > 100_000_000, "{star}");
        let brazil = market_value(130, 135, 26, 60, 13, 3.0);
        assert!((3_000_000..15_000_000).contains(&brazil), "{brazil}");
        assert!(market_value(80, 85, 27, 20, 5, 2.0) < 500_000);
        // Idade, contrato e liga
        assert!(market_value(140, 150, 33, 100, 20, 3.0) < market_value(140, 150, 26, 100, 20, 3.0) / 3);
        assert_eq!(market_value(140, 150, 26, 100, 20, 0.5) * 2, market_value(140, 150, 26, 100, 20, 3.0));
        assert_eq!(market_value(140, 150, 26, 100, 20, 0.0), 0);
        assert!(market_value(140, 150, 26, 100, 20, 3.0) > market_value(140, 150, 26, 100, 10, 3.0));
    }

    #[test]
    fn money_is_rounded_like_the_cm() {
        assert_eq!(round_money(12_345.0), 10_000);
        assert_eq!(round_money(612_000.0), 600_000);
        assert_eq!(round_money(4_520_000.0), 4_500_000);
        assert_eq!(round_money(16_300_000.0), 16_500_000);
    }

    #[test]
    fn wage_demand_depends_on_value_and_club() {
        let v = 20_000_000;
        assert!(wage_demand(v, 5_000, 9000) > wage_demand(v, 5_000, 5000));
        assert!(wage_demand(v, 50_000, 5000) >= 55_000, "nunca menos do que ganha + 10%");
        assert!(wage_demand(0, 0, 3000) >= 100);
    }

    #[test]
    fn revenue_tracks_reputation() {
        assert!((season_revenue(6500) / typical_wage_bill(6500) - 1.25).abs() < 1e-9);
        assert!(season_revenue(9500) > 100_000_000.0 && season_revenue(4000) < 2_000_000.0);
    }
}
