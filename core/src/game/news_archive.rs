//! O arquivo das notícias: as das temporadas anteriores saem da caixa de mensagens (e da
//! memória) para a tabela `news_archive` do save, na virada da temporada do clube do
//! treinador. Ficam as do último mês, as não lidas e as que têm uma proposta por responder.

use rusqlite::{OptionalExtension, params};

use super::Game;
use crate::Result;
use crate::regulation::SeasonKind;

/// Save 80: as notícias arquivadas (o mesmo formato de `news`; o id pode repetir-se).
pub(super) const NEWS_ARCHIVE_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS news_archive (
    id INTEGER NOT NULL, date TEXT NOT NULL, kind TEXT NOT NULL, title TEXT NOT NULL, body TEXT NOT NULL,
    read INTEGER NOT NULL, offer_id INTEGER, link TEXT, part INTEGER NOT NULL);";

impl Game {
    /// O primeiro dia da temporada em curso do clube do treinador (1/1 ou 1/7).
    fn own_season_start(&self) -> String {
        let kind = if self.sacked() { SeasonKind::Calendar } else { self.kind_of(self.club_id).unwrap_or_default() };
        let year: i32 = self.date[..4].parse().unwrap_or(2025);
        match kind {
            SeasonKind::Calendar => format!("{year}-01-01"),
            _ if &self.date[5..7] >= "07" => format!("{year}-07-01"),
            _ => format!("{}-07-01", year - 1),
        }
    }

    /// Arquiva as notícias de antes do último mês da temporada passada (as lidas e sem proposta
    /// aberta): o fim da temporada continua na caixa.
    pub(super) fn archive_news(&mut self) -> Result<()> {
        let season = self.own_season_start();
        let year: i32 = season[..4].parse().unwrap_or(2025);
        let start = if &season[5..7] == "01" { format!("{}-12-01", year - 1) } else { format!("{year}-06-01") };
        let mut keep = Vec::with_capacity(self.news.len());
        let mut moved = 0;
        for n in std::mem::take(&mut self.news) {
            let open_offer = match n.offer_id {
                Some(o) => self.world.conn.query_row("SELECT 1 FROM offers WHERE id = ?", [o], |_| Ok(())).optional()?.is_some(),
                None => false,
            };
            if n.date >= start || !n.read || open_offer {
                keep.push(n);
                continue;
            }
            self.world.conn.execute(
                "INSERT INTO news_archive (id, date, kind, title, body, read, offer_id, link, part) VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?)",
                params![n.id, n.date, n.kind, n.title, n.body, n.offer_id, n.link, n.part],
            )?;
            self.world.conn.execute("DELETE FROM news WHERE id = ? AND date = ?", params![n.id, n.date])?;
            moved += 1;
        }
        self.news = keep;
        if moved > 0 {
            self.unsaved = true;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::World;
    use crate::game::Game;

    #[test]
    fn old_season_news_go_to_the_archive() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = crate::TempDir::new(format!("nextcm-arquivo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Arquivo".into(),
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
        let mut g = Game::create(&path, &dir, &new, 9).unwrap();
        // Duas da temporada passada: a lida vai para o arquivo, a por ler fica
        let old = |id: i32, read: bool| {
            let mut n = g.news_item(id, "message", format!("Velha {id}"), String::new());
            n.date = "2024-11-10".into();
            n.read = read;
            n
        };
        let (a, b) = (old(9001, true), old(9002, false));
        g.news.extend([a, b]);
        let total = g.news.len();
        g.archive_news().unwrap();
        assert_eq!(g.news.len(), total - 1);
        assert!(g.news.iter().all(|n| n.id != 9001) && g.news.iter().any(|n| n.id == 9002));
        let archived: i64 = g.world.conn.query_row("SELECT COUNT(*) FROM news_archive WHERE id = 9001", [], |r| r.get(0)).unwrap();
        assert_eq!(archived, 1);
        // Gravar e carregar: a arquivada não volta à caixa
        g.save().unwrap();
        let file = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&file).unwrap();
        assert!(g.news.iter().all(|n| n.id != 9001) && g.news.iter().any(|n| n.id == 9002));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
