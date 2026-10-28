//! O treinador (marco 3.13, passo 4): Rescindir com Clube, Retirar-se, Ir de Férias (o
//! adjunto trata de tudo durante uns dias) e o Currículo com os clubes por onde passou
//! (`manager_history`), as compras, as vendas e os títulos de cada um.

use rusqlite::params;
use serde::Serialize;
use ts_rs::TS;

use super::{Game, GameInfo};
use crate::person::ClubRef;
use crate::{Error, Result, date};

/// Dias máximos de férias de uma vez.
const MAX_HOLIDAY: i32 = 180;

/// Um clube no currículo do treinador.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct CareerClub {
    pub club: ClubRef,
    pub joined: String,
    /// `None` = o clube atual.
    pub left: Option<String>,
    pub purchases: i32,
    #[ts(type = "number")]
    pub spent: i64,
    pub sales: i32,
    #[ts(type = "number")]
    pub received: i64,
    pub releases: i32,
    /// Os títulos ganhos ("Série A 2026").
    pub titles: Vec<String>,
}

impl Game {
    /// Abre a passagem por um clube (ao começar o jogo e ao aceitar outro emprego).
    pub(super) fn career_join(&self, club: i32) -> Result<()> {
        self.world.conn.execute("INSERT INTO manager_history (club_id, joined) VALUES (?, ?)", params![club, self.date])?;
        Ok(())
    }

    /// Fecha a passagem atual (despedido, rescindiu ou mudou de clube).
    pub(super) fn career_leave(&self) -> Result<()> {
        self.world.conn.execute("UPDATE manager_history SET left = ? WHERE left IS NULL", [&self.date])?;
        Ok(())
    }

    /// "Rescindir com Clube": sai do clube por vontade própria e fica sem emprego.
    pub fn resign(&mut self) -> Result<String> {
        if self.sacked() {
            return Err(Error::Invalid("Não tem clube.".into()));
        }
        let club = self.club(self.club_id).name;
        self.world.conn.execute("UPDATE board SET sacked = 1 WHERE id = 1", [])?;
        self.world.conn.execute("UPDATE people SET club_id = NULL, club_job = NULL WHERE id = ?", [self.manager_id])?;
        self.world.conn.execute("INSERT OR REPLACE INTO vacancies (club_id, since) VALUES (?, ?)", params![self.club_id, self.date])?;
        self.career_leave()?;
        self.selection = None;
        let text = format!("Rescindiu o contrato com o {club}. Procure um novo clube em Informação de Empregos.");
        self.post_news_as("jobs", format!("Saída do {club}"), text.clone());
        self.unsaved = true;
        Ok(text)
    }

    /// "Retirar-se": deixa o futebol (sai do clube, se tiver um).
    pub fn retire_manager(&mut self) -> Result<String> {
        if !self.sacked() {
            self.resign()?;
        }
        self.world.conn.execute("INSERT OR IGNORE INTO retired (person_id, date) VALUES (?, ?)", params![self.manager_id, self.date])?;
        let name = self.person_name(self.manager_id)?;
        self.post_news_as("jobs", format!("{name} retira-se"), format!("{name} anunciou o fim da carreira de treinador."));
        self.unsaved = true;
        Ok(format!("{name} retirou-se do futebol."))
    }

    /// "Ir de Férias": o adjunto escolhe a equipa e os dias passam sem ver os jogos, até
    /// `days` dias ou até o treinador ser despedido. As propostas e as conferências ficam
    /// sem resposta (expiram como se ele não estivesse).
    pub fn holiday(&mut self, days: i32) -> Result<GameInfo> {
        if !(1..=MAX_HOLIDAY).contains(&days) {
            return Err(Error::Invalid(format!("Férias de 1 a {MAX_HOLIDAY} dias.")));
        }
        if self.live.is_some() {
            return Err(Error::Invalid("Não pode ir de férias no dia de um jogo.".into()));
        }
        let watch = self.watch;
        let selection = self.selection.clone();
        self.watch = false;
        if let Some(sel) = &mut self.selection {
            sel.auto = true;
        }
        let back = date::add_days(&self.date, days).unwrap_or_default();
        let result: Result<()> = (|| {
            while self.date.as_str() < back.as_str() && !self.sacked() {
                self.advance_day()?;
            }
            Ok(())
        })();
        self.watch = watch;
        // A escolha do treinador volta (sem os que saíram entretanto)
        if !self.sacked() {
            self.selection = selection;
            self.clean_selection()?;
        }
        result?;
        self.post_news_as("message", "De volta das férias".into(), format!("Voltou das férias a {}.", super::cm_date(&self.date)));
        self.unsaved = true;
        self.info()
    }

    /// "Currículo": os clubes do treinador, com os negócios e os títulos de cada passagem.
    pub fn manager_career(&self) -> Result<Vec<CareerClub>> {
        let rows: Vec<(i32, String, Option<String>)> = self
            .world
            .conn
            .prepare_cached("SELECT club_id, joined, left FROM manager_history ORDER BY id DESC")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut out = Vec::new();
        for (club, joined, left) in rows {
            let until = left.clone().unwrap_or_else(|| "9999-12-31".into());
            let sum = |sql: &str| -> Result<(i32, i64)> {
                Ok(self.world.conn.query_row(sql, params![club, joined, until], |r| Ok((r.get(0)?, r.get(1)?)))?)
            };
            let (purchases, spent) = sum(
                "SELECT COUNT(*), COALESCE(SUM(fee), 0) FROM transfers WHERE to_club = ?1 AND date >= ?2 AND date <= ?3 AND kind IN ('transfer', 'free')",
            )?;
            let (sales, received) = sum(
                "SELECT COUNT(*), COALESCE(SUM(fee), 0) FROM transfers WHERE from_club = ?1 AND date >= ?2 AND date <= ?3 AND kind = 'transfer'",
            )?;
            let (releases, _) =
                sum("SELECT COUNT(*), 0 FROM transfers WHERE from_club = ?1 AND date >= ?2 AND date <= ?3 AND kind = 'release'")?;
            let (from_year, to_year): (i32, i32) =
                (joined[0..4].parse().unwrap_or(0), left.as_deref().map_or(9999, |l| l[0..4].parse().unwrap_or(9999)));
            let titles: Vec<String> = self
                .world
                .conn
                .prepare_cached(
                    "SELECT competition_id, year FROM competition_history WHERE winner_id = ? AND year >= ? AND year <= ? ORDER BY year",
                )?
                .query_map(params![club, from_year, to_year], |r| Ok((r.get::<_, i32>(0)?, r.get::<_, i32>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .into_iter()
                .map(|(c, y)| format!("{} {y}", self.competition_name(c)))
                .collect();
            out.push(CareerClub { club: self.club(club), joined, left, purchases, spent, sales, received, releases, titles });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holiday_resign_and_career() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-ferias-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Férias".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
        };
        let mut g = Game::create(&path, &dir, &new, 23).unwrap();
        g.set_watch(true);
        let start = g.date.clone();
        let info = g.holiday(14).unwrap();
        assert_eq!(info.date, date::add_days(&start, 14).unwrap());
        assert!(!info.live, "volta fora de um jogo");
        let career = g.manager_career().unwrap();
        assert_eq!(career.len(), 1);
        assert_eq!(career[0].club.id, 7568);
        g.resign().unwrap();
        assert!(g.sacked());
        assert!(g.manager_career().unwrap()[0].left.is_some());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
