//! Um jogo em curso (Fase 3): o save é uma cópia do `world.sqlite` com as tabelas
//! `game`, `news`, `fixtures`, `player_state`, `selection` e `selection_players`. O estado
//! que muda dia a dia fica em memória e só vai para o arquivo em [`Game::save`].
//!
//! Num dia com jogo do treinador (e com `watch` ligado, como no app), [`Game::advance_day`]
//! pára antes de o jogar: os outros jogos do dia já estão decididos, e o do treinador corre
//! aos bocados pela tela da partida (`live_*`) até [`Game::live_finish`] fechar o dia.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rand_chacha::ChaCha8Rng;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::engine;

mod board;
mod competitions;
mod deals;
mod finances;
mod market;
mod numbers;
mod regen;
mod scout_tasks;
mod scouting;
mod season;
mod unhappy;
use crate::league::{self, Fixture, Match, StandingRow};
use crate::live::{LiveMatch, LiveView, Occasion, TeamStats};
use crate::person::ClubRef;
use crate::regulation::{self, SeasonKind};
use crate::search::fold;
use crate::squad::{Event, PlayerState, PlayerStatus, Squads};
use crate::tactics::{ClubTactics, Instructions, Mentality, Passing, Selection, Tackling};
use crate::{Error, Result, World, date};
pub use board::BoardView;
pub use competitions::{GroupTable, KoRoundView, StageView, TieView};
pub use deals::{ContractReply, ContractTerms, OfferAction, OfferView, RoleOption, TransferView};
pub use market::TransferItem;
pub use numbers::{NumberAssign, NumberEntry, SquadNumbers};
pub use scout_tasks::{ScoutFind, ScoutList};
pub use season::DivisionStats;
pub(crate) use season::age_on;

/// Versão do formato do save; saves de outra versão são recusados até haver migração.
/// 2: tabela `fixtures` (marco 3.2). 3: estado dos jogadores e a escolha da equipa (3.3).
/// 4: posições livres no campo (`selection_players.x/y`) e titulares na ordem do CM.
/// 5: setas de movimento (`selection_players.run_x/run_y`).
/// 6: jogos na época (`player_state.apps`) e reformados (`retired`), marco 3.5.
/// 7: fases, grupos, eliminatórias e penáltis em `fixtures`; cartões por competição (3.6).
/// 8: livro de contas dos clubes (`finances`), marco 3.8.
/// 9: transferências (`transfers`) e empréstimos (`loans`), marco 3.8.
/// 10: propostas (`offers`), lista de transferências, negociações, funções no clube e
/// `news.offer_id` (3.8).
/// 11: direção do clube do treinador (`board`), marco 3.8.
/// 12: número preferido de cada jogador (`players.preferred_number`) e `news.link`.
/// 13: insatisfação (`unhappy`), jogos seguidos sem jogar (`player_state.missed`) e
/// listados a pedido (`transfer_list.requested`).
/// 14: conhecimento dos jogadores observados (`knowledge`) e olheiros em missão (`scouting`).
/// 15: tarefas dos olheiros (`scout_tasks`) e os jogadores que encontraram (`scout_finds`).
pub const SAVE_VERSION: i32 = 15;

/// Tarefas dos olheiros (save 15).
const SCOUT_TASKS_SCHEMA: &str = "
CREATE TABLE scout_tasks (slot INTEGER PRIMARY KEY, scout_id INTEGER NOT NULL, kind TEXT NOT NULL, target INTEGER, started TEXT NOT NULL);
CREATE TABLE scout_finds (slot INTEGER NOT NULL, person_id INTEGER NOT NULL, found TEXT NOT NULL, stars INTEGER NOT NULL,
                          PRIMARY KEY (slot, person_id));
";

/// Olheiros (save 14).
const SCOUTING_SCHEMA: &str = "
CREATE TABLE knowledge (person_id INTEGER PRIMARY KEY, until TEXT NOT NULL);
CREATE TABLE scouting (person_id INTEGER PRIMARY KEY, ready_on TEXT NOT NULL, scout_id INTEGER);
";

/// Insatisfação dos jogadores e pedidos para sair (save 13).
const UNHAPPY_SCHEMA: &str = "
ALTER TABLE player_state ADD COLUMN missed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE transfer_list ADD COLUMN requested INTEGER NOT NULL DEFAULT 0;
CREATE TABLE unhappy (person_id INTEGER NOT NULL, reason TEXT NOT NULL, since TEXT NOT NULL, PRIMARY KEY (person_id, reason));
";

/// Números escolhidos pelo treinador (save 12).
const NUMBERS_SCHEMA: &str = "
ALTER TABLE players ADD COLUMN preferred_number INTEGER;
ALTER TABLE news ADD COLUMN link TEXT;
";

/// A direção do clube do treinador (save 11): uma linha só.
const BOARD_SCHEMA: &str = "
CREATE TABLE board (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    goal TEXT NOT NULL,
    target INTEGER NOT NULL,
    confidence INTEGER NOT NULL,
    transfer_budget INTEGER NOT NULL,
    wage_budget INTEGER NOT NULL,
    max_wage INTEGER NOT NULL,
    warning INTEGER NOT NULL,
    sacked INTEGER NOT NULL
);
";

/// Tabelas do mercado do treinador (save 10).
const DEALS_SCHEMA: &str = "
-- kind: transfer, loan. status: pending, countered, agreed, raised, done, rejected, withdrawn, collapsed
CREATE TABLE offers (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    person_id INTEGER NOT NULL,
    buyer INTEGER NOT NULL,
    seller INTEGER,
    fee INTEGER NOT NULL,
    asked INTEGER,
    status TEXT NOT NULL,
    answer_on TEXT,
    created TEXT NOT NULL
);
CREATE TABLE transfer_list (person_id INTEGER PRIMARY KEY, transfer INTEGER NOT NULL, loan INTEGER NOT NULL, asking INTEGER);
CREATE TABLE talks (person_id INTEGER PRIMARY KEY, tries INTEGER NOT NULL, blocked_until TEXT);
-- Função no clube combinada no contrato (`deals::SQUAD_ROLES`)
CREATE TABLE squad_roles (person_id INTEGER PRIMARY KEY, role TEXT NOT NULL);
";

/// Quantos dias "Continuar Jogo" avança no máximo quando não acontece nada.
const MAX_IDLE_DAYS: i32 = 42;

/// Dia em que o jogo começa (o CM 01/02 começa a 1 de julho; patch 25-26).
const START_MONTH_DAY: &str = "07-01";

/// Cargo "Treinador" na tabela `jobs`.
const HEAD_COACH: i32 = 5;

const SCHEMA: &str = "
CREATE TABLE game (
    version INTEGER NOT NULL,
    name TEXT NOT NULL,
    date TEXT NOT NULL,
    seed INTEGER NOT NULL,
    manager_id INTEGER NOT NULL,
    club_id INTEGER NOT NULL,
    created TEXT NOT NULL
);
CREATE TABLE news (
    id INTEGER PRIMARY KEY,
    date TEXT NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    read INTEGER NOT NULL DEFAULT 0,
    offer_id INTEGER
);
CREATE TABLE fixtures (
    id INTEGER PRIMARY KEY,
    competition_id INTEGER NOT NULL,
    round INTEGER NOT NULL,
    date TEXT NOT NULL,
    home_id INTEGER NOT NULL,
    away_id INTEGER NOT NULL,
    home_goals INTEGER,
    away_goals INTEGER,
    stage INTEGER NOT NULL DEFAULT 0,
    grp INTEGER,
    tie INTEGER,
    leg INTEGER NOT NULL DEFAULT 0,
    neutral INTEGER NOT NULL DEFAULT 0,
    home_pens INTEGER,
    away_pens INTEGER
);
CREATE INDEX fixtures_date ON fixtures (date);
CREATE TABLE player_state (
    person_id INTEGER PRIMARY KEY,
    condition INTEGER NOT NULL,
    morale INTEGER NOT NULL,
    yellows INTEGER NOT NULL,
    suspended INTEGER NOT NULL,
    injury TEXT,
    injury_until TEXT,
    apps INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE cards (
    person_id INTEGER NOT NULL,
    competition_id INTEGER NOT NULL,
    yellows INTEGER NOT NULL,
    suspended INTEGER NOT NULL,
    PRIMARY KEY (person_id, competition_id)
);
CREATE TABLE finances (
    club_id INTEGER NOT NULL,
    season INTEGER NOT NULL,
    category TEXT NOT NULL,
    amount INTEGER NOT NULL,
    PRIMARY KEY (club_id, season, category)
);
CREATE TABLE club_economy (
    club_id INTEGER PRIMARY KEY,
    base REAL NOT NULL
);
-- kind: transfer, free, loan, return, release
CREATE TABLE transfers (
    id INTEGER PRIMARY KEY,
    date TEXT NOT NULL,
    person_id INTEGER NOT NULL,
    from_club INTEGER,
    to_club INTEGER,
    fee INTEGER NOT NULL,
    kind TEXT NOT NULL
);
CREATE INDEX transfers_date ON transfers (date);
CREATE TABLE loans (
    person_id INTEGER PRIMARY KEY,
    parent_id INTEGER NOT NULL,
    until TEXT NOT NULL
);
CREATE TABLE retired (
    person_id INTEGER PRIMARY KEY,
    date TEXT NOT NULL
);
CREATE TABLE selection (
    club_id INTEGER PRIMARY KEY,
    formation INTEGER NOT NULL,
    auto INTEGER NOT NULL,
    mentality TEXT NOT NULL,
    passing TEXT NOT NULL,
    tackling TEXT NOT NULL,
    pressing INTEGER NOT NULL,
    offside_trap INTEGER NOT NULL,
    counter_attack INTEGER NOT NULL,
    tight_marking INTEGER NOT NULL
);
-- slot 0–10: titulares (na ordem das funções, como na lista do CM); 100–108: suplentes.
-- x/y: posição livre no campo, quando o treinador a mexeu.
CREATE TABLE selection_players (
    club_id INTEGER NOT NULL,
    slot INTEGER NOT NULL,
    person_id INTEGER,
    x INTEGER,
    y INTEGER,
    run_x INTEGER,
    run_y INTEGER,
    PRIMARY KEY (club_id, slot)
);
";

/// Dados do ecrã "Novo Jogo".
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NewGame {
    /// Nome do save (vira o nome do arquivo).
    pub save_name: String,
    pub first_name: String,
    pub last_name: String,
    /// "AAAA-MM-DD"
    pub birth_date: Option<String>,
    pub nation_id: Option<i32>,
    pub club_id: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub id: i32,
    pub date: String,
    /// "message" (mensagens), "competition", "injury"
    pub kind: String,
    pub title: String,
    pub body: String,
    pub read: bool,
    /// Proposta ligada à mensagem (botões para responder), marco 3.8.
    pub offer_id: Option<i32>,
    /// Tela que a mensagem abre ("squadNumbers").
    pub link: Option<String>,
}

/// O que a coluna esquerda e o menu do treinador precisam.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub name: String,
    pub date: String,
    pub manager_id: i32,
    pub manager_name: String,
    pub club: ClubRef,
    pub unread_news: i32,
    /// Há mudanças desde a última gravação.
    pub unsaved: bool,
    /// O jogo do treinador de hoje está por acabar (tela da partida).
    pub live: bool,
    /// A direção despediu o treinador: o jogo acabou.
    pub sacked: bool,
    /// Há convocados sem número para o jogo de amanhã: o Continuar Jogo abre os números.
    pub needs_numbers: bool,
}

/// Uma linha da lista "Carregar Jogo".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SaveSummary {
    pub file: String,
    pub name: String,
    pub manager_name: String,
    pub club_name: String,
    pub date: String,
    /// Segundos desde 1970 da última gravação (data do arquivo).
    #[ts(type = "number")]
    pub saved_at: i64,
}

/// "Próximo Jogo" do CM: onde, quando, o último confronto, o tempo e as notícias de cada lado.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MatchPreview {
    pub fixture: Fixture,
    /// Estádio e cidade do clube da casa.
    pub venue: Option<String>,
    pub city: Option<String>,
    /// Último jogo entre os dois nesta save (o mais recente).
    pub last_meeting: Option<Fixture>,
    /// Ex.: "Seco, 24°C" (sorteado com a seed e a partida, igual sempre que se abre).
    pub weather: String,
    /// "Normais", "Molhado"...
    pub pitch: String,
    /// Lesionados e suspensos de cada lado.
    pub home_news: Vec<String>,
    pub away_news: Vec<String>,
}

pub struct Game {
    world: World,
    path: PathBuf,
    name: String,
    date: String,
    seed: i64,
    manager_id: i32,
    club_id: i32,
    news: Vec<NewsItem>,
    /// Calendário de todas as competições vivas (no 3.2, só a Série A).
    matches: Vec<Match>,
    /// Clubes das competições vivas, por id.
    clubs: HashMap<i32, ClubRef>,
    /// Nome curto das competições vivas ("Série A").
    competitions: HashMap<i32, String>,
    /// Condição, moral, lesões e cartões dos jogadores dos clubes vivos.
    squads: Squads,
    /// A equipa escolhida pelo treinador; `None` = o adjunto escolhe (instruções normais).
    selection: Option<Selection>,
    unsaved: bool,
    /// O treinador vê os jogos dele ao vivo (o app); sem isso (CLI, testes) jogam-se de seguida.
    watch: bool,
    /// O dia de jogo à espera de o treinador acabar a partida dele.
    live: Option<LiveDay>,
    /// Estatísticas dos jogos desde que o jogo foi aberto (para a CLI).
    records: Vec<MatchRecord>,
}

/// Os jogos de um dia: os outros já jogados até ao fim, o do treinador a decorrer.
struct LiveDay {
    /// (índice em `matches`, partida)
    games: Vec<(usize, LiveMatch)>,
    /// Posição do jogo do treinador em `games`.
    user: usize,
    /// Notícias do início do dia (recuperações, alterações do adjunto).
    news: Vec<NewsItem>,
    /// Estádio, tempo, árbitro e assistência do jogo do treinador.
    extras: Option<MatchExtras>,
}

#[derive(Debug, Clone)]
struct MatchExtras {
    venue: Option<String>,
    city: Option<String>,
    weather: String,
    referee: String,
    attendance: i32,
}

/// Números de um jogo acabado.
#[derive(Debug, Clone)]
pub struct MatchRecord {
    pub fixture_id: i32,
    pub stats: [TeamStats; 2],
    pub subs: [i32; 2],
    pub injuries: usize,
}

/// "Nome do save" → nome de arquivo seguro (letras, números, espaço, - e _).
fn file_stem(name: &str) -> Result<String> {
    let stem: String = name.trim().chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    if stem.trim().is_empty() {
        return Err(Error::Invalid("Escolha um nome para o jogo.".into()));
    }
    Ok(stem)
}

impl Game {
    /// Cria o save a partir de `world_path`, põe o treinador no clube e grava.
    pub fn create(world_path: &Path, saves_dir: &Path, new: &NewGame, seed: i64) -> Result<Game> {
        let first = new.first_name.trim();
        let last = new.last_name.trim();
        if first.is_empty() || last.is_empty() {
            return Err(Error::Invalid("Indique o nome e o apelido do treinador.".into()));
        }
        if let Some(b) = &new.birth_date
            && date::parse(b).is_none()
        {
            return Err(Error::Invalid("Data de nascimento inválida.".into()));
        }
        std::fs::create_dir_all(saves_dir)?;
        let path = saves_dir.join(format!("{}.sqlite", file_stem(&new.save_name)?));
        if path.exists() {
            return Err(Error::Invalid(format!("Já existe um jogo chamado \"{}\".", new.save_name.trim())));
        }
        std::fs::copy(world_path, &path)?;

        let result = (|| {
            let world = World::open_save(&path)?;
            let start = format!("{}-{START_MONTH_DAY}", world.start_year());
            let club = world.club_ref(Some(new.club_id))?.ok_or_else(|| Error::NotFound(format!("clube {}", new.club_id)))?;

            let tx = world.conn.unchecked_transaction()?;
            tx.execute_batch(SCHEMA)?;
            tx.execute_batch(DEALS_SCHEMA)?;
            tx.execute_batch(BOARD_SCHEMA)?;
            tx.execute_batch(NUMBERS_SCHEMA)?;
            tx.execute_batch(UNHAPPY_SCHEMA)?;
            tx.execute_batch(SCOUTING_SCHEMA)?;
            tx.execute_batch(SCOUT_TASKS_SCHEMA)?;
            let manager_id: i32 = tx.query_row("SELECT COALESCE(MAX(id), 0) + 1 FROM people", [], |r| r.get(0))?;
            // O treinador anterior fica sem clube, como no CM
            tx.execute(
                "UPDATE people SET club_id = NULL, club_job = NULL WHERE club_id = ? AND club_job = ?",
                params![new.club_id, HEAD_COACH],
            )?;
            let display = format!("{first} {last}");
            let birth_year = new.birth_date.as_deref().and_then(|b| b.get(0..4)?.parse::<i32>().ok());
            let contract = format!("{}-06-30", world.start_year() + 2);
            tx.execute(
                "INSERT INTO people (id, display_name, first_name, second_name, common_name, search_name, list_name,
                                     birth_date, birth_year, nation_id, international_apps, international_goals,
                                     club_id, club_job, club_joined, contract_expires,
                                     is_player, is_non_player, birth_sort, positions)
                 VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9, 0, 0, ?10, ?11, ?12, ?13, 0, 0, ?7, 0)",
                params![
                    manager_id,
                    display,
                    first,
                    last,
                    fold(&display),
                    format!("{last}, {first}"),
                    new.birth_date,
                    birth_year,
                    new.nation_id,
                    new.club_id,
                    HEAD_COACH,
                    start,
                    contract
                ],
            )?;
            tx.execute(
                "INSERT INTO game (version, name, date, seed, manager_id, club_id, created) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![SAVE_VERSION, new.save_name.trim(), start, seed, manager_id, new.club_id, start],
            )?;
            tx.commit()?;
            // Daqui em diante tudo fica numa transação aberta: Gravar Jogo faz o commit
            world.conn.execute_batch("BEGIN")?;

            let welcome = welcome_news(&world, &club, &display, &start)?;
            let year = world.start_year();
            let mut game = Game {
                world,
                path: path.clone(),
                name: new.save_name.trim().into(),
                date: start,
                seed,
                manager_id,
                club_id: new.club_id,
                news: vec![welcome],
                competitions: HashMap::new(),
                matches: Vec::new(),
                clubs: HashMap::new(),
                squads: Squads { states: HashMap::new(), traits: HashMap::new() },
                selection: None,
                unsaved: true,
                watch: false,
                live: None,
                records: Vec::new(),
            };
            // Calendários da época de transição e da época europeia de 2025/26, sorteados com a seed do jogo
            let (mut fresh, mut next_id) = (Vec::new(), 2);
            game.competitions = competition_names(&game.world, &[])?;
            // Como o CM ao começar: os atributos que o patch não define são gerados
            game.fill_missing_attributes()?;
            game.schedule_season(SeasonKind::Calendar, year, &mut fresh, &mut next_id)?;
            game.schedule_season(SeasonKind::European, year, &mut fresh, &mut next_id)?;
            game.complete_squads()?;
            numbers::fix_squad_numbers(&game.world, &[game.club_id], game.seed, None)?;
            game.refresh_live()?;
            for kind in [SeasonKind::Calendar, SeasonKind::European] {
                if let Some(n) = game.season_news(next_id, kind) {
                    fresh.push(n);
                    next_id += 1;
                }
            }
            for n in fresh {
                game.news.insert(0, n);
            }
            game.board_setup()?;
            game.save()?;
            Ok(game)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&path);
        }
        result
    }

    pub fn load(path: &Path) -> Result<Game> {
        let world = World::open_save(path)?;
        let row = world
            .conn
            .query_row("SELECT version, name, date, seed, manager_id, club_id FROM game", [], |r| {
                Ok((r.get::<_, i32>(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })
            .optional()
            .map_err(|_| Error::Invalid("Este arquivo não é um jogo do NextCM.".into()))?
            .ok_or_else(|| Error::Invalid("Este arquivo não é um jogo do NextCM.".into()))?;
        let (version, name, date, seed, manager_id, club_id): (i32, String, String, i64, i32, i32) = row;
        let mut version = version;
        if version == 3 {
            migrate_v3(&world, club_id)?;
            version = 4;
        }
        if version == 4 {
            world.conn.execute_batch(
                "ALTER TABLE selection_players ADD COLUMN run_x INTEGER;
                 ALTER TABLE selection_players ADD COLUMN run_y INTEGER;
                 UPDATE game SET version = 5;",
            )?;
            version = 5;
        }
        if version == 5 {
            world.conn.execute_batch(
                "ALTER TABLE player_state ADD COLUMN apps INTEGER NOT NULL DEFAULT 0;
                 CREATE TABLE retired (person_id INTEGER PRIMARY KEY, date TEXT NOT NULL);
                 UPDATE game SET version = 6;",
            )?;
            version = 6;
        }
        // 6 → 7: a época passou para o ano civil (3.6). Sem jogos disputados, os calendários
        // voltam a ser sorteados; com jogos, não há como os encaixar e o save é recusado.
        let mut redraw = false;
        if version == 6 {
            let played: i64 = world.conn.query_row("SELECT COUNT(*) FROM fixtures WHERE home_goals IS NOT NULL", [], |r| r.get(0))?;
            if played > 0 {
                return Err(Error::Invalid(
                    "Este jogo foi gravado com a época de julho a junho (antes das novas competições) e já tem jogos                      disputados. Comece um jogo novo."
                        .into(),
                ));
            }
            world.conn.execute_batch(
                "ALTER TABLE fixtures ADD COLUMN stage INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE fixtures ADD COLUMN grp INTEGER;
                 ALTER TABLE fixtures ADD COLUMN tie INTEGER;
                 ALTER TABLE fixtures ADD COLUMN leg INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE fixtures ADD COLUMN neutral INTEGER NOT NULL DEFAULT 0;
                 ALTER TABLE fixtures ADD COLUMN home_pens INTEGER;
                 ALTER TABLE fixtures ADD COLUMN away_pens INTEGER;
                 DELETE FROM fixtures;
                 CREATE TABLE cards (person_id INTEGER NOT NULL, competition_id INTEGER NOT NULL, yellows INTEGER NOT NULL,
                                     suspended INTEGER NOT NULL, PRIMARY KEY (person_id, competition_id));
                 INSERT INTO cards SELECT person_id, 65, yellows, suspended FROM player_state WHERE yellows > 0 OR suspended > 0;
                 UPDATE game SET version = 7;",
            )?;
            version = 7;
            redraw = true;
        }
        if version == 7 {
            world.conn.execute_batch(
                "CREATE TABLE finances (club_id INTEGER NOT NULL, season INTEGER NOT NULL, category TEXT NOT NULL,
                                        amount INTEGER NOT NULL, PRIMARY KEY (club_id, season, category));
                 CREATE TABLE club_economy (club_id INTEGER PRIMARY KEY, base REAL NOT NULL);
                 UPDATE game SET version = 8;",
            )?;
            version = 8;
        }
        if version == 8 {
            world.conn.execute_batch(
                "CREATE TABLE transfers (id INTEGER PRIMARY KEY, date TEXT NOT NULL, person_id INTEGER NOT NULL, from_club INTEGER,
                                         to_club INTEGER, fee INTEGER NOT NULL, kind TEXT NOT NULL);
                 CREATE INDEX transfers_date ON transfers (date);
                 CREATE TABLE loans (person_id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL, until TEXT NOT NULL);
                 UPDATE game SET version = 9;",
            )?;
            version = 9;
        }
        if version == 9 {
            world.conn.execute_batch(DEALS_SCHEMA)?;
            world.conn.execute_batch("ALTER TABLE news ADD COLUMN offer_id INTEGER; UPDATE game SET version = 10;")?;
            version = 10;
        }
        let new_board = version == 10;
        if version == 10 {
            world.conn.execute_batch(BOARD_SCHEMA)?;
            world.conn.execute("UPDATE game SET version = 11", [])?;
            version = 11;
        }
        if version == 11 {
            world.conn.execute_batch(NUMBERS_SCHEMA)?;
            world.conn.execute("UPDATE game SET version = 12", [])?;
            version = 12;
        }
        if version == 12 {
            world.conn.execute_batch(UNHAPPY_SCHEMA)?;
            world.conn.execute("UPDATE game SET version = 13", [])?;
            version = 13;
        }
        if version == 13 {
            world.conn.execute_batch(SCOUTING_SCHEMA)?;
            world.conn.execute("UPDATE game SET version = 14", [])?;
            version = 14;
        }
        if version == 14 {
            world.conn.execute_batch(SCOUT_TASKS_SCHEMA)?;
            world.conn.execute("UPDATE game SET version = 15", [])?;
            version = 15;
        }
        if version != SAVE_VERSION {
            return Err(Error::Invalid(format!(
                "Jogo gravado numa versão diferente do NextCM (formato {version}, esperado {SAVE_VERSION})."
            )));
        }
        let news = world
            .conn
            .prepare("SELECT id, date, kind, title, body, read, offer_id, link FROM news ORDER BY date DESC, id DESC")?
            .query_map([], |r| {
                Ok(NewsItem {
                    id: r.get(0)?,
                    date: r.get(1)?,
                    kind: r.get(2)?,
                    title: r.get(3)?,
                    body: r.get(4)?,
                    read: r.get::<_, i32>(5)? != 0,
                    offer_id: r.get(6)?,
                    link: r.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let matches: Vec<Match> = world
            .conn
            .prepare(
                "SELECT id, competition_id, round, date, home_id, away_id, home_goals, away_goals,
                        stage, grp, tie, leg, neutral, home_pens, away_pens
                 FROM fixtures ORDER BY id",
            )?
            .query_map([], |r| {
                let goals: (Option<u8>, Option<u8>) = (r.get(6)?, r.get(7)?);
                let pens: (Option<u8>, Option<u8>) = (r.get(13)?, r.get(14)?);
                Ok(Match {
                    id: r.get(0)?,
                    competition_id: r.get(1)?,
                    round: r.get(2)?,
                    date: r.get(3)?,
                    home_id: r.get(4)?,
                    away_id: r.get(5)?,
                    result: goals.0.zip(goals.1),
                    stage: r.get(8)?,
                    group: r.get(9)?,
                    tie: r.get(10)?,
                    leg: r.get(11)?,
                    neutral: r.get::<_, i32>(12)? != 0,
                    pens: pens.0.zip(pens.1),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let states = world
            .conn
            .prepare("SELECT person_id, condition, morale, yellows, suspended, injury, injury_until, apps, missed FROM player_state")?
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    PlayerState {
                        condition: r.get(1)?,
                        morale: r.get(2)?,
                        cards: Default::default(),
                        injury: r.get(5)?,
                        injury_until: r.get(6)?,
                        apps: r.get(7)?,
                        missed: r.get(8)?,
                    },
                ))
            })?
            .collect::<rusqlite::Result<HashMap<i32, PlayerState>>>()?;
        let mut states = states;
        let cards: Vec<(i32, i32, i32, i32)> = world
            .conn
            .prepare("SELECT person_id, competition_id, yellows, suspended FROM cards")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, comp, yellows, suspended) in cards {
            states.entry(id).or_default().cards.insert(comp, crate::squad::Cards { yellows, suspended });
        }
        world.conn.execute_batch("BEGIN")?;
        let selection = load_selection(&world, club_id)?;
        let mut world = world;
        world.start_year = season_of(&date);
        let mut game = Game {
            world,
            path: path.to_path_buf(),
            name,
            date,
            seed,
            manager_id,
            club_id,
            news,
            matches,
            clubs: HashMap::new(),
            competitions: HashMap::new(),
            squads: Squads { states, traits: HashMap::new() },
            selection,
            unsaved: false,
            watch: false,
            live: None,
            records: Vec::new(),
        };
        // Saves de antes dos atributos gerados: completa-os uma vez
        if game.fill_missing_attributes()? > 0 {
            game.unsaved = true;
        }
        if new_board {
            game.board_setup()?;
            game.unsaved = true;
        }
        if redraw {
            let (mut fresh, mut next_id) = (Vec::new(), game.news.iter().map(|n| n.id).max().unwrap_or(0) + 1);
            let year = game.season();
            game.competitions = competition_names(&game.world, &[])?;
            game.schedule_season(SeasonKind::Calendar, year, &mut fresh, &mut next_id)?;
            game.schedule_season(
                SeasonKind::European,
                regulation::season_of(crate::league::PREMIER_LEAGUE, &game.date),
                &mut fresh,
                &mut next_id,
            )?;
            game.news.splice(0..0, fresh.into_iter().rev());
            game.unsaved = true;
        }
        game.refresh_live()?;
        Ok(game)
    }

    /// Grava o estado em memória no arquivo do save.
    pub fn save(&mut self) -> Result<()> {
        if self.live.is_some() {
            return Err(Error::Invalid("Não é possível gravar durante um jogo.".into()));
        }
        // O save vive numa transação aberta (ver `create`): aqui grava a memória e faz o commit
        let tx = &self.world.conn;
        tx.execute("UPDATE game SET date = ?, seed = ?", params![self.date, self.seed])?;
        tx.execute("DELETE FROM news", [])?;
        {
            let mut stmt =
                tx.prepare("INSERT INTO news (id, date, kind, title, body, read, offer_id, link) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")?;
            for n in &self.news {
                stmt.execute(params![n.id, n.date, n.kind, n.title, n.body, n.read as i32, n.offer_id, n.link])?;
            }
        }
        tx.execute("DELETE FROM fixtures", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO fixtures (id, competition_id, round, date, home_id, away_id, home_goals, away_goals,
                                       stage, grp, tie, leg, neutral, home_pens, away_pens)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )?;
            for m in &self.matches {
                let (hg, ag) = (m.result.map(|r| r.0), m.result.map(|r| r.1));
                let (hp, ap) = (m.pens.map(|r| r.0), m.pens.map(|r| r.1));
                stmt.execute(params![
                    m.id,
                    m.competition_id,
                    m.round,
                    m.date,
                    m.home_id,
                    m.away_id,
                    hg,
                    ag,
                    m.stage,
                    m.group,
                    m.tie,
                    m.leg,
                    m.neutral as i32,
                    hp,
                    ap
                ])?;
            }
        }
        tx.execute("DELETE FROM player_state", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO player_state (person_id, condition, morale, yellows, suspended, injury, injury_until, apps, missed)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )?;
            for (id, st) in &self.squads.states {
                if *st != PlayerState::default() {
                    stmt.execute(params![id, st.condition, st.morale, 0, 0, st.injury, st.injury_until, st.apps, st.missed])?;
                }
            }
        }
        tx.execute("DELETE FROM cards", [])?;
        {
            let mut stmt = tx.prepare("INSERT INTO cards (person_id, competition_id, yellows, suspended) VALUES (?, ?, ?, ?)")?;
            for (id, st) in &self.squads.states {
                for (comp, k) in &st.cards {
                    if k.yellows > 0 || k.suspended > 0 {
                        stmt.execute(params![id, comp, k.yellows, k.suspended])?;
                    }
                }
            }
        }
        tx.execute("DELETE FROM selection", [])?;
        tx.execute("DELETE FROM selection_players", [])?;
        if let Some(sel) = &self.selection {
            let i = &sel.instructions;
            let (mentality, passing, tackling) = instruction_keys(i);
            tx.execute(
                "INSERT INTO selection (club_id, formation, auto, mentality, passing, tackling, pressing, offside_trap,
                                        counter_attack, tight_marking) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    self.club_id,
                    sel.formation,
                    sel.auto,
                    mentality,
                    passing,
                    tackling,
                    i.pressing,
                    i.offside_trap,
                    i.counter_attack,
                    i.tight_marking
                ],
            )?;
            let mut stmt =
                tx.prepare("INSERT INTO selection_players (club_id, slot, person_id, x, y, run_x, run_y) VALUES (?, ?, ?, ?, ?, ?, ?)")?;
            for (slot, pid) in sel.starters.iter().enumerate() {
                let xy = sel.positions.get(slot);
                let run = sel.runs.get(slot).copied().flatten();
                stmt.execute(params![
                    self.club_id,
                    slot as i32,
                    pid,
                    xy.map(|p| p[0]),
                    xy.map(|p| p[1]),
                    run.map(|r| r[0]),
                    run.map(|r| r[1])
                ])?;
            }
            for (i, pid) in sel.bench.iter().enumerate() {
                stmt.execute(params![self.club_id, 100 + i as i32, pid, None::<i32>, None::<i32>, None::<i32>, None::<i32>])?;
            }
        }
        tx.execute_batch("COMMIT; BEGIN")?;
        self.unsaved = false;
        Ok(())
    }

    /// Avança um dia e joga as partidas desse dia; devolve as notícias novas. Com `watch`
    /// e um jogo do treinador hoje, pára antes dele (ver [`Game::live_view`]); chamar de
    /// novo nesse estado acaba o jogo e fecha o dia, sem mudar a data.
    pub fn advance_day(&mut self) -> Result<Vec<NewsItem>> {
        if self.live.is_some() {
            return self.live_finish();
        }
        if self.sacked() {
            return Err(Error::Invalid("Foi despedido: o jogo acabou.".into()));
        }
        // Convocados sem número: no app o treinador escolhe; sem ele (CLI, testes) vai o Auto
        let unnumbered = self.unnumbered_for_next_match()?;
        if !unnumbered.is_empty() {
            if self.watch {
                return Err(Error::Invalid("Há convocados sem número de camisola: defina os números do plantel.".into()));
            }
            let current: Vec<NumberAssign> =
                self.squad_numbers()?.players.iter().map(|p| NumberAssign { id: p.id, number: p.number }).collect();
            let auto = self.auto_squad_numbers(&current)?;
            self.set_squad_numbers(&auto)?;
        }
        self.date = date::add_days(&self.date, 1).ok_or_else(|| Error::Invalid("data do jogo inválida".into()))?;
        self.unsaved = true;

        let mut fresh = Vec::new();
        let mut next_id = self.news.iter().map(|n| n.id).max().unwrap_or(0) + 1;

        // Recuperação do dia; quem do clube do treinador voltou de lesão vira notícia
        for id in self.squads.new_day(&self.date.clone()) {
            if self.squads.traits.get(&id).is_some_and(|t| t.club_id == self.club_id) {
                let name = self.person_name(id)?;
                fresh.push(self.news_item(
                    next_id,
                    "injury",
                    format!("{name} recuperado"),
                    format!("{name} já está recuperado da lesão e pode voltar a jogar."),
                ));
                next_id += 1;
            }
        }
        // Evolução do CA, juniores, fim de época e nova época
        self.calendar_events(&mut fresh, &mut next_id)?;
        // Salários à segunda-feira, receitas mensais no dia 1; o mercado nas janelas
        self.daily_finances()?;
        self.market_day(&mut fresh, &mut next_id)?;
        self.offers_day(&mut fresh, &mut next_id)?;
        self.board_day(&mut fresh, &mut next_id)?;
        self.unhappy_day(&mut fresh, &mut next_id)?;
        self.scouting_day(&mut fresh, &mut next_id)?;
        self.scout_tasks_day()?;

        let today: Vec<usize> =
            (0..self.matches.len()).filter(|&i| self.matches[i].date == self.date && self.matches[i].result.is_none()).collect();
        let mut games = Vec::new();
        let mut user = None;
        for &i in &today {
            let (home, away) = (self.matches[i].home_id, self.matches[i].away_id);
            let comp = self.matches[i].competition_id;
            let (home_t, home_changes) = self.lineup(home, comp)?;
            let (away_t, away_changes) = self.lineup(away, comp)?;
            if let Some(changes) = home_changes.or(away_changes)
                && !changes.is_empty()
            {
                fresh.push(self.changes_news(next_id, &changes)?);
                next_id += 1;
            }
            let side = if home == self.club_id {
                Some(0)
            } else if away == self.club_id {
                Some(1)
            } else {
                None
            };
            let user_ai = self.selection.as_ref().is_none_or(|s| s.auto);
            let rng = engine::match_rng(self.seed, self.matches[i].id);
            let occasion = self.occasion(&self.matches[i])?;
            let mut live =
                LiveMatch::new(&self.world, self.fixture(&self.matches[i]), home_t, away_t, side, user_ai, rng)?.with_occasion(occasion);
            if side.is_some() && self.watch {
                user = Some(games.len());
                live = live.detailed(engine::match_rng(self.seed ^ 0x0F1A_7E55, self.matches[i].id));
            } else {
                live.play_until(90);
            }
            games.push((i, live));
        }
        let user_match = user.is_some();
        let extras = match user {
            Some(u) => Some(self.match_extras(games[u].0)?),
            None => None,
        };
        self.live = Some(LiveDay { games, user: user.unwrap_or(usize::MAX), news: fresh, extras });
        if user_match {
            return Ok(Vec::new());
        }
        self.live_finish()
    }

    /// Acaba os jogos do dia (o do treinador até ao fim, se ainda corre), guarda os
    /// resultados e o que deixaram nos jogadores, e entrega as notícias do dia.
    pub fn live_finish(&mut self) -> Result<Vec<NewsItem>> {
        let Some(mut day) = self.live.take() else { return Ok(Vec::new()) };
        let mut fresh = std::mem::take(&mut day.news);
        let mut next_id = self.news.iter().chain(&fresh).map(|n| n.id).max().unwrap_or(0) + 1;
        let date = self.date.clone();
        for (i, live) in &mut day.games {
            live.play_until(90);
            self.matches[*i].result = Some(live.score());
            self.settle_tie(*i);
            let played = self.matches[*i].clone();
            self.gate(&played)?;
            let sides = live.sides();
            let injuries = sides[0].injured.len() + sides[1].injured.len();
            self.records.push(MatchRecord { fixture_id: live.fixture.id, stats: live.stats(), subs: live.subs_made(), injuries });
            // As durações das lesões continuam o sorteio da partida
            let mut rng = engine::match_rng(self.seed ^ 0x1E5A_0C0D, live.fixture.id);
            for event in self.squads.after_match(sides, &date, live.fixture.competition_id, &mut rng) {
                if let Some(n) = self.event_news(next_id, &event)? {
                    fresh.push(n);
                    next_id += 1;
                }
            }
        }
        for (i, live) in &day.games {
            let m = &self.matches[*i];
            if m.home_id == self.club_id || m.away_id == self.club_id {
                fresh.push(self.result_news(next_id, m, live));
                next_id += 1;
                let (h, a) = live.score();
                let (ours, theirs) = if m.home_id == self.club_id { (h, a) } else { (a, h) };
                self.board_result(ours as i32, theirs as i32, &mut fresh, &mut next_id)?;
            }
        }
        // Rondas e fases que acabaram hoje passam à seguinte; as competições que acabaram têm campeão
        let mut played: Vec<i32> = day.games.iter().map(|(i, _)| self.matches[*i].competition_id).collect();
        played.sort_unstable();
        played.dedup();
        for comp in self.progress(&played, &mut fresh, &mut next_id)? {
            self.prizes(comp, self.current_season(comp))?;
            if let Some(n) = self.champion_news(next_id, comp)? {
                fresh.push(n);
                next_id += 1;
            }
        }
        for n in &fresh {
            self.news.insert(0, n.clone());
        }
        fresh.reverse();
        self.unsaved = true;
        Ok(fresh)
    }

    /// "Continuar Jogo": avança dia a dia até haver notícias (o jogo do treinador, o fim
    /// de uma competição...), até à véspera de um jogo do treinador (para ver o Próximo
    /// Jogo antes de o jogar) ou até [`MAX_IDLE_DAYS`] dias sem nada.
    pub fn advance_to_next_event(&mut self) -> Result<Vec<NewsItem>> {
        for _ in 0..MAX_IDLE_DAYS {
            let news = self.advance_day()?;
            // As transferências do dia ficam nas notícias mas não param o Continuar Jogo
            if news.iter().any(|n| n.kind != "transfer") || self.live.is_some() {
                return Ok(news);
            }
            let tomorrow = date::add_days(&self.date, 1).unwrap_or_default();
            if self.matches.iter().any(|m| m.date == tomorrow && (m.home_id == self.club_id || m.away_id == self.club_id)) {
                break;
            }
        }
        Ok(Vec::new())
    }

    /// A equipa de um clube para o jogo de hoje. No clube do treinador, a escolha dele
    /// (com os indisponíveis trocados pelo adjunto, devolvidos em `Some(trocas)`).
    fn lineup(&self, club: i32, comp: i32) -> Result<(ClubTactics, Option<Vec<(Option<i32>, i32)>>)> {
        let state = self.squads.squad_state(club, Some(comp));
        if club != self.club_id {
            return Ok((self.world.auto_tactics(club, Some(&state), Instructions::default())?, None));
        }
        match &self.selection {
            Some(sel) if !sel.auto => {
                let mut sel = sel.clone();
                let changes = self.world.cover_unavailable(club, &mut sel, &state)?;
                Ok((self.world.tactics_from_selection(club, &sel, Some(&state))?, Some(changes)))
            }
            other => {
                let instructions = other.as_ref().map(|s| s.instructions).unwrap_or_default();
                Ok((self.world.auto_tactics(club, Some(&state), instructions)?, None))
            }
        }
    }

    /// Tácticas de um clube agora: o do treinador é editável; os outros, o que o adjunto escolheria.
    pub fn club_tactics(&self, club: i32) -> Result<ClubTactics> {
        let state = self.squads.squad_state(club, self.next_competition(club));
        let mut t = match (&self.selection, club == self.club_id) {
            (Some(sel), true) if !sel.auto => self.world.tactics_from_selection(club, sel, Some(&state))?,
            (sel, true) => self.world.auto_tactics(club, Some(&state), sel.as_ref().map(|s| s.instructions).unwrap_or_default())?,
            _ => self.world.auto_tactics(club, Some(&state), Instructions::default())?,
        };
        t.editable = club == self.club_id;
        Ok(t)
    }

    /// Como ficaria a equipa com esta escolha (sem gravar): para a tela de Tácticas.
    pub fn preview_selection(&self, sel: &Selection) -> Result<ClubTactics> {
        let state = self.squads.squad_state(self.club_id, self.next_competition(self.club_id));
        let mut t = if sel.auto {
            self.world.auto_tactics(self.club_id, Some(&state), sel.instructions)?
        } else {
            self.world.tactics_from_selection(self.club_id, sel, Some(&state))?
        };
        t.editable = true;
        Ok(t)
    }

    /// Põe ou tira um jogador da escolha e mostra como fica (sem gravar).
    pub fn toggle_player(&self, sel: &Selection, player: i32) -> Result<ClubTactics> {
        let mut sel = sel.clone();
        self.world.toggle_player(self.club_id, &mut sel, player)?;
        self.preview_selection(&sel)
    }

    /// Grava a escolha do treinador (Ok na tela de Tácticas).
    pub fn set_selection(&mut self, sel: Selection) -> Result<ClubTactics> {
        let t = self.preview_selection(&sel)?;
        self.selection = Some(sel);
        self.unsaved = true;
        Ok(t)
    }

    /// Condição, moral, lesão e castigos de um jogador de um clube vivo.
    pub fn player_status(&self, id: i32) -> Option<PlayerStatus> {
        let mut s = self.squads.status(id)?;
        s.unhappy = self.unhappy_reasons(id).unwrap_or_default();
        Some(s)
    }

    fn person_name(&self, id: i32) -> Result<String> {
        Ok(self.world.conn.query_row("SELECT display_name FROM people WHERE id = ?", [id], |r| r.get(0))?)
    }

    fn news_item(&self, id: i32, kind: &str, title: String, body: String) -> NewsItem {
        NewsItem { id, date: self.date.clone(), kind: kind.into(), title, body, read: false, offer_id: None, link: None }
    }

    /// Lesões e suspensões dos jogadores do treinador.
    fn event_news(&self, id: i32, event: &Event) -> Result<Option<NewsItem>> {
        Ok(match event {
            Event::Injured { player, club, injury, days } if *club == self.club_id => {
                let name = self.person_name(*player)?;
                let back = self.squads.state(*player).injury_until.map(|d| cm_date(&d)).unwrap_or_default();
                Some(self.news_item(
                    id,
                    "injury",
                    format!("{name} lesionado"),
                    format!(
                        "{name} sofreu uma lesão no jogo de hoje: {}.\n\nO departamento médico prevê cerca de {days} dias de paragem (regresso a {back}).",
                        injury.to_lowercase()
                    ),
                ))
            }
            Event::Suspended { player, club, red } if *club == self.club_id => {
                let name = self.person_name(*player)?;
                let why = if *red { "foi expulso" } else { "viu o terceiro cartão amarelo" };
                Some(self.news_item(id, "injury", format!("{name} suspenso"), format!("{name} {why} e cumpre um jogo de suspensão.")))
            }
            _ => None,
        })
    }

    /// O adjunto mexeu na equipa porque havia indisponíveis.
    fn changes_news(&self, id: i32, changes: &[(Option<i32>, i32)]) -> Result<NewsItem> {
        let mut lines = Vec::new();
        for &(out, inn) in changes {
            let inn = self.person_name(inn)?;
            lines.push(match out {
                Some(out) => {
                    let st = self.squads.state(out);
                    let why = if st.injury.is_some() { "lesionado" } else { "suspenso" };
                    format!("Sai {} ({why}), entra {inn}.", self.person_name(out)?)
                }
                None => format!("Entra {inn} para completar a equipa."),
            });
        }
        Ok(self.news_item(
            id,
            "message",
            "Alterações na equipa".into(),
            format!("O seu adjunto fez alterações na equipa para o jogo de hoje:\n\n{}", lines.join("\n")),
        ))
    }

    /// O que rodeia uma partida: decisiva (eliminatória ou clássico entre rivais), a lotação
    /// do estádio da casa e as reputações dos dois clubes (o peso da camisa).
    fn occasion(&self, m: &Match) -> Result<Occasion> {
        let facts = |id: i32| -> Result<(i32, Option<i32>, [Option<i32>; 3])> {
            Ok(self
                .world
                .conn
                .prepare_cached(
                    "SELECT COALESCE(c.reputation, 0), s.capacity, c.rival1_id, c.rival2_id, c.rival3_id
                     FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id WHERE c.id = ?",
                )?
                .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, [r.get(2)?, r.get(3)?, r.get(4)?])))?)
        };
        let (home_rep, capacity, home_rivals) = facts(m.home_id)?;
        let (away_rep, _, away_rivals) = facts(m.away_id)?;
        let derby = home_rivals.contains(&Some(m.away_id)) || away_rivals.contains(&Some(m.home_id));
        Ok(Occasion::new(m.tie.is_some() || derby, m.neutral, capacity, home_rep, away_rep))
    }

    /// Competição do próximo jogo de um clube (para saber quem está castigado).
    fn next_competition(&self, club: i32) -> Option<i32> {
        self.matches
            .iter()
            .filter(|m| m.result.is_none() && m.date.as_str() >= self.date.as_str() && (m.home_id == club || m.away_id == club))
            .min_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)))
            .map(|m| m.competition_id)
    }

    fn competition_name(&self, id: i32) -> &str {
        self.competitions.get(&id).map_or("Liga", String::as_str)
    }

    /// "da Série A", "do Campeonato Paulista".
    fn of_competition(&self, id: i32) -> String {
        let name = self.competition_name(id);
        format!("{} {name}", if name.starts_with("Campeonato") { "do" } else { "da" })
    }

    /// "a Série A", "o Campeonato Paulista".
    fn the_competition(&self, id: i32) -> String {
        let name = self.competition_name(id);
        format!("{} {name}", if name.starts_with("Campeonato") { "o" } else { "a" })
    }

    /// Um clube por id: dos clubes vivos em memória ou, para um que acabou de entrar numa
    /// competição (sorteios da nova época), do banco.
    fn club(&self, id: i32) -> ClubRef {
        self.clubs.get(&id).cloned().or_else(|| self.world.club_ref(Some(id)).ok().flatten()).unwrap_or_else(|| ClubRef {
            id,
            name: format!("clube {id}"),
            short_name: format!("clube {id}"),
            fg: None,
            bg: None,
        })
    }

    fn fixture(&self, m: &Match) -> Fixture {
        Fixture {
            id: m.id,
            competition_id: m.competition_id,
            competition: self.competition_name(m.competition_id).into(),
            round: m.round,
            date: m.date.clone(),
            home: self.club(m.home_id),
            away: self.club(m.away_id),
            home_goals: m.result.map(|r| r.0 as i32),
            away_goals: m.result.map(|r| r.1 as i32),
            label: self.match_label(m),
            season: regulation::season_label(m.competition_id, regulation::season_of(m.competition_id, &m.date)),
            neutral: m.neutral,
            home_pens: m.pens.map(|r| r.0 as i32),
            away_pens: m.pens.map(|r| r.1 as i32),
        }
    }

    /// Ficha do "Próximo Jogo" para uma partida.
    pub fn match_preview(&self, fixture_id: i32) -> Result<MatchPreview> {
        let m = self.matches.iter().find(|m| m.id == fixture_id).ok_or_else(|| Error::NotFound(format!("partida {fixture_id}")))?;
        let (venue, city): (Option<String>, Option<String>) = self.world.conn.query_row(
            "SELECT s.name, ci.name FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id
             LEFT JOIN cities ci ON ci.id = s.city_id WHERE c.id = ?",
            [m.home_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let last_meeting = self
            .matches
            .iter()
            .filter(|o| {
                o.result.is_some()
                    && o.date < m.date
                    && ((o.home_id == m.home_id && o.away_id == m.away_id) || (o.home_id == m.away_id && o.away_id == m.home_id))
            })
            .max_by(|a, b| a.date.cmp(&b.date))
            .map(|o| self.fixture(o));

        // Tempo: estação do ano no Brasil e um sorteio fixo por partida
        let mut rng = engine::match_rng(self.seed ^ 0x7E47_4E52, m.id);
        let roll = |rng: &mut ChaCha8Rng| engine::uniform(rng);
        let month: u32 = m.date.get(5..7).and_then(|v| v.parse().ok()).unwrap_or(7);
        let (min, max) = match month {
            12 | 1 | 2 => (24, 34),
            3..=5 => (20, 30),
            6..=8 => (14, 26),
            _ => (18, 29),
        };
        let temp = min + (roll(&mut rng) * (max - min + 1) as f64) as i32;
        let sky = roll(&mut rng);
        let (weather, pitch) = if sky < 0.55 {
            ("Seco", "Normais")
        } else if sky < 0.78 {
            ("Nublado", "Normais")
        } else if sky < 0.95 {
            ("Chuva", "Molhado")
        } else {
            ("Chuva forte", "Encharcado")
        };

        let news = |club: i32| -> Result<Vec<String>> {
            let mut out = Vec::new();
            let mut ids: Vec<i32> = self.squads.traits.iter().filter(|(_, t)| t.club_id == club).map(|(&id, _)| id).collect();
            ids.sort_unstable();
            for id in ids {
                let st = self.squads.state(id);
                if let Some(injury) = &st.injury {
                    let back = st.injury_until.as_deref().map(cm_date).unwrap_or_default();
                    out.push(format!("{} lesionado ({}, regresso a {back})", self.person_name(id)?, injury.to_lowercase()));
                } else if st.suspended_in(Some(m.competition_id)) > 0 {
                    out.push(format!("{} suspenso", self.person_name(id)?));
                }
            }
            Ok(out)
        };
        Ok(MatchPreview {
            fixture: self.fixture(m),
            venue,
            city,
            last_meeting,
            weather: format!("{weather}, {temp}°C"),
            pitch: pitch.into(),
            home_news: news(m.home_id)?,
            away_news: news(m.away_id)?,
        })
    }

    /// Calendário de uma competição viva, por jornada (vazio nas outras).
    pub fn fixtures(&self, competition_id: i32) -> Vec<Fixture> {
        self.season_matches().filter(|m| m.competition_id == competition_id).map(|m| self.fixture(m)).collect()
    }

    /// Partidas de um clube, por data.
    pub fn club_fixtures(&self, club_id: i32) -> Vec<Fixture> {
        let mut out: Vec<&Match> = self.season_matches().filter(|m| m.home_id == club_id || m.away_id == club_id).collect();
        out.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
        out.into_iter().map(|m| self.fixture(m)).collect()
    }

    /// Classificação da 1ª fase de uma competição viva, se for de pontos corridos (vazia nas outras).
    pub fn table(&self, competition_id: i32) -> Vec<StandingRow> {
        self.stage_table(competition_id, self.current_season(competition_id), 0, None)
    }

    /// Notícia do resultado do jogo do treinador, com a posição na tabela e os marcadores.
    fn result_news(&self, id: i32, m: &Match, live: &LiveMatch) -> NewsItem {
        let (hg, ag) = m.result.unwrap_or((0, 0));
        let (home, away) = (self.club(m.home_id), self.club(m.away_id));
        let at_home = m.home_id == self.club_id;
        let (ours, theirs, gf, ga) = if at_home { (&home, &away, hg, ag) } else { (&away, &home, ag, hg) };
        let place = if at_home { "em casa" } else { "fora" };
        let what = if gf > ga {
            format!("O {} venceu o {} {place} por {gf}-{ga}.", ours.short_name, theirs.short_name)
        } else if gf == ga {
            format!("O {} empatou {place} com o {}: {gf}-{ga}.", ours.short_name, theirs.short_name)
        } else {
            format!("O {} perdeu {place} com o {} por {gf}-{ga}.", ours.short_name, theirs.short_name)
        };
        let standing = self
            .table_for(m)
            .iter()
            .find(|r| r.club.id == self.club_id)
            .map(|r| format!("\n\nCom este resultado, o {} é {}º classificado com {} pontos.", ours.short_name, r.position, r.points))
            .unwrap_or_default();
        let pens = m
            .pens
            .map(|(h, a)| {
                let (pf, pa) = if at_home { (h, a) } else { (a, h) };
                let how = if pf > pa { "passou" } else { "foi eliminado" };
                format!(" Nos penáltis, o {} {how} ({pf}-{pa}).", ours.short_name)
            })
            .unwrap_or_default();
        let mut scorers = String::new();
        for (side, club) in [(0, &home), (1, &away)] {
            let goals = live.scorers(side);
            if !goals.is_empty() {
                let list: Vec<String> = goals.iter().map(|(n, min)| format!("{n} {min}'")).collect();
                scorers += &format!("\n{}: {}", club.short_name, list.join(", "));
            }
        }
        if !scorers.is_empty() {
            scorers = format!("\n\nMarcadores:{scorers}");
        }
        NewsItem {
            id,
            date: self.date.clone(),
            kind: "competition".into(),
            title: format!("{} {hg}-{ag} {}", home.short_name, away.short_name),
            body: format!("{}, {}.\n\n{what}{pens}{scorers}{standing}", self.competition_name(m.competition_id), self.match_label(m)),
            read: false,
            offer_id: None,
            link: None,
        }
    }

    /// Notícia do campeão, com o que aconteceu à equipa do treinador.
    fn champion_news(&self, id: i32, competition_id: i32) -> Result<Option<NewsItem>> {
        let year = self.current_season(competition_id);
        let Some((winner, second, _)) = self.podium(competition_id, year) else { return Ok(None) };
        let label = regulation::season_label(competition_id, year);
        let champion = self.club(winner).short_name;
        let table = self.table(competition_id);
        let single_stage = crate::regulation::get(competition_id).is_some_and(|r| r.stages.len() == 1);
        let mut body = match (table.first(), single_stage) {
            (Some(first), true) => {
                let mut b =
                    format!("O {champion} sagrou-se campeão {} {label} com {} pontos", self.of_competition(competition_id), first.points);
                if let Some(s) = table.get(1) {
                    b += &format!(", {} à frente do {}", first.points - s.points, s.club.short_name);
                }
                b + "."
            }
            _ => match second {
                Some(s) => format!(
                    "O {champion} venceu {} {label}: na final bateu o {}.",
                    self.the_competition(competition_id),
                    self.club(s).short_name
                ),
                None => format!("O {champion} venceu {} {label}.", self.the_competition(competition_id)),
            },
        };
        if winner != self.club_id
            && let Some(fate) = self.club_fate(competition_id, year, self.club_id)
        {
            body += &format!("\n\n{fate}");
        }
        Ok(Some(NewsItem {
            id,
            date: self.date.clone(),
            kind: "competition".into(),
            title: format!("{champion} campeão {}", self.of_competition(competition_id)),
            body,
            read: false,
            offer_id: None,
            link: None,
        }))
    }

    /// Notícia do início de uma época (do ano civil ou europeia): as competições dessa época
    /// em que joga o clube do treinador e o primeiro jogo; `None` se não joga nenhuma.
    fn season_news(&self, id: i32, kind: SeasonKind) -> Option<NewsItem> {
        let mine =
            |m: &&Match| (m.home_id == self.club_id || m.away_id == self.club_id) && regulation::season_kind(m.competition_id) == kind;
        let mut ours: Vec<&Match> = self.season_matches().filter(mine).collect();
        ours.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
        let first = ours.first()?;
        let mut comps: Vec<i32> = Vec::new();
        for m in &ours {
            if !comps.contains(&m.competition_id) {
                comps.push(m.competition_id);
            }
        }
        let us = self.club(self.club_id).short_name;
        let label = regulation::season_label(first.competition_id, regulation::season_of(first.competition_id, &first.date));
        let mut body = format!("Época {label}: o {us} joga ");
        let names: Vec<String> = comps
            .iter()
            .map(|&c| {
                let dates: Vec<&str> = self.season_matches().filter(|m| m.competition_id == c).map(|m| m.date.as_str()).collect();
                let from = dates.iter().min().copied().unwrap_or_default();
                format!("{} (a partir de {})", self.competition_name(c), cm_date(from))
            })
            .collect();
        body += &season::join_names(&names);
        body += ".";
        let (verb, other) = if first.home_id == self.club_id { ("recebe", first.away_id) } else { ("visita", first.home_id) };
        body += &format!(
            "\n\nO primeiro jogo é a {}: o {us} {verb} o {} ({}, {}).",
            cm_date(&first.date),
            self.club(other).short_name,
            self.competition_name(first.competition_id),
            self.match_label(first)
        );
        Some(NewsItem {
            id,
            date: self.date.clone(),
            kind: "competition".into(),
            title: format!("Calendário {label}"),
            body,
            read: false,
            offer_id: None,
            link: None,
        })
    }

    /// A época em curso (o ano).
    pub fn season(&self) -> i32 {
        season_of(&self.date)
    }

    /// Jogos da época em curso de cada competição (o ano civil ou a época europeia).
    fn season_matches(&self) -> impl Iterator<Item = &Match> {
        self.matches.iter().filter(move |m| regulation::season_of(m.competition_id, &m.date) == self.current_season(m.competition_id))
    }

    pub fn info(&self) -> Result<GameInfo> {
        let manager_name: String =
            self.world.conn.query_row("SELECT display_name FROM people WHERE id = ?", [self.manager_id], |r| r.get(0))?;
        let club = self.world.club_ref(Some(self.club_id))?.ok_or_else(|| Error::NotFound(format!("clube {}", self.club_id)))?;
        Ok(GameInfo {
            name: self.name.clone(),
            date: self.date.clone(),
            manager_id: self.manager_id,
            manager_name,
            club,
            unread_news: self.news.iter().filter(|n| !n.read).count() as i32,
            unsaved: self.unsaved,
            live: self.live.is_some(),
            sacked: self.sacked(),
            needs_numbers: self.live.is_none() && !self.unnumbered_for_next_match()?.is_empty(),
        })
    }

    /// Liga/desliga ver os jogos do treinador ao vivo (o app liga; a CLI não).
    pub fn set_watch(&mut self, watch: bool) {
        self.watch = watch;
    }

    /// Números dos jogos acabados desde que o jogo foi aberto.
    pub fn records(&self) -> &[MatchRecord] {
        &self.records
    }

    fn live_day(&self) -> Result<&LiveDay> {
        self.live.as_ref().filter(|d| d.user < d.games.len()).ok_or_else(|| Error::Invalid("Não há nenhum jogo a decorrer.".into()))
    }

    fn live_match_mut(&mut self) -> Result<&mut LiveMatch> {
        self.live_day()?;
        let day = self.live.as_mut().expect("dia de jogo");
        Ok(&mut day.games[day.user].1)
    }

    /// O jogo do treinador como está agora, com os outros jogos do dia no mesmo minuto.
    pub fn live_view(&self) -> Result<LiveView> {
        let day = self.live_day()?;
        let (index, live) = &day.games[day.user];
        // Os outros jogos do dia da mesma competição
        let comp = live.fixture.competition_id;
        let others: Vec<&LiveMatch> =
            day.games.iter().enumerate().filter(|(k, g)| *k != day.user && g.1.fixture.competition_id == comp).map(|(_, g)| &g.1).collect();
        let mut v = live.view(&others);
        if let Some(x) = &day.extras {
            v.venue = x.venue.clone();
            v.city = x.city.clone();
            v.weather = x.weather.clone();
            v.referee = x.referee.clone();
            v.attendance = x.attendance;
        }
        // Classificação com os jogos do dia no minuto em que vai o do treinador
        let comp = self.matches[*index].competition_id;
        let mut matches: Vec<Match> = self.matches.iter().filter(|m| m.competition_id == comp).cloned().collect();
        if v.minute > 0 {
            for (i, g) in &day.games {
                let (h, a) = g.score_at(v.minute as u8);
                if let Some(m) = matches.iter_mut().find(|m| m.id == self.matches[*i].id) {
                    m.result = Some((h as u8, a as u8));
                }
            }
        }
        let mut ids: Vec<i32> = matches.iter().flat_map(|m| [m.home_id, m.away_id]).collect();
        ids.sort_unstable();
        ids.dedup();
        let clubs: Vec<ClubRef> = ids.into_iter().map(|id| self.club(id)).collect();
        v.table = league::standings(&matches, &clubs);
        Ok(v)
    }

    /// Estádio e tempo (os do Próximo Jogo), árbitro e assistência de uma partida.
    fn match_extras(&self, index: usize) -> Result<MatchExtras> {
        let m = &self.matches[index];
        let preview = self.match_preview(m.id)?;
        let mut rng = engine::match_rng(self.seed ^ 0x00A7_7E4D, m.id);
        // Árbitro: um nome e um apelido de pessoas da nação do clube da casa (um nome inventado)
        let nation: Option<i32> = self.world.conn.query_row("SELECT nation_id FROM clubs WHERE id = ?", [m.home_id], |r| r.get(0))?;
        let count: i64 = self.world.conn.query_row(
            "SELECT COUNT(*) FROM people WHERE nation_id IS ? AND first_name <> '' AND second_name <> ''",
            [nation],
            |r| r.get(0),
        )?;
        let mut name_at = |col: &str| -> Result<String> {
            let offset = (rand_core::RngCore::next_u64(&mut rng) % count.max(1) as u64) as i64;
            Ok(self.world.conn.query_row(
                &format!("SELECT {col} FROM people WHERE nation_id IS ? AND first_name <> '' AND second_name <> '' LIMIT 1 OFFSET ?"),
                params![nation, offset],
                |r| r.get::<_, String>(0),
            )?)
        };
        let referee = if count > 0 {
            let (first, last) = (name_at("first_name")?, name_at("second_name")?);
            let first = first.split_whitespace().next().unwrap_or_default().to_string();
            format!("{first} {}", last.split_whitespace().last().unwrap_or_default())
        } else {
            "-".into()
        };
        let attendance = self.attendance(m)?;
        Ok(MatchExtras { venue: preview.venue, city: preview.city, weather: preview.weather, referee, attendance })
    }

    /// Joga até ao minuto `minute` (no máximo 90).
    pub fn live_step(&mut self, minute: i32) -> Result<LiveView> {
        self.live_match_mut()?.play_until(minute.clamp(0, 90) as u8);
        self.live_view()
    }

    /// Substituição na equipa do treinador.
    pub fn live_substitute(&mut self, out: i32, inn: i32) -> Result<LiveView> {
        self.live_match_mut()?.substitute(out, inn)?;
        self.live_view()
    }

    /// Troca as funções de dois jogadores do treinador em campo.
    pub fn live_swap(&mut self, a: i32, b: i32) -> Result<LiveView> {
        self.live_match_mut()?.swap(a, b)?;
        self.live_view()
    }

    /// As Tácticas de um lado do jogo do treinador (0 casa, 1 fora), como a tela as mostra.
    pub fn live_tactics(&self, side: i32) -> Result<ClubTactics> {
        let day = self.live_day()?;
        day.games[day.user].1.tactics(&self.world, side.clamp(0, 1) as usize, None)
    }

    /// Como ficaria a equipa do treinador com esta escolha (sem mexer no jogo).
    pub fn live_preview_tactics(&self, sel: &Selection) -> Result<ClubTactics> {
        let day = self.live_day()?;
        let live = &day.games[day.user].1;
        let side = live.user_side().ok_or_else(|| Error::Invalid("Este jogo não é da sua equipa.".into()))?;
        live.tactics(&self.world, side, Some(sel))
    }

    /// O Ok das Tácticas a meio do jogo: aplica a escolha toda de uma vez.
    pub fn live_set_tactics(&mut self, sel: &Selection) -> Result<LiveView> {
        self.live_day()?;
        let day = self.live.as_mut().expect("dia de jogo");
        day.games[day.user].1.apply_selection(&self.world, sel)?;
        self.live_view()
    }

    /// Instruções à equipa a meio do jogo.
    pub fn live_instructions(&mut self, instructions: Instructions) -> Result<LiveView> {
        self.live_match_mut()?.set_instructions(instructions)?;
        self.live_view()
    }

    /// Notícias, da mais recente para a mais antiga.
    pub fn news(&self) -> &[NewsItem] {
        &self.news
    }

    pub fn mark_news_read(&mut self, id: i32) {
        if let Some(n) = self.news.iter_mut().find(|n| n.id == id && !n.read) {
            n.read = true;
            self.unsaved = true;
        }
    }

    /// O mundo deste jogo (consultas da enciclopédia já veem o treinador no clube).
    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn date(&self) -> &str {
        &self.date
    }
}

/// Clubes de uma divisão, por id.
/// Instruções no save, pelas mesmas palavras do JSON da interface.
fn instruction_keys(i: &Instructions) -> (&'static str, &'static str, &'static str) {
    let m = match i.mentality {
        Mentality::Defensive => "defensive",
        Mentality::Normal => "normal",
        Mentality::Attacking => "attacking",
    };
    let p = match i.passing {
        Passing::Mixed => "mixed",
        Passing::Short => "short",
        Passing::Direct => "direct",
        Passing::Long => "long",
    };
    let t = match i.tackling {
        Tackling::Normal => "normal",
        Tackling::Soft => "soft",
        Tackling::Hard => "hard",
    };
    (m, p, t)
}

/// Lê a escolha do treinador gravada no save.
fn load_selection(world: &World, club_id: i32) -> Result<Option<Selection>> {
    let row = world
        .conn
        .query_row(
            "SELECT formation, auto, mentality, passing, tackling, pressing, offside_trap, counter_attack, tight_marking
             FROM selection WHERE club_id = ?",
            [club_id],
            |r| {
                Ok((
                    r.get::<_, i32>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    [r.get::<_, bool>(5)?, r.get(6)?, r.get(7)?, r.get(8)?],
                ))
            },
        )
        .optional()?;
    let Some((formation, auto, mentality, passing, tackling, flags)) = row else { return Ok(None) };
    let instructions = Instructions {
        mentality: match mentality.as_str() {
            "defensive" => Mentality::Defensive,
            "attacking" => Mentality::Attacking,
            _ => Mentality::Normal,
        },
        passing: match passing.as_str() {
            "short" => Passing::Short,
            "direct" => Passing::Direct,
            "long" => Passing::Long,
            _ => Passing::Mixed,
        },
        tackling: match tackling.as_str() {
            "soft" => Tackling::Soft,
            "hard" => Tackling::Hard,
            _ => Tackling::Normal,
        },
        pressing: flags[0],
        offside_trap: flags[1],
        counter_attack: flags[2],
        tight_marking: flags[3],
    };
    let mut starters = vec![None; 11];
    let mut positions = vec![[0, 0]; 11];
    let mut custom = 0;
    let mut bench = Vec::new();
    let mut runs = vec![None; 11];
    let mut stmt =
        world.conn.prepare("SELECT slot, person_id, x, y, run_x, run_y FROM selection_players WHERE club_id = ? ORDER BY slot")?;
    let rows = stmt.query_map([club_id], |r| {
        Ok((
            r.get::<_, i32>(0)?,
            r.get::<_, Option<i32>>(1)?,
            r.get::<_, Option<i32>>(2)?,
            r.get::<_, Option<i32>>(3)?,
            r.get::<_, Option<i32>>(4)?,
            r.get::<_, Option<i32>>(5)?,
        ))
    })?;
    for row in rows {
        let (slot, pid, x, y, rx, ry) = row?;
        match slot {
            0..=10 => {
                starters[slot as usize] = pid;
                runs[slot as usize] = rx.zip(ry).map(|(x, y)| [x, y]);
                if let (Some(x), Some(y)) = (x, y) {
                    positions[slot as usize] = [x, y];
                    custom += 1;
                }
            }
            100.. => bench.extend(pid),
            _ => {}
        }
    }
    let positions = if custom == 11 { positions } else { Vec::new() };
    Ok(Some(Selection { formation, starters, bench, instructions, auto, positions, runs }))
}

/// Save do formato 3 → 4: colunas x/y e os titulares reordenados para a ordem do CM
/// (antes ficavam pela ordem da formação).
fn migrate_v3(world: &World, club_id: i32) -> Result<()> {
    let tx = world.conn.unchecked_transaction()?;
    tx.execute_batch("ALTER TABLE selection_players ADD COLUMN x INTEGER; ALTER TABLE selection_players ADD COLUMN y INTEGER;")?;
    let formation: Option<i32> = tx.query_row("SELECT formation FROM selection WHERE club_id = ?", [club_id], |r| r.get(0)).optional()?;
    if let Some(formation) = formation {
        let old: HashMap<i32, Option<i32>> = tx
            .prepare("SELECT slot, person_id FROM selection_players WHERE club_id = ? AND slot <= 10")?
            .query_map([club_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        tx.execute("DELETE FROM selection_players WHERE club_id = ? AND slot <= 10", [club_id])?;
        for (k, from) in crate::tactics::cm_order_permutation(formation).into_iter().enumerate() {
            let pid = old.get(&(from as i32)).copied().flatten();
            tx.execute("INSERT INTO selection_players (club_id, slot, person_id) VALUES (?, ?, ?)", params![club_id, k as i32, pid])?;
        }
    }
    tx.execute("UPDATE game SET version = 4", [])?;
    tx.commit()?;
    Ok(())
}

/// Época de uma data: o ano civil, como no futebol brasileiro (desde o 3.6).
pub(crate) fn season_of(date: &str) -> i32 {
    date.get(0..4).and_then(|y| y.parse().ok()).unwrap_or(0)
}

fn league_club_ids(world: &World, division_id: i32) -> Result<Vec<i32>> {
    let ids = world
        .conn
        .prepare("SELECT id FROM clubs WHERE division_id = ? ORDER BY id")?
        .query_map([division_id], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

fn club_refs(world: &World, ids: &[i32]) -> Result<HashMap<i32, ClubRef>> {
    let mut out = HashMap::with_capacity(ids.len());
    for &id in ids {
        if let Some(c) = world.club_ref(Some(id))? {
            out.insert(id, c);
        }
    }
    Ok(out)
}

/// Nomes das competições dos jogos e de todas as que têm regulamento (as notícias dos
/// sorteios da nova época saem antes de haver jogos).
fn competition_names(world: &World, matches: &[Match]) -> Result<HashMap<i32, String>> {
    let mut out = HashMap::new();
    let ids = matches.iter().map(|m| m.competition_id).chain(regulation::all().iter().map(|r| r.competition));
    for comp in ids {
        if let std::collections::hash_map::Entry::Vacant(e) = out.entry(comp) {
            // O nome do regulamento quando o do banco não serve ("Copa Sudamericana")
            if let Some(name) = crate::regulation::get(comp).and_then(|r| r.name.clone()) {
                e.insert(name);
                continue;
            }
            let (name, short): (String, String) =
                world.conn.query_row("SELECT name, short_name FROM competitions WHERE id = ?", [comp], |r| Ok((r.get(0)?, r.get(1)?)))?;
            e.insert(if short.is_empty() { name } else { short });
        }
    }
    Ok(out)
}

/// "2025-08-02" → "2.8.2025", como nas notícias do CM.
fn cm_date(iso: &str) -> String {
    let mut it = iso.split('-').map(|p| p.trim_start_matches('0'));
    let (y, m, d) = (it.next().unwrap_or(""), it.next().unwrap_or(""), it.next().unwrap_or(""));
    format!("{d}.{m}.{y}")
}

/// Mensagem de boas-vindas da direção, com a expectativa pela reputação do clube na divisão.
fn welcome_news(world: &World, club: &ClubRef, manager: &str, date: &str) -> Result<NewsItem> {
    let (_, goal, division) = board::expectation(world, club.id)?;
    let league = division.map(|d| format!(" no {d}")).unwrap_or_default();
    Ok(NewsItem {
        id: 1,
        date: date.into(),
        kind: "message".into(),
        title: format!("Bem-vindo ao {}", club.short_name),
        body: format!(
            "A Direcção do {} dá as boas-vindas a {manager} como novo treinador.\n\n\
             Para esta época, a Direcção espera que a equipa consiga {goal}{league}.",
            club.name
        ),
        read: false,
        offer_id: None,
        link: None,
    })
}

/// Apaga um jogo gravado de `dir` (só pelo nome do arquivo, nunca um caminho qualquer).
pub fn delete_save(dir: &Path, file: &str) -> Result<()> {
    let name = Path::new(file).file_name().ok_or_else(|| Error::Invalid("Arquivo inválido.".into()))?;
    let path = dir.join(name);
    if path.extension().and_then(|e| e.to_str()) != Some("sqlite") || !path.exists() {
        return Err(Error::Invalid("Esse jogo gravado não existe.".into()));
    }
    std::fs::remove_file(&path)?;
    Ok(())
}

/// Jogos gravados em `dir`, do mais recente para o mais antigo. Arquivos que não são
/// saves do NextCM são ignorados.
pub fn list_saves(dir: &Path) -> Result<Vec<SaveSummary>> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(out) };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("sqlite") {
            continue;
        }
        let Ok(world) = World::open(&path) else { continue };
        let row = world.conn.query_row(
            "SELECT g.name, p.display_name, c.name, g.date FROM game g
             LEFT JOIN people p ON p.id = g.manager_id LEFT JOIN clubs c ON c.id = g.club_id",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, String>(3)?)),
        );
        let Ok((name, manager, club, date)) = row else { continue };
        let saved_at = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        out.push(SaveSummary {
            file: path.file_name().unwrap().to_string_lossy().into(),
            name,
            manager_name: manager.unwrap_or_default(),
            club_name: club.unwrap_or_default(),
            date,
            saved_at,
        });
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.saved_at));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::TeamStrength;
    use crate::league::SERIE_A;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nextcm-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn new_game(club_id: i32) -> NewGame {
        NewGame {
            save_name: "Teste".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: Some("1990-05-10".into()),
            nation_id: None,
            club_id,
        }
    }

    #[test]
    fn file_stem_is_safe() {
        assert_eq!(file_stem(" Minha época/1 ").unwrap(), "Minha época_1");
        assert!(file_stem("   ").is_err());
    }

    #[test]
    fn create_advance_save_and_load() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("game");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 42).unwrap();

        let info = g.info().unwrap();
        assert_eq!(info.date, "2025-07-01");
        assert_eq!(info.manager_name, "Victor Araujo");
        assert_eq!(info.club.short_name, "Palmeiras");
        // Boas-vindas e calendário da época de transição
        assert_eq!(info.unread_news, 2);
        assert_eq!(g.news()[0].title, "Calendário 2025");
        assert!(g.news()[1].body.contains("Victor Araujo"));

        // O treinador está no plantel técnico e o Abel Ferreira saiu
        let club = g.world().get_club(palmeiras).unwrap();
        let coaches: Vec<_> = club.staff.iter().filter(|s| s.job_id == Some(HEAD_COACH)).map(|s| s.list_name.clone()).collect();
        assert_eq!(coaches, ["Araujo, Victor"]);

        // Nome repetido é recusado
        assert!(matches!(Game::create(&world, &dir, &new_game(palmeiras), 1), Err(Error::Invalid(_))));

        for _ in 0..62 {
            g.advance_day().unwrap();
        }
        assert_eq!(g.date(), "2025-09-01");
        // Transição: em agosto há 9 jornadas (5 fins de semana e 4 quartas); uma notícia por jogo do Palmeiras
        let mine = g.club_fixtures(palmeiras);
        assert_eq!(mine.len(), 38);
        assert_eq!(mine.iter().filter(|f| f.home_goals.is_some()).count(), 9);
        // 2 iniciais + 9 resultados (+ lesões/suspensões do Palmeiras, que variam com a seed)
        let results = g.news().iter().filter(|n| n.kind == "competition" && n.title.contains('-')).count();
        assert_eq!(results, 9);
        assert!(g.info().unwrap().unread_news >= 11);
        let ids: Vec<i32> = g.news().iter().map(|n| n.id).collect();
        for id in ids {
            g.mark_news_read(id);
        }
        assert!(g.info().unwrap().unsaved);
        let table = g.table(SERIE_A);
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);

        let g = Game::load(&path).unwrap();
        let info = g.info().unwrap();
        assert_eq!(info.date, "2025-09-01");
        assert_eq!(info.unread_news, 0);
        assert!(!info.unsaved);
        // Resultados e classificação voltam iguais do save
        let reloaded = g.table(SERIE_A);
        assert_eq!(reloaded.len(), 20);
        let key = |t: &[StandingRow]| t.iter().map(|r| (r.club.id, r.points, r.goals_for)).collect::<Vec<_>>();
        assert_eq!(key(&table), key(&reloaded));

        let saves = list_saves(&dir).unwrap();
        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].club_name, "Sociedade Esportiva Palmeiras");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Joga a Série A inteira; devolve (resultados por id da partida, classificação).
    fn full_season(world: &Path, name: &str, seed: i64) -> (Vec<(i32, Option<i32>, Option<i32>)>, Vec<StandingRow>) {
        let dir = temp_dir(name);
        let mut g = Game::create(world, &dir, &new_game(7568), seed).unwrap();
        // Continuar Jogo pára na véspera do 1º jogo do Palmeiras e, a seguir, depois dele
        let first = g.club_fixtures(7568)[0].clone();
        // (as propostas dos outros clubes pelos jogadores também param)
        while date::add_days(g.date(), 1).unwrap() < first.date {
            let news = g.advance_to_next_event().unwrap();
            assert!(news.iter().all(|n| n.offer_id.is_some() || n.kind == "transfer"), "{}", news[0].title);
        }
        assert_eq!(date::add_days(g.date(), 1).unwrap(), first.date);
        let news = g.advance_to_next_event().unwrap();
        assert_eq!(g.date(), first.date);
        assert_eq!(news.len(), 1);
        assert!(news[0].body.starts_with("Série A, 1ª jornada."), "{}", news[0].body);
        let mut days = 0;
        while g.fixtures(SERIE_A).iter().any(|f| f.home_goals.is_none()) {
            g.advance_to_next_event().unwrap();
            days += 1;
            assert!(days < 400, "a época não acaba");
        }
        let results = g.fixtures(SERIE_A).iter().map(|f| (f.id, f.home_goals, f.away_goals)).collect();
        let table = g.table(SERIE_A);
        // O campeão da A e o da B saem no mesmo dia
        assert!(g.news().iter().take(3).any(|n| n.title.ends_with("campeão da Série A")), "{}", g.news()[0].title);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
        (results, table)
    }

    #[test]
    fn full_season_is_deterministic_and_coherent() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let (results, table) = full_season(&world, "season-a", 99);
        let (again, _) = full_season(&world, "season-b", 99);
        assert_eq!(results, again, "mesma seed, mesmos resultados");

        assert_eq!(results.len(), 380);
        assert_eq!(table.len(), 20);
        assert!(table.iter().all(|r| r.played == 38 && r.won + r.drawn + r.lost == 38));
        // Pontos: 3 por jogo com vencedor, 2 por empate
        let draws = results.iter().filter(|r| r.1 == r.2).count() as i32;
        let points: i32 = table.iter().map(|r| r.points).sum();
        assert_eq!(points, 3 * (380 - draws) + 2 * draws);
        let goals: i32 = results.iter().map(|r| r.1.unwrap() + r.2.unwrap()).sum();
        assert_eq!(goals, table.iter().map(|r| r.goals_for).sum::<i32>());
        assert_eq!(goals, table.iter().map(|r| r.goals_against).sum::<i32>());
        // Números plausíveis: 2,0–3,2 golos por jogo e a tabela ordenada por pontos
        let per_game = goals as f64 / 380.0;
        assert!((2.0..3.2).contains(&per_game), "{per_game:.2} golos por jogo");
        assert!(table.windows(2).all(|w| w[0].points >= w[1].points));
    }

    #[test]
    fn selection_is_saved_and_changes_strength() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("selection");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 7).unwrap();

        // O adjunto escolhe; a tela recebe a táctica editável
        let auto = g.club_tactics(palmeiras).unwrap();
        assert!(auto.editable && auto.auto);
        assert!(!g.club_tactics(3675).unwrap().editable, "Flamengo não é do treinador");

        // Onze de reservas: troca os 11 titulares pelos 11 piores do plantel
        let mut sel = auto.selection();
        sel.auto = false;
        let reserves: Vec<i32> = auto.reserves.iter().rev().map(|p| p.id).collect();
        let mut pool: Vec<i32> = reserves.into_iter().chain(auto.bench.iter().rev().map(|p| p.id)).collect();
        pool.truncate(11);
        sel.starters = pool.iter().map(|&id| Some(id)).collect();
        sel.bench.retain(|id| !pool.contains(id));
        sel.instructions.mentality = Mentality::Attacking;
        let weak = g.set_selection(sel.clone()).unwrap();
        assert!(!weak.auto);
        let (a, w) = (TeamStrength::from_tactics(&auto), TeamStrength::from_tactics(&weak));
        assert!(w.attack + w.midfield + w.defence < 0.9 * (a.attack + a.midfield + a.defence), "{w:?} vs {a:?}");

        // Escolha inválida é recusada
        let mut bad = sel.clone();
        bad.starters[1] = bad.starters[0];
        assert!(matches!(g.set_selection(bad), Err(Error::Invalid(_))));

        // Gravar e carregar mantém a escolha e as instruções
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&path).unwrap();
        assert_eq!(g.selection.as_ref(), Some(&sel));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn assistant_covers_injured_starter() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("cover");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 9).unwrap();
        let mut sel = g.club_tactics(palmeiras).unwrap().selection();
        sel.auto = false;
        g.set_selection(sel.clone()).unwrap();

        // O guarda-redes titular lesiona-se antes do jogo
        let gk = sel.starters[0].unwrap();
        let st = g.squads.states.entry(gk).or_default();
        st.injury = Some("Entorse no tornozelo".into());
        st.injury_until = Some("2025-12-31".into());
        let (t, changes) = g.lineup(palmeiras, SERIE_A).unwrap();
        let changes = changes.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].0, Some(gk));
        let new_gk = t.starters[0].player.as_ref().unwrap();
        assert_ne!(new_gk.id, gk);
        assert!(new_gk.fit.unwrap() >= 15, "entra outro GR");
        // A escolha gravada não muda: o titular volta quando estiver bom
        assert_eq!(g.selection.as_ref().unwrap().starters[0], Some(gk));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn starters_get_tired_and_keep_their_place() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("tired");
        let botafogo = g_club(&world, "Botafogo");
        let mut g = Game::create(&world, &dir, &new_game(botafogo), 11).unwrap();
        let ids = |t: &ClubTactics| t.starters.iter().filter_map(|s| s.player.as_ref().map(|p| p.id)).collect::<Vec<_>>();
        let before = ids(&g.club_tactics(botafogo).unwrap());
        // Até ao dia do 1º jogo do Botafogo
        while g.club_fixtures(botafogo).iter().all(|f| f.home_goals.is_none()) {
            g.advance_day().unwrap();
        }
        let after = g.club_tactics(botafogo).unwrap();
        // Quem jogou (sem lesão/suspensão) cansou; os suplentes não
        for s in &after.starters {
            let p = s.player.as_ref().unwrap();
            if before.contains(&p.id) && !p.injured && !p.suspended {
                // Jogou pelo menos uma parte (o adjunto pode tê-lo trocado ao intervalo)
                assert!(p.condition.unwrap() < 92, "{} jogou e está a {:?}", p.list_name, p.condition);
            }
        }
        // Os suplentes não jogaram ou entraram já na segunda parte
        for p in &after.bench {
            assert!(p.condition >= Some(85), "{} está a {:?}", p.list_name, p.condition);
        }
        // O adjunto mantém o onze (só troca lesionados/suspensos)
        let kept = ids(&after).iter().filter(|id| before.contains(id)).count();
        assert!(kept >= 9, "o onze mudou demais: {kept} de 11");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Avança até o jogo do Palmeiras começar (com `watch`) e devolve o jogo.
    /// `manual`: o treinador escolhe a equipa (o adjunto não faz substituições tácticas).
    fn until_kickoff(world: &Path, name: &str, seed: i64, watch: bool, manual: bool) -> (Game, PathBuf) {
        let dir = temp_dir(name);
        let mut g = Game::create(world, &dir, &new_game(7568), seed).unwrap();
        g.set_watch(watch);
        if manual {
            let mut sel = g.club_tactics(7568).unwrap().selection();
            sel.auto = false;
            g.set_selection(sel).unwrap();
        }
        let first = g.club_fixtures(7568)[0].date.clone();
        while g.date() < first.as_str() {
            g.advance_to_next_event().unwrap();
        }
        (g, dir)
    }

    #[test]
    fn watched_match_is_the_same_match() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let (mut quick, d1) = until_kickoff(&world, "live-quick", 5, false, false);
        let (mut g, d2) = until_kickoff(&world, "live-watch", 5, true, false);
        assert!(quick.live.is_none());
        assert!(g.info().unwrap().live, "o jogo do treinador fica por jogar");
        assert!(matches!(g.save(), Err(Error::Invalid(_))), "não grava a meio do jogo");
        // Os resultados do dia só aparecem no fim
        let today = g.date().to_string();
        assert!(g.fixtures(SERIE_A).iter().filter(|f| f.date == today).all(|f| f.home_goals.is_none()));

        let v = g.live_view().unwrap();
        assert_eq!((v.minute, v.home.players.len(), v.away.players.len()), (0, 11, 11));
        assert_eq!(v.others.len(), g.fixtures(SERIE_A).iter().filter(|f| f.date == today).count() - 1);
        let half = g.live_step(45).unwrap();
        assert!(half.half_time && !half.finished);
        assert!(half.events.iter().any(|e| e.kind == "half"));
        let end = g.live_step(200).unwrap();
        assert!(end.finished && end.minute == 90);
        let shots = end.home.stats.shots + end.away.stats.shots;
        assert!(shots > 5, "{shots} remates");
        assert_eq!(end.home.stats.possession + end.away.stats.possession, 100);
        let goals = end.events.iter().filter(|e| e.kind == "goal").count() as i32;
        assert_eq!(goals, end.home_goals + end.away_goals);
        let news = g.live_finish().unwrap();
        assert!(!g.info().unwrap().live);

        // Sem mexer na equipa, é o mesmo jogo que sem ver
        let key = |g: &Game| g.fixtures(SERIE_A).iter().map(|f| (f.id, f.home_goals, f.away_goals)).collect::<Vec<_>>();
        assert_eq!(key(&g), key(&quick));
        assert_eq!(g.date(), quick.date());
        let titles = |n: &[NewsItem]| n.iter().map(|n| n.title.clone()).collect::<Vec<_>>();
        assert_eq!(titles(&news), titles(&quick.news()[..news.len()].iter().rev().cloned().collect::<Vec<_>>()));
        let body = &news.iter().find(|n| n.kind == "competition").unwrap().body;
        if goals > 0 {
            assert!(body.contains("Marcadores:"), "{body}");
        }
        g.save().unwrap();
        quick.save().unwrap();
        drop((g, quick));
        let _ = std::fs::remove_dir_all(&d1);
        let _ = std::fs::remove_dir_all(&d2);
    }

    #[test]
    fn manager_changes_the_team_at_half_time() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let (mut g, dir) = until_kickoff(&world, "live-subs", 8, true, true);
        let v = g.live_step(45).unwrap();
        let side = v.user_side.unwrap();
        let ours = if side == 0 { &v.home } else { &v.away };
        let (out, inn) = (ours.players[10].id, ours.bench.iter().find(|p| p.position != "GR").unwrap().id);
        assert!(g.live_substitute(out, out).is_err(), "quem entra tem de estar no banco");
        let v = g.live_substitute(out, inn).unwrap();
        let ours = if side == 0 { &v.home } else { &v.away };
        assert!(ours.players.iter().any(|p| p.id == inn && p.on_minute == Some(45)));
        assert!(ours.out.iter().any(|p| p.id == out && p.off_minute == Some(45)));
        assert_eq!(ours.subs_left, MAX_SUBS_TEST - 1);
        assert!(g.live_substitute(out, inn).is_err(), "já saiu");
        let mut i = ours.instructions;
        i.mentality = Mentality::Attacking;
        let v = g.live_instructions(i).unwrap();
        assert!(v.events.last().unwrap().kind == "tactics");
        let (a, b) = (ours.players[1].id, ours.players[2].id);
        g.live_swap(a, b).unwrap();
        let v = g.live_step(90).unwrap();
        let ours = if side == 0 { &v.home } else { &v.away };
        let sub = ours.players.iter().chain(&ours.out).find(|p| p.id == inn).unwrap();
        assert!(sub.condition < 100, "{:?}", sub);
        assert!(g.live_substitute(a, b).is_err(), "o jogo acabou");
        g.live_finish().unwrap();
        // Quem entrou jogou (cansou-se um pouco); quem saiu também conta como tendo jogado
        assert!(g.squads.state(inn).condition < 100);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    const MAX_SUBS_TEST: i32 = crate::live::MAX_SUBS;

    #[test]
    fn tactics_screen_during_the_match() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let (mut g, dir) = until_kickoff(&world, "live-tactics", 12, true, true);
        let side = g.live_view().unwrap().user_side.unwrap();
        let t = g.live_tactics(side).unwrap();
        assert!(t.editable && t.reserves.is_empty());
        assert!(!g.live_tactics(1 - side).unwrap().editable, "a do adversário só se vê");

        // Antes do apito: trocar um titular por um suplente não gasta substituições
        let mut sel = t.selection();
        let (starter, sub) = (sel.starters[10].unwrap(), sel.bench[1]);
        sel.starters[10] = Some(sub);
        sel.bench[1] = starter;
        g.live_set_tactics(&sel).unwrap();
        let v = g.live_view().unwrap();
        let ours = if side == 0 { &v.home } else { &v.away };
        assert_eq!(ours.subs_left, MAX_SUBS_TEST);
        assert!(ours.bench.iter().any(|p| p.id == starter), "volta ao banco");

        // Ao intervalo: outra formação (o preview mostra as funções novas) e uma substituição
        g.live_step(45).unwrap();
        let mut sel = g.live_tactics(side).unwrap().selection();
        sel.formation = 4; // 4-3-3
        sel.positions.clear();
        let preview = g.live_preview_tactics(&sel).unwrap();
        assert_eq!(preview.formation, "4-3-3");
        let (out, inn) = (sel.starters[5].unwrap(), sel.bench[0]);
        sel.starters[5] = Some(inn);
        sel.bench.remove(0);
        let v = g.live_set_tactics(&sel).unwrap();
        let ours = if side == 0 { &v.home } else { &v.away };
        assert_eq!(ours.formation, "4-3-3");
        assert_eq!(ours.subs_left, MAX_SUBS_TEST - 1);
        assert!(ours.out.iter().any(|p| p.id == out));
        assert!(v.events.iter().any(|e| e.kind == "tactics"));

        // Quem saiu não volta
        let mut back = g.live_tactics(side).unwrap().selection();
        back.starters[0] = Some(out);
        assert!(g.live_preview_tactics(&back).is_err());
        assert!(g.live_substitute(back.starters[1].unwrap(), out).is_err());

        // Só 5 substituições: com 4 feitas, trocar 2 de uma vez é recusado e 1 passa
        g.live_step(60).unwrap();
        for k in 0..3 {
            let t = g.live_tactics(side).unwrap();
            let sel = t.selection();
            let inn = *sel.bench.iter().find(|&&b| t.bench.iter().any(|p| p.id == b && p.position != "GR")).unwrap();
            g.live_substitute(sel.starters[9 - k].unwrap(), inn).unwrap();
        }
        let t = g.live_tactics(side).unwrap();
        let mut two = t.selection();
        assert!(two.bench.len() >= 2);
        two.starters[2] = Some(two.bench.remove(0));
        two.starters[3] = Some(two.bench.remove(0));
        let err = g.live_preview_tactics(&two).unwrap_err().to_string();
        assert!(err.contains("5 substituições"), "{err}");
        let mut one = t.selection();
        one.starters[2] = Some(one.bench.remove(0));
        g.live_set_tactics(&one).unwrap();
        let v = g.live_view().unwrap();
        assert_eq!(if side == 0 { &v.home } else { &v.away }.subs_left, 0);
        let last = g.live_tactics(side).unwrap().selection();
        assert!(g.live_substitute(last.starters[4].unwrap(), last.bench[0]).is_err(), "a 6ª é recusada");
        let end = g.live_step(90).unwrap();
        assert_eq!(end.events.iter().filter(|e| e.kind == "goal").count() as i32, end.home_goals + end.away_goals);
        g.live_finish().unwrap();
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn g_club(world: &Path, short: &str) -> i32 {
        let w = World::open(world).unwrap();
        w.conn.query_row("SELECT id FROM clubs WHERE short_name = ? AND division_id = 65", [short], |r| r.get(0)).unwrap()
    }

    #[test]
    fn toggle_and_unique_numbers() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("toggle");
        let palmeiras = 7568;
        let g = Game::create(&world, &dir, &new_game(palmeiras), 3).unwrap();

        // Números de camisola sem repetidos no plantel
        let club = g.world().get_club(palmeiras).unwrap();
        assert!(club.players.iter().all(|p| p.squad_number.is_some()), "todos com número");
        let mut numbers: Vec<i32> = club.players.iter().filter_map(|p| p.squad_number).collect();
        let total = numbers.len();
        numbers.sort_unstable();
        numbers.dedup();
        assert_eq!(numbers.len(), total, "números repetidos");

        // Tirar o GR deixa a função vazia; pôr de volta um GR reserva ocupa essa função
        let t = g.club_tactics(palmeiras).unwrap();
        let sel = t.selection();
        let gk = sel.starters[0].unwrap();
        let out = g.toggle_player(&sel, gk).unwrap();
        assert!(out.starters[0].player.is_none() && !out.auto);
        let reserve_gk = t.reserves.iter().find(|p| p.position == "GR").unwrap().id;
        let back = g.toggle_player(&out.selection(), reserve_gk).unwrap();
        assert_eq!(back.starters[0].player.as_ref().unwrap().id, reserve_gk);
        // Com o onze completo, quem entra vai para o banco (se houver lugar)
        let mut full = back.selection();
        full.bench.truncate(6);
        let outsider = back.reserves.iter().find(|p| !full.bench.contains(&p.id)).unwrap().id;
        let benched = g.toggle_player(&full, outsider).unwrap();
        assert!(benched.bench.iter().any(|p| p.id == outsider));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn free_positions_change_roles_and_order() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("positions");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 5).unwrap();
        let t = g.club_tactics(palmeiras).unwrap();
        // 4-4-2 na ordem do CM: o último é um avançado (A C)
        assert_eq!(t.starters[10].role, "A C");
        let striker = t.starters[10].player.as_ref().unwrap().id;
        // Arrastar o avançado para o meio-campo: vira médio e a lista reordena
        let mut sel = t.selection();
        sel.positions = t.starters.iter().map(|s| [s.x, s.y]).collect();
        sel.positions[10] = [50, 45];
        sel.auto = false;
        let moved = g.set_selection(sel).unwrap();
        assert!(moved.custom);
        let at = moved.starters.iter().position(|s| s.player.as_ref().is_some_and(|p| p.id == striker)).unwrap();
        assert_eq!(moved.starters[at].role, "M C");
        let roles: Vec<&str> = moved.starters.iter().map(|s| s.role.as_str()).collect();
        assert_eq!(roles.iter().filter(|r| **r == "A C").count(), 1);
        assert!(at < 10, "a lista segue a ordem das funções");
        // Gravar e carregar mantém as posições
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&path).unwrap();
        let again = g.club_tactics(palmeiras).unwrap();
        assert!(again.custom);
        assert_eq!(again.starters[at].role, "M C");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn match_preview_has_venue_and_is_stable() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("preview");
        let palmeiras = 7568;
        let g = Game::create(&world, &dir, &new_game(palmeiras), 6).unwrap();
        let first = g.club_fixtures(palmeiras)[0].clone();
        let p = g.match_preview(first.id).unwrap();
        assert!(p.venue.is_some() && p.city.is_some());
        assert!(p.last_meeting.is_none(), "1ª jornada: ainda não se defrontaram");
        assert_eq!(p.weather, g.match_preview(first.id).unwrap().weather, "o tempo não muda ao reabrir");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn runs_are_saved_and_count_in_the_engine() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("runs");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 4).unwrap();
        let t = g.club_tactics(palmeiras).unwrap();
        let mut sel = t.selection();
        sel.auto = false;
        // Os dois laterais (2 e 3 na ordem do CM) sobem pelo corredor
        sel.runs = vec![None; 11];
        for i in [1, 2] {
            sel.runs[i] = Some([t.starters[i].x, t.starters[i].y - 30]);
        }
        let with = g.set_selection(sel.clone()).unwrap();
        assert_eq!(with.starters[1].run, sel.runs[1]);
        let (a, b) = (TeamStrength::from_tactics(&t), TeamStrength::from_tactics(&with));
        assert!(b.attack > a.attack && b.defence < a.defence);
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&path).unwrap();
        assert_eq!(g.club_tactics(palmeiras).unwrap().starters[2].run, sel.runs[2]);
        drop(g);
        assert!(delete_save(&dir, "../world.sqlite").is_err());
        delete_save(&dir, path.file_name().unwrap().to_str().unwrap()).unwrap();
        assert!(list_saves(&dir).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_3_saves_are_migrated() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = temp_dir("migrate");
        let palmeiras = 7568;
        let mut g = Game::create(&world, &dir, &new_game(palmeiras), 8).unwrap();
        let mut sel = g.club_tactics(palmeiras).unwrap().selection();
        sel.auto = false;
        let expected = g.set_selection(sel.clone()).unwrap();
        g.save().unwrap();
        let path = g.path().to_path_buf();
        drop(g);

        // Volta o arquivo ao formato 3: sem x/y e titulares na ordem antiga da formação
        {
            let w = World::open_save(&path).unwrap();
            let perm = crate::tactics::cm_order_permutation(sel.formation);
            let mut old = [None; 11];
            for (k, &from) in perm.iter().enumerate() {
                old[from] = sel.starters[k];
            }
            w.conn.execute("DELETE FROM selection_players WHERE slot <= 10", []).unwrap();
            w.conn
                .execute_batch("ALTER TABLE selection_players DROP COLUMN x; ALTER TABLE selection_players DROP COLUMN y; ALTER TABLE selection_players DROP COLUMN run_x; ALTER TABLE selection_players DROP COLUMN run_y; ALTER TABLE player_state DROP COLUMN missed; ALTER TABLE player_state DROP COLUMN apps; DROP TABLE unhappy; DROP TABLE knowledge; DROP TABLE scouting; DROP TABLE scout_tasks; DROP TABLE scout_finds; DROP TABLE retired; DROP TABLE cards; ALTER TABLE fixtures DROP COLUMN stage; ALTER TABLE fixtures DROP COLUMN grp; ALTER TABLE fixtures DROP COLUMN tie; ALTER TABLE fixtures DROP COLUMN leg; ALTER TABLE fixtures DROP COLUMN neutral; ALTER TABLE fixtures DROP COLUMN home_pens; ALTER TABLE fixtures DROP COLUMN away_pens; DROP TABLE finances; DROP TABLE club_economy; DROP TABLE transfers; DROP TABLE loans; DROP TABLE offers; DROP TABLE transfer_list; DROP TABLE talks; DROP TABLE squad_roles; DROP TABLE board; ALTER TABLE news DROP COLUMN offer_id; ALTER TABLE news DROP COLUMN link; ALTER TABLE players DROP COLUMN preferred_number; UPDATE game SET version = 3;")
                .unwrap();
            for (slot, pid) in old.iter().enumerate() {
                w.conn
                    .execute(
                        "INSERT INTO selection_players (club_id, slot, person_id) VALUES (?, ?, ?)",
                        params![palmeiras, slot as i32, pid],
                    )
                    .unwrap();
            }
        }

        let g = Game::load(&path).unwrap();
        let t = g.club_tactics(palmeiras).unwrap();
        let ids = |t: &ClubTactics| t.starters.iter().map(|s| (s.role.clone(), s.player.as_ref().map(|p| p.id))).collect::<Vec<_>>();
        assert_eq!(ids(&t), ids(&expected), "cada jogador fica na sua função");
        // Sem jogos disputados, as competições voltam a ser sorteadas no formato novo
        assert_eq!(g.fixtures(SERIE_A).len(), 380);
        assert!(g.fixtures(SERIE_A).iter().all(|f| f.date.as_str() <= "2025-12-10"));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn world_file_is_not_a_save() {
        let Some(world) = World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        // Abrir o mundo original como jogo tem de falhar sem estragar nada
        let dir = temp_dir("notsave");
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join("world.sqlite");
        std::fs::copy(&world, &copy).unwrap();
        assert!(matches!(Game::load(&copy), Err(Error::Invalid(_))));
        assert!(list_saves(&dir).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
