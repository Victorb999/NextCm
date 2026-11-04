//! Prémios e cláusulas do contrato (abas Prémios e Cláusulas do Propôr Contrato, pelos prints
//! do CLZ de 30/10). Prémios por golo marcado, por assistência e por jogo sem sofrer golos;
//! cláusulas de venda (um clube que ofereça o valor leva-o), de dispensa por despromoção, de
//! rescisão (sair se o clube não subir) e de dispensa por oferta de lugar de treinador. O
//! jogador exige alguns pela posição e pela personalidade; a resposta à proposta conta com
//! isso. Só nos contratos do treinador; os da IA não têm. Regras no doc do 3.15.

use rand_core::RngCore;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::deals::{Facts, round_wage};
use super::market::money_text;
use super::{Game, NewsItem};
use crate::live::PlayerLine;
use crate::{Result, finance};

/// Save 34: os prémios e cláusulas da proposta em curso e os dos contratos assinados.
pub(super) const EXTRAS_SCHEMA: &str = "
ALTER TABLE contract_talks ADD COLUMN extras TEXT;
CREATE TABLE contract_extras (person_id INTEGER PRIMARY KEY, club_id INTEGER NOT NULL, extras TEXT NOT NULL);
ALTER TABLE player_matches ADD COLUMN minute_on INTEGER;
ALTER TABLE player_matches ADD COLUMN minute_off INTEGER;
ALTER TABLE player_matches ADD COLUMN cards INTEGER NOT NULL DEFAULT 0;
";

/// Prémios pedidos, sobre o salário semanal pedido: por golo, por assistência e por jogo sem
/// sofrer golos.
const GOAL_BONUS: f64 = 0.06;
const ASSIST_BONUS: f64 = 0.03;
const CLEAN_SHEET_BONUS: f64 = 0.03;
/// Cláusula de venda: a que ele pede e a que o jogo sugere (vezes o valor de mercado).
const MIN_FEE_ASKED: f64 = 2.0;
const MIN_FEE_SUGGESTED: f64 = 3.0;
/// Atributos a partir dos quais pede o prémio por golo (Finalização) e por assistência (Passe).
const SCORER: i32 = 13;
const PROVIDER: i32 = 14;

/// Os prémios e as cláusulas de um contrato (0 / `None` / `false` = não tem).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ContractExtras {
    #[ts(type = "number")]
    pub goal_bonus: i64,
    #[ts(type = "number")]
    pub assist_bonus: i64,
    #[ts(type = "number")]
    pub clean_sheet_bonus: i64,
    /// Cláusula de venda: um clube que ofereça isto ou mais leva-o (o clube não pode recusar).
    #[ts(type = "number | null")]
    pub min_fee: Option<i64>,
    /// Cláusula de dispensa por despromoção: sai livre se o clube descer.
    pub relegation: bool,
    /// Cláusula de rescisão: sai livre se o clube, numa divisão abaixo da principal, não subir.
    pub non_promotion: bool,
    /// Cláusula de dispensa por oferta de lugar de treinador (fica no contrato; o efeito vem
    /// quando os jogadores puderem ser treinadores).
    pub manager_job: bool,
}

impl ContractExtras {
    /// O que falta nesta proposta para cobrir o que ele exige (os nomes, para a mensagem).
    pub(super) fn missing(&self, asks: &ContractExtras) -> Vec<&'static str> {
        let short = |offered: i64, asked: i64| asked > 0 && (offered as f64) < 0.9 * asked as f64;
        let mut out = Vec::new();
        if short(self.goal_bonus, asks.goal_bonus) {
            out.push("prémio por golo marcado");
        }
        if short(self.assist_bonus, asks.assist_bonus) {
            out.push("prémio por assistência");
        }
        if short(self.clean_sheet_bonus, asks.clean_sheet_bonus) {
            out.push("prémio por jogo sem sofrer golos");
        }
        if let Some(fee) = asks.min_fee
            && self.min_fee.is_none_or(|m| m > fee)
        {
            out.push("cláusula de venda");
        }
        if asks.relegation && !self.relegation {
            out.push("cláusula de dispensa por despromoção");
        }
        if asks.non_promotion && !self.non_promotion {
            out.push("cláusula de rescisão");
        }
        if asks.manager_job && !self.manager_job {
            out.push("cláusula de dispensa por oferta de lugar de treinador");
        }
        out
    }

    /// Esta proposta com o que ele exige (a contraproposta).
    pub(super) fn with_asks(&self, asks: &ContractExtras) -> ContractExtras {
        ContractExtras {
            goal_bonus: self.goal_bonus.max(asks.goal_bonus),
            assist_bonus: self.assist_bonus.max(asks.assist_bonus),
            clean_sheet_bonus: self.clean_sheet_bonus.max(asks.clean_sheet_bonus),
            min_fee: match (self.min_fee, asks.min_fee) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            },
            relegation: self.relegation || asks.relegation,
            non_promotion: self.non_promotion || asks.non_promotion,
            manager_job: self.manager_job || asks.manager_job,
        }
    }

    /// Em palavras, para as mensagens e a coluna Cláusulas do plantel.
    pub fn labels(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.goal_bonus > 0 {
            out.push(format!("{} por golo", money_text(self.goal_bonus)));
        }
        if self.assist_bonus > 0 {
            out.push(format!("{} por assistência", money_text(self.assist_bonus)));
        }
        if self.clean_sheet_bonus > 0 {
            out.push(format!("{} por jogo sem sofrer golos", money_text(self.clean_sheet_bonus)));
        }
        if let Some(fee) = self.min_fee {
            out.push(format!("Cláusula de venda de {}", money_text(fee)));
        }
        if self.relegation {
            out.push("Dispensa por despromoção".into());
        }
        if self.non_promotion {
            out.push("Rescisão se não subir".into());
        }
        if self.manager_job {
            out.push("Dispensa por oferta de lugar de treinador".into());
        }
        out
    }
}

impl Game {
    /// O que o jogador exige e os valores que o jogo sugere para o resto, para um salário.
    pub(super) fn extras_demand(&self, person: i32, f: &Facts, wage: i64) -> Result<(ContractExtras, ContractExtras)> {
        let (finishing, passing): (i32, i32) = self.world.conn.query_row(
            "SELECT COALESCE(finishing, 10), COALESCE(passing, 10) FROM players WHERE person_id = ?",
            [person],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let keeper = self.is_keeper(person)?;
        let mut rng = self.deal_rng(person, 0xC1A5);
        let mut roll = |p: f64| (rng.next_u32() % 1000) as f64 / 1000.0 < p;
        let bonus = |share: f64| round_wage(wage as f64 * share).max(100);
        let suggested = ContractExtras {
            goal_bonus: bonus(GOAL_BONUS),
            assist_bonus: bonus(ASSIST_BONUS),
            // O prémio por jogo sem sofrer golos é só dos guarda-redes (0 = não aparece na proposta)
            clean_sheet_bonus: if keeper { bonus(CLEAN_SHEET_BONUS) } else { 0 },
            min_fee: Some(finance::round_money(f.value.max(100_000) as f64 * MIN_FEE_SUGGESTED)),
            relegation: false,
            non_promotion: false,
            manager_job: false,
        };
        // A divisão do clube do treinador: há outra acima dela no país?
        let (division_rep, top_rep): (i32, i32) = self.world.conn.query_row(
            "SELECT COALESCE(d.reputation, 0),
                    COALESCE((SELECT MAX(k.reputation) FROM clubs o JOIN competitions k ON k.id = o.division_id
                              WHERE o.nation_id = c.nation_id), 0)
             FROM clubs c LEFT JOIN competitions d ON d.id = c.division_id WHERE c.id = ?",
            [self.club_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let lower_division = division_rep < top_rep;
        let ambitious = f.ambition >= 15.0;
        let asks = ContractExtras {
            goal_bonus: if !keeper && finishing >= SCORER && roll(0.6) { suggested.goal_bonus } else { 0 },
            assist_bonus: if !keeper && passing >= PROVIDER && roll(0.5) { suggested.assist_bonus } else { 0 },
            clean_sheet_bonus: if keeper && roll(0.4) { suggested.clean_sheet_bonus } else { 0 },
            min_fee: (f.ambition >= 14.0 && roll(0.5)).then(|| finance::round_money(f.value.max(100_000) as f64 * MIN_FEE_ASKED)),
            relegation: f.ambition >= 12.0 && !lower_division && roll(0.35),
            non_promotion: ambitious && lower_division && roll(0.5),
            manager_job: f.age >= 30 && roll(0.25),
        };
        Ok((asks, suggested))
    }

    /// Os prémios e cláusulas da proposta em curso.
    pub(super) fn talk_extras(&self, person: i32) -> Result<ContractExtras> {
        let text: Option<Option<String>> =
            self.world.conn.query_row("SELECT extras FROM contract_talks WHERE person_id = ?", [person], |r| r.get(0)).optional()?;
        Ok(text.flatten().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default())
    }

    /// Guarda-redes (para o prémio por jogo sem sofrer golos).
    fn is_keeper(&self, person: i32) -> Result<bool> {
        Ok(self
            .world
            .conn
            .query_row("SELECT COALESCE(pos_goalkeeper, 0) >= 15 FROM players WHERE person_id = ?", [person], |r| r.get(0))
            .optional()?
            .unwrap_or(false))
    }

    pub(super) fn set_talk_extras(&self, person: i32, extras: &ContractExtras) -> Result<()> {
        let mut extras = extras.clone();
        if !self.is_keeper(person)? {
            extras.clean_sheet_bonus = 0;
        }
        let text = serde_json::to_string(&extras).unwrap_or_default();
        self.world.conn.execute("UPDATE contract_talks SET extras = ? WHERE person_id = ?", params![text, person])?;
        Ok(())
    }

    /// O contrato assinado fica com os prémios e cláusulas da proposta.
    pub(super) fn sign_extras(&self, person: i32) -> Result<()> {
        let extras = self.talk_extras(person)?;
        if extras == ContractExtras::default() {
            self.world.conn.execute("DELETE FROM contract_extras WHERE person_id = ?", [person])?;
        } else {
            let text = serde_json::to_string(&extras).unwrap_or_default();
            self.world.conn.execute(
                "INSERT OR REPLACE INTO contract_extras (person_id, club_id, extras) VALUES (?, ?, ?)",
                params![person, self.club_id, text],
            )?;
        }
        Ok(())
    }

    /// Os prémios e cláusulas do contrato de um jogador (só valem no clube onde os assinou).
    pub fn contract_extras(&self, person: i32) -> Result<Option<ContractExtras>> {
        let row: Option<(i32, String)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT e.club_id, e.extras FROM contract_extras e JOIN people p ON p.id = e.person_id
                 WHERE e.person_id = ? AND p.club_id = e.club_id",
            )?
            .query_row([person], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        Ok(row.and_then(|(_, t)| serde_json::from_str(&t).ok()))
    }

    /// Depois de um jogo do clube do treinador: os prémios por golo, assistência e jogo sem
    /// sofrer golos (só a quem foi titular).
    pub(super) fn pay_match_bonuses(&self, club: i32, lines: &[PlayerLine], conceded: u32) -> Result<()> {
        if club != self.club_id {
            return Ok(());
        }
        let mut total = 0;
        for l in lines {
            let Some(e) = self.contract_extras(l.id)? else { continue };
            total += e.goal_bonus * l.goals as i64 + e.assist_bonus * l.assists as i64;
            if conceded == 0 && !l.sub && self.is_keeper(l.id)? {
                total += e.clean_sheet_bonus;
            }
        }
        self.book(club, "match_bonuses", total)
    }

    /// Cláusula de venda: uma proposta recebida que a paga não se pode recusar; o comprador
    /// passa logo a negociar com o jogador.
    pub(super) fn release_clause_offers(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        for o in self.offers_where("seller = ?1 AND kind = 'transfer' AND status IN ('pending', 'countered')", params![self.club_id])? {
            let Some(fee) = self.contract_extras(o.person)?.and_then(|e| e.min_fee) else { continue };
            if o.fee < fee {
                continue;
            }
            let until = self.start_sale_talks(&o)?;
            let name = self.person_name(o.person)?;
            let buyer = self.club(o.buyer).short_name;
            let body = format!(
                "O {buyer} pagou a cláusula de venda de {name} ({}): o clube não pode recusar. O {buyer} negoceia agora o contrato com o jogador; a resposta chega a {}.",
                money_text(fee),
                super::cm_date(&until)
            );
            self.offer_news(fresh, next_id, format!("Cláusula de {name} paga"), body, o.id);
        }
        Ok(())
    }

    /// No fim da época, as cláusulas de dispensa por despromoção e de rescisão: quem as tem
    /// sai livre se o clube desceu (ou, abaixo da divisão principal, se não subiu).
    pub(super) fn release_by_clauses(&mut self, before: Option<i32>, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let after = self.division_of(self.club_id)?;
        let rep = |d: Option<i32>| -> Result<i32> {
            Ok(match d {
                Some(d) => self.world.conn.query_row("SELECT COALESCE(reputation, 0) FROM competitions WHERE id = ?", [d], |r| r.get(0))?,
                None => 0,
            })
        };
        let (old, new) = (rep(before)?, rep(after)?);
        let top: i32 = self.world.conn.query_row(
            "SELECT COALESCE(MAX(k.reputation), 0) FROM clubs o JOIN competitions k ON k.id = o.division_id
             WHERE o.nation_id = (SELECT nation_id FROM clubs WHERE id = ?)",
            [self.club_id],
            |r| r.get(0),
        )?;
        let relegated = new < old;
        let stayed_below = before == after && old < top;
        if !relegated && !stayed_below {
            return Ok(());
        }
        let squad: Vec<i32> = self
            .world
            .conn
            .prepare("SELECT person_id FROM contract_extras WHERE club_id = ? ORDER BY person_id")?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for person in squad {
            let Some(e) = self.contract_extras(person)? else { continue };
            let why = if relegated && e.relegation {
                "a cláusula de dispensa por despromoção"
            } else if stayed_below && e.non_promotion {
                "a cláusula de rescisão (o clube não subiu)"
            } else {
                continue;
            };
            let name = self.person_name(person)?;
            self.world.conn.execute("UPDATE people SET club_id = NULL, club_job = NULL, contract_expires = NULL WHERE id = ?", [person])?;
            self.number_after_move(person, None)?;
            self.log_transfer(person, Some(self.club_id), None, 0, "release")?;
            for table in ["transfer_list", "squad_roles", "shortlist", "contract_extras"] {
                self.world.conn.execute(&format!("DELETE FROM {table} WHERE person_id = ?"), [person])?;
            }
            let body = format!("{name} usou {why} e deixou o clube a custo zero.");
            self.push_news(fresh, next_id, "contract", format!("{name} sai pela cláusula"), body);
        }
        self.squad_changed()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::league::{SERIE_A, SERIE_B};

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn bonuses_and_clauses_in_a_contract() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-clausulas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Cláusulas".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
        };
        let mut g = Game::create(&path, &dir, &new, 31).unwrap();
        let squad: Vec<i32> = g
            .world
            .conn
            .prepare("SELECT id FROM people WHERE club_id = ? AND is_player = 1 ORDER BY id")
            .unwrap()
            .query_map([PALMEIRAS], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        // Um que exige alguma coisa na renovação
        let (p, t) = squad
            .iter()
            .find_map(|&p| g.contract_terms(p, None).ok().filter(|t| t.asks != ContractExtras::default()).map(|t| (p, t)))
            .expect("alguém exige prémios ou cláusulas");
        let next_day = |g: &mut Game| {
            g.date = crate::date::add_days(&g.date, 1).unwrap();
            let (mut fresh, mut id) = (Vec::new(), 9000);
            g.contract_replies(&mut fresh, &mut id).unwrap();
            fresh
        };
        // Sem o que exige, faz uma contraproposta com isso, mesmo com o salário pedido
        let wage = t.wage * 12 / 10;
        g.negotiate(p, &t.role, wage, t.bonus * 2, t.years, &ContractExtras::default()).unwrap();
        let news = next_day(&mut g);
        assert!(news.iter().any(|n| n.body.contains("Exige também")), "{news:?}");
        assert!(g.talk_extras(p).unwrap().missing(&t.asks).is_empty(), "a contraproposta traz as exigências");
        let offer = g.open_contract(p).unwrap().unwrap();
        assert_eq!(g.answer_offer(offer.id, "accept", None).unwrap().status, "done");
        assert_eq!(g.contract_extras(p).unwrap().unwrap().missing(&t.asks), Vec::<&str>::new());

        // Prémios pagos depois do jogo: 2 golos e um jogo sem sofrer golos (de um guarda-redes)
        let scorer = squad.iter().copied().find(|&x| x != p && g.is_keeper(x).unwrap()).unwrap();
        let extras =
            ContractExtras { goal_bonus: 1000, clean_sheet_bonus: 500, min_fee: Some(1_000_000), relegation: true, ..Default::default() };
        let text = serde_json::to_string(&extras).unwrap();
        g.world
            .conn
            .execute("INSERT INTO contract_extras (person_id, club_id, extras) VALUES (?, ?, ?)", params![scorer, PALMEIRAS, text])
            .unwrap();
        let line = PlayerLine {
            id: scorer,
            rating: 8.0,
            goals: 2,
            assists: 0,
            sub: false,
            stats: Default::default(),
            on: 0,
            off: None,
            yellow: false,
            red: false,
        };
        let cash = g.cash_of(PALMEIRAS).unwrap();
        g.pay_match_bonuses(PALMEIRAS, &[line], 0).unwrap();
        assert_eq!(g.cash_of(PALMEIRAS).unwrap(), cash - 2500);

        // Cláusula de venda paga: a proposta passa logo às conversas e não se pode recusar
        g.world
            .conn
            .execute(
                "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('transfer', ?, 7570, ?, 1500000, 'pending', ?, ?)",
                params![scorer, PALMEIRAS, g.date, g.date],
            )
            .unwrap();
        let id = g.world.conn.last_insert_rowid() as i32;
        assert!(g.answer_offer(id, "reject", None).is_err());
        let (mut fresh, mut next) = (Vec::new(), 9500);
        g.release_clause_offers(&mut fresh, &mut next).unwrap();
        assert_eq!(g.offer_view(id).unwrap().status, "talks");
        g.world.conn.execute("UPDATE offers SET status = 'withdrawn' WHERE id = ?", [id]).unwrap();

        // Dispensa por despromoção: o clube desce e ele sai livre
        g.world.conn.execute("UPDATE clubs SET division_id = ? WHERE id = ?", params![SERIE_B, PALMEIRAS]).unwrap();
        g.release_by_clauses(Some(SERIE_A), &mut fresh, &mut next).unwrap();
        let club: Option<i32> = g.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [scorer], |r| r.get(0)).unwrap();
        assert_eq!(club, None);
        assert!(fresh.iter().any(|n| n.title.contains("sai pela cláusula")));
        // O fecho da janela: o prazo e a confiança da direção
        let mut closed = Vec::new();
        g.window_closed_news(&mut closed, &mut next).unwrap();
        assert_eq!(closed[0].title, "Prazo limite para transferências");
        assert!(closed[0].body.contains("Brazil"));
        assert!(closed[1].title.starts_with("Actualização") && closed[1].body.contains("direcção"));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_is_missing_and_the_counter() {
        let asks = ContractExtras { goal_bonus: 5000, min_fee: Some(50_000_000), relegation: true, ..Default::default() };
        let offer = ContractExtras { goal_bonus: 4000, min_fee: Some(60_000_000), ..Default::default() };
        assert_eq!(offer.missing(&asks), ["prémio por golo marcado", "cláusula de venda", "cláusula de dispensa por despromoção"]);
        let counter = offer.with_asks(&asks);
        assert!(counter.missing(&asks).is_empty());
        assert_eq!(counter.min_fee, Some(50_000_000));
        // Uma cláusula de venda mais baixa do que a pedida também serve
        let lower = ContractExtras { goal_bonus: 4600, min_fee: Some(40_000_000), relegation: true, ..Default::default() };
        assert!(lower.missing(&asks).is_empty());
        assert_eq!(counter.labels().len(), 3);
    }
}
