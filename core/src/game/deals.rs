//! Mercado do treinador (marco 3.8, passo 3): propostas por jogadores de outros clubes,
//! empréstimos pedidos, propostas dos clubes da IA pelos jogadores do plantel, negociação
//! do contrato (contratações e renovações) e lista de transferências. Os outros clubes
//! respondem em 1 a 4 dias, com uma mensagem que traz os botões (`news.offer_id`); as
//! condições e as rondas da negociação estão em `negotiation.rs`.
//! Regras e números em docs/roadmap/3.08-mercado.md.

use std::collections::HashMap;

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::contract_extras::ContractExtras;
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
use crate::willingness;
use crate::{Error, Result, date};

/// Dias que uma proposta recebida espera pela resposta do treinador.
const INCOMING_DAYS: i32 = 7;
/// Dias para fechar o contrato depois de o clube aceitar (ou para responder à contraproposta).
pub(super) const AGREED_DAYS: i32 = 14;
/// Venda aceite: dias que o comprador leva a negociar com o jogador (de 2 a 2 + isto) e dias
/// que o treinador tem para confirmar a transferência depois de o jogador aceitar.
const SALE_TALK_DAYS: u32 = 4;
const SALE_CONFIRM_DAYS: i32 = 5;
/// Propostas recusadas antes de o jogador romper as negociações, e dias até voltar a falar.
pub(super) const TRIES: i32 = 3;
pub(super) const COOLDOWN_DAYS: i32 = 30;
/// Probabilidades, num dia de mercado: proposta por cada jogador da lista de transferências,
/// pedido de empréstimo por cada um da lista de empréstimos, e uma proposta por alguém de fora.
const LISTED_BID: f64 = 0.35;
const LOAN_BID: f64 = 0.3;
const UNLISTED_BID: f64 = 0.12;
/// Oferecer a Clubes: até quantos clubes respondem e em quantos dias chegam as respostas.
const SHOP_MAX: usize = 3;
const SHOP_DAYS: (u64, u64) = (2, 5);
/// Uma resposta que espera por outra proposta pelo mesmo jogador desiste ao fim destes dias.
const SHOP_WAIT: i64 = 21;
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

/// Um jogador do treinador que o clube vendedor aceita na troca: o valor de mercado e o que
/// lhe vale na proposta (o que desconta no valor da transferência).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeCandidate {
    pub id: i32,
    #[ts(type = "number")]
    pub value: i64,
    #[ts(type = "number")]
    pub worth: i64,
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
    /// Emprestado pelo clube do treinador a outro: chamá-lo de volta já.
    pub can_recall: bool,
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
    /// As frases do Futuro, como no CM (o lugar no plantel e o que mais o preocupa).
    pub future: Vec<String>,
    /// Possíveis Despesas: os prémios do contrato que o clube pode ter de pagar.
    pub expenses: Vec<String>,
    /// Os Comentários do agente na proposta de transferência (FM08): o que o clube procura, a
    /// cláusula, o prazo e os extras. Vazio para os do treinador e os livres.
    pub comments: Vec<String>,
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
/// A função de quem vai para a lista de transferências (ou o treinador põe de lado): fora
/// das funções do contrato, "Dispensável" no plantel.
pub const NOT_NEEDED: (&str, &str) = ("notneeded", "Este jogador não é necessário para o clube");
/// O degrau de [`NOT_NEEDED`], abaixo do apoio e dos jovens (para a moral ao mudar de função).
const NOT_NEEDED_STEP: i32 = 4;

/// "Definir Função no Plantel": as funções (as do contrato e a de dispensável) e a de agora.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SquadRoleView {
    pub person_id: i32,
    pub name: String,
    /// A função gravada (`None`: pelo lugar no plantel).
    pub current: Option<String>,
    pub roles: Vec<RoleOption>,
    /// A função no clube ("11" Jogador, "12" Jogador/Treinador, "13" Jogador/Preparador), só
    /// para os veteranos (30 anos ou mais) e quem já tem ficha de staff (4.5); vazia nos outros.
    pub jobs: Vec<RoleOption>,
    pub job: Option<String>,
}

/// As funções no clube que um jogador pode ter (a tabela `jobs`).
const CLUB_JOBS: [(i32, &str); 3] = [(11, "Jogador"), (12, "Jogador/Treinador"), (13, "Jogador/Preparador")];
/// A idade a partir da qual um jogador pode acumular uma função de staff.
const PLAYER_STAFF_AGE: i32 = 30;

/// O adjectivo da nação para "o estilo de vida Italiano" (as nações das ligas do jogo).
fn lifestyle(nation: &str) -> Option<&'static str> {
    Some(match crate::nation_names::nation_en(nation) {
        "England" => "Inglês",
        "France" => "Francês",
        "Germany" => "Alemão",
        "Netherlands" => "Holandês",
        "Italy" => "Italiano",
        "Portugal" => "Português",
        "Spain" => "Espanhol",
        "Argentina" => "Argentino",
        "Brazil" => "Brasileiro",
        "Saudi Arabia" => "Saudita",
        _ => return None,
    })
}

/// O que se responde a uma acção sobre um jogador de enchimento (no CM só se vêem os atributos).
pub(super) const GHOST_ONLY: &str = "Jogador de enchimento: não pode ser usado para nada.";
/// Lugar na liga até ao qual um jogador satisfeito se orgulha da posição do clube.
const PROUD_PLACE: i32 = 3;
/// Idades máximas das funções de jovem (acima, a direção veta).
pub(super) const PROSPECT_AGE: i32 = 21;
const YOUNGSTER_AGE: i32 = 23;
/// Prémio de assinatura pedido, em semanas de salário (um livre pede muito mais).
const BONUS_WEEKS: f64 = 10.0;
const FREE_BONUS_WEEKS: f64 = 30.0;
/// Dias antes do fim da janela em que chega o aviso.
const WINDOW_WARNING_DAYS: i32 = 7;

/// Uma função da lista "Definir Função No Clube".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RoleOption {
    pub key: String,
    pub label: String,
    /// Função de jovem (o "+" roxo do CM).
    pub youth: bool,
    /// Vetada pela direção (pela idade do jogador ou pelos valores envolvidos).
    pub allowed: bool,
    /// O texto do veto ("Vetado pela direcção devido à idade do jogador"...).
    pub veto: Option<String>,
    /// O salário semanal máximo que a direção autoriza nesta função (só nas propostas de contrato).
    #[ts(type = "number | null")]
    pub max_wage: Option<i64>,
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
    /// A folha salarial de hoje e o limite da direcção (por semana): quanto sobra para este.
    #[ts(type = "number")]
    pub wage_bill: i64,
    #[ts(type = "number")]
    pub wage_budget: i64,
    /// Prémios e cláusulas: o que ele exige (o "*" do CM), os valores sugeridos para o resto
    /// e os da proposta em curso (a contraproposta, se houver).
    pub asks: ContractExtras,
    pub suggested: ContractExtras,
    pub extras: ContractExtras,
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
    pub(super) caps_bonus: i64,
    pub(super) caps_needed: i32,
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

impl Facts {
    /// Num clube sem contrato nem salário (o jogador amador do banco, como o "Jogador/Treinador"
    /// de um clube pequeno): como no CM, está livre e assina só com uma proposta de contrato.
    pub(super) fn non_contract(&self) -> bool {
        self.club.is_some() && self.expires.is_none() && self.wage <= 0
    }

    /// Livre: sem clube ou sem contrato.
    pub(super) fn is_free(&self) -> bool {
        self.club.is_none() || self.non_contract()
    }
}

const OFFER_COLUMNS: &str = "id, kind, person_id, buyer, seller, fee, asked, status, answer_on, months, sell_on, apps_bonus, rounds, loan_months, wage_share, monthly_fee, option_fee, apps_needed, public, exchange, caps_bonus, caps_needed";

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
        caps_bonus: r.get(20)?,
        caps_needed: r.get(21)?,
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
                "person_id = ?1 AND (buyer = ?2 OR seller = ?2) AND kind <> 'contract' AND status IN ('pending', 'countered', 'agreed', 'raised', 'talks', 'confirm')",
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

    /// O emprestado ao clube do treinador: o clube dono dele, a quem se faz a proposta de compra.
    pub(super) fn loaned_in_from(&self, person: i32, f: &Facts) -> Result<Option<i32>> {
        Ok(self.loan_of(person)?.map(|l| l.0).filter(|&p| f.club == Some(self.club_id) && p != self.club_id))
    }

    pub(super) fn club_rep(&self, club: i32) -> Result<i32> {
        Ok(self.world.conn.query_row("SELECT COALESCE(reputation, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?)
    }

    /// O bónus das continentais de cada clube que as joga nesta época (o maior, se joga mais de uma).
    pub(super) fn continental_bonuses(&self) -> HashMap<i32, i32> {
        let mut out: HashMap<i32, i32> = HashMap::new();
        for m in &self.matches {
            let pull = personality::continental_pull(m.competition_id);
            if pull == 0 {
                continue;
            }
            for c in [m.home_id, m.away_id] {
                let e = out.entry(c).or_default();
                *e = (*e).max(pull);
            }
        }
        out
    }

    /// O peso de um clube para um jogador que pensa em mudar-se: a reputação e as continentais.
    pub(super) fn club_pull(&self, club: i32) -> Result<i32> {
        Ok(self.club_rep(club)? + self.continental_bonuses().get(&club).copied().unwrap_or(0))
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
    /// O valor médio dos 11 melhores (pelo CA) de um clube.
    pub(super) fn core_value(&self, club: i32) -> Result<f64> {
        let ids: Vec<i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC, p.id LIMIT 11",
            )?
            .query_map([club], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut sum = 0i64;
        for &id in &ids {
            sum += self.world.market_value_of(id, &self.date)?.unwrap_or(0);
        }
        Ok(if ids.is_empty() { 0.0 } else { sum as f64 / ids.len() as f64 })
    }

    /// O lugar de um jogador entre os da sua posição no clube, pelo CA (0 = o melhor).
    pub(super) fn role_rank(&self, club: i32, person: i32, role: usize) -> Result<usize> {
        let rows: Vec<(i32, [i32; 4])> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, COALESCE(pl.pos_goalkeeper, 1),
                        MAX(COALESCE(pl.pos_sweeper, 1), COALESCE(pl.pos_defender, 1), COALESCE(pl.pos_wing_back, 1)),
                        MAX(COALESCE(pl.pos_defensive_midfielder, 1), COALESCE(pl.pos_midfielder, 1), COALESCE(pl.pos_attacking_midfielder, 1)),
                        COALESCE(pl.pos_attacker, 1)
                 FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1 AND p.id NOT IN (SELECT person_id FROM ghosts)
                   AND p.id NOT IN (SELECT person_id FROM retired)
                 ORDER BY pl.current_ability DESC, p.id",
            )?
            .query_map([club], |r| Ok((r.get(0)?, [r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?])))?
            .collect::<rusqlite::Result<_>>()?;
        let same: Vec<i32> = rows.into_iter().filter(|r| role_of(r.1[0], r.1[1], r.1[2], r.1[3]) == role).map(|r| r.0).collect();
        Ok(same.iter().position(|&id| id == person).unwrap_or(same.len()))
    }

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

    /// Como o `club` que vende vê o negócio de `person` com o clube do treinador.
    pub(super) fn sale_view(&self, person: i32, f: &Facts, club: i32) -> Result<willingness::Sale> {
        let (listed, requested): (bool, bool) = self
            .world
            .conn
            .query_row("SELECT transfer = 1, COALESCE(requested, 0) = 1 FROM transfer_list WHERE person_id = ?", [person], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?
            .unwrap_or((false, false));
        let (rank, _) = self.squad_rank(club, person, f.role)?;
        Ok(willingness::Sale {
            listed,
            unhappy: requested,
            starter: rank < 11,
            star: f.value as f64 >= super::market::STAR_SHARE * self.core_value(club)?,
            recent: self.recent(f),
            rival: self.are_rivals(club, self.club_id)?,
        })
    }

    /// O jogador aceita ir para o rival do seu clube? Sorteado uma vez por jogador e ano (repetir
    /// a proposta não muda a resposta): o leal quase nunca, quem pediu para sair mais vezes.
    pub(super) fn joins_rival(&self, person: i32, f: &Facts) -> Result<bool> {
        let requested: bool = self
            .world
            .conn
            .query_row("SELECT COALESCE(requested, 0) = 1 FROM transfer_list WHERE person_id = ?", [person], |r| r.get(0))
            .optional()?
            .unwrap_or(false);
        let year: u64 = self.date[0..4].parse().unwrap_or(0);
        Ok(uniform(&mut self.deal_rng(person, 0x21A1 ^ year)) < willingness::rival_move_chance(f.loyalty, requested))
    }

    /// Os clubes da IA que podem fazer proposta por um jogador do treinador: um rival do clube só
    /// se o jogador aceitar ir para o rival (`joins_rival`, como nas compras do treinador).
    fn willing_buyers(&self, person: i32, f: &Facts, fits: Vec<i32>, as_loan: bool) -> Result<Vec<i32>> {
        let mut rival_ok: Option<bool> = None;
        let mut out = Vec::with_capacity(fits.len());
        for c in fits {
            // O jogador aceita? Não desce demais (a não ser para jogar) nem vai emprestado para o
            // banco de um clube menor (4.0, item 20); a cidade pesa
            let refused = if as_loan {
                self.loan_refused(person, f, self.club_id, c)?.is_some()
            } else {
                self.world.club_preference(person, c)? == Some(false)
                    || (self.world.club_preference(person, c)? != Some(true)
                        && personality::move_steps_down(&self.player_move(person, f.ca, f.age, f.ambition, self.club_id, c)?))
            };
            if refused {
                continue;
            }
            if self.are_rivals(c, self.club_id)? {
                let ok = match rival_ok {
                    Some(ok) => ok,
                    None => *rival_ok.insert(self.joins_rival(person, f)?),
                };
                if !ok {
                    continue;
                }
            }
            out.push(c);
        }
        Ok(out)
    }

    /// O jogador recusa ir emprestado do `parent` para o `to`? O motivo, para a mensagem: as
    /// Preferências do CM primeiro (o favorito aceita sempre, o detestado nunca), depois a
    /// descida e os minutos (`personality::loan_refusal`).
    pub(super) fn loan_refused(&self, person: i32, f: &Facts, parent: i32, to: i32) -> Result<Option<String>> {
        let name = self.person_name(person)?;
        match self.world.club_preference(person, to)? {
            Some(true) => return Ok(None),
            Some(false) => return Ok(Some(format!("{name} recusa-se a jogar no {}.", self.club(to).short_name))),
            None => {}
        }
        let m = self.player_move(person, f.ca, f.age, f.ambition, parent, to)?;
        Ok(personality::loan_refusal(&m).map(|r| match r {
            personality::LoanRefusal::StepsDown => {
                format!("{name} não aceita ir emprestado para um clube tão inferior ao {}.", self.club(parent).short_name)
            }
            personality::LoanRefusal::Bench => {
                format!("{name} não quer ir emprestado para o {} para ficar no banco.", self.club(to).short_name)
            }
        }))
    }

    /// `a` e `b` são rivais (`clubs.rival1..3`, num sentido ou no outro).
    pub(super) fn are_rivals(&self, a: i32, b: i32) -> Result<bool> {
        Ok(self.world.conn.query_row(
            "SELECT COUNT(*) FROM clubs WHERE (id = ?1 AND ?2 IN (rival1_id, rival2_id, rival3_id))
                                        OR (id = ?2 AND ?1 IN (rival1_id, rival2_id, rival3_id))",
            params![a, b],
            |r| r.get::<_, i64>(0),
        )? > 0)
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
        let sale = self.sale_view(person, f, club)?;
        if let Some(asking) = listed {
            // Listado: o preço da lista (ao rival, um pouco mais)
            let ask = asking.unwrap_or(f.value) as f64 * if sale.rival { 1.2 } else { 1.0 };
            return Ok(finance::round_money(ask).max(10_000));
        }
        // O titular, o indispensável, o acabado de chegar e o rival custam mais; quem pediu para
        // sair, menos (`willingness`); um leal até mais 20%
        let mut ask = f.value as f64 * willingness::sale_terms(&sale).0 * personality::loyalty_fee(f.loyalty);
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
        let base = wage_demand(f.value.max(f.ca as i64 * 20_000), f.wage, self.wage_wealth(self.club_id)?, f.age);
        let year: u64 = self.date[0..4].parse().unwrap_or(0);
        let mut ask = base as f64 * (0.95 + 0.15 * uniform(&mut self.deal_rng(person, 0xDE3A ^ year)));
        let expected = self.expected_role(person, f)?;
        let diff = SQUAD_ROLES[role].2 - SQUAD_ROLES[expected].2;
        ask *= if diff > 0 { 1.0 + 0.12 * diff as f64 } else { 1.0 + 0.04 * diff as f64 };
        ask *= personality::ambition_wage(f.ambition, expected == 0);
        if renewal {
            ask *= personality::loyalty_renewal(f.loyalty);
        } else {
            // Uma cidade pouco atraente pede mais, sobretudo a quem vem de fora (4.0, item 23)
            ask *= self.city_wage(person, self.club_id)?;
        }
        if !renewal && !free {
            ask *= personality::loyalty_move_wage(f.loyalty);
            if let Some(club) = f.club
                && self.are_rivals(club, self.club_id)?
            {
                ask *= willingness::RIVAL_WAGE;
            }
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

    /// O recorde anterior do clube do treinador (a compra mais cara ou a venda maior no histórico
    /// do jogo, sem o negócio de `person` que acabou de entrar), se `fee` o bate (de €1M para cima).
    pub(super) fn beats_club_record(&self, person: i32, fee: i64, buying: bool) -> Result<Option<i64>> {
        if fee < 1_000_000 {
            return Ok(None);
        }
        let side = if buying { "to_club" } else { "from_club" };
        let before: i64 = self.world.conn.query_row(
            &format!(
                "SELECT COALESCE(MAX(fee), 0) FROM transfers WHERE {side} = ?1 AND kind = 'transfer'
                 AND id <> (SELECT COALESCE(MAX(id), -1) FROM transfers WHERE person_id = ?2)"
            ),
            params![self.club_id, person],
            |r| r.get(0),
        )?;
        Ok((fee > before).then_some(before))
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
        // Emprestado, sai das listas do clube dono
        self.world.conn.execute("DELETE FROM transfer_list WHERE person_id = ?", [person])?;
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
        let ghost = self.is_ghost(person);
        let loan = self.loan_of(person)?;
        let loaned_in = loan.as_ref().is_some_and(|l| l.0 != self.club_id) && f.club == Some(self.club_id);
        let mine = f.club == Some(self.club_id) && !loaned_in;
        // Emprestado pelo clube do treinador: renova-se e chama-se de volta, como no CM
        let loaned_out = loan.as_ref().is_some_and(|l| l.0 == self.club_id) && f.club != Some(self.club_id);
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
        } else if let (Some(b), Some(club)) = (&bored, loan.as_ref().filter(|_| loaned_in).map(|l| l.0).or(f.club)) {
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
            "person_id = ?1 AND kind IN ('transfer', 'loan') AND status IN ('pending', 'countered', 'agreed', 'raised', 'talks', 'confirm')",
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
            expenses: self.expenses(person, mine)?,
            comments: if mine || f.is_free() || loan.is_some() { Vec::new() } else { self.offer_comments(person, &f)? },
            person_id: person,
            mine,
            free: f.is_free(),
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
            // Um emprestado ao treinador compra-se ao clube dono
            can_offer: !ghost
                && !busy
                && bored.is_none()
                && !f.non_contract()
                && (loaned_in || (f.club != Some(self.club_id) && self.loan_of(person)?.is_none())),
            can_loan: !ghost && f.club.is_some() && f.club != Some(self.club_id) && !busy && window && self.loan_of(person)?.is_none(),
            can_renew: (mine || loaned_out) && blocked.is_none() && !busy,
            can_recall: loaned_out && !busy && !self.sacked(),
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
    ) -> Result<(String, Vec<String>)> {
        let Some(club) = f.club else { return Ok(("Livre: sem contrato".into(), Vec::new())) };
        let days_left = f.expires.as_deref().and_then(date::parse).zip(date::parse(&self.date)).map(|(e, t)| e - t);
        let (rank, _) = self.squad_rank(club, person, f.role)?;
        let loan = self.loan_of(person)?.filter(|l| Some(l.0) != f.club);
        let ghost = self.is_ghost(person);
        let availability = match listing {
            _ if ghost => "Jogador de enchimento: não está no mercado".into(),
            _ if loan.is_some() => format!("Emprestado pelo {}", self.club(loan.as_ref().map_or(0, |l| l.0)).short_name),
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
        let mut future = vec![match role.as_deref().and_then(|k| SQUAD_ROLES.iter().find(|r| r.0 == k)) {
            Some(r) => r.1.to_string(),
            None if role.as_deref() == Some(NOT_NEEDED.0) => NOT_NEEDED.1.to_string(),
            None => match rank {
                0..3 => "Pensa que é um membro essencial ao plantel".into(),
                3..11 => "Pensa que é importante para a equipa principal".into(),
                11..16 => "Pensa que faz parte da rotação do plantel".into(),
                _ if f.age <= PROSPECT_AGE => "Pensa que tem uma boa perspectiva futura no clube".into(),
                _ => "Pensa que é um apoio à primeira equipa".into(),
            },
        }];
        // O que mais lhe passa pela cabeça, pela personalidade e pelo lugar no plantel
        let (adaptability, ambition, abroad): (Option<i32>, Option<i32>, bool) = self.world.conn.query_row(
            "SELECT p.adaptability, p.ambition, COALESCE(p.nation_id <> c.nation_id, 0)
             FROM people p JOIN clubs c ON c.id = p.club_id WHERE p.id = ?",
            [person],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let outside = rank >= 11;
        // Nas reservas do treinador: os jovens estão contentes; aos 24, querem saber o futuro
        let reserve = mine && self.squads.state(person).reserve;
        let young_reserve = mine && self.content_in_reserves(person, f.age)?;
        let absence = if mine { self.absence(person)? } else { None };
        match absence.as_ref().map(|a| a.0.as_str()) {
            Some("awol") => future.push("Ausente sem permissão".into()),
            Some(_) => future.push(format!("De folga no seu país até {}", super::cm_date(&absence.as_ref().unwrap().1))),
            None => {}
        }
        if outside && ambition.unwrap_or(10) >= 15 && !young_reserve {
            future.push("Espera impressionar os olheiros de outros clubes".into());
        }
        let homesick = mine && self.unhappy_reasons(person)?.iter().any(|r| r == super::unhappy::reason_text("homesick"));
        if homesick || (!mine && abroad && adaptability.unwrap_or(10) <= 8) {
            future.push("Tem saudades do seu país".into());
            // "... ao estilo de vida Italiano", pela nação do clube (as das ligas do jogo)
            let nation: Option<String> = self
                .world
                .conn
                .query_row("SELECT n.name FROM clubs c JOIN nations n ON n.id = c.nation_id WHERE c.id = ?", [club], |r| r.get(0))
                .optional()?;
            if let Some(adjective) = nation.as_deref().and_then(lifestyle) {
                future.push(format!("Está a ter dificuldades em adaptar-se ao estilo de vida {adjective}"));
            }
        }
        if young_reserve {
            future.push("Entende que o clube tem um bom ambiente no vestiário".into());
            future.push("Contente com situação no clube".into());
        } else if reserve && !self.on_any_list(person)? {
            // Na lista (de transferências ou de empréstimos) o futuro já está esclarecido
            future.push("Deseja ver o seu futuro esclarecido".into());
        } else if outside && f.age <= 23 {
            future.push("Está a tentar conquistar um lugar na equipa principal".into());
        }
        let requested: bool = self
            .world
            .conn
            .query_row("SELECT requested FROM transfer_list WHERE person_id = ?", [person], |r| r.get(0))
            .optional()?
            .unwrap_or(false);
        if requested {
            future.push("Pediu para ser transferido".into());
        }
        // Quem está bem no clube diz porquê, como no CM, no lugar do papel no plantel
        let unhappy = mine && !self.unhappy_reasons(person)?.is_empty();
        let listed = listing.is_some_and(|l| l.0 || l.1);
        if future.len() == 1 && !unhappy && !listed && loan.is_none() && !ghost {
            let content = self.content_lines(person, club, rank, f)?;
            if !content.is_empty() {
                future = content;
            }
        }
        Ok((availability, future))
    }

    /// As frases do Futuro de um jogador satisfeito (até três): o ambiente, o orgulho no clube,
    /// a forma, o colega que admira, o contrato longo e a situação no clube.
    fn content_lines(&self, person: i32, club: i32, rank: usize, f: &Facts) -> Result<Vec<String>> {
        let today = date::parse(&self.date).unwrap_or(0);
        let years_here = f.joined.as_deref().and_then(date::parse).map_or(0, |j| (today - j) / 365);
        let years_left = f.expires.as_deref().and_then(date::parse).map_or(0, |e| (e - today) / 365);
        let morale = self.squads.states.get(&person).map(|s| s.morale);
        let form = self.form_of(person)?;
        let form = (!form.is_empty()).then(|| form.iter().sum::<f64>() / form.len() as f64);
        let mut out: Vec<String> = Vec::new();
        if morale.is_none_or(|m| m >= 12) {
            out.push("Entende que o clube tem um bom ambiente no vestiário".into());
        }
        if f.loyalty >= 14.0 || years_here >= 4 {
            out.push(format!("Está orgulhoso por jogar {}", self.club(club).short_name));
        }
        if form.is_some_and(|v| v >= 7.0) {
            out.push("Está a ter prazer em jogar no clube".into());
        }
        // O clube nos três primeiros da liga (com quatro jogos ou mais)
        let division: Option<i32> = self.world.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [club], |r| r.get(0))?;
        if let Some(div) = division
            && self.table(div).iter().any(|r| r.club.id == club && r.played >= 4 && r.position <= PROUD_PLACE)
        {
            out.push("Orgulhoso da posição do clube na liga".into());
        }
        // O melhor do plantel, para quem não está entre os três primeiros
        if rank >= 3 && f.age <= 25 {
            let best: Option<i32> = self
                .world
                .conn
                .query_row(
                    "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id
                     WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC, p.id LIMIT 1",
                    [club],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(b) = best.filter(|b| *b != person) {
                out.push(format!("Pensa que {} é um membro essencial ao elenco", self.person_name(b)?));
            }
        }
        if years_left >= 3 {
            out.push("Feliz pelo compromisso de futuro com o clube".into());
        }
        // Leal e de moral alta, com o treinador no clube há uma época ou mais
        if f.loyalty >= 12.0 && morale.is_some_and(|m| m >= 15) {
            let coach: Option<(i32, Option<String>)> = self
                .world
                .conn
                .query_row(
                    "SELECT id, club_joined FROM people WHERE club_id = ? AND club_job = ? ORDER BY id LIMIT 1",
                    params![club, super::HEAD_COACH],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let Some((coach, joined)) = coach
                && joined.as_deref().and_then(date::parse).is_some_and(|j| today - j >= 365)
            {
                out.push(format!("Espera que {} se mantenha no clube", self.person_name(coach)?));
            }
        }
        if rank < 16 || f.age <= 21 {
            out.push("Contente com situação no clube".into());
        }
        out.truncate(3);
        Ok(out)
    }

    /// Possíveis Despesas de um jogador do plantel do treinador: os prémios do contrato.
    fn expenses(&self, person: i32, mine: bool) -> Result<Vec<String>> {
        let Some(e) = self.contract_extras(person)?.filter(|_| mine) else { return Ok(Vec::new()) };
        let mut out = Vec::new();
        if e.goal_bonus > 0 {
            out.push(format!("{} por golo marcado", money_text(e.goal_bonus)));
        }
        if e.assist_bonus > 0 {
            out.push(format!("{} por assistência", money_text(e.assist_bonus)));
        }
        if e.clean_sheet_bonus > 0 {
            out.push(format!("{} por jogo sem sofrer golos", money_text(e.clean_sheet_bonus)));
        }
        Ok(out)
    }

    /// O veto da direção a uma função: pela idade (as de jovem) ou pelos valores envolvidos
    /// (o salário que ele pediria passa o que a direção autoriza).
    pub(super) fn role_veto(&self, person: i32, f: &Facts, role: usize, renewal: bool, free: bool) -> Result<Option<String>> {
        if !Self::role_allowed(role, f.age) {
            return Ok(Some("Vetado pela direcção devido à idade do jogador".into()));
        }
        if self.demand(person, f, role, renewal, free)?.wage > self.role_max_wage(role)? {
            return Ok(Some("Vetado pela direcção devido aos valores envolvidos".into()));
        }
        Ok(None)
    }

    /// A mensagem da proposta ganha uma linha com o que o treinador fez, como no CM ("Juventus
    /// fez uma proposta de contrato.").
    pub(super) fn note_action(&mut self, offer: i32, what: &str) {
        let club = self.club(self.club_id).short_name;
        if let Some(n) = self.news.iter_mut().filter(|n| n.offer_id == Some(offer)).max_by_key(|n| n.id) {
            n.body = format!("{}\n\n{club} {what}.", n.body.trim_end());
            self.unsaved = true;
        }
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
                    vec![action("confirm", "Confirmar"), action("withdraw", "Desistir")],
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
                vec![action("negotiate", "Propôr Contrato"), action("withdraw", "Desistir")],
            ),
            (false, "agreed") => (
                format!("O {seller_name} aceitou {terms} por {name}. Falta o contrato com o jogador (até {until})."),
                vec![action("negotiate", "Propôr Contrato"), action("withdraw", "Desistir")],
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
            (true, "talks") => (
                format!(
                    "Aceitou {terms} do {} por {name}. O {} negoceia o contrato com o jogador; a resposta chega a {until}.",
                    buyer.short_name, buyer.short_name
                ),
                Vec::new(),
            ),
            (true, "confirm") => (
                format!("Vender {name} ao {} por {terms}? Confirme até {until}.", buyer.short_name),
                vec![action("confirm", "Confirmar"), action("reject", "Cancelar Negócio")],
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

    /// "Adicionar Jogador à Proposta": os jogadores do treinador que o clube do `person`
    /// aceitaria numa troca (vazio = "não tem interesse num negócio que envolva troca").
    pub fn exchange_candidates(&self, person: i32) -> Result<Vec<ExchangeCandidate>> {
        let Some(f) = self.facts(person)? else { return Ok(Vec::new()) };
        let Some(seller) = self.loaned_in_from(person, &f)?.or(f.club.filter(|&c| c != self.club_id)) else {
            return Ok(Vec::new());
        };
        let ids: Vec<i32> = self
            .world
            .conn
            .prepare(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1 AND p.id NOT IN (SELECT person_id FROM ghosts)
                   AND p.id NOT IN (SELECT person_id FROM retired)
                 ORDER BY pl.current_ability DESC, p.id",
            )?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for id in ids {
            if self.loan_of(id)?.is_some() {
                continue;
            }
            if let Some(f) = self.facts(id)?
                && self.wants_exchange(seller, &f)?
            {
                // O que o clube lhe dá na troca (como em `exchange_worth`)
                let worth = (f.value as f64 * EXCHANGE_WORTH).round() as i64;
                out.push(ExchangeCandidate { id, value: f.value, worth });
            }
        }
        Ok(out)
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
        // Emprestado ao treinador: a proposta vai para o clube dono
        let loaned_in = self.loaned_in_from(person, &f)?;
        if f.club == Some(self.club_id) && loaned_in.is_none() {
            return Err(Error::Invalid("Já é do seu plantel.".into()));
        }
        if f.non_contract() && loaned_in.is_none() {
            return Err(Error::Invalid("Não tem contrato com o clube: proponha-lhe um contrato.".into()));
        }
        if loaned_in.is_none()
            && let Some((parent, _)) = self.loan_of(person)?
        {
            return Err(Error::Invalid(format!("Está emprestado pelo {}: não pode ser vendido agora.", self.club(parent).short_name)));
        }
        let countered = self.countered(person, "transfer")?;
        if countered.is_none() && self.open_offer(person)?.is_some() {
            return Err(Error::Invalid("Já há uma proposta em curso por este jogador.".into()));
        }
        let created = self.date.clone();
        match loaned_in.or(f.club) {
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
                        self.note_action(c.id, "fez uma nova proposta");
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
                // O comprador vai negociar o contrato com o jogador; a confirmação vem depois
                self.start_sale_talks(&o)?;
            }
            (true, "confirm", "confirm") => {
                if !self.window_open_for(o.buyer)? || self.facts(o.person)?.is_none_or(|f| f.club != Some(self.club_id)) {
                    self.set_status(id, "collapsed", None)?;
                    return Err(Error::Invalid("O negócio já não se pode fazer.".into()));
                }
                let (title, body) = self.complete_sale(&o, o.fee)?;
                self.post_news(title, body, Some(id));
            }
            (true, "confirm", "reject") => {
                self.set_status(id, "rejected", None)?;
                if let Some(text) = self.letdown(&o)? {
                    let name = self.person_name(o.person)?;
                    self.post_news(format!("{name} desiludido"), text, Some(id));
                }
            }
            (true, "pending" | "countered", "reject" | "counter") if self.clause_paid(&o)? => {
                return Err(Error::Invalid("A cláusula de venda foi paga: o clube não pode recusar.".into()));
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
        let what = match (incoming, key) {
            (_, "withdraw") => "desistiu do negócio",
            (true, "accept") => "aceitou a proposta",
            (true, "reject") => "recusou a proposta",
            (true, "confirm") => "confirmou a transferência",
            (false, _) => "aceitou as condições",
            _ => "",
        };
        if !what.is_empty() {
            self.note_action(id, what);
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
        Ok(Some(format!("{name} queria muito jogar no {} e ficou desiludido com a recusa.", self.club(o.buyer).short_name)))
    }

    /// Uma semana antes do fim da janela do país do treinador: o aviso, com a data do último dia.
    fn window_closing_news(&mut self, last: &str, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let nation: String = self.world.conn.query_row(
            "SELECT COALESCE(n.name, '') FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id WHERE c.id = ?",
            [self.club_id],
            |r| r.get(0),
        )?;
        let body = format!(
            "A janela de transferências para os clubes de {nation} fecha dentro de uma semana: o último dia para fechar negócios é {}.",
            cm_date(last)
        );
        self.offer_news_plain(fresh, next_id, "Uma semana para o fecho do mercado".into(), body);
        Ok(())
    }

    /// "Prazo limite para transferências" e a "Actualização da confiança da Direcção".
    pub(super) fn window_closed_news(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let nation: String = self.world.conn.query_row(
            "SELECT COALESCE(n.name, '') FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id WHERE c.id = ?",
            [self.club_id],
            |r| r.get(0),
        )?;
        let body = format!("O prazo de transferências para os clubes de {nation} esgotou-se.");
        self.offer_news_plain(fresh, next_id, "Prazo limite para transferências".into(), body);
        let b = self.board_confidence()?;
        let body = b.lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n\n");
        self.offer_news_plain(fresh, next_id, "Actualização da confiança da Direcção".into(), body);
        Ok(())
    }

    fn offer_news_plain(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32, title: String, body: String) {
        fresh.push(NewsItem {
            id: *next_id,
            date: self.date.clone(),
            kind: "message".into(),
            title,
            body,
            read: false,
            offer_id: None,
            link: None,
            awaiting: false,
            part: self.part,
        });
        *next_id += 1;
    }

    /// A proposta paga a cláusula de venda do contrato do jogador.
    fn clause_paid(&self, o: &Offer) -> Result<bool> {
        Ok(o.kind == "transfer" && self.contract_extras(o.person)?.and_then(|e| e.min_fee).is_some_and(|fee| o.fee >= fee))
    }

    /// Venda aceite pelo treinador: o comprador negoceia com o jogador uns dias. Devolve a data
    /// da resposta.
    pub(super) fn start_sale_talks(&mut self, o: &Offer) -> Result<String> {
        let days = 2 + (self.deal_rng(o.person, 0x7A1C).next_u32() % (SALE_TALK_DAYS + 1)) as i32;
        let until = date::add_days(&self.date, days).unwrap_or_default();
        self.set_status(o.id, "talks", Some(&until))?;
        Ok(until)
    }

    /// As conversas do comprador com o jogador acabam: ele aceita (falta a confirmação do
    /// treinador) ou recusa; e as confirmações que ficaram sem resposta caem.
    fn sale_talks_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let today = self.date.clone();
        for o in self.offers_where("seller = ?1 AND status = 'talks' AND answer_on <= ?2", params![self.club_id, today])? {
            let name = self.person_name(o.person)?;
            let buyer = self.club(o.buyer).short_name;
            if !self.window_open_for(o.buyer)? || self.facts(o.person)?.is_none_or(|f| f.club != Some(self.club_id)) {
                self.set_status(o.id, "collapsed", None)?;
                let body = format!("A transferência de {name} para o {buyer} já não se pode fazer.");
                self.offer_news(fresh, next_id, format!("Negócio por {name} caiu"), body, o.id);
                continue;
            }
            // Um ambicioso não desce para um clube mais pequeno (quase nunca)
            let ambition: i32 =
                self.world.conn.query_row("SELECT COALESCE(ambition, 10) FROM people WHERE id = ?", [o.person], |r| r.get(0))?;
            let smaller = self.club_pull(o.buyer)? < self.club_pull(self.club_id)?;
            let refuses = smaller && ambition >= personality::AMBITIOUS && self.deal_rng(o.person, 0x5E11).next_u32() % 10 < 7;
            if refuses {
                self.set_status(o.id, "collapsed", None)?;
                let body = format!(
                    "{name} recusou a proposta do {buyer}: não quer trocar de clube para um de menor dimensão, que vê como um passo atrás na carreira. O negócio caiu."
                );
                self.offer_news(fresh, next_id, format!("{name} rejeita o {buyer}"), body, o.id);
                continue;
            }
            if self.clause_paid(&o)? {
                let (title, body) = self.complete_sale(&o, o.fee)?;
                self.offer_news(fresh, next_id, title, body, o.id);
                continue;
            }
            let until = date::add_days(&today, SALE_CONFIRM_DAYS).unwrap_or_default();
            self.set_status(o.id, "confirm", Some(&until))?;
            // Como no CM: o comprador "prepara-se para comprar" e falta a confirmação do treinador
            let body = format!(
                "O negócio da transferência no valor de {} que corresponde à mudança de {name} {} para {buyer} pode agora ser concretizado.

Confirme até {}.",
                self.terms_text(&o.deal_terms())?,
                self.club(self.club_id).short_name,
                cm_date(&until)
            );
            self.offer_news(fresh, next_id, format!("{buyer} prepara-se para comprar {name}"), body, o.id);
        }
        for o in self.offers_where("seller = ?1 AND status = 'confirm' AND answer_on < ?2", params![self.club_id, today])? {
            self.set_status(o.id, "collapsed", None)?;
            let name = self.person_name(o.person)?;
            let buyer = self.club(o.buyer).short_name;
            let body = format!("Sem a confirmação do negócio a tempo, o {buyer} desistiu de {name}.");
            self.offer_news(fresh, next_id, format!("Negócio por {name} caiu"), body, o.id);
        }
        Ok(())
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
            let wage = wage_demand(f.value.max(f.ca as i64 * 20_000), f.wage, self.wage_wealth(o.buyer)?, f.age);
            let years = contract_years(f.age, &mut self.deal_rng(o.person, 0x5A1E));
            let kind = self.kind_of(o.buyer)?;
            // Um dos valiosos: a direção e os adeptos não gostam (antes de ele sair)
            let idol = self.idol_sold(o.person)?;
            let terms = DealTerms { fee, ..o.deal_terms() };
            self.execute_move(o.person, Some(self.club_id), Some(o.buyer), fee, wage, years, "transfer", kind)?;
            self.settle_terms(o.person, o.buyer, self.club_id, &terms)?;
            self.world.conn.execute("UPDATE offers SET fee = ? WHERE id = ?", params![fee, o.id])?;
            self.sale_to_budget(terms.budget_cost() - terms.bonuses())?;
            let mut body = format!("{name} foi vendido ao {buyer} por {}.", terms.text());
            // A maior venda da história do clube (no jogo)
            if let Some(before) = self.beats_club_record(o.person, fee, false)? {
                body.push_str(&format!(
                    "

É a maior venda de sempre do clube{}.",
                    record_before(before)
                ));
            }
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
        // Emprestado pelo clube a outro: continua a ser do plantel para o contrato
        let loaned_out = loan.as_ref().is_some_and(|l| l.0 == self.club_id) && f.club != Some(self.club_id);
        let mine = (f.club == Some(self.club_id) && !loaned_in) || loaned_out;
        let deal = self.open_offer(person)?.filter(|o| o.buyer == self.club_id && o.status == "agreed");
        let days_left = f.expires.as_deref().and_then(date::parse).zip(date::parse(&self.date)).map_or(i64::MAX, |(e, t)| (e - t) as i64);
        let party = if mine {
            Party::Renewal
        } else if let Some(d) = deal {
            if d.seller.is_some() { Party::Transfer(d) } else { Party::Free(Some(d)) }
        } else if f.is_free() {
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
        let (asks, suggested) = self.extras_demand(person, &f, d.wage)?;
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
                .map(|(i, r)| {
                    let veto = self.role_veto(person, &f, i, renewal, free)?;
                    let max_wage = Some(self.role_max_wage(i)?);
                    Ok(RoleOption { key: r.0.into(), label: r.1.into(), youth: i >= 4, allowed: veto.is_none(), veto, max_wage })
                })
                .collect::<Result<_>>()?,
            role: SQUAD_ROLES[chosen].0.into(),
            expected_role: SQUAD_ROLES[expected].0.into(),
            wage: d.wage,
            bonus: d.bonus,
            years: d.years,
            ends,
            fee,
            cash: self.cash_of(self.club_id)?,
            budget: self.transfer_budget()?,
            max_wage: self.role_max_wage(chosen)?,
            wage_bill: self.board_view()?.wage_bill,
            wage_budget: self.board_view()?.wage_budget,
            asks,
            suggested,
            extras: if self.open_contract(person)?.is_some() { self.talk_extras(person)? } else { ContractExtras::default() },
        })
    }

    // ---------------------------------------------------------------- lista de transferências

    /// Põe ou tira um jogador do plantel das listas de transferências e de empréstimos.
    /// Na lista de transferências ou na de empréstimos.
    pub(super) fn on_any_list(&self, person: i32) -> Result<bool> {
        Ok(self
            .world
            .conn
            .query_row("SELECT 1 FROM transfer_list WHERE person_id = ? AND (transfer = 1 OR loan = 1)", [person], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// Um jogador do plantel do treinador (não os emprestados por outros nem os de enchimento).
    fn own_squad_player(&self, person: i32) -> Result<Facts> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só os jogadores têm função no plantel.".into()))?;
        if f.club != Some(self.club_id) || self.loaned_in_from(person, &f)?.is_some() {
            return Err(Error::Invalid("Só os jogadores do seu plantel.".into()));
        }
        if self.is_ghost(person) {
            return Err(Error::Invalid(GHOST_ONLY.into()));
        }
        Ok(f)
    }

    /// O degrau de uma função (0 = indispensável); sem função gravada, a que ele espera.
    fn role_step(&self, person: i32, f: &Facts, key: Option<&str>) -> Result<i32> {
        Ok(match key {
            Some(k) if k == NOT_NEEDED.0 => NOT_NEEDED_STEP,
            Some(k) => SQUAD_ROLES[Self::role_index(k)?].2,
            None => SQUAD_ROLES[self.expected_role(person, f)?].2,
        })
    }

    fn stored_role(&self, person: i32) -> Result<Option<String>> {
        Ok(self.world.conn.query_row("SELECT role FROM squad_roles WHERE person_id = ?", [person], |r| r.get(0)).optional()?)
    }

    /// "Definir Função no Plantel" (o "Set Squad Status" do FM): as funções que pode ter.
    pub fn squad_role_view(&self, person: i32) -> Result<SquadRoleView> {
        let f = self.own_squad_player(person)?;
        let mut roles: Vec<RoleOption> = SQUAD_ROLES
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let allowed = Self::role_allowed(i, f.age);
                RoleOption {
                    key: r.0.into(),
                    label: r.1.into(),
                    youth: i >= 4,
                    allowed,
                    veto: (!allowed).then(|| "Não é possível pela idade do jogador".into()),
                    max_wage: None,
                }
            })
            .collect();
        roles.push(RoleOption {
            key: NOT_NEEDED.0.into(),
            label: NOT_NEEDED.1.into(),
            youth: false,
            allowed: true,
            veto: None,
            max_wage: None,
        });
        let jobs = if self.can_join_staff(person, f.age)? {
            CLUB_JOBS
                .iter()
                .map(|&(id, label)| RoleOption {
                    key: id.to_string(),
                    label: label.into(),
                    youth: false,
                    allowed: true,
                    veto: None,
                    max_wage: None,
                })
                .collect()
        } else {
            Vec::new()
        };
        let job: Option<i32> = self.world.conn.query_row("SELECT club_job FROM people WHERE id = ?", [person], |r| r.get(0))?;
        Ok(SquadRoleView {
            person_id: person,
            name: self.person_name(person)?,
            current: self.stored_role(person)?,
            roles,
            jobs,
            job: job.map(|j| j.to_string()),
        })
    }

    /// Um jogador pode ser também staff: 30 anos ou mais, ou já com ficha de staff.
    fn can_join_staff(&self, person: i32, age: i32) -> Result<bool> {
        let staff: bool = self.world.conn.query_row("SELECT COUNT(*) > 0 FROM non_players WHERE person_id = ?", [person], |r| r.get(0))?;
        Ok(age >= PLAYER_STAFF_AGE || staff)
    }

    /// "Definir Função No Clube": Jogador, Jogador/Treinador (treina como um adjunto) ou
    /// Jogador/Preparador. A parte de staff conta nos treinos e nos relatórios.
    fn set_club_job(&mut self, person: i32, job: &str) -> Result<String> {
        let f = self.own_squad_player(person)?;
        let &(id, label) =
            CLUB_JOBS.iter().find(|(id, _)| id.to_string() == job).ok_or_else(|| Error::Invalid("Função desconhecida.".into()))?;
        if id != super::regen::PLAYER_JOB && !self.can_join_staff(person, f.age)? {
            return Err(Error::Invalid(format!("Só os jogadores com {PLAYER_STAFF_AGE} anos ou mais podem acumular uma função de staff.")));
        }
        if id != super::regen::PLAYER_JOB {
            self.make_staff(person)?;
        }
        self.world.conn.execute("UPDATE people SET club_job = ? WHERE id = ?", params![id, person])?;
        self.unsaved = true;
        Ok(format!("{label}: {}.", self.person_name(person)?))
    }

    /// Muda a função no plantel sem mexer no contrato: subir agrada-lhe (+1 de moral), descer
    /// custa 2 por degrau. Devolve a frase para o perfil.
    pub fn set_squad_role(&mut self, person: i32, role: &str) -> Result<String> {
        if let Some(job) = role.strip_prefix("job:") {
            return self.set_club_job(person, job);
        }
        let f = self.own_squad_player(person)?;
        let label = if role == NOT_NEEDED.0 {
            NOT_NEEDED.1
        } else {
            let i = Self::role_index(role)?;
            if !Self::role_allowed(i, f.age) {
                return Err(Error::Invalid("Essa função não é possível pela idade do jogador.".into()));
            }
            SQUAD_ROLES[i].1
        };
        let before = self.stored_role(person)?;
        let (old, new) = (self.role_step(person, &f, before.as_deref())?, self.role_step(person, &f, Some(role))?);
        self.world.conn.execute("INSERT OR REPLACE INTO squad_roles (person_id, role) VALUES (?, ?)", params![person, role])?;
        let name = self.person_name(person)?;
        let mood = match new.cmp(&old) {
            std::cmp::Ordering::Less => {
                self.change_morale(person, 1);
                " Ficou satisfeito."
            }
            std::cmp::Ordering::Greater => {
                self.change_morale(person, -2 * (new - old));
                " Não gostou de perder importância."
            }
            std::cmp::Ordering::Equal => "",
        };
        self.unsaved = true;
        Ok(format!("{label}: {name}.{mood}"))
    }

    pub fn set_listing(&mut self, person: i32, transfer: bool, loan: bool, asking: Option<i64>) -> Result<TransferView> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só os jogadores vão para a lista.".into()))?;
        if f.club != Some(self.club_id) || self.loan_of(person)?.is_some() {
            return Err(Error::Invalid("Só os jogadores do seu plantel.".into()));
        }
        if self.is_ghost(person) {
            return Err(Error::Invalid(GHOST_ONLY.into()));
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
        // Na lista de transferências deixa de ser necessário (como no CM); tirado, volta à função
        // pelo lugar no plantel. Numa das listas, o futuro fica esclarecido
        if transfer {
            self.world.conn.execute("INSERT OR REPLACE INTO squad_roles (person_id, role) VALUES (?, ?)", params![person, NOT_NEEDED.0])?;
        } else if was_listed {
            self.world.conn.execute("DELETE FROM squad_roles WHERE person_id = ? AND role = ?", params![person, NOT_NEEDED.0])?;
        }
        if transfer || loan {
            self.remove_unhappy(person, "future")?;
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
            let mut body = format!("Sem resposta à proposta, o {} desistiu de {name}.", self.club(o.buyer).short_name);
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
        self.release_clause_offers(fresh, next_id)?;
        self.sale_talks_day(fresh, next_id)?;
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
        self.shop_replies(fresh, next_id)?;
        // A janela do país do treinador fechou ontem: o prazo e a confiança da direção (como no CM)
        if !self.sacked() {
            let kind = self.kind_of(self.club_id)?;
            let yesterday = date::add_days(&today, -1).unwrap_or_default();
            if window_open(kind, &yesterday[5..]) && !window_open(kind, &today[5..]) {
                self.window_closed_news(fresh, next_id)?;
            }
            // Uma semana antes de fechar, o aviso (o último dia aberto é daqui a 7 dias)
            let in_week = date::add_days(&today, WINDOW_WARNING_DAYS).unwrap_or_default();
            let after = date::add_days(&in_week, 1).unwrap_or_default();
            if window_open(kind, &in_week[5..]) && !window_open(kind, &after[5..]) {
                self.window_closing_news(&in_week, fresh, next_id)?;
            }
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
        // O dono: o clube dele ou, emprestado ao treinador, o clube que o emprestou
        let loaned_to_buyer = self.loan_of(o.person)?.is_some_and(|l| l.0 == seller && f.as_ref().is_some_and(|f| f.club == Some(o.buyer)));
        let Some(f) = f.filter(|f| f.club == Some(seller) || loaned_to_buyer) else {
            self.set_status(o.id, "collapsed", None)?;
            let body = format!("{name} já não é jogador do {seller_name}.");
            self.offer_news(fresh, next_id, format!("Proposta por {name}"), body, o.id);
            return Ok(());
        };
        let (rank, count) = self.squad_rank(seller, o.person, f.role)?;
        let on_loan = self.loan_of(o.person)?.is_some() && !loaned_to_buyer;
        let deadline = date::add_days(&self.date, AGREED_DAYS).unwrap_or_default();
        let asking = self.asking_price(o.person, &f, seller)?;
        if o.kind == "loan" {
            let t = o.loan_terms();
            // Só empresta quem está fora dos planos (depois dos melhores da posição) e não é uma estrela
            let in_plans = self.role_rank(seller, o.person, f.role)? < MIN_ROLE[f.role];
            let star = f.value as f64 >= super::market::STAR_SHARE * self.core_value(seller)?;
            let important = in_plans || star;
            let reply = if on_loan || important || self.recent(&f) || !self.window_open_for(self.club_id)? {
                LoanReply::Reject
            } else {
                // Fora dos planos, o lugar no plantel já não o segura
                loan_reply(&t, rank.max(super::negotiation::LOAN_KEEP), f.age, f.value, asking, o.rounds)
            };
            // O clube deixa: falta o jogador querer (4.0, item 20)
            if !matches!(reply, LoanReply::Reject)
                && let Some(why) = self.loan_refused(o.person, &f, seller, self.club_id)?
            {
                self.set_status(o.id, "rejected", None)?;
                let body = format!("O {seller_name} estava disposto a emprestar {name}, mas o jogador recusou. {why}");
                self.offer_news(fresh, next_id, format!("{name} recusa o empréstimo"), body, o.id);
                return Ok(());
            }
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
                    let body = if rank < 14 || important {
                        format!("O {seller_name} não quer emprestar {name}: é importante para a equipa.")
                    } else {
                        format!("O {seller_name} recusou o pedido de empréstimo de {name}.")
                    };
                    self.offer_news(fresh, next_id, format!("Empréstimo de {name} recusado"), body, o.id);
                }
            }
            return Ok(());
        }
        // O emprestado ao comprador já não conta no plantel do dono
        if on_loan || (!loaned_to_buyer && count <= MIN_ROLE[f.role]) {
            self.set_status(o.id, "rejected", None)?;
            let body = format!("O {seller_name} recusou a proposta por {name}: não está à venda.");
            self.offer_news(fresh, next_id, format!("Proposta por {name} recusada"), body, o.id);
            return Ok(());
        }
        let t = o.deal_terms();
        // Já acordado com um clube da IA: só cede a uma proposta melhor do que a do acordo
        if let Some((other, agreed)) = self.ai_agreement(o.person)?
            && t.fee <= agreed
        {
            self.set_status(o.id, "rejected", None)?;
            let body = format!(
                "O {seller_name} rejeitou a proposta de {} por {name}: já tem um acordo com o {} por {}.",
                super::market::money_text(t.fee),
                self.club(other).short_name,
                super::market::money_text(agreed)
            );
            self.offer_news(fresh, next_id, format!("Proposta de transferência de {name} rejeitada"), body, o.id);
            return Ok(());
        }
        // O clube resiste a vender um indispensável, um acabado de chegar ou a um rival: recusa
        // muitas vezes, menos quanto mais a proposta passa do pedido (`willingness`)
        let sale = self.sale_view(o.person, &f, seller)?;
        if let Some(why) = willingness::holdout(&sale) {
            let base = willingness::sale_terms(&sale).1;
            let chance = willingness::sale_chance_for(base, t.fee as f64, asking as f64);
            if uniform(&mut self.deal_rng(o.person, 0x401D ^ o.rounds as u64)) >= chance {
                self.set_status(o.id, "rejected", None)?;
                let body =
                    format!("O {seller_name} rejeitou a proposta de {} por {name}. {}", super::market::money_text(t.fee), why.text());
                self.offer_news(fresh, next_id, format!("Proposta de transferência de {name} rejeitada"), body, o.id);
                return Ok(());
            }
        }
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
                let body =
                    format!("O {seller_name} recusou a sua proposta por {name} ({}): fica muito aquém do que o clube pretende.", t.text());
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
        let budget = self.ai_budget(o.buyer, self.cash_of(o.buyer)?)?;
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
                let until = self.start_sale_talks(o)?;
                let body = format!(
                    "O {buyer} aceitou {} por {name} e vai negociar o contrato com o jogador; a resposta chega a {}.",
                    asked.text(),
                    cm_date(&until)
                );
                self.offer_news(fresh, next_id, format!("{buyer} aceita as condições por {name}"), body, o.id);
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
                let body = format!("O {buyer} achou {} demasiado por {name} e desistiu do negócio.", asked.text());
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
        self.note_action(id, "pediu mais");
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
        let buyers = self.bid_clubs()?;
        let my_rep = self.club_pull(self.club_id)?;
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
            let fits = self.willing_buyers(id, &f, fitting_buyers(&buyers, ca, as_loan, fee, my_rep), as_loan)?;
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
            let (deal, loan) = bid_terms(&f, as_loan, fee, &mut rng);
            self.set_terms(offer, &deal, &loan)?;
            self.bid_news(fresh, next_id, offer, id, buyer, &f, &deal, &loan, &until)?;
        }
        Ok(())
    }

    /// A notícia de uma proposta da IA por um jogador do treinador.
    #[allow(clippy::too_many_arguments)]
    fn bid_news(
        &mut self,
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
        offer: i32,
        id: i32,
        buyer: i32,
        f: &Facts,
        deal: &DealTerms,
        loan: &LoanTerms,
        until: &str,
    ) -> Result<()> {
        let name = self.person_name(id)?;
        let buyer_name = self.club(buyer).short_name;
        let (title, body) = if deal.fee == 0 && loan.wage_share > 0 {
            (
                format!("Pedido de empréstimo de {name}"),
                format!("O {buyer_name} quer {name} emprestado {}. Responda até {}.", loan.text(), cm_date(until)),
            )
        } else {
            (
                format!("Proposta por {name}"),
                format!(
                    "O {buyer_name} oferece {} por {name} (valor {}). Responda até {}.",
                    deal.text(),
                    money_text(f.value),
                    cm_date(until)
                ),
            )
        };
        self.offer_news(fresh, next_id, title, body, offer);
        Ok(())
    }

    /// Os clubes geridos com a janela aberta que podem fazer propostas: reputação, quanto podem
    /// gastar (metade do saldo e de um quinto das receitas) e o nível (média dos 11 melhores).
    fn bid_clubs(&self) -> Result<Vec<Buyer>> {
        let md = self.date[5..].to_string();
        let clubs: Vec<(i32, i32, i64, Option<i32>)> = self
            .world
            .conn
            .prepare_cached(&format!(
                "SELECT id, COALESCE(reputation, 0), COALESCE(cash, 0), division_id FROM clubs WHERE division_id IN ({}) AND id <> ? ORDER BY id",
                self.managed_ids()
            ))?
            .query_map([self.club_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut buyers: Vec<Buyer> = Vec::new();
        let bonus = self.continental_bonuses();
        for (id, rep, cash, division) in clubs {
            // O peso do clube para o jogador: a reputação e as continentais
            let rep = rep + bonus.get(&id).copied().unwrap_or(0);
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
            let budget = self.ai_budget(id, cash)?;
            buyers.push((id, rep, budget, level as i32));
        }
        Ok(buyers)
    }

    /// "Oferecer a Clubes" (do FM): os serviços do clube mandam a proposta a todos os clubes e,
    /// dentro de 2 a 5 dias, até 3 interessados respondem com uma proposta (ou chega a notícia
    /// de que ninguém quis). `fee` é o valor pedido (0 = o valor do jogador); no empréstimo
    /// não conta. As respostas ficam guardadas com o estado `faxed` até ao dia em que chegam.
    pub fn offer_to_clubs(&mut self, person: i32, loan: bool, fee: i64, exclude_rivals: bool) -> Result<String> {
        let f = self.facts(person)?.ok_or_else(|| Error::Invalid("Só se oferecem jogadores.".into()))?;
        if f.club != Some(self.club_id) || self.loan_of(person)?.is_some() || self.is_ghost(person) {
            return Err(Error::Invalid("Só pode oferecer jogadores do seu plantel.".into()));
        }
        if !self.window_open_for(self.club_id)? {
            return Err(Error::Invalid("A janela de transferências está fechada.".into()));
        }
        if self.open_offer(person)?.is_some() {
            return Err(Error::Invalid("Já há uma proposta em curso por este jogador.".into()));
        }
        let waiting: i64 =
            self.world.conn.query_row("SELECT COUNT(*) FROM offers WHERE person_id = ? AND status = 'faxed'", [person], |r| r.get(0))?;
        if waiting > 0 {
            return Err(Error::Invalid("Já foi oferecido aos clubes: espere pelas respostas.".into()));
        }
        let fee = if loan { 0 } else { finance::round_money(if fee > 0 { fee } else { f.value } as f64).max(10_000) };
        let rivals: [Option<i32>; 3] =
            self.world.conn.query_row("SELECT rival1_id, rival2_id, rival3_id FROM clubs WHERE id = ?", [self.club_id], |r| {
                Ok([r.get(0)?, r.get(1)?, r.get(2)?])
            })?;
        let ca: i32 =
            self.world.conn.query_row("SELECT COALESCE(current_ability, 1) FROM players WHERE person_id = ?", [person], |r| r.get(0))?;
        let my_rep = self.club_pull(self.club_id)?;
        let buyers = self.bid_clubs()?;
        let mut fits = self.willing_buyers(person, &f, fitting_buyers(&buyers, ca, loan, fee, my_rep), loan)?;
        if exclude_rivals {
            fits.retain(|c| !rivals.contains(&Some(*c)));
        }
        let year: u64 = self.date[0..4].parse().unwrap_or(0);
        let mut rng = self.deal_rng(person, 0x5809 ^ (year << 8));
        // Quanto mais acima do valor pede, menos clubes se interessam (o dobro: nenhum)
        let ratio = fee as f64 / f.value.max(1) as f64;
        let chance = if loan { 0.4 } else { (0.5 * (2.0 - ratio)).clamp(0.0, 0.5) };
        let mut picked = Vec::new();
        while !fits.is_empty() && picked.len() < SHOP_MAX {
            let c = fits.swap_remove((rng.next_u64() % fits.len() as u64) as usize);
            if uniform(&mut rng) < chance {
                picked.push(c);
            }
        }
        let days = |rng: &mut ChaCha8Rng| (SHOP_DAYS.0 + rng.next_u64() % (SHOP_DAYS.1 - SHOP_DAYS.0 + 1)) as i32;
        if picked.is_empty() {
            // Ninguém: uma marca para a notícia chegar no dia certo
            let on = date::add_days(&self.date, days(&mut rng)).unwrap_or_default();
            self.world.conn.execute(
                "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('shop', ?, ?, ?, ?, 'faxed', ?, ?)",
                params![person, self.club_id, self.club_id, fee, on, self.date],
            )?;
        }
        for buyer in picked {
            let on = date::add_days(&self.date, days(&mut rng)).unwrap_or_default();
            // Um clube paga o pedido; outro tenta um pouco menos (dá para pedir mais)
            let bid = if loan { 0 } else { finance::round_money(fee as f64 * (0.85 + 0.15 * uniform(&mut rng))).max(10_000) };
            self.world.conn.execute(
                "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES (?, ?, ?, ?, ?, 'faxed', ?, ?)",
                params![if loan { "loan" } else { "transfer" }, person, buyer, self.club_id, bid, on, self.date],
            )?;
            let offer = self.world.conn.last_insert_rowid() as i32;
            let (deal, loan_terms) = bid_terms(&f, loan, bid, &mut rng);
            self.set_terms(offer, &deal, &loan_terms)?;
        }
        self.unsaved = true;
        let name = self.person_name(person)?;
        Ok(if loan {
            format!(
                "Os serviços administrativos do clube enviaram a todos os clubes a proposta de empréstimo de {name}. Será contactado nos próximos dias se houver interesse."
            )
        } else {
            format!(
                "Os serviços administrativos do clube enviaram a todos os clubes a proposta de transferência de {name} por {}. Será contactado nos próximos dias se houver interesse.",
                money_text(fee)
            )
        })
    }

    /// As respostas ao Oferecer a Clubes que chegam hoje: viram propostas normais (uma de cada
    /// vez por jogador; as outras esperam que essa acabe) ou a notícia de que ninguém quis.
    pub(super) fn shop_replies(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let today = self.date.clone();
        let due = self.offers_where("seller = ?1 AND status = 'faxed' AND answer_on <= ?2", params![self.club_id, today])?;
        for o in due {
            let name = self.person_name(o.person)?;
            if o.kind == "shop" {
                self.set_status(o.id, "done", None)?;
                let body = format!("Nenhum clube mostrou interesse em {name} depois de o ter oferecido.");
                self.offer_news(fresh, next_id, format!("Sem interessados em {name}"), body, o.id);
                continue;
            }
            let Some(f) = self.facts(o.person)?.filter(|f| f.club == Some(self.club_id)) else {
                self.set_status(o.id, "collapsed", None)?;
                continue;
            };
            if self.open_offer(o.person)?.is_some() {
                // Espera pelo fim da proposta em curso (e desiste ao fim de três semanas)
                let created: String =
                    self.world.conn.query_row("SELECT COALESCE(created, '') FROM offers WHERE id = ?", [o.id], |r| r.get(0))?;
                let waited = date::parse(&created).zip(date::parse(&today)).map_or(0, |(c, t)| (t - c) as i64);
                if waited > SHOP_WAIT {
                    self.set_status(o.id, "collapsed", None)?;
                } else {
                    let next = date::add_days(&today, 1).unwrap_or_default();
                    self.set_status(o.id, "faxed", Some(&next))?;
                }
                continue;
            }
            let until = date::add_days(&today, INCOMING_DAYS).unwrap_or_default();
            self.set_status(o.id, "pending", Some(&until))?;
            self.bid_news(fresh, next_id, o.id, o.person, o.buyer, &f, &o.deal_terms(), &o.loan_terms(), &until)?;
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

/// Um clube que pode fazer propostas: id, reputação, quanto pode gastar e nível.
type Buyer = (i32, i32, f64, i32);

/// Os clubes que querem um jogador: quem compra, um clube onde ele jogue (e que pague `fee`);
/// quem pede emprestado, um mais pequeno. Se ele aceita, vê-se depois (`willing_buyers`).
fn fitting_buyers(buyers: &[Buyer], ca: i32, as_loan: bool, fee: i64, my_rep: i32) -> Vec<i32> {
    buyers
        .iter()
        .filter(
            |&&(_, rep, budget, level)| {
                if as_loan { rep < my_rep && ca + 10 >= level } else { ca >= level - 5 && ca <= level + 25 && fee as f64 <= budget }
            },
        )
        .map(|b| b.0)
        .collect()
}

/// Condições variadas de uma proposta da IA: prestações (pagando um pouco mais), valor por
/// jogos; no empréstimo, parte do salário e às vezes uma taxa ou uma opção de compra.
fn bid_terms(f: &Facts, as_loan: bool, fee: i64, rng: &mut ChaCha8Rng) -> (DealTerms, LoanTerms) {
    if as_loan {
        let share = [50, 75, 100][(rng.next_u32() % 3) as usize];
        let monthly = if uniform(rng) < 0.2 { finance::round_money(f.value as f64 * 0.005) } else { 0 };
        let option = if uniform(rng) < 0.3 { finance::round_money(f.value as f64 * 1.2) } else { 0 };
        (DealTerms::default(), LoanTerms { months: 0, wage_share: share, monthly_fee: monthly, option_fee: option })
    } else {
        let mut d = DealTerms { fee, ..Default::default() };
        if uniform(rng) < 0.25 {
            d.months = [12, 24][(rng.next_u32() % 2) as usize];
            d.fee = finance::round_money(fee as f64 * 1.05);
        }
        if uniform(rng) < 0.2 {
            d.apps_bonus = finance::round_money(fee as f64 * 0.1);
        }
        (d, LoanTerms::default())
    }
}

/// "(o recorde anterior era de €X)", ou nada se o clube nunca tinha feito um negócio assim.
pub(super) fn record_before(before: i64) -> String {
    if before > 0 { format!(" (o recorde anterior era de {})", super::market::money_text(before)) } else { String::new() }
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
                // Sem resistência do clube (indispensável, acabado de chegar): esses são do `willingness`
                let free_to_sell = crate::willingness::holdout(&g.sale_view(id, &f, FLAMENGO).unwrap()).is_none();
                (f.value > 0 && count > MIN_ROLE[f.role] && g.loan_of(id).unwrap().is_none() && free_to_sell)
                    .then(|| (id, g.asking_price(id, &f, FLAMENGO).unwrap()))
            })
            .collect()
    }

    #[test]
    fn listing_and_squad_roles() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-funcoes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Funções".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 3).unwrap();
        let id: i32 = g
            .world
            .conn
            .query_row(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1
                   AND p.id NOT IN (SELECT person_id FROM ghosts) AND p.id NOT IN (SELECT person_id FROM loans)
                 ORDER BY pl.current_ability LIMIT 1",
                [PALMEIRAS],
                |r| r.get(0),
            )
            .unwrap();
        // Quer ver o futuro esclarecido; posto na lista, deixa de ser necessário e já não pede
        g.add_unhappy(id, "future").unwrap();
        let v = g.set_listing(id, true, false, None).unwrap();
        assert_eq!(v.future.first().map(String::as_str), Some(NOT_NEEDED.1));
        assert!(!v.future.iter().any(|t| t.contains("esclarecido")), "{:?}", v.future);
        assert!(!g.unhappy_reasons(id).unwrap().iter().any(|r| r.contains("esclarecido")));
        // Tirado da lista, volta à função pelo lugar no plantel
        g.set_listing(id, false, false, None).unwrap();
        assert_eq!(g.stored_role(id).unwrap(), None);
        // Definir Função no Plantel: subir agrada, descer custa moral
        let view = g.squad_role_view(id).unwrap();
        assert_eq!(view.roles.len(), SQUAD_ROLES.len() + 1);
        let before = g.squads.state(id).morale;
        g.set_squad_role(id, "key").unwrap();
        assert_eq!(g.squads.state(id).morale, (before + 1).min(20));
        let up = g.squads.state(id).morale;
        assert!(g.set_squad_role(id, NOT_NEEDED.0).unwrap().contains("perder importância"));
        assert!(g.squads.state(id).morale < up);
        assert!(g.set_squad_role(id, "nada").is_err());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
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
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
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
        let reply = g.negotiate(buy, &t.role, 100, 0, t.years, &ContractExtras::default()).unwrap();
        assert!(reply.pending && !reply.accepted && !reply.over, "{}", reply.text);
        assert!(g.negotiate(buy, &t.role, 100, 0, t.years, &ContractExtras::default()).is_err(), "uma proposta de cada vez");
        assert!(next_day(&mut g).iter().any(|n| n.title.starts_with("Contraproposta")));
        let offer = g.open_contract(buy).unwrap().unwrap();
        assert_eq!(offer.status, "countered");
        assert_eq!(g.transfer_view(buy).unwrap().offer.unwrap().kind, "contract", "a aba mostra a proposta de contrato");
        // Aceitar a contraproposta fecha o negócio: o clube paga o valor e o prémio
        let cash = g.cash_of(PALMEIRAS).unwrap();
        assert_eq!(g.answer_offer(offer.id, "accept", None).unwrap().status, "done");
        assert_eq!(g.facts(buy).unwrap().unwrap().club, Some(PALMEIRAS));
        // Como no CM: a mensagem ganha o que o treinador fez, e a contratação "firma acordo"
        let mut n = g.news_item(99_999, "deal", "Proposta".into(), "Texto.".into());
        n.offer_id = Some(offer.id);
        g.news.insert(0, n);
        g.note_action(offer.id, "fez uma nova proposta");
        assert_eq!(
            g.news[0].body,
            "Texto.

Palmeiras fez uma nova proposta."
        );
        assert!(g.news().iter().any(|n| n.title.contains("firma acordo com")));
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
            g.negotiate(mine, &role, 100, 0, 2, &ContractExtras::default()).unwrap();
            let news = next_day(&mut g);
            assert_eq!(news.iter().any(|n| n.title.contains("rompe")), k == 2);
        }
        assert!(g.negotiate(mine, &role, 10_000_000, 0, 2, &ContractExtras::default()).is_err());
        assert!(!g.transfer_view(mine).unwrap().can_renew);
        let other = g.world.conn.query_row(
            "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1 AND p.id <> ? AND p.id <> ? ORDER BY pl.current_ability DESC LIMIT 1",
            params![PALMEIRAS, mine, buy],
            |r| r.get::<_, i32>(0),
        ).unwrap();
        // Renovação com 5% a menos de salário, compensados no prémio: ele aceita e falta confirmar
        let before = g.facts(other).unwrap().unwrap().expires;
        let t = g.contract_terms(other, None).unwrap();
        // Os limites são por função: a que ele espera passa; uma função menor (mais cara) pode não passar
        assert!(t.renewal && t.roles.iter().any(|r| r.key == t.expected_role && r.allowed));
        assert!(t.roles.iter().all(|r| r.max_wage.is_some()));
        let wage = t.wage * 95 / 100;
        let bonus = t.bonus + (t.wage - wage) * 52 * t.years as i64 + 10_000;
        assert!(g.negotiate(other, &t.role, wage, bonus, t.years, &t.asks).unwrap().pending);
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
        // Aceitar abre as conversas com o jogador; depois falta confirmar
        assert_eq!(g.answer_offer(o.id, "accept", None).unwrap().status, "talks");
        assert_eq!(g.facts(sell).unwrap().unwrap().club, Some(PALMEIRAS), "ainda não saiu");
        g.world.conn.execute("UPDATE people SET ambition = 5 WHERE id = ?", [sell]).unwrap();
        for _ in 0..8 {
            if g.offer_view(o.id).unwrap().status != "talks" {
                break;
            }
            g.date = date::add_days(&g.date, 1).unwrap();
            let (mut fresh, mut id) = (Vec::new(), 9500);
            g.sale_talks_day(&mut fresh, &mut id).unwrap();
        }
        let v = g.offer_view(o.id).unwrap();
        assert_eq!(v.status, "confirm");
        assert!(v.actions.iter().any(|a| a.key == "confirm"));
        let cash = g.cash_of(PALMEIRAS).unwrap();
        assert_eq!(g.answer_offer(o.id, "confirm", None).unwrap().status, "done");
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
        g.negotiate(pre, &t.role, t.wage, t.bonus, t.years, &t.asks).unwrap();
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
        g.negotiate(free, &t.role, t.wage, t.bonus, t.years, &t.asks).unwrap();
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

        // Um emprestado ao treinador compra-se ao clube dono: a proposta vai para o Flamengo
        let lent = for_sale(&g).into_iter().map(|t| t.0).find(|&id| g.open_offer(id).unwrap().is_none()).unwrap();
        g.execute_loan(lent, FLAMENGO, PALMEIRAS, &LoanTerms::default()).unwrap();
        let view = g.transfer_view(lent).unwrap();
        assert!(view.can_offer && !view.mine, "comprável ao dono");
        let value = g.facts(lent).unwrap().unwrap().value;
        g.make_offer(lent, &cash_only(value * 3)).unwrap();
        let o = g.open_offer(lent).unwrap().unwrap();
        assert_eq!(o.seller, Some(FLAMENGO));
        for _ in 0..4 {
            g.advance_day().unwrap();
        }
        assert_eq!(
            status(&g, lent).as_deref(),
            Some("agreed"),
            "o dono vende pelo triplo do valor: {:?}",
            g.news().iter().filter(|n| n.offer_id == Some(o.id)).map(|n| &n.body).collect::<Vec<_>>()
        );

        // Oferecer a Clubes: as respostas chegam em 2 a 5 dias (uma proposta ou "sem interessados").
        // Só com a janela aberta: a data salta para lá
        for _ in 0..240 {
            if g.window_open_for(PALMEIRAS).unwrap() {
                break;
            }
            g.date = date::add_days(&g.date, 1).unwrap();
        }
        let shopped: Vec<i32> = g
            .world
            .conn
            .prepare(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1
                   AND p.id NOT IN (SELECT person_id FROM loans) AND p.id NOT IN (SELECT person_id FROM ghosts)
                 ORDER BY pl.current_ability DESC LIMIT 3 OFFSET 12",
            )
            .unwrap()
            .query_map([PALMEIRAS], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let mut sent = Vec::new();
        for (i, &p) in shopped.iter().enumerate() {
            if g.open_offer(p).unwrap().is_some() {
                continue;
            }
            match g.offer_to_clubs(p, i == 2, 0, true) {
                Ok(text) => {
                    assert!(text.contains("todos os clubes"), "{text}");
                    assert!(g.offer_to_clubs(p, false, 0, true).is_err(), "uma vez de cada vez");
                    sent.push(p);
                }
                Err(e) => panic!("{e}"),
            }
        }
        assert!(!sent.is_empty(), "com a janela aberta, há ofertas enviadas");
        for _ in 0..6 {
            g.advance_day().unwrap();
        }
        for p in sent {
            let name = g.person_name(p).unwrap();
            let bid = g.open_offer(p).unwrap().is_some_and(|o| o.seller == Some(PALMEIRAS) && o.status == "pending");
            let nobody = g.news().iter().any(|n| n.title == format!("Sem interessados em {name}"));
            assert!(bid || nobody, "{name}: nem proposta nem notícia");
        }
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
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
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

    #[test]
    fn a_player_without_contract_signs_as_a_free_agent() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-sem-contrato-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Sem contrato".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 6271,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 31).unwrap();
        // Num clube, sem contrato nem salário (o Luca Crudo, "Jogador/Treinador" de um clube alemão)
        let (id, club): (i32, i32) = g
            .world
            .conn
            .query_row(
                "SELECT id, club_id FROM people WHERE is_player = 1 AND club_id IS NOT NULL AND club_id <> 6271
                 AND contract_expires IS NULL AND COALESCE(wage, 0) = 0
                 AND id NOT IN (SELECT person_id FROM loans) ORDER BY id LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let v = g.transfer_view(id).unwrap();
        assert!(v.free && !v.can_offer, "só contrato");
        assert!(matches!(g.contract_party(id).unwrap().2, Party::Free(None)));
        assert!(g.make_offer(id, &DealTerms::default()).is_err());
        // Assinar: sai do clube dele a custo zero
        let terms = g.contract_terms(id, None).unwrap();
        let reply = g.negotiate(id, &terms.role, terms.wage, terms.bonus, 2, &Default::default()).unwrap();
        assert!(!reply.text.is_empty());
        let _ = club;
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn players_weigh_role_money_minutes_and_the_city() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-vontades-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Vontades".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 3).unwrap();
        let squad: Vec<i32> = g
            .world
            .conn
            .prepare(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1
                   AND p.id NOT IN (SELECT person_id FROM loans) AND p.contract_expires IS NOT NULL
                 ORDER BY pl.current_ability DESC, p.id",
            )
            .unwrap()
            .query_map([PALMEIRAS], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();

        // A cidade (item 23): São Paulo é das mais atraentes; quem vem de fora sente-a mais
        if g.city_attraction(PALMEIRAS).unwrap() > 0 {
            assert!(g.city_attraction(PALMEIRAS).unwrap() >= 15);
            assert!(g.city_weight(squad[0], PALMEIRAS).unwrap() <= 1.0);
            // A cidade não baixa o salário de ninguém
            assert_eq!(g.city_wage(squad[0], PALMEIRAS).unwrap(), 1.0);
        }

        // O papel (item 21): ao titular, "apoio" pelo mesmo salário não serve; com 60% a mais, sim
        let star = squad[0];
        let f = g.facts(star).unwrap().unwrap();
        let backup = SQUAD_ROLES.iter().position(|r| r.0 == "backup").unwrap();
        let err = g.role_refusal(star, &f, backup, &Party::Renewal, f.wage, "X").unwrap();
        assert!(err.is_some_and(|e| e.contains("não aceita a função")));
        assert!(g.role_refusal(star, &f, backup, &Party::Renewal, f.wage * 16 / 10, "X").unwrap().is_none());
        let key = SQUAD_ROLES.iter().position(|r| r.0 == "key").unwrap();
        assert!(g.role_refusal(star, &f, key, &Party::Renewal, f.wage, "X").unwrap().is_none());

        // O contrato flexível (item 22): 20% a menos de salário, compensados no prémio, passa;
        // 30% a menos não passa, por maior que seja o prémio
        let (a, b) = (squad[3], squad[4]);
        let t = g.contract_terms(a, None).unwrap();
        let wage = t.wage * 80 / 100;
        let bonus = t.bonus + (t.wage - wage) * 52 * t.years as i64 + 10_000;
        assert!(g.negotiate(a, &t.role, wage, bonus, t.years, &t.asks).unwrap().pending);
        let t2 = g.contract_terms(b, None).unwrap();
        let wage2 = t2.wage * 70 / 100;
        let bonus2 = t2.bonus + (t2.wage - wage2) * 52 * t2.years as i64 * 3;
        assert!(g.negotiate(b, &t2.role, wage2, bonus2, t2.years, &t2.asks).unwrap().pending);
        g.date = date::add_days(&g.date, 1).unwrap();
        let (mut fresh, mut id) = (Vec::new(), 9000);
        g.contract_replies(&mut fresh, &mut id).unwrap();
        assert_eq!(g.open_contract(a).unwrap().unwrap().status, "agreed");
        assert_eq!(g.open_contract(b).unwrap().unwrap().status, "countered");

        // O empréstimo (item 20): o jogador do banco do Flamengo segue as contas da personalidade,
        // e para um clube que detesta não vai
        let bench = *for_sale(&g).last().map(|(id, _)| id).unwrap();
        let f = g.facts(bench).unwrap().unwrap();
        let m = g.player_move(bench, f.ca, f.age, f.ambition, FLAMENGO, PALMEIRAS).unwrap();
        assert_eq!(g.loan_refused(bench, &f, FLAMENGO, PALMEIRAS).unwrap().is_some(), personality::loan_refusal(&m).is_some());
        g.world
            .conn
            .execute("INSERT INTO preferences (person_id, kind, target_id, liked) VALUES (?, 'club', ?, 0)", [bench, PALMEIRAS])
            .unwrap();
        assert!(g.loan_refused(bench, &f, FLAMENGO, PALMEIRAS).unwrap().is_some_and(|w| w.contains("recusa-se a jogar")));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
