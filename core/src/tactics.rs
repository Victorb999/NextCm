//! Tácticas de um clube: a escolha automática do adjunto (formação preferida do treinador
//! e o melhor onze disponível pelas notas de posição, o CA e a condição) e a escolha do
//! treinador (marco 3.3), validada e com as notas de cada um na função.

use std::collections::HashMap;

use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::search::position_label;
use crate::{Error, Result, World};

/// Formações na ordem do campo `preferred_formation` (lista do cm0102ed: 0 = "Not set").
pub const FORMATIONS: [&str; 10] = ["4-4-2", "5-3-2 Líbero", "5-3-2", "4-4-2", "4-3-3", "4-2-4", "3-5-2", "4-1-2-1-2", "4-5-1", "3-4-3"];

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
            (L, CENTRE, 50, 82),
            (D, CENTRE, 66, 72),
            (D, CENTRE, 34, 72),
            (AL, RIGHT, 88, 60),
            (AL, LEFT, 12, 60),
            (M, CENTRE, 72, 45),
            (M, CENTRE, 50, 48),
            (M, CENTRE, 28, 45),
        ]),
        2 => s.extend([
            (D, CENTRE, 72, 75),
            (D, CENTRE, 50, 77),
            (D, CENTRE, 28, 75),
            (AL, RIGHT, 88, 60),
            (AL, LEFT, 12, 60),
            (M, CENTRE, 72, 45),
            (M, CENTRE, 50, 48),
            (M, CENTRE, 28, 45),
        ]),
        4 => s.extend(back4.into_iter().chain([
            (M, CENTRE, 72, 48),
            (M, CENTRE, 50, 50),
            (M, CENTRE, 28, 48),
            (A, RIGHT, 82, 18),
            (A, CENTRE, 50, 12),
            (A, LEFT, 18, 18),
        ])),
        5 => s.extend(back4.into_iter().chain([
            (M, CENTRE, 62, 48),
            (M, CENTRE, 38, 48),
            (A, RIGHT, 85, 18),
            (A, CENTRE, 62, 12),
            (A, CENTRE, 38, 12),
            (A, LEFT, 15, 18),
        ])),
        6 => s.extend(back3.into_iter().chain([
            (AL, RIGHT, 88, 50),
            (M, CENTRE, 64, 50),
            (M, CENTRE, 36, 50),
            (AL, LEFT, 12, 50),
            (MA, CENTRE, 50, 32),
        ])),
        7 => s.extend(back4.into_iter().chain([(MD, CENTRE, 50, 58), (M, CENTRE, 70, 45), (M, CENTRE, 30, 45), (MA, CENTRE, 50, 32)])),
        8 => s.extend(back4.into_iter().chain([
            (M, RIGHT, 86, 46),
            (M, CENTRE, 64, 50),
            (M, CENTRE, 36, 50),
            (M, LEFT, 14, 46),
            (MA, CENTRE, 50, 32),
            (A, CENTRE, 50, 12),
        ])),
        9 => s.extend(back3.into_iter().chain([
            (M, RIGHT, 86, 46),
            (M, CENTRE, 62, 50),
            (M, CENTRE, 38, 50),
            (M, LEFT, 14, 46),
            (A, RIGHT, 82, 18),
            (A, CENTRE, 50, 12),
            (A, LEFT, 18, 18),
        ])),
        // 4-4-2 (3) e "não definida"
        _ => s.extend(back4.into_iter().chain([(M, RIGHT, 86, 46), (M, CENTRE, 62, 50), (M, CENTRE, 38, 50), (M, LEFT, 14, 46)])),
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

/// Nota (1–20) de quem tem estas notas de posição numa função ("D C", "GR", "MA E"...):
/// a menor entre a posição e o lado, como na escolha da equipa.
pub(crate) fn fit_for_role(pos: &[Option<i32>; 12], role: &str) -> i32 {
    let mut it = role.split(' ');
    let p = match it.next() {
        Some("GR") => GR,
        Some("L") => L,
        Some("D") => D,
        Some("MD") => MD,
        Some("MA") => MA,
        Some("A") => A,
        Some("AL") => AL,
        _ => M,
    };
    let v = pos[p].unwrap_or(1);
    if p == GR {
        return v;
    }
    let side = match it.next() {
        Some("D") => RIGHT,
        Some("E") => LEFT,
        _ => CENTRE,
    };
    v.min(pos[side].unwrap_or(1))
}

/// Mentalidade, como no diálogo "Instruções à Equipa" do CM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Mentality {
    Defensive,
    #[default]
    Normal,
    Attacking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Passing {
    #[default]
    Mixed,
    Short,
    Direct,
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Tackling {
    #[default]
    Normal,
    Soft,
    Hard,
}

/// Instruções à Equipa. Os batedores de bolas paradas e o organizador chegam no 3.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Instructions {
    pub mentality: Mentality,
    pub passing: Passing,
    pub tackling: Tackling,
    pub pressing: bool,
    pub offside_trap: bool,
    pub counter_attack: bool,
    pub tight_marking: bool,
}

/// O que conta de um jogador para a escolha: vem do jogo em curso (vazio na enciclopédia).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Availability {
    /// 0–100
    pub condition: i32,
    /// 1–20
    pub morale: i32,
    pub injured: bool,
    pub suspended: bool,
}

impl Availability {
    pub fn can_play(&self) -> bool {
        !self.injured && !self.suspended
    }
}

/// Estado dos jogadores por id; quem não está no mapa está a 100% e disponível.
pub type SquadState = HashMap<i32, Availability>;

/// A equipa que o treinador escolheu: formação, titulares por função, suplentes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Selection {
    /// Índice em [`FORMATIONS`].
    pub formation: i32,
    /// Um jogador por função da formação (11), na ordem de `ClubTactics::starters`.
    pub starters: Vec<Option<i32>>,
    /// Até [`BENCH`] suplentes.
    pub bench: Vec<i32>,
    pub instructions: Instructions,
    /// O adjunto escolhe a equipa antes de cada jogo.
    pub auto: bool,
    /// Posições livres (x, y) de cada titular, quando o treinador as mexeu no campo;
    /// vazio = as da formação.
    #[serde(default)]
    pub positions: Vec<[i32; 2]>,
    /// Setas de movimento (botão direito no campo): para onde cada titular corre, alinhado
    /// com `starters`; vazio ou `None` = sem seta.
    #[serde(default)]
    pub runs: Vec<Option<[i32; 2]>>,
}

/// Suplentes no banco (o patch CLZ usa 9, como o CM 01/02 com a Coalizão).
pub const BENCH: usize = 9;

/// Função de quem está em (x, y) no campo (x 0–100 da esquerda para a direita, y 0–100
/// do ataque para a baliza): a zona dá a posição e a coluna dá o lado. As formações
/// pré-definidas caem todas nas funções delas (ver o teste `formations_roles_follow_the_pitch`).
pub fn role_at(x: i32, y: i32) -> (usize, usize) {
    let side = if x < 25 {
        LEFT
    } else if x > 75 {
        RIGHT
    } else {
        CENTRE
    };
    let wide = !(22..=78).contains(&x);
    let pos = if y >= 88 {
        GR
    } else if y >= 80 {
        L
    } else if y >= 64 {
        D
    } else if y >= 48 && wide {
        AL
    } else if y >= 55 {
        MD
    } else if y >= 40 {
        M
    } else if y >= 26 {
        MA
    } else {
        A
    };
    if pos == GR || pos == L { (pos, CENTRE) } else { (pos, side) }
}

/// Ordem das funções na lista, como no CM: GR, líbero, defesas, alas, médios defensivos,
/// médios, médios ofensivos, avançados; em cada linha, esquerda, direita e depois o centro.
fn slot_order(&(pos, side, x, _): &Slot) -> (usize, usize, i32) {
    let line = [0, 1, 2, 4, 5, 6, 7, 3][pos];
    let side = match side {
        LEFT => 0,
        RIGHT => 1,
        _ => 2,
    };
    (line, side, x)
}

/// Para migrar escolhas gravadas antes da ordem do CM (save formato 3): `perm[k]` é a
/// função da formação (na ordem antiga) que fica em `k` na ordem nova.
pub(crate) fn cm_order_permutation(formation: i32) -> Vec<usize> {
    let layout = slots(formation.clamp(0, FORMATIONS.len() as i32 - 1) as usize);
    let mut idx: Vec<usize> = (0..layout.len()).collect();
    idx.sort_by_key(|&i| slot_order(&layout[i]));
    idx
}

/// É guarda-redes natural (nota 15+ na posição).
pub(crate) fn is_goalkeeper(rating: Option<i32>) -> bool {
    rating.unwrap_or(0) >= 15
}

/// Funções da escolha: as posições livres do treinador (se as mexeu) ou as da formação.
fn layout_for(sel: &Selection) -> Vec<Slot> {
    if sel.positions.len() == 11 {
        sel.positions
            .iter()
            .map(|&[x, y]| {
                let (x, y) = (x.clamp(4, 96), y.clamp(4, 96));
                let (pos, side) = role_at(x, y);
                (pos, side, x, y)
            })
            .collect()
    } else {
        let mut layout = slots(sel.formation.clamp(0, FORMATIONS.len() as i32 - 1) as usize);
        layout.sort_by_key(slot_order);
        layout
    }
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
    /// Nota (1–20) do jogador na função onde está; `None` fora do onze.
    pub fit: Option<i32>,
    /// Condição (0–100) e moral (1–20) num jogo em curso.
    pub condition: Option<i32>,
    pub morale: Option<i32>,
    pub injured: bool,
    pub suspended: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TacticSlot {
    pub role: String,
    pub x: i32,
    pub y: i32,
    pub player: Option<TacticPlayer>,
    /// Seta de movimento: para onde corre (acima = ataca, abaixo = recua).
    pub run: Option<[i32; 2]>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClubTactics {
    pub club_id: i32,
    pub club_name: String,
    pub formation: String,
    /// Índice em [`FORMATIONS`].
    pub formation_index: i32,
    pub coach_id: Option<i32>,
    pub coach: Option<String>,
    pub starters: Vec<TacticSlot>,
    /// Suplentes (até [`BENCH`], com um GR quando houver).
    pub bench: Vec<TacticPlayer>,
    /// O resto do plantel.
    pub reserves: Vec<TacticPlayer>,
    pub instructions: Instructions,
    /// É o clube do treinador num jogo em curso (a tela deixa editar).
    pub editable: bool,
    /// O adjunto escolhe a equipa.
    pub auto: bool,
    /// O treinador mexeu nas posições (o CM mostra "*" no nome da táctica).
    pub custom: bool,
}

impl ClubTactics {
    /// A escolha que esta táctica representa.
    pub fn selection(&self) -> Selection {
        Selection {
            formation: self.formation_index,
            starters: self.starters.iter().map(|s| s.player.as_ref().map(|p| p.id)).collect(),
            bench: self.bench.iter().map(|p| p.id).collect(),
            instructions: self.instructions,
            auto: self.auto,
            positions: if self.custom { self.starters.iter().map(|s| [s.x, s.y]).collect() } else { Vec::new() },
            runs: self.starters.iter().map(|s| s.run).collect(),
        }
    }
}

const NO_STATE: Availability = Availability { condition: 100, morale: 12, injured: false, suspended: false };

/// Abaixo disto o adjunto só põe o jogador se não houver outro. Um jogo tira no máximo
/// ~31%, então um só jogo nunca tira ninguém do onze: só a sequência de jogos seguidos.
pub const MIN_CONDITION: i32 = 60;

struct Candidate {
    id: i32,
    list_name: String,
    squad_number: Option<i32>,
    ca: i32,
    pos: [Option<i32>; 12],
    state: Availability,
    in_game: bool,
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
    /// Num jogo em curso, quem está cansado (abaixo de [`MIN_CONDITION`]) só entra se não
    /// houver outro; acima disso a condição não conta, para o onze não mudar a cada jogo.
    fn score(&self, slot: Slot) -> i64 {
        let fit = self.fit(slot);
        // Fora da posição (nota < 15) vale metade; sem divisão inteira, para a nota 1 não zerar
        let factor = if fit >= 15 { 2 * fit } else { fit } as i64;
        let tired = if self.state.condition < MIN_CONDITION { 4 } else { 1 };
        self.ca as i64 * factor / tired
    }

    fn player(&self, fit: Option<i32>) -> TacticPlayer {
        TacticPlayer {
            id: self.id,
            list_name: self.list_name.clone(),
            squad_number: self.squad_number,
            position: position_label(&self.pos),
            current_ability: Some(self.ca),
            fit,
            condition: self.in_game.then_some(self.state.condition),
            morale: self.in_game.then_some(self.state.morale),
            injured: self.state.injured,
            suspended: self.state.suspended,
        }
    }
}

/// Plantel e treinador de um clube.
struct Squad {
    club_name: String,
    coach: Option<(i32, String, Option<i32>)>,
    pool: Vec<Candidate>,
}

impl World {
    fn squad(&self, id: i32, state: Option<&SquadState>) -> Result<Squad> {
        let (club_name, is_national): (String, bool) = self
            .conn
            .prepare_cached("SELECT name, is_national_team FROM clubs WHERE id = ?")?
            .query_row([id], |r| Ok((r.get(0)?, r.get::<_, i32>(1)? != 0)))
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("clube {id}")))?;
        let (link, job) = if is_national { ("p.national_team_id", "p.national_team_job") } else { ("p.club_id", "p.club_job") };

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

        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT p.id, p.list_name, pl.squad_number, pl.current_ability,
                    pl.pos_goalkeeper, pl.pos_sweeper, pl.pos_defender, pl.pos_defensive_midfielder,
                    pl.pos_midfielder, pl.pos_attacking_midfielder, pl.pos_attacker, pl.pos_wing_back,
                    pl.pos_right_side, pl.pos_left_side, pl.pos_central, pl.pos_free_role
             FROM people p JOIN players pl ON pl.person_id = p.id
             WHERE {link} = ?
             ORDER BY pl.current_ability DESC, p.id"
        ))?;
        let pool = stmt
            .query_map([id], |r| {
                let mut pos = [None; 12];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = r.get(4 + i)?;
                }
                let pid: i32 = r.get(0)?;
                Ok(Candidate {
                    id: pid,
                    list_name: r.get(1)?,
                    squad_number: r.get(2)?,
                    ca: r.get::<_, Option<i32>>(3)?.unwrap_or(1),
                    pos,
                    state: state.and_then(|s| s.get(&pid)).copied().unwrap_or(NO_STATE),
                    in_game: state.is_some(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Squad { club_name, coach, pool })
    }

    /// Táctica do clube na enciclopédia (sem jogo em curso): o adjunto escolhe.
    pub fn get_club_tactics(&self, id: i32) -> Result<ClubTactics> {
        self.auto_tactics(id, None, Instructions::default())
    }

    /// O adjunto escolhe: formação preferida do treinador e o melhor onze disponível.
    pub fn auto_tactics(&self, id: i32, state: Option<&SquadState>, instructions: Instructions) -> Result<ClubTactics> {
        let Squad { club_name, coach, pool } = self.squad(id, state)?;
        let formation = coach.as_ref().and_then(|c| c.2).filter(|f| (1..FORMATIONS.len() as i32).contains(f)).unwrap_or(3) as usize;
        let (mut available, unavailable): (Vec<Candidate>, Vec<Candidate>) = pool.into_iter().partition(|c| c.state.can_play());

        // Escolha gulosa: o melhor par (função, jogador) de todos, até fechar o onze
        let mut layout = slots(formation);
        layout.sort_by_key(slot_order);
        let mut chosen: Vec<Option<TacticPlayer>> = vec![None; layout.len()];
        let mut open: Vec<usize> = (0..layout.len()).collect();
        while !open.is_empty() && !available.is_empty() {
            let layout = &layout;
            let (si, pi) = open
                .iter()
                .enumerate()
                .flat_map(|(si, &s)| available.iter().enumerate().map(move |(pi, c)| (si, pi, (c.score(layout[s]), c.ca))))
                // Empate: o de maior CA (e, entre iguais, o primeiro da lista)
                .max_by_key(|&(si, pi, key)| (key, std::cmp::Reverse((pi, si))))
                .map(|(si, pi, _)| (si, pi))
                .unwrap();
            let slot = open.remove(si);
            let c = available.remove(pi);
            chosen[slot] = Some(c.player(Some(c.fit(layout[slot]))));
        }

        // Suplentes: os melhores que sobraram (descansados primeiro), garantindo um GR
        available.sort_by_key(|c| (c.state.condition < MIN_CONDITION, std::cmp::Reverse(c.ca)));
        let mut bench: Vec<TacticPlayer> = Vec::new();
        if let Some(i) = available.iter().position(|c| c.pos[GR].unwrap_or(1) >= 15) {
            bench.push(available.remove(i).player(None));
        }
        let take = BENCH.saturating_sub(bench.len()).min(available.len());
        bench.extend(available.drain(..take).map(|c| c.player(None)));
        let reserves = available.iter().chain(&unavailable).map(|c| c.player(None)).collect();

        Ok(ClubTactics {
            club_id: id,
            club_name,
            formation: FORMATIONS[formation].into(),
            formation_index: formation as i32,
            coach_id: coach.as_ref().map(|c| c.0),
            coach: coach.map(|c| c.1),
            starters: layout
                .iter()
                .zip(chosen)
                .map(|(&(pos, side, x, y), player)| TacticSlot { role: role_label(pos, side), x, y, player, run: None })
                .collect(),
            bench,
            reserves,
            instructions,
            editable: false,
            auto: true,
            custom: false,
        })
    }

    /// A táctica de uma escolha do treinador. Recusa jogadores de fora do plantel,
    /// repetidos, onze com tamanho errado ou suplentes a mais. Os titulares saem por ordem
    /// das funções (GR, defesas, médios, avançados), como na lista do CM.
    pub fn tactics_from_selection(&self, id: i32, sel: &Selection, state: Option<&SquadState>) -> Result<ClubTactics> {
        let formation = usize::try_from(sel.formation)
            .ok()
            .filter(|&f| f < FORMATIONS.len())
            .ok_or_else(|| Error::Invalid("Formação desconhecida.".into()))?;
        let layout = layout_for(sel);
        if sel.starters.len() != layout.len() {
            return Err(Error::Invalid(format!("A equipa tem de ter {} titulares.", layout.len())));
        }
        if sel.bench.len() > BENCH {
            return Err(Error::Invalid(format!("No máximo {BENCH} suplentes.")));
        }
        let Squad { club_name, coach, pool } = self.squad(id, state)?;
        let mut seen = std::collections::HashSet::new();
        for pid in sel.starters.iter().flatten().chain(&sel.bench) {
            if !pool.iter().any(|c| c.id == *pid) {
                return Err(Error::Invalid("Esse jogador não é do plantel.".into()));
            }
            if !seen.insert(*pid) {
                return Err(Error::Invalid("Um jogador não pode estar em dois lugares.".into()));
            }
        }
        let find = |pid: i32| pool.iter().find(|c| c.id == pid).unwrap();
        let run = |i: usize| sel.runs.get(i).copied().flatten().map(|[x, y]| [x.clamp(2, 98), y.clamp(2, 98)]);
        let mut pairs: Vec<(Slot, Option<i32>, Option<[i32; 2]>)> =
            layout.into_iter().zip(sel.starters.iter().copied()).enumerate().map(|(i, (slot, pid))| (slot, pid, run(i))).collect();
        pairs.sort_by_key(|(slot, ..)| slot_order(slot));
        let starters = pairs
            .iter()
            .map(|&(slot, pid, run)| {
                let (pos, side, x, y) = slot;
                TacticSlot {
                    role: role_label(pos, side),
                    x,
                    y,
                    player: pid.map(|pid| {
                        let c = find(pid);
                        c.player(Some(c.fit(slot)))
                    }),
                    run,
                }
            })
            .collect();
        let bench = sel.bench.iter().map(|&pid| find(pid).player(None)).collect();
        let reserves = pool.iter().filter(|c| !seen.contains(&c.id)).map(|c| c.player(None)).collect();
        Ok(ClubTactics {
            club_id: id,
            club_name,
            formation: FORMATIONS[formation].into(),
            formation_index: formation as i32,
            coach_id: coach.as_ref().map(|c| c.0),
            coach: coach.map(|c| c.1),
            starters,
            bench,
            reserves,
            instructions: sel.instructions,
            editable: false,
            auto: sel.auto,
            custom: sel.positions.len() == 11,
        })
    }

    /// Põe ou tira um jogador da equipa (clicar na casa ou no número, como no CM): quem
    /// está no onze ou no banco sai; quem está de fora entra na função vazia onde joga
    /// melhor, senão no banco (até [`BENCH`]).
    pub fn toggle_player(&self, id: i32, sel: &mut Selection, player: i32) -> Result<()> {
        sel.auto = false;
        if let Some(i) = sel.starters.iter().position(|s| *s == Some(player)) {
            sel.starters[i] = None;
            return Ok(());
        }
        if let Some(i) = sel.bench.iter().position(|&b| b == player) {
            sel.bench.remove(i);
            return Ok(());
        }
        let Squad { pool, .. } = self.squad(id, None)?;
        let c = pool.iter().find(|c| c.id == player).ok_or_else(|| Error::Invalid("Esse jogador não é do plantel.".into()))?;
        let layout = layout_for(sel);
        sel.starters.resize(layout.len(), None);
        let empty = (0..layout.len()).filter(|&i| sel.starters[i].is_none()).max_by_key(|&i| (c.fit(layout[i]), std::cmp::Reverse(i)));
        if let Some(i) = empty {
            sel.starters[i] = Some(player);
        } else if sel.bench.len() < BENCH {
            sel.bench.push(player);
        } else {
            return Err(Error::Invalid("O onze e o banco estão completos: tire alguém primeiro.".into()));
        }
        Ok(())
    }

    /// Antes do jogo: troca os titulares lesionados/suspensos (e as funções vazias) pelo
    /// melhor suplente disponível para a função, senão das reservas. Devolve (saiu, entrou);
    /// `saiu` é `None` quando a função estava vazia.
    pub fn cover_unavailable(&self, id: i32, sel: &mut Selection, state: &SquadState) -> Result<Vec<(Option<i32>, i32)>> {
        let Squad { pool, .. } = self.squad(id, Some(state))?;
        let layout = layout_for(sel);
        sel.starters.resize(layout.len(), None);
        let can_play = |pid: i32| pool.iter().any(|c| c.id == pid && c.state.can_play());
        let mut changes = Vec::new();
        for (i, &slot) in layout.iter().enumerate() {
            let current = sel.starters[i];
            if current.is_some_and(can_play) {
                continue;
            }
            let taken: Vec<i32> = sel.starters.iter().flatten().copied().collect();
            let pick = |from_bench: bool| {
                pool.iter()
                    .filter(|c| c.state.can_play() && !taken.contains(&c.id) && sel.bench.contains(&c.id) == from_bench)
                    .max_by_key(|c| (c.score(slot), c.ca))
                    .map(|c| c.id)
            };
            let Some(new) = pick(true).or_else(|| pick(false)) else { continue };
            sel.bench.retain(|&b| b != new);
            sel.starters[i] = Some(new);
            changes.push((current, new));
        }
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formations_roles_follow_the_pitch() {
        for (f, name) in FORMATIONS.iter().enumerate().skip(1) {
            for (pos, side, x, y) in slots(f) {
                assert_eq!(role_at(x, y), (pos, side), "{name} em ({x}, {y})");
            }
        }
        // Um avançado arrastado para o meio-campo vira médio
        assert_eq!(role_at(50, 45), (M, CENTRE));
        assert_eq!(role_at(85, 20), (A, RIGHT));
    }

    #[test]
    fn starters_come_in_cm_order() {
        let mut layout = slots(3);
        layout.sort_by_key(slot_order);
        let roles: Vec<String> = layout.iter().map(|&(p, s, ..)| role_label(p, s)).collect();
        assert_eq!(roles, ["GR", "D E", "D D", "D C", "D C", "M E", "M D", "M C", "M C", "A C", "A C"]);
    }

    #[test]
    fn every_formation_has_eleven() {
        for (f, name) in FORMATIONS.iter().enumerate() {
            assert_eq!(slots(f).len(), 11, "{name}");
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
        assert_eq!(t.bench.len(), BENCH);
        let ids: std::collections::HashSet<_> =
            t.starters.iter().filter_map(|s| s.player.as_ref().map(|p| p.id)).chain(t.bench.iter().map(|p| p.id)).collect();
        assert_eq!(ids.len(), 11 + BENCH, "ninguém aparece duas vezes");
    }
}
