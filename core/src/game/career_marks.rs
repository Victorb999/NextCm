//! Marcos & Recordes do perfil (item 25 do 3.19.1): os marcos da carreira com data (estreias,
//! primeiros golos, hat-tricks, o 100.º jogo e o 50.º golo...) e os recordes do jogador. As
//! épocas antes do jogo vêm do `career_history` do banco (só jogos e golos por clube); o resto
//! sai dos jogos gravados (`player_matches`).

use std::collections::{BTreeMap, HashMap};

use super::Game;
use crate::person::{CareerMark, PersonProfile, RecordLine};
use crate::{Result, regulation};

/// Os jogos e os golos da carreira que fazem marco.
const APPS_STEP: i64 = 100;
const GOALS_STEP: i64 = 50;
/// Golos num jogo para um hat-trick.
const HAT_TRICK: i32 = 3;

/// Um jogo gravado do jogador.
struct Played {
    date: String,
    club: i32,
    competition: i32,
    goals: i32,
    rating: f64,
    mom: bool,
    opponent: Option<i32>,
}

impl Game {
    pub(super) fn fill_marks(&self, p: &mut PersonProfile) -> Result<()> {
        if p.player.is_none() {
            return Ok(());
        }
        let games: Vec<Played> = self
            .world
            .conn
            .prepare_cached(
                "SELECT m.date, m.club, m.competition, m.goals, m.rating, m.mom,
                        CASE WHEN f.home_id = m.club THEN f.away_id WHEN f.away_id = m.club THEN f.home_id END
                 FROM player_matches m LEFT JOIN fixtures f ON f.id = m.fixture
                 WHERE m.person_id = ? ORDER BY m.date, m.fixture",
            )?
            .query_map([p.id], |r| {
                Ok(Played {
                    date: r.get(0)?,
                    club: r.get(1)?,
                    competition: r.get(2)?,
                    goals: r.get(3)?,
                    rating: r.get(4)?,
                    mom: r.get(5)?,
                    opponent: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        // Antes do jogo: por clube, do banco; pela selecção, o que as internacionalizações dizem
        let before: HashMap<i32, (i64, i64)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT club_id, SUM(apps), SUM(goals) FROM career_history WHERE person_id = ? AND club_id IS NOT NULL GROUP BY club_id",
            )?
            .query_map([p.id], |r| Ok((r.get(0)?, (r.get::<_, Option<i64>>(1)?.unwrap_or(0), r.get::<_, Option<i64>>(2)?.unwrap_or(0)))))?
            .collect::<rusqlite::Result<_>>()?;
        let (caps, intl_goals): (i64, i64) = self.world.conn.query_row(
            "SELECT COALESCE(international_apps, 0), COALESCE(international_goals, 0) FROM people WHERE id = ?",
            [p.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let national = |club: i32| self.is_national_team(club);
        let intl_played = games.iter().filter(|g| national(g.club)).count() as i64;
        let intl_scored: i64 = games.iter().filter(|g| national(g.club)).map(|g| g.goals as i64).sum();
        let mut seen: HashMap<i32, (i64, i64)> = before.clone();
        if let Some(n) = games.iter().find(|g| national(g.club)).map(|g| g.club) {
            seen.insert(n, ((caps - intl_played).max(0), (intl_goals - intl_scored).max(0)));
        }
        let (mut apps, mut goals): (i64, i64) = before.values().fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
        let mut marks: Vec<CareerMark> = Vec::new();
        let mut mark = |date: &str, text: String| marks.push(CareerMark { date: date.to_string(), text });
        for g in &games {
            let name = self.club(g.club).short_name;
            let of = if national(g.club) { "pela" } else { "pelo" };
            let (club_apps, club_goals) = seen.entry(g.club).or_default();
            if *club_apps == 0 {
                mark(&g.date, format!("Estreia {of} {name}"));
            }
            if *club_goals == 0 && g.goals > 0 {
                mark(&g.date, format!("Primeiro golo {of} {name}"));
            }
            *club_apps += 1;
            *club_goals += g.goals as i64;
            let against = g.opponent.map(|o| format!(" contra o {}", self.club(o).short_name)).unwrap_or_default();
            if g.goals >= HAT_TRICK {
                let what = if g.goals == HAT_TRICK { "Hat-trick".to_string() } else { format!("{} golos", g.goals) };
                mark(&g.date, format!("{what}{against}"));
            }
            if national(g.club) {
                continue;
            }
            // Os da carreira contam só os dos clubes, como o Historial do banco
            apps += 1;
            if apps % APPS_STEP == 0 {
                mark(&g.date, format!("{apps}.º jogo da carreira{against}"));
            }
            let old = goals;
            goals += g.goals as i64;
            if goals / GOALS_STEP > old / GOALS_STEP {
                mark(&g.date, format!("{}.º golo da carreira{against}", goals / GOALS_STEP * GOALS_STEP));
            }
        }
        marks.reverse();
        p.marks = marks;
        p.records = self.records_of(&games, apps, goals, caps, intl_goals);
        Ok(())
    }

    fn records_of(&self, games: &[Played], apps: i64, goals: i64, caps: i64, intl_goals: i64) -> Vec<RecordLine> {
        let line = |label: &str, value: String, detail: Option<String>| RecordLine { label: label.into(), value, detail };
        let against = |g: &Played| {
            let vs = g.opponent.map(|o| format!("contra o {}, ", self.club(o).short_name)).unwrap_or_default();
            format!("{vs}{}", super::cm_date(&g.date))
        };
        let mut out = vec![line("Jogos na carreira", apps.to_string(), None), line("Golos na carreira", goals.to_string(), None)];
        if caps > 0 {
            out.push(line("Internacionalizações", caps.to_string(), Some(format!("{intl_goals} golos"))));
        }
        if let Some(g) = games.iter().filter(|g| g.goals > 0).max_by(|a, b| a.goals.cmp(&b.goals).then(b.date.cmp(&a.date))) {
            out.push(line("Mais golos num jogo", g.goals.to_string(), Some(against(g))));
        }
        if let Some(g) = games.iter().max_by(|a, b| a.rating.total_cmp(&b.rating).then(b.date.cmp(&a.date))) {
            out.push(line("Melhor nota", format!("{:.1}", g.rating), Some(against(g))));
        }
        let mom = games.iter().filter(|g| g.mom).count();
        if mom > 0 {
            out.push(line("Homem do Jogo", mom.to_string(), None));
        }
        // A melhor época em golos, pelos jogos gravados (época da competição e clube)
        let mut seasons: BTreeMap<(i32, i32), (i32, String)> = BTreeMap::new();
        for g in games.iter().filter(|g| !self.is_national_team(g.club)) {
            let year = regulation::season_of(g.competition, &g.date);
            let e = seasons.entry((year, g.club)).or_insert_with(|| (0, regulation::season_label(g.competition, year)));
            e.0 += g.goals;
        }
        if let Some(((_, club), (n, label))) = seasons.iter().filter(|(_, v)| v.0 > 0).max_by(|a, b| a.1.0.cmp(&b.1.0).then(b.0.cmp(a.0))) {
            out.push(line("Melhor época (golos)", n.to_string(), Some(format!("{label}, {}", self.club(*club).short_name))));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;

    #[test]
    fn marks_and_records_from_the_games() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-marcos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Marcos".into(),
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
        let g = Game::create(&path, &dir, &new, 31).unwrap();
        // Um jovem sem jogos no banco: estreia, primeiro golo e um hat-trick
        let id: i32 = g
            .world
            .conn
            .query_row(
                "SELECT p.id FROM people p WHERE p.club_id = 7568 AND p.is_player = 1
                 AND p.id NOT IN (SELECT person_id FROM career_history WHERE club_id = 7568) LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        for (date, goals, rating) in [("2026-02-01", 0, 6.5), ("2026-02-08", 3, 9.1)] {
            g.world
                .conn
                .execute(
                    "INSERT INTO player_matches (person_id, fixture, date, competition, club, rating, goals, assists) VALUES (?, -1, ?, 16, 7568, ?, ?, 0)",
                    params![id, date, rating, goals],
                )
                .unwrap();
        }
        let mut p = g.world.get_person(id).unwrap();
        g.fill_marks(&mut p).unwrap();
        let texts: Vec<&str> = p.marks.iter().map(|m| m.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.starts_with("Estreia pelo")), "{texts:?}");
        assert!(texts.iter().any(|t| t.starts_with("Primeiro golo pelo")), "{texts:?}");
        assert!(texts.contains(&"Hat-trick"), "{texts:?}");
        assert_eq!(p.marks[0].date, "2026-02-08", "do mais recente");
        let record = |label: &str| p.records.iter().find(|r| r.label == label).map(|r| r.value.clone());
        assert_eq!(record("Mais golos num jogo").as_deref(), Some("3"));
        assert_eq!(record("Melhor nota").as_deref(), Some("9.1"));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
