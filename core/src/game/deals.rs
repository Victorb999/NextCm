//! Mercado do treinador (marco 3.8, passo 3): propostas por jogadores de outros clubes,
//! empréstimos pedidos, propostas dos clubes da IA pelos jogadores do plantel, negociação
//! do contrato (contratações e renovações) e lista de transferências. Os outros clubes
//! respondem em 1 a 4 dias, com uma mensagem que traz os botões (`news.offer_id`); as
//! condições e as rondas da negociação estão em `negotiation.rs`.
//! Regras e números no detalhe do 3.8 no ROADMAP.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::market::{
    MIN_ROLE, contract_until, contract_years, leaves_europe, leaving_europe, loan_until, money_text, role_of, window_open,
};
use super::negotiation::{
    BuyerReply, DealTerms, EXCHANGE_WORTH, LoanReply, LoanTerms, PUBLIC_DISCOUNT, REPLY_DAYS, SellerReply, buyer_reply, loan_reply,
    seller_reply,
};
use super::{Game, NewsItem, age_on, cm_date};
use crate::engine::uniform;
use crate::finance::{self, wage_demand};
use crate::person::ClubRef;
use crate::personality;
use crate::regulation::SeasonKind;
use crate::{Error, Result, date};

/// Dias que uma proposta recebida espera pela resposta do treinador.
const INCOMING_DAYS: i32 = 7;
/// Dias para fechar o contrato depois de o clube aceitar (ou para responder à contraproposta).
pub(super) const AGREED_DAYS: i32 = 14;
/// Propostas recusadas antes de o jogador romper as negociações, e dias até voltar a falar.
pub(super) const TRIES: i32 = 3;
pub(super) const COOLDOWN_DAYS: i32 = 30;
/// Probabilidades, num dia de mercado: proposta por cada jogador da lista de transferências,
/// pedido de empréstimo por cada um da lista de empréstimos, e uma proposta por alguém de fora.
const LISTED_BID: f64 = 0.35;
const LOAN_BID: f64 = 0.3;
const UNLISTED_BID: f64 = 0.12;
/// Dias no clube antes de o clube o vender pelo preço normal (antes, +25%).
const SETTLE_DAYS: i32 = 180;

/// Um botão de uma proposta ("accept", "reject", "counter", "pay", "withdraw", "negotiate").
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct OfferAction {
    pub key: String,
    pub label: String,
}

/// Uma proposta, como a mensagem e o perfil do jogador a mostram.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct OfferView {
    pub id: i32,
    /// "transfer" ou "loan"
    pub kind: String,
    /// Proposta de outro clube por um jogador do treinador.
    pub incoming: bool,
    pub person_id: i32,
    pub name: String,
    pub buyer: ClubRef,
    pub seller: Option<ClubRef>,
    #[ts(type = "number")]
    pub fee: i64,
    pub status: String,
    /// A situação em palavras.
    pub text: String,
    pub actions: Vec<OfferAction>,
    /// As condições (a última proposta ou a contraproposta do clube).
    pub deal: DealTerms,
    pub loan: LoanTerms,
    /// Propostas anteriores pelo jogador nesta negociação.
    pub rounds: i32,
}

/// Aba Transferência do perfil de um jogador.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TransferView {
    pub person_id: i32,
    /// Do plantel do treinador (e não emprestado por outro clube).
    pub mine: bool,
    /// Sem clube.
    pub free: bool,
    /// O clube onde joga.
    pub club: Option<ClubRef>,
    #[ts(type = "number")]
    pub value: i64,
    #[ts(type = "number")]
    pub wage: i64,
    pub contract_expires: Option<String>,
    /// Emprestado: o clube dono e até quando.
    pub loan_from: Option<ClubRef>,
    pub loan_until: Option<String>,
    pub transfer_listed: bool,
    pub loan_listed: bool,
    #[ts(type = "number | null")]
    pub asking: Option<i64>,
    /// Janela do clube do treinador aberta hoje.
    pub window_open: bool,
    /// Saldo do clube do treinador e o que a direção autoriza em transferências.
    #[ts(type = "number")]
    pub cash: i64,
    #[ts(type = "number")]
    pub budget: i64,
    pub offer: Option<OfferView>,
    pub can_offer: bool,
    pub can_loan: bool,
    pub can_renew: bool,
    /// Emprestado ao treinador com opção de compra: o valor (0 = sem) e se dá para a exercer hoje.
    #[ts(type = "number")]
    pub option_fee: i64,
    pub can_option: bool,
    pub note: Option<String>,
    /// Sem conhecimento do jogador, a direção limita a proposta: a frase do CM.
    pub budget_note: Option<String>,
    /// As linhas da aba como no CM: disponibilidade, propostas, interessados e o futuro.
    pub availability: String,
    pub offers: Vec<String>,
    pub interested: Vec<ClubRef>,
    pub future: Option<String>,
}

/// Funções no clube ("Definir Função No Clube" do CM): chave, texto e peso (0 = a mais
/// importante). As duas últimas são de jovens.
pub const SQUAD_ROLES: [(&str, &str, i32); 6] = [
    ("key", "Este jogador é indispensável ao clube", 0),
    ("important", "Este jogador é importante para a equipa principal", 1),
    ("rotation", "Este jogador é usado num sistema de rotação do plantel", 2),
    ("backup", "Este jogador é um apoio à primeira equipa", 3),
    ("prospect", "Este jogador tem uma excelente perspectiva futura", 2),
    ("youngster", "Este jogador é um jovem com valor", 3),
];
/// Idades máximas das funções de jovem (acima, a direção veta).
const PROSPECT_AGE: i32 = 21;
const YOUNGSTER_AGE: i32 = 23;
/// Prémio de assinatura pedido, em semanas de salário (um livre pede muito mais).
const BONUS_WEEKS: f64 = 10.0;
const FREE_BONUS_WEEKS: f64 = 30.0;

/// Uma função da lista "Definir Função No Clube".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RoleOption {
    pub key: String,
    pub label: String,
    /// Função de jovem (o "+" roxo do CM).
    pub youth: bool,
    /// Vetada pela direção pela idade do jogador.
    pub allowed: bool,
}

/// A tela "Propôr Contrato": o que o jogador quer pela função escolhida.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ContractTerms {
    pub person_id: i32,
    pub name: String,
    pub club: Option<ClubRef>,
    /// Renovação (do plantel) ou contratação.
    pub renewal: bool,
    /// Pré-contrato: o contrato atual dele acaba em 6 meses ou menos; chega livre nessa data.
    pub precontract: bool,
    #[ts(type = "number")]
    pub current_wage: i64,
    pub current_expires: Option<String>,
    pub roles: Vec<RoleOption>,
    /// A função desta proposta (a escolhida ou a que ele espera).
    pub role: String,
    /// A função que ele espera ter no plantel do treinador.
    pub expected_role: String,
    /// O que pede: salário semanal, prémio de assinatura e anos (fim em `ends[years - 1]`).
    #[ts(type = "number")]
    pub wage: i64,
    #[ts(type = "number")]
    pub bonus: i64,
    pub years: i32,
    /// Fim do contrato para 1 a 5 anos.
    pub ends: Vec<String>,
    /// Valor da transferência acordada (0 numa renovação ou num livre) e saldo do clube.
    #[ts(type = "number")]
    pub fee: i64,
    #[ts(type = "number")]
    pub cash: i64,
    /// O que a direção autoriza: transferências (valor + prémio) e salário semanal.
    #[ts(type = "number")]
    pub budget: i64,
    #[ts(type = "number")]
    pub max_wage: i64,
}

/// Resposta do jogador a uma proposta de contrato.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ContractReply {
    pub accepted: bool,
    /// Negociações terminadas (rompidas ou fechadas).
    pub over: bool,
    /// A proposta seguiu: a resposta do jogador chega amanhã.
    pub pending: bool,
    /// O que pede (ou o acordado): salário semanal, prémio e anos.
    #[ts(type = "number")]
    pub wage: i64,
    #[ts(type = "number")]
    pub bonus: i64,
    pub years: i32,
    pub text: String,
}

/// O que um jogador pede por um contrato.
pub(super) struct Demand {
    pub(super) wage: i64,
    pub(super) bonus: i64,
    pub(super) years: i32,
}

pub(super) struct Offer {
    pub(super) id: i32,
    pub(super) kind: String,
    pub(super) person: i32,
    pub(super) buyer: i32,
    pub(super) seller: Option<i32>,
    pub(super) fee: i64,
    pub(super) asked: Option<i64>,
    pub(super) status: String,
    pub(super) answer_on: Option<String>,
    pub(super) months: i32,
    pub(super) sell_on: i32,
    pub(super) apps_bonus: i64,
    pub(super) rounds: i32,
    pub(super) loan_months: i32,
    pub(super) wage_share: i32,
    pub(super) monthly_fee: i64,
    pub(super) option_fee: i64,
    pub(super) apps_needed: i32,
    pub(super) public: bool,
    pub(super) exchange: Option<i32>,
}

/// Com quem é um contrato: renovação, transferência acordada com o clube, livre (com ou sem
/// a proposta a abrir o negócio) ou pré-contrato.
pub(super) enum Party {
    Renewal,
    Transfer(Offer),
    Free(Option<Offer>),
    Precontract,
}

/// O que o mercado precisa de saber de um jogador hoje.
pub(super) struct Facts {
    pub(super) club: Option<i32>,
    pub(super) ca: i32,
    pub(super) role: usize,
    pub(super) age: i32,
    pub(super) wage: i64,
    pub(super) value: i64,
    pub(super) expires: Option<String>,
    pub(super) joined: Option<String>,
    pub(super) ambition: f64,
    pub(super) loyalty: f64,
}

const OFFER_COLUMNS: &str = "id, kind, person_id, buyer, seller, fee, asked, status, answer_on, months, sell_on, apps_bonus, rounds, loan_months, wage_share, monthly_fee, option_fee, apps_needed, public, exchange";

fn offer_row(r: &rusqlite::Row) -> rusqlite::Result<Offer> {
    Ok(Offer {
        id: r.get(0)?,
        kind: r.get(1)?,
        person: r.get(2)?,
        buyer: r.get(3)?,
        seller: r.get(4)?,
        fee: r.get(5)?,
        asked: r.get(6)?,
        status: r.get(7)?,
        answer_on: r.get(8)?,
        months: r.get(9)?,
        sell_on: r.get(10)?,
        apps_bonus: r.get(11)?,
        rounds: r.get(12)?,
        loan_months: r.get(13)?,
        wage_share: r.get(14)?,
        monthly_fee: r.get(15)?,
        option_fee: r.get(16)?,
        apps_needed: r.get(17)?,
        public: r.get(18)?,
        exchange: r.get(19)?,
    })
}

/// Salário arredondado a €100 (abaixo de €10K por semana) ou a €1K.
pub(super) fn round_wage(v: f64) -> i64 {
    let step = if v < 10_000.0 { 100.0 } else { 1_000.0 };
    ((v / step).round() * step) as i64
}

fn action(key: &str, label: &str) -> OfferAction {
    OfferAction { key: key.into(), label: label.into() }
}

impl Game {
    pub(super) fn offer(&self, id: i32) -> Result<Offer> {
        self.world
            .conn
            .query_row(&format!("SELECT {OFFER_COLUMNS} FROM offers WHERE id = ?"), [id], offer_row)
            .optional()?
            .ok_or_else(|| Error::Invalid("Proposta não encontrada.".into()))
    }

    pub(super) fn offers_where(&self, filter: &str, args: impl rusqlite::Params) -> Result<Vec<Offer>> {
        Ok(self
            .world
            .conn
            .prepare_cached(&format!("SELECT {OFFER_COLUMNS} FROM offers WHERE {filter} ORDER BY id"))?
            .query_map(args, offer_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// A proposta em aberto por um jogador: do treinador por ele, ou de outro clube se for do treinador.
    pub(super) fn open_offer(&self, person: i32) -> Result<Option<Offer>> {
        Ok(self
            .offers_where(
                "person_id = ?1 AND (buyer = ?2 OR seller = ?2) AND kind <> 'contract' AND status IN ('pending', 'countered', 'agreed', 'raised')",
                params![person, self.club_id],
            )?
            .pop())
    }

    pub(super) fn set_status(&self, id: i32, status: &str, answer_on: Option<&str>) -> Result<()> {
        self.world.conn.execute("UPDATE offers SET status = ?, answer_on = ? WHERE id = ?", params![status, answer_on, id])?;
        Ok(())
    }

    pub(super) fn facts(&self, id: i32) -> Result<Option<Facts>> {
        type Row = (Option<i32>, i32, [i32; 4], Option<String>, Option<i32>, i64, Option<String>, Option<String>, [i32; 2]);
        let row: Option<Row> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.club_id, COALESCE(pl.current_ability, 1),
                        COALESCE(pl.pos_goalkeeper, 1),
                        MAX(COALESCE(pl.pos_sweeper, 1), COALESCE(pl.pos_defender, 1), COALESCE(pl.pos_wing_back, 1)),
                        MAX(COALESCE(pl.pos_defensive_midfielder, 1), COALESCE(pl.pos_midfielder, 1), COALESCE(pl.pos_attacking_midfielder, 1)),
                        COALESCE(pl.pos_attacker, 1),
                        p.birth_date, p.birth_year, COALESCE(p.wage, 0), p.contract_expires, p.club_joined,
                        COALESCE(p.ambition, 10), COALESCE(p.loyalty, 10)
                 FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.id = ? AND p.is_player = 1 AND p.id NOT IN (SELECT person_id FROM retired)",
            )?
            .query_row([id], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    [r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?],
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    [r.get(11)?, r.get(12)?],
                ))
            })
            .optional()?;
        let Some((club, ca, pos, birth, birth_year, wage, expires, joined, [ambition, loyalty])) = row else { return Ok(None) };
        Ok(Some(Facts {
            club,
            ca,
            role: role_of(pos[0], pos[1], pos[2], pos[3]),
            age: age_on(birth.as_deref(), birth_year, &self.date),
            wage,
            value: self.world.market_value_of(id, &self.date)?.unwrap_or(0),
            expires,
            joined,
            ambition: ambition as f64,
            loyalty: loyalty as f64,
        }))
    }

    /// Clube dono de um emprestado e até quando.
    pub(super) fn loan_of(&self, person: i32) -> Result<Option<(i32, String)>> {
        Ok(self
            .world
            .conn
            .query_row("SELECT parent_id, until FROM loans WHERE person_id = ?", [person], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?)
    }

    pub(super) fn club_rep(&self, club: i32) -> Result<i32> {
        Ok(self.world.conn.query_row("SELECT COALESCE(reputation, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?)
    }

    pub(super) fn cash_of(&self, club: i32) -> Result<i64> {
        Ok(self.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?)
    }

    pub(super) fn kind_of(&self, club: i32) -> Result<SeasonKind> {
        let division: Option<i32> = self.world.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [club], |r| r.get(0))?;
        Ok(self.club_kind(division))
    }

    pub(super) fn window_open_for(&self, club: i32) -> Result<bool> {
        Ok(window_open(self.kind_of(club)?, &self.date[5..]))
    }

    /// Posição de um jogador no plantel pelo CA (0 = o melhor) e quantos há no papel dele.
    fn squad_rank(&self, club: i32, person: i32, role: usize) -> Result<(usize, usize)> {
        let rows: Vec<(i32, [i32; 4])> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, COALESCE(pl.pos_goalkeeper, 1),
                        MAX(COALESCE(pl.pos_sweeper, 1), COALESCE(pl.pos_defender, 1), COALESCE(pl.pos_wing_back, 1)),
                        MAX(COALESCE(pl.pos_defensive_midfielder, 1), COALESCE(pl.pos_midfielder, 1), COALESCE(pl.pos_attacking_midfielder, 1)),
                        COALESCE(pl.pos_attacker, 1)
                 FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC, p.id",
            )?
            .query_map([club], |r| Ok((r.get(0)?, [r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?])))?
            .collect::<rusqlite::Result<_>>()?;
        let rank = rows.iter().position(|r| r.0 == person).unwrap_or(rows.len());
        let count = rows.iter().filter(|r| role_of(r.1[0], r.1[1], r.1[2], r.1[3]) == role).count();
        Ok((rank, count))
    }

    /// Sorteio fixo por jogador e por motivo (a mesma pergunta dá a mesma resposta).
    pub(super) fn deal_rng(&self, person: i32, salt: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64((self.seed as u64) ^ (person as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt)
    }

    fn recent(&self, f: &Facts) -> bool {
        let since = date::add_days(&self.date, -SETTLE_DAYS).unwrap_or_default();
        f.joined.as_deref().is_some_and(|j| j > since.as_str())
    }

    /// Quanto o clube pede por um jogador: o preço da lista de transferências, se lá estiver;
    /// senão o valor, +40% se for dos 11 melhores, +25% se chegou há pouco, até +20% se for
    /// leal, e um acerto de −5% a +15% fixo por jogador e época.
    fn asking_price(&self, person: i32, f: &Facts, club: i32) -> Result<i64> {
        // Na lista de transferências: o preço da lista
        let listed: Option<Option<i64>> = self
            .world
            .conn
            .query_row("SELECT asking FROM transfer_list WHERE person_id = ? AND transfer = 1", [person], |r| r.get(0))
            .optional()?;
        if let Some(asking) = listed {
            return Ok(asking.unwrap_or(f.value).max(10_000));
        }
        let (rank, _) = self.squad_rank(club, person, f.role)?;
        let mut ask = f.value as f64 * if rank < 11 { 1.4 } else { 1.0 } * personality::loyalty_fee(f.loyalty);
        if self.recent(f) {
            ask *= 1.25;
        }
        let year: u64 = self.date[0..4].parse().unwrap_or(0);
        ask *= 0.95 + 0.2 * uniform(&mut self.deal_rng(person, 0xA5C1 ^ year));
        Ok(finance::round_money(ask).max(10_000))
    }

    /// Função que o jogador espera no plantel do treinador, pelos que lá são melhores do que ele.
    pub(super) fn expected_role(&self, person: i32, f: &Facts) -> Result<usize> {
        let better: i64 = self.world.conn.query_row(
            "SELECT COUNT(*) FROM people p JOIN players pl ON pl.person_id = p.id
             WHERE p.club_id = ? AND p.is_player = 1 AND p.id <> ? AND pl.current_ability > ?",
            params![self.club_id, person, f.ca],
            |r| r.get(0),
        )?;
        Ok(match better {
            0..=2 => 0,
            3..=10 => 1,
            11..=15 => 2,
            _ if f.age <= PROSPECT_AGE => 4,
            _ if f.age <= YOUNGSTER_AGE => 5,
            _ => 3,
        })
    }

    /// O que o jogador pede ao clube do treinador pela função `role`: o salário pedido do 3.8
    /// (acerto de −5% a +10% fixo por jogador e época), mais 12% por cada degrau abaixo da
    /// função que espera (−4% por degrau acima); o prémio em semanas de salário; os anos pela idade.
    /// A personalidade: o ambicioso que seria dos indispensáveis pede mais, o leal pede menos
    /// para renovar e mais para deixar o clube onde está.
    pub(super) fn demand(&self, person: i32, f: &Facts, role: usize, renewal: bool, free: bool) -> Result<Demand> {
        let base = wage_demand(f.value.max(f.ca as i64 * 20_000), f.wage, self.club_rep(self.club_id)?);
        let year: u64 = self.date[0..4].parse().unwrap_or(0);
        let mut ask = base as f64 * (0.95 + 0.15 * uniform(&mut self.deal_rng(person, 0xDE3A ^ year)));
        let expected = self.expected_role(person, f)?;
        let diff = SQUAD_ROLES[role].2 - SQUAD_ROLES[expected].2;
        ask *= if diff > 0 { 1.0 + 0.12 * diff as f64 } else { 1.0 + 0.04 * diff as f64 };
        ask *= personality::ambition_wage(f.ambition, expected == 0);
        if renewal {
            ask *= personality::loyalty_renewal(f.loyalty);
        } else if !free {
            ask *= personality::loyalty_move_wage(f.loyalty);
        }
        let wage = round_wage(ask);
        let weeks = if free { FREE_BONUS_WEEKS } else { BONUS_WEEKS };
        let years = match f.age {
            ..=23 => 5,
            24..=29 => 4,
            30..=32 => 3,
            _ => 2,
        };
        Ok(Demand { wage, bonus: finance::round_money(wage as f64 * weeks), years })
    }

    /// Fim de um contrato de `years` épocas: numa renovação conta a partir do fim do atual.
    pub(super) fn contract_end(&self, f: &Facts, renewal: bool, years: i32) -> Result<String> {
        let kind = self.kind_of(self.club_id)?;
        let from = if renewal { f.expires.clone().filter(|e| e.as_str() > self.date.as_str()) } else { None };
        let start = from.and_then(|e| date::add_days(&e, 1)).unwrap_or_else(|| self.date.clone());
        Ok(contract_until(kind, &start, years))
    }

    /// A direção autoriza este valor em transferências?
    pub(super) fn check_budget(&self, amount: i64) -> Result<()> {
        let budget = self.transfer_budget()?;
        if amount > budget {
            return Err(Error::Invalid(format!("A sua direcção autoriza transferências até {}.", money_text(budget))));
        }
        Ok(())
    }

    /// Uma mensagem fora do avanço do dia (respostas imediatas do treinador).
    pub(super) fn post_news(&mut self, title: String, body: String, offer: Option<i32>) {
        let kind = offer.map_or("message", |o| self.offer_topic(o));
        self.post_news_as(kind, title, body);
        self.news[0].offer_id = offer;
    }

    /// Uma mensagem de um assunto ("press", "jobs"...) fora do avanço do dia.
    pub(super) fn post_news_as(&mut self, kind: &str, title: String, body: String) {
        let id = self.news.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let n = self.news_item(id, kind, title, body);
        self.news.insert(0, n);
    }

    /// O assunto da mensagem de uma proposta: negócios, contratos ou empregos.
    fn offer_topic(&self, offer: i32) -> &'static str {
        let kind: Option<String> =
            self.world.conn.query_row("SELECT kind FROM offers WHERE id = ?", [offer], |r| r.get(0)).optional().ok().flatten();
        match kind.as_deref() {
            Some("contract") => "contract",
            Some("job") => "jobs",
            _ => "deal",
        }
    }

    pub(super) fn offer_news(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32, title: String, body: String, offer: i32) {
        let mut n = self.news_item(*next_id, self.offer_topic(offer), title, body);
        n.offer_id = Some(offer);
        fresh.push(n);
        *next_id += 1;
    }

    /// Depois de um jogador entrar ou sair do clube do treinador.
    pub(super) fn squad_changed(&mut self) -> Result<()> {
        self.clean_selection()?;
        self.refresh_live()?;
        self.unsaved = true;
        Ok(())
    }

    /// Empresta um jogador até ao fim da época do clube que o recebe.
    fn execute_loan(&mut self, person: i32, parent: i32, to: i32, terms: &LoanTerms) -> Result<String> {
        let season_end = loan_until(self.kind_of(to)?, &self.date);
        self.world
            .conn
            .execute("INSERT OR REPLACE INTO loans (person_id, parent_id, until) VALUES (?, ?, ?)", params![person, parent, season_end])?;
        let until = self.loan_terms_start(person, parent, to, terms, &season_end)?;
        self.world.conn.execute("UPDATE people SET club_id = ?, club_joined = ? WHERE id = ?", params![to, self.date, person])?;
        self.world.conn.execute("DELETE FROM transfer_list WHERE person_id = ?", [person])?;
        self.number_after_move(person, Some(to))?;
        self.log_transfer(person, Some(parent), Some(to), 0, "loan")?;
        Ok(until)
    }

    // ---------------------------------------------------------------- vistas

    /// A aba Transferência do perfil de um jogador.
    pub fn transfer_view(&self, person: i32) -> Result<TransferView> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só os jogadores se transferem.".into()))?;
        let loan = self.loan_of(person)?;
        let loaned_in = loan.as_ref().is_some_and(|l| l.0 != self.club_id) && f.club == Some(self.club_id);
        let mine = f.club == Some(self.club_id) && !loaned_in;
        let listing: Option<(bool, bool, Option<i64>)> = self
            .world
            .conn
            .query_row("SELECT transfer, loan, asking FROM transfer_list WHERE person_id = ?", [person], |r| {
                Ok((r.get::<_, i32>(0)? != 0, r.get::<_, i32>(1)? != 0, r.get(2)?))
            })
            .optional()?;
        let offer = match self.open_contract(person)?.or(self.open_offer(person)?) {
            Some(o) => Some(self.offer_view(o.id)?),
            None => None,
        };
        let window = self.window_open_for(self.club_id)?;
        let blocked = self.blocked_until(person)?;
        let busy = offer.is_some();
        let bored = if mine { None } else { self.past_rounds(person, "transfer")?.1 };
        let option = self.option_of(person)?;
        let mut note = None;
        if let Some(b) = &blocked {
            note = Some(format!("Não quer falar de contrato até {}.", cm_date(b)));
        } else if let (Some(b), Some(club)) = (&bored, f.club) {
            note = Some(format!("O {} não quer voltar a negociar até {}.", self.club(club).short_name, cm_date(b)));
        } else if let Some((parent, fee)) = option {
            note = Some(format!("Emprestado com opção de compra: {} ao {}.", money_text(fee), self.club(parent).short_name));
        } else if !mine && f.club.is_some() && !window {
            let opens = super::market::next_window(self.kind_of(self.club_id)?, &self.date);
            note = Some(format!("A janela de transferências está fechada: um negócio fechado agora só se consuma a {}.", cm_date(&opens)));
        } else if loan.is_some() && !mine && f.club != Some(self.club_id) {
            note = Some("Está emprestado: o clube onde joga não o pode vender.".into());
        }
        let (availability, future) = self.availability(person, &f, mine, listing)?;
        let open = self.offers_where(
            "person_id = ?1 AND kind IN ('transfer', 'loan') AND status IN ('pending', 'countered', 'agreed', 'raised')",
            [person],
        )?;
        let offers = open
            .iter()
            .map(|o| {
                let what = if o.kind == "loan" { "empréstimo".to_string() } else { o.deal_terms().text() };
                format!("{} ({what})", self.club(o.buyer).short_name)
            })
            .collect();
        let mut interested: Vec<ClubRef> = Vec::new();
        if !mine && (self.in_shortlist(person)? || open.iter().any(|o| o.buyer == self.club_id)) {
            interested.push(self.club(self.club_id));
        }
        for o in &open {
            if !interested.iter().any(|c| c.id == o.buyer) {
                interested.push(self.club(o.buyer));
            }
        }
        Ok(TransferView {
            availability,
            offers,
            interested,
            future,
            person_id: person,
            mine,
            free: f.club.is_none(),
            club: f.club.map(|c| self.club(c)),
            value: f.value,
            wage: f.wage,
            contract_expires: f.expires.clone(),
            loan_from: loan.as_ref().map(|l| self.club(l.0)),
            loan_until: loan.map(|l| l.1),
            transfer_listed: listing.is_some_and(|l| l.0),
            loan_listed: listing.is_some_and(|l| l.1),
            asking: listing.and_then(|l| l.2),
            window_open: window,
            cash: self.cash_of(self.club_id)?,
            budget: if mine { self.transfer_budget()? } else { self.budget_for(person)?.0 },
            budget_note: if mine { None } else { self.budget_for(person)?.1 },
            can_offer: f.club != Some(self.club_id) && !busy && bored.is_none() && self.loan_of(person)?.is_none(),
            can_loan: f.club.is_some() && f.club != Some(self.club_id) && !busy && window && self.loan_of(person)?.is_none(),
            can_renew: mine && blocked.is_none() && !busy,
            option_fee: option.map_or(0, |o| o.1),
            can_option: option.is_some() && !busy && window,
            offer,
            note,
        })
    }

    /// "Disponibilidade" e "Futuro" da aba Transferência: a lista, o fim do contrato e o lugar
    /// do jogador no plantel (a função combinada, no clube do treinador).
    fn availability(
        &self,
        person: i32,
        f: &Facts,
        mine: bool,
        listing: Option<(bool, bool, Option<i64>)>,
    ) -> Result<(String, Option<String>)> {
        let Some(club) = f.club else { return Ok(("Livre: sem contrato".into(), None)) };
        let days_left = f.expires.as_deref().and_then(date::parse).zip(date::parse(&self.date)).map(|(e, t)| e - t);
        let (rank, _) = self.squad_rank(club, person, f.role)?;
        let availability = match listing {
            Some((true, _, Some(ask))) => format!("Na lista de transferências ({})", money_text(ask)),
            Some((true, _, None)) => "Na lista de transferências".into(),
            Some((false, true, _)) => "Disponível para empréstimo".into(),
            _ if days_left.is_some_and(|d| d <= 183) => "Em fim de contrato".into(),
            _ if rank < 3 => "Não está à venda".into(),
            _ => "Desconhecida".into(),
        };
        let role: Option<String> = if mine {
            self.world.conn.query_row("SELECT role FROM squad_roles WHERE person_id = ?", [person], |r| r.get(0)).optional()?
        } else {
            None
        };
        let future = match role.and_then(|k| SQUAD_ROLES.iter().find(|r| r.0 == k)) {
            Some(r) => r.1.to_string(),
            None => match rank {
                0..3 => "Pensa que é um membro essencial ao plantel".into(),
                3..11 => "Pensa que é importante para a equipa principal".into(),
                11..16 => "Pensa que faz parte da rotação do plantel".into(),
                _ if f.age <= PROSPECT_AGE => "Pensa que tem uma boa perspectiva futura no clube".into(),
                _ => "Pensa que é um apoio à primeira equipa".into(),
            },
        };
        Ok((availability, Some(future)))
    }

    /// Uma proposta com o texto e os botões da situação.
    pub fn offer_view(&self, id: i32) -> Result<OfferView> {
        let o = self.offer(id)?;
        let name = self.person_name(o.person)?;
        let incoming = o.buyer != self.club_id;
        let buyer = self.club(o.buyer);
        let seller = o.seller.map(|c| self.club(c));
        let seller_name = seller.as_ref().map_or(String::new(), |c| c.short_name.clone());
        let deal = o.deal_terms();
        let loan_terms = o.loan_terms();
        let terms = self.terms_text(&deal)?;
        let loan = o.kind == "loan";
        let until = o.answer_on.as_deref().map(cm_date).unwrap_or_default();
        let (text, actions) = match (incoming, o.status.as_str()) {
            _ if o.kind == "contract" => match o.status.as_str() {
                "pending" => {
                    (format!("Propôs um contrato a {name}; a resposta chega a {until}."), vec![action("withdraw", "Retirar Proposta")])
                }
                "agreed" => (
                    format!("{name} aceitou o contrato. Confirme até {until} para fechar o negócio."),
                    vec![action("confirm", "Confirmar Contratação"), action("withdraw", "Desistir")],
                ),
                "countered" => (
                    format!("{name} fez uma contraproposta. Responda até {until}."),
                    vec![action("accept", "Aceitar Contraproposta"), action("negotiate", "Negociar"), action("withdraw", "Desistir")],
                ),
                "signed" => (format!("Pré-contrato assinado: {name} chega a {until}."), Vec::new()),
                "waiting" => (format!("{name} assinou: chega a {until}, quando abrir a janela de transferências."), Vec::new()),
                "done" => ("Contrato assinado.".into(), Vec::new()),
                "withdrawn" => ("Proposta retirada.".into(), Vec::new()),
                _ => ("Negociações terminadas.".into(), Vec::new()),
            },
            _ if o.kind == "job" => match o.status.as_str() {
                "applied" => (format!("Concorreu ao lugar no {}; a resposta chega a {until}.", buyer.name), Vec::new()),
                "pending" => (
                    format!("O {} quer contratá-lo para treinador. Responda até {until}.", buyer.name),
                    vec![action("accept", "Aceitar"), action("reject", "Recusar")],
                ),
                "done" => (format!("É o treinador do {}.", buyer.name), Vec::new()),
                _ => ("Proposta encerrada.".into(), Vec::new()),
            },
            _ if o.kind == "request" => match o.status.as_str() {
                "pending" => (
                    format!("{name} pede para ser posto na lista de transferências. Responda até {until}."),
                    vec![action("accept", "Pôr na Lista"), action("reject", "Recusar")],
                ),
                "done" => (format!("{name} está na lista de transferências, a pedido dele."), Vec::new()),
                _ => (format!("Recusou o pedido de {name} para sair."), Vec::new()),
            },
            (false, "pending") if loan => (
                format!("Pediu {name} emprestado ao {seller_name} ({}); a resposta chega a {until}.", loan_terms.text()),
                vec![action("withdraw", "Retirar")],
            ),
            (false, "pending") => (
                format!("Ofereceu {terms} ao {seller_name} por {name}; a resposta chega a {until}."),
                vec![action("withdraw", "Retirar Proposta")],
            ),
            (false, "countered") if loan => (
                format!("O {seller_name} empresta {name} {}. Responda até {until}.", loan_terms.text()),
                vec![action("accept", "Aceitar Condições"), action("offer", "Novo Pedido"), action("withdraw", "Desistir")],
            ),
            (false, "countered") => (
                format!("O {seller_name} pede {terms} por {name}. Responda até {until}."),
                vec![action("accept", "Aceitar Condições"), action("offer", "Nova Proposta"), action("withdraw", "Desistir")],
            ),
            (false, "agreed") if o.seller.is_none() => (
                format!("{name} está livre: negoceie o contrato até {until}."),
                vec![action("negotiate", "Negociar Contrato"), action("withdraw", "Desistir")],
            ),
            (false, "agreed") => (
                format!("O {seller_name} aceitou {terms} por {name}. Falta o contrato com o jogador (até {until})."),
                vec![action("negotiate", "Negociar Contrato"), action("withdraw", "Desistir")],
            ),
            (false, "bored") => (format!("O {seller_name} não quer voltar a negociar {name} até {until}."), Vec::new()),
            (true, "pending") if loan => (
                format!("O {} quer {name} emprestado {}. Responda até {until}.", buyer.short_name, loan_terms.text()),
                vec![action("accept", "Aceitar"), action("reject", "Recusar")],
            ),
            (true, "pending") => (
                format!("O {} oferece {terms} por {name}. Responda até {until}.", buyer.short_name),
                vec![action("accept", "Aceitar"), action("counter", "Pedir Mais"), action("reject", "Recusar")],
            ),
            (true, "raised") => (format!("Pediu {terms} ao {} por {name}; a resposta chega a {until}.", buyer.short_name), Vec::new()),
            (true, "countered") => (
                format!("O {} responde com {terms} por {name}. Responda até {until}.", buyer.short_name),
                vec![action("accept", "Aceitar"), action("counter", "Pedir Mais"), action("reject", "Recusar")],
            ),
            (_, "done") => ("Negócio concluído.".into(), Vec::new()),
            (_, "rejected") => ("Proposta recusada.".into(), Vec::new()),
            (_, "withdrawn") => ("Proposta retirada.".into(), Vec::new()),
            _ => ("Negócio desfeito.".into(), Vec::new()),
        };
        Ok(OfferView {
            id,
            kind: o.kind,
            incoming,
            person_id: o.person,
            name,
            buyer,
            seller,
            fee: o.fee,
            status: o.status,
            text,
            actions,
            deal,
            loan: loan_terms,
            rounds: o.rounds,
        })
    }

    // ---------------------------------------------------------------- o treinador compra

    /// As condições em palavras, com o jogador da troca.
    pub(super) fn terms_text(&self, t: &DealTerms) -> Result<String> {
        Ok(match t.exchange {
            Some(x) => format!("{} mais {}", t.text(), self.person_name(x)?),
            None => t.text(),
        })
    }

    /// O clube aceita este jogador do treinador numa troca? Só se entrar nos 18 melhores dele.
    fn wants_exchange(&self, club: i32, player: &Facts) -> Result<bool> {
        let better: i64 = self.world.conn.query_row(
            "SELECT COUNT(*) FROM people p JOIN players pl ON pl.person_id = p.id
             WHERE p.club_id = ? AND p.is_player = 1 AND pl.current_ability > ?",
            params![club, player.ca],
            |r| r.get(0),
        )?;
        Ok(better < 18)
    }

    /// O que vale para o clube o jogador da troca (0 se já não é do treinador ou não o quer).
    fn exchange_worth(&self, club: i32, t: &DealTerms) -> Result<f64> {
        let Some(x) = t.exchange else { return Ok(0.0) };
        let Some(xf) = self.facts(x)?.filter(|xf| xf.club == Some(self.club_id)) else { return Ok(0.0) };
        Ok(if self.wants_exchange(club, &xf)? { xf.value as f64 * EXCHANGE_WORTH } else { 0.0 })
    }

    /// A proposta do treinador por um jogador que espera a decisão dele (contraproposta do
    /// clube): a nova proposta substitui-a e conta como mais uma ronda.
    fn countered(&self, person: i32, kind: &str) -> Result<Option<Offer>> {
        Ok(self.open_offer(person)?.filter(|o| o.buyer == self.club_id && o.kind == kind && o.status == "countered"))
    }

    /// Proposta de transferência por um jogador de outro clube (ou por um livre, que passa
    /// logo à negociação do contrato), com as condições do diálogo do CM. O clube responde
    /// em 1 a 4 dias.
    pub fn make_offer(&mut self, person: i32, terms: &DealTerms) -> Result<TransferView> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só se fazem propostas por jogadores.".into()))?;
        if f.club == Some(self.club_id) {
            return Err(Error::Invalid("Já é do seu plantel.".into()));
        }
        if let Some((parent, _)) = self.loan_of(person)? {
            return Err(Error::Invalid(format!("Está emprestado pelo {}: não pode ser vendido agora.", self.club(parent).short_name)));
        }
        let countered = self.countered(person, "transfer")?;
        if countered.is_none() && self.open_offer(person)?.is_some() {
            return Err(Error::Invalid("Já há uma proposta em curso por este jogador.".into()));
        }
        let created = self.date.clone();
        match f.club {
            None => {
                let until = date::add_days(&self.date, AGREED_DAYS).unwrap_or_default();
                self.world.conn.execute(
                    "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('transfer', ?, ?, NULL, 0, 'agreed', ?, ?)",
                    params![person, self.club_id, until, created],
                )?;
            }
            Some(seller) => {
                // Com a janela fechada a proposta segue: o negócio só se consuma quando abrir
                terms.check()?;
                let (limit, why) = self.budget_for(person)?;
                if terms.budget_cost() > limit {
                    return Err(Error::Invalid(
                        why.unwrap_or_else(|| format!("A sua direcção autoriza transferências até {}.", money_text(limit))),
                    ));
                }
                let (rounds, bored) = self.past_rounds(person, "transfer")?;
                if let Some(b) = bored {
                    return Err(Error::Invalid(format!(
                        "O {} não quer voltar a negociar até {}.",
                        self.club(seller).short_name,
                        cm_date(&b)
                    )));
                }
                if let Some(x) = terms.exchange {
                    let xf = self
                        .facts(x)?
                        .filter(|xf| xf.club == Some(self.club_id) && x != person)
                        .ok_or_else(|| Error::Invalid("Na troca só entram jogadores do seu plantel.".into()))?;
                    if self.loan_of(x)?.is_some() {
                        return Err(Error::Invalid("Um jogador emprestado não pode entrar na troca.".into()));
                    }
                    if !self.wants_exchange(seller, &xf)? {
                        return Err(Error::Invalid(format!(
                            "O {} não tem interesse num negócio que envolva troca de jogadores.",
                            self.club(seller).short_name
                        )));
                    }
                }
                let terms = DealTerms { fee: finance::round_money(terms.fee as f64), ..*terms };
                if terms.public {
                    let name = self.person_name(person)?;
                    let body = format!(
                        "O {} tornou pública a proposta de {} ao {} por {name}.",
                        self.club(self.club_id).short_name,
                        self.terms_text(&terms)?,
                        self.club(seller).short_name
                    );
                    self.post_news_as("deal", format!("Proposta pública por {name}"), body);
                }
                let reply = self.reply_on(person, REPLY_DAYS);
                let id = match countered {
                    Some(c) => {
                        self.world.conn.execute(
                            "UPDATE offers SET status = 'pending', rounds = rounds + 1, answer_on = ? WHERE id = ?",
                            params![reply, c.id],
                        )?;
                        c.id
                    }
                    None => {
                        self.world.conn.execute(
                            "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created, rounds)
                             VALUES ('transfer', ?, ?, ?, ?, 'pending', ?, ?, ?)",
                            params![person, self.club_id, seller, terms.fee, reply, created, rounds],
                        )?;
                        self.world.conn.last_insert_rowid() as i32
                    }
                };
                self.set_terms(id, &terms, &LoanTerms::default())?;
            }
        }
        self.unsaved = true;
        self.transfer_view(person)
    }

    /// Pedido de empréstimo de um jogador de outro clube, com as condições (duração, parte do
    /// salário, taxa mensal, opção de compra).
    pub fn ask_loan(&mut self, person: i32, terms: &LoanTerms) -> Result<TransferView> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só se pedem jogadores emprestados.".into()))?;
        let Some(seller) = f.club.filter(|&c| c != self.club_id) else {
            return Err(Error::Invalid("Só se pedem emprestados jogadores de outros clubes.".into()));
        };
        if !self.window_open_for(self.club_id)? {
            return Err(Error::Invalid("A janela de transferências está fechada.".into()));
        }
        terms.check()?;
        let countered = self.countered(person, "loan")?;
        if self.loan_of(person)?.is_some() || (countered.is_none() && self.open_offer(person)?.is_some()) {
            return Err(Error::Invalid("Este jogador não pode ser pedido agora.".into()));
        }
        let reply = self.reply_on(person, REPLY_DAYS);
        let id = match countered {
            Some(c) => {
                self.world.conn.execute(
                    "UPDATE offers SET status = 'pending', rounds = rounds + 1, answer_on = ? WHERE id = ?",
                    params![reply, c.id],
                )?;
                c.id
            }
            None => {
                let rounds = self.past_rounds(person, "loan")?.0;
                self.world.conn.execute(
                    "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created, rounds)
                     VALUES ('loan', ?, ?, ?, 0, 'pending', ?, ?, ?)",
                    params![person, self.club_id, seller, reply, self.date, rounds],
                )?;
                self.world.conn.last_insert_rowid() as i32
            }
        };
        self.set_terms(id, &DealTerms::default(), terms)?;
        self.unsaved = true;
        self.transfer_view(person)
    }

    /// Resposta do treinador numa proposta: "accept", "reject", "counter" (com `fee`), "withdraw".
    pub fn answer_offer(&mut self, id: i32, key: &str, fee: Option<i64>) -> Result<OfferView> {
        let o = self.offer(id)?;
        if o.kind == "request" {
            return self.answer_request(&o, key);
        }
        if o.kind == "contract" {
            self.answer_contract(&o, key)?;
            return self.offer_view(id);
        }
        if o.kind == "job" {
            self.answer_job(&o, key)?;
            return self.offer_view(id);
        }
        let incoming = o.buyer != self.club_id;
        match (incoming, o.status.as_str(), key) {
            (false, "pending" | "countered" | "agreed", "withdraw") => self.set_status(id, "withdrawn", None)?,
            (false, "countered", "accept" | "pay") if o.kind == "loan" => {
                let seller = o.seller.ok_or_else(|| Error::Invalid("Proposta sem clube.".into()))?;
                if !self.window_open_for(self.club_id)? {
                    return Err(Error::Invalid("A janela de transferências está fechada.".into()));
                }
                if self.facts(o.person)?.is_none_or(|f| f.club != Some(seller)) {
                    self.set_status(id, "collapsed", None)?;
                    return Err(Error::Invalid("O jogador já não é deste clube.".into()));
                }
                let until = self.execute_loan(o.person, seller, self.club_id, &o.loan_terms())?;
                self.set_status(id, "done", None)?;
                let name = self.person_name(o.person)?;
                self.post_news(
                    format!("{name} emprestado ao clube"),
                    format!("{name} chega emprestado pelo {} até {}.", self.club(seller).short_name, cm_date(&until)),
                    Some(id),
                );
                self.squad_changed()?;
            }
            (false, "countered", "accept" | "pay") => {
                self.check_budget(o.deal_terms().budget_cost())?;
                let until = date::add_days(&self.date, AGREED_DAYS).unwrap_or_default();
                self.set_status(id, "agreed", Some(&until))?;
            }
            (true, "pending" | "countered", "accept") => {
                if !self.window_open_for(o.buyer)? {
                    self.set_status(id, "collapsed", None)?;
                    return Err(Error::Invalid("A janela do clube que propôs já fechou.".into()));
                }
                let (title, body) = self.complete_sale(&o, o.fee)?;
                self.post_news(title, body, Some(id));
            }
            (true, "pending" | "countered", "reject") => {
                self.set_status(id, "rejected", None)?;
                if let Some(text) = self.letdown(&o)? {
                    let name = self.person_name(o.person)?;
                    self.post_news(format!("{name} desiludido"), text, Some(id));
                }
            }
            (true, "pending" | "countered", "counter") if o.kind == "transfer" => {
                let asked = fee.filter(|&f| f > o.fee).ok_or_else(|| Error::Invalid("Peça mais do que a proposta.".into()))?;
                return self.ask_terms(id, &DealTerms { fee: asked, ..o.deal_terms() });
            }
            _ => return Err(Error::Invalid("Esta proposta já não aceita essa resposta.".into())),
        }
        self.unsaved = true;
        self.offer_view(id)
    }

    /// Jogador de um clube europeu que não aceita vir para o clube do treinador, fora da Europa.
    pub(super) fn stays_in_europe(&self, person: i32, f: &Facts, club: i32) -> Result<bool> {
        let (from, _) = self.club_place(club)?;
        let (to, nation) = self.club_place(self.club_id)?;
        if !leaving_europe(from, to) {
            return Ok(false);
        }
        let own: Option<i32> = self.world.conn.query_row("SELECT nation_id FROM people WHERE id = ?", [person], |r| r.get(0))?;
        let (rank, _) = self.squad_rank(club, person, f.role)?;
        Ok(!leaves_europe(f.age, rank < 11, own == Some(nation)))
    }

    /// Resposta a um pedido para sair: pô-lo na lista (a pedido) ou recusar.
    fn answer_request(&mut self, o: &Offer, key: &str) -> Result<OfferView> {
        match (o.status.as_str(), key) {
            ("pending", "accept") => {
                self.world.conn.execute(
                    "INSERT INTO transfer_list (person_id, transfer, loan, asking, requested) VALUES (?, 1, 0, NULL, 1)
                     ON CONFLICT (person_id) DO UPDATE SET transfer = 1, requested = 1",
                    [o.person],
                )?;
                self.remove_unhappy(o.person, "blocked")?;
                self.set_status(o.id, "done", None)?;
            }
            ("pending", "reject") => {
                self.set_status(o.id, "rejected", None)?;
                self.refuse_request(o.person)?;
            }
            _ => return Err(Error::Invalid("Este pedido já não aceita essa resposta.".into())),
        }
        self.unsaved = true;
        self.offer_view(o.id)
    }

    /// Um ambicioso a quem o clube recusa (ou deixa sem resposta) uma proposta de um clube
    /// maior fica desiludido: perde moral. Devolve a frase para a mensagem.
    fn letdown(&mut self, o: &Offer) -> Result<Option<String>> {
        if o.kind != "transfer" || self.club_rep(o.buyer)? <= self.club_rep(self.club_id)? {
            return Ok(None);
        }
        let Some(f) = self.facts(o.person)?.filter(|f| f.club == Some(self.club_id)) else { return Ok(None) };
        if (f.ambition as i32) < personality::AMBITIOUS {
            return Ok(None);
        }
        let s = self.squads.states.entry(o.person).or_default();
        s.morale = (s.morale - personality::LETDOWN_MORALE).max(1);
        self.add_unhappy(o.person, "bigger")?;
        let name = self.person_name(o.person)?;
        Ok(Some(format!("{name} queria jogar no {} e ficou desiludido.", self.club(o.buyer).short_name)))
    }

    /// Venda (ou empréstimo) de um jogador do treinador ao clube que propôs; devolve o
    /// título e o texto da mensagem.
    fn complete_sale(&mut self, o: &Offer, fee: i64) -> Result<(String, String)> {
        let f = self.facts(o.person)?.ok_or_else(|| Error::Invalid("Jogador não encontrado.".into()))?;
        let name = self.person_name(o.person)?;
        let buyer = self.club(o.buyer).short_name;
        let news = if o.kind == "loan" {
            let t = o.loan_terms();
            let until = self.execute_loan(o.person, self.club_id, o.buyer, &t)?;
            (format!("{name} emprestado"), format!("{name} vai jogar emprestado no {buyer} até {} ({}).", cm_date(&until), t.text()))
        } else {
            let wage = wage_demand(f.value.max(f.ca as i64 * 20_000), f.wage, self.club_rep(o.buyer)?);
            let years = contract_years(f.age, &mut self.deal_rng(o.person, 0x5A1E));
            let kind = self.kind_of(o.buyer)?;
            // Um dos valiosos: a direção e os adeptos não gostam (antes de ele sair)
            let idol = self.idol_sold(o.person)?;
            let terms = DealTerms { fee, ..o.deal_terms() };
            self.execute_move(o.person, Some(self.club_id), Some(o.buyer), fee, wage, years, "transfer", kind)?;
            self.settle_terms(o.person, o.buyer, self.club_id, &terms)?;
            self.world.conn.execute("UPDATE offers SET fee = ? WHERE id = ?", params![fee, o.id])?;
            self.sale_to_budget(terms.budget_cost() - terms.apps_bonus)?;
            let mut body = format!("{name} foi vendido ao {buyer} por {}.", terms.text());
            if let Some(idol) = idol {
                body = format!("{body}\n\n{idol}");
            }
            (format!("{name} vendido"), body)
        };
        self.set_status(o.id, "done", None)?;
        self.squad_changed()?;
        Ok(news)
    }

    // ---------------------------------------------------------------- contratos

    pub(super) fn blocked_until(&self, person: i32) -> Result<Option<String>> {
        let b: Option<Option<String>> =
            self.world.conn.query_row("SELECT blocked_until FROM talks WHERE person_id = ?", [person], |r| r.get(0)).optional()?;
        Ok(b.flatten().filter(|d| d.as_str() > self.date.as_str()))
    }

    /// Com quem o treinador pode negociar um contrato: um do plantel (renovação) ou um jogador
    /// cujo clube aceitou a proposta (ou um livre). Devolve os factos, o nome e a proposta acordada.
    pub(super) fn contract_party(&self, person: i32) -> Result<(Facts, String, Party)> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só os jogadores assinam contratos.".into()))?;
        let name = self.person_name(person)?;
        let loan = self.loan_of(person)?;
        let loaned_in = loan.as_ref().is_some_and(|l| l.0 != self.club_id);
        let mine = f.club == Some(self.club_id) && !loaned_in;
        let deal = self.open_offer(person)?.filter(|o| o.buyer == self.club_id && o.status == "agreed");
        let days_left = f.expires.as_deref().and_then(date::parse).zip(date::parse(&self.date)).map_or(i64::MAX, |(e, t)| (e - t) as i64);
        let party = if mine {
            Party::Renewal
        } else if let Some(d) = deal {
            if d.seller.is_some() { Party::Transfer(d) } else { Party::Free(Some(d)) }
        } else if f.club.is_none() {
            Party::Free(None)
        } else if loan.is_none() && days_left <= super::contracts::PRECONTRACT_DAYS {
            Party::Precontract
        } else {
            return Err(Error::Invalid("Primeiro o clube dele tem de aceitar uma proposta.".into()));
        };
        if let Some(b) = self.blocked_until(person)? {
            return Err(Error::Invalid(format!("{name} não quer falar de contrato até {}.", cm_date(&b))));
        }
        Ok((f, name, party))
    }

    pub(super) fn role_index(key: &str) -> Result<usize> {
        SQUAD_ROLES.iter().position(|r| r.0 == key).ok_or_else(|| Error::Invalid("Função desconhecida.".into()))
    }

    pub(super) fn role_allowed(role: usize, age: i32) -> bool {
        match role {
            4 => age <= PROSPECT_AGE,
            5 => age <= YOUNGSTER_AGE,
            _ => true,
        }
    }

    /// As telas "Definir Função No Clube" e "Propôr Contrato": as funções e o que o jogador
    /// pede pela função `role` (sem ela, pela que ele espera).
    pub fn contract_terms(&self, person: i32, role: Option<&str>) -> Result<ContractTerms> {
        let (f, name, party) = self.contract_party(person)?;
        let expected = self.expected_role(person, &f)?;
        let chosen = match role {
            Some(k) => Self::role_index(k)?,
            None => expected,
        };
        let renewal = matches!(party, Party::Renewal);
        let precontract = matches!(party, Party::Precontract);
        let free = matches!(party, Party::Free(_) | Party::Precontract);
        let d = self.demand(person, &f, chosen, renewal, free)?;
        let ends = (1..=5).map(|y| self.contract_end(&f, renewal || precontract, y)).collect::<Result<Vec<_>>>()?;
        let fee = if let Party::Transfer(o) = &party { o.fee } else { 0 };
        Ok(ContractTerms {
            person_id: person,
            name,
            club: f.club.map(|c| self.club(c)),
            renewal,
            precontract,
            current_wage: f.wage,
            current_expires: f.expires.clone(),
            roles: SQUAD_ROLES
                .iter()
                .enumerate()
                .map(|(i, r)| RoleOption { key: r.0.into(), label: r.1.into(), youth: i >= 4, allowed: Self::role_allowed(i, f.age) })
                .collect(),
            role: SQUAD_ROLES[chosen].0.into(),
            expected_role: SQUAD_ROLES[expected].0.into(),
            wage: d.wage,
            bonus: d.bonus,
            years: d.years,
            ends,
            fee,
            cash: self.cash_of(self.club_id)?,
            budget: self.transfer_budget()?,
            max_wage: self.board_view()?.max_wage,
        })
    }

    // ---------------------------------------------------------------- lista de transferências

    /// Põe ou tira um jogador do plantel das listas de transferências e de empréstimos.
    pub fn set_listing(&mut self, person: i32, transfer: bool, loan: bool, asking: Option<i64>) -> Result<TransferView> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só os jogadores vão para a lista.".into()))?;
        if f.club != Some(self.club_id) || self.loan_of(person)?.is_some() {
            return Err(Error::Invalid("Só os jogadores do seu plantel.".into()));
        }
        let (was_listed, requested): (bool, bool) = self
            .world
            .conn
            .query_row("SELECT transfer, requested FROM transfer_list WHERE person_id = ?", [person], |r| {
                Ok((r.get::<_, i32>(0)? != 0, r.get::<_, i32>(1)? != 0))
            })
            .optional()?
            .unwrap_or((false, false));
        if transfer || loan {
            self.world.conn.execute(
                "INSERT INTO transfer_list (person_id, transfer, loan, asking, requested) VALUES (?1, ?2, ?3, ?4, 0)
                 ON CONFLICT (person_id) DO UPDATE SET transfer = ?2, loan = ?3, asking = ?4, requested = requested AND ?2",
                params![person, transfer as i32, loan as i32, asking.filter(|&a| a > 0)],
            )?;
        } else {
            self.world.conn.execute("DELETE FROM transfer_list WHERE person_id = ?", [person])?;
        }
        // Posto na lista sem pedir perde moral; tirado da lista depois de pedir fica frustrado
        if transfer && !was_listed {
            self.change_morale(person, -super::unhappy::LISTED_MORALE);
        }
        if requested && !transfer {
            self.refuse_request(person)?;
        }
        self.unsaved = true;
        self.transfer_view(person)
    }

    // ---------------------------------------------------------------- o dia

    /// Respostas dos outros clubes, prazos e propostas novas pelos jogadores do treinador.
    pub(super) fn offers_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let today = self.date.clone();
        for o in self.offers_where(
            "buyer = ?1 AND kind NOT IN ('request', 'contract') AND status = 'pending' AND answer_on <= ?2",
            params![self.club_id, today],
        )? {
            self.seller_answers(&o, fresh, next_id)?;
        }
        for o in self.offers_where("seller = ?1 AND status = 'raised' AND answer_on <= ?2", params![self.club_id, today])? {
            self.buyer_answers(&o, fresh, next_id)?;
        }
        // Prazos: propostas recebidas sem resposta, acordos sem contrato
        for o in self.offers_where(
            "seller = ?1 AND kind NOT IN ('request', 'contract') AND status IN ('pending', 'countered') AND answer_on < ?2",
            params![self.club_id, today],
        )? {
            self.set_status(o.id, "withdrawn", None)?;
            let name = self.person_name(o.person)?;
            let mut body = format!("O {} desistiu de {name}: a proposta ficou sem resposta.", self.club(o.buyer).short_name);
            if let Some(text) = self.letdown(&o)? {
                body = format!("{body} {text}");
            }
            self.offer_news(fresh, next_id, format!("Proposta por {name} expirou"), body, o.id);
        }
        for o in
            self.offers_where("kind = 'request' AND seller = ?1 AND status = 'pending' AND answer_on < ?2", params![self.club_id, today])?
        {
            self.set_status(o.id, "rejected", None)?;
            self.refuse_request(o.person)?;
            let name = self.person_name(o.person)?;
            let body = format!("O pedido de {name} para sair ficou sem resposta: ele sente que não o deixam sair.");
            self.offer_news(fresh, next_id, format!("{name} frustrado"), body, o.id);
        }
        self.contract_replies(fresh, next_id)?;
        self.precontracts_due(fresh, next_id)?;
        self.waiting_deals_due(fresh, next_id)?;
        self.option_reminders(fresh, next_id)?;
        for o in self.offers_where("buyer = ?1 AND status IN ('countered', 'agreed') AND answer_on < ?2", params![self.club_id, today])? {
            self.set_status(o.id, "collapsed", None)?;
            let name = self.person_name(o.person)?;
            self.offer_news(
                fresh,
                next_id,
                format!("Negócio por {name} caiu"),
                format!("O prazo para fechar o negócio por {name} acabou."),
                o.id,
            );
        }
        let any_window = [SeasonKind::European, SeasonKind::Calendar].iter().any(|&k| window_open(k, &today[5..]));
        if any_window && matches!(date::weekday(&today), Some(1 | 4)) {
            self.ai_bids(fresh, next_id)?;
        }
        // Aviso dos contratos a acabar, dois meses antes de cada fim de contratos
        match &today[5..] {
            "05-01" => self.contract_warning(&format!("{}-06-30", &today[0..4]), fresh, next_id)?,
            "11-01" => self.contract_warning(&format!("{}-12-31", &today[0..4]), fresh, next_id)?,
            _ => {}
        }
        Ok(())
    }

    /// O clube que vende responde a uma proposta do treinador: aceita, faz uma contraproposta
    /// com as condições dele, recusa, ou farta-se depois de várias rondas.
    fn seller_answers(&mut self, o: &Offer, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let name = self.person_name(o.person)?;
        let seller = o.seller.unwrap_or(0);
        let seller_name = self.club(seller).short_name;
        let f = self.facts(o.person)?;
        let Some(f) = f.filter(|f| f.club == Some(seller)) else {
            self.set_status(o.id, "collapsed", None)?;
            let body = format!("{name} já não é jogador do {seller_name}.");
            self.offer_news(fresh, next_id, format!("Proposta por {name}"), body, o.id);
            return Ok(());
        };
        let (rank, count) = self.squad_rank(seller, o.person, f.role)?;
        let on_loan = self.loan_of(o.person)?.is_some();
        let deadline = date::add_days(&self.date, AGREED_DAYS).unwrap_or_default();
        let asking = self.asking_price(o.person, &f, seller)?;
        if o.kind == "loan" {
            let t = o.loan_terms();
            let reply = if on_loan || self.recent(&f) || !self.window_open_for(self.club_id)? {
                LoanReply::Reject
            } else {
                loan_reply(&t, rank, f.age, f.value, asking, o.rounds)
            };
            match reply {
                LoanReply::Accept => {
                    let until = self.execute_loan(o.person, seller, self.club_id, &t)?;
                    self.set_status(o.id, "done", None)?;
                    let body =
                        format!("O {seller_name} aceitou emprestar {name} até {} ({}). Já está no seu plantel.", cm_date(&until), t.text());
                    self.offer_news(fresh, next_id, format!("{name} emprestado ao clube"), body, o.id);
                    self.squad_changed()?;
                }
                LoanReply::Counter(c) => {
                    self.set_terms(o.id, &o.deal_terms(), &c)?;
                    let until = date::add_days(&self.date, super::negotiation::COUNTER_DAYS).unwrap_or_default();
                    self.set_status(o.id, "countered", Some(&until))?;
                    let body =
                        format!("O {seller_name} está disposto a emprestar {name}, mas só {}. Responda até {}.", c.text(), cm_date(&until));
                    self.offer_news(fresh, next_id, format!("Condições para o empréstimo de {name}"), body, o.id);
                }
                LoanReply::Reject => {
                    self.set_status(o.id, "rejected", None)?;
                    let body = if rank < 14 {
                        format!("O {seller_name} não quer emprestar {name}: é importante para a equipa.")
                    } else {
                        format!("O {seller_name} recusou o pedido de empréstimo de {name}.")
                    };
                    self.offer_news(fresh, next_id, format!("Empréstimo de {name} recusado"), body, o.id);
                }
            }
            return Ok(());
        }
        if on_loan || count <= MIN_ROLE[f.role] {
            self.set_status(o.id, "rejected", None)?;
            let body = format!("O {seller_name} recusou a proposta por {name}: não está à venda.");
            self.offer_news(fresh, next_id, format!("Proposta por {name} recusada"), body, o.id);
            return Ok(());
        }
        let t = o.deal_terms();
        let mut rng = self.deal_rng(o.person, 0xC0DE ^ o.rounds as u64);
        // Uma proposta pública de um clube maior desestabiliza um ambicioso: o clube pede menos
        let unsettled = t.public && f.ambition as i32 >= personality::AMBITIOUS && self.club_rep(self.club_id)? > self.club_rep(seller)?;
        let asking = if unsettled { finance::round_money(asking as f64 * (1.0 - PUBLIC_DISCOUNT)) } else { asking };
        let extra = self.exchange_worth(seller, &t)?;
        match seller_reply(&t, asking, f.value, f.age, o.rounds, extra, &mut rng) {
            SellerReply::Accept => {
                self.set_status(o.id, "agreed", Some(&deadline))?;
                let body = format!(
                    "O {seller_name} aceitou a sua proposta por {name}: {}. Negoceie agora o contrato com o jogador até {}.",
                    t.text(),
                    cm_date(&deadline)
                );
                self.offer_news(fresh, next_id, format!("Proposta por {name} aceite"), body, o.id);
            }
            SellerReply::Counter(c) => {
                self.set_terms(o.id, &c, &o.loan_terms())?;
                let until = date::add_days(&self.date, super::negotiation::COUNTER_DAYS).unwrap_or_default();
                self.set_status(o.id, "countered", Some(&until))?;
                let mut body = format!("O {seller_name} recusou a sua proposta por {name}, mas responde que aceitaria {}.", c.text());
                if c.sell_on > t.sell_on {
                    body.push_str(" Quer ficar com uma parte de uma futura venda.");
                }
                body.push_str(&format!(" Pode aceitar as condições ou fazer uma nova proposta até {}.", cm_date(&until)));
                self.offer_news(fresh, next_id, format!("Contraproposta por {name}"), body, o.id);
            }
            SellerReply::Reject => {
                self.set_status(o.id, "rejected", None)?;
                let body = format!("O {seller_name} recusou a sua proposta por {name} ({}): está longe do que o clube pretende.", t.text());
                self.offer_news(fresh, next_id, format!("Proposta por {name} recusada"), body, o.id);
            }
            SellerReply::Bored => {
                let until = self.bored(o.id)?;
                let body = format!(
                    "O {seller_name} rejeitou mais uma proposta por {name} e está farto das negociações: não quer ouvir falar de propostas por ele até {}.",
                    cm_date(&until)
                );
                self.offer_news(fresh, next_id, format!("{seller_name} rompe negociações por {name}"), body, o.id);
            }
        }
        Ok(())
    }

    /// O clube que propôs responde ao "Pedir Mais" do treinador.
    fn buyer_answers(&mut self, o: &Offer, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let name = self.person_name(o.person)?;
        let buyer = self.club(o.buyer).short_name;
        let asked = o.deal_terms();
        let budget = (self.cash_of(o.buyer)? as f64 + 0.2 * self.club_base(o.buyer)?) * 0.5;
        let Some(f) = self.facts(o.person)?.filter(|f| f.club == Some(self.club_id)) else {
            self.set_status(o.id, "collapsed", None)?;
            return Ok(());
        };
        // `asked` guarda quanto valia a proposta inicial do clube
        let offered = o.asked.unwrap_or(o.fee) as f64;
        let reply =
            if self.window_open_for(o.buyer)? { buyer_reply(&asked, f.value, offered, budget, o.rounds) } else { BuyerReply::Withdraw };
        match reply {
            BuyerReply::Accept => {
                let (title, body) = self.complete_sale(o, asked.fee)?;
                self.offer_news(fresh, next_id, title, format!("O {buyer} aceitou as suas condições. {body}"), o.id);
            }
            BuyerReply::Counter(c) => {
                self.set_terms(o.id, &c, &o.loan_terms())?;
                let until = date::add_days(&self.date, INCOMING_DAYS).unwrap_or_default();
                self.set_status(o.id, "countered", Some(&until))?;
                let body = format!(
                    "O {buyer} não aceita {} por {name}, mas sobe a proposta para {}. Responda até {}.",
                    asked.text(),
                    c.text(),
                    cm_date(&until)
                );
                self.offer_news(fresh, next_id, format!("Nova proposta por {name}"), body, o.id);
            }
            BuyerReply::Withdraw => {
                self.set_status(o.id, "withdrawn", None)?;
                let body = format!("O {buyer} não aceita {} por {name} e desistiu do negócio.", asked.text());
                self.offer_news(fresh, next_id, format!("{buyer} desiste de {name}"), body, o.id);
            }
        }
        Ok(())
    }

    /// "Pedir Mais" numa proposta recebida, com condições (valor, prestações, percentagem da
    /// próxima venda, valor por jogos). O clube responde em 1 a 3 dias.
    pub fn ask_terms(&mut self, id: i32, terms: &DealTerms) -> Result<OfferView> {
        let o = self.offer(id)?;
        if o.kind != "transfer" || o.seller != Some(self.club_id) || !matches!(o.status.as_str(), "pending" | "countered") {
            return Err(Error::Invalid("Esta proposta já não aceita essa resposta.".into()));
        }
        terms.check()?;
        let value = self.facts(o.person)?.map_or(0, |f| f.value);
        let current = o.deal_terms();
        let asked = DealTerms { fee: finance::round_money(terms.fee as f64), exchange: None, public: false, ..*terms };
        if asked.worth(value) <= current.worth(value) {
            return Err(Error::Invalid("Peça mais do que a proposta.".into()));
        }
        // Na primeira vez, guarda quanto valia a proposta do clube
        let offered = o.asked.unwrap_or_else(|| current.worth(value).round() as i64);
        let reply = self.reply_on(o.person, (1, 3));
        let rounds = if o.status == "countered" { o.rounds + 1 } else { o.rounds };
        self.world.conn.execute(
            "UPDATE offers SET status = 'raised', asked = ?, answer_on = ?, rounds = ? WHERE id = ?",
            params![offered, reply, rounds, id],
        )?;
        self.set_terms(id, &asked, &o.loan_terms())?;
        self.unsaved = true;
        self.offer_view(id)
    }

    /// Propostas dos clubes da IA pelos jogadores do treinador (dias de mercado).
    fn ai_bids(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        type Mine = (i32, i32, Option<i32>, Option<i32>, Option<i64>);
        // Os do plantel (não os emprestados por outros clubes), com a lista
        let mine: Vec<Mine> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, COALESCE(pl.current_ability, 1), t.transfer, t.loan, t.asking
                 FROM people p JOIN players pl ON pl.person_id = p.id LEFT JOIN transfer_list t ON t.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1 AND p.id NOT IN (SELECT person_id FROM loans) ORDER BY p.id",
            )?
            .query_map([self.club_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        if mine.is_empty() {
            return Ok(());
        }
        // Os clubes geridos com a janela aberta: reputação, saldo, receitas e nível (média dos 11 melhores)
        let md = self.date[5..].to_string();
        let clubs: Vec<(i32, i32, i64, Option<i32>)> = self
            .world
            .conn
            .prepare_cached(&format!(
                "SELECT id, COALESCE(reputation, 0), COALESCE(cash, 0), division_id FROM clubs WHERE division_id IN ({}) AND id <> ? ORDER BY id",
                super::market::managed_ids()
            ))?
            .query_map([self.club_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut buyers: Vec<(i32, i32, f64, i32)> = Vec::new();
        for (id, rep, cash, division) in clubs {
            if !window_open(self.club_kind(division), &md) {
                continue;
            }
            let level: f64 = self
                .world
                .conn
                .prepare_cached(
                    "SELECT COALESCE(AVG(ca), 0) FROM (SELECT pl.current_ability AS ca FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC LIMIT 11)",
                )?
                .query_row([id], |r| r.get(0))?;
            let budget = (cash as f64 + 0.2 * self.club_base(id)?) * 0.5;
            buyers.push((id, rep, budget, level as i32));
        }
        let my_rep = self.club_rep(self.club_id)?;
        let mut rng = self.season_rng(0xB1D5);
        // Um de fora da lista, de vez em quando
        let unlisted: Vec<&Mine> = mine.iter().filter(|m| m.2.unwrap_or(0) == 0 && m.3.unwrap_or(0) == 0).collect();
        let surprise = (!unlisted.is_empty() && uniform(&mut rng) < UNLISTED_BID)
            .then(|| unlisted[(rng.next_u64() % unlisted.len() as u64) as usize].0);
        for &(id, ca, transfer, loan, asking) in &mine {
            let listed = transfer.unwrap_or(0) != 0;
            let loan_listed = loan.unwrap_or(0) != 0;
            let wanted = if loan_listed {
                uniform(&mut rng) < LOAN_BID
            } else if listed {
                uniform(&mut rng) < LISTED_BID
            } else {
                surprise == Some(id)
            };
            if !wanted || self.open_offer(id)?.is_some() {
                continue;
            }
            let Some(f) = self.facts(id)? else { continue };
            let as_loan = loan_listed && !listed;
            let fee = if as_loan {
                0
            } else if listed {
                let base = asking.unwrap_or(f.value) as f64 * (0.8 + 0.2 * uniform(&mut rng));
                finance::round_money(base.max(f.value as f64 * 0.7)).max(10_000)
            } else {
                finance::round_money(f.value as f64 * (1.0 + 0.3 * uniform(&mut rng))).max(10_000)
            };
            // Quem compra: um clube onde ele jogue e que ele aceite; quem pede emprestado: um mais pequeno
            let fits: Vec<i32> = buyers
                .iter()
                .filter(|&&(_, rep, budget, level)| {
                    if as_loan {
                        rep < my_rep && ca + 10 >= level
                    } else {
                        rep + personality::step_down(f.ambition) >= my_rep && ca >= level - 5 && ca <= level + 25 && fee as f64 <= budget
                    }
                })
                .map(|b| b.0)
                .collect();
            if fits.is_empty() {
                continue;
            }
            let buyer = fits[(rng.next_u64() % fits.len() as u64) as usize];
            let until = date::add_days(&self.date, INCOMING_DAYS).unwrap_or_default();
            self.world.conn.execute(
                "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES (?, ?, ?, ?, ?, 'pending', ?, ?)",
                params![if as_loan { "loan" } else { "transfer" }, id, buyer, self.club_id, fee, until, self.date],
            )?;
            let offer = self.world.conn.last_insert_rowid() as i32;
            // Condições variadas: prestações (pagando um pouco mais), valor por jogos; no
            // empréstimo, parte do salário e às vezes uma taxa ou uma opção de compra
            let (deal, loan) = if as_loan {
                let share = [50, 75, 100][(rng.next_u32() % 3) as usize];
                let monthly = if uniform(&mut rng) < 0.2 { finance::round_money(f.value as f64 * 0.005) } else { 0 };
                let option = if uniform(&mut rng) < 0.3 { finance::round_money(f.value as f64 * 1.2) } else { 0 };
                (DealTerms::default(), LoanTerms { months: 0, wage_share: share, monthly_fee: monthly, option_fee: option })
            } else {
                let mut d = DealTerms { fee, ..Default::default() };
                if uniform(&mut rng) < 0.25 {
                    d.months = [12, 24][(rng.next_u32() % 2) as usize];
                    d.fee = finance::round_money(fee as f64 * 1.05);
                }
                if uniform(&mut rng) < 0.2 {
                    d.apps_bonus = finance::round_money(fee as f64 * 0.1);
                }
                (d, LoanTerms::default())
            };
            self.set_terms(offer, &deal, &loan)?;
            let name = self.person_name(id)?;
            let buyer_name = self.club(buyer).short_name;
            let (title, body) = if as_loan {
                (
                    format!("Pedido de empréstimo de {name}"),
                    format!("O {buyer_name} quer {name} emprestado {}. Responda até {}.", loan.text(), cm_date(&until)),
                )
            } else {
                (
                    format!("Proposta por {name}"),
                    format!(
                        "O {buyer_name} oferece {} por {name} (valor {}). Responda até {}.",
                        deal.text(),
                        money_text(f.value),
                        cm_date(&until)
                    ),
                )
            };
            self.offer_news(fresh, next_id, title, body, offer);
        }
        Ok(())
    }

    /// Mensagem com os contratos do plantel que acabam até `cutoff` (sem adjunto a renovar).
    fn contract_warning(&mut self, cutoff: &str, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let names: Vec<String> = self
            .world
            .conn
            .prepare_cached(
                "SELECT display_name FROM people WHERE club_id = ? AND is_player = 1 AND contract_expires IS NOT NULL
                   AND contract_expires <= ? AND id NOT IN (SELECT person_id FROM loans) ORDER BY display_name",
            )?
            .query_map(params![self.club_id, cutoff], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if names.is_empty() {
            return Ok(());
        }
        let body = format!(
            "Os contratos destes jogadores acabam a {}: {}.\n\nOfereça-lhes um contrato novo no perfil (aba Transferência) ou saem livres.",
            cm_date(cutoff),
            names.join(", ")
        );
        fresh.push(self.news_item(*next_id, "contract", "Contratos a acabar".into(), body));
        *next_id += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;
    const FLAMENGO: i32 = 3675;

    /// Jogadores do Flamengo que o clube aceita vender (fora dos 11 melhores, sem empréstimo).
    fn for_sale(g: &Game) -> Vec<(i32, i64)> {
        let ids: Vec<i32> = g
            .world
            .conn
            .prepare("SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC, p.id")
            .unwrap()
            .query_map([FLAMENGO], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        ids.into_iter()
            .skip(11)
            .filter_map(|id| {
                let f = g.facts(id).unwrap()?;
                let (_, count) = g.squad_rank(FLAMENGO, id, f.role).unwrap();
                (f.value > 0 && count > MIN_ROLE[f.role] && g.loan_of(id).unwrap().is_none())
                    .then(|| (id, g.asking_price(id, &f, FLAMENGO).unwrap()))
            })
            .collect()
    }

    #[test]
    fn manager_buys_sells_and_renews() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-negocios-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Negócios".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
        };
        let mut g = Game::create(&path, &dir, &new, 3).unwrap();
        g.world.conn.execute("UPDATE clubs SET cash = 200000000 WHERE id = ?", [PALMEIRAS]).unwrap();
        g.refresh_budgets().unwrap();
        let targets = for_sale(&g);
        assert!(targets.len() >= 3, "{targets:?}");
        let (buy, ask) = targets[0];
        let (haggle, ask2) = targets[1];
        let (lowball, ask3) = targets[2];
        let cash_only = |fee: i64| DealTerms { fee, ..Default::default() };
        g.make_offer(buy, &cash_only(ask)).unwrap();
        g.make_offer(haggle, &cash_only((ask2 as f64 * 0.7) as i64 + 10_000)).unwrap();
        g.make_offer(lowball, &cash_only(ask3 / 10)).unwrap();
        assert!(g.make_offer(buy, &cash_only(ask)).is_err(), "uma proposta de cada vez");
        assert!(g.make_offer(lowball, &DealTerms { months: 7, ..cash_only(ask3) }).is_err(), "prestações de 12, 24 ou 36 meses");
        // Os clubes respondem em 1 a 4 dias
        for _ in 0..4 {
            g.advance_day().unwrap();
        }
        let status = |g: &Game, id: i32| g.transfer_view(id).unwrap().offer.map(|o| o.status);
        assert_eq!(status(&g, buy).as_deref(), Some("agreed"));
        assert_eq!(status(&g, haggle).as_deref(), Some("countered"));
        assert_eq!(status(&g, lowball), None, "recusada: já não há proposta em aberto");
        assert!(g.news().iter().any(|n| n.offer_id.is_some() && n.title.contains("aceite")));

        // O contrato: uma função abaixo da que espera pede mais; pouco dinheiro, recusa e diz
        // quanto quer; com o que pede, assina (e o clube paga o valor e o prémio)
        let t = g.contract_terms(buy, None).unwrap();
        assert!(!t.renewal && t.fee == ask && t.ends.len() == 5);
        if t.expected_role != "backup" {
            assert!(g.contract_terms(buy, Some("backup")).unwrap().wage > t.wage);
        }
        // A resposta do jogador chega no dia seguinte
        let next_day = |g: &mut Game| {
            g.date = date::add_days(&g.date, 1).unwrap();
            let (mut fresh, mut id) = (Vec::new(), 9000);
            g.contract_replies(&mut fresh, &mut id).unwrap();
            fresh
        };
        let reply = g.negotiate(buy, &t.role, 100, 0, t.years).unwrap();
        assert!(reply.pending && !reply.accepted && !reply.over, "{}", reply.text);
        assert!(g.negotiate(buy, &t.role, 100, 0, t.years).is_err(), "uma proposta de cada vez");
        assert!(next_day(&mut g).iter().any(|n| n.title.starts_with("Contraproposta")));
        let offer = g.open_contract(buy).unwrap().unwrap();
        assert_eq!(offer.status, "countered");
        assert_eq!(g.transfer_view(buy).unwrap().offer.unwrap().kind, "contract", "a aba mostra a proposta de contrato");
        // Aceitar a contraproposta fecha o negócio: o clube paga o valor e o prémio
        let cash = g.cash_of(PALMEIRAS).unwrap();
        assert_eq!(g.answer_offer(offer.id, "accept", None).unwrap().status, "done");
        assert_eq!(g.facts(buy).unwrap().unwrap().club, Some(PALMEIRAS));
        assert_eq!(g.cash_of(PALMEIRAS).unwrap(), cash - ask - t.bonus);

        // A contraproposta do clube vale o que ele pede; aceitar as condições passa ao contrato
        let offer = g.transfer_view(haggle).unwrap().offer.unwrap();
        let value = g.facts(haggle).unwrap().unwrap().value;
        assert!(offer.deal.worth(value) >= ask2 as f64 * 0.99, "{:?} {ask2}", offer.deal);
        assert!(offer.actions.iter().any(|a| a.key == "offer"), "pode fazer uma nova proposta");
        assert!(g.answer_offer(offer.id, "accept", None).unwrap().status == "agreed");

        // Renovação: três recusas e ele não volta a falar durante 30 dias
        let mine = g.world.conn.query_row(
            "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC LIMIT 1",
            [PALMEIRAS],
            |r| r.get::<_, i32>(0),
        ).unwrap();
        let role = g.contract_terms(mine, None).unwrap().role;
        for k in 0..3 {
            g.negotiate(mine, &role, 100, 0, 2).unwrap();
            let news = next_day(&mut g);
            assert_eq!(news.iter().any(|n| n.title.contains("rompe")), k == 2);
        }
        assert!(g.negotiate(mine, &role, 10_000_000, 0, 2).is_err());
        assert!(!g.transfer_view(mine).unwrap().can_renew);
        let other = g.world.conn.query_row(
            "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1 AND p.id <> ? AND p.id <> ? ORDER BY pl.current_ability DESC LIMIT 1",
            params![PALMEIRAS, mine, buy],
            |r| r.get::<_, i32>(0),
        ).unwrap();
        // Renovação com 5% a menos de salário, compensados no prémio: ele aceita e falta confirmar
        let before = g.facts(other).unwrap().unwrap().expires;
        let t = g.contract_terms(other, None).unwrap();
        assert!(t.renewal && t.roles.iter().filter(|r| !r.allowed).all(|r| r.youth));
        let wage = t.wage * 95 / 100;
        let bonus = t.bonus + (t.wage - wage) * 52 * t.years as i64 + 10_000;
        assert!(g.negotiate(other, &t.role, wage, bonus, t.years).unwrap().pending);
        assert!(next_day(&mut g).iter().any(|n| n.title.contains("aceita o contrato")));
        let offer = g.open_contract(other).unwrap().unwrap();
        assert_eq!(offer.status, "agreed");
        assert_eq!(g.facts(other).unwrap().unwrap().expires, before, "só depois de confirmar");
        assert_eq!(g.answer_offer(offer.id, "confirm", None).unwrap().status, "done");
        let after = g.facts(other).unwrap().unwrap();
        assert!(after.expires > before && after.wage == wage);
        assert_eq!(after.expires.as_deref(), Some(t.ends[t.years as usize - 1].as_str()));

        // Venda: na lista e barato, aparece uma proposta; aceitar vende-o
        let sell = g.world.conn.query_row(
            "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1 AND p.id <> ? ORDER BY pl.current_ability DESC LIMIT 1 OFFSET 15",
            params![PALMEIRAS, buy],
            |r| r.get::<_, i32>(0),
        ).unwrap();
        g.set_listing(sell, true, false, Some(10_000)).unwrap();
        let mut incoming = None;
        for _ in 0..40 {
            g.advance_day().unwrap();
            if let Some(o) = g.transfer_view(sell).unwrap().offer.filter(|o| o.incoming) {
                incoming = Some(o);
                break;
            }
        }
        let o = incoming.expect("uma proposta pelo jogador da lista");
        let cash = g.cash_of(PALMEIRAS).unwrap();
        assert_eq!(g.answer_offer(o.id, "accept", None).unwrap().status, "done");
        assert_eq!(g.facts(sell).unwrap().unwrap().club, Some(o.buyer.id));
        // Com prestações, hoje só entra a primeira
        let now = if o.deal.months > 0 { o.fee - o.fee / o.deal.months as i64 * (o.deal.months as i64 - 1) } else { o.fee };
        assert_eq!(g.cash_of(PALMEIRAS).unwrap(), cash + now);

        // Pré-contrato: um jogador de outro clube com 3 meses de contrato chega livre quando acaba
        let (pre, _) = for_sale(&g)[3];
        let expiry = date::add_days(&g.date, 90).unwrap();
        g.world.conn.execute("UPDATE people SET contract_expires = ? WHERE id = ?", params![expiry, pre]).unwrap();
        let t = g.contract_terms(pre, None).unwrap();
        assert!(t.precontract && t.fee == 0);
        g.negotiate(pre, &t.role, t.wage, t.bonus, t.years).unwrap();
        // Com outras propostas pode demorar até 5 dias; tira-se o rival para o teste ser certo
        g.world.conn.execute("UPDATE contract_talks SET rival = NULL, rival_wage = NULL WHERE person_id = ?", [pre]).unwrap();
        for _ in 0..6 {
            if g.open_contract(pre).unwrap().is_some_and(|o| o.status != "pending") {
                break;
            }
            next_day(&mut g);
        }
        let offer = g.open_contract(pre).unwrap().unwrap();
        assert_eq!(g.answer_offer(offer.id, "confirm", None).unwrap().status, "signed");
        assert_eq!(g.facts(pre).unwrap().unwrap().club, Some(FLAMENGO), "fica até ao fim do contrato");
        g.date = date::add_days(&expiry, 1).unwrap();
        let (mut fresh, mut id) = (Vec::new(), 9000);
        g.precontracts_due(&mut fresh, &mut id).unwrap();
        assert_eq!(g.facts(pre).unwrap().unwrap().club, Some(PALMEIRAS));
        assert!(fresh.iter().any(|n| n.title.contains("chega ao clube")));

        // Um livre com uma proposta melhor de outro clube escolhe o outro clube
        let free: i32 = g
            .world
            .conn
            .query_row(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id IS NULL AND p.is_player = 1
                 AND p.id NOT IN (SELECT person_id FROM retired) ORDER BY pl.current_ability DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let t = g.contract_terms(free, None).unwrap();
        g.negotiate(free, &t.role, t.wage, t.bonus, t.years).unwrap();
        let rival: i32 = g
            .world
            .conn
            .query_row("SELECT id FROM clubs WHERE id <> ? ORDER BY reputation DESC LIMIT 1", [PALMEIRAS], |r| r.get(0))
            .unwrap();
        g.world
            .conn
            .execute("UPDATE contract_talks SET rival = ?, rival_wage = ? WHERE person_id = ?", params![rival, t.wage * 3, free])
            .unwrap();
        let mut news = Vec::new();
        for _ in 0..6 {
            news.extend(next_day(&mut g));
            if g.open_contract(free).unwrap().is_none() {
                break;
            }
        }
        assert!(news.iter().any(|n| n.title.contains("escolhe o")), "{:?}", news.iter().map(|n| &n.title).collect::<Vec<_>>());
        assert_eq!(g.facts(free).unwrap().unwrap().club, Some(rival));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn personality_in_the_market() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-personalidade-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Personalidade".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
        };
        let mut g = Game::create(&path, &dir, &new, 5).unwrap();
        let set = |g: &Game, id: i32, col: &str, v: i32| {
            g.world.conn.execute(&format!("UPDATE people SET {col} = ? WHERE id = ?"), [v, id]).unwrap();
        };
        let mine = g
            .world
            .conn
            .query_row(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC LIMIT 1 OFFSET 3",
                [PALMEIRAS],
                |r| r.get::<_, i32>(0),
            )
            .unwrap();

        // Lealdade: o leal pede menos para renovar
        set(&g, mine, "loyalty", 20);
        let loyal = g.contract_terms(mine, None).unwrap().wage;
        set(&g, mine, "loyalty", 1);
        let mercenary = g.contract_terms(mine, None).unwrap().wage;
        assert!(loyal < mercenary, "{loyal} {mercenary}");

        // ... e o clube dele pede mais para o vender
        let (theirs, _) = for_sale(&g)[0];
        set(&g, theirs, "loyalty", 1);
        let cheap = g.asking_price(theirs, &g.facts(theirs).unwrap().unwrap(), FLAMENGO).unwrap();
        set(&g, theirs, "loyalty", 20);
        let dear = g.asking_price(theirs, &g.facts(theirs).unwrap().unwrap(), FLAMENGO).unwrap();
        assert!(dear > cheap, "{dear} {cheap}");

        // Ambição: recusar a proposta de um clube maior deixa o ambicioso desiludido
        let big: i32 = g.world.conn.query_row("SELECT id FROM clubs ORDER BY reputation DESC LIMIT 1", [], |r| r.get(0)).unwrap();
        let offer = |g: &mut Game, id: i32| {
            g.world
                .conn
                .execute(
                    "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('transfer', ?, ?, ?, 1000000, 'pending', ?, ?)",
                    params![id, big, PALMEIRAS, g.date, g.date],
                )
                .unwrap();
            g.world.conn.last_insert_rowid() as i32
        };
        for (ambition, letdown) in [(20, true), (8, false)] {
            set(&g, mine, "ambition", ambition);
            let before = g.squads.state(mine).morale;
            let id = offer(&mut g, mine);
            g.answer_offer(id, "reject", None).unwrap();
            let after = g.squads.state(mine).morale;
            assert_eq!(after < before, letdown, "Ambição {ambition}: {before} → {after}");
            assert_eq!(g.news().iter().any(|n| n.offer_id == Some(id) && n.title.contains("desiludido")), letdown);
        }
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
