//! Tácticas de um clube antes do 1º jogo: a formação preferida do treinador e um onze
//! escolhido pelas notas de posição (1–20) e pelo CA. Na Fase 3 isto vira a escolha da
//! equipa pela IA / pelo jogador.

use rusqlite::OptionalExtension;
use serde::Serialize;
use ts_rs::TS;

use crate::search::position_label;
use crate::{Error, Result, World};

/// Formações na ordem do campo `preferred_formation` (lista do cm0102ed: 0 = "Not set").
pub const FORMATIONS: [&str; 10] =
    ["4-4-2", "5-3-2 Líbero", "5-3-2", "4-4-2", "4-3-3", "4-2-4", "3-5-2", "4-1-2-1-2", "4-5-1", "3-4-3"];

// Índices das notas de posição (pos_goalkeeper..pos_free_role) e de lado
const GR: usize = 0;
const L: usize = 1;
const D: usize = 2;
const MD: usize = 3;
const M: usize = 4;
const MA: usize = 5;
const A: usize = 6;
const AL: usize = 7;
const RIGHT: usize = 8;
const LEFT: usize = 9;
const CENTRE: usize = 10;

/// Lugar no campo: posição, lado e coordenadas (x 0–100 da esquerda para a direita,
/// y 0–100 do ataque para a baliza).
type Slot = (usize, usize, i32, i32);

fn slots(formation: usize) -> Vec<Slot> {
    let gk = (GR, CENTRE, 50, 92);
    let back4 = [(D, RIGHT, 85, 72), (D, CENTRE, 62, 75), (D, CENTRE, 38, 75), (D, LEFT, 15, 72)];
    let back3 = [(D, CENTRE, 72, 75), (D, CENTRE, 50, 77), (D, CENTRE, 28, 75)];
    let two_up = [(A, CENTRE, 62, 14), (A, CENTRE, 38, 14)];
    let mut s = vec![gk];
    match formation {
        1 => s.extend([
            (L, CENTRE, 50, 82), (D, CENTRE, 66, 72), (D, CENTRE, 34, 72), (AL, RIGHT, 88, 60), (AL, LEFT, 12, 60),
            (M, CENTRE, 72, 45), (M, CENTRE, 50, 48), (M, CENTRE, 28, 45),
        ]),
        2 => s.extend([
            (D, CENTRE, 72, 75), (D, CENTRE, 50, 77), (D, CENTRE, 28, 75), (AL, RIGHT, 88, 60), (AL, LEFT, 12, 60),
            (M, CENTRE, 72, 45), (M, CENTRE, 50, 48), (M, CENTRE, 28, 45),
        ]),
        4 => s.extend(back4.into_iter().chain([
            (M, CENTRE, 72, 48), (M, CENTRE, 50, 50), (M, CENTRE, 28, 48),
            (A, RIGHT, 82, 18), (A, CENTRE, 50, 12), (A, LEFT, 18, 18),
        ])),
        5 => s.extend(back4.into_iter().chain([
            (M, CENTRE, 62, 48), (M, CENTRE, 38, 48),
            (A, RIGHT, 85, 18), (A, CENTRE, 62, 12), (A, CENTRE, 38, 12), (A, LEFT, 15, 18),
        ])),
        6 => s.extend(back3.into_iter().chain([
            (AL, RIGHT, 88, 50), (M, CENTRE, 64, 50), (M, CENTRE, 36, 50), (AL, LEFT, 12, 50), (MA, CENTRE, 50, 32),
        ])),
        7 => s.extend(back4.into_iter().chain([
            (MD, CENTRE, 50, 58), (M, CENTRE, 70, 45), (M, CENTRE, 30, 45), (MA, CENTRE, 50, 32),
        ])),
        8 => s.extend(back4.into_iter().chain([
            (M, RIGHT, 86, 46), (M, CENTRE, 64, 50), (M, CENTRE, 36, 50), (M, LEFT, 14, 46), (MA, CENTRE, 50, 32),
            (A, CENTRE, 50, 12),
        ])),
        9 => s.extend(back3.into_iter().chain([
            (M, RIGHT, 86, 46), (M, CENTRE, 62, 50), (M, CENTRE, 38, 50), (M, LEFT, 14, 46),
            (A, RIGHT, 82, 18), (A, CENTRE, 50, 12), (A, LEFT, 18, 18),
        ])),
        // 4-4-2 (3) e "não definida"
        _ => s.extend(back4.into_iter().chain([
            (M, RIGHT, 86, 46), (M, CENTRE, 62, 50), (M, CENTRE, 38, 50), (M, LEFT, 14, 46),
        ])),
    }
    // As formações sem avançados na lista acima jogam com dois pontas de lança
    if matches!(formation, 0..=3 | 6 | 7) {
        s.extend(two_up);
    }
    s
}

/// Sigla da função no campo, como o CM: "GR", "D D", "M C", "AL E"...
fn role_label(pos: usize, side: usize) -> String {
    let p = ["GR", "L", "D", "MD", "M", "MA", "A", "AL"][pos];
    if pos == GR {
        return p.into();
    }
    let s = match side {
        RIGHT => "D",
        LEFT => "E",
        _ => "C",
    };
    format!("{p} {s}")
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TacticPlayer {
    pub id: i32,
    pub list_name: String,
    pub squad_number: Option<i32>,
    /// Posições do jogador, ex.: "D/MD C".
    pub position: String,
    pub current_ability: Option<i32>,
    /// Nota (1–20) do jogador na função onde está; `None` no banco.
    pub fit: Option<i32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TacticSlot {
    pub role: String,
    pub x: i32,
    pub y: i32,
    pub player: Option<TacticPlayer>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClubTactics {
    pub club_id: i32,
    pub club_name: String,
    pub formation: String,
    pub coach_id: Option<i32>,
    pub coach: Option<String>,
    pub starters: Vec<TacticSlot>,
    /// Suplentes (7, com um GR quando houver).
    pub bench: Vec<TacticPlayer>,
}

struct Candidate {
    id: i32,
    list_name: String,
    squad_number: Option<i32>,
    ca: i32,
    pos: [Option<i32>; 12],
}

impl Candidate {
    /// Nota na função: a menor entre a posição e o lado.
    fn fit(&self, (pos, side, ..): Slot) -> i32 {
        let p = self.pos[pos].unwrap_or(1);
        if pos == GR {
            return p;
        }
        p.min(self.pos[side].unwrap_or(1))
    }

    /// Quanto vale pôr o jogador na função: o CA pesa a adaptação (nota 20 = CA inteiro).
    fn score(&self, slot: Slot) -> i32 {
        let fit = self.fit(slot);
        // Fora da posição (nota < 15) vale metade; sem divisão inteira, para a nota 1 não zerar
        let factor = if fit >= 15 { 2 * fit } else { fit };
        self.ca * factor
    }

    fn player(&self, fit: Option<i32>) -> TacticPlayer {
        TacticPlayer {
            id: self.id,
            list_name: self.list_name.clone(),
            squad_number: self.squad_number,
            position: position_label(&self.pos),
            current_ability: Some(self.ca),
            fit,
        }
    }
}

impl World {
    pub fn get_club_tactics(&self, id: i32) -> Result<ClubTactics> {
        let (club_name, is_national): (String, bool) = self
            .conn
            .prepare_cached("SELECT name, is_national_team FROM clubs WHERE id = ?")?
            .query_row([id], |r| Ok((r.get(0)?, r.get::<_, i32>(1)? != 0)))
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("clube {id}")))?;
        let (link, job) = if is_national {
            ("p.national_team_id", "p.national_team_job")
        } else {
            ("p.club_id", "p.club_job")
        };

        // Treinador principal (cargo 5) e a formação preferida dele
        let coach: Option<(i32, String, Option<i32>)> = self
            .conn
            .prepare_cached(&format!(
                "SELECT p.id, p.display_name, np.preferred_formation FROM people p
                 LEFT JOIN non_players np ON np.person_id = p.id
                 WHERE {link} = ? AND {job} = 5 LIMIT 1"
            ))?
            .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        let formation = coach
            .as_ref()
            .and_then(|c| c.2)
            .filter(|f| (1..FORMATIONS.len() as i32).contains(f))
            .unwrap_or(3) as usize;

        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT p.id, p.list_name, pl.squad_number, pl.current_ability,
                    pl.pos_goalkeeper, pl.pos_sweeper, pl.pos_defender, pl.pos_defensive_midfielder,
                    pl.pos_midfielder, pl.pos_attacking_midfielder, pl.pos_attacker, pl.pos_wing_back,
                    pl.pos_right_side, pl.pos_left_side, pl.pos_central, pl.pos_free_role
             FROM people p JOIN players pl ON pl.person_id = p.id
             WHERE {link} = ?
             ORDER BY pl.current_ability DESC, p.id"
        ))?;
        let mut pool: Vec<Candidate> = stmt
            .query_map([id], |r| {
                let mut pos = [None; 12];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = r.get(4 + i)?;
                }
                Ok(Candidate {
                    id: r.get(0)?,
                    list_name: r.get(1)?,
                    squad_number: r.get(2)?,
                    ca: r.get::<_, Option<i32>>(3)?.unwrap_or(1),
                    pos,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        // Escolha gulosa: o melhor par (função, jogador) de todos, até fechar o onze
        let layout = slots(formation);
        let mut chosen: Vec<Option<TacticPlayer>> = vec![None; layout.len()];
        let mut open: Vec<usize> = (0..layout.len()).collect();
        while !open.is_empty() && !pool.is_empty() {
            let layout = &layout;
            let (si, pi) = open
                .iter()
                .enumerate()
                .flat_map(|(si, &s)| pool.iter().enumerate().map(move |(pi, c)| (si, pi, (c.score(layout[s]), c.ca))))
                // Empate: o de maior CA (e, entre iguais, o primeiro da lista)
                .max_by_key(|&(si, pi, key)| (key, std::cmp::Reverse((pi, si))))
                .map(|(si, pi, _)| (si, pi))
                .unwrap();
            let slot = open.remove(si);
            let c = pool.remove(pi);
            chosen[slot] = Some(c.player(Some(c.fit(layout[slot]))));
        }

        // Suplentes: os melhores que sobraram, garantindo um GR
        let mut bench: Vec<TacticPlayer> = Vec::new();
        if let Some(i) = pool.iter().position(|c| c.pos[GR].unwrap_or(1) >= 15) {
            bench.push(pool.remove(i).player(None));
        }
        bench.extend(pool.iter().take(7 - bench.len()).map(|c| c.player(None)));

        Ok(ClubTactics {
            club_id: id,
            club_name,
            formation: FORMATIONS[formation].into(),
            coach_id: coach.as_ref().map(|c| c.0),
            coach: coach.map(|c| c.1),
            starters: layout
                .iter()
                .zip(chosen)
                .map(|(&(pos, side, x, y), player)| TacticSlot { role: role_label(pos, side), x, y, player })
                .collect(),
            bench,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_formation_has_eleven() {
        for f in 0..FORMATIONS.len() {
            assert_eq!(slots(f).len(), 11, "{}", FORMATIONS[f]);
        }
    }

    #[test]
    fn known_club_tactics() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        // Palmeiras: Abel Ferreira prefere 4-3-3
        let t = w.get_club_tactics(7568).unwrap();
        assert_eq!(t.formation, "4-3-3");
        assert_eq!(t.coach.as_deref(), Some("Abel Ferreira"));
        assert_eq!(t.starters.len(), 11);
        assert!(t.starters.iter().all(|s| s.player.is_some()));
        assert_eq!(t.starters[0].role, "GR");
        assert!(t.starters[0].player.as_ref().unwrap().fit.unwrap() >= 15);
        assert_eq!(t.bench.len(), 7);
        let ids: std::collections::HashSet<_> =
            t.starters.iter().filter_map(|s| s.player.as_ref().map(|p| p.id)).chain(t.bench.iter().map(|p| p.id)).collect();
        assert_eq!(ids.len(), 18, "ninguém aparece duas vezes");
    }
}
