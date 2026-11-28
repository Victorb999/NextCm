//! O contrato do treinador: nos últimos meses, a Diretoria propõe a renovação se confia no
//! trabalho dele (`offers.kind = 'renewal'`: `fee` é o salário por semana, `asked` os anos e
//! `rounds` as vezes que o treinador pediu mais). O treinador aceita, recusa ou negocia: a
//! Diretoria aceita o que couber no teto dela, contrapõe duas vezes e depois encerra. Sem
//! renovar, no fim do contrato o treinador sai do clube e fica livre.
//!
//! Os técnicos da IA renovam no fim do contrato pela confiança da diretoria e pelo que pedem
//! (a ambição); sem acordo saem, e o lugar vago preenche-se como num despedimento.

use rusqlite::{OptionalExtension, params};

use super::deals::{Offer, OfferView};
use super::market::money_text;
use super::{Game, NewsItem, cm_date};
use crate::{Error, Result, date};

/// Dias antes do fim do contrato em que a Diretoria decide, e para o treinador responder.
const RENEW_DAYS: i32 = 120;
const ANSWER_DAYS: i32 = 14;
/// Confiança mínima para a Diretoria renovar (com o treinador e com os da IA).
const RENEW_CONFIDENCE: i32 = 50;
/// Vezes que a Diretoria contrapõe antes de encerrar a negociação.
const COUNTERS: i32 = 2;

fn seasons(years: i64) -> &'static str {
    if years == 1 { "temporada" } else { "temporadas" }
}

/// O contrato até `years` temporadas depois do fim do atual (o mesmo dia e mês).
fn extend(expires: &str, years: i64) -> String {
    let year: i64 = expires[..4].parse().unwrap_or(0);
    format!("{}{}", year + years, &expires[4..])
}

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

    /// O teto da Diretoria: o salário e os anos que ela ainda aceita, pela confiança.
    fn board_ceiling(&self, club: i32, wage: i64, confidence: i32) -> Result<(i64, i64)> {
        let base = self.coach_wage(club)? as f64 * (0.9 + confidence as f64 * 0.008);
        let years = match confidence {
            80.. => 4,
            65.. => 3,
            _ => 2,
        };
        Ok((super::board::round_wage(base.max(wage as f64 * 1.1) as i64), years))
    }

    fn confidence(&self) -> Result<i32> {
        Ok(self.world.conn.query_row("SELECT confidence FROM board WHERE id = 1", [], |r| r.get(0))?)
    }

    /// O dia dos contratos dos treinadores: os da IA e o do treinador (a proposta de renovação,
    /// a resposta que não veio e o fim do contrato).
    pub(super) fn manager_contract_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        self.ai_coach_contracts(fresh, next_id)?;
        if self.sacked() || self.prestart {
            return Ok(());
        }
        let (Some(expires), wage) = self.manager_terms()? else { return Ok(()) };
        let club = self.club(self.club_id).short_name;
        if expires < self.date {
            self.leave_club()?;
            let body =
                format!("O seu contrato com o {club} acabou e você deixou o clube.\n\nProcure um novo clube em Informação de Empregos.");
            self.push_news(fresh, next_id, "jobs", "Fim de contrato".into(), body);
            return Ok(());
        }
        // A proposta sem resposta cai
        for o in self.offers_where("kind = 'renewal' AND status IN ('pending', 'countered') AND answer_on < ?1", params![self.date])? {
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
        let confidence = self.confidence()?;
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
        let base = self.coach_wage(self.club_id)? as f64 * (0.8 + confidence as f64 * 0.004);
        let offer = super::board::round_wage(base.max(wage as f64 * 1.05) as i64);
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
            seasons(years),
            money_text(offer),
            cm_date(&until)
        );
        self.offer_news(fresh, next_id, "Proposta de renovação".into(), body, id);
        self.unsaved = true;
        Ok(())
    }

    /// O texto da proposta de renovação e se ainda tem botões (para a vista da proposta).
    pub(super) fn renewal_view(o: &Offer, until: &str) -> (String, bool) {
        let years = o.asked.unwrap_or(1);
        let terms = format!("mais {years} {} com {} por semana", seasons(years), money_text(o.fee));
        match o.status.as_str() {
            "pending" => (format!("A Diretoria propõe {terms}. Responda até {until}."), true),
            "countered" => (format!("A Diretoria fez uma contraproposta: {terms}. Responda até {until}."), true),
            "done" => ("Contrato renovado.".into(), false),
            _ => ("Proposta encerrada.".into(), false),
        }
    }

    /// Renova o contrato do treinador com estas condições.
    fn renew_manager(&mut self, o: &Offer, wage: i64, years: i64) -> Result<()> {
        let (Some(expires), _) = self.manager_terms()? else { return Err(Error::Invalid("Você não tem contrato.".into())) };
        let until = extend(&expires, years);
        self.world.conn.execute("UPDATE people SET contract_expires = ?, wage = ? WHERE id = ?", params![until, wage, self.manager_id])?;
        self.world.conn.execute("UPDATE offers SET fee = ?, asked = ?, status = 'done' WHERE id = ?", params![wage, years, o.id])?;
        let club = self.club(self.club_id).short_name;
        let body = format!("Você renovou com o {club} até {}, com {} por semana.", cm_date(&until), money_text(wage));
        self.post_news_as("jobs", "Contrato renovado".into(), body);
        self.unsaved = true;
        Ok(())
    }

    /// Botões da proposta de renovação.
    pub(super) fn answer_renewal(&mut self, o: &Offer, key: &str) -> Result<()> {
        let (Some(expires), _) = self.manager_terms()? else { return Err(Error::Invalid("Você não tem contrato.".into())) };
        match (o.status.as_str(), key) {
            ("pending" | "countered", "accept") => self.renew_manager(o, o.fee, o.asked.unwrap_or(1))?,
            ("pending" | "countered", "reject") => {
                self.set_status(o.id, "rejected", None)?;
                let body = format!("Você recusou a renovação. O contrato acaba em {} e você deixará o clube.", cm_date(&expires));
                self.post_news_as("jobs", "Renovação recusada".into(), body);
            }
            _ => return Err(Error::Invalid("Essa proposta não aceita mais essa resposta.".into())),
        }
        self.unsaved = true;
        Ok(())
    }

    /// "Negociar": o treinador pede um salário e os anos. Dentro do teto, a Diretoria aceita;
    /// acima, contrapõe (no meio do caminho, depois o teto) e, à terceira, encerra.
    pub fn ask_renewal(&mut self, id: i32, wage: i64, years: i32) -> Result<OfferView> {
        let o = self.offer(id)?;
        if o.kind != "renewal" || !matches!(o.status.as_str(), "pending" | "countered") {
            return Err(Error::Invalid("Essa proposta não aceita mais negociação.".into()));
        }
        if wage <= 0 || !(1..=5).contains(&years) {
            return Err(Error::Invalid("Peça um salário e de 1 a 5 temporadas.".into()));
        }
        let (_, current) = self.manager_terms()?;
        let (max_wage, max_years) = self.board_ceiling(self.club_id, current, self.confidence()?)?;
        let years = years as i64;
        if wage <= max_wage && years <= max_years {
            self.renew_manager(&o, wage, years)?;
            return self.offer_view(id);
        }
        if o.rounds >= COUNTERS {
            self.set_status(id, "rejected", None)?;
            let club = self.club(self.club_id).short_name;
            let body =
                format!("A Diretoria do {club} encerrou as negociações: não vai além do que ofereceu. O seu contrato não será renovado.");
            self.post_news_as("jobs", "Negociação encerrada".into(), body);
            self.unsaved = true;
            return self.offer_view(id);
        }
        // A contraproposta: o salário pedido até ao teto, no meio do caminho na primeira vez
        let target = wage.min(max_wage);
        let fee = if o.rounds == 0 { super::board::round_wage(o.fee.max((o.fee + target) / 2)) } else { target.max(o.fee) };
        let until = date::add_days(&self.date, ANSWER_DAYS).unwrap_or_default();
        self.world.conn.execute(
            "UPDATE offers SET fee = ?, asked = ?, status = 'countered', answer_on = ?, rounds = rounds + 1 WHERE id = ?",
            params![fee, years.min(max_years), until, id],
        )?;
        self.unsaved = true;
        self.offer_view(id)
    }

    /// Os técnicos da IA com o contrato acabado: a diretoria renova se confia e se o que ele pede
    /// cabe no teto dela; senão ele sai e o lugar fica vago.
    /// Técnicos da IA com o contrato da base de dados já vencido: mais um ano a partir de hoje
    /// (mesmo dia e mês), para não renovarem ou saírem todos no primeiro dia.
    pub(super) fn extend_expired_coaches(&self) -> Result<usize> {
        let next = self.date[..4].parse::<i32>().unwrap_or(0) + 1;
        Ok(self.world.conn.execute(
            "UPDATE people SET contract_expires = ? || substr(contract_expires, 5)
             WHERE club_job = 5 AND id <> ? AND contract_expires < ?",
            params![next.to_string(), self.manager_id, self.date],
        )?)
    }

    fn ai_coach_contracts(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        type Row = (i32, i32, String, i64, i32, String);
        let ended: Vec<Row> = self
            .world
            .conn
            .prepare_cached(&format!(
                "SELECT id, club_id, display_name, COALESCE(wage, 0), COALESCE(ambition, 10), contract_expires FROM people
                 WHERE club_job = 5 AND id <> ? AND contract_expires < ?
                   AND club_id IN (SELECT id FROM clubs WHERE division_id IN ({})) ORDER BY id",
                self.managed_sql(None)
            ))?
            .query_map(params![self.manager_id, self.date], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (coach, club, name, wage, ambition, expires) in ended {
            let confidence = self.ai_confidence(club)?;
            // O que ele pede: o salário de hoje (ou o do clube) e mais pela ambição
            let asked = super::board::round_wage((wage.max(self.coach_wage(club)?) as f64 * (1.0 + ambition as f64 / 40.0)) as i64);
            let (max_wage, max_years) = self.board_ceiling(club, wage, confidence)?;
            let newsworthy = self.newsworthy(club)?;
            let club_name = self.club(club).short_name;
            if confidence >= RENEW_CONFIDENCE && asked <= max_wage {
                // Uma temporada a mais por cada 15 pontos de confiança acima do mínimo, até ao teto
                let years = (1 + (confidence - RENEW_CONFIDENCE) as i64 / 15).min(max_years);
                // Um contrato de antes deste ano (save antigo) conta a partir deste ano
                let base = if expires[..4] < self.date[..4] { format!("{}{}", &self.date[..4], &expires[4..]) } else { expires };
                let mut until = extend(&base, years);
                if until <= self.date {
                    until = extend(&until, 1);
                }
                self.world.conn.execute("UPDATE people SET contract_expires = ?, wage = ? WHERE id = ?", params![until, asked, coach])?;
                if newsworthy {
                    let body = format!("{name} renovou com o {club_name} até {}.", cm_date(&until));
                    self.push_news(fresh, next_id, "jobs", format!("{name} renova com o {club_name}"), body);
                }
                continue;
            }
            self.world
                .conn
                .execute("UPDATE people SET club_id = NULL, club_job = NULL, wage = 0, contract_expires = NULL WHERE id = ?", [coach])?;
            self.log_staff_move(coach, Some(club), None)?;
            self.world.conn.execute("DELETE FROM ai_boards WHERE club_id = ?", [club])?;
            self.world.conn.execute("INSERT OR REPLACE INTO vacancies (club_id, since) VALUES (?, ?)", params![club, self.date])?;
            if newsworthy {
                let why = if confidence < RENEW_CONFIDENCE {
                    format!("a diretoria do {club_name} não quis renovar o contrato")
                } else {
                    format!("{name} e o {club_name} não chegaram a acordo para renovar")
                };
                self.push_news(fresh, next_id, "jobs", format!("{name} deixa o {club_name}"), format!("Fim de contrato: {why}."));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::World;
    use crate::game::Game;

    fn new_game(name: &str) -> Option<(Game, crate::TempDir)> {
        let path = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))?;
        let dir = crate::TempDir::new(format!("nextcm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: name.into(),
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
        Some((g, dir))
    }

    fn set(g: &Game, expires: &str, confidence: i32) {
        g.world.conn.execute("UPDATE people SET contract_expires = ? WHERE id = ?", rusqlite::params![expires, g.manager_id]).unwrap();
        g.world.conn.execute("UPDATE board SET confidence = ? WHERE id = 1", [confidence]).unwrap();
    }

    #[test]
    fn board_renews_or_lets_the_manager_go() {
        let Some((mut g, dir)) = new_game("renovacao") else { return };
        let (_, start_wage) = g.manager_terms().unwrap();
        assert!(start_wage > 0, "o treinador começa com o salário do clube");
        let soon = crate::date::add_days(&g.date, 60).unwrap();
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
        assert_eq!(expires.unwrap(), super::extend(&soon, 3));
        assert!(wage >= start_wage);
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

    #[test]
    fn manager_negotiates_the_renewal() {
        let Some((mut g, dir)) = new_game("negociacao") else { return };
        let soon = crate::date::add_days(&g.date, 60).unwrap();
        let (mut fresh, mut next) = (Vec::new(), 100_000);
        set(&g, &soon, 70);
        g.manager_contract_day(&mut fresh, &mut next).unwrap();
        let o = g.offers_where("kind = 'renewal' AND status = 'pending'", []).unwrap().remove(0);
        // Pedir o dobro: contrapõe duas vezes, subindo, e à terceira encerra
        let v = g.ask_renewal(o.id, o.fee * 2, 5).unwrap();
        assert_eq!(v.status, "countered");
        assert!(v.fee > o.fee && v.years <= 3);
        let v2 = g.ask_renewal(o.id, o.fee * 2, 5).unwrap();
        assert!(v2.status == "countered" && v2.fee >= v.fee);
        let v3 = g.ask_renewal(o.id, o.fee * 2, 5).unwrap();
        assert_eq!(v3.status, "rejected");
        // Uma nova proposta: pedir dentro do teto renova logo
        g.world.conn.execute("UPDATE offers SET created = '2000-01-01' WHERE kind = 'renewal'", []).unwrap();
        g.manager_contract_day(&mut fresh, &mut next).unwrap();
        let o = g.offers_where("kind = 'renewal' AND status = 'pending'", []).unwrap().remove(0);
        let v = g.ask_renewal(o.id, o.fee + 1000, 2).unwrap();
        assert_eq!(v.status, "done");
        assert_eq!(g.manager_terms().unwrap(), (Some(super::extend(&soon, 2)), o.fee + 1000));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ai_coaches_renew_or_leave() {
        let Some((mut g, dir)) = new_game("tecnicos") else { return };
        // Ao criar o jogo, nenhum contrato de técnico começa vencido
        let expired: i64 = g
            .world
            .conn
            .query_row("SELECT COUNT(*) FROM people WHERE club_job = 5 AND contract_expires < ?", [&g.date], |r| r.get(0))
            .unwrap();
        assert_eq!(expired, 0);
        let coaches: Vec<(i32, i32)> = g
            .world
            .conn
            .prepare(&format!(
                "SELECT id, club_id FROM people WHERE club_job = 5 AND id <> {} AND club_id IN (SELECT id FROM clubs WHERE division_id IN ({}))
                 ORDER BY id LIMIT 2",
                g.manager_id,
                g.managed_sql(None)
            ))
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let [(keep, keep_club), (gone, gone_club)] = coaches[..] else { panic!("dois técnicos da IA") };
        for (coach, club, confidence) in [(keep, keep_club, 95), (gone, gone_club, 20)] {
            g.world.conn.execute("UPDATE people SET contract_expires = '2000-06-30', ambition = 1 WHERE id = ?", [coach]).unwrap();
            g.world.conn.execute("INSERT OR REPLACE INTO ai_boards (club_id, confidence) VALUES (?, ?)", [club, confidence]).unwrap();
        }
        let (mut fresh, mut next) = (Vec::new(), 100_000);
        g.manager_contract_day(&mut fresh, &mut next).unwrap();
        let club_of =
            |id: i32| -> Option<i32> { g.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [id], |r| r.get(0)).unwrap() };
        assert_eq!(club_of(keep), Some(keep_club), "renovou");
        let expires: String = g.world.conn.query_row("SELECT contract_expires FROM people WHERE id = ?", [keep], |r| r.get(0)).unwrap();
        assert!(expires > g.date);
        assert_eq!(club_of(gone), None, "saiu no fim do contrato");
        let vacant: i64 = g.world.conn.query_row("SELECT COUNT(*) FROM vacancies WHERE club_id = ?", [gone_club], |r| r.get(0)).unwrap();
        assert_eq!(vacant, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
