//! Fair-play financeiro (marco 3.22): o resultado das últimas 3 épocas (receitas menos
//! despesas do livro de contas, sem os dividendos) não pode ficar abaixo de −15% das receitas
//! de uma época. No fim de cada época, quem passou do limite leva um aviso; se continuar no fim
//! da seguinte, fica proibido de contratar (só livres e empréstimos) até voltar ao limite. Vale
//! para todos os clubes geridos, os da IA também. O CM não tinha isto.

use rusqlite::{OptionalExtension, params};

use super::{Game, NewsItem};
use crate::Result;
use crate::finance;
use crate::regulation::{self, SeasonKind};

/// Perdas permitidas em 3 épocas, sobre as receitas de uma época.
pub(super) const FFP_LOSS: f64 = 0.15;
/// Épocas que contam.
const FFP_SEASONS: i32 = 3;
/// Anos pelos quais uma compra se amortiza (o máximo da UEFA desde 2023).
const AMORTIZATION_YEARS: f64 = 5.0;

/// Avisos e proibições (save 56).
pub(super) const FAIR_PLAY_SCHEMA: &str = "
CREATE TABLE fair_play (
    club_id INTEGER PRIMARY KEY,
    strikes INTEGER NOT NULL DEFAULT 0,
    banned INTEGER NOT NULL DEFAULT 0
);
";

/// Os clubes que a CLI acompanha na calibração da economia.
const WATCHED: [(i32, &str); 8] = [
    (6269, "Man City"),
    (6271, "Man Utd"),
    (8383, "Real Madrid"),
    (1350, "Bayern"),
    (8117, "PSG"),
    (4847, "Inter"),
    (3675, "Flamengo"),
    (7568, "Palmeiras"),
];

impl Game {
    /// A economia numa época, para a CLI `sim` (calibração do 3.22): as transferências acima de
    /// €100M, quem mais gastou, os grandes clubes (saldo, folha sobre receitas, fair-play) e os
    /// clubes geridos (folha sobre receitas, dívidas, avisos e proibições).
    pub fn economy_report(&self, from: &str, to: &str) -> Result<Vec<String>> {
        let m = |v: f64| format!("€{:.0}M", v / 1e6);
        let conn = &self.world.conn;
        let mut out = Vec::new();
        let (big, top): (i64, i64) = conn.query_row(
            "SELECT COALESCE(SUM(fee >= 100000000), 0), COALESCE(MAX(fee), 0) FROM transfers WHERE date >= ? AND date < ?",
            params![from, to],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        out.push(format!("transferências de €100M ou mais: {big} (a maior {})", m(top as f64)));
        let spenders: Vec<(i32, i64, i64)> = conn
            .prepare(
                "SELECT c, SUM(bought), SUM(sold) FROM (
                    SELECT to_club AS c, fee AS bought, 0 AS sold FROM transfers WHERE date >= ?1 AND date < ?2 AND to_club IS NOT NULL
                    UNION ALL
                    SELECT from_club, 0, fee FROM transfers WHERE date >= ?1 AND date < ?2 AND from_club IS NOT NULL)
                 GROUP BY c ORDER BY SUM(bought) DESC, c LIMIT 5",
            )?
            .query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let spent: Vec<String> =
            spenders.iter().map(|&(c, b, s)| format!("{} {} (vendeu {})", self.club(c).short_name, m(b as f64), m(s as f64))).collect();
        out.push(format!("quem mais comprou: {}", spent.join(", ")));
        let bill = |c: i32| -> Result<f64> {
            Ok(conn
                .query_row("SELECT COALESCE(SUM(wage), 0) FROM people WHERE is_player = 1 AND club_id = ?", [c], |r| r.get::<_, i64>(0))?
                as f64
                * 52.0)
        };
        let mut big_clubs = Vec::new();
        for (c, name) in WATCHED {
            let base = self.club_base(c)?;
            big_clubs.push(format!(
                "{name} {} folha {:.0}% FFP {}{}",
                m(self.cash_of(c)? as f64),
                100.0 * bill(c)? / base,
                m(self.fair_play_result(c)? as f64),
                if self.fair_play_banned(c)? { " PROIBIDO" } else { "" }
            ));
        }
        out.push(format!("saldos: {}", big_clubs.join(" | ")));
        let mut ratios = Vec::new();
        let mut in_debt = 0;
        for c in self.managed_clubs()? {
            ratios.push(bill(c)? / self.club_base(c)?);
            if self.cash_of(c)? < 0 {
                in_debt += 1;
            }
        }
        ratios.sort_by(f64::total_cmp);
        let at = |q: f64| ratios.get(((ratios.len() as f64 - 1.0) * q) as usize).copied().unwrap_or(0.0) * 100.0;
        let (strikes, banned): (i64, i64) =
            conn.query_row("SELECT COALESCE(SUM(strikes > 0 AND banned = 0), 0), COALESCE(SUM(banned), 0) FROM fair_play", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        out.push(format!(
            "clubes geridos: folha/receitas {:.0}% / {:.0}% / {:.0}% (quartis), {in_debt} no vermelho, fair-play {strikes} avisados e {banned} proibidos",
            at(0.25),
            at(0.5),
            at(0.75)
        ));
        let banned_clubs: Vec<i32> = conn
            .prepare("SELECT club_id FROM fair_play WHERE banned = 1 ORDER BY club_id LIMIT 8")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        if !banned_clubs.is_empty() {
            let mut names = Vec::new();
            for c in banned_clubs {
                names.push(format!(
                    "{} (receitas {}, folha {:.0}%, saldo {})",
                    self.club(c).short_name,
                    m(self.club_base(c)?),
                    100.0 * bill(c)? / self.club_base(c)?,
                    m(self.cash_of(c)? as f64)
                ));
            }
            out.push(format!("proibidos: {}", names.join(", ")));
        }
        Ok(out)
    }

    /// O resultado do clube nas últimas 3 épocas, a contar com a corrente (sem dividendos). As
    /// compras não entram pelo que se pagou mas amortizadas, como na UEFA e na Premier.
    pub(super) fn fair_play_result(&self, club: i32) -> Result<i64> {
        let division = self.division_of(club)?.unwrap_or(0);
        let season = regulation::season_of(division, &self.date);
        let rows: Vec<(String, i64)> = self
            .world
            .conn
            .prepare_cached("SELECT category, amount FROM finances WHERE club_id = ? AND season > ? AND season <= ?")?
            .query_map(params![club, season - FFP_SEASONS, season], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let ledger: i64 = rows
            .iter()
            .map(|(k, v)| {
                if k == "dividends" || k == "purchases" {
                    0
                } else if finance::EXPENSES.iter().any(|(e, _)| e == k) {
                    -v
                } else {
                    *v
                }
            })
            .sum();
        let first = season - FFP_SEASONS + 1;
        let from = match regulation::season_kind(division) {
            SeasonKind::Calendar => format!("{first}-01-01"),
            _ => format!("{first}-07-01"),
        };
        Ok(ledger - self.amortization(club, &from)?)
    }

    /// As compras do clube que pesam desde `from` até hoje: cada transferência dividida por
    /// `AMORTIZATION_YEARS`, dia a dia, e o que faltava amortizar quando o jogador sai antes; os
    /// empréstimos contam inteiros no dia.
    fn amortization(&self, club: i32, from: &str) -> Result<i64> {
        let (Some(start), Some(today)) = (crate::date::parse(from), crate::date::parse(&self.date)) else { return Ok(0) };
        let span = AMORTIZATION_YEARS * 365.0;
        let bought: Vec<(String, i32, i64, String, Option<String>)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT t.date, t.person_id, t.fee, t.kind,
                        (SELECT MIN(s.date) FROM transfers s WHERE s.person_id = t.person_id AND s.from_club = t.to_club AND s.id > t.id)
                 FROM transfers t WHERE t.to_club = ? AND t.fee > 0 AND t.date >= ?",
            )?
            .query_map(params![club, crate::date::format(start - span as i32)], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut total = 0.0;
        for (date, _, fee, kind, sold) in bought {
            let Some(day) = crate::date::parse(&date) else { continue };
            let fee = fee as f64;
            if kind.starts_with("loan") {
                if day >= start {
                    total += fee;
                }
                continue;
            }
            let end = day + span as i32;
            let left = sold.as_deref().and_then(crate::date::parse).filter(|&s| s <= today && s < end);
            let stop = left.unwrap_or(end).min(today);
            let days = (stop - day.max(start)).max(0) as f64;
            total += fee * days / span;
            if let Some(s) = left.filter(|&s| s >= start) {
                total += fee * (1.0 - (s - day) as f64 / span);
            }
        }
        Ok(total.round() as i64)
    }

    /// O limite do clube (negativo): as perdas permitidas.
    pub(super) fn fair_play_limit(&self, club: i32) -> Result<i64> {
        Ok(-(FFP_LOSS * self.club_base(club)?) as i64)
    }

    /// Quanto o clube ainda pode perder sem passar do limite (0 se já passou).
    pub(super) fn fair_play_room(&self, club: i32) -> Result<f64> {
        Ok((self.fair_play_result(club)? - self.fair_play_limit(club)?).max(0) as f64)
    }

    /// Quanto o clube pode gastar em compras sem passar do limite: cada compra pesa no máximo
    /// `FFP_SEASONS / AMORTIZATION_YEARS` dela nas 3 épocas que contam.
    pub(super) fn fair_play_spend(&self, club: i32) -> Result<f64> {
        Ok(self.fair_play_room(club)? * AMORTIZATION_YEARS / FFP_SEASONS as f64)
    }

    /// O clube está proibido de contratar pelo fair-play?
    pub(super) fn fair_play_banned(&self, club: i32) -> Result<bool> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT banned FROM fair_play WHERE club_id = ?")?
            .query_row([club], |r| r.get::<_, i32>(0))
            .optional()?
            .is_some_and(|b| b != 0))
    }

    /// Fim de época: o exame do fair-play dos clubes geridos deste tipo de época. Ao treinador,
    /// a direção avisa e, depois, anuncia a proibição (ou o fim dela).
    pub(super) fn fair_play_review(&mut self, kind: SeasonKind, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        for club in self.managed_clubs()? {
            if self.kind_of(club)? != kind {
                continue;
            }
            let over = self.fair_play_result(club)? < self.fair_play_limit(club)?;
            let (strikes, banned): (i32, bool) = self
                .world
                .conn
                .prepare_cached("SELECT strikes, banned FROM fair_play WHERE club_id = ?")?
                .query_row([club], |r| Ok((r.get(0)?, r.get::<_, i32>(1)? != 0)))
                .optional()?
                .unwrap_or((0, false));
            let (strikes, now_banned) = if over { (strikes + 1, strikes >= 1) } else { (0, false) };
            self.world.conn.execute(
                "INSERT INTO fair_play (club_id, strikes, banned) VALUES (?1, ?2, ?3)
                 ON CONFLICT (club_id) DO UPDATE SET strikes = ?2, banned = ?3",
                params![club, strikes, now_banned as i32],
            )?;
            if club != self.club_id || self.sacked() {
                continue;
            }
            let loss = super::market::money_text(-self.fair_play_result(club)?);
            let limit = super::market::money_text(-self.fair_play_limit(club)?);
            let news = if now_banned && !banned {
                Some((
                    "Proibição de contratar",
                    format!(
                        "O clube perdeu {loss} nas últimas três épocas, acima do limite de {limit} do fair-play financeiro, pela segunda época seguida.\n\nAté as contas voltarem ao limite, só pode contratar jogadores livres ou por empréstimo."
                    ),
                ))
            } else if over && !now_banned {
                Some((
                    "Aviso do fair-play financeiro",
                    format!(
                        "O clube perdeu {loss} nas últimas três épocas, acima do limite de {limit} do fair-play financeiro.\n\nA Direcção avisa: se as contas não melhorarem até ao fim da próxima época, o clube fica proibido de contratar."
                    ),
                ))
            } else if banned && !now_banned {
                Some((
                    "Fim da proibição de contratar",
                    "As contas do clube voltaram ao limite do fair-play financeiro e a proibição de contratar acabou.".to_string(),
                ))
            } else {
                None
            };
            if let Some((title, body)) = news {
                self.push_news(fresh, next_id, "message", title.into(), body);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Game, World};
    use rusqlite::params;

    fn new_game(club_id: i32, tag: &str) -> (Game, std::path::PathBuf) {
        let path = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        let dir = std::env::temp_dir().join(format!("nextcm-ffp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: format!("FFP {tag}"),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        (Game::create(&path, &dir, &new, 41).unwrap(), dir)
    }

    #[test]
    fn purchases_are_amortized_over_five_years() {
        if World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).is_none() {
            return;
        }
        let (g, dir) = new_game(6271, "amortizacao");
        let today = crate::date::parse(&g.date).unwrap();
        let log = |days: i32, person: i32, from: Option<i32>, to: Option<i32>, fee: i64, kind: &str| {
            g.world
                .conn
                .execute(
                    "INSERT INTO transfers (date, person_id, from_club, to_club, fee, kind) VALUES (?, ?, ?, ?, ?, ?)",
                    params![crate::date::format(today - days), person, from, to, fee, kind],
                )
                .unwrap();
        };
        let from = crate::date::format(today - 2000);
        // €100M há um ano: pesa €20M
        log(365, 1, Some(1350), Some(6271), 100_000_000, "transfer");
        assert!((g.amortization(6271, &from).unwrap() - 20_000_000).abs() < 100_000);
        // Um empréstimo conta inteiro
        log(10, 2, Some(1350), Some(6271), 5_000_000, "loan");
        assert!((g.amortization(6271, &from).unwrap() - 25_000_000).abs() < 100_000);
        // Vendido hoje: o que faltava amortizar entra todo (os €100M ficam pagos)
        log(0, 1, Some(6271), Some(1350), 60_000_000, "transfer");
        assert!((g.amortization(6271, &from).unwrap() - 105_000_000).abs() < 100_000);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn budgets_of_the_big_clubs_look_like_today() {
        if World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))).is_none() {
            return;
        }
        for (club, low, high) in [(8383, 180, 260), (6271, 130, 190), (2587, 5, 60)] {
            let (g, dir) = new_game(club, &format!("orcamento-{club}"));
            let b = g.board_view().unwrap();
            let funds = b.transfer_budget / 1_000_000;
            println!("{club}: fundos €{funds}M, folha {} limite {} por semana", b.wage_bill, b.wage_budget);
            assert!((low..=high).contains(&funds), "{club}: €{funds}M");
            drop(g);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
