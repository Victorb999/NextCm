//! Contratos como no CM (ajuste do mercado do treinador): o treinador propõe o contrato e o
//! jogador responde no dia seguinte (aceita, faz uma contraproposta ou rompe); quando aceita,
//! falta a confirmação do treinador, que fecha o negócio. Um jogador com 6 meses ou menos de
//! contrato pode assinar um pré-contrato e chega livre quando o contrato acabar. A proposta é
//! uma linha de `offers` com `kind = 'contract'` (os botões nas mensagens) e os termos ficam
//! em `contract_talks`. Regras em docs/roadmap/3.09c-contratacao-como-no-cm.md.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use rusqlite::{OptionalExtension, params};

use super::deals::Facts;
use super::deals::{COOLDOWN_DAYS, ContractReply, Offer, Party, TRIES, round_wage};
use super::market::money_text;
use super::{Game, NewsItem, cm_date};
use crate::engine::uniform;
use crate::{Error, Result, date, personality};

/// Dias para o treinador confirmar (ou responder a uma contraproposta) depois da resposta.
const CONFIRM_DAYS: i32 = 7;
/// Contrato que falta para poder assinar um pré-contrato.
pub(super) const PRECONTRACT_DAYS: i64 = 183;
/// Propostas de outros clubes a um livre ou a um jogador em fim de contrato: a probabilidade
/// sobe com o CA (de 0 a 60%), o rival tem reputação até isto acima ou abaixo, e o jogador
/// demora até 5 dias a decidir. Na escolha, cada 1000 de reputação vale 8% do contrato (mais
/// para os ambiciosos).
const RIVAL_CA: (f64, f64) = (90.0, 80.0);
const RIVAL_MAX: f64 = 0.6;
const RIVAL_REP: i32 = 2000;
const REP_WEIGHT: f64 = 0.08;

/// Os termos de uma proposta de contrato.
pub(super) struct Talk {
    pub deal: Option<i32>,
    /// A proposta de outro clube que ele está a ponderar: o clube e o salário.
    pub rival: Option<(i32, i64)>,
    pub role: String,
    pub wage: i64,
    pub bonus: i64,
    pub years: i32,
}

impl Game {
    pub(super) fn talk(&self, person: i32) -> Result<Option<Talk>> {
        Ok(self
            .world
            .conn
            .query_row(
                "SELECT deal_id, role, wage, bonus, years, rival, rival_wage FROM contract_talks WHERE person_id = ?",
                [person],
                |r| {
                    let rival: Option<i32> = r.get(5)?;
                    let rival_wage: Option<i64> = r.get(6)?;
                    Ok(Talk {
                        deal: r.get(0)?,
                        rival: rival.zip(rival_wage),
                        role: r.get(1)?,
                        wage: r.get(2)?,
                        bonus: r.get(3)?,
                        years: r.get(4)?,
                    })
                },
            )
            .optional()?)
    }

    /// A proposta de contrato em curso de um jogador (à espera de resposta, aceite ou com
    /// contraproposta).
    pub(super) fn open_contract(&self, person: i32) -> Result<Option<Offer>> {
        Ok(self
            .offers_where(
                "person_id = ?1 AND buyer = ?2 AND kind = 'contract' AND status IN ('pending', 'agreed', 'countered', 'waiting')",
                params![person, self.club_id],
            )?
            .pop())
    }

    /// Os limites da direção para um contrato (salário máximo, folha e orçamento).
    fn check_contract(&self, party: &Party, old_wage: i64, wage: i64, bonus: i64) -> Result<()> {
        let board = self.board_view()?;
        if wage > board.max_wage {
            return Err(Error::Invalid(format!("A sua direcção autoriza salários até {} por semana.", money_text(board.max_wage))));
        }
        let old = if matches!(party, Party::Renewal) { old_wage } else { 0 };
        if board.wage_bill - old + wage > board.wage_budget {
            return Err(Error::Invalid(format!(
                "A folha salarial passaria o limite da direcção ({} por semana).",
                money_text(board.wage_budget)
            )));
        }
        let fee = if let Party::Transfer(o) = party { o.deal_terms().budget_cost() } else { 0 };
        self.check_budget(fee + bonus)
    }

    /// Proposta de contrato: função, salário semanal, prémio de assinatura e anos. Os limites
    /// da direção e as recusas de princípio (descer de clube, sair da Europa) são já; a
    /// resposta do jogador chega amanhã.
    pub fn negotiate(&mut self, person: i32, role: &str, wage: i64, bonus: i64, years: i32) -> Result<ContractReply> {
        if !(1..=5).contains(&years) {
            return Err(Error::Invalid("Contratos de 1 a 5 anos.".into()));
        }
        let (f, name, party) = self.contract_party(person)?;
        let role_i = Self::role_index(role)?;
        if !Self::role_allowed(role_i, f.age) {
            return Err(Error::Invalid("Vetado pela direcção devido à idade do jogador.".into()));
        }
        if self.open_contract(person)?.is_some_and(|o| o.status == "pending") {
            return Err(Error::Invalid(format!("Já fez uma proposta a {name}: a resposta chega amanhã.")));
        }
        let (wage, bonus) = (wage.max(0), bonus.max(0));
        self.check_contract(&party, f.wage, wage, bonus)?;
        let deal = if let Party::Transfer(o) | Party::Free(Some(o)) = &party { Some(o) } else { None };
        // Não desce para um clube muito menor (quanto mais ambicioso, menos aceita descer)
        let refuse = |text: String| ContractReply { accepted: false, over: true, pending: false, wage: 0, bonus: 0, years, text };
        if let (Party::Transfer(o), Some(club)) = (&party, f.club)
            && self.club_rep(club)? > self.club_rep(self.club_id)? + personality::step_down(f.ambition)
        {
            self.set_status(o.id, "collapsed", None)?;
            self.unsaved = true;
            return Ok(refuse(format!("{name} não está interessado em mudar para um clube menor.")));
        }
        if let (Party::Transfer(o), Some(club)) = (&party, f.club)
            && self.stays_in_europe(person, &f, club)?
        {
            self.set_status(o.id, "collapsed", None)?;
            self.unsaved = true;
            return Ok(refuse(format!("{name} não quer deixar o futebol europeu.")));
        }
        // A proposta: uma nova (a anterior, se houver, fica substituída)
        if let Some(old) = self.open_contract(person)? {
            self.set_status(old.id, "withdrawn", None)?;
        }
        // Um livre ou um jogador em fim de contrato pode ter outras propostas: demora mais a decidir
        let rival = if matches!(party, Party::Free(_) | Party::Precontract) { self.rival_bid(person, &f, role_i)? } else { None };
        let days = match rival {
            Some(_) => 2 + (person.unsigned_abs() + date::parse(&self.date).unwrap_or(0) as u32) % 4,
            None => 1,
        };
        let answer_on = date::add_days(&self.date, days as i32).unwrap_or_default();
        self.world.conn.execute(
            "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('contract', ?, ?, ?, ?, 'pending', ?, ?)",
            params![person, self.club_id, f.club, deal.map_or(0, |o| o.fee), answer_on, self.date],
        )?;
        let offer = self.world.conn.last_insert_rowid() as i32;
        self.world.conn.execute(
            "INSERT OR REPLACE INTO contract_talks (person_id, offer_id, deal_id, role, wage, bonus, years, precontract, rival, rival_wage)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                person,
                offer,
                deal.map(|o| o.id),
                role,
                wage,
                bonus,
                years,
                matches!(party, Party::Precontract) as i32,
                rival.map(|r| r.0),
                rival.map(|r| r.1)
            ],
        )?;
        self.unsaved = true;
        let text = match rival {
            Some(_) => format!(
                "A proposta foi enviada a {name}, que tem outras propostas e vai pensar: a resposta chega a {}.",
                cm_date(&answer_on)
            ),
            None => format!("A proposta foi enviada a {name}. A resposta chega amanhã às mensagens."),
        };
        Ok(ContractReply { accepted: false, over: false, pending: true, wage, bonus, years, text })
    }

    /// A proposta de outro clube a um livre ou a um jogador em fim de contrato (fixa pela seed,
    /// pelo jogador e pelo dia): um clube gerido de reputação parecida, com um salário perto do
    /// que ele pede.
    fn rival_bid(&self, person: i32, f: &Facts, role: usize) -> Result<Option<(i32, i64)>> {
        let day = date::parse(&self.date).unwrap_or(0) as u64;
        let mut rng = ChaCha8Rng::seed_from_u64((self.seed as u64) ^ (person as u64).wrapping_mul(0x9E37_79B9) ^ (day << 32) ^ 0x0417_A001);
        let chance = ((f.ca as f64 - RIVAL_CA.0) / RIVAL_CA.1).clamp(0.0, RIVAL_MAX);
        if uniform(&mut rng) >= chance {
            return Ok(None);
        }
        let my_rep = self.club_rep(self.club_id)?;
        let clubs: Vec<i32> = self
            .world
            .conn
            .prepare_cached(&format!(
                "SELECT id FROM clubs WHERE division_id IN ({}) AND id <> ? AND COALESCE(reputation, 0) BETWEEN ? AND ? ORDER BY id",
                self.managed_ids()
            ))?
            .query_map(params![self.club_id, my_rep - RIVAL_REP, my_rep + RIVAL_REP], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if clubs.is_empty() || f.club.is_some_and(|c| clubs.len() == 1 && clubs[0] == c) {
            return Ok(None);
        }
        let club = clubs[(rng.next_u64() % clubs.len() as u64) as usize];
        if Some(club) == f.club {
            return Ok(None);
        }
        let d = self.demand(person, f, role, false, true)?;
        Ok(Some((club, round_wage(d.wage as f64 * (0.95 + 0.3 * uniform(&mut rng))))))
    }

    /// As respostas dos jogadores às propostas de ontem.
    pub(super) fn contract_replies(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let due = self.offers_where(
            "kind = 'contract' AND buyer = ?1 AND status = 'pending' AND answer_on <= ?2",
            params![self.club_id, self.date],
        )?;
        for o in due {
            let Some(t) = self.talk(o.person)? else {
                self.set_status(o.id, "collapsed", None)?;
                continue;
            };
            let name = self.person_name(o.person)?;
            // Já não dá (o clube vendeu-o a outro, o negócio caiu...): acaba aqui
            let Ok((f, _, party)) = self.contract_party(o.person) else {
                self.set_status(o.id, "collapsed", None)?;
                self.offer_news(
                    fresh,
                    next_id,
                    format!("Negociações com {name}"),
                    format!("As negociações com {name} acabaram: o negócio já não é possível."),
                    o.id,
                );
                continue;
            };
            let role_i = Self::role_index(&t.role)?;
            let free = matches!(party, Party::Free(_) | Party::Precontract);
            let d = self.demand(o.person, &f, role_i, matches!(party, Party::Renewal), free)?;
            let wanted = d.wage as f64 * (1.0 + 0.04 * (t.years - d.years).abs() as f64);
            let weeks = 52.0 * t.years as f64;
            let enough = t.wage as f64 >= 0.9 * wanted && t.wage as f64 * weeks + t.bonus as f64 >= wanted * weeks + d.bonus as f64;
            let deadline = date::add_days(&self.date, CONFIRM_DAYS).unwrap_or_default();
            // Com outra proposta na mão, escolhe a melhor (o contrato todo, pesado pela reputação)
            if let Some((rival, rival_wage)) = t.rival {
                let ours = t.wage as f64 * weeks + t.bonus as f64;
                let theirs = rival_wage as f64 * weeks + d.bonus as f64;
                let diff = (self.club_rep(self.club_id)? - self.club_rep(rival)?) as f64 / 1000.0;
                let weight = REP_WEIGHT * (1.0 + (f.ambition - 10.0).max(0.0) / 10.0);
                if theirs > ours * (1.0 + weight * diff) {
                    self.lost_to_rival(&o, &t, &party, &f, rival, rival_wage, fresh, next_id)?;
                    continue;
                }
            }
            if enough {
                self.set_status(o.id, "agreed", Some(&deadline))?;
                let body = format!(
                    "{name} aceitou a sua proposta: {} por semana, prémio de {}, {} ano(s).\n\nConfirme até {} para fechar o negócio.",
                    money_text(t.wage),
                    money_text(t.bonus),
                    t.years,
                    cm_date(&deadline)
                );
                self.offer_news(fresh, next_id, format!("{name} aceita o contrato"), body, o.id);
                continue;
            }
            let tries: i32 =
                self.world.conn.query_row("SELECT tries FROM talks WHERE person_id = ?", [o.person], |r| r.get(0)).optional()?.unwrap_or(0)
                    + 1;
            let over = tries >= TRIES;
            let blocked = over.then(|| date::add_days(&self.date, COOLDOWN_DAYS).unwrap_or_default());
            self.world.conn.execute(
                "INSERT OR REPLACE INTO talks (person_id, tries, blocked_until) VALUES (?, ?, ?)",
                params![o.person, if over { 0 } else { tries }, blocked],
            )?;
            if over {
                self.set_status(o.id, "collapsed", None)?;
                if let Some(deal) = t.deal {
                    self.set_status(deal, "collapsed", None)?;
                }
                let body = format!("{name} rompeu as negociações e não volta a falar de contrato nos próximos {COOLDOWN_DAYS} dias.");
                self.offer_news(fresh, next_id, format!("{name} rompe negociações"), body, o.id);
            } else {
                // A contraproposta: o que ele quer
                let (w, b, y) = (round_wage(wanted), d.bonus, d.years);
                self.world
                    .conn
                    .execute("UPDATE contract_talks SET wage = ?, bonus = ?, years = ? WHERE person_id = ?", params![w, b, y, o.person])?;
                self.set_status(o.id, "countered", Some(&deadline))?;
                let body = format!(
                    "{name} recusou a proposta e pede {} por semana, um prémio de {} e {} ano(s) de contrato.\n\nResponda até {}.",
                    money_text(w),
                    money_text(b),
                    y,
                    cm_date(&deadline)
                );
                self.offer_news(fresh, next_id, format!("Contraproposta de {name}"), body, o.id);
            }
        }
        Ok(())
    }

    /// O jogador escolheu a proposta do outro clube: um livre assina já por ele; um jogador em
    /// fim de contrato não volta a falar connosco até lá.
    #[allow(clippy::too_many_arguments)]
    fn lost_to_rival(
        &mut self,
        o: &Offer,
        t: &Talk,
        party: &Party,
        f: &Facts,
        rival: i32,
        rival_wage: i64,
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
    ) -> Result<()> {
        let name = self.person_name(o.person)?;
        let club = self.club(rival).short_name;
        self.set_status(o.id, "collapsed", None)?;
        if let Some(deal) = t.deal {
            self.set_status(deal, "collapsed", None)?;
        }
        self.world.conn.execute("DELETE FROM contract_talks WHERE person_id = ?", [o.person])?;
        let body = if matches!(party, Party::Precontract) {
            let until = f.expires.clone().unwrap_or_else(|| self.date.clone());
            self.world
                .conn
                .execute("INSERT OR REPLACE INTO talks (person_id, tries, blocked_until) VALUES (?, 0, ?)", params![o.person, until])?;
            format!("{name} preferiu a proposta do {club} e não vai assinar connosco.")
        } else {
            let kind = self.kind_of(rival)?;
            self.execute_move(o.person, None, Some(rival), 0, rival_wage, t.years, "free", kind)?;
            format!("{name} preferiu a proposta do {club} ({} por semana) e assinou por eles.", money_text(rival_wage))
        };
        self.offer_news(fresh, next_id, format!("{name} escolhe o {club}"), body, o.id);
        Ok(())
    }

    /// Botões de uma proposta de contrato: confirmar, aceitar a contraproposta ou desistir.
    pub(super) fn answer_contract(&mut self, o: &Offer, key: &str) -> Result<()> {
        match (o.status.as_str(), key) {
            ("agreed" | "countered", "confirm" | "accept") => self.finalize_contract(o)?,
            ("pending" | "agreed" | "countered", "withdraw") => {
                self.set_status(o.id, "withdrawn", None)?;
                self.world.conn.execute("DELETE FROM contract_talks WHERE person_id = ?", [o.person])?;
            }
            _ => return Err(Error::Invalid("Esta proposta já não aceita essa resposta.".into())),
        }
        self.unsaved = true;
        Ok(())
    }

    /// Fecha o contrato com os termos acordados: renovação, transferência, livre ou pré-contrato.
    fn finalize_contract(&mut self, o: &Offer) -> Result<()> {
        let t = self.talk(o.person)?.ok_or_else(|| Error::Invalid("Não há contrato para fechar.".into()))?;
        let (f, name, party) = self.contract_party(o.person)?;
        self.check_contract(&party, f.wage, t.wage, t.bonus)?;
        let fee = if let Party::Transfer(d) = &party { d.deal_terms().budget_cost() } else { 0 };
        let kind = self.kind_of(self.club_id)?;
        let from_expiry = matches!(party, Party::Renewal | Party::Precontract);
        let until = self.contract_end(&f, from_expiry, t.years)?;
        match &party {
            Party::Renewal => {
                self.world
                    .conn
                    .execute("UPDATE people SET contract_expires = ?, wage = ? WHERE id = ?", params![until, t.wage, o.person])?;
                self.post_news(
                    format!("{name} renovou"),
                    format!("{name} renovou até {} por {} por semana.", cm_date(&until), money_text(t.wage)),
                    None,
                );
                self.set_status(o.id, "done", None)?;
            }
            Party::Precontract => {
                // Fica assinado: chega livre no dia a seguir ao fim do contrato atual
                let starts = f.expires.as_deref().and_then(|e| date::add_days(e, 1)).unwrap_or_else(|| self.date.clone());
                self.world.conn.execute("UPDATE offers SET status = 'signed', answer_on = ? WHERE id = ?", params![starts, o.id])?;
                self.world.conn.execute("UPDATE contract_talks SET ends = ? WHERE person_id = ?", params![until, o.person])?;
                let body = format!(
                    "{name} assinou um pré-contrato até {} ({} por semana). Chega a custo zero a {}, quando acabar o contrato atual.",
                    cm_date(&until),
                    money_text(t.wage),
                    cm_date(&starts)
                );
                self.post_news(format!("Pré-contrato com {name}"), body, Some(o.id));
            }
            Party::Transfer(deal) | Party::Free(Some(deal)) => {
                // Janela fechada: o contrato fica assinado e a mudança espera pela janela
                if deal.seller.is_some() && !self.window_open_for(self.club_id)? {
                    let opens = super::market::next_window(kind, &self.date);
                    let keep = date::add_days(&opens, 30).unwrap_or_default();
                    self.world.conn.execute("UPDATE offers SET status = 'waiting', answer_on = ? WHERE id = ?", params![opens, o.id])?;
                    self.world.conn.execute("UPDATE offers SET answer_on = ? WHERE id = ?", params![keep, deal.id])?;
                    self.post_news(
                        format!("{name} assina"),
                        format!(
                            "{name} aceitou o contrato. Esta transferência não pode ser consumada até à data de {}: chega ao clube nesse dia.",
                            cm_date(&opens)
                        ),
                        Some(o.id),
                    );
                    self.unsaved = true;
                    return Ok(());
                }
                let move_kind = if deal.seller.is_some() { "transfer" } else { "free" };
                self.execute_move(o.person, deal.seller, Some(self.club_id), deal.fee, t.wage, t.years, move_kind, kind)?;
                if let Some(seller) = deal.seller {
                    self.settle_terms(o.person, self.club_id, seller, &deal.deal_terms())?;
                    // O jogador da troca vai para o outro clube com o contrato que tem
                    if let Some(x) = deal.exchange
                        && let Some(xf) = self.facts(x)?.filter(|xf| xf.club == Some(self.club_id))
                    {
                        let years = super::market::contract_years(xf.age, &mut self.deal_rng(x, 0x7E0C));
                        self.execute_move(x, Some(self.club_id), Some(seller), 0, xf.wage, years, "transfer", kind)?;
                    }
                }
                // Um emprestado comprado (opção de compra) deixa de ser emprestado
                self.world.conn.execute("DELETE FROM loans WHERE person_id = ?", [o.person])?;
                self.set_status(deal.id, "done", None)?;
                self.set_status(o.id, "done", None)?;
                let how = match deal.seller {
                    Some(s) => format!("do {} por {}", self.club(s).short_name, self.terms_text(&deal.deal_terms())?),
                    None => "a custo zero".into(),
                };
                self.post_news(
                    format!("{name} contratado"),
                    format!("{name} assinou até {} ({} por semana). Chega {how}.", cm_date(&until), money_text(t.wage)),
                    None,
                );
                self.squad_changed()?;
            }
            Party::Free(None) => {
                self.execute_move(o.person, None, Some(self.club_id), 0, t.wage, t.years, "free", kind)?;
                self.set_status(o.id, "done", None)?;
                self.post_news(
                    format!("{name} contratado"),
                    format!("{name} assinou até {} ({} por semana). Chega a custo zero.", cm_date(&until), money_text(t.wage)),
                    None,
                );
                self.squad_changed()?;
            }
        }
        self.book(self.club_id, "bonuses", t.bonus)?;
        self.spend_budget(fee + t.bonus)?;
        self.world.conn.execute("DELETE FROM talks WHERE person_id = ?", [o.person])?;
        self.world.conn.execute("INSERT OR REPLACE INTO squad_roles (person_id, role) VALUES (?, ?)", params![o.person, t.role])?;
        if !matches!(party, Party::Precontract) {
            self.world.conn.execute("DELETE FROM contract_talks WHERE person_id = ?", [o.person])?;
        }
        self.remove_unhappy(o.person, "contract")?;
        Ok(())
    }

    /// Os negócios fechados com a janela fechada cujo dia chegou: a transferência consuma-se.
    pub(super) fn waiting_deals_due(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let due = self.offers_where(
            "kind = 'contract' AND buyer = ?1 AND status = 'waiting' AND answer_on <= ?2",
            params![self.club_id, self.date],
        )?;
        for o in due {
            self.set_status(o.id, "agreed", None)?;
            if let Err(e) = self.finalize_contract(&o) {
                self.set_status(o.id, "collapsed", None)?;
                let name = self.person_name(o.person)?;
                let body = format!("A transferência de {name} não se consumou: {e}");
                self.offer_news(fresh, next_id, format!("Negócio por {name} caiu"), body, o.id);
            }
        }
        Ok(())
    }

    /// Os pré-contratos cujo dia chegou: o jogador muda-se, livre, para o clube do treinador.
    pub(super) fn precontracts_due(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let due = self
            .offers_where("kind = 'contract' AND buyer = ?1 AND status = 'signed' AND answer_on <= ?2", params![self.club_id, self.date])?;
        for o in due {
            let Some(t) = self.talk(o.person)? else { continue };
            let from: Option<i32> = self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [o.person], |r| r.get(0))?;
            let kind = self.kind_of(self.club_id)?;
            self.execute_move(o.person, from, Some(self.club_id), 0, t.wage, t.years, "free", kind)?;
            // O fim do contrato combinado no pré-contrato
            let until: Option<String> =
                self.world.conn.query_row("SELECT ends FROM contract_talks WHERE person_id = ?", [o.person], |r| r.get(0))?;
            if let Some(u) = until {
                self.world.conn.execute("UPDATE people SET contract_expires = ? WHERE id = ?", params![u, o.person])?;
            }
            self.set_status(o.id, "done", None)?;
            self.world.conn.execute("DELETE FROM contract_talks WHERE person_id = ?", [o.person])?;
            let name = self.person_name(o.person)?;
            let body = format!("{name} chegou ao clube: o pré-contrato assinado entrou em vigor.");
            self.offer_news(fresh, next_id, format!("{name} chega ao clube"), body, o.id);
            self.squad_changed()?;
        }
        Ok(())
    }

    /// Jogadores com pré-contrato assinado com o treinador (o clube dele não os renova).
    pub(super) fn precontracted(&self) -> Result<Vec<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT person_id FROM offers WHERE kind = 'contract' AND status = 'signed'")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
}
