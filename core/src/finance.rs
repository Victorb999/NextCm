//! Dinheiro (marco 3.8): valor de mercado e salário pedido de um jogador, e as partes das
//! receitas e despesas de um clube. Funções puras, calibradas com o patch: a folha salarial
//! anual de um clube cresce com a reputação, ≈ e^(10,06 + 0,000929 × reputação) (€1M com
//! 4000, €6M com 6000, €40M com 8000 no banco; o jogo paga `WAGE_SCALE` vezes isso, como o CM);
//! serve de referência ao salário pedido. As receitas de cada clube partem da folha dele
//! (`REVENUE_OVER_WAGES`). Economia do CM medida nos prints de 4/11: docs/roadmap/3.15c-pendencias.md.

use serde::Serialize;
use ts_rs::TS;

/// Categorias do livro de contas (a chave no save e o texto da tela de Finanças), pela ordem
/// do CM. Os cartões anuais e os investimentos ficam a zero, como no CM do patch.
pub const INCOME: [(&str, &str); 9] = [
    ("gate", "Receitas Bilheteira"),
    ("season_tickets", "Cartões Anuais"),
    ("tv", "Receitas TV"),
    ("merchandising", "Merchandising"),
    ("sales", "Jogadores Vendidos"),
    ("prizes", "Prémios em Dinheiro"),
    ("investments", "Investimentos"),
    ("interest_income", "Juros"),
    ("other", "Outros"),
];
pub const EXPENSES: [(&str, &str); 9] = [
    ("wages", "Salário"),
    ("match_bonuses", "Prémios"),
    ("bonuses", "Luvas"),
    ("dividends", "Dividendos"),
    ("purchases", "Jogadores Comprados"),
    ("stadium", "Estádio"),
    ("fines", "Multas da Liga"),
    ("interest", "Juros"),
    ("other_expenses", "Outros"),
];

/// A linha onde entram as chaves dos saves antigos (antes do save 43).
pub fn line_key(key: &str) -> &str {
    match key {
        "sponsors" => "merchandising",
        "player_fines" => "other",
        "staff" => "wages",
        "youth" => "other_expenses",
        k => k,
    }
}

/// O dinheiro do CM 01/02 (patch CLZ) sobre o do banco: os salários e os saldos do 1.º dia
/// são cerca de 3× (o Kane tem €75.000 por semana no banco e €875.000 por mês no CM; o Bayern
/// paga ~€14,7M por mês para uma folha de €957.700 por semana no banco).
pub const WAGE_SCALE: f64 = 3.0;
/// Receitas de uma época normal sobre a folha salarial anual do clube quando entra no jogo
/// (a referência dos orçamentos e das partes das receitas).
pub const REVENUE_OVER_WAGES: f64 = 1.3;
/// Receitas mínimas de uma época (clubes do patch sem salários).
pub const MIN_REVENUE: f64 = 900_000.0;
/// O saldo de um clube no 1.º dia de um jogo novo, sobre o do banco: o CM começa com 2,9 a
/// 3,1× (Bayern €56M → €172,8M, Hoffenheim €10M → €28,65M, 1860 €1,82M → €5,32M).
pub const START_CASH: f64 = 3.0;

/// Partes das receitas de uma época normal (o Bayern do CM: merchandising €62,9M, TV €3,9M
/// numa época com receitas de referência ~€194M).
pub const MERCH_SHARE: f64 = 0.36;
pub const TV_SHARE: f64 = 0.06;
/// Prémios, sobre as receitas de referência de cada clube: na liga de 0,3× (último) a 1,5×
/// (campeão) esta parte; nas taças, metade a quem joga; no fim de taças e continentais, mais
/// 1× ao campeão e ½× ao vice. Nas continentais cada jogo paga `CONTINENTAL_MATCH` aos dois,
/// como o CM, que paga prémios durante a época. O Bayern do CM ganha €173M numa época.
pub const LEAGUE_PRIZES: f64 = 0.4;
pub const CUP_PRIZES: f64 = 0.08;
pub const CONTINENTAL_PRIZES: f64 = 0.35;
pub const CONTINENTAL_MATCH: f64 = 0.06;
/// Despesas fixas: staff sobre a folha dos jogadores (entra no Salário), estádio sobre as receitas.
pub const STAFF_SHARE: f64 = 0.12;
pub const STADIUM_SHARE: f64 = 0.04;
/// Juros mensais de um saldo negativo e de um positivo (o Bayern do CM: ~€0,6M por mês com
/// ~€220M no banco).
pub const MONTHLY_INTEREST: f64 = 0.01;
pub const SAVINGS_INTEREST: f64 = 0.0025;
/// Dividendos aos accionistas, a 1/7: esta parte do saldo acima da reserva (épocas de receitas).
pub const DIVIDEND_SHARE: f64 = 0.25;
pub const DIVIDEND_RESERVE: f64 = 1.0;

/// Preço médio de um bilhete pela reputação do clube da casa: €4 nos clubes sem nome, ~€25
/// com 7000 e €54 com 10 000 (o Bayern do CM fez €80,9M de bilheteira numa época).
pub fn ticket_price(reputation: i32) -> f64 {
    4.0 + 50.0 * (reputation.clamp(0, 10_000) as f64 / 10_000.0).powf(2.5)
}

/// Folha salarial anual típica de um clube com esta reputação (0–10 000).
pub fn typical_wage_bill(reputation: i32) -> f64 {
    WAGE_SCALE * (10.062 + 0.000929 * reputation.clamp(0, 10_000) as f64).exp()
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

/// Salário semanal que um jogador pede: ~2,3% do valor por ano (a mediana do patch, × `WAGE_SCALE`),
/// ajustado à riqueza do clube que paga (um clube rico paga mais), e nunca menos de 10%
/// acima do que ganha hoje.
pub fn wage_demand(value: i64, current_wage: i64, club_rep: i32) -> i64 {
    let by_value = value as f64 * 0.023 * WAGE_SCALE / 52.0;
    let wealth = (typical_wage_bill(club_rep) / typical_wage_bill(6000)).powf(0.35).clamp(0.3, 2.5);
    let ask = (by_value * wealth).max(current_wage as f64 * 1.1).max(100.0 * WAGE_SCALE);
    // Arredondado a €100 (abaixo de €10K por semana) ou a €1K
    let step = if ask < 10_000.0 { 100.0 } else { 1_000.0 };
    ((ask / step).round() * step) as i64
}

/// Uma rubrica da tela de Finanças: Este Mês, Último Mês, Este Ano (a época) e Último Ano.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FinanceRow {
    pub label: String,
    #[ts(type = "[number, number, number, number]")]
    pub amounts: [i64; 4],
}

/// A tela Finanças do clube, como no CM.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClubFinances {
    pub club_id: i32,
    /// Balanço.
    #[ts(type = "number")]
    pub cash: i64,
    /// Fundos Transferências (no clube do treinador, o orçamento da direção).
    #[ts(type = "number")]
    pub funds: i64,
    /// Folha salarial semanal dos jogadores.
    #[ts(type = "number")]
    pub wage_bill: i64,
    /// Receitas de referência de uma época.
    #[ts(type = "number")]
    pub expected_revenue: i64,
    /// "2026" ou "2025/26".
    pub season: String,
    pub income: Vec<FinanceRow>,
    pub expenses: Vec<FinanceRow>,
    pub total_income: FinanceRow,
    pub total_expenses: FinanceRow,
    pub result: FinanceRow,
}

/// O valor de mercado numa data pelas colunas do banco (o perfil e a ordenação da pesquisa).
#[allow(clippy::too_many_arguments)]
pub fn value_on(
    ca: i32,
    pa: i32,
    birth: Option<&str>,
    birth_year: Option<i32>,
    rep: i32,
    expires: Option<&str>,
    league: i32,
    today: &str,
) -> i64 {
    let age = crate::game::age_on(birth, birth_year, today);
    let years = match (expires.and_then(crate::date::parse), crate::date::parse(today)) {
        (Some(e), Some(t)) => (e - t) as f64 / 365.25,
        _ => 0.0,
    };
    // PA negativo no banco = "aleatório": conta como um pouco acima do CA
    let pa = if pa > 0 { pa } else { ca + 10 };
    market_value(ca, pa, age, rep, league, years)
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
        Ok(Some(value_on(ca, pa, birth.as_deref(), birth_year, rep, expires.as_deref(), league, today)))
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
        assert!(wage_demand(0, 0, 3000) >= 300);
    }

    #[test]
    fn revenue_tracks_reputation() {
        assert!((season_revenue(6500) / typical_wage_bill(6500) - 1.25).abs() < 1e-9);
        assert!(season_revenue(9500) > 300_000_000.0 && season_revenue(4000) < 6_000_000.0);
    }
}
