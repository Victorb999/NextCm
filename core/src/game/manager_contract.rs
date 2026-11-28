//! O contrato do treinador: nos últimos meses, a Diretoria propõe a renovação se confia no
//! trabalho dele (`offers.kind = 'renewal'`: `fee` é o salário por semana e `asked` os anos).
//! Sem renovar, no fim do contrato o treinador sai do clube e fica livre.

use rusqlite::{OptionalExtension, params};

use super::market::money_text;
use super::{Game, NewsItem, cm_date};
use crate::{Error, Result, date};

/// Dias antes do fim do contrato em que a Diretoria decide, e para o treinador responder.
const RENEW_DAYS: i32 = 120;
const ANSWER_DAYS: i32 = 14;
/// Confiança mínima para a Diretoria propor a renovação.
const RENEW_CONFIDENCE: i32 = 50;

impl Game {
    /// O fim do contrato e o salário do treinador.
    fn manager_terms(&self) -> Result<(Option<String>, i64)> {
        Ok(self.world.conn.query_row("SELECT contract_expires, COALESCE(wage, 0) FROM people WHERE id = ?", [self.manager_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?)
    }

    /// O salário de um treinador no clube: um pouco acima do salário médio do elenco.
    pub(super) fn coach_wage(&self, club: i32) -> Result<i64> {
        Ok(super::board::round_wage((self.average_wage(club)? * 1.2) as i64).max(1000))
    }

    /// O dia do contrato do treinador: a proposta de renovação, a resposta que não veio e o fim
    /// do contrato.
    pub(super) fn manager_contract_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if self.sacked() || self.prestart {
            return Ok(());
        }
        let (Some(expires), wage) = self.manager_terms()? else { return Ok(()) };
        let club = self.club(self.club_id).name;
        if expires < self.date {
            self.leave_club()?;
            let body =
                format!("O seu contrato com o {club} acabou e você deixou o clube.\n\nProcure um novo clube em Informação de Empregos.");
            self.push_news(fresh, next_id, "jobs", "Fim de contrato".into(), body);
            return Ok(());
        }
        // A proposta sem resposta cai
        for o in self.offers_where("kind = 'renewal' AND status = 'pending' AND answer_on < ?1", params![self.date])? {
            self.set_status(o.id, "rejected", None)?;
            let body = format!("Sem resposta da sua parte, a Diretoria retirou a proposta. O contrato acaba em {}.", cm_date(&expires));
            self.push_news(fresh, next_id, "jobs", "Renovação retirada".into(), body);
        }
        let (Some(window), Some(since)) = (date::add_days(&self.date, RENEW_DAYS), date::add_days(&expires, -RENEW_DAYS - 1)) else {
            return Ok(());
        };
        // Uma decisão por contrato
        let decided = self
            .world
            .conn
            .query_row(
                "SELECT 1 FROM offers WHERE kind = 'renewal' AND person_id = ? AND created >= ?",
                params![self.manager_id, since],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if expires > window || decided {
            return Ok(());
        }
        let confidence: i32 = self.world.conn.query_row("SELECT confidence FROM board WHERE id = 1", [], |r| r.get(0))?;
        if confidence < RENEW_CONFIDENCE {
            // Guarda a decisão (recusada pela Diretoria) para não voltar a ela
            self.world.conn.execute(
                "INSERT INTO offers (kind, person_id, buyer, seller, fee, asked, status, answer_on, created)
                 VALUES ('renewal', ?, ?, NULL, 0, 0, 'declined', NULL, ?)",
                params![self.manager_id, self.club_id, self.date],
            )?;
            let body = format!(
                "A Diretoria do {club} não pretende renovar o seu contrato, que acaba em {}. Se não houver mudança, você deixará o clube.",
                cm_date(&expires)
            );
            self.push_news(fresh, next_id, "jobs", "Contrato não será renovado".into(), body);
            return Ok(());
        }
        let years = match confidence {
            80.. => 3,
            65.. => 2,
            _ => 1,
        };
        let base = self.coach_wage(self.club_id)? as f64 * (0.8 + confidence as f64 / 250.0);
        let offer = super::board::round_wage(base.max(wage as f64 * 1.1) as i64);
        let until = date::add_days(&self.date, ANSWER_DAYS).unwrap_or_default();
        self.world.conn.execute(
            "INSERT INTO offers (kind, person_id, buyer, seller, fee, asked, status, answer_on, created)
             VALUES ('renewal', ?, ?, NULL, ?, ?, 'pending', ?, ?)",
            params![self.manager_id, self.club_id, offer, years, until, self.date],
        )?;
        let id = self.world.conn.last_insert_rowid() as i32;
        let body = format!(
            "A Diretoria do {club} está satisfeita com o seu trabalho e quer renovar o seu contrato, que acaba em {}.\n\n\
             A proposta: mais {years} {} e um salário de {} por semana. Responda até {}.",
            cm_date(&expires),
            if years == 1 { "temporada" } else { "temporadas" },
            money_text(offer),
            cm_date(&until)
        );
        self.offer_news(fresh, next_id, "Proposta de renovação".into(), body, id);
        self.unsaved = true;
        Ok(())
    }

    /// Botões da proposta de renovação.
    pub(super) fn answer_renewal(&mut self, o: &super::deals::Offer, key: &str) -> Result<()> {
        let (Some(expires), _) = self.manager_terms()? else { return Err(Error::Invalid("Você não tem contrato.".into())) };
        match (o.status.as_str(), key) {
            ("pending", "accept") => {
                let years = o.asked.unwrap_or(1) as i32;
                let year: i32 = expires[..4].parse().unwrap_or(0);
                let until = format!("{}{}", year + years, &expires[4..]);
                self.world
                    .conn
                    .execute("UPDATE people SET contract_expires = ?, wage = ? WHERE id = ?", params![until, o.fee, self.manager_id])?;
                self.set_status(o.id, "done", None)?;
                let club = self.club(self.club_id).name;
                let body = format!("Você renovou com o {club} até {}, com {} por semana.", cm_date(&until), money_text(o.fee));
                self.post_news_as("jobs", "Contrato renovado".into(), body);
            }
            ("pending", "reject") => {
                self.set_status(o.id, "rejected", None)?;
                let body = format!("Você recusou a renovação. O contrato acaba em {} e você deixará o clube.", cm_date(&expires));
                self.post_news_as("jobs", "Renovação recusada".into(), body);
            }
            _ => return Err(Error::Invalid("Essa proposta não aceita mais essa resposta.".into())),
        }
        self.unsaved = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::World;
    use crate::game::Game;

    #[test]
    fn board_renews_or_lets_the_manager_go() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = crate::TempDir::new(format!("nextcm-renovacao-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Renovacao".into(),
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
        let soon = crate::date::add_days(&g.date, 60).unwrap();
        let set = |g: &Game, expires: &str, confidence: i32| {
            g.world.conn.execute("UPDATE people SET contract_expires = ? WHERE id = ?", rusqlite::params![expires, g.manager_id]).unwrap();
            g.world.conn.execute("UPDATE board SET confidence = ? WHERE id = 1", [confidence]).unwrap();
        };
        let (mut fresh, mut next) = (Vec::new(), 100_000);
        // Com confiança, a Diretoria propõe; aceitar estende o contrato com salário
        set(&g, &soon, 85);
        g.manager_contract_day(&mut fresh, &mut next).unwrap();
        let offer = g.offers_where("kind = 'renewal' AND status = 'pending'", []).unwrap();
        assert_eq!(offer.len(), 1, "uma proposta de renovação");
        assert_eq!(offer[0].asked, Some(3));
        g.manager_contract_day(&mut fresh, &mut next).unwrap();
        assert_eq!(g.offers_where("kind = 'renewal'", []).unwrap().len(), 1, "uma decisão por contrato");
        g.answer_offer(offer[0].id, "accept", None).unwrap();
        let (expires, wage) = g.manager_terms().unwrap();
        let year: i32 = soon[..4].parse().unwrap();
        assert_eq!(expires.unwrap(), format!("{}{}", year + 3, &soon[4..]));
        assert!(wage > 0);
        // Sem confiança, não renova; no fim do contrato o treinador sai
        let later = crate::date::add_days(&g.date, 30).unwrap();
        set(&g, &later, 30);
        g.world.conn.execute("UPDATE offers SET created = '2000-01-01' WHERE kind = 'renewal'", []).unwrap();
        g.manager_contract_day(&mut fresh, &mut next).unwrap();
        assert_eq!(g.offers_where("kind = 'renewal' AND status = 'declined'", []).unwrap().len(), 1);
        set(&g, "2000-01-01", 30);
        g.manager_contract_day(&mut fresh, &mut next).unwrap();
        assert!(g.sacked(), "saiu do clube no fim do contrato");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
