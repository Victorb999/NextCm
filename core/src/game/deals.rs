//! Mercado do treinador (marco 3.8, passo 3): propostas por jogadores de outros clubes,
//! empréstimos pedidos, propostas dos clubes da IA pelos jogadores do plantel, negociação
//! do contrato (contratações e renovações) e lista de transferências. Os outros clubes
//! respondem no dia seguinte, com uma mensagem que traz os botões (`news.offer_id`).
//! Regras e números no detalhe do 3.8 no ROADMAP.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::market::{
    MIN_ROLE, contract_until, contract_years, leaves_europe, leaving_europe, loan_until, money_text, role_of, window_open,
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
/// Dias para fechar o contrato depois de o clube aceitar (ou para pagar o que ele pede).
const AGREED_DAYS: i32 = 14;
/// Propostas recusadas antes de o jogador romper as negociações, e dias até voltar a falar.
const TRIES: i32 = 3;
const COOLDOWN_DAYS: i32 = 30;
/// O clube que vende faz uma contraproposta se a oferta chegar a esta parte do que pede.
const COUNTER_FROM: f64 = 0.6;
/// Um clube que compra paga até mais 30% do que ofereceu quando o treinador pede mais.
const RAISE_UP_TO: f64 = 1.3;
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
    pub note: Option<String>,
    /// Sem conhecimento do jogador, a direção limita a proposta: a frase do CM.
    pub budget_note: Option<String>,
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
    /// O que pede (ou o acordado): salário semanal, prémio e anos.
    #[ts(type = "number")]
    pub wage: i64,
    #[ts(type = "number")]
    pub bonus: i64,
    pub years: i32,
    pub text: String,
}

/// O que um jogador pede por um contrato.
struct Demand {
    wage: i64,
    bonus: i64,
    years: i32,
}

pub(super) struct Offer {
    id: i32,
    kind: String,
    person: i32,
    buyer: i32,
    seller: Option<i32>,
    fee: i64,
    asked: Option<i64>,
    status: String,
    answer_on: Option<String>,
}

/// O que o mercado precisa de saber de um jogador hoje.
struct Facts {
    club: Option<i32>,
    ca: i32,
    role: usize,
    age: i32,
    wage: i64,
    value: i64,
    expires: Option<String>,
    joined: Option<String>,
    ambition: f64,
    loyalty: f64,
}

const OFFER_COLUMNS: &str = "id, kind, person_id, buyer, seller, fee, asked, status, answer_on";

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
    })
}

/// Salário arredondado a €100 (abaixo de €10K por semana) ou a €1K.
fn round_wage(v: f64) -> i64 {
    let step = if v < 10_000.0 { 100.0 } else { 1_000.0 };
    ((v / step).round() * step) as i64
}

fn action(key: &str, label: &str) -> OfferAction {
    OfferAction { key: key.into(), label: label.into() }
}

impl Game {
    fn offer(&self, id: i32) -> Result<Offer> {
        self.world
            .conn
            .query_row(&format!("SELECT {OFFER_COLUMNS} FROM offers WHERE id = ?"), [id], offer_row)
            .optional()?
            .ok_or_else(|| Error::Invalid("Proposta não encontrada.".into()))
    }

    fn offers_where(&self, filter: &str, args: impl rusqlite::Params) -> Result<Vec<Offer>> {
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
                "person_id = ?1 AND (buyer = ?2 OR seller = ?2) AND status IN ('pending', 'countered', 'agreed', 'raised')",
                params![person, self.club_id],
            )?
            .pop())
    }

    fn set_status(&self, id: i32, status: &str, answer_on: Option<&str>) -> Result<()> {
        self.world.conn.execute("UPDATE offers SET status = ?, answer_on = ? WHERE id = ?", params![status, answer_on, id])?;
        Ok(())
    }

    fn facts(&self, id: i32) -> Result<Option<Facts>> {
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
    fn loan_of(&self, person: i32) -> Result<Option<(i32, String)>> {
        Ok(self
            .world
            .conn
            .query_row("SELECT parent_id, until FROM loans WHERE person_id = ?", [person], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?)
    }

    fn club_rep(&self, club: i32) -> Result<i32> {
        Ok(self.world.conn.query_row("SELECT COALESCE(reputation, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?)
    }

    fn cash_of(&self, club: i32) -> Result<i64> {
        Ok(self.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?)
    }

    pub(super) fn kind_of(&self, club: i32) -> Result<SeasonKind> {
        let division: Option<i32> = self.world.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [club], |r| r.get(0))?;
        Ok(self.club_kind(division))
    }

    fn window_open_for(&self, club: i32) -> Result<bool> {
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
    fn deal_rng(&self, person: i32, salt: u64) -> ChaCha8Rng {
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
    fn expected_role(&self, person: i32, f: &Facts) -> Result<usize> {
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
    fn demand(&self, person: i32, f: &Facts, role: usize, renewal: bool, free: bool) -> Result<Demand> {
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
    fn contract_end(&self, f: &Facts, renewal: bool, years: i32) -> Result<String> {
        let kind = self.kind_of(self.club_id)?;
        let from = if renewal { f.expires.clone().filter(|e| e.as_str() > self.date.as_str()) } else { None };
        let start = from.and_then(|e| date::add_days(&e, 1)).unwrap_or_else(|| self.date.clone());
        Ok(contract_until(kind, &start, years))
    }

    /// A direção autoriza este valor em transferências?
    fn check_budget(&self, amount: i64) -> Result<()> {
        let budget = self.transfer_budget()?;
        if amount > budget {
            return Err(Error::Invalid(format!("A sua direcção autoriza transferências até {}.", money_text(budget))));
        }
        Ok(())
    }

    /// Uma mensagem fora do avanço do dia (respostas imediatas do treinador).
    pub(super) fn post_news(&mut self, title: String, body: String, offer: Option<i32>) {
        let id = self.news.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let mut n = self.news_item(id, "message", title, body);
        n.offer_id = offer;
        self.news.insert(0, n);
    }

    fn offer_news(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32, title: String, body: String, offer: i32) {
        let mut n = self.news_item(*next_id, "message", title, body);
        n.offer_id = Some(offer);
        fresh.push(n);
        *next_id += 1;
    }

    /// Depois de um jogador entrar ou sair do clube do treinador.
    fn squad_changed(&mut self) -> Result<()> {
        self.clean_selection()?;
        self.refresh_live()?;
        self.unsaved = true;
        Ok(())
    }

    /// Empresta um jogador até ao fim da época do clube que o recebe.
    fn execute_loan(&mut self, person: i32, parent: i32, to: i32) -> Result<String> {
        let until = loan_until(self.kind_of(to)?, &self.date);
        self.world
            .conn
            .execute("INSERT OR REPLACE INTO loans (person_id, parent_id, until) VALUES (?, ?, ?)", params![person, parent, until])?;
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
        let offer = match self.open_offer(person)? {
            Some(o) => Some(self.offer_view(o.id)?),
            None => None,
        };
        let window = self.window_open_for(self.club_id)?;
        let blocked = self.blocked_until(person)?;
        let busy = offer.is_some();
        let mut note = None;
        if let Some(b) = &blocked {
            note = Some(format!("Não quer falar de contrato até {}.", cm_date(b)));
        } else if !mine && f.club.is_some() && !window {
            note = Some("A janela de transferências está fechada: as propostas esperam pela próxima.".into());
        } else if loan.is_some() && !mine && f.club != Some(self.club_id) {
            note = Some("Está emprestado: o clube onde joga não o pode vender.".into());
        }
        Ok(TransferView {
            person_id: person,
            mine,
            free: f.club.is_none(),
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
            can_offer: f.club != Some(self.club_id) && !busy && (f.club.is_none() || window) && self.loan_of(person)?.is_none(),
            can_loan: f.club.is_some() && f.club != Some(self.club_id) && !busy && window && self.loan_of(person)?.is_none(),
            can_renew: mine && blocked.is_none() && !busy,
            offer,
            note,
        })
    }

    /// Uma proposta com o texto e os botões da situação.
    pub fn offer_view(&self, id: i32) -> Result<OfferView> {
        let o = self.offer(id)?;
        let name = self.person_name(o.person)?;
        let incoming = o.buyer != self.club_id;
        let buyer = self.club(o.buyer);
        let seller = o.seller.map(|c| self.club(c));
        let seller_name = seller.as_ref().map_or(String::new(), |c| c.short_name.clone());
        let fee = money_text(o.fee);
        let loan = o.kind == "loan";
        let until = o.answer_on.as_deref().map(cm_date).unwrap_or_default();
        let (text, actions) = match (incoming, o.status.as_str()) {
            _ if o.kind == "request" => match o.status.as_str() {
                "pending" => (
                    format!("{name} pede para ser posto na lista de transferências. Responda até {until}."),
                    vec![action("accept", "Pôr na Lista"), action("reject", "Recusar")],
                ),
                "done" => (format!("{name} está na lista de transferências, a pedido dele."), Vec::new()),
                _ => (format!("Recusou o pedido de {name} para sair."), Vec::new()),
            },
            (false, "pending") if loan => {
                (format!("Pediu {name} emprestado ao {seller_name}; a resposta chega amanhã."), vec![action("withdraw", "Retirar")])
            }
            (false, "pending") => (
                format!("Ofereceu {fee} ao {seller_name} por {name}; a resposta chega amanhã."),
                vec![action("withdraw", "Retirar Proposta")],
            ),
            (false, "countered") => (
                format!("O {seller_name} pede {fee} por {name}. Tem até {until} para decidir."),
                vec![action("pay", &format!("Pagar {fee}")), action("withdraw", "Desistir")],
            ),
            (false, "agreed") if o.seller.is_none() => (
                format!("{name} está livre: negoceie o contrato até {until}."),
                vec![action("negotiate", "Negociar Contrato"), action("withdraw", "Desistir")],
            ),
            (false, "agreed") => (
                format!("O {seller_name} aceitou {fee} por {name}. Falta o contrato com o jogador (até {until})."),
                vec![action("negotiate", "Negociar Contrato"), action("withdraw", "Desistir")],
            ),
            (true, "pending") if loan => (
                format!("O {} quer {name} emprestado até ao fim da época. Responda até {until}.", buyer.short_name),
                vec![action("accept", "Aceitar"), action("reject", "Recusar")],
            ),
            (true, "pending") => (
                format!("O {} oferece {fee} por {name}. Responda até {until}.", buyer.short_name),
                vec![action("accept", "Aceitar"), action("counter", "Pedir Mais"), action("reject", "Recusar")],
            ),
            (true, "raised") => (
                format!("Pediu {} ao {} por {name}; a resposta chega amanhã.", money_text(o.asked.unwrap_or(0)), buyer.short_name),
                Vec::new(),
            ),
            (_, "done") => ("Negócio concluído.".into(), Vec::new()),
            (_, "rejected") => ("Proposta recusada.".into(), Vec::new()),
            (_, "withdrawn") => ("Proposta retirada.".into(), Vec::new()),
            _ => ("Negócio desfeito.".into(), Vec::new()),
        };
        Ok(OfferView { id, kind: o.kind, incoming, person_id: o.person, name, buyer, seller, fee: o.fee, status: o.status, text, actions })
    }

    // ---------------------------------------------------------------- o treinador compra

    /// Proposta de transferência por um jogador de outro clube (ou por um livre, que passa
    /// logo à negociação do contrato). O clube responde no dia seguinte.
    pub fn make_offer(&mut self, person: i32, fee: i64) -> Result<TransferView> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só se fazem propostas por jogadores.".into()))?;
        if f.club == Some(self.club_id) {
            return Err(Error::Invalid("Já é do seu plantel.".into()));
        }
        if let Some((parent, _)) = self.loan_of(person)? {
            return Err(Error::Invalid(format!("Está emprestado pelo {}: não pode ser vendido agora.", self.club(parent).short_name)));
        }
        if self.open_offer(person)?.is_some() {
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
                if !self.window_open_for(self.club_id)? {
                    return Err(Error::Invalid("A janela de transferências está fechada.".into()));
                }
                if fee <= 0 {
                    return Err(Error::Invalid("Indique o valor da proposta.".into()));
                }
                let (limit, why) = self.budget_for(person)?;
                if fee > limit {
                    return Err(Error::Invalid(
                        why.unwrap_or_else(|| format!("A sua direcção autoriza transferências até {}.", money_text(limit))),
                    ));
                }
                let tomorrow = date::add_days(&self.date, 1).unwrap_or_default();
                self.world.conn.execute(
                    "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('transfer', ?, ?, ?, ?, 'pending', ?, ?)",
                    params![person, self.club_id, seller, finance::round_money(fee as f64), tomorrow, created],
                )?;
            }
        }
        self.unsaved = true;
        self.transfer_view(person)
    }

    /// Pedido de empréstimo de um jogador de outro clube até ao fim da época.
    pub fn ask_loan(&mut self, person: i32) -> Result<TransferView> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só se pedem jogadores emprestados.".into()))?;
        let Some(seller) = f.club.filter(|&c| c != self.club_id) else {
            return Err(Error::Invalid("Só se pedem emprestados jogadores de outros clubes.".into()));
        };
        if !self.window_open_for(self.club_id)? {
            return Err(Error::Invalid("A janela de transferências está fechada.".into()));
        }
        if self.loan_of(person)?.is_some() || self.open_offer(person)?.is_some() {
            return Err(Error::Invalid("Este jogador não pode ser pedido agora.".into()));
        }
        let tomorrow = date::add_days(&self.date, 1).unwrap_or_default();
        self.world.conn.execute(
            "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('loan', ?, ?, ?, 0, 'pending', ?, ?)",
            params![person, self.club_id, seller, tomorrow, self.date],
        )?;
        self.unsaved = true;
        self.transfer_view(person)
    }

    /// Resposta do treinador numa proposta: "accept", "reject", "counter" (com `fee`), "pay", "withdraw".
    pub fn answer_offer(&mut self, id: i32, key: &str, fee: Option<i64>) -> Result<OfferView> {
        let o = self.offer(id)?;
        if o.kind == "request" {
            return self.answer_request(&o, key);
        }
        let incoming = o.buyer != self.club_id;
        match (incoming, o.status.as_str(), key) {
            (false, "pending" | "countered" | "agreed", "withdraw") => self.set_status(id, "withdrawn", None)?,
            (false, "countered", "pay") => {
                self.check_budget(o.fee)?;
                let until = date::add_days(&self.date, AGREED_DAYS).unwrap_or_default();
                self.set_status(id, "agreed", Some(&until))?;
            }
            (true, "pending", "accept") => {
                if !self.window_open_for(o.buyer)? {
                    self.set_status(id, "collapsed", None)?;
                    return Err(Error::Invalid("A janela do clube que propôs já fechou.".into()));
                }
                let (title, body) = self.complete_sale(&o, o.fee)?;
                self.post_news(title, body, Some(id));
            }
            (true, "pending", "reject") => {
                self.set_status(id, "rejected", None)?;
                if let Some(text) = self.letdown(&o)? {
                    let name = self.person_name(o.person)?;
                    self.post_news(format!("{name} desiludido"), text, Some(id));
                }
            }
            (true, "pending", "counter") if o.kind == "transfer" => {
                let asked = fee.filter(|&f| f > o.fee).ok_or_else(|| Error::Invalid("Peça mais do que a proposta.".into()))?;
                let tomorrow = date::add_days(&self.date, 1).unwrap_or_default();
                self.world.conn.execute(
                    "UPDATE offers SET status = 'raised', asked = ?, answer_on = ? WHERE id = ?",
                    params![finance::round_money(asked as f64), tomorrow, id],
                )?;
            }
            _ => return Err(Error::Invalid("Esta proposta já não aceita essa resposta.".into())),
        }
        self.unsaved = true;
        self.offer_view(id)
    }

    /// Jogador de um clube europeu que não aceita vir para o clube do treinador, fora da Europa.
    fn stays_in_europe(&self, person: i32, f: &Facts, club: i32) -> Result<bool> {
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
            let until = self.execute_loan(o.person, self.club_id, o.buyer)?;
            (format!("{name} emprestado"), format!("{name} vai jogar emprestado no {buyer} até {}.", cm_date(&until)))
        } else {
            let wage = wage_demand(f.value.max(f.ca as i64 * 20_000), f.wage, self.club_rep(o.buyer)?);
            let years = contract_years(f.age, &mut self.deal_rng(o.person, 0x5A1E));
            let kind = self.kind_of(o.buyer)?;
            // Um dos valiosos: a direção e os adeptos não gostam (antes de ele sair)
            let idol = self.idol_sold(o.person)?;
            self.execute_move(o.person, Some(self.club_id), Some(o.buyer), fee, wage, years, "transfer", kind)?;
            self.world.conn.execute("UPDATE offers SET fee = ? WHERE id = ?", params![fee, o.id])?;
            self.sale_to_budget(fee)?;
            let mut body = format!("{name} foi vendido ao {buyer} por {}.", money_text(fee));
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

    fn blocked_until(&self, person: i32) -> Result<Option<String>> {
        let b: Option<Option<String>> =
            self.world.conn.query_row("SELECT blocked_until FROM talks WHERE person_id = ?", [person], |r| r.get(0)).optional()?;
        Ok(b.flatten().filter(|d| d.as_str() > self.date.as_str()))
    }

    /// Com quem o treinador pode negociar um contrato: um do plantel (renovação) ou um jogador
    /// cujo clube aceitou a proposta (ou um livre). Devolve os factos, o nome e a proposta acordada.
    fn contract_party(&self, person: i32) -> Result<(Facts, String, Option<Offer>)> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só os jogadores assinam contratos.".into()))?;
        let name = self.person_name(person)?;
        let loaned_in = self.loan_of(person)?.is_some_and(|l| l.0 != self.club_id);
        let mine = f.club == Some(self.club_id) && !loaned_in;
        let deal = self.open_offer(person)?.filter(|o| o.buyer == self.club_id && o.status == "agreed");
        if !mine && deal.is_none() {
            return Err(Error::Invalid("Primeiro o clube dele tem de aceitar uma proposta.".into()));
        }
        if let Some(b) = self.blocked_until(person)? {
            return Err(Error::Invalid(format!("{name} não quer falar de contrato até {}.", cm_date(&b))));
        }
        Ok((f, name, deal))
    }

    fn role_index(key: &str) -> Result<usize> {
        SQUAD_ROLES.iter().position(|r| r.0 == key).ok_or_else(|| Error::Invalid("Função desconhecida.".into()))
    }

    fn role_allowed(role: usize, age: i32) -> bool {
        match role {
            4 => age <= PROSPECT_AGE,
            5 => age <= YOUNGSTER_AGE,
            _ => true,
        }
    }

    /// As telas "Definir Função No Clube" e "Propôr Contrato": as funções e o que o jogador
    /// pede pela função `role` (sem ela, pela que ele espera).
    pub fn contract_terms(&self, person: i32, role: Option<&str>) -> Result<ContractTerms> {
        let (f, name, deal) = self.contract_party(person)?;
        let expected = self.expected_role(person, &f)?;
        let chosen = match role {
            Some(k) => Self::role_index(k)?,
            None => expected,
        };
        let renewal = deal.is_none();
        let free = deal.as_ref().is_some_and(|o| o.seller.is_none());
        let d = self.demand(person, &f, chosen, renewal, free)?;
        let ends = (1..=5).map(|y| self.contract_end(&f, renewal, y)).collect::<Result<Vec<_>>>()?;
        Ok(ContractTerms {
            person_id: person,
            name,
            club: f.club.map(|c| self.club(c)),
            renewal,
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
            fee: deal.map_or(0, |o| o.fee),
            cash: self.cash_of(self.club_id)?,
            budget: self.transfer_budget()?,
            max_wage: self.board_view()?.max_wage,
        })
    }

    /// Proposta de contrato: função, salário semanal, prémio de assinatura e anos. O jogador
    /// aceita se o contrato todo (salário × semanas + prémio) chegar ao que pede, com o salário
    /// a pelo menos 90% do pedido (cada ano a mais ou a menos do que quer pede +4%); senão diz
    /// o que quer e, à terceira recusa, rompe as negociações por 30 dias.
    pub fn negotiate(&mut self, person: i32, role: &str, wage: i64, bonus: i64, years: i32) -> Result<ContractReply> {
        if !(1..=5).contains(&years) {
            return Err(Error::Invalid("Contratos de 1 a 5 anos.".into()));
        }
        let (f, name, deal) = self.contract_party(person)?;
        let role_i = Self::role_index(role)?;
        if !Self::role_allowed(role_i, f.age) {
            return Err(Error::Invalid("Vetado pela direcção devido à idade do jogador.".into()));
        }
        let (wage, bonus) = (wage.max(0), bonus.max(0));
        // Os limites da direção: salário máximo, folha salarial e orçamento de transferências
        let board = self.board_view()?;
        if wage > board.max_wage {
            return Err(Error::Invalid(format!("A sua direcção autoriza salários até {} por semana.", money_text(board.max_wage))));
        }
        let old = if deal.is_none() { f.wage } else { 0 };
        if board.wage_bill - old + wage > board.wage_budget {
            return Err(Error::Invalid(format!(
                "A folha salarial passaria o limite da direcção ({} por semana).",
                money_text(board.wage_budget)
            )));
        }
        let fee = deal.as_ref().map_or(0, |o| o.fee);
        self.check_budget(fee + bonus)?;
        // Não desce para um clube muito menor (quanto mais ambicioso, menos aceita descer)
        if let (Some(o), Some(club)) = (&deal, f.club)
            && self.club_rep(club)? > self.club_rep(self.club_id)? + personality::step_down(f.ambition)
        {
            self.set_status(o.id, "collapsed", None)?;
            self.unsaved = true;
            return Ok(ContractReply {
                accepted: false,
                over: true,
                wage: 0,
                bonus: 0,
                years,
                text: format!("{name} não está interessado em mudar para um clube menor."),
            });
        }
        // Um europeu raramente aceita sair da Europa
        if let (Some(o), Some(club)) = (&deal, f.club)
            && self.stays_in_europe(person, &f, club)?
        {
            self.set_status(o.id, "collapsed", None)?;
            self.unsaved = true;
            return Ok(ContractReply {
                accepted: false,
                over: true,
                wage: 0,
                bonus: 0,
                years,
                text: format!("{name} não quer deixar o futebol europeu."),
            });
        }
        let free = deal.as_ref().is_some_and(|o| o.seller.is_none());
        let d = self.demand(person, &f, role_i, deal.is_none(), free)?;
        let wanted = d.wage as f64 * (1.0 + 0.04 * (years - d.years).abs() as f64);
        let weeks = 52.0 * years as f64;
        let enough = wage as f64 >= 0.9 * wanted && wage as f64 * weeks + bonus as f64 >= wanted * weeks + d.bonus as f64;
        if !enough {
            let tries: i32 =
                self.world.conn.query_row("SELECT tries FROM talks WHERE person_id = ?", [person], |r| r.get(0)).optional()?.unwrap_or(0)
                    + 1;
            let over = tries >= TRIES;
            let blocked = over.then(|| date::add_days(&self.date, COOLDOWN_DAYS).unwrap_or_default());
            self.world.conn.execute(
                "INSERT OR REPLACE INTO talks (person_id, tries, blocked_until) VALUES (?, ?, ?)",
                params![person, if over { 0 } else { tries }, blocked],
            )?;
            if over && let Some(o) = &deal {
                self.set_status(o.id, "collapsed", None)?;
            }
            self.unsaved = true;
            let text = if over {
                format!("{name} rompeu as negociações e não volta a falar de contrato nos próximos {COOLDOWN_DAYS} dias.")
            } else {
                format!("{name} recusa a proposta.")
            };
            return Ok(ContractReply { accepted: false, over, wage: round_wage(wanted), bonus: d.bonus, years: d.years, text });
        }
        let kind = self.kind_of(self.club_id)?;
        let until = self.contract_end(&f, deal.is_none(), years)?;
        let text = match deal {
            None => {
                self.world.conn.execute("UPDATE people SET contract_expires = ?, wage = ? WHERE id = ?", params![until, wage, person])?;
                format!("{name} renovou até {} por {} por semana.", cm_date(&until), money_text(wage))
            }
            Some(o) => {
                if o.seller.is_some() && !self.window_open_for(self.club_id)? {
                    self.set_status(o.id, "collapsed", None)?;
                    self.unsaved = true;
                    return Ok(ContractReply {
                        accepted: false,
                        over: true,
                        wage,
                        bonus,
                        years,
                        text: "A janela de transferências fechou.".into(),
                    });
                }
                let move_kind = if o.seller.is_some() { "transfer" } else { "free" };
                self.execute_move(person, o.seller, Some(self.club_id), o.fee, wage, years, move_kind, kind)?;
                self.set_status(o.id, "done", None)?;
                let how = match o.seller {
                    Some(s) => format!("do {} por {}", self.club(s).short_name, money_text(o.fee)),
                    None => "a custo zero".into(),
                };
                let text = format!("{name} assinou até {} ({} por semana). Chega {how}.", cm_date(&until), money_text(wage));
                self.post_news(format!("{name} contratado"), text.clone(), None);
                self.squad_changed()?;
                text
            }
        };
        self.book(self.club_id, "bonuses", bonus)?;
        self.spend_budget(fee + bonus)?;
        self.world.conn.execute("DELETE FROM talks WHERE person_id = ?", [person])?;
        self.world.conn.execute("INSERT OR REPLACE INTO squad_roles (person_id, role) VALUES (?, ?)", params![person, role])?;
        self.remove_unhappy(person, "contract")?;
        self.unsaved = true;
        Ok(ContractReply { accepted: true, over: true, wage, bonus, years, text })
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
        for o in
            self.offers_where("buyer = ?1 AND kind <> 'request' AND status = 'pending' AND answer_on <= ?2", params![self.club_id, today])?
        {
            self.seller_answers(&o, fresh, next_id)?;
        }
        for o in self.offers_where("seller = ?1 AND status = 'raised' AND answer_on <= ?2", params![self.club_id, today])? {
            self.buyer_answers(&o, fresh, next_id)?;
        }
        // Prazos: propostas recebidas sem resposta, acordos sem contrato
        for o in
            self.offers_where("seller = ?1 AND kind <> 'request' AND status = 'pending' AND answer_on < ?2", params![self.club_id, today])?
        {
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

    /// O clube que vende responde a uma proposta do treinador.
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
        if o.kind == "loan" {
            // Empresta quem não é dos 18 melhores e é jovem ou está mesmo fora das contas
            let yes = !on_loan && !self.recent(&f) && rank >= 18 && (f.age <= 23 || rank >= 22) && self.window_open_for(self.club_id)?;
            if yes {
                let until = self.execute_loan(o.person, seller, self.club_id)?;
                self.set_status(o.id, "done", None)?;
                let body = format!("O {seller_name} aceitou emprestar {name} até {}. Já está no seu plantel.", cm_date(&until));
                self.offer_news(fresh, next_id, format!("{name} emprestado ao clube"), body, o.id);
                self.squad_changed()?;
            } else {
                self.set_status(o.id, "rejected", None)?;
                let body = format!("O {seller_name} não quer emprestar {name}: conta com ele.");
                self.offer_news(fresh, next_id, format!("Empréstimo de {name} recusado"), body, o.id);
            }
            return Ok(());
        }
        if on_loan || count <= MIN_ROLE[f.role] {
            self.set_status(o.id, "rejected", None)?;
            let body = format!("O {seller_name} recusou a proposta por {name}: não está à venda.");
            self.offer_news(fresh, next_id, format!("Proposta por {name} recusada"), body, o.id);
            return Ok(());
        }
        let asking = self.asking_price(o.person, &f, seller)?;
        let fee = money_text(o.fee);
        let deadline = date::add_days(&self.date, AGREED_DAYS).unwrap_or_default();
        if o.fee >= asking {
            self.set_status(o.id, "agreed", Some(&deadline))?;
            let body = format!("O {seller_name} aceitou a sua proposta de {fee} por {name}. Negoceie agora o contrato com o jogador.");
            self.offer_news(fresh, next_id, format!("Proposta por {name} aceite"), body, o.id);
        } else if o.fee as f64 >= COUNTER_FROM * asking as f64 {
            self.world
                .conn
                .execute("UPDATE offers SET status = 'countered', fee = ?, answer_on = ? WHERE id = ?", params![asking, deadline, o.id])?;
            let body = format!("O {seller_name} recusou {fee} por {name}, mas aceita vendê-lo por {}.", money_text(asking));
            self.offer_news(fresh, next_id, format!("Contraproposta por {name}"), body, o.id);
        } else {
            self.set_status(o.id, "rejected", None)?;
            let body = format!("O {seller_name} recusou a sua proposta de {fee} por {name}: está longe do que pede.");
            self.offer_news(fresh, next_id, format!("Proposta por {name} recusada"), body, o.id);
        }
        Ok(())
    }

    /// O clube que propôs responde ao "Pedir Mais" do treinador.
    fn buyer_answers(&mut self, o: &Offer, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let name = self.person_name(o.person)?;
        let buyer = self.club(o.buyer).short_name;
        let asked = o.asked.unwrap_or(o.fee);
        let budget = (self.cash_of(o.buyer)? as f64 + 0.2 * self.club_base(o.buyer)?) * 0.5;
        let still_mine = self.facts(o.person)?.is_some_and(|f| f.club == Some(self.club_id));
        if still_mine && asked as f64 <= o.fee as f64 * RAISE_UP_TO && asked as f64 <= budget && self.window_open_for(o.buyer)? {
            let (title, body) = self.complete_sale(o, asked)?;
            self.offer_news(fresh, next_id, title, format!("O {buyer} aceitou pagar o que pediu. {body}"), o.id);
        } else {
            self.set_status(o.id, "withdrawn", None)?;
            let body = format!("O {buyer} não paga {} por {name} e desistiu.", money_text(asked));
            self.offer_news(fresh, next_id, format!("{buyer} desiste de {name}"), body, o.id);
        }
        Ok(())
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
            let name = self.person_name(id)?;
            let buyer_name = self.club(buyer).short_name;
            let (title, body) = if as_loan {
                (
                    format!("Pedido de empréstimo de {name}"),
                    format!("O {buyer_name} quer {name} emprestado até ao fim da época. Responda até {}.", cm_date(&until)),
                )
            } else {
                (
                    format!("Proposta por {name}"),
                    format!(
                        "O {buyer_name} oferece {} por {name} (valor {}). Responda até {}.",
                        money_text(fee),
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
        fresh.push(self.news_item(*next_id, "message", "Contratos a acabar".into(), body));
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
        g.make_offer(buy, ask).unwrap();
        g.make_offer(haggle, (ask2 as f64 * 0.7) as i64 + 10_000).unwrap();
        g.make_offer(lowball, ask3 / 10).unwrap();
        assert!(g.make_offer(buy, ask).is_err(), "uma proposta de cada vez");
        g.advance_day().unwrap();
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
        let reply = g.negotiate(buy, &t.role, 100, 0, t.years).unwrap();
        assert!(!reply.accepted && !reply.over && reply.wage == t.wage && reply.bonus == t.bonus, "{}", reply.text);
        let cash = g.cash_of(PALMEIRAS).unwrap();
        let reply = g.negotiate(buy, &t.role, t.wage, t.bonus, t.years).unwrap();
        assert!(reply.accepted, "{}", reply.text);
        assert_eq!(g.facts(buy).unwrap().unwrap().club, Some(PALMEIRAS));
        assert_eq!(g.cash_of(PALMEIRAS).unwrap(), cash - ask - t.bonus);

        // A contraproposta: paga o que pedem e passa ao contrato
        let offer = g.transfer_view(haggle).unwrap().offer.unwrap();
        assert_eq!(offer.fee, ask2);
        assert!(g.answer_offer(offer.id, "pay", None).unwrap().status == "agreed");

        // Renovação: três recusas e ele não volta a falar durante 30 dias
        let mine = g.world.conn.query_row(
            "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC LIMIT 1",
            [PALMEIRAS],
            |r| r.get::<_, i32>(0),
        ).unwrap();
        let role = g.contract_terms(mine, None).unwrap().role;
        for _ in 0..2 {
            assert!(!g.negotiate(mine, &role, 100, 0, 2).unwrap().over);
        }
        assert!(g.negotiate(mine, &role, 100, 0, 2).unwrap().over);
        assert!(g.negotiate(mine, &role, 10_000_000, 0, 2).is_err());
        assert!(!g.transfer_view(mine).unwrap().can_renew);
        let other = g.world.conn.query_row(
            "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1 AND p.id <> ? AND p.id <> ? ORDER BY pl.current_ability DESC LIMIT 1",
            params![PALMEIRAS, mine, buy],
            |r| r.get::<_, i32>(0),
        ).unwrap();
        // Renovação com 5% a menos de salário, compensados no prémio
        let before = g.facts(other).unwrap().unwrap().expires;
        let t = g.contract_terms(other, None).unwrap();
        assert!(t.renewal && t.roles.iter().filter(|r| !r.allowed).all(|r| r.youth));
        let wage = t.wage * 95 / 100;
        let bonus = t.bonus + (t.wage - wage) * 52 * t.years as i64 + 10_000;
        let reply = g.negotiate(other, &t.role, wage, bonus, t.years).unwrap();
        assert!(reply.accepted, "{}", reply.text);
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
        assert_eq!(g.cash_of(PALMEIRAS).unwrap(), cash + o.fee);
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
