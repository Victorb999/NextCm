//! Números de camisola, como no CM: os clubes da IA numeram sozinhos; no clube do treinador
//! quem chega fica sem número e o treinador escolhe na tela "Números Plantel" (o Auto
//! sugere). Cada jogador prefere o número que tinha e, depois, os números clássicos da
//! posição (1 o guarda-redes, 4 o central, 9 o ponta de lança, 10 o criativo...), com os
//! melhores (pelo CA) a escolher primeiro. Um jogo do treinador só começa com os
//! convocados numerados.

use std::collections::HashSet;

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Game, NewsItem};
use crate::person::ClubRef;
use crate::{Error, Result, World};

/// Um jogador na tela "Números Plantel".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NumberEntry {
    pub id: i32,
    pub list_name: String,
    /// Siglas do CM, ex.: "D DC".
    pub position: String,
    pub position_order: i32,
    pub number: Option<i32>,
}

/// A tela "Números Plantel".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SquadNumbers {
    pub club: ClubRef,
    pub players: Vec<NumberEntry>,
}

/// Um número escolhido (ou nenhum) para um jogador.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NumberAssign {
    pub id: i32,
    pub number: Option<i32>,
}

/// O que um jogador sabe jogar, para os números clássicos da posição (atributos de 1 a 20).
struct Shape {
    gk: i32,
    def: i32,
    dm: i32,
    mid: i32,
    am: i32,
    att: i32,
    wb: i32,
    right: i32,
    left: i32,
    central: i32,
}

const PLAYS: i32 = 15;

/// Os números clássicos de uma posição, do preferido para o menos.
fn classic_numbers(s: &Shape) -> &'static [i32] {
    let side = |right: &'static [i32], left: &'static [i32], central: &'static [i32]| {
        if s.central >= PLAYS {
            central
        } else if s.right >= s.left {
            right
        } else {
            left
        }
    };
    if s.gk >= PLAYS {
        return &[1, 12, 22, 23, 13, 31, 33];
    }
    // A posição mais forte (empates para a mais ofensiva)
    let best = [s.att, s.am, s.mid, s.dm, s.def.max(s.wb)].into_iter().max().unwrap_or(0);
    if s.att == best {
        side(&[7, 17, 11, 19], &[11, 17, 7, 19], &[9, 19, 11, 21, 18])
    } else if s.am == best {
        side(&[7, 17, 16], &[11, 17, 16], &[10, 8, 20, 18])
    } else if s.mid == best {
        side(&[7, 17, 16], &[11, 17, 16], &[8, 10, 16, 18, 14])
    } else if s.dm == best {
        &[5, 8, 15, 16, 6]
    } else {
        side(&[2, 13, 22, 24], &[6, 3, 16, 26], &[4, 3, 5, 6, 14, 15, 13])
    }
}

/// Dá números livres aos `missing` (já pela ordem do CA): primeiro o número que o jogador
/// tinha, depois os da posição, senão um ao acaso entre 2 e 40 (ou até 99).
fn assign(missing: &[(i32, Option<i32>, Shape)], used: &mut HashSet<i32>, rng: &mut ChaCha8Rng) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for (id, preferred, shape) in missing {
        let wanted = preferred.iter().copied().chain(classic_numbers(shape).iter().copied());
        let free = match wanted.filter(|n| (1..=99).contains(n)).find(|n| !used.contains(n)) {
            Some(n) => n,
            None => {
                let pool: Vec<i32> = (2..=40).filter(|n| !used.contains(n)).collect();
                let pool = if pool.is_empty() { (41..=99).filter(|n| !used.contains(n)).collect() } else { pool };
                let Some(&n) = pool.get((rng.next_u64() % pool.len().max(1) as u64) as usize) else { continue };
                n
            }
        };
        used.insert(free);
        out.push((*id, free));
    }
    out
}

/// Os jogadores de um clube pelo CA (o melhor primeiro): id, número, número preferido e posições.
fn squad_rows(world: &World, club: i32) -> Result<Vec<(i32, Option<i32>, Option<i32>, Shape)>> {
    Ok(world
        .conn
        .prepare_cached(
            "SELECT pl.person_id, pl.squad_number, pl.preferred_number,
                    COALESCE(pl.pos_goalkeeper, 1), MAX(COALESCE(pl.pos_defender, 1), COALESCE(pl.pos_sweeper, 1)),
                    COALESCE(pl.pos_defensive_midfielder, 1), COALESCE(pl.pos_midfielder, 1),
                    COALESCE(pl.pos_attacking_midfielder, 1), COALESCE(pl.pos_attacker, 1), COALESCE(pl.pos_wing_back, 1),
                    COALESCE(pl.pos_right_side, 1), COALESCE(pl.pos_left_side, 1), COALESCE(pl.pos_central, 1)
             FROM players pl JOIN people p ON p.id = pl.person_id
             WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC, pl.person_id",
        )?
        .query_map([club], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                Shape {
                    gk: r.get(3)?,
                    def: r.get(4)?,
                    dm: r.get(5)?,
                    mid: r.get(6)?,
                    am: r.get(7)?,
                    att: r.get(8)?,
                    wb: r.get(9)?,
                    right: r.get(10)?,
                    left: r.get(11)?,
                    central: r.get(12)?,
                },
            ))
        })?
        .collect::<rusqlite::Result<_>>()?)
}

fn club_rng(seed: i64, club: i32) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64((seed as u64) ^ (club as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// Números únicos em cada clube. O patch repete números (o Palmeiras tem dois 4 e dois 6) e
/// deixa jogadores sem número: quem tem mais CA fica com o número e os outros recebem um
/// livre pelas preferências. No clube `manual` (o do treinador) os repetidos ficam sem
/// número e ninguém recebe um: é o treinador que escolhe.
pub(super) fn fix_squad_numbers(world: &World, clubs: &[i32], seed: i64, manual: Option<i32>) -> Result<()> {
    // Corre dentro da transação aberta do save (centenas de UPDATEs soltos fariam um fsync cada)
    for &club in clubs {
        let rows = squad_rows(world, club)?;
        let mut used = HashSet::new();
        let mut missing = Vec::new();
        for (id, n, preferred, shape) in rows {
            match n.filter(|n| (1..=99).contains(n)) {
                Some(n) if used.insert(n) => {}
                had => {
                    if manual == Some(club) {
                        if had.is_some() {
                            world.conn.execute("UPDATE players SET squad_number = NULL WHERE person_id = ?", [id])?;
                        }
                    } else {
                        missing.push((id, preferred.or(had), shape));
                    }
                }
            }
        }
        for (id, n) in assign(&missing, &mut used, &mut club_rng(seed, club)) {
            world.conn.execute("UPDATE players SET squad_number = ? WHERE person_id = ?", params![n, id])?;
        }
    }
    Ok(())
}

impl Game {
    /// Um jogador mudou de clube (ou ficou livre): o número antigo passa a preferido e, se
    /// chega ao clube do treinador, fica sem número até o treinador escolher.
    pub(super) fn number_after_move(&self, id: i32, to: Option<i32>) -> Result<()> {
        self.world.conn.execute(
            "UPDATE players SET preferred_number = COALESCE(squad_number, preferred_number),
                    squad_number = CASE WHEN ?2 IS NULL OR ?2 = ?3 THEN NULL ELSE squad_number END
             WHERE person_id = ?1",
            params![id, to, self.club_id],
        )?;
        Ok(())
    }

    /// A tela "Números Plantel" do clube do treinador.
    pub fn squad_numbers(&self) -> Result<SquadNumbers> {
        let profile = self.world.get_club(self.club_id)?;
        let mut players: Vec<NumberEntry> = profile
            .players
            .into_iter()
            .map(|p| NumberEntry {
                id: p.id,
                list_name: p.list_name,
                position: p.position,
                position_order: p.position_order,
                number: p.squad_number,
            })
            .collect();
        players.sort_by(|a, b| a.position_order.cmp(&b.position_order).then(a.list_name.cmp(&b.list_name)));
        Ok(SquadNumbers { club: profile.club, players })
    }

    /// O botão Auto: números para quem não tem, sem mexer nos escolhidos.
    pub fn auto_squad_numbers(&self, current: &[NumberAssign]) -> Result<Vec<NumberAssign>> {
        let mut used: HashSet<i32> = current.iter().filter_map(|a| a.number).collect();
        let taken: HashSet<i32> = current.iter().filter(|a| a.number.is_some()).map(|a| a.id).collect();
        let listed: HashSet<i32> = current.iter().map(|a| a.id).collect();
        let missing: Vec<(i32, Option<i32>, Shape)> = squad_rows(&self.world, self.club_id)?
            .into_iter()
            .filter(|r| listed.contains(&r.0) && !taken.contains(&r.0))
            .map(|(id, n, preferred, shape)| (id, preferred.or(n), shape))
            .collect();
        let got = assign(&missing, &mut used, &mut club_rng(self.seed, self.club_id));
        Ok(current
            .iter()
            .map(|a| NumberAssign { id: a.id, number: a.number.or_else(|| got.iter().find(|g| g.0 == a.id).map(|g| g.1)) })
            .collect())
    }

    /// Grava os números escolhidos (de 1 a 99, sem repetir).
    pub fn set_squad_numbers(&mut self, numbers: &[NumberAssign]) -> Result<SquadNumbers> {
        let squad: HashSet<i32> = squad_rows(&self.world, self.club_id)?.into_iter().map(|r| r.0).collect();
        let mut seen = HashSet::new();
        for a in numbers {
            if !squad.contains(&a.id) {
                return Err(Error::Invalid("Só os jogadores do seu plantel.".into()));
            }
            if let Some(n) = a.number {
                if !(1..=99).contains(&n) {
                    return Err(Error::Invalid("Os números vão de 1 a 99.".into()));
                }
                if !seen.insert(n) {
                    return Err(Error::Invalid(format!("O número {n} está repetido.")));
                }
            }
        }
        // Os que não vêm na lista mantêm o número (se não chocar com um escolhido)
        for a in numbers {
            self.world.conn.execute("UPDATE players SET squad_number = ? WHERE person_id = ?", params![a.number, a.id])?;
        }
        fix_squad_numbers(&self.world, &[self.club_id], self.seed, Some(self.club_id))?;
        self.refresh_live()?;
        self.unsaved = true;
        self.squad_numbers()
    }

    /// Convocados sem número para o próximo jogo do treinador (amanhã): o jogo não começa
    /// enquanto houver algum.
    pub(super) fn unnumbered_for_next_match(&self) -> Result<Vec<i32>> {
        let tomorrow = crate::date::add_days(&self.date, 1).unwrap_or_default();
        let Some(m) = self
            .matches
            .iter()
            .find(|m| m.date == tomorrow && m.result.is_none() && (m.home_id == self.club_id || m.away_id == self.club_id))
        else {
            return Ok(Vec::new());
        };
        let (tactics, _) = self.lineup(self.club_id, m.competition_id)?;
        let mut ids: Vec<i32> = tactics
            .starters
            .iter()
            .filter_map(|s| s.player.as_ref())
            .chain(tactics.bench.iter())
            .filter(|p| p.squad_number.is_none())
            .map(|p| p.id)
            .collect();
        let numbered: HashSet<i32> = squad_rows(&self.world, self.club_id)?.into_iter().filter(|r| r.1.is_some()).map(|r| r.0).collect();
        ids.retain(|id| !numbered.contains(id));
        Ok(ids)
    }

    /// Época nova na liga do treinador: a mensagem para rever os números.
    pub(super) fn numbers_news(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let missing = squad_rows(&self.world, self.club_id)?.iter().filter(|r| r.1.is_none()).count();
        let extra = if missing > 0 { format!(" {missing} jogadores ainda não têm número.") } else { String::new() };
        let mut n = self.news_item(
            *next_id,
            "message",
            "Números do plantel".into(),
            format!("Defina os números de camisola do plantel para a nova época.{extra}"),
        );
        n.link = Some("squadNumbers".into());
        fresh.push(n);
        *next_id += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn shape(gk: i32, def: i32, dm: i32, mid: i32, am: i32, att: i32, right: i32, left: i32, central: i32) -> Shape {
        Shape { gk, def, dm, mid, am, att, wb: 1, right, left, central }
    }

    #[test]
    fn classic_numbers_by_position() {
        assert_eq!(classic_numbers(&shape(20, 1, 1, 1, 1, 1, 1, 1, 20))[0], 1);
        assert_eq!(classic_numbers(&shape(1, 20, 1, 1, 1, 1, 1, 1, 20))[0], 4);
        assert_eq!(classic_numbers(&shape(1, 20, 1, 1, 1, 1, 20, 1, 1))[0], 2);
        assert_eq!(classic_numbers(&shape(1, 1, 20, 1, 1, 1, 1, 1, 20))[0], 5);
        assert_eq!(classic_numbers(&shape(1, 1, 1, 1, 20, 1, 1, 1, 20))[0], 10);
        assert_eq!(classic_numbers(&shape(1, 1, 1, 1, 20, 18, 20, 1, 1))[0], 7);
        assert_eq!(classic_numbers(&shape(1, 1, 1, 1, 1, 20, 1, 1, 20))[0], 9);
    }

    #[test]
    fn a_match_needs_numbered_players() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-numeros-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Números".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
        };
        let mut g = Game::create(&path, &dir, &new, 2).unwrap();
        // O plantel do treinador começa todo numerado e sem repetidos
        let numbers = g.squad_numbers().unwrap();
        assert!(numbers.players.iter().all(|p| p.number.is_some()));
        let mut seen = HashSet::new();
        assert!(numbers.players.iter().all(|p| seen.insert(p.number)));
        // Na véspera do primeiro jogo, o melhor jogador fica sem número: o jogo não começa no app
        while g.unnumbered_for_next_match().unwrap().is_empty() && !g.info().unwrap().live {
            let tomorrow = crate::date::add_days(g.date(), 1).unwrap();
            if g.matches.iter().any(|m| m.date == tomorrow && (m.home_id == 7568 || m.away_id == 7568)) {
                let star = squad_rows(&g.world, 7568).unwrap()[0].0;
                g.world.conn.execute("UPDATE players SET squad_number = NULL, preferred_number = 10 WHERE person_id = ?", [star]).unwrap();
                break;
            }
            g.advance_day().unwrap();
        }
        assert!(g.info().unwrap().needs_numbers);
        g.set_watch(true);
        assert!(g.advance_day().is_err());
        // O Auto dá-lhe um número (o 10 se estiver livre) e o jogo segue
        let current: Vec<NumberAssign> =
            g.squad_numbers().unwrap().players.iter().map(|p| NumberAssign { id: p.id, number: p.number }).collect();
        let auto = g.auto_squad_numbers(&current).unwrap();
        assert!(auto.iter().all(|a| a.number.is_some()));
        g.set_squad_numbers(&auto).unwrap();
        assert!(!g.info().unwrap().needs_numbers);
        // Números repetidos são recusados
        let mut dup = auto.clone();
        dup[1].number = dup[0].number;
        assert!(g.set_squad_numbers(&dup).is_err());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn best_players_pick_first_and_keep_their_number() {
        let mut used = HashSet::from([1]);
        let mut rng = club_rng(1, 1);
        let missing = vec![
            (10, Some(7), shape(1, 1, 1, 1, 1, 20, 1, 1, 20)), // um avançado que usava o 7
            (11, None, shape(1, 1, 1, 1, 1, 20, 1, 1, 20)),    // outro avançado: o 9
            (12, None, shape(1, 1, 1, 1, 1, 20, 1, 1, 20)),    // e o seguinte: o 19
            (13, None, shape(20, 1, 1, 1, 1, 1, 1, 1, 20)),    // um GR com o 1 ocupado: o 12
        ];
        assert_eq!(assign(&missing, &mut used, &mut rng), vec![(10, 7), (11, 9), (12, 19), (13, 12)]);
    }
}
