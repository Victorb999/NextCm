//! Treino (marco 3.17): os programas do treinador ("<Clube> - Treinos" do CM), cada um com a
//! intensidade de Prep. Física, Táctica, Remate, Técnica e G-Redes e, se quiser, uma posição
//! ou um lado novos. O programa de cada jogador mexe na evolução mensal do CA (com a
//! qualidade dos treinadores), na condição de cada dia, no risco de lesão no treino, em
//! aprender posições e no motivo "Infeliz com o treino". Números em
//! docs/roadmap/3.17-treino.md.

use std::collections::HashMap;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Game, NewsItem, age_on};
use crate::club::SquadPlayer;
use crate::engine::uniform;
use crate::person::ClubRef;
use crate::{Error, Result};

/// Os programas do patch CLZ (save 33): por posição, mais Nenhum, Geral e BASE (os jovens).
pub(super) const TRAINING_SCHEMA: &str = "
CREATE TABLE training_programs (id INTEGER PRIMARY KEY, name TEXT NOT NULL, fitness INTEGER NOT NULL,
                                tactics INTEGER NOT NULL, shooting INTEGER NOT NULL, technique INTEGER NOT NULL,
                                goalkeeping INTEGER NOT NULL, new_position TEXT, new_side TEXT);
INSERT INTO training_programs (id, name, fitness, tactics, shooting, technique, goalkeeping) VALUES
    (1, 'Nenhum', 0, 0, 0, 0, 0), (2, 'Geral', 2, 2, 2, 2, 0), (3, 'GK', 2, 1, 0, 1, 3), (4, 'LAT', 2, 2, 1, 2, 0),
    (5, 'DC', 2, 3, 0, 1, 0), (6, 'VOL', 2, 3, 1, 2, 0), (7, 'MEIAS', 2, 2, 2, 3, 0), (8, 'ATACANTES', 2, 1, 3, 2, 0),
    (9, 'BASE', 2, 2, 2, 3, 0);
CREATE TABLE training_players (person_id INTEGER PRIMARY KEY, program_id INTEGER NOT NULL);
";

/// As posições e os lados que se podem treinar, com a coluna de `players`.
const POSITIONS: [(&str, &str); 8] = [
    ("GR", "pos_goalkeeper"),
    ("L", "pos_sweeper"),
    ("D", "pos_defender"),
    ("AL", "pos_wing_back"),
    ("MD", "pos_defensive_midfielder"),
    ("M", "pos_midfielder"),
    ("MA", "pos_attacking_midfielder"),
    ("A", "pos_attacker"),
];
const SIDES: [(&str, &str); 3] = [("E", "pos_left_side"), ("C", "pos_central"), ("D", "pos_right_side")];

/// Idade até à qual um jogador vai para a BASE.
const BASE_AGE: i32 = 19;
/// Condição por dia de treino, pela intensidade da Prep. Física.
const CONDITION_PER_DAY: [i32; 4] = [1, 1, 0, -1];
/// Probabilidade por dia de uma lesão no treino (com Propensão a Lesões 10).
const INJURY_PER_DAY: [f64; 4] = [0.0, 0.0, 0.000_05, 0.000_4];
/// Pontos por mês ao aprender uma posição (Versatilidade 10, até aos 23 anos).
const LEARN_PER_MONTH: f64 = 1.8;
/// Atributos de cada área (Prep. Física, Táctica, Remate, Técnica, G-Redes), colunas de `players`.
/// Os relativos (remate, passe...) mudam o perfil do jogador; os outros, o valor.
const AREAS: [&[&str]; 5] = [
    &["pace", "acceleration", "stamina", "strength", "agility", "balance", "jumping"],
    &["positioning", "anticipation", "decisions", "marking", "off_the_ball", "teamwork", "work_rate"],
    &["finishing", "long_shots", "penalties"],
    &["technique", "dribbling", "passing", "crossing", "free_kicks"],
    &["handling", "reflexes", "one_on_ones"],
];
/// Peso de cada intensidade (Nenhum, Ligeiro, Médio, Intensivo) no ganho por área.
const AREA_WEIGHT: [f64; 4] = [0.0, 0.4, 1.0, 1.6];
/// Probabilidade por mês de +1 num atributo de uma área Média, num jovem com bons treinadores.
const AREA_GAIN: f64 = 0.15;
/// Os clubes da IA treinam como o "Geral" do patch (os guarda-redes como o "GK").
const AI_FIELD: [i32; 5] = [2, 2, 2, 2, 0];
const AI_KEEPER: [i32; 5] = [2, 1, 0, 1, 3];

/// Ambição a partir da qual um jogador sem treino fica infeliz, e o Profissionalismo até ao
/// qual o treino físico intensivo o deixa infeliz.
const BORED_AMBITION: i32 = 14;
const LAZY_PROFESSIONALISM: i32 = 7;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TrainingProgram {
    /// 0 num programa novo (o core dá o id ao gravar).
    pub id: i32,
    pub name: String,
    /// Intensidades 0–3 (Nenhum, Ligeiro, Médio, Intensivo).
    pub fitness: i32,
    pub tactics: i32,
    pub shooting: i32,
    pub technique: i32,
    pub goalkeeping: i32,
    /// Nova Posição ("GR", "L", "D", "AL", "MD", "M", "MA", "A") e Novo Lado ("E", "C", "D").
    pub new_position: Option<String>,
    pub new_side: Option<String>,
}

impl TrainingProgram {
    /// A carga do programa (0–1) nas áreas que contam para o jogador.
    fn load(&self, keeper: bool) -> f64 {
        let areas = if keeper {
            [self.fitness, self.tactics, self.goalkeeping].iter().sum::<i32>() as f64 / 3.0
        } else {
            [self.fitness, self.tactics, self.shooting, self.technique].iter().sum::<i32>() as f64 / 4.0
        };
        areas / 3.0
    }

    fn idle(&self) -> bool {
        self.fitness + self.tactics + self.shooting + self.technique + self.goalkeeping == 0
    }
}

/// O programa de um jogador na tela (sem o que o plantel já traz).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ProgramAssign {
    pub person: i32,
    pub program: i32,
}

/// Os programas e quem treina em cada um; o Ok da tela grava tudo de uma vez.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TrainingPlan {
    pub programs: Vec<TrainingProgram>,
    pub players: Vec<ProgramAssign>,
}

/// Um treinador do clube (o botão Treinadores).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Coach {
    pub id: i32,
    pub name: String,
    pub job: String,
    /// O que treina: "Prep. Física", "G-Redes" ou "Táctica e Técnica".
    pub works: String,
    /// Qualidade a treinar ("Excelente" … "Fraco").
    pub quality: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TrainingView {
    pub club: ClubRef,
    pub programs: Vec<TrainingProgram>,
    pub players: Vec<SquadPlayer>,
    pub assign: Vec<ProgramAssign>,
    pub coaches: Vec<Coach>,
    /// O Vêr Atributos: a média (1–20) de cada área de treino de cada jogador.
    pub areas: Vec<PlayerAreas>,
}

/// A média dos atributos de cada área (Prep. Física, Táctica, Remate, Técnica, G-Redes), como
/// o perfil os mostra; `None` numa área sem atributos.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PlayerAreas {
    pub person: i32,
    pub areas: [Option<i32>; 5],
}

fn quality_label(v: i32) -> &'static str {
    match v {
        17.. => "Excelente",
        14..=16 => "Muito Bom",
        11..=13 => "Bom",
        8..=10 => "Razoável",
        _ => "Fraco",
    }
}

fn column(list: &[(&str, &'static str)], code: Option<&str>) -> Option<&'static str> {
    code.and_then(|c| list.iter().find(|(k, _)| *k == c).map(|(_, col)| *col))
}

/// O programa do patch para um jogador pela idade e pela posição mais defensiva (nome do
/// programa), como a ordem do plantel.
fn default_program_name(pos: &[i32; 12], age: i32) -> &'static str {
    let natural = |i: usize| pos[i] >= crate::search::NATURAL_POSITION;
    if natural(0) {
        "GK"
    } else if age <= BASE_AGE {
        "BASE"
    } else if natural(1) || natural(2) {
        if natural(7) || !natural(10) { "LAT" } else { "DC" }
    } else if natural(7) {
        "LAT"
    } else if natural(3) {
        "VOL"
    } else if natural(4) || natural(5) {
        "MEIAS"
    } else if natural(6) {
        "ATACANTES"
    } else {
        "Geral"
    }
}

/// Efeito do treino num jogador do treinador, para a evolução do mês.
pub(super) struct Effect {
    /// Multiplica a evolução para cima (0,6 sem treino a ~1,4 intensivo com bons treinadores).
    pub growth: f64,
    /// Multiplica o declínio dos veteranos (a Prep. Física abranda-o).
    pub decline: f64,
}

impl Game {
    pub fn training_programs(&self) -> Result<Vec<TrainingProgram>> {
        Ok(self
            .world
            .conn
            .prepare_cached(
                "SELECT id, name, fitness, tactics, shooting, technique, goalkeeping, new_position, new_side
                 FROM training_programs ORDER BY id",
            )?
            .query_map([], |r| {
                Ok(TrainingProgram {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    fitness: r.get(2)?,
                    tactics: r.get(3)?,
                    shooting: r.get(4)?,
                    technique: r.get(5)?,
                    goalkeeping: r.get(6)?,
                    new_position: r.get(7)?,
                    new_side: r.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// O programa de cada jogador do plantel: o escolhido, senão o do patch pela posição.
    fn assignments(&self, programs: &[TrainingProgram]) -> Result<HashMap<i32, i32>> {
        type Row = (i32, Option<String>, Option<i32>, [Option<i32>; 12], Option<i32>);
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, p.birth_date, p.birth_year,
                        pl.pos_goalkeeper, pl.pos_sweeper, pl.pos_defender, pl.pos_defensive_midfielder,
                        pl.pos_midfielder, pl.pos_attacking_midfielder, pl.pos_attacker, pl.pos_wing_back,
                        pl.pos_right_side, pl.pos_left_side, pl.pos_central, pl.pos_free_role,
                        (SELECT program_id FROM training_players t WHERE t.person_id = p.id)
                 FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1",
            )?
            .query_map([self.club_id], |r| {
                let mut pos = [None; 12];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = r.get(3 + i)?;
                }
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, pos, r.get(15)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let first = programs.first().map_or(0, |p| p.id);
        let mut out = HashMap::new();
        for (id, birth, birth_year, pos, chosen) in rows {
            let program = match chosen.filter(|c| programs.iter().any(|p| p.id == *c)) {
                Some(c) => c,
                None => {
                    let age = age_on(birth.as_deref(), birth_year, &self.date);
                    let name = default_program_name(&pos.map(|v| v.unwrap_or(0)), age);
                    programs.iter().find(|p| p.name == name).map_or(first, |p| p.id)
                }
            };
            out.insert(id, program);
        }
        Ok(out)
    }

    /// O programa de um jogador do plantel do treinador.
    fn program_of(&self, person: i32) -> Result<Option<TrainingProgram>> {
        let programs = self.training_programs()?;
        let assign = self.assignments(&programs)?;
        Ok(assign.get(&person).and_then(|id| programs.iter().find(|p| p.id == *id)).cloned())
    }

    /// Os treinadores do clube do treinador, os melhores primeiro.
    fn coaches_of(&self) -> Result<Vec<(Coach, i32, i32)>> {
        let rows: Vec<(i32, String, i32, String, i32, i32)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, p.display_name, j.id, j.name, COALESCE(np.coaching, 1), COALESCE(np.coaching_gks, 1)
                 FROM people p JOIN jobs j ON j.id = p.club_job JOIN non_players np ON np.person_id = p.id
                 WHERE p.club_id = ? AND j.id IN (5, 6, 7, 8, 12, 13) ORDER BY np.coaching DESC, p.id",
            )?
            .query_map([self.club_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .map(|(id, name, job_id, job, coaching, gks)| {
                let works = if matches!(job_id, 8 | 13) {
                    "Prep. Física"
                } else if gks > coaching {
                    "G-Redes"
                } else {
                    "Táctica e Técnica"
                };
                let quality = quality_label(if works == "G-Redes" { gks } else { coaching });
                (Coach { id, name, job, works: works.into(), quality: quality.into() }, coaching, gks)
            })
            .collect())
    }

    /// Qualidade (0–1) dos treinadores: a média dos três melhores a treinar (e o melhor de
    /// guarda-redes); sem treinadores, 0,5.
    fn coach_quality(&self) -> Result<(f64, f64)> {
        let coaches = self.coaches_of()?;
        let mut outfield: Vec<i32> = coaches.iter().map(|c| c.1).collect();
        outfield.sort_unstable_by(|a, b| b.cmp(a));
        let top: Vec<i32> = outfield.into_iter().take(3).collect();
        let field = if top.is_empty() { 0.5 } else { top.iter().sum::<i32>() as f64 / top.len() as f64 / 20.0 };
        let keepers = coaches.iter().map(|c| c.2).max().map_or(0.5, |v| v as f64 / 20.0);
        Ok((field, keepers))
    }

    /// Para a evolução do mês: o efeito do treino em cada jogador do plantel do treinador.
    pub(super) fn training_effects(&self) -> Result<HashMap<i32, Effect>> {
        if self.sacked() {
            return Ok(HashMap::new());
        }
        let programs = self.training_programs()?;
        let assign = self.assignments(&programs)?;
        let (field, keepers) = self.coach_quality()?;
        let mut out = HashMap::new();
        for (person, program) in assign {
            let Some(p) = programs.iter().find(|p| p.id == program) else { continue };
            let keeper: bool =
                self.world
                    .conn
                    .query_row("SELECT COALESCE(pos_goalkeeper, 0) >= 15 FROM players WHERE person_id = ?", [person], |r| r.get(0))?;
            let q = if keeper { keepers } else { field };
            out.insert(
                person,
                Effect { growth: 0.6 + 0.8 * p.load(keeper) * (0.5 + 0.5 * q), decline: 1.25 - 0.5 * p.fitness as f64 / 3.0 },
            );
        }
        Ok(out)
    }

    /// Dia 1 de cada mês, a evolução por área: cada área treinada pode subir um atributo dela
    /// (mais nos jovens que ainda não chegaram ao potencial, com bons treinadores e mais
    /// intensidade), e os veteranos perdem físico, mais depressa com pouca Prep. Física. O
    /// plantel do treinador segue os programas; os outros clubes das ligas vivas, o "Geral".
    pub(super) fn train_attributes(&mut self) -> Result<()> {
        let mine: HashMap<i32, [i32; 5]> = if self.sacked() {
            HashMap::new()
        } else {
            let programs = self.training_programs()?;
            self.assignments(&programs)?
                .into_iter()
                .filter_map(|(person, id)| {
                    let p = programs.iter().find(|p| p.id == id)?;
                    Some((person, [p.fitness, p.tactics, p.shooting, p.technique, p.goalkeeping]))
                })
                .collect()
        };
        let (field, keepers) = if self.sacked() { (0.5, 0.5) } else { self.coach_quality()? };
        let columns: Vec<&str> = AREAS.iter().flat_map(|a| a.iter().copied()).collect();
        let managed = self.managed_sql(None);
        let sql = format!(
            "SELECT p.id, p.club_id, p.birth_date, p.birth_year, pl.current_ability, pl.potential_ability,
                    COALESCE(pl.pos_goalkeeper, 0), {}
             FROM people p JOIN players pl ON pl.person_id = p.id JOIN clubs c ON c.id = p.club_id
             WHERE c.division_id IN ({managed}) AND p.is_player = 1 ORDER BY p.id",
            columns.iter().map(|c| format!("pl.{c}")).collect::<Vec<_>>().join(", ")
        );
        type Row = (i32, i32, Option<String>, Option<i32>, i32, i32, i32, Vec<Option<i32>>);
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare(&sql)?
            .query_map([], |r| {
                let values = (0..columns.len()).map(|i| r.get(7 + i)).collect::<rusqlite::Result<Vec<_>>>()?;
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get::<_, Option<i32>>(4)?.unwrap_or(1),
                    r.get::<_, Option<i32>>(5)?.unwrap_or(0),
                    r.get(6)?,
                    values,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut rng = self.season_rng(0xA7EA);
        let today = self.date.clone();
        let mut changes: Vec<(i32, &str, i32)> = Vec::new();
        for (id, club, birth, birth_year, ca, pa, gk, values) in rows {
            let keeper = gk >= 15;
            let own = club == self.club_id && !self.sacked();
            let load = match mine.get(&id) {
                Some(l) if own => *l,
                _ if keeper => AI_KEEPER,
                _ => AI_FIELD,
            };
            let quality = if own { if keeper { keepers } else { field } } else { 0.5 };
            let age = age_on(birth.as_deref(), birth_year, &today);
            let youth = match age {
                ..=21 => 1.0,
                22..=24 => 0.8,
                25..=28 => 0.5,
                29..=30 => 0.25,
                _ => 0.0,
            };
            let room = if pa <= 0 || ca < pa { 1.0 } else { 0.0 };
            let mut at = 0;
            for (area, cols) in AREAS.iter().enumerate() {
                let (from, to) = (at, at + cols.len());
                at = to;
                // G-Redes só nos guarda-redes; o remate não conta neles
                if (area == 4) != keeper && (area == 4 || area == 2) {
                    continue;
                }
                let weight = AREA_WEIGHT[load[area].clamp(0, 3) as usize];
                let chance = AREA_GAIN * weight * youth * room * (0.5 + 0.5 * quality);
                if chance > 0.0 && uniform(&mut rng) < chance {
                    let open: Vec<usize> = (from..to).filter(|&i| values[i].is_some_and(|v| v < 20)).collect();
                    if !open.is_empty() {
                        let i = open[(uniform(&mut rng) * open.len() as f64) as usize % open.len()];
                        changes.push((id, columns[i], 1));
                    }
                }
            }
            // Declínio físico dos veteranos (guarda-redes dois anos mais tarde)
            let old = age - if keeper { 2 } else { 0 };
            if old >= 30 {
                let chance = (0.10 + 0.04 * (old - 30) as f64) * (1.25 - 0.5 * load[0].clamp(0, 3) as f64 / 3.0);
                if uniform(&mut rng) < chance {
                    let open: Vec<usize> = (0..AREAS[0].len()).filter(|&i| values[i].is_some_and(|v| v > 1)).collect();
                    if !open.is_empty() {
                        let i = open[(uniform(&mut rng) * open.len() as f64) as usize % open.len()];
                        changes.push((id, columns[i], -1));
                    }
                }
            }
        }
        for (id, col, step) in changes {
            self.world.conn.execute(
                &format!("UPDATE players SET {col} = MAX(1, MIN(20, COALESCE({col}, 10) + ?)) WHERE person_id = ?"),
                params![step, id],
            )?;
        }
        self.unsaved = true;
        Ok(())
    }

    /// Dia 1 de cada mês: quem treina uma posição ou um lado novos ganha jeito para eles,
    /// mais depressa com Versatilidade e juventude.
    pub(super) fn learn_positions(&mut self) -> Result<()> {
        if self.sacked() {
            return Ok(());
        }
        let programs = self.training_programs()?;
        let assign = self.assignments(&programs)?;
        let mut rng = self.season_rng(0x1EA5);
        let mut people: Vec<(i32, i32)> = assign.into_iter().collect();
        people.sort_unstable();
        for (person, program) in people {
            let Some(p) = programs.iter().find(|p| p.id == program) else { continue };
            let targets: Vec<&str> =
                [column(&POSITIONS, p.new_position.as_deref()), column(&SIDES, p.new_side.as_deref())].into_iter().flatten().collect();
            if targets.is_empty() {
                continue;
            }
            let (versatility, birth, birth_year): (Option<i32>, Option<String>, Option<i32>) = self.world.conn.query_row(
                "SELECT pl.versatility, p.birth_date, p.birth_year FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.id = ?",
                [person],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let age = age_on(birth.as_deref(), birth_year, &self.date);
            let youth = match age {
                ..=23 => 1.0,
                24..=29 => 0.75,
                _ => 0.5,
            };
            let gain = LEARN_PER_MONTH * (0.5 + versatility.unwrap_or(10).clamp(1, 20) as f64 / 20.0) * youth;
            for col in targets {
                let step = gain.floor() as i32 + (uniform(&mut rng) < gain.fract()) as i32;
                self.world.conn.execute(
                    &format!("UPDATE players SET {col} = MIN(20, COALESCE({col}, 1) + ?) WHERE person_id = ?"),
                    params![step, person],
                )?;
            }
        }
        self.unsaved = true;
        Ok(())
    }

    /// Cada dia: a condição pelo treino físico e as lesões no treino.
    pub(super) fn training_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let programs = self.training_programs()?;
        let assign = self.assignments(&programs)?;
        let mut rng = self.season_rng(0x7EA1);
        let mut people: Vec<(i32, i32)> = assign.into_iter().collect();
        people.sort_unstable();
        for (person, program) in people {
            let Some(p) = programs.iter().find(|p| p.id == program) else { continue };
            let i = p.fitness.clamp(0, 3) as usize;
            self.squads.train(person, CONDITION_PER_DAY[i]);
            let proneness: i32 = self
                .world
                .conn
                .query_row("SELECT COALESCE(injury_proneness, 10) FROM players WHERE person_id = ?", [person], |r| r.get(0))
                .optional()?
                .unwrap_or(10);
            let roll = uniform(&mut rng);
            if roll >= INJURY_PER_DAY[i] * proneness.clamp(1, 20) as f64 / 10.0 {
                continue;
            }
            let today = self.date.clone();
            let Some((injury, days)) = self.squads.training_injury(person, &today, &mut rng) else { continue };
            self.world.conn.execute(
                "INSERT INTO injury_log (person_id, date, injury, kind, days) VALUES (?, ?, ?, 'Treino', ?)",
                params![person, today, injury, days],
            )?;
            let name = self.person_name(person)?;
            fresh.push(self.news_item(
                *next_id,
                "injury",
                format!("{name} lesionado no treino"),
                format!(
                    "{name} lesionou-se no treino: {}.\n\nO departamento médico prevê cerca de {days} dias de paragem.",
                    injury.to_lowercase()
                ),
            ));
            *next_id += 1;
        }
        Ok(())
    }

    /// "Infeliz com o treino": um ambicioso sem treino, ou um pouco profissional com Prep.
    /// Física intensiva.
    pub(super) fn training_grievance(&self, person: i32) -> Result<bool> {
        let Some(p) = self.program_of(person)? else { return Ok(false) };
        let (ambition, professionalism): (Option<i32>, Option<i32>) =
            self.world
                .conn
                .query_row("SELECT ambition, professionalism FROM people WHERE id = ?", [person], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok((p.idle() && ambition.unwrap_or(10) >= BORED_AMBITION)
            || (p.fitness >= 3 && professionalism.unwrap_or(10) <= LAZY_PROFESSIONALISM))
    }

    /// A tela "<Clube> - Treinos".
    pub fn training_view(&self) -> Result<TrainingView> {
        let mut club = self.world.get_club(self.club_id)?;
        self.fill_squad(&mut club)?;
        let programs = self.training_programs()?;
        let mut assign: Vec<ProgramAssign> =
            self.assignments(&programs)?.into_iter().map(|(person, program)| ProgramAssign { person, program }).collect();
        assign.sort_by_key(|a| a.person);
        let coaches = self.coaches_of()?.into_iter().map(|c| c.0).collect();
        let mut areas = Vec::with_capacity(club.players.len());
        for p in &club.players {
            let Some(pl) = self.world.get_person(p.id)?.player else { continue };
            let mean = |keys: &[&str]| {
                let v: Vec<i32> = keys.iter().filter_map(|k| pl.attributes.get(*k).copied().flatten()).collect();
                (!v.is_empty()).then(|| (v.iter().sum::<i32>() as f64 / v.len() as f64).round() as i32)
            };
            areas.push(PlayerAreas { person: p.id, areas: AREAS.map(mean) });
        }
        Ok(TrainingView { club: club.club, programs, players: club.players, assign, coaches, areas })
    }

    /// O Ok da tela: grava os programas (novos com id 0, os que faltam são apagados) e o
    /// programa de cada jogador.
    pub fn set_training(&mut self, plan: &TrainingPlan) -> Result<TrainingView> {
        if plan.programs.is_empty() {
            return Err(Error::Invalid("Tem de haver pelo menos um programa de treino.".into()));
        }
        for p in &plan.programs {
            if p.name.trim().is_empty() {
                return Err(Error::Invalid("Todos os programas precisam de um nome.".into()));
            }
            if [p.fitness, p.tactics, p.shooting, p.technique, p.goalkeeping].iter().any(|v| !(0..=3).contains(v)) {
                return Err(Error::Invalid("Intensidade de treino inválida.".into()));
            }
            if column(&POSITIONS, p.new_position.as_deref()).is_none() && p.new_position.is_some()
                || column(&SIDES, p.new_side.as_deref()).is_none() && p.new_side.is_some()
            {
                return Err(Error::Invalid("Posição ou lado a treinar inválidos.".into()));
            }
        }
        let squad: Vec<i32> = self.assignments(&self.training_programs()?)?.into_keys().collect();
        let conn = &self.world.conn;
        let keep: Vec<i32> = plan.programs.iter().map(|p| p.id).filter(|id| *id > 0).collect();
        let existing: Vec<i32> =
            conn.prepare("SELECT id FROM training_programs")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        for id in existing.iter().filter(|id| !keep.contains(id)) {
            conn.execute("DELETE FROM training_programs WHERE id = ?", [id])?;
            conn.execute("DELETE FROM training_players WHERE program_id = ?", [id])?;
        }
        // Os ids dos programas novos (0, -1, -2... na tela) passam a ids do banco
        let mut ids: HashMap<i32, i32> = HashMap::new();
        for p in &plan.programs {
            let values = params![p.name.trim(), p.fitness, p.tactics, p.shooting, p.technique, p.goalkeeping, p.new_position, p.new_side];
            if p.id > 0 && existing.contains(&p.id) {
                conn.execute(
                    "UPDATE training_programs SET name = ?1, fitness = ?2, tactics = ?3, shooting = ?4, technique = ?5,
                            goalkeeping = ?6, new_position = ?7, new_side = ?8 WHERE id = ?9",
                    params![p.name.trim(), p.fitness, p.tactics, p.shooting, p.technique, p.goalkeeping, p.new_position, p.new_side, p.id],
                )?;
                ids.insert(p.id, p.id);
            } else {
                conn.execute(
                    "INSERT INTO training_programs (name, fitness, tactics, shooting, technique, goalkeeping, new_position, new_side)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                    values,
                )?;
                ids.insert(p.id, conn.last_insert_rowid() as i32);
            }
        }
        for a in &plan.players {
            if !squad.contains(&a.person) {
                return Err(Error::Invalid("Esse jogador não é do plantel.".into()));
            }
            let Some(program) = ids.get(&a.program) else {
                return Err(Error::Invalid("Programa de treino desconhecido.".into()));
            };
            conn.execute("INSERT OR REPLACE INTO training_players (person_id, program_id) VALUES (?, ?)", params![a.person, program])?;
        }
        self.unsaved = true;
        self.training_view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_program_by_position_and_age() {
        let mut pos = [1; 12];
        pos[0] = 20;
        assert_eq!(default_program_name(&pos, 30), "GK");
        let mut pos = [1; 12];
        pos[2] = 20;
        pos[10] = 20;
        assert_eq!(default_program_name(&pos, 25), "DC");
        assert_eq!(default_program_name(&pos, 18), "BASE");
        pos[10] = 1;
        pos[9] = 20;
        assert_eq!(default_program_name(&pos, 25), "LAT");
        let mut pos = [1; 12];
        pos[6] = 20;
        assert_eq!(default_program_name(&pos, 25), "ATACANTES");
        pos[5] = 15;
        assert_eq!(default_program_name(&pos, 25), "MEIAS");
        // Defesa/ala que também joga a médio: a posição mais defensiva manda
        pos[2] = 15;
        pos[7] = 15;
        assert_eq!(default_program_name(&pos, 25), "LAT");
    }

    #[test]
    fn load_counts_the_areas_of_the_player() {
        let p = TrainingProgram {
            id: 1,
            name: "X".into(),
            fitness: 3,
            tactics: 3,
            shooting: 0,
            technique: 0,
            goalkeeping: 3,
            new_position: None,
            new_side: None,
        };
        assert_eq!(p.load(true), 1.0);
        assert_eq!(p.load(false), 0.5);
        assert!(!p.idle());
    }
}
