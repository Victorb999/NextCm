//! Regulamentos das competições vivas (marco 3.6): fases, participantes, datas e subidas e
//! descidas, num arquivo JSON por competição em `core/regulations/` (embutidos no executável).
//!
//! Datas por semana ISO: `w14` é o fim de semana da semana 14 (sábado; numa jornada metade
//! dos jogos passa para domingo) e `m15` o meio da semana 15 (quarta-feira). Intervalos como
//! `w15-w49`. Assim o mesmo regulamento serve para qualquer ano. Numa época europeia (agosto a
//! maio, marco 3.7) as semanas 27 a 53 são do ano em que a época começa e as outras do seguinte.

use std::sync::OnceLock;

use serde::Deserialize;

use crate::date;

/// Quando corre a época de uma competição.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeasonKind {
    /// Ano civil (Brasil, Argentina): a época é o ano.
    #[default]
    Calendar,
    /// Agosto a maio (Europa, Arábia Saudita): a época é o ano em que começa ("2025/26" = 2025).
    European,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageKind {
    /// Pontos corridos.
    League,
    /// Fase de liga da Liga dos Campeões: `matchdays` jornadas contra adversários diferentes, uma tabela só.
    LeaguePhase,
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

/// Os clubes mais fortes de um continente fora das nações já vivas (vagas europeias das
/// outras nações): sorteio pesado pela reputação, no máximo `per_nation` por nação.
#[derive(Debug, Clone, Deserialize)]
pub struct ContinentPick {
    pub continent: i32,
    pub count: usize,
    pub per_nation: usize,
    #[serde(default)]
    pub exclude_nations: Vec<String>,
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
    /// Como `league`, de várias ligas (vagas europeias).
    #[serde(default)]
    pub leagues: Vec<LeagueSlots>,
    #[serde(default)]
    pub continent: Option<ContinentPick>,
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
    /// Eliminatórias: os N primeiros só entram na 2ª ronda (os outros jogam a 1ª).
    #[serde(default)]
    pub byes: usize,
    /// Fase de liga: quantas jornadas.
    #[serde(default)]
    pub matchdays: usize,
}

fn two() -> u8 {
    2
}

/// Subida ou descida no fim da época.
#[derive(Debug, Clone, Deserialize)]
pub struct Movement {
    pub to: i32,
    /// Quantos, quando a divisão não jogou (sorteio pela força); por omissão 4.
    #[serde(default)]
    pub count: Option<usize>,
    #[serde(flatten)]
    pub pick: Pick,
}

/// Subida de uma divisão parada (a 3ª): sorteio pesado pela força entre os clubes de `from`.
#[derive(Debug, Clone, Deserialize)]
pub struct DrawUp {
    pub from: Vec<i32>,
    pub count: usize,
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
    #[serde(default)]
    pub season: SeasonKind,
    /// Divisão cujos plantéis evoluem, renovam contratos e recebem juniores (marco 3.5).
    #[serde(default)]
    pub managed: bool,
    /// Datas da fase 0 na época de transição, a do ano em que o jogo começa (só as competições que jogam nela).
    #[serde(default)]
    pub transition_dates: Option<String>,
    #[serde(default)]
    pub promotion: Option<Movement>,
    #[serde(default)]
    pub relegation: Option<Movement>,
    #[serde(default)]
    pub draw_up: Option<DrawUp>,
}

const FILES: [&str; 39] = [
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
    include_str!("../regulations/premier_league.json"),
    include_str!("../regulations/championship.json"),
    include_str!("../regulations/la_liga.json"),
    include_str!("../regulations/la_liga_2.json"),
    include_str!("../regulations/serie_a_italia.json"),
    include_str!("../regulations/serie_b_italia.json"),
    include_str!("../regulations/bundesliga.json"),
    include_str!("../regulations/bundesliga_2.json"),
    include_str!("../regulations/ligue_1.json"),
    include_str!("../regulations/ligue_2.json"),
    include_str!("../regulations/liga_portugal.json"),
    include_str!("../regulations/liga_portugal_2.json"),
    include_str!("../regulations/eredivisie.json"),
    include_str!("../regulations/eerste_divisie.json"),
    include_str!("../regulations/primera_argentina.json"),
    include_str!("../regulations/primera_nacional.json"),
    include_str!("../regulations/saudi_pro_league.json"),
    include_str!("../regulations/saudi_first_division.json"),
    include_str!("../regulations/champions_league.json"),
    include_str!("../regulations/europa_league.json"),
];

/// Uma faixa da classificação, como as linhas do CM: as vagas noutra competição (pela
/// classificação final), a subida e a descida. A descida conta a partir do fim.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub from: usize,
    pub to: usize,
    /// "continental" (a primeira competição com vagas), "continental2", "continental3",
    /// "promotion", "relegation".
    pub kind: &'static str,
    /// A competição para onde vão (vagas, subida ou descida).
    pub competition: Option<i32>,
    pub from_bottom: bool,
}

/// As faixas da classificação final de `competition`, tiradas dos regulamentos.
pub fn zones(competition: i32) -> Vec<Zone> {
    let mut slots: Vec<(usize, usize, i32)> = Vec::new();
    for r in all() {
        for stage in &r.stages {
            for l in stage.entrants.leagues.iter().chain(&stage.entrants.league) {
                if l.competition == competition && l.count > 0 && !slots.iter().any(|s| s.2 == r.competition) {
                    slots.push((l.from, l.from + l.count - 1, r.competition));
                }
            }
        }
    }
    slots.sort();
    const KINDS: [&str; 3] = ["continental", "continental2", "continental3"];
    let mut zones: Vec<Zone> = slots
        .iter()
        .enumerate()
        .map(|(i, &(from, to, comp))| Zone { from, to, kind: KINDS[i.min(2)], competition: Some(comp), from_bottom: false })
        .collect();
    if let Some(reg) = all().iter().find(|r| r.competition == competition) {
        if let Some(top) = reg.promotion.as_ref().and_then(|m| m.pick.top) {
            let to = reg.promotion.as_ref().map(|m| m.to);
            zones.push(Zone { from: 1, to: top, kind: "promotion", competition: to, from_bottom: false });
        }
        if let Some(bottom) = reg.relegation.as_ref().and_then(|m| m.pick.bottom) {
            let to = reg.relegation.as_ref().map(|m| m.to);
            zones.push(Zone { from: 1, to: bottom, kind: "relegation", competition: to, from_bottom: true });
        }
    }
    zones
}

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

/// Tipo de época de uma competição (as que não têm regulamento contam como ano civil).
pub fn season_kind(competition: i32) -> SeasonKind {
    get(competition).map_or(SeasonKind::Calendar, |r| r.season)
}

/// Época de um jogo de `competition` na data `day` ("AAAA-MM-DD").
pub fn season_of(competition: i32, day: &str) -> i32 {
    let year: i32 = day.get(0..4).and_then(|y| y.parse().ok()).unwrap_or(0);
    match season_kind(competition) {
        SeasonKind::Calendar => year,
        SeasonKind::European => {
            if day.get(5..7).unwrap_or("07") >= "07" {
                year
            } else {
                year - 1
            }
        }
    }
}

/// "2026" ou "2025/26".
pub fn season_label(competition: i32, season: i32) -> String {
    match season_kind(competition) {
        SeasonKind::Calendar => season.to_string(),
        SeasonKind::European => format!("{season}/{:02}", (season + 1) % 100),
    }
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
    dates_in(year, spec, SeasonKind::Calendar)
}

/// Como [`dates`]; numa época europeia as semanas abaixo de 27 são do ano seguinte.
pub fn dates_in(year: i32, spec: &str, kind: SeasonKind) -> Vec<String> {
    let mut days: Vec<i32> = Vec::new();
    for part in spec.split_whitespace() {
        let (a, b) = part.split_once('-').unwrap_or((part, part));
        let (Some((from, weekend)), Some((to, weekend2))) = (token(a), token(b)) else {
            panic!("data inválida no regulamento: {part}");
        };
        assert_eq!(weekend, weekend2, "intervalo misturado no regulamento: {part}");
        for week in from..=to {
            let y = if kind == SeasonKind::European && week < 27 { year + 1 } else { year };
            days.push(iso_week_one(y) + 7 * (week - 1) + if weekend { 5 } else { 2 });
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
        // Época europeia de 2025/26: de 9.8.2025 a 16.5.2026, sem as semanas do Natal
        let e = dates_in(2025, "w32-w51 w2-w20", SeasonKind::European);
        assert_eq!((e.len(), e[0].as_str(), e[38].as_str()), (39, "2025-08-09", "2026-05-16"));
        assert_eq!(season_of(7, "2026-05-16"), 2025);
        assert_eq!(season_of(7, "2025-08-09"), 2025);
        assert_eq!(season_of(65, "2026-05-16"), 2026);
        assert_eq!(season_label(7, 2025), "2025/26");
    }

    #[test]
    fn regulations_load_and_have_enough_dates() {
        for r in all() {
            for (i, s) in r.stages.iter().enumerate() {
                match s.kind {
                    StageKind::Knockout => {
                        assert!(!s.rounds.is_empty(), "{} fase {i}", r.competition);
                        for round in &s.rounds {
                            let n = dates_in(2026, &round.dates, r.season).len();
                            assert_eq!(n, round.legs as usize, "{} {}", r.competition, round.name);
                        }
                    }
                    StageKind::LeaguePhase => assert_eq!(dates_in(2026, &s.dates, r.season).len(), s.matchdays, "{}", r.competition),
                    _ => assert!(!dates_in(2026, &s.dates, r.season).is_empty(), "{} fase {i}", r.competition),
                }
            }
        }
        assert!(get(65).is_some_and(|r| r.transition_dates.is_some()));
    }
}
