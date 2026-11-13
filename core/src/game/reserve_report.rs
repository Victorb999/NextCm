//! O relatório mensal do adjunto sobre as reservas (marco 4.1), nas Notícias no dia 1 de cada
//! mês, nos dois sentidos:
//! - **Quem merece subir**: até 3 das reservas com boas notas nos jogos delas no último mês
//!   (pelo menos 2 jogos e 6,9 de média, ou 0,4 acima da média das reservas),
//!   pela evolução no treino (o CA de agora contra o do relatório anterior, guardado em
//!   `reserve_report_ca`) e pelo nível perto do titular mais fraco da posição;
//! - **Quem devia descer**: até 3 do plantel principal sem minutos no último mês ou em má forma.
//!
//! Os nomes são os do jogo, por isso a tela de Notícias liga-os ao perfil. Sem nada a dizer (ou
//! com uma equipa B, que não tem reservas), não há relatório.

use std::collections::HashMap;

use rusqlite::params;

use super::reserves::ReserveMode;
use super::{Game, NewsItem};
use crate::{Result, date};

/// O CA de cada jogador do treinador no último relatório (save 61).
pub(super) const REPORT_SCHEMA: &str = "CREATE TABLE reserve_report_ca (person_id INTEGER PRIMARY KEY, ca INTEGER NOT NULL);";

/// Quantos nomes em cada lista.
const NAMES: usize = 3;
/// O período do relatório (dias).
const PERIOD: i32 = 31;
/// Para subir: pelo menos tantos jogos nas reservas, com esta nota média.
const MIN_GAMES: i64 = 2;
/// As notas das reservas andam pelos 6,6: a partir disto são boas; de [`GREAT_RATING`], grandes.
const GOOD_RATING: f64 = 6.9;
const GREAT_RATING: f64 = 7.3;
/// Ou destaca-se: tanto acima da média das reservas no mês (numa equipa que perde muito, as
/// notas de todos descem).
const STANDOUT: f64 = 0.4;
/// "Perto do titular": o CA a menos disto do titular mais fraco da posição.
const CLOSE: i32 = 10;
/// Para descer: menos minutos do que isto no mês (com a equipa a ter jogado pelo menos
/// [`MIN_TEAM_GAMES`]), ou esta nota média nos últimos jogos (com pelo menos [`FORM_APPS`]).
const FEW_MINUTES: i32 = 90;
const MIN_TEAM_GAMES: usize = 3;
const POOR_FORM: f64 = 6.3;
const FORM_APPS: usize = 3;

/// "7,6"
fn decimal(v: f64) -> String {
    format!("{v:.1}").replace('.', ",")
}

impl Game {
    /// Dia 1 de cada mês: o relatório do adjunto (depois da evolução do mês).
    pub(super) fn reserve_report(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if !self.date.ends_with("-01") || self.sacked() || self.is_national_team(self.club_id) {
            return Ok(());
        }
        let ca_now: HashMap<i32, i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, COALESCE(pl.current_ability, 1) FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1",
            )?
            .query_map([self.club_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let ca_before: HashMap<i32, i32> = self
            .world
            .conn
            .prepare_cached("SELECT person_id, ca FROM reserve_report_ca")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let body = if matches!(self.reserve_mode()?, ReserveMode::BTeam(_)) { None } else { self.report_body(&ca_now, &ca_before)? };
        // O CA de agora fica para o próximo relatório
        self.world.conn.execute("DELETE FROM reserve_report_ca", [])?;
        for (id, ca) in &ca_now {
            self.world.conn.execute("INSERT INTO reserve_report_ca (person_id, ca) VALUES (?, ?)", params![id, ca])?;
        }
        if let Some(body) = body {
            fresh.push(self.news_item(*next_id, "message", "Relatório das reservas".into(), body));
            *next_id += 1;
        }
        Ok(())
    }

    /// O texto do relatório; `None` sem ninguém para subir nem para descer.
    fn report_body(&self, ca_now: &HashMap<i32, i32>, ca_before: &HashMap<i32, i32>) -> Result<Option<String>> {
        let from = date::add_days(&self.date, -PERIOD).unwrap_or_default();
        let up = self.deserve_promotion(&from, ca_now, ca_before)?;
        let down = self.need_reserves(&from)?;
        if up.is_empty() && down.is_empty() {
            return Ok(None);
        }
        let mut body = "O seu adjunto fez o relatório do mês sobre as reservas.".to_string();
        if !up.is_empty() {
            body += "\n\nQuem merece subir:";
            for line in up {
                body += &format!("\n- {line}");
            }
        }
        if !down.is_empty() {
            body += "\n\nQuem devia descer:";
            for line in down {
                body += &format!("\n- {line}");
            }
        }
        Ok(Some(body))
    }

    /// Os das reservas que merecem uma oportunidade (os melhores primeiro).
    fn deserve_promotion(&self, from: &str, ca_now: &HashMap<i32, i32>, ca_before: &HashMap<i32, i32>) -> Result<Vec<String>> {
        let rows: Vec<(i32, i64, f64)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT person_id, COUNT(*), AVG(rating) FROM reserve_apps WHERE club = ? AND date >= ?
                 GROUP BY person_id ORDER BY person_id",
            )?
            .query_map(params![self.club_id, from], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let weakest = self.weakest_starters()?;
        let (total, count) = rows.iter().fold((0.0, 0i64), |(t, c), r| (t + r.2 * r.1 as f64, c + r.1));
        let mean = if count > 0 { total / count as f64 } else { 0.0 };
        let mut out: Vec<(f64, String)> = Vec::new();
        for (id, games, rating) in rows {
            let good = rating >= GOOD_RATING || (count > 0 && rating >= mean + STANDOUT);
            if games < MIN_GAMES || !good || !self.squads.state(id).reserve || self.is_ghost(id) {
                continue;
            }
            let Some(&ca) = ca_now.get(&id) else { continue };
            let grew = ca_before.get(&id).is_some_and(|&b| ca > b);
            let close = weakest.get(&self.sector_of(id)?).is_some_and(|&w| ca >= w - CLOSE);
            let name = self.person_name(id)?;
            let how = if rating >= GREAT_RATING { "grandes" } else { "boas" };
            let mut line = format!("{name} tem feito {how} exibições pelas reservas, nota média {} em {games} jogos", decimal(rating));
            line += match (grew, close) {
                (true, true) => ", tem evoluído no treino e já está perto do nível dos titulares",
                (true, false) => ", tem evoluído no treino",
                (false, true) => ", já está perto do nível dos titulares",
                (false, false) => "",
            };
            line += ", e merece uma oportunidade.";
            let score = rating + if grew { 0.3 } else { 0.0 } + if close { 0.5 } else { 0.0 };
            out.push((score, line));
        }
        out.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        Ok(out.into_iter().take(NAMES).map(|o| o.1).collect())
    }

    /// Os do plantel principal a quem jogar pelas reservas dava ritmo (os de menos minutos
    /// primeiro, depois os de pior forma).
    fn need_reserves(&self, from: &str) -> Result<Vec<String>> {
        let team_games = self
            .matches
            .iter()
            .filter(|m| {
                m.result.is_some()
                    && m.date.as_str() >= from
                    && m.competition_id != super::friendlies::CLUB_FRIENDLY
                    && (m.home_id == self.club_id || m.away_id == self.club_id)
            })
            .count();
        let squad: Vec<i32> = self
            .world
            .conn
            .prepare_cached("SELECT id FROM people WHERE club_id = ? AND is_player = 1 ORDER BY id")?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let minutes: HashMap<i32, i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT person_id, SUM(MAX(COALESCE(minute_off, 90) - COALESCE(minute_on, 0), 0)) FROM player_matches
                 WHERE club = ? AND date >= ? GROUP BY person_id",
            )?
            .query_map(params![self.club_id, from], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut few: Vec<(i32, String)> = Vec::new();
        let mut poor: Vec<(f64, String)> = Vec::new();
        for id in squad {
            let st = self.squads.state(id);
            if st.reserve || st.injury.is_some() || self.is_ghost(id) || self.is_keeper(id)? {
                continue;
            }
            let name = self.person_name(id)?;
            let played = minutes.get(&id).copied().unwrap_or(0);
            if team_games >= MIN_TEAM_GAMES && played < FEW_MINUTES {
                let line = if played == 0 {
                    format!("{name} não jogou no último mês: uns jogos pelas reservas davam-lhe ritmo.")
                } else {
                    format!("{name} só jogou {played} minutos no último mês: uns jogos pelas reservas davam-lhe ritmo.")
                };
                few.push((played, line));
                continue;
            }
            let form = self.form_of(id)?;
            if form.len() >= FORM_APPS {
                let avg = form.iter().sum::<f64>() / form.len() as f64;
                if avg < POOR_FORM {
                    poor.push((
                        avg,
                        format!(
                            "{name} está em má forma (nota média {} nos últimos {} jogos): jogar pelas reservas ajudava-o a recuperar a confiança.",
                            decimal(avg),
                            form.len()
                        ),
                    ));
                }
            }
        }
        few.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        poor.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        Ok(few.into_iter().map(|f| f.1).chain(poor.into_iter().map(|p| p.1)).take(NAMES).collect())
    }

    /// O CA do titular mais fraco de cada sector (GR, defesa, médio, avançado) no onze de agora.
    fn weakest_starters(&self) -> Result<HashMap<usize, i32>> {
        let t = self.club_tactics(self.club_id)?;
        let mut out: HashMap<usize, i32> = HashMap::new();
        for s in &t.starters {
            let Some(p) = &s.player else { continue };
            let ca = p.current_ability.unwrap_or(1);
            let e = out.entry(crate::engine::sector(&s.role)).or_insert(ca);
            *e = (*e).min(ca);
        }
        Ok(out)
    }

    /// O sector em que um jogador joga melhor (0 GR, 1 defesa, 2 médio, 3 avançado).
    fn sector_of(&self, id: i32) -> Result<usize> {
        let pos: [i32; 4] = self.world.conn.query_row(
            "SELECT COALESCE(pos_goalkeeper, 1),
                    MAX(COALESCE(pos_sweeper, 1), COALESCE(pos_defender, 1), COALESCE(pos_wing_back, 1)),
                    MAX(COALESCE(pos_defensive_midfielder, 1), COALESCE(pos_midfielder, 1), COALESCE(pos_attacking_midfielder, 1)),
                    COALESCE(pos_attacker, 1)
             FROM players WHERE person_id = ?",
            [id],
            |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?]),
        )?;
        Ok(super::market::role_of(pos[0], pos[1], pos[2], pos[3]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn assistant_picks_the_reserves_and_reports() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-relatorio-reservas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Relatório das reservas".into(),
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
        let mut g = Game::create(&path, &dir, &new, 12).unwrap();
        let comp = g.division_of(PALMEIRAS).unwrap().unwrap();
        let conn = |g: &Game, sql: &str, p: &[&dyn rusqlite::ToSql]| {
            g.world.conn.execute(sql, p).unwrap();
        };
        let squad: Vec<i32> = g
            .world
            .conn
            .prepare("SELECT id FROM people WHERE club_id = ? AND is_player = 1 ORDER BY id")
            .unwrap()
            .query_map([PALMEIRAS], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let t = g.club_tactics(PALMEIRAS).unwrap();
        let xi: Vec<i32> = t.starters.iter().filter_map(|s| s.player.as_ref().map(|p| p.id)).collect();
        let reserves: Vec<i32> = squad.iter().copied().filter(|id| g.squads.state(*id).reserve && !g.is_ghost(*id)).collect();
        let outside: Vec<i32> =
            squad.iter().copied().filter(|id| !g.squads.state(*id).reserve && !xi.contains(id) && !g.is_keeper(*id).unwrap()).collect();
        assert!(reserves.len() >= 2 && outside.len() >= 2);
        let day = "2025-07-20";

        // De volta de lesão: desce para ganhar ritmo; um titular não desce
        conn(
            &g,
            "INSERT INTO injury_log (person_id, date, injury, kind, days) VALUES (?, '2025-07-05', 'Entorse', 'Jogo', 10)",
            &[&outside[0]],
        );
        conn(&g, "INSERT INTO injury_log (person_id, date, injury, kind, days) VALUES (?, '2025-07-05', 'Entorse', 'Jogo', 10)", &[&xi[3]]);
        // Jogou na véspera: não joga
        conn(
            &g,
            "INSERT INTO player_matches (person_id, fixture, date, competition, club, rating, goals, assists) VALUES (?, 1, '2025-07-19', 1, ?, 6.5, 0, 0)",
            &[&reserves[1], &PALMEIRAS],
        );
        let plan = g.reserve_plan(comp, day).unwrap();
        assert!(plan.first.contains(&outside[0]), "de volta de lesão");
        assert!(!plan.first.contains(&xi[3]), "o titular do próximo jogo não desce");
        assert!(!plan.first.contains(&reserves[1]) && !plan.second.contains(&reserves[1]), "jogou na véspera");
        assert!(plan.first.contains(&reserves[0]));
        assert!(squad.iter().filter(|id| g.is_ghost(**id)).all(|id| !plan.first.contains(id)), "os de enchimento só completam");
        // Sem 3 jornadas jogadas, ninguém desce só por falta de minutos
        assert!(!plan.first.contains(&outside[1]));

        // O onze e o banco das reservas: quem desce aparece marcado; o banco cobre os sectores
        let lineup = g.reserve_lineup().unwrap();
        assert_eq!(lineup.starters.len(), 11);
        // Na baliza, um guarda-redes (mesmo que seja de enchimento)
        let keeper = lineup.starters.iter().find(|s| s.role == "GR").unwrap().person;
        assert!(g.is_keeper(keeper).unwrap(), "um guarda-redes na baliza");
        assert!(!lineup.bench.is_empty() && lineup.bench.len() <= RESERVE_BENCH_TEST);
        assert!(lineup.starters.iter().chain(&lineup.bench).all(|s| !s.first_team || !xi.contains(&s.person)));

        // O relatório: uma reserva com boas notas sobe; um do principal em má forma desce
        for (i, d) in ["2025-06-10", "2025-06-17", "2025-06-24"].iter().enumerate() {
            conn(
                &g,
                "INSERT INTO reserve_apps (person_id, fixture, date, club, rating, goals, assists) VALUES (?, ?, ?, ?, 7.8, 1, 0)",
                &[&reserves[0], &(900 + i as i32), d, &PALMEIRAS],
            );
            conn(
                &g,
                "INSERT INTO player_matches (person_id, fixture, date, competition, club, rating, goals, assists) VALUES (?, ?, ?, 1, ?, 5.6, 0, 0)",
                &[&outside[1], &(950 + i as i32), d, &PALMEIRAS],
            );
        }
        assert!(g.date.ends_with("-01"));
        let (mut fresh, mut next_id) = (Vec::new(), 1000);
        g.reserve_report(&mut fresh, &mut next_id).unwrap();
        let n = fresh.iter().find(|n| n.title == "Relatório das reservas").expect("o relatório");
        let (up, down) = (g.person_name(reserves[0]).unwrap(), g.person_name(outside[1]).unwrap());
        assert!(n.body.contains("Quem merece subir") && n.body.contains(&format!("{up} tem feito grandes exibições")), "{}", n.body);
        assert!(n.body.contains("nota média 7,8 em 3 jogos"), "{}", n.body);
        assert!(n.body.contains("Quem devia descer") && n.body.contains(&format!("{down} está em má forma")), "{}", n.body);
        let saved: i64 = g.world.conn.query_row("SELECT COUNT(*) FROM reserve_report_ca", [], |r| r.get(0)).unwrap();
        assert_eq!(saved as usize, squad.len());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    const RESERVE_BENCH_TEST: usize = 5;
}
