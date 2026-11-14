//! Os golos esperados (marco 4.3): a aba Estatísticas de uma competição (os jogadores e as
//! equipas na época em curso) e os golos esperados recentes de um clube, para a Análise do
//! Pré-Jogo. Os golos esperados de cada jogo são a soma da probabilidade de golo de cada
//! remate no motor por lances (`player_matches.xg`); os jogos das ligas de fundo, pelo motor
//! rápido, não os têm.

use std::collections::HashMap;

use serde::Serialize;
use ts_rs::TS;

use super::Game;
use super::player_record::xg2;
use crate::person::ClubRef;
use crate::{Result, regulation};

/// Um jogador nas estatísticas de uma competição.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStatRow {
    pub person: i32,
    pub name: String,
    pub club: ClubRef,
    pub apps: i32,
    pub goals: i32,
    pub assists: i32,
    /// Nota média.
    pub rating: f64,
    pub shots: i32,
    pub xg: f64,
    /// Homem do jogo.
    pub mom: i32,
}

/// Uma equipa nas estatísticas de uma competição: golos e golos esperados, a favor e contra.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TeamStatRow {
    pub club: ClubRef,
    pub games: i32,
    pub goals_for: i32,
    pub goals_against: i32,
    pub xg_for: f64,
    pub xg_against: f64,
}

/// A aba Estatísticas de uma competição (a época em curso).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionStats {
    pub season: String,
    pub players: Vec<PlayerStatRow>,
    pub teams: Vec<TeamStatRow>,
}

/// Os golos esperados recentes de um clube (a Análise do Pré-Jogo).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct RecentXg {
    pub games: i32,
    pub goals_for: i32,
    pub goals_against: i32,
    pub xg_for: f64,
    pub xg_against: f64,
}

/// Jogos da Análise do Pré-Jogo.
const RECENT: usize = 5;

impl Game {
    /// As estatísticas de uma competição viva na época em curso (vazias numa liga de fundo).
    pub fn competition_stats(&self, comp: i32) -> Result<CompetitionStats> {
        let year = self.current_season(comp);
        let national = super::internationals::is_national_comp(comp);
        // Os jogos das selecções ficam em `intl_apps`, com a selecção no lugar do clube e sem remates
        let (table, side, shots) = if national { ("intl_apps", "team", "0") } else { ("player_matches", "club", "m.shots") };
        type Row = (i32, i32, i32, String, i32, i32, f64, i32, f64, bool);
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare_cached(&format!(
                "SELECT m.person_id, m.fixture, m.{side}, COALESCE((SELECT nickname FROM nicknames n WHERE n.person_id = p.id), p.list_name),
                        m.goals, m.assists, m.rating, {shots}, m.xg, m.mom
                 FROM {table} m JOIN people p ON p.id = m.person_id WHERE m.competition = ?"
            ))?
            .query_map([comp], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        // Só os jogos desta época da competição
        let fixtures: HashMap<i32, (i32, i32, (u8, u8))> =
            self.year_matches(comp, year).filter_map(|m| m.result.map(|r| (m.id, (m.home_id, m.away_id, r)))).collect();
        let mut players: HashMap<i32, PlayerStatRow> = HashMap::new();
        let mut team_xg: HashMap<(i32, i32), f64> = HashMap::new();
        for (person, fixture, club, name, goals, assists, rating, shots, xg, mom) in rows {
            if !fixtures.contains_key(&fixture) {
                continue;
            }
            *team_xg.entry((fixture, club)).or_default() += xg;
            let row = players.entry(person).or_insert_with(|| PlayerStatRow {
                person,
                name,
                club: self.club(club),
                apps: 0,
                goals: 0,
                assists: 0,
                rating: 0.0,
                shots: 0,
                xg: 0.0,
                mom: 0,
            });
            // O clube do último jogo
            if row.club.id != club {
                row.club = self.club(club);
            }
            row.apps += 1;
            row.goals += goals;
            row.assists += assists;
            row.rating += rating;
            row.shots += shots;
            row.xg += xg;
            row.mom += mom as i32;
        }
        let mut players: Vec<PlayerStatRow> = players
            .into_values()
            .map(|mut p| {
                p.rating = (p.rating / p.apps.max(1) as f64 * 100.0).round() / 100.0;
                p.xg = xg2(p.xg);
                p
            })
            .collect();
        players.sort_by(|a, b| b.goals.cmp(&a.goals).then(b.apps.cmp(&a.apps)).then(a.person.cmp(&b.person)));
        let mut teams: HashMap<i32, TeamStatRow> = HashMap::new();
        for (&fixture, &(home, away, (hg, ag))) in &fixtures {
            for (club, other, gf, ga) in [(home, away, hg, ag), (away, home, ag, hg)] {
                let t = teams.entry(club).or_insert_with(|| TeamStatRow {
                    club: self.club(club),
                    games: 0,
                    goals_for: 0,
                    goals_against: 0,
                    xg_for: 0.0,
                    xg_against: 0.0,
                });
                t.games += 1;
                t.goals_for += gf as i32;
                t.goals_against += ga as i32;
                t.xg_for += team_xg.get(&(fixture, club)).copied().unwrap_or(0.0);
                t.xg_against += team_xg.get(&(fixture, other)).copied().unwrap_or(0.0);
            }
        }
        let mut teams: Vec<TeamStatRow> = teams
            .into_values()
            .map(|mut t| {
                t.xg_for = xg2(t.xg_for);
                t.xg_against = xg2(t.xg_against);
                t
            })
            .collect();
        teams.sort_by(|a, b| b.xg_for.total_cmp(&a.xg_for).then(a.club.id.cmp(&b.club.id)));
        // Numa liga de fundo não há jogadores gravados: só os golos das equipas
        Ok(CompetitionStats { season: regulation::season_label(comp, year), players, teams })
    }

    /// Os golos e golos esperados de um clube nos últimos [`RECENT`] jogos oficiais jogados
    /// pelo motor por lances (os que têm jogadores gravados).
    pub(super) fn recent_xg(&self, club: i32) -> Result<Option<RecentXg>> {
        let mut games: Vec<&crate::league::Match> = self
            .matches
            .iter()
            .filter(|m| {
                m.result.is_some()
                    && m.competition_id != super::friendlies::CLUB_FRIENDLY
                    && (m.home_id == club || m.away_id == club)
                    && m.date.as_str() <= self.date.as_str()
            })
            .collect();
        games.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
        let mut out = RecentXg { games: 0, goals_for: 0, goals_against: 0, xg_for: 0.0, xg_against: 0.0 };
        for m in games {
            if out.games as usize >= RECENT {
                break;
            }
            let national = super::internationals::is_national_comp(m.competition_id);
            let (table, side) = if national { ("intl_apps", "team") } else { ("player_matches", "club") };
            let sums: Vec<(i32, f64)> = self
                .world
                .conn
                .prepare_cached(&format!("SELECT {side}, SUM(xg) FROM {table} WHERE fixture = ? GROUP BY {side}"))?
                .query_map([m.id], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            if sums.is_empty() {
                continue;
            }
            let (hg, ag) = m.result.unwrap_or((0, 0));
            let other = if m.home_id == club { m.away_id } else { m.home_id };
            let (gf, ga) = if m.home_id == club { (hg, ag) } else { (ag, hg) };
            out.games += 1;
            out.goals_for += gf as i32;
            out.goals_against += ga as i32;
            out.xg_for += sums.iter().filter(|s| s.0 == club).map(|s| s.1).sum::<f64>();
            out.xg_against += sums.iter().filter(|s| s.0 == other).map(|s| s.1).sum::<f64>();
        }
        if out.games == 0 {
            return Ok(None);
        }
        out.xg_for = xg2(out.xg_for);
        out.xg_against = xg2(out.xg_against);
        Ok(Some(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn expected_goals_in_stats_sheet_and_analysis() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-golos-esperados-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Golos esperados".into(),
            first_name: "A".into(),
            last_name: "B".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: false,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 4).unwrap();
        let comp = g.division_of(PALMEIRAS).unwrap().unwrap();
        // Até à 2.ª jornada do Palmeiras
        let mut played = 0;
        while played < 2 {
            g.advance_day().unwrap();
            played = g
                .matches
                .iter()
                .filter(|m| m.competition_id == comp && m.result.is_some() && (m.home_id == PALMEIRAS || m.away_id == PALMEIRAS))
                .count();
        }
        let s = g.competition_stats(comp).unwrap();
        assert!(!s.players.is_empty() && !s.teams.is_empty());
        let xg_for: f64 = s.teams.iter().map(|t| t.xg_for).sum();
        let xg_against: f64 = s.teams.iter().map(|t| t.xg_against).sum();
        assert!(xg_for > 1.0 && (xg_for - xg_against).abs() < 0.05, "{xg_for} {xg_against}");
        let players: f64 = s.players.iter().map(|p| p.xg).sum();
        assert!((players - xg_for).abs() < 0.5, "{players} {xg_for}");
        // Os golos esperados andam perto dos golos (são a probabilidade de golo de cada remate)
        let goals: i32 = s.teams.iter().map(|t| t.goals_for).sum();
        assert!(xg_for > 0.5 * goals as f64 && xg_for < 2.0 * goals.max(1) as f64, "{xg_for} {goals}");
        // Na ficha do jogo do treinador e no Mais Dados
        let last = g
            .matches
            .iter()
            .filter(|m| m.competition_id == comp && m.result.is_some() && (m.home_id == PALMEIRAS || m.away_id == PALMEIRAS))
            .max_by(|a, b| a.date.cmp(&b.date))
            .unwrap()
            .id;
        let sheet = g.match_sheet(last).unwrap();
        assert!(sheet.home.iter().chain(&sheet.away).any(|l| l.xg > 0.0));
        let more = g.squad_more(PALMEIRAS, "").unwrap();
        assert!(more.iter().any(|m| m.xg > 0.0 && m.shots > 0));
        // Na Análise: os últimos jogos do Palmeiras
        let recent = g.recent_xg(PALMEIRAS).unwrap().expect("golos esperados recentes");
        assert!(recent.games >= 2 && recent.xg_for > 0.0);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
