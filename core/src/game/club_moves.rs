//! A aba Transferências do clube, como no CM (prints de 9/11; plano no 3.22): « Temporada /
//! Temporada » e o menu Ver com Entradas e Saídas de jogadores, Transferências Futuras, os
//! empréstimos ao clube e pelo clube e as Entradas e Saídas de staff. Os movimentos de jogadores
//! vêm de `transfers`; os do staff (treinadores contratados e despedidos) ficam em `staff_moves`
//! (save 54); as futuras são as propostas em curso (`offers`).

use rusqlite::params;
use serde::Serialize;
use ts_rs::TS;

use super::Game;
use super::market::money_text;
use crate::person::ClubRef;
use crate::regulation::SeasonKind;
use crate::{Error, Result};

/// Os movimentos do staff (save 54).
pub(super) const STAFF_MOVES_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS staff_moves (id INTEGER PRIMARY KEY, date TEXT NOT NULL, person_id INTEGER NOT NULL,
                                        from_club INTEGER, to_club INTEGER);
CREATE INDEX IF NOT EXISTS staff_moves_date ON staff_moves (date);
";

/// As vistas do menu Ver, pela ordem do CM.
const MOVE_VIEWS: [(&str, &str); 7] = [
    ("in", "Entradas Jogadores"),
    ("out", "Saídas Jogadores"),
    ("future", "Transferências Futuras"),
    ("loan_in", "Emprest. Ao Clube"),
    ("loan_out", "Emprest. Pelo Clube"),
    ("staff_in", "Entradas Staff"),
    ("staff_out", "Saídas Staff"),
];

/// Uma vista da aba Transferências.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClubMoves {
    pub view: String,
    /// "Entradas Jogadores - Temporada 2025/26"; nas futuras, só o nome da vista.
    pub title: String,
    pub season: i32,
    /// As épocas para as setas « Temporada / Temporada » (nas futuras, nenhuma).
    pub prev: Option<i32>,
    pub next: Option<i32>,
    pub rows: Vec<MoveRow>,
    /// O saldo dos valores (o que entra menos o que sai do cofre), se houver valores.
    #[ts(type = "number | null")]
    pub total: Option<i64>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MoveRow {
    /// A data ("2026-07-14") ou "Oferecer" numa proposta que ainda corre.
    pub when: String,
    pub person_id: i32,
    pub name: String,
    /// O outro clube (de onde vem ou para onde vai).
    pub club: Option<ClubRef>,
    /// "€17.75M", "Livre", "Bos", "Empréstimo", "Dispensado", "Fim de empréstimo".
    pub value: String,
}

impl Game {
    /// Regista a mudança de clube de alguém do staff (treinadores).
    pub(super) fn log_staff_move(&self, person: i32, from: Option<i32>, to: Option<i32>) -> Result<()> {
        if from.is_none() && to.is_none() {
            return Ok(());
        }
        self.world.conn.execute(
            "INSERT INTO staff_moves (date, person_id, from_club, to_club) VALUES (?, ?, ?, ?)",
            params![self.date, person, from, to],
        )?;
        Ok(())
    }

    /// A época de um clube em que cai um dia, e os dias em que começa e acaba.
    fn move_season(kind: SeasonKind, day: &str) -> i32 {
        let year: i32 = day[0..4].parse().unwrap_or(0);
        match kind {
            SeasonKind::Calendar => year,
            _ if &day[5..] >= "07-01" => year,
            _ => year - 1,
        }
    }

    fn move_season_range(kind: SeasonKind, season: i32) -> (String, String, String) {
        match kind {
            SeasonKind::Calendar => (format!("{season}-01-01"), format!("{}-01-01", season + 1), season.to_string()),
            _ => (format!("{season}-07-01"), format!("{}-07-01", season + 1), format!("{season}/{:02}", (season + 1) % 100)),
        }
    }

    /// A vista `view` da aba Transferências do clube, na época `season` (a de hoje sem ela).
    pub fn club_moves(&self, club: i32, view: &str, season: Option<i32>) -> Result<ClubMoves> {
        let label = MOVE_VIEWS.iter().find(|v| v.0 == view).map(|v| v.1).ok_or_else(|| Error::Invalid("Vista desconhecida.".into()))?;
        if view == "future" {
            let (rows, total) = self.future_moves(club)?;
            return Ok(ClubMoves { view: view.into(), title: label.into(), season: 0, prev: None, next: None, rows, total });
        }
        let kind = self.kind_of(club)?;
        let now = Self::move_season(kind, &self.date);
        let created: String = self.world.conn.query_row("SELECT created FROM game", [], |r| r.get(0))?;
        let first = Self::move_season(kind, &created).min(now);
        let season = season.unwrap_or(now).clamp(first, now);
        let (start, end, season_label) = Self::move_season_range(kind, season);
        let (rows, total) = if view.starts_with("staff") {
            (self.staff_moves(club, view == "staff_in", &start, &end)?, None)
        } else {
            self.player_moves(club, view, &start, &end)?
        };
        Ok(ClubMoves {
            view: view.into(),
            title: format!("{label} - Temporada {season_label}"),
            season,
            prev: (season > first).then_some(season - 1),
            next: (season < now).then_some(season + 1),
            rows,
            total,
        })
    }

    fn player_moves(&self, club: i32, view: &str, start: &str, end: &str) -> Result<(Vec<MoveRow>, Option<i64>)> {
        let (side, kinds) = match view {
            "in" => ("to_club", "'transfer', 'free'"),
            "out" => ("from_club", "'transfer', 'free', 'release'"),
            "loan_in" => ("to_club", "'loan'"),
            _ => ("from_club", "'loan'"),
        };
        let other = if side == "to_club" { "from_club" } else { "to_club" };
        let rows: Vec<(String, i32, Option<i32>, i64, String)> = self
            .world
            .conn
            .prepare(&format!(
                "SELECT date, person_id, {other}, fee, kind FROM transfers
                 WHERE {side} = ?1 AND date >= ?2 AND date < ?3 AND kind IN ({kinds}) ORDER BY date, id"
            ))?
            .query_map(params![club, start, end], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::with_capacity(rows.len());
        let mut total = 0i64;
        for (date, person, other, fee, kind) in rows {
            // O cofre: o que se paga sai, o que se recebe entra
            total += if view == "in" { -fee } else { fee };
            out.push(MoveRow {
                when: date,
                person_id: person,
                name: self.person_name(person)?,
                club: other.map(|c| self.club(c)),
                value: match kind.as_str() {
                    "loan" => "Empréstimo".into(),
                    "release" => "Dispensado".into(),
                    _ if fee > 0 => money_text(fee),
                    _ => "Livre".into(),
                },
            });
        }
        let money = matches!(view, "in" | "out") && total != 0;
        Ok((out, money.then_some(total)))
    }

    fn staff_moves(&self, club: i32, incoming: bool, start: &str, end: &str) -> Result<Vec<MoveRow>> {
        let (side, other) = if incoming { ("to_club", "from_club") } else { ("from_club", "to_club") };
        let rows: Vec<(String, i32, Option<i32>)> = self
            .world
            .conn
            .prepare(&format!(
                "SELECT date, person_id, {other} FROM staff_moves WHERE {side} = ?1 AND date >= ?2 AND date < ?3 ORDER BY date, id"
            ))?
            .query_map(params![club, start, end], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        rows.into_iter()
            .map(|(date, person, other)| {
                Ok(MoveRow {
                    when: date,
                    person_id: person,
                    name: self.person_name(person)?,
                    club: other.map(|c| self.club(c)),
                    value: if incoming { "Contratado".into() } else { "Saiu".into() },
                })
            })
            .collect()
    }

    /// As Transferências Futuras: as propostas em curso por jogadores do clube ou do clube por
    /// outros ("Oferecer"), os negócios fechados à espera da janela (com a data) e os
    /// pré-contratos ("Bos", livres a custo zero, com a data em que chegam).
    fn future_moves(&self, club: i32) -> Result<(Vec<MoveRow>, Option<i64>)> {
        type Row = (i32, String, i32, i32, Option<i32>, i64, String, Option<String>);
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare(
                "SELECT o.id, o.kind, o.person_id, o.buyer, o.seller, o.fee, o.status,
                        (SELECT c.answer_on FROM offers c WHERE c.kind = 'contract' AND c.person_id = o.person_id
                           AND c.buyer = o.buyer AND c.status IN ('waiting', 'signed') ORDER BY c.id DESC LIMIT 1)
                 FROM offers o
                 WHERE (o.buyer = ?1 OR o.seller = ?1)
                   AND ((o.kind IN ('transfer', 'loan') AND o.status IN ('pending', 'countered', 'raised', 'agreed', 'waiting'))
                        OR (o.kind = 'contract' AND o.status = 'signed'))
                 ORDER BY o.id",
            )?
            .query_map([club], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out: Vec<MoveRow> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut total = 0i64;
        for (_, kind, person, buyer, seller, fee, status, dated) in rows {
            // Um jogador uma vez: o negócio mais recente
            if !seen.insert(person) {
                continue;
            }
            let incoming = buyer == club;
            let pre = kind == "contract";
            let when = if pre { dated.clone() } else { dated.clone().filter(|_| status != "pending") }.unwrap_or_else(|| "Oferecer".into());
            let other = if incoming { seller } else { Some(buyer) };
            let value = if pre || (kind == "transfer" && fee == 0) {
                "Bos".into()
            } else if kind == "loan" {
                "Empréstimo".into()
            } else {
                total += if incoming { -fee } else { fee };
                money_text(fee)
            };
            out.push(MoveRow { when, person_id: person, name: self.person_name(person)?, club: other.map(|c| self.club(c)), value });
        }
        Ok((out, (total != 0).then_some(total)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_by_view_and_season() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-movimentos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Movimentos".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 6271,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let g = Game::create(&path, &dir, &new, 31).unwrap();
        let today = g.date.clone();
        let log = |person: i32, from: Option<i32>, to: Option<i32>, fee: i64, kind: &str| {
            g.world
                .conn
                .execute(
                    "INSERT INTO transfers (date, person_id, from_club, to_club, fee, kind) VALUES (?, ?, ?, ?, ?, ?)",
                    params![today, person, from, to, fee, kind],
                )
                .unwrap();
        };
        log(1, Some(1350), Some(6271), 20_000_000, "transfer");
        log(2, None, Some(6271), 0, "free");
        log(3, Some(6271), Some(8383), 30_000_000, "transfer");
        log(4, Some(6271), Some(1283), 0, "loan");
        g.log_staff_move(5, Some(1350), Some(6271)).unwrap();
        let v = g.club_moves(6271, "in", None).unwrap();
        assert!(v.title.starts_with("Entradas Jogadores - Temporada"), "{}", v.title);
        assert_eq!(v.rows.len(), 2);
        assert_eq!(v.rows[1].value, "Livre");
        assert_eq!(v.total, Some(-20_000_000));
        assert!(v.prev.is_none() && v.next.is_none(), "só a época de hoje");
        assert_eq!(g.club_moves(6271, "out", None).unwrap().total, Some(30_000_000));
        let loans = g.club_moves(6271, "loan_out", None).unwrap();
        assert_eq!((loans.rows.len(), loans.rows[0].value.as_str()), (1, "Empréstimo"));
        assert_eq!(g.club_moves(6271, "staff_in", None).unwrap().rows.len(), 1);
        assert!(g.club_moves(6271, "future", None).unwrap().rows.is_empty());
        assert!(g.club_moves(6271, "x", None).is_err());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
