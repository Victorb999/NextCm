//! Jogos que batem (marco 4.6): um jogo novo (uma ronda de copa, uma fase) a menos de 3 dias de
//! outro jogo de um dos clubes empurra o de menos prioridade (liga < copa < continental) para a
//! primeira data livre, como no CM: o clube fica com jogos a menos na tabela até lá.

use std::collections::{HashMap, HashSet};

use super::{Game, NewsItem, cm_date};
use crate::regulation;
use crate::{Result, date};

/// Dias entre dois jogos do mesmo clube: no mínimo 3 (sábado e terça).
const GAP: i32 = 2;
/// Até quantos dias se procura uma data nova.
const SEARCH: i32 = 70;

impl Game {
    /// Quem cede quando dois jogos batem: amigáveis 0, ligas 1, copas 2, continentais 3.
    fn clash_priority(&self, comp: i32) -> u8 {
        if comp == regulation::CLUB_FRIENDLY {
            return 0;
        }
        match regulation::get(comp) {
            Some(r) if self.comp_nations.contains_key(&comp) => {
                if r.is_cup() {
                    2
                } else {
                    1
                }
            }
            Some(_) => 3,
            None => 1,
        }
    }

    /// Os jogos de clubes (não de selecções) por clube: (dia, índice).
    fn club_days(&self) -> HashMap<i32, Vec<(i32, usize)>> {
        let mut out: HashMap<i32, Vec<(i32, usize)>> = HashMap::new();
        for (i, m) in self.matches.iter().enumerate() {
            if regulation::season_kind(m.competition_id) == regulation::SeasonKind::International {
                continue;
            }
            let Some(d) = date::parse(&m.date) else { continue };
            for c in [m.home_id, m.away_id] {
                out.entry(c).or_default().push((d, i));
            }
        }
        out
    }

    /// Os dias de jogos de selecções (as datas FIFA): os clubes não jogam perto deles.
    fn national_days(&self) -> HashSet<i32> {
        self.matches
            .iter()
            .filter(|m| regulation::season_kind(m.competition_id) == regulation::SeasonKind::International)
            .filter_map(|m| date::parse(&m.date))
            .collect()
    }

    /// Resolve os choques dos jogos a partir de `from` (os acabados de criar).
    pub(super) fn settle_clashes(&mut self, from: usize, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if from >= self.matches.len() {
            return Ok(());
        }
        let mut days = self.club_days();
        let national = self.national_days();
        let today = date::parse(&self.date).unwrap_or(0);
        for i in from..self.matches.len() {
            // Um jogo pode bater com mais de um
            for _ in 0..4 {
                let m = &self.matches[i];
                if m.result.is_some() || regulation::season_kind(m.competition_id) == regulation::SeasonKind::International {
                    break;
                }
                let Some(d) = date::parse(&m.date) else { break };
                let other = [m.home_id, m.away_id].iter().find_map(|c| {
                    days.get(c)?.iter().find(|&&(x, j)| {
                        j != i
                            && (x - d).abs() <= GAP
                            && self.matches[j].competition_id != m.competition_id
                            && self.matches[j].result.is_none()
                    })
                });
                let Some(&(_, j)) = other else { break };
                let (pi, pj) = (self.clash_priority(m.competition_id), self.clash_priority(self.matches[j].competition_id));
                // Cede o de menos prioridade; empatados, o novo. Nunca um de hoje ou de antes.
                let movable = |k: usize| date::parse(&self.matches[k].date).is_some_and(|x| x > today);
                let (first, second) = if pj < pi { (j, i) } else { (i, j) };
                // Sem data livre para o que devia ceder, cede o outro
                let found = [first, second]
                    .into_iter()
                    .filter(|&k| movable(k))
                    .find_map(|k| self.clash_free_day(k, &days, &national, today).map(|d| (k, d)));
                let Some((mover, new)) = found else { break };
                let cause = if mover == j { self.matches[i].competition_id } else { self.matches[j].competition_id };
                let old = self.matches[mover].date.clone();
                let new_iso = date::format(new);
                for c in [self.matches[mover].home_id, self.matches[mover].away_id] {
                    if let Some(v) = days.get_mut(&c) {
                        for e in v.iter_mut().filter(|e| e.1 == mover) {
                            e.0 = new;
                        }
                    }
                }
                self.matches[mover].date = new_iso.clone();
                let mv = &self.matches[mover];
                if mv.home_id == self.club_id || mv.away_id == self.club_id {
                    let rival = self.club(if mv.home_id == self.club_id { mv.away_id } else { mv.home_id }).short_name;
                    let body = format!(
                        "O jogo com o {rival} {} passa de {} para {}, por causa {}.",
                        self.of_competition(mv.competition_id),
                        cm_date(&old),
                        cm_date(&new_iso),
                        self.of_competition(cause)
                    );
                    self.push_news(fresh, next_id, "competition", "Jogo adiado".into(), body);
                }
            }
        }
        Ok(())
    }

    /// Uma data nova para o jogo `k`: a primeira quarta (depois terça ou quinta, depois qualquer
    /// dia) sem jogos dos dois clubes a menos de 3 dias e longe das datas FIFA, até ao fim da
    /// competição (na eliminatória, entre as mãos e antes da ronda seguinte); se não houver depois, antes.
    fn clash_free_day(&self, k: usize, days: &HashMap<i32, Vec<(i32, usize)>>, national: &HashSet<i32>, today: i32) -> Option<i32> {
        let m = &self.matches[k];
        let d0 = date::parse(&m.date)?;
        let season = regulation::season_of(m.competition_id, &m.date);
        let same = || {
            self.matches
                .iter()
                .filter(|x| x.competition_id == m.competition_id && regulation::season_of(x.competition_id, &x.date) == season)
        };
        let mut hi = same().filter_map(|x| date::parse(&x.date)).max().unwrap_or(d0).max(d0);
        let mut lo = today + 1;
        if let Some(tie) = m.tie {
            // Uma ronda de copa fica antes da seguinte (a final, até uma semana depois)
            let kind = regulation::season_kind(m.competition_id);
            let first = |spec: &str| regulation::dates_in(season, spec, kind).first().and_then(|d| date::parse(d));
            let next = regulation::get(m.competition_id).and_then(|r| {
                let st = r.stages.get(m.stage as usize)?;
                match st.rounds.get(m.round as usize) {
                    Some(round) => first(&round.dates),
                    // A última ronda da fase: antes da fase seguinte
                    None => r.stages.get(m.stage as usize + 1).and_then(|n| first(n.rounds.first().map_or(&n.dates, |x| &x.dates))),
                }
            });
            hi = next.map_or(d0 + 7, |n| n - 1 - GAP);
            lo = lo.max(d0 - 7);
            // Na eliminatória de duas mãos, a ordem das mãos
            for x in same().filter(|x| x.stage == m.stage && x.round == m.round && x.tie == Some(tie) && x.id != m.id) {
                let Some(xd) = date::parse(&x.date) else { continue };
                if x.leg > m.leg {
                    hi = hi.min(xd - 1 - GAP);
                } else {
                    lo = lo.max(xd + 1 + GAP);
                }
            }
        }
        let free = |day: i32, gap: i32| {
            day >= lo
                && day <= hi
                && !(day - 3..=day + 1).any(|x| national.contains(&x))
                && [m.home_id, m.away_id]
                    .iter()
                    .all(|c| days.get(c).is_none_or(|v| v.iter().all(|&(x, j)| j == k || (x - day).abs() > gap)))
        };
        let weekday = |day: i32| (day + 4).rem_euclid(7);
        let passes: [&dyn Fn(i32) -> bool; 3] = [&|day| weekday(day) == 3, &|day| matches!(weekday(day), 2 | 4), &|_| true];
        // Sem 2 dias de folga para ninguém (o calendário cheio dos estaduais), só 1
        for gap in [GAP, 1] {
            for pass in passes {
                if let Some(day) = (1..=SEARCH).map(|o| d0 + o).find(|&day| pass(day) && free(day, gap)) {
                    return Some(day);
                }
                if let Some(day) = (1..=SEARCH).map(|o| d0 - o).find(|&day| pass(day) && free(day, gap)) {
                    return Some(day);
                }
            }
        }
        None
    }
}

impl Game {
    /// Os pares de jogos de um clube em dias seguidos ou no mesmo dia (para a CLI `sim` e os testes).
    pub fn fixture_clashes(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (club, v) in self.club_days() {
            let mut v = v.clone();
            v.sort_unstable();
            for w in v.windows(2) {
                let (a, b) = (&self.matches[w[0].1], &self.matches[w[1].1]);
                if w[1].0 - w[0].0 <= 1 && a.competition_id != b.competition_id {
                    out.push(format!(
                        "{}: {} {} / {} {}",
                        self.club(club).short_name,
                        a.date,
                        self.competition_name(a.competition_id),
                        b.date,
                        self.competition_name(b.competition_id)
                    ));
                }
            }
        }
        out.sort();
        out
    }
}
