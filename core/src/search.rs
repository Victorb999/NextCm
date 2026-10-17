use rusqlite::{ToSql, params_from_iter};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use unicode_normalization::UnicodeNormalization;

use crate::{Result, World};

/// Valor mínimo (1–20) para considerar uma posição "natural".
const NATURAL_POSITION: i32 = 15;
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
    pub sort: Option<SearchSort>,
    #[ts(optional)]
    pub limit: Option<u32>,
    #[ts(optional)]
    pub offset: Option<u32>,
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
    pub current_ability: Option<i32>,
    pub potential_ability: Option<i32>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub total: u32,
    pub items: Vec<PersonSummary>,
}

/// Minúsculas sem acento, igual ao `fold()` do extract.py.
fn fold(s: &str) -> String {
    s.nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

fn position_label(p: &[Option<i32>; 12]) -> String {
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
    let sides: String = [(8, 'D'), (9, 'E'), (10, 'C')]
        .into_iter()
        .filter(|(i, _)| natural(*i))
        .map(|(_, s)| s)
        .collect();
    match (roles.is_empty(), sides.is_empty()) {
        (true, _) => String::new(),
        (false, true) => roles.join("/"),
        (false, false) => format!("{} {}", roles.join("/"), sides),
    }
}

impl World {
    pub fn search_people(&self, q: &SearchQuery) -> Result<SearchResult> {
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
        let where_sql = if wheres.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", wheres.join(" AND "))
        };

        let total: u32 = self.conn.prepare_cached(&format!("SELECT COUNT(*) {filter_from} {where_sql}"))?.query_row(
            params_from_iter(args.iter()),
            |r| r.get(0),
        )?;

        // Ordenação pelas colunas pré-calculadas (com índice); os joins de
        // exibição são feitos só nas linhas da página.
        let order = match q.sort.unwrap_or_default() {
            SearchSort::Ability => "p.ability DESC NULLS LAST, p.display_name",
            SearchSort::Name => "p.list_name COLLATE NOCASE",
            SearchSort::Age => "p.birth_sort DESC NULLS LAST, p.display_name",
        };
        let limit = q.limit.unwrap_or(50).min(MAX_LIMIT);
        let offset = q.offset.unwrap_or(0);

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
                    COALESCE(pl.current_ability, np.current_ability),
                    COALESCE(pl.potential_ability, np.potential_ability),
                    pl.pos_goalkeeper, pl.pos_sweeper, pl.pos_defender, pl.pos_defensive_midfielder,
                    pl.pos_midfielder, pl.pos_attacking_midfielder, pl.pos_attacker, pl.pos_wing_back,
                    pl.pos_right_side, pl.pos_left_side, pl.pos_central, pl.pos_free_role
             FROM page
             JOIN people p ON p.id = page.id
             LEFT JOIN players pl ON pl.person_id = p.id
             LEFT JOIN non_players np ON np.person_id = p.id
             LEFT JOIN nations n ON n.id = p.nation_id
             LEFT JOIN clubs c ON c.id = p.club_id
             LEFT JOIN jobs j ON j.id = p.club_job
             ORDER BY {order}"
        );
        let mut stmt = self.conn.prepare_cached(&sql)?;
        let items = stmt
            .query_map(params_from_iter(args.iter()), |r| {
                let mut pos = [None; 12];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = r.get(11 + i)?;
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
                    current_ability: r.get(9)?,
                    potential_ability: r.get(10)?,
                    position: position_label(&pos),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        Ok(SearchResult { total, items })
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
        p[0] = Some(20);
        assert_eq!(position_label(&p), "GR");
    }

    #[test]
    fn search_by_accentless_name() {
        let Some(w) = world() else { return }; // sem banco gerado, pula
        let r = w
            .search_people(&SearchQuery { text: Some("paqueta lucas".into()), ..Default::default() })
            .unwrap();
        assert_eq!(r.total, 1);
        let lucas = &r.items[0];
        assert_eq!(lucas.name, "Lucas Paquetá");
        assert_eq!(lucas.list_name, "Lucas Paquetá"); // nome comum aparece como está

        let r = w
            .search_people(&SearchQuery { text: Some("arrascaeta".into()), ..Default::default() })
            .unwrap();
        assert_eq!(r.items[0].list_name, "de Arrascaeta, Giorgian");
        assert_eq!(lucas.club.as_deref(), Some("Flamengo"));
        assert_eq!(lucas.age, Some(27)); // nascido em 27/08/1997, idade em 01/07/2025
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
        // ordenado por habilidade
        let cas: Vec<_> = r.items.iter().map(|p| p.current_ability.unwrap_or(0)).collect();
        assert!(cas.windows(2).all(|w| w[0] >= w[1]));
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
            ("meias esquerda", SearchQuery { positions: Some(vec![Position::DM, Position::M, Position::AM]), side: Some(Side::Left), ..Default::default() }),
            ("página 500", SearchQuery { offset: Some(50_000), ..Default::default() }),
        ] {
            let t = std::time::Instant::now();
            let r = w.search_people(&q).unwrap();
            println!("{label}: {} em {:?}", r.total, t.elapsed());
        }
    }
}
