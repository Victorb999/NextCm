//! Épocas (marco 3.5; ano civil desde o 3.6): o fim de época a 20 de dezembro (subidas e
//! descidas, historial, contratos, reformas e plantéis mínimos), a nova época a 1 de janeiro
//! (sorteios e calendários), os contratos de meio do ano a 30 de junho, a evolução do CA no
//! dia 1 de cada mês e os juniores a 1 de março. Regras e números no ROADMAP (detalhe do 3.5
//! e do 3.6). Tudo sorteado com a seed do jogo e a data, então a mesma seed dá a mesma carreira.

use std::collections::HashMap;

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use rusqlite::{OptionalExtension, params};

use super::regen::{self, PLAYER_JOB};
use super::{Game, NewsItem, club_refs, competition_names, fix_squad_numbers, league_club_ids, season_label, season_of};
use crate::engine::{expected_goals, poisson, uniform};
use crate::league::{self, SERIE_A, SERIE_C, SERIE_D, StandingRow};
use crate::person::ClubRef;
use crate::squad::Squads;
use crate::{Result, regulation};

/// Divisões cujos plantéis evoluem, renovam contratos e recebem juniores (A, B e C).
const MANAGED_SQL: &str = "65, 79, 80";
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
            "12-20" => self.end_of_season(fresh, next_id)?,
            "01-01" => self.new_season(fresh, next_id)?,
            _ => {}
        }
        Ok(())
    }

    fn push_news(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32, kind: &str, title: String, body: String) {
        fresh.push(self.news_item(*next_id, kind, title, body));
        *next_id += 1;
    }

    fn managed_clubs(&self) -> Result<Vec<i32>> {
        let sql = format!("SELECT id FROM clubs WHERE division_id IN ({MANAGED_SQL}) ORDER BY id");
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

    fn end_of_season(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let a = self.table(SERIE_A);
        if a.is_empty() || a.iter().any(|r| r.played == 0) {
            return Ok(()); // a época não chegou a ser jogada
        }
        let year = self.season();

        // Historial: as competições vivas acabadas (a C, se não jogou, calculada com o motor v1)
        let c_virtual = if self.year_matches(SERIE_C, year).next().is_none() { Some(self.virtual_league(SERIE_C)?) } else { None };
        let mut podiums: Vec<(i32, (i32, Option<i32>, Option<i32>))> =
            regulation::all().iter().filter_map(|r| self.podium(r.competition, year).map(|p| (r.competition, p))).collect();
        if let Some(c) = &c_virtual {
            let at = |i: usize| c.get(i).map(|r| r.club.id);
            podiums.push((SERIE_C, (c[0].club.id, at(1), at(2))));
        }
        for (comp, (winner, second, third)) in &podiums {
            self.world.conn.execute("DELETE FROM competition_history WHERE competition_id = ? AND year = ?", params![comp, year])?;
            self.world.conn.execute(
                "INSERT INTO competition_history (competition_id, year, winner_id, runner_up_id, third_id) VALUES (?, ?, ?, ?, ?)",
                params![comp, year, winner, second, third],
            )?;
        }
        // Posição final de cada clube nas divisões com tabela
        for div in [SERIE_A, league::SERIE_B, SERIE_C] {
            let table = match (&c_virtual, div) {
                (Some(c), SERIE_C) => c.clone(),
                _ => self.stage_table(div, year, 0, None),
            };
            for r in &table {
                self.world.conn.execute(
                    "UPDATE clubs SET last_division_id = ?, last_position = ? WHERE id = ?",
                    params![div, r.position, r.club.id],
                )?;
            }
        }

        // Subidas e descidas pelos regulamentos (calculadas todas antes de mexer nas divisões).
        // Uma divisão que não jogou (a C e a D na transição) usa a tabela do motor v1 ou o sorteio.
        let mut moves: Vec<(Vec<i32>, i32, String)> = Vec::new();
        for div in [SERIE_A, league::SERIE_B, SERIE_C, SERIE_D] {
            let Some(reg) = regulation::get(div) else { continue };
            let played = self.year_matches(div, year).next().is_some();
            for (mv, up) in [(&reg.relegation, false), (&reg.promotion, true)] {
                let Some(mv) = mv else { continue };
                let clubs = if played {
                    self.pick(reg, &mv.pick, 0, year)?
                } else if let Some(c) = c_virtual.as_ref().filter(|_| div == SERIE_C) {
                    let ids = c.iter().map(|r| r.club.id);
                    if up { ids.take(SWAP).collect() } else { ids.rev().take(SWAP).collect() }
                } else if up {
                    self.promoted_by_draw(div, SWAP)?
                } else {
                    continue;
                };
                let label = format!("{} à {}", if up { "Sobem" } else { "Descem" }, self.short_name(mv.to));
                moves.push((clubs, mv.to, label));
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
        let us = self.club_short(self.club_id);
        let mut body = format!("A época {} chegou ao fim.\n\n{}", season_label(year), lines.join("\n"));
        if let Some(label) = ours {
            body += &format!("\n\nO {us} {}.", label.to_lowercase().replacen("sobem", "sobe", 1).replacen("descem", "desce", 1));
        }
        self.push_news(fresh, next_id, "competition", format!("Fim da época {}", season_label(year)), body);

        self.contracts(fresh, next_id, &format!("{year}-12-31"))?;
        self.retirements(fresh, next_id)?;
        self.fill_squads(fresh, next_id)?;
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

    /// Os `n` que sobem de uma divisão sem tabela: sorteio pesado pela força de cada clube.
    fn promoted_by_draw(&self, comp: i32, n: usize) -> Result<Vec<i32>> {
        let mut rng = self.season_rng(comp as u64);
        let mut scored = Vec::new();
        for id in league_club_ids(&self.world, comp)? {
            let s = self.world.team_strength(id)?;
            scored.push((id, (s.attack + s.midfield + s.defence) * (0.85 + 0.3 * uniform(&mut rng))));
        }
        scored.sort_by(|x, y| y.1.total_cmp(&x.1).then(x.0.cmp(&y.0)));
        Ok(scored.into_iter().take(n).map(|s| s.0).collect())
    }

    /// Contratos que acabam até `cutoff`: renova quem está entre os melhores do plantel e não
    /// é velho; os outros saem livres. No clube do treinador decide o adjunto (a negociação é
    /// do 3.7). Os contratos sem data só se tratam no fim da época.
    fn contracts(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32, cutoff: &str) -> Result<()> {
        let today = self.date.clone();
        let year_end = today.ends_with("12-20");
        let year: i32 = today[0..4].parse().unwrap_or(0);
        let mut rng = self.season_rng(0xC0DE);
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
                match expires.as_deref() {
                    Some(e) if e > cutoff => continue,
                    None if !year_end => continue,
                    _ => {}
                }
                let age = age_on(birth.as_deref(), birth_year, &today);
                // Fica quem é dos melhores do plantel; os veteranos só se forem titulares e os
                // juniores só se já forem do grupo principal
                let keep = match age {
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
                let mut body = String::from("O seu adjunto tratou dos contratos que acabavam esta época.");
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
    fn retirements(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let today = self.date.clone();
        let mut rng = self.season_rng(0x0E7E);
        let sql = format!(
            "SELECT p.id, p.display_name, p.club_id, p.birth_date, p.birth_year, pl.pos_goalkeeper
             FROM people p JOIN players pl ON pl.person_id = p.id
             WHERE p.is_player = 1 AND p.id NOT IN (SELECT person_id FROM retired)
               AND (p.club_id IN (SELECT id FROM clubs WHERE division_id IN ({MANAGED_SQL}))
                    OR (p.club_id IS NULL AND p.nation_id IN (SELECT DISTINCT nation_id FROM clubs WHERE division_id IN ({MANAGED_SQL}))))
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
    fn fill_squads(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let today = self.date.clone();
        let year: i32 = today[0..4].parse().unwrap_or(0);
        let mut rng = self.season_rng(0xF111);
        // Os clubes com mais reputação escolhem primeiro
        let sql = format!("SELECT id FROM clubs WHERE division_id IN ({MANAGED_SQL}) ORDER BY reputation DESC, id");
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
    fn clean_selection(&mut self) -> Result<()> {
        let Some(sel) = &mut self.selection else { return Ok(()) };
        let squad: Vec<i32> = self
            .world
            .conn
            .prepare("SELECT id FROM people WHERE club_id = ? AND is_player = 1")?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for s in &mut sel.starters {
            if s.is_some_and(|id| !squad.contains(&id)) {
                *s = None;
            }
        }
        sel.bench.retain(|id| squad.contains(id));
        Ok(())
    }

    // ---- Nova época (1 de janeiro) ----

    fn new_season(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let season = self.season();
        self.world.start_year = season; // as idades nas telas contam a partir daqui
        for st in self.squads.states.values_mut() {
            st.apps = 0;
            st.clear_cards();
        }
        self.schedule_season(season, fresh, next_id)?;
        // Clubes pequenos sem jogadores no patch ganham plantel, como no CM
        self.complete_squads()?;
        self.refresh_live()?;
        if let Some(n) = self.season_news(*next_id) {
            fresh.push(n);
            *next_id += 1;
        }
        Ok(())
    }

    /// Volta a ler os clubes vivos (os que jogam nesta época, mais o do treinador): nomes,
    /// números de camisola e os atributos que o estado dos jogadores usa.
    pub(super) fn refresh_live(&mut self) -> Result<()> {
        let year = self.season();
        let mut live: Vec<i32> =
            self.matches.iter().filter(|m| season_of(&m.date) == year).flat_map(|m| [m.home_id, m.away_id]).chain([self.club_id]).collect();
        live.sort_unstable();
        live.dedup();
        fix_squad_numbers(&self.world, &live, self.seed)?;
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
             WHERE c.division_id IN ({MANAGED_SQL}) AND p.is_player = 1 ORDER BY p.id"
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
        let sql = format!("SELECT id, nation_id, reputation FROM clubs WHERE division_id IN ({MANAGED_SQL}) ORDER BY id");
        let clubs: Vec<(i32, Option<i32>, Option<i32>)> =
            self.world.conn.prepare(&sql)?.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<_>>()?;
        let mut factory = self.player_factory()?;
        let mut ours = Vec::new();
        for (club, nation, reputation) in clubs {
            // O potencial dos juniores vem da reputação do clube, que não muda no save. Medido no
            // patch: PA médio dos 11 melhores ≈ 84 + 0,0114 × reputação (Série A ≈ 147, B ≈ 129,
            // C ≈ 115). Tirado do próprio plantel haveria uma espiral (cada geração puxava a seguinte)
            let level = 84.0 + 0.0114 * reputation.unwrap_or(3000) as f64;
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
    use crate::league::SERIE_B;
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

    /// O "pronto quando" do 3.6: a época de 2026 inteira, com todas as competições.
    #[test]
    fn a_full_2026_season() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("2026");
        let mut g = Game::create(&world, &dir, &new_game(7568), 5).unwrap();
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
        // Todas as competições acabaram, com campeão no historial
        for r in regulation::all() {
            assert!(g.podium(r.competition, 2026).is_some(), "competição {} sem campeão", r.competition);
        }
        let ids: Vec<String> = regulation::all().iter().map(|r| r.competition.to_string()).collect();
        let sql = format!("SELECT COUNT(*) FROM competition_history WHERE year = 2026 AND competition_id IN ({})", ids.join(","));
        let history: i64 = g.world.conn.query_row(&sql, [], |r| r.get(0)).unwrap();
        assert_eq!(history as usize, regulation::all().len());
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
