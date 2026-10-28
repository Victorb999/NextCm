//! Finanças dos clubes que jogam (marco 3.8, passo 1): salários à segunda-feira, direitos de
//! TV, patrocínios, merchandising e estádio no dia 1 de cada mês, bilheteira em cada jogo em
//! casa e prémios no fim de cada competição. O saldo é `clubs.cash`; o livro de contas
//! (`finances`) guarda o total de cada categoria por época do clube.
//!
//! As receitas de uma época normal de um clube (`club_economy.base`) ficam fixas quando ele
//! entra no jogo: 1,3 vezes a folha salarial anual de então, para todos começarem perto do
//! equilíbrio. A TV é da liga: cada clube de uma liga recebe o mesmo, a média das bases dela.
//! Números e partes em `finance.rs` e no detalhe do 3.8 no ROADMAP.

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{OptionalExtension, params};

use super::Game;
use crate::engine;
use crate::finance::{self, ClubFinances, FinanceLine, SeasonBooks};
use crate::league::{CHAMPIONS_LEAGUE, EUROPA_LEAGUE, LIBERTADORES, Match, SUL_AMERICANA};
use crate::regulation::{self, StageKind};
use crate::{Result, date};

/// Competições continentais (prémios maiores).
const CONTINENTAL: [i32; 4] = [LIBERTADORES, SUL_AMERICANA, CHAMPIONS_LEAGUE, EUROPA_LEAGUE];

impl Game {
    /// Divisão de um clube (a liga das contas dele).
    fn division_of(&self, club: i32) -> Result<Option<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT division_id FROM clubs WHERE id = ?")?
            .query_row([club], |r| r.get(0))
            .optional()?
            .flatten())
    }

    /// Época das contas de um clube: a da liga dele (europeia ou ano civil).
    fn books_season(&self, club: i32) -> Result<i32> {
        Ok(regulation::season_of(self.division_of(club)?.unwrap_or(0), &self.date))
    }

    /// Receitas de uma época normal do clube, fixadas na primeira vez que ele aparece.
    pub(super) fn club_base(&self, club: i32) -> Result<f64> {
        if let Some(base) =
            self.world.conn.prepare_cached("SELECT base FROM club_economy WHERE club_id = ?")?.query_row([club], |r| r.get(0)).optional()?
        {
            return Ok(base);
        }
        let bill: i64 = self
            .world
            .conn
            .prepare_cached("SELECT COALESCE(SUM(wage), 0) FROM people WHERE is_player = 1 AND club_id = ?")?
            .query_row([club], |r| r.get(0))?;
        let base = (finance::REVENUE_OVER_WAGES * bill as f64 * 52.0).max(finance::MIN_REVENUE);
        self.world.conn.prepare_cached("INSERT INTO club_economy (club_id, base) VALUES (?, ?)")?.execute(params![club, base])?;
        Ok(base)
    }

    /// Direitos de TV por mês de um clube: a média das bases da liga dele (fora das ligas
    /// vivas, a dele).
    fn monthly_tv(&self, club: i32, league_tv: &mut HashMap<i32, f64>) -> Result<f64> {
        let division = self.division_of(club)?;
        let Some(div) = division.filter(|d| regulation::get(*d).is_some_and(|r| r.managed)) else {
            return Ok(self.club_base(club)? * finance::TV_SHARE / 12.0);
        };
        if let Some(&tv) = league_tv.get(&div) {
            return Ok(tv);
        }
        let members: Vec<i32> = self
            .world
            .conn
            .prepare_cached("SELECT id FROM clubs WHERE division_id = ? ORDER BY id")?
            .query_map([div], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut total = 0.0;
        for &c in &members {
            total += self.club_base(c)?;
        }
        let tv = total / members.len().max(1) as f64 * finance::TV_SHARE / 12.0;
        league_tv.insert(div, tv);
        Ok(tv)
    }

    /// Lança um movimento: as receitas somam ao saldo e as despesas subtraem; o livro
    /// guarda o total (positivo) de cada categoria na época do clube.
    pub(super) fn book(&self, club: i32, category: &str, amount: i64) -> Result<()> {
        if amount == 0 {
            return Ok(());
        }
        let season = self.books_season(club)?;
        self.world
            .conn
            .prepare_cached(
                "INSERT INTO finances (club_id, season, category, amount) VALUES (?, ?, ?, ?)
             ON CONFLICT (club_id, season, category) DO UPDATE SET amount = amount + excluded.amount",
            )?
            .execute(params![club, season, category, amount])?;
        let signed = if finance::EXPENSES.iter().any(|(k, _)| *k == category) { -amount } else { amount };
        self.world.conn.prepare_cached("UPDATE clubs SET cash = COALESCE(cash, 0) + ? WHERE id = ?")?.execute(params![signed, club])?;
        Ok(())
    }

    /// Clubes que jogam nesta época, por ordem (os que têm finanças vivas).
    fn paying_clubs(&self) -> Vec<i32> {
        let mut clubs: Vec<i32> = self.squads.traits.values().map(|t| t.club_id).collect::<HashSet<_>>().into_iter().collect();
        clubs.sort_unstable();
        clubs
    }

    /// O dinheiro do dia: salários à segunda-feira, receitas e despesas mensais no dia 1.
    pub(super) fn daily_finances(&mut self) -> Result<()> {
        let monday = date::weekday(&self.date) == Some(1);
        let first = self.date.ends_with("-01");
        if !monday && !first {
            return Ok(());
        }
        let clubs = self.paying_clubs();
        if monday {
            let bills: HashMap<i32, i64> = self
                .world
                .conn
                .prepare_cached(
                    "SELECT club_id, SUM(COALESCE(wage, 0)) FROM people WHERE is_player = 1 AND club_id IS NOT NULL GROUP BY club_id",
                )?
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            // Emprestados: o clube dono paga a parte do salário combinada
            let mut bills = bills;
            for (club, parent, part) in self.loan_wage_shifts()? {
                *bills.entry(club).or_default() -= part;
                *bills.entry(parent).or_default() += part;
            }
            for &club in &clubs {
                let bill = bills.get(&club).copied().unwrap_or(0);
                self.book(club, "wages", bill)?;
                self.book(club, "staff", (bill as f64 * finance::STAFF_SHARE).round() as i64)?;
            }
        }
        if first {
            let mut league_tv = HashMap::new();
            for &club in &clubs {
                let revenue = self.club_base(club)?;
                let month = |share: f64| (revenue * share / 12.0).round() as i64;
                self.book(club, "tv", self.monthly_tv(club, &mut league_tv)?.round() as i64)?;
                self.book(club, "sponsors", month(finance::SPONSOR_SHARE))?;
                self.book(club, "merchandising", month(finance::MERCH_SHARE))?;
                self.book(club, "stadium", month(finance::STADIUM_SHARE))?;
                let cash: i64 = self.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?;
                if cash < 0 {
                    self.book(club, "interest", (-cash as f64 * finance::MONTHLY_INTEREST).round() as i64)?;
                }
            }
        }
        self.unsaved = true;
        Ok(())
    }

    /// Assistência de um jogo: a lotação do estádio da casa, mais cheia nos clubes com mais
    /// reputação, com um sorteio fixo por partida (a tela do jogo mostra a mesma).
    pub(super) fn attendance(&self, m: &Match) -> Result<i32> {
        let (capacity, reputation): (Option<i32>, i32) = self
            .world
            .conn
            .prepare_cached(
                "SELECT s.capacity, COALESCE(c.reputation, 0) FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id WHERE c.id = ?",
            )?
            .query_row([m.home_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut rng = engine::match_rng(self.seed ^ 0x00A7_7E4D, m.id);
        let fill = (0.45 + 0.30 * reputation as f64 / 10_000.0 + 0.25 * engine::uniform(&mut rng)).min(0.98);
        Ok((capacity.unwrap_or(15_000).max(1_000) as f64 * fill).round() as i32)
    }

    /// Bilheteira de um jogo acabado: a parte das receitas de uma época que um jogo em casa
    /// rende, pela assistência (com três quartos da lotação rende a média). Em campo neutro,
    /// metade para cada um.
    pub(super) fn gate(&self, m: &Match) -> Result<()> {
        let attendance = self.attendance(m)? as f64;
        let capacity: Option<i32> = self
            .world
            .conn
            .prepare_cached("SELECT s.capacity FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id WHERE c.id = ?")?
            .query_row([m.home_id], |r| r.get(0))?;
        let full = 0.75 * capacity.unwrap_or(15_000).max(1_000) as f64;
        let per_game = |club: i32| -> Result<i64> {
            let revenue = self.club_base(club)?;
            Ok((revenue * finance::GATE_SHARE / finance::HOME_GAMES * (attendance / full)).round() as i64)
        };
        if m.neutral {
            for club in [m.home_id, m.away_id] {
                self.book(club, "gate", per_game(club)? / 2)?;
            }
        } else {
            self.book(m.home_id, "gate", per_game(m.home_id)?)?;
        }
        Ok(())
    }

    /// Prémios de uma competição acabada: uma parte das receitas de todos os participantes.
    /// Numa liga, repartida pela posição final (o 1º recebe n partes, o último 1); nas taças
    /// e continentais, 40% ao campeão, 20% ao vice e o resto por todos.
    pub(super) fn prizes(&self, comp: i32, year: i32) -> Result<()> {
        let Some(reg) = regulation::get(comp) else { return Ok(()) };
        let clubs: Vec<i32> = {
            let mut c: Vec<i32> =
                self.year_matches(comp, year).flat_map(|m| [m.home_id, m.away_id]).collect::<HashSet<_>>().into_iter().collect();
            c.sort_unstable();
            c
        };
        if clubs.is_empty() {
            return Ok(());
        }
        let mut pool = 0.0;
        for &c in &clubs {
            pool += self.club_base(c)?;
        }
        let league = reg.stages[0].kind == StageKind::League && !reg.stages[0].entrants.divisions.is_empty();
        let rate = if CONTINENTAL.contains(&comp) {
            finance::CONTINENTAL_PRIZES
        } else if league {
            finance::LEAGUE_PRIZES
        } else {
            finance::CUP_PRIZES
        };
        let pool = pool * rate;
        let mut share: BTreeMap<i32, f64> = BTreeMap::new();
        if league {
            let table = self.stage_table(comp, year, 0, None);
            let n = table.len() as f64;
            let parts = n * (n + 1.0) / 2.0;
            for r in &table {
                *share.entry(r.club.id).or_default() += pool * (n - r.position as f64 + 1.0) / parts;
            }
        } else if let Some((winner, second, _)) = self.podium(comp, year) {
            *share.entry(winner).or_default() += pool * 0.4;
            if let Some(s) = second {
                *share.entry(s).or_default() += pool * 0.2;
            }
            for &c in &clubs {
                *share.entry(c).or_default() += pool * 0.4 / clubs.len() as f64;
            }
        }
        for (club, amount) in share {
            self.book(club, "prizes", amount.round() as i64)?;
        }
        Ok(())
    }

    fn season_books(&self, club: i32, season: i32) -> Result<Option<SeasonBooks>> {
        let rows: HashMap<String, i64> = self
            .world
            .conn
            .prepare_cached("SELECT category, amount FROM finances WHERE club_id = ? AND season = ?")?
            .query_map(params![club, season], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        if rows.is_empty() {
            return Ok(None);
        }
        let lines = |cats: &[(&str, &str)]| -> Vec<FinanceLine> {
            cats.iter().map(|(k, label)| FinanceLine { label: (*label).into(), amount: rows.get(*k).copied().unwrap_or(0) }).collect()
        };
        let (income, expenses) = (lines(&finance::INCOME), lines(&finance::EXPENSES));
        let division = self.division_of(club)?.unwrap_or(0);
        Ok(Some(SeasonBooks {
            season: regulation::season_label(division, season),
            total_income: income.iter().map(|l| l.amount).sum(),
            total_expenses: expenses.iter().map(|l| l.amount).sum(),
            income,
            expenses,
        }))
    }

    /// A tela Finanças de um clube: saldo, folha, e as contas desta época e da anterior.
    pub fn club_finances(&self, club: i32) -> Result<ClubFinances> {
        let cash: i64 = self.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?;
        let wage_bill: i64 =
            self.world
                .conn
                .query_row("SELECT COALESCE(SUM(wage), 0) FROM people WHERE is_player = 1 AND club_id = ?", [club], |r| r.get(0))?;
        let season = self.books_season(club)?;
        let division = self.division_of(club)?.unwrap_or(0);
        let this_season = self.season_books(club, season)?.unwrap_or_else(|| SeasonBooks {
            season: regulation::season_label(division, season),
            income: finance::INCOME.iter().map(|(_, l)| FinanceLine { label: (*l).into(), amount: 0 }).collect(),
            expenses: finance::EXPENSES.iter().map(|(_, l)| FinanceLine { label: (*l).into(), amount: 0 }).collect(),
            total_income: 0,
            total_expenses: 0,
        });
        Ok(ClubFinances {
            club_id: club,
            cash,
            wage_bill,
            expected_revenue: finance::round_money(self.club_base(club)?),
            this_season,
            last_season: self.season_books(club, season - 1)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{Game, NewGame, World};

    #[test]
    fn books_add_up_after_two_months() {
        let Some(path) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-finances-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = NewGame {
            save_name: "Contas".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
        };
        let mut g = Game::create(&path, &dir, &new, 6).unwrap();
        let start = g.club_finances(7568).unwrap().cash;
        while g.date() < "2025-09-01" {
            g.advance_day().unwrap();
        }
        let f = g.club_finances(7568).unwrap();
        let amount = |label: &str, lines: &[crate::FinanceLine]| lines.iter().find(|l| l.label == label).map_or(0, |l| l.amount);
        let books = &f.this_season;
        assert!(amount("Salários dos jogadores", &books.expenses) > 0);
        assert!(amount("Direitos de TV", &books.income) > 0, "TV a 1/8 e 1/9");
        assert!(amount("Bilheteira", &books.income) > 0, "jogos em casa em agosto");
        assert_eq!(f.cash, start + books.total_income - books.total_expenses);
        // A TV é igual para todos os clubes da Série A
        let flamengo = g.club_finances(3675).unwrap();
        assert_eq!(amount("Direitos de TV", &flamengo.this_season.income), amount("Direitos de TV", &books.income));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
