//! nextcm-core: dados e regras do NextCM, sem dependência do Tauri.
//!
//! Fase 2: só leitura do `world.sqlite` gerado por `scripts/extract.py`.

mod search;
mod world;

pub use search::{PersonKind, PersonSummary, Position, SearchQuery, SearchResult, SearchSort, Side};
pub use world::{NationOption, World};

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    NotFound(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "erro no banco: {e}"),
            Error::NotFound(what) => write!(f, "não encontrado: {what}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
