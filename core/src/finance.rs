//! Dinheiro (marco 3.8; economia moderna no 3.22): valor de mercado e salário pedido de um
//! jogador, e as receitas e despesas de um clube. A folha salarial anual típica cresce com a
//! reputação, ≈ e^(10,06 + 0,000929 × reputação) no banco (o jogo paga `WAGE_SCALE` vezes isso)
//! e serve de referência ao salário pedido. As receitas de um clube são as reais (a tabela de
//! `core/economy.json`) ou a curva por reputação vezes o peso da liga ([`club_revenue`]).
//! Números e decisões em docs/roadmap/3.22-economia-moderna.md.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
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

/// Os salários do jogo sobre os do banco. O CM paga 3× (o Kane tem €75.000 por semana no banco
/// e €875.000 por mês no CM); o NextCM paga os de hoje (3.22): a folha do City fica em ~€410M
/// por ano, ~50% das receitas reais.
pub const WAGE_SCALE: f64 = 7.0;
/// Receitas mínimas de uma época (clubes do patch sem salários).
pub const MIN_REVENUE: f64 = 900_000.0;
/// O saldo de um clube no 1.º dia de um jogo novo, sobre o do banco: a mesma escala dos salários
/// (o CM começa com ~3×).
pub const START_CASH: f64 = WAGE_SCALE;
/// O saldo do 1.º dia fica até esta parte das receitas (o City do banco começaria com €805M).
pub const START_CASH_CAP: f64 = 0.15;

/// O saldo do 1.º dia de um clube: o do banco × `START_CASH`, até `START_CASH_CAP` das receitas
/// (as dívidas ficam como estão).
pub fn start_cash(bank: i64, revenue: f64) -> i64 {
    if bank <= 0 { bank } else { (bank as f64 * START_CASH).min(START_CASH_CAP * revenue) as i64 }
}
/// A folha salarial que a direção aceita, sobre as receitas de uma época (os clubes europeus
/// andam pelos 55–70%).
pub const WAGE_SHARE: f64 = 0.7;

/// Partes das receitas de uma época normal (3.22, como nos clubes reais: comercial ~40%, TV e
/// prémios ~40%, bilheteira o resto). A TV é igual para todos na liga (a média das receitas dela).
pub const MERCH_SHARE: f64 = 0.42;
pub const TV_SHARE: f64 = 0.25;
/// Prémios, sobre as receitas de referência de cada clube: na liga de 0,6× (último) a 1,2×
/// (campeão) esta parte; nas taças, metade a quem joga; no fim de taças e continentais, mais
/// 1× ao campeão e ½× ao vice. Nas continentais cada jogo paga `CONTINENTAL_MATCH` desta parte
/// aos dois (~1,2% das receitas por jogo).
pub const LEAGUE_PRIZES: f64 = 0.17;
pub const CUP_PRIZES: f64 = 0.04;
pub const CONTINENTAL_PRIZES: f64 = 0.2;
pub const CONTINENTAL_MATCH: f64 = 0.06;
/// Despesas fixas: staff sobre a folha dos jogadores (entra no Salário), estádio sobre as receitas
/// e o resto da operação (viagens, comercial, administração: ~25% das receitas no Real e no United
/// de 2025) em Outros.
pub const STAFF_SHARE: f64 = 0.12;
pub const STADIUM_SHARE: f64 = 0.04;
pub const RUNNING_SHARE: f64 = 0.15;
/// Juros mensais de um saldo negativo e de um positivo (o Bayern do CM: ~€0,6M por mês com
/// ~€220M no banco).
pub const MONTHLY_INTEREST: f64 = 0.01;
pub const SAVINGS_INTEREST: f64 = 0.0025;
/// Dividendos aos accionistas, a 1/7: esta parte do saldo acima da reserva (épocas de receitas).
pub const DIVIDEND_SHARE: f64 = 0.25;
pub const DIVIDEND_RESERVE: f64 = 0.5;

/// Preço médio de um bilhete pela reputação do clube da casa: €4 nos clubes sem nome, ~€25
/// com 7000 e €54 com 10 000 (o Bayern do CM fez €80,9M de bilheteira numa época).
pub fn ticket_price(reputation: i32) -> f64 {
    4.0 + 50.0 * (reputation.clamp(0, 10_000) as f64 / 10_000.0).powf(2.5)
}

/// Folha salarial anual típica de um clube com esta reputação (0–10 000).
pub fn typical_wage_bill(reputation: i32) -> f64 {
    WAGE_SCALE * (10.062 + 0.000929 * reputation.clamp(0, 10_000) as f64).exp()
}

/// Receitas de uma época normal pela reputação do clube (0–10 000), numa liga de peso 1:
/// ~€850M com 10 000, ~€260M com 8000, €77M com 6000, €13M com 4000, ~€2M com 2000.
pub fn revenue_curve(reputation: i32) -> f64 {
    let r = reputation.clamp(0, 10_000) as f64;
    let ln = if r >= 6000.0 { 6.75 + 0.0006 * (r - 10_000.0) } else { 4.35 + 0.0009 * (r - 6000.0) };
    (ln.exp() * 1e6).max(MIN_REVENUE)
}

/// As receitas reais e os pesos das ligas (`core/economy.json`).
#[derive(Deserialize)]
struct Economy {
    leagues: HashMap<String, f64>,
    clubs: Vec<RealRevenue>,
}

#[derive(Deserialize)]
struct RealRevenue {
    id: i32,
    division: i32,
    /// Milhões de euros.
    revenue: f64,
}

fn economy() -> &'static Economy {
    static ECONOMY: OnceLock<Economy> = OnceLock::new();
    ECONOMY.get_or_init(|| serde_json::from_str(include_str!("../economy.json")).unwrap_or_else(|e| panic!("economy.json inválido: {e}")))
}

/// Peso de uma liga nas receitas (1 nas que não estão na tabela).
pub fn league_weight(division: Option<i32>) -> f64 {
    division.and_then(|d| economy().leagues.get(&d.to_string()).copied()).unwrap_or(1.0)
}

/// Receitas de uma época normal de um clube: as reais (fora da divisão delas, vezes a razão dos
/// pesos, para quem desce ou sobe) ou a curva por reputação vezes o peso da liga.
pub fn club_revenue(club: i32, reputation: i32, division: Option<i32>) -> f64 {
    let weight = league_weight(division);
    match economy().clubs.iter().find(|c| c.id == club) {
        Some(c) => (c.revenue * 1e6 * weight / league_weight(Some(c.division))).max(MIN_REVENUE),
        None => revenue_curve(reputation) * weight,
    }
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

/// Valor de mercado (forma da curva ajustada aos prints do CM de 9/11, escala de hoje): CA pela
/// curva 199 × (CA/10)^4,5 (CA 80 ≈ €2,3M, 140 ≈ €29M, 180 ≈ €89M na liga mais forte); peso da
/// liga (reputação 1–20, as mais fracas valem ~¼); jovens com potencial acima do CA valem mais,
/// e a partir dos 28 anos menos, cada vez mais depressa depois dos 30; a reputação do jogador
/// (0–200) de 80% a 120%; o contrato pesa (anos/5)^0,5 (2 anos 63%, 7 anos 118%); sem contrato, nada.
pub fn market_value(ca: i32, pa: i32, age: i32, player_rep: i32, league_rep: i32, years_left: f64) -> i64 {
    if years_left <= 0.0 {
        return 0;
    }
    let ca = ca.clamp(1, 200) as f64;
    let base = 199.0 * (ca / 10.0).powf(4.5);
    let league = 0.15 + 0.85 * (league_rep.clamp(1, 20) as f64 / 20.0).powf(1.5);
    let gap = (pa.max(0) as f64 - ca).max(0.0);
    let age_factor = match age {
        ..=21 => 1.0 + gap / 45.0,
        22..=23 => 1.0 + gap / 160.0,
        24..=27 => 1.0,
        _ => {
            let past = (age - 27) as f64;
            let late = (age - 30).max(0) as f64;
            (1.0 - 0.05 * past - 0.02 * late * late).max(0.15)
        }
    };
    let rep = 0.8 + 0.4 * player_rep.clamp(0, 200) as f64 / 200.0;
    let contract = (years_left / 5.0).sqrt().clamp(0.3, 1.2);
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
    /// Fair-play financeiro: o resultado das últimas 3 épocas (sem dividendos), o limite (as
    /// perdas permitidas, negativo) e se o clube está proibido de contratar.
    #[ts(type = "number")]
    pub fair_play_result: i64,
    #[ts(type = "number")]
    pub fair_play_limit: i64,
    pub fair_play_banned: bool,
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
        assert!(market_value(80, 85, 27, 20, 5, 2.0) < 1_000_000);
        // Idade, contrato e liga
        assert!(market_value(140, 150, 34, 100, 20, 3.0) < market_value(140, 150, 26, 100, 20, 3.0) / 3);
        assert!(market_value(140, 150, 26, 100, 20, 6.0) > market_value(140, 150, 26, 100, 20, 2.0) * 3 / 2);
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
    fn revenue_tracks_reputation_league_and_the_real_table() {
        assert!(revenue_curve(9500) > 500_000_000.0 && revenue_curve(4000) < 20_000_000.0);
        assert!(revenue_curve(0) >= MIN_REVENUE);
        // Um clube médio da Premier fatura mais do que um da Liga Portugal com a mesma reputação
        assert!(club_revenue(-1, 6500, Some(7)) > club_revenue(-1, 6500, Some(46)) * 1.4);
        // O City pela tabela; se descer, as receitas descem com o peso da liga
        assert!((club_revenue(6269, 10_000, Some(7)) - 829_300_000.0).abs() < 1.0);
        assert!(club_revenue(6269, 10_000, Some(8)) < club_revenue(6269, 10_000, Some(7)));
    }
}
