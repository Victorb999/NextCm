//! Editor de jogos salvos (marco 3.20, primeira fatia): abre um save fechado, procura um jogador
//! e edita os atributos pelo valor de 1 a 20 que o jogo mostra. O valor guardado dos 18
//! atributos "relativos" (ver `attributes.rs`) não é o que se vê: a conta é feita no sentido
//! contrário aqui, no core, e o valor guardado fica ao lado, só de leitura.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::attributes::{self, RELATIVE};
use crate::game::SAVE_VERSION;
use crate::person::Attributes;
use crate::{Error, Result, World};

/// Atributos de jogador por grupo, na ordem do editor do CM: técnicos, físicos e os ocultos.
const TECHNICAL: [&str; 17] = [
    "corners",
    "crossing",
    "dribbling",
    "finishing",
    "free_kicks",
    "handling",
    "heading",
    "long_shots",
    "marking",
    "off_the_ball",
    "passing",
    "tackling",
    "technique",
    "anticipation",
    "decisions",
    "positioning",
    "reflexes",
];
const OTHER: [&str; 17] = [
    "acceleration",
    "agility",
    "balance",
    "jumping",
    "natural_fitness",
    "pace",
    "stamina",
    "strength",
    "aggression",
    "bravery",
    "flair",
    "leadership",
    "teamwork",
    "work_rate",
    "vision",
    "left_foot",
    "right_foot",
];
/// Os atributos que o jogo não mostra (destacados no editor, como no do CM).
const HIDDEN: [&str; 11] = [
    "consistency",
    "dirtiness",
    "important_matches",
    "injury_proneness",
    "natural_fitness",
    "versatility",
    "left_foot",
    "right_foot",
    "penalties",
    "throw_ins",
    "one_on_ones",
];
const EXTRA: [&str; 6] = ["consistency", "dirtiness", "important_matches", "injury_proneness", "versatility", "penalties"];
const GOALKEEPING: [&str; 2] = ["throw_ins", "one_on_ones"];
/// Os 8 atributos mentais, guardados na pessoa e não no jogador.
const MENTAL: [&str; 8] =
    ["adaptability", "ambition", "determination", "loyalty", "pressure", "professionalism", "sportsmanship", "temperament"];

/// Um resultado da procura.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorHit {
    pub id: i32,
    pub name: String,
    pub club: Option<String>,
    pub ability: Option<i32>,
}

/// Um atributo na tela do editor.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorAttribute {
    pub key: String,
    /// O valor de 1 a 20 que o jogo mostra (o que se edita).
    pub shown: i32,
    /// O valor guardado no save (só leitura): para os relativos, o do `.dat`; nos outros, igual ao mostrado.
    pub stored: i32,
    /// Atributo que o jogo não mostra (destacado).
    pub hidden: bool,
    /// Os relativos mostram um valor diferente do guardado.
    pub relative: bool,
    /// "mental" (guardado na pessoa), "technical" ou "other".
    pub group: String,
}

/// Um jogador aberto no editor.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorPlayer {
    pub id: i32,
    pub name: String,
    pub club: Option<String>,
    pub ability: Option<i32>,
    pub potential: Option<i32>,
    pub attributes: Vec<EditorAttribute>,
}

/// Um valor editado: o atributo e o novo valor de 1 a 20.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorValue {
    pub key: String,
    pub value: i32,
}

/// O valor guardado (1–20) que faz o jogo mostrar `target` neste atributo relativo, mantendo os
/// outros; sem valor exacto, o mais próximo.
pub fn stored_for_shown(raw: &Attributes, ca: i32, gk: i32, key: &str, target: i32) -> i32 {
    let current = raw.get(key).copied().flatten().unwrap_or(10);
    let mut best = (i32::MAX, i32::MAX, current);
    for v in 1..=20 {
        let mut trial = raw.clone();
        trial.insert(key.to_string(), Some(v));
        let shown = attributes::displayed_attributes(&trial, Some(ca), Some(gk)).get(key).copied().flatten().unwrap_or(0);
        let score = ((shown - target).abs(), (v - current).abs(), v);
        if score < best {
            best = score;
        }
    }
    best.2
}

fn open_save(saves: &Path, file: &str) -> Result<World> {
    let name = Path::new(file).file_name().ok_or_else(|| Error::Invalid("Arquivo inválido.".into()))?;
    let world = World::open_save(&saves.join(name))?;
    let version: i32 = world
        .conn
        .query_row("SELECT version FROM game", [], |r| r.get(0))
        .map_err(|_| Error::Invalid("Este arquivo não é um jogo do NextCM.".into()))?;
    if version != SAVE_VERSION {
        return Err(Error::Invalid(format!(
            "Este jogo é de uma versão anterior ({version}): abra-o e grave-o primeiro no jogo para o editar."
        )));
    }
    Ok(world)
}

fn hit_name(world: &World, id: i32) -> Result<(String, Option<String>)> {
    Ok(world.conn.query_row(
        "SELECT p.display_name, c.name FROM people p LEFT JOIN clubs c ON c.id = p.club_id WHERE p.id = ?",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?)
}

/// Procura jogadores pelo nome num save fechado (até 40 resultados, os melhores primeiro).
pub fn search(saves: &Path, file: &str, query: &str) -> Result<Vec<EditorHit>> {
    let world = open_save(saves, file)?;
    let text = crate::search::fold(query.trim());
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut st = world.conn.prepare(
        "SELECT p.id, p.display_name, c.name, pl.current_ability FROM people p
         JOIN players pl ON pl.person_id = p.id LEFT JOIN clubs c ON c.id = p.club_id
         WHERE p.search_name LIKE ? ESCAPE '!' ORDER BY pl.current_ability DESC, p.display_name LIMIT 40",
    )?;
    let like = format!("%{}%", text.replace('!', "!!").replace('%', "!%").replace('_', "!_"));
    let rows = st
        .query_map([like], |r| Ok(EditorHit { id: r.get(0)?, name: r.get(1)?, club: r.get(2)?, ability: r.get(3)? }))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn build(world: &World, id: i32) -> Result<EditorPlayer> {
    let profile = world.get_person(id)?;
    let player = profile.player.ok_or_else(|| Error::Invalid("Só se editam jogadores.".into()))?;
    let (name, club) = hit_name(world, id)?;
    let mut attributes = Vec::new();
    let mut push = |key: &str, group: &str| {
        let shown = player.attributes.get(key).copied().flatten();
        let stored = player.raw_attributes.get(key).copied().flatten();
        let Some(shown) = shown.or(stored) else { return };
        attributes.push(EditorAttribute {
            key: key.to_string(),
            shown,
            stored: stored.unwrap_or(shown),
            hidden: HIDDEN.contains(&key),
            relative: RELATIVE.contains(&key),
            group: group.to_string(),
        });
    };
    for k in TECHNICAL {
        push(k, "technical");
    }
    for k in OTHER.iter().chain(&EXTRA).chain(&GOALKEEPING) {
        push(k, "other");
    }
    for k in MENTAL {
        push(k, "mental");
    }
    Ok(EditorPlayer { id, name, club, ability: player.current_ability, potential: player.potential_ability, attributes })
}

/// Um jogador de um save fechado, com os atributos para editar.
pub fn player(saves: &Path, file: &str, id: i32) -> Result<EditorPlayer> {
    build(&open_save(saves, file)?, id)
}

/// Grava os atributos editados (valores de 1 a 20 como o jogo os mostra) e devolve o jogador
/// como ficou: os relativos passam por [`stored_for_shown`], por isso o valor mostrado pode
/// diferir um ponto quando o par CA/atributo não permite o valor pedido.
pub fn set_attributes(saves: &Path, file: &str, id: i32, values: &[EditorValue]) -> Result<EditorPlayer> {
    let world = open_save(saves, file)?;
    for v in values {
        if !(1..=20).contains(&v.value) {
            return Err(Error::Invalid(format!("O valor de um atributo vai de 1 a 20 (recebi {}).", v.value)));
        }
    }
    let profile = world.get_person(id)?;
    let player = profile.player.ok_or_else(|| Error::Invalid("Só se editam jogadores.".into()))?;
    let known = |k: &str| TECHNICAL.contains(&k) || OTHER.contains(&k) || EXTRA.contains(&k) || GOALKEEPING.contains(&k);
    if let Some(bad) = values.iter().find(|v| !known(&v.key) && !RELATIVE.contains(&v.key.as_str()) && !MENTAL.contains(&v.key.as_str())) {
        return Err(Error::Invalid(format!("Atributo desconhecido: {}.", bad.key)));
    }
    let ca = player.current_ability.unwrap_or(0);
    let gk: i32 = world.conn.query_row("SELECT COALESCE(pos_goalkeeper, 0) FROM players WHERE person_id = ?", [id], |r| r.get(0))?;
    let mut raw = player.raw_attributes.clone();
    // Duas passagens: os relativos influenciam-se uns aos outros nas normalizações do jogo
    for _ in 0..2 {
        for v in values.iter().filter(|v| RELATIVE.contains(&v.key.as_str())) {
            let stored = if ca > 0 { stored_for_shown(&raw, ca, gk, &v.key, v.value) } else { v.value };
            raw.insert(v.key.clone(), Some(stored));
        }
    }
    let tx = world.conn.unchecked_transaction()?;
    for v in values {
        if MENTAL.contains(&v.key.as_str()) {
            tx.execute(&format!("UPDATE people SET {} = ? WHERE id = ?", v.key), rusqlite::params![v.value, id])?;
        } else {
            let stored = if RELATIVE.contains(&v.key.as_str()) { raw.get(&v.key).copied().flatten().unwrap_or(v.value) } else { v.value };
            tx.execute(&format!("UPDATE players SET {} = ? WHERE person_id = ?", v.key), rusqlite::params![stored, id])?;
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
    fn edit_attributes_by_shown_value() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-editor-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Editor".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
        };
        let g = Game::create(&path, &dir, &new, 7).unwrap();
        let file = g.path().file_name().unwrap().to_string_lossy().to_string();
        drop(g);

        let hits = search(&dir, &file, "gabriel").unwrap();
        assert!(!hits.is_empty() && hits.len() <= 40);
        let id = hits[0].id;
        let before = player(&dir, &file, id).unwrap();
        assert!(before.attributes.iter().any(|a| a.key == "finishing" && a.relative));
        assert!(before.attributes.iter().any(|a| a.key == "consistency" && a.hidden));
        assert!(before.attributes.iter().any(|a| a.key == "determination" && a.group == "mental"));

        // Os valores fora de 1 a 20 e os atributos desconhecidos são recusados
        assert!(set_attributes(&dir, &file, id, &[EditorValue { key: "finishing".into(), value: 21 }]).is_err());
        assert!(set_attributes(&dir, &file, id, &[EditorValue { key: "nada".into(), value: 5 }]).is_err());

        // Pelo valor mostrado: o relativo converte-se, o físico e o oculto ficam como pedidos
        let edits = [
            EditorValue { key: "finishing".into(), value: 17 },
            EditorValue { key: "pace".into(), value: 3 },
            EditorValue { key: "consistency".into(), value: 19 },
            EditorValue { key: "ambition".into(), value: 4 },
        ];
        let after = set_attributes(&dir, &file, id, &edits).unwrap();
        let get = |p: &EditorPlayer, k: &str| p.attributes.iter().find(|a| a.key == k).unwrap().clone();
        assert!((get(&after, "finishing").shown - 17).abs() <= 1, "{:?}", get(&after, "finishing"));
        assert_eq!(get(&after, "pace").shown, 3);
        assert_eq!(get(&after, "consistency").shown, 19);
        assert_eq!(get(&after, "ambition").shown, 4);
        // Reabrir mostra o mesmo (gravado no save)
        let again = player(&dir, &file, id).unwrap();
        assert_eq!(get(&again, "finishing").shown, get(&after, "finishing").shown);
        assert_ne!(get(&again, "finishing").stored, get(&before, "finishing").stored);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
