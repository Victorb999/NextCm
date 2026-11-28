//! O jogador que queria sair e o clube não deixou (4.0): quando o clube recusa uma proposta a
//! sério por quem aceitaria ir, ele pode ficar insatisfeito e passa a pedir para sair durante
//! `BLOCKED_DAYS` (como o "pediu para sair": o clube pede 15% menos e vende mais vezes). As
//! mesmas contas nas propostas do treinador e nas da IA; sorteado uma vez por jogador e janela.

use rusqlite::{OptionalExtension, params};
use std::collections::HashSet;

use super::{Game, NewsItem};
use crate::engine::uniform;
use crate::{Result, date};

/// Save 82: quem ficou frustrado, no clube que o segurou, e até quando.
pub(super) const BLOCKED_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS blocked_moves (
    person_id INTEGER PRIMARY KEY, club_id INTEGER NOT NULL, buyer INTEGER NOT NULL,
    until TEXT NOT NULL, told INTEGER NOT NULL DEFAULT 0);";

/// Quanto dura a frustração (uns 6 meses: até à janela seguinte).
const BLOCKED_DAYS: i32 = 180;

/// A chance de ficar frustrado: 25% se o clube que o queria é maior (reputação), 10% se não;
/// +25% para o ambicioso (15+); metade para o leal (15+).
pub(super) fn blocked_chance(bigger: bool, ambition: f64, loyalty: f64) -> f64 {
    let mut chance = if bigger { 0.25 } else { 0.10 };
    if ambition >= 15.0 {
        chance += 0.25;
    }
    if loyalty >= 15.0 {
        chance *= 0.5;
    }
    chance
}

impl Game {
    /// O `seller` recusou vender `person` ao `buyer`: ele fica frustrado? Só a primeira vez na
    /// janela conta (o sorteio é fixo por jogador e janela). O plantel do treinador tem a sua
    /// própria insatisfação.
    pub(super) fn block_move(&self, person: i32, seller: i32, buyer: i32, ambition: f64, loyalty: f64) -> Result<bool> {
        if seller == self.club_id || self.frustrated(person, seller)? {
            return Ok(false);
        }
        let bigger = self.club_rep(buyer)? > self.club_rep(seller)?;
        let window = self.date[0..4].parse::<u64>().unwrap_or(0) * 2 + u64::from(&self.date[5..7] >= "07");
        if uniform(&mut self.deal_rng(person, 0xB10C_0000 ^ window)) >= blocked_chance(bigger, ambition, loyalty) {
            return Ok(false);
        }
        let until = date::add_days(&self.date, BLOCKED_DAYS).unwrap_or_default();
        self.world.conn.execute(
            "INSERT OR REPLACE INTO blocked_moves (person_id, club_id, buyer, until, told) VALUES (?, ?, ?, ?, 0)",
            params![person, seller, buyer, until],
        )?;
        Ok(true)
    }

    /// Está frustrado no `club` (ainda lá e dentro do prazo)?
    pub(super) fn frustrated(&self, person: i32, club: i32) -> Result<bool> {
        Ok(self
            .world
            .conn
            .query_row(
                "SELECT 1 FROM blocked_moves b JOIN people p ON p.id = b.person_id AND p.club_id = b.club_id
                 WHERE b.person_id = ? AND b.club_id = ? AND b.until >= ?",
                params![person, club, self.date],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    /// Os frustrados que ainda estão no clube que os segurou (de um clube, ou de todos com 0).
    pub(super) fn frustrated_ids(&self, club: i32) -> Result<HashSet<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached(
                "SELECT b.person_id FROM blocked_moves b JOIN people p ON p.id = b.person_id AND p.club_id = b.club_id
                 WHERE b.until >= ?1 AND (?2 = 0 OR b.club_id = ?2)",
            )?
            .query_map(params![self.date, club], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// A notícia dos frustrados da Lista Preferencial (os da IA; os do treinador vêm na resposta
    /// à proposta) e a limpeza dos prazos acabados.
    pub(super) fn blocked_news(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        self.world.conn.execute("DELETE FROM blocked_moves WHERE until < ?", [&self.date])?;
        let new: Vec<(i32, i32, i32)> = self
            .world
            .conn
            .prepare_cached("SELECT person_id, club_id, buyer FROM blocked_moves WHERE told = 0 ORDER BY person_id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        if new.is_empty() {
            return Ok(());
        }
        self.world.conn.execute("UPDATE blocked_moves SET told = 1 WHERE told = 0", [])?;
        if !self.shortlist_sub()? {
            return Ok(());
        }
        for (person, club, buyer) in new {
            if !self.in_shortlist(person)? {
                continue;
            }
            let name = self.person_name(person)?;
            let (club, buyer) = (self.club(club).short_name, self.club(buyer).short_name);
            let body = format!(
                "{name} ficou insatisfeito porque o {club} recusou vendê-lo ao {buyer}. Deve pedir para sair, e o clube tende a facilitar a venda.\n\n{name} está na sua Lista Preferencial."
            );
            let mut n = self.news_item(*next_id, "transfer", format!("{name} insatisfeito no {club}"), body);
            n.quiet = !self.news_stops(super::news_subs::SHORTLIST)?;
            fresh.push(n);
            *next_id += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::blocked_chance;
    use crate::World;
    use crate::game::Game;

    #[test]
    fn a_blocked_player_wants_out() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = crate::TempDir::new(format!("nextcm-bloqueado-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "bloqueado".into(),
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
        let mut g = Game::create(&path, &dir, &new, 3).unwrap();
        g.prestart = false;
        const FLAMENGO: i32 = 3675;
        let star: i32 = g
            .world
            .conn
            .query_row(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1
                 ORDER BY pl.current_ability DESC LIMIT 1",
                [FLAMENGO],
                |r| r.get(0),
            )
            .unwrap();
        let f = g.facts(star).unwrap().unwrap();
        assert!(!g.sale_view(star, &f, FLAMENGO).unwrap().unhappy);
        // Sorteia até ficar frustrado (o sorteio é por janela: muda a data de ano em ano)
        let start = g.date.clone();
        let mut year = 0;
        while !g.block_move(star, FLAMENGO, 7568, 20.0, 1.0).unwrap() {
            year += 1;
            assert!(year < 40, "nunca fica frustrado");
            g.date = format!("{}{}", start[..4].parse::<i32>().unwrap() + year, &start[4..]);
        }
        assert!(!g.block_move(star, FLAMENGO, 7568, 20.0, 1.0).unwrap(), "já frustrado: não conta de novo");
        // Pede para sair: o clube pede menos e a IA o lista como quem pediu
        assert!(g.sale_view(star, &f, FLAMENGO).unwrap().unhappy);
        let upset = g.asking_price(star, &f, FLAMENGO).unwrap();
        g.world.conn.execute("UPDATE blocked_moves SET until = '2000-01-01'", []).unwrap();
        assert!(upset < g.asking_price(star, &f, FLAMENGO).unwrap());
        g.world.conn.execute("UPDATE blocked_moves SET until = '2100-01-01'", []).unwrap();
        assert!(g.frustrated_ids(FLAMENGO).unwrap().contains(&star));
        // A notícia só para a Lista Preferencial, uma vez
        g.set_shortlist(star, true).unwrap();
        let (mut fresh, mut id) = (Vec::new(), 1);
        g.blocked_news(&mut fresh, &mut id).unwrap();
        assert!(fresh.iter().any(|n| n.title.contains("insatisfeito")), "{fresh:?}");
        let n = fresh.len();
        g.blocked_news(&mut fresh, &mut id).unwrap();
        assert_eq!(fresh.len(), n);
        // Mudou de clube: a frustração acaba
        g.world.conn.execute("UPDATE people SET club_id = 7568 WHERE id = ?", [star]).unwrap();
        assert!(!g.frustrated(star, FLAMENGO).unwrap() && g.frustrated_ids(0).unwrap().is_empty());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ambition_and_loyalty_change_the_chance() {
        assert_eq!(blocked_chance(true, 10.0, 10.0), 0.25);
        assert_eq!(blocked_chance(false, 10.0, 10.0), 0.10);
        assert_eq!(blocked_chance(true, 18.0, 10.0), 0.5);
        assert_eq!(blocked_chance(true, 18.0, 18.0), 0.25);
    }
}
