//! Competições vivas (marco 3.6): a partir dos regulamentos (`regulation.rs`), quem entra,
//! os sorteios (grupos e eliminatórias), os calendários de cada fase, o desempate por
//! penáltis, a passagem de fase em fase e os campeões. Tudo sorteado com a seed do jogo,
//! a época e a competição, então a mesma seed dá as mesmas competições.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use serde::Serialize;
use ts_rs::TS;

use super::{Game, NewsItem, cm_date};
use crate::engine::{self, uniform};
use crate::league::{self, Fixture, Match, SERIE_A, SERIE_B, SERIE_C, SERIE_D, StandingRow};
use crate::person::ClubRef;
use crate::regulation::{self, ContinentPick, GroupDraw, Pairing, Pick, Regulation, SeasonKind, SingleHome, StageKind, TRANSITION_YEAR};
use crate::{Error, Result};

/// Uma tabela de uma fase: a classificação toda ou um grupo.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct GroupTable {
    /// "" numa classificação única, "Grupo A"...
    pub name: String,
    pub rows: Vec<StandingRow>,
}

/// Uma eliminatória: os jogos (um ou dois) e quem passou.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TieView {
    /// O clube da casa no primeiro jogo e o outro.
    pub first: ClubRef,
    pub second: ClubRef,
    pub games: Vec<Fixture>,
    /// Golos no total, do ponto de vista de `first`.
    pub aggregate: Option<(i32, i32)>,
    /// Penáltis, do ponto de vista de `first`.
    pub pens: Option<(i32, i32)>,
    pub winner: Option<i32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct KoRoundView {
    pub name: String,
    pub ties: Vec<TieView>,
}

/// Uma fase de uma competição, como a tela Classificação a mostra.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct StageView {
    pub name: String,
    /// "league", "groups" ou "knockout"
    pub kind: String,
    pub tables: Vec<GroupTable>,
    pub rounds: Vec<KoRoundView>,
}

/// Mando dos jogos de uma fase de liga: cada clube joga um número par de jogos, então
/// orientar cada jogo no sentido em que um circuito de Euler (Hierholzer) o percorre dá a
/// todos metade em casa e metade fora.
fn orient_evenly(days: &mut [Vec<(i32, i32)>]) {
    let games: Vec<(usize, usize)> = days.iter().enumerate().flat_map(|(d, g)| (0..g.len()).map(move |i| (d, i))).collect();
    let mut adjacency: HashMap<i32, Vec<usize>> = HashMap::new();
    let mut clubs = Vec::new();
    for (e, &(d, i)) in games.iter().enumerate() {
        let (a, b) = days[d][i];
        for c in [a, b] {
            if !adjacency.contains_key(&c) {
                clubs.push(c);
            }
            adjacency.entry(c).or_default().push(e);
        }
    }
    let mut used = vec![false; games.len()];
    let mut next: HashMap<i32, usize> = HashMap::new();
    for start in clubs {
        let mut stack = vec![start];
        while let Some(&v) = stack.last() {
            let edges = &adjacency[&v];
            let k = next.entry(v).or_default();
            while *k < edges.len() && used[edges[*k]] {
                *k += 1;
            }
            if *k == edges.len() {
                stack.pop();
                continue;
            }
            let e = edges[*k];
            used[e] = true;
            let (d, i) = games[e];
            let (a, b) = days[d][i];
            let w = if a == v { b } else { a };
            days[d][i] = (v, w); // percorrido de v para w: v em casa
            stack.push(w);
        }
    }
}

/// Ordem das posições num quadro de `n` (potência de 2): 1, 8, 4, 5, 2, 7, 3, 6 para 8.
fn bracket_order(n: usize) -> Vec<usize> {
    let mut order = vec![1];
    while order.len() < n {
        let size = order.len() * 2;
        order = order.iter().flat_map(|&s| [s, size + 1 - s]).collect();
    }
    order
}

fn shuffle<T>(v: &mut [T], rng: &mut impl RngCore) {
    for i in (1..v.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

fn group_name(g: u8) -> String {
    format!("Grupo {}", (b'A' + g) as char)
}

/// Divisão de um clube para "o mais fraco joga em casa": A = 0 ... D = 3, outras 4.
fn division_rank(division: Option<i32>) -> i32 {
    match division {
        Some(SERIE_A) => 0,
        Some(SERIE_B) => 1,
        Some(SERIE_C) => 2,
        Some(SERIE_D) => 3,
        _ => 4,
    }
}

impl Game {
    fn comp_rng(&self, comp: i32, year: i32, tag: u64) -> ChaCha8Rng {
        let mix = (self.seed as u64)
            ^ ((year as u64) << 40)
            ^ (comp as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ tag.wrapping_mul(0xD1B5_4A32_D192_ED03);
        ChaCha8Rng::seed_from_u64(mix)
    }

    /// Jogos de uma competição numa época (o ano, ou o ano em que começa numa época europeia).
    pub(super) fn year_matches(&self, comp: i32, year: i32) -> impl Iterator<Item = &Match> {
        self.matches.iter().filter(move |m| m.competition_id == comp && regulation::season_of(comp, &m.date) == year)
    }

    /// A época em curso de uma competição.
    pub(super) fn current_season(&self, comp: i32) -> i32 {
        regulation::season_of(comp, &self.date)
    }

    fn next_match_id(&self) -> i32 {
        self.matches.iter().map(|m| m.id).max().unwrap_or(0) + 1
    }

    /// Sorteia as competições de um tipo de época que começa em `year` (as do ano civil a 1
    /// de janeiro, as europeias a 1 de julho; na criação do jogo, as da transição e a época
    /// europeia de 2025/26). As fases seguintes saem à medida que as anteriores acabam.
    pub(super) fn schedule_season(&mut self, kind: SeasonKind, year: i32, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        for reg in regulation::all() {
            if reg.season != kind || (kind == SeasonKind::Calendar && year == TRANSITION_YEAR && reg.transition_dates.is_none()) {
                continue;
            }
            self.create_stage(reg, 0, year, fresh, next_id)?;
        }
        Ok(())
    }

    fn clubs_where(&self, sql_where: &str, arg: i32) -> Result<Vec<i32>> {
        let sql = format!("SELECT id FROM clubs WHERE {sql_where} ORDER BY reputation DESC, id");
        Ok(self.world.conn.prepare_cached(&sql)?.query_map([arg], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
    }

    /// Clubes numa competição numa época.
    fn clubs_in(&self, comp: i32, year: i32) -> HashSet<i32> {
        self.year_matches(comp, year).flat_map(|m| [m.home_id, m.away_id]).collect()
    }

    /// Quem entra numa fase, do mais forte (ou mais bem classificado) para o mais fraco.
    fn entrants(&self, reg: &Regulation, stage: usize, year: i32) -> Result<Vec<i32>> {
        let e = &reg.stages[stage].entrants;
        let mut out: Vec<i32> = Vec::new();
        let add = |out: &mut Vec<i32>, ids: Vec<i32>| {
            for id in ids {
                if !out.contains(&id) {
                    out.push(id);
                }
            }
        };
        for &d in &e.divisions {
            add(&mut out, self.clubs_where("division_id = ?", d)?);
        }
        if let Some(b) = &e.best_of {
            let best: Vec<i32> =
                self.clubs_where("division_id = ?", b.division)?.into_iter().filter(|c| !out.contains(c)).take(b.count).collect();
            add(&mut out, best);
        }
        if e.estadual {
            add(&mut out, self.clubs_where("reserve_division_id = ? AND is_national_team = 0", reg.competition)?);
        }
        if let Some(p) = &e.previous {
            add(&mut out, self.pick(reg, p, stage.saturating_sub(1), year)?);
        }
        let excluded: HashSet<i32> = e.exclude.iter().flat_map(|&c| self.clubs_in(c, year)).collect();
        if let Some(l) = &e.league {
            let order = self.final_order(l.competition, year - 1)?;
            let mut pos = l.from.saturating_sub(1);
            let mut taken = 0;
            let free = |id: &i32, out: &Vec<i32>| !excluded.contains(id) && !out.contains(id);
            while taken < l.count && pos < order.len() {
                let id = order[pos];
                pos += 1;
                if free(&id, &out) {
                    out.push(id);
                    taken += 1;
                }
            }
            if let Some(cup) = e.cup_winner {
                match self.podium(cup, year - 1).map(|p| p.0) {
                    Some(w) if free(&w, &out) => out.push(w),
                    // Campeão já dentro (ou sem Copa): entra o seguinte da tabela
                    _ => {
                        while pos < order.len() {
                            let id = order[pos];
                            pos += 1;
                            if free(&id, &out) {
                                out.push(id);
                                break;
                            }
                        }
                    }
                }
            }
        }
        // Vagas de várias ligas pela classificação da época anterior (Liga dos Campeões, Liga Europa)
        for l in &e.leagues {
            let order = self.final_order(l.competition, year - 1)?;
            let picked: Vec<i32> = order
                .into_iter()
                .skip(l.from.saturating_sub(1))
                .filter(|id| !excluded.contains(id) && !out.contains(id))
                .take(l.count)
                .collect();
            add(&mut out, picked);
        }
        for (k, q) in e.foreign.iter().enumerate() {
            let drawn = self.foreign_draw(reg.competition, year, k as u64, &q.nation, q.count, &out, &excluded)?;
            add(&mut out, drawn);
        }
        if let Some(c) = &e.continent {
            let drawn = self.continent_draw(reg.competition, year, c, &out, &excluded)?;
            add(&mut out, drawn);
        }
        Ok(out)
    }

    /// As vagas das outras nações de um continente: sorteio pesado pela reputação (ao quadrado)
    /// entre os clubes mais fortes com plantel, no máximo `per_nation` por nação.
    fn continent_draw(&self, comp: i32, year: i32, c: &ContinentPick, taken: &[i32], excluded: &HashSet<i32>) -> Result<Vec<i32>> {
        let rows: Vec<(i32, i32, i32, String)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT c.id, COALESCE(c.reputation, 1), n.id, n.name FROM clubs c JOIN nations n ON n.id = c.nation_id
                 WHERE n.continent_id = ? AND c.is_national_team = 0
                   AND (SELECT COUNT(*) FROM people p WHERE p.club_id = c.id AND p.is_player = 1) >= 11
                 ORDER BY c.reputation DESC, c.id",
            )?
            .query_map([c.continent], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        // Os mais fortes de cada nação, e destes os 3 × `count` com mais reputação
        let mut per_nation: HashMap<i32, usize> = HashMap::new();
        let mut pool: Vec<(i32, i32, f64)> = Vec::new();
        for (id, rep, nation, name) in rows {
            if c.exclude_nations.contains(&name) || taken.contains(&id) || excluded.contains(&id) {
                continue;
            }
            let n = per_nation.entry(nation).or_default();
            if *n < c.per_nation + 1 {
                *n += 1;
                pool.push((id, nation, (rep as f64).powi(2)));
            }
        }
        pool.truncate(3 * c.count);
        let mut rng = self.comp_rng(comp, year, 0xC0_4714);
        let mut chosen: HashMap<i32, usize> = HashMap::new();
        let mut out = Vec::new();
        while out.len() < c.count {
            pool.retain(|p| chosen.get(&p.1).copied().unwrap_or(0) < c.per_nation);
            if pool.is_empty() {
                break;
            }
            let total: f64 = pool.iter().map(|p| p.2).sum();
            let mut roll = uniform(&mut rng) * total;
            let mut i = 0;
            while i + 1 < pool.len() && roll >= pool[i].2 {
                roll -= pool[i].2;
                i += 1;
            }
            let (id, nation, _) = pool.remove(i);
            *chosen.entry(nation).or_default() += 1;
            out.push(id);
        }
        Ok(out)
    }

    /// Clubes de uma nação para uma continental: sorteio pesado pela reputação (ao quadrado)
    /// entre os 8 mais fortes com plantel, sem os que já entraram.
    #[allow(clippy::too_many_arguments)]
    fn foreign_draw(
        &self,
        comp: i32,
        year: i32,
        tag: u64,
        nation: &str,
        count: usize,
        taken: &[i32],
        excluded: &HashSet<i32>,
    ) -> Result<Vec<i32>> {
        let pool: Vec<(i32, i32)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT c.id, COALESCE(c.reputation, 1) FROM clubs c JOIN nations n ON n.id = c.nation_id
                 WHERE n.name = ? AND c.is_national_team = 0
                   AND (SELECT COUNT(*) FROM people p WHERE p.club_id = c.id AND p.is_player = 1) >= 11
                 ORDER BY c.reputation DESC, c.id",
            )?
            .query_map([nation], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut pool: Vec<(i32, f64)> = pool
            .into_iter()
            .filter(|(id, _)| !taken.contains(id) && !excluded.contains(id))
            .take(8)
            .map(|(id, rep)| (id, (rep as f64).powi(2)))
            .collect();
        let mut rng = self.comp_rng(comp, year, 0xF0_0000 + tag);
        let mut out = Vec::new();
        while out.len() < count && !pool.is_empty() {
            let total: f64 = pool.iter().map(|p| p.1).sum();
            let mut roll = uniform(&mut rng) * total;
            let mut i = 0;
            while i + 1 < pool.len() && roll >= pool[i].1 {
                roll -= pool[i].1;
                i += 1;
            }
            out.push(pool.remove(i).0);
        }
        Ok(out)
    }

    /// Clubes de uma competição pela ordem final de uma época: a classificação da 1ª fase,
    /// ou (sem jogos nessa época) quem esteve nela pela última posição do banco (a época de
    /// 2024/25 no patch), e na falta disso a divisão atual.
    fn final_order(&self, comp: i32, year: i32) -> Result<Vec<i32>> {
        let table = self.stage_table(comp, year, 0, None);
        if !table.is_empty() {
            return Ok(table.iter().map(|r| r.club.id).collect());
        }
        let last: Vec<i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT id FROM clubs WHERE last_division_id = ? ORDER BY last_position IS NULL, last_position, reputation DESC, id",
            )?
            .query_map([comp], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if !last.is_empty() {
            return Ok(last);
        }
        Ok(self
            .world
            .conn
            .prepare_cached(
                "SELECT id FROM clubs WHERE division_id = ? ORDER BY last_position IS NULL, last_position, reputation DESC, id",
            )?
            .query_map([comp], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Clubes escolhidos pelos resultados de uma fase (a de `p.stage` ou `default_stage`).
    pub(super) fn pick(&self, reg: &Regulation, p: &Pick, default_stage: usize, year: i32) -> Result<Vec<i32>> {
        let comp = reg.competition;
        let stage = p.stage.unwrap_or(default_stage);
        if let Some(n) = p.top {
            return Ok(self.stage_table(comp, year, stage, None).iter().take(n).map(|r| r.club.id).collect());
        }
        if let Some(n) = p.bottom {
            return Ok(self.stage_table(comp, year, stage, None).iter().rev().take(n).map(|r| r.club.id).collect());
        }
        if let Some(n) = p.group_top {
            // 1º A, 1º B, ... 2º A, 2º B, ...
            let groups = self.group_count(comp, year, stage);
            let tables: Vec<Vec<StandingRow>> = (0..groups).map(|g| self.stage_table(comp, year, stage, Some(g))).collect();
            let mut out = Vec::new();
            for rank in 0..n {
                for t in &tables {
                    if let Some(r) = t.get(rank) {
                        out.push(r.club.id);
                    }
                }
            }
            return Ok(out);
        }
        if let Some(r) = p.round_winners {
            return Ok(self.round_winners(comp, year, stage, r as i32 + 1).unwrap_or_default());
        }
        Err(Error::Invalid(format!("regulamento da competição {comp}: escolha de clubes vazia")))
    }

    fn group_count(&self, comp: i32, year: i32, stage: usize) -> u8 {
        self.year_matches(comp, year).filter(|m| m.stage as usize == stage).filter_map(|m| m.group).max().map_or(0, |g| g + 1)
    }

    /// Classificação de uma fase de pontos corridos (ou de um grupo dela).
    pub(super) fn stage_table(&self, comp: i32, year: i32, stage: usize, group: Option<u8>) -> Vec<StandingRow> {
        let ms: Vec<Match> =
            self.year_matches(comp, year).filter(|m| m.stage as usize == stage && m.group == group && m.tie.is_none()).cloned().collect();
        if ms.is_empty() {
            return Vec::new();
        }
        let mut ids: Vec<i32> = ms.iter().flat_map(|m| [m.home_id, m.away_id]).collect();
        ids.sort_unstable();
        ids.dedup();
        let clubs: Vec<ClubRef> = ids.into_iter().map(|id| self.club(id)).collect();
        league::standings(&ms, &clubs)
    }

    /// Reputação e nação dos clubes (para os potes e para evitar a mesma nação num grupo).
    fn club_facts(&self, ids: &[i32]) -> Result<HashMap<i32, (i32, Option<i32>, Option<i32>)>> {
        let mut out = HashMap::new();
        let mut stmt = self.world.conn.prepare_cached("SELECT COALESCE(reputation, 0), nation_id, division_id FROM clubs WHERE id = ?")?;
        for &id in ids {
            out.insert(id, stmt.query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?);
        }
        Ok(out)
    }

    /// Fase de liga: `matchdays` jornadas tiradas de um calendário de todos contra todos (cada
    /// clube encontra adversários diferentes), escolhido entre várias tentativas o que tem menos
    /// jogos entre clubes da mesma nação; depois o mando fica em metade em casa e metade fora.
    fn league_phase(&self, entrants: &[i32], matchdays: usize, rng: &mut impl RngCore) -> Result<Vec<Vec<(i32, i32)>>> {
        let facts = self.club_facts(entrants)?;
        let n = entrants.len() - entrants.len() % 2;
        let mut best: Option<(usize, Vec<Vec<(i32, i32)>>)> = None;
        for _ in 0..40 {
            let mut t = entrants[..n].to_vec();
            shuffle(&mut t, rng);
            let mut rounds: Vec<Vec<(i32, i32)>> = Vec::with_capacity(n - 1);
            for r in 0..n - 1 {
                rounds.push((0..n / 2).map(|i| if (r + i) % 2 == 0 { (t[i], t[n - 1 - i]) } else { (t[n - 1 - i], t[i]) }).collect());
                t[1..].rotate_right(1);
            }
            let mut idx: Vec<usize> = (0..n - 1).collect();
            shuffle(&mut idx, rng);
            let chosen: Vec<Vec<(i32, i32)>> = idx.iter().take(matchdays).map(|&r| rounds[r].clone()).collect();
            let clashes = chosen.iter().flatten().filter(|(a, b)| facts[a].1.is_some() && facts[a].1 == facts[b].1).count();
            if best.as_ref().is_none_or(|b| clashes < b.0) {
                best = Some((clashes, chosen));
                if clashes == 0 {
                    break;
                }
            }
        }
        let mut days = best.map(|b| b.1).unwrap_or_default();
        orient_evenly(&mut days);
        Ok(days)
    }

    fn draw_groups(&self, entrants: &[i32], size: usize, draw: GroupDraw, rng: &mut impl RngCore) -> Result<Vec<Vec<i32>>> {
        let n_groups = (entrants.len() / size.max(2)).max(1);
        let mut groups: Vec<Vec<i32>> = vec![Vec::new(); n_groups];
        match draw {
            GroupDraw::Snake => {
                for (i, &c) in entrants.iter().enumerate() {
                    let (row, col) = (i / n_groups, i % n_groups);
                    groups[if row % 2 == 0 { col } else { n_groups - 1 - col }].push(c);
                }
            }
            GroupDraw::Pots => {
                let facts = self.club_facts(entrants)?;
                let mut sorted = entrants.to_vec();
                sorted.sort_by_key(|c| (Reverse(facts[c].0), *c));
                for (p, pot) in sorted.chunks(n_groups).enumerate() {
                    let mut pot = pot.to_vec();
                    shuffle(&mut pot, rng);
                    for c in pot {
                        let nation = facts[&c].1;
                        let open = |g: &Vec<i32>| g.len() <= p;
                        let pick = (0..n_groups)
                            .find(|&g| open(&groups[g]) && !groups[g].iter().any(|o| facts[o].1 == nation))
                            .or_else(|| (0..n_groups).find(|&g| open(&groups[g])))
                            .unwrap_or_else(|| (0..n_groups).min_by_key(|&g| groups[g].len()).unwrap_or(0));
                        groups[pick].push(c);
                    }
                }
            }
        }
        Ok(groups)
    }

    /// Cria os jogos de uma fase (numa eliminatória, a 1ª ronda).
    pub(super) fn create_stage(
        &mut self,
        reg: &Regulation,
        stage: usize,
        year: i32,
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
    ) -> Result<()> {
        let st = &reg.stages[stage];
        let comp = reg.competition;
        let entrants = self.entrants(reg, stage, year)?;
        if entrants.len() < 2 {
            return Ok(());
        }
        let mut rng = self.comp_rng(comp, year, stage as u64 + 1);
        let spec = match (&reg.transition_dates, stage, year) {
            (Some(t), 0, TRANSITION_YEAR) => t.as_str(),
            _ => st.dates.as_str(),
        };
        match st.kind {
            StageKind::League => {
                let dates = regulation::dates_in(year, spec, reg.season);
                let mut ms = league::round_robin(comp, &entrants, &dates, st.legs, self.next_match_id(), &mut rng)?;
                for m in &mut ms {
                    m.stage = stage as u8;
                }
                self.matches.extend(ms);
            }
            StageKind::Groups => {
                let groups = self.draw_groups(&entrants, st.group_size, st.draw, &mut rng)?;
                let dates = regulation::dates_in(year, spec, reg.season);
                for (g, clubs) in groups.iter().enumerate() {
                    let mut ms = league::round_robin(comp, clubs, &dates, st.legs, self.next_match_id(), &mut rng)?;
                    for m in &mut ms {
                        m.stage = stage as u8;
                        m.group = Some(g as u8);
                    }
                    self.matches.extend(ms);
                    if clubs.contains(&self.club_id) {
                        let others: Vec<String> = clubs.iter().filter(|&&c| c != self.club_id).map(|&c| self.club(c).short_name).collect();
                        let body = format!(
                            "{} {}: o {} fica no {} com {}.",
                            self.competition_name(comp),
                            regulation::season_label(comp, year),
                            self.club(self.club_id).short_name,
                            group_name(g as u8),
                            super::season::join_names(&others)
                        );
                        fresh.push(self.news_item(*next_id, "competition", format!("Sorteio {}", self.of_competition(comp)), body));
                        *next_id += 1;
                    }
                }
            }
            StageKind::LeaguePhase => {
                let dates = regulation::dates_in(year, spec, reg.season);
                let days = self.league_phase(&entrants, st.matchdays, &mut rng)?;
                let mut id = self.next_match_id();
                for (d, games) in days.iter().enumerate() {
                    for &(home_id, away_id) in games {
                        self.matches.push(Match {
                            id,
                            competition_id: comp,
                            stage: stage as u8,
                            round: d as i32 + 1,
                            date: dates[d].clone(),
                            home_id,
                            away_id,
                            ..Default::default()
                        });
                        id += 1;
                    }
                }
                if entrants.contains(&self.club_id) {
                    let mut rivals: Vec<String> = Vec::new();
                    for &(h, a) in days.iter().flatten() {
                        if h == self.club_id {
                            rivals.push(format!("{} (C)", self.club(a).short_name));
                        } else if a == self.club_id {
                            rivals.push(format!("{} (F)", self.club(h).short_name));
                        }
                    }
                    let body = format!(
                        "{} {}, {}: o {} joga com {}.",
                        self.competition_name(comp),
                        regulation::season_label(comp, year),
                        st.name,
                        self.club(self.club_id).short_name,
                        super::season::join_names(&rivals)
                    );
                    fresh.push(self.news_item(*next_id, "competition", format!("Sorteio {}", self.of_competition(comp)), body));
                    *next_id += 1;
                }
            }
            StageKind::Knockout => {
                // Os primeiros `byes` só entram na 2ª ronda
                let entrants = &entrants[st.byes.min(entrants.len())..];
                let n = entrants.len();
                let pairs: Vec<(i32, i32)> = match st.pairing {
                    Pairing::Bracket => {
                        let size = 1usize << (usize::BITS - 1 - n.leading_zeros());
                        bracket_order(size).chunks(2).map(|c| (entrants[c[0] - 1], entrants[c[1] - 1])).collect()
                    }
                    Pairing::Cross => {
                        let (firsts, seconds) = entrants.split_at(n / 2);
                        let g = firsts.len();
                        // 1º A - 2º B, 1º C - 2º D... e depois 1º B - 2º A, 1º D - 2º C...: os dois de um grupo em metades diferentes do quadro
                        let even = (0..g).step_by(2).filter(|&i| i + 1 < g).map(|i| (firsts[i], seconds[i + 1]));
                        let odd = (1..g).step_by(2).map(|i| (firsts[i], seconds[i - 1]));
                        even.chain(odd).collect()
                    }
                    Pairing::Pots => {
                        let (top, bottom) = entrants.split_at(n / 2);
                        let mut bottom = bottom.to_vec();
                        shuffle(&mut bottom, &mut rng);
                        top.iter().copied().zip(bottom).collect()
                    }
                };
                self.create_ko_round(reg, stage, 0, &pairs, year, fresh, next_id)?;
            }
        }
        Ok(())
    }

    /// O mais fraco de dois clubes: divisão mais baixa, senão menos reputação.
    fn weaker(&self, a: i32, b: i32) -> Result<i32> {
        let f = self.club_facts(&[a, b])?;
        let key = |c: i32| (division_rank(f[&c].2), Reverse(f[&c].0), c);
        Ok(if key(a) >= key(b) { a } else { b })
    }

    /// Cria uma ronda de eliminatórias. Em cada par `(a, b)`, `a` é o mais bem colocado:
    /// joga a 2ª mão em casa (ou o jogo único, se o regulamento não disser o contrário).
    #[allow(clippy::too_many_arguments)]
    fn create_ko_round(
        &mut self,
        reg: &Regulation,
        stage: usize,
        round: usize,
        pairs: &[(i32, i32)],
        year: i32,
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
    ) -> Result<()> {
        let st = &reg.stages[stage];
        let Some(r) = st.rounds.get(round) else { return Ok(()) };
        let dates = regulation::dates_in(year, &r.dates, reg.season);
        let comp = reg.competition;
        let mut id = self.next_match_id();
        for (t, &(a, b)) in pairs.iter().enumerate() {
            let base = Match {
                competition_id: comp,
                stage: stage as u8,
                round: round as i32 + 1,
                tie: Some(t as u16),
                neutral: r.neutral,
                ..Default::default()
            };
            let (home, away) = if r.legs >= 2 {
                self.matches.push(Match { id, leg: 1, date: dates[0].clone(), home_id: b, away_id: a, ..base.clone() });
                self.matches.push(Match { id: id + 1, leg: 2, date: dates[1].clone(), home_id: a, away_id: b, ..base });
                id += 2;
                (b, a)
            } else {
                let home = if !r.neutral && st.single_home == SingleHome::Weaker { self.weaker(a, b)? } else { a };
                let away = if home == a { b } else { a };
                self.matches.push(Match { id, leg: 0, date: dates[0].clone(), home_id: home, away_id: away, ..base });
                id += 1;
                (home, away)
            };
            if a == self.club_id || b == self.club_id {
                let when = if r.legs >= 2 {
                    format!("A 1ª mão é a {} em casa do {}; a 2ª a {}.", cm_date(&dates[0]), self.club(home).short_name, cm_date(&dates[1]))
                } else if r.neutral {
                    format!("Joga-se a {} em campo neutro.", cm_date(&dates[0]))
                } else {
                    format!("Joga-se a {} em casa do {}.", cm_date(&dates[0]), self.club(home).short_name)
                };
                let body = format!(
                    "{} {}, {}: {} - {}.\n\n{when}",
                    self.competition_name(comp),
                    regulation::season_label(comp, year),
                    r.name,
                    self.club(home).short_name,
                    self.club(away).short_name
                );
                fresh.push(self.news_item(*next_id, "competition", format!("Sorteio {}", self.of_competition(comp)), body));
                *next_id += 1;
            }
        }
        Ok(())
    }

    /// Os jogos de uma eliminatória, por mão.
    fn tie_games(&self, comp: i32, year: i32, stage: usize, round: i32, tie: u16) -> Vec<&Match> {
        let mut out: Vec<&Match> =
            self.year_matches(comp, year).filter(|m| m.stage as usize == stage && m.round == round && m.tie == Some(tie)).collect();
        out.sort_by_key(|m| m.leg);
        out
    }

    /// Golos no total dos clubes `x` e `y` numa eliminatória.
    fn aggregate(games: &[&Match], x: i32) -> Option<(i32, i32)> {
        let mut total = (0, 0);
        for m in games {
            let (h, a) = m.result?;
            let (gx, gy) = if m.home_id == x { (h, a) } else { (a, h) };
            total.0 += gx as i32;
            total.1 += gy as i32;
        }
        Some(total)
    }

    /// Quem passou numa eliminatória (golos no total e, empatado, penáltis).
    fn tie_winner(games: &[&Match]) -> Option<i32> {
        let last = games.last()?;
        let (x, y) = (last.home_id, last.away_id);
        let (gx, gy) = Self::aggregate(games, x)?;
        if gx != gy {
            return Some(if gx > gy { x } else { y });
        }
        let (px, py) = last.pens?;
        Some(if px > py { x } else { y })
    }

    /// Vencedores de uma ronda, pela ordem do quadro; `None` se ainda falta decidir alguma.
    fn round_winners(&self, comp: i32, year: i32, stage: usize, round: i32) -> Option<Vec<i32>> {
        let ties = self.year_matches(comp, year).filter(|m| m.stage as usize == stage && m.round == round).filter_map(|m| m.tie).max()?;
        (0..=ties).map(|t| Self::tie_winner(&self.tie_games(comp, year, stage, round, t))).collect()
    }

    /// Depois de um jogo de eliminatória: se acabou a eliminatória empatada, penáltis.
    pub(super) fn settle_tie(&mut self, index: usize) {
        let m = &self.matches[index];
        let (Some(tie), true) = (m.tie, m.leg != 1) else { return };
        let year = regulation::season_of(m.competition_id, &m.date);
        let games = self.tie_games(m.competition_id, year, m.stage as usize, m.round, tie);
        let Some((gx, gy)) = Self::aggregate(&games, m.home_id) else { return };
        if gx == gy {
            let pens = engine::shootout(&mut engine::match_rng(self.seed ^ 0x9E7A_17E5, m.id));
            self.matches[index].pens = Some(pens);
        }
    }

    /// Depois de um dia de jogos: as competições que acabaram uma ronda ou uma fase passam
    /// à seguinte. Devolve as que acabaram de vez.
    pub(super) fn progress(&mut self, comps: &[i32], fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<Vec<i32>> {
        let mut finished = Vec::new();
        for &comp in comps {
            let Some(reg) = regulation::get(comp) else { continue };
            let year = self.current_season(comp);
            let ms: Vec<&Match> = self.year_matches(comp, year).collect();
            if ms.is_empty() || ms.iter().any(|m| m.result.is_none()) {
                continue;
            }
            let stage = ms.iter().map(|m| m.stage).max().unwrap_or(0) as usize;
            let st = &reg.stages[stage];
            if st.kind == StageKind::Knockout {
                let round = ms.iter().filter(|m| m.stage as usize == stage).map(|m| m.round).max().unwrap_or(1);
                if (round as usize) < st.rounds.len() {
                    let winners = self.round_winners(comp, year, stage, round).unwrap_or_default();
                    let pairs: Vec<(i32, i32)> = if round == 1 && st.byes > 0 {
                        self.bye_pairs(reg, stage, year, &winners)?
                    } else {
                        winners.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0], c[1])).collect()
                    };
                    self.create_ko_round(reg, stage, round as usize, &pairs, year, fresh, next_id)?;
                    continue;
                }
            }
            if stage + 1 < reg.stages.len() {
                self.create_stage(reg, stage + 1, year, fresh, next_id)?;
                continue;
            }
            finished.push(comp);
        }
        Ok(finished)
    }

    /// A 2ª ronda de uma eliminatória com isentos (Liga dos Campeões): o isento k (1º, 2º...)
    /// encontra o vencedor da eliminatória do play-off cujo cabeça de série é o 2·isentos − k
    /// (o 1º com o vencedor de 16º-17º, o 8º com o de 9º-24º), pela ordem do quadro dos isentos.
    fn bye_pairs(&self, reg: &Regulation, stage: usize, year: i32, winners: &[i32]) -> Result<Vec<(i32, i32)>> {
        let byes = reg.stages[stage].byes;
        let seeded = self.entrants(reg, stage, year)?;
        let comp = reg.competition;
        // O cabeça de série de cada eliminatória é quem jogou a 2ª mão em casa
        let top: Vec<Option<usize>> = (0..winners.len())
            .map(|t| {
                let games = self.tie_games(comp, year, stage, 1, t as u16);
                games.last().and_then(|g| seeded.iter().position(|&c| c == g.home_id))
            })
            .collect();
        let mut pairs = Vec::new();
        for k in bracket_order(byes) {
            let seed = k - 1;
            let target = 2 * byes - 1 - seed;
            if let (Some(&club), Some(t)) = (seeded.get(seed), top.iter().position(|&p| p == Some(target))) {
                pairs.push((club, winners[t]));
            }
        }
        Ok(pairs)
    }

    /// Campeão, vice e 3º de uma competição acabada numa época.
    pub(super) fn podium(&self, comp: i32, year: i32) -> Option<(i32, Option<i32>, Option<i32>)> {
        let reg = regulation::get(comp)?;
        let ms: Vec<&Match> = self.year_matches(comp, year).collect();
        if ms.is_empty() || ms.iter().any(|m| m.result.is_none()) {
            return None;
        }
        let stage = ms.iter().map(|m| m.stage).max()? as usize;
        match reg.stages.get(stage)?.kind {
            StageKind::Knockout => {
                let round = ms.iter().filter(|m| m.stage as usize == stage).map(|m| m.round).max()?;
                let games = self.tie_games(comp, year, stage, round, 0);
                let winner = Self::tie_winner(&games)?;
                let last = games.last()?;
                let loser = if last.home_id == winner { last.away_id } else { last.home_id };
                Some((winner, Some(loser), None))
            }
            // Só grupos (Primera Nacional): o vencedor de grupo com mais pontos é campeão
            StageKind::Groups => {
                let mut firsts: Vec<StandingRow> = (0..self.group_count(comp, year, stage))
                    .filter_map(|g| self.stage_table(comp, year, stage, Some(g)).into_iter().next())
                    .collect();
                firsts.sort_by(|a, b| {
                    b.points
                        .cmp(&a.points)
                        .then(b.won.cmp(&a.won))
                        .then((b.goals_for - b.goals_against).cmp(&(a.goals_for - a.goals_against)))
                });
                Some((firsts.first()?.club.id, firsts.get(1).map(|r| r.club.id), firsts.get(2).map(|r| r.club.id)))
            }
            _ => {
                let t = self.stage_table(comp, year, stage, None);
                Some((t.first()?.club.id, t.get(1).map(|r| r.club.id), t.get(2).map(|r| r.club.id)))
            }
        }
    }

    /// "12ª jornada", "1ª fase · 12ª jornada", "Grupo B · 3ª jornada", "Oitavas de final · 1ª mão".
    pub(super) fn match_label(&self, m: &Match) -> String {
        let Some(st) = regulation::get(m.competition_id).and_then(|r| r.stages.get(m.stage as usize).map(|s| (r, s))) else {
            return format!("{}ª jornada", m.round);
        };
        let (reg, st) = st;
        match st.kind {
            StageKind::Knockout => {
                let name = st.rounds.get(m.round as usize - 1).map_or("Eliminatória", |r| r.name.as_str());
                match m.leg {
                    1 => format!("{name} · 1ª mão"),
                    2 => format!("{name} · 2ª mão"),
                    _ => name.to_string(),
                }
            }
            StageKind::Groups => format!("{} · {}ª jornada", group_name(m.group.unwrap_or(0)), m.round),
            StageKind::LeaguePhase => format!("{} · {}ª jornada", st.name, m.round),
            StageKind::League if reg.stages.len() > 1 && !st.name.is_empty() => format!("{} · {}ª jornada", st.name, m.round),
            StageKind::League => format!("{}ª jornada", m.round),
        }
    }

    /// O que aconteceu a um clube numa competição acabada ("caiu nos Quartos de final").
    pub(super) fn club_fate(&self, comp: i32, year: i32, club: i32) -> Option<String> {
        let reg = regulation::get(comp)?;
        let mine: Vec<&Match> = self.year_matches(comp, year).filter(|m| m.home_id == club || m.away_id == club).collect();
        let last = mine.iter().max_by_key(|m| (m.stage, m.round, m.leg))?;
        let st = reg.stages.get(last.stage as usize)?;
        let us = self.club(club).short_name;
        match st.kind {
            StageKind::Knockout => {
                let games = self.tie_games(comp, year, last.stage as usize, last.round, last.tie?);
                let round = st.rounds.get(last.round as usize - 1)?;
                if Self::tie_winner(&games)? == club {
                    return None; // campeão
                }
                Some(if round.name == "Final" { format!("O {us} perdeu a final.") } else { format!("O {us} caiu nos {}.", round.name) })
            }
            StageKind::Groups => Some(format!("O {us} ficou pela {}.", if st.name.is_empty() { "fase de grupos" } else { &st.name })),
            StageKind::LeaguePhase => {
                let t = self.stage_table(comp, year, last.stage as usize, None);
                let r = t.iter().find(|r| r.club.id == club)?;
                Some(format!("O {us} ficou pela {} ({}º lugar).", st.name.to_lowercase(), r.position))
            }
            StageKind::League => {
                let t = self.stage_table(comp, year, last.stage as usize, None);
                let r = t.iter().find(|r| r.club.id == club)?;
                Some(format!("O {us} terminou em {}º lugar, com {} pontos.", r.position, r.points))
            }
        }
    }

    /// As fases de uma competição viva na época em curso, para a tela Classificação.
    pub fn competition_stages(&self, comp: i32) -> Vec<StageView> {
        let year = self.current_season(comp);
        let Some(reg) = regulation::get(comp) else { return Vec::new() };
        let mut out = Vec::new();
        for (i, st) in reg.stages.iter().enumerate() {
            let ms: Vec<&Match> = self.year_matches(comp, year).filter(|m| m.stage as usize == i).collect();
            if ms.is_empty() {
                continue;
            }
            let name = if st.name.is_empty() { self.competition_name(comp).to_string() } else { st.name.clone() };
            let view = match st.kind {
                StageKind::League | StageKind::LeaguePhase => StageView {
                    name,
                    kind: "league".into(),
                    tables: vec![GroupTable { name: String::new(), rows: self.stage_table(comp, year, i, None) }],
                    rounds: Vec::new(),
                },
                StageKind::Groups => StageView {
                    name,
                    kind: "groups".into(),
                    tables: (0..self.group_count(comp, year, i))
                        .map(|g| GroupTable { name: group_name(g), rows: self.stage_table(comp, year, i, Some(g)) })
                        .collect(),
                    rounds: Vec::new(),
                },
                StageKind::Knockout => {
                    let last_round = ms.iter().map(|m| m.round).max().unwrap_or(1);
                    let rounds = (1..=last_round)
                        .map(|r| {
                            let ties = ms.iter().filter(|m| m.round == r).filter_map(|m| m.tie).max().map_or(0, |t| t + 1);
                            KoRoundView {
                                name: st.rounds.get(r as usize - 1).map_or_else(String::new, |x| x.name.clone()),
                                ties: (0..ties)
                                    .map(|t| {
                                        let games = self.tie_games(comp, year, i, r, t);
                                        let first = games[0];
                                        let agg = Self::aggregate(&games, first.home_id);
                                        let last = games[games.len() - 1];
                                        let pens = last.pens.map(|(h, a)| {
                                            if last.home_id == first.home_id { (h as i32, a as i32) } else { (a as i32, h as i32) }
                                        });
                                        TieView {
                                            first: self.club(first.home_id),
                                            second: self.club(first.away_id),
                                            games: games.iter().map(|m| self.fixture(m)).collect(),
                                            aggregate: if games.len() > 1 { agg } else { None },
                                            pens,
                                            winner: Self::tie_winner(&games),
                                        }
                                    })
                                    .collect(),
                            }
                        })
                        .collect();
                    StageView { name, kind: "knockout".into(), tables: Vec::new(), rounds }
                }
            };
            out.push(view);
        }
        out
    }

    /// A tabela onde o clube joga uma partida (pontos corridos ou o grupo dele), para as notícias.
    pub(super) fn table_for(&self, m: &Match) -> Vec<StandingRow> {
        if m.tie.is_some() {
            return Vec::new();
        }
        self.stage_table(m.competition_id, regulation::season_of(m.competition_id, &m.date), m.stage as usize, m.group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bracket_seeds() {
        assert_eq!(bracket_order(2), [1, 2]);
        assert_eq!(bracket_order(4), [1, 4, 2, 3]);
        assert_eq!(bracket_order(8), [1, 8, 4, 5, 2, 7, 3, 6]);
    }
}
