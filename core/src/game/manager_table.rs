//! Estatísticas dos Técnicos (4.0 item 26; o "Estatuto Treinador" do menu Opções Jogo, como no
//! CM): os treinadores de clubes e de selecções por reputação num país, por reputação mundial e
//! pela Pontuação do Técnico, 16 por página.
//!
//! A pontuação: a reputação mundial × 150 (0–200), mais 30 por vitória e 10 por empate e os
//! títulos (3000 a liga, 4000 uma continental, 1500 uma copa) desde que chegou ao clube, no
//! jogo. Os da IA contam só o clube onde estão; o do treinador, todos os clubes do Currículo.

use std::collections::HashMap;

use rusqlite::OptionalExtension;
use serde::Serialize;
use ts_rs::TS;

use super::Game;
use super::internationals::is_national_comp;
use super::leagues::league_nations;
use crate::person::ClubRef;
use crate::regulation;
use crate::world::NationOption;
use crate::{Result, date};

/// Linhas por página, como no CM.
const PAGE: usize = 16;
/// Pontos da pontuação por ponto de reputação mundial (0–200).
const REP_POINTS: i64 = 150;
const WIN_POINTS: i64 = 30;
const DRAW_POINTS: i64 = 10;
const LEAGUE_TITLE: i64 = 3000;
const CONTINENTAL_TITLE: i64 = 4000;
const CUP_TITLE: i64 = 1500;

/// Um treinador na lista.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ManagerRow {
    pub rank: i32,
    pub person_id: i32,
    pub name: String,
    /// O clube ou a selecção.
    pub team: Option<ClubRef>,
    /// "Classe", "Excelente", "Boa"... (nas vistas da reputação).
    pub level: String,
    /// A pontuação (na vista da pontuação).
    #[ts(type = "number")]
    pub score: i64,
    /// O treinador do utilizador (a amarelo).
    pub me: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ManagersTable {
    /// "nation", "world" ou "score".
    pub view: String,
    /// "Reputação Itália", "Reputação Mundial", "Pontuação do Técnico".
    pub title: String,
    pub nation_id: Option<i32>,
    pub nations: Vec<NationOption>,
    pub page: i32,
    pub pages: i32,
    pub rows: Vec<ManagerRow>,
}

/// A reputação em palavras (0–200).
fn rep_words(rep: i32) -> &'static str {
    match rep {
        160.. => "Classe",
        130.. => "Excelente",
        100.. => "Boa",
        70.. => "Regular",
        40.. => "Razoável",
        _ => "Fraca",
    }
}

struct Candidate {
    id: i32,
    name: String,
    team: Option<i32>,
    nation: Option<i32>,
    joined: Option<String>,
    home_rep: i32,
    world_rep: i32,
    me: bool,
}

impl Game {
    /// Os pontos de um título.
    fn title_points(&self, comp: i32) -> i64 {
        match regulation::get(comp) {
            Some(r) if self.comp_nations.contains_key(&comp) => {
                if r.is_cup() {
                    CUP_TITLE
                } else {
                    LEAGUE_TITLE
                }
            }
            _ if is_national_comp(comp) => CUP_TITLE,
            _ => CONTINENTAL_TITLE,
        }
    }

    /// A pontuação de um clube entre duas datas (`None` = até hoje): vitórias, empates e títulos.
    fn spell_points(&self, club: i32, from: i32, to: Option<i32>, titles: &HashMap<i32, Vec<(i32, i32)>>) -> i64 {
        let to = to.unwrap_or(i32::MAX);
        let mut pts = 0;
        for m in self.matches.iter().filter(|m| m.home_id == club || m.away_id == club) {
            let (Some((h, a)), Some(d)) = (m.result, date::parse(&m.date)) else { continue };
            if d < from || d > to || m.competition_id == regulation::CLUB_FRIENDLY {
                continue;
            }
            let (us, them) = if m.home_id == club { (h, a) } else { (a, h) };
            pts += match us.cmp(&them) {
                std::cmp::Ordering::Greater => WIN_POINTS,
                std::cmp::Ordering::Equal => DRAW_POINTS,
                std::cmp::Ordering::Less => 0,
            };
        }
        for &(comp, d) in titles.get(&club).into_iter().flatten() {
            if d >= from && d <= to {
                pts += self.title_points(comp);
            }
        }
        pts
    }

    /// Estatísticas dos Técnicos: `view` "nation" (com `nation`, sem ela a do clube do
    /// treinador), "world" ou "score"; `page` `None` abre na página do treinador.
    pub fn managers_table(&self, view: &str, nation: Option<i32>, page: Option<i32>) -> Result<ManagersTable> {
        let nations: Vec<NationOption> =
            league_nations(&self.world)?.into_iter().map(|n| NationOption { id: n.nation_id, name: n.name }).collect();
        let sacked = self.sacked();
        let own_nation: Option<i32> =
            self.world.conn.query_row("SELECT nation_id FROM clubs WHERE id = ?", [self.club_id], |r| r.get(0)).optional()?.flatten();

        let mut people: Vec<Candidate> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, p.display_name, p.club_id, p.club_job, p.national_team_id, p.club_joined,
                        COALESCE(n.current_reputation, 0), COALESCE(n.world_reputation, 0), c.nation_id, nt.nation_id
                 FROM people p JOIN non_players n ON n.person_id = p.id
                 LEFT JOIN clubs c ON c.id = p.club_id LEFT JOIN clubs nt ON nt.id = p.national_team_id
                 WHERE p.id <> ? AND ((p.club_job = 5 AND p.club_id IS NOT NULL) OR p.national_team_job = 5)",
            )?
            .query_map([self.manager_id], |r| {
                let club_job: Option<i32> = r.get(3)?;
                let club: Option<i32> = r.get(2)?;
                let at_club = club_job == Some(5) && club.is_some();
                Ok(Candidate {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    team: if at_club { club } else { r.get(4)? },
                    nation: if at_club { r.get(8)? } else { r.get(9)? },
                    joined: if at_club { r.get(5)? } else { None },
                    home_rep: r.get(6)?,
                    world_rep: r.get(7)?,
                    me: false,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mine = (self.manager_rep()? / 50).clamp(0, 200);
        people.push(Candidate {
            id: self.manager_id,
            name: self.person_name(self.manager_id)?,
            team: (!sacked).then_some(self.club_id),
            nation: if sacked { None } else { own_nation },
            joined: None,
            home_rep: mine,
            world_rep: mine,
            me: true,
        });

        let view = match view {
            "world" | "score" => view,
            _ => "nation",
        };
        let nation = if view == "nation" { nation.or(own_nation).or(nations.first().map(|n| n.id)) } else { nation };
        let mut scored: Vec<(Candidate, i64)> = match view {
            "nation" => people
                .into_iter()
                .filter(|c| c.nation.is_some() && c.nation == nation)
                .map(|c| {
                    let v = c.home_rep as i64;
                    (c, v)
                })
                .collect(),
            "world" => people
                .into_iter()
                .map(|c| {
                    let v = c.world_rep as i64;
                    (c, v)
                })
                .collect(),
            _ => {
                // Os títulos de cada clube no jogo (o dia em que acabou a competição)
                let mut titles: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();
                let rows: Vec<(i32, i32, String)> = self
                    .world
                    .conn
                    .prepare_cached("SELECT DISTINCT club, competition, date FROM player_honours WHERE kind IN ('champion', 'winner')")?
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                    .collect::<rusqlite::Result<_>>()?;
                for (club, comp, d) in rows {
                    if let Some(d) = date::parse(&d) {
                        titles.entry(club).or_default().push((comp, d));
                    }
                }
                let spells: Vec<(i32, String, Option<String>)> = self
                    .world
                    .conn
                    .prepare_cached("SELECT club_id, joined, left FROM manager_history ORDER BY id")?
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                    .collect::<rusqlite::Result<_>>()?;
                people
                    .into_iter()
                    .map(|c| {
                        let mut pts = c.world_rep as i64 * REP_POINTS;
                        if c.me {
                            for (club, joined, left) in &spells {
                                let from = date::parse(joined).unwrap_or(0);
                                let to = left.as_deref().and_then(date::parse);
                                pts += self.spell_points(*club, from, to, &titles);
                            }
                        } else if let Some(team) = c.team {
                            let from = c.joined.as_deref().and_then(date::parse).unwrap_or(0);
                            pts += self.spell_points(team, from, None, &titles);
                        }
                        (c, pts)
                    })
                    .collect()
            }
        };
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));

        let total = scored.len();
        let pages = total.div_ceil(PAGE).max(1) as i32;
        let page = match page {
            Some(p) => p.clamp(0, pages - 1),
            None => scored.iter().position(|(c, _)| c.me).map_or(0, |i| (i / PAGE) as i32),
        };
        let rows = scored
            .iter()
            .enumerate()
            .skip(page as usize * PAGE)
            .take(PAGE)
            .map(|(i, (c, v))| ManagerRow {
                rank: i as i32 + 1,
                person_id: c.id,
                name: c.name.clone(),
                team: c.team.map(|t| self.club(t)),
                level: if view == "score" { String::new() } else { rep_words(*v as i32).into() },
                score: *v,
                me: c.me,
            })
            .collect();
        let title = match view {
            "world" => "Reputação Mundial".to_string(),
            "score" => "Pontuação do Técnico".to_string(),
            _ => {
                let name = nation.and_then(|n| nations.iter().find(|x| x.id == n)).map(|n| n.name.clone());
                let name = match (name, nation) {
                    (Some(n), _) => n,
                    (None, Some(n)) => self
                        .world
                        .conn
                        .query_row("SELECT name FROM nations WHERE id = ?", [n], |r| r.get(0))
                        .optional()?
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                format!("Reputação {name}")
            }
        };
        Ok(ManagersTable { view: view.into(), title, nation_id: nation, nations, page, pages, rows })
    }
}
