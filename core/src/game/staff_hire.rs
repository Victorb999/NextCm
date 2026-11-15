//! Contratar e despedir staff, como no CM 01/02 (4.0, item 24): no perfil de alguém do staff,
//! "Oferecer Contrato" com a função, o salário semanal e os anos. A resposta chega no dia
//! seguinte às mensagens, como a dos jogadores: aceita (falta confirmar), contrapõe o salário ou
//! recusa (clube menor, função abaixo da que tem, lealdade). Quem tem clube custa uma multa ao
//! clube dele (um ano do salário que pede) e esse clube contrata um substituto livre uns dias
//! depois. Só os contratados pelo treinador têm salário (o resto do staff continua nos 12% da
//! folha); despedir paga o resto do contrato. A proposta é uma linha de `offers` com
//! `kind = 'staff'` e os termos ficam em `staff_talks`.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::deals::{COOLDOWN_DAYS, OfferView, TRIES, round_wage};
use super::market::{contract_until, money_text};
use super::{Game, NewsItem, cm_date};
use crate::{ClubRef, Error, Result, date};

pub(super) const STAFF_HIRE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS staff_talks (person_id INTEGER PRIMARY KEY, offer_id INTEGER NOT NULL, job INTEGER NOT NULL,
                                        wage INTEGER NOT NULL, years INTEGER NOT NULL, fee INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS staff_refills (club_id INTEGER NOT NULL, job INTEGER NOT NULL, due TEXT NOT NULL);
";

/// As funções que o treinador contrata: adjunto, treinador do time B, preparador, olheiro e
/// fisioterapeuta (o presidente e os directores não são dele).
pub(super) const HIRE_JOBS: [i32; 5] = [6, 7, 8, 9, 10];
/// O treinador principal (o de outro clube não se contrata) e o interino.
const MANAGER: i32 = 5;
const INTERIM: i32 = 15;
/// Dias para confirmar (ou responder à contraproposta).
const CONFIRM_DAYS: i32 = 7;
/// O clube que perdeu alguém contrata um livre ao fim destes dias.
const REFILL_DAYS: i32 = 5;
/// A multa: semanas do salário que ele pede.
const FEE_WEEKS: i64 = 52;
/// Clube menor: a reputação que aceita perder (o ambicioso menos) e a que tem de ganhar por
/// cada degrau de função que desce.
const SMALLER_REP: i32 = 1500;
const STEP_REP: i32 = 1500;
/// Lealdade: com isto ou mais e estes anos no clube, só sai para um clube bem maior.
const LOYAL: i32 = 15;
const LOYAL_YEARS: i32 = 4;
const LOYAL_REP: i32 = 1000;

/// O nome da função (tabela `jobs` do banco, em português do CM).
pub(super) fn job_label(job: i32) -> &'static str {
    match job {
        1 => "Presidente",
        2 => "Diretor Executivo",
        3 => "Diretor de Futebol",
        4 => "Diretor",
        5 => "Treinador",
        6 => "Auxiliar",
        7 => "Treinador do Time B",
        8 => "Preparador",
        9 => "Olheiro",
        10 => "Fisioterapeuta",
        15 => "Treinador Interino",
        _ => "Staff",
    }
}

/// O degrau da função, para a recusa de descer (treinador acima do adjunto, e por aí fora).
fn job_rank(job: i32) -> i32 {
    match job {
        1..=5 => 4,
        6 | 15 => 3,
        7 | 8 => 2,
        _ => 1,
    }
}

/// O salário de cada função sobre o salário médio de um jogador do clube que contrata.
fn job_share(job: i32) -> f64 {
    match job {
        6 => 0.5,
        7 => 0.2,
        8 => 0.15,
        10 => 0.12,
        _ => 0.06,
    }
}

/// Quantos o clube pode ter de cada função: cresce com a reputação (a direcção não deixa mais).
pub(super) fn job_max(job: i32, rep: i32) -> i32 {
    let rep = rep.clamp(0, 10_000);
    match job {
        6 | 7 => 1,
        // O banco tem no máximo 5, 7 e 3 (na reputação 6500, o Palmeiras): um pouco mais nos grandes
        8 => (3 + rep / 2500).min(6),
        9 => (3 + rep / 1500).min(9),
        10 => (2 + rep / 4000).min(4),
        _ => 0,
    }
}

/// Uma função na lista do diálogo: quantos o clube tem, o máximo e o salário que ele pede.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct StaffJobOption {
    pub id: i32,
    pub label: String,
    pub count: i32,
    pub max: i32,
    /// O salário semanal que ele pede para esta função.
    #[ts(type = "number")]
    pub asked: i64,
}

/// O diálogo de contrato de um membro do staff (e o Despedir, se for do clube do treinador).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct StaffHireView {
    pub person_id: i32,
    pub name: String,
    pub club: Option<ClubRef>,
    /// A função de agora (ou a última, se está sem clube).
    pub job: Option<String>,
    /// Do clube do treinador: a proposta é uma renovação e pode ser despedido.
    pub mine: bool,
    /// A função que se pode oferecer.
    pub jobs: Vec<StaffJobOption>,
    /// A multa ao clube dele (0 se está livre ou é nosso).
    #[ts(type = "number")]
    pub fee: i64,
    #[ts(type = "number")]
    pub wage: i64,
    pub contract_expires: Option<String>,
    /// Porque não se pode propor (o treinador de outro clube, um director...).
    pub blocked: Option<String>,
    /// O custo de o despedir (o resto do contrato); nenhum se não pode.
    #[ts(type = "number | null")]
    pub sack_cost: Option<i64>,
    /// A proposta em curso.
    pub offer: Option<OfferView>,
}

/// O que se sabe de alguém do staff para o contratar.
struct StaffFacts {
    club: Option<i32>,
    job: Option<i32>,
    ca: i32,
    wage: i64,
    expires: Option<String>,
    joined: Option<String>,
    ambition: i32,
    loyalty: i32,
}

/// Os termos de uma proposta.
struct StaffTalk {
    job: i32,
    wage: i64,
    years: i32,
    fee: i64,
}

impl Game {
    fn staff_facts(&self, person: i32) -> Result<StaffFacts> {
        self.world
            .conn
            .query_row(
                "SELECT p.club_id, p.club_job, COALESCE(np.current_ability, 80), COALESCE(p.wage, 0), p.contract_expires, p.club_joined,
                        COALESCE(p.ambition, 10), COALESCE(p.loyalty, 10)
                 FROM people p JOIN non_players np ON np.person_id = p.id
                 WHERE p.id = ? AND p.is_player = 0 AND p.id NOT IN (SELECT person_id FROM retired)",
                [person],
                |r| {
                    Ok(StaffFacts {
                        club: r.get(0)?,
                        job: r.get(1)?,
                        ca: r.get(2)?,
                        wage: r.get(3)?,
                        expires: r.get(4)?,
                        joined: r.get(5)?,
                        ambition: r.get(6)?,
                        loyalty: r.get(7)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| Error::Invalid("Só se contrata quem é do staff.".into()))
    }

    /// Quantos o clube tem de uma função.
    fn job_count(&self, club: i32, job: i32) -> Result<i32> {
        Ok(self.world.conn.query_row(
            "SELECT COUNT(*) FROM people WHERE club_id = ? AND club_job = ? AND is_player = 0",
            params![club, job],
            |r| r.get(0),
        )?)
    }

    /// O salário semanal do staff de um clube (só quem tem contrato com salário).
    pub(super) fn staff_bill(&self, club: i32) -> Result<i64> {
        Ok(self
            .world
            .conn
            .query_row("SELECT COALESCE(SUM(wage), 0) FROM people WHERE club_id = ? AND is_player = 0", [club], |r| r.get(0))?)
    }

    /// O salário médio de um jogador do plantel de um clube (a base dos salários do staff).
    fn average_wage(&self, club: i32) -> Result<f64> {
        let (bill, n): (i64, i64) = self.world.conn.query_row(
            "SELECT COALESCE(SUM(wage), 0), COUNT(*) FROM people WHERE club_id = ? AND is_player = 1",
            [club],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(bill as f64 / n.max(15) as f64)
    }

    /// O salário que pede para `job` no clube `club`: pela função, pelo CA e pelo clube; quem já
    /// ganha pede mais para mudar.
    fn staff_demand(&self, f: &StaffFacts, club: i32, job: i32) -> Result<i64> {
        let quality = (f.ca as f64 / 120.0).powf(1.5).clamp(0.3, 2.5);
        let mut wage = self.average_wage(club)? * job_share(job) * quality;
        if f.club.is_none() {
            wage *= 0.9;
        }
        if f.wage > 0 && f.club != Some(club) {
            wage = wage.max(f.wage as f64 * 1.15);
        }
        Ok(round_wage(wage.max(100.0)))
    }

    /// A multa ao clube dele: um ano do salário que pede pela função que tem lá.
    fn staff_fee(&self, f: &StaffFacts) -> Result<i64> {
        match (f.club, f.job) {
            (Some(club), Some(job)) if club != self.club_id => Ok(self.staff_demand(f, club, job)? * FEE_WEEKS),
            _ => Ok(0),
        }
    }

    /// O resto do contrato de quem tem salário (o staff do banco sai sem custo).
    fn sack_cost_of(&self, f: &StaffFacts) -> i64 {
        let weeks = f
            .expires
            .as_deref()
            .and_then(date::parse)
            .zip(date::parse(&self.date))
            .map_or(0, |(end, today)| ((end - today).max(0) as i64 + 6) / 7);
        f.wage * weeks
    }

    /// Porque não se pode oferecer contrato.
    fn hire_blocked(&self, f: &StaffFacts) -> Option<String> {
        match (f.club, f.job) {
            (Some(c), _) if c == self.club_id && f.job.is_some_and(|j| !HIRE_JOBS.contains(&j)) => {
                Some("Essa função não depende do treinador.".into())
            }
            (Some(c), Some(MANAGER | INTERIM)) if c != self.club_id => Some("Não pode tirar o treinador a outro clube.".into()),
            (Some(c), Some(1..=4)) if c != self.club_id => Some("Os diretores dos clubes não se contratam.".into()),
            _ => None,
        }
    }

    pub(super) fn open_staff_offer(&self, person: i32) -> Result<Option<super::deals::Offer>> {
        Ok(self
            .offers_where(
                "person_id = ?1 AND buyer = ?2 AND kind = 'staff' AND status IN ('pending', 'agreed', 'countered')",
                params![person, self.club_id],
            )?
            .pop())
    }

    fn staff_talk(&self, person: i32) -> Result<Option<StaffTalk>> {
        Ok(self
            .world
            .conn
            .query_row("SELECT job, wage, years, fee FROM staff_talks WHERE person_id = ?", [person], |r| {
                Ok(StaffTalk { job: r.get(0)?, wage: r.get(1)?, years: r.get(2)?, fee: r.get(3)? })
            })
            .optional()?)
    }

    /// O diálogo de contrato do staff.
    pub fn staff_hire_view(&self, person: i32) -> Result<StaffHireView> {
        let f = self.staff_facts(person)?;
        let mine = f.club == Some(self.club_id);
        let rep = self.club_rep(self.club_id)?;
        let mut jobs = Vec::new();
        for job in HIRE_JOBS {
            jobs.push(StaffJobOption {
                id: job,
                label: job_label(job).into(),
                count: self.job_count(self.club_id, job)?,
                max: job_max(job, rep),
                asked: self.staff_demand(&f, self.club_id, job)?,
            });
        }
        let sack = mine && f.job.is_some_and(|j| HIRE_JOBS.contains(&j));
        Ok(StaffHireView {
            person_id: person,
            name: self.person_name(person)?,
            club: f.club.map(|c| self.club(c)),
            job: f.job.map(|j| job_label(j).into()),
            mine,
            jobs,
            fee: self.staff_fee(&f)?,
            wage: f.wage,
            contract_expires: f.expires.clone(),
            blocked: self.hire_blocked(&f),
            sack_cost: sack.then(|| self.sack_cost_of(&f)),
            offer: self.open_staff_offer(person)?.map(|o| self.offer_view(o.id)).transpose()?,
        })
    }

    /// Os limites antes de propor e de assinar: a função, o máximo da direcção, a folha e a multa.
    fn check_staff_hire(&self, f: &StaffFacts, job: i32, wage: i64, fee: i64) -> Result<()> {
        if let Some(why) = self.hire_blocked(f) {
            return Err(Error::Invalid(why));
        }
        if !HIRE_JOBS.contains(&job) {
            return Err(Error::Invalid("Essa função não se pode oferecer.".into()));
        }
        // Quem já é nosso nessa função não conta para o máximo
        let same = f.club == Some(self.club_id) && f.job == Some(job);
        let max = job_max(job, self.club_rep(self.club_id)?);
        if !same && self.job_count(self.club_id, job)? >= max {
            return Err(Error::Invalid(format!(
                "A diretoria não autoriza mais de {max} na função de {} (demita alguém primeiro).",
                job_label(job)
            )));
        }
        let board = self.board_view()?;
        let old = if f.club == Some(self.club_id) { f.wage } else { 0 };
        let bill = board.wage_bill + self.staff_bill(self.club_id)? - old + wage;
        if bill > board.wage_budget {
            return Err(Error::Invalid(format!(
                "A folha salarial (com o staff) passaria o limite da diretoria: ficaria em {} por semana, com {} autorizados.",
                money_text(bill),
                money_text(board.wage_budget)
            )));
        }
        self.check_budget(fee)
    }

    /// Oferecer contrato: função, salário semanal e anos. A resposta chega amanhã.
    pub fn offer_staff_contract(&mut self, person: i32, job: i32, wage: i64, years: i32) -> Result<String> {
        if !(1..=5).contains(&years) {
            return Err(Error::Invalid("Contratos de 1 a 5 anos.".into()));
        }
        let f = self.staff_facts(person)?;
        let name = self.person_name(person)?;
        if self.open_staff_offer(person)?.is_some_and(|o| o.status == "pending") {
            return Err(Error::Invalid(format!("Já fez uma proposta a {name}: a resposta chega amanhã.")));
        }
        let blocked: Option<String> =
            self.world.conn.query_row("SELECT blocked_until FROM talks WHERE person_id = ?", [person], |r| r.get(0)).optional()?.flatten();
        if let Some(until) = blocked.filter(|u| u.as_str() > self.date.as_str()) {
            return Err(Error::Invalid(format!("{name} não quer voltar a negociar até {}.", cm_date(&until))));
        }
        let wage = wage.max(0);
        let fee = self.staff_fee(&f)?;
        self.check_staff_hire(&f, job, wage, fee)?;
        if let Some(old) = self.open_staff_offer(person)? {
            self.set_status(old.id, "withdrawn", None)?;
        }
        let answer_on = date::add_days(&self.date, 1).unwrap_or_default();
        self.world.conn.execute(
            "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('staff', ?, ?, ?, ?, 'pending', ?, ?)",
            params![person, self.club_id, f.club, fee, answer_on, self.date],
        )?;
        let offer = self.world.conn.last_insert_rowid() as i32;
        self.world.conn.execute(
            "INSERT OR REPLACE INTO staff_talks (person_id, offer_id, job, wage, years, fee) VALUES (?, ?, ?, ?, ?, ?)",
            params![person, offer, job, wage, years, fee],
        )?;
        self.unsaved = true;
        Ok(format!("A proposta foi enviada a {name}. A resposta chega amanhã às mensagens."))
    }

    /// Recusa por princípio: clube menor (pesa a Ambição), uma função abaixo da que tem ou a
    /// lealdade ao clube onde está há anos.
    fn staff_refusal(&self, f: &StaffFacts, job: i32, name: &str) -> Result<Option<String>> {
        let Some(club) = f.club.filter(|&c| c != self.club_id) else {
            // Sem clube (ou já nosso): só recusa descer dois degraus ou mais
            let steps = f.job.map_or(0, |j| job_rank(j) - job_rank(job));
            return Ok((steps >= 2)
                .then(|| format!("{name} não aceita ser {} depois de ter sido {}.", job_label(job), f.job.map_or("", job_label))));
        };
        let gain = self.club_rep(self.club_id)? - self.club_rep(club)?;
        let short = self.club(club).short_name;
        let smaller = SMALLER_REP - 100 * (f.ambition - 10);
        if gain < -smaller.clamp(500, 2500) {
            return Ok(Some(format!("{name} não está interessado em trocar o {short} por um clube de menor dimensão.")));
        }
        let steps = f.job.map_or(0, |j| job_rank(j) - job_rank(job));
        if steps >= 1 && gain < STEP_REP * steps {
            return Ok(Some(format!(
                "{name} não aceita deixar de ser {} do {short} para ser {}.",
                f.job.map_or("", job_label),
                job_label(job)
            )));
        }
        let years = f.joined.as_deref().and_then(date::parse).zip(date::parse(&self.date)).map_or(0, |(j, t)| (t - j) / 365);
        if f.loyalty >= LOYAL && years >= LOYAL_YEARS && gain < LOYAL_REP {
            return Ok(Some(format!("{name} é muito leal ao {short} e não quer sair.")));
        }
        Ok(None)
    }

    /// As respostas do staff às propostas de ontem.
    pub(super) fn staff_replies(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let due = self
            .offers_where("kind = 'staff' AND buyer = ?1 AND status = 'pending' AND answer_on <= ?2", params![self.club_id, self.date])?;
        for o in due {
            let name = self.person_name(o.person)?;
            let (Some(t), Ok(f)) = (self.staff_talk(o.person)?, self.staff_facts(o.person)) else {
                self.set_status(o.id, "collapsed", None)?;
                continue;
            };
            // Mudou de clube entretanto
            if f.club != o.seller && f.club != Some(self.club_id) {
                self.set_status(o.id, "collapsed", None)?;
                let body = format!("As negociações com {name} acabaram: já não está no mesmo clube.");
                self.offer_news(fresh, next_id, format!("Negociações com {name}"), body, o.id);
                continue;
            }
            if let Some(text) = self.staff_refusal(&f, t.job, &name)? {
                self.set_status(o.id, "rejected", None)?;
                self.offer_news(fresh, next_id, format!("{name} recusa o contrato"), text, o.id);
                continue;
            }
            let wanted = self.staff_demand(&f, self.club_id, t.job)?;
            let deadline = date::add_days(&self.date, CONFIRM_DAYS).unwrap_or_default();
            let fee = if t.fee > 0 {
                format!(" O {} recebe {} de multa.", o.seller.map(|c| self.club(c).short_name).unwrap_or_default(), money_text(t.fee))
            } else {
                String::new()
            };
            if t.wage >= wanted {
                self.set_status(o.id, "agreed", Some(&deadline))?;
                let body = format!(
                    "{name} aceitou ser {} ({} por semana, {} ano(s)).{fee}\n\nConfirme até {} para fechar o contrato.",
                    job_label(t.job),
                    money_text(t.wage),
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
                let body = format!("{name} rompeu as negociações e não volta a falar de contrato nos próximos {COOLDOWN_DAYS} dias.");
                self.offer_news(fresh, next_id, format!("{name} rompe negociações"), body, o.id);
            } else {
                self.world.conn.execute("UPDATE staff_talks SET wage = ? WHERE person_id = ?", params![wanted, o.person])?;
                self.set_status(o.id, "countered", Some(&deadline))?;
                let body = format!(
                    "{name} recusou a proposta e pede {} por semana para ser {}.{fee}\n\nResponda até {}.",
                    money_text(wanted),
                    job_label(t.job),
                    cm_date(&deadline)
                );
                self.offer_news(fresh, next_id, format!("Contraproposta de {name}"), body, o.id);
            }
        }
        Ok(())
    }

    /// Os botões da proposta: "confirm" (aceite), "accept" (a contraproposta), "withdraw".
    pub(super) fn answer_staff(&mut self, o: &super::deals::Offer, key: &str) -> Result<()> {
        match (o.status.as_str(), key) {
            ("pending" | "agreed" | "countered", "withdraw") => self.set_status(o.id, "withdrawn", None),
            ("agreed", "confirm") | ("countered", "accept") => self.sign_staff(o),
            _ => Err(Error::Invalid("Esta proposta já não aceita essa resposta.".into())),
        }
    }

    /// Assina: paga a multa, muda de clube e de função e o clube dele procura um substituto.
    fn sign_staff(&mut self, o: &super::deals::Offer) -> Result<()> {
        let t = self.staff_talk(o.person)?.ok_or_else(|| Error::Invalid("Proposta sem termos.".into()))?;
        let f = self.staff_facts(o.person)?;
        if f.club != o.seller && f.club != Some(self.club_id) {
            self.set_status(o.id, "collapsed", None)?;
            return Err(Error::Invalid("Já não está no mesmo clube.".into()));
        }
        self.check_staff_hire(&f, t.job, t.wage, t.fee)?;
        let name = self.person_name(o.person)?;
        let from = f.club.filter(|&c| c != self.club_id);
        if let Some(club) = from {
            self.book(self.club_id, "other_expenses", t.fee)?;
            self.book(club, "other", t.fee)?;
            if let Some(job) = f.job {
                let due = date::add_days(&self.date, REFILL_DAYS).unwrap_or_default();
                self.world.conn.execute("INSERT INTO staff_refills (club_id, job, due) VALUES (?, ?, ?)", params![club, job, due])?;
            }
        }
        let until = contract_until(self.kind_of(self.club_id)?, &self.date, t.years);
        let joined = if f.club == Some(self.club_id) { f.joined.clone() } else { Some(self.date.clone()) };
        self.world.conn.execute(
            "UPDATE people SET club_id = ?, club_job = ?, wage = ?, contract_expires = ?, club_joined = ? WHERE id = ?",
            params![self.club_id, t.job, t.wage, until, joined, o.person],
        )?;
        if f.club != Some(self.club_id) {
            self.log_staff_move(o.person, f.club, Some(self.club_id))?;
        }
        self.set_status(o.id, "done", None)?;
        self.world.conn.execute("DELETE FROM staff_talks WHERE person_id = ?", [o.person])?;
        self.world.conn.execute("DELETE FROM talks WHERE person_id = ?", [o.person])?;
        let club = self.club(self.club_id).short_name;
        let body = match from {
            Some(c) => format!(
                "{name} é o novo {} do {club}, vindo do {} ({} de multa), com {} por semana até {}.",
                job_label(t.job),
                self.club(c).short_name,
                money_text(t.fee),
                money_text(t.wage),
                cm_date(&until)
            ),
            None => format!("{name} é {} do {club}, com {} por semana até {}.", job_label(t.job), money_text(t.wage), cm_date(&until)),
        };
        self.post_news(format!("{name} assina pelo {club}"), body, Some(o.id));
        self.unsaved = true;
        Ok(())
    }

    /// Despedir alguém do staff do treinador: paga o resto do contrato e fica sem clube.
    pub fn sack_staff(&mut self, person: i32) -> Result<String> {
        let f = self.staff_facts(person)?;
        if f.club != Some(self.club_id) || !f.job.is_some_and(|j| HIRE_JOBS.contains(&j)) {
            return Err(Error::Invalid("Só se demite o staff técnico do seu clube.".into()));
        }
        let cost = self.sack_cost_of(&f);
        self.book(self.club_id, "other_expenses", cost)?;
        self.world
            .conn
            .execute("UPDATE people SET club_id = NULL, wage = 0, contract_expires = NULL, club_joined = NULL WHERE id = ?", [person])?;
        self.log_staff_move(person, Some(self.club_id), None)?;
        if let Some(o) = self.open_staff_offer(person)? {
            self.set_status(o.id, "withdrawn", None)?;
        }
        let name = self.person_name(person)?;
        let text = if cost > 0 {
            format!("{name} foi demitido; o clube pagou {} do resto do contrato.", money_text(cost))
        } else {
            format!("{name} foi demitido, sem custos para o clube.")
        };
        self.post_news_as("contract", format!("{name} demitido"), text.clone());
        self.unsaved = true;
        Ok(text)
    }

    /// O staff do treinador cujo contrato acabou sai do clube; os clubes da IA que perderam
    /// alguém para o treinador contratam um livre da mesma função.
    pub(super) fn staff_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let ended: Vec<i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT id FROM people WHERE club_id = ? AND is_player = 0 AND contract_expires IS NOT NULL AND contract_expires < ? ORDER BY id",
            )?
            .query_map(params![self.club_id, self.date], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for id in ended {
            self.world
                .conn
                .execute("UPDATE people SET club_id = NULL, wage = 0, contract_expires = NULL, club_joined = NULL WHERE id = ?", [id])?;
            self.log_staff_move(id, Some(self.club_id), None)?;
            let name = self.person_name(id)?;
            let body = format!("O contrato de {name} acabou e ele deixou o clube.");
            self.push_news(fresh, next_id, "contract", format!("{name} sai do clube"), body);
        }
        let due: Vec<(i64, i32, i32)> = self
            .world
            .conn
            .prepare_cached("SELECT rowid, club_id, job FROM staff_refills WHERE due <= ? ORDER BY rowid")?
            .query_map([&self.date], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (row, club, job) in due {
            self.world.conn.execute("DELETE FROM staff_refills WHERE rowid = ?", [row])?;
            // O melhor livre da função com o nível do clube (o CA médio do staff dele)
            let level: f64 = self.world.conn.query_row(
                "SELECT COALESCE(AVG(np.current_ability), 80) FROM people p JOIN non_players np ON np.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 0",
                [club],
                |r| r.get(0),
            )?;
            let pick: Option<i32> = self
                .world
                .conn
                .query_row(
                    "SELECT p.id FROM people p JOIN non_players np ON np.person_id = p.id
                     WHERE p.club_id IS NULL AND p.is_player = 0 AND p.club_job = ? AND np.current_ability <= ?
                       AND p.id NOT IN (SELECT person_id FROM retired) AND p.id NOT IN (SELECT person_id FROM staff_talks)
                     ORDER BY np.current_ability DESC, p.id LIMIT 1",
                    params![job, level * 1.1],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(id) = pick {
                self.world.conn.execute(
                    "UPDATE people SET club_id = ?, club_job = ?, club_joined = ? WHERE id = ?",
                    params![club, job, self.date, id],
                )?;
                self.log_staff_move(id, None, Some(club))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;

    /// Um membro do staff (o primeiro pelo CA) que cumpre o filtro.
    fn staff_where(g: &Game, filter: &str) -> i32 {
        g.world
            .conn
            .query_row(
                &format!(
                    "SELECT p.id FROM people p JOIN non_players np ON np.person_id = p.id
                     WHERE p.is_player = 0 AND {filter} ORDER BY np.current_ability DESC, p.id LIMIT 1"
                ),
                [],
                |r| r.get(0),
            )
            .unwrap()
    }

    /// O dia seguinte, só com as respostas e o dia do staff.
    fn next_day(g: &mut Game) -> Vec<NewsItem> {
        g.date = date::add_days(&g.date, 1).unwrap();
        let (mut fresh, mut id) = (Vec::new(), 10_000);
        g.staff_replies(&mut fresh, &mut id).unwrap();
        g.staff_day(&mut fresh, &mut id).unwrap();
        fresh
    }

    #[test]
    fn hire_counter_fee_refill_and_sack() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-staff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Staff".into(),
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
        g.world.conn.execute("UPDATE clubs SET cash = 200000000 WHERE id = ?", [PALMEIRAS]).unwrap();
        g.refresh_budgets().unwrap();
        // Um olheiro livre: abaixo do pedido contrapõe; aceitar a contraproposta assina
        let free = staff_where(&g, "p.club_id IS NULL AND p.club_job = 9");
        let v = g.staff_hire_view(free).unwrap();
        assert!(v.club.is_none() && v.fee == 0 && v.blocked.is_none());
        let asked = v.jobs.iter().find(|j| j.id == 9).unwrap().asked;
        assert!(asked > 0);
        let before = g.job_count(PALMEIRAS, 9).unwrap();
        // Sem lugar para mais olheiros, a direcção não deixa
        let max = job_max(9, g.club_rep(PALMEIRAS).unwrap());
        if before >= max {
            assert!(g.offer_staff_contract(free, 9, asked, 2).is_err());
        }
        while g.job_count(PALMEIRAS, 9).unwrap() >= max {
            let ours = staff_where(&g, &format!("p.club_id = {PALMEIRAS} AND p.club_job = 9"));
            g.sack_staff(ours).unwrap();
        }
        let before = g.job_count(PALMEIRAS, 9).unwrap();
        g.offer_staff_contract(free, 9, asked / 2, 2).unwrap();
        assert!(g.offer_staff_contract(free, 9, asked, 2).is_err(), "uma proposta de cada vez");
        let news = next_day(&mut g);
        assert!(news.iter().any(|n| n.title.starts_with("Contraproposta")), "{news:?}");
        let offer = g.open_staff_offer(free).unwrap().unwrap();
        assert_eq!(offer.status, "countered");
        g.answer_offer(offer.id, "accept", None).unwrap();
        assert_eq!(g.job_count(PALMEIRAS, 9).unwrap(), before + 1);
        let f = g.staff_facts(free).unwrap();
        assert!(f.wage >= asked && f.expires.is_some(), "assinou com salário e contrato");
        assert!(g.staff_bill(PALMEIRAS).unwrap() >= f.wage);

        // Um fisioterapeuta de um clube menor: o salário pedido chega, confirma e paga a multa
        g.sack_staff(free).unwrap();
        let rep = g.club_rep(PALMEIRAS).unwrap();
        let taken = staff_where(
            &g,
            &format!(
                "p.club_job = 10 AND p.club_id IN (SELECT id FROM clubs WHERE reputation BETWEEN {} AND {}) AND COALESCE(p.loyalty, 10) < 15",
                rep - 1500,
                rep - 200
            ),
        );
        let fisio_count = g.job_count(PALMEIRAS, 10).unwrap();
        for _ in 0..(fisio_count - job_max(10, rep) + 1).max(0) {
            let ours = staff_where(&g, &format!("p.club_id = {PALMEIRAS} AND p.club_job = 10"));
            g.sack_staff(ours).unwrap();
        }
        let v = g.staff_hire_view(taken).unwrap();
        let seller = v.club.as_ref().unwrap().id;
        assert!(v.fee > 0, "quem tem clube custa uma multa");
        let asked = v.jobs.iter().find(|j| j.id == 10).unwrap().asked;
        let cash = |g: &Game, c: i32| g.world.conn.query_row("SELECT cash FROM clubs WHERE id = ?", [c], |r| r.get::<_, i64>(0)).unwrap();
        let (mine, theirs) = (cash(&g, PALMEIRAS), cash(&g, seller));
        g.offer_staff_contract(taken, 10, asked, 3).unwrap();
        next_day(&mut g);
        let offer = g.open_staff_offer(taken).unwrap().unwrap();
        assert_eq!(offer.status, "agreed");
        g.answer_offer(offer.id, "confirm", None).unwrap();
        assert_eq!(g.staff_facts(taken).unwrap().club, Some(PALMEIRAS));
        assert_eq!((mine - cash(&g, PALMEIRAS), cash(&g, seller) - theirs), (v.fee, v.fee), "a multa passa de um clube ao outro");
        let left = g.job_count(seller, 10).unwrap();
        for _ in 0..REFILL_DAYS {
            next_day(&mut g);
        }
        assert_eq!(g.job_count(seller, 10).unwrap(), left + 1, "o clube contratou um substituto livre");

        // Despedir paga o resto do contrato
        let cost = g.staff_hire_view(taken).unwrap().sack_cost.unwrap();
        assert!(cost > 0);
        let mine = cash(&g, PALMEIRAS);
        g.sack_staff(taken).unwrap();
        assert_eq!(mine - cash(&g, PALMEIRAS), cost);
        assert!(g.staff_facts(taken).unwrap().club.is_none());

        // O treinador de outro clube não se contrata
        let coach = staff_where(&g, &format!("p.club_job = 5 AND p.club_id IS NOT NULL AND p.club_id <> {PALMEIRAS}"));
        assert!(g.staff_hire_view(coach).unwrap().blocked.is_some());
        assert!(g.offer_staff_contract(coach, 6, 1_000_000, 2).is_err());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn limits_grow_with_reputation() {
        assert_eq!((job_max(6, 9000), job_max(7, 0)), (1, 1));
        assert!(job_max(9, 9000) > job_max(9, 2000) && job_max(9, 6500) >= 7, "o banco tem até 7 olheiros");
        assert_eq!(job_max(1, 9000), 0, "os diretores não são do treinador");
        assert!(job_rank(5) > job_rank(6) && job_rank(6) > job_rank(9));
    }
}
