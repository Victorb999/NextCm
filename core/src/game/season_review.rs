//! O resumo da temporada do treinador (4.0 item 27): quando a liga do clube dele acaba, uma
//! notícia com o lugar na liga, as copas (o título ou até onde foi), as compras e as vendas, o
//! artilheiro e o melhor jogador do elenco e a reputação do treinador antes e depois.

use rusqlite::{OptionalExtension, params};

use super::market::money_text;
use super::{Game, NewsItem};
use crate::regulation::{self, StageKind};
use crate::{Result, date};

/// Save 75: a reputação do treinador no começo da temporada (posta no fim de cada resumo).
pub(super) const SEASON_REP_SCHEMA: &str = "ALTER TABLE manager_info ADD COLUMN season_rep INTEGER;";

/// Jogos mínimos para o melhor jogador (a nota média).
const BEST_GAMES: i64 = 10;

impl Game {
    /// Guarda a reputação do treinador como a do começo da temporada.
    pub(super) fn mark_season_rep(&self) -> Result<()> {
        let rep = self.manager_rep()?;
        self.world.conn.execute("UPDATE manager_info SET season_rep = ? WHERE id = 1", [rep])?;
        Ok(())
    }

    /// A fase de um jogo de copa: "Quartas de final", "Fase de grupos".
    fn phase_name(&self, comp: i32, stage: u8, round: i32) -> String {
        let Some(st) = regulation::get(comp).and_then(|r| r.stages.get(stage as usize).cloned()) else { return String::new() };
        match st.kind {
            StageKind::Knockout => {
                st.rounds.get((round - 1).max(0) as usize).map(|r| r.name.clone()).filter(|n| !n.is_empty()).unwrap_or(st.name)
            }
            _ if st.name.is_empty() => "Fase de grupos".into(),
            _ => st.name,
        }
    }

    /// O resumo, quando a competição que acabou hoje é a liga do clube do treinador.
    pub(super) fn season_review(&mut self, comp: i32, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if self.sacked() || self.division_of(self.club_id)? != Some(comp) {
            return Ok(());
        }
        let club = self.club_id;
        let year = self.current_season(comp);
        let label = regulation::season_label(comp, year);
        let short = self.club(club).short_name;
        let from = self.season_start(Some(club))?;
        let mut lines: Vec<String> = Vec::new();

        // A liga
        let podium = self.podium(comp, year);
        let last_stage = self.year_matches(comp, year).map(|m| m.stage).max().unwrap_or(0);
        let league_table =
            regulation::get(comp).and_then(|r| r.stages.get(last_stage as usize)).is_some_and(|s| s.kind == StageKind::League);
        let place = if podium.is_some_and(|p| p.0 == club) {
            Some(1)
        } else if league_table {
            self.stage_table(comp, year, last_stage as usize, None).iter().find(|r| r.club.id == club).map(|r| r.position)
        } else {
            None
        };
        let league = self.competition_name(comp).to_string();
        lines.push(match place {
            Some(1) => format!("{league}: campeão."),
            Some(p) => format!("{league}: {p}º lugar."),
            None => format!("{league}: {}.", self.phase_name(comp, last_stage, 0)),
        });

        // As copas e as continentais da temporada
        let start = date::parse(&from).unwrap_or(0);
        let mut comps: Vec<i32> = Vec::new();
        for m in &self.matches {
            if (m.home_id == club || m.away_id == club)
                && m.competition_id != comp
                && m.competition_id != regulation::CLUB_FRIENDLY
                && date::parse(&m.date).is_some_and(|d| d >= start)
                && !comps.contains(&m.competition_id)
            {
                comps.push(m.competition_id);
            }
        }
        let mut titles = usize::from(place == Some(1));
        for c in comps {
            let season = regulation::season_of(c, &self.date);
            let ours: Vec<_> = self
                .matches
                .iter()
                .filter(|m| {
                    m.competition_id == c && (m.home_id == club || m.away_id == club) && date::parse(&m.date).is_some_and(|d| d >= start)
                })
                .collect();
            let Some(last) = ours.iter().max_by_key(|m| (date::parse(&m.date), m.stage, m.round)) else { continue };
            let name = self.competition_name(c).to_string();
            let phase = self.phase_name(c, last.stage, last.round);
            let done = self.podium(c, season);
            lines.push(match done {
                Some((w, ..)) if w == club => {
                    titles += 1;
                    format!("{name}: campeão.")
                }
                Some((_, Some(r), _)) if r == club && phase.starts_with("Final") => format!("{name}: vice-campeão."),
                Some(_) => format!("{name}: caiu em {}.", phase.to_lowercase()),
                None if last.result.is_none() => format!("{name}: em andamento ({}).", phase.to_lowercase()),
                None => format!("{name}: em andamento."),
            });
        }

        // O mercado
        let (buys, spent): (i64, i64) = self.world.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(fee), 0) FROM transfers WHERE to_club = ? AND date >= ? AND kind IN ('transfer', 'free', 'loan')",
            params![club, from],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let (sales, received): (i64, i64) = self.world.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(fee), 0) FROM transfers WHERE from_club = ? AND date >= ? AND kind IN ('transfer', 'free')",
            params![club, from],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let balance = received - spent;
        let market = format!(
            "Contratações: {buys} ({}). Saídas: {sales} ({}). Saldo: {}{}.",
            money_text(spent),
            money_text(received),
            if balance < 0 { "-" } else { "" },
            money_text(balance.abs())
        );

        // Os jogadores
        let scorer: Option<(String, i64)> = self
            .world
            .conn
            .query_row(
                "SELECT p.display_name, SUM(m.goals) g FROM player_matches m JOIN people p ON p.id = m.person_id
                 WHERE m.club = ? AND m.date >= ? GROUP BY m.person_id HAVING g > 0 ORDER BY g DESC LIMIT 1",
                params![club, from],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let best: Option<(String, f64, i64)> = self
            .world
            .conn
            .query_row(
                "SELECT p.display_name, AVG(m.rating) a, COUNT(*) n FROM player_matches m JOIN people p ON p.id = m.person_id
                 WHERE m.club = ? AND m.date >= ? GROUP BY m.person_id HAVING n >= ? ORDER BY a DESC LIMIT 1",
                params![club, from, BEST_GAMES],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;

        // A reputação
        let before: Option<i32> =
            self.world.conn.query_row("SELECT season_rep FROM manager_info WHERE id = 1", [], |r| r.get(0)).optional()?.flatten();
        let now = self.manager_rep()?;
        let level = |r: i32| super::manager_career::rep_level(r).to_lowercase();
        let rep = match before {
            Some(b) if level(b) != level(now) => format!("A reputação do treinador passou de {} para {}.", level(b), level(now)),
            Some(b) if now > b => format!("A reputação do treinador subiu na temporada (continua {}).", level(now)),
            Some(b) if now < b => format!("A reputação do treinador caiu na temporada (continua {}).", level(now)),
            Some(_) => format!("A reputação do treinador ficou igual ({}).", level(now)),
            None => String::new(),
        };
        self.mark_season_rep()?;

        let mut body = format!("O balanço da temporada {label} do {short}.\n\n{}\n\n{market}", lines.join("\n"));
        if let Some((name, goals)) = scorer {
            body.push_str(&format!("\nArtilheiro: {name}, {goals} {}.", if goals == 1 { "gol" } else { "gols" }));
        }
        if let Some((name, avg, games)) = best {
            body.push_str(&format!("\nMelhor jogador: {name}, nota média {avg:.2} em {games} jogos."));
        }
        if !rep.is_empty() {
            body.push_str(&format!("\n\n{rep}"));
        }
        let title = match titles {
            0 => format!("Resumo da temporada {label}"),
            1 => format!("Resumo da temporada {label}: um título"),
            n => format!("Resumo da temporada {label}: {n} títulos"),
        };
        self.push_news(fresh, next_id, "board", title, body);
        Ok(())
    }
}
