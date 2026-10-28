//! Condições dos negócios (correções dos testes de 24 e 25/10): a proposta de transferência
//! como no diálogo do CM (valor, prestações mensais, percentagem da próxima venda, um valor
//! depois de 10 a 50 jogos, proposta pública e um jogador do plantel na troca), o empréstimo com duração, parte do salário, taxa mensal e opção de compra, e as
//! respostas dos clubes em rondas (contraproposta com condições, paciência que acaba). Os
//! pagamentos a prazo ficam em `payments`, as cláusulas em `sell_on` e `apps_bonus`.

use rand_chacha::ChaCha8Rng;
use rand_core::RngCore;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::deals::Offer;
use super::market::money_text;
use super::{Game, NewsItem, cm_date};
use crate::engine::uniform;
use crate::finance;
use crate::{Error, Result, date};

/// Prestações mensais possíveis (0 = tudo à cabeça).
pub const INSTALMENT_MONTHS: [i32; 5] = [0, 6, 12, 24, 36];
/// Percentagem máxima da próxima venda.
pub const MAX_SELL_ON: i32 = 50;
/// Jogos pelo clube até pagar o valor extra (o treinador escolhe).
pub const APPS_FOR_BONUS: [i32; 5] = [10, 20, 30, 40, 50];
/// Durações de um empréstimo em meses (0 = até ao fim da época).
pub const LOAN_MONTHS: [i32; 3] = [0, 6, 12];
/// Quanto vale cada parte para quem vende: cada ano de prestações tira 6% ao valor, a
/// percentagem da próxima venda vale metade dessa parte do valor do jogador e o valor por
/// jogos vale 70% depois de 10 jogos, menos 1% por cada jogo a mais (pode não chegar a ser pago).
const INSTALMENT_DISCOUNT: f64 = 0.06;
const SELL_ON_WORTH: f64 = 0.5;
const APPS_BONUS_WORTH: f64 = 0.8;
/// Um jogador na troca vale esta parte do valor dele; uma proposta pública por um ambicioso
/// (de um clube maior) desestabiliza-o e o clube baixa o que pede.
pub(super) const EXCHANGE_WORTH: f64 = 0.8;
pub(super) const PUBLIC_DISCOUNT: f64 = 0.05;
/// O clube que vende faz uma contraproposta se a oferta chegar a esta parte do que pede.
const COUNTER_FROM: f64 = 0.6;
/// Propostas pelo mesmo jogador antes de o clube se fartar, quanto o preço desce a cada ronda
/// (até 10%) e quantos dias fica sem querer negociar.
const MAX_ROUNDS: i32 = 3;
const PATIENCE_STEP: f64 = 0.035;
const BORED_DAYS: i32 = 30;
/// Dias para o treinador responder a uma contraproposta.
pub(super) const COUNTER_DAYS: i32 = 7;
/// Dias para os clubes responderem (o primeiro e o último possíveis).
pub(super) const REPLY_DAYS: (i32, i32) = (1, 4);
/// Empréstimos: quem não está nos `LOAN_KEEP` melhores do plantel pode sair; os que estão
/// fora dos 18 melhores (e jovens, ou fora dos 22) saem com metade do salário pago, os outros
/// só com o salário todo e 1% do valor por mês.
const LOAN_KEEP: usize = 14;
const LOAN_EASY_SHARE: i32 = 50;
const LOAN_MONTHLY: f64 = 0.01;
/// A opção de compra tem de chegar a esta parte do que o clube pede.
const OPTION_FROM: f64 = 0.9;

/// Condições de uma proposta de transferência (o diálogo "Proposta de transferência" do CM).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DealTerms {
    #[ts(type = "number")]
    pub fee: i64,
    /// Prestações mensais: 0 (tudo agora), 12, 24 ou 36 meses.
    pub months: i32,
    /// Percentagem da próxima venda (0–50).
    pub sell_on: i32,
    /// Valor pago depois de `apps_needed` jogos pelo clube.
    #[ts(type = "number")]
    pub apps_bonus: i64,
    /// 10, 20, 30, 40 ou 50 jogos (0 = 10).
    #[serde(default)]
    pub apps_needed: i32,
    /// "Tornar proposta pública".
    #[serde(default)]
    pub public: bool,
    /// Um jogador do plantel do treinador que vai na troca.
    #[serde(default)]
    pub exchange: Option<i32>,
}

/// Condições de um pedido de empréstimo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LoanTerms {
    /// Meses: 0 = até ao fim da época, 6 ou 12.
    pub months: i32,
    /// Parte do salário paga por quem pede emprestado (0–100).
    pub wage_share: i32,
    /// Taxa paga por mês ao clube dono.
    #[ts(type = "number")]
    pub monthly_fee: i64,
    /// Opção de compra (0 = sem).
    #[ts(type = "number")]
    pub option_fee: i64,
}

impl Default for LoanTerms {
    fn default() -> Self {
        LoanTerms { months: 0, wage_share: 100, monthly_fee: 0, option_fee: 0 }
    }
}

impl DealTerms {
    pub(super) fn check(&self) -> Result<()> {
        if self.fee <= 0 {
            return Err(Error::Invalid("Indique o valor da proposta.".into()));
        }
        if !INSTALMENT_MONTHS.contains(&self.months) {
            return Err(Error::Invalid("Prestações de 6, 12, 24 ou 36 meses.".into()));
        }
        if self.apps_needed != 0 && !APPS_FOR_BONUS.contains(&self.apps_needed) {
            return Err(Error::Invalid("O valor por jogos é depois de 10, 20, 30, 40 ou 50 jogos.".into()));
        }
        if !(0..=MAX_SELL_ON).contains(&self.sell_on) {
            return Err(Error::Invalid(format!("A percentagem da próxima venda vai até {MAX_SELL_ON}%.")));
        }
        if self.apps_bonus < 0 {
            return Err(Error::Invalid("O valor por jogos não pode ser negativo.".into()));
        }
        Ok(())
    }

    /// Quanto vale a proposta para quem vende um jogador de valor `value`.
    pub fn worth(&self, value: i64) -> f64 {
        self.fee as f64 * (1.0 - INSTALMENT_DISCOUNT * self.months as f64 / 12.0)
            + self.apps_bonus as f64 * self.apps_worth()
            + value as f64 * self.sell_on as f64 / 100.0 * SELL_ON_WORTH
    }

    /// Jogos para o valor extra (0 conta como 10).
    pub fn games(&self) -> i32 {
        if self.apps_needed > 0 { self.apps_needed } else { APPS_FOR_BONUS[0] }
    }

    fn apps_worth(&self) -> f64 {
        (APPS_BONUS_WORTH - 0.01 * self.games() as f64).max(0.3)
    }

    /// O que sai do orçamento da direção: a parte do valor paga nos primeiros 12 meses e o
    /// valor por jogos.
    pub fn budget_cost(&self) -> i64 {
        self.first_year() + self.apps_bonus
    }

    fn first_year(&self) -> i64 {
        if self.months <= 0 { self.fee } else { self.fee * self.months.min(12) as i64 / self.months as i64 }
    }

    /// As condições em palavras ("€10M em 24 prestações, 15% da próxima venda...").
    pub fn text(&self) -> String {
        let mut parts = vec![money_text(self.fee)];
        if self.months > 0 {
            parts[0] = format!("{} em {} prestações mensais", parts[0], self.months);
        }
        if self.sell_on > 0 {
            parts.push(format!("{}% da próxima venda", self.sell_on));
        }
        if self.apps_bonus > 0 {
            parts.push(format!("{} depois de {} jogos", money_text(self.apps_bonus), self.games()));
        }
        join(&parts)
    }
}

impl LoanTerms {
    pub(super) fn check(&self) -> Result<()> {
        if !LOAN_MONTHS.contains(&self.months) {
            return Err(Error::Invalid("Empréstimos até ao fim da época, de 6 ou de 12 meses.".into()));
        }
        if !(0..=100).contains(&self.wage_share) {
            return Err(Error::Invalid("A parte do salário vai de 0 a 100%.".into()));
        }
        if self.monthly_fee < 0 || self.option_fee < 0 {
            return Err(Error::Invalid("Os valores não podem ser negativos.".into()));
        }
        Ok(())
    }

    pub fn text(&self) -> String {
        let mut parts = vec![match self.months {
            0 => "até ao fim da época".to_string(),
            m => format!("por {m} meses"),
        }];
        parts.push(format!("{}% do salário pago", self.wage_share));
        if self.monthly_fee > 0 {
            parts.push(format!("{} por mês", money_text(self.monthly_fee)));
        }
        if self.option_fee > 0 {
            parts.push(format!("opção de compra por {}", money_text(self.option_fee)));
        }
        join(&parts)
    }
}

/// "a, b e c".
fn join(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} e {last}", rest.join(", ")),
    }
}

/// A resposta de um clube a uma proposta de transferência.
#[derive(Debug, PartialEq)]
pub(super) enum SellerReply {
    Accept,
    Counter(DealTerms),
    Reject,
    /// Fartou-se das negociações.
    Bored,
}

/// O clube que vende responde a `t` (`rounds` = propostas anteriores pelo jogador): aceita se
/// valer o que pede (um pouco menos a cada ronda), faz uma contraproposta se não estiver longe
/// (às vezes, por um jovem, pede uma parte da próxima venda) e farta-se à terceira.
/// `extra`: o que vale o jogador da troca.
pub(super) fn seller_reply(t: &DealTerms, asking: i64, value: i64, age: i32, rounds: i32, extra: f64, rng: &mut ChaCha8Rng) -> SellerReply {
    let target = asking as f64 * (1.0 - PATIENCE_STEP * rounds as f64).max(0.9);
    let worth = t.worth(value) + extra;
    if worth >= target {
        return SellerReply::Accept;
    }
    if rounds + 1 >= MAX_ROUNDS {
        return SellerReply::Bored;
    }
    if worth < COUNTER_FROM * asking as f64 {
        return SellerReply::Reject;
    }
    let mut c = *t;
    if age <= 23 && c.sell_on == 0 && uniform(rng) < 0.5 {
        c.sell_on = 10 + 5 * (rng.next_u32() % 3) as i32;
    }
    let rest = target - c.apps_bonus as f64 * c.apps_worth() - value as f64 * c.sell_on as f64 / 100.0 * SELL_ON_WORTH - extra;
    let fee = rest / (1.0 - INSTALMENT_DISCOUNT * c.months as f64 / 12.0);
    // Um pouco acima: arredondada, a contraproposta tem de valer o que o clube pede
    c.fee = finance::round_money(fee * 1.01).max(t.fee);
    SellerReply::Counter(c)
}

/// Um clube que compra paga até mais 30% do que valia a proposta dele quando o treinador pede mais.
pub(super) const RAISE_UP_TO: f64 = 1.3;

/// A resposta de um clube que propôs ao "Pedir Mais" do treinador.
#[derive(Debug, PartialEq)]
pub(super) enum BuyerReply {
    Accept,
    Counter(DealTerms),
    Withdraw,
}

/// `offered`: quanto valia a proposta do clube; paga até [`RAISE_UP_TO`] disso e o que o
/// orçamento deixa (`budget`, no que conta no primeiro ano). Senão responde a meio caminho,
/// nas condições que o treinador pediu, e à terceira desiste.
pub(super) fn buyer_reply(asked: &DealTerms, value: i64, offered: f64, budget: f64, rounds: i32) -> BuyerReply {
    let max = offered * RAISE_UP_TO;
    let worth = asked.worth(value);
    if worth <= max && asked.budget_cost() as f64 <= budget {
        return BuyerReply::Accept;
    }
    if rounds + 1 >= MAX_ROUNDS {
        return BuyerReply::Withdraw;
    }
    let target = ((offered + worth) / 2.0).min(max);
    let mut c = *asked;
    let rest = target - c.apps_bonus as f64 * c.apps_worth() - value as f64 * c.sell_on as f64 / 100.0 * SELL_ON_WORTH;
    c.fee = finance::round_money(rest / (1.0 - INSTALMENT_DISCOUNT * c.months as f64 / 12.0));
    while c.fee > 0 && c.budget_cost() as f64 > budget {
        c.fee = finance::round_money(c.fee as f64 * 0.9);
    }
    if c.fee <= 0 || c.worth(value) <= offered {
        return BuyerReply::Withdraw;
    }
    BuyerReply::Counter(c)
}

/// A resposta de um clube a um pedido de empréstimo.
#[derive(Debug, PartialEq)]
pub(super) enum LoanReply {
    Accept,
    Counter(LoanTerms),
    Reject,
}

/// `rank`: lugar do jogador no plantel (0 = o melhor). Os indispensáveis não saem; os de fora
/// dos 18 melhores (jovens, ou fora dos 22) saem com metade do salário pago; os outros só com
/// o salário todo e uma taxa mensal. Uma opção de compra tem de chegar perto do que o clube pede.
pub(super) fn loan_reply(t: &LoanTerms, rank: usize, age: i32, value: i64, asking: i64, rounds: i32) -> LoanReply {
    if rank < LOAN_KEEP {
        return LoanReply::Reject;
    }
    let easy = rank >= 18 && (age <= 23 || rank >= 22);
    let mut want = *t;
    if easy {
        want.wage_share = want.wage_share.max(LOAN_EASY_SHARE);
    } else {
        want.wage_share = 100;
        want.monthly_fee = want.monthly_fee.max(finance::round_money(value as f64 * LOAN_MONTHLY));
    }
    if want.option_fee > 0 {
        want.option_fee = want.option_fee.max(finance::round_money(asking as f64 * OPTION_FROM));
    }
    if want == *t {
        LoanReply::Accept
    } else if rounds + 1 >= MAX_ROUNDS {
        LoanReply::Reject
    } else {
        LoanReply::Counter(want)
    }
}

impl Offer {
    pub(super) fn deal_terms(&self) -> DealTerms {
        DealTerms {
            fee: self.fee,
            months: self.months,
            sell_on: self.sell_on,
            apps_bonus: self.apps_bonus,
            apps_needed: self.apps_needed,
            public: self.public,
            exchange: self.exchange,
        }
    }

    pub(super) fn loan_terms(&self) -> LoanTerms {
        LoanTerms { months: self.loan_months, wage_share: self.wage_share, monthly_fee: self.monthly_fee, option_fee: self.option_fee }
    }
}

impl Game {
    /// Dia da resposta de um clube: de `lo` a `hi` dias, fixo por jogador e por dia.
    pub(super) fn reply_on(&self, person: i32, (lo, hi): (i32, i32)) -> String {
        let day = date::parse(&self.date).unwrap_or(0) as u64;
        let mut rng = self.deal_rng(person, 0x2E91 ^ day);
        let days = lo + (rng.next_u32() % (hi - lo + 1) as u32) as i32;
        date::add_days(&self.date, days).unwrap_or_default()
    }

    /// Grava as condições de uma proposta (transferência ou empréstimo).
    pub(super) fn set_terms(&self, id: i32, d: &DealTerms, l: &LoanTerms) -> Result<()> {
        self.world.conn.execute(
            "UPDATE offers SET fee = ?, months = ?, sell_on = ?, apps_bonus = ?, loan_months = ?, wage_share = ?, monthly_fee = ?,
                    option_fee = ?, apps_needed = ?, public = ?, exchange = ? WHERE id = ?",
            params![
                d.fee,
                d.months,
                d.sell_on,
                d.apps_bonus,
                l.months,
                l.wage_share,
                l.monthly_fee,
                l.option_fee,
                d.games(),
                d.public,
                d.exchange,
                id
            ],
        )?;
        Ok(())
    }

    /// Propostas anteriores do treinador pelo jogador no último mês (as rondas da negociação),
    /// e até quando o clube não quer negociar, se se fartou.
    pub(super) fn past_rounds(&self, person: i32, kind: &str) -> Result<(i32, Option<String>)> {
        let since = date::add_days(&self.date, -BORED_DAYS).unwrap_or_default();
        let last: Option<(i32, String, Option<String>)> = self
            .world
            .conn
            .query_row(
                "SELECT rounds, status, answer_on FROM offers WHERE person_id = ? AND buyer = ? AND kind = ? AND created >= ?
                 AND status IN ('rejected', 'bored') ORDER BY id DESC LIMIT 1",
                params![person, self.club_id, kind, since],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        Ok(match last {
            Some((_, status, until)) if status == "bored" => (MAX_ROUNDS, until.filter(|u| u.as_str() > self.date.as_str())),
            Some((rounds, _, _)) => (rounds + 1, None),
            None => (0, None),
        })
    }

    /// O clube farta-se: não quer voltar a negociar durante [`BORED_DAYS`].
    pub(super) fn bored(&self, id: i32) -> Result<String> {
        let until = date::add_days(&self.date, BORED_DAYS).unwrap_or_default();
        self.set_status(id, "bored", Some(&until))?;
        Ok(until)
    }

    /// Depois de uma transferência com condições: as prestações que faltam saem das contas de
    /// hoje e ficam marcadas para os meses seguintes; a percentagem da próxima venda e o valor
    /// por jogos ficam guardados.
    pub(super) fn settle_terms(&self, person: i32, buyer: i32, seller: i32, t: &DealTerms) -> Result<()> {
        if t.months > 0 {
            let each = t.fee / t.months as i64;
            let deferred = each * (t.months as i64 - 1);
            self.book(buyer, "purchases", -deferred)?;
            self.book(seller, "sales", -deferred)?;
            for m in 1..t.months {
                let due = date::add_days(&self.date, 30 * m).unwrap_or_default();
                self.world.conn.execute(
                    "INSERT INTO payments (person_id, payer, payee, amount, due) VALUES (?, ?, ?, ?, ?)",
                    params![person, buyer, seller, each, due],
                )?;
            }
        }
        if t.sell_on > 0 {
            self.world.conn.execute(
                "INSERT OR REPLACE INTO sell_on (person_id, payer, payee, pct) VALUES (?, ?, ?, ?)",
                params![person, buyer, seller, t.sell_on],
            )?;
        }
        if t.apps_bonus > 0 {
            let apps = self.squads.states.get(&person).map_or(0, |s| s.apps);
            self.world.conn.execute(
                "INSERT OR REPLACE INTO apps_bonus (person_id, payer, payee, amount, apps_at, needed) VALUES (?, ?, ?, ?, ?, ?)",
                params![person, buyer, seller, t.apps_bonus, apps, t.games()],
            )?;
        }
        Ok(())
    }

    /// Um jogador sai do clube `from` por `fee`: o clube que o vendeu com uma percentagem da
    /// próxima venda recebe a parte dele, e o valor por jogos que faltava cai.
    pub(super) fn clauses_on_move(&self, person: i32, from: Option<i32>, fee: i64) -> Result<()> {
        let clause: Option<(i32, i32, i32)> = self
            .world
            .conn
            .query_row("SELECT payer, payee, pct FROM sell_on WHERE person_id = ?", [person], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        if let Some((payer, payee, pct)) = clause
            && Some(payer) == from
        {
            let share = fee * pct as i64 / 100;
            self.book(payer, "sales", -share)?;
            self.book(payee, "sales", share)?;
            self.world.conn.execute("DELETE FROM sell_on WHERE person_id = ?", [person])?;
        }
        self.world.conn.execute("DELETE FROM apps_bonus WHERE person_id = ? AND payer IS ?", params![person, from])?;
        Ok(())
    }

    /// Os pagamentos do dia: prestações e taxas de empréstimo que vencem, e os valores por
    /// jogos que chegaram aos 10 jogos. Devolve as mensagens para o clube do treinador.
    pub(super) fn payments_day(&mut self) -> Result<Vec<(String, String)>> {
        let due: Vec<(i32, i32, i32, i32, i64)> = self
            .world
            .conn
            .prepare_cached("SELECT id, person_id, payer, payee, amount FROM payments WHERE due <= ? ORDER BY id")?
            .query_map([&self.date], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, _, payer, payee, amount) in due {
            self.book(payer, "purchases", amount)?;
            self.book(payee, "sales", amount)?;
            self.world.conn.execute("DELETE FROM payments WHERE id = ?", [id])?;
        }
        let bonuses: Vec<(i32, i32, i32, i64, i32, i32)> = self
            .world
            .conn
            .prepare_cached("SELECT person_id, payer, payee, amount, apps_at, needed FROM apps_bonus ORDER BY person_id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut news = Vec::new();
        for (person, payer, payee, amount, at, needed) in bonuses {
            let apps = self.squads.states.get(&person).map_or(0, |s| s.apps);
            if apps - at < needed {
                continue;
            }
            self.book(payer, "purchases", amount)?;
            self.book(payee, "sales", amount)?;
            self.world.conn.execute("DELETE FROM apps_bonus WHERE person_id = ?", [person])?;
            if payer == self.club_id || payee == self.club_id {
                let name = self.person_name(person)?;
                let (who, verb) = if payer == self.club_id {
                    (self.club(payee).short_name, "pagou ao")
                } else {
                    (self.club(payer).short_name, "recebeu do")
                };
                news.push((
                    format!("Valor por jogos de {name}"),
                    format!(
                        "{name} cumpriu os jogos combinados: o clube {verb} {who} {}, como ficou na transferência.",
                        money_text(amount)
                    ),
                ));
            }
        }
        Ok(news)
    }

    /// Antes de os jogos da época voltarem a zero: o que falta para o valor por jogos passa
    /// para a época nova.
    pub(super) fn carry_apps_bonus(&self) -> Result<()> {
        let rows: Vec<(i32, i32)> = self
            .world
            .conn
            .prepare_cached("SELECT person_id, apps_at FROM apps_bonus")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (person, at) in rows {
            let apps = self.squads.states.get(&person).map_or(at, |s| s.apps);
            self.world.conn.execute(
                "UPDATE apps_bonus SET needed = MAX(1, needed - ?), apps_at = 0 WHERE person_id = ?",
                params![apps - at, person],
            )?;
        }
        Ok(())
    }

    /// Condições de um empréstimo que começa hoje: a data de regresso, a parte do salário, a
    /// opção de compra e as taxas mensais. Devolve a data de regresso.
    pub(super) fn loan_terms_start(&self, person: i32, parent: i32, to: i32, t: &LoanTerms, season_end: &str) -> Result<String> {
        let until = match t.months {
            0 => season_end.to_string(),
            m => date::add_days(&self.date, 30 * m).unwrap_or_else(|| season_end.to_string()),
        };
        self.world.conn.execute(
            "UPDATE loans SET until = ?, wage_share = ?, option_fee = ? WHERE person_id = ?",
            params![until, t.wage_share, t.option_fee, person],
        )?;
        if t.monthly_fee > 0 {
            let mut m = 0;
            loop {
                let due = date::add_days(&self.date, 30 * m).unwrap_or_default();
                if due.as_str() > until.as_str() || m > 24 {
                    break;
                }
                self.world.conn.execute(
                    "INSERT INTO payments (person_id, payer, payee, amount, due) VALUES (?, ?, ?, ?, ?)",
                    params![person, to, parent, t.monthly_fee, due],
                )?;
                m += 1;
            }
        }
        Ok(until)
    }

    /// Salários dos emprestados que o clube dono continua a pagar em parte: (clube, a mais).
    pub(super) fn loan_wage_shifts(&self) -> Result<Vec<(i32, i32, i64)>> {
        Ok(self
            .world
            .conn
            .prepare_cached(
                "SELECT p.club_id, l.parent_id, COALESCE(p.wage, 0) * (100 - l.wage_share) / 100
                 FROM loans l JOIN people p ON p.id = l.person_id WHERE l.wage_share < 100 AND p.club_id IS NOT NULL ORDER BY l.person_id",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// A opção de compra de um emprestado ao clube do treinador.
    pub(super) fn option_of(&self, person: i32) -> Result<Option<(i32, i64)>> {
        Ok(self
            .world
            .conn
            .query_row(
                "SELECT l.parent_id, l.option_fee FROM loans l JOIN people p ON p.id = l.person_id
                 WHERE l.person_id = ? AND p.club_id = ? AND l.option_fee > 0",
                params![person, self.club_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    /// "Exercer Opção de Compra": o negócio com o clube fica fechado pelo valor da opção e
    /// falta o contrato com o jogador.
    pub fn exercise_option(&mut self, person: i32) -> Result<super::TransferView> {
        let (parent, fee) = self.option_of(person)?.ok_or_else(|| Error::Invalid("Não há opção de compra deste jogador.".into()))?;
        if !self.window_open_for(self.club_id)? {
            return Err(Error::Invalid("A janela de transferências está fechada.".into()));
        }
        if self.open_offer(person)?.is_some() {
            return Err(Error::Invalid("Já há um negócio em curso por este jogador.".into()));
        }
        self.check_budget(fee)?;
        let until = date::add_days(&self.date, super::deals::AGREED_DAYS).unwrap_or_default();
        self.world.conn.execute(
            "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('transfer', ?, ?, ?, ?, 'agreed', ?, ?)",
            params![person, self.club_id, parent, fee, until, self.date],
        )?;
        let name = self.person_name(person)?;
        self.post_news(
            format!("Opção de compra de {name}"),
            format!(
                "O {} aceita vender {name} por {}, como combinado no empréstimo. Negoceie o contrato com o jogador até {}.",
                self.club(parent).short_name,
                money_text(fee),
                cm_date(&until)
            ),
            Some(self.world.conn.last_insert_rowid() as i32),
        );
        self.unsaved = true;
        self.transfer_view(person)
    }

    /// Um mês antes do fim de um empréstimo com opção de compra, a mensagem que a lembra.
    pub(super) fn option_reminders(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let day = date::add_days(&self.date, 30).unwrap_or_default();
        let rows: Vec<(i32, i32, i64)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT l.person_id, l.parent_id, l.option_fee FROM loans l JOIN people p ON p.id = l.person_id
                 WHERE p.club_id = ? AND l.option_fee > 0 AND l.until = ? ORDER BY l.person_id",
            )?
            .query_map(params![self.club_id, day], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (person, parent, fee) in rows {
            let name = self.person_name(person)?;
            let n = self.news_item(
                *next_id,
                "message",
                format!("Opção de compra de {name}"),
                format!(
                    "O empréstimo de {name} acaba a {}. Pode exercer a opção de compra ({} ao {}) na aba Transferência do perfil dele.",
                    cm_date(&day),
                    money_text(fee),
                    self.club(parent).short_name
                ),
            );
            fresh.push(n);
            *next_id += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rand_core::SeedableRng;

    use super::*;

    #[test]
    fn instalments_and_clauses_change_the_worth() {
        let cash = DealTerms { fee: 10_000_000, ..Default::default() };
        let spread = DealTerms { months: 24, ..cash };
        assert!(spread.worth(10_000_000) < cash.worth(10_000_000));
        assert_eq!(spread.budget_cost(), 5_000_000, "só os primeiros 12 meses contam no orçamento");
        let sell_on = DealTerms { sell_on: 20, ..cash };
        assert!(sell_on.worth(10_000_000) > cash.worth(10_000_000));
        assert_eq!(spread.text(), "€10,0M em 24 prestações mensais");
    }

    #[test]
    fn the_seller_counters_then_gets_bored() {
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        let low = DealTerms { fee: 7_000_000, ..Default::default() };
        let SellerReply::Counter(c) = seller_reply(&low, 10_000_000, 10_000_000, 28, 0, 0.0, &mut rng) else { panic!("contraproposta") };
        assert!(c.worth(10_000_000) >= 10_000_000.0, "a contraproposta vale o que pede: {c:?}");
        assert_eq!(seller_reply(&c, 10_000_000, 10_000_000, 28, 1, 0.0, &mut rng), SellerReply::Accept);
        assert_eq!(seller_reply(&DealTerms { fee: 1_000_000, ..low }, 10_000_000, 10_000_000, 28, 0, 0.0, &mut rng), SellerReply::Reject);
        assert_eq!(seller_reply(&low, 10_000_000, 10_000_000, 28, 2, 0.0, &mut rng), SellerReply::Bored);
    }

    #[test]
    fn instalments_and_sell_on_reach_the_books() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-condicoes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Condições".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
        };
        let mut g = Game::create(&path, &dir, &new, 5).unwrap();
        let (person, seller, other): (i32, i32, i32) = g
            .world
            .conn
            .query_row(
                "SELECT p.id, p.club_id, (SELECT id FROM clubs WHERE division_id = c.division_id AND id NOT IN (7568, c.id) LIMIT 1)
                 FROM people p JOIN clubs c ON c.id = p.club_id
                 WHERE c.division_id = (SELECT division_id FROM clubs WHERE id = 7568) AND c.id <> 7568 AND p.is_player = 1 LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        let terms = DealTerms { fee: 12_000_000, months: 12, sell_on: 20, ..Default::default() };
        let (mine, theirs) = (g.cash_of(7568).unwrap(), g.cash_of(seller).unwrap());
        let kind = g.kind_of(7568).unwrap();
        g.execute_move(person, Some(seller), Some(7568), terms.fee, 10_000, 3, "transfer", kind).unwrap();
        g.settle_terms(person, 7568, seller, &terms).unwrap();
        assert_eq!(g.cash_of(7568).unwrap(), mine - 1_000_000, "hoje só a primeira prestação");
        assert_eq!(g.cash_of(seller).unwrap(), theirs + 1_000_000);
        g.date = date::add_days(&g.date, 30).unwrap();
        g.payments_day().unwrap();
        assert_eq!(g.cash_of(7568).unwrap(), mine - 2_000_000, "a segunda prestação um mês depois");
        // Vendido de novo: 20% da venda para o clube de onde veio
        g.execute_move(person, Some(7568), Some(other), 5_000_000, 10_000, 3, "transfer", kind).unwrap();
        assert_eq!(g.cash_of(seller).unwrap(), theirs + 2_000_000 + 1_000_000);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_buyer_pays_a_bit_more_or_meets_halfway() {
        let offer = DealTerms { fee: 10_000_000, ..Default::default() };
        let offered = offer.worth(10_000_000);
        let more = DealTerms { fee: 12_000_000, ..offer };
        assert_eq!(buyer_reply(&more, 10_000_000, offered, 50_000_000.0, 0), BuyerReply::Accept);
        let greedy = DealTerms { fee: 20_000_000, ..offer };
        let BuyerReply::Counter(c) = buyer_reply(&greedy, 10_000_000, offered, 50_000_000.0, 0) else { panic!("meio caminho") };
        assert!(c.fee > 10_000_000 && c.fee <= 13_000_000, "{c:?}");
        assert_eq!(buyer_reply(&greedy, 10_000_000, offered, 50_000_000.0, 2), BuyerReply::Withdraw);
        // Pedir uma percentagem também conta: 30% vale +15%, dentro do que o clube paga a mais
        let sell_on = DealTerms { sell_on: 30, ..offer };
        assert_eq!(buyer_reply(&sell_on, 10_000_000, offered, 50_000_000.0, 0), BuyerReply::Accept);
        let both = DealTerms { fee: 12_000_000, sell_on: 30, ..offer };
        assert!(matches!(buyer_reply(&both, 10_000_000, offered, 50_000_000.0, 0), BuyerReply::Counter(c) if c.sell_on == 30));
    }

    #[test]
    fn loans_need_the_right_terms() {
        let t = LoanTerms::default();
        assert_eq!(loan_reply(&t, 3, 22, 1_000_000, 1_000_000, 0), LoanReply::Reject, "titular não sai");
        assert_eq!(loan_reply(&t, 20, 21, 1_000_000, 1_000_000, 0), LoanReply::Accept);
        let half = LoanTerms { wage_share: 25, ..t };
        assert_eq!(loan_reply(&half, 20, 21, 1_000_000, 1_000_000, 0), LoanReply::Counter(LoanTerms { wage_share: 50, ..t }));
        let LoanReply::Counter(c) = loan_reply(&t, 15, 27, 1_000_000, 1_000_000, 0) else { panic!("contraproposta") };
        assert_eq!((c.wage_share, c.monthly_fee), (100, 10_000));
    }
}
