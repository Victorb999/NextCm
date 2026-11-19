//! Ir de Férias (item 19 do 4.0): as opções do CM 01/02 e do FM08 (empregos, vendas, táticas,
//! a data de volta), os dias a passar um a um (o app mostra-os e pode voltar a qualquer
//! momento), a volta antecipada por uma proposta de emprego ou um aviso da Diretoria e o
//! resumo do que se passou ao voltar.

use std::collections::HashSet;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::market::money_text;
use super::{Game, GameInfo, cm_date};
use crate::tactics::Selection;
use crate::{Error, Result, date};

/// Dias máximos de umas férias com data (as indefinidas param pelo botão ou por um evento).
const MAX_HOLIDAY: i32 = 366;

/// O que o adjunto faz com as propostas pelos jogadores do plantel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum HolidaySales {
    /// Aceitar todas.
    Accept,
    /// Aceitar só as que pagam o valor exigido; recusar as outras.
    #[default]
    Asking,
    /// Recusar todas.
    Reject,
}

/// As opções do diálogo "Ir de Férias".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct HolidayPlan {
    /// Concorrer aos lugares vagos de clubes de maior reputação.
    pub apply_jobs: bool,
    /// Voltar ao receber uma proposta de emprego.
    pub return_on_job: bool,
    /// Voltar quando a Diretoria avisar (preocupada ou ultimato).
    pub return_on_warning: bool,
    pub sales: HolidaySales,
    /// Vender só os da lista de transferências (os outros, recusados).
    pub listed_only: bool,
    /// O adjunto escala o time com as táticas atuais (formação, instruções).
    pub use_tactics: bool,
    /// O time escolhido pelo treinador, com o adjunto a trocar só os indisponíveis.
    pub use_team: bool,
    /// Voltar depois de tantos dias...
    pub days: Option<i32>,
    /// ...ou nesse dia ("AAAA-MM-DD"); sem nenhum dos dois, férias indefinidas.
    pub until: Option<String>,
}

impl Default for HolidayPlan {
    fn default() -> Self {
        HolidayPlan {
            apply_jobs: false,
            return_on_job: true,
            return_on_warning: true,
            sales: HolidaySales::Asking,
            listed_only: false,
            use_tactics: true,
            use_team: false,
            days: None,
            until: None,
        }
    }
}

/// Um jogo do clube durante as férias.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct HolidayGame {
    pub date: String,
    pub competition: String,
    pub opponent: String,
    /// "C" em casa, "F" fora.
    pub venue: String,
    /// "2-1", do lado do clube.
    pub score: String,
    /// "V", "E" ou "D".
    pub outcome: String,
}

/// As férias a decorrer, para o diálogo "De Férias".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct HolidayView {
    pub date: String,
    /// `None` = férias indefinidas.
    pub until: Option<String>,
    /// Dias já passados.
    pub days: i32,
    pub games: Vec<HolidayGame>,
    /// O que o adjunto fez (propostas, candidaturas).
    pub notes: Vec<String>,
    /// Porque as férias acabaram antes ("Recebeu uma proposta de emprego do Santos.").
    pub stop: Option<String>,
    /// Acabaram (pela data ou por um evento): falta só voltar.
    pub done: bool,
}

/// As férias em curso (só na memória: gravar a meio grava o jogo sem elas).
pub(super) struct Holiday {
    plan: HolidayPlan,
    start: String,
    until: Option<String>,
    watch: bool,
    selection: Option<Selection>,
    sacked: bool,
    warning: i32,
    jobs: HashSet<i32>,
    /// Jogos do clube já jogados no dia da partida (não contam).
    before: HashSet<i32>,
    notes: Vec<String>,
    stop: Option<String>,
    done: bool,
}

impl Game {
    /// Começa as férias com as opções do diálogo.
    pub fn start_holiday(&mut self, plan: &HolidayPlan) -> Result<HolidayView> {
        if self.holiday.is_some() {
            return Err(Error::Invalid("Você já está de férias.".into()));
        }
        if self.live.is_some() {
            return Err(Error::Invalid("Você não pode sair de férias no dia de um jogo.".into()));
        }
        let until = match (&plan.until, plan.days) {
            (Some(d), _) => {
                if date::parse(d).is_none() || d.as_str() <= self.date.as_str() {
                    return Err(Error::Invalid("Escolha um dia de volta depois de hoje.".into()));
                }
                let last = date::add_days(&self.date, MAX_HOLIDAY).unwrap_or_default();
                if d.as_str() > last.as_str() {
                    return Err(Error::Invalid(format!("Férias de no máximo {MAX_HOLIDAY} dias.")));
                }
                Some(d.clone())
            }
            (None, Some(days)) => {
                if !(1..=MAX_HOLIDAY).contains(&days) {
                    return Err(Error::Invalid(format!("Férias de 1 a {MAX_HOLIDAY} dias.")));
                }
                date::add_days(&self.date, days)
            }
            (None, None) => None,
        };
        let selection = self.selection.clone();
        // O time durante as férias: o escolhido, só as táticas, ou tudo do adjunto
        if !plan.use_team {
            if plan.use_tactics {
                if let Some(sel) = &mut self.selection {
                    sel.auto = true;
                }
            } else {
                self.selection = None;
            }
        }
        let before = self
            .matches
            .iter()
            .filter(|m| m.date == self.date && m.result.is_some() && (m.home_id == self.club_id || m.away_id == self.club_id))
            .map(|m| m.id)
            .collect();
        self.holiday = Some(Holiday {
            plan: plan.clone(),
            start: self.date.clone(),
            until,
            watch: self.watch,
            selection,
            sacked: self.sacked(),
            warning: self.board_warning()?,
            jobs: self.pending_jobs()?,
            before,
            notes: Vec::new(),
            stop: None,
            done: false,
        });
        self.watch = false;
        self.unsaved = true;
        self.holiday_view()
    }

    /// Passa um dia de férias: o adjunto trata das propostas e das candidaturas e o dia corre
    /// sem o treinador. Pára na data de volta, ao ser despedido ou num evento das opções.
    pub fn holiday_day(&mut self) -> Result<HolidayView> {
        let Some(h) = &self.holiday else { return Err(Error::Invalid("Você não está de férias.".into())) };
        if h.done {
            return self.holiday_view();
        }
        let plan = h.plan.clone();
        let mut notes = Vec::new();
        if !self.sacked() {
            self.holiday_offers(&plan, &mut notes)?;
        }
        if plan.apply_jobs && date::weekday(&self.date) == Some(1) {
            self.holiday_jobs(&mut notes)?;
        }
        self.advance_day()?;
        let sacked = self.sacked();
        let warning = self.board_warning()?;
        let jobs = self.pending_jobs()?;
        let mut stop = None;
        let h = self.holiday.as_ref().expect("de férias");
        if sacked && !h.sacked {
            stop = Some("Você foi demitido pela Diretoria.".to_string());
        } else if plan.return_on_warning && warning > h.warning {
            stop = Some("A Diretoria está descontente e quer falar com você.".to_string());
        } else if plan.return_on_job
            && let Some(&id) = jobs.iter().find(|id| !h.jobs.contains(id))
        {
            let club: i32 = self.world.conn.query_row("SELECT buyer FROM offers WHERE id = ?", [id], |r| r.get(0))?;
            stop = Some(format!("Você recebeu uma proposta de emprego do {}.", self.club(club).name));
        }
        let reached = h.until.as_ref().is_some_and(|u| self.date.as_str() >= u.as_str());
        let h = self.holiday.as_mut().expect("de férias");
        h.notes.extend(notes);
        h.warning = warning;
        h.jobs = jobs;
        h.done = stop.is_some() || reached;
        h.stop = stop;
        self.holiday_view()
    }

    /// Volta das férias (no fim ou pelo botão): a escolha do treinador volta e chega o resumo.
    pub fn end_holiday(&mut self) -> Result<GameInfo> {
        let view = self.holiday_view()?;
        let h = self.holiday.take().ok_or_else(|| Error::Invalid("Você não está de férias.".into()))?;
        self.watch = h.watch;
        // A escolha do treinador volta (sem os que saíram entretanto)
        if !self.sacked() {
            self.selection = h.selection.clone();
            self.clean_selection()?;
        }
        let body = self.holiday_summary(&h, &view)?;
        self.post_news_as("message", "De volta das férias".into(), body);
        self.unsaved = true;
        self.info()
    }

    /// Férias de uma vez, até ao fim (a CLI e os testes).
    pub fn holiday(&mut self, plan: &HolidayPlan) -> Result<GameInfo> {
        let mut plan = plan.clone();
        if plan.days.is_none() && plan.until.is_none() {
            plan.days = Some(MAX_HOLIDAY);
        }
        self.start_holiday(&plan)?;
        while !self.holiday_day()?.done {}
        self.end_holiday()
    }

    pub fn holiday_view(&self) -> Result<HolidayView> {
        let h = self.holiday.as_ref().ok_or_else(|| Error::Invalid("Você não está de férias.".into()))?;
        let days = date::parse(&self.date).zip(date::parse(&h.start)).map_or(0, |(a, b)| a - b);
        Ok(HolidayView {
            date: self.date.clone(),
            until: h.until.clone(),
            days,
            games: self.holiday_games(h),
            notes: h.notes.clone(),
            stop: h.stop.clone(),
            done: h.done,
        })
    }

    fn board_warning(&self) -> Result<i32> {
        Ok(self.world.conn.query_row("SELECT COALESCE(MAX(warning), 0) FROM board", [], |r| r.get(0))?)
    }

    fn pending_jobs(&self) -> Result<HashSet<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT id FROM offers WHERE kind = 'job' AND status = 'pending'")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// O adjunto responde às propostas pelos jogadores do plantel, como pedem as opções.
    fn holiday_offers(&mut self, plan: &HolidayPlan, notes: &mut Vec<String>) -> Result<()> {
        let offers = self.offers_where(
            "seller = ?1 AND buyer <> ?1 AND kind IN ('transfer', 'loan') AND status IN ('pending', 'countered', 'confirm')",
            params![self.club_id],
        )?;
        for o in offers {
            let Some(f) = self.facts(o.person)? else { continue };
            let listed: bool =
                self.world.conn.query_row("SELECT COUNT(*) FROM transfer_list WHERE person_id = ? AND transfer = 1", [o.person], |r| {
                    r.get::<_, i32>(0)
                })? > 0;
            let sell = match plan.sales {
                _ if plan.listed_only && !listed => false,
                HolidaySales::Accept => true,
                HolidaySales::Reject => false,
                HolidaySales::Asking => o.kind == "transfer" && o.fee >= self.asking_price(o.person, &f, o.buyer)?,
            };
            let name = self.person_name(o.person)?;
            let buyer = self.club(o.buyer).name;
            let (key, note) = match (o.status.as_str(), sell) {
                ("confirm", true) => ("confirm", format!("Vendeu {name} ao {buyer} por {}.", money_text(o.fee))),
                ("confirm", false) => continue,
                (_, true) => ("accept", format!("Aceitou a proposta do {buyer} por {name} ({}).", money_text(o.fee))),
                (_, false) => ("reject", format!("Recusou a proposta do {buyer} por {name}.")),
            };
            // Uma resposta que já não cabe (janela fechada, cláusula paga) fica para o treinador
            if self.answer_offer(o.id, key, None).is_ok() {
                notes.push(note);
            }
        }
        Ok(())
    }

    /// À segunda-feira, concorre aos lugares vagos de clubes com mais reputação que o seu.
    fn holiday_jobs(&mut self, notes: &mut Vec<String>) -> Result<()> {
        let own = self.club_rep_of(self.club_id)?;
        for club in self.managed_clubs()? {
            if club == self.club_id || self.coach_of(club)?.is_some() || self.club_rep_of(club)? <= own {
                continue;
            }
            if self.apply_job(club).is_ok() {
                notes.push(format!("Você se candidatou à vaga de treinador do {}.", self.club(club).name));
            }
        }
        Ok(())
    }

    fn holiday_games(&self, h: &Holiday) -> Vec<HolidayGame> {
        self.matches
            .iter()
            .filter(|m| {
                m.date.as_str() >= h.start.as_str() && !h.before.contains(&m.id) && (m.home_id == self.club_id || m.away_id == self.club_id)
            })
            .filter_map(|m| {
                let (hg, ag) = m.result?;
                let home = m.home_id == self.club_id;
                let (us, them) = if home { (hg, ag) } else { (ag, hg) };
                let pens = m.pens.map(|(a, b)| if home { (a, b) } else { (b, a) });
                let outcome = match us.cmp(&them) {
                    std::cmp::Ordering::Greater => "V",
                    std::cmp::Ordering::Less => "D",
                    _ => match pens {
                        Some((a, b)) if a > b => "V",
                        Some(_) => "D",
                        None => "E",
                    },
                };
                Some(HolidayGame {
                    date: m.date.clone(),
                    competition: self.competitions.get(&m.competition_id).cloned().unwrap_or_default(),
                    opponent: self.club(if home { m.away_id } else { m.home_id }).name,
                    venue: if home { "C" } else { "F" }.into(),
                    score: format!("{us}-{them}"),
                    outcome: outcome.into(),
                })
            })
            .collect()
    }

    /// A mensagem "De volta das férias": os jogos, a liga, os lesionados, as transferências e
    /// o que o adjunto fez.
    fn holiday_summary(&self, h: &Holiday, view: &HolidayView) -> Result<String> {
        let mut parts = vec![match &h.stop {
            Some(why) => format!("Você voltou das férias em {}, depois de {} dias. {why}", cm_date(&self.date), view.days),
            None => format!("Você voltou das férias em {}, depois de {} dias.", cm_date(&self.date), view.days),
        }];
        if view.games.is_empty() {
            parts.push("O time não jogou durante as férias.".into());
        } else {
            let count = |o: &str| view.games.iter().filter(|g| g.outcome == o).count();
            let (gf, ga) = view.games.iter().fold((0, 0), |(f, a), g| {
                let mut s = g.score.split('-').map(|x| x.parse::<i32>().unwrap_or(0));
                (f + s.next().unwrap_or(0), a + s.next().unwrap_or(0))
            });
            let mut text = format!(
                "Jogos: {} vitórias, {} empates e {} derrotas; {gf} gols marcados e {ga} sofridos.",
                count("V"),
                count("E"),
                count("D")
            );
            for g in &view.games {
                text.push_str(&format!("\n{} {}: {} ({}) {} {}", cm_date(&g.date), g.competition, g.opponent, g.venue, g.score, g.outcome));
            }
            parts.push(text);
        }
        if !self.sacked() {
            let division: Option<i32> =
                self.world.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [self.club_id], |r| r.get(0))?;
            if let Some(div) = division
                && let Some(row) = self.table(div).into_iter().find(|r| r.club.id == self.club_id && r.played > 0)
            {
                let comp = self.competitions.get(&div).cloned().unwrap_or_default();
                parts.push(format!("{comp}: {}º lugar, {} pontos em {} jogos.", row.position, row.points, row.played));
            }
            let mut injured: Vec<String> = Vec::new();
            for (&id, s) in &self.squads.states {
                if self.squads.traits.get(&id).is_some_and(|t| t.club_id == self.club_id)
                    && let Some(until) = s.injury_until.as_deref().filter(|u| *u > self.date.as_str())
                {
                    injured.push(format!("{} (até {})", self.person_name(id)?, cm_date(until)));
                }
            }
            injured.sort();
            if !injured.is_empty() {
                parts.push(format!("Lesionados: {}.", injured.join(", ")));
            }
        }
        let moves: Vec<(i32, Option<i32>, Option<i32>, i64)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT person_id, from_club, to_club, fee FROM transfers
                 WHERE date >= ?1 AND (from_club = ?2 OR to_club = ?2) ORDER BY date, id",
            )?
            .query_map(params![h.start, self.club_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        if !moves.is_empty() {
            let mut text = "Transferências:".to_string();
            for (person, from, to, fee) in moves {
                let name = self.person_name(person)?;
                let fee = if fee > 0 { format!(" ({})", money_text(fee)) } else { String::new() };
                let line = if to == Some(self.club_id) {
                    format!("{name} chegou do {}{fee}", from.map_or("sem clube".into(), |c| self.club(c).name))
                } else {
                    format!("{name} saiu para o {}{fee}", to.map_or("sem clube".into(), |c| self.club(c).name))
                };
                text.push_str(&format!("\n{line}"));
            }
            parts.push(text);
        }
        if !h.notes.is_empty() {
            parts.push(format!("O que o adjunto fez:\n{}", h.notes.join("\n")));
        }
        Ok(parts.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(tag: &str) -> Option<(Game, std::path::PathBuf)> {
        let path = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))?;
        let dir = std::env::temp_dir().join(format!("nextcm-ferias-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Férias".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        Some((Game::create(&path, &dir, &new, 23).unwrap(), dir))
    }

    #[test]
    fn holiday_days_return_button_and_summary() {
        let Some((mut g, dir)) = game("dias") else { return };
        g.set_watch(true);
        let start = g.date.clone();
        // Data no passado e duração fora dos limites
        assert!(g.start_holiday(&HolidayPlan { until: Some(start.clone()), ..Default::default() }).is_err());
        assert!(g.start_holiday(&HolidayPlan { days: Some(0), ..Default::default() }).is_err());
        // Férias indefinidas: voltar a meio pelo botão
        let v = g.start_holiday(&HolidayPlan { use_team: true, ..Default::default() }).unwrap();
        assert!(v.until.is_none() && !v.done && g.info().unwrap().holiday);
        let mut v = v;
        for _ in 0..40 {
            v = g.holiday_day().unwrap();
            if v.done {
                break;
            }
        }
        assert!(v.days > 0);
        let info = g.end_holiday().unwrap();
        assert!(!info.holiday && g.watch, "volta a ver os jogos");
        let msg = &g.news[0];
        assert_eq!(msg.title, "De volta das férias");
        assert!(msg.body.contains("depois de"), "{}", msg.body);
        if !v.games.is_empty() {
            assert!(msg.body.contains("Jogos:"), "{}", msg.body);
        }
        assert!(g.holiday_day().is_err(), "já voltou");
        // Com data: pára no dia e devolve a escolha do treinador
        let sel = g.selection.clone();
        let back = date::add_days(&g.date, 10).unwrap();
        g.start_holiday(&HolidayPlan { until: Some(back.clone()), use_tactics: false, ..Default::default() }).unwrap();
        assert!(g.selection.is_none(), "o adjunto escolhe tudo");
        let mut n = 0;
        while !g.holiday_day().unwrap().done {
            n += 1;
            assert!(n < 20);
        }
        assert!(g.date >= back || g.holiday_view().unwrap().stop.is_some());
        g.end_holiday().unwrap();
        assert_eq!(g.selection.is_some(), sel.is_some());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn holiday_assistant_answers_offers() {
        let Some((mut g, dir)) = game("propostas") else { return };
        // Um jogador do plantel com duas propostas: uma ao valor exigido, outra muito abaixo
        let ids: Vec<i32> = g
            .world
            .conn
            .prepare("SELECT id FROM people WHERE club_id = 7568 AND is_player = 1 ORDER BY id LIMIT 2")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let rich = 7570;
        let ask = {
            let f = g.facts(ids[0]).unwrap().unwrap();
            g.asking_price(ids[0], &f, rich).unwrap()
        };
        let answer = date::add_days(&g.date, 3).unwrap();
        for (person, fee) in [(ids[0], ask * 2), (ids[1], 10_000)] {
            g.world
                .conn
                .execute(
                    "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('transfer', ?, ?, 7568, ?, 'pending', ?, ?)",
                    params![person, rich, fee, answer, g.date],
                )
                .unwrap();
        }
        g.start_holiday(&HolidayPlan { days: Some(1), return_on_job: false, return_on_warning: false, ..Default::default() }).unwrap();
        // O primeiro dia pode acabar o de hoje (começado de manhã)
        let mut v = g.holiday_day().unwrap();
        while !v.done {
            v = g.holiday_day().unwrap();
        }
        let status = |p: i32| -> String {
            g.world
                .conn
                .query_row("SELECT status FROM offers WHERE person_id = ? AND buyer = ? ORDER BY id DESC", params![p, rich], |r| r.get(0))
                .unwrap()
        };
        assert_ne!(status(ids[0]), "pending", "a boa foi respondida");
        assert_ne!(status(ids[0]), "rejected");
        assert_eq!(status(ids[1]), "rejected", "a baixa foi recusada");
        assert!(v.notes.iter().any(|n| n.starts_with("Recusou")), "{:?}", v.notes);
        g.end_holiday().unwrap();
        assert!(g.news[0].body.contains("O que o adjunto fez"), "{}", g.news[0].body);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
