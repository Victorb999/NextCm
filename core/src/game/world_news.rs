//! As notícias do resto do mundo, como no CM: as lesões graves das ligas escolhidas, os
//! resultados que chamam a atenção nas competições do treinador e o que acontece aos jogadores
//! da Lista Preferencial (lesões, castigos, transferências). Também as lesões fora dos jogos
//! nos plantéis da IA (as do treinador vêm do treino, em `training.rs`).

use std::collections::{HashMap, HashSet};

use rusqlite::{OptionalExtension, params};

use super::market::money_text;
use super::{Game, NewsItem, friendlies, internationals};
use crate::Result;
use crate::engine::uniform;
use crate::squad::Event;

/// Probabilidade por dia de uma lesão fora dos jogos (Propensão a Lesões 10) nos plantéis da IA.
const AI_INJURY_PER_DAY: f64 = 0.0012;
/// Uma lesão é notícia no mundo a partir de tantos dias de baixa.
const SERIOUS_DAYS: i32 = 28;
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
        let listed = self.world.conn.query_row("SELECT 1 FROM shortlist WHERE person_id = ?", [player], |_| Ok(())).optional()?.is_some();
        let notable = days >= SERIOUS_DAYS && {
            let rep: i32 = self
                .world
                .conn
                .query_row("SELECT COALESCE(current_reputation, 0) FROM players WHERE person_id = ?", [player], |r| r.get(0))
                .optional()?
                .unwrap_or(0);
            rep >= STAR_REP
                || (rep >= LEAGUE_REP && self.division_of(club)?.is_some_and(|d| Some(d) == self.division_of(self.club_id).ok().flatten()))
        };
        if !listed && !notable {
            return Ok(None);
        }
        let name = self.person_name(player)?;
        let team = self.club(club).short_name;
        let when = if in_match { "no jogo de hoje" } else { "no treino" };
        let time = if days >= 14 { format!("cerca de {} semanas", (days + 3) / 7) } else { format!("cerca de {days} dias") };
        let title = if notable { format!("Lesão grave: {name}") } else { format!("{name} lesionado") };
        let mut body = format!(
            "{name} ({team}) se lesionou {when}: {}.\n\nO departamento médico do {team} prevê {time} de recuperação.",
            injury.to_lowercase()
        );
        if listed {
            body += &format!("\n\n{name} está na sua Lista Preferencial.");
        }
        Ok(Some(self.news_item(id, "injury", title, body)))
    }

    /// Lesões e castigos dos jogos de hoje fora do clube do treinador que são notícia.
    pub(super) fn world_event_news(&self, id: i32, event: &Event, shortlist: &HashSet<i32>) -> Result<Option<NewsItem>> {
        match event {
            Event::Injured { player, club, injury, days } => self.world_injury_news(id, *player, *club, injury, *days, true),
            Event::Suspended { player, club, red } if *club != self.club_id && shortlist.contains(player) => {
                let name = self.person_name(*player)?;
                let why = if *red { "foi expulso" } else { "levou o terceiro cartão amarelo" };
                let body = format!(
                    "{name} ({}) {why} e cumpre um jogo de suspensão.\n\n{name} está na sua Lista Preferencial.",
                    self.club(*club).short_name
                );
                Ok(Some(self.news_item(id, "injury", format!("{name} suspenso"), body)))
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

    /// Os líderes das competições do treinador antes dos jogos de hoje.
    pub(super) fn leaders(&self) -> HashMap<i32, i32> {
        if self.sacked() {
            return HashMap::new();
        }
        self.own_competitions().into_iter().filter_map(|c| self.table(c).first().filter(|r| r.played > 0).map(|r| (c, r.club.id))).collect()
    }

    /// Os resultados de hoje que chamam a atenção nas competições do treinador (sem o jogo
    /// dele): o líder derrotado, a zebra e a goleada. No máximo [`MAX_HEADLINES`].
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
        let mine = self.own_competitions();
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
            fresh.push(self.news_item(*next_id, "competition", title, body));
            *next_id += 1;
        }
        Ok(())
    }

    /// Os jogadores da Lista Preferencial que mudaram de clube hoje (os negócios do treinador
    /// já têm as notícias deles).
    pub(super) fn shortlist_moves(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let rows: Vec<(i32, Option<i32>, Option<i32>, i64, String)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT t.person_id, t.from_club, t.to_club, t.fee, t.kind FROM transfers t JOIN shortlist s ON s.person_id = t.person_id
                 WHERE t.date = ?1 AND COALESCE(t.to_club, -1) <> ?2 AND COALESCE(t.from_club, -1) <> ?2 ORDER BY t.id",
            )?
            .query_map(params![self.date, self.club_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (person, from, to, fee, kind) in rows {
            let name = self.person_name(person)?;
            let from = from.map(|c| self.club(c).short_name);
            let to = to.map(|c| self.club(c).short_name);
            let (title, what) = match (kind.as_str(), &from, &to) {
                ("loan", Some(f), Some(t)) => (format!("{name} emprestado ao {t}"), format!("{name} foi emprestado pelo {f} ao {t}.")),
                ("return", _, Some(t)) => (format!("{name} volta ao {t}"), format!("{name} acabou o empréstimo e voltou ao {t}.")),
                ("release", Some(f), _) => (format!("{name} dispensado"), format!("{name} foi dispensado pelo {f} e está sem clube.")),
                ("free", _, Some(t)) => (format!("{name} assina com o {t}"), format!("{name} assinou com o {t} sem custos.")),
                (_, Some(f), Some(t)) => {
                    (format!("{name} vai para o {t}"), format!("{name} trocou o {f} pelo {t} por {}.", money_text(fee)))
                }
                _ => continue,
            };
            let body = format!("{what}\n\n{name} está na sua Lista Preferencial.");
            fresh.push(self.news_item(*next_id, "transfer", title, body));
            *next_id += 1;
        }
        Ok(())
    }
}
