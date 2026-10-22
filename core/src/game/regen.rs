//! Jogadores criados pelo jogo: os juniores (marco 3.5) e os plantéis completados dos
//! clubes pequenos que o patch deixou sem jogadores (3.6), como o CM faz. Nome e apelido de
//! pessoas da nação do clube e atributos copiados de um jogador real da mesma posição (os
//! atributos do banco são relativos ao CA, então continuam coerentes).

use std::collections::HashMap;
use std::collections::hash_map::Entry;

use rand_chacha::ChaCha8Rng;
use rand_core::RngCore;
use rusqlite::params;

use super::Game;
use crate::engine::uniform;
use crate::search::fold;
use crate::{Result, date};

/// Cargo "Jogador" na tabela `jobs`.
pub(super) const PLAYER_JOB: i32 = 11;

/// Posição de um jogador criado: a coluna de nota de posição e a sigla.
pub(super) type Position = (&'static str, &'static str);
pub(super) const GK: Position = ("pos_goalkeeper", "GR");
pub(super) const DEF: Position = ("pos_defender", "D");
pub(super) const MID: Position = ("pos_midfielder", "M");
pub(super) const ATT: Position = ("pos_attacker", "A");

/// Uma posição sorteada como nos juniores: 10% GR, 30% D, 35% M, 25% A.
pub(super) fn random_position(rng: &mut impl RngCore) -> Position {
    match uniform(rng) {
        r if r < 0.10 => GK,
        r if r < 0.40 => DEF,
        r if r < 0.75 => MID,
        _ => ATT,
    }
}

pub(super) struct NewPlayer {
    pub club: i32,
    pub nation: Option<i32>,
    pub birth_year: i32,
    pub ca: i32,
    pub pa: i32,
    pub position: Position,
    pub contract_until: String,
}

/// O que se lê uma vez para criar muitos jogadores: os modelos por nação e posição e os
/// nomes por nação ficam na memória (um `OFFSET` por jogador varria a tabela toda).
pub(super) struct Factory {
    insert_player: String,
    next_person: i32,
    templates: HashMap<(Option<i32>, &'static str), Vec<i32>>,
    names: HashMap<Option<i32>, Vec<(String, String)>>,
}

impl Game {
    pub(super) fn player_factory(&self) -> Result<Factory> {
        // Colunas de `players` copiadas do modelo (todas menos id, número, CA/PA e reputações)
        let copied: Vec<String> = self
            .world
            .conn
            .prepare("PRAGMA table_info(players)")?
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|c| {
                !matches!(
                    c.as_str(),
                    "person_id"
                        | "squad_number"
                        | "current_ability"
                        | "potential_ability"
                        | "home_reputation"
                        | "current_reputation"
                        | "world_reputation"
                )
            })
            .collect();
        let insert_player = format!(
            "INSERT INTO players (person_id, squad_number, current_ability, potential_ability, home_reputation, current_reputation, world_reputation, {cols})
             SELECT ?1, NULL, ?2, ?3, 100, 100, 10, {cols} FROM players WHERE person_id = ?4",
            cols = copied.join(", ")
        );
        let next_person: i32 = self.world.conn.query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM people", [], |r| r.get(0))?;
        Ok(Factory { insert_player, next_person, templates: HashMap::new(), names: HashMap::new() })
    }

    /// Cria um jogador; `None` se a nação não tiver modelo para a posição. Devolve o id e o nome.
    pub(super) fn create_player(&self, f: &mut Factory, p: &NewPlayer, rng: &mut ChaCha8Rng) -> Result<Option<(i32, String)>> {
        let (column, _) = p.position;
        let nation = p.nation;
        if let Entry::Vacant(e) = f.templates.entry((nation, column)) {
            let ids: Vec<i32> = self
                .world
                .conn
                .prepare(&format!(
                    "SELECT pl.person_id FROM players pl JOIN people p ON p.id = pl.person_id
                     WHERE p.nation_id IS ? AND pl.{column} >= 15 ORDER BY pl.person_id"
                ))?
                .query_map([nation], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            e.insert(ids);
        }
        let templates = &f.templates[&(nation, column)];
        if templates.is_empty() {
            return Ok(None);
        }
        // Modelo: um jogador real da nação na mesma posição
        let template = templates[(rng.next_u64() % templates.len() as u64) as usize];
        // Nome e apelido de duas pessoas da nação (um nome inventado)
        if let Entry::Vacant(e) = f.names.entry(nation) {
            let names: Vec<(String, String)> = self
                .world
                .conn
                .prepare("SELECT first_name, second_name FROM people WHERE nation_id IS ? AND first_name <> '' AND second_name <> '' ORDER BY id")?
                .query_map([nation], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            e.insert(names);
        }
        let names = &f.names[&nation];
        let pick = |rng: &mut ChaCha8Rng| (rng.next_u64() % names.len().max(1) as u64) as usize;
        let first = names.get(pick(rng)).map_or("João", |n| n.0.as_str()).split_whitespace().next().unwrap_or("João").to_string();
        let last = names.get(pick(rng)).map_or_else(|| "Silva".to_string(), |n| n.1.clone());
        let display = format!("{first} {last}");
        let born = date::format(date::parse(&format!("{}-01-01", p.birth_year)).unwrap_or(0) + (rng.next_u32() % 365) as i32);
        let id = f.next_person;
        f.next_person += 1;
        self.world.conn.execute(
            "INSERT INTO people (id, display_name, first_name, second_name, common_name, search_name, list_name,
                                 birth_date, birth_year, nation_id, international_apps, international_goals,
                                 club_id, club_job, club_joined, contract_expires, wage,
                                 adaptability, ambition, determination, loyalty, pressure, professionalism, sportsmanship, temperament,
                                 is_player, is_non_player, ability, birth_sort, positions)
             SELECT ?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, 0, 0, ?10, ?11, ?12, ?13, 500,
                    adaptability, ambition, determination, loyalty, pressure, professionalism, sportsmanship, temperament,
                    1, 0, ?14, ?7, positions
             FROM people WHERE id = ?15",
            params![
                id,
                display,
                first,
                last,
                fold(&display),
                format!("{last}, {first}"),
                born,
                p.birth_year,
                nation,
                p.club,
                PLAYER_JOB,
                self.date,
                p.contract_until,
                p.ca,
                template
            ],
        )?;
        self.world.conn.execute(&f.insert_player, params![id, p.ca, p.pa, template])?;
        Ok(Some((id, display)))
    }

    /// Completa até 18 jogadores (2 GR, 6 D, 6 M e 4 A no mínimo) os plantéis dos clubes que
    /// jogam nesta época. O nível vem da reputação do clube: o CA médio dos 11 melhores no
    /// patch é ≈ 74,6 + 0,0101 × reputação.
    pub(super) fn complete_squads(&mut self) -> Result<usize> {
        let year = self.season();
        let mut clubs: Vec<i32> =
            self.matches.iter().filter(|m| m.date.starts_with(&year.to_string())).flat_map(|m| [m.home_id, m.away_id]).collect();
        clubs.sort_unstable();
        clubs.dedup();
        let mut rng = self.season_rng(0x5EED_F111);
        let mut factory = self.player_factory()?;
        let mut created = 0;
        for club in clubs {
            let (count, gk, d, m, a, nation, reputation): (i64, i64, i64, i64, i64, Option<i32>, Option<i32>) = self.world.conn.query_row(
                "SELECT COUNT(p.id), COALESCE(SUM(pl.pos_goalkeeper >= 15), 0), COALESCE(SUM(pl.pos_defender >= 15), 0),
                        COALESCE(SUM(pl.pos_midfielder >= 15), 0), COALESCE(SUM(pl.pos_attacker >= 15), 0), c.nation_id, c.reputation
                 FROM clubs c LEFT JOIN people p ON p.club_id = c.id AND p.is_player = 1
                 LEFT JOIN players pl ON pl.person_id = p.id WHERE c.id = ? GROUP BY c.id",
                [club],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )?;
            if count >= 18 {
                continue;
            }
            let mut need: Vec<Position> = Vec::new();
            for (have, min, pos) in [(gk, 2, GK), (d, 6, DEF), (m, 6, MID), (a, 4, ATT)] {
                need.extend(std::iter::repeat_n(pos, (min - have).max(0) as usize));
            }
            while (count as usize) + need.len() < 18 {
                need.push(if need.len().is_multiple_of(2) { MID } else { DEF });
            }
            need.truncate(18 - count as usize);
            let level = 74.6 + 0.0101 * reputation.unwrap_or(1000) as f64;
            for position in need {
                let ca = (level * (0.75 + 0.25 * uniform(&mut rng))).round().clamp(30.0, 170.0) as i32;
                let p = NewPlayer {
                    club,
                    nation,
                    birth_year: year - 18 - (rng.next_u32() % 15) as i32,
                    ca,
                    pa: ca + (rng.next_u32() % 15) as i32,
                    position,
                    contract_until: format!("{}-12-31", year + 1),
                };
                if self.create_player(&mut factory, &p, &mut rng)?.is_some() {
                    created += 1;
                }
            }
        }
        if created > 0 {
            self.unsaved = true;
        }
        Ok(created)
    }
}
