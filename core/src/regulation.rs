//! Regulamentos das competições vivas (marco 3.6): fases, participantes, datas e subidas e
//! descidas, num arquivo JSON por competição em `core/regulations/` (embutidos no executável).
//!
//! Datas por semana ISO: `w14` é o fim de semana da semana 14 (sábado; numa jornada metade
//! dos jogos passa para domingo) e `m15` o meio da semana 15 (quarta-feira). Intervalos como
//! `w15-w49`. Assim o mesmo regulamento serve para qualquer ano.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::date;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageKind {
    /// Pontos corridos.
    League,
    /// Grupos de `group_size`, pontos corridos dentro de cada grupo.
    Groups,
    /// Eliminatórias.
    Knockout,
}

/// Como se formam os grupos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupDraw {
    /// Potes pela reputação, um clube de cada pote por grupo (evitando a mesma nação).
    #[default]
    Pots,
    /// Pela classificação da fase anterior, em serpentina: 1, 4, 5, 8 / 2, 3, 6, 7.
    Snake,
}

/// Como se emparelha a 1ª eliminatória.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pairing {
    /// Pela classificação: 1º-8º, 4º-5º, 2º-7º, 3º-6º.
    #[default]
    Bracket,
    /// Dos grupos: 1º A - 2º B, 1º B - 2º A, 1º C - 2º D...
    Cross,
    /// Metade de cima contra a metade de baixo, sorteado (Copa do Brasil).
    Pots,
}

/// Quem joga em casa nas eliminatórias de jogo único.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SingleHome {
    /// O mais bem colocado.
    #[default]
    Better,
    /// O de divisão mais baixa (ou menos reputação), como na Copa do Brasil.
    Weaker,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KoRound {
    pub name: String,
    /// 1 = jogo único, 2 = ida e volta.
    pub legs: u8,
    pub dates: String,
    #[serde(default)]
    pub neutral: bool,
}

/// Uma escolha de clubes a partir dos resultados de uma fase.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Pick {
    /// Fase de onde vêm (por omissão, a anterior).
    #[serde(default)]
    pub stage: Option<usize>,
    /// Os N primeiros da classificação.
    #[serde(default)]
    pub top: Option<usize>,
    /// Os N últimos da classificação.
    #[serde(default)]
    pub bottom: Option<usize>,
    /// Os N primeiros de cada grupo.
    #[serde(default)]
    pub group_top: Option<usize>,
    /// Os vencedores da eliminatória com este índice.
    #[serde(default)]
    pub round_winners: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LeagueSlots {
    pub competition: i32,
    /// Posições na classificação final da época anterior, a partir de 1.
    pub from: usize,
    pub count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Quota {
    pub nation: String,
    pub count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BestOf {
    pub division: i32,
    pub count: usize,
}

/// Quem entra numa fase.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Entrants {
    /// Clubes destas divisões, por esta ordem.
    #[serde(default)]
    pub divisions: Vec<i32>,
    /// Os N com mais reputação de uma divisão.
    #[serde(default)]
    pub best_of: Option<BestOf>,
    /// Clubes cujo estadual é esta competição.
    #[serde(default)]
    pub estadual: bool,
    /// Da fase anterior (ou de `stage`).
    #[serde(default)]
    pub previous: Option<Pick>,
    /// Da classificação final de outra competição na época anterior.
    #[serde(default)]
    pub league: Option<LeagueSlots>,
    /// O campeão da época anterior desta competição; se já estiver dentro, entra o seguinte de `league`.
    #[serde(default)]
    pub cup_winner: Option<i32>,
    /// Clubes de outras nações: sorteio pesado pela reputação entre os mais fortes.
    #[serde(default)]
    pub foreign: Vec<Quota>,
    /// Sem os clubes que já estão nestas competições nesta época.
    #[serde(default)]
    pub exclude: Vec<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Stage {
    pub kind: StageKind,
    #[serde(default)]
    pub name: String,
    /// Pontos corridos e grupos: 1 = turno único, 2 = turno e returno.
    #[serde(default = "two")]
    pub legs: u8,
    #[serde(default)]
    pub group_size: usize,
    #[serde(default)]
    pub draw: GroupDraw,
    #[serde(default)]
    pub pairing: Pairing,
    #[serde(default)]
    pub single_home: SingleHome,
    pub entrants: Entrants,
    /// Pontos corridos e grupos: as datas das jornadas.
    #[serde(default)]
    pub dates: String,
    /// Eliminatórias: as rondas, da primeira à final.
    #[serde(default)]
    pub rounds: Vec<KoRound>,
}

fn two() -> u8 {
    2
}

/// Subida ou descida no fim da época.
#[derive(Debug, Clone, Deserialize)]
pub struct Movement {
    pub to: i32,
    #[serde(flatten)]
    pub pick: Pick,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Regulation {
    pub competition: i32,
    /// Nome nas telas e notícias, quando o do banco não serve ("Copa Sudamericana").
    #[serde(default)]
    pub name: Option<String>,
    /// Ordem em que as competições se sorteiam na nova época (participantes que dependem de outras vêm depois).
    #[serde(default)]
    pub order: i32,
    pub stages: Vec<Stage>,
    /// Datas da fase 0 na época de transição de 2025 (só as competições que jogam nela).
    #[serde(default)]
    pub transition_dates: Option<String>,
    #[serde(default)]
    pub promotion: Option<Movement>,
    #[serde(default)]
    pub relegation: Option<Movement>,
}

/// Ano da época de transição: o jogo começa a meio dela (1.7.2025).
pub const TRANSITION_YEAR: i32 = 2025;

const FILES: [&str; 19] = [
    include_str!("../regulations/serie_a.json"),
    include_str!("../regulations/serie_b.json"),
    include_str!("../regulations/serie_c.json"),
    include_str!("../regulations/serie_d.json"),
    include_str!("../regulations/copa_do_brasil.json"),
    include_str!("../regulations/libertadores.json"),
    include_str!("../regulations/sul_americana.json"),
    include_str!("../regulations/carioca.json"),
    include_str!("../regulations/paulista.json"),
    include_str!("../regulations/mineiro.json"),
    include_str!("../regulations/gaucho.json"),
    include_str!("../regulations/paranaense.json"),
    include_str!("../regulations/catarinense.json"),
    include_str!("../regulations/baiano.json"),
    include_str!("../regulations/pernambucano.json"),
    include_str!("../regulations/goiano.json"),
    include_str!("../regulations/nordeste.json"),
    include_str!("../regulations/norte.json"),
    include_str!("../regulations/central.json"),
];

/// Todos os regulamentos, pela ordem de sorteio.
pub fn all() -> &'static [Regulation] {
    static ALL: OnceLock<Vec<Regulation>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut regs: Vec<Regulation> =
            FILES.iter().map(|f| serde_json::from_str(f).unwrap_or_else(|e| panic!("regulamento inválido: {e}"))).collect();
        regs.sort_by_key(|r| (r.order, r.competition));
        regs
    })
}

pub fn get(competition: i32) -> Option<&'static Regulation> {
    all().iter().find(|r| r.competition == competition)
}

/// Segunda-feira da semana ISO 1 de `year` (a que tem o dia 4 de janeiro).
fn iso_week_one(year: i32) -> i32 {
    let jan4 = date::days_from_civil(year, 1, 4);
    // 1970-01-01 foi quinta-feira: (dias + 3) % 7 = 0 à segunda
    jan4 - (jan4 + 3).rem_euclid(7)
}

/// Uma ficha de data: (semana, é fim de semana).
fn token(t: &str) -> Option<(i32, bool)> {
    let weekend = match t.as_bytes().first()? {
        b'w' => true,
        b'm' => false,
        _ => return None,
    };
    Some((t[1..].parse().ok()?, weekend))
}

/// As datas de uma especificação em `year`, por ordem: sábados (fim de semana) e quartas.
pub fn dates(year: i32, spec: &str) -> Vec<String> {
    let monday = iso_week_one(year);
    let mut days: Vec<i32> = Vec::new();
    for part in spec.split_whitespace() {
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        let (Some((from, weekend)), Some((to, weekend2))) = (token(a), token(b)) else {
            panic!("data inválida no regulamento: {part}");
        };
        assert_eq!(weekend, weekend2, "intervalo misturado no regulamento: {part}");
        for week in from..=to {
            days.push(monday + 7 * (week - 1) + if weekend { 5 } else { 2 });
        }
    }
    days.sort_unstable();
    days.dedup();
    days.into_iter().map(date::format).collect()
}

/// Sábado = fim de semana (metade dos jogos ao domingo).
pub fn is_weekend(day: &str) -> bool {
    date::weekday(day) == Some(6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_weeks_to_dates() {
        // 2026: a semana 1 começa a 29.12.2025; a 15 tem quarta 8.4 e sábado 11.4
        assert_eq!(dates(2026, "m15 w15"), ["2026-04-08", "2026-04-11"]);
        assert_eq!(dates(2026, "w3"), ["2026-01-17"]);
        // Transição: a A de 2025 começa a 2.8 e acaba a 10.12, 38 datas
        let t = dates(2025, "w31-w49 m32-m50");
        assert_eq!((t.len(), t[0].as_str(), t[37].as_str()), (38, "2025-08-02", "2025-12-10"));
        assert!(t.iter().all(|d| matches!(date::weekday(d), Some(3 | 6))));
    }

    #[test]
    fn regulations_load_and_have_enough_dates() {
        for r in all() {
            for (i, s) in r.stages.iter().enumerate() {
                match s.kind {
                    StageKind::Knockout => {
                        assert!(!s.rounds.is_empty(), "{} fase {i}", r.competition);
                        for round in &s.rounds {
                            assert_eq!(dates(2026, &round.dates).len(), round.legs as usize, "{} {}", r.competition, round.name);
                        }
                    }
                    _ => assert!(!dates(2026, &s.dates).is_empty(), "{} fase {i}", r.competition),
                }
            }
        }
        assert!(get(65).is_some_and(|r| r.transition_dates.is_some()));
    }
}
