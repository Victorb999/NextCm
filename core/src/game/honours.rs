//! Quadro de Honra (4.0 item 25; menu Opções Jogo, como no CM): os campeões e os vices de cada
//! competição viva, ano a ano, por país ou as internacionais de clubes, com o Melhor Jogador e o
//! Artilheiro de cada uma (4.4) e a Bola de Ouro. Os anos antes do jogo vêm do historial do banco.

use std::collections::BTreeSet;

use serde::Serialize;
use ts_rs::TS;

use super::Game;
use super::internationals::is_national_comp;
use super::leagues::league_nations;
use crate::person::ClubRef;
use crate::regulation::{self, SeasonKind};
use crate::world::NationOption;
use crate::{Result, date};

/// Um prémio no Quadro de Honra: quem ganhou, o clube e o número (a nota ou os gols).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct HonourAward {
    pub person_id: i32,
    pub name: String,
    pub club: Option<ClubRef>,
    pub value: Option<f64>,
}

/// Uma competição num ano.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct HonourRow {
    pub competition_id: i32,
    pub competition: String,
    /// "2025/26" ou "2026".
    pub season: String,
    pub winner: Option<ClubRef>,
    pub runner_up: Option<ClubRef>,
    pub best_player: Option<HonourAward>,
    pub top_scorer: Option<HonourAward>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct HonoursBoard {
    /// O país escolhido (0 = internacionais de clubes).
    pub group: i32,
    pub groups: Vec<NationOption>,
    /// O ano escolhido (o ano em que a temporada acaba) e os que têm campeões.
    pub year: Option<i32>,
    pub years: Vec<i32>,
    pub rows: Vec<HonourRow>,
    /// A Bola de Ouro desse ano (só nas internacionais).
    pub ballon: Option<HonourAward>,
}

/// O ano do historial de uma temporada: o ano em que acaba.
fn history_year(comp: i32, season: i32) -> i32 {
    if regulation::season_kind(comp) == SeasonKind::European { season + 1 } else { season }
}

impl Game {
    /// As competições vivas de um país (0 = as internacionais de clubes), pela ordem dos regulamentos.
    fn honour_competitions(&self, group: i32) -> Vec<i32> {
        regulation::all()
            .iter()
            .map(|r| r.competition)
            .filter(|c| match self.comp_nations.get(c) {
                Some(&n) => n == group,
                None => group == 0 && !is_national_comp(*c) && *c != regulation::CLUB_FRIENDLY,
            })
            .collect()
    }

    /// O Quadro de Honra de um país (0 = internacionais) num ano (`None` = o mais recente).
    pub fn honours_board(&self, group: Option<i32>, year: Option<i32>) -> Result<HonoursBoard> {
        let mut groups: Vec<NationOption> = league_nations(&self.world)?
            .into_iter()
            .filter(|n| self.comp_nations.values().any(|&x| x == n.nation_id))
            .map(|n| NationOption { id: n.nation_id, name: n.name })
            .collect();
        groups.insert(0, NationOption { id: 0, name: "Internacionais".into() });
        // Sem escolha: o país do clube do treinador
        let own = self.division_of(self.club_id)?.and_then(|d| self.comp_nations.get(&d).copied());
        let group = group.or(own).filter(|g| groups.iter().any(|x| x.id == *g)).unwrap_or(0);
        let comps = self.honour_competitions(group);

        let mut years: BTreeSet<i32> = BTreeSet::new();
        {
            let mut st =
                self.world.conn.prepare_cached("SELECT year FROM competition_history WHERE competition_id = ? AND year IS NOT NULL")?;
            for &c in &comps {
                for y in st.query_map([c], |r| r.get::<_, i32>(0))? {
                    years.insert(y?);
                }
            }
        }
        if group == 0 {
            let mut st = self.world.conn.prepare_cached("SELECT DISTINCT substr(date, 1, 4) FROM awards WHERE competition = 0")?;
            for y in st.query_map([], |r| r.get::<_, String>(0))? {
                if let Ok(y) = y?.parse() {
                    years.insert(y);
                }
            }
        }
        let years: Vec<i32> = years.into_iter().rev().collect();
        let year = year.filter(|y| years.contains(y)).or(years.first().copied());

        let mut rows = Vec::new();
        let mut ballon = None;
        if let Some(year) = year {
            for &c in &comps {
                let hist: Option<(Option<i32>, Option<i32>)> = self
                    .world
                    .conn
                    .prepare_cached("SELECT winner_id, runner_up_id FROM competition_history WHERE competition_id = ? AND year = ?")?
                    .query_row([c, year], |r| Ok((r.get(0)?, r.get(1)?)))
                    .ok();
                let Some((winner, runner_up)) = hist else { continue };
                let season = if regulation::season_kind(c) == SeasonKind::European { year - 1 } else { year };
                rows.push(HonourRow {
                    competition_id: c,
                    competition: self.competition_name(c).into(),
                    season: regulation::season_label(c, season),
                    winner: winner.map(|w| self.club(w)),
                    runner_up: runner_up.map(|w| self.club(w)),
                    best_player: self.honour_award(c, "player_season", year)?,
                    top_scorer: self.honour_award(c, "top_scorer", year)?,
                });
            }
            if group == 0 {
                ballon = self.honour_award(0, "ballon", year)?;
            }
        }
        Ok(HonoursBoard { group, groups, year, years, rows, ballon })
    }

    /// O vencedor de um prémio da temporada de uma competição que acaba no ano do historial.
    fn honour_award(&self, comp: i32, kind: &str, year: i32) -> Result<Option<HonourAward>> {
        let rows: Vec<(i32, String, Option<i32>, Option<f64>, String)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT a.person_id, a.date, a.club, a.value, p.display_name FROM awards a JOIN people p ON p.id = a.person_id
                 WHERE a.competition = ? AND a.kind = ? AND a.rank = 1 ORDER BY a.date DESC",
            )?
            .query_map(rusqlite::params![comp, kind], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .find(|(_, d, ..)| {
                if comp == 0 {
                    d.get(0..4).and_then(|y| y.parse().ok()) == Some(year)
                } else {
                    date::parse(d).is_some() && history_year(comp, regulation::season_of(comp, d)) == year
                }
            })
            .map(|(person_id, _, club, value, name)| HonourAward { person_id, name, club: club.map(|c| self.club(c)), value }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quadro de Honra, Estatísticas dos Técnicos e o resumo da temporada num jogo novo.
    #[test]
    fn honours_managers_and_season_review() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-honra-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Honra".into(),
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
        let mut g = Game::create(&path, &dir, &new, 23).unwrap();

        // Quadro de Honra: abre no Brasil, com os campeões do historial do banco
        let b = g.honours_board(None, None).unwrap();
        assert_eq!(b.groups[0].id, 0);
        assert_ne!(b.group, 0, "abre no país do clube");
        let year = b.year.expect("anos com campeões");
        assert!(!b.rows.is_empty(), "campeões em {year}");
        assert!(b.rows.iter().all(|r| r.winner.is_some()));
        println!("{year}: {:?}", b.rows.iter().map(|r| (&r.competition, r.winner.as_ref().map(|w| &w.short_name))).collect::<Vec<_>>());
        let intl = g.honours_board(Some(0), None).unwrap();
        assert_eq!(intl.group, 0);
        assert!(intl.rows.iter().any(|r| r.competition_id == crate::league::LIBERTADORES || r.competition_id == 326));
        let older = g.honours_board(Some(b.group), b.years.get(1).copied()).unwrap();
        assert_eq!(older.year, b.years.get(1).copied());

        // Técnicos: o treinador aparece na página que abre, nas três vistas
        for view in ["nation", "world", "score"] {
            let t = g.managers_table(view, None, None).unwrap();
            assert!(t.rows.iter().any(|r| r.me), "{view}: o treinador na página {}", t.page);
            assert!(t.rows.len() <= PAGE_ROWS);
            assert!(t.rows.windows(2).all(|w| w[0].rank + 1 == w[1].rank));
        }
        let br = g.managers_table("nation", None, Some(0)).unwrap();
        assert!(br.title.starts_with("Reputação "));
        assert_eq!(br.rows[0].rank, 1);
        assert!(!br.rows[0].level.is_empty());
        let world = g.managers_table("world", None, Some(0)).unwrap();
        println!("{:?}", world.rows.iter().map(|r| (&r.name, &r.level)).collect::<Vec<_>>());
        assert!(world.pages > 100);
        // Uma vitória do Palmeiras soma 30 pontos ao treinador
        let mine = |g: &Game| g.managers_table("score", None, None).unwrap().rows.into_iter().find(|r| r.me).unwrap().score;
        let before = mine(&g);
        let i = g.matches.iter().position(|m| m.home_id == 7568 && m.result.is_none()).unwrap();
        let d = g.matches[i].date.clone();
        g.world.conn.execute("UPDATE manager_history SET joined = ?", [&g.date]).unwrap();
        assert!(d >= g.date);
        g.matches[i].result = Some((2, 0));
        assert_eq!(mine(&g), before + 30);
        g.matches[i].result = None;

        // O resumo da temporada da liga do treinador
        let league = g.division_of(7568).unwrap().unwrap();
        let (mut fresh, mut next_id) = (Vec::new(), 1_000_000);
        g.season_review(league, &mut fresh, &mut next_id).unwrap();
        let n = fresh.last().expect("o resumo");
        assert!(n.title.starts_with("Resumo da temporada"), "{}", n.title);
        assert!(n.body.contains("Contratações:"), "{}", n.body);
        assert!(n.body.contains("A reputação do treinador"), "{}", n.body);
        println!("{}\n{}", n.title, n.body);
        // Outra competição não dá resumo
        let before = fresh.len();
        g.season_review(crate::league::LIBERTADORES, &mut fresh, &mut next_id).unwrap();
        assert_eq!(fresh.len(), before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    const PAGE_ROWS: usize = 16;
}
