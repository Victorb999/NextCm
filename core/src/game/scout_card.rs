//! O relatório completo do olheiro (o "Report Card" do FM08): quando chega o relatório de um
//! Observar Jogador, o olheiro escreve um cartão com a melhor posição, a habilidade e o potencial
//! (em estrelas, contra o elenco e a liga do clube), o caráter, os pontos fortes e fracos, se ele
//! viria e quando, e a recomendação. Fica guardado (o último por jogador) e aparece na aba
//! Relatórios do perfil; a mensagem traz só o resumo.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::market::money_text;
use super::personal_info::personality_label;
use super::{Game, cm_date};
use crate::search::NATURAL_POSITION;
use crate::{Result, date};

/// Save 76: o último cartão de cada jogador observado.
pub(super) const SCOUT_CARDS_SCHEMA: &str =
    "CREATE TABLE scout_cards (person_id INTEGER PRIMARY KEY, date TEXT NOT NULL, card TEXT NOT NULL);";

/// Os atributos de que o olheiro fala (jogadores de campo e goleiros), com o nome do CM.
const OUTFIELD: [(&str, &str); 22] = [
    ("acceleration", "aceleração"),
    ("agility", "agilidade"),
    ("anticipation", "antecipação"),
    ("bravery", "bravura"),
    ("heading", "jogo aéreo"),
    ("vision", "criatividade"),
    ("crossing", "cruzamentos"),
    ("decisions", "decisões"),
    ("tackling", "desarme"),
    ("finishing", "finalização"),
    ("dribbling", "drible"),
    ("strength", "força física"),
    ("work_rate", "trabalho"),
    ("flair", "instinto"),
    ("marking", "marcação"),
    ("passing", "passe"),
    ("positioning", "posicionamento"),
    ("long_shots", "chutes de longe"),
    ("stamina", "resistência"),
    ("off_the_ball", "movimentação sem bola"),
    ("technique", "técnica"),
    ("pace", "velocidade"),
];
const KEEPER: [(&str, &str); 7] = [
    ("handling", "jogo de mãos"),
    ("reflexes", "reflexos"),
    ("one_on_ones", "um contra um"),
    ("positioning", "posicionamento"),
    ("decisions", "decisões"),
    ("agility", "agilidade"),
    ("anticipation", "antecipação"),
];

/// Uma parte do cartão: "Melhor Posição", "Habilidade"...; o valor à direita (texto ou estrelas).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ScoutSection {
    pub title: String,
    pub value: Option<String>,
    /// 1–5 (meias estrelas não há).
    pub stars: Option<u8>,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ScoutCard {
    pub scout: String,
    pub date: String,
    pub sections: Vec<ScoutSection>,
}

fn section(title: &str, value: Option<String>, stars: Option<u8>, lines: Vec<String>) -> ScoutSection {
    ScoutSection { title: title.into(), value, stars, lines }
}

/// O nome de uma posição (índice das colunas de `players`, como em `search::position_label`) com o lado.
fn position_name(role: usize, side: Option<usize>) -> &'static str {
    match (role, side) {
        (0, _) => "goleiro",
        (1, _) => "líbero",
        (2, Some(8)) => "lateral-direito",
        (2, Some(9)) => "lateral-esquerdo",
        (2, _) => "zagueiro",
        (7, Some(8)) => "ala direito",
        (7, Some(9)) => "ala esquerdo",
        (7, _) => "ala",
        (3, _) => "volante",
        (4, Some(8)) => "meia pela direita",
        (4, Some(9)) => "meia pela esquerda",
        (4, _) => "meio-campista",
        (5, Some(8)) => "ponta-direita",
        (5, Some(9)) => "ponta-esquerda",
        (5, _) => "meia-atacante",
        (6, Some(8)) => "atacante pela direita",
        (6, Some(9)) => "atacante pela esquerda",
        _ => "centroavante",
    }
}

/// As colunas das posições em `players`, na ordem de `search::position_label`.
const POSITION_COLUMNS: [&str; 12] = [
    "pos_goalkeeper",
    "pos_sweeper",
    "pos_defender",
    "pos_defensive_midfielder",
    "pos_midfielder",
    "pos_attacking_midfielder",
    "pos_attacker",
    "pos_wing_back",
    "pos_right_side",
    "pos_left_side",
    "pos_central",
    "pos_free_role",
];

/// As duas melhores posições: (nota, função, lado), a melhor primeiro.
fn best_positions(p: &[i32; 12]) -> Vec<(i32, usize, Option<usize>)> {
    if p[0] >= NATURAL_POSITION {
        return vec![(p[0], 0, None)];
    }
    let mut all: Vec<(i32, usize, Option<usize>)> = Vec::new();
    for role in [1, 2, 7, 3, 4, 5, 6] {
        let sided = matches!(role, 2 | 4 | 5 | 6 | 7);
        if sided {
            for side in [10, 8, 9] {
                all.push((p[role].min(p[side]), role, Some(side)));
            }
        } else {
            all.push((p[role], role, None));
        }
    }
    all.sort_by_key(|x| std::cmp::Reverse(x.0));
    all.into_iter().filter(|x| x.0 >= NATURAL_POSITION).take(2).collect()
}

/// Os atributos (pelo nome do CM em `OUTFIELD`) que cada função precisa.
fn role_needs(role: usize) -> &'static [&'static str] {
    match role {
        1 | 2 | 7 => {
            &["desarme", "marcação", "jogo aéreo", "posicionamento", "força física", "velocidade", "antecipação", "bravura", "decisões"]
        }
        3 | 4 => &[
            "passe",
            "criatividade",
            "técnica",
            "decisões",
            "trabalho",
            "resistência",
            "desarme",
            "movimentação sem bola",
            "posicionamento",
        ],
        5 => &[
            "passe",
            "criatividade",
            "técnica",
            "drible",
            "instinto",
            "movimentação sem bola",
            "chutes de longe",
            "finalização",
            "decisões",
        ],
        _ => &[
            "finalização",
            "movimentação sem bola",
            "drible",
            "velocidade",
            "técnica",
            "jogo aéreo",
            "aceleração",
            "força física",
            "decisões",
        ],
    }
}

/// "meia-atacante" → "Meia-atacante".
fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
}

fn stars_from(d: f64) -> u8 {
    match d {
        d if d >= 10.0 => 5,
        d if d >= 0.0 => 4,
        d if d >= -10.0 => 3,
        d if d >= -20.0 => 2,
        _ => 1,
    }
}

impl Game {
    /// O cartão de `person` escrito por um olheiro com estes Avaliar Jogadores e Avaliar Potencial.
    pub(super) fn write_scout_card(&self, person: i32, scout_name: &str, judgement: i32, potential: i32) -> Result<ScoutCard> {
        let conn = &self.world.conn;
        let name = self.person_name(person)?;
        let first = name.split_whitespace().last().unwrap_or(&name).to_string();
        let scout_short = scout_name.split_whitespace().last().unwrap_or(scout_name).to_string();
        let level = self.squad_level()?;
        let (stars, age, room, seen) = self.judge_seen(person, judgement, potential, level)?;
        let stars = stars as u8;
        let mut sections = Vec::new();

        // Melhor posição
        let cols = POSITION_COLUMNS.iter().map(|c| format!("COALESCE({c}, 0)")).collect::<Vec<_>>().join(", ");
        let pos: [i32; 12] = conn.query_row(&format!("SELECT {cols} FROM players WHERE person_id = ?"), [person], |r| {
            let mut out = [0; 12];
            for (i, v) in out.iter_mut().enumerate() {
                *v = r.get(i)?;
            }
            Ok(out)
        })?;
        let best = best_positions(&pos);
        let main = best.first().copied();
        if let Some((_, role, side)) = main {
            let mut lines = vec![format!("{scout_name} acha que {name} joga melhor como {}.", position_name(role, side))];
            if let Some((_, r2, s2)) = best.get(1).copied().filter(|b| b.0 >= 18) {
                lines.push(format!("Mas também acha que ele se sai bem como {}.", position_name(r2, s2)));
            }
            sections.push(section("Melhor Posição", Some(capital(position_name(role, side))), None, lines));
        }

        // Habilidade: contra o melhor do elenco na mesma função e contra a liga do clube
        let mut lines = Vec::new();
        if let Some((_, role, _)) = main {
            let rival: Option<(String, i32)> = conn
                .query_row(
                    &format!(
                        "SELECT p.display_name, COALESCE(pl.current_ability, 1) FROM people p JOIN players pl ON pl.person_id = p.id
                         WHERE p.club_id = ? AND p.is_player = 1 AND p.id <> ? AND COALESCE(pl.{}, 0) >= ?
                         ORDER BY pl.current_ability DESC LIMIT 1",
                        POSITION_COLUMNS[role]
                    ),
                    params![self.club_id, person, NATURAL_POSITION],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let pos = position_name(role, None);
            lines.push(match rival {
                Some((who, ca)) => {
                    let d = seen - ca as f64;
                    let how = match d {
                        d if d >= 12.0 => "muito melhor do que",
                        d if d >= 4.0 => "melhor do que",
                        d if d > -4.0 => "do mesmo nível que",
                        d if d > -12.0 => "pior do que",
                        _ => "muito pior do que",
                    };
                    format!("{scout_short} acha que {first} é {how} o nosso melhor {pos}, {who}.")
                }
                None => format!("{scout_short} lembra que o elenco não tem nenhum {pos} de origem."),
            });
        }
        if let Some(div) = self.division_of(self.club_id)? {
            let league: Option<f64> = conn.query_row(
                "SELECT AVG(ca) FROM (SELECT pl.current_ability AS ca,
                        ROW_NUMBER() OVER (PARTITION BY p.club_id ORDER BY pl.current_ability DESC) AS n
                 FROM people p JOIN players pl ON pl.person_id = p.id JOIN clubs c ON c.id = p.club_id
                 WHERE c.division_id = ? AND p.is_player = 1) WHERE n <= 11",
                [div],
                |r| r.get(0),
            )?;
            if let Some(avg) = league {
                let comp = self.competition_name(div).to_string();
                lines.push(match seen - avg {
                    d if d >= 15.0 => format!("É da opinião de que {first} seria uma estrela na maioria dos clubes da {comp}."),
                    d if d >= 3.0 => format!("É da opinião de que {first} seria titular na maioria dos clubes da {comp}."),
                    d if d >= -8.0 => format!("É da opinião de que {first} teria lugar na maioria dos clubes da {comp}."),
                    _ => format!("É da opinião de que {first} teria dificuldades para jogar na {comp}."),
                });
            }
        }
        sections.push(section("Habilidade", None, Some(stars), lines));

        // Potencial
        let top = seen + room.max(0.0);
        let pot_stars = stars_from(top - level);
        let line = if age >= 30 {
            format!("{scout_name} acha que {first} já está no auge e só deve cair daqui para a frente.")
        } else if room >= 25.0 {
            format!("{scout_name} acha que {first} tem muito potencial e pode se tornar um jogador bem melhor.")
        } else if room >= 10.0 {
            format!("{scout_name} acha que {first} ainda pode melhorar bastante.")
        } else if room >= 4.0 {
            format!("{scout_name} acha que {first} ainda pode melhorar um pouco.")
        } else {
            format!("{scout_name} acha que {first} já joga perto do seu potencial máximo.")
        };
        sections.push(section("Potencial", None, Some(pot_stars), vec![line]));

        // Caráter
        let mental: [i32; 6] = conn.query_row(
            "SELECT COALESCE(determination, 10), COALESCE(professionalism, 10), COALESCE(ambition, 10), COALESCE(loyalty, 10),
                    COALESCE(temperament, 10), COALESCE(sportsmanship, 10) FROM people WHERE id = ?",
            [person],
            |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?]),
        )?;
        let [det, pro, amb, loy, tem, spo] = mental;
        let label = personality_label(det, pro, amb, loy, tem, spo);
        let mut lines = vec![match () {
            _ if pro >= 17 => format!("{scout_name} acha que {first} é um profissional exemplar."),
            _ if det >= 17 => format!("{scout_name} acha que {first} é muito determinado."),
            _ if amb >= 17 => format!("{scout_name} acha que {first} é muito ambicioso."),
            _ if tem <= 5 => format!("{scout_name} avisa que {first} tem o pavio curto."),
            _ if pro <= 5 => format!("{scout_name} avisa que {first} não é muito profissional."),
            _ => format!("{scout_name} acha que {first} tem uma personalidade equilibrada."),
        }];
        let squad: f64 = conn.query_row(
            "SELECT COALESCE(AVG(COALESCE(determination, 10) + COALESCE(professionalism, 10)), 20) FROM people
             WHERE club_id = ? AND is_player = 1",
            [self.club_id],
            |r| r.get(0),
        )?;
        let mine = (det + pro) as f64;
        lines.push(if squad >= 28.0 && mine >= squad - 2.0 {
            format!("{scout_short} acha que o elenco é muito dedicado e que {first} deve se encaixar bem no grupo.")
        } else if mine + 6.0 < squad {
            format!("{scout_short} teme que {first} não se encaixe num grupo tão dedicado.")
        } else {
            format!("{scout_short} não vê problemas para {first} se adaptar ao grupo.")
        });
        let first_label = label.split(',').next().unwrap_or(&label).trim().to_string();
        sections.push(section("Caráter", Some(first_label), None, lines));

        // Pontos fortes e fracos
        let keeper = main.is_some_and(|m| m.1 == 0);
        let p = self.world.get_person(person)?;
        if let Some(pl) = &p.player {
            let mut ranked: Vec<(&str, i32)> = (if keeper { &KEEPER[..] } else { &OUTFIELD[..] })
                .iter()
                .filter_map(|(k, label)| pl.attributes.get(*k).copied().flatten().map(|v| (*label, v)))
                .collect();
            ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            if let Some(&(best, v)) = ranked.first().filter(|r| r.1 >= 13) {
                let mut lines = vec![format!("{scout_name} acha que {first} se destaca em {best}.")];
                if let Some(&(second, _)) = ranked.get(1).filter(|r| r.1 >= 13) {
                    lines.push(format!("Também elogia {second}{}.", if v >= 17 { ", que já é de alto nível" } else { "" }));
                }
                sections.push(section("Pontos Fortes", Some(capital(best)), None, lines));
            } else {
                sections.push(section(
                    "Pontos Fortes",
                    Some("-".into()),
                    None,
                    vec![format!("{scout_name} não vê nada de especial em {first}.")],
                ));
            }
            // O ponto fraco é um dos que a função dele pede
            let role = main.map_or(6, |m| m.1);
            if let Some(&(weak, _)) = ranked.iter().rev().find(|r| r.1 <= 9 && (keeper || role_needs(role).contains(&r.0))) {
                sections.push(section(
                    "Pontos Fracos",
                    Some(capital(weak)),
                    None,
                    vec![format!("{scout_name} acha que {first} é fraco em {weak} e que isso pode ser explorado pelos adversários.")],
                ));
            } else {
                sections.push(section(
                    "Pontos Fracos",
                    Some("-".into()),
                    None,
                    vec![format!("{scout_name} não encontrou nenhum ponto fraco evidente em {first}.")],
                ));
            }
        }

        // Transferência: se viria, o valor e quando pode assinar um pré-contrato
        let mut lines = Vec::new();
        let unwilling = match self.unwilling_sql()? {
            Some(sql) => conn.query_row(&format!("SELECT COUNT(*) > 0 FROM ({sql}) WHERE id = ?"), [person], |r| r.get::<_, bool>(0))?,
            None => false,
        };
        let facts = self.facts(person)?;
        let free = facts.as_ref().is_some_and(|f| f.is_free());
        lines.push(if unwilling {
            format!("{scout_name} acha que {first} não tem a menor intenção de vir para o nosso clube neste momento.")
        } else {
            format!("{scout_name} acha que {first} aceitaria vir para o nosso clube.")
        });
        if let Some(f) = &facts {
            if free {
                lines.push(format!("{scout_short} lembra que {first} está sem contrato e pode assinar já."));
            } else if f.club != Some(self.club_id) {
                if f.value > 0 {
                    lines.push(format!("Calcula que ele vale cerca de {}.", money_text(f.value)));
                }
                if let Some(exp) = f.expires.as_deref() {
                    let from = date::add_days(exp, -(super::contracts::PRECONTRACT_DAYS as i32)).unwrap_or_default();
                    if from.as_str() <= self.date.as_str() {
                        lines.push(format!("O contrato acaba em {}: já se pode propor um pré-contrato.", cm_date(exp)));
                    } else {
                        lines.push(format!(
                            "O contrato acaba em {}: dá para propor um pré-contrato a partir de {}.",
                            cm_date(exp),
                            cm_date(&from)
                        ));
                    }
                }
            }
        }
        let value = if unwilling {
            "Não viria"
        } else if free {
            "Livre"
        } else {
            "Viria"
        };
        sections.push(section("Transferência", Some(value.into()), None, lines));

        // Recomendação: a habilidade, e o potencial nos jovens
        let rec = if age <= 23 { ((stars as u32 + 2 * pot_stars as u32) as f64 / 3.0).round() as u8 } else { stars };
        let rec = if unwilling && rec > 1 { rec - 1 } else { rec }.clamp(1, 5);
        let line = match rec {
            5 => format!("{scout_short} acredita que {first} seria uma contratação de muita qualidade."),
            4 => format!("{scout_short} acredita que {first} seria um bom reforço para o elenco."),
            3 => format!("{scout_short} acha que {first} pode ser útil, mas não é prioridade."),
            2 => format!("{scout_short} acha que {first} não acrescentaria muito ao elenco."),
            _ => format!("{scout_short} não recomenda a contratação de {first}."),
        };
        sections.push(section("Recomendação", None, Some(rec), vec![line]));

        Ok(ScoutCard { scout: scout_name.into(), date: self.date.clone(), sections })
    }

    /// Guarda o cartão (substitui o anterior do mesmo jogador).
    pub(super) fn keep_scout_card(&self, person: i32, card: &ScoutCard) -> Result<()> {
        let json = serde_json::to_string(card).map_err(|e| crate::Error::Invalid(e.to_string()))?;
        self.world
            .conn
            .execute("INSERT OR REPLACE INTO scout_cards (person_id, date, card) VALUES (?, ?, ?)", params![person, card.date, json])?;
        Ok(())
    }

    /// O último cartão de um jogador, se foi observado.
    pub(super) fn scout_card(&self, person: i32) -> Result<Option<ScoutCard>> {
        let json: Option<String> =
            self.world.conn.query_row("SELECT card FROM scout_cards WHERE person_id = ?", [person], |r| r.get(0)).optional()?;
        Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
    }
}
