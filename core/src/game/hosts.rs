//! Os anfitriões das fases finais (Mundial, Euro, Copa América): os já decididos vêm do
//! regulamento, os seguintes escolhem-se anos antes, num dia do ano, com notícia. O anfitrião
//! entra direto na fase final, sai das eliminatórias e ocupa uma vaga do seu continente.

use std::collections::HashSet;

use rand_core::RngCore;
use rusqlite::{OptionalExtension, params};

use super::{Game, NewsItem};
use crate::Result;
use crate::regulation::{self, Regulation};

/// Os anfitriões de cada edição (save 47), pela época internacional da fase final.
pub(super) const HOSTS_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS hosts (competition INTEGER NOT NULL, season INTEGER NOT NULL, team INTEGER NOT NULL, ord INTEGER NOT NULL,
                                  PRIMARY KEY (competition, season, team));
";

/// Um índice sorteado pelos pesos.
fn weighted(rng: &mut impl RngCore, weights: &[u64]) -> usize {
    let total: u64 = weights.iter().sum();
    if total == 0 {
        return 0;
    }
    let mut r = rng.next_u64() % total;
    for (i, &w) in weights.iter().enumerate() {
        if r < w {
            return i;
        }
        r -= w;
    }
    weights.len() - 1
}

impl Game {
    /// Os anfitriões de uma edição, pela ordem do anúncio.
    pub(super) fn hosts_of(&self, comp: i32, season: i32) -> Result<Vec<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT team FROM hosts WHERE competition = ? AND season = ? ORDER BY ord")?
            .query_map(params![comp, season], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// O continente de uma selecção.
    pub(super) fn team_continent(&self, team: i32) -> Result<Option<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT n.continent_id FROM clubs c JOIN nations n ON n.id = c.nation_id WHERE c.id = ?")?
            .query_row([team], |r| r.get(0))
            .optional()?
            .flatten())
    }

    /// Quantos anfitriões de `hosts` são do continente `continent`.
    pub(super) fn hosts_in(&self, hosts: &[i32], continent: Option<i32>) -> Result<usize> {
        let mut n = 0;
        for &h in hosts {
            if continent.is_some() && self.team_continent(h)? == continent {
                n += 1;
            }
        }
        Ok(n)
    }

    /// As edições da época `year` em diante que já têm de ter anfitriões: as do regulamento e,
    /// sem notícia, as que se jogam até ao fim da época seguinte (decididas antes do jogo começar).
    pub(super) fn settle_hosts(&mut self, year: i32) -> Result<()> {
        for reg in regulation::all().iter().filter(|r| r.hosts.is_some()) {
            let rule = reg.hosts.as_ref().expect("anfitriões");
            for k in &rule.known {
                if !self.hosts_of(reg.competition, k.season)?.is_empty() {
                    continue;
                }
                let mut ids = Vec::new();
                for name in &k.teams {
                    ids.extend(self.senior_team_of(name)?);
                }
                self.store_hosts(reg.competition, k.season, &ids)?;
            }
            for season in year..=year + 1 {
                if reg.in_cycle(season) && self.hosts_of(reg.competition, season)?.is_empty() {
                    let ids = self.draw_hosts(reg, season)?;
                    self.store_hosts(reg.competition, season, &ids)?;
                }
            }
        }
        Ok(())
    }

    /// O dia do anúncio: os anfitriões das edições que entram no prazo, com notícia.
    pub(super) fn announce_hosts(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let year = regulation::season_of(super::internationals::ANY_NATIONAL, &self.date);
        for reg in regulation::all().iter().filter(|r| r.hosts.is_some()) {
            let rule = reg.hosts.as_ref().expect("anfitriões");
            if self.date.get(5..) != Some(rule.on.as_str()) {
                continue;
            }
            for season in year..=year + rule.years {
                if !reg.in_cycle(season) || !self.hosts_of(reg.competition, season)?.is_empty() {
                    continue;
                }
                let ids = self.draw_hosts(reg, season)?;
                if ids.is_empty() {
                    continue;
                }
                self.store_hosts(reg.competition, season, &ids)?;
                let names: Vec<String> = ids.iter().map(|&t| self.club(t).short_name).collect();
                let list = match names.as_slice() {
                    [one] => one.clone(),
                    [rest @ .., last] => format!("{} e {last}", rest.join(", ")),
                    [] => String::new(),
                };
                // "o Mundial 2038", "a Copa América 2028"
                let label = format!("{} {}", self.the_competition(reg.competition), regulation::season_label(reg.competition, season));
                let (verb, them) = if ids.len() > 1 {
                    ("organizam", "As selecções anfitriãs ficam apuradas")
                } else {
                    ("organiza", "A selecção anfitriã fica apurada")
                };
                self.push_news(
                    fresh,
                    next_id,
                    "competition",
                    format!("{list} {verb} {label}"),
                    format!(
                        "{} escolheu {list} para organizar {label}.\n\n{them} directamente para a fase final, sem passar pelas eliminatórias.",
                        rule.by
                    ),
                );
            }
        }
        Ok(())
    }

    fn store_hosts(&self, comp: i32, season: i32, ids: &[i32]) -> Result<()> {
        for (i, id) in ids.iter().enumerate() {
            self.world.conn.execute(
                "INSERT OR IGNORE INTO hosts (competition, season, team, ord) VALUES (?, ?, ?, ?)",
                params![comp, season, id, i as i32],
            )?;
        }
        Ok(())
    }

    /// Sorteia os anfitriões de uma edição: um continente que não organizou as duas edições
    /// anteriores (com um só continente, as nações dessas edições ficam de fora), a primeira
    /// entre as de mais reputação pesada pelo lugar e as outras do mesmo continente.
    fn draw_hosts(&self, reg: &Regulation, season: i32) -> Result<Vec<i32>> {
        let rule = reg.hosts.as_ref().expect("anfitriões");
        let every = reg.cycle.map_or(1, |c| c.every);
        let mut recent: Vec<i32> = Vec::new();
        for back in 1..=2 {
            recent.extend(self.hosts_of(reg.competition, season - back * every)?);
        }
        let recent_continents: HashSet<i32> = if rule.continents.len() > 1 {
            recent.iter().filter_map(|&t| self.team_continent(t).ok().flatten()).collect()
        } else {
            HashSet::new()
        };
        let mut pool: Vec<(i32, i32, i32)> = Vec::new();
        for &c in rule.continents.iter().filter(|c| !recent_continents.contains(c)) {
            for (id, _) in self.senior_teams(c)? {
                if recent.contains(&id) {
                    continue;
                }
                let rep: i32 = self
                    .world
                    .conn
                    .prepare_cached("SELECT COALESCE(reputation, 0) FROM clubs WHERE id = ?")?
                    .query_row([id], |r| r.get(0))?;
                pool.push((id, rep, c));
            }
        }
        pool.sort_by_key(|t| (std::cmp::Reverse(t.1), t.0));
        pool.truncate(rule.pool);
        if pool.is_empty() {
            return Ok(Vec::new());
        }
        let mut rng = self.comp_rng(reg.competition, season, 0x4057);
        let weights: Vec<u64> = (0..pool.len()).map(|i| (pool.len() - i) as u64).collect();
        let first = pool.remove(weighted(&mut rng, &weights));
        let count = 1 + weighted(&mut rng, &rule.count.iter().map(|&w| w as u64).collect::<Vec<_>>());
        let mut out = vec![first.0];
        let mut mates: Vec<(i32, i32, i32)> = pool.into_iter().filter(|t| t.2 == first.2).collect();
        while out.len() < count && !mates.is_empty() {
            let weights: Vec<u64> = (0..mates.len()).map(|i| (mates.len() - i) as u64).collect();
            out.push(mates.remove(weighted(&mut rng, &weights)).0);
        }
        Ok(out)
    }
}
