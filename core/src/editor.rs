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
pub(crate) const TECHNICAL: [&str; 17] = [
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
pub(crate) const OTHER: [&str; 17] = [
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
pub(crate) const EXTRA: [&str; 6] = ["consistency", "dirtiness", "important_matches", "injury_proneness", "versatility", "penalties"];
pub(crate) const GOALKEEPING: [&str; 2] = ["throw_ins", "one_on_ones"];
/// Os 8 atributos mentais, guardados na pessoa e não no jogador.
pub(crate) const MENTAL: [&str; 8] =
    ["adaptability", "ambition", "determination", "loyalty", "pressure", "professionalism", "sportsmanship", "temperament"];

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
    pub club_id: Option<i32>,
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

pub(crate) fn open_save(saves: &Path, file: &str) -> Result<World> {
    let name = Path::new(file).file_name().ok_or_else(|| Error::Invalid("Arquivo inválido.".into()))?;
    let world = World::open_save(&saves.join(name))?;
    let version: i32 = world
        .conn
        .query_row("SELECT version FROM game", [], |r| r.get(0))
        .map_err(|_| Error::Invalid("Este arquivo não é um jogo do NextCM.".into()))?;
    if version != SAVE_VERSION {
        return Err(Error::Invalid(format!(
            "Este jogo é de uma versão anterior ({version}): abra-o e salve-o primeiro no jogo para o editar."
        )));
    }
    Ok(world)
}

fn hit_name(world: &World, id: i32) -> Result<(String, Option<String>, Option<i32>)> {
    Ok(world.conn.query_row(
        "SELECT p.display_name, c.name, p.club_id FROM people p LEFT JOIN clubs c ON c.id = p.club_id WHERE p.id = ?",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?)
}

fn build(world: &World, id: i32) -> Result<EditorPlayer> {
    let profile = world.get_person(id)?;
    let player = profile.player.ok_or_else(|| Error::Invalid("Só se editam jogadores.".into()))?;
    let (name, club, club_id) = hit_name(world, id)?;
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
    Ok(EditorPlayer { id, name, club, club_id, ability: player.current_ability, potential: player.potential_ability, attributes })
}

/// Um jogador de um save fechado, com os atributos para editar.
pub fn player(saves: &Path, file: &str, id: i32) -> Result<EditorPlayer> {
    build(&open_save(saves, file)?, id)
}

/// Exporta para CSV (`;`, para o Excel em português) os jogadores `ids`: nome, clube, CA/PA e
/// os atributos como o jogo os mostra. Grava em `exports/<save>.csv`, ao lado dos saves (por cima
/// do anterior), e devolve o caminho.
pub fn export_csv(saves: &Path, file: &str, ids: &[i32]) -> Result<String> {
    let world = open_save(saves, file)?;
    let players: Vec<EditorPlayer> = ids.iter().filter_map(|&id| build(&world, id).ok()).collect();
    let Some(first) = players.first() else {
        return Err(Error::Invalid("Nenhum jogador para exportar.".into()));
    };
    let keys: Vec<&str> = first.attributes.iter().map(|a| a.key.as_str()).collect();
    let cell = |s: &str| if s.contains([';', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let num = |v: Option<i32>| v.map_or(String::new(), |v| v.to_string());
    let mut out = format!("id;nome;clube;ca;pa;{}\n", keys.join(";"));
    for p in &players {
        let mut row = vec![p.id.to_string(), cell(&p.name), cell(p.club.as_deref().unwrap_or("")), num(p.ability), num(p.potential)];
        row.extend(keys.iter().map(|k| num(p.attributes.iter().find(|a| a.key == *k).map(|a| a.shown))));
        out.push_str(&row.join(";"));
        out.push('\n');
    }
    let dir = saves.join("exports");
    std::fs::create_dir_all(&dir)?;
    let stem = Path::new(file).file_stem().and_then(|s| s.to_str()).unwrap_or("jogo");
    let path = dir.join(format!("{stem}.csv"));
    // Com BOM, para o Excel ler os acentos
    std::fs::write(&path, format!("\u{feff}{out}"))?;
    Ok(path.display().to_string())
}

/// Grava os atributos editados (valores de 1 a 20 como o jogo os mostra) e devolve o jogador
/// como ficou: os relativos passam por [`stored_for_shown`], por isso o valor mostrado pode
/// diferir um ponto quando o par CA/atributo não permite o valor pedido.
#[cfg(test)]
pub fn set_attributes(saves: &Path, file: &str, id: i32, values: &[EditorValue]) -> Result<EditorPlayer> {
    let world = open_save(saves, file)?;
    in_tx(&world, || apply_values(&world, id, values))?;
    build(&world, id)
}

/// Valida e grava os valores (1 a 20, como o jogo os mostra) de um jogador.
fn apply_values(world: &World, id: i32, values: &[EditorValue]) -> Result<()> {
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
    let tx = &world.conn;
    for v in values {
        if MENTAL.contains(&v.key.as_str()) {
            tx.execute(&format!("UPDATE people SET {} = ? WHERE id = ?", v.key), rusqlite::params![v.value, id])?;
        } else {
            let stored = if RELATIVE.contains(&v.key.as_str()) { raw.get(&v.key).copied().flatten().unwrap_or(v.value) } else { v.value };
            tx.execute(&format!("UPDATE players SET {} = ? WHERE person_id = ?", v.key), rusqlite::params![stored, id])?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- comandos em massa

/// Os comandos do "Custom Command" do editor do CM que o NextCM tem (os da inscrição nas
/// competições europeias ficam de fora: o jogo não as tem).
pub const COMMANDS: [(&str, &str); 20] = [
    ("train_willing", "Todos com vontade de treinar"),
    ("equalise", "Igualar capacidade ao potencial"),
    ("consistency", "Consistência a 20"),
    ("abilities_200", "Capacidade e potencial a 200"),
    ("injury_1", "Propensão a lesões a 1"),
    ("morale_20", "Moral a 20"),
    ("strong_pacey", "Fortes e rápidos"),
    ("fitness_100", "Condição a 100%"),
    ("reputations", "Reputações ao máximo"),
    ("mental_20", "Atributos mentais a 20"),
    ("morale_1", "Moral a 1"),
    ("psychos", "Todos uns 'Psicopatas'"),
    ("injury_20", "Propensão a lesões a 20"),
    ("like_manager", "Gostam do treinador atual"),
    ("dislike_manager", "Detestam o treinador atual"),
    ("like_club", "Gostam do clube atual"),
    ("dislike_club", "Detestam o clube atual"),
    ("extend_contracts", "Prolongar os contratos 1 ano"),
    ("two_footed", "Todos com os dois pés"),
    ("call_up", "Convocar para a seleção"),
];

/// A reputação máxima de um jogador no banco.
const MAX_PLAYER_REP: i32 = 200;

fn to_20(keys: &[&str]) -> Vec<EditorValue> {
    keys.iter().map(|k| EditorValue { key: k.to_string(), value: 20 }).collect()
}

/// Aplica os comandos aos jogadores `ids` (a lista do editor ou só o jogador aberto); com
/// `first_team`, só aos do plantel principal (fora das reservas). Devolve quantos mudaram.
pub fn command(saves: &Path, file: &str, ids: &[i32], commands: &[String], first_team: bool) -> Result<usize> {
    let world = open_save(saves, file)?;
    if commands.is_empty() {
        return Err(Error::Invalid("Escolha pelo menos um comando.".into()));
    }
    if let Some(bad) = commands.iter().find(|c| !COMMANDS.iter().any(|k| k.0 == c.as_str())) {
        return Err(Error::Invalid(format!("Comando desconhecido: {bad}.")));
    }
    let has = |k: &str| commands.iter().any(|c| c == k);
    let mut done = 0;
    for &id in ids {
        let (club, is_player): (Option<i32>, bool) = world
            .conn
            .query_row("SELECT club_id, is_player = 1 FROM people WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| Error::Invalid("Pessoa não encontrada.".into()))?;
        if !is_player {
            continue;
        }
        let reserve: bool =
            world.conn.query_row("SELECT reserve FROM player_state WHERE person_id = ?", [id], |r| r.get(0)).unwrap_or(false);
        if first_team && reserve {
            continue;
        }
        // Os atributos: pelo valor que o jogo mostra (os relativos convertidos)
        let mut values: Vec<EditorValue> = Vec::new();
        let mut set = |key: &str, value: i32| {
            values.retain(|v| v.key != key);
            values.push(EditorValue { key: key.into(), value });
        };
        if has("train_willing") {
            set("professionalism", 20);
        }
        if has("consistency") {
            set("consistency", 20);
        }
        if has("injury_1") {
            set("injury_proneness", 1);
        }
        if has("injury_20") {
            set("injury_proneness", 20);
        }
        if has("strong_pacey") {
            for v in to_20(&["strength", "pace", "acceleration"]) {
                set(&v.key, v.value);
            }
        }
        if has("mental_20") {
            for k in MENTAL {
                set(k, 20);
            }
        }
        if has("psychos") {
            set("temperament", 1);
            set("sportsmanship", 1);
            set("aggression", 20);
            set("dirtiness", 20);
        }
        if has("two_footed") {
            set("left_foot", 20);
            set("right_foot", 20);
        }
        if has("morale_20") || has("morale_1") || has("fitness_100") {
            ensure_state(&world, id)?;
        }
        let tx = world.conn.unchecked_transaction()?;
        if has("abilities_200") {
            tx.execute("UPDATE players SET current_ability = 200, potential_ability = 200 WHERE person_id = ?", [id])?;
        } else if has("equalise") {
            tx.execute("UPDATE players SET current_ability = MAX(COALESCE(current_ability, 0), COALESCE(potential_ability, 0)) WHERE person_id = ?", [id])?;
        }
        if has("reputations") {
            tx.execute(
                "UPDATE players SET home_reputation = ?1, current_reputation = ?1, world_reputation = ?1 WHERE person_id = ?2",
                [MAX_PLAYER_REP, id],
            )?;
        }
        for (key, morale) in [("morale_20", 20), ("morale_1", 1)] {
            if has(key) {
                tx.execute("UPDATE player_state SET morale = ? WHERE person_id = ?", [morale, id])?;
                tx.execute("UPDATE players SET morale = ? WHERE person_id = ?", [morale, id])?;
            }
        }
        if has("fitness_100") {
            tx.execute("UPDATE player_state SET condition = 100 WHERE person_id = ?", [id])?;
        }
        if has("extend_contracts") {
            tx.execute(
                "UPDATE people SET contract_expires = date(contract_expires, '+1 year') WHERE id = ? AND contract_expires IS NOT NULL",
                [id],
            )?;
        }
        // As Preferências (save 52): o clube e o treinador de hoje entre os favoritos ou os detestados
        let coach: Option<i32> = match club {
            Some(c) => tx.query_row("SELECT id FROM people WHERE club_id = ? AND club_job = 5 LIMIT 1", [c], |r| r.get(0)).ok(),
            None => None,
        };
        for (key, kind, target, liked) in [
            ("like_club", "club", club, 1),
            ("dislike_club", "club", club, 0),
            ("like_manager", "person", coach, 1),
            ("dislike_manager", "person", coach, 0),
        ] {
            if let (true, Some(t)) = (has(key), target) {
                tx.execute("DELETE FROM preferences WHERE person_id = ? AND kind = ? AND target_id = ?", rusqlite::params![id, kind, t])?;
                tx.execute(
                    "INSERT INTO preferences (person_id, kind, target_id, liked) VALUES (?, ?, ?, ?)",
                    rusqlite::params![id, kind, t, liked],
                )?;
            }
        }
        // "Call Up To National Squad": entra no plantel da selecção da sua nação (`national_team_id`,
        // o plantel dela fora das janelas); a próxima convocatória do seleccionador decide de novo
        if has("call_up") {
            tx.execute(
                "UPDATE people SET national_team_id = (SELECT c.id FROM clubs c WHERE c.is_national_team = 1
                    AND c.nation_id = people.nation_id ORDER BY c.reputation DESC, c.id LIMIT 1)
                 WHERE id = ? AND nation_id IS NOT NULL
                   AND EXISTS (SELECT 1 FROM clubs c WHERE c.is_national_team = 1 AND c.nation_id = people.nation_id)",
                [id],
            )?;
        }
        tx.commit()?;
        if !values.is_empty() {
            apply_values(&world, id, &values)?;
        }
        done += 1;
    }
    Ok(done)
}

/// Os comandos para a lista do diálogo (chave e texto).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorCommand {
    pub key: String,
    pub label: String,
}

pub fn commands() -> Vec<EditorCommand> {
    COMMANDS.iter().map(|(k, l)| EditorCommand { key: k.to_string(), label: l.to_string() }).collect()
}

// ---------------------------------------------------------------- ficha do jogador

pub(crate) const POSITIONS: [&str; 12] = [
    "goalkeeper",
    "sweeper",
    "defender",
    "defensive_midfielder",
    "midfielder",
    "attacking_midfielder",
    "attacker",
    "wing_back",
    "right_side",
    "left_side",
    "central",
    "free_role",
];

/// Uma preferência: clube ou pessoa favorito (`liked`) ou detestado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorPreference {
    /// "club" ou "person".
    pub kind: String,
    pub target_id: i32,
    /// Só para mostrar (ignorado ao gravar).
    #[serde(default)]
    pub name: Option<String>,
    pub liked: bool,
}

/// Um empréstimo (a sub-aba Empréstimo): o clube dono do passe, até quando e a parte do
/// salário que o clube onde joga paga.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorLoan {
    pub parent_id: i32,
    /// Só para mostrar (ignorado ao gravar).
    #[serde(default)]
    pub parent_name: Option<String>,
    /// "AAAA-MM-DD"
    pub until: String,
    /// 0 a 100.
    pub wage_share: i32,
}

/// Uma opção de uma lista (chave e texto).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorChoice {
    pub key: String,
    pub label: String,
}

/// A ficha de um jogador (abas Dados e Contrato): identidade, reputações, contrato, posições,
/// estado, empréstimo e preferências.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorProfile {
    /// "AAAA-MM-DD"
    pub birth_date: Option<String>,
    pub nation_id: Option<i32>,
    pub second_nation_id: Option<i32>,
    pub caps: i32,
    pub goals: i32,
    pub ability: i32,
    pub potential: i32,
    pub home_reputation: i32,
    pub current_reputation: i32,
    pub world_reputation: i32,
    /// "AAAA-MM-DD"
    pub contract_expires: Option<String>,
    /// Início do contrato (quando entrou no clube), "AAAA-MM-DD".
    pub contract_start: Option<String>,
    /// Salário semanal.
    pub wage: i32,
    /// Moral de 1 a 20 (`None` se o save não tem estado do jogador).
    pub morale: Option<i32>,
    /// Chaves dos motivos de insatisfação marcados (pessoais e do clube).
    pub unhappy: Vec<String>,
    /// As 12 posições (1 a 20) na ordem de [`POSITIONS`].
    pub positions: Vec<i32>,
    /// A função no clube (as do Definir Função, mais "notneeded"); `None` = pelo lugar no plantel.
    pub role: Option<String>,
    /// Na lista de transferências pelo clube, a pedido dele e na lista de empréstimos.
    pub listed: bool,
    pub requested: bool,
    pub loan_listed: bool,
    /// Jogos seguidos sem jogar, condição física (0–100) e a lesão (`None` = sem estado no save).
    pub missed: Option<i32>,
    pub condition: Option<i32>,
    pub injury: Option<String>,
    /// "AAAA-MM-DD"
    pub injury_until: Option<String>,
    pub loan: Option<EditorLoan>,
    /// Clubes e pessoas favoritos e detestados (as Preferências do CM).
    pub favourites: Vec<EditorPreference>,
}

/// Um motivo de insatisfação (caixa de seleção).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorReason {
    pub key: String,
    pub label: String,
    /// Queixa sobre o clube (grupo "Insatisfação com o clube").
    pub club: bool,
}

/// Uma nação (ou divisão) para a lista de escolha.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorNation {
    pub id: i32,
    pub name: String,
}

/// A ficha de um jogador, com as chaves das posições e as listas de escolha.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorProfileView {
    pub id: i32,
    pub profile: EditorProfile,
    pub position_keys: Vec<String>,
    pub reasons: Vec<EditorReason>,
    pub nations: Vec<EditorNation>,
    /// As funções no clube.
    pub roles: Vec<EditorChoice>,
    /// Do plantel do treinador do jogo: só lá a insatisfação e a função contam (nos outros
    /// clubes o jogo apaga a insatisfação no dia seguinte).
    pub managed: bool,
    /// As cláusulas e prémios do contrato, em palavras (só leitura).
    pub clauses: Vec<String>,
}

/// Máximo de favoritos (ou de detestados) de cada tipo, como no CM.
const MAX_PREFERENCES: usize = 3;

fn person_or_club_name(world: &World, kind: &str, id: i32) -> Option<String> {
    let sql = if kind == "club" { "SELECT name FROM clubs WHERE id = ?" } else { "SELECT display_name FROM people WHERE id = ?" };
    world.conn.query_row(sql, [id], |r| r.get(0)).ok().flatten()
}

fn profile_view(world: &World, id: i32) -> Result<EditorProfileView> {
    use rusqlite::OptionalExtension;
    let ok: i64 = world.conn.query_row("SELECT COUNT(*) FROM players WHERE person_id = ?", [id], |r| r.get(0))?;
    if ok == 0 {
        return Err(Error::Invalid("Só se editam jogadores.".into()));
    }
    let pos_cols = POSITIONS.iter().map(|p| format!("COALESCE(pl.pos_{p}, 1)")).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT p.birth_date, p.nation_id, p.second_nation_id, COALESCE(p.international_apps, 0), COALESCE(p.international_goals, 0),
                COALESCE(pl.current_ability, 0), COALESCE(pl.potential_ability, 0),
                COALESCE(pl.home_reputation, 0), COALESCE(pl.current_reputation, 0), COALESCE(pl.world_reputation, 0),
                p.contract_expires, COALESCE(p.wage, 0), p.club_joined, {pos_cols}
         FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.id = ?"
    );
    let mut profile = world.conn.query_row(&sql, [id], |r| {
        let mut positions = Vec::new();
        for i in 0..POSITIONS.len() {
            positions.push(r.get::<_, i32>(13 + i)?);
        }
        Ok(EditorProfile {
            birth_date: r.get(0)?,
            nation_id: r.get(1)?,
            second_nation_id: r.get(2)?,
            caps: r.get(3)?,
            goals: r.get(4)?,
            ability: r.get(5)?,
            potential: r.get(6)?,
            home_reputation: r.get(7)?,
            current_reputation: r.get(8)?,
            world_reputation: r.get(9)?,
            contract_expires: r.get(10)?,
            wage: r.get(11)?,
            contract_start: r.get(12)?,
            morale: None,
            unhappy: Vec::new(),
            positions,
            role: None,
            listed: false,
            requested: false,
            loan_listed: false,
            missed: None,
            condition: None,
            injury: None,
            injury_until: None,
            loan: None,
            favourites: Vec::new(),
        })
    })?;
    let conn = &world.conn;
    // Sem linha em `player_state`, o estado é o de partida (o jogo só grava os que mudaram)
    let (morale, missed, condition, injury, until) = conn
        .query_row("SELECT morale, missed, condition, injury, injury_until FROM player_state WHERE person_id = ?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .optional()?
        .unwrap_or((crate::squad::START_MORALE, 0, 100, None, None));
    profile.morale = Some(morale);
    profile.missed = Some(missed);
    profile.condition = Some(condition);
    profile.injury = injury;
    profile.injury_until = until;
    profile.unhappy = conn
        .prepare("SELECT reason FROM unhappy WHERE person_id = ? ORDER BY since, reason")?
        .query_map([id], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    profile.role = conn.query_row("SELECT role FROM squad_roles WHERE person_id = ?", [id], |r| r.get(0)).optional()?;
    if let Some((listed, loan, requested)) = conn
        .query_row("SELECT transfer = 1, loan = 1, requested = 1 FROM transfer_list WHERE person_id = ?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .optional()?
    {
        (profile.listed, profile.loan_listed, profile.requested) = (listed, loan, requested);
    }
    profile.loan = conn
        .query_row(
            "SELECT l.parent_id, c.name, l.until, l.wage_share FROM loans l LEFT JOIN clubs c ON c.id = l.parent_id WHERE l.person_id = ?",
            [id],
            |r| Ok(EditorLoan { parent_id: r.get(0)?, parent_name: r.get(1)?, until: r.get(2)?, wage_share: r.get(3)? }),
        )
        .optional()?;
    profile.favourites = conn
        .prepare("SELECT kind, target_id, liked != 0 FROM preferences WHERE person_id = ? ORDER BY kind, liked DESC, rowid")?
        .query_map([id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?, r.get::<_, bool>(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .map(|(kind, target_id, liked)| EditorPreference { name: person_or_club_name(world, &kind, target_id), kind, target_id, liked })
        .collect();
    let reasons = crate::game::unhappy::REASONS
        .iter()
        .map(|(k, l)| EditorReason { key: k.to_string(), label: l.to_string(), club: false })
        .chain(crate::game::unhappy::CLUB_REASONS.iter().map(|(k, l)| EditorReason {
            key: k.to_string(),
            label: l.to_string(),
            club: true,
        }))
        .collect();
    let mut st = conn.prepare("SELECT id, name FROM nations WHERE name IS NOT NULL ORDER BY name")?;
    let nations = st.query_map([], |r| Ok(EditorNation { id: r.get(0)?, name: r.get(1)? }))?.collect::<rusqlite::Result<_>>()?;
    let roles = crate::game::deals::SQUAD_ROLES
        .iter()
        .map(|r| (r.0, r.1))
        .chain([crate::game::deals::NOT_NEEDED])
        .map(|(k, l)| EditorChoice { key: k.into(), label: l.into() })
        .collect();
    let managed: bool =
        conn.query_row("SELECT COUNT(*) > 0 FROM people p, game g WHERE p.id = ? AND p.club_id = g.club_id", [id], |r| r.get(0))?;
    Ok(EditorProfileView {
        id,
        profile,
        reasons,
        position_keys: POSITIONS.iter().map(|p| p.to_string()).collect(),
        nations,
        roles,
        managed,
        clauses: crate::game::clause_labels(&world.conn, id)?,
    })
}

/// A ficha (aba Dados) de um jogador de um save fechado.
pub fn profile(saves: &Path, file: &str, id: i32) -> Result<EditorProfileView> {
    profile_view(&open_save(saves, file)?, id)
}

/// Cria a linha de `player_state` com o estado de partida, se o jogador ainda não a tem.
fn ensure_state(world: &World, id: i32) -> Result<()> {
    world.conn.execute(
        "INSERT OR IGNORE INTO player_state (person_id, condition, morale, yellows, suspended) VALUES (?, 100, ?, 0, 0)",
        rusqlite::params![id, crate::squad::START_MORALE],
    )?;
    Ok(())
}

pub(crate) fn check_range(label: &str, v: i32, min: i32, max: i32) -> Result<()> {
    if (min..=max).contains(&v) { Ok(()) } else { Err(Error::Invalid(format!("{label}: o valor vai de {min} a {max} (recebi {v})."))) }
}

fn check_date(label: &str, s: Option<&str>) -> Result<()> {
    match s.filter(|s| !s.is_empty()) {
        Some(s) if crate::date::parse(s).is_none() => Err(Error::Invalid(format!("{label} inválido (use AAAA-MM-DD)."))),
        _ => Ok(()),
    }
}

/// Valida as preferências: tipo, alvo que existe, sem repetidos e no máximo 3 de cada lista.
fn check_preferences(world: &World, owner: Option<i32>, list: &[EditorPreference], kinds: &[&str]) -> Result<()> {
    for p in list {
        if !kinds.contains(&p.kind.as_str()) {
            return Err(Error::Invalid(format!("Preferência desconhecida: {}.", p.kind)));
        }
        if person_or_club_name(world, &p.kind, p.target_id).is_none() {
            return Err(Error::Invalid("Preferência: clube ou pessoa desconhecido.".into()));
        }
        if p.kind == "person" && Some(p.target_id) == owner {
            return Err(Error::Invalid("Ninguém é favorito de si mesmo.".into()));
        }
        if list.iter().filter(|q| q.kind == p.kind && q.target_id == p.target_id).count() > 1 {
            return Err(Error::Invalid(format!("{} aparece duas vezes nas preferências.", p.name.as_deref().unwrap_or("Alguém"))));
        }
        if list.iter().filter(|q| q.kind == p.kind && q.liked == p.liked).count() > MAX_PREFERENCES {
            return Err(Error::Invalid(format!("No máximo {MAX_PREFERENCES} favoritos e {MAX_PREFERENCES} detestados de cada tipo.")));
        }
    }
    Ok(())
}

/// Valida e grava a ficha do jogador. CA até 200 e PA ≥ CA; as datas têm de existir.
#[cfg(test)]
pub fn set_profile(saves: &Path, file: &str, id: i32, p: &EditorProfile) -> Result<EditorProfileView> {
    let world = open_save(saves, file)?;
    in_tx(&world, || apply_profile(&world, id, p))?;
    profile_view(&world, id)
}

/// Valida e grava a ficha de um jogador, sem transacção própria (ver [`in_tx`]).
fn apply_profile(world: &World, id: i32, p: &EditorProfile) -> Result<()> {
    let before = profile_view(world, id)?.profile;
    check_range("Capacidade atual (CA)", p.ability, 0, 200)?;
    check_range("Potencial (PA)", p.potential, 0, 200)?;
    if p.potential < p.ability {
        return Err(Error::Invalid("O potencial não pode ser menor que a capacidade atual.".into()));
    }
    for (label, v) in
        [("Reputação local", p.home_reputation), ("Reputação atual", p.current_reputation), ("Reputação mundial", p.world_reputation)]
    {
        check_range(label, v, 0, 10000)?;
    }
    check_range("Jogos pela seleção", p.caps, 0, 999)?;
    check_range("Gols pela seleção", p.goals, 0, 999)?;
    check_range("Salário", p.wage, 0, 100_000_000)?;
    if p.positions.len() != POSITIONS.len() {
        return Err(Error::Invalid("Faltam posições.".into()));
    }
    for v in &p.positions {
        check_range("Posição", *v, 1, 20)?;
    }
    let birth = match p.birth_date.as_deref().filter(|s| !s.is_empty()) {
        Some(s) => {
            let days = crate::date::parse(s).ok_or_else(|| Error::Invalid("Data de nascimento inválida (use AAAA-MM-DD).".into()))?;
            let year = crate::date::civil_from_days(days).0;
            check_range("Ano de nascimento", year, 1900, 2030)?;
            Some((s.to_string(), year))
        }
        None => None,
    };
    check_date("Fim do contrato", p.contract_expires.as_deref())?;
    check_date("Início do contrato", p.contract_start.as_deref())?;
    check_date("Fim da lesão", p.injury_until.as_deref())?;
    if let Some(m) = p.morale {
        check_range("Moral", m, 1, 20)?;
    }
    if let Some(m) = p.missed {
        check_range("Jogos seguidos sem jogar", m, 0, 99)?;
    }
    if let Some(c) = p.condition {
        check_range("Condição física", c, 0, 100)?;
    }
    let known: Vec<&str> = crate::game::unhappy::REASONS.iter().chain(&crate::game::unhappy::CLUB_REASONS).map(|r| r.0).collect();
    if let Some(bad) = p.unhappy.iter().find(|k| !known.contains(&k.as_str())) {
        return Err(Error::Invalid(format!("Motivo de insatisfação desconhecido: {bad}.")));
    }
    if let Some(role) = &p.role
        && !crate::game::deals::SQUAD_ROLES.iter().any(|r| r.0 == role)
        && role != crate::game::deals::NOT_NEEDED.0
    {
        return Err(Error::Invalid(format!("Função desconhecida: {role}.")));
    }
    for n in [p.nation_id, p.second_nation_id].into_iter().flatten() {
        let ok: i64 = world.conn.query_row("SELECT COUNT(*) FROM nations WHERE id = ?", [n], |r| r.get(0))?;
        if ok == 0 {
            return Err(Error::Invalid("Nação desconhecida.".into()));
        }
    }
    let club: Option<i32> = world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [id], |r| r.get(0))?;
    if let Some(l) = &p.loan {
        let Some(club) = club else {
            return Err(Error::Invalid("Sem clube, não pode estar emprestado.".into()));
        };
        if l.parent_id == club {
            return Err(Error::Invalid("O clube dono do passe tem de ser outro que não o clube onde joga.".into()));
        }
        if person_or_club_name(world, "club", l.parent_id).is_none() {
            return Err(Error::Invalid("Empréstimo: clube desconhecido.".into()));
        }
        if crate::date::parse(&l.until).is_none() {
            return Err(Error::Invalid("Fim do empréstimo inválido (use AAAA-MM-DD).".into()));
        }
        check_range("Parte do salário", l.wage_share, 0, 100)?;
    }
    check_preferences(world, Some(id), &p.favourites, &["club", "person"])?;
    let tx = &world.conn;
    tx.execute(
        "UPDATE people SET birth_date = ?, birth_year = ?, birth_sort = ?, nation_id = ?, second_nation_id = ?,
                international_apps = ?, international_goals = ?, contract_expires = ?, wage = ?, ability = ?, club_joined = ?
         WHERE id = ?",
        rusqlite::params![
            birth.as_ref().map(|b| b.0.clone()),
            birth.as_ref().map(|b| b.1),
            birth.as_ref().map(|b| b.0.clone()),
            p.nation_id,
            p.second_nation_id,
            p.caps,
            p.goals,
            p.contract_expires.as_deref().filter(|s| !s.is_empty()),
            p.wage,
            p.ability,
            p.contract_start.as_deref().filter(|s| !s.is_empty()),
            id
        ],
    )?;
    // O estado do jogador (a linha nasce com o estado de partida, se ainda não a tinha)
    ensure_state(world, id)?;
    let injury = p.injury.as_deref().map(str::trim).filter(|s| !s.is_empty());
    tx.execute(
        "UPDATE player_state SET morale = COALESCE(?, morale), missed = COALESCE(?, missed), condition = COALESCE(?, condition),
                injury = ?, injury_until = ? WHERE person_id = ?",
        rusqlite::params![p.morale, p.missed, p.condition, injury, injury.and(p.injury_until.as_deref().filter(|s| !s.is_empty())), id],
    )?;
    if let Some(m) = p.morale {
        tx.execute("UPDATE players SET morale = ? WHERE person_id = ?", rusqlite::params![m, id])?;
    }
    // Os motivos que já estavam guardam a data e o pormenor (que colega, que jogo)
    let today: String = tx.query_row("SELECT date FROM game", [], |r| r.get(0))?;
    for k in before.unhappy.iter().filter(|k| !p.unhappy.contains(k)) {
        tx.execute("DELETE FROM unhappy WHERE person_id = ? AND reason = ?", rusqlite::params![id, k])?;
    }
    for k in &p.unhappy {
        tx.execute("INSERT OR IGNORE INTO unhappy (person_id, reason, since) VALUES (?, ?, ?)", rusqlite::params![id, k, today])?;
    }
    match &p.role {
        Some(r) => tx.execute("INSERT OR REPLACE INTO squad_roles (person_id, role) VALUES (?, ?)", rusqlite::params![id, r])?,
        None => tx.execute("DELETE FROM squad_roles WHERE person_id = ?", [id])?,
    };
    if p.listed || p.loan_listed || p.requested {
        tx.execute(
            "INSERT INTO transfer_list (person_id, transfer, loan, requested) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(person_id) DO UPDATE SET transfer = ?2, loan = ?3, requested = ?4",
            rusqlite::params![id, p.listed || p.requested, p.loan_listed, p.requested],
        )?;
    } else {
        tx.execute("DELETE FROM transfer_list WHERE person_id = ?", [id])?;
    }
    match &p.loan {
        Some(l) => tx.execute(
            "INSERT INTO loans (person_id, parent_id, until, wage_share) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(person_id) DO UPDATE SET parent_id = ?2, until = ?3, wage_share = ?4",
            rusqlite::params![id, l.parent_id, l.until, l.wage_share],
        )?,
        // Sem empréstimo: fica de vez no clube onde joga
        None => tx.execute("DELETE FROM loans WHERE person_id = ?", [id])?,
    };
    if p.favourites != before.favourites {
        tx.execute("DELETE FROM preferences WHERE person_id = ?", [id])?;
        for f in &p.favourites {
            tx.execute(
                "INSERT INTO preferences (person_id, kind, target_id, liked) VALUES (?, ?, ?, ?)",
                rusqlite::params![id, f.kind, f.target_id, f.liked],
            )?;
        }
    }
    let sets = POSITIONS.iter().map(|k| format!("pos_{k} = ?")).collect::<Vec<_>>().join(", ");
    let mut values: Vec<rusqlite::types::Value> =
        [p.ability, p.potential, p.home_reputation, p.current_reputation, p.world_reputation].iter().map(|v| (*v as i64).into()).collect();
    values.extend(p.positions.iter().map(|v| rusqlite::types::Value::from(*v as i64)));
    values.push((id as i64).into());
    tx.execute(
        &format!(
            "UPDATE players SET current_ability = ?, potential_ability = ?, home_reputation = ?, current_reputation = ?,
                    world_reputation = ?, {sets} WHERE person_id = ?"
        ),
        rusqlite::params_from_iter(values),
    )?;
    Ok(())
}

// ---------------------------------------------------------------- clubes

/// Um clube ou uma pessoa numa lista do editor (os rivais).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorRef {
    pub id: i32,
    /// Só para mostrar (ignorado ao gravar).
    #[serde(default)]
    pub name: Option<String>,
}

/// Um salário da folha do clube.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorWage {
    pub id: i32,
    /// Só para mostrar (ignorado ao gravar).
    #[serde(default)]
    pub name: Option<String>,
    /// Jogador (senão, staff).
    #[serde(default)]
    pub player: bool,
    /// Salário semanal.
    pub wage: i32,
}

/// O clube aberto no editor (Edit Club): o que o NextCM guarda.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorClub {
    pub id: i32,
    pub name: String,
    pub reputation: i32,
    /// Como no CM: 1 profissional, 2 semiprofissional, 3 amador.
    pub professional_status: i32,
    /// Instalações de treino, 1 a 20.
    pub training: i32,
    #[ts(type = "number")]
    pub cash: i64,
    pub stadium_name: Option<String>,
    pub capacity: i32,
    pub seating: i32,
    pub expansion: i32,
    pub undersoil_heating: bool,
    /// Cores "#rrggbb": principal (letra, fundo) e alternativo.
    pub home_fg: Option<String>,
    pub home_bg: Option<String>,
    pub away_fg: Option<String>,
    pub away_bg: Option<String>,
    /// Até 3 clubes rivais.
    pub rivals: Vec<EditorRef>,
    /// Jogadores/staff favoritos e detestados do clube (`kind` = "person").
    pub favourites: Vec<EditorPreference>,
    /// A folha: os salários de quem está no clube, do maior para o menor.
    pub wages: Vec<EditorWage>,
    /// Só no clube do treinador: a folha permitida pela direcção (muda a troca com os fundos,
    /// para durar além da próxima revisão) e o salário máximo de hoje (só leitura).
    #[ts(type = "number | null")]
    pub wage_budget: Option<i64>,
    #[ts(type = "number | null")]
    pub max_wage: Option<i64>,
}

fn club_view(world: &World, id: i32) -> Result<EditorClub> {
    use rusqlite::OptionalExtension;
    let (mut club, rivals) = world
        .conn
        .query_row(
            "SELECT c.id, c.name, COALESCE(c.reputation, 0), COALESCE(c.professional_status, 1), COALESCE(c.training, 0),
                    COALESCE(c.cash, 0), s.name, COALESCE(s.capacity, 0), COALESCE(s.seating_capacity, 0),
                    COALESCE(s.expansion_capacity, 0), COALESCE(s.undersoil_heating, 0),
                    c.home_fg, c.home_bg, c.away_fg, c.away_bg, c.rival1_id, c.rival2_id, c.rival3_id
             FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id WHERE c.id = ? AND c.is_national_team = 0",
            [id],
            |r| {
                Ok((
                    EditorClub {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        reputation: r.get(2)?,
                        professional_status: r.get(3)?,
                        training: r.get(4)?,
                        cash: r.get(5)?,
                        stadium_name: r.get(6)?,
                        capacity: r.get(7)?,
                        seating: r.get(8)?,
                        expansion: r.get(9)?,
                        undersoil_heating: r.get::<_, i32>(10)? != 0,
                        home_fg: r.get(11)?,
                        home_bg: r.get(12)?,
                        away_fg: r.get(13)?,
                        away_bg: r.get(14)?,
                        rivals: Vec::new(),
                        favourites: Vec::new(),
                        wages: Vec::new(),
                        wage_budget: None,
                        max_wage: None,
                    },
                    [r.get::<_, Option<i32>>(15)?, r.get(16)?, r.get(17)?],
                ))
            },
        )
        .map_err(|_| Error::Invalid("Clube não encontrado.".into()))?;
    club.rivals = rivals.into_iter().flatten().map(|r| EditorRef { id: r, name: person_or_club_name(world, "club", r) }).collect();
    let conn = &world.conn;
    let has_prefs: i64 =
        conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'club_preferences'", [], |r| r.get(0))?;
    if has_prefs > 0 {
        club.favourites = conn
            .prepare("SELECT person_id, liked != 0 FROM club_preferences WHERE club_id = ? ORDER BY liked DESC, rowid")?
            .query_map([id], |r| Ok((r.get::<_, i32>(0)?, r.get::<_, bool>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|(target_id, liked)| EditorPreference {
                kind: "person".into(),
                target_id,
                name: person_or_club_name(world, "person", target_id),
                liked,
            })
            .collect();
    }
    club.wages = conn
        .prepare(
            "SELECT id, display_name, is_player = 1, COALESCE(wage, 0) FROM people WHERE club_id = ?
             ORDER BY COALESCE(wage, 0) DESC, display_name",
        )?
        .query_map([id], |r| Ok(EditorWage { id: r.get(0)?, name: r.get(1)?, player: r.get(2)?, wage: r.get(3)? }))?
        .collect::<rusqlite::Result<_>>()?;
    let mine: i32 = conn.query_row("SELECT club_id FROM game", [], |r| r.get(0))?;
    if mine == id
        && let Some((budget, max)) =
            conn.query_row("SELECT wage_budget, max_wage FROM board WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?))).optional()?
    {
        club.wage_budget = Some(budget);
        club.max_wage = Some(max);
    }
    Ok(club)
}

/// Um clube de um save fechado.
pub fn club(saves: &Path, file: &str, id: i32) -> Result<EditorClub> {
    club_view(&open_save(saves, file)?, id)
}

fn check_color(label: &str, c: &Option<String>) -> Result<()> {
    match c.as_deref() {
        None | Some("") => Ok(()),
        Some(s) if s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|ch| ch.is_ascii_hexdigit()) => Ok(()),
        _ => Err(Error::Invalid(format!("{label}: use uma cor no formato #rrggbb."))),
    }
}

/// Valida e grava o clube (e o estádio dele).
#[cfg(test)]
pub fn set_club(saves: &Path, file: &str, c: &EditorClub) -> Result<EditorClub> {
    let world = open_save(saves, file)?;
    in_tx(&world, || apply_club(&world, c))?;
    club_view(&world, c.id)
}

/// Valida e grava um clube, sem transacção própria (ver [`in_tx`]).
fn apply_club(world: &World, c: &EditorClub) -> Result<()> {
    let before = club_view(world, c.id)?;
    check_range("Reputação", c.reputation, 0, 10000)?;
    check_range("Estatuto profissional", c.professional_status, 1, 3)?;
    check_range("Instalações de treino", c.training, 1, 20)?;
    if c.cash.abs() > 10_000_000_000 {
        return Err(Error::Invalid("Saldo: o valor máximo é 10 mil milhões.".into()));
    }
    check_range("Capacidade", c.capacity, 0, 200_000)?;
    check_range("Lugares sentados", c.seating, 0, 200_000)?;
    check_range("Expansão", c.expansion, 0, 200_000)?;
    if c.seating > c.capacity {
        return Err(Error::Invalid("Os lugares sentados não podem passar a capacidade.".into()));
    }
    check_color("Cor principal (letra)", &c.home_fg)?;
    check_color("Cor principal (fundo)", &c.home_bg)?;
    check_color("Cor alternativa (letra)", &c.away_fg)?;
    check_color("Cor alternativa (fundo)", &c.away_bg)?;
    if c.rivals.len() > 3 {
        return Err(Error::Invalid("No máximo 3 rivais.".into()));
    }
    for (i, r) in c.rivals.iter().enumerate() {
        if r.id == c.id {
            return Err(Error::Invalid("Um clube não é rival de si mesmo.".into()));
        }
        if c.rivals[..i].iter().any(|o| o.id == r.id) {
            return Err(Error::Invalid("Rival repetido.".into()));
        }
        let ok: i64 = world.conn.query_row("SELECT COUNT(*) FROM clubs WHERE id = ? AND is_national_team = 0", [r.id], |x| x.get(0))?;
        if ok == 0 {
            return Err(Error::Invalid("Rival: clube desconhecido.".into()));
        }
    }
    check_preferences(world, None, &c.favourites, &["person"])?;
    for w in &c.wages {
        check_range("Salário", w.wage, 0, 100_000_000)?;
        if !before.wages.iter().any(|b| b.id == w.id) {
            return Err(Error::Invalid("Salário de alguém que não está no clube.".into()));
        }
    }
    if let Some(v) = c.wage_budget {
        if before.wage_budget.is_none() {
            return Err(Error::Invalid("Só o clube do treinador tem orçamento salarial da diretoria.".into()));
        }
        if !(0..=10_000_000_000).contains(&v) {
            return Err(Error::Invalid("Folha permitida: o valor vai de 0 a 10 mil milhões.".into()));
        }
    }
    if let Some(v) = c.max_wage {
        if before.max_wage.is_none() {
            return Err(Error::Invalid("Só o clube do treinador tem salário máximo da diretoria.".into()));
        }
        if !(0..=100_000_000).contains(&v) {
            return Err(Error::Invalid("Salário máximo: o valor vai de 0 a 100 milhões por semana.".into()));
        }
    }
    let norm = |v: &Option<String>| v.as_deref().filter(|s| !s.is_empty()).map(str::to_lowercase);
    let rival = |i: usize| c.rivals.get(i).map(|r| r.id);
    let tx = &world.conn;
    tx.execute(
        "UPDATE clubs SET reputation = ?, professional_status = ?, training = ?, cash = ?,
                home_fg = ?, home_bg = ?, away_fg = ?, away_bg = ?, rival1_id = ?, rival2_id = ?, rival3_id = ? WHERE id = ?",
        rusqlite::params![
            c.reputation,
            c.professional_status,
            c.training,
            c.cash,
            norm(&c.home_fg),
            norm(&c.home_bg),
            norm(&c.away_fg),
            norm(&c.away_bg),
            rival(0),
            rival(1),
            rival(2),
            c.id
        ],
    )?;
    tx.execute(
        "UPDATE stadiums SET capacity = ?, seating_capacity = ?, expansion_capacity = ?, undersoil_heating = ?
         WHERE id = (SELECT stadium_id FROM clubs WHERE id = ?)",
        rusqlite::params![c.capacity, c.seating, c.expansion, c.undersoil_heating as i32, c.id],
    )?;
    if c.favourites != before.favourites {
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS club_preferences (club_id INTEGER NOT NULL, person_id INTEGER NOT NULL, liked INTEGER NOT NULL)",
        )?;
        tx.execute("DELETE FROM club_preferences WHERE club_id = ?", [c.id])?;
        for f in &c.favourites {
            tx.execute(
                "INSERT INTO club_preferences (club_id, person_id, liked) VALUES (?, ?, ?)",
                rusqlite::params![c.id, f.target_id, f.liked],
            )?;
        }
    }
    for w in c.wages.iter().filter(|w| !before.wages.contains(w)) {
        tx.execute("UPDATE people SET wage = ? WHERE id = ? AND club_id = ?", rusqlite::params![w.wage, w.id, c.id])?;
    }
    // A folha permitida: a diferença vai para a troca com os fundos, que a direcção mantém
    if let (Some(new), Some(old)) = (c.wage_budget, before.wage_budget)
        && new != old
    {
        tx.execute("UPDATE board SET wage_budget = ?1, wage_shift = wage_shift + ?2 WHERE id = 1", rusqlite::params![new, new - old])?;
    }
    // O salário máximo por jogador: o escrito à mão ou, sem ele, o que acompanha a folha e os
    // salários mudados (a conta da direção, que o refaz no começo de cada janela)
    if let (Some(new), Some(old)) = (c.max_wage, before.max_wage)
        && new != old
    {
        tx.execute("UPDATE board SET max_wage = ? WHERE id = 1", [new])?;
    } else if before.wage_budget.is_some() && (c.wage_budget != before.wage_budget || c.wages != before.wages) {
        let top: i64 =
            tx.query_row("SELECT COALESCE(MAX(wage), 0) FROM people WHERE club_id = ? AND is_player = 1", [c.id], |r| r.get(0))?;
        let budget: i64 = tx.query_row("SELECT wage_budget FROM board WHERE id = 1", [], |r| r.get(0))?;
        tx.execute("UPDATE board SET max_wage = ? WHERE id = 1", [crate::game::max_wage_for(top, budget)])?;
    }
    Ok(())
}

// ---------------------------------------------------------------- gravar tudo de uma vez

/// As mudanças de um jogador por gravar: a ficha (se mudou) e os atributos mudados.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorPlayerChange {
    pub id: i32,
    /// Para as mensagens de erro.
    pub name: String,
    pub profile: Option<EditorProfile>,
    pub values: Vec<EditorValue>,
}

/// As mudanças de um membro do staff por gravar.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct EditorStaffChange {
    pub id: i32,
    pub name: String,
    pub data: crate::editor_staff::EditorStaffData,
    pub values: Vec<EditorValue>,
}

/// Tudo o que o editor acumulou para gravar de uma vez.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct EditorChanges {
    pub players: Vec<EditorPlayerChange>,
    pub staff: Vec<EditorStaffChange>,
    pub clubs: Vec<EditorClub>,
}

/// Corre `job` numa transacção: tudo ou nada.
pub(crate) fn in_tx<T>(world: &World, job: impl FnOnce() -> Result<T>) -> Result<T> {
    world.conn.execute_batch("BEGIN IMMEDIATE")?;
    match job() {
        Ok(v) => {
            world.conn.execute_batch("COMMIT")?;
            Ok(v)
        }
        Err(e) => {
            let _ = world.conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// A mensagem de erro com o nome de quem a causou.
fn named(name: &str) -> impl Fn(Error) -> Error + '_ {
    move |e| match e {
        Error::Invalid(m) => Error::Invalid(format!("{name}: {m}")),
        e => e,
    }
}

/// Grava todas as mudanças acumuladas no editor de uma vez: se uma for recusada, nada fica
/// gravado e o erro diz de quem é. Devolve quantos jogadores, staff e clubes mudaram.
pub fn save_changes(saves: &Path, file: &str, ch: &EditorChanges) -> Result<usize> {
    let world = open_save(saves, file)?;
    in_tx(&world, || {
        for c in &ch.clubs {
            apply_club(&world, c).map_err(named(&c.name))?;
        }
        // A ficha antes dos atributos: os relativos convertem-se com a CA nova
        for p in &ch.players {
            if let Some(profile) = &p.profile {
                apply_profile(&world, p.id, profile).map_err(named(&p.name))?;
            }
            if !p.values.is_empty() {
                apply_values(&world, p.id, &p.values).map_err(named(&p.name))?;
            }
        }
        for s in &ch.staff {
            crate::editor_staff::apply_staff(&world, s.id, &s.data, &s.values).map_err(named(&s.name))?;
        }
        Ok(ch.clubs.len() + ch.players.len() + ch.staff.len())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Game;
    use crate::editor_search::{EditorFilter, SEARCH_LIMIT, options, search, search_clubs, squad};

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
            experience: 0,
            favourite_club: None,
        };
        let g = Game::create(&path, &dir, &new, 7).unwrap();
        let file = g.path().file_name().unwrap().to_string_lossy().to_string();
        drop(g);

        let hits = search(&dir, &file, &EditorFilter { query: "gabriel".into(), ..Default::default() }).unwrap();
        assert!(!hits.is_empty() && hits.len() <= SEARCH_LIMIT);
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

        // Ficha: valida CA/PA, datas e posições; grava e reabre igual
        let view = profile(&dir, &file, id).unwrap();
        let mut p = view.profile.clone();
        assert_eq!(p.positions.len(), 12);
        let mut bad = p.clone();
        bad.ability = 201;
        assert!(set_profile(&dir, &file, id, &bad).is_err());
        let mut bad = p.clone();
        bad.ability = 150;
        bad.potential = 140;
        assert!(set_profile(&dir, &file, id, &bad).is_err());
        let mut bad = p.clone();
        bad.birth_date = Some("2000-02-30".into());
        assert!(set_profile(&dir, &file, id, &bad).is_err());
        p.ability = 120;
        p.potential = 150;
        p.wage = 123_456;
        p.caps = 7;
        p.birth_date = Some("2001-03-04".into());
        p.contract_expires = Some("2030-06-30".into());
        p.positions[6] = 20;
        p.unhappy = vec!["bigger".into(), "club_depth".into()];
        p.morale = Some(4);
        let saved = set_profile(&dir, &file, id, &p).unwrap();
        let again = profile(&dir, &file, id).unwrap();
        for v in [&saved, &again] {
            assert_eq!((v.profile.ability, v.profile.potential, v.profile.wage, v.profile.caps), (120, 150, 123_456, 7));
            assert_eq!(v.profile.birth_date.as_deref(), Some("2001-03-04"));
            assert_eq!(v.profile.positions[6], 20);
            assert_eq!(v.profile.unhappy.len(), 2);
        }

        // Clube: procura, edita saldo/estádio/cores e recusa valores impossíveis
        let clubs = search_clubs(&dir, &file, "palmeiras").unwrap();
        assert!(!clubs.is_empty());
        let all = squad(&dir, &file, clubs[0].id).unwrap();
        assert!(all.len() > 10 && all.windows(2).all(|w| w[0].ability >= w[1].ability));
        let mut c = club(&dir, &file, clubs[0].id).unwrap();
        let mut bad = c.clone();
        bad.training = 21;
        assert!(set_club(&dir, &file, &bad).is_err());
        let mut bad = c.clone();
        bad.home_bg = Some("vermelho".into());
        assert!(set_club(&dir, &file, &bad).is_err());
        c.cash = 987_654_321;
        c.reputation = 8000;
        c.capacity = 50_000;
        c.seating = 40_000;
        c.home_bg = Some("#AA0000".into());
        set_club(&dir, &file, &c).unwrap();
        let again = club(&dir, &file, c.id).unwrap();
        assert_eq!((again.cash, again.reputation, again.capacity, again.seating), (987_654_321, 8000, 50_000, 40_000));
        assert_eq!(again.home_bg.as_deref(), Some("#aa0000"));
        // A folha permitida do clube do treinador: o salário máximo acompanha (10% da folha)
        let mut c = again.clone();
        c.wage_budget = Some(c.wage_budget.unwrap() * 10);
        set_club(&dir, &file, &c).unwrap();
        let again = club(&dir, &file, c.id).unwrap();
        assert_eq!(again.wage_budget, c.wage_budget);
        assert!(again.max_wage.unwrap() >= c.wage_budget.unwrap() / 10 - 1000, "{:?}", again.max_wage);
        // Ou escrito à mão
        let mut c = again.clone();
        c.max_wage = Some(1_234_000);
        set_club(&dir, &file, &c).unwrap();
        assert_eq!(club(&dir, &file, c.id).unwrap().max_wage, Some(1_234_000));

        // Comandos em massa (o "Custom Command" do CM): no plantel todo e só num jogador
        let ids: Vec<i32> = all.iter().map(|h| h.id).collect();
        assert!(command(&dir, &file, &ids, &[], false).is_err());
        assert!(command(&dir, &file, &ids, &["nada".into()], false).is_err());
        let cmds: Vec<String> =
            ["two_footed", "morale_20", "reputations", "like_club", "extend_contracts"].iter().map(|s| s.to_string()).collect();
        let before_contract: Option<String> = World::open_save(&dir.join(&file))
            .unwrap()
            .conn
            .query_row("SELECT contract_expires FROM people WHERE id = ?", [ids[0]], |r| r.get(0))
            .unwrap();
        assert_eq!(command(&dir, &file, &ids, &cmds, false).unwrap(), ids.len());
        let p = player(&dir, &file, ids[0]).unwrap();
        let get = |k: &str| p.attributes.iter().find(|a| a.key == k).unwrap().shown;
        assert_eq!((get("left_foot"), get("right_foot")), (20, 20));
        let w = World::open_save(&dir.join(&file)).unwrap();
        let (rep, liked, contract): (i32, i32, Option<String>) = w
            .conn
            .query_row(
                "SELECT pl.world_reputation, (SELECT liked FROM preferences WHERE person_id = p.id AND kind = 'club' AND target_id = p.club_id),
                        p.contract_expires FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.id = ?",
                [ids[0]],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((rep, liked), (200, 1));
        if let (Some(b), Some(a)) = (before_contract, contract) {
            assert_eq!(a[0..4].parse::<i32>().unwrap(), b[0..4].parse::<i32>().unwrap() + 1);
        }
        drop(w);
        // Só o jogador aberto
        assert_eq!(command(&dir, &file, &ids[1..2], &["abilities_200".into()], false).unwrap(), 1);
        assert_eq!(profile(&dir, &file, ids[1]).unwrap().profile.ability, 200);
        assert!(profile(&dir, &file, ids[2]).unwrap().profile.ability < 200);
        // Convocar para a selecção: fica no plantel da selecção da sua nação
        assert_eq!(command(&dir, &file, &ids[2..3], &["call_up".into()], false).unwrap(), 1);
        let w = World::open_save(&dir.join(&file)).unwrap();
        let (team, same): (Option<i32>, bool) = w
            .conn
            .query_row(
                "SELECT p.national_team_id, (SELECT c.nation_id FROM clubs c WHERE c.id = p.national_team_id) = p.nation_id
                 FROM people p WHERE p.id = ?",
                [ids[2]],
                |r| Ok((r.get(0)?, r.get::<_, Option<bool>>(1)?.unwrap_or(false))),
            )
            .unwrap();
        assert!(team.is_some() && same, "{team:?}");
        drop(w);

        // Filtros: CA, idade e posição; todos os resultados cumprem
        let f = EditorFilter { min_ability: Some(140), max_age: Some(30), positions: vec!["attacker".into()], ..Default::default() };
        let found = search(&dir, &file, &f).unwrap();
        assert!(!found.is_empty() && found.iter().all(|h| h.ability.unwrap_or(0) >= 140 && h.age.is_some_and(|a| a <= 30)));
        assert!(found.iter().all(|h| h.position.as_deref().is_some_and(|p| p.contains('A')) && h.value.is_some()));
        assert!(search(&dir, &file, &EditorFilter { positions: vec!["nada".into()], ..Default::default() }).is_err());
        // Contrato, valor e atributos mostrados (calculados no core)
        let rich = EditorFilter { min_value: Some(20_000_000), min_wage: Some(1), ..Default::default() };
        let found = search(&dir, &file, &rich).unwrap();
        assert!(!found.is_empty() && found.iter().all(|h| h.value.unwrap_or(0) >= 20_000_000));
        let fast = EditorFilter {
            attributes: vec![crate::search::AttributeMin { key: "pace".into(), min: 18 }],
            club_status: Some("club".into()),
            ..Default::default()
        };
        let found = search(&dir, &file, &fast).unwrap();
        assert!(!found.is_empty() && found.iter().all(|h| h.club.is_some()));
        let pace = player(&dir, &file, found[0].id).unwrap();
        assert!(pace.attributes.iter().any(|a| a.key == "pace" && a.shown >= 18));
        assert!(search(&dir, &file, &EditorFilter { listing: Some("nada".into()), ..Default::default() }).is_err());
        let opts = options(&dir, &file).unwrap();
        assert!(!opts.divisions.is_empty() && !opts.nations.is_empty());

        // Gravar tudo de uma vez: função, listas, empréstimo, favoritos, rivais e salários
        let star = ids[3];
        let other_club = search_clubs(&dir, &file, "flamengo").unwrap()[0].id;
        let mut prof = profile(&dir, &file, star).unwrap().profile;
        prof.role = Some("key".into());
        prof.requested = true;
        prof.unhappy = vec!["contract".into()];
        prof.loan = Some(EditorLoan { parent_id: other_club, parent_name: None, until: "2027-06-30".into(), wage_share: 50 });
        prof.favourites = vec![EditorPreference { kind: "club".into(), target_id: other_club, name: None, liked: true }];
        let mut cl = club(&dir, &file, clubs[0].id).unwrap();
        let rival = search_clubs(&dir, &file, "corinthians").unwrap()[0].id;
        cl.rivals = vec![EditorRef { id: rival, name: None }];
        cl.favourites = vec![EditorPreference { kind: "person".into(), target_id: star, name: None, liked: false }];
        cl.wages[0].wage = 77_777;
        let paid = cl.wages[0].id;
        let ch = EditorChanges {
            players: vec![EditorPlayerChange {
                id: star,
                name: "Estrela".into(),
                profile: Some(prof),
                values: vec![EditorValue { key: "pace".into(), value: 18 }],
            }],
            staff: Vec::new(),
            clubs: vec![cl.clone()],
        };
        assert_eq!(save_changes(&dir, &file, &ch).unwrap(), 2);
        let v = profile(&dir, &file, star).unwrap().profile;
        assert_eq!(v.role.as_deref(), Some("key"));
        assert!(v.requested && v.listed && v.unhappy == ["contract"]);
        assert_eq!(v.loan.as_ref().map(|l| (l.parent_id, l.wage_share)), Some((other_club, 50)));
        assert_eq!(v.favourites.len(), 1);
        let again = club(&dir, &file, cl.id).unwrap();
        assert_eq!(again.rivals.iter().map(|r| r.id).collect::<Vec<_>>(), vec![rival]);
        assert_eq!(again.favourites.len(), 1);
        assert_eq!(again.wages.iter().find(|w| w.id == paid).unwrap().wage, 77_777);
        // Uma mudança recusada: nada fica gravado e o erro diz de quem é
        let mut bad_club = again.clone();
        bad_club.reputation = 5;
        let mut bad = profile(&dir, &file, star).unwrap().profile;
        bad.potential = 1;
        let ch = EditorChanges {
            players: vec![EditorPlayerChange { id: star, name: "Estrela".into(), profile: Some(bad), values: Vec::new() }],
            staff: Vec::new(),
            clubs: vec![bad_club],
        };
        let err = save_changes(&dir, &file, &ch).unwrap_err().to_string();
        assert!(err.contains("Estrela"), "{err}");
        assert_eq!(club(&dir, &file, cl.id).unwrap().reputation, again.reputation);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
