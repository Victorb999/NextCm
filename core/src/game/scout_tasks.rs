//! Olheiros com tarefas (marco 3.10, passo 2): "Designar Olheiro" em Pesquisa Jogador & Staff.
//! Cada olheiro do staff tem uma aba (Olheiro #1 a #7) e uma tarefa (um clube, o próximo
//! adversário, jogadores jovens, um país ou uma região); todos os dias observa alguns jogadores
//! do alvo, que passam a conhecidos e entram na lista com a recomendação em estrelas.
//! Regras e números no detalhe do 3.10 no ROADMAP.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::Game;
use super::scouting::{KNOWN_DAYS, SCOUT_JOB};
use crate::person::ClubRef;
use crate::search::position_label;
use crate::{Error, Result, date};

/// Abas de olheiros na pesquisa (o CM tem 7).
pub const SCOUT_SLOTS: usize = 7;
/// Jogadores na lista de cada olheiro, no máximo.
const MAX_FINDS: i64 = 300;
/// Pela pool larga (jovens, país, região): só os de nível (CA até isto abaixo do plantel) ou
/// os jovens (até 21) com potencial para o plantel.
const LEVEL_BELOW: f64 = 25.0;
const YOUTH_AGE: i32 = 21;

/// Uma linha da lista de um olheiro.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ScoutFind {
    pub id: i32,
    pub list_name: String,
    pub club: Option<ClubRef>,
    /// Onde joga: a divisão do clube (ou a nação, sem clube).
    pub base: Option<String>,
    pub position: String,
    #[ts(type = "number | null")]
    pub value: Option<i64>,
    /// Recomendação do olheiro, 1 a 5 estrelas.
    pub stars: i32,
    pub injured: bool,
}

/// Uma aba "Olheiro #N".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ScoutList {
    pub slot: i32,
    /// Quantos olheiros o clube tem (as abas a mais ficam apagadas).
    pub scouts: i32,
    pub scout: Option<String>,
    /// "Olheiro X indicado para espiar próximo adversário".
    pub task: Option<String>,
    pub finds: Vec<ScoutFind>,
}

/// Uma tarefa: "club", "opponent", "youth", "nation" ou "region", e o alvo (clube, nação ou
/// continente; o adversário é o do próximo jogo).
struct Task {
    slot: i32,
    scout: i32,
    kind: String,
    target: Option<i32>,
}

fn region_name(id: i32) -> &'static str {
    match id {
        0 => "África",
        1 => "Ásia",
        2 => "Europa",
        3 => "América do Norte",
        4 => "Oceânia",
        _ => "América do Sul",
    }
}

impl Game {
    /// Os olheiros do clube, pela ordem das abas (o melhor primeiro).
    fn scouts(&self) -> Result<Vec<(i32, String, i32, i32)>> {
        Ok(self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, p.display_name, COALESCE(np.judgement, 10), COALESCE(np.judging_potential, 10)
                 FROM people p LEFT JOIN non_players np ON np.person_id = p.id
                 WHERE p.club_id = ? AND p.club_job = ?
                 ORDER BY COALESCE(np.judgement, 10) + COALESCE(np.judging_potential, 10) DESC, p.id",
            )?
            .query_map(params![self.club_id, SCOUT_JOB], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .take(SCOUT_SLOTS)
            .collect::<rusqlite::Result<_>>()?)
    }

    fn task(&self, slot: i32) -> Result<Option<Task>> {
        Ok(self
            .world
            .conn
            .query_row("SELECT slot, scout_id, kind, target FROM scout_tasks WHERE slot = ?", [slot], |r| {
                Ok(Task { slot: r.get(0)?, scout: r.get(1)?, kind: r.get(2)?, target: r.get(3)? })
            })
            .optional()?)
    }

    /// O adversário do próximo jogo do clube do treinador.
    fn next_opponent(&self) -> Option<i32> {
        self.matches
            .iter()
            .filter(|m| {
                m.result.is_none() && m.date.as_str() >= self.date.as_str() && (m.home_id == self.club_id || m.away_id == self.club_id)
            })
            .min_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)))
            .map(|m| if m.home_id == self.club_id { m.away_id } else { m.home_id })
    }

    fn task_text(&self, scout: &str, t: &Task) -> Result<String> {
        let target = t.target.unwrap_or(0);
        Ok(match t.kind.as_str() {
            "club" => format!("Olheiro {scout} indicado para observar o {}", self.club(target).short_name),
            "opponent" => format!("Olheiro {scout} indicado para espiar próximo adversário"),
            "youth" => format!("Olheiro {scout} indicado para procurar jogadores jovens"),
            "nation" => {
                let n: String = self.world.conn.query_row("SELECT name FROM nations WHERE id = ?", [target], |r| r.get(0))?;
                format!("Olheiro {scout} indicado para observar jogadores em {n}")
            }
            _ => format!("Olheiro {scout} indicado para observar jogadores na {}", region_name(target)),
        })
    }

    /// A aba "Olheiro #N": o olheiro, a tarefa e os jogadores encontrados (melhores primeiro).
    pub fn scout_list(&self, slot: i32) -> Result<ScoutList> {
        let scouts = self.scouts()?;
        let scout = scouts.get((slot - 1).max(0) as usize).map(|s| s.1.clone());
        let task = match (self.task(slot)?, &scout) {
            (Some(t), Some(name)) => Some(self.task_text(name, &t)?),
            _ => None,
        };
        type Row = (i32, String, Option<i32>, Option<String>, Option<String>, i32, [Option<i32>; 12]);
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, p.list_name, p.club_id, COALESCE(k.short_name, k.name), n.name, f.stars,
                        pl.pos_goalkeeper, pl.pos_sweeper, pl.pos_defender, pl.pos_defensive_midfielder,
                        pl.pos_midfielder, pl.pos_attacking_midfielder, pl.pos_attacker, pl.pos_wing_back,
                        pl.pos_right_side, pl.pos_left_side, pl.pos_central, pl.pos_free_role
                 FROM scout_finds f JOIN people p ON p.id = f.person_id JOIN players pl ON pl.person_id = p.id
                 LEFT JOIN clubs c ON c.id = p.club_id LEFT JOIN competitions k ON k.id = c.division_id
                 LEFT JOIN nations n ON n.id = p.nation_id
                 WHERE f.slot = ? ORDER BY f.stars DESC, pl.current_ability DESC, p.id",
            )?
            .query_map([slot], |r| {
                let mut pos = [None; 12];
                for (i, v) in pos.iter_mut().enumerate() {
                    *v = r.get(6 + i)?;
                }
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, pos))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut finds = Vec::with_capacity(rows.len());
        for (id, list_name, club, division, nation, stars, pos) in rows {
            finds.push(ScoutFind {
                id,
                list_name,
                club: club.map(|c| self.club(c)),
                base: if club.is_some() { division } else { nation },
                position: position_label(&pos),
                value: self.world.market_value_of(id, &self.date)?,
                stars,
                injured: self.player_status(id).is_some_and(|s| s.injury.is_some()),
            });
        }
        Ok(ScoutList { slot, scouts: scouts.len() as i32, scout, task, finds })
    }

    /// "Designar Olheiro": a tarefa nova apaga a lista antiga.
    pub fn assign_scout(&mut self, slot: i32, kind: &str, target: Option<i32>) -> Result<ScoutList> {
        let scouts = self.scouts()?;
        let Some(scout) = scouts.get((slot - 1).max(0) as usize).filter(|_| slot >= 1) else {
            return Err(Error::Invalid("O clube não tem um olheiro para esta aba.".into()));
        };
        let target = match kind {
            "club" | "nation" | "region" => Some(target.ok_or_else(|| Error::Invalid("Escolha o alvo do olheiro.".into()))?),
            "opponent" => self.next_opponent(),
            "youth" => None,
            _ => return Err(Error::Invalid("Tarefa de olheiro desconhecida.".into())),
        };
        if kind == "club" && target == Some(self.club_id) {
            return Err(Error::Invalid("Já conhece os jogadores do seu plantel.".into()));
        }
        self.world.conn.execute("DELETE FROM scout_finds WHERE slot = ?", [slot])?;
        self.world.conn.execute(
            "INSERT OR REPLACE INTO scout_tasks (slot, scout_id, kind, target, started) VALUES (?, ?, ?, ?, ?)",
            params![slot, scout.0, kind, target, self.date],
        )?;
        self.unsaved = true;
        self.scout_list(slot)
    }

    /// "Apagar": o olheiro fica sem tarefa e a lista vazia.
    pub fn clear_scout(&mut self, slot: i32) -> Result<ScoutList> {
        self.world.conn.execute("DELETE FROM scout_finds WHERE slot = ?", [slot])?;
        self.world.conn.execute("DELETE FROM scout_tasks WHERE slot = ?", [slot])?;
        self.unsaved = true;
        self.scout_list(slot)
    }

    /// Cada olheiro com tarefa observa alguns jogadores do alvo: de 2 (Avaliar Jogadores 1) a
    /// 6 (20) por dia; o próximo adversário, 6 por dia.
    pub(super) fn scout_tasks_day(&mut self) -> Result<()> {
        let tasks: Vec<Task> = self
            .world
            .conn
            .prepare_cached("SELECT slot, scout_id, kind, target FROM scout_tasks ORDER BY slot")?
            .query_map([], |r| Ok(Task { slot: r.get(0)?, scout: r.get(1)?, kind: r.get(2)?, target: r.get(3)? }))?
            .collect::<rusqlite::Result<_>>()?;
        if tasks.is_empty() {
            return Ok(());
        }
        let level = self.squad_level()?;
        let until = date::add_days(&self.date, KNOWN_DAYS).unwrap_or_default();
        // Nascidos depois disto têm até 21 anos
        let youth_born = format!("{}{}", self.date[0..4].parse::<i32>().unwrap_or(2025) - YOUTH_AGE - 1, &self.date[4..]);
        let my_continent: i32 = self.world.conn.query_row(
            "SELECT COALESCE(n.continent_id, -1) FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id WHERE c.id = ?",
            [self.club_id],
            |r| r.get(0),
        )?;
        for mut t in tasks {
            // O olheiro saiu do clube: a tarefa acaba
            let Some((judgement, potential)) = self
                .world
                .conn
                .query_row(
                    "SELECT COALESCE(np.judgement, 10), COALESCE(np.judging_potential, 10) FROM people p
                     LEFT JOIN non_players np ON np.person_id = p.id WHERE p.id = ? AND p.club_id = ?",
                    params![t.scout, self.club_id],
                    |r| Ok((r.get::<_, i32>(0)?, r.get::<_, i32>(1)?)),
                )
                .optional()?
            else {
                self.world.conn.execute("DELETE FROM scout_tasks WHERE slot = ?", [t.slot])?;
                continue;
            };
            // O próximo adversário mudou: lista nova
            if t.kind == "opponent" {
                let now = self.next_opponent();
                if now != t.target {
                    self.world.conn.execute("DELETE FROM scout_finds WHERE slot = ?", [t.slot])?;
                    self.world.conn.execute("UPDATE scout_tasks SET target = ? WHERE slot = ?", params![now, t.slot])?;
                    t.target = now;
                }
            }
            let found: i64 = self.world.conn.query_row("SELECT COUNT(*) FROM scout_finds WHERE slot = ?", [t.slot], |r| r.get(0))?;
            if found >= MAX_FINDS {
                continue;
            }
            let per_day = if t.kind == "opponent" { 6 } else { 2 + (judgement.clamp(1, 20) - 1) * 4 / 19 };
            let target = t.target.unwrap_or(-1);
            let wide = format!(
                "(pl.current_ability >= {} OR (p.birth_sort > '{youth_born}' AND pl.potential_ability >= {}))",
                (level - LEVEL_BELOW) as i32,
                level as i32
            );
            let filter = match t.kind.as_str() {
                "club" | "opponent" => format!("p.club_id = {target}"),
                "youth" => format!(
                    "p.birth_sort > '{youth_born}' AND n.continent_id = {my_continent} AND pl.potential_ability >= {}",
                    (level - 10.0) as i32
                ),
                "nation" => format!("c.nation_id = {target} AND {wide}"),
                _ => format!("n.continent_id = {target} AND {wide}"),
            };
            // Uma ordem fixa por olheiro e jogador, sem os já encontrados nem os do plantel
            let sql = format!(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id
                 LEFT JOIN clubs c ON c.id = p.club_id LEFT JOIN nations n ON n.id = COALESCE(c.nation_id, p.nation_id)
                 WHERE {filter} AND p.is_player = 1 AND COALESCE(p.club_id, 0) <> {mine}
                   AND p.id NOT IN (SELECT person_id FROM scout_finds WHERE slot = {slot})
                   AND p.id NOT IN (SELECT person_id FROM retired)
                 ORDER BY (p.id * 2654435761 + {salt}) % 1000003 LIMIT {per_day}",
                mine = self.club_id,
                slot = t.slot,
                salt = (self.seed as u64 % 1_000_003) + t.slot as u64 * 7919,
            );
            let ids: Vec<i32> = self.world.conn.prepare(&sql)?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            for id in ids {
                let (stars, age, room) = self.judge(id, judgement, potential, level)?;
                // Nos jovens conta também o que podem vir a ser
                let stars = if age <= YOUTH_AGE && room >= 15.0 { (stars + 1).min(5) } else { stars };
                self.world.conn.execute(
                    "INSERT OR IGNORE INTO scout_finds (slot, person_id, found, stars) VALUES (?, ?, ?, ?)",
                    params![t.slot, id, self.date, stars as i32],
                )?;
                self.world.conn.execute(
                    "INSERT INTO knowledge (person_id, until) VALUES (?1, ?2) ON CONFLICT (person_id) DO UPDATE SET until = MAX(until, ?2)",
                    params![id, until],
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;
    const FLAMENGO: i32 = 3675;

    #[test]
    fn scouts_fill_their_lists() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-tarefas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Tarefas".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
        };
        let mut g = Game::create(&path, &dir, &new, 11).unwrap();
        let list = g.scout_list(1).unwrap();
        assert!(list.scouts >= 2 && list.scout.is_some() && list.task.is_none() && list.finds.is_empty());
        assert!(g.assign_scout(1, "club", Some(PALMEIRAS)).is_err());
        assert!(g.assign_scout(1, "club", None).is_err());
        let list = g.assign_scout(1, "club", Some(FLAMENGO)).unwrap();
        assert!(list.task.unwrap().contains("Flamengo"));
        g.assign_scout(2, "youth", None).unwrap();
        let brazil: i32 = g.world.conn.query_row("SELECT id FROM nations WHERE name = 'Brazil'", [], |r| r.get(0)).unwrap();
        g.assign_scout(3, "nation", Some(brazil)).unwrap();
        for _ in 0..10 {
            g.scout_tasks_day().unwrap();
            g.date = date::add_days(&g.date, 1).unwrap();
        }
        let flamengo = g.scout_list(1).unwrap();
        assert!(flamengo.finds.len() >= 20, "{}", flamengo.finds.len());
        assert!(flamengo.finds.iter().all(|f| f.club.as_ref().is_some_and(|c| c.id == FLAMENGO) && (1..=5).contains(&f.stars)));
        assert_eq!(g.knowledge(flamengo.finds[0].id).unwrap(), crate::person::Knowledge::Known);
        let youth = g.scout_list(2).unwrap();
        assert!(!youth.finds.is_empty());
        assert!(!g.scout_list(3).unwrap().finds.is_empty());
        // Apagar
        assert!(g.clear_scout(1).unwrap().finds.is_empty());
        assert!(g.scout_list(1).unwrap().task.is_none());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
