//! Editor de jogos salvos: o staff (marco 4.0, item 16). Os não jogadores (treinadores,
//! adjuntos, olheiros, fisioterapeutas, directores) com os 21 atributos de staff e os 8 mentais,
//! CA/PA, as reputações, o contrato e o clube com a função. Como nos jogadores, edita-se o valor
//! que o jogo mostra (1 a 20); os 8 relativos (`STAFF_RELATIVE`) são convertidos aqui.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::attributes::{STAFF_RELATIVE, show_staff_attributes};
use crate::editor::{EditorAttribute, EditorHit, EditorValue, MENTAL, check_range, open_save};
use crate::person::Attributes;
use crate::{Error, Result, World};

/// Os atributos de staff do banco (tabela `non_players`), na ordem do editor.
const STAFF: [&str; 21] = [
    "attacking",
    "business",
    "coaching",
    "coaching_gks",
    "coaching_technique",
    "directness",
    "discipline",
    "free_roles",
    "interference",
    "judgement",
    "judging_potential",
    "man_handling",
    "marking",
    "motivating",
    "offside",
    "patience",
    "physiotherapy",
    "pressing",
    "resources",
    "tactics",
    "youngsters",
];

/// As funções de staff num clube (tabela `jobs`): as de jogador ficam de fora.
const STAFF_JOBS: [i32; 11] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 15];
/// Treinador principal: um por clube.
const MANAGER: i32 = 5;

/// Uma função para a lista de escolha.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorJob {
    pub id: i32,
    pub label: String,
}

/// Um membro do staff aberto no editor.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorStaff {
    pub id: i32,
    pub name: String,
    pub club: Option<String>,
    /// O que se edita (aba Dados do staff).
    pub data: EditorStaffData,
    /// Os 21 de staff (grupo "staff") e os 8 mentais (grupo "mental").
    pub attributes: Vec<EditorAttribute>,
    pub jobs: Vec<EditorJob>,
}

/// A ficha do staff: CA/PA, reputações, contrato, clube e função.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorStaffData {
    pub ability: i32,
    pub potential: i32,
    pub home_reputation: i32,
    pub current_reputation: i32,
    pub world_reputation: i32,
    /// Sem clube: livre (tirá-lo do clube apaga a função, o salário e o contrato).
    pub club_id: Option<i32>,
    pub job_id: Option<i32>,
    /// Salário semanal.
    pub wage: i32,
    /// "AAAA-MM-DD"
    pub contract_expires: Option<String>,
}

/// Procura staff pelo nome num save fechado (até 40, os melhores primeiro).
pub fn search(saves: &Path, file: &str, query: &str) -> Result<Vec<EditorHit>> {
    let world = open_save(saves, file)?;
    let text = crate::search::fold(query.trim());
    let like = format!("%{}%", text.replace('!', "!!").replace('%', "!%").replace('_', "!_"));
    let mut st = world.conn.prepare(
        "SELECT p.id, p.display_name, c.name, np.current_ability FROM people p
         JOIN non_players np ON np.person_id = p.id LEFT JOIN clubs c ON c.id = p.club_id
         WHERE p.is_player = 0 AND p.search_name LIKE ? ESCAPE '!' ORDER BY np.current_ability DESC, p.display_name LIMIT 40",
    )?;
    let rows = st
        .query_map([like], |r| Ok(EditorHit { id: r.get(0)?, name: r.get(1)?, club: r.get(2)?, ability: r.get(3)? }))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Os valores do banco (staff e mentais) e a CA.
fn raw(world: &World, id: i32) -> Result<(Attributes, i32)> {
    let cols = STAFF.iter().map(|k| format!("np.{k}")).chain(MENTAL.iter().map(|k| format!("p.{k}"))).collect::<Vec<_>>().join(", ");
    world
        .conn
        .query_row(
            &format!(
                "SELECT COALESCE(np.current_ability, 0), {cols} FROM people p JOIN non_players np ON np.person_id = p.id
             WHERE p.id = ? AND p.is_player = 0"
            ),
            [id],
            |r| {
                let mut out = Attributes::new();
                for (i, k) in STAFF.iter().chain(&MENTAL).enumerate() {
                    out.insert(k.to_string(), r.get(i + 1)?);
                }
                Ok((out, r.get(0)?))
            },
        )
        .map_err(|_| Error::Invalid("Só se edita aqui o staff (não jogadores).".into()))
}

fn build(world: &World, id: i32) -> Result<EditorStaff> {
    let (stored, ca) = raw(world, id)?;
    let mut shown = stored.clone();
    show_staff_attributes(&mut shown, Some(ca));
    let mut attributes = Vec::new();
    for (k, group) in STAFF.iter().map(|k| (k, "staff")).chain(MENTAL.iter().map(|k| (k, "mental"))) {
        let Some(s) = stored.get(*k).copied().flatten() else { continue };
        attributes.push(EditorAttribute {
            key: k.to_string(),
            shown: shown.get(*k).copied().flatten().unwrap_or(s),
            stored: s,
            hidden: false,
            relative: STAFF_RELATIVE.contains(k),
            group: group.into(),
        });
    }
    let (name, club, data) = world.conn.query_row(
        "SELECT p.display_name, c.name, COALESCE(np.current_ability, 0), COALESCE(np.potential_ability, 0),
                COALESCE(np.home_reputation, 0), COALESCE(np.current_reputation, 0), COALESCE(np.world_reputation, 0),
                p.club_id, p.club_job, COALESCE(p.wage, 0), p.contract_expires
         FROM people p JOIN non_players np ON np.person_id = p.id LEFT JOIN clubs c ON c.id = p.club_id WHERE p.id = ?",
        [id],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                EditorStaffData {
                    ability: r.get(2)?,
                    potential: r.get(3)?,
                    home_reputation: r.get(4)?,
                    current_reputation: r.get(5)?,
                    world_reputation: r.get(6)?,
                    club_id: r.get(7)?,
                    job_id: r.get(8)?,
                    wage: r.get(9)?,
                    contract_expires: r.get(10)?,
                },
            ))
        },
    )?;
    let jobs = world
        .conn
        .prepare("SELECT id, name FROM jobs ORDER BY id")?
        .query_map([], |r| Ok(EditorJob { id: r.get(0)?, label: r.get(1)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|j| STAFF_JOBS.contains(&j.id))
        .collect();
    Ok(EditorStaff { id, name, club, data, attributes, jobs })
}

/// Um membro do staff de um save fechado.
pub fn staff(saves: &Path, file: &str, id: i32) -> Result<EditorStaff> {
    build(&open_save(saves, file)?, id)
}

/// O valor guardado que faz o jogo mostrar `target` num relativo do staff (o mais próximo).
fn stored_for_shown(ca: i32, key: &str, target: i32) -> i32 {
    let mut best = (i32::MAX, 10);
    for v in 1..=20 {
        let mut one = Attributes::new();
        one.insert(key.to_string(), Some(v));
        show_staff_attributes(&mut one, Some(ca));
        let shown = one.get(key).copied().flatten().unwrap_or(v);
        let score = (shown - target).abs();
        if score < best.0 {
            best = (score, v);
        }
    }
    best.1
}

/// Valida e grava a ficha e os atributos editados (valores de 1 a 20 como o jogo os mostra).
/// O treinador do jogo não muda de clube aqui, e um clube só tem um treinador principal.
pub fn set_staff(saves: &Path, file: &str, id: i32, data: &EditorStaffData, values: &[EditorValue]) -> Result<EditorStaff> {
    let world = open_save(saves, file)?;
    let before = build(&world, id)?;
    check_range("Capacidade actual (CA)", data.ability, 0, 200)?;
    check_range("Potencial (PA)", data.potential, 0, 200)?;
    if data.potential < data.ability {
        return Err(Error::Invalid("O potencial não pode ser menor que a capacidade actual.".into()));
    }
    for (label, v) in [
        ("Reputação local", data.home_reputation),
        ("Reputação actual", data.current_reputation),
        ("Reputação mundial", data.world_reputation),
    ] {
        check_range(label, v, 0, 10000)?;
    }
    check_range("Salário", data.wage, 0, 100_000_000)?;
    for v in values {
        check_range("Atributo", v.value, 1, 20)?;
        if !STAFF.contains(&v.key.as_str()) && !MENTAL.contains(&v.key.as_str()) {
            return Err(Error::Invalid(format!("Atributo desconhecido: {}.", v.key)));
        }
    }
    let expires = data.contract_expires.as_deref().filter(|s| !s.is_empty());
    if let Some(s) = expires
        && crate::date::parse(s).is_none()
    {
        return Err(Error::Invalid("Fim do contrato inválido (use AAAA-MM-DD).".into()));
    }
    let manager: i32 = world.conn.query_row("SELECT manager_id FROM game", [], |r| r.get(0))?;
    let moved = data.club_id != before.data.club_id || data.job_id != before.data.job_id;
    if id == manager && moved {
        return Err(Error::Invalid("O treinador do jogo muda de clube no jogo, não no editor.".into()));
    }
    let (club, job) = match data.club_id {
        Some(c) => {
            let ok: i64 = world.conn.query_row("SELECT COUNT(*) FROM clubs WHERE id = ?", [c], |r| r.get(0))?;
            if ok == 0 {
                return Err(Error::Invalid("Clube desconhecido.".into()));
            }
            let job = data.job_id.ok_or_else(|| Error::Invalid("Escolha a função no clube.".into()))?;
            if !STAFF_JOBS.contains(&job) {
                return Err(Error::Invalid("Função de staff desconhecida.".into()));
            }
            if job == MANAGER {
                let other: Option<String> = world
                    .conn
                    .query_row(
                        "SELECT display_name FROM people WHERE club_id = ? AND club_job = ? AND id <> ? LIMIT 1",
                        rusqlite::params![c, MANAGER, id],
                        |r| r.get(0),
                    )
                    .ok();
                if let Some(other) = other {
                    return Err(Error::Invalid(format!("Esse clube já tem treinador ({other}).")));
                }
            }
            (Some(c), Some(job))
        }
        // Já sem clube no save: fica como estava
        None if before.data.club_id.is_none() => (None, data.job_id),
        None => (None, None),
    };
    // Tirado do clube agora: livre, sem função, salário nem contrato
    let released = club.is_none() && before.data.club_id.is_some();
    let ca = data.ability;
    let tx = world.conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE non_players SET current_ability = ?, potential_ability = ?, home_reputation = ?, current_reputation = ?,
                world_reputation = ? WHERE person_id = ?",
        rusqlite::params![data.ability, data.potential, data.home_reputation, data.current_reputation, data.world_reputation, id],
    )?;
    // Ao entrar num clube, conta a data de hoje
    let today: String = tx.query_row("SELECT date FROM game", [], |r| r.get(0))?;
    let joined = if club != before.data.club_id && club.is_some() { Some(today) } else { None };
    tx.execute(
        "UPDATE people SET club_id = ?, club_job = ?, wage = ?, contract_expires = ?, ability = ?,
                club_joined = COALESCE(?, CASE WHEN ? THEN NULL ELSE club_joined END) WHERE id = ?",
        rusqlite::params![
            club,
            job,
            if released { 0 } else { data.wage },
            if released { None } else { expires },
            data.ability,
            joined,
            released,
            id
        ],
    )?;
    for v in values {
        if MENTAL.contains(&v.key.as_str()) {
            tx.execute(&format!("UPDATE people SET {} = ? WHERE id = ?", v.key), rusqlite::params![v.value, id])?;
        } else {
            let stored = if STAFF_RELATIVE.contains(&v.key.as_str()) && ca > 0 { stored_for_shown(ca, &v.key, v.value) } else { v.value };
            tx.execute(&format!("UPDATE non_players SET {} = ? WHERE person_id = ?", v.key), rusqlite::params![stored, id])?;
        }
    }
    tx.commit()?;
    build(&world, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Game;

    #[test]
    fn edit_staff_attributes_contract_and_club() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-editor-staff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Staff".into(),
            first_name: "A".into(),
            last_name: "B".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let g = Game::create(&path, &dir, &new, 7).unwrap();
        let file = g.path().file_name().unwrap().to_string_lossy().to_string();
        let manager = g.info().unwrap().manager_id;
        drop(g);

        let hits = search(&dir, &file, "").unwrap();
        assert!(!hits.is_empty());
        // Um adjunto com clube (função 6) e um clube com treinador
        let world = open_save(&dir, &file).unwrap();
        let (id, club): (i32, i32) = world
            .conn
            .query_row(
                "SELECT p.id, p.club_id FROM people p JOIN non_players np ON np.person_id = p.id
                 WHERE p.is_player = 0 AND p.club_job = 6 AND np.current_ability > 50 LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let other: i32 = world
            .conn
            .query_row("SELECT club_id FROM people WHERE club_job = 5 AND club_id <> ? AND id <> ? LIMIT 1", [club, manager], |r| r.get(0))
            .unwrap();
        drop(world);
        let s = staff(&dir, &file, id).unwrap();
        assert_eq!(s.attributes.len(), 29);
        // Um relativo pelo valor mostrado
        let after = set_staff(&dir, &file, id, &s.data, &[EditorValue { key: "tactics".into(), value: 15 }]).unwrap();
        let t = after.attributes.iter().find(|a| a.key == "tactics").unwrap();
        assert!((t.shown - 15).abs() <= 1, "mostra {}", t.shown);
        // Contrato e mudança de clube
        let moved = EditorStaffData {
            club_id: Some(other),
            job_id: Some(6),
            wage: 5000,
            contract_expires: Some("2028-06-30".into()),
            ..after.data.clone()
        };
        let after = set_staff(&dir, &file, id, &moved, &[]).unwrap();
        assert_eq!((after.data.club_id, after.data.job_id, after.data.wage), (Some(other), Some(6), 5000));
        // Um segundo treinador principal, não
        let manager_too = EditorStaffData { job_id: Some(MANAGER), ..after.data.clone() };
        assert!(set_staff(&dir, &file, id, &manager_too, &[]).is_err());
        // Livre: sem função, salário nem contrato
        let free = EditorStaffData { club_id: None, ..after.data.clone() };
        let after = set_staff(&dir, &file, id, &free, &[]).unwrap();
        assert_eq!((after.data.job_id, after.data.wage, after.data.contract_expires), (None, 0, None));
        // O treinador do jogo não muda de clube no editor
        let mine = staff(&dir, &file, manager);
        if let Ok(m) = mine {
            let away = EditorStaffData { club_id: None, ..m.data.clone() };
            assert!(set_staff(&dir, &file, manager, &away, &[]).is_err());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
