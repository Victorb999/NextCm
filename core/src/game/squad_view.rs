//! O Plantel como no CM (marco 3.16): o que as vistas do Vêr mostram de cada jogador num
//! jogo em curso (Situação, contrato, Forma, Moral, a época, o valor), a sigla da coluna Inf,
//! os jogadores-chave com `*` e a ordem dos batedores de penáltis.

use rusqlite::{OptionalExtension, params};

use super::Game;
use super::deals::PROSPECT_AGE;
use crate::club::ClubProfile;
use crate::tactics::ClubTactics;
use crate::{Result, live};

/// Jogadores-chave por plantel (o `*` do CM).
const KEY_PLAYERS: usize = 3;

/// A Situação do contrato ("Informação sobre o Contrato" do CM), pela função gravada.
fn role_label(role: &str) -> &'static str {
    match role {
        "key" | "important" => "Importante",
        "rotation" => "Rotação",
        "prospect" | "youngster" => "Aposta",
        _ => "Reserva",
    }
}

impl Game {
    /// Os jogadores-chave de um clube: os de maior valor (sem os emprestados por outros).
    pub(super) fn key_players(&self, club: i32) -> Result<Vec<i32>> {
        let ids: Vec<i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id FROM people p WHERE p.club_id = ? AND p.is_player = 1
                 AND p.id NOT IN (SELECT person_id FROM loans WHERE parent_id <> ?) ORDER BY p.id",
            )?
            .query_map([club, club], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut valued: Vec<(i64, i32)> = Vec::with_capacity(ids.len());
        for id in ids {
            valued.push((self.world.market_value_of(id, &self.date)?.unwrap_or(0), id));
        }
        valued.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        Ok(valued.into_iter().take(KEY_PLAYERS).map(|v| v.1).collect())
    }

    /// O `*` dos jogadores-chave também nas Tácticas.
    pub(super) fn mark_key_players(&self, t: &mut ClubTactics) -> Result<()> {
        let keys = self.key_players(t.club_id)?;
        let players = t.starters.iter_mut().filter_map(|s| s.player.as_mut()).chain(&mut t.bench).chain(&mut t.reserves);
        for p in players {
            p.key = keys.contains(&p.id);
        }
        Ok(())
    }

    /// Completa o plantel de um clube com o estado no jogo em curso.
    pub fn fill_squad(&self, c: &mut ClubProfile) -> Result<()> {
        let id = c.club.id;
        let mine = id == self.club_id;
        let keys = if c.is_national_team { Vec::new() } else { self.key_players(id)? };
        let start = self.season_start((!c.is_national_team).then_some(id))?;
        let conn = &self.world.conn;
        for p in &mut c.players {
            p.ghost = self.is_ghost(p.id);
            if let Some(st) = self.player_status(p.id) {
                p.condition = Some(st.condition);
                p.injured = st.injury.is_some();
                p.suspended = st.suspended > 0;
                p.reserve = st.reserve;
                p.morale = Some(st.morale_label);
            }
            if !c.is_national_team {
                p.key = keys.contains(&p.id);
            }
            let loan: Option<i32> = conn.query_row("SELECT parent_id FROM loans WHERE person_id = ?", [p.id], |r| r.get(0)).optional()?;
            p.loaned = loan.is_some_and(|l| l != id);
            p.free = p.contract_expires.as_deref().is_none_or(|e| e < self.date.as_str());
            let listed: bool =
                conn.query_row("SELECT transfer FROM transfer_list WHERE person_id = ?", [p.id], |r| r.get(0)).optional()?.unwrap_or(false);
            let unhappy = conn.query_row("SELECT 1 FROM unhappy WHERE person_id = ? LIMIT 1", [p.id], |_| Ok(())).optional()?.is_some();
            p.info = if p.injured {
                Some("Les")
            } else if p.suspended {
                Some("Sus")
            } else if p.loaned {
                Some("Emp")
            } else if listed {
                Some("Lst")
            } else if unhappy {
                Some("Des")
            } else if p.international {
                Some("Int")
            } else {
                None
            }
            .map(Into::into);
            // Situação: a função do contrato (gravada no plantel do treinador), senão pelo lugar no plantel
            let role: Option<String> = if mine {
                conn.query_row("SELECT role FROM squad_roles WHERE person_id = ?", [p.id], |r| r.get(0)).optional()?
            } else {
                None
            };
            p.role = Some(
                match role {
                    _ if listed => "Dispensável",
                    Some(r) => role_label(&r),
                    None if p.ability_rank <= 11 => "Importante",
                    None if p.ability_rank <= 16 => "Rotação",
                    None if p.age.is_some_and(|a| a <= PROSPECT_AGE) => "Aposta",
                    None => "Reserva",
                }
                .into(),
            );
            let mut clauses = Vec::new();
            if let Some(pct) = conn.query_row("SELECT pct FROM sell_on WHERE person_id = ?", [p.id], |r| r.get::<_, i32>(0)).optional()? {
                clauses.push(format!("{pct}% venda futura"));
            }
            if conn.query_row("SELECT 1 FROM apps_bonus WHERE person_id = ?", [p.id], |_| Ok(())).optional()?.is_some() {
                clauses.push("Prémio por jogos".into());
            }
            p.clauses = clauses;
            p.value = if p.free { None } else { self.world.market_value_of(p.id, &self.date)? };
            p.form = self.form_of(p.id)?;
            let (apps, subs, goals, assists, mom, rating): (i32, i32, i32, i32, i32, Option<f64>) = conn
                .prepare_cached(
                    "SELECT COUNT(*), COALESCE(SUM(sub), 0), COALESCE(SUM(goals), 0), COALESCE(SUM(assists), 0),
                            COALESCE(SUM(mom), 0), AVG(rating)
                     FROM player_matches WHERE person_id = ? AND date >= ?",
                )?
                .query_row(params![p.id, start], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
            (p.apps, p.subs, p.goals, p.assists, p.mom) = (apps, subs, goals, assists, mom);
            p.rating = rating.map(|r| (r * 100.0).round() / 100.0);
        }
        self.order_penalty_takers(c)
    }

    /// Marcadores Penalties: o escolhido nas Tácticas primeiro, depois pelo jeito, como o
    /// adjunto escolhe no jogo; os guarda-redes ficam de fora.
    fn order_penalty_takers(&self, c: &mut ClubProfile) -> Result<()> {
        let chosen = if c.club.id == self.club_id { self.selection.as_ref().and_then(|s| s.takers.penalty) } else { None };
        let mut skill = Vec::new();
        for (i, p) in c.players.iter().enumerate() {
            if p.lines.iter().any(|l| l == "GR") {
                continue;
            }
            let first = Some(p.id) == chosen;
            skill.push((first, live::penalty_skill(&self.world, p.id, &self.date)?, p.id, i));
        }
        skill.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.total_cmp(&a.1)).then(a.2.cmp(&b.2)));
        for (order, (.., i)) in skill.into_iter().enumerate() {
            c.players[i].penalty_order = Some(order as i32 + 1);
        }
        Ok(())
    }
}
