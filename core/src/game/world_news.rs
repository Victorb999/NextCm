//! As notícias do resto do mundo, como no CM, conforme as assinaturas (`news_subs.rs`): as
//! lesões graves das ligas escolhidas, os resultados que chamam a atenção e as lesões e os
//! castigos dos jogadores da Lista Preferencial. Também as lesões fora dos jogos nos plantéis
//! da IA (as do treinador vêm do treino, em `training.rs`).

use std::collections::{HashMap, HashSet};

use rusqlite::{OptionalExtension, params};

use super::news_subs::{INJURIES, RESULTS, SHORTLIST};
use super::{Game, NewsItem, friendlies, internationals};
use crate::Result;
use crate::engine::uniform;
use crate::squad::Event;

/// Probabilidade por dia de uma lesão fora dos jogos (Propensão a Lesões 10) nos plantéis da IA.
const AI_INJURY_PER_DAY: f64 = 0.0012;
/// Uma lesão é notícia no mundo a partir de tantos dias de baixa (4 meses: as que tiram a temporada).
const SERIOUS_DAYS: i32 = 120;
/// Reputação (1–200) de um jogador cuja lesão grave é notícia em qualquer liga escolhida...
const STAR_REP: i32 = 140;
/// ... e na liga do treinador.
const LEAGUE_REP: i32 = 100;
/// Resultados de impacto por dia, no máximo.
const MAX_HEADLINES: usize = 3;

impl Game {
    pub(super) fn shortlist_ids(&self) -> Result<HashSet<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT person_id FROM shortlist")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// "Lesão grave" de um jogador de outro clube, se for notícia: os da Lista Preferencial
    /// (qualquer lesão) e, de baixa longa, as estrelas e os bons jogadores da liga do treinador.
    pub(super) fn world_injury_news(
        &self,
        id: i32,
        player: i32,
        club: i32,
        injury: &str,
        days: i32,
        in_match: bool,
    ) -> Result<Option<NewsItem>> {
        if club == self.club_id {
            return Ok(None);
        }
        let listed = self.shortlist_sub()? && self.in_shortlist(player)?;
        // As lesões graves das ligas assinadas: as estrelas e, na liga do treinador, os bons
        let division = self.division_of(club)?;
        let notable = days >= SERIOUS_DAYS
            && match division {
                Some(d) if self.league_sub(d, INJURIES)? => {
                    let rep: i32 = self
                        .world
                        .conn
                        .query_row("SELECT COALESCE(current_reputation, 0) FROM players WHERE person_id = ?", [player], |r| r.get(0))
                        .optional()?
                        .unwrap_or(0);
                    rep >= STAR_REP || (rep >= LEAGUE_REP && division == self.division_of(self.club_id)?)
                }
                _ => false,
            };
        if !listed && !notable {
            return Ok(None);
        }
        let name = self.person_name(player)?;
        let team = self.club(club).short_name;
        let when = if in_match { "no jogo de hoje" } else { "no treino" };
        let time = match days {
            ..14 => format!("cerca de {days} dias"),
            14..60 => format!("cerca de {} semanas", (days + 3) / 7),
            _ => format!("cerca de {} meses", (days + 15) / 30),
        };
        let title = if notable { format!("Lesão grave: {name}") } else { format!("{name} lesionado") };
        let mut body = format!(
            "{name} ({team}) se lesionou {when}: {}.\n\nO departamento médico do {team} prevê {time} de recuperação.",
            injury.to_lowercase()
        );
        if listed {
            body += &format!("\n\n{name} está na sua Lista Preferencial.");
        }
        let mut n = self.news_item(id, "injury", title, body);
        n.quiet = !((notable && self.news_stops(INJURIES)?) || (listed && self.news_stops(SHORTLIST)?));
        Ok(Some(n))
    }

    /// Lesões e castigos dos jogos de hoje fora do clube do treinador que são notícia.
    pub(super) fn world_event_news(&self, id: i32, event: &Event, shortlist: &HashSet<i32>) -> Result<Option<NewsItem>> {
        match event {
            Event::Injured { player, club, injury, days } => self.world_injury_news(id, *player, *club, injury, *days, true),
            Event::Suspended { player, club, red } if *club != self.club_id && shortlist.contains(player) && self.shortlist_sub()? => {
                let name = self.person_name(*player)?;
                let why = if *red { "foi expulso" } else { "levou o terceiro cartão amarelo" };
                let body = format!(
                    "{name} ({}) {why} e cumpre um jogo de suspensão.\n\n{name} está na sua Lista Preferencial.",
                    self.club(*club).short_name
                );
                let mut n = self.news_item(id, "injury", format!("{name} suspenso"), body);
                n.quiet = !self.news_stops(SHORTLIST)?;
                Ok(Some(n))
            }
            _ => Ok(None),
        }
    }

    /// As lesões fora dos jogos nos plantéis da IA (um treino, um azar em casa): mais ou menos
    /// uma por clube e mês, como no CM. O clube do treinador tem as do treino.
    pub(super) fn ai_injuries_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let mut rng = self.season_rng(0x1A0D_17E5);
        let mut ids: Vec<i32> = self.squads.traits.iter().filter(|(_, t)| t.club_id != self.club_id).map(|(&id, _)| id).collect();
        ids.sort_unstable();
        let today = self.date.clone();
        for id in ids {
            // Sorteio com a propensão máxima (20); só os que passam leem a deles
            let roll = uniform(&mut rng);
            if roll >= 2.0 * AI_INJURY_PER_DAY || self.squads.state(id).injury.is_some() {
                continue;
            }
            let proneness: i32 = self
                .world
                .conn
                .query_row("SELECT COALESCE(injury_proneness, 10) FROM players WHERE person_id = ?", [id], |r| r.get(0))
                .optional()?
                .unwrap_or(10);
            if roll >= AI_INJURY_PER_DAY * proneness.clamp(1, 20) as f64 / 10.0 {
                continue;
            }
            let Some((injury, days)) = self.squads.training_injury(id, &today, &mut rng) else { continue };
            self.world.conn.execute(
                "INSERT INTO injury_log (person_id, date, injury, kind, days) VALUES (?, ?, ?, 'Treino', ?)",
                params![id, today, injury, days],
            )?;
            let club = self.squads.traits[&id].club_id;
            if let Some(n) = self.world_injury_news(*next_id, id, club, &injury, days, false)? {
                fresh.push(n);
                *next_id += 1;
            }
        }
        Ok(())
    }

    /// As competições (oficiais, de clubes) em que o clube do treinador joga nesta temporada.
    pub(super) fn own_competitions(&self) -> HashSet<i32> {
        self.season_matches()
            .filter(|m| {
                (m.home_id == self.club_id || m.away_id == self.club_id)
                    && m.competition_id != friendlies::CLUB_FRIENDLY
                    && !internationals::is_national_comp(m.competition_id)
            })
            .map(|m| m.competition_id)
            .collect()
    }

    /// Os líderes das ligas com os resultados assinados antes dos jogos de hoje.
    pub(super) fn leaders(&self) -> Result<HashMap<i32, i32>> {
        if self.sacked() {
            return Ok(HashMap::new());
        }
        Ok(self
            .subscribed(RESULTS)?
            .into_iter()
            .filter(|&c| crate::regulation::get(c).is_some_and(|r| !r.is_cup()))
            .filter_map(|c| self.table(c).first().filter(|r| r.played > 0).map(|r| (c, r.club.id)))
            .collect())
    }

    /// Os resultados de hoje que chamam a atenção nas competições assinadas (sem o jogo do
    /// treinador): o líder derrotado, a zebra e a goleada. No máximo [`MAX_HEADLINES`].
    pub(super) fn result_headlines(
        &self,
        played: &[usize],
        leaders: &HashMap<i32, i32>,
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
    ) -> Result<()> {
        if self.sacked() {
            return Ok(());
        }
        let mine = self.subscribed(RESULTS)?;
        let quiet = !self.news_stops(RESULTS)?;
        let rep = |club: i32| -> i32 {
            self.world.conn.query_row("SELECT COALESCE(reputation, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0)).unwrap_or(0)
        };
        // (prioridade, título, texto)
        let mut found: Vec<(i32, String, String)> = Vec::new();
        for &i in played {
            let m = &self.matches[i];
            let Some((hg, ag)) = m.result else { continue };
            if !mine.contains(&m.competition_id) || m.home_id == self.club_id || m.away_id == self.club_id {
                continue;
            }
            let comp = self.competition_name(m.competition_id).to_string();
            let (home, away) = (self.club(m.home_id).short_name, self.club(m.away_id).short_name);
            let score = format!("{hg}-{ag}");
            if hg == ag {
                continue;
            }
            let (winner, loser, w_name, l_name) =
                if hg > ag { (m.home_id, m.away_id, &home, &away) } else { (m.away_id, m.home_id, &away, &home) };
            let place = if winner == m.home_id { "em casa" } else { "fora" };
            if leaders.get(&m.competition_id) == Some(&loser) {
                let table = self.table(m.competition_id);
                let after = match table.first() {
                    Some(r) if r.club.id == loser => format!("Mesmo assim, o {l_name} continua na liderança, com {} pontos.", r.points),
                    Some(r) => format!("A liderança passa para o {}, com {} pontos.", r.club.short_name, r.points),
                    None => String::new(),
                };
                let body = format!("{comp}: o líder {l_name} perdeu para o {w_name} por {score}.\n\n{after}");
                found.push((0, format!("Líder {l_name} tropeça"), body.trim_end().to_string()));
                continue;
            }
            let (rw, rl) = (rep(winner), rep(loser));
            if rl - rw >= 2000 && rl >= 5000 {
                let body = format!("{comp}: o {w_name} surpreendeu o favorito {l_name} e venceu {place} por {score}.");
                found.push((1, format!("Zebra: {w_name} vence o {l_name}"), body));
            } else if (hg as i32 - ag as i32).abs() >= 4 {
                let body = format!("{comp}: o {w_name} goleou o {l_name} {place} por {score}.");
                found.push((2, format!("{w_name} goleia o {l_name}"), body));
            }
        }
        found.sort_by_key(|f| f.0);
        for (_, title, body) in found.into_iter().take(MAX_HEADLINES) {
            let mut n = self.news_item(*next_id, "competition", title, body);
            n.quiet = quiet;
            fresh.push(n);
            *next_id += 1;
        }
        Ok(())
    }
}
