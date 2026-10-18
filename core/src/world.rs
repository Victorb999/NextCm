use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use ts_rs::TS;

use crate::{Error, Result};

/// Conexão somente-leitura com o `world.sqlite`.
pub struct World {
    pub(crate) conn: Connection,
    /// Ano em que o jogo começa (2001 no CM 01/02 + year_offset do patch).
    pub(crate) start_year: i32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NationOption {
    pub id: i32,
    pub name: String,
}

impl World {
    pub fn open(path: &Path) -> Result<Self> {
        Self::open_with(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    }

    /// Abre um save (cópia do mundo) para leitura e escrita.
    pub(crate) fn open_save(path: &Path) -> Result<Self> {
        Self::open_with(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
    }

    fn open_with(path: &Path, mode: OpenFlags) -> Result<Self> {
        if !path.exists() {
            return Err(Error::NotFound(path.display().to_string()));
        }
        let conn = Connection::open_with_flags(path, mode | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        // O banco inteiro (~70 MB) cabe na memória: buscas com filtro ficam ~10x mais rápidas.
        conn.execute_batch("PRAGMA cache_size = -131072; PRAGMA mmap_size = 268435456;")?;
        let year_offset: i32 = conn
            .query_row("SELECT value FROM meta WHERE key = 'year_offset'", [], |r| {
                r.get::<_, String>(0)
            })
            .map(|v| v.parse().unwrap_or(0))?;
        Ok(World { conn, start_year: 2001 + year_offset })
    }

    /// Procura `data/world.sqlite` subindo a partir de `start` (útil em dev,
    /// onde o executável roda de `target/debug`).
    pub fn locate(start: &Path) -> Option<PathBuf> {
        start
            .ancestors()
            .map(|dir| dir.join("data").join("world.sqlite"))
            .find(|p| p.exists())
    }

    pub fn start_year(&self) -> i32 {
        self.start_year
    }

    /// Nações que têm pelo menos uma pessoa, para o filtro da busca.
    pub fn list_nations(&self) -> Result<Vec<NationOption>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT n.id, n.name FROM nations n
             WHERE EXISTS (SELECT 1 FROM people p WHERE p.nation_id = n.id)
             ORDER BY n.name",
        )?;
        let rows = stmt
            .query_map([], |r| Ok(NationOption { id: r.get(0)?, name: r.get(1)? }))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }
}
