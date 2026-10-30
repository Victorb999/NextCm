//! Mercado da IA (marco 3.8, passo 2): transferências e empréstimos entre clubes nas janelas,
//! jogadores livres, devoluções de emprestados e dispensas no fim da janela. Corre às segundas
//! e quintas com a janela aberta; nesses dias lê-se uma vez um índice de todos os jogadores e
//! cada clube gerido (menos o do treinador, que decide ele no passo 3) olha para o plantel:
//! posições com poucos jogadores ou com titulares fracos para o nível do clube. Regras e
//! números em docs/roadmap/3.08-mercado.md.

use std::collections::{HashMap, HashSet};

use rand_chacha::ChaCha8Rng;
use rand_core::RngCore;
use rusqlite::params;
use serde::Serialize;
use ts_rs::TS;

use super::{Game, NewsItem, age_on, cm_date};
use crate::engine::uniform;
use crate::finance::{self, market_value, wage_demand};
use crate::person::ClubRef;
use crate::personality;
use crate::regulation::{self, SeasonKind};
use crate::{Result, date};

/// Papel de um jogador: 0 GR, 1 defesa, 2 meio, 3 ataque.
const ROLES: usize = 4;
/// Mínimo por papel num plantel (abaixo disto o clube procura e não vende).
pub(super) const MIN_ROLE: [usize; ROLES] = [2, 6, 6, 3];
/// Titulares por papel (para o titular mais fraco de cada posição).
const STARTERS: [usize; ROLES] = [1, 4, 4, 2];
/// Plantel máximo (jogadores de 21 anos ou mais) para comprar; acima de `RELEASE_ABOVE`
/// dispensa os piores no fim da janela, até `MAX_RELEASES`. Os mais novos não contam.
const MAX_SQUAD: usize = 30;
const RELEASE_ABOVE: usize = 32;
const MAX_RELEASES: usize = 5;
/// Dias no clube antes de poder ser vendido outra vez.
const SETTLE_DAYS: i32 = 180;
/// Idade a partir da qual um jogador conta para o plantel.
const SENIOR_AGE: i32 = 21;
/// Entradas e saídas por clube numa janela (sem contar empréstimos e dispensas).
const MAX_DEALS: usize = 3;
/// Empréstimos de jovens por clube numa janela.
const MAX_LOANS_OUT: usize = 2;
/// Parte do saldo que um clube gasta numa janela, e o teto da folha sobre as receitas.
const BUDGET_SHARE: f64 = 0.5;
/// Dívida (saldo negativo sobre as receitas de uma época) a partir da qual um clube da IA vende
/// o jogador mais valioso num dia de mercado, a 80% do valor; e a partir da qual vai à falência
/// no fim da época (passo 4).
const FORCED_SALE_DEBT: f64 = 0.25;
pub(super) const BANKRUPT_DEBT: f64 = 0.6;
/// Parte das receitas de uma época que entra no orçamento mesmo sem saldo.
const BUDGET_REVENUE: f64 = 0.2;
const WAGE_CAP: f64 = 0.90;
/// Listas da IA: quantos na de transferências e na de empréstimos, o preço dos listados (parte
/// do valor) e a vantagem (em CA) que têm na escolha de quem compra.
const MAX_LISTED: usize = 3;
const MAX_LOAN_LISTED: usize = 2;
const LISTED_FEE: f64 = 0.9;
const LISTED_EDGE: i32 = 5;

/// Um jogador no índice do mercado.
#[derive(Debug, Clone)]
struct Listing {
    id: i32,
    club: Option<i32>,
    ca: i32,
    pa: i32,
    role: usize,
    age: i32,
    rep: i32,
    wage: i64,
    years_left: f64,
    club_rep: i32,
    league_rep: i32,
    /// Chegou ao clube há menos de [`SETTLE_DAYS`] dias: não é vendido.
    recent: bool,
    ambition: f64,
    loyalty: f64,
    /// Continente do clube (−1 sem clube) e nação do jogador.
    continent: i32,
    nation: i32,
}

impl Listing {
    fn value(&self) -> i64 {
        market_value(self.ca, if self.pa > 0 { self.pa } else { self.ca + 10 }, self.age, self.rep, self.league_rep, self.years_left)
    }
}

/// O mercado de um dia: o índice e as vistas que a IA consulta, atualizadas a cada negócio.
struct Market {
    listings: Vec<Listing>,
    by_id: HashMap<i32, usize>,
    /// Jogadores de cada clube, do melhor para o pior (CA).
    by_club: HashMap<i32, Vec<usize>>,
    /// Jogadores de cada papel, do melhor para o pior (CA).
    by_role: [Vec<usize>; ROLES],
    /// Os 11 melhores de cada clube (custam mais).
    key: HashSet<i32>,
    /// Na lista de transferências do clube: mais baratos e os primeiros que a IA procura.
    listed: HashSet<i32>,
}

impl Market {
    fn new(listings: Vec<Listing>) -> Market {
        let by_id = listings.iter().enumerate().map(|(i, l)| (l.id, i)).collect();
        let mut by_role: [Vec<usize>; ROLES] = Default::default();
        for (i, l) in listings.iter().enumerate() {
            by_role[l.role].push(i);
        }
        for r in &mut by_role {
            r.sort_by(|&a, &b| listings[b].ca.cmp(&listings[a].ca).then(listings[a].id.cmp(&listings[b].id)));
        }
        let mut m = Market { listings, by_id, by_club: HashMap::new(), by_role, key: HashSet::new(), listed: HashSet::new() };
        for i in 0..m.listings.len() {
            if let Some(c) = m.listings[i].club {
                m.by_club.entry(c).or_default().push(i);
            }
        }
        let clubs: Vec<i32> = m.by_club.keys().copied().collect();
        for c in clubs {
            m.resort(c);
        }
        m
    }

    /// Volta a ordenar um clube e a marcar os 11 melhores dele.
    fn resort(&mut self, club: i32) {
        let listings = &self.listings;
        let Some(v) = self.by_club.get_mut(&club) else { return };
        v.sort_by(|&a, &b| listings[b].ca.cmp(&listings[a].ca).then(listings[a].id.cmp(&listings[b].id)));
        for (k, &i) in v.iter().enumerate() {
            if k < 11 {
                self.key.insert(listings[i].id);
            } else {
                self.key.remove(&listings[i].id);
            }
        }
    }

    fn squad(&self, club: i32) -> impl Iterator<Item = &Listing> {
        self.by_club.get(&club).into_iter().flatten().map(move |&i| &self.listings[i])
    }

    fn role_count(&self, club: i32, role: usize) -> usize {
        self.squad(club).filter(|l| l.role == role).count()
    }

    /// Um negócio: o jogador passa para o clube `to` (ou fica livre).
    fn move_player(&mut self, id: i32, to: Option<i32>) {
        let Some(&i) = self.by_id.get(&id) else { return };
        if let Some(from) = self.listings[i].club {
            if let Some(v) = self.by_club.get_mut(&from) {
                v.retain(|&x| x != i);
            }
            self.key.remove(&id);
            self.resort(from);
        }
        self.listings[i].club = to;
        self.listings[i].recent = true;
        if let Some(t) = to {
            self.by_club.entry(t).or_default().push(i);
            self.resort(t);
        }
    }

    /// Tamanho (só os de 21 anos ou mais), contagens por papel, titular mais fraco de cada papel e nível (média dos 11 melhores).
    fn shape(&self, club: i32) -> (usize, [usize; ROLES], [i32; ROLES], i32) {
        let mut count = [0usize; ROLES];
        let mut weakest = [0i32; ROLES];
        let mut seen = [0usize; ROLES];
        let (mut n, mut top, mut sum) = (0usize, 0usize, 0i32);
        // O clube já está ordenado pelo CA
        for l in self.squad(club) {
            n += (l.age >= SENIOR_AGE) as usize;
            count[l.role] += 1;
            seen[l.role] += 1;
            if seen[l.role] == STARTERS[l.role] {
                weakest[l.role] = l.ca;
            }
            if top < 11 {
                top += 1;
                sum += l.ca;
            }
        }
        (n, count, weakest, sum / top.max(1) as i32)
    }
}

/// Uma entrada ou saída na aba Transferências do clube.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TransferItem {
    pub date: String,
    pub person_id: i32,
    pub name: String,
    pub from: Option<ClubRef>,
    pub to: Option<ClubRef>,
    #[ts(type = "number")]
    pub fee: i64,
    /// "transfer", "free", "loan", "return" ou "release"
    pub kind: String,
}

pub(super) fn role_of(gk: i32, def: i32, mid: i32, att: i32) -> usize {
    if gk >= 15 {
        0
    } else if att > mid && att > def {
        3
    } else if def > mid {
        1
    } else {
        2
    }
}

/// Janela de transferências de um tipo de época num dia ("MM-DD").
pub(super) fn window_open(kind: SeasonKind, md: &str) -> bool {
    match kind {
        SeasonKind::European => ("07-01"..="08-31").contains(&md) || ("01-01"..="01-31").contains(&md),
        SeasonKind::Calendar => ("01-01"..="03-15").contains(&md) || ("07-01"..="08-15").contains(&md),
    }
}

/// O último dia de uma janela (dispensas).
fn window_closing(kind: SeasonKind, md: &str) -> bool {
    match kind {
        SeasonKind::European => md == "08-31" || md == "01-31",
        SeasonKind::Calendar => md == "03-15" || md == "08-15",
    }
}

/// Fim de um empréstimo: o fim da época do clube que recebe.
/// O próximo dia (a partir de hoje) com a janela de transferências aberta.
pub(super) fn next_window(kind: SeasonKind, today: &str) -> String {
    (0..400).filter_map(|d| date::add_days(today, d)).find(|day| window_open(kind, &day[5..])).unwrap_or_else(|| today.to_string())
}

pub(super) fn loan_until(kind: SeasonKind, today: &str) -> String {
    let year: i32 = today[0..4].parse().unwrap_or(0);
    match kind {
        SeasonKind::Calendar => format!("{year}-12-31"),
        SeasonKind::European => {
            if &today[5..] >= "07-01" {
                format!("{}-06-30", year + 1)
            } else {
                format!("{year}-06-30")
            }
        }
    }
}

/// Fim de um contrato de `years` épocas assinado hoje: o fim de uma época do clube.
pub(super) fn contract_until(kind: SeasonKind, today: &str, years: i32) -> String {
    let year: i32 = today[0..4].parse().unwrap_or(0);
    match kind {
        SeasonKind::Calendar => format!("{}-12-31", year + years - 1),
        SeasonKind::European => format!("{}-06-30", year + years - if &today[5..] >= "07-01" { 0 } else { 1 }),
    }
}

/// Anos de contrato que um clube da IA oferece pela idade.
pub(super) fn contract_years(age: i32, rng: &mut ChaCha8Rng) -> i32 {
    match age {
        ..=24 => 4 + (rng.next_u32() % 2) as i32,
        25..=28 => 3 + (rng.next_u32() % 2) as i32,
        _ => 1 + (rng.next_u32() % 2) as i32,
    }
}

/// Um clube que compra, como o mercado o vê hoje.
struct Buyer {
    rep: i32,
    kind: SeasonKind,
    budget: f64,
    wage_room: f64,
    squad: usize,
    count: [usize; ROLES],
    /// Titular mais fraco de cada papel e o nível do clube (média dos 11 melhores).
    weakest: [i32; ROLES],
    level: i32,
    continent: i32,
    nation: i32,
}

impl Game {
    /// Tipo de época (e de janela) de um clube: o da liga dele.
    pub(super) fn club_kind(&self, division: Option<i32>) -> SeasonKind {
        division.map_or(SeasonKind::Calendar, regulation::season_kind)
    }

    /// Índice de todos os jogadores com clube e dos livres com algum nível.
    fn market_index(&self) -> Result<Vec<Listing>> {
        let today = self.date.clone();
        let today_days = date::parse(&today).unwrap_or(0);
        type Row = (i32, Option<i32>, i32, i32, [i32; 4], Option<String>, Option<i32>, i32, i64, Option<String>, [i32; 2], [i32; 4]);
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare_cached(
                "SELECT pl.person_id, p.club_id, COALESCE(pl.current_ability, 1), COALESCE(pl.potential_ability, 0),
                        COALESCE(pl.pos_goalkeeper, 1),
                        MAX(COALESCE(pl.pos_sweeper, 1), COALESCE(pl.pos_defender, 1), COALESCE(pl.pos_wing_back, 1)),
                        MAX(COALESCE(pl.pos_defensive_midfielder, 1), COALESCE(pl.pos_midfielder, 1), COALESCE(pl.pos_attacking_midfielder, 1)),
                        COALESCE(pl.pos_attacker, 1),
                        p.birth_date, p.birth_year, COALESCE(pl.current_reputation, 0), COALESCE(p.wage, 0), p.contract_expires,
                        COALESCE(c.reputation, 0), COALESCE(k.reputation, 5), COALESCE(p.ambition, 10), COALESCE(p.loyalty, 10),
                        COALESCE(cn.continent_id, -1), COALESCE(p.nation_id, -1)
                 FROM players pl JOIN people p ON p.id = pl.person_id
                 LEFT JOIN clubs c ON c.id = p.club_id LEFT JOIN competitions k ON k.id = c.division_id
                 LEFT JOIN nations cn ON cn.id = c.nation_id
                 WHERE p.is_player = 1 AND (p.club_id IS NOT NULL OR pl.current_ability >= 50)
                   AND p.id NOT IN (SELECT person_id FROM retired) AND p.id NOT IN (SELECT person_id FROM ghosts)
                 ORDER BY pl.person_id",
            )?
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    [r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?],
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    [r.get(13)?, r.get(14)?],
                    [r.get(15)?, r.get(16)?, r.get(17)?, r.get(18)?],
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let since = date::add_days(&today, -SETTLE_DAYS).unwrap_or_default();
        let recent: HashSet<i32> = self
            .world
            .conn
            .prepare_cached("SELECT id FROM people WHERE is_player = 1 AND club_joined > ?")?
            .query_map([&since], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    club,
                    ca,
                    pa,
                    pos,
                    birth,
                    birth_year,
                    rep,
                    wage,
                    expires,
                    [club_rep, league_rep],
                    [ambition, loyalty, continent, nation],
                )| Listing {
                    id,
                    club,
                    ca,
                    pa,
                    role: role_of(pos[0], pos[1], pos[2], pos[3]),
                    age: age_on(birth.as_deref(), birth_year, &today),
                    rep,
                    wage,
                    years_left: expires.as_deref().and_then(date::parse).map_or(0.0, |e| (e - today_days) as f64 / 365.25),
                    club_rep,
                    league_rep,
                    recent: recent.contains(&id),
                    ambition: ambition as f64,
                    loyalty: loyalty as f64,
                    continent,
                    nation,
                },
            )
            .collect())
    }

    /// O mercado do dia: devoluções de emprestados todos os dias; às segundas e quintas, com a
    /// janela aberta, compras, empréstimos e (no último dia) dispensas. Devolve a notícia com
    /// as transferências do dia, se houver alguma que interesse ao treinador.
    pub(super) fn market_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let mut moved = self.return_loans()?;
        let md = self.date[5..].to_string();
        let weekday = date::weekday(&self.date);
        let trading = matches!(weekday, Some(1 | 4));
        let closing = [SeasonKind::European, SeasonKind::Calendar].iter().any(|&k| window_closing(k, &md));
        let any_open = [SeasonKind::European, SeasonKind::Calendar].iter().any(|&k| window_open(k, &md));
        if !any_open || !(trading || closing) {
            if moved {
                self.clean_selection()?;
                self.refresh_live()?;
            }
            return Ok(());
        }
        let mut market = Market::new(self.market_index()?);
        market.listed = self
            .world
            .conn
            .prepare_cached("SELECT person_id FROM transfer_list WHERE transfer = 1")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut deals: Vec<TransferItem> = Vec::new();
        let mut rng = self.season_rng(0x3A_4B37);
        // Os clubes geridos, pela ordem do id (determinismo), menos o do treinador
        let managed: Vec<(i32, i32, Option<i32>)> = self
            .world
            .conn
            .prepare_cached(&format!(
                "SELECT id, COALESCE(reputation, 0), division_id FROM clubs WHERE division_id IN ({}) ORDER BY id",
                self.managed_ids()
            ))?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let loaned: HashSet<i32> = self.loaned_players()?;
        let mut done: HashMap<i32, usize> = self.window_deals()?;
        for &(club, rep, division) in &managed {
            if club == self.club_id {
                continue;
            }
            let kind = self.club_kind(division);
            if !window_open(kind, &md) {
                continue;
            }
            if trading {
                self.ai_listings(club, &market, &loaned)?;
            }
            if trading && let Some(deal) = self.ai_forced_sale(club, rep, &market, &loaned, &managed, &mut done, &mut rng)? {
                market.move_player(deal.person_id, deal.to.as_ref().map(|c| c.id));
                deals.push(deal);
                moved = true;
            }
            if trading && uniform(&mut rng) < 0.5 {
                if let Some(deal) = self.ai_buy(club, rep, kind, &market, &loaned, &mut done, &mut rng)? {
                    market.move_player(deal.person_id, deal.to.as_ref().map(|c| c.id));
                    deals.push(deal);
                    moved = true;
                }
                if let Some(deal) = self.ai_loan_out(club, rep, &market, &loaned, &mut done, &mut rng)? {
                    market.move_player(deal.person_id, deal.to.as_ref().map(|c| c.id));
                    deals.push(deal);
                    moved = true;
                }
            }
            if window_closing(kind, &md) {
                for deal in self.release_excess(club, &market, &loaned)? {
                    market.move_player(deal.person_id, None);
                    deals.push(deal);
                    moved = true;
                }
            }
        }
        if moved {
            self.clean_selection()?;
            self.refresh_live()?;
            self.unsaved = true;
        }
        self.transfer_news(&deals, fresh, next_id)?;
        Ok(())
    }

    /// Listas de transferências e de empréstimos de um clube da IA, refeitas em cada dia de
    /// mercado: na de transferências os que estão fora dos 18 melhores (23+) e os veteranos
    /// fora do onze (32+), até 3, sem deixar o papel abaixo do mínimo; na de empréstimos os
    /// jovens (até 21) fora dos 18 melhores, até 2. Os listados a pedido ficam.
    fn ai_listings(&mut self, club: i32, market: &Market, loaned: &HashSet<i32>) -> Result<()> {
        self.world
            .conn
            .execute("DELETE FROM transfer_list WHERE requested = 0 AND person_id IN (SELECT id FROM people WHERE club_id = ?)", [club])?;
        let squad: Vec<&Listing> = market.squad(club).filter(|l| !loaned.contains(&l.id)).collect();
        let mut count = [0; ROLES];
        for l in &squad {
            count[l.role] += 1;
        }
        let mut sell: Vec<&Listing> = Vec::new();
        // Do pior para o melhor, os mais velhos primeiro
        let mut out: Vec<(usize, &Listing)> =
            squad.iter().copied().enumerate().filter(|(rank, l)| (*rank >= 18 && l.age >= 23) || (*rank >= 11 && l.age >= 32)).collect();
        out.sort_by_key(|(rank, l)| (std::cmp::Reverse(l.age), std::cmp::Reverse(*rank), l.id));
        for (_, l) in out {
            if sell.len() >= MAX_LISTED || count[l.role] <= MIN_ROLE[l.role] + 1 {
                continue;
            }
            count[l.role] -= 1;
            sell.push(l);
        }
        let lend: Vec<&Listing> = squad
            .iter()
            .copied()
            .enumerate()
            .filter(|(rank, l)| *rank >= 18 && l.age <= 21 && !l.recent)
            .map(|(_, l)| l)
            .take(MAX_LOAN_LISTED)
            .collect();
        let mut insert = self
            .world
            .conn
            .prepare_cached("INSERT OR IGNORE INTO transfer_list (person_id, transfer, loan, asking) VALUES (?, ?, ?, ?)")?;
        for l in sell {
            insert.execute(params![l.id, 1, 0, finance::round_money(l.value() as f64).max(10_000)])?;
        }
        for l in lend {
            insert.execute(params![l.id, 0, 1, Option::<i64>::None])?;
        }
        Ok(())
    }

    /// Negócios de cada clube na janela em curso (os últimos 70 dias), para o limite por janela.
    fn window_deals(&self) -> Result<HashMap<i32, usize>> {
        let since = date::add_days(&self.date, -70).unwrap_or_default();
        let mut out: HashMap<i32, usize> = HashMap::new();
        let rows: Vec<(Option<i32>, Option<i32>, String)> = self
            .world
            .conn
            .prepare_cached("SELECT from_club, to_club, kind FROM transfers WHERE date >= ?")?
            .query_map([since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (from, to, kind) in rows {
            if kind == "loan" {
                if let Some(f) = from {
                    *out.entry(-f).or_default() += 1; // empréstimos cedidos, com a chave negativa
                }
                continue;
            }
            if kind == "transfer" || kind == "free" {
                for c in [from, to].into_iter().flatten() {
                    *out.entry(c).or_default() += 1;
                }
            }
        }
        Ok(out)
    }

    fn loaned_players(&self) -> Result<HashSet<i32>> {
        Ok(self.world.conn.prepare_cached("SELECT person_id FROM loans")?.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?)
    }

    /// Como um clube está: saldo, folha, contagens e titulares mais fracos por papel.
    fn buyer(&self, club: i32, rep: i32, kind: SeasonKind, market: &Market) -> Result<Buyer> {
        let cash: i64 =
            self.world.conn.prepare_cached("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?")?.query_row([club], |r| r.get(0))?;
        let bill: i64 = market.squad(club).map(|l| l.wage).sum();
        let base = self.club_base(club)?;
        let (squad, count, weakest, level) = market.shape(club);
        let (continent, nation) = self.club_place(club)?;
        Ok(Buyer {
            rep,
            kind,
            budget: ((cash as f64 + base * BUDGET_REVENUE) * BUDGET_SHARE).max(0.0),
            wage_room: base * WAGE_CAP / 52.0 - bill as f64,
            squad,
            count,
            weakest,
            level,
            continent,
            nation,
        })
    }

    /// Continente e nação de um clube (−1 se não houver).
    pub(super) fn club_place(&self, club: i32) -> Result<(i32, i32)> {
        Ok(self.world.conn.prepare_cached(
            "SELECT COALESCE(n.continent_id, -1), COALESCE(c.nation_id, -1) FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id WHERE c.id = ?",
        )?
        .query_row([club], |r| Ok((r.get(0)?, r.get(1)?)))?)
    }

    /// Um clube da IA com dívidas grandes vende o jogador mais valioso a um clube que o possa
    /// pagar (reputação até 1000 abaixo, janela aberta), a 80% do valor.
    #[allow(clippy::too_many_arguments)]
    fn ai_forced_sale(
        &mut self,
        club: i32,
        rep: i32,
        market: &Market,
        loaned: &HashSet<i32>,
        managed: &[(i32, i32, Option<i32>)],
        done: &mut HashMap<i32, usize>,
        rng: &mut ChaCha8Rng,
    ) -> Result<Option<TransferItem>> {
        let cash: i64 =
            self.world.conn.prepare_cached("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?")?.query_row([club], |r| r.get(0))?;
        if cash as f64 >= -FORCED_SALE_DEBT * self.club_base(club)? || done.get(&club).copied().unwrap_or(0) >= MAX_DEALS {
            return Ok(None);
        }
        let Some(pick) =
            market.squad(club).filter(|l| !loaned.contains(&l.id) && !l.recent && l.age <= 31).max_by_key(|l| (l.value(), -l.id))
        else {
            return Ok(None);
        };
        let value = pick.value();
        if value <= 0 {
            return Ok(None);
        }
        let fee = finance::round_money(value as f64 * 0.8).max(10_000);
        let md = self.date[5..].to_string();
        let buyers: Vec<(i32, i32, SeasonKind)> = managed
            .iter()
            .filter(|&&(id, r, _)| id != club && id != self.club_id && r + 1000 >= rep)
            .map(|&(id, r, div)| (id, r, self.club_kind(div)))
            .filter(|&(_, _, k)| window_open(k, &md))
            .collect();
        if buyers.is_empty() {
            return Ok(None);
        }
        // Até 10 tentativas, a começar num clube ao acaso
        let start = (rng.next_u64() % buyers.len() as u64) as usize;
        for i in 0..buyers.len().min(10) {
            let (b, b_rep, kind) = buyers[(start + i) % buyers.len()];
            let buyer = self.buyer(b, b_rep, kind, market)?;
            if buyer.budget < fee as f64 || buyer.squad >= MAX_SQUAD {
                continue;
            }
            let wage = wage_demand(value.max(pick.ca as i64 * 20_000), pick.wage, b_rep);
            let years = contract_years(pick.age, rng);
            let id = pick.id;
            self.execute_move(id, Some(club), Some(b), fee, wage, years, "transfer", kind)?;
            for c in [club, b] {
                *done.entry(c).or_default() += 1;
            }
            return Ok(Some(self.transfer_item(id, Some(club), Some(b), fee, "transfer")?));
        }
        Ok(None)
    }

    /// Uma compra (ou um livre) de um clube da IA para a posição que mais precisa.
    #[allow(clippy::too_many_arguments)]
    fn ai_buy(
        &mut self,
        club: i32,
        rep: i32,
        kind: SeasonKind,
        market: &Market,
        loaned: &HashSet<i32>,
        done: &mut HashMap<i32, usize>,
        rng: &mut ChaCha8Rng,
    ) -> Result<Option<TransferItem>> {
        if done.get(&club).copied().unwrap_or(0) >= MAX_DEALS {
            return Ok(None);
        }
        let b = self.buyer(club, rep, kind, market)?;
        if b.squad >= MAX_SQUAD {
            return Ok(None);
        }
        // A posição: a que tem menos jogadores do que o mínimo ou, senão, o titular mais fraco
        let short = (0..ROLES).filter(|&r| b.count[r] < MIN_ROLE[r]).max_by_key(|&r| MIN_ROLE[r] - b.count[r]);
        let weak = (0..ROLES).filter(|&r| b.weakest[r] < b.level - 8).min_by_key(|&r| b.weakest[r]);
        let Some(role) = short.or(weak) else { return Ok(None) };
        let floor = if short == Some(role) { b.level - 20 } else { b.weakest[role] + 5 };
        let ceiling = b.level + 30;
        let mut candidates: Vec<(&Listing, i64, i64)> = Vec::new();
        for &i in &market.by_role[role] {
            let l = &market.listings[i];
            if l.ca > ceiling {
                continue;
            }
            if l.ca < floor {
                break;
            }
            if !(17..=31).contains(&l.age) {
                continue;
            }
            if l.club == Some(club)
                || l.club == Some(self.club_id)
                || loaned.contains(&l.id)
                || l.id == self.manager_id
                || (l.club.is_some() && l.recent)
            {
                continue;
            }
            // Vem de um clube muito maior? Não quer descer (o ambicioso ainda menos)
            if l.club.is_some() && l.club_rep > b.rep + personality::step_down(l.ambition) {
                continue;
            }
            // Da Europa para fora dela, quase só veteranos
            if l.club.is_some()
                && leaving_europe(l.continent, b.continent)
                && !leaves_europe(l.age, market.key.contains(&l.id), l.nation == b.nation)
            {
                continue;
            }
            // O clube que vende não fica abaixo do mínimo nem faz mais negócios do que o limite
            if let Some(seller) = l.club
                && (done.get(&seller).copied().unwrap_or(0) >= MAX_DEALS || market.role_count(seller, role) <= MIN_ROLE[role])
            {
                continue;
            }
            let value = if l.club.is_some() { l.value() } else { 0 };
            // Um titular do clube que vende (entre os 11 melhores) custa mais 40%; um leal até mais
            // 20%; um da lista de transferências sai por 90% do valor
            let key = if market.key.contains(&l.id) { 1.4 } else { 1.0 };
            let fee = match l.club {
                None => 0,
                Some(_) if market.listed.contains(&l.id) => (value as f64 * LISTED_FEE) as i64,
                Some(_) => (value as f64 * key * personality::loyalty_fee(l.loyalty)) as i64,
            };
            let wage = wage_demand(value.max(l.ca as i64 * 20_000), l.wage, b.rep);
            if fee as f64 > b.budget || wage as f64 > b.wage_room {
                continue;
            }
            candidates.push((l, fee, wage));
            // Bastam uns quantos dos melhores da faixa
            if candidates.len() >= 12 {
                break;
            }
        }
        if candidates.is_empty() {
            return Ok(None);
        }
        // Os 5 melhores por CA (os da lista de transferências com 5 de vantagem), um deles ao acaso
        let score = |l: &Listing| l.ca + if market.listed.contains(&l.id) { LISTED_EDGE } else { 0 };
        candidates.sort_by(|x, y| score(y.0).cmp(&score(x.0)).then(x.1.cmp(&y.1)).then(x.0.id.cmp(&y.0.id)));
        candidates.truncate(5);
        let (pick, fee, wage) = candidates[(rng.next_u64() % candidates.len() as u64) as usize];
        // Contrato: 2 a 5 anos, menos para os mais velhos
        let years = contract_years(pick.age, rng);
        let from = pick.club;
        // Quem sai de um clube custa pelo menos €10K (mesmo com o contrato no fim)
        let fee = if from.is_some() { finance::round_money(fee as f64).max(10_000) } else { 0 };
        self.execute_move(pick.id, from, Some(club), fee, wage, years, if from.is_some() { "transfer" } else { "free" }, b.kind)?;
        // Um livre recebe um prémio de assinatura (8 semanas de salário)
        if from.is_none() {
            self.book(club, "bonuses", wage * 8)?;
        }
        for c in [from, Some(club)].into_iter().flatten() {
            *done.entry(c).or_default() += 1;
        }
        Ok(Some(self.transfer_item(pick.id, from, Some(club), fee, if from.is_some() { "transfer" } else { "free" })?))
    }

    /// Empréstimo de um jovem (até 21 anos, fora dos 18 melhores) de um clube grande a um mais
    /// pequeno que precise da posição e em que ele jogue.
    fn ai_loan_out(
        &mut self,
        club: i32,
        rep: i32,
        market: &Market,
        loaned: &HashSet<i32>,
        done: &mut HashMap<i32, usize>,
        rng: &mut ChaCha8Rng,
    ) -> Result<Option<TransferItem>> {
        if rep < 6000 || done.get(&-club).copied().unwrap_or(0) >= MAX_LOANS_OUT {
            return Ok(None);
        }
        // Já ordenado pelo CA
        let mine: Vec<&Listing> = market.squad(club).collect();
        if mine.len() < 26 {
            return Ok(None);
        }
        let youth: Vec<&Listing> = mine.iter().skip(18).filter(|l| l.age <= 21 && l.ca >= 60 && !loaned.contains(&l.id)).copied().collect();
        if youth.is_empty() {
            return Ok(None);
        }
        let player = youth[(rng.next_u64() % youth.len() as u64) as usize];
        // Quem recebe: um clube gerido mais pequeno, com a janela aberta, plantel curto e onde ele jogue
        let md = self.date[5..].to_string();
        let borrowers: Vec<(i32, i32, Option<i32>)> = self
            .world
            .conn
            .prepare_cached(&format!(
                "SELECT id, COALESCE(reputation, 0), division_id FROM clubs WHERE division_id IN ({}) AND reputation < ? AND id <> ? ORDER BY id",
                self.managed_ids()
            ))?
            .query_map(params![rep - 1000, self.club_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut fits: Vec<(i32, SeasonKind)> = Vec::new();
        for (b, _, division) in borrowers {
            let kind = self.club_kind(division);
            if !window_open(kind, &md) {
                continue;
            }
            let (squad, count, _, level) = market.shape(b);
            if squad < 28 && player.ca + 10 >= level && count[player.role] < MIN_ROLE[player.role] + 3 {
                fits.push((b, kind));
            }
        }
        if fits.is_empty() {
            return Ok(None);
        }
        let (to, kind) = fits[(rng.next_u64() % fits.len() as u64) as usize];
        let until = loan_until(kind, &self.date);
        self.world
            .conn
            .execute("INSERT OR REPLACE INTO loans (person_id, parent_id, until) VALUES (?, ?, ?)", params![player.id, club, until])?;
        self.world.conn.execute("UPDATE people SET club_id = ?, club_joined = ? WHERE id = ?", params![to, self.date, player.id])?;
        self.number_after_move(player.id, Some(to))?;
        self.log_transfer(player.id, Some(club), Some(to), 0, "loan")?;
        *done.entry(-club).or_default() += 1;
        Ok(Some(self.transfer_item(player.id, Some(club), Some(to), 0, "loan")?))
    }

    /// Emprestados cujo empréstimo acabou voltam ao clube de origem.
    fn return_loans(&mut self) -> Result<bool> {
        let due: Vec<(i32, i32)> = self
            .world
            .conn
            .prepare_cached("SELECT person_id, parent_id FROM loans WHERE until < ? ORDER BY person_id")?
            .query_map([&self.date], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for &(id, parent) in &due {
            let from: Option<i32> = self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [id], |r| r.get(0))?;
            self.world.conn.execute("UPDATE people SET club_id = ?, club_joined = ? WHERE id = ?", params![parent, self.date, id])?;
            self.number_after_move(id, Some(parent))?;
            self.world.conn.execute("DELETE FROM loans WHERE person_id = ?", [id])?;
            self.log_transfer(id, from, Some(parent), 0, "return")?;
        }
        Ok(!due.is_empty())
    }

    /// No último dia da janela, um plantel com mais de [`RELEASE_ABOVE`] jogadores de 21 anos ou
    /// mais dispensa os piores deles, até [`MAX_RELEASES`] (os emprestados por outros clubes ficam).
    fn release_excess(&mut self, club: i32, market: &Market, loaned: &HashSet<i32>) -> Result<Vec<TransferItem>> {
        let seniors: Vec<&Listing> = market.squad(club).filter(|l| l.age >= SENIOR_AGE).collect();
        let excess = seniors.len().saturating_sub(RELEASE_ABOVE).min(MAX_RELEASES);
        // Do pior para o melhor (o clube já está ordenado pelo CA, do melhor para o pior)
        let out_ids: Vec<i32> = seniors.iter().rev().filter(|l| !loaned.contains(&l.id)).take(excess).map(|l| l.id).collect();
        let mut out = Vec::new();
        for id in out_ids {
            self.world.conn.execute("UPDATE people SET club_id = NULL, club_job = NULL, contract_expires = NULL WHERE id = ?", [id])?;
            self.number_after_move(id, None)?;
            self.log_transfer(id, Some(club), None, 0, "release")?;
            out.push(self.transfer_item(id, Some(club), None, 0, "release")?);
        }
        Ok(out)
    }

    /// Muda um jogador de clube: contrato novo, salário, dinheiro e o registo.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn execute_move(
        &mut self,
        id: i32,
        from: Option<i32>,
        to: Option<i32>,
        fee: i64,
        wage: i64,
        years: i32,
        kind: &str,
        season_kind: SeasonKind,
    ) -> Result<()> {
        let until = contract_until(season_kind, &self.date, years);
        self.clauses_on_move(id, from, fee)?;
        self.world.conn.execute(
            "UPDATE people SET club_id = ?, club_job = ?, club_joined = ?, contract_expires = ?, wage = ? WHERE id = ?",
            params![to, super::regen::PLAYER_JOB, self.date, until, wage, id],
        )?;
        self.world.conn.execute("DELETE FROM transfer_list WHERE person_id = ?", [id])?;
        self.number_after_move(id, to)?;
        if fee > 0 {
            if let Some(t) = to {
                self.book(t, "purchases", fee)?;
            }
            if let Some(f) = from {
                self.book(f, "sales", fee)?;
            }
        }
        self.log_transfer(id, from, to, fee, kind)
    }

    pub(super) fn log_transfer(&self, id: i32, from: Option<i32>, to: Option<i32>, fee: i64, kind: &str) -> Result<()> {
        self.world.conn.execute(
            "INSERT INTO transfers (date, person_id, from_club, to_club, fee, kind) VALUES (?, ?, ?, ?, ?, ?)",
            params![self.date, id, from, to, fee, kind],
        )?;
        Ok(())
    }

    fn transfer_item(&self, id: i32, from: Option<i32>, to: Option<i32>, fee: i64, kind: &str) -> Result<TransferItem> {
        Ok(TransferItem {
            date: self.date.clone(),
            person_id: id,
            name: self.person_name(id)?,
            from: from.map(|c| self.club(c)),
            to: to.map(|c| self.club(c)),
            fee,
            kind: kind.into(),
        })
    }

    /// Uma notícia com as transferências do dia que interessam ao treinador: as do seu clube,
    /// dos rivais e dos clubes da liga do mesmo tamanho ou maiores, as de jogadores por quem
    /// fez propostas e as maiores do mundo (até 3, de €20M ou mais). O resto fica só nas abas
    /// Transferências dos clubes.
    fn transfer_news(&self, deals: &[TransferItem], fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let (division, my_rep, rivals): (Option<i32>, i32, [Option<i32>; 3]) = self.world.conn.query_row(
            "SELECT division_id, COALESCE(reputation, 0), rival1_id, rival2_id, rival3_id FROM clubs WHERE id = ?",
            [self.club_id],
            |r| Ok((r.get(0)?, r.get(1)?, [r.get(2)?, r.get(3)?, r.get(4)?])),
        )?;
        let mut ours: HashSet<i32> = match division {
            Some(d) => self
                .world
                .conn
                .prepare_cached("SELECT id FROM clubs WHERE division_id = ? AND COALESCE(reputation, 0) >= ?")?
                .query_map(params![d, my_rep - CONTENDER_REP], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?,
            None => HashSet::new(),
        };
        ours.extend(rivals.into_iter().flatten());
        ours.insert(self.club_id);
        let wanted: HashSet<i32> = self
            .world
            .conn
            .prepare_cached("SELECT DISTINCT person_id FROM offers WHERE buyer = ? AND kind <> 'request'")?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut biggest: Vec<&TransferItem> = deals.iter().filter(|d| d.fee >= WORLD_NEWS_FEE).collect();
        biggest.sort_by_key(|d| (std::cmp::Reverse(d.fee), d.person_id));
        let biggest: HashSet<i32> = biggest.iter().take(3).map(|d| d.person_id).collect();
        let in_ours = |c: &Option<ClubRef>| c.as_ref().is_some_and(|c| ours.contains(&c.id));
        let mut lines: Vec<String> = Vec::new();
        for d in deals {
            let local = (in_ours(&d.from) || in_ours(&d.to)) && d.kind != "release" && d.kind != "return";
            if !(local || biggest.contains(&d.person_id) || wanted.contains(&d.person_id)) {
                continue;
            }
            let from = d.from.as_ref().map_or("sem clube".to_string(), |c| c.short_name.clone());
            let to = d.to.as_ref().map_or("sem clube".to_string(), |c| c.short_name.clone());
            lines.push(match d.kind.as_str() {
                "loan" => format!("{} ({from}) emprestado ao {to}", d.name),
                "free" => format!("{} assina pelo {to} a custo zero", d.name),
                _ => format!("{}: {from} → {to} por {}", d.name, money_text(d.fee)),
            });
        }
        if lines.is_empty() {
            return Ok(());
        }
        let body = format!("Transferências de {}:\n\n{}", cm_date(&self.date), lines.join("\n"));
        fresh.push(self.news_item(*next_id, "transfer", "Transferências do dia".into(), body));
        *next_id += 1;
        Ok(())
    }

    /// Entradas e saídas de um clube nos últimos 12 meses, da mais recente para a mais antiga.
    pub fn club_transfers(&self, club: i32) -> Result<Vec<TransferItem>> {
        let since = date::add_days(&self.date, -365).unwrap_or_default();
        let rows: Vec<(String, i32, Option<i32>, Option<i32>, i64, String)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT date, person_id, from_club, to_club, fee, kind FROM transfers
                 WHERE (from_club = ?1 OR to_club = ?1) AND date >= ?2 ORDER BY date DESC, id DESC",
            )?
            .query_map(params![club, since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        rows.into_iter()
            .map(|(date, id, from, to, fee, kind)| {
                Ok(TransferItem {
                    date,
                    person_id: id,
                    name: self.person_name(id)?,
                    from: from.map(|c| self.club(c)),
                    to: to.map(|c| self.club(c)),
                    fee,
                    kind,
                })
            })
            .collect()
    }

    /// Totais do mercado entre duas datas (CLI): transferências, livres, empréstimos, dispensas e dinheiro.
    pub fn market_totals(&self, from: &str, to: &str) -> Result<(i64, i64, i64, i64, i64)> {
        Ok(self.world.conn.query_row(
            "SELECT COALESCE(SUM(kind = 'transfer'), 0), COALESCE(SUM(kind = 'free'), 0), COALESCE(SUM(kind = 'loan'), 0),
                    COALESCE(SUM(kind = 'release'), 0), COALESCE(SUM(fee), 0)
             FROM transfers WHERE date >= ? AND date < ?",
            params![from, to],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?)
    }
}

/// "€12,5M", "€850K", "€1.900" (os salários pequenos por inteiro)
/// Clubes da liga do treinador que contam como concorrentes: reputação até isto abaixo da dele.
const CONTENDER_REP: i32 = 500;
/// Transferências do mundo que viram notícia (as 3 maiores do dia a partir disto).
const WORLD_NEWS_FEE: i64 = 20_000_000;

/// Continente da Europa em `continents`, e o da Ásia (a Arábia Saudita paga para os levar).
const EUROPE: i32 = 2;
const ASIA: i32 = 1;

/// Um jogador de um clube europeu a ir para um clube das Américas, de África ou da Oceânia.
pub(super) fn leaving_europe(from: i32, to: i32) -> bool {
    from == EUROPE && to != EUROPE && to != ASIA
}

/// Aceita sair da Europa (para o Brasil, por exemplo)? Os veteranos (31+), os que voltam ao
/// país deles (27+) e os que já não são titulares (29+); os outros recusam.
pub(super) fn leaves_europe(age: i32, starter: bool, home: bool) -> bool {
    age >= 31 || (home && age >= 27) || (!starter && age >= 29)
}

pub(super) fn money_text(v: i64) -> String {
    if v >= 1_000_000 {
        format!("€{:.1}M", v as f64 / 1e6).replace('.', ",")
    } else if v >= 10_000 {
        format!("€{}K", v / 1000)
    } else if v >= 1000 {
        format!("€{}.{:03}", v / 1000, v % 1000)
    } else {
        format!("€{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn europeans_rarely_leave_europe() {
        assert!(
            leaving_europe(EUROPE, 5) && !leaving_europe(EUROPE, EUROPE) && !leaving_europe(EUROPE, ASIA) && !leaving_europe(5, EUROPE)
        );
        assert!(!leaves_europe(24, false, false), "jovem fica");
        assert!(!leaves_europe(27, true, false), "titular no auge fica");
        assert!(leaves_europe(27, true, true), "volta a casa");
        assert!(leaves_europe(29, false, false) && leaves_europe(32, true, false));
    }

    #[test]
    fn windows_by_season_kind() {
        assert!(window_open(SeasonKind::European, "08-31") && !window_open(SeasonKind::European, "09-01"));
        assert!(window_open(SeasonKind::European, "01-15") && !window_open(SeasonKind::European, "02-01"));
        assert!(window_open(SeasonKind::Calendar, "03-15") && !window_open(SeasonKind::Calendar, "03-16"));
        assert!(window_open(SeasonKind::Calendar, "08-15") && !window_open(SeasonKind::Calendar, "08-16"));
        assert_eq!(loan_until(SeasonKind::European, "2025-08-10"), "2026-06-30");
        assert_eq!(loan_until(SeasonKind::European, "2026-01-10"), "2026-06-30");
        assert_eq!(loan_until(SeasonKind::Calendar, "2026-02-10"), "2026-12-31");
        assert_eq!(money_text(12_500_000), "€12,5M");
        assert_eq!((money_text(850_000), money_text(1_900), money_text(500)), ("€850K".into(), "€1.900".into(), "€500".into()));
    }

    fn summer_game(dir: &std::path::Path, until: &str) -> Option<Game> {
        let path = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))?;
        let _ = std::fs::remove_dir_all(dir);
        let new = crate::NewGame {
            save_name: "Mercado".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
        };
        let mut g = Game::create(&path, dir, &new, 11).unwrap();
        while g.date() < until {
            g.advance_day().unwrap();
        }
        Some(g)
    }

    fn all_transfers(g: &Game) -> Vec<(String, i32, Option<i32>, Option<i32>, i64, String)> {
        g.world
            .conn
            .prepare("SELECT date, person_id, from_club, to_club, fee, kind FROM transfers ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn summer_window_moves_players_and_is_deterministic() {
        let base = std::env::temp_dir().join(format!("nextcm-mercado-{}", std::process::id()));
        let Some(mut g) = summer_game(&base.join("a"), "2025-07-15") else { return };
        let other = summer_game(&base.join("b"), "2025-07-15").unwrap();
        let first = all_transfers(&g);
        assert!(!first.is_empty(), "negócios nas duas primeiras semanas");
        assert_eq!(first, all_transfers(&other), "mesma seed, mesmo mercado");
        drop(other);

        // Até ao fim da janela europeia: compras, livres e empréstimos; o treinador não é tocado
        let squad_before: i64 =
            g.world.conn.query_row("SELECT COUNT(*) FROM people WHERE club_id = 7568 AND is_player = 1", [], |r| r.get(0)).unwrap();
        // Um empréstimo que acaba já, para ver a devolução
        g.world.conn.execute("UPDATE loans SET until = '2025-07-20' WHERE person_id = (SELECT MIN(person_id) FROM loans)", []).unwrap();
        let back: Option<(i32, i32)> = g
            .world
            .conn
            .query_row("SELECT person_id, parent_id FROM loans WHERE until = '2025-07-20'", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .ok();
        while g.date() < "2025-09-02" {
            g.advance_day().unwrap();
        }
        let (sold, free, loans, _, fees) = g.market_totals("2025-07-01", "2025-09-02").unwrap();
        assert!(sold >= 50 && loans >= 10 && free >= 10 && fees > 0, "{sold} {free} {loans} {fees}");
        // Os clubes da IA põem jogadores nas listas de transferências e de empréstimos
        let (listed, lent): (i64, i64) = g
            .world
            .conn
            .query_row("SELECT SUM(transfer), SUM(loan) FROM transfer_list WHERE requested = 0", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert!(listed >= 50 && lent >= 20, "{listed} {lent}");
        for (_, id, from, to, _, kind) in all_transfers(&g) {
            assert!(from != Some(7568) && to != Some(7568) || kind == "return", "o clube do treinador fica de fora");
            let club: Option<i32> = g.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [id], |r| r.get(0)).unwrap();
            // O último negócio de cada jogador é onde ele está
            let last: (Option<i32>,) = g
                .world
                .conn
                .query_row("SELECT to_club FROM transfers WHERE person_id = ? ORDER BY id DESC LIMIT 1", [id], |r| Ok((r.get(0)?,)))
                .unwrap();
            assert_eq!(club, last.0, "jogador {id}");
        }
        // Quem acabou de chegar não volta a ser vendido na mesma janela
        let flips: i64 = g
            .world
            .conn
            .query_row(
                "SELECT COUNT(*) FROM (SELECT person_id FROM transfers WHERE kind IN ('transfer', 'free') GROUP BY person_id HAVING COUNT(*) > 1)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(flips, 0);
        let squad_after: i64 =
            g.world.conn.query_row("SELECT COUNT(*) FROM people WHERE club_id = 7568 AND is_player = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(squad_before, squad_after);
        // Voltou ao clube dono (que pode voltar a emprestá-lo na mesma janela)
        if let Some((id, parent)) = back {
            let returned: i64 = g
                .world
                .conn
                .query_row("SELECT COUNT(*) FROM transfers WHERE person_id = ? AND to_club = ? AND kind = 'return'", [id, parent], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(returned, 1, "voltou ao clube dono");
        }
        // A aba Transferências de quem comprou mostra a entrada
        let (_, id, _, to, _, _) = first[0].clone();
        assert!(g.club_transfers(to.unwrap()).unwrap().iter().any(|t| t.person_id == id));
        drop(g);
        let _ = std::fs::remove_dir_all(&base);
    }
}
