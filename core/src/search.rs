use rusqlite::{ToSql, params_from_iter};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use unicode_normalization::UnicodeNormalization;

use crate::{Result, World};

/// Atributos mentais, na tabela `people`.
const MENTAL: [&str; 8] =
    ["adaptability", "ambition", "determination", "loyalty", "pressure", "professionalism", "sportsmanship", "temperament"];
/// Os que a normalização física do perfil pode baixar 1–2 pontos (`attributes::normalise_physical`).
const PHYSICAL_CUT: [&str; 4] = ["acceleration", "pace", "technique", "work_rate"];
/// O que essa normalização lê.
const PHYSICAL_INPUTS: [&str; 9] =
    ["pace", "leadership", "jumping", "acceleration", "teamwork", "technique", "left_foot", "right_foot", "work_rate"];

/// Valor mínimo (1–20) para considerar uma posição "natural".
pub(crate) const NATURAL_POSITION: i32 = 15;
const MAX_LIMIT: u32 = 200;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum PersonKind {
    #[default]
    All,
    Players,
    Staff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Position {
    GK,
    D,
    WB,
    DM,
    M,
    AM,
    F,
}

impl Position {
    /// Bit em `people.positions` (mesmos valores de POSITION_BITS no extract.py).
    fn bit(self) -> i32 {
        match self {
            Position::GK => 1,
            Position::D => 2,
            Position::WB => 4,
            Position::DM => 8,
            Position::M => 16,
            Position::AM => 32,
            Position::F => 64,
        }
    }
}

/// Lado do campo (bits 128/256/512 de `people.positions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Right,
    Left,
    Center,
}

impl Side {
    fn bit(self) -> i32 {
        match self {
            Side::Right => 128,
            Side::Left => 256,
            Side::Center => 512,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum SearchSort {
    #[default]
    Ability,
    Name,
    Age,
    /// Pelo valor de mercado na data do jogo (calculado, não há coluna).
    Value,
}

/// Situação no mercado (o "Vêr" da pesquisa do CM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum MarketStatus {
    /// Na lista de transferências do clube.
    Transfer,
    /// Na lista de empréstimos do clube.
    Loan,
    /// Jogador sem clube.
    Free,
    /// Na Lista Preferencial do treinador.
    Shortlist,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(default, rename_all = "camelCase")]
pub struct SearchQuery {
    #[ts(optional)]
    pub text: Option<String>,
    #[ts(optional)]
    pub kind: Option<PersonKind>,
    #[ts(optional)]
    pub nation_id: Option<i32>,
    /// Qualquer uma das posições (OU). Vazio = qualquer posição.
    #[ts(optional)]
    pub positions: Option<Vec<Position>>,
    #[ts(optional)]
    pub side: Option<Side>,
    #[ts(optional)]
    pub min_age: Option<i32>,
    #[ts(optional)]
    pub max_age: Option<i32>,
    #[ts(optional)]
    pub status: Option<MarketStatus>,
    /// Só os com o contrato a acabar (6 meses ou menos).
    #[ts(optional)]
    pub expiring: Option<bool>,
    /// Lesionados: só eles (`true`) ou sem eles (`false`); o jogo em curso põe a lista em `injured_ids`.
    #[ts(optional)]
    pub injured: Option<bool>,
    /// Só os que aceitam vir para o clube do treinador; o jogo em curso põe em `unwilling` quem recusa.
    #[ts(optional)]
    pub interested: Option<bool>,
    #[serde(skip)]
    #[ts(skip)]
    pub unwilling: Option<String>,
    /// A aba Atributos dos filtros: o mínimo de cada atributo (como o perfil o mostra).
    #[ts(optional)]
    pub attributes: Option<Vec<AttributeMin>>,
    /// O "Igualar" do CM: quantos desses mínimos tem de cumprir (sem nada, todos).
    #[ts(optional)]
    pub attributes_needed: Option<u32>,
    /// Sem os jogadores deste clube ("Incluir Plantel: Não").
    #[ts(optional)]
    pub exclude_club: Option<i32>,
    #[serde(skip)]
    #[ts(skip)]
    pub injured_ids: Vec<i32>,
    #[ts(optional)]
    pub sort: Option<SearchSort>,
    #[ts(optional)]
    pub limit: Option<u32>,
    #[ts(optional)]
    pub offset: Option<u32>,
}

/// Um mínimo da aba Atributos dos filtros ("Velocidade 15+").
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AttributeMin {
    pub key: String,
    pub min: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PersonSummary {
    pub id: i32,
    pub name: String,
    /// Como nas listas do CM: "Sobrenome, Nome" (ou o nome comum).
    pub list_name: String,
    pub age: Option<i32>,
    pub nation_id: Option<i32>,
    pub nation: Option<String>,
    pub club_id: Option<i32>,
    pub club: Option<String>,
    pub job: Option<String>,
    /// Siglas do CM em português. Ex.: "MA/A DEC", "GR". Vazio para quem não é jogador.
    pub position: String,
    /// Valor de mercado (só jogadores). CA e PA não saem do core: são internos.
    #[ts(type = "number | null")]
    pub value: Option<i64>,
    /// A coluna Inf do CM (num jogo em curso): Pro, Les, Sus, Ped, Lst, Emp, Amr.
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub total: u32,
    pub items: Vec<PersonSummary>,
}

/// O Procurar do CM: tudo o que tem o nome escrito, por grupos.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FindResult {
    pub clubs: Vec<crate::ClubListItem>,
    pub nations: Vec<crate::NationOption>,
    pub competitions: Vec<crate::CompetitionRef>,
    pub players: Vec<PersonSummary>,
    pub staff: Vec<PersonSummary>,
}

/// Letras mínimas para procurar, e quantos de cada grupo.
pub const FIND_MIN: usize = 2;
const FIND_GROUP: usize = 15;

impl World {
    /// As nações e as competições com o nome escrito (as competições também pelo nome do
    /// regulamento, "Mundial"); os clubes pela pesquisa de clubes.
    pub fn find_places(&self, text: &str) -> Result<FindResult> {
        let text = text.trim();
        if text.chars().count() < FIND_MIN {
            return Ok(FindResult::default());
        }
        let like = format!("%{text}%");
        let nations: Vec<crate::NationOption> = self
            .conn
            .prepare_cached(
                "SELECT n.id, n.name FROM nations n WHERE n.name LIKE ?1
                   AND EXISTS (SELECT 1 FROM people p WHERE p.nation_id = n.id) ORDER BY n.name LIMIT ?2",
            )?
            .query_map(rusqlite::params![like, FIND_GROUP as i64], |r| Ok(crate::NationOption { id: r.get(0)?, name: r.get(1)? }))?
            .collect::<rusqlite::Result<_>>()?;
        let wanted = fold(text);
        let rows: Vec<(i32, String, String)> = self
            .conn
            .prepare_cached(
                "SELECT id, COALESCE(name, ''), COALESCE(short_name, '') FROM competitions ORDER BY COALESCE(reputation, 0) DESC, id",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut competitions = Vec::new();
        for (id, name, short) in rows {
            let ours = crate::regulation::get(id).and_then(|r| r.name.clone());
            if ![Some(&name), Some(&short), ours.as_ref()].into_iter().flatten().any(|n| fold(n).contains(&wanted)) {
                continue;
            }
            let shown = ours.unwrap_or_else(|| if short.is_empty() { name.clone() } else { short.clone() });
            competitions.push(crate::CompetitionRef { id, name: shown.clone(), short_name: shown });
            if competitions.len() >= FIND_GROUP {
                break;
            }
        }
        let mut clubs = self.search_clubs(text)?;
        clubs.truncate(FIND_GROUP);
        Ok(FindResult { clubs, nations, competitions, ..Default::default() })
    }
}

/// Minúsculas sem acento, igual ao `fold()` do extract.py.
pub(crate) fn fold(s: &str) -> String {
    s.nfkd().filter(|c| !unicode_normalization::char::is_combining_mark(*c)).collect::<String>().to_lowercase()
}

pub(crate) fn position_label(p: &[Option<i32>; 12]) -> String {
    let natural = |i: usize| p[i].unwrap_or(0) >= NATURAL_POSITION;
    if natural(0) {
        return "GR".into();
    }
    // Ordem das colunas: gk, sweeper, defender, dm, m, am, attacker, wing_back, r, l, c, free
    let roles: Vec<&str> = [(1, "L"), (2, "D"), (7, "AL"), (3, "MD"), (4, "M"), (5, "MA"), (6, "A")]
        .into_iter()
        .filter(|(i, _)| natural(*i))
        .map(|(_, label)| label)
        .collect();
    // Lados: Direita, Esquerda, Centro
    let sides: String = [(8, 'D'), (9, 'E'), (10, 'C')].into_iter().filter(|(i, _)| natural(*i)).map(|(_, s)| s).collect();
    match (roles.is_empty(), sides.is_empty()) {
        (true, _) => String::new(),
        (false, true) => roles.join("/"),
        (false, false) => format!("{} {}", roles.join("/"), sides),
    }
}

/// Uma posição com lado ("MAD", "AC", "GR") para as etiquetas do perfil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PositionTag {
    pub code: String,
    /// "GR", "L", "D", "AL", "MD", "M", "MA" ou "A".
    pub role: String,
    /// "D", "E" ou "C" (nada no guarda-redes e em quem não tem lado).
    pub side: Option<String>,
    /// Posição e lado com 20: a de origem.
    pub natural: bool,
}

/// As posições em que joga (função e lado com 15 ou mais), cada uma com o seu lado.
pub(crate) fn position_tags(p: &[Option<i32>; 12]) -> Vec<PositionTag> {
    let r = |i: usize| p[i].unwrap_or(0);
    if r(0) >= NATURAL_POSITION {
        return vec![PositionTag { code: "GR".into(), role: "GR".into(), side: None, natural: r(0) >= 20 }];
    }
    let sides: Vec<(usize, &str)> = [(8, "D"), (9, "E"), (10, "C")].into_iter().filter(|&(i, _)| r(i) >= NATURAL_POSITION).collect();
    let mut out = Vec::new();
    for (i, role) in [(1, "L"), (2, "D"), (7, "AL"), (3, "MD"), (4, "M"), (5, "MA"), (6, "A")] {
        if r(i) < NATURAL_POSITION {
            continue;
        }
        if sides.is_empty() {
            out.push(PositionTag { code: role.into(), role: role.into(), side: None, natural: r(i) >= 20 });
        }
        for &(s, side) in &sides {
            out.push(PositionTag {
                code: format!("{role}{side}"),
                role: role.into(),
                side: Some(side.into()),
                natural: r(i).min(r(s)) >= 20,
            });
        }
    }
    out
}

impl World {
    /// Pesquisa no início do jogo (sem jogo aberto).
    pub fn search_people(&self, q: &SearchQuery) -> Result<SearchResult> {
        self.search_people_on(q, &format!("{}-07-01", self.start_year))
    }

    /// Pesquisa com os valores de mercado do dia `today`.
    pub fn search_people_on(&self, q: &SearchQuery, today: &str) -> Result<SearchResult> {
        let y = self.start_year;
        let mut wheres: Vec<String> = Vec::new();
        let mut args: Vec<Box<dyn ToSql>> = Vec::new();

        if let Some(text) = q.text.as_deref() {
            for word in fold(text).split_whitespace() {
                wheres.push("p.search_name LIKE ?".into());
                args.push(Box::new(format!("%{word}%")));
            }
        }
        match q.kind.unwrap_or_default() {
            PersonKind::All => {}
            PersonKind::Players => wheres.push("p.is_player = 1".into()),
            PersonKind::Staff => wheres.push("p.is_non_player = 1".into()),
        }
        if let Some(nation) = q.nation_id {
            wheres.push("p.nation_id = ?".into());
            args.push(Box::new(nation));
        }
        let mask = q.positions.iter().flatten().fold(0, |m, p| m | p.bit());
        if mask != 0 {
            wheres.push(format!("p.positions & {mask} != 0"));
        }
        if let Some(side) = q.side {
            wheres.push(format!("p.positions & {} != 0", side.bit()));
        }
        // As listas e os reformados só existem no save
        let has = |table: &str| -> Result<bool> {
            Ok(self
                .conn
                .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?", [table], |r| r.get::<_, i64>(0))?
                > 0)
        };
        match q.status {
            None => {}
            Some(MarketStatus::Free) => {
                wheres.push("p.is_player = 1 AND p.club_id IS NULL".into());
                if has("retired")? {
                    wheres.push("p.id NOT IN (SELECT person_id FROM retired)".into());
                }
            }
            Some(MarketStatus::Shortlist) if has("shortlist")? => {
                wheres.push("p.id IN (SELECT person_id FROM shortlist)".into());
            }
            Some(MarketStatus::Transfer | MarketStatus::Loan) if has("transfer_list")? => {
                let status = q.status.unwrap_or(MarketStatus::Transfer);
                let column = if status == MarketStatus::Transfer { "transfer" } else { "loan" };
                // Só quem tem clube (um livre não se lista; saves antigos ainda os podem ter)
                wheres.push(format!("p.club_id IS NOT NULL AND p.id IN (SELECT person_id FROM transfer_list WHERE {column} = 1)"));
            }
            Some(_) => wheres.push("0".into()),
        }
        if q.expiring == Some(true) {
            wheres.push("p.club_id IS NOT NULL AND p.contract_expires IS NOT NULL AND p.contract_expires <= ?".into());
            args.push(Box::new(crate::date::add_days(today, 183).unwrap_or_default()));
        }
        match q.injured {
            Some(only) if !q.injured_ids.is_empty() => {
                let ids = q.injured_ids.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
                wheres.push(format!("p.id {} ({ids})", if only { "IN" } else { "NOT IN" }));
            }
            Some(true) => wheres.push("0".into()),
            _ => {}
        }
        if q.interested == Some(true)
            && let Some(sql) = &q.unwilling
        {
            wheres.push(format!("p.id NOT IN ({sql})"));
        }
        if let Some(club) = q.exclude_club {
            wheres.push("(p.club_id IS NULL OR p.club_id <> ?)".into());
            args.push(Box::new(club));
        }
        // Idade em 1º de julho do ano inicial: idade >= a  <=>  nasceu até (y-a)-07-01
        if let Some(min) = q.min_age {
            wheres.push("p.birth_sort <= ?".into());
            args.push(Box::new(format!("{}-07-01", y - min)));
        }
        if let Some(max) = q.max_age {
            wheres.push("p.birth_sort > ?".into());
            args.push(Box::new(format!("{}-07-01", y - max - 1)));
        }

        let filter_from = "FROM people p";
        let mut where_sql = if wheres.is_empty() { String::new() } else { format!("WHERE {}", wheres.join(" AND ")) };

        // Ordenação pelas colunas pré-calculadas (com índice); os joins de
        // exibição são feitos só nas linhas da página.
        let order = match q.sort.unwrap_or_default() {
            SearchSort::Ability => "p.ability DESC NULLS LAST, p.display_name",
            SearchSort::Name => "p.list_name COLLATE NOCASE",
            SearchSort::Age => "p.birth_sort DESC NULLS LAST, p.display_name",
            // A página sai dos ids já ordenados pelo valor (abaixo)
            SearchSort::Value => "p.id",
        };
        let limit = q.limit.unwrap_or(50).min(MAX_LIMIT);
        let offset = q.offset.unwrap_or(0);

        let mins: Vec<&AttributeMin> = q.attributes.iter().flatten().filter(|a| a.min > 1).collect();
        let total: u32 = if mins.is_empty() {
            self.conn
                .prepare_cached(&format!("SELECT COUNT(*) {filter_from} {where_sql}"))?
                .query_row(params_from_iter(args.iter()), |r| r.get(0))?
        } else {
            // Os atributos mostrados saem de contas com o CA: filtra em Rust e a página
            // ordena só os ids que ficaram
            // A mesma consulta no mesmo dia (outra página ou outra ordem) reaproveita os ids
            let filters = SearchQuery { sort: None, limit: None, offset: None, ..q.clone() };
            let key = format!("{today}|{where_sql}|{filters:?}");
            let cached = self.attribute_search.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, ids)| ids.clone());
            let ids = match cached {
                Some(ids) => ids,
                None => {
                    let ids = self.match_attributes(q.kind.unwrap_or_default(), &mins, q.attributes_needed, &where_sql, &args)?;
                    *self.attribute_search.borrow_mut() = Some((key, ids.clone()));
                    ids
                }
            };
            where_sql = "WHERE p.id IN (SELECT value FROM json_each(?))".into();
            args = vec![Box::new(format!("[{}]", ids.iter().map(i32::to_string).collect::<Vec<_>>().join(",")))];
            ids.len() as u32
        };

        // Pelo valor: o valor de cada um dos que passam nos filtros, do maior para o menor, e a
        // página são esses ids
        let mut by_value: Option<Vec<i32>> = None;
        if q.sort == Some(SearchSort::Value) {
            let sql = format!(
                "SELECT p.id, COALESCE(pl.current_ability, 1), COALESCE(pl.potential_ability, 0), p.birth_date, p.birth_year,
                        COALESCE(pl.current_reputation, 0), p.contract_expires, COALESCE(k.reputation, 5), p.is_player
                 {filter_from} LEFT JOIN players pl ON pl.person_id = p.id
                 LEFT JOIN clubs vc ON vc.id = p.club_id LEFT JOIN competitions k ON k.id = vc.division_id
                 {where_sql}"
            );
            type Row = (i32, i32, i32, Option<String>, Option<i32>, i32, Option<String>, i32, i32);
            let rows: Vec<Row> = self
                .conn
                .prepare_cached(&sql)?
                .query_map(params_from_iter(args.iter()), |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?))
                })?
                .collect::<rusqlite::Result<_>>()?;
            let mut valued: Vec<(i64, i32)> = rows
                .into_iter()
                .map(|(id, ca, pa, birth, by, rep, exp, league, player)| {
                    let v = if player == 1 {
                        crate::finance::value_on(ca, pa, birth.as_deref(), by, rep, exp.as_deref(), league, today)
                    } else {
                        -1
                    };
                    (v, id)
                })
                .collect();
            valued.sort_by_key(|&(v, id)| (std::cmp::Reverse(v), id));
            let page: Vec<i32> = valued.into_iter().skip(offset as usize).take(limit as usize).map(|(_, id)| id).collect();
            where_sql = "WHERE p.id IN (SELECT value FROM json_each(?))".into();
            args = vec![Box::new(format!("[{}]", page.iter().map(i32::to_string).collect::<Vec<_>>().join(",")))];
            by_value = Some(page);
        }
        let (limit, offset) = if by_value.is_some() { (limit, 0) } else { (limit, offset) };

        let sql = format!(
            "WITH page AS (
                SELECT p.id {filter_from} {where_sql}
                ORDER BY {order}
                LIMIT {limit} OFFSET {offset}
             )
             SELECT p.id, p.display_name, p.list_name,
                    CASE WHEN p.birth_sort IS NULL THEN NULL
                         ELSE {y} - CAST(substr(p.birth_sort, 1, 4) AS INTEGER)
                              - (substr(p.birth_sort, 6, 5) > '07-01') END,
                    p.nation_id, n.name,
                    p.club_id, COALESCE(NULLIF(c.short_name, ''), c.name), j.name,
                    p.is_player,
                    pl.pos_goalkeeper, pl.pos_sweeper, pl.pos_defender, pl.pos_defensive_midfielder,
                    pl.pos_midfielder, pl.pos_attacking_midfielder, pl.pos_attacker, pl.pos_wing_back,
                    pl.pos_right_side, pl.pos_left_side, pl.pos_central, pl.pos_free_role
             FROM page
             JOIN people p ON p.id = page.id
             LEFT JOIN players pl ON pl.person_id = p.id
             LEFT JOIN nations n ON n.id = p.nation_id
             LEFT JOIN clubs c ON c.id = p.club_id
             LEFT JOIN jobs j ON j.id = p.club_job
             ORDER BY {order}"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let mut items = stmt
            .query_map(params_from_iter(args.iter()), |r| {
                let mut pos = [None; 12];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = r.get(10 + i)?;
                }
                Ok(PersonSummary {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    list_name: r.get(2)?,
                    age: r.get(3)?,
                    nation_id: r.get(4)?,
                    nation: r.get(5)?,
                    club_id: r.get(6)?,
                    club: r.get(7)?,
                    job: r.get(8)?,
                    position: position_label(&pos),
                    // Preenchido abaixo
                    value: r.get::<_, Option<i32>>(9)?.filter(|&v| v == 1).map(|_| 0),
                    tags: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for p in &mut items {
            if p.value.is_some() {
                p.value = self.market_value_of(p.id, today)?;
            }
        }
        if let Some(page) = by_value {
            items.sort_by_key(|p| page.iter().position(|&id| id == p.id).unwrap_or(usize::MAX));
        }

        Ok(SearchResult { total, items })
    }

    /// Os ids de quem cumpre pelo menos `needed` dos mínimos, com os atributos como o perfil
    /// os mostra. Só jogadores ou só staff.
    fn match_attributes(
        &self,
        kind: PersonKind,
        mins: &[&AttributeMin],
        needed: Option<u32>,
        where_sql: &str,
        args: &[Box<dyn ToSql>],
    ) -> Result<Vec<i32>> {
        let staff = kind == PersonKind::Staff;
        let table = if staff { "non_players" } else { "players" };
        // Colunas de atributos que existem (as da tabela, sem as de identificação, e as mentais de `people`)
        let skip = [
            "person_id",
            "squad_number",
            "current_ability",
            "potential_ability",
            "home_reputation",
            "current_reputation",
            "world_reputation",
            "morale",
            "preferred_formation",
        ];
        let mut columns: Vec<String> = self
            .conn
            .prepare_cached(&format!("SELECT name FROM pragma_table_info('{table}')"))?
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|c| !skip.contains(&c.as_str()) && !c.starts_with("pos_"))
            .map(|c| format!("x.{c}"))
            .collect();
        columns.extend(MENTAL.map(|m| format!("p.{m}")));
        let column = |key: &str| columns.iter().find(|c| c[2..] == *key).cloned();
        let mins: Vec<(String, &str, i32)> = mins.iter().filter_map(|m| Some((column(&m.key)?, m.key.as_str(), m.min))).collect();
        if mins.is_empty() {
            return Ok(Vec::new());
        }
        let needed = needed.map_or(mins.len(), |n| (n as usize).clamp(1, mins.len()));

        // As contas do perfil só mexem nos relativos (e, nos jogadores, cortam um pouco o físico):
        // os outros mostram-se como estão. Quando todos os mínimos contam, os que não sobem
        // nas contas já cortam em SQL
        let relative: &[&str] = if staff { &crate::attributes::STAFF_RELATIVE } else { &crate::attributes::RELATIVE };
        let mut wheres = vec![where_sql.trim_start_matches("WHERE ").to_string()];
        let mut computed = false;
        for (col, key, min) in &mins {
            if relative.contains(key) {
                computed = true;
            } else if needed == mins.len() {
                wheres.push(format!("{col} >= {min}"));
            }
        }
        if !staff && mins.iter().any(|(_, k, _)| PHYSICAL_CUT.contains(k)) {
            computed = true;
        }
        if !computed && needed < mins.len() {
            let sum = mins.iter().map(|(col, _, min)| format!("({col} >= {min})")).collect::<Vec<_>>().join(" + ");
            wheres.push(format!("{sum} >= {needed}"));
        }
        let wheres: Vec<String> = wheres.into_iter().filter(|w| !w.is_empty()).collect();
        let where_sql = if wheres.is_empty() { String::new() } else { format!("WHERE {}", wheres.join(" AND ")) };
        // Só as colunas que as contas usam e as pedidas
        let used: Vec<&String> = columns
            .iter()
            .filter(|c| {
                let key = &c[2..];
                relative.contains(&key) || (!staff && PHYSICAL_INPUTS.contains(&key)) || mins.iter().any(|(_, k, _)| *k == key)
            })
            .collect();
        let select = if computed {
            let used = used.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", ");
            format!("x.current_ability, {}{used}", if staff { "" } else { "x.pos_goalkeeper, " })
        } else {
            "1".into()
        };
        let mut stmt =
            self.conn.prepare(&format!("SELECT p.id, {select} FROM people p JOIN {table} x ON x.person_id = p.id {where_sql}"))?;
        let names: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
        let first = if staff { 2 } else { 3 };
        let mut rows = stmt.query(params_from_iter(args.iter()))?;
        let mut out = Vec::new();
        // Um só mapa para todas as linhas: as chaves ficam, os valores trocam
        let mut shown: crate::person::Attributes = names.iter().skip(first).map(|n| (n.clone(), None)).collect();
        while let Some(r) = rows.next()? {
            if !computed {
                out.push(r.get(0)?);
                continue;
            }
            for (i, name) in names.iter().enumerate().skip(first) {
                if let Some(slot) = shown.get_mut(name) {
                    *slot = r.get(i)?;
                }
            }
            let ca: Option<i32> = r.get(1)?;
            if staff {
                crate::attributes::show_staff_attributes(&mut shown, ca);
            } else {
                crate::attributes::show_attributes(&mut shown, ca, r.get(2)?);
            }
            let hits = mins.iter().filter(|(_, k, min)| shown.get(*k).copied().flatten().is_some_and(|v| v >= *min)).count();
            if hits >= needed {
                out.push(r.get(0)?);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> Option<World> {
        let path = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))?;
        Some(World::open(&path).expect("abrir world.sqlite"))
    }

    #[test]
    fn find_by_name() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        // As competições também pelo nome do regulamento ("Mundial" é o FIFA World Cup)
        assert!(w.find_places("mundial").unwrap().competitions.iter().any(|c| c.id == 448));
        let bayern = w.find_places("Bayern").unwrap();
        assert!(bayern.clubs.iter().any(|c| c.club.id == 1350));
        assert!(w.find_places("Brazil").unwrap().nations.iter().any(|n| n.name == "Brazil"));
        // Uma letra só, nada
        let one = w.find_places("b").unwrap();
        assert!(one.clubs.is_empty() && one.nations.is_empty() && one.competitions.is_empty());
    }

    #[test]
    fn fold_removes_accents() {
        assert_eq!(fold("Paquetá Júnior"), "paqueta junior");
    }

    #[test]
    fn position_labels() {
        let mut p = [Some(1); 12];
        p[5] = Some(15);
        p[6] = Some(20);
        p[8] = Some(15);
        p[10] = Some(20);
        assert_eq!(position_label(&p), "MA/A DC");
        let tags: Vec<(String, bool)> = position_tags(&p).into_iter().map(|t| (t.code, t.natural)).collect();
        assert_eq!(tags, [("MAD".into(), false), ("MAC".into(), false), ("AD".into(), false), ("AC".into(), true)]);
        p[0] = Some(20);
        assert_eq!(position_label(&p), "GR");
    }

    #[test]
    fn search_by_accentless_name() {
        let Some(w) = world() else { return }; // sem banco gerado, pula
        let r = w.search_people(&SearchQuery { text: Some("paqueta lucas".into()), ..Default::default() }).unwrap();
        assert_eq!(r.total, 1);
        let lucas = &r.items[0];
        assert_eq!(lucas.name, "Lucas Paquetá");
        assert_eq!(lucas.list_name, "Lucas Paquetá"); // nome comum aparece como está

        let r = w.search_people(&SearchQuery { text: Some("arrascaeta".into()), ..Default::default() }).unwrap();
        assert_eq!(r.items[0].list_name, "de Arrascaeta, Giorgian");
        assert_eq!(lucas.club.as_deref(), Some("Flamengo"));
        assert_eq!(lucas.age, Some(27)); // nascido em 27/08/1997, idade em 01/07/2025
    }

    #[test]
    fn sort_by_value() {
        let Some(w) = world() else { return };
        let q = |offset| SearchQuery {
            kind: Some(PersonKind::Players),
            positions: Some(vec![Position::GK]),
            sort: Some(SearchSort::Value),
            limit: Some(20),
            offset: Some(offset),
            ..Default::default()
        };
        let first = w.search_people(&q(0)).unwrap();
        let second = w.search_people(&q(20)).unwrap();
        let values: Vec<i64> = first.items.iter().chain(&second.items).map(|p| p.value.unwrap_or(0)).collect();
        assert_eq!(values.len(), 40);
        assert!(values.windows(2).all(|v| v[0] >= v[1]), "{values:?}");
        // Sem filtros (todos os jogadores) continua rápido
        let t = std::time::Instant::now();
        let all = w.search_people(&SearchQuery { sort: Some(SearchSort::Value), limit: Some(20), ..Default::default() }).unwrap();
        println!("todos por valor: {:?} ({} pessoas)", t.elapsed(), all.total);
        assert!(all.items.windows(2).all(|v| v[0].value.unwrap_or(-1) >= v[1].value.unwrap_or(-1)));
    }

    #[test]
    fn search_filters() {
        let Some(w) = world() else { return };
        let brazil = w.list_nations().unwrap().into_iter().find(|n| n.name == "Brazil").unwrap();
        let r = w
            .search_people(&SearchQuery {
                kind: Some(PersonKind::Players),
                nation_id: Some(brazil.id),
                positions: Some(vec![Position::GK]),
                max_age: Some(23),
                limit: Some(10),
                ..Default::default()
            })
            .unwrap();
        assert!(r.total > 0);
        for p in &r.items {
            assert_eq!(p.position, "GR");
            assert!(p.age.unwrap() <= 23);
            assert_eq!(p.nation.as_deref(), Some("Brazil"));
        }
        // Os jogadores trazem o valor
        assert!(r.items.iter().all(|p| p.value.is_some()));

        // Aba Atributos: Velocidade 18+ e Finalização 17+, os dois
        let mins = vec![AttributeMin { key: "pace".into(), min: 18 }, AttributeMin { key: "finishing".into(), min: 17 }];
        let both = SearchQuery { kind: Some(PersonKind::Players), attributes: Some(mins), limit: Some(20), ..Default::default() };
        let r = w.search_people(&both).unwrap();
        assert!(r.total > 0 && r.total < 2000, "{}", r.total);
        for p in &r.items {
            let shown = w.get_person(p.id).unwrap().player.unwrap().attributes;
            assert!(shown["pace"].unwrap() >= 18 && shown["finishing"].unwrap() >= 17, "{}", p.name);
        }
        // Igualar 1/2: basta um dos dois
        let any = w.search_people(&SearchQuery { attributes_needed: Some(1), ..both }).unwrap();
        assert!(any.total > r.total);
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    #[test]
    #[ignore]
    fn timing() {
        let path = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let w = World::open(&path).unwrap();
        for (label, q) in [
            ("vazia", SearchQuery::default()),
            ("texto", SearchQuery { text: Some("silva".into()), ..Default::default() }),
            ("jogadores", SearchQuery { kind: Some(PersonKind::Players), ..Default::default() }),
            ("idade", SearchQuery { sort: Some(SearchSort::Age), ..Default::default() }),
            ("nome", SearchQuery { sort: Some(SearchSort::Name), ..Default::default() }),
            ("goleiros sub-21", SearchQuery { positions: Some(vec![Position::GK]), max_age: Some(21), ..Default::default() }),
            (
                "meias esquerda",
                SearchQuery {
                    positions: Some(vec![Position::DM, Position::M, Position::AM]),
                    side: Some(Side::Left),
                    ..Default::default()
                },
            ),
            ("página 500", SearchQuery { offset: Some(50_000), ..Default::default() }),
            (
                "atributos",
                SearchQuery {
                    kind: Some(PersonKind::Players),
                    attributes: Some(vec![AttributeMin { key: "pace".into(), min: 15 }]),
                    ..Default::default()
                },
            ),
            (
                "atributos relativos",
                SearchQuery {
                    kind: Some(PersonKind::Players),
                    attributes: Some(vec![AttributeMin { key: "finishing".into(), min: 15 }]),
                    ..Default::default()
                },
            ),
        ] {
            let t = std::time::Instant::now();
            let r = w.search_people(&q).unwrap();
            println!("{label}: {} em {:?}", r.total, t.elapsed());
        }
    }
}
