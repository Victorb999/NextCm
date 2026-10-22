//! Pontos corridos: calendário de turno (e returno) pelo método do círculo e a
//! classificação calculada dos resultados. As datas vêm dos regulamentos (`regulation.rs`).

use rand_core::RngCore;
use serde::Serialize;
use ts_rs::TS;

use crate::date;
use crate::person::ClubRef;
use crate::regulation;
use crate::{Error, Result};

pub const SERIE_A: i32 = 65;
pub const SERIE_B: i32 = 79;
pub const SERIE_C: i32 = 80;
pub const SERIE_D: i32 = 175;
pub const COPA_DO_BRASIL: i32 = 68;
pub const LIBERTADORES: i32 = 58;
pub const SUL_AMERICANA: i32 = 92;

/// Uma partida do calendário, como fica na memória e na tabela `fixtures` do save.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Match {
    pub id: i32,
    pub competition_id: i32,
    /// Fase do regulamento (0 = a primeira).
    pub stage: u8,
    /// Pontos corridos e grupos: a jornada, a partir de 1. Eliminatórias: a ronda, a partir de 1.
    pub round: i32,
    /// Grupo (0 = A) na fase de grupos.
    pub group: Option<u8>,
    /// Eliminatória dentro da ronda (0 = a primeira do quadro).
    pub tie: Option<u16>,
    /// 0 = jogo de liga ou jogo único; 1 e 2 = ida e volta.
    pub leg: u8,
    /// Em campo neutro (sem vantagem de casa).
    pub neutral: bool,
    pub date: String,
    pub home_id: i32,
    pub away_id: i32,
    pub result: Option<(u8, u8)>,
    /// Penáltis no fim de uma eliminatória empatada.
    pub pens: Option<(u8, u8)>,
}

/// Uma partida para a interface (com os clubes por extenso).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Fixture {
    pub id: i32,
    pub competition_id: i32,
    pub competition: String,
    pub round: i32,
    pub date: String,
    pub home: ClubRef,
    pub away: ClubRef,
    pub home_goals: Option<i32>,
    pub away_goals: Option<i32>,
    /// "12ª jornada", "Grupo B · 3ª jornada", "Oitavas de final · 1ª mão", "Final".
    pub label: String,
    pub neutral: bool,
    pub home_pens: Option<i32>,
    pub away_pens: Option<i32>,
}

/// Uma linha da classificação: J V E D M S P.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct StandingRow {
    pub position: i32,
    pub club: ClubRef,
    pub played: i32,
    pub won: i32,
    pub drawn: i32,
    pub lost: i32,
    pub goals_for: i32,
    pub goals_against: i32,
    pub points: i32,
    /// Últimos 5 resultados, do mais antigo para o mais recente ("V", "E", "D").
    pub form: String,
}

/// Calendário pelo método do círculo, em turno único (`legs` = 1) ou turno e returno (2).
/// A ordem dos clubes é sorteada, então cada jogo novo tem um calendário diferente. Uma
/// jornada num sábado tem metade das partidas ao domingo; a meio da semana são à quarta.
/// Os ids começam em `first_id`; `dates` são as datas das jornadas (ver `regulation::dates`).
pub fn round_robin(
    competition_id: i32,
    clubs: &[i32],
    dates: &[String],
    legs: u8,
    first_id: i32,
    rng: &mut impl RngCore,
) -> Result<Vec<Match>> {
    let mut teams: Vec<Option<i32>> = clubs.iter().copied().map(Some).collect();
    // Fisher-Yates com o RNG do jogo
    for i in (1..teams.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        teams.swap(i, j);
    }
    if teams.len() % 2 == 1 {
        teams.push(None); // folga
    }
    let n = teams.len();
    let half_rounds = n.saturating_sub(1);
    let rounds = half_rounds * legs.max(1) as usize;
    if dates.len() < rounds {
        return Err(Error::Invalid(format!("regulamento da competição {competition_id}: {rounds} jornadas e só {} datas", dates.len())));
    }

    let mut first_half: Vec<Vec<(i32, i32)>> = Vec::with_capacity(half_rounds);
    for r in 0..half_rounds {
        let mut pairs = Vec::new();
        for i in 0..n / 2 {
            let (a, b) = (teams[i], teams[n - 1 - i]);
            let (Some(a), Some(b)) = (a, b) else { continue };
            // Alterna o mando: o clube fixo (posição 0) troca a cada jornada, os outros pela paridade
            let home_first = if i == 0 { r % 2 == 0 } else { i % 2 == 1 };
            pairs.push(if home_first { (a, b) } else { (b, a) });
        }
        first_half.push(pairs);
        teams[1..].rotate_right(1);
    }

    let mut out = Vec::with_capacity(rounds * n / 2);
    let mut id = first_id;
    for (r, pairs) in first_half.iter().cycle().take(rounds).enumerate() {
        let returno = r >= half_rounds;
        let day = &dates[r];
        let weekend = regulation::is_weekend(day);
        let sunday = date::add_days(day, 1).expect("data válida");
        for (i, &(h, a)) in pairs.iter().enumerate() {
            let (home_id, away_id) = if returno { (a, h) } else { (h, a) };
            out.push(Match {
                id,
                competition_id,
                round: r as i32 + 1,
                date: if weekend && i % 2 == 1 { sunday.clone() } else { day.clone() },
                home_id,
                away_id,
                ..Default::default()
            });
            id += 1;
        }
    }
    Ok(out)
}

/// Classificação dos jogos já disputados. Desempate como no Brasileirão: pontos,
/// vitórias, saldo, golos marcados; depois o nome.
pub fn standings(matches: &[Match], clubs: &[ClubRef]) -> Vec<StandingRow> {
    let mut rows: Vec<StandingRow> = clubs
        .iter()
        .map(|c| StandingRow {
            position: 0,
            club: c.clone(),
            played: 0,
            won: 0,
            drawn: 0,
            lost: 0,
            goals_for: 0,
            goals_against: 0,
            points: 0,
            form: String::new(),
        })
        .collect();
    let idx = |id: i32| clubs.iter().position(|c| c.id == id);
    let mut played: Vec<&Match> = matches.iter().filter(|m| m.result.is_some()).collect();
    played.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
    for m in played {
        let (hg, ag) = m.result.unwrap();
        let (hg, ag) = (hg as i32, ag as i32);
        for (team, gf, ga) in [(m.home_id, hg, ag), (m.away_id, ag, hg)] {
            let Some(i) = idx(team) else { continue };
            let r = &mut rows[i];
            r.played += 1;
            r.goals_for += gf;
            r.goals_against += ga;
            let letter = if gf > ga {
                r.won += 1;
                r.points += 3;
                'V'
            } else if gf == ga {
                r.drawn += 1;
                r.points += 1;
                'E'
            } else {
                r.lost += 1;
                'D'
            };
            r.form.push(letter);
            if r.form.len() > 5 {
                r.form.remove(0);
            }
        }
    }
    rows.sort_by(|a, b| {
        b.points
            .cmp(&a.points)
            .then(b.won.cmp(&a.won))
            .then((b.goals_for - b.goals_against).cmp(&(a.goals_for - a.goals_against)))
            .then(b.goals_for.cmp(&a.goals_for))
            .then(a.club.short_name.cmp(&b.club.short_name))
    });
    for (i, r) in rows.iter_mut().enumerate() {
        r.position = i as i32 + 1;
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;
    use std::collections::{HashMap, HashSet};

    #[test]
    fn double_round_robin_is_complete_and_fair() {
        let clubs: Vec<i32> = (1..=20).collect();
        let dates = regulation::dates(2025, "w31-w49 m32-m50");
        let ms = round_robin(SERIE_A, &clubs, &dates, 2, 1, &mut ChaCha8Rng::seed_from_u64(3)).unwrap();
        assert_eq!(ms.len(), 380);
        // Cada par joga uma vez em casa de cada um
        let pairs: HashSet<(i32, i32)> = ms.iter().map(|m| (m.home_id, m.away_id)).collect();
        assert_eq!(pairs.len(), 380);
        // Cada clube joga uma vez por jornada, 19 em casa e 19 fora
        for r in 1..=38 {
            let teams: HashSet<i32> = ms.iter().filter(|m| m.round == r).flat_map(|m| [m.home_id, m.away_id]).collect();
            assert_eq!(teams.len(), 20, "jornada {r}");
        }
        let mut home: HashMap<i32, i32> = HashMap::new();
        for m in &ms {
            *home.entry(m.home_id).or_default() += 1;
        }
        assert!(home.values().all(|&h| h == 19), "{home:?}");
        // Começa num sábado de agosto; fins de semana (sábado e domingo) e quartas
        assert_eq!(ms[0].date, "2025-08-02");
        assert!(ms.iter().all(|m| matches!(date::weekday(&m.date), Some(0 | 3 | 6))));
        assert_eq!(ms.iter().map(|m| m.date.as_str()).max(), Some("2025-12-10"));
        // Mesma seed, mesmo calendário
        assert_eq!(ms, round_robin(SERIE_A, &clubs, &dates, 2, 1, &mut ChaCha8Rng::seed_from_u64(3)).unwrap());
        // Turno único: 19 jornadas; datas a menos é um erro
        assert_eq!(round_robin(SERIE_A, &clubs, &dates, 1, 1, &mut ChaCha8Rng::seed_from_u64(3)).unwrap().len(), 190);
        assert!(round_robin(SERIE_A, &clubs, &dates[..10], 2, 1, &mut ChaCha8Rng::seed_from_u64(3)).is_err());
    }

    #[test]
    fn standings_order_and_tiebreak() {
        let club = |id: i32, name: &str| ClubRef { id, name: name.into(), short_name: name.into(), fg: None, bg: None };
        let clubs = vec![club(1, "A"), club(2, "B"), club(3, "C")];
        let m = |id, h, a, r: Option<(u8, u8)>| Match {
            id,
            competition_id: SERIE_A,
            round: id,
            date: format!("2025-08-0{id}"),
            home_id: h,
            away_id: a,
            result: r,
            ..Default::default()
        };
        let ms = vec![m(1, 1, 2, Some((2, 0))), m(2, 3, 1, Some((1, 1))), m(3, 2, 3, Some((3, 0))), m(4, 2, 1, None)];
        let t = standings(&ms, &clubs);
        let order: Vec<_> = t.iter().map(|r| r.club.short_name.as_str()).collect();
        // A: 4 pts; B: 3 pts; C: 1 pt
        assert_eq!(order, ["A", "B", "C"]);
        assert_eq!((t[0].played, t[0].won, t[0].drawn, t[0].goals_for, t[0].goals_against, t[0].points), (2, 1, 1, 3, 1, 4));
        assert_eq!(t[0].form, "VE");
        assert_eq!(t[1].form, "DV");
    }
}
