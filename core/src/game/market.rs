//! Mercado da IA (marco 3.8, passo 2): transferências e empréstimos entre clubes nas janelas,
//! jogadores livres, devoluções de emprestados e dispensas no fim da janela. Corre às segundas
//! e quintas com a janela aberta; nesses dias lê-se uma vez um índice de todos os jogadores e
//! cada clube gerido (menos o do treinador, que decide ele no passo 3) olha para o plantel:
//! posições com poucos jogadores ou com titulares fracos para o nível do clube. Regras e
//! números em docs/roadmap/3.08-mercado.md.

use std::collections::{HashMap, HashSet};

use rand_chacha::ChaCha8Rng;
use rand_core::RngCore;
use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::{Game, NewsItem, age_on, cm_date};
use crate::engine::uniform;
use crate::finance::{self, market_value, wage_demand};
use crate::person::ClubRef;
use crate::personality;
use crate::regulation::{self, SeasonKind};
use crate::willingness;
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
const MAX_DEALS: usize = 4;
/// Empréstimos de jovens por clube numa janela.
const MAX_LOANS_OUT: usize = 2;
/// O clube que recebe um emprestado: até isto de reputação abaixo do que empresta.
const LOAN_REP_GAP: i32 = 4000;
/// Parte do saldo que um clube gasta numa janela, e o teto da folha sobre as receitas.
const BUDGET_SHARE: f64 = 0.5;
/// Dívida (saldo negativo sobre as receitas de uma época) a partir da qual um clube da IA vende
/// o jogador mais valioso num dia de mercado, a 80% do valor; e a partir da qual vai à falência
/// no fim da época (passo 4).
const FORCED_SALE_DEBT: f64 = 0.25;
pub(super) const BANKRUPT_DEBT: f64 = 0.6;
/// Parte das receitas de uma época que entra no orçamento mesmo sem saldo.
const BUDGET_REVENUE: f64 = 0.2;
/// Parte do saldo que um clube rico aceita gastar por época em salários, além do teto (sem isto
/// o saldo crescia sem parar). Era 40% com o dinheiro do CM; com os saldos de hoje e o fair-play
/// (3.22), 10%: mais fazia a folha dos clubes médios subir até à proibição de contratar.
const CASH_WAGES: f64 = 0.1;
/// A contratação bombástica, como no CM: um clube com o saldo acima de `SPLURGE_CASH` épocas de
/// receitas, de vez em quando (`SPLURGE_CHANCE` por tentativa de compra), vai ao melhor que
/// encontrar acima do nível da equipa (até `SPLURGE_CEILING` de CA), precise ou não da posição,
/// e paga 1,5 a 2,5× o valor com até `SPLURGE_BUDGET` do saldo e mais 20% de salário.
const SPLURGE_CASH: f64 = 2.0;
const SPLURGE_CHANCE: f64 = 0.04;
const SPLURGE_BUDGET: f64 = 0.8;
const SPLURGE_CEILING: i32 = 50;
/// Um titular este CA abaixo do nível da equipa (a média dos 11 melhores) é para trocar.
const WEAK_STARTER: i32 = 6;
/// Listas da IA: quantos na de transferências e na de empréstimos, o preço dos listados (parte
/// do valor) e a vantagem (em CA) que têm na escolha de quem compra.
const MAX_LISTED: usize = 3;
/// A vontade de vender a partir da qual a IA põe um jogador na lista (`Game::sell_score`).
const SELL_AT: i32 = 40;

/// O que um clube da IA sabe do plantel para decidir quem vende.
struct SellContext {
    /// O treinador gosta (true) ou detesta (false) destes jogadores.
    coach_likes: HashMap<i32, bool>,
    /// Os jogadores que detestam o treinador e os que detestam um colega.
    hates_coach: HashSet<i32>,
    feuds: HashSet<i32>,
    temper: HashMap<i32, i32>,
    /// 0 com o saldo positivo, 1 no vermelho, 2 em dívida grande.
    debt: i32,
    /// O peso do clube (a reputação e as continentais).
    pull: i32,
}
const MAX_LOAN_LISTED: usize = 2;
/// Nos primeiros dias da janela ([`early_window`]) um negócio a partir disto só passa com
/// [`EARLY_BIG_CHANCE`]: os grandes ficam para mais tarde, como no CM.
const EARLY_BIG_FEE: i64 = 20_000_000;
const EARLY_BIG_CHANCE: f64 = 0.2;
/// Um negócio da IA a partir disto faz-se em duas etapas: o acordo ("firma acordo com") e,
/// [`AGREEMENT_DAYS`] dias depois, a assinatura. Entretanto o treinador pode entrar no negócio.
const STAGED_FEE: i64 = 15_000_000;
const AGREEMENT_DAYS: (i32, i32) = (3, 10);

/// Os acordos da IA à espera da assinatura (save 57).
pub(super) const AGREEMENTS_SCHEMA: &str = "
CREATE TABLE ai_agreements (person_id INTEGER PRIMARY KEY, from_club INTEGER NOT NULL, to_club INTEGER NOT NULL,
                            fee INTEGER NOT NULL, wage INTEGER NOT NULL, years INTEGER NOT NULL, complete_on TEXT NOT NULL);
";
const LISTED_EDGE: i32 = 5;
/// As estrelas: quem vale esta parte do valor médio dos 11 melhores, ou mais, nunca vai para as
/// listas nem sai por empréstimo, mesmo fora dos planos.
pub(super) const STAR_SHARE: f64 = 0.75;

/// Quem está fora dos planos de um clube, como no CM: depois dos `MIN_ROLE` melhores da sua
/// posição (2 guarda-redes, 6 defesas, 6 médios, 3 avançados), pelo CA. É quem o clube vende
/// (os mais velhos) ou empresta (os jovens), valha o que valer, menos as estrelas.
fn out_of_plans(squad: &[&Listing]) -> HashSet<i32> {
    let mut seen = [0usize; ROLES];
    let mut out = HashSet::new();
    // A lista vem ordenada pelo CA
    for l in squad {
        if seen[l.role] >= MIN_ROLE[l.role] {
            out.insert(l.id);
        }
        seen[l.role] += 1;
    }
    out
}

/// O valor médio dos 11 melhores (pelo CA) de uma lista ordenada.
fn core_value<'a>(squad: impl Iterator<Item = &'a Listing>) -> f64 {
    let top: Vec<i64> = squad.take(11).map(|l| l.value()).collect();
    if top.is_empty() { 0.0 } else { top.iter().sum::<i64>() as f64 / top.len() as f64 }
}

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
    /// Pediram para sair (listados a pedido): o clube quer resolver (`willingness`).
    requested: HashSet<i32>,
    /// Os rivais de cada clube (`clubs.rival1..3`), nos dois sentidos.
    rivals: HashMap<i32, Vec<i32>>,
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
        let mut m = Market {
            listings,
            by_id,
            by_club: HashMap::new(),
            by_role,
            key: HashSet::new(),
            listed: HashSet::new(),
            requested: HashSet::new(),
            rivals: HashMap::new(),
        };
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

    /// `a` e `b` são rivais (num sentido ou no outro).
    fn is_rival(&self, a: i32, b: i32) -> bool {
        self.rivals.get(&a).is_some_and(|r| r.contains(&b)) || self.rivals.get(&b).is_some_and(|r| r.contains(&a))
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
    /// "transfer", "free", "loan", "return", "release" ou "agreement" (acordo da IA à espera da assinatura)
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
        SeasonKind::European | SeasonKind::International => ("07-01"..="08-31").contains(&md) || ("01-01"..="01-31").contains(&md),
        SeasonKind::Calendar => ("01-01"..="03-15").contains(&md) || ("07-01"..="08-15").contains(&md),
    }
}

/// Os primeiros 10 dias de uma janela (todas abrem a 1/1 ou a 1/7).
fn early_window(md: &str) -> bool {
    ("07-01"..="07-10").contains(&md) || ("01-01"..="01-10").contains(&md)
}

/// O último dia de uma janela (dispensas).
fn window_closing(kind: SeasonKind, md: &str) -> bool {
    match kind {
        SeasonKind::European | SeasonKind::International => md == "08-31" || md == "01-31",
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
        SeasonKind::European | SeasonKind::International => {
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
        SeasonKind::European | SeasonKind::International => format!("{}-06-30", year + years - if &today[5..] >= "07-01" { 0 } else { 1 }),
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
    cash: i64,
    base: f64,
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
        // Os acordos da IA que chegam hoje à assinatura (todos os dias, não só nos de mercado)
        let signed = self.complete_agreements()?;
        moved |= !signed.is_empty();
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
            return self.transfer_news(&signed, fresh, next_id);
        }
        // Quem tem um acordo à espera da assinatura já não está no mercado da IA
        let pending: HashSet<i32> = self
            .world
            .conn
            .prepare_cached("SELECT person_id FROM ai_agreements")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut market = Market::new(self.market_index()?.into_iter().filter(|l| !pending.contains(&l.id)).collect());
        market.listed = self
            .world
            .conn
            .prepare_cached("SELECT person_id FROM transfer_list WHERE transfer = 1")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        market.requested = self
            .world
            .conn
            .prepare_cached("SELECT person_id FROM transfer_list WHERE requested = 1")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        market.rivals = self.rival_map()?;
        let mut deals: Vec<TransferItem> = signed;
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
        let bonus = self.continental_bonuses();
        let mut done: HashMap<i32, usize> = self.window_deals()?;
        for &(club, rep, division) in &managed {
            if club == self.club_id {
                continue;
            }
            // As listas refazem-se mesmo com a janela do clube fechada: os outros continuam a
            // comprar-lhe jogadores, e uma lista velha deixava titulares listados
            if trading {
                self.ai_listings(club, &market, &loaned, &bonus)?;
            }
            let kind = self.club_kind(division);
            if !window_open(kind, &md) {
                continue;
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
    /// mercado: na de transferências os que estão fora dos planos (24+) e os veteranos fora do
    /// onze (32+), até 3, sem deixar o papel abaixo do mínimo; na de empréstimos os jovens (até
    /// 23) fora dos planos, até 2. As estrelas nunca; os listados a pedido ficam.
    /// As listas de um clube da IA, como no CM: vai para a venda quem o clube quer vender por
    /// vários motivos somados ([`Game::sell_score`]: o lugar no plantel, a idade, o contrato, o
    /// dinheiro, o treinador, as brigas e o pedido para sair), até [`MAX_LISTED`] e sem deixar
    /// uma posição abaixo do mínimo. Quem pede para sair fica marcado (`requested`). Os jovens fora
    /// dos planos vão para a lista de empréstimos.
    fn ai_listings(&mut self, club: i32, market: &Market, loaned: &HashSet<i32>, bonus: &HashMap<i32, i32>) -> Result<()> {
        self.world.conn.execute("DELETE FROM transfer_list WHERE person_id IN (SELECT id FROM people WHERE club_id = ?)", [club])?;
        // Os planos contam com todos (os emprestados ao clube também ocupam um lugar); as listas não
        let all: Vec<&Listing> = market.squad(club).collect();
        let spare = out_of_plans(&all);
        let core = core_value(all.iter().copied());
        let ctx = self.sell_context(club, bonus)?;
        let star = |l: &Listing| l.value() as f64 >= STAR_SHARE * core;
        let mut count = [0; ROLES];
        for l in all.iter().filter(|l| !loaned.contains(&l.id)) {
            count[l.role] += 1;
        }
        let mut scored: Vec<(i32, bool, &Listing)> = Vec::new();
        for (rank, l) in all.iter().copied().enumerate() {
            if loaned.contains(&l.id) || l.recent {
                continue;
            }
            let (score, wants_out) = self.sell_score(l, rank, spare.contains(&l.id), star(l), &ctx);
            if score >= SELL_AT {
                scored.push((score, wants_out, l));
            }
        }
        scored.sort_by_key(|(score, _, l)| (std::cmp::Reverse(*score), l.id));
        let mut sell: Vec<(bool, &Listing)> = Vec::new();
        for (_, wants_out, l) in scored {
            if sell.len() >= MAX_LISTED || count[l.role] <= MIN_ROLE[l.role] + 1 {
                continue;
            }
            count[l.role] -= 1;
            sell.push((wants_out, l));
        }
        let listed: HashSet<i32> = sell.iter().map(|(_, l)| l.id).collect();
        let lend: Vec<&Listing> = all
            .iter()
            .copied()
            .filter(|l| !loaned.contains(&l.id) && !listed.contains(&l.id))
            .filter(|l| spare.contains(&l.id) && l.age <= 23 && !l.recent && !star(l))
            .take(MAX_LOAN_LISTED)
            .collect();
        let mut insert = self
            .world
            .conn
            .prepare_cached("INSERT OR IGNORE INTO transfer_list (person_id, transfer, loan, asking, requested) VALUES (?, ?, ?, ?, ?)")?;
        for (wants_out, l) in sell {
            insert.execute(params![l.id, 1, 0, finance::round_money(l.value() as f64).max(10_000), wants_out])?;
        }
        for l in lend {
            insert.execute(params![l.id, 0, 1, Option::<i64>::None, false])?;
        }
        Ok(())
    }

    /// O que um clube da IA sabe do plantel para decidir quem vende.
    fn sell_context(&self, club: i32, bonus: &HashMap<i32, i32>) -> Result<SellContext> {
        let conn = &self.world.conn;
        let coach: Option<i32> =
            conn.query_row("SELECT id FROM people WHERE club_id = ? AND club_job = 5 LIMIT 1", [club], |r| r.get(0)).optional()?;
        let mut coach_likes = HashMap::new();
        let mut hates_coach = HashSet::new();
        if let Some(c) = coach {
            let rows: Vec<(i32, bool)> = conn
                .prepare_cached("SELECT target_id, liked FROM preferences WHERE person_id = ? AND kind = 'person'")?
                .query_map([c], |r| Ok((r.get(0)?, r.get::<_, i32>(1)? != 0)))?
                .collect::<rusqlite::Result<_>>()?;
            coach_likes.extend(rows);
            let rows: Vec<i32> = conn
                .prepare_cached("SELECT person_id FROM preferences WHERE kind = 'person' AND liked = 0 AND target_id = ?")?
                .query_map([c], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            hates_coach.extend(rows);
        }
        // Quem detesta um colega do plantel (as Preferências)
        let feuds: HashSet<i32> = conn
            .prepare_cached(
                "SELECT f.person_id FROM preferences f JOIN people a ON a.id = f.person_id JOIN people b ON b.id = f.target_id
                 WHERE f.kind = 'person' AND f.liked = 0 AND a.club_id = ?1 AND b.club_id = ?1 AND b.is_player = 1",
            )?
            .query_map([club], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let temper: HashMap<i32, i32> = conn
            .prepare_cached("SELECT id, COALESCE(temperament, 10) FROM people WHERE club_id = ? AND is_player = 1")?
            .query_map([club], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let base = self.club_base(club)?;
        let cash = self.cash_of(club)? as f64;
        let debt = if cash < -0.1 * base {
            2
        } else if cash < 0.0 {
            1
        } else {
            0
        };
        let pull = self.club_rep(club)? + bonus.get(&club).copied().unwrap_or(0);
        Ok(SellContext { coach_likes, hates_coach, feuds, temper, debt, pull })
    }

    /// A vontade de vender um jogador (pontos; vende a partir de [`SELL_AT`]) e se ele pediu
    /// para sair. Os pesos estão em docs/roadmap/3.19.1-pendencias.md (9/11).
    fn sell_score(&self, l: &Listing, rank: usize, spare: bool, star: bool, ctx: &SellContext) -> (i32, bool) {
        let mut score = 0;
        // O lugar no plantel
        if spare {
            score += 40;
        }
        if rank < 11 {
            score -= 50;
        } else if rank < 18 {
            score -= 15;
        }
        // A idade: os veteranos fora do onze saem; os jovens com margem ficam
        if l.age >= 32 && rank >= 11 {
            score += 30;
        } else if l.age >= 30 && spare {
            score += 10;
        }
        if l.age <= 21 && l.pa - l.ca >= 15 {
            score -= 25;
        }
        // O contrato: no último ano, vende-se antes de o perder de graça
        if l.years_left <= 1.0 && (24..=31).contains(&l.age) {
            score += 20;
        }
        // O dinheiro: um clube no vermelho vende os que valem alguma coisa
        if ctx.debt > 0 && l.value() >= 1_000_000 {
            score += 15 * ctx.debt;
        }
        // O treinador
        match ctx.coach_likes.get(&l.id) {
            Some(false) => score += 45,
            Some(true) => score -= 40,
            None => {}
        }
        // As brigas: com o treinador, com um colega, o mau feitio
        if ctx.hates_coach.contains(&l.id) {
            score += 25;
        }
        if ctx.feuds.contains(&l.id) {
            score += 15;
        }
        if ctx.temper.get(&l.id).copied().unwrap_or(10) <= 4 {
            score += 10;
        }
        // O pedido para sair, como a insatisfação do plantel do treinador: o ambicioso que se acha
        // grande demais para o clube (pela reputação dele), ou quem quer jogar e não joga
        let too_big = l.ambition >= 14.0 && l.rep * 50 > ctx.pull + personality::step_down(l.ambition);
        let wants_games = rank >= 16 && (23..=30).contains(&l.age) && l.ambition >= 13.0 && l.ca >= 100;
        let wants_out = too_big || wants_games;
        if wants_out {
            score += 35;
        }
        // As estrelas ficam, a não ser que queiram sair ou o treinador não as queira
        if star && !wants_out && ctx.coach_likes.get(&l.id) != Some(&false) {
            score -= 60;
        }
        // Um toque de acaso pela seed, igual na janela toda
        let window = self.date[0..4].parse::<i32>().unwrap_or(0) * 2 + i32::from(&self.date[5..7] >= "07");
        let mut rng = self.deal_rng(l.id, 0x5E11_0000 ^ window as u64);
        score += (rng.next_u32() % 21) as i32 - 10;
        (score, wants_out)
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
            cash,
            base,
            budget: self.ai_budget(club, cash)?,
            wage_room: (base * finance::WAGE_SHARE + cash.max(0) as f64 * CASH_WAGES) / 52.0 - bill as f64,
            squad,
            count,
            weakest,
            level,
            continent,
            nation,
        })
    }

    /// O que um clube da IA pode gastar numa janela: metade do saldo mais 20% das receitas, sem
    /// passar do que o fair-play ainda deixa perder (as vendas abrem espaço); nada se estiver
    /// proibido de contratar.
    pub(super) fn ai_budget(&self, club: i32, cash: i64) -> Result<f64> {
        if self.fair_play_banned(club)? {
            return Ok(0.0);
        }
        let base = self.club_base(club)?;
        let budget = ((cash as f64 + base * BUDGET_REVENUE) * BUDGET_SHARE).max(0.0);
        Ok(budget.min(self.fair_play_room(club)?))
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
            // Mesmo a precisar de dinheiro, ao rival vende pouco, e o jogador vai menos
            if market.is_rival(club, b) {
                let chance = willingness::sale_terms(&willingness::Sale { rival: true, ..Default::default() }).1
                    * willingness::rival_move_chance(pick.loyalty, false);
                if uniform(rng) >= chance {
                    continue;
                }
            }
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

    /// O dia da assinatura de um acordo: 3 a 10 dias depois, ainda com a janela do comprador
    /// aberta; `None` se a janela fecha já (o negócio faz-se de uma vez).
    fn agreement_day(&self, kind: SeasonKind, rng: &mut ChaCha8Rng) -> Option<String> {
        let (lo, hi) = AGREEMENT_DAYS;
        let days = lo + (rng.next_u32() % (hi - lo + 1) as u32) as i32;
        (1..=days).rev().filter_map(|d| date::add_days(&self.date, d)).find(|day| window_open(kind, &day[5..]))
    }

    /// Assina os acordos da IA de hoje: o jogador muda-se se ainda está no clube que o vendeu
    /// (o treinador pode tê-lo comprado entretanto) e não se reformou.
    fn complete_agreements(&mut self) -> Result<Vec<TransferItem>> {
        type Row = (i32, i32, i32, i64, i64, i32);
        let due: Vec<Row> = self
            .world
            .conn
            .prepare_cached(
                "SELECT person_id, from_club, to_club, fee, wage, years FROM ai_agreements WHERE complete_on <= ? ORDER BY person_id",
            )?
            .query_map([&self.date], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for (person, from, to, fee, wage, years) in due {
            self.world.conn.execute("DELETE FROM ai_agreements WHERE person_id = ?", [person])?;
            let at: Option<Option<i32>> = self
                .world
                .conn
                .query_row("SELECT club_id FROM people WHERE id = ? AND id NOT IN (SELECT person_id FROM retired)", [person], |r| r.get(0))
                .optional()?;
            if at != Some(Some(from)) || to == self.club_id {
                continue;
            }
            let kind = self.kind_of(to)?;
            self.execute_move(person, Some(from), Some(to), fee, wage, years, "transfer", kind)?;
            out.push(self.transfer_item(person, Some(from), Some(to), fee, "transfer")?);
        }
        Ok(out)
    }

    /// O acordo da IA à espera da assinatura por um jogador: o comprador e o valor.
    pub(super) fn ai_agreement(&self, person: i32) -> Result<Option<(i32, i64)>> {
        Ok(self
            .world
            .conn
            .query_row("SELECT to_club, fee FROM ai_agreements WHERE person_id = ?", [person], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?)
    }

    /// Os rivais de cada clube (`clubs.rival1..3`).
    pub(super) fn rival_map(&self) -> Result<HashMap<i32, Vec<i32>>> {
        let rows: Vec<(i32, [Option<i32>; 3])> = self
            .world
            .conn
            .prepare_cached(
                "SELECT id, rival1_id, rival2_id, rival3_id FROM clubs
                 WHERE rival1_id IS NOT NULL OR rival2_id IS NOT NULL OR rival3_id IS NOT NULL",
            )?
            .query_map([], |r| Ok((r.get(0)?, [r.get(1)?, r.get(2)?, r.get(3)?])))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows.into_iter().map(|(id, r)| (id, r.into_iter().flatten().filter(|&c| c > 0).collect())).collect())
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
        // As Preferências do CM: quem detesta o clube não vem; quem o tem por favorito vem mesmo
        // descendo ou saindo da Europa
        let fans = self.world.club_fans(club)?;
        // O peso dos clubes para os jogadores: a reputação e as continentais desta época
        let bonus = self.continental_bonuses();
        let pull = |c: Option<i32>| c.and_then(|c| bonus.get(&c).copied()).unwrap_or(0);
        let buyer_pull = b.rep + pull(Some(club));
        // A contratação bombástica: um clube muito rico, de vez em quando
        let splurge = b.cash as f64 >= SPLURGE_CASH * b.base && uniform(rng) < SPLURGE_CHANCE;
        let (premium, budget, wage_room) = if splurge {
            (1.5 + uniform(rng), b.cash as f64 * SPLURGE_BUDGET, b.wage_room.max(0.0) + b.cash as f64 * CASH_WAGES / 52.0)
        } else {
            (1.0, b.budget, b.wage_room)
        };
        // A posição: a que tem menos jogadores do que o mínimo ou, senão, o titular mais fraco
        // (na bombástica, o titular mais fraco mesmo sem precisar)
        let short = (0..ROLES).filter(|&r| b.count[r] < MIN_ROLE[r]).max_by_key(|&r| MIN_ROLE[r] - b.count[r]);
        let weak = (0..ROLES).filter(|&r| splurge || b.weakest[r] < b.level - WEAK_STARTER).min_by_key(|&r| b.weakest[r]);
        let Some(role) = (if splurge { weak } else { short.or(weak) }) else { return Ok(None) };
        let floor = if splurge {
            b.weakest[role].max(b.level) + 5
        } else if short == Some(role) {
            b.level - 20
        } else {
            b.weakest[role] + 5
        };
        let ceiling = b.level + if splurge { SPLURGE_CEILING } else { 30 };
        let mut candidates: Vec<(&Listing, i64, i64)> = Vec::new();
        // O valor do plantel de cada clube que vende (para o indispensável) e o dia da janela
        let mut cores: HashMap<i32, f64> = HashMap::new();
        let md = self.date[5..].to_string();
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
                // Entre clubes da IA, quem acabou de chegar não volta a ser vendido na mesma janela
                // (o treinador ainda o pode tentar, caro e raro: `willingness`)
                || (l.club.is_some() && l.recent)
            {
                continue;
            }
            let fan = fans.get(&l.id).copied();
            if fan == Some(false) {
                continue;
            }
            // Vem de um clube muito maior? Não quer descer (o ambicioso ainda menos)
            if l.club.is_some() && fan.is_none() && personality::steps_down(l.club_rep + pull(l.club), buyer_pull, l.ambition) {
                continue;
            }
            // Da Europa para fora dela, quase só veteranos
            if l.club.is_some()
                && fan.is_none()
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
            // As vontades do clube que vende e do jogador (`willingness`, as mesmas das propostas
            // do treinador): o listado sai por 90% e quase sempre; o titular custa mais 40%; o
            // indispensável, o acabado de chegar e a venda ao rival muito mais e raramente; um
            // leal até mais 20%
            let (price, mut chance) = match l.club {
                None => (0.0, 1.0),
                Some(seller) => {
                    let core = *cores.entry(seller).or_insert_with(|| core_value(market.squad(seller)));
                    let sale = willingness::Sale {
                        listed: market.listed.contains(&l.id),
                        unhappy: market.requested.contains(&l.id),
                        starter: market.key.contains(&l.id),
                        star: l.value() as f64 >= STAR_SHARE * core,
                        recent: false,
                        rival: market.is_rival(club, seller),
                    };
                    let (price, chance) = willingness::sale_terms(&sale);
                    let loyal = if sale.listed { 1.0 } else { personality::loyalty_fee(l.loyalty) };
                    (price * loyal, chance)
                }
            };
            // Na bombástica o prémio já paga a resistência do clube: conta o maior dos dois (o
            // produto dava negócios de €480M)
            let fee = (value as f64 * if splurge { price.max(premium) } else { price }) as i64;
            let wage = wage_demand(value.max(l.ca as i64 * 20_000), l.wage, b.rep);
            let wage = if splurge { (wage as f64 * 1.2) as i64 } else { wage };
            // Para o rival do seu clube vai pouco (o leal quase nunca) e pede mais salário
            let to_rival = l.club.is_some_and(|c| market.is_rival(club, c));
            let wage = if to_rival {
                chance *= willingness::rival_move_chance(l.loyalty, market.requested.contains(&l.id));
                (wage as f64 * willingness::RIVAL_WAGE) as i64
            } else {
                wage
            };
            if fee as f64 > budget || wage as f64 > wage_room {
                continue;
            }
            // Na primeira semana e pouco da janela os negócios grandes são raros
            if fee >= EARLY_BIG_FEE && early_window(&md) {
                chance *= EARLY_BIG_CHANCE;
            }
            if chance < 1.0 && uniform(rng) >= chance {
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
        // A bombástica vai a um dos 3 melhores (sempre o melhor dava o mesmo negócio em todos os saves)
        let at = (rng.next_u64() % candidates.len().min(if splurge { 3 } else { 5 }) as u64) as usize;
        let (pick, fee, wage) = candidates[at];
        // Contrato: 2 a 5 anos, menos para os mais velhos
        let years = contract_years(pick.age, rng);
        let from = pick.club;
        // Quem sai de um clube custa pelo menos €10K (mesmo com o contrato no fim)
        let fee = if from.is_some() { finance::round_money(fee as f64).max(10_000) } else { 0 };
        // Um negócio grande fica acordado e só se assina uns dias depois (dentro da janela)
        if let Some(seller) = from
            && fee >= STAGED_FEE
            && let Some(on) = self.agreement_day(b.kind, rng)
        {
            self.world.conn.execute(
                "INSERT OR REPLACE INTO ai_agreements (person_id, from_club, to_club, fee, wage, years, complete_on) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![pick.id, seller, club, fee, wage, years, on],
            )?;
            for c in [seller, club] {
                *done.entry(c).or_default() += 1;
            }
            return Ok(Some(self.transfer_item(pick.id, from, Some(club), fee, "agreement")?));
        }
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
    /// pequeno que precise da posição e em que ele jogue: do mesmo país e, se nenhum servir, do
    /// mesmo continente (nunca o Man Utd ao Ituano), e não muito abaixo em reputação.
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
        let core = core_value(mine.iter().copied());
        let spare = out_of_plans(&mine);
        let youth: Vec<&Listing> = mine
            .iter()
            .filter(|l| {
                spare.contains(&l.id) && l.age <= 21 && l.ca >= 60 && !loaned.contains(&l.id) && (l.value() as f64) < STAR_SHARE * core
            })
            .copied()
            .collect();
        if youth.is_empty() {
            return Ok(None);
        }
        let player = youth[(rng.next_u64() % youth.len() as u64) as usize];
        // Quem recebe: um clube gerido mais pequeno, com a janela aberta, plantel curto e onde ele jogue
        let md = self.date[5..].to_string();
        let (nation, continent): (Option<i32>, Option<i32>) = self.world.conn.query_row(
            "SELECT c.nation_id, n.continent_id FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id WHERE c.id = ?",
            [club],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let borrowers: Vec<(i32, Option<i32>, Option<i32>, Option<i32>)> = self
            .world
            .conn
            .prepare_cached(&format!(
                "SELECT c.id, c.division_id, c.nation_id, n.continent_id FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id
                 WHERE c.division_id IN ({}) AND c.reputation < ? AND c.reputation >= ? AND c.id <> ? ORDER BY c.id",
                self.managed_ids()
            ))?
            .query_map(params![rep - 1000, rep - LOAN_REP_GAP, self.club_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        // Os que servem, primeiro no mesmo país (0) e depois no mesmo continente (1)
        let mut fits: Vec<(usize, i32, SeasonKind)> = Vec::new();
        for (b, division, b_nation, b_continent) in borrowers {
            let tier = if b_nation.is_some() && b_nation == nation {
                0
            } else if b_continent.is_some() && b_continent == continent {
                1
            } else {
                continue;
            };
            let kind = self.club_kind(division);
            if !window_open(kind, &md) {
                continue;
            }
            let (squad, count, _, level) = market.shape(b);
            if squad < 28 && player.ca + 10 >= level && count[player.role] < MIN_ROLE[player.role] + 3 {
                fits.push((tier, b, kind));
            }
        }
        let Some(best) = fits.iter().map(|f| f.0).min() else { return Ok(None) };
        fits.retain(|f| f.0 == best);
        let (_, to, kind) = fits[(rng.next_u64() % fits.len() as u64) as usize];
        let until = loan_until(kind, &self.date);
        self.world
            .conn
            .execute("INSERT OR REPLACE INTO loans (person_id, parent_id, until) VALUES (?, ?, ?)", params![player.id, club, until])?;
        // Emprestado, sai das listas do clube dono (senão aparecia listado no clube que o recebe)
        self.world.conn.execute("DELETE FROM transfer_list WHERE person_id = ?", [player.id])?;
        self.world.conn.execute("UPDATE people SET club_id = ?, club_joined = ? WHERE id = ?", params![to, self.date, player.id])?;
        self.number_after_move(player.id, Some(to))?;
        self.log_transfer(player.id, Some(club), Some(to), 0, "loan")?;
        *done.entry(-club).or_default() += 1;
        Ok(Some(self.transfer_item(player.id, Some(club), Some(to), 0, "loan")?))
    }

    /// Emprestados cujo empréstimo acabou voltam ao clube de origem.
    /// Chamar de volta um jogador emprestado pelo clube do treinador: volta já ao plantel.
    pub fn recall_loan(&mut self, person: i32) -> Result<String> {
        if self.sacked() {
            return Err(crate::Error::Invalid("Não tem clube.".into()));
        }
        let loan: Option<i32> =
            self.world.conn.query_row("SELECT parent_id FROM loans WHERE person_id = ?", [person], |r| r.get(0)).optional()?;
        let from: Option<i32> = self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [person], |r| r.get(0))?;
        if loan != Some(self.club_id) || from == Some(self.club_id) {
            return Err(crate::Error::Invalid("Só para os jogadores emprestados pelo seu clube.".into()));
        }
        let open: i64 = self.world.conn.query_row(
            "SELECT COUNT(*) FROM offers WHERE person_id = ? AND status IN ('pending', 'countered', 'agreed', 'raised', 'waiting')",
            [person],
            |r| r.get(0),
        )?;
        if open > 0 {
            return Err(crate::Error::Invalid("Há um negócio em curso por este jogador.".into()));
        }
        self.world.conn.execute("UPDATE people SET club_id = ?, club_joined = ? WHERE id = ?", params![self.club_id, self.date, person])?;
        self.number_after_move(person, Some(self.club_id))?;
        self.world.conn.execute("DELETE FROM loans WHERE person_id = ?", [person])?;
        self.log_transfer(person, from, Some(self.club_id), 0, "return")?;
        self.refresh_live()?;
        self.unsaved = true;
        let name = self.person_name(person)?;
        let club = from.map(|c| self.club(c).short_name).unwrap_or_default();
        Ok(format!("{name} foi chamado de volta do empréstimo ao {club}."))
    }

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
            // As propostas do treinador, a Lista Preferencial e as listas dos olheiros
            .prepare_cached(
                "SELECT person_id FROM offers WHERE buyer = ?1 AND kind <> 'request'
                 UNION SELECT person_id FROM shortlist UNION SELECT person_id FROM scout_finds WHERE stars >= 4",
            )?
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
                "agreement" => format!("{}: {to} firma acordo com o {from} por {}", d.name, money_text(d.fee)),
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
pub(super) const EUROPE: i32 = 2;
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
    fn out_of_plans_after_the_best_of_each_position() {
        // 3 guarda-redes e 8 médios, pelo CA: o 3.º guarda-redes e o 7.º e 8.º médios estão fora
        let mk = |id: i32, role: usize, ca: i32| Listing {
            id,
            club: Some(1),
            ca,
            pa: ca,
            role,
            age: 22,
            rep: 0,
            wage: 0,
            years_left: 2.0,
            club_rep: 0,
            league_rep: 10,
            ambition: 10.0,
            loyalty: 10.0,
            continent: 0,
            nation: 0,
            recent: false,
        };
        let mut v: Vec<Listing> = (0..3).map(|i| mk(i, 0, 150 - i)).collect();
        v.extend((0..8).map(|i| mk(10 + i, 2, 160 - i)));
        v.sort_by_key(|l| std::cmp::Reverse(l.ca));
        let refs: Vec<&Listing> = v.iter().collect();
        let out = out_of_plans(&refs);
        let mut ids: Vec<i32> = out.into_iter().collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![2, 16, 17]);
    }

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
            license: 0,
            real_players: true,
            masking: true,
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
        // Só quem está fora dos planos (depois dos melhores da posição) e nenhuma estrela. As
        // listas são do último dia de mercado (no fecho os outros ainda compram), então
        // refazem-se com o plantel de agora antes de as conferir
        let index = g.market_index().unwrap();
        let fresh_market = Market::new(index);
        let loaned = g.loaned_players().unwrap();
        let clubs: Vec<i32> = g
            .world
            .conn
            .prepare("SELECT DISTINCT p.club_id FROM transfer_list l JOIN people p ON p.id = l.person_id WHERE l.requested = 0 AND p.club_id <> 7568")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let bonus = g.continental_bonuses();
        for &c in &clubs {
            g.ai_listings(c, &fresh_market, &loaned, &bonus).unwrap();
        }
        // Cada um tem motivos para sair (a vontade de vender) e só poucos são do onze
        let (mut listed_now, mut starters) = (0, 0);
        for &c in &clubs {
            let all: Vec<&Listing> = fresh_market.squad(c).collect();
            let spare = out_of_plans(&all);
            let core = core_value(all.iter().copied());
            let ctx = g.sell_context(c, &bonus).unwrap();
            for (rank, l) in all.iter().copied().enumerate() {
                let on: bool = g
                    .world
                    .conn
                    .query_row("SELECT COUNT(*) > 0 FROM transfer_list WHERE person_id = ? AND transfer = 1", [l.id], |r| r.get(0))
                    .unwrap();
                if !on {
                    continue;
                }
                let star = l.value() as f64 >= STAR_SHARE * core;
                let (score, _) = g.sell_score(l, rank, spare.contains(&l.id), star, &ctx);
                assert!(score >= SELL_AT, "jogador {} do clube {c}: {score}", l.id);
                listed_now += 1;
                starters += usize::from(rank < 11);
            }
        }
        assert!(listed_now > 0 && starters * 5 <= listed_now, "{starters} do onze em {listed_now}");
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
        // Os empréstimos da IA ficam no continente de quem empresta (nunca o Man Utd ao Ituano)
        let crossed: i64 = g
            .world
            .conn
            .query_row(
                "SELECT COUNT(*) FROM transfers t JOIN clubs a ON a.id = t.from_club JOIN clubs b ON b.id = t.to_club
                 JOIN nations na ON na.id = a.nation_id JOIN nations nb ON nb.id = b.nation_id
                 WHERE t.kind = 'loan' AND t.date > '2025-07-01' AND na.continent_id <> nb.continent_id",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(crossed, 0, "empréstimos para outro continente");
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
