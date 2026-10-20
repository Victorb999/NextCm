//! nextcm-core: dados e regras do NextCM, sem dependência do Tauri.
//!
//! Fase 2: leitura do `world.sqlite` gerado por `scripts/extract.py`.
//! Fase 3: jogo em curso (save, calendário, match engine).

pub mod attributes;
mod club;
mod competition;
pub mod date;
mod engine;
mod game;
mod league;
mod live;
mod nation;
mod person;
mod search;
mod session;
pub mod squad;
mod tactics;
mod world;

pub use club::{ClubProfile, CompetitionRef, Honour, SquadPlayer, StaffMember};
pub use competition::{ClubListItem, CompetitionListItem, CompetitionProfile, ContinentRef, HistoryEntry};
pub use engine::{TeamStrength, expected_goals};
pub use game::{Game, GameInfo, MatchPreview, MatchRecord, NewGame, NewsItem, SaveSummary, delete_save, list_saves};
pub use league::{Fixture, SERIE_A, StandingRow};
pub use live::{LivePlayer, LiveTeam, LiveView, MAX_SUBS, MatchEvent, ScoreLine, TeamStats};
pub use nation::{NationListItem, NationProfile};
pub use person::{Attributes, CareerEntry, ClubRef, PersonProfile, PlayerDetails, StaffDetails};
pub use search::{PersonKind, PersonSummary, Position, SearchQuery, SearchResult, SearchSort, Side};
pub use session::Session;
pub use squad::PlayerStatus;
pub use tactics::{ClubTactics, FORMATIONS, Instructions, Mentality, Passing, Selection, Tackling, TacticPlayer, TacticSlot};
pub use world::{NationOption, World};

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    NotFound(String),
    Io(std::io::Error),
    /// Pedido inválido (ex.: nome de save repetido), com a mensagem para o utilizador.
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "erro no banco: {e}"),
            Error::NotFound(what) => write!(f, "não encontrado: {what}"),
            Error::Io(e) => write!(f, "erro de arquivo: {e}"),
            Error::Invalid(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
