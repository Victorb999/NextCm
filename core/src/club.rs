use std::collections::HashMap;

use rusqlite::OptionalExtension;
use serde::Serialize;
use ts_rs::TS;

use crate::person::ClubRef;
use crate::search::{NATURAL_POSITION, position_label};
use crate::{Error, Result, World};

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionRef {
    pub id: i32,
    pub name: String,
    pub short_name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SquadPlayer {
    pub id: i32,
    /// "Sobrenome, Nome" ou o nome comum.
    pub list_name: String,
    pub squad_number: Option<i32>,
    pub age: Option<i32>,
    pub nation: Option<String>,
    /// Siglas do CM, ex.: "D DC".
    pub position: String,
    /// Para ordenar por posição como o CM: GR, defesas, médios, avançados.
    pub position_order: i32,
    pub international: bool,
    /// Internacionalizações e golos pela selecção, e a sigla da nação ("ESP"), para o Vêr Outros.
    pub caps: i32,
    pub intl_goals: i32,
    pub nation_code: Option<String>,
    /// Do clube mas emprestado a outro (no plantel do dono, a azul, como no CM).
    pub loaned_out: bool,
    /// Lugar no plantel pela habilidade (1 = o melhor), para o critério "Habilidade" sem
    /// mostrar o CA, que é interno.
    pub ability_rank: i32,
    #[serde(skip)]
    #[ts(skip)]
    pub current_ability: Option<i32>,
    #[serde(skip)]
    #[ts(skip)]
    pub potential_ability: Option<i32>,
    /// Num jogo em curso: condição (0–100) e indisponibilidade.
    pub condition: Option<i32>,
    pub injured: bool,
    pub suspended: bool,
    /// Na equipa de reservas (só no clube do treinador, num jogo em curso).
    pub reserve: bool,
    /// Jogador de enchimento (nome a roxo, como no CM).
    pub ghost: bool,
    /// Grupos de posição para o Filtrar: "GR", "D", "M", "A".
    pub lines: Vec<String>,
    /// Lados para o Filtrar: "E", "C", "D" (o guarda-redes conta como central).
    pub sides: Vec<String>,
    pub wage: Option<i32>,
    pub contract_expires: Option<String>,
    /// Um dos três jogadores-chave do plantel (o `*` depois do nome, como no CM).
    pub key: bool,
    /// A sigla da coluna Inf: "Les", "Sus", "Emp", "Ctr" (contrato acabado, ainda no clube), "Lst",
    /// "Des" (desejado por outro clube) ou "Ins" (insatisfeito) ("Int", fora na selecção,
    /// quando houver convocatórias).
    pub info: Option<String>,
    /// Emprestado por outro clube (nome a azul-claro).
    pub loaned: bool,
    /// Sem contrato (nome a roxo, "Livre" no valor).
    pub free: bool,
    /// Num jogo em curso: o resto do Vêr (Situação, Moral, Forma, a época e o valor).
    pub role: Option<String>,
    pub morale: Option<String>,
    pub form: Vec<f64>,
    pub clauses: Vec<String>,
    #[ts(type = "number | null")]
    pub value: Option<i64>,
    pub apps: i32,
    pub subs: i32,
    pub goals: i32,
    pub assists: i32,
    pub mom: i32,
    pub rating: Option<f64>,
    /// Lugar na ordem dos batedores de penáltis (1 = o primeiro); `None` no guarda-redes.
    pub penalty_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct StaffMember {
    pub id: i32,
    pub list_name: String,
    pub job: Option<String>,
    pub job_id: Option<i32>,
    pub age: Option<i32>,
    pub nation: Option<String>,
    #[serde(skip)]
    #[ts(skip)]
    pub current_ability: Option<i32>,
}

/// Títulos e vice-campeonatos do clube numa competição (do historial das competições).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Honour {
    pub competition: CompetitionRef,
    /// Anos, do mais recente para o mais antigo.
    pub won: Vec<i32>,
    pub runner_up: Vec<i32>,
}

/// A equipa B ou II de um clube: o clube, a liga dela e se é uma "II" (a Amateure do Bayern),
/// que no CM faz de equipa de reservas; as "B" (Barça B, Benfica B) são clubes à parte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BTeam {
    pub club: i32,
    pub division: i32,
    pub second: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClubProfile {
    pub club: ClubRef,
    pub is_national_team: bool,
    pub nation_id: Option<i32>,
    pub nation: Option<String>,
    pub division: Option<CompetitionRef>,
    /// No patch CLZ: o campeonato estadual.
    pub reserve_division: Option<CompetitionRef>,
    /// No clube do treinador: o nome do ecrã das reservas ("Reservas Manchester United", ou o
    /// da II, como a Amateure do Bayern); `None` sem reservas (com uma equipa B à parte).
    pub reserves_name: Option<String>,
    /// Num jogo em curso: o lugar na classificação do campeonato (depois da 1ª jornada).
    pub league_position: Option<i32>,
    /// Divisão e posição na temporada anterior.
    pub last_division: Option<CompetitionRef>,
    pub last_position: Option<i32>,
    pub stadium: Option<String>,
    pub stadium_capacity: Option<i32>,
    /// Cidade do estádio e lugares sentados.
    pub stadium_city: Option<String>,
    pub stadium_seated: Option<i32>,
    /// Com aquecimento subterrâneo no relvado.
    pub undersoil_heating: bool,
    /// Com telhado retráctil.
    pub retractable_roof: bool,
    /// O estádio é do clube (e não alugado ou partilhado).
    pub owns_stadium: bool,
    /// O dia da semana dos jogos em casa ("Sábado"), quando o banco o tem.
    pub match_day: Option<String>,
    /// A assistência do clube no banco: média, mínima e máxima.
    pub attendance: Option<[i32; 3]>,
    /// As instalações de treino, como no CM ("Excelentes condições, com academia para Jovens").
    pub facilities: Option<String>,
    /// "Profissional", "Semi-profissional" ou "Amador".
    pub status: Option<String>,
    pub reputation: Option<i32>,
    pub cash: Option<i32>,
    pub rivals: Vec<ClubRef>,
    pub players: Vec<SquadPlayer>,
    pub staff: Vec<StaffMember>,
    /// Competições com título ou vice, as com mais títulos primeiro.
    pub honours: Vec<Honour>,
    /// Clube de reservas ou juniores (U21, B, II, "(R)"): o clube principal.
    pub parent: Option<ClubRef>,
    /// No clube principal: os clubes de reservas e juniores ligados a ele.
    pub affiliates: Vec<ClubRef>,
}

/// A competição "Reserve" do banco do CM (87): os clubes "(R)" e, num jogo do CM, os jogos
/// das reservas.
const RESERVE_DIVISION: i32 = 87;

/// Sufixos do nome curto das equipas B, e os do nome completo. Como no CM, só a equipa B (ou
/// II, "Amateure") fica ligada ao clube principal (o "Bayern Munich II" no menu do Bayern); os
/// sub-19, sub-21 e "(R)" são outros clubes.
const SHORT_SUFFIXES: [&str; 2] = [" II", " B"];
const NAME_SUFFIXES: [&str; 3] = [" B", " II", " Amateure"];

/// Espaços repetidos do banco ("Botafogo  U21") contam como um.
fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// O nome sem o sufixo de reservas, se o tiver.
fn strip_suffix<'a>(name: &'a str, suffixes: &[&str]) -> Option<&'a str> {
    suffixes.iter().find_map(|s| name.strip_suffix(s)).filter(|b| !b.is_empty())
}

/// Grupos de posição e lados (posição com 15 ou mais), para o Filtrar do plantel.
pub(crate) fn lines_and_sides(p: &[Option<i32>; 12]) -> (Vec<String>, Vec<String>) {
    let natural = |i: usize| p[i].unwrap_or(0) >= NATURAL_POSITION;
    if natural(0) {
        return (vec!["GR".into()], vec!["C".into()]);
    }
    // Ordem das colunas: gk, sweeper, defender, dm, m, am, attacker, wing_back, r, l, c, free
    let mut lines = Vec::new();
    if natural(1) || natural(2) || natural(7) {
        lines.push("D".into());
    }
    if natural(3) || natural(4) || natural(5) {
        lines.push("M".into());
    }
    if natural(6) {
        lines.push("A".into());
    }
    let sides = [(9, "E"), (10, "C"), (8, "D")].into_iter().filter(|(i, _)| natural(*i)).map(|(_, s)| s.to_string()).collect();
    (lines, sides)
}

/// Ordem dos bits de `people.positions` (GR=1, D=2, AL=4, MD=8, M=16, MA=32, A=64):
/// a posição mais defensiva define o lugar na lista.
fn position_order(mask: i32) -> i32 {
    (0..7).find(|b| mask & (1 << b) != 0).unwrap_or(7)
}

/// O dia dos jogos em casa do banco (1 terça a 6 domingo; a segunda não existe: 0 é "sem dados").
pub fn match_day_text(day: i32) -> Option<&'static str> {
    Some(match day {
        1 => "Terça-feira",
        2 => "Quarta-feira",
        3 => "Quinta-feira",
        4 => "Sexta-feira",
        5 => "Sábado",
        6 => "Domingo",
        _ => return None,
    })
}

/// As instalações de treino (1 a 20) pelas frases da Informação Geral do CM; 0 é sem dados.
pub fn facilities_text(training: i32) -> Option<&'static str> {
    Some(match training {
        18.. => "Excelentes condições, com academia para Jovens",
        15..=17 => "Excelentes condições",
        11..=14 => "Boas condições",
        7..=10 => "Condições razoáveis",
        4..=6 => "Condições fracas",
        1..=3 => "Condições básicas",
        _ => return None,
    })
}

impl World {
    fn competition_ref(&self, id: Option<i32>) -> Result<Option<CompetitionRef>> {
        let Some(id) = id else { return Ok(None) };
        Ok(self
            .conn
            .prepare_cached("SELECT id, name, short_name FROM competitions WHERE id = ?")?
            .query_row([id], |r| {
                let name: String = r.get(1)?;
                let short: String = r.get(2)?;
                Ok(CompetitionRef { id: r.get(0)?, short_name: if short.is_empty() { name.clone() } else { short }, name })
            })
            .optional()?)
    }

    /// Os jogadores de um plantel (`filter` sobre `people p`, com um `?` para `arg`).
    pub(crate) fn squad_players(&self, filter: &str, arg: i32) -> Result<Vec<SquadPlayer>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT p.id, p.list_name, pl.squad_number, p.birth_date, p.birth_year, n.name,
                    p.positions, p.international_apps, pl.current_ability, pl.potential_ability,
                    pl.pos_goalkeeper, pl.pos_sweeper, pl.pos_defender, pl.pos_defensive_midfielder,
                    pl.pos_midfielder, pl.pos_attacking_midfielder, pl.pos_attacker, pl.pos_wing_back,
                    pl.pos_right_side, pl.pos_left_side, pl.pos_central, pl.pos_free_role,
                    p.wage, p.contract_expires, n.code, p.international_goals
             FROM people p
             JOIN players pl ON pl.person_id = p.id
             LEFT JOIN nations n ON n.id = p.nation_id
             WHERE {filter}"
        ))?;
        let players = stmt
            .query_map([arg], |r| {
                let mut pos = [None; 12];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = r.get(10 + i)?;
                }
                let birth_date: Option<String> = r.get(3)?;
                let (lines, sides) = lines_and_sides(&pos);
                Ok(SquadPlayer {
                    id: r.get(0)?,
                    list_name: r.get(1)?,
                    squad_number: r.get(2)?,
                    age: self.age_from(birth_date.as_deref(), r.get(4)?),
                    nation: r.get(5)?,
                    position_order: position_order(r.get(6)?),
                    international: r.get::<_, i32>(7)? > 0,
                    caps: r.get::<_, Option<i32>>(7)?.unwrap_or(0),
                    intl_goals: r.get::<_, Option<i32>>(25)?.unwrap_or(0),
                    nation_code: r.get(24)?,
                    loaned_out: false,
                    ability_rank: 0,
                    current_ability: r.get(8)?,
                    potential_ability: r.get(9)?,
                    condition: None,
                    injured: false,
                    suspended: false,
                    reserve: false,
                    ghost: false,
                    position: position_label(&pos),
                    lines,
                    sides,
                    wage: r.get::<_, Option<i32>>(22)?.filter(|w| *w > 0),
                    contract_expires: r.get(23)?,
                    key: false,
                    info: None,
                    loaned: false,
                    free: false,
                    role: None,
                    morale: None,
                    form: Vec::new(),
                    clauses: Vec::new(),
                    value: None,
                    apps: 0,
                    subs: 0,
                    goals: 0,
                    assists: 0,
                    mom: 0,
                    rating: None,
                    penalty_order: None,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(players)
    }

    pub fn get_club(&self, id: i32) -> Result<ClubProfile> {
        let Some(club) = self.club_ref(Some(id))? else {
            return Err(Error::NotFound(format!("clube {id}")));
        };
        type Row = (
            bool,
            Option<i32>,
            Option<String>,
            Option<i32>,
            Option<i32>,
            Option<String>,
            Option<i32>,
            Option<i32>,
            Option<i32>,
            [Option<i32>; 3],
            Option<i32>,
            Option<i32>,
        );
        let (is_national, nation_id, nation, division, reserve, stadium, capacity, reputation, cash, rivals, last_division, last_position): Row = self
            .conn
            .prepare_cached(
                "SELECT c.is_national_team, c.nation_id, n.name, c.division_id, c.reserve_division_id,
                        s.name, s.capacity, c.reputation, c.cash,
                        c.rival1_id, c.rival2_id, c.rival3_id, c.last_division_id, c.last_position
                 FROM clubs c
                 LEFT JOIN nations n ON n.id = c.nation_id
                 LEFT JOIN stadiums s ON s.id = c.stadium_id
                 WHERE c.id = ?",
            )?
            .query_row([id], |r| {
                Ok((
                    r.get::<_, i32>(0)? != 0,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    [r.get(9)?, r.get(10)?, r.get(11)?],
                    r.get(12)?,
                    r.get(13)?,
                ))
            })?;

        // Seleções: os jogadores ficam ligados por national_team_id, não por club_id.
        let link = if is_national { "p.national_team_id" } else { "p.club_id" };

        let mut players = self.squad_players(&format!("{link} = ?"), id)?;
        let mut order: Vec<usize> = (0..players.len()).collect();
        order.sort_by_key(|&i| (std::cmp::Reverse(players[i].current_ability.unwrap_or(0)), players[i].id));
        for (rank, i) in order.into_iter().enumerate() {
            players[i].ability_rank = rank as i32 + 1;
            // Sem jogo: os três melhores pela habilidade (no jogo, pelo valor: `Game::fill_squad`)
            players[i].key = rank < 3;
        }

        let job_link = if is_national { "p.national_team_job" } else { "p.club_job" };
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT p.id, p.list_name, j.name, j.id, p.birth_date, p.birth_year, n.name, np.current_ability
             FROM people p
             JOIN jobs j ON j.id = {job_link}
             LEFT JOIN nations n ON n.id = p.nation_id
             LEFT JOIN non_players np ON np.person_id = p.id
             WHERE {link} = ? AND j.id != 11
             ORDER BY j.id, p.list_name"
        ))?;
        let staff = stmt
            .query_map([id], |r| {
                let birth_date: Option<String> = r.get(4)?;
                Ok(StaffMember {
                    id: r.get(0)?,
                    list_name: r.get(1)?,
                    job: r.get(2)?,
                    job_id: r.get(3)?,
                    age: self.age_from(birth_date.as_deref(), r.get(5)?),
                    nation: r.get(6)?,
                    current_ability: r.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        // Cidade do estádio, lugares sentados e estatuto profissional
        let (stadium_city, seated, professional_status): (Option<String>, Option<i32>, Option<i32>) = self
            .conn
            .prepare_cached(
                "SELECT ci.name, s.seating_capacity, c.professional_status FROM clubs c
                 LEFT JOIN stadiums s ON s.id = c.stadium_id LEFT JOIN cities ci ON ci.id = s.city_id
                 WHERE c.id = ?",
            )?
            .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;

        // Instalações de treino e aquecimento do relvado (num banco de antes do esquema 8, nada)
        let (training, heating): (Option<i32>, bool) = self
            .conn
            .prepare_cached(
                "SELECT c.training, COALESCE(s.undersoil_heating, 0) <> 0 FROM clubs c
                 LEFT JOIN stadiums s ON s.id = c.stadium_id WHERE c.id = ?",
            )
            .and_then(|mut st| st.query_row([id], |r| Ok((r.get(0)?, r.get(1)?))))
            .unwrap_or((None, false));

        // O telhado, o estádio próprio, o dia do jogo e a assistência (banco de antes do esquema 9: nada)
        type Extras = (bool, bool, Option<i32>, [Option<i32>; 3]);
        let (roof, owns, day, crowd): Extras = self
            .conn
            .prepare_cached(
                "SELECT COALESCE(s.retractable_roof, 0) <> 0, COALESCE(c.owns_stadium, 0) <> 0, c.match_day,
                        c.avg_attendance, c.min_attendance, c.max_attendance
                 FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id WHERE c.id = ?",
            )
            .and_then(|mut st| st.query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, [r.get(3)?, r.get(4)?, r.get(5)?]))))
            .unwrap_or((false, false, None, [None; 3]));

        let mut rival_refs = Vec::new();
        for rival in rivals.into_iter().flatten() {
            rival_refs.extend(self.club_ref(Some(rival))?);
        }

        let mut honours: Vec<Honour> = Vec::new();
        let rows: Vec<(i32, i32, bool)> = self
            .conn
            .prepare_cached(
                "SELECT competition_id, year, winner_id = ?1 FROM competition_history
                 WHERE (winner_id = ?1 OR runner_up_id = ?1) AND year IS NOT NULL
                 ORDER BY year DESC",
            )?
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (comp, year, won) in rows {
            let i = match honours.iter().position(|h| h.competition.id == comp) {
                Some(i) => i,
                None => {
                    let Some(competition) = self.competition_ref(Some(comp))? else { continue };
                    honours.push(Honour { competition, won: Vec::new(), runner_up: Vec::new() });
                    honours.len() - 1
                }
            };
            // O historial do patch repete alguns anos; cada ano conta uma vez
            let years = if won { &mut honours[i].won } else { &mut honours[i].runner_up };
            if !years.contains(&year) {
                years.push(year);
            }
        }
        honours.sort_by(|a, b| b.won.len().cmp(&a.won.len()).then(b.runner_up.len().cmp(&a.runner_up.len())));

        let parent = if is_national { None } else { self.parent_club(&club, nation_id)? };
        let affiliates = if is_national { Vec::new() } else { self.affiliates(&club, nation_id)? };
        Ok(ClubProfile {
            club,
            is_national_team: is_national,
            nation_id,
            nation,
            division: self.competition_ref(division)?,
            reserve_division: self.competition_ref(reserve)?,
            reserves_name: None,
            league_position: None,
            last_division: self.competition_ref(last_division)?,
            last_position: last_position.filter(|p| *p > 0),
            stadium,
            stadium_capacity: capacity.filter(|c| *c > 0),
            stadium_city,
            stadium_seated: seated.filter(|c| *c > 0),
            undersoil_heating: heating,
            retractable_roof: roof,
            owns_stadium: owns,
            match_day: day.and_then(match_day_text).map(String::from),
            attendance: match crowd {
                [Some(avg), min, max] => Some([avg, min.unwrap_or(avg), max.unwrap_or(avg)]),
                _ => None,
            },
            facilities: training.and_then(facilities_text).map(String::from),
            status: match professional_status {
                Some(1) => Some("Profissional".into()),
                Some(2) => Some("Semi-profissional".into()),
                Some(3) => Some("Amador".into()),
                _ => None,
            },
            reputation,
            cash,
            rivals: rival_refs,
            players,
            staff,
            honours,
            parent,
            affiliates,
        })
    }

    /// O clube principal de um clube de reservas ou juniores: o nome sem o sufixo, na mesma nação.
    fn parent_club(&self, club: &ClubRef, nation: Option<i32>) -> Result<Option<ClubRef>> {
        let short = squash(&club.short_name);
        let name = squash(&club.name);
        let candidates = [(strip_suffix(&short, &SHORT_SUFFIXES), "short_name"), (strip_suffix(&name, &NAME_SUFFIXES), "name")];
        for (base, column) in candidates {
            let Some(base) = base else { continue };
            let id: Option<i32> = self
                .conn
                .prepare_cached(&format!(
                    "SELECT id FROM clubs WHERE nation_id IS ?1 AND id <> ?2 AND is_national_team = 0
                     AND TRIM(REPLACE({column}, '  ', ' ')) = ?3 ORDER BY reputation DESC LIMIT 1"
                ))?
                .query_row(rusqlite::params![nation, club.id, base], |r| r.get(0))
                .optional()?;
            if let Some(id) = id {
                return self.club_ref(Some(id));
            }
        }
        Ok(None)
    }

    /// As Preferências do CM (tabela 22 do `staff.dat`): `Some(true)` se o clube é um dos
    /// favoritos da pessoa, `Some(false)` se o detesta.
    pub fn club_preference(&self, person: i32, club: i32) -> Result<Option<bool>> {
        Ok(self
            .conn
            .prepare_cached("SELECT liked FROM preferences WHERE person_id = ? AND kind = 'club' AND target_id = ? LIMIT 1")?
            .query_row([person, club], |r| r.get::<_, i32>(0))
            .optional()?
            .map(|l| l != 0))
    }

    /// Os jogadores que têm o clube entre os favoritos (`true`) ou os detestados (`false`).
    pub fn club_fans(&self, club: i32) -> Result<HashMap<i32, bool>> {
        Ok(self
            .conn
            .prepare_cached("SELECT person_id, liked FROM preferences WHERE kind = 'club' AND target_id = ?")?
            .query_map([club], |r| Ok((r.get(0)?, r.get::<_, i32>(1)? != 0)))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// O clube tem uma equipa B ou II a jogar numa liga (o Barcelona B, o Bayern II).
    pub fn has_b_team(&self, id: i32) -> Result<bool> {
        Ok(self.b_team(id)?.is_some())
    }

    /// A liga onde joga a equipa B ou II do clube, se tiver uma.
    pub fn b_team_division(&self, id: i32) -> Result<Option<i32>> {
        Ok(self.b_team(id)?.map(|b| b.division))
    }

    /// A equipa B ou II do clube (pelo nome, como no CM: o `club.dat` não as liga).
    pub fn b_team(&self, id: i32) -> Result<Option<BTeam>> {
        let Some(club) = self.club_ref(Some(id))? else { return Ok(None) };
        let nation: Option<i32> = self.conn.query_row("SELECT nation_id FROM clubs WHERE id = ?", [id], |r| r.get(0))?;
        for c in self.affiliates(&club, nation)? {
            let short = squash(&c.short_name);
            let division: Option<i32> = self.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [c.id], |r| r.get(0))?;
            let Some(division) = division.filter(|d| *d != RESERVE_DIVISION) else { continue };
            if short.ends_with(" B") || short.ends_with(" II") {
                return Ok(Some(BTeam { club: c.id, division, second: short.ends_with(" II") }));
            }
        }
        Ok(None)
    }

    /// Os clubes de reservas e juniores cujo clube principal é este.
    fn affiliates(&self, club: &ClubRef, nation: Option<i32>) -> Result<Vec<ClubRef>> {
        if strip_suffix(&squash(&club.short_name), &SHORT_SUFFIXES).is_some() {
            return Ok(Vec::new());
        }
        let ids: Vec<i32> = self
            .conn
            .prepare_cached(
                "SELECT id FROM clubs WHERE nation_id IS ?1 AND id <> ?2 AND is_national_team = 0
                 AND (short_name LIKE ?3 || ' %' OR name LIKE ?4 || ' %') ORDER BY short_name",
            )?
            .query_map(rusqlite::params![nation, club.id, squash(&club.short_name), squash(&club.name)], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for id in ids {
            let Some(c) = self.club_ref(Some(id))? else { continue };
            if self.parent_club(&c, nation)?.is_some_and(|p| p.id == club.id) {
                out.push(c);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_by_most_defensive_position() {
        assert_eq!(position_order(1), 0); // GR
        assert_eq!(position_order(2 | 64), 1); // D/A -> defesa
        assert_eq!(position_order(64 | 512), 6); // A
        assert_eq!(position_order(0), 7); // sem posição natural
    }

    #[test]
    fn barcelona_squad() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        let yamal =
            w.search_people(&crate::SearchQuery { text: Some("lamine yamal".into()), ..Default::default() }).unwrap().items.remove(0);
        let club = w.get_club(yamal.club_id.unwrap()).unwrap();
        assert_eq!(club.club.short_name, "Barcelona");
        assert_eq!(club.division.as_ref().map(|d| d.short_name.as_str()), Some("La Liga"));
        assert!(club.players.len() >= 20);
        assert!(club.players.iter().any(|p| p.id == yamal.id));
        assert!(club.staff.iter().any(|s| s.job_id == Some(5)), "tem treinador");
        assert_eq!(club.players.iter().filter(|p| p.key).count(), 3);
    }

    #[test]
    fn facilities_and_undersoil_heating() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        // Bayern: instalações 20 e o Allianz Arena aquecido; o Allianz Parque não tem aquecimento
        let bayern = w.get_club(1350).unwrap();
        assert_eq!(bayern.facilities.as_deref(), Some("Excelentes condições, com academia para Jovens"));
        assert!(bayern.undersoil_heating);
        let palmeiras = w.get_club(7568).unwrap();
        assert!(!palmeiras.undersoil_heating);
        assert_eq!(facilities_text(3), Some("Condições básicas"));
        assert_eq!(facilities_text(0), None);
    }

    #[test]
    fn stadium_roof_attendance_and_match_day() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        // Bayern: estádio próprio, jogos ao sábado e assistência do banco
        let bayern = w.get_club(1350).unwrap();
        assert!(bayern.owns_stadium);
        assert_eq!(bayern.match_day.as_deref(), Some("Sábado"));
        assert_eq!(bayern.attendance, Some([75_000, 75_000, 88_000]));
        assert!(!w.get_club(7568).unwrap().retractable_roof);
        let roofs: i64 = w.conn.query_row("SELECT COUNT(*) FROM stadiums WHERE retractable_roof <> 0", [], |r| r.get(0)).unwrap();
        assert!(roofs > 10, "{roofs} estádios com telhado");
        assert_eq!(match_day_text(6), Some("Domingo"));
        assert_eq!(match_day_text(0), None);
    }

    #[test]
    fn reserve_clubs_link_to_the_main_club() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        // Só as equipas B: Bayern Munich II (1353) -> Bayern (1350); o U19 (1351) e o U17 são outros
        // clubes, como o Palmeiras U21 (7570) e o Alavés (R) (301); Alavés B (302) -> Alavés
        assert_eq!(w.get_club(1353).unwrap().parent.map(|c| c.id), Some(1350));
        let bayern = w.get_club(1350).unwrap();
        assert_eq!(bayern.affiliates.iter().map(|c| c.id).collect::<Vec<_>>(), vec![1353]);
        assert!(w.get_club(1351).unwrap().parent.is_none());
        assert!(w.get_club(7570).unwrap().parent.is_none());
        assert!(w.get_club(7568).unwrap().affiliates.iter().all(|c| c.id != 7570));
        assert!(w.get_club(302).unwrap().parent.is_some());
        assert!(w.get_club(301).unwrap().parent.is_none());
    }

    #[test]
    fn lines_and_sides_for_the_filter() {
        let mut p = [Some(1); 12];
        p[2] = Some(20);
        p[7] = Some(15);
        p[9] = Some(20);
        assert_eq!(lines_and_sides(&p), (vec!["D".to_string()], vec!["E".to_string()]));
        p[0] = Some(20);
        assert_eq!(lines_and_sides(&p).0, ["GR"]);
    }
}
