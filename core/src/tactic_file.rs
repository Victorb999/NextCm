//! Tácticas gravadas em ficheiro (Gravar/Carregar Táctica nas Tácticas), em JSON na pasta
//! `tactics`, ao lado dos saves.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::tactics::{Instructions, PlayerOrders, Selection};
use crate::{Error, Result};

/// Uma táctica gravada: posições e setas das 11 funções (o GR primeiro), as instruções à
/// equipa e as individuais de cada função. Sem jogadores: ao carregar, o plantel arruma-se.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SavedTactic {
    pub name: String,
    pub positions: Vec<[i32; 2]>,
    pub runs: Vec<Option<[i32; 2]>>,
    pub instructions: Instructions,
    /// Instruções individuais por função (índice em `positions`).
    #[serde(default)]
    pub orders: Vec<(usize, PlayerOrders)>,
}

/// Uma linha do Carregar Táctica.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TacticFile {
    /// Nome do ficheiro na pasta das tácticas.
    pub file: String,
    pub name: String,
}

/// A pasta das tácticas, ao lado dos saves.
pub fn tactics_dir(saves_dir: &Path) -> PathBuf {
    saves_dir.join("tactics")
}

/// As tácticas da pasta, por nome.
pub fn list_tactics(dir: &Path) -> Vec<TacticFile> {
    let mut out: Vec<TacticFile> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            let file = path.file_name()?.to_str()?.to_string();
            let stem = path.file_stem()?.to_str()?.to_string();
            path.extension()?.eq_ignore_ascii_case("json").then_some(TacticFile { file, name: stem })
        })
        .collect();
    out.sort_by_key(|t| t.name.to_lowercase());
    out
}

/// Lê uma táctica da pasta pelo nome do ficheiro (nunca um caminho qualquer).
pub fn read_tactic(dir: &Path, file: &str) -> Result<SavedTactic> {
    if file.contains(['/', '\\']) || file.starts_with('.') {
        return Err(Error::Invalid("Ficheiro de táctica inválido.".into()));
    }
    let bytes = std::fs::read(dir.join(file)).map_err(|_| Error::NotFound(format!("táctica {file}")))?;
    let t: SavedTactic = serde_json::from_slice(&bytes).map_err(|e| Error::Invalid(format!("Táctica estragada: {e}")))?;
    check(t)
}

/// Grava a táctica na pasta como `<nome>.json` (substitui uma com o mesmo nome).
pub fn write_tactic(dir: &Path, t: &SavedTactic) -> Result<TacticFile> {
    let name = t.name.trim();
    if name.is_empty() || name.chars().any(|c| "/\\:*?\"<>|".contains(c)) || name.starts_with('.') {
        return Err(Error::Invalid("Escolha um nome sem / \\ : * ? \" < > |.".into()));
    }
    let t = check(SavedTactic { name: name.to_string(), ..t.clone() })?;
    std::fs::create_dir_all(dir)?;
    let file = format!("{name}.json");
    let json = serde_json::to_vec_pretty(&t).map_err(|e| Error::Invalid(e.to_string()))?;
    std::fs::write(dir.join(&file), json)?;
    Ok(TacticFile { file, name: name.to_string() })
}

fn check(t: SavedTactic) -> Result<SavedTactic> {
    if t.positions.len() != 11 || t.runs.len() != 11 {
        return Err(Error::Invalid("A táctica tem de ter 11 posições.".into()));
    }
    Ok(t)
}

/// A táctica de uma escolha: as posições e as setas de cada função como estão na tela.
pub fn from_view(name: &str, t: &crate::ClubTactics) -> SavedTactic {
    let orders = t
        .starters
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let pid = s.player.as_ref()?.id;
            t.orders.iter().find(|o| o.person_id == pid).map(|o| (i, o.orders))
        })
        .collect();
    SavedTactic {
        name: name.to_string(),
        positions: t.starters.iter().map(|s| [s.x, s.y]).collect(),
        runs: t.starters.iter().map(|s| s.run).collect(),
        instructions: t.instructions,
        orders,
    }
}

/// Aplica a táctica a uma escolha: posições, setas e instruções; os titulares arrumam-se
/// pelas funções novas noutro passo (`World::place_starters`).
pub fn apply(t: &SavedTactic, sel: &Selection) -> Selection {
    let mut out = sel.clone();
    out.auto = false;
    out.positions = t.positions.clone();
    out.runs = t.runs.clone();
    out.instructions = t.instructions;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_lists() {
        let dir = std::env::temp_dir().join(format!("nextcm-tacticas-{}", std::process::id()));
        let t = SavedTactic {
            name: "Pressão alta".into(),
            positions: vec![[50, 50]; 11],
            runs: vec![None; 11],
            instructions: Instructions::default(),
            orders: vec![(3, PlayerOrders { free_role: true, ..Default::default() })],
        };
        let f = write_tactic(&dir, &t).unwrap();
        assert_eq!(f.file, "Pressão alta.json");
        assert!(write_tactic(&dir, &SavedTactic { name: "a/b".into(), ..t.clone() }).is_err());
        let list = list_tactics(&dir);
        assert_eq!(list.len(), 1);
        let back = read_tactic(&dir, &f.file).unwrap();
        assert_eq!(back.orders[0].0, 3);
        assert!(read_tactic(&dir, "../x.json").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
