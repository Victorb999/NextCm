//! Jogos das reservas (marco 3.19, item 4.4), como no CM. A equipa de reservas não é um clube
//! nem tem plantel fixo: é o plantel à parte com os jogadores que o treinador despromoveu
//! (`reserve`), que podem voltar à equipa principal a qualquer hora; só o treinador a vê. Os
//! clubes "(R)" do banco (o Primavera da Juventus) são outros clubes e não entram aqui.
//!
//! Regras tiradas de um jogo gravado do CM 01/02 (a Juventus em 2032, competição 87 "Reserve";
//! docs/roadmap/3.19-teste-01-03-11.md): as reservas jogam dois dias antes de cada jogo do
//! campeonato, contra as reservas do mesmo adversário e com o campo trocado (a equipa
//! principal em casa, as reservas fora), sem tabela ("Reservas" no Calendário). Não há jogo se
//! o clube joga nesse dia ou no seguinte, se as reservas jogaram há três dias ou menos, ou se
//! um dos clubes tem uma equipa B à parte (como em Espanha e em Portugal; a II alemã faz de
//! reservas, passo 3.19.2). Não há
//! calendário guardado: os jogos saem dos do campeonato (seguem os adiamentos), e só o
//! resultado fica em `reserve_matches`. A IA não despromove ninguém: as reservas do adversário
//! são quem fica fora dos 18 melhores dele. Há sempre jogo: as funções sem jogador ficam com
//! fantasmas, como no CM (só nesse jogo, com o nível do enchimento do clube, e sem registo).
//!
//! O treinador pode escolher quem joga (item 4.5, `reserve_picks`): o adjunto põe os escolhidos
//! no onze e completa as funções que faltam com as outras reservas.
//!
//! O resultado sai dos golos esperados do motor com um sorteio à parte (os jogos oficiais e as
//! médias do motor não mudam) e conta no perfil como "Não Competitivos", fora dos Jogos
//! Oficiais.

use rand_core::RngCore;
use rusqlite::params;
use serde::Serialize;
use ts_rs::TS;

use super::Game;
use crate::engine::{self, TeamStrength, poisson, uniform};
use crate::league::Fixture;
use crate::tactics::{Availability, Instructions, SquadState};
use crate::{ClubTactics, Result, date, regulation};

/// Os resultados e quem jogou (save 40).
pub(super) const RESERVES_SCHEMA: &str = "
CREATE TABLE reserve_matches (fixture INTEGER PRIMARY KEY, date TEXT NOT NULL, home_id INTEGER NOT NULL, away_id INTEGER NOT NULL,
                              home_goals INTEGER NOT NULL, away_goals INTEGER NOT NULL);
CREATE TABLE reserve_apps (person_id INTEGER NOT NULL, fixture INTEGER NOT NULL, date TEXT NOT NULL, club INTEGER NOT NULL,
                           rating REAL NOT NULL, goals INTEGER NOT NULL, assists INTEGER NOT NULL);
CREATE INDEX reserve_apps_person ON reserve_apps (person_id, date);
";

/// Os escolhidos pelo treinador para o onze das reservas (save 41).
pub(super) const PICKS_SCHEMA: &str = "CREATE TABLE reserve_picks (person_id INTEGER PRIMARY KEY, ord INTEGER NOT NULL);";

/// O onze das reservas do treinador para o próximo jogo delas: a função de cada um e quem
/// foi escolhido por ele (os outros, o adjunto).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ReserveLineup {
    pub starters: Vec<ReserveSpot>,
    pub picks: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ReserveSpot {
    pub person: i32,
    /// "GR", "D D", "M C"...
    pub role: String,
}

/// O nome da competição no Calendário e no perfil.
pub(super) const RESERVES_LABEL: &str = "Reservas";
/// Dias antes do jogo do campeonato.
const DAYS_BEFORE: i32 = 2;
/// Dias mínimos entre dois jogos das reservas (mais do que isto).
const MIN_GAP: i32 = 3;
/// As reservas da IA: quem fica fora dos melhores do plantel (os que jogam no dia seguinte).
const FIRST_TEAM: usize = 18;
/// Os fantasmas das reservas valem esta parte do nível do enchimento do clube.
const GHOST_SHARE: f64 = 0.7;
/// Mistura na seed do jogo: o sorteio das reservas é só delas.
const RNG_KEY: i64 = 0x5E5E_8E5E;
/// Peso de cada setor (GR, defesa, meio, ataque) para marcar e para assistir.
const SCORE_WEIGHT: [f64; 4] = [0.0, 1.0, 3.0, 6.0];
const ASSIST_WEIGHT: [f64; 4] = [0.2, 1.0, 3.0, 2.0];
/// Probabilidade de um golo ter assistência.
const ASSIST_CHANCE: f64 = 0.7;

/// O jogo de um jogador nas reservas.
struct App {
    person: i32,
    club: i32,
    rating: f64,
    goals: i32,
    assists: i32,
}

/// Um jogo das reservas: o jogo do campeonato de que sai, a data e o campo (trocado).
struct Slot {
    fixture: i32,
    competition: i32,
    round: i32,
    league_date: String,
    date: String,
    home: i32,
    away: i32,
}

impl Game {
    /// O clube tem uma equipa B à parte (o Barça B, o Benfica B; ou uma II numa liga jogada): no
    /// CM, esses clubes não têm reservas nem jogos delas. A II numa liga fora do jogo (o Bayern
    /// II na Regionalliga) faz de reservas e joga-os.
    fn b_team_plays(&self, club: i32) -> Result<bool> {
        Ok(matches!(self.reserve_mode_of(club)?, super::reserves::ReserveMode::BTeam(_)))
    }

    /// Os jogos das reservas do clube nesta época, pelos jogos do campeonato dele.
    fn reserve_slots(&self, club: i32) -> Result<Vec<Slot>> {
        let Some(division) = self.division_of(club)? else { return Ok(Vec::new()) };
        if self.b_team_plays(club)? {
            return Ok(Vec::new());
        }
        let mine = |home: i32, away: i32| home == club || away == club;
        let busy: std::collections::HashSet<&str> =
            self.matches.iter().filter(|m| mine(m.home_id, m.away_id)).map(|m| m.date.as_str()).collect();
        let mut league: Vec<_> = self.season_matches().filter(|m| mine(m.home_id, m.away_id) && m.competition_id == division).collect();
        league.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
        let mut out: Vec<Slot> = Vec::new();
        let mut b_teams = std::collections::HashMap::new();
        for m in league {
            let Some(day) = date::add_days(&m.date, -DAYS_BEFORE) else { continue };
            let Some(next) = date::add_days(&day, 1) else { continue };
            if busy.contains(day.as_str()) || busy.contains(next.as_str()) {
                continue;
            }
            let gap = |s: &Slot| Some(date::parse(&day)? - date::parse(&s.date)?);
            if out.last().is_some_and(|s| gap(s).is_some_and(|d| d <= MIN_GAP)) {
                continue;
            }
            let opponent = if m.home_id == club { m.away_id } else { m.home_id };
            let b_team = match b_teams.get(&opponent) {
                Some(&b) => b,
                None => *b_teams.entry(opponent).or_insert(self.b_team_plays(opponent)?),
            };
            if b_team {
                continue;
            }
            out.push(Slot {
                fixture: m.id,
                competition: m.competition_id,
                round: m.round,
                league_date: m.date.clone(),
                date: day,
                home: m.away_id,
                away: m.home_id,
            });
        }
        Ok(out)
    }

    /// O Calendário das reservas do clube do treinador (vazio nos outros): os jogados com o
    /// resultado e os que faltam.
    pub fn reserve_fixtures(&self, club: i32) -> Result<Vec<Fixture>> {
        if club != self.club_id || self.sacked() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for s in self.reserve_slots(club)? {
            let result: Option<(i32, i32)> = self
                .world
                .conn
                .query_row("SELECT home_goals, away_goals FROM reserve_matches WHERE fixture = ?", [s.fixture], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .ok();
            out.push(Fixture {
                // Negativo: a ficha do jogo das reservas, não a do jogo do campeonato de que sai
                id: -s.fixture,
                competition_id: s.competition,
                competition: RESERVES_LABEL.into(),
                round: s.round,
                date: s.date,
                time: super::AFTERNOON_KICKOFF.into(),
                home: self.club(s.home),
                away: self.club(s.away),
                home_goals: result.map(|r| r.0),
                away_goals: result.map(|r| r.1),
                label: RESERVES_LABEL.into(),
                season: regulation::season_label(s.competition, regulation::season_of(s.competition, &s.league_date)),
                neutral: false,
                home_pens: None,
                away_pens: None,
            });
        }
        Ok(out)
    }

    /// Cada dia: o jogo das reservas do clube do treinador, se for hoje, com a notícia do
    /// resultado (em Competições, com o botão para a ficha do jogo).
    pub(super) fn reserve_day(&mut self, fresh: &mut Vec<crate::NewsItem>, next_id: &mut i32) -> Result<()> {
        if self.sacked() {
            return Ok(());
        }
        let today: Vec<Slot> = self.reserve_slots(self.club_id)?.into_iter().filter(|s| s.date == self.date).collect();
        for s in today {
            let done: bool =
                self.world.conn.query_row("SELECT COUNT(*) > 0 FROM reserve_matches WHERE fixture = ?", [s.fixture], |r| r.get(0))?;
            if done {
                continue;
            }
            self.play_reserves(s.fixture, s.home, s.away, s.competition)?;
            fresh.push(self.reserve_news(*next_id, s.fixture)?);
            *next_id += 1;
        }
        Ok(())
    }

    /// A notícia do resultado das reservas: o resultado e os marcadores.
    fn reserve_news(&self, id: i32, fixture: i32) -> Result<crate::NewsItem> {
        let sheet = self.reserve_sheet(fixture)?;
        let f = &sheet.fixture;
        let (hg, ag) = (f.home_goals.unwrap_or(0), f.away_goals.unwrap_or(0));
        let at_home = f.home.id == self.club_id;
        let (ours, theirs, gf, ga) = if at_home { (&f.home, &f.away, hg, ag) } else { (&f.away, &f.home, ag, hg) };
        let place = if at_home { "em casa" } else { "fora" };
        let what = match gf.cmp(&ga) {
            std::cmp::Ordering::Greater => {
                format!("As reservas do {} venceram {place} as do {} por {gf}-{ga}.", ours.short_name, theirs.short_name)
            }
            std::cmp::Ordering::Equal => {
                format!("As reservas do {} empataram {place} com as do {}: {gf}-{ga}.", ours.short_name, theirs.short_name)
            }
            std::cmp::Ordering::Less => {
                format!("As reservas do {} perderam {place} com as do {} por {gf}-{ga}.", ours.short_name, theirs.short_name)
            }
        };
        let mut scorers = String::new();
        for (club, lines) in [(&f.home, &sheet.home), (&f.away, &sheet.away)] {
            let list: Vec<String> = lines
                .iter()
                .filter(|l| l.goals > 0)
                .map(|l| if l.goals > 1 { format!("{} ({})", l.name, l.goals) } else { l.name.clone() })
                .collect();
            if !list.is_empty() {
                scorers += &format!("\n{}: {}", club.short_name, list.join(", "));
            }
        }
        if !scorers.is_empty() {
            scorers = format!("\n\nMarcadores:{scorers}");
        }
        let mut n = self.news_item(
            id,
            "competition",
            format!("Reservas: {} {hg}-{ag} {}", f.home.short_name, f.away.short_name),
            format!("Jogo das reservas.\n\n{what}{scorers}"),
        );
        n.link = Some(format!("sheet:{}", -fixture));
        Ok(n)
    }

    /// A ficha de um jogo das reservas já jogado (o `fixture` é o do jogo do campeonato).
    pub(super) fn reserve_sheet(&self, fixture: i32) -> Result<super::player_record::MatchSheet> {
        let (date, home, away, hg, ag): (String, i32, i32, i32, i32) = self
            .world
            .conn
            .query_row("SELECT date, home_id, away_id, home_goals, away_goals FROM reserve_matches WHERE fixture = ?", [fixture], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .map_err(|_| crate::Error::Invalid("Este jogo das reservas ainda não se jogou.".into()))?;
        let league = self.matches.iter().find(|m| m.id == fixture);
        let competition = league.map(|m| m.competition_id).or(self.division_of(home)?).unwrap_or(0);
        let round = league.map(|m| m.round).unwrap_or(0);
        let league_date = league.map(|m| m.date.clone()).unwrap_or_else(|| date.clone());
        let rows: Vec<(i32, i32, String, Option<i32>, f64, i32, i32)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT a.person_id, a.club, COALESCE((SELECT nickname FROM nicknames n WHERE n.person_id = p.id), p.list_name),
                        pl.squad_number, ROUND(a.rating, 1), a.goals, a.assists
                 FROM reserve_apps a JOIN people p ON p.id = a.person_id LEFT JOIN players pl ON pl.person_id = p.id
                 WHERE a.fixture = ? ORDER BY a.rowid",
            )?
            .query_map([fixture], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let best = rows.iter().map(|r| r.4).fold(f64::MIN, f64::max);
        let (mut home_lines, mut away_lines) = (Vec::new(), Vec::new());
        for (id, club, name, number, rating, goals, assists) in rows {
            let line = super::player_record::SheetLine {
                id,
                name,
                number,
                rating,
                goals,
                assists,
                mom: rating >= best,
                on: Some(0),
                off: None,
                yellow: false,
                red: false,
            };
            if club == away { away_lines.push(line) } else { home_lines.push(line) }
        }
        let fixture_view = Fixture {
            id: -fixture,
            competition_id: competition,
            competition: RESERVES_LABEL.into(),
            round,
            date,
            time: super::AFTERNOON_KICKOFF.into(),
            home: self.club(home),
            away: self.club(away),
            home_goals: Some(hg),
            away_goals: Some(ag),
            label: RESERVES_LABEL.into(),
            season: regulation::season_label(competition, regulation::season_of(competition, &league_date)),
            neutral: false,
            home_pens: None,
            away_pens: None,
        };
        Ok(super::player_record::MatchSheet { fixture: fixture_view, home: home_lines, away: away_lines })
    }

    /// A equipa das reservas de um clube: o adjunto escolhe o onze entre quem joga nelas (os
    /// despromovidos, no clube do treinador; na IA, quem fica fora dos 18 melhores).
    pub(super) fn reserve_team(&self, club: i32, comp: i32) -> Result<ClubTactics> {
        let state = self.club_state(club, Some(comp));
        let players: Vec<i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC, p.id",
            )?
            .query_map([club], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let own = club == self.club_id;
        let fresh = Availability { condition: 100, morale: 12, injured: false, suspended: false, reserve: false };
        // As reservas relacionadas pelo treinador para a equipa principal não jogam nas reservas
        let first_team: std::collections::HashSet<i32> = match (&self.selection, own) {
            (Some(sel), true) => sel.starters.iter().flatten().chain(&sel.bench).copied().collect(),
            _ => Default::default(),
        };
        let mut pool = SquadState::new();
        for (rank, id) in players.into_iter().enumerate() {
            let a = state.get(&id).copied().unwrap_or(fresh);
            let plays = if own { a.reserve && !first_team.contains(&id) } else { rank >= FIRST_TEAM };
            // Os castigos são das competições oficiais; quem não joga fica "lesionado" para o adjunto
            pool.insert(id, Availability { injured: a.injured || !plays, suspended: false, reserve: false, ..a });
        }
        // Os escolhidos pelo treinador: o adjunto arruma-os no onze e completa com as outras reservas
        let picks: Vec<i32> = if own { self.reserve_picks()? } else { Vec::new() };
        let picks: Vec<i32> = picks.into_iter().filter(|id| pool.get(id).is_some_and(|a| a.can_play())).collect();
        if picks.is_empty() {
            return self.world.auto_tactics(club, Some(&pool), Instructions::default());
        }
        let mut only = pool.clone();
        for (id, a) in only.iter_mut() {
            a.injured |= !picks.contains(id);
        }
        let mut sel = self.world.auto_tactics(club, Some(&only), Instructions::default())?.selection();
        sel.bench.clear();
        self.world.cover_unavailable(club, &mut sel, &pool)?;
        self.world.tactics_from_selection(club, &sel, Some(&pool))
    }

    /// Os escolhidos para as reservas, pela ordem em que foram escolhidos.
    fn reserve_picks(&self) -> Result<Vec<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT person_id FROM reserve_picks ORDER BY ord, person_id")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// O onze das reservas para o próximo jogo delas (o que o adjunto faria hoje).
    pub fn reserve_lineup(&self) -> Result<ReserveLineup> {
        let comp = self.division_of(self.club_id)?.unwrap_or(0);
        let t = self.reserve_team(self.club_id, comp)?;
        let starters =
            t.starters.iter().filter_map(|s| s.player.as_ref().map(|p| ReserveSpot { person: p.id, role: s.role.clone() })).collect();
        let reserves: Vec<i32> =
            self.squads.squad_state(self.club_id, None).into_iter().filter(|(_, a)| a.reserve).map(|(id, _)| id).collect();
        let picks = self.reserve_picks()?.into_iter().filter(|id| reserves.contains(id)).collect();
        Ok(ReserveLineup { starters, picks })
    }

    /// O treinador escolhe quem joga nas reservas (até 11, todos das reservas); vazio, o adjunto.
    pub fn set_reserve_picks(&mut self, picks: &[i32]) -> Result<ReserveLineup> {
        if picks.len() > 11 {
            return Err(crate::Error::Invalid("A equipa das reservas tem 11 jogadores.".into()));
        }
        for id in picks {
            if !self.squads.state(*id).reserve
                || !self
                    .world
                    .conn
                    .query_row("SELECT club_id = ? FROM people WHERE id = ?", params![self.club_id, id], |r| r.get::<_, bool>(0))?
            {
                return Err(crate::Error::Invalid("Só jogam nas reservas os jogadores que estão nelas.".into()));
            }
        }
        self.world.conn.execute("DELETE FROM reserve_picks", [])?;
        for (i, id) in picks.iter().enumerate() {
            self.world.conn.execute("INSERT OR IGNORE INTO reserve_picks (person_id, ord) VALUES (?, ?)", params![id, i as i32])?;
        }
        self.unsaved = true;
        self.reserve_lineup()
    }

    /// O nível (CA) dos fantasmas que completam as reservas de um clube: o dos jogadores de
    /// enchimento do clube (`regen::complete_squads`), um pouco abaixo por serem das reservas.
    pub(super) fn ghost_ability(&self, club: i32) -> Result<f64> {
        let reputation: Option<i32> = self.world.conn.query_row("SELECT reputation FROM clubs WHERE id = ?", [club], |r| r.get(0))?;
        Ok(GHOST_SHARE * (74.6 + 0.0101 * reputation.unwrap_or(1000) as f64))
    }

    /// Joga o jogo das reservas e grava o resultado. Há sempre jogo: as funções sem jogador
    /// ficam com fantasmas, como no CM (só neste jogo, sem registo).
    fn play_reserves(&mut self, fixture: i32, home: i32, away: i32, comp: i32) -> Result<()> {
        let teams = [self.reserve_team(home, comp)?, self.reserve_team(away, comp)?];
        let strength = |t: &ClubTactics| -> Result<TeamStrength> { Ok(TeamStrength::from_tactics_with(t, self.ghost_ability(t.club_id)?)) };
        let mut rng = engine::match_rng(self.seed ^ RNG_KEY, fixture);
        let (xh, xa) = engine::expected_goals(&strength(&teams[0])?, &strength(&teams[1])?);
        let goals = [poisson(xh, &mut rng) as i32, poisson(xa, &mut rng) as i32];

        // Quem jogou, os marcadores e as assistências (pelo setor de cada um, os fantasmas
        // também: os golos deles não ficam com nome) e as notas
        let mut apps: Vec<App> = Vec::new();
        for (side, t) in teams.iter().enumerate() {
            let start = apps.len();
            let sectors: Vec<usize> = t.starters.iter().map(|s| engine::sector(&s.role)).collect();
            // A posição em `apps` de cada função (os fantasmas não têm)
            let mut at: Vec<Option<usize>> = Vec::new();
            for s in &t.starters {
                at.push(s.player.as_ref().map(|p| {
                    apps.push(App { person: p.id, club: t.club_id, rating: 0.0, goals: 0, assists: 0 });
                    apps.len() - 1
                }));
            }
            for _ in 0..goals[side] {
                let scorer = pick(&sectors, &SCORE_WEIGHT, None, &mut rng);
                if let Some(i) = at[scorer] {
                    apps[i].goals += 1;
                }
                if uniform(&mut rng) < ASSIST_CHANCE
                    && let Some(i) = at[pick(&sectors, &ASSIST_WEIGHT, Some(scorer), &mut rng)]
                {
                    apps[i].assists += 1;
                }
            }
            let diff = (goals[side] - goals[1 - side]).clamp(-3, 3) as f64;
            for a in &mut apps[start..] {
                let noise = uniform(&mut rng) * 1.2 - 0.6;
                let r = 6.5 + 0.25 * diff + 0.6 * a.goals as f64 + 0.3 * a.assists as f64 + noise;
                a.rating = (r.clamp(4.0, 10.0) * 10.0).round() / 10.0;
            }
        }

        let conn = &self.world.conn;
        conn.execute(
            "INSERT INTO reserve_matches (fixture, date, home_id, away_id, home_goals, away_goals) VALUES (?, ?, ?, ?, ?, ?)",
            params![fixture, self.date, home, away, goals[0], goals[1]],
        )?;
        for a in &apps {
            // O clube do jogador (o "(R)" ou o principal), para a época dele no perfil
            let club: i32 =
                conn.query_row("SELECT COALESCE(club_id, ?) FROM people WHERE id = ?", params![a.club, a.person], |r| r.get(0))?;
            conn.execute(
                "INSERT INTO reserve_apps (person_id, fixture, date, club, rating, goals, assists) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![a.person, fixture, self.date, club, a.rating, a.goals, a.assists],
            )?;
        }
        self.unsaved = true;
        Ok(())
    }
}

/// Um índice ao acaso, pelo peso do setor de cada um (sem `skip`, quem marcou).
fn pick(sectors: &[usize], weight: &[f64; 4], skip: Option<usize>, rng: &mut impl RngCore) -> usize {
    let w = |i: usize| if Some(i) == skip { 0.0 } else { weight[sectors[i]] };
    let total: f64 = (0..sectors.len()).map(w).sum();
    if total <= 0.0 {
        return 0;
    }
    let mut roll = uniform(rng) * total;
    for i in 0..sectors.len() {
        roll -= w(i);
        if roll <= 0.0 {
            return i;
        }
    }
    sectors.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn reserves_play_two_days_before_the_league() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-jogos-reservas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Jogos das reservas".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
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
        let mut g = Game::create(&path, &dir, &new, 31).unwrap();
        // Clubes com equipa B numa liga não têm reservas (Barcelona B, Bayern II); a Juventus tem
        assert!(g.world.has_b_team(1283).unwrap() && g.world.has_b_team(1350).unwrap());
        assert!(!g.world.has_b_team(5138).unwrap() && !g.world.has_b_team(PALMEIRAS).unwrap());
        // O Bayern II joga na Regionalliga, que não está no jogo: as reservas do Bayern jogam
        assert!(!g.b_team_plays(1350).unwrap());

        // Só três despromovidos: o jogo faz-se na mesma, com fantasmas
        let worst: Vec<i32> = g
            .world
            .conn
            .prepare("SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability, p.id")
            .unwrap()
            .query_map([PALMEIRAS], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for id in worst.iter().filter(|id| !g.squads.state(**id).reserve).take(3).copied().collect::<Vec<_>>() {
            g.set_reserve(id, true).unwrap();
        }
        let demoted = worst.iter().filter(|id| g.squads.state(**id).reserve).count();

        // O treinador escolhe quem joga: os escolhidos entram no onze; de fora das reservas, recusa
        let reserves: Vec<i32> = worst.iter().filter(|id| g.squads.state(**id).reserve).copied().collect();
        let pick = *reserves.last().unwrap();
        let lineup = g.set_reserve_picks(&[pick]).unwrap();
        assert_eq!(lineup.picks, vec![pick]);
        assert!(lineup.starters.iter().any(|s| s.person == pick));
        let first_team = *worst.iter().rev().find(|id| !g.squads.state(**id).reserve).unwrap();
        assert!(g.set_reserve_picks(&[first_team]).is_err());
        assert!(g.set_reserve_picks(&[pick; 12]).is_err());
        assert!(g.set_reserve_picks(&[]).unwrap().picks.is_empty());
        g.set_reserve_picks(&[pick]).unwrap();

        // Dois dias antes de um jogo do campeonato, com o campo trocado, e nunca com menos de 4 dias entre eles
        let reserves = g.reserve_fixtures(PALMEIRAS).unwrap();
        assert!(reserves.len() >= 10, "{}", reserves.len());
        let league = g.club_fixtures(PALMEIRAS);
        for r in &reserves {
            let m = league.iter().find(|f| f.id == -r.id).unwrap();
            assert_eq!(date::add_days(&r.date, DAYS_BEFORE).as_deref(), Some(m.date.as_str()));
            assert_eq!((r.home.id, r.away.id), (m.away.id, m.home.id));
            assert!(!league.iter().any(|f| f.date == r.date), "o clube não joga no dia das reservas");
        }
        for w in reserves.windows(2) {
            assert!(date::parse(&w[1].date).unwrap() - date::parse(&w[0].date).unwrap() > MIN_GAP);
        }

        // Joga o primeiro: o resultado fica no Calendário e no perfil ("Não Competitivos")
        let first = reserves[0].clone();
        while g.date <= first.date {
            g.advance_day().unwrap();
        }
        let played = g.reserve_fixtures(PALMEIRAS).unwrap();
        let done = played.iter().find(|f| f.id == first.id).unwrap();
        assert!(done.home_goals.is_some() && done.away_goals.is_some());
        let player: i32 = g
            .world
            .conn
            .query_row("SELECT person_id FROM reserve_apps WHERE fixture = ? AND club = ? LIMIT 1", params![-first.id, PALMEIRAS], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(g.squads.state(player).reserve, "só os despromovidos jogam nas reservas do treinador");
        let ours: i64 = g
            .world
            .conn
            .query_row("SELECT COUNT(*) FROM reserve_apps WHERE fixture = ? AND club = ?", params![-first.id, PALMEIRAS], |r| r.get(0))
            .unwrap();
        assert!(ours as usize <= demoted.min(11), "{ours} {demoted}");
        let picked: bool = g
            .world
            .conn
            .query_row("SELECT COUNT(*) > 0 FROM reserve_apps WHERE fixture = ? AND person_id = ?", params![-first.id, pick], |r| r.get(0))
            .unwrap();
        assert!(picked, "o escolhido jogou");
        // A notícia do resultado, com a ficha do jogo (o id negativo)
        let news = g.news.iter().find(|n| n.title.starts_with("Reservas:")).expect("notícia das reservas");
        assert_eq!(news.link.as_deref(), Some(format!("sheet:{}", first.id).as_str()));
        let sheet = g.match_sheet(first.id).unwrap();
        assert_eq!(sheet.fixture.id, first.id);
        assert!(sheet.home.iter().chain(&sheet.away).any(|l| l.id == pick));
        let mut p = g.world.get_person(player).unwrap();
        g.fill_record(&mut p).unwrap();
        let season = &p.seasons[0];
        assert_eq!(season.by_kind[0].0, "Não Competitivos");
        assert_eq!(season.by_kind[0].1.apps, 1);
        assert_eq!(season.by_kind.last().unwrap().0, "Jogos Oficiais");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
