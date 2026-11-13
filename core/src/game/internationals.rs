//! Selecções (marco 3.18): a época internacional (de 1/8 a 31/7, num ciclo de 4 anos), os
//! sorteios das fases finais, as convocatórias dos seleccionadores, os convocados fora do
//! clube ("Int") e o que os jogos pela selecção deixam: internacionalizações, golos, moral e
//! lesões. Os convocados ficam ligados à selecção por `people.national_team_id`, então o
//! plantel da selecção e a escolha da equipa são os de qualquer clube.

use std::collections::{BTreeMap, HashMap, HashSet};

use rand_core::RngCore;
use rusqlite::{OptionalExtension, params};

use super::{Game, NewsItem, cm_date};
use crate::league::Match;
use crate::live::LiveMatch;
use crate::personality;
use crate::regulation::{self, SeasonKind};
use crate::squad::Event;
use crate::tactics::SquadState;
use crate::{Result, date};

/// Os convocados de cada selecção e o dia em que saem do clube (save 45).
pub(super) const CALL_UPS_SCHEMA: &str = "
CREATE TABLE call_ups (person_id INTEGER PRIMARY KEY, team_id INTEGER NOT NULL, leaves TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS idx_people_national ON people (national_team_id);
";

/// Os amigáveis internacionais (save 46): uma competição sem regulamento, criada no save.
pub(super) use crate::regulation::FRIENDLY;

/// A competição dos amigáveis e os jogos de cada jogador pelas selecções, para o Historial (save 46).
pub(super) const FRIENDLY_SCHEMA: &str = "
INSERT OR IGNORE INTO competitions (id, kind, name, short_name, abbreviation, continent_id, nation_id, reputation)
    VALUES (458, 'national', 'Amigáveis Internacionais', 'Amigáveis', 'AMI', NULL, NULL, 5);
CREATE TABLE intl_apps (person_id INTEGER NOT NULL, date TEXT NOT NULL, club INTEGER, team INTEGER NOT NULL,
                        competition INTEGER NOT NULL, goals INTEGER NOT NULL, assists INTEGER NOT NULL, rating REAL NOT NULL);
CREATE INDEX intl_apps_person ON intl_apps (person_id);
";

/// A ficha da partida dos jogos de selecções (save 47): o jogo, quem entrou e saiu, cartões e
/// o homem do jogo de cada linha de `intl_apps`.
pub(super) const SHEET_SCHEMA: &str = "
ALTER TABLE intl_apps ADD COLUMN fixture INTEGER;
ALTER TABLE intl_apps ADD COLUMN sub INTEGER NOT NULL DEFAULT 0;
ALTER TABLE intl_apps ADD COLUMN mom INTEGER NOT NULL DEFAULT 0;
ALTER TABLE intl_apps ADD COLUMN minute_on INTEGER;
ALTER TABLE intl_apps ADD COLUMN minute_off INTEGER;
ALTER TABLE intl_apps ADD COLUMN cards INTEGER NOT NULL DEFAULT 0;
CREATE INDEX intl_apps_fixture ON intl_apps (fixture);
";

/// As datas FIFA de uma época internacional (sábado e quarta), onde cabem os amigáveis.
const WINDOWS: [&str; 5] = ["w36 m37", "w41 m42", "w46 m47", "w12 m13", "w23 m24"];
/// Só as selecções mais fortes marcam amigáveis.
const FRIENDLY_TEAMS: usize = 120;

/// Dias antes do primeiro jogo da janela em que sai a convocatória.
const CALL_AHEAD: i32 = 7;
/// Dias antes do primeiro jogo em que os convocados saem do clube.
const LEAVE_BEFORE: i32 = 3;
/// Uma selecção só entra nas competições com estes jogadores com clube.
const MIN_PLAYERS: i32 = 18;
/// A convocatória: guarda-redes, defesas, médios e avançados (23).
const QUOTAS: [usize; 4] = [3, 8, 8, 4];
const SQUAD_SIZE: usize = 23;

/// O sector de um jogador pelas posições do banco: 0 GR, 1 defesa, 2 médio, 3 avançado.
fn sector(gk: i32, d: i32, m: i32, a: i32) -> usize {
    if gk >= 15 {
        return 0;
    }
    if d >= m && d >= a {
        1
    } else if m >= a {
        2
    } else {
        3
    }
}

/// Os 23 de uma lista ordenada pelo CA: as quotas por sector e o resto pelos melhores (sem
/// mais guarda-redes).
fn pick_squad(candidates: &[(i32, usize)]) -> Vec<i32> {
    let mut taken = [0usize; 4];
    let mut out: Vec<i32> = Vec::new();
    for &(id, s) in candidates {
        if out.len() < SQUAD_SIZE && taken[s] < QUOTAS[s] {
            taken[s] += 1;
            out.push(id);
        }
    }
    for &(id, s) in candidates {
        if out.len() >= SQUAD_SIZE {
            break;
        }
        if s != 0 && !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

pub(super) fn is_national_comp(comp: i32) -> bool {
    regulation::season_kind(comp) == SeasonKind::International
}

impl Game {
    /// As selecções principais de um continente (a de mais reputação de cada nação, com
    /// jogadores que cheguem), da mais forte para a mais fraca, com o nome da nação.
    pub(super) fn senior_teams(&self, continent: i32) -> Result<Vec<(i32, String)>> {
        let rows: Vec<(i32, i32, String)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT c.id, c.nation_id, n.name FROM clubs c JOIN nations n ON n.id = c.nation_id
                 WHERE c.is_national_team = 1 AND n.continent_id = ?1
                   AND (SELECT COUNT(*) FROM people p WHERE p.nation_id = c.nation_id AND p.is_player = 1 AND p.club_id IS NOT NULL) >= ?2
                 ORDER BY c.reputation DESC, c.id",
            )?
            .query_map(params![continent, MIN_PLAYERS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut seen = HashSet::new();
        Ok(rows.into_iter().filter(|(_, nation, _)| seen.insert(*nation)).map(|(id, _, name)| (id, name)).collect())
    }

    /// A selecção principal de uma nação, pelo nome.
    pub(super) fn senior_team_of(&self, nation: &str) -> Result<Option<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached(
                "SELECT c.id FROM clubs c JOIN nations n ON n.id = c.nation_id
                 WHERE c.is_national_team = 1 AND n.name = ? ORDER BY c.reputation DESC, c.id LIMIT 1",
            )?
            .query_row([nation], |r| r.get(0))
            .optional()?)
    }

    pub(super) fn is_national_team(&self, club: i32) -> bool {
        self.world
            .conn
            .prepare_cached("SELECT is_national_team FROM clubs WHERE id = ?")
            .and_then(|mut s| s.query_row([club], |r| r.get::<_, i32>(0)).optional())
            .ok()
            .flatten()
            .is_some_and(|v| v != 0)
    }

    /// Os convocados que já saíram do clube (indisponíveis para ele).
    pub(super) fn away_internationals(&self) -> Vec<i32> {
        let rows = || -> Result<Vec<i32>> {
            Ok(self
                .world
                .conn
                .prepare_cached("SELECT person_id FROM call_ups WHERE leaves <= ?")?
                .query_map([&self.date], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?)
        };
        rows().unwrap_or_default()
    }

    fn called_by(&self, team: i32) -> Result<Vec<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT person_id FROM call_ups WHERE team_id = ? ORDER BY person_id")?
            .query_map([team], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// A disponibilidade dos convocados de uma selecção para um jogo de `comp`.
    pub(super) fn national_state(&self, team: i32, comp: Option<i32>) -> SquadState {
        let ids = self.called_by(team).unwrap_or_default();
        ids.into_iter()
            .map(|id| {
                let mut a = self.squads.state(id).availability(comp);
                a.reserve = false;
                (id, a)
            })
            .collect()
    }

    /// 1/8: a época internacional. Os amarelos das competições de selecções acabam e
    /// sorteiam-se as do ciclo (as fases finais esperam pela data do sorteio).
    pub(super) fn new_international_season(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let year = regulation::season_of(ANY_NATIONAL, &self.date);
        for st in self.squads.states.values_mut() {
            st.clear_cards(is_national_comp);
        }
        self.settle_hosts(year)?;
        self.schedule_season(SeasonKind::International, year, fresh, next_id)?;
        self.schedule_friendlies(year)?;
        self.refresh_live()
    }

    /// As datas oficiais (em dias) dos jogos por fazer de cada selecção, sem os amigáveis.
    fn official_days(&self) -> HashMap<i32, Vec<i32>> {
        let mut out: HashMap<i32, Vec<i32>> = HashMap::new();
        for m in self.matches.iter().filter(|m| m.result.is_none() && m.competition_id != FRIENDLY && is_national_comp(m.competition_id)) {
            let Some(d) = date::parse(&m.date) else { continue };
            for t in [m.home_id, m.away_id] {
                out.entry(t).or_default().push(d);
            }
        }
        out
    }

    /// Amigáveis nas datas FIFA da época em que as selecções mais fortes não têm jogo oficial:
    /// cada uma com uma das seis seguintes pela reputação (de preferência do mesmo continente),
    /// sem repetir o adversário na mesma janela. Sorteados com a seed, a época e a janela.
    fn schedule_friendlies(&mut self, year: i32) -> Result<()> {
        let mut teams: Vec<(i32, i32, i32)> = Vec::new();
        for continent in 0..6 {
            for (id, _) in self.senior_teams(continent)? {
                let rep: i32 = self
                    .world
                    .conn
                    .prepare_cached("SELECT COALESCE(reputation, 0) FROM clubs WHERE id = ?")?
                    .query_row([id], |r| r.get(0))?;
                teams.push((id, rep, continent));
            }
        }
        teams.sort_by_key(|t| (std::cmp::Reverse(t.1), t.0));
        teams.truncate(FRIENDLY_TEAMS);
        let official = self.official_days();
        let mut id = self.next_match_id();
        for (w, spec) in WINDOWS.iter().enumerate() {
            let mut rng = self.comp_rng(FRIENDLY, year, w as u64 + 1);
            let mut met: HashSet<(i32, i32)> = HashSet::new();
            for (k, day) in regulation::dates_in(year, spec, SeasonKind::International).into_iter().enumerate() {
                let Some(d) = date::parse(&day) else { continue };
                let mut open: Vec<(i32, i32, i32)> = teams
                    .iter()
                    .copied()
                    .filter(|t| official.get(&t.0).is_none_or(|days| days.iter().all(|x| (x - d).abs() > 1)))
                    .collect();
                while open.len() >= 2 {
                    let a = open.remove(0);
                    let near: Vec<usize> =
                        (0..open.len().min(6)).filter(|&i| !met.contains(&(a.0.min(open[i].0), a.0.max(open[i].0)))).collect();
                    let same: Vec<usize> = near.iter().copied().filter(|&i| open[i].2 == a.2).collect();
                    let pool = if same.is_empty() { &near } else { &same };
                    if pool.is_empty() {
                        continue;
                    }
                    let b = open.remove(pool[(rng.next_u64() % pool.len() as u64) as usize]);
                    met.insert((a.0.min(b.0), a.0.max(b.0)));
                    let (home_id, away_id) = if rng.next_u64().is_multiple_of(2) { (a.0, b.0) } else { (b.0, a.0) };
                    self.matches.push(Match {
                        id,
                        competition_id: FRIENDLY,
                        round: (2 * w + k + 1) as i32,
                        date: day.clone(),
                        home_id,
                        away_id,
                        ..Default::default()
                    });
                    id += 1;
                }
            }
        }
        Ok(())
    }

    /// Um amigável que choca com um jogo oficial sorteado depois (uma fase final) sai do calendário.
    fn drop_clashing_friendlies(&mut self) {
        let official = self.official_days();
        let clash = |team: i32, d: i32| official.get(&team).is_some_and(|days| days.iter().any(|x| (x - d).abs() <= 1));
        self.matches.retain(|m| {
            if m.competition_id != FRIENDLY || m.result.is_some() {
                return true;
            }
            let Some(d) = date::parse(&m.date) else { return true };
            !(clash(m.home_id, d) || clash(m.away_id, d))
        });
    }

    /// Os sorteios das fases finais marcados para hoje, com uma notícia dos grupos.
    fn international_draws(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let year = regulation::season_of(ANY_NATIONAL, &self.date);
        let mut drawn = false;
        for reg in regulation::all().iter().filter(|r| r.is_national() && r.in_cycle(year)) {
            let Some(spec) = &reg.draw else { continue };
            let on = regulation::dates_in(year, spec, SeasonKind::International);
            if on.first().map(String::as_str) != Some(self.date.as_str()) || self.year_matches(reg.competition, year).next().is_some() {
                continue;
            }
            self.create_stage(reg, 0, year, fresh, next_id)?;
            drawn = true;
            let mut groups: BTreeMap<u8, Vec<i32>> = BTreeMap::new();
            for m in self.year_matches(reg.competition, year) {
                let g = groups.entry(m.group.unwrap_or(0)).or_default();
                for c in [m.home_id, m.away_id] {
                    if !g.contains(&c) {
                        g.push(c);
                    }
                }
            }
            if groups.is_empty() {
                continue;
            }
            let label = regulation::season_label(reg.competition, year);
            let lines: Vec<String> = groups
                .iter()
                .map(|(g, teams)| {
                    let names: Vec<String> = teams.iter().map(|&t| self.club(t).short_name).collect();
                    format!("Grupo {}: {}", (b'A' + g) as char, names.join(", "))
                })
                .collect();
            let name = self.competition_name(reg.competition).to_string();
            self.push_news(fresh, next_id, "competition", format!("Sorteio {} {label}", self.of_competition(reg.competition)), {
                format!("Os grupos {} {label}:\n\n{}", super::of_name(&name), lines.join("\n"))
            });
        }
        if drawn {
            self.refresh_live()?;
        }
        Ok(())
    }

    /// As datas dos jogos por fazer de cada selecção, de hoje em diante.
    fn national_fixtures(&self) -> HashMap<i32, Vec<String>> {
        let mut out: HashMap<i32, Vec<String>> = HashMap::new();
        for m in self.matches.iter().filter(|m| m.result.is_none() && m.date >= self.date && is_national_comp(m.competition_id)) {
            for t in [m.home_id, m.away_id] {
                out.entry(t).or_default().push(m.date.clone());
            }
        }
        for d in out.values_mut() {
            d.sort();
        }
        out
    }

    /// A manhã das selecções: os sorteios do dia, quem volta ao clube (a janela acabou ou
    /// está lesionado) e as convocatórias para os jogos dos próximos dias.
    pub(super) fn internationals_day(&mut self, unemployed: bool, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        // Um save de antes dos anfitriões (save 47) fica com os do regulamento
        let hosts: i32 = self.world.conn.query_row("SELECT COUNT(*) FROM hosts", [], |r| r.get(0))?;
        if hosts == 0 {
            self.settle_hosts(regulation::season_of(ANY_NATIONAL, &self.date))?;
        }
        self.announce_hosts(fresh, next_id)?;
        self.international_draws(fresh, next_id)?;
        self.drop_clashing_friendlies();
        let fixtures = self.national_fixtures();
        let horizon = date::add_days(&self.date, CALL_AHEAD).unwrap_or_default();
        let next_of = |team: i32| fixtures.get(&team).and_then(|d| d.first()).filter(|d| d.as_str() <= horizon.as_str()).cloned();

        // Regressos: a janela acabou; dispensados: lesionados para o próximo jogo
        let called: Vec<(i32, i32)> = self
            .world
            .conn
            .prepare("SELECT person_id, team_id FROM call_ups ORDER BY person_id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for &(id, team) in &called {
            match next_of(team) {
                None => self.release(id)?,
                Some(next) => {
                    let st = self.squads.state(id);
                    if st.injury_until.as_deref().is_some_and(|u| u > next.as_str()) {
                        self.release(id)?;
                        if !unemployed && self.player_club(id)? == Some(self.club_id) {
                            let name = self.person_name(id)?;
                            let team_name = self.club(team).short_name;
                            self.push_news(
                                fresh,
                                next_id,
                                "injury",
                                format!("{name} dispensado da selecção"),
                                format!("{name} foi dispensado da selecção de {team_name} devido à lesão e regressa ao clube."),
                            );
                        }
                    }
                }
            }
        }

        // Convocatórias das selecções com jogo nos próximos dias e ainda sem convocados
        let busy: HashSet<i32> = self
            .world
            .conn
            .prepare("SELECT DISTINCT team_id FROM call_ups")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut teams: Vec<(i32, String)> =
            fixtures.keys().filter(|t| !busy.contains(t)).filter_map(|&t| next_of(t).map(|d| (t, d))).collect();
        teams.sort();
        let mut ours: Vec<(i32, i32, String)> = Vec::new();
        for (team, first) in teams {
            let leaves = date::add_days(&first, -LEAVE_BEFORE).unwrap_or_else(|| first.clone());
            let leaves = if leaves < self.date { self.date.clone() } else { leaves };
            // A convocatória nova substitui a anterior no plantel da selecção
            self.world.conn.execute("UPDATE people SET national_team_id = NULL WHERE national_team_id = ? AND is_player = 1", [team])?;
            for id in self.choose_call_up(team, &first)? {
                self.world
                    .conn
                    .execute("INSERT OR REPLACE INTO call_ups (person_id, team_id, leaves) VALUES (?, ?, ?)", params![id, team, leaves])?;
                self.world.conn.execute("UPDATE people SET national_team_id = ? WHERE id = ?", params![team, id])?;
                if self.player_club(id)? == Some(self.club_id) {
                    ours.push((id, team, first.clone()));
                }
            }
        }
        if !unemployed && !ours.is_empty() {
            let mut lines = Vec::new();
            for (id, team, first) in &ours {
                lines.push(format!("{} ({}, a partir de {})", self.person_name(*id)?, self.club(*team).short_name, cm_date(first)));
            }
            let (title, body) = match ours.as_slice() {
                [(id, team, first)] => {
                    let name = self.person_name(*id)?;
                    (
                        format!("{name} convocado para a selecção"),
                        format!(
                            "{name} foi convocado para a selecção de {}. Fica fora do clube a partir de {} e regressa depois dos jogos da selecção.",
                            self.club(*team).short_name,
                            cm_date(&date::add_days(first, -LEAVE_BEFORE).unwrap_or_else(|| first.clone()))
                        ),
                    )
                }
                _ => (
                    "Convocatórias para as selecções".into(),
                    format!(
                        "Foram convocados para as selecções:\n\n{}\n\nFicam fora do clube durante os jogos das selecções.",
                        lines.join("\n")
                    ),
                ),
            };
            self.push_news(fresh, next_id, "message", title, body);
        }
        Ok(())
    }

    /// As notícias de um jogador do treinador que jogou pela selecção: golos e expulsão, e a
    /// linha do resumo do dia.
    #[allow(clippy::too_many_arguments)]
    fn our_international(
        &self,
        line: &crate::live::PlayerLine,
        debut: bool,
        score: &str,
        context: &str,
        played: &mut Vec<String>,
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
    ) -> Result<()> {
        let name = self.person_name(line.id)?;
        let minutes = (line.off.map_or(90, i32::from) - i32::from(line.on)).max(1);
        let mut text = format!("{name}: {score}, {minutes} min, nota {:.1}", line.rating);
        if line.goals > 0 {
            text.push_str(&format!(", {} {}", line.goals, if line.goals == 1 { "golo" } else { "golos" }));
        }
        if line.red {
            text.push_str(", expulso");
        }
        if debut {
            text.push_str(" (estreia)");
        }
        played.push(text);
        let first = if debut { " na estreia pela selecção" } else { "" };
        if line.goals > 0 {
            let goals = if line.goals == 1 { "um golo".to_string() } else { format!("{} golos", line.goals) };
            self.push_news(
                fresh,
                next_id,
                "message",
                format!("{name} marca pela selecção"),
                format!("{name} marcou {goals}{first} no {score}, {context}."),
            );
        }
        if line.red {
            self.push_news(
                fresh,
                next_id,
                "injury",
                format!("{name} expulso ao serviço da selecção"),
                format!("{name} foi expulso{first} no {score}, {context}. O castigo cumpre-se nos jogos da selecção."),
            );
        }
        Ok(())
    }

    fn player_club(&self, id: i32) -> Result<Option<i32>> {
        Ok(self.world.conn.prepare_cached("SELECT club_id FROM people WHERE id = ?")?.query_row([id], |r| r.get(0)).optional()?.flatten())
    }

    /// Um convocado volta ao clube. Continua ligado à selecção (`national_team_id`): o plantel
    /// dela fora das janelas é a última convocatória, até à próxima.
    fn release(&self, id: i32) -> Result<()> {
        self.world.conn.execute("DELETE FROM call_ups WHERE person_id = ?", [id])?;
        Ok(())
    }

    /// Os 23 que o seleccionador chama para os jogos a partir de `first`: os de mais CA da
    /// nação com clube, por sectores, sem os lesionados até lá, os castigados, os retirados da
    /// selecção e os de enchimento.
    pub(super) fn choose_call_up(&self, team: i32, first: &str) -> Result<Vec<i32>> {
        let banned: HashSet<i32> = self.banned_players().into_iter().collect();
        let rows: Vec<(i32, [i32; 8])> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, pl.pos_goalkeeper, pl.pos_sweeper, pl.pos_defender, pl.pos_wing_back,
                        pl.pos_defensive_midfielder, pl.pos_midfielder, pl.pos_attacking_midfielder, pl.pos_attacker
                 FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.nation_id = (SELECT nation_id FROM clubs WHERE id = ?1) AND p.is_player = 1 AND p.club_id IS NOT NULL
                   AND p.id NOT IN (SELECT person_id FROM ghosts) AND p.id NOT IN (SELECT person_id FROM retired)
                 ORDER BY pl.current_ability DESC, p.id LIMIT 120",
            )?
            .query_map([team], |r| {
                let mut pos = [0; 8];
                for (i, p) in pos.iter_mut().enumerate() {
                    *p = r.get::<_, Option<i32>>(1 + i)?.unwrap_or(1);
                }
                Ok((r.get(0)?, pos))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let candidates: Vec<(i32, usize)> = rows
            .into_iter()
            .filter(|(id, _)| !banned.contains(id) && !super::player_setup::intl_retired().contains(id))
            .filter(|(id, _)| self.squads.state(*id).injury_until.as_deref().is_none_or(|u| u <= first))
            .map(|(id, p)| (id, sector(p[0], p[1].max(p[2]).max(p[3]), p[4].max(p[5]).max(p[6]), p[7])))
            .collect();
        Ok(pick_squad(&candidates))
    }

    /// Depois de um jogo de selecções: internacionalizações e golos no perfil, a ficha da
    /// partida, a moral pelo resultado e, nos jogadores do treinador, as notícias (golos,
    /// expulsão, lesão ao serviço da selecção, que o manda de volta ao clube) e uma linha em
    /// `played` para o resumo do dia.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn international_match_done(
        &mut self,
        live: &LiveMatch,
        events: &[Event],
        unemployed: bool,
        played: &mut Vec<String>,
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
    ) -> Result<()> {
        let (h, a) = live.score();
        let comp = live.fixture.competition_id;
        let fixture = live.fixture.id;
        let score = format!("{} {h}-{a} {}", self.club(live.fixture.home.id).short_name, self.club(live.fixture.away.id).short_name);
        let context = if comp == FRIENDLY { "num amigável".to_string() } else { format!("a contar para {}", self.the_competition(comp)) };
        let mom = (0..2).flat_map(|s| live.player_lines(s)).max_by(|x, y| x.rating.total_cmp(&y.rating)).map(|l| l.id);
        for side in 0..2 {
            let result = if side == 0 { h.cmp(&a) as i32 } else { a.cmp(&h) as i32 };
            // Os castigados cumprem o jogo (o estado do clube não os vê como da selecção)
            let lines = live.player_lines(side);
            let team = if side == 0 { live.fixture.home.id } else { live.fixture.away.id };
            for id in self.called_by(team)? {
                if lines.iter().any(|l| l.id == id) {
                    continue;
                }
                if let Some(k) = self.squads.states.get_mut(&id).and_then(|s| s.cards.get_mut(&comp))
                    && k.suspended > 0
                {
                    k.suspended -= 1;
                }
            }
            for line in lines {
                let ours = !unemployed && self.player_club(line.id)? == Some(self.club_id);
                let caps_before: i32 =
                    self.world
                        .conn
                        .query_row("SELECT COALESCE(international_apps, 0) FROM people WHERE id = ?", [line.id], |r| r.get(0))?;
                // Para o Historial e a ficha da partida: o jogo pela selecção, com o clube de agora
                self.world.conn.execute(
                    "INSERT INTO intl_apps (person_id, date, club, team, competition, goals, assists, rating,
                                            fixture, sub, mom, minute_on, minute_off, cards)
                     VALUES (?, ?, (SELECT club_id FROM people WHERE id = ?), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        line.id,
                        self.date,
                        line.id,
                        team,
                        comp,
                        line.goals,
                        line.assists,
                        line.rating,
                        fixture,
                        line.sub,
                        mom == Some(line.id),
                        line.on,
                        line.off,
                        if line.red { 2 } else { line.yellow as i32 }
                    ],
                )?;
                if ours {
                    self.our_international(&line, caps_before == 0, &score, &context, played, fresh, next_id)?;
                }
                self.world.conn.execute(
                    "UPDATE people SET international_apps = COALESCE(international_apps, 0) + 1,
                                       international_goals = COALESCE(international_goals, 0) + ? WHERE id = ?",
                    params![line.goals, line.id],
                )?;
                if let Some(t) = self.squads.traits.get(&line.id).copied() {
                    let s = self.squads.states.entry(line.id).or_default();
                    s.morale = (s.morale + personality::morale_swing(result, t.temperament)).clamp(1, 20);
                }
            }
        }
        for e in events {
            let Event::Injured { player, club, injury, days } = e else { continue };
            self.release(*player)?;
            if unemployed || self.player_club(*player)? != Some(self.club_id) {
                continue;
            }
            let name = self.person_name(*player)?;
            let back = self.squads.state(*player).injury_until.map(|d| cm_date(&d)).unwrap_or_default();
            self.push_news(
                fresh,
                next_id,
                "injury",
                format!("{name} lesionou-se ao serviço da selecção"),
                format!(
                    "{name} lesionou-se no jogo de hoje pela selecção de {}: {}.\n\nO departamento médico prevê cerca de {days} dias de paragem (regresso a {back}). O jogador regressa ao clube.",
                    self.club(*club).short_name,
                    injury.to_lowercase()
                ),
            );
        }
        Ok(())
    }

    /// Depois de um jogo de clubes: os convocados que estavam fora não contam como "não jogou".
    pub(super) fn excuse_internationals(&mut self, clubs: [i32; 2]) {
        for id in self.away_internationals() {
            if self.squads.traits.get(&id).is_some_and(|t| clubs.contains(&t.club_id))
                && let Some(s) = self.squads.states.get_mut(&id)
            {
                s.missed = (s.missed - 1).max(0);
            }
        }
    }

    /// Uma competição de selecções acabou: o campeão no historial (no ano em que acabou) e o
    /// título no currículo dos convocados.
    pub(super) fn international_finished(&self, comp: i32) -> Result<()> {
        let year = self.current_season(comp);
        let Some((winner, second, third)) = self.podium(comp, year) else { return Ok(()) };
        let history_year: i32 = self.date[0..4].parse().unwrap_or(year + 1);
        self.world.conn.execute("DELETE FROM competition_history WHERE competition_id = ? AND year = ?", params![comp, history_year])?;
        self.world.conn.execute(
            "INSERT INTO competition_history (competition_id, year, winner_id, runner_up_id, third_id) VALUES (?, ?, ?, ?, ?)",
            params![comp, history_year, winner, second, third],
        )?;
        let mut honours = vec![(winner, "winner")];
        if let Some(s) = second {
            honours.push((s, "runner_up"));
        }
        for (team, kind) in honours {
            self.world.conn.execute(
                "INSERT INTO player_honours (person_id, date, club, competition, kind)
                 SELECT person_id, ?, ?, ?, ? FROM call_ups WHERE team_id = ?",
                params![self.date, team, comp, kind, team],
            )?;
        }
        Ok(())
    }
}

/// Uma competição de selecções qualquer, para a época internacional de uma data.
pub(super) const ANY_NATIONAL: i32 = 448;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::NewGame;
    use super::*;
    use crate::World;

    const BAYERN: i32 = 1350;
    const WCQ_EUROPE: i32 = 443;

    fn bayern() -> NewGame {
        NewGame {
            save_name: "Selecções".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: BAYERN,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        }
    }

    fn count(g: &Game, sql: &str) -> i32 {
        g.world.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn september_window_calls_up_plays_and_sends_back() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-test-int-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut g = Game::create(&world, &dir, &bayern(), 11).unwrap();
        let caps = "SELECT COALESCE(SUM(international_apps), 0) FROM people WHERE club_id = 1350";
        let caps_before = count(&g, caps);
        while g.date() < "2025-08-01" {
            g.advance_day().unwrap();
        }
        // 1/8: as eliminatórias da época do Mundial, sem o Mundial (sorteia-se em junho)
        assert_eq!(g.fixtures(WCQ_EUROPE).len(), 12 * 12);
        assert!(g.fixtures(448).is_empty());
        assert!(g.fixtures(455).is_empty(), "a Liga das Nações é nas outras épocas");

        // A convocatória sai uma semana antes do 1º jogo (6/9) e os convocados saem a 3/9
        while g.date() < "2025-08-31" {
            g.advance_day().unwrap();
        }
        let called = count(&g, "SELECT COUNT(*) FROM call_ups c JOIN people p ON p.id = c.person_id WHERE p.club_id = 1350");
        assert!(called >= 5, "o Bayern tem internacionais: {called}");
        assert!(g.away_internationals().is_empty(), "ainda no clube");
        assert!(g.news.iter().any(|n| n.title.contains("convoca") || n.title.starts_with("Convocatórias")));
        let teams = count(&g, "SELECT COUNT(DISTINCT team_id) FROM call_ups");
        let some_team = count(&g, "SELECT team_id FROM call_ups GROUP BY team_id HAVING COUNT(*) >= 18 ORDER BY team_id LIMIT 1");
        assert!(teams > 100, "{teams} selecções convocadas");
        assert!(count(&g, "SELECT MAX(n) FROM (SELECT COUNT(*) n FROM call_ups GROUP BY team_id)") <= 23);
        assert_eq!(
            count(&g, "SELECT COUNT(*) FROM call_ups c JOIN people p ON p.id = c.person_id WHERE p.national_team_id = c.team_id"),
            count(&g, "SELECT COUNT(*) FROM call_ups")
        );

        while g.date() < "2025-09-04" {
            g.advance_day().unwrap();
        }
        let away = g.away_internationals();
        assert!(!away.is_empty());
        // No plantel com "Int" e fora da equipa do clube
        let mut club = g.world.get_club(BAYERN).unwrap();
        g.fill_squad(&mut club).unwrap();
        let int: Vec<i32> = club.players.iter().filter(|p| p.info.as_deref() == Some("Int")).map(|p| p.id).collect();
        assert!(!int.is_empty() && int.iter().all(|id| away.contains(id)));
        let state = g.club_state(BAYERN, Some(16));
        assert!(int.iter().all(|id| state.get(id).is_some_and(|a| !a.can_play())));
        let t = g.world.auto_tactics(BAYERN, Some(&state), Default::default()).unwrap();
        assert!(t.starters.iter().filter_map(|s| s.player.as_ref()).all(|p| !int.contains(&p.id)));

        // Depois da janela (6 e 10/9): jogos feitos, classificação e todos de volta
        while g.date() < "2025-09-12" {
            g.advance_day().unwrap();
        }
        let played = g.fixtures(WCQ_EUROPE).iter().filter(|f| f.home_goals.is_some()).count();
        assert_eq!(played, 2 * 24);
        // Amigáveis para quem não joga as eliminatórias (os anfitriões do Mundial), um por data
        let friendlies: Vec<_> = g.fixtures(FRIENDLY).into_iter().filter(|f| f.date.as_str() <= "2025-09-11").collect();
        assert!(friendlies.len() >= 2 && friendlies.iter().all(|f| f.home_goals.is_some()));
        let usa = g.senior_team_of("United States").unwrap().unwrap();
        assert_eq!(friendlies.iter().filter(|f| f.home.id == usa || f.away.id == usa).count(), 2);
        let wcq: HashSet<i32> = g.fixtures(WCQ_EUROPE).iter().flat_map(|f| [f.home.id, f.away.id]).collect();
        assert!(
            friendlies.iter().all(|f| !wcq.contains(&f.home.id) && !wcq.contains(&f.away.id)),
            "quem joga as eliminatórias não faz amigáveis"
        );
        assert!(g.fixtures(FRIENDLY).len() > 100, "os amigáveis de março e junho");
        // O Historial: "Internacionais" na época do clube, fora dos Jogos Oficiais
        let capped: i32 =
            count(&g, "SELECT a.person_id FROM intl_apps a JOIN people p ON p.id = a.person_id WHERE p.club_id = 1350 LIMIT 1");
        let mut person = g.world.get_person(capped).unwrap();
        g.fill_record(&mut person).unwrap();
        let line = person.seasons[0].by_kind.iter().find(|(k, _)| k == "Internacionais").map(|(_, l)| l.apps);
        assert!(line.is_some_and(|n| n >= 1), "{:?}", person.seasons[0].by_kind);
        // A ficha da partida de um jogo das eliminatórias, com as selecções
        let fx = g.fixtures(WCQ_EUROPE).into_iter().find(|f| f.home_goals.is_some()).unwrap();
        let sheet = g.match_sheet(fx.id).unwrap();
        assert!(sheet.home.len() >= 11 && sheet.away.len() >= 11, "{} e {}", sheet.home.len(), sheet.away.len());
        assert_eq!(sheet.home.iter().chain(&sheet.away).filter(|l| l.mom).count(), 1);
        assert!(sheet.home.iter().map(|l| l.goals).sum::<i32>() <= fx.home_goals.unwrap() as i32, "golos na ficha");
        // Os internacionais do clube em campo
        assert!(g.news.iter().any(|n| n.body.starts_with("Jogaram hoje pelas selecções")));
        let stages = g.competition_stages(WCQ_EUROPE);
        assert_eq!(stages[0].tables.len(), 12);
        assert!(stages[0].tables.iter().all(|t| t.rows.iter().all(|r| r.played == 2)));
        assert_eq!(count(&g, "SELECT COUNT(*) FROM call_ups"), 0, "a janela acabou");
        // O plantel das selecções fica com a última convocatória
        let last = g.world.get_club(some_team).unwrap();
        assert!(last.players.len() >= 18, "fora da janela: {}", last.players.len());
        assert!(count(&g, caps) > caps_before, "internacionalizações somadas");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A época 2025/26 até ao fim do Mundial (demora: `cargo test --release -- --ignored mundial`).
    #[test]
    #[ignore]
    fn mundial_2026() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-test-wc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut g = Game::create(&world, &dir, &bayern(), 5).unwrap();
        while g.date() < "2026-06-11" {
            g.advance_day().unwrap();
        }
        // Sorteado a 10/6 com os 48 apurados
        let wc = g.fixtures(448);
        let mut teams: Vec<i32> = wc.iter().flat_map(|f| [f.home.id, f.away.id]).collect();
        teams.sort_unstable();
        teams.dedup();
        assert_eq!(teams.len(), 48);
        assert!(g.news.iter().any(|n| n.title.starts_with("Sorteio do Mundial")));
        while g.date() < "2026-07-20" {
            g.advance_day().unwrap();
        }
        let history: (i32, i32) = g
            .world
            .conn
            .query_row("SELECT winner_id, year FROM competition_history WHERE competition_id = 448 ORDER BY year DESC LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(history.1, 2026);
        assert!(teams.contains(&history.0));
        // Amigáveis antes do Mundial sem chocar com ele
        let june: Vec<_> = g.fixtures(FRIENDLY).into_iter().filter(|f| f.date.as_str() >= "2026-06-01").collect();
        assert!(!june.is_empty());
        let stages = g.competition_stages(448);
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[1].rounds.len(), 5, "dos dezasseisavos à final");
        assert!(count(&g, "SELECT COUNT(*) FROM player_honours WHERE competition = 448 AND kind = 'winner'") >= 18);
        assert_eq!(count(&g, "SELECT COUNT(*) FROM call_ups"), 0);
        for st in &stages {
            println!("{}", st.name);
        }
        println!("campeão do mundo: {}", g.club(history.0).name);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hosts_known_then_drawn_years_before() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-test-hosts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut g = Game::create(&world, &dir, &bayern(), 3).unwrap();
        g.settle_hosts(2025).unwrap();
        let names = |g: &Game, comp: i32, season: i32| -> Vec<String> {
            g.hosts_of(comp, season).unwrap().into_iter().map(|t| g.world.club_ref(Some(t)).unwrap().unwrap().name).collect()
        };
        assert_eq!(g.hosts_of(448, 2025).unwrap().len(), 3);
        assert_eq!(g.hosts_of(448, 2029).unwrap().len(), 3, "{:?}", names(&g, 448, 2029));
        assert_eq!(g.hosts_of(448, 2033).unwrap().len(), 1);
        assert_eq!(g.hosts_of(450, 2027).unwrap().len(), 4);
        assert!(g.hosts_of(451, 2027).unwrap().is_empty(), "a Copa América de 2028 escolhe-se em abril de 2026");

        // Os anúncios: a Copa América de 2028 em abril de 2026, com notícia
        let (mut fresh, mut next) = (Vec::new(), 1000);
        g.date = "2026-04-15".into();
        g.announce_hosts(&mut fresh, &mut next).unwrap();
        let copa = g.hosts_of(451, 2027).unwrap();
        assert!(!copa.is_empty() && copa.iter().all(|&t| g.team_continent(t).unwrap() == Some(5)));
        assert!(fresh.iter().any(|n| n.title.contains("Copa América 2028")), "{:?}", fresh.iter().map(|n| &n.title).collect::<Vec<_>>());
        // O Mundial de 2038 em dezembro de 2029, longe dos continentes de 2030 e 2034
        g.date = "2029-12-11".into();
        g.announce_hosts(&mut fresh, &mut next).unwrap();
        let wc = g.hosts_of(448, 2037).unwrap();
        assert!(!wc.is_empty());
        assert!(wc.iter().all(|&t| matches!(g.team_continent(t).unwrap(), Some(3 | 5))), "{:?}", names(&g, 448, 2037));
        assert!(fresh.iter().any(|n| n.title.contains("Mundial 2038")));
        for n in &fresh {
            println!("{}: {}", n.title, n.body);
        }
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn retired_from_the_national_team_is_not_called() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-test-intret-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let g = Game::create(&world, &dir, &bayern(), 3).unwrap();
        // O Courtois, o de mais CA da Bélgica, retirou-se da selecção no player_setup.cfg
        const COURTOIS: i32 = 24472;
        assert!(super::super::player_setup::intl_retired().contains(&COURTOIS));
        assert!(super::super::player_setup::intl_retired().len() >= 75);
        let belgium = g.senior_team_of("Belgium").unwrap().unwrap();
        let squad = g.choose_call_up(belgium, "2025-09-06").unwrap();
        assert_eq!(squad.len(), SQUAD_SIZE);
        assert!(!squad.contains(&COURTOIS));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn squad_quotas_then_the_best() {
        // 10 GR, 10 defesas, 10 médios e 2 avançados, por CA
        let mut c: Vec<(i32, usize)> = Vec::new();
        for i in 0..32 {
            c.push((i, if i < 2 { 3 } else { (i as usize) % 3 }));
        }
        let squad = pick_squad(&c);
        assert_eq!(squad.len(), SQUAD_SIZE);
        let count = |s: usize| squad.iter().filter(|&&id| c[id as usize].1 == s).count();
        assert_eq!(count(0), 3);
        assert_eq!(count(3), 2, "só havia dois avançados");
        assert_eq!(count(1) + count(2), 18, "os lugares que sobram vão para os melhores");
        assert_eq!(sector(18, 1, 1, 1), 0);
        assert_eq!(sector(1, 15, 18, 5), 2);
        assert_eq!(sector(1, 5, 5, 20), 3);
    }
}
