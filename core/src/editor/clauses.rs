//! A caixa Cláusulas da aba Contrato do editor: os prêmios e cláusulas do contrato
//! (`contract_extras`) e as da transferência (venda futura, valor por jogos e por jogos pela
//! seleção), todos editáveis.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::game::ContractExtras;
use crate::{Error, Result};

/// Uma cláusula da transferência: o clube que recebe, o valor (na venda futura, a percentagem)
/// e os jogos que faltam (só nos valores por jogos).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorTransferClause {
    pub club_id: i32,
    /// Só para mostrar (ignorado ao gravar).
    #[serde(default)]
    pub club_name: Option<String>,
    #[ts(type = "number")]
    pub amount: i64,
    pub needed: i32,
}

/// Tudo o que o contrato tem a mais (`None` = não tem).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorClauses {
    pub extras: ContractExtras,
    pub sell_on: Option<EditorTransferClause>,
    pub apps_bonus: Option<EditorTransferClause>,
    pub caps_bonus: Option<EditorTransferClause>,
}

fn has_table(conn: &Connection, name: &str) -> Result<bool> {
    Ok(conn.query_row("SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?", [name], |r| r.get(0))?)
}

/// Uma cláusula da transferência (`table` é uma das três, nunca vem de fora).
fn transfer_clause(conn: &Connection, table: &str, value: &str, needed: &str, id: i32) -> Result<Option<EditorTransferClause>> {
    if !has_table(conn, table)? {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            &format!(
                "SELECT t.payee, c.name, t.{value}, {needed} FROM {table} t LEFT JOIN clubs c ON c.id = t.payee WHERE t.person_id = ?"
            ),
            [id],
            |r| Ok(EditorTransferClause { club_id: r.get(0)?, club_name: r.get(1)?, amount: r.get(2)?, needed: r.get(3)? }),
        )
        .optional()?)
}

pub(super) fn read(conn: &Connection, id: i32) -> Result<EditorClauses> {
    Ok(EditorClauses {
        extras: crate::game::extras_of(conn, id)?.unwrap_or_default(),
        sell_on: transfer_clause(conn, "sell_on", "pct", "0", id)?,
        apps_bonus: transfer_clause(conn, "apps_bonus", "amount", "t.needed", id)?,
        caps_bonus: transfer_clause(conn, "caps_bonus", "amount", "t.needed", id)?,
    })
}

const MAX_MONEY: i64 = 1_000_000_000;

fn check_money(label: &str, v: i64) -> Result<()> {
    if (0..=MAX_MONEY).contains(&v) { Ok(()) } else { Err(Error::Invalid(format!("{label}: o valor vai de 0 a {MAX_MONEY}."))) }
}

/// Grava as cláusulas do jogador `id`, que joga em `club` (sem clube, não há contrato).
pub(super) fn write(conn: &Connection, id: i32, club: Option<i32>, c: &EditorClauses) -> Result<()> {
    let e = &c.extras;
    check_money("Prêmio por gol", e.goal_bonus)?;
    check_money("Prêmio por assistência", e.assist_bonus)?;
    check_money("Prêmio por jogo sem sofrer gols", e.clean_sheet_bonus)?;
    if let Some(f) = e.min_fee {
        check_money("Cláusula de venda", f)?;
    }
    if let Some(s) = &c.sell_on
        && !(1..=crate::game::MAX_SELL_ON as i64).contains(&s.amount)
    {
        return Err(Error::Invalid(format!("Venda futura: a porcentagem vai de 1 a {}.", crate::game::MAX_SELL_ON)));
    }
    for (label, b) in [("Valor por jogos", &c.apps_bonus), ("Valor por jogos pela seleção", &c.caps_bonus)] {
        if let Some(b) = b {
            check_money(label, b.amount)?;
            super::check_range(&format!("{label}: jogos"), b.needed, 1, 500)?;
        }
    }
    let any = *e != ContractExtras::default() || c.sell_on.is_some() || c.apps_bonus.is_some() || c.caps_bonus.is_some();
    let Some(club) = club else {
        return if any { Err(Error::Invalid("Sem clube, o jogador não tem cláusulas.".into())) } else { Ok(()) };
    };
    for b in [&c.sell_on, &c.apps_bonus, &c.caps_bonus].into_iter().flatten() {
        if b.club_id == club {
            return Err(Error::Invalid("O clube que recebe a cláusula precisa ser diferente do clube onde ele joga.".into()));
        }
        let ok: bool = conn.query_row("SELECT COUNT(*) > 0 FROM clubs WHERE id = ?", [b.club_id], |r| r.get(0))?;
        if !ok {
            return Err(Error::Invalid("Cláusula: clube desconhecido.".into()));
        }
    }

    if has_table(conn, "contract_extras")? {
        if *e == ContractExtras::default() {
            conn.execute("DELETE FROM contract_extras WHERE person_id = ?", [id])?;
        } else {
            let text = serde_json::to_string(e).unwrap_or_default();
            conn.execute("INSERT OR REPLACE INTO contract_extras (person_id, club_id, extras) VALUES (?, ?, ?)", params![id, club, text])?;
        }
    } else if *e != ContractExtras::default() {
        return Err(Error::Invalid("Este save é antigo demais para prêmios e cláusulas do contrato.".into()));
    }
    for t in ["sell_on", "apps_bonus", "caps_bonus"] {
        if !has_table(conn, t)? {
            let wanted = match t {
                "sell_on" => c.sell_on.is_some(),
                "apps_bonus" => c.apps_bonus.is_some(),
                _ => c.caps_bonus.is_some(),
            };
            if wanted {
                return Err(Error::Invalid("Este save é antigo demais para as cláusulas da transferência.".into()));
            }
        }
    }
    if has_table(conn, "sell_on")? {
        match &c.sell_on {
            Some(s) => conn.execute(
                "INSERT OR REPLACE INTO sell_on (person_id, payer, payee, pct) VALUES (?, ?, ?, ?)",
                params![id, club, s.club_id, s.amount],
            )?,
            None => conn.execute("DELETE FROM sell_on WHERE person_id = ?", [id])?,
        };
    }
    // Os jogos contam a partir de agora: o ponto de partida que já havia fica
    if has_table(conn, "apps_bonus")? {
        match &c.apps_bonus {
            Some(b) => {
                let now: i32 =
                    conn.query_row("SELECT COALESCE((SELECT apps FROM player_state WHERE person_id = ?), 0)", [id], |r| r.get(0))?;
                conn.execute(
                    "INSERT INTO apps_bonus (person_id, payer, payee, amount, apps_at, needed) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(person_id) DO UPDATE SET payer = ?2, payee = ?3, amount = ?4, needed = ?6",
                    params![id, club, b.club_id, b.amount, now, b.needed],
                )?
            }
            None => conn.execute("DELETE FROM apps_bonus WHERE person_id = ?", [id])?,
        };
    }
    if has_table(conn, "caps_bonus")? {
        match &c.caps_bonus {
            Some(b) => {
                let now: i32 = conn.query_row("SELECT COALESCE(international_apps, 0) FROM people WHERE id = ?", [id], |r| r.get(0))?;
                conn.execute(
                    "INSERT INTO caps_bonus (person_id, payer, payee, amount, caps_at, needed) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(person_id) DO UPDATE SET payer = ?2, payee = ?3, amount = ?4, needed = ?6",
                    params![id, club, b.club_id, b.amount, now, b.needed],
                )?
            }
            None => conn.execute("DELETE FROM caps_bonus WHERE person_id = ?", [id])?,
        };
    }
    Ok(())
}
