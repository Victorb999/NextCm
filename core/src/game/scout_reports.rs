//! Notícias dos olheiros (3.19.1, itens 34 e 35): o resumo semanal do que as tarefas encontraram
//! (à segunda-feira) e o relatório do próximo adversário, 3 dias antes de cada jogo oficial.

use rusqlite::params;

use super::season::join_names;
use super::{Game, NewsItem};
use crate::Result;
use crate::date;
use crate::regulation::{CLUB_FRIENDLY, FRIENDLY};

/// Estrelas a partir das quais um jogador entra no resumo semanal.
const DIGEST_STARS: i32 = 4;
/// Jogadores no resumo, no máximo.
const DIGEST_MAX: usize = 15;
/// Dias antes do jogo em que chega o relatório do adversário.
const REPORT_AHEAD: i32 = 3;
/// Diferença na média dos 11 melhores CA para dizer que uma equipa é mais forte.
const STRONGER_BY: i32 = 6;

impl Game {
    /// Segunda-feira: os jogadores que as tarefas dos olheiros recomendaram nos últimos 7 dias
    /// (4 estrelas ou mais; sem o espiar o adversário). Semana sem recomendações, sem notícia.
    pub(super) fn scout_digest(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if date::weekday(&self.date) != Some(1) {
            return Ok(());
        }
        let since = date::add_days(&self.date, -7).unwrap_or_default();
        let seen: i64 = self.world.conn.query_row(
            "SELECT COUNT(DISTINCT f.person_id) FROM scout_finds f JOIN scout_tasks t ON t.slot = f.slot
             WHERE t.kind <> 'opponent' AND f.found >= ?1 AND f.found < ?2",
            params![since, self.date],
            |r| r.get(0),
        )?;
        let rows: Vec<(i32, String, i32, Option<String>, Option<i32>)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, p.display_name, MAX(f.stars), p.positions, p.club_id
                 FROM scout_finds f JOIN scout_tasks t ON t.slot = f.slot
                 JOIN people p ON p.id = f.person_id JOIN players pl ON pl.person_id = p.id
                 WHERE t.kind <> 'opponent' AND f.found >= ?1 AND f.found < ?2 AND f.stars >= ?3
                 GROUP BY p.id ORDER BY MAX(f.stars) DESC, pl.current_ability DESC, p.id",
            )?
            .query_map(params![since, self.date, DIGEST_STARS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        if rows.is_empty() {
            return Ok(());
        }
        let lines: Vec<String> = rows
            .iter()
            .take(DIGEST_MAX)
            .map(|(_, name, stars, pos, club)| {
                let club = club.map_or_else(|| "sem clube".to_string(), |c| self.club(c).short_name);
                let pos = pos.as_deref().filter(|p| !p.is_empty()).map_or(String::new(), |p| format!("{p}, "));
                format!("{name} ({pos}{club}): {stars} estrelas")
            })
            .collect();
        let more = rows.len().saturating_sub(DIGEST_MAX);
        let mut body = format!("Os olheiros observaram {seen} jogadores esta semana. Recomendam:\n\n{}", lines.join("\n"));
        if more > 0 {
            body += &format!("\n\nE mais {more} nas listas dos olheiros (Pesquisa Jogador & Staff).");
        }
        fresh.push(self.news_item(*next_id, "message", "Relatório semanal dos olheiros".into(), body));
        *next_id += 1;
        Ok(())
    }

    /// O relatório do próximo adversário, 3 dias antes de cada jogo oficial do clube do
    /// treinador: a força comparada, a formação, os últimos resultados e os jogadores-chave.
    /// Assina o olheiro que espia o adversário ou, sem ele, o adjunto.
    pub(super) fn opponent_report(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let Some(day) = date::add_days(&self.date, REPORT_AHEAD) else { return Ok(()) };
        let Some(m) = self
            .matches
            .iter()
            .filter(|m| m.date == day && m.result.is_none() && m.competition_id != FRIENDLY && m.competition_id != CLUB_FRIENDLY)
            .find(|m| m.home_id == self.club_id || m.away_id == self.club_id)
        else {
            return Ok(());
        };
        let home = m.home_id == self.club_id;
        let opp = if home { m.away_id } else { m.home_id };
        let name = self.club(opp).short_name;
        let level = |club: i32| -> Result<i32> {
            Ok(self
                .world
                .conn
                .prepare_cached(
                    "SELECT COALESCE(AVG(ca), 0) FROM (SELECT pl.current_ability AS ca FROM people p JOIN players pl ON pl.person_id = p.id
                     WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC LIMIT 11)",
                )?
                .query_row([club], |r| r.get::<_, f64>(0))? as i32)
        };
        let diff = level(opp)? - level(self.club_id)?;
        let strength = match diff {
            d if d >= STRONGER_BY * 2 => "bastante mais forte do que nós",
            d if d >= STRONGER_BY => "um pouco mais forte do que nós",
            d if d <= -STRONGER_BY * 2 => "bastante mais fraca do que nós",
            d if d <= -STRONGER_BY => "um pouco mais fraca do que nós",
            _ => "do nosso nível",
        };
        let mut body = format!(
            "{name} ({}, {}, {}).\n\nNo papel, é uma equipa {strength}.",
            if home { "em casa" } else { "fora" },
            self.competition_name(m.competition_id),
            super::cm_date(&m.date)
        );
        // O lugar no campeonato, se for de pontos
        if let Some(r) = self.table(m.competition_id).into_iter().find(|r| r.club.id == opp && r.played > 0) {
            body += &format!(" Está em {}.º lugar, com {} pontos em {} jogos.", r.position, r.points, r.played);
        }
        // Os últimos 5 jogos dele
        let mut played: Vec<&crate::league::Match> =
            self.matches.iter().filter(|x| x.result.is_some() && (x.home_id == opp || x.away_id == opp)).collect();
        played.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
        let form: Vec<String> = played
            .iter()
            .take(5)
            .rev()
            .map(|x| {
                let (h, a) = x.result.unwrap_or_default();
                let (gf, ga) = if x.home_id == opp { (h, a) } else { (a, h) };
                let other = self.club(if x.home_id == opp { x.away_id } else { x.home_id }).short_name;
                let mark = match gf.cmp(&ga) {
                    std::cmp::Ordering::Greater => "V",
                    std::cmp::Ordering::Equal => "E",
                    std::cmp::Ordering::Less => "D",
                };
                format!("{mark} {gf}-{ga} {other}")
            })
            .collect();
        if !form.is_empty() {
            body += &format!("\n\nÚltimos jogos: {}.", form.join(", "));
        }
        // A equipa que o treinador deles deve escolher
        let state = self.club_state(opp, Some(m.competition_id));
        let mut t = self.world.auto_tactics(opp, Some(&state), Default::default())?;
        self.mark_key_players(&mut t)?;
        body += &format!("\n\nJoga habitualmente em {}.", t.formation);
        let mut keys: Vec<&crate::tactics::TacticPlayer> = t.starters.iter().filter_map(|s| s.player.as_ref()).collect();
        keys.sort_by_key(|p| (!p.key, std::cmp::Reverse(p.current_ability.unwrap_or(0)), p.id));
        let keys: Vec<String> = keys
            .iter()
            .take(3)
            .map(|p| {
                let name = self.person_name(p.id).unwrap_or_else(|_| p.list_name.clone());
                match p.goals {
                    0 => format!("{name} ({})", p.position),
                    1 => format!("{name} ({}, 1 golo)", p.position),
                    g => format!("{name} ({}, {g} golos)", p.position),
                }
            })
            .collect();
        if !keys.is_empty() {
            body += &format!(" Os jogadores a vigiar: {}.", join_names(&keys));
        }
        let out: Vec<String> = t
            .reserves
            .iter()
            .filter(|p| p.injured || p.suspended)
            .filter(|p| p.key)
            .map(|p| self.person_name(p.id).unwrap_or_else(|_| p.list_name.clone()))
            .collect();
        if !out.is_empty() {
            body += &format!("\n\nDesfalques importantes: {}.", join_names(&out));
        }
        // Quem assina: o olheiro que espia o adversário, senão o adjunto
        let scout: Option<String> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.display_name FROM scout_tasks t JOIN people p ON p.id = t.scout_id
                 WHERE t.kind = 'opponent' AND p.club_id = ? ORDER BY t.slot LIMIT 1",
            )?
            .query_row([self.club_id], |r| r.get(0))
            .ok();
        if let Some(s) = scout {
            body += &format!("\n\n{s}, olheiro");
        }
        fresh.push(self.news_item(*next_id, "message", format!("Relatório: {name}"), body));
        *next_id += 1;
        Ok(())
    }
}
