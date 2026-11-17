//! A procura do editor de jogos salvos (como o Player Search do CM Scout): os filtros das abas
//! Geral, Contrato e Atributos, e as linhas da lista detalhada (jogadores, staff e clubes).

use std::path::Path;

use rusqlite::types::Value;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::editor::{EXTRA, EditorChoice, EditorNation, GOALKEEPING, MENTAL, OTHER, POSITIONS, TECHNICAL, open_save};
use crate::search::{AttributeMin, PersonKind};
use crate::{Error, Result, World};

/// Uma linha da lista do editor. Nos jogadores e no staff, tudo; nos clubes, `nation`,
/// `position` (a divisão) e `reputation`.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorHit {
    pub id: i32,
    pub name: String,
    pub club: Option<String>,
    pub ability: Option<i32>,
    pub potential: Option<i32>,
    pub nation: Option<String>,
    /// As siglas das posições ("MA/A DEC"); no staff, o cargo; nos clubes, a divisão.
    pub position: Option<String>,
    pub age: Option<i32>,
    /// Valor de mercado no dia do jogo (só jogadores).
    #[ts(type = "number | null")]
    pub value: Option<i64>,
    /// Salário semanal.
    #[ts(type = "number | null")]
    pub wage: Option<i64>,
    /// Fim do contrato, "AAAA-MM-DD".
    pub expires: Option<String>,
    /// Reputação atual (nos clubes, a do clube).
    pub reputation: Option<i32>,
    /// Listas e empréstimo: "Lista", "Pedido", "Lista emp.", "Emprestado" (separados por vírgula).
    pub status: String,
}

/// Os filtros da procura de jogadores; `None` (ou lista vazia) = sem filtro.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct EditorFilter {
    pub query: String,
    // Geral
    pub min_age: Option<i32>,
    pub max_age: Option<i32>,
    pub nation_id: Option<i32>,
    /// País do clube (o "Based" do CM).
    pub based_id: Option<i32>,
    pub division_id: Option<i32>,
    /// Chaves de [`POSITIONS`] (joga lá com 15 ou mais): das funções (GR, D, M...) basta uma, e
    /// dos lados (direita, esquerda, centro) também.
    pub positions: Vec<String>,
    pub min_ability: Option<i32>,
    pub max_ability: Option<i32>,
    pub min_potential: Option<i32>,
    pub max_potential: Option<i32>,
    pub min_home_reputation: Option<i32>,
    pub max_home_reputation: Option<i32>,
    /// Reputação atual.
    pub min_reputation: Option<i32>,
    pub max_reputation: Option<i32>,
    pub min_world_reputation: Option<i32>,
    pub max_world_reputation: Option<i32>,
    // Contrato
    /// "club" (com clube) ou "free" (sem clube).
    pub club_status: Option<String>,
    /// Ano em que o contrato acaba.
    pub min_expires: Option<i32>,
    pub max_expires: Option<i32>,
    #[ts(type = "number | null")]
    pub min_wage: Option<i64>,
    #[ts(type = "number | null")]
    pub max_wage: Option<i64>,
    #[ts(type = "number | null")]
    pub min_value: Option<i64>,
    #[ts(type = "number | null")]
    pub max_value: Option<i64>,
    /// "transfer", "loan", "requested" ou "none" (em lista nenhuma).
    pub listing: Option<String>,
    /// Função gravada no elenco (`squad_roles`); "none" = pelo lugar no elenco.
    pub role: Option<String>,
    /// Emprestados: só eles (`true`) ou sem eles (`false`).
    pub on_loan: Option<bool>,
    // Atributos 1 e 2
    /// O mínimo de cada atributo, como o jogo o mostra.
    pub attributes: Vec<AttributeMin>,
}

/// As listas dos filtros.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorOptions {
    pub nations: Vec<EditorNation>,
    pub divisions: Vec<EditorNation>,
    pub position_keys: Vec<String>,
    pub roles: Vec<EditorChoice>,
    /// Os atributos de jogo (aba Atributos 1) e os mentais (Atributos 2).
    pub attributes: Vec<String>,
    pub mental: Vec<String>,
}

/// Máximo de resultados de uma procura.
pub(crate) const SEARCH_LIMIT: usize = 500;

const SIDES: [&str; 3] = ["right_side", "left_side", "central"];

pub(crate) fn like_of(query: &str) -> String {
    let text = crate::search::fold(query.trim());
    format!("%{}%", text.replace('!', "!!").replace('%', "!%").replace('_', "!_"))
}

fn today(world: &World) -> Result<String> {
    Ok(world.conn.query_row("SELECT date FROM game", [], |r| r.get(0))?)
}

/// As linhas dos jogadores que cumprem `wheres` (os melhores primeiro); `value` corta pelo valor
/// e `attributes` pelos atributos mostrados, ambos calculados aqui.
fn player_rows(
    world: &World,
    wheres: &[String],
    args: Vec<Value>,
    value: (Option<i64>, Option<i64>),
    attributes: &[AttributeMin],
    limit: Option<usize>,
) -> Result<Vec<EditorHit>> {
    let today = today(world)?;
    let positions = POSITIONS.iter().map(|k| format!("pl.pos_{k}")).collect::<Vec<_>>().join(", ");
    let mins: Vec<&AttributeMin> = attributes.iter().filter(|a| a.min > 1).collect();
    // Com contas em Rust, o corte do número de linhas fica para depois
    let post = value.0.is_some() || value.1.is_some() || !mins.is_empty();
    let cut = match limit {
        Some(n) if !post => format!(" LIMIT {n}"),
        _ => String::new(),
    };
    let sql = format!(
        "SELECT p.id, p.display_name, c.name, pl.current_ability, pl.potential_ability, n.name, p.birth_date, p.birth_year,
                pl.current_reputation, p.contract_expires, COALESCE(k.reputation, 5), p.wage,
                COALESCE(tl.transfer, 0), COALESCE(tl.loan, 0), COALESCE(tl.requested, 0), lo.person_id IS NOT NULL, {positions}
         FROM people p JOIN players pl ON pl.person_id = p.id
         LEFT JOIN clubs c ON c.id = p.club_id LEFT JOIN nations n ON n.id = p.nation_id
         LEFT JOIN competitions k ON k.id = c.division_id
         LEFT JOIN transfer_list tl ON tl.person_id = p.id LEFT JOIN loans lo ON lo.person_id = p.id
         WHERE {} ORDER BY pl.current_ability DESC, p.display_name{cut}",
        if wheres.is_empty() { "1".into() } else { wheres.join(" AND ") }
    );
    let mut st = world.conn.prepare(&sql)?;
    let mut rows = st.query(rusqlite::params_from_iter(args))?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let (ca, pa): (Option<i32>, Option<i32>) = (r.get(3)?, r.get(4)?);
        let (birth, birth_year): (Option<String>, Option<i32>) = (r.get(6)?, r.get(7)?);
        let rep: Option<i32> = r.get(8)?;
        let expires: Option<String> = r.get(9)?;
        let v = crate::finance::value_on(
            ca.unwrap_or(1),
            pa.unwrap_or(0),
            birth.as_deref(),
            birth_year,
            rep.unwrap_or(0),
            expires.as_deref(),
            r.get(10)?,
            &today,
        );
        if value.0.is_some_and(|m| v < m) || value.1.is_some_and(|m| v > m) {
            continue;
        }
        let mut pos = [None; 12];
        for (i, slot) in pos.iter_mut().enumerate() {
            *slot = r.get(16 + i)?;
        }
        let status: Vec<&str> = [(12, "Lista"), (14, "Pedido"), (13, "Lista emp."), (15, "Emprestado")]
            .into_iter()
            .filter_map(|(i, label)| r.get::<_, bool>(i).ok().filter(|b| *b).map(|_| label))
            .collect();
        out.push(EditorHit {
            id: r.get(0)?,
            name: r.get(1)?,
            club: r.get(2)?,
            ability: ca,
            potential: pa,
            nation: r.get(5)?,
            position: Some(crate::search::position_label(&pos)),
            age: (birth.is_some() || birth_year.is_some()).then(|| crate::game::age_on(birth.as_deref(), birth_year, &today)),
            value: Some(v),
            wage: r.get(11)?,
            expires,
            reputation: rep,
            status: status.join(", "),
        });
    }
    drop(rows);
    if !mins.is_empty() && !out.is_empty() {
        let ids = format!("[{}]", out.iter().map(|h| h.id.to_string()).collect::<Vec<_>>().join(","));
        let args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(ids)];
        let keep: std::collections::HashSet<i32> = world
            .match_attributes(PersonKind::Players, &mins, None, "WHERE p.id IN (SELECT value FROM json_each(?))", &args)?
            .into_iter()
            .collect();
        out.retain(|h| keep.contains(&h.id));
    }
    if let Some(n) = limit {
        out.truncate(n);
    }
    Ok(out)
}

/// Procura jogadores num save fechado pelo nome e pelos filtros (os melhores primeiro).
pub fn search(saves: &Path, file: &str, f: &EditorFilter) -> Result<Vec<EditorHit>> {
    let world = open_save(saves, file)?;
    let mut wheres = vec!["p.search_name LIKE ? ESCAPE '!'".to_string()];
    let mut args: Vec<Value> = vec![like_of(&f.query).into()];
    let mut range = |expr: &str, min: Option<i64>, max: Option<i64>| {
        for (op, v) in [(">=", min), ("<=", max)] {
            if let Some(v) = v {
                wheres.push(format!("{expr} {op} ?"));
                args.push(v.into());
            }
        }
    };
    let wide = |v: Option<i32>| v.map(i64::from);
    range("COALESCE(pl.current_ability, 0)", wide(f.min_ability), wide(f.max_ability));
    range("COALESCE(pl.potential_ability, 0)", wide(f.min_potential), wide(f.max_potential));
    range("COALESCE(pl.home_reputation, 0)", wide(f.min_home_reputation), wide(f.max_home_reputation));
    range("COALESCE(pl.current_reputation, 0)", wide(f.min_reputation), wide(f.max_reputation));
    range("COALESCE(pl.world_reputation, 0)", wide(f.min_world_reputation), wide(f.max_world_reputation));
    // A idade no dia do jogo (sem data de nascimento, fica de fora)
    range("CAST((julianday((SELECT date FROM game)) - julianday(p.birth_sort)) / 365.25 AS INTEGER)", wide(f.min_age), wide(f.max_age));
    range("CAST(substr(p.contract_expires, 1, 4) AS INTEGER)", wide(f.min_expires), wide(f.max_expires));
    range("COALESCE(p.wage, 0)", f.min_wage, f.max_wage);
    for (expr, v) in [("p.nation_id", f.nation_id), ("c.nation_id", f.based_id), ("c.division_id", f.division_id)] {
        if let Some(v) = v {
            wheres.push(format!("{expr} = ?"));
            args.push(i64::from(v).into());
        }
    }
    if let Some(bad) = f.positions.iter().find(|p| !POSITIONS.contains(&p.as_str())) {
        return Err(Error::Invalid(format!("Posição desconhecida: {bad}.")));
    }
    for sides in [false, true] {
        let any: Vec<String> =
            f.positions.iter().filter(|p| SIDES.contains(&p.as_str()) == sides).map(|p| format!("COALESCE(pl.pos_{p}, 1) >= 15")).collect();
        if !any.is_empty() {
            wheres.push(format!("({})", any.join(" OR ")));
        }
    }
    match f.club_status.as_deref() {
        None => {}
        Some("club") => wheres.push("p.club_id IS NOT NULL".into()),
        Some("free") => wheres.push("p.club_id IS NULL AND p.id NOT IN (SELECT person_id FROM retired)".into()),
        Some(other) => return Err(Error::Invalid(format!("Situação desconhecida: {other}."))),
    }
    match f.listing.as_deref() {
        None => {}
        Some("transfer") => wheres.push("tl.transfer = 1".into()),
        Some("loan") => wheres.push("tl.loan = 1".into()),
        Some("requested") => wheres.push("tl.requested = 1".into()),
        Some("none") => wheres.push("COALESCE(tl.transfer, 0) = 0 AND COALESCE(tl.loan, 0) = 0 AND COALESCE(tl.requested, 0) = 0".into()),
        Some(other) => return Err(Error::Invalid(format!("Lista desconhecida: {other}."))),
    }
    match f.role.as_deref() {
        None => {}
        Some("none") => wheres.push("p.id NOT IN (SELECT person_id FROM squad_roles)".into()),
        Some(role) => {
            if !role_keys().any(|k| k == role) {
                return Err(Error::Invalid(format!("Função desconhecida: {role}.")));
            }
            wheres.push("p.id IN (SELECT person_id FROM squad_roles WHERE role = ?)".into());
            args.push(role.to_string().into());
        }
    }
    if let Some(loaned) = f.on_loan {
        wheres.push(format!("lo.person_id IS {}NULL", if loaned { "NOT " } else { "" }));
    }
    player_rows(&world, &wheres, args, (f.min_value, f.max_value), &f.attributes, Some(SEARCH_LIMIT))
}

fn role_keys() -> impl Iterator<Item = &'static str> {
    crate::game::deals::SQUAD_ROLES.iter().map(|r| r.0).chain([crate::game::deals::NOT_NEEDED.0])
}

/// As listas dos filtros da procura.
pub fn options(saves: &Path, file: &str) -> Result<EditorOptions> {
    let world = open_save(saves, file)?;
    let list = |sql: &str| -> Result<Vec<EditorNation>> {
        Ok(world
            .conn
            .prepare(sql)?
            .query_map([], |r| Ok(EditorNation { id: r.get(0)?, name: r.get(1)? }))?
            .collect::<rusqlite::Result<_>>()?)
    };
    let label = |key: &str| -> String {
        crate::game::deals::SQUAD_ROLES
            .iter()
            .map(|r| (r.0, r.1))
            .chain([crate::game::deals::NOT_NEEDED])
            .find(|r| r.0 == key)
            .map_or_else(|| key.to_string(), |r| r.1.to_string())
    };
    Ok(EditorOptions {
        nations: list("SELECT id, name FROM nations WHERE name IS NOT NULL ORDER BY name")?,
        divisions: list(
            "SELECT d.id, d.name || COALESCE(' (' || n.name || ')', '') FROM competitions d LEFT JOIN nations n ON n.id = d.nation_id
             WHERE d.id IN (SELECT division_id FROM clubs WHERE is_national_team = 0) AND d.name IS NOT NULL
             ORDER BY n.name, d.reputation DESC, d.name",
        )?,
        position_keys: POSITIONS.iter().map(|p| p.to_string()).collect(),
        roles: role_keys().map(|k| EditorChoice { key: k.to_string(), label: label(k) }).collect(),
        attributes: TECHNICAL.iter().chain(&OTHER).chain(&EXTRA).chain(&GOALKEEPING).map(|k| k.to_string()).collect(),
        mental: MENTAL.iter().map(|k| k.to_string()).collect(),
    })
}

/// Procura clubes pelo nome num save fechado (os de maior reputação primeiro).
pub fn search_clubs(saves: &Path, file: &str, query: &str) -> Result<Vec<EditorHit>> {
    let world = open_save(saves, file)?;
    let text = query.trim().to_lowercase();
    let mut st = world.conn.prepare(&format!(
        "SELECT c.id, c.name, n.name, d.name, c.reputation FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id
         LEFT JOIN competitions d ON d.id = c.division_id
         WHERE c.is_national_team = 0 AND LOWER(c.name) LIKE ? ESCAPE '!' ORDER BY c.reputation DESC, c.name LIMIT {SEARCH_LIMIT}"
    ))?;
    let like = format!("%{}%", text.replace('!', "!!").replace('%', "!%").replace('_', "!_"));
    let rows = st
        .query_map([like], |r| {
            Ok(EditorHit {
                id: r.get(0)?,
                name: r.get(1)?,
                nation: r.get(2)?,
                position: r.get(3)?,
                reputation: r.get(4)?,
                ..Default::default()
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Todos os jogadores de um clube (o elenco completo, melhores primeiro).
pub fn squad(saves: &Path, file: &str, club_id: i32) -> Result<Vec<EditorHit>> {
    let world = open_save(saves, file)?;
    player_rows(&world, &["p.club_id = ?".into()], vec![i64::from(club_id).into()], (None, None), &[], None)
}
