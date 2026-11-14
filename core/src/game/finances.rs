//! Finanças dos clubes que jogam (marco 3.8, passo 1; economia do CM no passo 3.15c): salários
//! (com o staff) à segunda-feira, TV, merchandising, estádio e juros no dia 1 de cada mês,
//! bilheteira em cada jogo em casa (assistência × bilhete) e prémios no fim de cada competição.
//! O saldo é `clubs.cash`; o livro de contas guarda o total de cada rubrica por época do clube
//! (`finances`) e por mês (`finances_month`, só o corrente e o anterior).
//!
//! As receitas de referência de um clube (`club_economy.base`) são as reais ou as da curva
//! (`finance::club_revenue`), refeitas em cada época nova. A TV é da liga: cada clube de uma liga
//! recebe o mesmo, a média das bases dela. Números e partes em `finance.rs` e
//! docs/roadmap/3.22-economia-moderna.md.

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{OptionalExtension, params};

use super::Game;
use crate::engine;
use crate::finance::{self, ClubFinances, FinanceRow};
use crate::league::{CHAMPIONS_LEAGUE, EUROPA_LEAGUE, LIBERTADORES, Match, SUL_AMERICANA};
use crate::regulation::{self, StageKind};
use crate::{Result, date};

/// Competições continentais (prémios maiores).
pub(super) const CONTINENTAL: [i32; 4] = [LIBERTADORES, SUL_AMERICANA, CHAMPIONS_LEAGUE, EUROPA_LEAGUE];

impl Game {
    /// Jogo novo: nos salários do patch × `WAGE_SCALE` os jovens das grandes equipas ganham como
    /// estrelas (o Mora com 19 anos €13,8M por ano, o Lennart Karl com 17 €22M). Nenhum jogador
    /// até `START_WAGE_AGE` anos começa acima de `START_WAGE_SLACK` vezes o que pediria pelo valor
    /// e pela idade ao clube onde está; os mais velhos e quem ganha menos ficam como estão.
    pub(super) fn fit_start_wages(&mut self) -> Result<()> {
        let rows: Vec<(i32, i64, i32, Option<String>, Option<i32>, i32)> = self
            .world
            .conn
            .prepare(
                "SELECT p.id, p.wage, COALESCE(pl.current_ability, 1), p.birth_date, p.birth_year, COALESCE(c.reputation, 0)
                 FROM people p JOIN players pl ON pl.person_id = p.id JOIN clubs c ON c.id = p.club_id
                 WHERE p.is_player = 1 AND p.wage >= ?",
            )?
            .query_map([(1000.0 * finance::WAGE_SCALE) as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, wage, ca, birth, birth_year, rep) in rows {
            let age = super::season::age_on(birth.as_deref(), birth_year, &self.date);
            if age > finance::START_WAGE_AGE {
                continue;
            }
            let value = self.world.market_value_of(id, &self.date)?.unwrap_or(0);
            let fair = finance::wage_demand(value.max(ca as i64 * 20_000), 0, rep, age) as f64 * finance::START_WAGE_SLACK;
            if (wage as f64) > fair {
                self.world.conn.execute("UPDATE people SET wage = ? WHERE id = ?", params![finance::round_wage(fair), id])?;
            }
        }
        Ok(())
    }

    /// Divisão de um clube (a liga das contas dele).
    pub(super) fn division_of(&self, club: i32) -> Result<Option<i32>> {
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

    /// Receitas de uma época normal do clube ([`finance::club_revenue`]), guardadas na primeira
    /// vez que ele aparece em cada época (pela divisão e reputação de então).
    pub(super) fn club_base(&self, club: i32) -> Result<f64> {
        if let Some(base) =
            self.world.conn.prepare_cached("SELECT base FROM club_economy WHERE club_id = ?")?.query_row([club], |r| r.get(0)).optional()?
        {
            return Ok(base);
        }
        let (rep, division): (i32, Option<i32>) = self
            .world
            .conn
            .prepare_cached("SELECT COALESCE(reputation, 0), division_id FROM clubs WHERE id = ?")?
            .query_row([club], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?
            .unwrap_or((0, None));
        let base = finance::club_revenue(club, rep, division);
        self.world.conn.prepare_cached("INSERT INTO club_economy (club_id, base) VALUES (?, ?)")?.execute(params![club, base])?;
        Ok(base)
    }

    /// Época nova: as receitas dos clubes deste tipo de época voltam a ser calculadas (quem subiu
    /// ou desceu muda de liga).
    pub(super) fn reset_bases(&self, kind: regulation::SeasonKind) -> Result<()> {
        let clubs: Vec<(i32, Option<i32>)> = self
            .world
            .conn
            .prepare("SELECT c.id, c.division_id FROM club_economy e JOIN clubs c ON c.id = e.club_id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (club, division) in clubs {
            if self.club_kind(division) == kind {
                self.world.conn.execute("DELETE FROM club_economy WHERE club_id = ?", [club])?;
            }
        }
        Ok(())
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
        debug_assert!(finance::INCOME.iter().chain(&finance::EXPENSES).any(|(k, _)| *k == category), "rubrica {category}");
        let season = self.books_season(club)?;
        self.world
            .conn
            .prepare_cached(
                "INSERT INTO finances (club_id, season, category, amount) VALUES (?, ?, ?, ?)
             ON CONFLICT (club_id, season, category) DO UPDATE SET amount = amount + excluded.amount",
            )?
            .execute(params![club, season, category, amount])?;
        self.world
            .conn
            .prepare_cached(
                "INSERT INTO finances_month (club_id, month, category, amount) VALUES (?, ?, ?, ?)
             ON CONFLICT (club_id, month, category) DO UPDATE SET amount = amount + excluded.amount",
            )?
            .execute(params![club, &self.date[..7], category, amount])?;
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

    /// O dinheiro do dia: salários (e o staff) à segunda-feira, receitas e despesas mensais e
    /// juros no dia 1.
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
                self.book(club, "wages", (bill as f64 * (1.0 + finance::STAFF_SHARE)).round() as i64)?;
            }
        }
        if first {
            // Só ficam o mês corrente e o anterior
            let previous = date::add_days(&self.date, -1).map(|d| d[..7].to_string()).unwrap_or_default();
            self.world.conn.execute("DELETE FROM finances_month WHERE month < ?", [&previous])?;
            let mut league_tv = HashMap::new();
            for &club in &clubs {
                let revenue = self.club_base(club)?;
                let month = |share: f64| (revenue * share / 12.0).round() as i64;
                self.book(club, "tv", self.monthly_tv(club, &mut league_tv)?.round() as i64)?;
                self.book(club, "merchandising", month(finance::MERCH_SHARE))?;
                self.book(club, "stadium", month(finance::STADIUM_SHARE))?;
                self.book(club, "other_expenses", month(finance::RUNNING_SHARE))?;
                let cash: i64 = self.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?;
                // 1/7: os dividendos aos accionistas, sobre o saldo acima da reserva
                let reserve = finance::DIVIDEND_RESERVE * revenue;
                let dividends = if self.date[5..].starts_with("07-01") && cash as f64 > reserve {
                    ((cash as f64 - reserve) * finance::DIVIDEND_SHARE).round() as i64
                } else {
                    0
                };
                if dividends > 0 {
                    self.book(club, "dividends", dividends)?;
                }
                let cash = cash - dividends;
                self.ai_stadium_works(club, cash)?;
                if cash < 0 {
                    self.book(club, "interest", (-cash as f64 * finance::MONTHLY_INTEREST).round() as i64)?;
                } else {
                    self.book(club, "interest_income", (cash as f64 * finance::SAVINGS_INTEREST).round() as i64)?;
                }
            }
        }
        self.unsaved = true;
        Ok(())
    }

    /// Assistência de um jogo: a do banco (média, mínima e máxima do clube) com um sorteio fixo
    /// por partida; sem esses dados, a lotação do estádio da casa, mais cheia nos clubes com mais
    /// reputação. Nunca passa da lotação (a tela do jogo mostra a mesma).
    pub(super) fn attendance(&self, m: &Match) -> Result<i32> {
        type Row = (Option<i32>, i32, [Option<i32>; 3]);
        let (capacity, reputation, crowd): Row = self
            .world
            .conn
            .prepare_cached(
                "SELECT s.capacity, COALESCE(c.reputation, 0), c.avg_attendance, c.min_attendance, c.max_attendance
                 FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id WHERE c.id = ?",
            )?
            .query_row([m.home_id], |r| Ok((r.get(0)?, r.get(1)?, [r.get(2)?, r.get(3)?, r.get(4)?])))?;
        let capacity = capacity.unwrap_or(15_000).max(1_000);
        let mut rng = engine::match_rng(self.seed ^ 0x00A7_7E4D, m.id);
        let u = engine::uniform(&mut rng);
        if let [Some(avg), min, max] = crowd {
            // A média no meio do sorteio: metade abaixo (até à mínima), metade acima (até à máxima)
            let (min, max) = (min.unwrap_or(avg).min(avg) as f64, max.unwrap_or(avg).max(avg) as f64);
            let avg = avg as f64;
            let people = if u < 0.5 { min + (avg - min) * 2.0 * u } else { avg + (max - avg) * (2.0 * u - 1.0) };
            return Ok((people.round() as i32).clamp(1_000.min(capacity), capacity));
        }
        let fill = (0.45 + 0.30 * reputation as f64 / 10_000.0 + 0.25 * u).min(0.98);
        Ok((capacity as f64 * fill).round() as i32)
    }

    /// Bilheteira de um jogo acabado: a assistência vezes o bilhete do clube da casa (pela
    /// reputação). Em campo neutro, metade para cada um. Nas continentais, o prémio do jogo.
    pub(super) fn gate(&self, m: &Match) -> Result<()> {
        if CONTINENTAL.contains(&m.competition_id) {
            for club in [m.home_id, m.away_id] {
                let prize = finance::CONTINENTAL_PRIZES * finance::CONTINENTAL_MATCH * self.club_base(club)?;
                self.book(club, "prizes", prize.round() as i64)?;
            }
        }
        let attendance = self.attendance(m)? as f64;
        let reputation: i32 = self
            .world
            .conn
            .prepare_cached("SELECT COALESCE(reputation, 0) FROM clubs WHERE id = ?")?
            .query_row([m.home_id], |r| r.get(0))?;
        let takings = (attendance * finance::ticket_price(reputation)).round() as i64;
        if m.neutral {
            for club in [m.home_id, m.away_id] {
                self.book(club, "gate", takings / 2)?;
            }
        } else {
            self.book(m.home_id, "gate", takings)?;
        }
        Ok(())
    }

    /// Prémios de uma competição acabada, sobre as receitas de referência de cada clube: numa
    /// liga de 0,6× (o último) a 1,2× (o campeão) a parte da competição; nas taças e
    /// continentais, metade a quem jogou, mais 1× ao campeão e ½× ao vice.
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
        let league = reg.stages[0].kind == StageKind::League && !reg.stages[0].entrants.divisions.is_empty();
        let rate = if CONTINENTAL.contains(&comp) {
            finance::CONTINENTAL_PRIZES
        } else if league {
            finance::LEAGUE_PRIZES
        } else {
            finance::CUP_PRIZES
        };
        let mut share: BTreeMap<i32, f64> = BTreeMap::new();
        if league {
            let table = self.stage_table(comp, year, 0, None);
            let last = (table.len() as f64 - 1.0).max(1.0);
            for r in &table {
                let place = (table.len() as f64 - r.position as f64) / last;
                *share.entry(r.club.id).or_default() += rate * self.club_base(r.club.id)? * (0.6 + 0.6 * place);
            }
        } else {
            let (winner, second) = self.podium(comp, year).map_or((None, None), |(w, s, _)| (Some(w), s));
            for &c in &clubs {
                let entry = if CONTINENTAL.contains(&comp) { 0.0 } else { 0.5 };
                let factor = entry
                    + if Some(c) == winner {
                        1.0
                    } else if Some(c) == second {
                        0.5
                    } else {
                        0.0
                    };
                *share.entry(c).or_default() += rate * self.club_base(c)? * factor;
            }
        }
        for (club, amount) in share {
            self.book(club, "prizes", amount.round() as i64)?;
        }
        Ok(())
    }

    /// O livro de um clube numa época ou num mês ("AAAA-MM"), por linha (as chaves antigas
    /// entram na linha delas).
    fn book_totals(&self, club: i32, table: &str, key: &str, period: &dyn rusqlite::ToSql) -> Result<HashMap<String, i64>> {
        let mut out: HashMap<String, i64> = HashMap::new();
        let sql = format!("SELECT category, amount FROM {table} WHERE club_id = ? AND {key} = ?");
        let rows: Vec<(String, i64)> = self
            .world
            .conn
            .prepare_cached(&sql)?
            .query_map(params![club, period], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (k, v) in rows {
            *out.entry(finance::line_key(&k).to_string()).or_default() += v;
        }
        Ok(out)
    }

    /// A tela Finanças de um clube, como no CM: cada rubrica no mês, no mês anterior, na época
    /// e na época anterior; o saldo e os fundos de transferências.
    pub fn club_finances(&self, club: i32) -> Result<ClubFinances> {
        let cash: i64 = self.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?;
        let wage_bill: i64 =
            self.world
                .conn
                .query_row("SELECT COALESCE(SUM(wage), 0) FROM people WHERE is_player = 1 AND club_id = ?", [club], |r| r.get(0))?;
        let season = self.books_season(club)?;
        let division = self.division_of(club)?.unwrap_or(0);
        let month = self.date[..7].to_string();
        let last_month = date::add_days(&format!("{month}-01"), -1).map(|d| d[..7].to_string()).unwrap_or_default();
        let books = [
            self.book_totals(club, "finances_month", "month", &month)?,
            self.book_totals(club, "finances_month", "month", &last_month)?,
            self.book_totals(club, "finances", "season", &season)?,
            self.book_totals(club, "finances", "season", &(season - 1))?,
        ];
        let rows = |cats: &[(&str, &str)]| -> Vec<FinanceRow> {
            cats.iter()
                .map(|(k, label)| FinanceRow { label: (*label).into(), amounts: books.each_ref().map(|b| b.get(*k).copied().unwrap_or(0)) })
                .collect()
        };
        let total = |label: &str, lines: &[FinanceRow]| FinanceRow {
            label: label.into(),
            amounts: std::array::from_fn(|i| lines.iter().map(|l| l.amounts[i]).sum()),
        };
        let (income, expenses) = (rows(&finance::INCOME), rows(&finance::EXPENSES));
        let (total_income, total_expenses) = (total("Receitas", &income), total("Despesas", &expenses));
        let result = FinanceRow {
            label: "Ganho/(Perda)".into(),
            amounts: std::array::from_fn(|i| total_income.amounts[i] - total_expenses.amounts[i]),
        };
        let funds = if club == self.club_id && !self.sacked() { self.transfer_budget()? } else { super::board::round_funds(cash) };
        Ok(ClubFinances {
            club_id: club,
            cash,
            funds,
            wage_bill,
            expected_revenue: finance::round_money(self.club_base(club)?),
            season: regulation::season_label(division, season),
            income,
            expenses,
            total_income,
            total_expenses,
            result,
            fair_play_result: self.fair_play_result(club)?,
            fair_play_limit: self.fair_play_limit(club)?,
            fair_play_banned: self.fair_play_banned(club)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{Game, NewGame, World};

    #[test]
    fn start_wages_follow_value_and_age() {
        let Some(path) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-salarios-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = NewGame {
            save_name: "Salários".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let g = Game::create(&path, &dir, &new, 6).unwrap();
        let year = |name: &str| -> f64 {
            g.world
                .conn
                .query_row("SELECT wage FROM people WHERE display_name = ? AND is_player = 1", [name], |r| r.get::<_, i64>(0))
                .unwrap() as f64
                * 52.0
                / 1e6
        };
        let bill = |club: i32| -> f64 {
            g.world
                .conn
                .query_row("SELECT COALESCE(SUM(wage), 0) FROM people WHERE club_id = ? AND is_player = 1", [club], |r| r.get::<_, i64>(0))
                .unwrap() as f64
                * 52.0
                / 1e6
        };
        for name in ["Rodrigo Mora", "Arda Güler", "Lennart Karl", "Lamine Yamal", "Kylian Mbappé", "Manu Koné", "Bruno Fernandes"] {
            println!("{name}: €{:.1}M por ano", year(name));
        }
        for (club, name) in [(8383, "Real"), (6271, "United"), (8609, "Roma"), (7568, "Palmeiras")] {
            println!("{name}: folha €{:.0}M por ano", bill(club));
        }
        // Os jovens deixam de ganhar como estrelas; as estrelas continuam
        assert!(year("Rodrigo Mora") < 6.0 && year("Lennart Karl") < 6.0, "{} {}", year("Rodrigo Mora"), year("Lennart Karl"));
        assert!(year("Kylian Mbappé") > 30.0 && year("Bruno Fernandes") > 20.0);
        // As folhas ficam na casa das reais (Real ~€450M, United ~€370M, Palmeiras ~€80M)
        assert!((300.0..500.0).contains(&bill(8383)) && (250.0..400.0).contains(&bill(6271)), "{} {}", bill(8383), bill(6271));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dividends_on_the_first_of_july() {
        let Some(path) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-dividendos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = NewGame {
            save_name: "Dividendos".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 6).unwrap();
        let base = g.club_base(7568).unwrap();
        let cash = |g: &Game| -> i64 { g.world.conn.query_row("SELECT cash FROM clubs WHERE id = 7568", [], |r| r.get(0)).unwrap() };
        let dividends = |g: &Game| -> i64 {
            g.world
                .conn
                .query_row("SELECT COALESCE(SUM(amount), 0) FROM finances WHERE club_id = 7568 AND category = 'dividends'", [], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        // Com o saldo abaixo da reserva, nada
        g.world.conn.execute("UPDATE clubs SET cash = ? WHERE id = 7568", [(0.3 * base) as i64]).unwrap();
        g.date = "2026-07-01".into();
        g.daily_finances().unwrap();
        assert_eq!(dividends(&g), 0);
        // Com três épocas de receitas, um quarto do que passa da reserva (2,5 épocas)
        g.world.conn.execute("UPDATE clubs SET cash = ? WHERE id = 7568", [(3.0 * base) as i64]).unwrap();
        g.date = "2027-07-01".into();
        g.daily_finances().unwrap();
        let paid = dividends(&g);
        // (as receitas e despesas do dia 1 entram antes)
        assert!((paid as f64 / (0.625 * base) - 1.0).abs() < 0.05, "{paid} contra {}", 0.625 * base);
        assert!(cash(&g) < (3.0 * base) as i64);
        // Noutro dia 1, nada
        g.date = "2027-08-01".into();
        g.daily_finances().unwrap();
        assert_eq!(dividends(&g), paid);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

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
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 6).unwrap();
        let start = g.club_finances(7568).unwrap().cash;
        while g.date() < "2025-09-01" {
            g.advance_day().unwrap();
        }
        let f = g.club_finances(7568).unwrap();
        // Época (coluna 2) e mês (coluna 0)
        let amount =
            |label: &str, lines: &[crate::FinanceRow], col: usize| lines.iter().find(|l| l.label == label).map_or(0, |l| l.amounts[col]);
        assert!(amount("Salário", &f.expenses, 2) > 0);
        assert!(amount("Receitas TV", &f.income, 2) > 0, "TV a 1/8 e 1/9");
        assert!(amount("Receitas Bilheteira", &f.income, 2) > 0, "jogos em casa em agosto");
        assert!(amount("Receitas TV", &f.income, 0) > 0 && amount("Receitas TV", &f.income, 1) > 0, "setembro e agosto");
        assert_eq!(f.cash, start + f.result.amounts[2]);
        assert_eq!(f.result.amounts[2], f.total_income.amounts[2] - f.total_expenses.amounts[2]);
        // A TV é igual para todos os clubes da Série A
        let flamengo = g.club_finances(3675).unwrap();
        assert_eq!(amount("Receitas TV", &flamengo.income, 2), amount("Receitas TV", &f.income, 2));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
