//! Conversão dos atributos do banco para os valores que o CM 01/02 mostra no perfil.
//!
//! No `.dat`, 18 atributos técnicos ficam numa escala 1–20 *relativa* (só a relação entre
//! eles importa). Ao criar o jogo, o CM transforma esses valores em "intrínsecos"
//! (−125 a 125) e, nas telas, mostra um valor 1–20 calculado com o intrínseco e o CA.
//! Os outros atributos (físicos e mentais) são mostrados como estão no banco.
//!
//! Tudo aqui foi lido do `cm0102.exe` 3.9.68 (endereços nos comentários):
//! - `0x5389ea` (fim de `database_player_initialisation`): `intr = (10 × valor − CA) / 2`;
//! - `0x538ec0` e `0x538bc0`: duas normalizações do conjunto ("especialista × generalista");
//! - `0x544380` / `0x5442f0`: a fórmula de exibição, também publicada pela comunidade
//!   (MadScientist, fórum champman0102, e `HighConvert`/`LowConvert` do CMScoutIntrinsic).
//!
//! O jogo ainda simula ~6 meses antes da 1ª tela (treino, evolução do CA), então os valores
//! do início de um jogo novo podem ficar 1–2 pontos acima do que calculamos aqui.

use crate::person::Attributes;

/// Os 18 atributos relativos, na ordem das células 14–31 do vetor de atributos do jogo
/// (`database_player_to_attribute_vector`, 0x53a610).
pub const RELATIVE: [&str; 18] = [
    "anticipation",
    "crossing",
    "decisions",
    "dribbling",
    "finishing",
    "handling",
    "heading",
    "long_shots",
    "marking",
    "off_the_ball",
    "one_on_ones",
    "passing",
    "penalties",
    "positioning",
    "reflexes",
    "tackling",
    "throw_ins",
    "vision",
];

const ANTICIPATION: usize = 0;
const DECISIONS: usize = 2;
const DRIBBLING: usize = 3;
const FINISHING: usize = 4;
const HANDLING: usize = 5;
const ONE_ON_ONES: usize = 10;
const POSITIONING: usize = 13;
const REFLEXES: usize = 14;
const THROW_INS: usize = 16;

/// Intrínsecos de um jogador, na ordem de [`RELATIVE`]; `None` = não definido no banco.
pub type Intrinsics = [Option<i32>; 18];

/// Valor 1–20 do banco → intrínseco, antes das normalizações (0x538a12).
pub fn initial_intrinsic(value: i32, ca: i32) -> i32 {
    (value * 10 - ca).clamp(-200, 200) / 2
}

/// Valor mostrado no perfil (0x544380). `low` = versão com o CA dividido por 10 (0x5442f0),
/// usada para atributos de GR em jogadores de linha e vice-versa.
pub fn displayed(intrinsic: i32, ca: i32, low: bool) -> i32 {
    let ca = if low { ca as f64 * 0.1 } else { ca as f64 };
    let x = (intrinsic as f64 * 2.0 + ca) * 0.05 + 10.0;
    let r = (x * x * 0.1 + x) / 3.0 + 0.5;
    r.clamp(1.0, 20.0) as i32
}

/// Truncamento do `__ftol` do MSVC (0x945640): `(x × k) as i32`, em direção ao zero.
fn ftol(x: i32, k: f64) -> i32 {
    (x as f64 * k) as i32
}

/// Uma passada dupla de reequilíbrio dos intrínsecos (núcleo de 0x538bc0 e 0x538ec0).
/// Os valores "típicos" (|v| ≤ limite) passam a ter média `target`; o maior valor fora do
/// limite fica intacto; os outros positivos fora do limite pagam o excesso acima de 85.
fn rebalance(cells: &mut Intrinsics, target: i32, weight: f64, skip: &[usize], gk: i32) {
    let mask: [bool; 18] = std::array::from_fn(|i| cells[i].is_some() && !skip.contains(&i));
    let used = |i: &usize| mask[*i];
    let present = mask.iter().filter(|&&m| m).count() as i32;
    if present == 0 {
        return;
    }
    for pass in 0..2 {
        let mut limit = 30;
        let (mut sum, mut count, mut top, mut typical);
        loop {
            (sum, count, top, typical) = (0, 0, None::<usize>, [false; 18]);
            for i in (0..cells.len()).filter(used) {
                let v = cells[i].unwrap();
                if v.abs() <= limit {
                    sum += v;
                    count += 1;
                    typical[i] = true;
                } else if top.is_none_or(|t| v > cells[t].unwrap()) {
                    top = Some(i);
                }
            }
            if count >= 12.min(present) {
                break;
            }
            limit += 5;
        }
        if let Some(t) = top {
            typical[t] = true;
        }
        let shift = target - if count > 0 { sum / count } else { 0 };
        let (mut over_count, mut over_sum) = (0, 0);
        for i in (0..cells.len()).filter(|i| used(i) && Some(*i) != top) {
            let v = cells[i].unwrap();
            if typical[i] {
                cells[i] = Some((v + shift).clamp(-125, 125));
            } else if pass == 0 && v > 0 && gk < 15 {
                over_count += 1;
                over_sum += if i == DRIBBLING || i == FINISHING { ftol(v, weight) } else { v };
            } else {
                typical[i] = true;
            }
            // Jogador de linha não segura a bola com as mãos.
            if gk < 15 && i == HANDLING {
                cells[i] = Some(-120);
            }
        }
        if over_count > 1 && over_sum > 85 {
            let cut = (over_sum - 85) / over_count;
            for i in (0..cells.len()).filter(|i| used(i) && !typical[*i]) {
                cells[i] = Some((cells[i].unwrap() - cut).max(0));
            }
        }
    }
}

/// Subtrai a média (truncada) de um grupo de intrínsecos, como o jogo faz com os de GR.
fn center(cells: &mut Intrinsics, group: &[usize]) {
    if group.iter().any(|&i| cells[i].is_none()) {
        return;
    }
    let mean = group.iter().map(|&i| cells[i].unwrap()).sum::<i32>() / group.len() as i32;
    for &i in group {
        cells[i] = cells[i].map(|v| v - mean);
    }
}

/// Normalização "física" (0x538ec0): mede o físico/técnico do jogador contra o CA. Se
/// passar muito do orçamento, tira 1–2 pontos de Aceleração, Velocidade, Técnica e Índice
/// de Trabalho, e define a média (−40 a 0) para o primeiro reequilíbrio dos intrínsecos.
fn normalise_physical(attrs: &mut Attributes, cells: &mut Intrinsics, ca: i32, gk: i32) {
    let get = |a: &Attributes, k: &str| a.get(k).copied().flatten();
    let inputs = ["pace", "leadership", "jumping", "acceleration", "teamwork", "technique", "left_foot", "right_foot", "work_rate"];
    let target = match inputs.map(|k| get(attrs, k)) {
        [Some(pace), Some(infl), Some(jump), Some(acc), Some(team), Some(tech), Some(lf), Some(rf), Some(work)] => {
            let sum = ftol(pace, 1.5) - ftol(infl, -0.8) - ftol(jump, -0.5) - ftol(acc, -1.2) + ftol(team, 0.8)
                - ftol(tech, -0.8)
                - ftol(lf.min(rf), -0.75)
                + work;
            let budget = (ca / 2 - ftol(sum, 1.3605442176870748) + 55) / 2;
            if budget < -14 {
                let cut = (budget / 15).max(-2);
                for k in ["acceleration", "pace", "technique", "work_rate"] {
                    if let Some(Some(v)) = attrs.get_mut(k) {
                        *v = (*v + cut).max(1);
                    }
                }
            }
            let (pace, acc) = (get(attrs, "pace").unwrap(), get(attrs, "acceleration").unwrap());
            let speed = pace * acc / 2;
            let mut target = budget - 20;
            if speed > ca {
                target -= (speed - ca).min(25);
            }
            target.clamp(-40, 0)
        }
        _ => -20,
    };
    rebalance(cells, target, 2.0, &[ONE_ON_ONES, THROW_INS], gk);
    if gk == 20 {
        center(cells, &[HANDLING, REFLEXES]);
        center(cells, &[ANTICIPATION, DECISIONS, POSITIONING]);
    }
}

/// Intrínsecos de um jogador novo, como o CM calcula a partir do banco. Também aplica em
/// `attrs` os cortes físicos da normalização (0x538ec0).
pub fn player_intrinsics(attrs: &mut Attributes, ca: i32, gk: i32) -> Intrinsics {
    let mut cells: Intrinsics = RELATIVE.map(|k| attrs.get(k).copied().flatten().map(|v| initial_intrinsic(v, ca)));
    normalise_physical(attrs, &mut cells, ca, gk);
    rebalance(&mut cells, -20, 1.75, &[], gk); // 0x538bc0
    if gk >= 15 {
        center(&mut cells, &[HANDLING, REFLEXES]);
    }
    cells
}

/// Se o atributo usa a fórmula "low" (CA/10) para este jogador (tabela em 0x543a94).
/// Um-para-um, Penáltis e Lançamentos não aparecem no perfil do CM; seguimos o CMScout.
fn uses_low(attr: usize, gk: i32) -> bool {
    match RELATIVE[attr] {
        "vision" | "crossing" | "dribbling" | "finishing" | "marking" | "off_the_ball" => gk == 20,
        "handling" | "reflexes" | "one_on_ones" => gk < 15,
        "throw_ins" => gk >= 15,
        _ => false,
    }
}

/// Atributos como o perfil do CM 01/02 mostra. `raw` são os valores do banco, `ca` o
/// Current Ability e `gk` a nota da posição de GR (20 = GR, 15+ = joga de GR).
pub fn displayed_attributes(raw: &Attributes, ca: Option<i32>, gk: Option<i32>) -> Attributes {
    let mut out = raw.clone();
    let Some(ca) = ca.filter(|ca| (1..=200).contains(ca)) else { return out };
    let gk = gk.unwrap_or(0);
    let cells = player_intrinsics(&mut out, ca, gk);
    for (i, cell) in cells.iter().enumerate() {
        out.insert(RELATIVE[i].into(), cell.map(|v| displayed(v, ca, uses_low(i, gk))));
    }
    out
}

/// Os 8 atributos relativos do staff, na ordem das células 13–20 do vetor
/// (`database_non_player_to_attribute_vector`, 0x53a8b0). As células 0–12 (Attacking,
/// Business, Coaching Technique, Directness, Discipline, Free Roles, Interference, Marking,
/// Offside, Patience, Pressing, Resources, Youngsters) são mostradas como estão no banco.
pub const STAFF_RELATIVE: [&str; 8] =
    ["coaching_gks", "coaching", "judgement", "judging_potential", "man_handling", "motivating", "physiotherapy", "tactics"];

/// Atributos de staff como o perfil do CM 01/02 mostra. Ao criar o jogo, os relativos viram
/// intrínsecos com a mesma conta dos jogadores (0x538ac6), mas sem as normalizações; a
/// exibição (`non_player_attribute_rating`, 0x544410) é a fórmula "normal" de [`displayed`].
pub fn displayed_staff_attributes(raw: &Attributes, ca: Option<i32>) -> Attributes {
    let mut out = raw.clone();
    let Some(ca) = ca.filter(|ca| (1..=200).contains(ca)) else { return out };
    for k in STAFF_RELATIVE {
        if let Some(Some(v)) = raw.get(k) {
            out.insert(k.into(), Some(displayed(initial_intrinsic(*v, ca), ca, false)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formula_matches_community_examples() {
        // MadScientist, champman0102.co.uk/viewtopic.php?t=4251 (exemplos com save real).
        assert_eq!(displayed(-3, 40, false), 8);
        assert_eq!(displayed(3, 42, false), 9);
        // 25,1 e 21,7 sem o limite; o perfil mostra 20.
        assert_eq!(displayed(73, 112, false), 20);
        assert_eq!(displayed(31, 158, false), 20);
        assert_eq!(displayed(-125, 1, false), 1);
    }

    #[test]
    fn low_formula_scales_ability_down() {
        // Intrínseco 0 e CA 150: x = 10,75 na "low" e 17,5 na normal.
        assert_eq!(displayed(0, 150, true), 7);
        assert_eq!(displayed(0, 150, false), 16);
    }

    #[test]
    fn initial_conversion() {
        assert_eq!(initial_intrinsic(11, 186), -38);
        assert_eq!(initial_intrinsic(20, 1), 99); // 199 / 2
        assert_eq!(initial_intrinsic(1, 200), -95);
        assert_eq!(initial_intrinsic(20, -100), 100); // limite de ±200 antes da divisão
    }

    #[test]
    fn staff_relative_attributes() {
        // Pep Guardiola no banco 25-26 (CA 190): o CA se cancela, x = 10 + v/2.
        let raw = attrs(&[("coaching", 5), ("coaching_gks", 1), ("motivating", 15), ("tactics", 20), ("business", 1), ("discipline", 15)]);
        let shown = displayed_staff_attributes(&raw, Some(190));
        assert_eq!(shown["coaching"], Some(9));
        assert_eq!(shown["coaching_gks"], Some(7));
        assert_eq!(shown["motivating"], Some(16));
        assert_eq!(shown["tactics"], Some(20));
        // Absolutos ficam como no banco
        assert_eq!(shown["business"], Some(1));
        assert_eq!(shown["discipline"], Some(15));
    }

    fn attrs(pairs: &[(&str, i32)]) -> Attributes {
        pairs.iter().map(|(k, v)| (k.to_string(), Some(*v))).collect()
    }

    #[test]
    fn missing_values_stay_missing() {
        let raw = attrs(&[("finishing", 15), ("pace", 12)]);
        let shown = displayed_attributes(&raw, Some(120), Some(1));
        assert_eq!(shown["pace"], Some(12));
        assert!(shown["finishing"].is_some());
        assert_eq!(shown.get("crossing").copied().flatten(), None);
        // Sem CA válido o jogo sorteia um; mostramos o banco.
        assert_eq!(displayed_attributes(&raw, None, Some(1)), raw);
    }

    #[test]
    fn outfield_player_cannot_use_hands() {
        let mut raw = attrs(&RELATIVE.map(|k| (k, 10)));
        raw.insert("handling".into(), Some(20));
        let cells = player_intrinsics(&mut raw.clone(), 150, 1);
        assert_eq!(cells[HANDLING], Some(-120));
        let shown = displayed_attributes(&raw, Some(150), Some(1));
        assert_eq!(shown["handling"], Some(1));
    }
}
