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
        "notneeded" => "Dispensável",
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
    /// Os jogadores-chave e, para o Vêr das Tácticas, os golos, a forma e a nota média da época.
    pub(super) fn mark_key_players(&self, t: &mut ClubTactics) -> Result<()> {
        let keys = self.key_players(t.club_id)?;
        let start = self.season_start(Some(t.club_id))?;
        let players = t.starters.iter_mut().filter_map(|s| s.player.as_mut()).chain(&mut t.bench).chain(&mut t.reserves);
        for p in players {
            p.key = keys.contains(&p.id);
            p.form = self.form_of(p.id)?;
            let (goals, rating): (i32, Option<f64>) = self
                .world
                .conn
                .prepare_cached("SELECT COALESCE(SUM(goals), 0), AVG(rating) FROM player_matches WHERE person_id = ? AND date >= ?")?
                .query_row(params![p.id, start], |r| Ok((r.get(0)?, r.get(1)?)))?;
            p.goals = goals;
            p.rating = rating.map(|r| (r * 100.0).round() / 100.0);
        }
        Ok(())
    }

    /// Completa o plantel de um clube com o estado no jogo em curso.
    pub fn fill_squad(&self, c: &mut ClubProfile) -> Result<()> {
        let id = c.club.id;
        let mine = id == self.club_id;
        let keys = if c.is_national_team { Vec::new() } else { self.key_players(id)? };
        let start = self.season_start((!c.is_national_team).then_some(id))?;
        // Os emprestados a outros clubes continuam na lista do dono, a azul, como no CM
        if !c.is_national_team {
            let mut out = self.world.squad_players("p.id IN (SELECT person_id FROM loans WHERE parent_id = ? AND person_id NOT IN (SELECT id FROM people WHERE club_id = loans.parent_id))", id)?;
            for p in &mut out {
                p.loaned_out = true;
            }
            c.players.extend(out);
        }
        let conn = &self.world.conn;
        let away: std::collections::HashSet<i32> = self.away_internationals().into_iter().collect();
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
            let unhappy = conn
                .query_row("SELECT 1 FROM unhappy WHERE person_id = ? AND substr(reason, 1, 5) <> 'club_' LIMIT 1", [p.id], |_| Ok(()))
                .optional()?
                .is_some();
            let future =
                conn.query_row("SELECT 1 FROM unhappy WHERE person_id = ? AND reason = 'future'", [p.id], |_| Ok(())).optional()?.is_some();
            let absence = if mine { self.absence(p.id)?.map(|a| a.0) } else { None };
            // Castigado pelo player_setup.cfg (má conduta, passaporte falso, doping)
            p.suspended |= self.ban_of(p.id)?.is_some();
            let wanted = self.wanted(p.id)?;
            p.info = if p.injured {
                Some("Les")
            } else if p.suspended {
                Some("Sus")
            } else if away.contains(&p.id) {
                // Fora na selecção
                Some("Int")
            } else if absence.as_deref() == Some("awol") {
                // Ausente sem permissão (faltou com saudades de casa), como o "Fit" do CM
                Some("Fit")
            } else if absence.is_some() {
                Some("Fol")
            } else if future {
                // Nas reservas com 24 anos: quer ver o futuro esclarecido
                Some("Fut")
            } else if p.loaned {
                Some("Emp")
            } else if conn
                .query_row("SELECT 1 FROM offers WHERE person_id = ? AND status = 'waiting' LIMIT 1", [p.id], |_| Ok(()))
                .optional()?
                .is_some()
            {
                // Transferência acertada, à espera da janela para sair
                Some("Trn")
            } else if p.contract_expires.as_deref().is_some_and(|e| e < self.date.as_str()) {
                // Contrato acabado, mas ainda no clube (até se tratar no fim da época)
                Some("Ctr")
            } else if listed {
                Some("Lst")
            } else if wanted {
                Some("Des")
            } else if unhappy {
                Some("Ins")
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
            // Os prémios e cláusulas do contrato (só nos contratos do treinador)
            if let Some(e) = self.contract_extras(p.id)? {
                clauses.extend(e.labels());
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

/// Uma linha do Vêr Mais Dados do plantel ("Estatísticas - Jogos Oficiais" no CM).
#[derive(Debug, Clone, Default, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SquadMore {
    pub person: i32,
    pub apps: i32,
    /// Golos sofridos a jogar à baliza.
    pub conceded: i32,
    pub yellows: i32,
    pub reds: i32,
    pub wins: i32,
    pub losses: i32,
    /// Golos marcados e sofridos pela equipa nos jogos em que entrou.
    pub team_for: i32,
    pub team_against: i32,
    pub rating: Option<f64>,
}

/// Os atributos de um jogador do plantel como o perfil os mostra (`None` = o clube não sabe).
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SquadAttributes {
    pub person: i32,
    pub attributes: crate::person::Attributes,
}

impl Game {
    /// O Vêr Mais Dados: por jogador, nesta época, nos jogos oficiais (`kind` vazio) ou só
    /// numa "Liga", "Taça" ou "Continentais".
    pub fn squad_more(&self, club: i32, kind: &str) -> Result<Vec<SquadMore>> {
        let start = self.season_start(Some(club))?;
        let rows: Vec<(i32, i32, i32, i32, Option<i32>, f64)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT pm.person_id, pm.fixture, pm.competition, pm.club, pm.cards, pm.rating
                 FROM player_matches pm JOIN people p ON p.id = pm.person_id
                 WHERE pm.date >= ? AND (p.club_id = ? OR p.id IN (SELECT person_id FROM loans WHERE parent_id = ?))",
            )?
            .query_map(params![start, club, club], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let keepers: std::collections::HashSet<i32> = self
            .world
            .conn
            .prepare_cached("SELECT person_id FROM players WHERE COALESCE(pos_goalkeeper, 0) >= 15")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out: Vec<SquadMore> = Vec::new();
        let mut ratings: std::collections::HashMap<i32, (f64, i32)> = std::collections::HashMap::new();
        for (person, fixture, comp, side_club, cards, rating) in rows {
            if !kind.is_empty() && self.comp_kind(comp) != kind {
                continue;
            }
            let Some(m) = self.matches.iter().find(|m| m.id == fixture) else { continue };
            let Some((h, a)) = m.result else { continue };
            let (ours, theirs) = if m.home_id == side_club { (h as i32, a as i32) } else { (a as i32, h as i32) };
            let row = match out.iter_mut().position(|r| r.person == person) {
                Some(i) => &mut out[i],
                None => {
                    out.push(SquadMore { person, ..Default::default() });
                    out.last_mut().unwrap()
                }
            };
            row.apps += 1;
            match cards.unwrap_or(0) {
                2 => row.reds += 1,
                1 => row.yellows += 1,
                _ => {}
            }
            row.wins += (ours > theirs) as i32;
            row.losses += (ours < theirs) as i32;
            row.team_for += ours;
            row.team_against += theirs;
            if keepers.contains(&person) {
                row.conceded += theirs;
            }
            let r = ratings.entry(person).or_default();
            *r = (r.0 + rating, r.1 + 1);
        }
        for row in &mut out {
            row.rating = ratings.get(&row.person).map(|(sum, n)| (sum / *n as f64 * 100.0).round() / 100.0);
        }
        Ok(out)
    }

    /// O Vêr Atributos do plantel e dos Treinos: os atributos de cada jogador, tapados como no
    /// perfil quando o clube não o conhece.
    pub fn squad_attributes(&self, club: i32) -> Result<Vec<SquadAttributes>> {
        let ids: Vec<i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT id FROM people WHERE is_player = 1 AND (club_id = ? OR id IN (SELECT person_id FROM loans WHERE parent_id = ?))",
            )?
            .query_map(params![club, club], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let mut p = self.world.get_person(id)?;
            self.mask_profile(&mut p)?;
            if let Some(pl) = p.player {
                out.push(SquadAttributes { person: id, attributes: pl.attributes });
            }
        }
        Ok(out)
    }
}
