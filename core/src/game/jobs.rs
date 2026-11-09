//! Empregos (marco 3.12, passo 3). Os clubes geridos da IA têm uma confiança no treinador,
//! revista no dia 1 de cada mês pela posição contra a esperada; a zero, despedem-no e o lugar
//! fica vago até contratarem outro (14 dias depois). A tela "Informação de Empregos" mostra,
//! por país, os lugares vagos e os treinadores inseguros; o treinador concorre a um lugar
//! vago, o clube responde em 3 dias pela reputação dele e, se o quiser, faz uma proposta
//! (`offers.kind = 'job'`). Despedido, o treinador fica sem clube e continua a jogar até
//! encontrar outro. Regras e números em docs/roadmap/3.12-empregos-e-gestao-do-plantel.md.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::board::expectation;
use super::deals::Offer;
use super::{Game, NewsItem, cm_date};
use crate::person::ClubRef;
use crate::{Error, Result, date};

/// Confiança de um treinador da IA ao chegar, e as fronteiras da situação dele.
const AI_START: i32 = 60;
/// Dias até um clube contratar um treinador para o lugar vago.
const VACANCY_DAYS: i32 = 14;
/// Dias até o clube responder a uma candidatura, e para o treinador aceitar a proposta.
const APPLY_DAYS: i32 = 3;
const OFFER_DAYS: i32 = 7;
/// Reputação de clube a mais do que a do treinador que um clube ainda aceita (o dobro para
/// quem está sem clube), e o que a reputação dele ganha ou perde.
const REP_REACH: i32 = 1000;
pub(super) const REP_SACKED: i32 = -500;
pub(super) const REP_SEASON: i32 = 300;
/// As qualificações do treinador (Novo Jogo), e quanto cada nível alarga os clubes que o
/// aceitam (reputação de clube a mais do que a dele).
pub const LICENSES: [&str; 5] = ["Nenhuma", "Licença C", "Licença B", "Licença A", "Licença Pro"];
const LICENSE_REACH: i32 = 250;

/// Confiança abaixo da qual um clube troca o treinador por um candidato (lugar ocupado), e
/// abaixo da qual também o convida por iniciativa própria.
const SHAKY: i32 = 40;
const DESPERATE: i32 = 20;
/// Probabilidade por mês de um clube fazer uma proposta ao treinador (a dobrar sem clube).
const MONTHLY_OFFER: f64 = 0.2;
/// Reputação a partir da qual um despedimento é notícia em qualquer liga.
const NEWS_REP: i32 = 7000;

/// Uma linha da Informação de Empregos.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct JobRow {
    pub club: ClubRef,
    /// O treinador ("Sem Treinador" quando o lugar está vago).
    pub coach: Option<String>,
    pub nation: String,
    /// Reputação do clube em letra (A a E).
    pub letter: String,
    /// "Disponível", "Ligeiramente Inseguro", "Inseguro", "Muito Inseguro".
    pub situation: String,
    pub vacant: bool,
    /// O treinador já concorreu a este lugar.
    pub applied: bool,
}

/// Um país com clubes geridos (o "Vêr" da tela).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct JobNation {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct JobsView {
    pub nations: Vec<JobNation>,
    pub nation: Option<JobNation>,
    pub rows: Vec<JobRow>,
    /// Sem clube (despedido).
    pub unemployed: bool,
}

fn letter(rep: i32) -> &'static str {
    match rep {
        7000.. => "A",
        5500.. => "B",
        4000.. => "C",
        2500.. => "D",
        _ => "E",
    }
}

fn situation(confidence: i32) -> &'static str {
    match confidence {
        60.. => "Seguro",
        40..=59 => "Ligeiramente Inseguro",
        20..=39 => "Inseguro",
        _ => "Muito Inseguro",
    }
}

impl Game {
    /// Reputação do treinador (começa na do clube onde começou o jogo).
    pub(super) fn manager_rep(&self) -> Result<i32> {
        let rep: Option<i32> =
            self.world.conn.query_row("SELECT reputation FROM manager_info WHERE id = 1", [], |r| r.get(0)).optional()?;
        match rep {
            Some(r) => Ok(r),
            None => {
                let r = self.club_rep_of(self.club_id)?;
                self.world.conn.execute("INSERT INTO manager_info (id, reputation) VALUES (1, ?)", [r])?;
                Ok(r)
            }
        }
    }

    /// As qualificações (0 Nenhuma a 4 Licença Pro).
    pub(super) fn license(&self) -> Result<i32> {
        self.manager_rep()?;
        Ok(self.world.conn.query_row("SELECT license FROM manager_info WHERE id = 1", [], |r| r.get(0))?)
    }

    pub(super) fn set_license(&self, level: i32) -> Result<()> {
        self.manager_rep()?;
        self.world.conn.execute("UPDATE manager_info SET license = ? WHERE id = 1", [level.clamp(0, LICENSES.len() as i32 - 1)])?;
        Ok(())
    }

    /// Reputação de clube a mais do que a dele que um clube ainda aceita: o dobro sem clube,
    /// mais o que as qualificações dão.
    fn reach(&self) -> Result<i32> {
        let base = if self.sacked() { 2 * REP_REACH } else { REP_REACH };
        Ok(base + LICENSE_REACH * self.license()?)
    }

    pub(super) fn change_manager_rep(&self, delta: i32) -> Result<()> {
        let r = self.manager_rep()?;
        self.world.conn.execute("UPDATE manager_info SET reputation = ? WHERE id = 1", [(r + delta).clamp(0, 10_000)])?;
        Ok(())
    }

    fn club_rep_of(&self, club: i32) -> Result<i32> {
        Ok(self.world.conn.query_row("SELECT COALESCE(reputation, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?)
    }

    /// O treinador de um clube (cargo 5).
    pub(super) fn coach_of(&self, club: i32) -> Result<Option<(i32, String)>> {
        Ok(self
            .world
            .conn
            .query_row(
                "SELECT id, display_name FROM people WHERE club_id = ? AND club_job = 5 AND id <> ? LIMIT 1",
                params![club, self.manager_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    fn ai_confidence(&self, club: i32) -> Result<i32> {
        Ok(self
            .world
            .conn
            .query_row("SELECT confidence FROM ai_boards WHERE club_id = ?", [club], |r| r.get(0))
            .optional()?
            .unwrap_or(AI_START))
    }

    /// A Informação de Empregos de um país (sem país: o do clube do treinador).
    pub fn jobs_view(&self, nation: Option<i32>) -> Result<JobsView> {
        let clubs = self.managed_clubs()?;
        let mut by_nation: Vec<(i32, String)> = Vec::new();
        for &c in &clubs {
            let row: Option<(i32, String)> = self
                .world
                .conn
                .query_row("SELECT n.id, n.name FROM clubs c JOIN nations n ON n.id = c.nation_id WHERE c.id = ?", [c], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()?;
            if let Some(n) = row
                && !by_nation.contains(&n)
            {
                by_nation.push(n);
            }
        }
        by_nation.sort_by(|a, b| a.1.cmp(&b.1));
        let own: Option<i32> = self.world.conn.query_row("SELECT nation_id FROM clubs WHERE id = ?", [self.club_id], |r| r.get(0))?;
        let chosen = nation.or(own).and_then(|n| by_nation.iter().find(|x| x.0 == n).cloned()).or_else(|| by_nation.first().cloned());
        let mut rows = Vec::new();
        if let Some((nid, nname)) = &chosen {
            for &c in &clubs {
                let club_nation: Option<i32> = self.world.conn.query_row("SELECT nation_id FROM clubs WHERE id = ?", [c], |r| r.get(0))?;
                if club_nation != Some(*nid) || (c == self.club_id && !self.sacked()) {
                    continue;
                }
                let coach = self.coach_of(c)?;
                let confidence = self.ai_confidence(c)?;
                if coach.is_some() && confidence >= 40 {
                    continue;
                }
                let applied: bool = self
                    .world
                    .conn
                    .query_row("SELECT 1 FROM offers WHERE kind = 'job' AND buyer = ? AND status IN ('applied', 'pending')", [c], |_| {
                        Ok(true)
                    })
                    .optional()?
                    .is_some();
                rows.push(JobRow {
                    club: self.club(c),
                    vacant: coach.is_none(),
                    situation: if coach.is_none() { "Disponível".into() } else { situation(confidence).into() },
                    coach: coach.map(|x| x.1),
                    nation: nname.clone(),
                    letter: letter(self.club_rep_of(c)?).into(),
                    applied,
                });
            }
            rows.sort_by(|a, b| b.vacant.cmp(&a.vacant).then(a.club.name.cmp(&b.club.name)));
        }
        Ok(JobsView {
            nations: by_nation.iter().map(|(id, name)| JobNation { id: *id, name: name.clone() }).collect(),
            nation: chosen.map(|(id, name)| JobNation { id, name }),
            rows,
            unemployed: self.sacked(),
        })
    }

    /// "Concorrer ao Lugar" de um clube sem treinador; devolve a frase para a tela.
    pub fn apply_job(&mut self, club: i32) -> Result<String> {
        if !self.managed_clubs()?.contains(&club) {
            return Err(Error::Invalid("Só se concorre a clubes das ligas em jogo.".into()));
        }
        if club == self.club_id && !self.sacked() {
            return Err(Error::Invalid("Já é o treinador deste clube.".into()));
        }
        if self.coach_of(club)?.is_some() && self.ai_confidence(club)? >= SHAKY {
            return Err(Error::Invalid("O treinador deste clube está seguro no lugar.".into()));
        }
        let open: bool = self
            .world
            .conn
            .query_row("SELECT 1 FROM offers WHERE kind = 'job' AND buyer = ? AND status IN ('applied', 'pending')", [club], |_| Ok(true))
            .optional()?
            .is_some();
        if open {
            return Err(Error::Invalid("Já concorreu a este lugar.".into()));
        }
        let answer = date::add_days(&self.date, APPLY_DAYS).unwrap_or_default();
        self.world.conn.execute(
            "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('job', ?, ?, NULL, 0, 'applied', ?, ?)",
            params![self.manager_id, club, answer, self.date],
        )?;
        self.unsaved = true;
        let name = self.club(club).name;
        Ok(format!("Concorreu ao lugar de treinador do {name}. A resposta chega a {}.", cm_date(&answer)))
    }

    /// Botões de uma proposta de emprego.
    pub(super) fn answer_job(&mut self, o: &Offer, key: &str) -> Result<()> {
        match (o.status.as_str(), key) {
            ("pending", "accept") => {
                self.set_status(o.id, "done", None)?;
                self.take_job(o.buyer)?;
            }
            ("pending", "reject") => self.set_status(o.id, "rejected", None)?,
            _ => return Err(Error::Invalid("Esta proposta já não aceita essa resposta.".into())),
        }
        self.unsaved = true;
        Ok(())
    }

    /// O treinador muda-se para outro clube: o anterior fica com o lugar vago, a direção e
    /// as listas do clube novo começam do zero.
    pub(super) fn take_job(&mut self, club: i32) -> Result<()> {
        let old = self.club_id;
        // Um lugar ocupado: o treinador que lá estava sai
        if let Some((coach, _)) = self.coach_of(club)? {
            self.world.conn.execute("UPDATE people SET club_id = NULL, club_job = NULL WHERE id = ?", [coach])?;
            self.log_staff_move(coach, Some(club), None)?;
        }
        if !self.sacked() {
            self.world.conn.execute("INSERT OR REPLACE INTO vacancies (club_id, since) VALUES (?, ?)", params![old, self.date])?;
        }
        self.world.conn.execute(
            "UPDATE offers SET status = 'collapsed' WHERE kind <> 'job' AND (buyer = ?1 OR seller = ?1)
             AND status IN ('pending', 'countered', 'agreed', 'raised', 'applied', 'signed')",
            [old],
        )?;
        self.world.conn.execute(
            "UPDATE offers SET status = 'withdrawn' WHERE kind = 'job' AND buyer <> ? AND status IN ('applied', 'pending')",
            [club],
        )?;
        self.world.conn.execute_batch(
            "DELETE FROM contract_talks; DELETE FROM unhappy; DELETE FROM scout_tasks; DELETE FROM scout_finds; DELETE FROM scouting;
             DELETE FROM talks; DELETE FROM squad_roles; DELETE FROM training_players;",
        )?;
        self.world.conn.execute("DELETE FROM vacancies WHERE club_id = ?", [club])?;
        self.world.conn.execute("DELETE FROM ai_boards WHERE club_id = ?", [club])?;
        self.world.conn.execute(
            "UPDATE people SET club_id = ?, club_job = 5, club_joined = ? WHERE id = ?",
            params![club, self.date, self.manager_id],
        )?;
        self.log_staff_move(self.manager_id, (!self.sacked()).then_some(old), Some(club))?;
        for s in self.squads.states.values_mut() {
            s.reserve = false;
        }
        self.career_leave()?;
        self.club_id = club;
        self.selection = None;
        self.board_setup()?;
        self.career_join(club)?;
        self.refresh_live()?;
        self.start_reserves()?;
        let next_id = self.news.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let (mut fresh, mut id) = (Vec::new(), next_id);
        self.numbers_news(&mut fresh, &mut id)?;
        self.news.splice(0..0, fresh);
        let name = self.club(club).name;
        self.post_news_as(
            "jobs",
            format!("Novo treinador do {name}"),
            format!("É o novo treinador do {name}. A Direcção espera ver resultados."),
        );
        self.unsaved = true;
        Ok(())
    }

    /// O dia dos empregos: respostas às candidaturas, propostas que expiram, lugares vagos
    /// que os clubes preenchem e, no dia 1 de cada mês, a confiança dos treinadores da IA.
    pub(super) fn jobs_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        // Respostas às candidaturas
        let due = self.offers_where("kind = 'job' AND status = 'applied' AND answer_on <= ?1", params![self.date])?;
        let reach = self.reach()?;
        for o in due {
            let name = self.club(o.buyer).name;
            let club_rep = self.club_rep_of(o.buyer)?;
            let within = club_rep <= self.manager_rep()? + reach;
            // Um lugar ocupado: o clube só troca um treinador inseguro, e por um com nome
            let occupied = self.coach_of(o.buyer)?.is_some();
            let confidence = self.ai_confidence(o.buyer)?;
            let wants = within && (!occupied || confidence < DESPERATE || (confidence < SHAKY && self.manager_rep()? >= club_rep));
            if wants {
                let until = date::add_days(&self.date, OFFER_DAYS).unwrap_or_default();
                self.set_status(o.id, "pending", Some(&until))?;
                let body = format!("O {name} quer contratá-lo para treinador. Responda até {}.", cm_date(&until));
                self.offer_news(fresh, next_id, format!("Proposta do {name}"), body, o.id);
            } else {
                self.set_status(o.id, "rejected", None)?;
                let body = if occupied && within {
                    format!("O {name} agradece o interesse, mas mantém a confiança no treinador actual.")
                } else {
                    format!("O {name} agradece o interesse, mas procura um treinador com outro perfil.")
                };
                self.offer_news(fresh, next_id, format!("Resposta do {name}"), body, o.id);
            }
        }
        for o in self.offers_where("kind = 'job' AND status = 'pending' AND answer_on < ?1", params![self.date])? {
            self.set_status(o.id, "withdrawn", None)?;
        }
        // Lugares vagos: ao fim de 14 dias o clube contrata o melhor treinador sem clube
        let since = date::add_days(&self.date, -VACANCY_DAYS).unwrap_or_default();
        let ready: Vec<i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT club_id FROM vacancies WHERE since <= ?
                 AND club_id NOT IN (SELECT buyer FROM offers WHERE kind = 'job' AND status IN ('applied', 'pending')) ORDER BY club_id",
            )?
            .query_map([&since], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for club in ready {
            if club == self.club_id && !self.sacked() {
                self.world.conn.execute("DELETE FROM vacancies WHERE club_id = ?", [club])?;
                continue;
            }
            let coach: Option<(i32, String)> = self
                .world
                .conn
                .query_row(
                    "SELECT p.id, p.display_name FROM people p JOIN non_players np ON np.person_id = p.id
                     WHERE p.club_id IS NULL AND p.is_non_player = 1 AND p.id <> ? AND p.id NOT IN (SELECT person_id FROM retired)
                     ORDER BY COALESCE(np.current_ability, 0) DESC, p.id LIMIT 1",
                    [self.manager_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((id, coach_name)) = coach else { continue };
            self.world
                .conn
                .execute("UPDATE people SET club_id = ?, club_job = 5, club_joined = ? WHERE id = ?", params![club, self.date, id])?;
            self.log_staff_move(id, None, Some(club))?;
            self.world.conn.execute("DELETE FROM vacancies WHERE club_id = ?", [club])?;
            self.world.conn.execute("INSERT OR REPLACE INTO ai_boards (club_id, confidence) VALUES (?, ?)", params![club, AI_START])?;
            if self.newsworthy(club)? {
                let name = self.club(club).name;
                self.push_news(
                    fresh,
                    next_id,
                    "jobs",
                    format!("{coach_name} no {name}"),
                    format!("{coach_name} é o novo treinador do {name}."),
                );
            }
        }
        if self.date.ends_with("-01") {
            self.ai_boards_month(fresh, next_id)?;
            self.unsolicited_offer(fresh, next_id)?;
        }
        Ok(())
    }

    /// No dia 1 de cada mês, às vezes um clube convida o treinador por iniciativa própria: um
    /// lugar vago ou um treinador à beira do despedimento, num clube ao alcance da reputação
    /// dele e não muito abaixo (sem clube, qualquer um abaixo serve). No máximo uma por mês.
    fn unsolicited_offer(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let mut rng = self.season_rng(0x10B5);
        let chance = if self.sacked() { 2.0 * MONTHLY_OFFER } else { MONTHLY_OFFER };
        if crate::engine::uniform(&mut rng) >= chance {
            return Ok(());
        }
        let rep = self.manager_rep()?;
        let mut options = Vec::new();
        for club in self.managed_clubs()? {
            // Nem o clube dele, nem o que o despediu
            if club == self.club_id {
                continue;
            }
            let club_rep = self.club_rep_of(club)?;
            let fits = club_rep <= rep + self.reach()? / 2 && (self.sacked() || club_rep >= rep - REP_REACH);
            // Um lugar vago há pelo menos uns dias, ou um treinador à beira do despedimento
            let settled = date::add_days(&self.date, -APPLY_DAYS).unwrap_or_default();
            let vacant_since: Option<String> =
                self.world.conn.query_row("SELECT since FROM vacancies WHERE club_id = ?", [club], |r| r.get(0)).optional()?;
            let open = match self.coach_of(club)? {
                None => vacant_since.is_none_or(|s| s <= settled),
                Some(_) => self.ai_confidence(club)? < DESPERATE,
            };
            let busy: bool = self
                .world
                .conn
                .query_row("SELECT 1 FROM offers WHERE kind = 'job' AND buyer = ? AND status IN ('applied', 'pending')", [club], |_| {
                    Ok(true)
                })
                .optional()?
                .is_some();
            if fits && open && !busy {
                options.push((club_rep, club));
            }
        }
        // O clube de maior nome entre os que servem
        let Some(&(_, club)) = options.iter().max() else { return Ok(()) };
        let until = date::add_days(&self.date, OFFER_DAYS).unwrap_or_default();
        self.world.conn.execute(
            "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('job', ?, ?, NULL, 0, 'pending', ?, ?)",
            params![self.manager_id, club, until, self.date],
        )?;
        let id = self.world.conn.last_insert_rowid() as i32;
        let name = self.club(club).name;
        let body = format!("O {name} gostava de o ter como treinador e faz-lhe uma proposta. Responda até {}.", cm_date(&until));
        self.offer_news(fresh, next_id, format!("Proposta do {name}"), body, id);
        Ok(())
    }

    /// Um despedimento ou uma contratação neste clube interessa ao treinador?
    fn newsworthy(&self, club: i32) -> Result<bool> {
        let same: bool = self.world.conn.query_row(
            "SELECT COALESCE((SELECT division_id FROM clubs WHERE id = ?) = (SELECT division_id FROM clubs WHERE id = ?), 0)",
            params![club, self.club_id],
            |r| r.get(0),
        )?;
        Ok(same || self.club_rep_of(club)? >= NEWS_REP)
    }

    /// No dia 1 de cada mês: a confiança de cada clube da IA no treinador, pela posição contra
    /// a esperada (−6 a +4, como a do treinador); a zero, despede-o.
    fn ai_boards_month(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        for club in self.managed_clubs()? {
            if club == self.club_id {
                continue;
            }
            let Some((coach, coach_name)) = self.coach_of(club)? else { continue };
            let division: Option<i32> = self.world.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [club], |r| r.get(0))?;
            let Some(div) = division else { continue };
            let Some(row) = self.table(div).into_iter().find(|r| r.club.id == club && r.played >= 4) else { continue };
            let (target, _, _) = expectation(&self.world, club)?;
            let confidence = (self.ai_confidence(club)? + (target - row.position).clamp(-6, 4)).min(100);
            if confidence > 0 {
                self.world
                    .conn
                    .execute("INSERT OR REPLACE INTO ai_boards (club_id, confidence) VALUES (?, ?)", params![club, confidence])?;
                continue;
            }
            self.world.conn.execute("UPDATE people SET club_id = NULL, club_job = NULL WHERE id = ?", [coach])?;
            self.log_staff_move(coach, Some(club), None)?;
            self.world.conn.execute("DELETE FROM ai_boards WHERE club_id = ?", [club])?;
            self.world.conn.execute("INSERT OR REPLACE INTO vacancies (club_id, since) VALUES (?, ?)", params![club, self.date])?;
            if self.newsworthy(club)? {
                let name = self.club(club).name;
                self.push_news(
                    fresh,
                    next_id,
                    "jobs",
                    format!("Treinador {coach_name} despedido"),
                    format!("O {name} despediu o treinador {coach_name}."),
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn sacked_coaches_and_new_jobs() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-empregos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Empregos".into(),
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
        };
        let mut g = Game::create(&path, &dir, &new, 37).unwrap();
        // Um clube da Série A com o treinador a zero de confiança: despedido no dia 1
        let div: i32 = g.world.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [PALMEIRAS], |r| r.get(0)).unwrap();
        let target = g.managed_clubs().unwrap().into_iter().find(|&c| {
            c != PALMEIRAS
                && g.coach_of(c).unwrap().is_some()
                && g.world.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [c], |r| r.get::<_, i32>(0)).unwrap() == div
        });
        let target = target.unwrap();
        g.world.conn.execute("INSERT OR REPLACE INTO ai_boards (club_id, confidence) VALUES (?, -100)", [target]).unwrap();
        // Jogos suficientes na tabela: força o mês com 4 jornadas jogadas
        while g.table(div).iter().find(|r| r.club.id == target).is_none_or(|r| r.played < 4) {
            g.advance_day().unwrap();
        }
        let first = date::add_days(&format!("{}-01", &g.date[0..7]), 40).unwrap();
        g.date = format!("{}-01", &first[0..7]);
        let (mut fresh, mut id) = (Vec::new(), 9000);
        g.jobs_day(&mut fresh, &mut id).unwrap();
        assert!(g.coach_of(target).unwrap().is_none(), "despedido");
        assert!(fresh.iter().any(|n| n.title.contains("despedido")));
        let view = g.jobs_view(None).unwrap();
        assert!(view.rows.iter().any(|r| r.club.id == target && r.vacant && r.situation == "Disponível"));

        // O treinador concorre; com reputação, o clube faz proposta e ele muda de clube
        g.world.conn.execute("INSERT OR REPLACE INTO manager_info (id, reputation) VALUES (1, 9000)", []).unwrap();
        assert!(g.apply_job(target).unwrap().contains("Concorreu"));
        assert!(g.apply_job(target).is_err());
        g.date = date::add_days(&g.date, APPLY_DAYS).unwrap();
        let (mut fresh, mut id) = (Vec::new(), 9000);
        g.jobs_day(&mut fresh, &mut id).unwrap();
        let offer = fresh.iter().find(|n| n.title.starts_with("Proposta do")).and_then(|n| n.offer_id).expect("proposta");
        g.answer_offer(offer, "accept", None).unwrap();
        assert_eq!(g.club_id, target);
        assert!(!g.sacked());
        assert_eq!(g.board_view().unwrap().confidence, 60);
        // O Palmeiras fica com o lugar vago e contrata outro ao fim de 14 dias
        assert!(g.coach_of(PALMEIRAS).unwrap().is_none());
        g.date = date::add_days(&g.date, VACANCY_DAYS).unwrap();
        let (mut fresh, mut id) = (Vec::new(), 9000);
        g.jobs_day(&mut fresh, &mut id).unwrap();
        assert!(g.coach_of(PALMEIRAS).unwrap().is_some());

        // Despedido: o jogo continua sem clube
        let (mut fresh, mut id) = (Vec::new(), 9000);
        g.change_confidence(-100, &mut fresh, &mut id).unwrap();
        assert!(g.sacked());
        g.advance_day().unwrap();
        assert!(g.jobs_view(None).unwrap().unemployed);

        // Lugar ocupado: um treinador seguro não sai; um à beira do despedimento sim
        let other =
            g.managed_clubs().unwrap().into_iter().find(|&c| c != target && c != PALMEIRAS && g.coach_of(c).unwrap().is_some()).unwrap();
        let old_coach = g.coach_of(other).unwrap().unwrap().0;
        g.world.conn.execute("INSERT OR REPLACE INTO ai_boards (club_id, confidence) VALUES (?, 80)", [other]).unwrap();
        assert!(g.apply_job(other).is_err(), "treinador seguro");
        g.world.conn.execute("INSERT OR REPLACE INTO ai_boards (club_id, confidence) VALUES (?, 10)", [other]).unwrap();
        assert!(g.apply_job(other).is_ok());
        g.date = date::add_days(&g.date, APPLY_DAYS).unwrap();
        let (mut fresh, mut id) = (Vec::new(), 9000);
        g.jobs_day(&mut fresh, &mut id).unwrap();
        let offer = fresh.iter().find(|n| n.title.starts_with("Proposta do")).and_then(|n| n.offer_id).expect("proposta do lugar ocupado");
        g.answer_offer(offer, "accept", None).unwrap();
        assert_eq!(g.club_id, other);
        let still: Option<i32> = g.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [old_coach], |r| r.get(0)).unwrap();
        assert_eq!(still, None, "o treinador antigo sai");
        assert_eq!(g.coach_of(other).unwrap(), None);

        // Propostas por iniciativa própria: sem clube, ao fim de uns meses aparece uma
        let (mut fresh, mut id) = (Vec::new(), 9000);
        g.change_confidence(-100, &mut fresh, &mut id).unwrap();
        assert!(g.sacked());
        let mut got = false;
        for _ in 0..36 {
            let next = date::add_days(&format!("{}-01", &g.date[0..7]), 40).unwrap();
            g.date = format!("{}-01", &next[0..7]);
            let (mut fresh, mut id) = (Vec::new(), 9000);
            g.unsolicited_offer(&mut fresh, &mut id).unwrap();
            if fresh.iter().any(|n| n.title.starts_with("Proposta do") && n.offer_id.is_some()) {
                got = true;
                break;
            }
        }
        assert!(got, "nenhum clube fez proposta em 3 anos");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
