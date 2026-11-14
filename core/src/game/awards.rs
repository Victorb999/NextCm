//! Prémios individuais (marco 4.4): os do mês (dia 1, nas ligas jogáveis), os da época (quando
//! cada liga acaba, também as de fundo) e a Bola de Ouro (10/12). Ficam em `awards`; os dos
//! jogadores aparecem no Historial ao lado dos títulos e os do treinador no Currículo. Ganhar
//! sobe a reputação (e, por ela, o valor de mercado e o salário pedido).
//!
//! As ligas jogáveis têm as notas dos jogos (`player_matches`); as de fundo só os resultados,
//! por isso os prémios delas saem da classificação e dos plantéis (o CA e a reputação dos
//! jogadores dos clubes de cima). Regras e números em docs/roadmap/4.4-premios.md.

use std::collections::HashMap;

use rusqlite::params;

use super::internationals::is_national_comp;
use super::leagues::LeagueMode;
use super::market::role_of;
use super::season::age_on;
use super::{Game, NewsItem};
use crate::regulation::{self, StageKind};
use crate::{Result, date};

/// Save 66: os prémios (`competition` 0 na Bola de Ouro; `rank` o lugar, 1 nos outros; `value`
/// a nota média, os golos ou a pontuação).
pub(super) const AWARDS_SCHEMA: &str = "
CREATE TABLE awards (person_id INTEGER NOT NULL, date TEXT NOT NULL, competition INTEGER NOT NULL, kind TEXT NOT NULL,
                     club INTEGER, rank INTEGER NOT NULL DEFAULT 1, value REAL);
CREATE INDEX awards_person ON awards (person_id);
CREATE INDEX awards_competition ON awards (competition, date);
";

/// Jogos no mês para os prémios do mês (jogadores e treinador).
const MONTH_GAMES: i64 = 3;
/// Idade máxima do Jovem do Mês e do Melhor Jovem.
const YOUNG_AGE: i32 = 21;
/// Parte dos jogos do clube que um jogador tem de ter feito para os prémios da época.
const SEASON_SHARE: f64 = 0.5;
/// O dia da Bola de Ouro e quantos ficam na lista.
pub(super) const BALLON_DAY: &str = "12-10";
const BALLON_TOP: usize = 10;
/// Os candidatos à Bola de Ouro: CA ou reputação mundial a partir disto.
const BALLON_LEVEL: i32 = 140;
/// A Equipa da Época, em 4-4-2: (papel, quantos).
const TEAM: [(usize, usize); 4] = [(0, 1), (1, 4), (2, 4), (3, 2)];
/// Pontos por jogo esperados do clube com mais e com menos reputação da liga (o treinador).
const EXPECTED_PPG: (f64, f64) = (2.0, 0.8);
/// Nas ligas de fundo, o clube campeão vale +20% no CA dos jogadores (o último, nada).
const TABLE_BONUS: f64 = 0.2;

/// Um prémio, como aparece no Historial ("Jogador do Mês", "Melhor Marcador"...).
pub(super) fn award_label(kind: &str) -> &'static str {
    match kind {
        "player_month" => "Jogador do Mês",
        "young_month" => "Jovem do Mês",
        "manager_month" => "Treinador do Mês",
        "player_season" => "Melhor Jogador",
        "young_season" => "Melhor Jovem",
        "top_scorer" => "Melhor Marcador",
        "best_keeper" => "Melhor Guarda-Redes",
        "manager_season" => "Treinador do Ano",
        "team_season" => "Equipa da Época",
        _ => "Bola de Ouro",
    }
}

/// Reputação (actual, mundial; 0–200) que um jogador ganha com um prémio, antes do peso da liga.
fn player_bump(kind: &str, rank: i32) -> (f64, f64) {
    match kind {
        "player_month" => (2.0, 1.0),
        "young_month" => (1.0, 0.0),
        "player_season" => (8.0, 5.0),
        "young_season" => (5.0, 3.0),
        "top_scorer" | "best_keeper" => (5.0, 3.0),
        "team_season" => (2.0, 1.0),
        "ballon" => match rank {
            1 => (10.0, 15.0),
            2 | 3 => (5.0, 8.0),
            _ => (2.0, 4.0),
        },
        _ => (0.0, 0.0),
    }
}

/// O mesmo para um treinador da IA (0–200); o do jogo (0–10 000) ganha 50 vezes isto.
fn manager_bump(kind: &str) -> f64 {
    if kind == "manager_season" { 8.0 } else { 2.0 }
}

/// Um jogador nas contas de um prémio.
struct Candidate {
    id: i32,
    club: i32,
    games: i64,
    rating: f64,
    goals: i64,
    age: i32,
    role: usize,
}

/// O melhor por `score` (empate: o de menor id, para ser sempre o mesmo).
fn best_by<T>(items: &[T], score: impl Fn(&T) -> f64, id: impl Fn(&T) -> i32) -> Option<&T> {
    items.iter().max_by(|a, b| score(a).total_cmp(&score(b)).then(id(b).cmp(&id(a))))
}

impl Game {
    /// As ligas com prémios: a primeira fase é a liga da própria divisão (sem os estaduais, as
    /// taças e as continentais), de uma nação que se joga ou de fundo.
    fn award_leagues(&self) -> Vec<i32> {
        regulation::all()
            .iter()
            .filter(|r| {
                r.stages.first().is_some_and(|s| s.kind == StageKind::League && s.entrants.divisions == [r.competition])
                    && self.comp_nations.contains_key(&r.competition)
                    && !is_national_comp(r.competition)
                    && self.comp_mode(r.competition) != LeagueMode::Off
            })
            .map(|r| r.competition)
            .collect()
    }

    /// O treinador de um clube: o do jogo no clube dele, senão o da IA.
    fn manager_of(&self, club: i32) -> Result<Option<i32>> {
        if club == self.club_id && !self.sacked() {
            return Ok(Some(self.manager_id));
        }
        use rusqlite::OptionalExtension;
        Ok(self
            .world
            .conn
            .query_row("SELECT id FROM people WHERE club_id = ? AND club_job = 5 ORDER BY id LIMIT 1", [club], |r| r.get(0))
            .optional()?)
    }

    /// O peso da liga (reputação 1–20 da competição) nos prémios dela: 0,05 a 1.
    fn league_weight(&self, comp: i32) -> f64 {
        let rep: i32 =
            self.world.conn.query_row("SELECT COALESCE(reputation, 10) FROM competitions WHERE id = ?", [comp], |r| r.get(0)).unwrap_or(10);
        (rep as f64 / 20.0).clamp(0.05, 1.0)
    }

    /// Grava um prémio e sobe a reputação de quem o ganha (a mundial só pelo peso da liga).
    #[allow(clippy::too_many_arguments)]
    fn give_award(&self, person: i32, comp: i32, kind: &str, club: Option<i32>, rank: i32, value: f64, weight: f64) -> Result<()> {
        self.world.conn.execute(
            "INSERT INTO awards (person_id, date, competition, kind, club, rank, value) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![person, self.date, comp, kind, club, rank, value],
        )?;
        if kind.starts_with("manager") {
            let bump = manager_bump(kind) * (0.5 + 0.5 * weight);
            if person == self.manager_id {
                let up = (bump * 50.0).round() as i32;
                self.world.conn.execute("UPDATE manager_info SET reputation = MIN(10000, reputation + ?) WHERE id = 1", [up])?;
            } else {
                self.world.conn.execute(
                    "UPDATE non_players SET current_reputation = MIN(200, COALESCE(current_reputation, 0) + ?1),
                            world_reputation = MIN(200, COALESCE(world_reputation, 0) + ?2) WHERE person_id = ?3",
                    params![bump.round() as i32, (bump * weight).round() as i32, person],
                )?;
            }
        } else {
            let (current, world) = player_bump(kind, rank);
            self.world.conn.execute(
                "UPDATE players SET current_reputation = MIN(200, COALESCE(current_reputation, 0) + ?1),
                        world_reputation = MIN(200, COALESCE(world_reputation, 0) + ?2) WHERE person_id = ?3",
                params![(current * (0.5 + 0.5 * weight)).round() as i32, (world * weight).round() as i32, person],
            )?;
        }
        Ok(())
    }

    /// Os jogadores de uma liga com as notas entre duas datas (`to` exclusive).
    fn rated(&self, comp: i32, from: &str, to: &str) -> Result<Vec<Candidate>> {
        let rows: Vec<(i32, i32, i64, f64, i64, Option<String>, Option<i32>, [i32; 4])> = self
            .world
            .conn
            .prepare_cached(
                "SELECT m.person_id, MAX(m.club), COUNT(*), AVG(m.rating), SUM(m.goals), p.birth_date, p.birth_year,
                        COALESCE(pl.pos_goalkeeper, 1),
                        MAX(COALESCE(pl.pos_sweeper, 1), COALESCE(pl.pos_defender, 1), COALESCE(pl.pos_wing_back, 1)),
                        MAX(COALESCE(pl.pos_defensive_midfielder, 1), COALESCE(pl.pos_midfielder, 1), COALESCE(pl.pos_attacking_midfielder, 1)),
                        COALESCE(pl.pos_attacker, 1)
                 FROM player_matches m JOIN people p ON p.id = m.person_id JOIN players pl ON pl.person_id = p.id
                 WHERE m.competition = ?1 AND m.date >= ?2 AND m.date < ?3 GROUP BY m.person_id ORDER BY m.person_id",
            )?
            .query_map(params![comp, from, to], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    [r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?],
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .map(|(id, club, games, rating, goals, birth, year, pos)| Candidate {
                id,
                club,
                games,
                rating,
                goals,
                age: age_on(birth.as_deref(), year, &self.date),
                role: role_of(pos[0], pos[1], pos[2], pos[3]),
            })
            .collect())
    }

    /// O treinador que mais passou do esperado: pontos por jogo face ao que a reputação do
    /// clube na liga prometia (do 1.º ao último, de 2,0 a 0,8). `points`: (pontos, jogos) por clube.
    fn best_manager(&self, points: &HashMap<i32, (i32, i32)>, min_games: i32) -> Result<Option<(i32, i32, f64)>> {
        let mut clubs: Vec<(i32, i32)> = Vec::new();
        for &club in points.keys() {
            clubs.push((club, self.club_rep(club)?));
        }
        clubs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let n = clubs.len().max(2) as f64;
        let mut best: Option<(i32, f64)> = None;
        for (i, &(club, _)) in clubs.iter().enumerate() {
            let (pts, games) = points[&club];
            if games < min_games {
                continue;
            }
            let expected = EXPECTED_PPG.0 - (EXPECTED_PPG.0 - EXPECTED_PPG.1) * i as f64 / (n - 1.0);
            let score = pts as f64 / games as f64 - expected;
            if best.is_none_or(|b| score > b.1) {
                best = Some((club, score));
            }
        }
        let Some((club, score)) = best else { return Ok(None) };
        Ok(self.manager_of(club)?.map(|m| (m, club, score)))
    }

    /// Pontos e jogos de cada clube nos jogos da liga entre duas datas.
    fn points_between(&self, comp: i32, from: &str, to: &str) -> HashMap<i32, (i32, i32)> {
        let mut out: HashMap<i32, (i32, i32)> = HashMap::new();
        for m in self.matches.iter().filter(|m| m.competition_id == comp && m.date.as_str() >= from && m.date.as_str() < to) {
            let Some((h, a)) = m.result else { continue };
            let (ph, pa) = match h.cmp(&a) {
                std::cmp::Ordering::Greater => (3, 0),
                std::cmp::Ordering::Equal => (1, 1),
                std::cmp::Ordering::Less => (0, 3),
            };
            for (club, p) in [(m.home_id, ph), (m.away_id, pa)] {
                let e = out.entry(club).or_default();
                e.0 += p;
                e.1 += 1;
            }
        }
        out
    }

    /// Dia 1: os prémios do mês que acabou em cada liga jogável (só com jogos no mês).
    pub(super) fn monthly_awards(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let (y, m): (i32, i32) = (self.date[0..4].parse().unwrap_or(0), self.date[5..7].parse().unwrap_or(1));
        let (py, pm) = if m == 1 { (y - 1, 12) } else { (y, m - 1) };
        let from = format!("{py}-{pm:02}-01");
        let to = self.date.clone();
        let mine = if self.sacked() { None } else { self.division_of(self.club_id)? };
        for comp in self.award_leagues() {
            if self.comp_mode(comp) != LeagueMode::Play {
                continue;
            }
            let rated = self.rated(comp, &from, &to)?;
            let regular: Vec<&Candidate> = rated.iter().filter(|c| c.games >= MONTH_GAMES).collect();
            if regular.is_empty() {
                continue;
            }
            let weight = self.league_weight(comp);
            let score = |c: &&Candidate| c.rating + c.goals as f64 * 0.01;
            let mut lines = Vec::new();
            if let Some(c) = best_by(&regular, score, |c| c.id) {
                self.give_award(c.id, comp, "player_month", Some(c.club), 1, c.rating, weight)?;
                lines.push(self.award_line("Jogador do Mês", c.id, c.club, Some((c.rating, c.games)))?);
            }
            let young: Vec<&Candidate> = regular.iter().copied().filter(|c| c.age <= YOUNG_AGE).collect();
            if let Some(c) = best_by(&young, score, |c| c.id) {
                self.give_award(c.id, comp, "young_month", Some(c.club), 1, c.rating, weight)?;
                lines.push(self.award_line("Jovem do Mês", c.id, c.club, Some((c.rating, c.games)))?);
            }
            let points = self.points_between(comp, &from, &to);
            if let Some((manager, club, score)) = self.best_manager(&points, MONTH_GAMES as i32)? {
                self.give_award(manager, comp, "manager_month", Some(club), 1, score, weight)?;
                lines.push(self.award_line("Treinador do Mês", manager, club, None)?);
            }
            if Some(comp) == mine && !lines.is_empty() {
                let title = format!("Prémios do mês: {}", self.competition_name(comp));
                let body = format!("Os prémios de {} na {}:\n\n{}", month_name(pm), self.competition_name(comp), lines.join("\n"));
                self.push_news(fresh, next_id, "competition", title, body);
            }
        }
        self.unsaved = true;
        Ok(())
    }

    /// "Jogador do Mês: X (Clube), nota média 7,4 em 4 jogos".
    fn award_line(&self, label: &str, person: i32, club: i32, rating: Option<(f64, i64)>) -> Result<String> {
        let name = self.person_name(person)?;
        let club = self.club(club).short_name;
        Ok(match rating {
            Some((r, games)) => format!("{label}: {name} ({club}), nota média {} em {games} jogos", decimal(r)),
            None => format!("{label}: {name} ({club})"),
        })
    }

    /// Uma liga acabou: os prémios da época (pelas notas nas jogáveis, pela classificação e
    /// pelos plantéis nas de fundo).
    pub(super) fn season_awards(&mut self, comp: i32, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if !self.award_leagues().contains(&comp) {
            return Ok(());
        }
        let year = self.current_season(comp);
        let Some(start) = self.year_matches(comp, year).filter(|m| m.stage == 0).map(|m| m.date.clone()).min() else {
            return Ok(());
        };
        let weight = self.league_weight(comp);
        let end = date::add_days(&self.date, 1).unwrap_or_else(|| self.date.clone());
        // Os escolhidos: (pessoa, prémio, clube, valor, linha da notícia)
        let mut picks: Vec<(i32, &'static str, i32, f64, Option<String>)> = Vec::new();
        let mut lines: Vec<String> = Vec::new();
        if self.comp_mode(comp) == LeagueMode::Play {
            let rated = self.rated(comp, &start, &end)?;
            let most = rated.iter().map(|c| c.games).max().unwrap_or(0);
            let regular: Vec<&Candidate> = rated.iter().filter(|c| c.games as f64 >= SEASON_SHARE * most as f64 && c.games > 0).collect();
            let score = |c: &&Candidate| c.rating + c.goals as f64 * 0.002;
            if let Some(c) = best_by(&regular, score, |c| c.id) {
                let line = self.award_line("Melhor Jogador", c.id, c.club, Some((c.rating, c.games)))?;
                picks.push((c.id, "player_season", c.club, c.rating, Some(line)));
            }
            let young: Vec<&Candidate> = regular.iter().copied().filter(|c| c.age <= YOUNG_AGE).collect();
            if let Some(c) = best_by(&young, score, |c| c.id) {
                let line = self.award_line("Melhor Jovem", c.id, c.club, Some((c.rating, c.games)))?;
                picks.push((c.id, "young_season", c.club, c.rating, Some(line)));
            }
            let scorers: Vec<&Candidate> = rated.iter().filter(|c| c.goals > 0).collect();
            if let Some(c) = best_by(&scorers, |c| c.goals as f64 - c.games as f64 * 0.001, |c| c.id) {
                let line = format!("Melhor Marcador: {} ({}), {} golos", self.person_name(c.id)?, self.club(c.club).short_name, c.goals);
                picks.push((c.id, "top_scorer", c.club, c.goals as f64, Some(line)));
            }
            let keepers: Vec<&Candidate> = regular.iter().copied().filter(|c| c.role == 0).collect();
            if let Some(c) = best_by(&keepers, score, |c| c.id) {
                let line = self.award_line("Melhor Guarda-Redes", c.id, c.club, Some((c.rating, c.games)))?;
                picks.push((c.id, "best_keeper", c.club, c.rating, Some(line)));
            }
            // A Equipa da Época: os melhores de cada papel, em 4-4-2
            let mut team: Vec<String> = Vec::new();
            for (role, count) in TEAM {
                let mut pool: Vec<&Candidate> = regular.iter().copied().filter(|c| c.role == role).collect();
                pool.sort_by(|a, b| b.rating.total_cmp(&a.rating).then(a.id.cmp(&b.id)));
                for c in pool.into_iter().take(count) {
                    picks.push((c.id, "team_season", c.club, c.rating, None));
                    team.push(format!("{} ({})", self.person_name(c.id)?, self.club(c.club).short_name));
                }
            }
            if !team.is_empty() {
                lines.push(format!("Equipa da Época: {}", team.join(", ")));
            }
        } else {
            // De fundo: pelo CA e pela reputação dos jogadores, com o lugar do clube a pesar
            let table = self.stage_table(comp, year, 0, None);
            let n = table.len().max(2) as f64;
            let place: HashMap<i32, f64> =
                table.iter().map(|r| (r.club.id, 1.0 + TABLE_BONUS * (1.0 - (r.position - 1) as f64 / (n - 1.0)))).collect();
            let mut ids: Vec<i32> = place.keys().copied().collect();
            ids.sort_unstable();
            if !ids.is_empty() {
                let list: Vec<String> = ids.iter().map(|c| c.to_string()).collect();
                let rows: Vec<(i32, i32, i32, i32, Option<String>, Option<i32>)> = self
                    .world
                    .conn
                    .prepare(&format!(
                        "SELECT p.id, p.club_id, COALESCE(pl.current_ability, 1), COALESCE(pl.world_reputation, 0), p.birth_date, p.birth_year
                         FROM people p JOIN players pl ON pl.person_id = p.id
                         WHERE p.is_player = 1 AND p.club_id IN ({}) ORDER BY p.id",
                        list.join(", ")
                    ))?
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
                    .collect::<rusqlite::Result<_>>()?;
                let scored: Vec<(i32, i32, f64, i32)> = rows
                    .into_iter()
                    .map(|(id, club, ca, rep, birth, year)| {
                        let score = ca as f64 * place.get(&club).copied().unwrap_or(1.0) + rep as f64 / 10.0;
                        (id, club, score, age_on(birth.as_deref(), year, &self.date))
                    })
                    .collect();
                if let Some(&(id, club, s, _)) = best_by(&scored, |r| r.2, |r| r.0) {
                    picks.push((id, "player_season", club, s, Some(self.award_line("Melhor Jogador", id, club, None)?)));
                }
                let young: Vec<(i32, i32, f64, i32)> = scored.iter().copied().filter(|r| r.3 <= YOUNG_AGE).collect();
                if let Some(&(id, club, s, _)) = best_by(&young, |r| r.2, |r| r.0) {
                    picks.push((id, "young_season", club, s, Some(self.award_line("Melhor Jovem", id, club, None)?)));
                }
            }
        }
        // O Treinador do Ano: o que mais passou do que a reputação do clube prometia
        let points = self.points_between(comp, &start, &end);
        let most = points.values().map(|p| p.1).max().unwrap_or(0);
        if let Some((manager, club, score)) = self.best_manager(&points, most / 2)? {
            picks.push((manager, "manager_season", club, score, Some(self.award_line("Treinador do Ano", manager, club, None)?)));
        }
        let mut winners: Vec<i32> = Vec::new();
        let mut first: Vec<String> = Vec::new();
        for (person, kind, club, value, line) in picks {
            self.give_award(person, comp, kind, Some(club), 1, value, weight)?;
            winners.push(club);
            first.extend(line);
        }
        first.extend(lines);
        let lines = first;
        // A notícia: a liga do treinador, ou quando um prémio é do clube dele
        let mine = !self.sacked() && (self.division_of(self.club_id)? == Some(comp) || winners.contains(&self.club_id));
        if mine && !lines.is_empty() {
            let title = format!("Prémios da época: {}", self.competition_name(comp));
            let body = format!("Os melhores da época na {}:\n\n{}", self.competition_name(comp), lines.join("\n"));
            self.push_news(fresh, next_id, "competition", title, body);
        }
        self.unsaved = true;
        Ok(())
    }

    /// 10/12: a Bola de Ouro do ano, com os 10 primeiros. Conta o ano no clube (as notas e os
    /// golos de todas as competições), os títulos (a selecção e as continentais pesam mais),
    /// os prémios da época, o CA e a reputação mundial.
    pub(super) fn ballon_dor(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let year: i32 = self.date[0..4].parse().unwrap_or(0);
        let from = format!("{year}-01-01");
        let candidates: Vec<(i32, i32, i32, Option<i32>)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, COALESCE(pl.current_ability, 1), COALESCE(pl.world_reputation, 0), p.club_id
                 FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.is_player = 1 AND (pl.current_ability >= ?1 OR pl.world_reputation >= ?1)
                   AND p.id NOT IN (SELECT person_id FROM retired) ORDER BY p.id",
            )?
            .query_map([BALLON_LEVEL], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let leagues = self.award_leagues();
        let mut scored: Vec<(i32, Option<i32>, f64)> = Vec::new();
        for (id, ca, rep, club) in candidates {
            let (games, rating, goals): (i64, f64, i64) = self.world.conn.query_row(
                "SELECT COUNT(*), COALESCE(AVG(rating), 0), COALESCE(SUM(goals), 0) FROM (
                    SELECT rating, goals FROM player_matches WHERE person_id = ?1 AND date >= ?2
                    UNION ALL SELECT rating, goals FROM intl_apps WHERE person_id = ?1 AND date >= ?2)",
                params![id, from],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let form =
                if games >= 10 { (rating - 6.8) * 15.0 * (games as f64 / 40.0).min(1.0) + (goals as f64 * 0.25).min(15.0) } else { 0.0 };
            let titles: Vec<(i32, String)> = self
                .world
                .conn
                .prepare_cached("SELECT competition, kind FROM player_honours WHERE person_id = ? AND date >= ?")?
                .query_map(params![id, from], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            let honours: f64 = titles
                .iter()
                .map(|(comp, kind)| {
                    let worth = if is_national_comp(*comp) {
                        12.0
                    } else if !self.comp_nations.contains_key(comp) {
                        10.0
                    } else if leagues.contains(comp) {
                        6.0
                    } else {
                        3.0
                    };
                    if kind == "runner_up" { worth / 4.0 } else { worth }
                })
                .sum();
            let awards: Vec<(i32, String)> = self
                .world
                .conn
                .prepare_cached(
                    "SELECT competition, kind FROM awards WHERE person_id = ? AND date >= ? AND kind IN ('player_season', 'top_scorer')",
                )?
                .query_map(params![id, from], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            let awards: f64 = awards.iter().map(|(c, k)| if k == "player_season" { 6.0 } else { 3.0 } * self.league_weight(*c)).sum();
            let score = ca as f64 / 10.0 + rep as f64 / 10.0 + form + honours + awards;
            scored.push((id, club, score));
        }
        scored.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
        scored.truncate(BALLON_TOP);
        if scored.is_empty() {
            return Ok(());
        }
        let mut lines = Vec::new();
        for (i, &(id, club, score)) in scored.iter().enumerate() {
            self.give_award(id, 0, "ballon", club, i as i32 + 1, score, 1.0)?;
            let club = club.map(|c| format!(" ({})", self.club(c).short_name)).unwrap_or_default();
            lines.push(format!("{}. {}{club}", i + 1, self.person_name(id)?));
        }
        let winner = self.person_name(scored[0].0)?;
        let body = format!("{winner} ganhou a Bola de Ouro de {year}.\n\nOs dez primeiros:\n{}", lines.join("\n"));
        self.push_news(fresh, next_id, "competition", format!("Bola de Ouro {year}: {winner}"), body);
        self.unsaved = true;
        Ok(())
    }

    /// Os prémios da época entre duas datas, para a CLI `sim`: os da liga do treinador, da
    /// Premier e da La Liga, e o pódio da Bola de Ouro.
    pub fn awards_report(&self, from: &str, to: &str) -> Result<String> {
        let rows: Vec<(i32, i32, String, i32, Option<f64>, Option<i32>, String)> = self
            .world
            .conn
            .prepare(
                "SELECT a.person_id, a.competition, a.kind, a.rank, a.value, a.club, a.date FROM awards a
                 WHERE a.date >= ?1 AND a.date < ?2 AND a.kind NOT LIKE '%_month' AND a.kind <> 'team_season'
                   AND (a.competition IN (7, 52) OR a.competition = (SELECT division_id FROM clubs WHERE id = ?3)
                        OR (a.kind = 'ballon' AND a.rank <= 3))
                 ORDER BY a.competition, a.rowid",
            )?
            .query_map(params![from, to, self.club_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for (person, comp, kind, rank, value, club, when) in rows {
            let ca: i32 = self
                .world
                .conn
                .query_row("SELECT COALESCE(current_ability, 0) FROM players WHERE person_id = ?", [person], |r| r.get(0))
                .unwrap_or(0);
            let club = club.filter(|&c| c > 0).map(|c| self.club(c).short_name).unwrap_or_default();
            out.push(format!(
                "  {}: {} ({club}, CA {ca}, {:.1})",
                self.award_text(&when, comp, &kind, rank, value),
                self.person_name(person)?,
                value.unwrap_or(0.0)
            ));
        }
        let months: i64 = self.world.conn.query_row(
            "SELECT COUNT(*) FROM awards WHERE date >= ?1 AND date < ?2 AND kind LIKE '%_month'",
            params![from, to],
            |r| r.get(0),
        )?;
        out.push(format!("  prémios do mês: {months}"));
        Ok(out.join(
            "
",
        ))
    }

    /// Os prémios de uma pessoa, como linhas do Historial (data, clube, texto), do mais recente.
    pub(super) fn award_lines(&self, person: i32) -> Result<Vec<(String, Option<i32>, String)>> {
        let rows: Vec<(String, i32, String, Option<i32>, i32, Option<f64>)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT date, competition, kind, club, rank, value FROM awards WHERE person_id = ? ORDER BY date DESC, rowid DESC",
            )?
            .query_map([person], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .map(|(when, comp, kind, club, rank, value)| {
                let text = self.award_text(&when, comp, &kind, rank, value);
                (when, club, text)
            })
            .collect())
    }

    /// Os prémios do treinador do jogo num clube, entre duas datas, para o Currículo: os do mês
    /// contados por liga ("Treinador do Mês Série A (3×)"), os da época por extenso.
    pub(super) fn career_awards(&self, club: i32, from: &str, to: &str) -> Result<Vec<String>> {
        let rows: Vec<(String, i32, String, i32, Option<f64>)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT date, competition, kind, rank, value FROM awards
                 WHERE person_id = ?1 AND club = ?2 AND date >= ?3 AND date <= ?4 ORDER BY date, rowid",
            )?
            .query_map(params![self.manager_id, club, from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut months: Vec<(i32, usize)> = Vec::new();
        let mut out = Vec::new();
        for (when, comp, kind, rank, value) in rows {
            if kind == "manager_month" {
                match months.iter_mut().find(|m| m.0 == comp) {
                    Some(m) => m.1 += 1,
                    None => months.push((comp, 1)),
                }
            } else {
                out.push(self.award_text(&when, comp, &kind, rank, value));
            }
        }
        for (comp, n) in months {
            out.push(format!("{} {} ({n}×)", award_label("manager_month"), self.competition_name(comp)));
        }
        Ok(out)
    }

    /// "Jogador do Mês Série A (agosto 2025)", "Melhor Marcador Série A 2025 (21 golos)",
    /// "Bola de Ouro 2025: 3.º lugar".
    pub(super) fn award_text(&self, when: &str, comp: i32, kind: &str, rank: i32, value: Option<f64>) -> String {
        let year: i32 = when[0..4].parse().unwrap_or(0);
        let label = award_label(kind);
        if kind == "ballon" {
            return if rank == 1 { format!("{label} {year}") } else { format!("{label} {year}: {rank}.º lugar") };
        }
        let name = self.competition_name(comp);
        if kind.ends_with("_month") {
            let m: i32 = when[5..7].parse().unwrap_or(1);
            let (y, m) = if m == 1 { (year - 1, 12) } else { (year, m - 1) };
            return format!("{label} {name} ({} {y})", month_name(m));
        }
        let season = regulation::season_of(comp, when);
        let season = if regulation::get(comp).is_some_and(|r| r.season == regulation::SeasonKind::European) {
            format!("{season}/{:02}", (season + 1) % 100)
        } else {
            season.to_string()
        };
        match (kind, value) {
            ("top_scorer", Some(goals)) => format!("{label} {name} {season} ({} golos)", goals as i32),
            _ => format!("{label} {name} {season}"),
        }
    }
}

/// O nome do mês, como nas notícias.
fn month_name(m: i32) -> &'static str {
    const NAMES: [&str; 12] =
        ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];
    NAMES[(m.clamp(1, 12) - 1) as usize]
}

/// 7.43 → "7,4".
fn decimal(v: f64) -> String {
    format!("{:.1}", v).replace('.', ",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_seasons_and_the_ballon_dor() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-premios-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Prémios".into(),
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
        let mut g = Game::create(&path, &dir, &new, 17).unwrap();
        let serie_a = g.division_of(7568).unwrap().unwrap();
        let count = |g: &Game, sql: &str| -> i64 { g.world.conn.query_row(sql, [], |r| r.get(0)).unwrap() };
        while g.date.as_str() < "2025-09-01" {
            g.advance_day().unwrap();
        }
        // 1/9: os do mês de agosto, na Série A e nas outras ligas jogáveis
        let month = format!("SELECT COUNT(*) FROM awards WHERE competition = {serie_a} AND date = '2025-09-01'");
        assert_eq!(count(&g, &month), 3, "jogador, jovem e treinador do mês");
        assert!(count(&g, "SELECT COUNT(DISTINCT competition) FROM awards WHERE kind = 'player_month'") > 5);
        assert!(g.news.iter().any(|n| n.title.starts_with("Prémios do mês")));
        // O vencedor sobe de reputação e o prémio aparece no Historial
        let (winner, rep): (i32, i32) = g
            .world
            .conn
            .query_row(
                &format!(
                    "SELECT a.person_id, pl.current_reputation FROM awards a JOIN players pl ON pl.person_id = a.person_id
                     WHERE a.competition = {serie_a} AND a.kind = 'player_month'"
                ),
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(rep > 0);
        let mut p = g.world.get_person(winner).unwrap();
        g.fill_record(&mut p).unwrap();
        assert!(p.honours.iter().any(|h| h.text.starts_with("Jogador do Mês") && h.text.contains("agosto 2025")), "{:?}", p.honours);

        // A Série A acaba em dezembro: os da época; a 10/12, a Bola de Ouro
        while g.date.as_str() < "2025-12-11" {
            g.advance_day().unwrap();
        }
        let season = |kind: &str| format!("SELECT COUNT(*) FROM awards WHERE competition = {serie_a} AND kind = '{kind}'");
        for kind in ["player_season", "young_season", "top_scorer", "best_keeper", "manager_season"] {
            assert_eq!(count(&g, &season(kind)), 1, "{kind}");
        }
        assert_eq!(count(&g, &season("team_season")), 11);
        let ballon: Vec<(i32, i32)> = g
            .world
            .conn
            .prepare("SELECT person_id, rank FROM awards WHERE kind = 'ballon' ORDER BY rank")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(ballon.iter().map(|b| b.1).collect::<Vec<_>>(), (1..=10).collect::<Vec<_>>());
        assert!(g.news.iter().any(|n| n.title.starts_with("Bola de Ouro 2025")));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn labels_and_bumps() {
        assert_eq!(award_label("top_scorer"), "Melhor Marcador");
        assert!(player_bump("ballon", 1).1 > player_bump("ballon", 5).1);
        assert!(player_bump("player_season", 1).0 > player_bump("player_month", 1).0);
        assert_eq!(decimal(7.43), "7,4");
        assert_eq!(month_name(8), "agosto");
    }
}
