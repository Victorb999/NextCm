//! Épocas (marco 3.5; ano civil desde o 3.6, europeias desde o 3.7). Ano civil: fim de época
//! a 20 de dezembro (subidas e descidas, historial, contratos, reformas e plantéis mínimos) e
//! nova época a 1 de janeiro (sorteios e calendários). Época europeia: fim a 10 de junho e nova
//! a 1 de julho. Os contratos de meio do ano a 30 de junho, a evolução do CA no dia 1 de cada
//! mês e os juniores a 1 de março. Regras e números no ROADMAP (detalhe do 3.5, 3.6 e 3.7).
//! Tudo sorteado com a seed do jogo e a data, então a mesma seed dá a mesma carreira.

use std::collections::HashMap;

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use rusqlite::{OptionalExtension, params};

use super::regen::{self, PLAYER_JOB};
use super::{Game, NewsItem, club_refs, competition_names, league_club_ids};
use crate::engine::{expected_goals, poisson, uniform};
use crate::league::{self, SERIE_A, SERIE_C, StandingRow};
use crate::person::ClubRef;
use crate::regulation::{Regulation, SeasonKind, StageKind};
use crate::squad::Squads;
use crate::{Result, regulation};

/// Divisões geridas (as de regulamento com `managed`: plantéis que evoluem, renovam contratos
/// e recebem juniores), de um tipo de época ou de todos, para um `IN (...)` do SQL.
fn managed_sql(kind: Option<SeasonKind>) -> String {
    let ids: Vec<String> =
        regulation::all().iter().filter(|r| r.managed && kind.is_none_or(|k| r.season == k)).map(|r| r.competition.to_string()).collect();
    if ids.is_empty() { "-1".into() } else { ids.join(", ") }
}
/// Quantos sobem e descem entre duas divisões.
const SWAP: usize = 4;
/// Abaixo disto o clube contrata jogadores livres (até [`FILL_TO`]).
const MIN_SQUAD: usize = 24;
/// O clube renova com quem estiver entre os melhores tantos do plantel...
const KEEP_BEST: usize = 25;
/// ...e, com 34 ou 35 anos, só se estiver entre estes.
const VETERAN_RANK: usize = 16;
/// Um júnior (até 20 anos) só renova se estiver entre estes; os outros procuram clube abaixo.
const YOUTH_RANK: usize = 18;
/// Um plantel curto completa-se até aqui.
const FILL_TO: usize = 27;

/// Números de uma divisão (CLI `sim`).
#[derive(Debug, Clone)]
pub struct DivisionStats {
    pub clubs: usize,
    pub players: usize,
    pub avg_ca: f64,
    /// CA médio dos 18 melhores de cada clube (a força real da divisão, sem os juniores).
    pub avg_ca_top: f64,
    pub avg_age: f64,
    pub min_squad: usize,
    pub max_squad: usize,
    /// Saldo médio dos clubes (€) e quantos estão negativos.
    pub avg_cash: f64,
    pub in_debt: usize,
}

/// Idade numa data ("AAAA-MM-DD").
pub(crate) fn age_on(birth_date: Option<&str>, birth_year: Option<i32>, on: &str) -> i32 {
    let year: i32 = on.get(0..4).and_then(|y| y.parse().ok()).unwrap_or(0);
    match birth_date.filter(|b| b.len() >= 10) {
        Some(b) => year - b[0..4].parse::<i32>().unwrap_or(year) - (b[5..10] > on[5..10]) as i32,
        None => birth_year.map_or(25, |y| year - y),
    }
}

/// Lista "A, B e C".
pub(super) fn join_names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} e {last}", rest.join(", ")),
    }
}

impl Game {
    /// RNG de um acontecimento do calendário: a seed do jogo, a data e uma etiqueta.
    pub(super) fn season_rng(&self, tag: u64) -> ChaCha8Rng {
        let day = self.date.bytes().fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
        ChaCha8Rng::seed_from_u64((self.seed as u64) ^ day.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ tag)
    }

    /// O que o calendário traz num dia (chamado no início de `advance_day`, antes dos jogos).
    pub(super) fn calendar_events(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if self.date.ends_with("-01") {
            self.develop()?;
        }
        match &self.date[5..] {
            "03-01" => self.youth_intake(fresh, next_id)?,
            // Contratos que acabam a meio do ano (o fim de época trata os de dezembro)
            "06-30" => {
                let today = self.date.clone();
                self.contracts(fresh, next_id, &today)?;
                self.clean_selection()?;
            }
            "06-10" => self.end_of_season(SeasonKind::European, fresh, next_id)?,
            "07-01" => self.new_season(SeasonKind::European, fresh, next_id)?,
            "12-20" => self.end_of_season(SeasonKind::Calendar, fresh, next_id)?,
            "01-01" => self.new_season(SeasonKind::Calendar, fresh, next_id)?,
            _ => {}
        }
        Ok(())
    }

    fn push_news(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32, kind: &str, title: String, body: String) {
        fresh.push(self.news_item(*next_id, kind, title, body));
        *next_id += 1;
    }

    fn managed_clubs(&self) -> Result<Vec<i32>> {
        let sql = format!("SELECT id FROM clubs WHERE division_id IN ({}) ORDER BY id", managed_sql(None));
        Ok(self.world.conn.prepare(&sql)?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
    }

    fn club_short(&self, id: i32) -> String {
        self.world.club_ref(Some(id)).ok().flatten().map_or_else(|| format!("clube {id}"), |c| c.short_name)
    }

    /// Números de uma divisão para a CLI: clubes, jogadores, CA e idade médios, plantéis.
    pub fn division_stats(&self, division: i32) -> Result<DivisionStats> {
        let today = self.date.clone();
        let rows: Vec<(i32, Option<i32>, Option<String>, Option<i32>)> = self
            .world
            .conn
            .prepare(
                "SELECT p.club_id, pl.current_ability, p.birth_date, p.birth_year FROM people p
                 JOIN players pl ON pl.person_id = p.id JOIN clubs c ON c.id = p.club_id
                 WHERE c.division_id = ? AND p.is_player = 1",
            )?
            .query_map([division], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut squads: HashMap<i32, usize> = HashMap::new();
        let mut by_club: HashMap<i32, Vec<i32>> = HashMap::new();
        let (mut ca, mut age) = (0.0, 0.0);
        for (club, c, birth, birth_year) in &rows {
            *squads.entry(*club).or_default() += 1;
            by_club.entry(*club).or_default().push(c.unwrap_or(0));
            ca += c.unwrap_or(0) as f64;
            age += age_on(birth.as_deref(), *birth_year, &today) as f64;
        }
        let n = rows.len().max(1) as f64;
        let cash: Vec<i64> = self
            .world
            .conn
            .prepare("SELECT COALESCE(cash, 0) FROM clubs WHERE division_id = ?")?
            .query_map([division], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let avg_cash = cash.iter().sum::<i64>() as f64 / cash.len().max(1) as f64;
        let in_debt = cash.iter().filter(|&&c| c < 0).count();
        let top: Vec<i32> = by_club
            .into_values()
            .flat_map(|mut v| {
                v.sort_unstable_by(|a, b| b.cmp(a));
                v.truncate(18);
                v
            })
            .collect();
        Ok(DivisionStats {
            clubs: squads.len(),
            players: rows.len(),
            avg_ca: ca / n,
            avg_ca_top: top.iter().sum::<i32>() as f64 / top.len().max(1) as f64,
            avg_age: age / n,
            min_squad: squads.values().copied().min().unwrap_or(0),
            max_squad: squads.values().copied().max().unwrap_or(0),
            avg_cash,
            in_debt,
        })
    }

    /// Totais da carreira para a CLI: maior id de pessoa (os juniores são ids novos),
    /// reformados e jogadores livres.
    pub fn career_counts(&self) -> Result<(i32, i64, i64)> {
        Ok(self.world.conn.query_row(
            "SELECT (SELECT MAX(id) FROM people), (SELECT COUNT(*) FROM retired),
                    (SELECT COUNT(*) FROM people WHERE is_player = 1 AND club_id IS NULL)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?)
    }

    // ---- Fim de época (20 de dezembro) ----

    fn short_name(&self, comp: i32) -> String {
        self.world
            .conn
            .query_row("SELECT name, short_name FROM competitions WHERE id = ?", [comp], |r| {
                let (name, short): (String, String) = (r.get(0)?, r.get(1)?);
                Ok(if short.is_empty() { name } else { short })
            })
            .unwrap_or_else(|_| format!("competição {comp}"))
    }

    /// Fim de uma época do ano civil (20/12) ou europeia (10/6): historial, posições finais,
    /// subidas e descidas, reformas e plantéis mínimos das divisões desse tipo de época.
    fn end_of_season(&mut self, kind: SeasonKind, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        // A época que acaba: o ano, ou a europeia que começou no ano anterior
        let year = if kind == SeasonKind::European { self.season() - 1 } else { self.season() };
        let regs: Vec<&'static Regulation> = regulation::all().iter().filter(|r| r.season == kind).collect();
        match kind {
            SeasonKind::Calendar => {
                let a = self.table(SERIE_A);
                if a.is_empty() || a.iter().any(|r| r.played == 0) {
                    return Ok(()); // a época não chegou a ser jogada
                }
            }
            // Um save de antes do 3.7 não tem as ligas europeias
            SeasonKind::European => {
                if !regs.iter().any(|r| r.managed && self.year_matches(r.competition, year).next().is_some()) {
                    return Ok(());
                }
            }
        }
        let label = regulation::season_label(regs[0].competition, year);
        // O historial guarda o ano em que a época acaba, como os anos do patch
        let history_year = if kind == SeasonKind::European { year + 1 } else { year };

        // Historial: as competições vivas acabadas (a Série C, se não jogou, calculada com o motor v1)
        let c_virtual = if kind == SeasonKind::Calendar && self.year_matches(SERIE_C, year).next().is_none() {
            Some(self.virtual_league(SERIE_C)?)
        } else {
            None
        };
        let mut podiums: Vec<(i32, (i32, Option<i32>, Option<i32>))> =
            regs.iter().filter_map(|r| self.podium(r.competition, year).map(|p| (r.competition, p))).collect();
        if let Some(c) = &c_virtual {
            let at = |i: usize| c.get(i).map(|r| r.club.id);
            podiums.push((SERIE_C, (c[0].club.id, at(1), at(2))));
        }
        for (comp, (winner, second, third)) in &podiums {
            self.world
                .conn
                .execute("DELETE FROM competition_history WHERE competition_id = ? AND year = ?", params![comp, history_year])?;
            self.world.conn.execute(
                "INSERT INTO competition_history (competition_id, year, winner_id, runner_up_id, third_id) VALUES (?, ?, ?, ?, ?)",
                params![comp, history_year, winner, second, third],
            )?;
        }
        // Posição final de cada clube nas divisões com tabela (e a do treinador, para a direção)
        let mut our_position = None;
        for reg in regs.iter().filter(|r| r.managed && r.stages[0].kind == StageKind::League) {
            let div = reg.competition;
            let table = match (&c_virtual, div) {
                (Some(c), SERIE_C) => c.clone(),
                _ => self.stage_table(div, year, 0, None),
            };
            for r in &table {
                self.world.conn.execute(
                    "UPDATE clubs SET last_division_id = ?, last_position = ? WHERE id = ?",
                    params![div, r.position, r.club.id],
                )?;
                if r.club.id == self.club_id {
                    our_position = Some(r.position);
                }
            }
        }

        // Subidas e descidas pelos regulamentos (calculadas todas antes de mexer nas divisões).
        // Uma divisão que não jogou (a C e a D na transição) usa a tabela do motor v1 ou o
        // sorteio; das 3ªs divisões paradas sobe-se por sorteio (`draw_up`).
        let mut moves: Vec<(Vec<i32>, i32, String)> = Vec::new();
        // Falências (3.8): clubes da IA com dívidas acima de 60% das receitas de uma época descem
        // no lugar do melhor dos que desciam, e as dívidas ficam a zero
        let mut bankrupt: Vec<(i32, Option<i32>)> = Vec::new();
        for reg in regs.iter().filter(|r| r.managed) {
            let clubs: Vec<(i32, i64)> = self
                .world
                .conn
                .prepare("SELECT id, COALESCE(cash, 0) FROM clubs WHERE division_id = ? ORDER BY id")?
                .query_map([reg.competition], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            for (id, cash) in clubs {
                if id != self.club_id && (cash as f64) < -super::market::BANKRUPT_DEBT * self.club_base(id)? {
                    bankrupt.push((id, Some(reg.competition)));
                }
            }
        }
        let mut relegated_for_debt: Vec<i32> = Vec::new();
        for reg in regs.iter().filter(|r| r.promotion.is_some() || r.relegation.is_some() || r.draw_up.is_some()) {
            let div = reg.competition;
            let played = self.year_matches(div, year).next().is_some();
            for (mv, up) in [(&reg.relegation, false), (&reg.promotion, true)] {
                let Some(mv) = mv else { continue };
                let count = mv.count.unwrap_or(SWAP);
                let clubs = if played {
                    self.pick(reg, &mv.pick, 0, year)?
                } else if let Some(c) = c_virtual.as_ref().filter(|_| div == SERIE_C) {
                    let ids = c.iter().map(|r| r.club.id);
                    if up { ids.take(count).collect() } else { ids.rev().take(count).collect() }
                } else if up {
                    self.promoted_by_draw(&[div], count)?
                } else {
                    continue;
                };
                let mut clubs = clubs;
                if !up && played {
                    let table = self.stage_table(div, year, 0, None);
                    let pos = |id: i32| table.iter().find(|r| r.club.id == id).map_or(i32::MAX, |r| r.position);
                    let here: Vec<i32> = bankrupt.iter().filter(|(b, d)| *d == Some(div) && !clubs.contains(b)).map(|b| b.0).collect();
                    for b in here {
                        // Fica o melhor colocado dos que desciam
                        let spared = (0..clubs.len()).filter(|&i| !relegated_for_debt.contains(&clubs[i])).min_by_key(|&i| pos(clubs[i]));
                        if let Some(i) = spared {
                            clubs[i] = b;
                            relegated_for_debt.push(b);
                        }
                    }
                }
                let label = format!("{} à {}", if up { "Sobem" } else { "Descem" }, self.short_name(mv.to));
                moves.push((clubs, mv.to, label));
            }
            if let Some(d) = &reg.draw_up {
                let clubs = self.promoted_by_draw(&d.from, d.count)?;
                moves.push((clubs, div, format!("Sobem à {}", self.short_name(div))));
            }
        }
        let mut lines = Vec::new();
        let mut ours = None;
        for (clubs, to, label) in &moves {
            for &id in clubs {
                self.world.conn.execute("UPDATE clubs SET division_id = ? WHERE id = ?", params![to, id])?;
                if id == self.club_id {
                    ours = Some(label.clone());
                }
            }
            let names: Vec<String> = clubs.iter().map(|&id| self.club_short(id)).collect();
            lines.push(format!("{label}: {}.", join_names(&names)));
        }
        for &(id, _) in &bankrupt {
            self.world.conn.execute("UPDATE clubs SET cash = 0 WHERE id = ?", [id])?;
            let what = if relegated_for_debt.contains(&id) {
                "desce de divisão administrativamente"
            } else {
                "fica com as dívidas reestruturadas"
            };
            lines.push(format!("Falência: o {} {what}.", self.club_short(id)));
        }
        // A notícia só para quem tem o clube num campeonato deste tipo de época
        let ours_here =
            regs.iter().any(|r| self.year_matches(r.competition, year).any(|m| m.home_id == self.club_id || m.away_id == self.club_id));
        if ours_here || kind == SeasonKind::Calendar {
            let us = self.club_short(self.club_id);
            let mut body = format!("A época {label} chegou ao fim.\n\n{}", lines.join("\n"));
            if let Some(l) = &ours {
                body += &format!("\n\nO {us} {}.", l.to_lowercase().replacen("sobem", "sobe", 1).replacen("descem", "desce", 1));
            }
            self.push_news(fresh, next_id, "competition", format!("Fim da época {label}"), body);
        }
        if let Some(pos) = our_position {
            let (up, down) = ours.as_deref().map_or((false, false), |l| (l.starts_with("Sobem"), l.starts_with("Descem")));
            self.board_season_end(pos, up, down, fresh, next_id)?;
        }

        // Os contratos de dezembro tratam-se aqui; os de junho já foram a 30/6
        if kind == SeasonKind::Calendar {
            self.contracts(fresh, next_id, &format!("{year}-12-31"))?;
        }
        self.retirements(kind, fresh, next_id)?;
        self.fill_squads(kind, fresh, next_id)?;
        self.clean_selection()?;
        self.unsaved = true;
        Ok(())
    }

    /// Turno e returno calculado com o motor v1 (força dos clubes, sem estado), para as
    /// divisões que não estão vivas.
    fn virtual_league(&self, comp: i32) -> Result<Vec<StandingRow>> {
        let ids = league_club_ids(&self.world, comp)?;
        let strength: HashMap<i32, _> = ids.iter().map(|&id| Ok((id, self.world.team_strength(id)?))).collect::<Result<_>>()?;
        let mut rng = self.season_rng(comp as u64);
        let dates = regulation::dates(self.season(), "w15-w49 m24 m33 m42");
        let mut matches = league::round_robin(comp, &ids, &dates, 2, 1, &mut rng)?;
        for m in &mut matches {
            let (xh, xa) = expected_goals(&strength[&m.home_id], &strength[&m.away_id]);
            m.result = Some((poisson(xh, &mut rng), poisson(xa, &mut rng)));
        }
        let clubs: Vec<ClubRef> = ids.iter().filter_map(|&id| self.world.club_ref(Some(id)).ok().flatten()).collect();
        Ok(league::standings(&matches, &clubs))
    }

    /// Os `n` que sobem de divisões sem tabela: sorteio pesado pela força de cada clube.
    fn promoted_by_draw(&self, divisions: &[i32], n: usize) -> Result<Vec<i32>> {
        let mut rng = self.season_rng(divisions.first().copied().unwrap_or(0) as u64);
        let mut scored = Vec::new();
        for &comp in divisions {
            for id in league_club_ids(&self.world, comp)? {
                let s = self.world.team_strength(id)?;
                scored.push((id, (s.attack + s.midfield + s.defence) * (0.85 + 0.3 * uniform(&mut rng))));
            }
        }
        scored.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(&y.0)));
        Ok(scored.into_iter().take(n).map(|s| s.0).collect())
    }

    /// Contratos que acabam até `cutoff`: renova quem está entre os melhores do plantel e não
    /// é velho; os outros saem livres. No clube do treinador renova só quem o treinador
    /// renovou (3.8); os contratos sem data, aí e nos outros, só se tratam no fim da época.
    fn contracts(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32, cutoff: &str) -> Result<()> {
        let today = self.date.clone();
        let year_end = today.ends_with("12-20");
        let year: i32 = today[0..4].parse().unwrap_or(0);
        let mut rng = self.season_rng(0xC0DE);
        // Quem assinou um pré-contrato com o treinador não renova: vai-se embora no fim do contrato
        let precontracted = self.precontracted()?;
        for club in self.managed_clubs()? {
            type Row = (i32, String, Option<String>, Option<String>, Option<i32>, i32, i32);
            let mut rows: Vec<Row> = self
                .world
                .conn
                .prepare_cached(
                    "SELECT p.id, p.display_name, p.contract_expires, p.birth_date, p.birth_year,
                            COALESCE(pl.current_ability, 1), COALESCE(pl.potential_ability, 0)
                     FROM people p JOIN players pl ON pl.person_id = p.id
                     WHERE p.club_id = ? AND p.is_player = 1 ORDER BY p.id",
                )?
                .query_map([club], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?
                .collect::<rusqlite::Result<_>>()?;
            // O que cada um vale para o clube: o CA e, até aos 20 anos, um terço do que ainda pode crescer
            let worth = |r: &Row| {
                let age = age_on(r.3.as_deref(), r.4, &today);
                let pa = if r.6 > 0 { r.6 } else { r.5 + 20 };
                r.5 + if age <= 20 { (pa - r.5).max(0) / 3 } else { 0 }
            };
            rows.sort_by_key(|r| (std::cmp::Reverse(worth(r)), r.0));
            let (mut renewed, mut left) = (Vec::new(), Vec::new());
            for (rank, (id, name, expires, birth, birth_year, ..)) in rows.into_iter().enumerate() {
                if precontracted.contains(&id) {
                    continue;
                }
                match expires.as_deref() {
                    Some(e) if e > cutoff => continue,
                    None if !year_end => continue,
                    _ => {}
                }
                let age = age_on(birth.as_deref(), birth_year, &today);
                // Fica quem é dos melhores do plantel; os veteranos só se forem titulares e os
                // juniores só se já forem do grupo principal
                let keep = match age {
                    _ if club == self.club_id && expires.is_some() => false,
                    ..=20 => rank < YOUTH_RANK,
                    21..=33 => rank < KEEP_BEST,
                    _ => rank < VETERAN_RANK && age < 36,
                };
                if keep {
                    let years = match age {
                        ..=23 => 3 + rng.next_u32() % 2,
                        24..=29 => 2 + rng.next_u32() % 3,
                        _ => 1 + rng.next_u32() % 2,
                    } as i32;
                    let until = format!("{}-06-30", year + years);
                    self.world.conn.execute("UPDATE people SET contract_expires = ? WHERE id = ?", params![until, id])?;
                    renewed.push(format!("{name} (até {})", super::cm_date(&until)));
                } else {
                    self.world
                        .conn
                        .execute("UPDATE people SET club_id = NULL, club_job = NULL, contract_expires = NULL WHERE id = ?", [id])?;
                    left.push(name);
                }
            }
            if club == self.club_id && !(renewed.is_empty() && left.is_empty()) {
                let mut body = String::from("Acabaram os contratos desta época.");
                if !renewed.is_empty() {
                    body += &format!("\n\nRenovaram: {}.", join_names(&renewed));
                }
                if !left.is_empty() {
                    body += &format!("\n\nSaem livres: {}.", join_names(&left));
                }
                self.push_news(fresh, next_id, "message", "Contratos".into(), body);
            }
        }
        Ok(())
    }

    /// Reformas a partir dos 34 anos (guarda-redes dois anos depois), nos plantéis das
    /// divisões geridas e entre os livres dessas nações.
    fn retirements(&mut self, kind: SeasonKind, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let managed = managed_sql(Some(kind));
        let today = self.date.clone();
        let mut rng = self.season_rng(0x0E7E);
        let sql = format!(
            "SELECT p.id, p.display_name, p.club_id, p.birth_date, p.birth_year, pl.pos_goalkeeper
             FROM people p JOIN players pl ON pl.person_id = p.id
             WHERE p.is_player = 1 AND p.id NOT IN (SELECT person_id FROM retired)
               AND (p.club_id IN (SELECT id FROM clubs WHERE division_id IN ({managed}))
                    OR (p.club_id IS NULL AND p.nation_id IN (SELECT DISTINCT nation_id FROM clubs WHERE division_id IN ({managed}))))
             ORDER BY p.id"
        );
        let rows: Vec<(i32, String, Option<i32>, Option<String>, Option<i32>, Option<i32>)> = self
            .world
            .conn
            .prepare(&sql)?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut ours = Vec::new();
        for (id, name, club, birth, birth_year, gk) in rows {
            let age = age_on(birth.as_deref(), birth_year, &today) - if gk.unwrap_or(0) >= 15 { 2 } else { 0 };
            let chance = match age {
                ..=33 => continue,
                34 => 0.10,
                35 => 0.20,
                36 => 0.40,
                37 => 0.60,
                38 => 0.80,
                _ => 1.0,
            };
            if uniform(&mut rng) >= chance {
                continue;
            }
            self.world.conn.execute("INSERT OR IGNORE INTO retired (person_id, date) VALUES (?, ?)", params![id, today])?;
            self.world.conn.execute("UPDATE people SET club_id = NULL, club_job = NULL, contract_expires = NULL WHERE id = ?", [id])?;
            if club == Some(self.club_id) {
                ours.push(name);
            }
        }
        if !ours.is_empty() {
            let verb = if ours.len() == 1 { "terminou a carreira" } else { "terminaram a carreira" };
            self.push_news(fresh, next_id, "message", "Reformas".into(), format!("{} {verb} no fim desta época.", join_names(&ours)));
        }
        Ok(())
    }

    /// Plantéis com menos de [`MIN_SQUAD`] jogadores contratam livres (primeiro do país, com
    /// CA à medida do plantel, um guarda-redes se faltar). Provisório até ao mercado do 3.7.
    fn fill_squads(&mut self, kind: SeasonKind, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let managed = managed_sql(Some(kind));
        let today = self.date.clone();
        let year: i32 = today[0..4].parse().unwrap_or(0);
        let mut rng = self.season_rng(0xF111);
        // Os clubes com mais reputação escolhem primeiro
        let sql = format!("SELECT id FROM clubs WHERE division_id IN ({managed}) ORDER BY reputation DESC, id");
        let clubs: Vec<i32> = self.world.conn.prepare(&sql)?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        for club in clubs {
            let (count, keepers, avg, nation): (i64, i64, Option<f64>, Option<i32>) = self.world.conn.query_row(
                "SELECT COUNT(*), SUM(pl.pos_goalkeeper >= 15), AVG(pl.current_ability), c.nation_id
                 FROM clubs c LEFT JOIN people p ON p.club_id = c.id AND p.is_player = 1
                 LEFT JOIN players pl ON pl.person_id = p.id WHERE c.id = ? GROUP BY c.id",
                [club],
                |r| Ok((r.get(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0), r.get(2)?, r.get(3)?)),
            )?;
            let mut count = count as usize;
            let mut keepers = keepers;
            let ceiling = (avg.unwrap_or(80.0) * 1.15).round() as i32;
            let mut signed = Vec::new();
            if count >= MIN_SQUAD {
                continue;
            }
            while count < FILL_TO {
                let mut need_gk = keepers < 2;
                let free_agent = |need_gk: bool| -> Result<Option<(i32, String)>> {
                    Ok(self
                        .world
                        .conn
                        .prepare_cached(
                            "SELECT p.id, p.display_name FROM people p JOIN players pl ON pl.person_id = p.id
                         WHERE p.is_player = 1 AND p.club_id IS NULL AND p.id NOT IN (SELECT person_id FROM retired)
                           AND ?1 - COALESCE(p.birth_year, ?1 - 25) BETWEEN 18 AND 33
                           AND pl.current_ability <= ?2 AND (?3 = 0 OR pl.pos_goalkeeper >= 15)
                         ORDER BY (p.nation_id IS ?4) DESC, pl.current_ability DESC, p.id LIMIT 1",
                        )?
                        .query_row(params![year, ceiling, need_gk as i32, nation], |r| Ok((r.get(0)?, r.get(1)?)))
                        .optional()?)
                };
                let mut pick = free_agent(need_gk)?;
                if pick.is_none() && need_gk {
                    // Sem guarda-redes livres: fica com o que houver
                    need_gk = false;
                    keepers = 2;
                    pick = free_agent(false)?;
                }
                let Some((id, name)) = pick else { break };
                let until = format!("{}-06-30", year + 1 + (rng.next_u32() % 2) as i32);
                self.world.conn.execute(
                    "UPDATE people SET club_id = ?, club_job = ?, club_joined = ?, contract_expires = ? WHERE id = ?",
                    params![club, PLAYER_JOB, today, until, id],
                )?;
                count += 1;
                if need_gk {
                    keepers += 1;
                }
                signed.push(name);
            }
            if club == self.club_id && !signed.is_empty() {
                let body = format!(
                    "O plantel tinha menos de {MIN_SQUAD} jogadores: o adjunto contratou jogadores livres: {}.",
                    join_names(&signed)
                );
                self.push_news(fresh, next_id, "message", "Reforços para o plantel".into(), body);
            }
        }
        Ok(())
    }

    /// Tira da escolha do treinador quem já não é do clube (saiu, reformou-se); as funções
    /// vazias o adjunto preenche no dia do jogo.
    pub(super) fn clean_selection(&mut self) -> Result<()> {
        if self.selection.is_none() {
            return Ok(());
        }
        let mut squad: Vec<i32> = self
            .world
            .conn
            .prepare("SELECT id FROM people WHERE club_id = ? AND is_player = 1")?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        // Os das reservas também saem da escolha da equipa principal
        squad.retain(|id| !self.squads.states.get(id).is_some_and(|s| s.reserve));
        let Some(sel) = &mut self.selection else { return Ok(()) };
        for s in &mut sel.starters {
            if s.is_some_and(|id| !squad.contains(&id)) {
                *s = None;
            }
        }
        sel.bench.retain(|id| squad.contains(id));
        Ok(())
    }

    // ---- Nova época (1 de janeiro) ----

    /// Nova época do ano civil (1/1) ou europeia (1/7): sorteios, calendários e plantéis.
    fn new_season(&mut self, kind: SeasonKind, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let season = self.season();
        let starts_here = |comp: i32| regulation::season_kind(comp) == kind;
        if kind == SeasonKind::Calendar {
            self.world.start_year = season; // as idades nas telas contam a partir daqui
        }
        for st in self.squads.states.values_mut() {
            if kind == SeasonKind::Calendar {
                st.apps = 0;
            }
            st.clear_cards(starts_here);
        }
        self.schedule_season(kind, season, fresh, next_id)?;
        // Clubes pequenos sem jogadores no patch ganham plantel, como no CM
        self.complete_squads()?;
        self.refresh_live()?;
        if let Some(n) = self.season_news(*next_id, kind) {
            fresh.push(n);
            *next_id += 1;
        }
        if self.kind_of(self.club_id)? == kind {
            self.board_new_season(fresh, next_id)?;
            self.numbers_news(fresh, next_id)?;
        }
        Ok(())
    }

    /// Volta a ler os clubes vivos (os que jogam nesta época, mais o do treinador): nomes,
    /// números de camisola e os atributos que o estado dos jogadores usa.
    pub(super) fn refresh_live(&mut self) -> Result<()> {
        let mut live: Vec<i32> = self.season_matches().flat_map(|m| [m.home_id, m.away_id]).chain([self.club_id]).collect();
        live.sort_unstable();
        live.dedup();
        super::numbers::fix_squad_numbers(&self.world, &live, self.seed, Some(self.club_id))?;
        let mut all: Vec<i32> = self.matches.iter().flat_map(|m| [m.home_id, m.away_id]).chain(live.iter().copied()).collect();
        all.sort_unstable();
        all.dedup();
        self.clubs = club_refs(&self.world, &all)?;
        self.competitions = competition_names(&self.world, &self.matches)?;
        self.squads.traits = Squads::load_traits(&self.world, &live)?;
        Ok(())
    }

    // ---- Evolução do CA (dia 1 de cada mês) ----

    fn develop(&mut self) -> Result<()> {
        let today = self.date.clone();
        let managed = managed_sql(None);
        let mut rng = self.season_rng(0xDE7E);
        // Jogos já feitos por cada clube nesta época, para medir quanto cada um jogou
        let mut club_games: HashMap<i32, i32> = HashMap::new();
        for m in self.season_matches().filter(|m| m.result.is_some()) {
            *club_games.entry(m.home_id).or_default() += 1;
            *club_games.entry(m.away_id).or_default() += 1;
        }
        let sql = format!(
            "SELECT p.id, p.club_id, p.birth_date, p.birth_year, pl.current_ability, pl.potential_ability, pl.pos_goalkeeper,
                    p.determination, p.professionalism
             FROM people p JOIN players pl ON pl.person_id = p.id JOIN clubs c ON c.id = p.club_id
             WHERE c.division_id IN ({managed}) AND p.is_player = 1 ORDER BY p.id"
        );
        type Row = (i32, i32, Option<String>, Option<i32>, Option<i32>, Option<i32>, Option<i32>, Option<i32>, Option<i32>);
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare(&sql)?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut set_player =
            self.world.conn.prepare("UPDATE players SET current_ability = ?, potential_ability = ? WHERE person_id = ?")?;
        let mut set_person = self.world.conn.prepare("UPDATE people SET ability = ? WHERE id = ?")?;
        for (id, club, birth, birth_year, ca, pa, gk, det, prof) in rows {
            let ca = ca.unwrap_or(1).max(1);
            let age = age_on(birth.as_deref(), birth_year, &today);
            // PA negativo = "aleatório" no CM: fica fixo na primeira vez
            let pa_new = match pa {
                Some(p) if p > 0 => p.max(ca),
                _ => {
                    let extra = if age <= 23 { 10 + rng.next_u32() % 31 } else { rng.next_u32() % 8 };
                    (ca + extra as i32).min(200)
                }
            };
            let keeper = gk.unwrap_or(0) >= 15;
            let gap = (pa_new - ca) as f64;
            let mut yearly = match age {
                ..=21 => gap * (0.22 + 0.12 * uniform(&mut rng)),
                22..=24 => gap * 0.15,
                25..=28 => gap * 0.08,
                29..=30 => 0.0,
                // Declínio: 1,5 por ano aos 31–32 e 3 × (idade − 32) depois (guarda-redes dois anos mais tarde)
                _ => {
                    let age = if keeper { age - 2 } else { age };
                    match age {
                        ..=30 => 0.0,
                        31..=32 => -1.5,
                        _ => -3.0 * (age - 32) as f64,
                    }
                }
            };
            if yearly > 0.0 {
                // Jogar conta: de 60% (sem jogos) a 140% (sempre a jogar); fora das ligas vivas, 100%
                let games = club_games.get(&club).copied().unwrap_or(0);
                let share = if games > 0 { (self.squads.state(id).apps as f64 / games as f64).min(1.0) } else { 0.5 };
                yearly *= 0.6 + 0.8 * share;
                let mind = (det.unwrap_or(10) + prof.unwrap_or(10)) as f64 / 2.0;
                yearly *= 0.8 + 0.4 * (mind - 1.0) / 19.0;
            }
            let monthly = yearly / 12.0 + (uniform(&mut rng) - 0.5) * 0.4;
            let step = monthly.floor() as i32 + (uniform(&mut rng) < monthly - monthly.floor()) as i32;
            let new_ca = (ca + step).clamp(1, pa_new.max(ca));
            if new_ca != ca || Some(pa_new) != pa {
                set_player.execute(params![new_ca, pa_new, id])?;
                set_person.execute(params![new_ca, id])?;
            }
        }
        self.unsaved = true;
        Ok(())
    }

    // ---- Juniores (1 de março) ----

    fn youth_intake(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let year: i32 = self.date[0..4].parse().unwrap_or(0);
        let mut rng = self.season_rng(0x1A7E);
        let sql = format!("SELECT id, nation_id, reputation FROM clubs WHERE division_id IN ({}) ORDER BY id", managed_sql(None));
        let clubs: Vec<(i32, Option<i32>, Option<i32>)> =
            self.world.conn.prepare(&sql)?.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut factory = self.player_factory()?;
        let mut ours = Vec::new();
        // O investimento da direção nas camadas jovens (Reunião com a Direcção)
        let invested: i32 =
            self.world.conn.query_row("SELECT youth_level FROM board WHERE id = 1", [], |r| r.get(0)).optional()?.unwrap_or(0);
        for (club, nation, reputation) in clubs {
            // O potencial dos juniores vem da reputação do clube, que não muda no save. Medido no
            // patch: PA médio dos 11 melhores ≈ 84 + 0,0114 × reputação (Série A ≈ 147, B ≈ 129,
            // C ≈ 115). Tirado do próprio plantel haveria uma espiral (cada geração puxava a seguinte)
            let mut level = 84.0 + 0.0114 * reputation.unwrap_or(3000) as f64;
            if club == self.club_id {
                level += super::board_meeting::YOUTH_PA * invested.min(super::board_meeting::YOUTH_MAX) as f64;
            }
            // Um júnior por ano e, a um em cada três clubes, um segundo
            for _ in 0..1 + rng.next_u32().is_multiple_of(3) as u32 {
                let position = regen::random_position(&mut rng);
                let ca = 25 + (rng.next_u32() % 31) as i32;
                let pa = ((level * (0.75 + 0.5 * uniform(&mut rng))).round() as i32).clamp(ca + 10, 195);
                let p = regen::NewPlayer {
                    club,
                    nation,
                    birth_year: year - 16,
                    ca,
                    pa,
                    position,
                    contract_until: format!("{}-12-31", year + 2),
                };
                if let Some((_, name)) = self.create_player(&mut factory, &p, &mut rng)?
                    && club == self.club_id
                {
                    ours.push(format!("{name} ({})", position.1));
                }
            }
        }
        if !ours.is_empty() {
            let body = format!("Chegaram aos juniores do clube: {}.\n\nTêm 16 anos e contrato de três anos.", join_names(&ours));
            self.push_news(fresh, next_id, "message", "Novos juniores".into(), body);
        }
        self.refresh_live()?;
        self.unsaved = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::date;
    use crate::league::{SERIE_B, SERIE_D};
    use crate::{NewGame, World};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nextcm-season-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn new_game(club_id: i32) -> NewGame {
        NewGame {
            save_name: "Época".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id,
        }
    }

    fn until(g: &mut Game, day: &str) {
        while g.date() < day {
            g.advance_day().unwrap();
        }
    }

    fn divisions(g: &Game) -> Vec<(i32, i32)> {
        g.world
            .conn
            .prepare("SELECT id, division_id FROM clubs WHERE division_id IN (65, 79, 80, 175) ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn age_on_counts_the_birthday() {
        assert_eq!(age_on(Some("2000-07-02"), None, "2025-07-01"), 24);
        assert_eq!(age_on(Some("2000-07-01"), None, "2025-07-01"), 25);
        assert_eq!(age_on(None, Some(1990), "2025-07-01"), 35);
    }

    #[test]
    fn a_season_ends_and_the_next_one_starts() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let run = |name: &str| {
            let dir = temp_dir(name);
            let mut g = Game::create(&world, &dir, &new_game(7568), 21).unwrap();
            let b_before = divisions(&g);
            until(&mut g, "2025-12-19");
            let b_table = g.table(SERIE_B);
            until(&mut g, "2026-01-01");
            (g, dir, b_before, b_table)
        };
        let (g, dir, before, b_table) = run("a");

        // Os 4 primeiros da B estão agora na A, e cada divisão continua com o mesmo número de clubes
        let now = divisions(&g);
        for r in b_table.iter().take(4) {
            assert!(now.contains(&(r.club.id, SERIE_A)), "{} subiu", r.club.short_name);
        }
        let count = |d: &[(i32, i32)], div: i32| d.iter().filter(|c| c.1 == div).count();
        for div in [SERIE_A, SERIE_B, SERIE_C, SERIE_D] {
            assert_eq!(count(&now, div), count(&before, div), "divisão {div}");
        }
        // Nova época 2026: todas as competições sorteadas, por jogar; idades a contar de 2026
        assert_eq!(g.season(), 2026);
        assert_eq!(g.world.start_year(), 2026);
        assert_eq!(g.fixtures(SERIE_A).len(), 380);
        assert!(g.fixtures(SERIE_A).iter().all(|f| f.home_goals.is_none()));
        assert_eq!(g.table(SERIE_B).len(), 20);
        assert_eq!(g.table(SERIE_C).len(), 20, "a C é viva, em turno único");
        assert_eq!(g.fixtures(SERIE_C).len(), 190);
        assert_eq!(g.fixtures(SERIE_D).len(), 16 * 30, "16 grupos de 6");
        assert_eq!(g.fixtures(crate::COPA_DO_BRASIL).len(), 32, "1ª fase em jogo único");
        for comp in [crate::LIBERTADORES, crate::SUL_AMERICANA] {
            let stages = g.competition_stages(comp);
            assert_eq!(stages[0].tables.len(), 8, "8 grupos");
            assert!(stages[0].tables.iter().all(|t| t.rows.len() == 4));
        }
        // Os clubes da Libertadores não estão na Sul-Americana, e o Brasil tem 6 em cada
        let clubs = |comp: i32| g.fixtures(comp).iter().flat_map(|f| [f.home.id, f.away.id]).collect::<std::collections::HashSet<_>>();
        assert!(clubs(crate::LIBERTADORES).is_disjoint(&clubs(crate::SUL_AMERICANA)));
        let brazil: Vec<i32> = g
            .world
            .conn
            .prepare("SELECT id FROM clubs WHERE nation_id = 27")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for comp in [crate::LIBERTADORES, crate::SUL_AMERICANA] {
            assert_eq!(clubs(comp).iter().filter(|c| brazil.contains(c)).count(), 6, "competição {comp}");
        }
        let paulista = g.fixtures(67);
        assert_eq!(paulista.len(), 120, "16 clubes em turno único");
        assert!(paulista.iter().all(|f| f.date.as_str() >= "2026-01-17"));
        let history: i64 = g
            .world
            .conn
            .query_row("SELECT COUNT(*) FROM competition_history WHERE year = 2025 AND competition_id IN (65, 79, 80)", [], |r| r.get(0))
            .unwrap();
        assert_eq!(history, 3, "A, B e C");
        assert!(g.news().iter().any(|n| n.title == "Fim da época 2025"));
        assert!(g.news().iter().any(|n| n.title == "Calendário 2026"));

        // Mesma seed, mesma carreira
        let (again, dir2, ..) = run("b");
        assert_eq!(divisions(&again), now);
        drop((g, again));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    /// O "pronto quando" do 3.6 e do 3.7: a época de 2026 inteira com as competições do ano
    /// civil, e a época europeia de 2025/26 inteira (ligas, Liga dos Campeões e Liga Europa).
    #[test]
    fn a_full_2026_season() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("2026");
        let mut g = Game::create(&world, &dir, &new_game(7568), 5).unwrap();
        until(&mut g, "2026-06-09");
        // Os 3 últimos da Premier League e os 3 sorteados da League One, antes do fim da época europeia
        let pl_bottom: Vec<i32> = g.stage_table(crate::PREMIER_LEAGUE, 2025, 0, None).iter().rev().take(3).map(|r| r.club.id).collect();
        until(&mut g, "2026-12-21");
        let year: Vec<&crate::league::Match> = g.matches.iter().filter(|m| m.date.starts_with("2026")).collect();
        assert!(year.len() > 2500, "{} jogos", year.len());
        assert!(year.iter().all(|m| m.result.is_some()), "todos jogados");
        // Ninguém joga duas vezes em menos de 3 dias
        let mut by_club: HashMap<i32, Vec<i32>> = HashMap::new();
        for m in &year {
            let day = date::parse(&m.date).unwrap();
            by_club.entry(m.home_id).or_default().push(day);
            by_club.entry(m.away_id).or_default().push(day);
        }
        for (club, mut days) in by_club {
            days.sort_unstable();
            let gap = days.windows(2).map(|w| w[1] - w[0]).min().unwrap_or(99);
            assert!(gap >= 3, "{} joga com {gap} dias de intervalo", g.club(club).short_name);
        }
        // Todas as competições acabaram, com campeão no historial: as do ano civil em 2026 e as
        // europeias na época de 2025/26 (o historial guarda o ano em que acaba, 2026)
        for r in regulation::all() {
            let season = if r.season == SeasonKind::European { 2025 } else { 2026 };
            assert!(g.podium(r.competition, season).is_some(), "competição {} sem campeão", r.competition);
        }
        let ids: Vec<String> = regulation::all().iter().map(|r| r.competition.to_string()).collect();
        let sql = format!("SELECT COUNT(*) FROM competition_history WHERE year = 2026 AND competition_id IN ({})", ids.join(","));
        let history: i64 = g.world.conn.query_row(&sql, [], |r| r.get(0)).unwrap();
        assert_eq!(history as usize, regulation::all().len());
        // Liga dos Campeões: 36 clubes na fase de liga, 8 jogos cada, 4 em casa e 4 fora
        let phase = g.stage_table(crate::CHAMPIONS_LEAGUE, 2025, 0, None);
        assert_eq!(phase.len(), 36);
        assert!(phase.iter().all(|r| r.played == 8));
        let homes = |club: i32| g.year_matches(crate::CHAMPIONS_LEAGUE, 2025).filter(|m| m.stage == 0 && m.home_id == club).count();
        assert!(phase.iter().all(|r| homes(r.club.id) == 4));
        // Os 3 últimos da Premier League desceram ao Championship
        let division =
            |club: i32| -> i32 { g.world.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [club], |r| r.get(0)).unwrap() };
        assert!(pl_bottom.iter().all(|&c| division(c) == 8), "desceram");
        assert_eq!(g.table(crate::PREMIER_LEAGUE).len(), 20, "a época 2026/27 já corre");
        // As notícias dos sorteios saem com os nomes dos clubes e das competições
        let draws: Vec<&NewsItem> = g.news().iter().filter(|n| n.title.starts_with("Sorteio")).collect();
        assert!(!draws.is_empty());
        assert!(draws.iter().all(|n| !n.body.contains("clube ") && n.title != "Sorteio da Liga"), "{:?}", draws[0].body);
        // Eliminatórias empatadas foram a penáltis
        assert!(year.iter().any(|m| m.pens.is_some()));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn leaving_without_saving_undoes_the_world_changes() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("rollback");
        let mut g = Game::create(&world, &dir, &new_game(7568), 3).unwrap();
        let path = g.path().to_path_buf();
        let ca = |g: &Game| -> i64 { g.world.conn.query_row("SELECT SUM(current_ability) FROM players", [], |r| r.get(0)).unwrap() };
        let original = ca(&g);
        until(&mut g, "2025-09-02"); // duas evoluções mensais
        assert_ne!(ca(&g), original, "o CA mudou");
        drop(g); // sair sem gravar
        let g = Game::load(&path).unwrap();
        assert_eq!(ca(&g), original, "nada ficou gravado");
        assert_eq!(g.date(), "2025-07-01");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
