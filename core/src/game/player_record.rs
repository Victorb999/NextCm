//! Polimento do perfil e do plantel (marco 3.13, passo 1): os jogos de cada jogador ficam
//! gravados (`player_matches`) para a Forma (as últimas 5 notas) e as estatísticas da época
//! por competição; a alcunha e a nota do treinador; Dispensar um jogador com a indemnização
//! do resto do contrato, como no CM.

use rusqlite::{OptionalExtension, params};

use super::market::money_text;
use super::{Game, cm_date};
use crate::person::{PersonProfile, SeasonLine};
use crate::regulation::SeasonKind;
use crate::{Error, Result, date};

/// Notas na Forma do perfil.
const FORM_GAMES: usize = 5;

impl Game {
    /// No fim do dia: a linha de cada jogador que jogou (nota, golos, assistências).
    pub(super) fn record_match(&self, fixture: i32, competition: i32, clubs: [i32; 2], lines: [Vec<(i32, f64, u8, u8)>; 2]) -> Result<()> {
        let mut insert = self.world.conn.prepare_cached(
            "INSERT INTO player_matches (person_id, fixture, date, competition, club, rating, goals, assists) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )?;
        for (side, players) in lines.iter().enumerate() {
            for &(id, rating, goals, assists) in players {
                insert.execute(params![
                    id,
                    fixture,
                    self.date,
                    competition,
                    clubs[side],
                    (rating * 10.0).round() / 10.0,
                    goals,
                    assists
                ])?;
            }
        }
        Ok(())
    }

    /// Forma, estatísticas da época, alcunha, nota e o custo de dispensar (jogador do treinador).
    pub fn fill_record(&self, p: &mut PersonProfile) -> Result<()> {
        p.nickname = self.world.conn.query_row("SELECT nickname FROM nicknames WHERE person_id = ?", [p.id], |r| r.get(0)).optional()?;
        if let Some(n) = &p.nickname {
            p.name = n.clone();
        }
        p.note = self.world.conn.query_row("SELECT text FROM notes WHERE person_id = ?", [p.id], |r| r.get(0)).optional()?;
        if p.player.is_none() {
            return Ok(());
        }
        p.form = self
            .world
            .conn
            .prepare_cached("SELECT rating FROM player_matches WHERE person_id = ? ORDER BY date DESC, fixture DESC LIMIT ?")?
            .query_map(params![p.id, FORM_GAMES as i64], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        p.form.reverse();
        // A época do clube onde joga (civil ou europeia)
        let kind = p.club.as_ref().map(|c| self.kind_of(c.id)).transpose()?.unwrap_or(SeasonKind::European);
        let year: i32 = self.date[0..4].parse().unwrap_or(0);
        let start = match kind {
            SeasonKind::Calendar => format!("{year}-01-01"),
            SeasonKind::European if &self.date[5..] >= "07-01" => format!("{year}-07-01"),
            SeasonKind::European => format!("{}-07-01", year - 1),
        };
        let rows: Vec<(i32, i32, i32, i32, f64)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT competition, COUNT(*), SUM(goals), SUM(assists), AVG(rating) FROM player_matches
                 WHERE person_id = ? AND date >= ? GROUP BY competition ORDER BY COUNT(*) DESC, competition",
            )?
            .query_map(params![p.id, start], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        p.season = rows
            .into_iter()
            .map(|(c, apps, goals, assists, rating)| SeasonLine {
                competition: self.competition_name(c).to_string(),
                apps,
                goals,
                assists,
                rating: (rating * 100.0).round() / 100.0,
            })
            .collect();
        if p.club.as_ref().is_some_and(|c| c.id == self.club_id) {
            p.release_cost = Some(self.release_cost(p.id)?);
        }
        Ok(())
    }

    /// "Definir Alcunha" (vazia apaga).
    pub fn set_nickname(&mut self, person: i32, nickname: &str) -> Result<()> {
        let nickname = nickname.trim();
        if nickname.chars().count() > 30 {
            return Err(Error::Invalid("A alcunha tem no máximo 30 letras.".into()));
        }
        if nickname.is_empty() {
            self.world.conn.execute("DELETE FROM nicknames WHERE person_id = ?", [person])?;
        } else {
            self.world.conn.execute("INSERT OR REPLACE INTO nicknames (person_id, nickname) VALUES (?, ?)", params![person, nickname])?;
        }
        self.unsaved = true;
        Ok(())
    }

    /// "Criar Nota Treinador" (vazia apaga).
    pub fn set_note(&mut self, person: i32, text: &str) -> Result<()> {
        let text = text.trim();
        if text.is_empty() {
            self.world.conn.execute("DELETE FROM notes WHERE person_id = ?", [person])?;
        } else {
            self.world.conn.execute("INSERT OR REPLACE INTO notes (person_id, text) VALUES (?, ?)", params![person, text])?;
        }
        self.unsaved = true;
        Ok(())
    }

    /// A indemnização para dispensar: o salário até ao fim do contrato.
    fn release_cost(&self, person: i32) -> Result<i64> {
        let (wage, expires): (Option<i64>, Option<String>) =
            self.world
                .conn
                .query_row("SELECT wage, contract_expires FROM people WHERE id = ?", [person], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let days = expires.as_deref().and_then(date::parse).zip(date::parse(&self.date)).map_or(0, |(e, t)| (e - t).max(0));
        Ok(crate::finance::round_money(wage.unwrap_or(0) as f64 * days as f64 / 7.0))
    }

    /// "Dispensar": o jogador fica livre e o clube paga o resto do contrato.
    pub fn release_player(&mut self, person: i32) -> Result<String> {
        let club: Option<i32> = self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [person], |r| r.get(0))?;
        if club != Some(self.club_id) || self.loan_of(person)?.is_some() {
            return Err(Error::Invalid("Só se dispensam jogadores do seu plantel.".into()));
        }
        let cost = self.release_cost(person)?;
        let name = self.person_name(person)?;
        self.book(self.club_id, "wages", cost)?;
        self.world.conn.execute("UPDATE people SET club_id = NULL, club_job = NULL, contract_expires = NULL WHERE id = ?", [person])?;
        self.number_after_move(person, None)?;
        self.log_transfer(person, Some(self.club_id), None, 0, "release")?;
        for table in ["transfer_list", "squad_roles", "shortlist"] {
            self.world.conn.execute(&format!("DELETE FROM {table} WHERE person_id = ?"), [person])?;
        }
        self.squad_changed()?;
        let text = if cost > 0 {
            format!("{name} foi dispensado. O clube pagou-lhe {} de indemnização pelo resto do contrato.", money_text(cost))
        } else {
            format!("{name} foi dispensado.")
        };
        self.post_news_as("contract", format!("{name} dispensado"), text.clone());
        Ok(text)
    }

    /// A pergunta do CM antes de dispensar.
    pub fn release_question(&self, person: i32) -> Result<String> {
        let name = self.person_name(person)?;
        let cost = self.release_cost(person)?;
        let until: Option<String> =
            self.world.conn.query_row("SELECT contract_expires FROM people WHERE id = ?", [person], |r| r.get(0))?;
        let club = self.club(self.club_id).short_name;
        Ok(if cost > 0 {
            format!(
                "Dispensar {name}? O {club} terá de o indemnizar em {} (contrato até {}).",
                money_text(cost),
                until.as_deref().map(cm_date).unwrap_or_default()
            )
        } else {
            format!("Dispensar {name}?")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_nickname_and_release() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-perfil-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Perfil".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
        };
        let mut g = Game::create(&path, &dir, &new, 17).unwrap();
        let first = g.matches.iter().filter(|m| m.home_id == 7568 || m.away_id == 7568).map(|m| m.date.clone()).min().unwrap();
        while g.date.as_str() <= first.as_str() {
            g.advance_day().unwrap();
        }
        let played: i32 =
            g.world.conn.query_row("SELECT person_id FROM player_matches WHERE club = 7568 LIMIT 1", [], |r| r.get(0)).unwrap();
        let mut p = g.world.get_person(played).unwrap();
        g.fill_record(&mut p).unwrap();
        assert_eq!(p.form.len(), 1);
        assert_eq!(p.season.iter().map(|s| s.apps).sum::<i32>(), 1);
        g.set_nickname(played, "Mágico").unwrap();
        g.set_note(played, "Bom no jogo aéreo").unwrap();
        g.fill_record(&mut p).unwrap();
        assert_eq!((p.name.as_str(), p.note.as_deref()), ("Mágico", Some("Bom no jogo aéreo")));
        // Dispensar: fica livre e o clube paga o resto do contrato
        let cost = p.release_cost.unwrap();
        let cash = g.cash_of(7568).unwrap();
        assert!(g.release_question(played).unwrap().starts_with("Dispensar"));
        g.release_player(played).unwrap();
        assert_eq!(g.facts(played).unwrap().unwrap().club, None);
        assert_eq!(g.cash_of(7568).unwrap(), cash - cost);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
