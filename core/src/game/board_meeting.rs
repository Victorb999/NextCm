//! Reunião com a Direcção (marco 3.11, passo 2): "Especificar os detalhes das exigências à
//! direcção". Cinco pedidos (estádio, camadas jovens, fundos para transferências, fundos para
//! salários, mais tempo); os que não fazem sentido agora aparecem apagados, com o motivo. A
//! resposta chega já, também como mensagem. Regras e números em docs/roadmap/3.11-direccao-e-federacao.md.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::market::money_text;
use super::{Game, NewsItem, cm_date};
use crate::{Error, Result, date, finance};

/// Um pedido da reunião: chave, texto do CM e se dá para pedir agora (senão, porquê).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct BoardRequest {
    pub key: String,
    pub label: String,
    pub available: bool,
    pub reason: Option<String>,
}

const REQUESTS: [(&str, &str, i32); 5] = [
    ("stadium", "Expandir o Estádio", 365),
    ("youth", "Investir nas camadas jovens", 365),
    ("transfers", "Disponibilizar mais fundos para transferências", 90),
    ("wages", "Disponibilizar mais fundos para salários e prémios", 90),
    ("time", "Mais tempo para reconstruir o plantel", 365),
];

/// Estádio: lugares a mais (parte da lotação, entre os limites), o custo por lugar e os dias da obra.
const STADIUM_SHARE: f64 = 0.15;
const STADIUM_SEATS: (i32, i32) = (2_000, 10_000);
const SEAT_COST: i64 = 2_500;
const STADIUM_DAYS: i32 = 300;
/// Camadas jovens: o custo (parte das receitas de uma época), o máximo de investimentos e o que
/// cada um junta ao potencial dos juniores do clube.
const YOUTH_COST: f64 = 0.03;
pub(super) const YOUTH_MAX: i32 = 3;
pub(super) const YOUTH_PA: f64 = 6.0;
/// Mais tempo: dias em que as quedas de confiança contam metade, e lugares a menos no objetivo.
const GRACE_DAYS: i32 = 120;
const GRACE_TARGET: i32 = 3;
/// Recusa com a confiança abaixo disto: a direção não gosta de ser incomodada.
const PUSHY_BELOW: i32 = 40;
const PUSHY_COST: i32 = 3;

impl Game {
    fn last_request(&self, key: &str) -> Result<Option<String>> {
        Ok(self.world.conn.query_row("SELECT last FROM board_requests WHERE kind = ?", [key], |r| r.get(0)).optional()?)
    }

    fn board_numbers(&self) -> Result<(i32, i64, f64, Option<String>, i32)> {
        let (confidence, grace, youth): (i32, Option<String>, i32) =
            self.world.conn.query_row("SELECT confidence, grace_until, youth_level FROM board WHERE id = 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?;
        let cash: i64 = self.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [self.club_id], |r| r.get(0))?;
        Ok((confidence, cash, self.club_base(self.club_id)?.max(1.0), grace, youth))
    }

    /// Lugares e custo de uma ampliação do estádio do clube.
    fn stadium_plan(&self) -> Result<Option<(i32, i32, i64)>> {
        let row: Option<(Option<i32>, Option<i32>)> = self
            .world
            .conn
            .query_row(
                "SELECT c.stadium_id, s.capacity FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id WHERE c.id = ?",
                [self.club_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(row.and_then(|(stadium, capacity)| {
            let stadium = stadium?;
            let seats = ((capacity.unwrap_or(15_000) as f64 * STADIUM_SHARE) as i32).clamp(STADIUM_SEATS.0, STADIUM_SEATS.1) / 100 * 100;
            Some((stadium, seats, seats as i64 * SEAT_COST))
        }))
    }

    /// Os pedidos da reunião, com o que está disponível agora.
    pub fn board_requests(&self) -> Result<Vec<BoardRequest>> {
        let (confidence, cash, base, grace, youth) = self.board_numbers()?;
        let works: bool = self.world.conn.query_row("SELECT COUNT(*) FROM stadium_works", [], |r| r.get::<_, i64>(0))? > 0;
        let mut out = Vec::new();
        for (key, label, cooldown) in REQUESTS {
            let recent =
                self.last_request(key)?.and_then(|d| date::add_days(&d, cooldown)).filter(|until| until.as_str() > self.date.as_str());
            let reason = if let Some(until) = recent {
                Some(format!("Já pediu isto há pouco: volte a pedir depois de {}.", cm_date(&until)))
            } else {
                match key {
                    "stadium" if works => Some("O estádio já está em obras.".into()),
                    "stadium" if self.stadium_plan()?.is_none() => Some("O clube não tem estádio próprio.".into()),
                    "stadium" if cash <= 0 => Some("O clube não tem dinheiro para obras.".into()),
                    "youth" if youth >= YOUTH_MAX => Some("A direcção já investiu tudo o que podia nas camadas jovens.".into()),
                    "youth" if (cash as f64) < YOUTH_COST * base => Some("O clube não tem dinheiro para investir.".into()),
                    "time" if confidence >= 50 => Some("A direcção está contente consigo: não precisa de mais tempo.".into()),
                    "time" if grace.as_deref().is_some_and(|g| g > self.date.as_str()) => Some("Já tem mais tempo da direcção.".into()),
                    _ => None,
                }
            };
            out.push(BoardRequest { key: key.into(), label: label.into(), available: reason.is_none(), reason });
        }
        Ok(out)
    }

    /// Faz um pedido à direção; devolve a resposta (que chega também como mensagem).
    pub fn board_request(&mut self, key: &str) -> Result<String> {
        let req = self.board_requests()?.into_iter().find(|r| r.key == key).ok_or_else(|| Error::Invalid("Pedido desconhecido.".into()))?;
        if !req.available {
            return Err(Error::Invalid(req.reason.unwrap_or_default()));
        }
        let (confidence, cash, base, _, youth) = self.board_numbers()?;
        let (accepted, text) = match key {
            "stadium" => {
                let (stadium, seats, cost) = self.stadium_plan()?.ok_or_else(|| Error::Invalid("Sem estádio.".into()))?;
                if confidence < 55 {
                    (false, "A direcção não confia o suficiente no seu trabalho para investir no estádio.".to_string())
                } else if (cash as f64) < cost as f64 * 1.2 {
                    (
                        false,
                        format!(
                            "A direcção gostaria de ampliar o estádio, mas a obra custa {} e o clube não tem esse dinheiro.",
                            money_text(cost)
                        ),
                    )
                } else {
                    let done = date::add_days(&self.date, STADIUM_DAYS).unwrap_or_default();
                    self.book(self.club_id, "stadium", cost)?;
                    self.world.conn.execute(
                        "INSERT INTO stadium_works (stadium_id, seats, done_on) VALUES (?, ?, ?)",
                        params![stadium, seats, done],
                    )?;
                    (
                        true,
                        format!(
                            "A direcção aceitou ampliar o estádio em {seats} lugares, por {}. A obra fica pronta a {}.",
                            money_text(cost),
                            cm_date(&done)
                        ),
                    )
                }
            }
            "youth" => {
                if confidence < 50 {
                    (false, "A direcção prefere não investir nas camadas jovens enquanto não confiar mais no seu trabalho.".to_string())
                } else {
                    let cost = finance::round_money(YOUTH_COST * base);
                    self.book(self.club_id, "youth", cost)?;
                    self.world.conn.execute("UPDATE board SET youth_level = ? WHERE id = 1", [youth + 1])?;
                    (
                        true,
                        format!(
                            "A direcção aceitou investir {} nas camadas jovens. Os próximos juniores do clube vão ser melhores.",
                            money_text(cost)
                        ),
                    )
                }
            }
            "transfers" => {
                let budget = self.transfer_budget()?;
                let room = cash - budget;
                let extra = if confidence >= 60 {
                    0.25
                } else if confidence >= 50 {
                    0.1
                } else {
                    0.0
                };
                let more =
                    finance::round_money((budget as f64 * extra).max(if extra > 0.0 { 250_000.0 } else { 0.0 }).min(room.max(0) as f64));
                if more <= 0 {
                    (false, "A direcção recusou: não há fundos para mais transferências.".to_string())
                } else {
                    self.spend_budget(-more)?;
                    let part = if extra < 0.25 { " em parte" } else { "" };
                    (
                        true,
                        format!(
                            "A direcção aceitou{part}: o orçamento de transferências sobe {} para {}.",
                            money_text(more),
                            money_text(budget + more)
                        ),
                    )
                }
            }
            "wages" => {
                let (wage_budget, max_wage): (i64, i64) =
                    self.world
                        .conn
                        .query_row("SELECT wage_budget, max_wage FROM board WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
                let extra = if confidence >= 60 && cash > 0 {
                    0.1
                } else if confidence >= 50 && cash > 0 {
                    0.05
                } else {
                    0.0
                };
                if extra == 0.0 {
                    (false, "A direcção recusou: o clube não pode pagar mais salários.".to_string())
                } else {
                    let round = |v: f64| (v as i64) / 1000 * 1000;
                    let (w, m) = (round(wage_budget as f64 * (1.0 + extra)), round(max_wage as f64 * (1.0 + extra)));
                    self.world.conn.execute("UPDATE board SET wage_budget = ?, max_wage = ? WHERE id = 1", params![w, m])?;
                    let part = if extra < 0.1 { " em parte" } else { "" };
                    (
                        true,
                        format!(
                            "A direcção aceitou{part}: a folha salarial pode ir até {} por semana e o salário máximo até {}.",
                            money_text(w),
                            money_text(m)
                        ),
                    )
                }
            }
            _ => {
                if confidence < 20 {
                    (false, "A direcção recusou: já lhe deu tempo suficiente.".to_string())
                } else {
                    let until = date::add_days(&self.date, GRACE_DAYS).unwrap_or_default();
                    let total: i32 = self.world.conn.query_row(
                        "SELECT COUNT(*) FROM clubs WHERE division_id = (SELECT division_id FROM clubs WHERE id = ?)",
                        [self.club_id],
                        |r| r.get(0),
                    )?;
                    self.world.conn.execute(
                        "UPDATE board SET grace_until = ?, target = MIN(?, target + ?) WHERE id = 1",
                        params![until, total.max(1), GRACE_TARGET],
                    )?;
                    (
                        true,
                        format!(
                            "A direcção aceitou dar-lhe mais tempo para reconstruir o plantel, até {}, e baixou o objetivo da época.",
                            cm_date(&until)
                        ),
                    )
                }
            }
        };
        if !accepted && confidence < PUSHY_BELOW {
            self.world.conn.execute("UPDATE board SET confidence = MAX(1, confidence - ?) WHERE id = 1", [PUSHY_COST])?;
        }
        self.world.conn.execute("INSERT OR REPLACE INTO board_requests (kind, last) VALUES (?, ?)", params![key, self.date])?;
        let title = if accepted { "Pedido à Direcção aceite" } else { "Pedido à Direcção recusado" };
        self.post_news(title.into(), format!("{}\n\n{text}", req.label), None);
        self.unsaved = true;
        Ok(text)
    }

    /// As obras do estádio que acabam hoje.
    pub(super) fn stadium_works_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let done: Vec<(i32, i32, i32)> = self
            .world
            .conn
            .prepare_cached("SELECT rowid, stadium_id, seats FROM stadium_works WHERE done_on <= ?")?
            .query_map([&self.date], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (row, stadium, seats) in done {
            self.world.conn.execute("UPDATE stadiums SET capacity = COALESCE(capacity, 0) + ? WHERE id = ?", params![seats, stadium])?;
            self.world.conn.execute("DELETE FROM stadium_works WHERE rowid = ?", [row])?;
            let capacity: i32 = self.world.conn.query_row("SELECT capacity FROM stadiums WHERE id = ?", [stadium], |r| r.get(0))?;
            let body = format!("As obras de ampliação acabaram: o estádio tem agora {capacity} lugares ({seats} novos).");
            fresh.push(self.news_item(*next_id, "message", "Estádio ampliado".into(), body));
            *next_id += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn requests_to_the_board() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-reuniao-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Reunião".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
            leagues: Vec::new(),
            start: Default::default(),
        };
        let mut g = Game::create(&path, &dir, &new, 17).unwrap();
        g.world.conn.execute("UPDATE clubs SET cash = 500000000 WHERE id = ?", [PALMEIRAS]).unwrap();
        g.world.conn.execute("UPDATE board SET confidence = 70 WHERE id = 1", []).unwrap();
        g.refresh_budgets().unwrap();
        let reqs = g.board_requests().unwrap();
        assert_eq!(reqs.len(), 5);
        assert!(!reqs.iter().find(|r| r.key == "time").unwrap().available, "contente: não precisa de tempo");

        // Mais fundos: o orçamento sobe 25%; pedir outra vez já não dá
        let budget = g.transfer_budget().unwrap();
        assert!(g.board_request("transfers").unwrap().contains("aceitou"));
        assert!(g.transfer_budget().unwrap() > budget);
        assert!(g.board_request("transfers").is_err());

        // Estádio: paga já e a obra acaba ao fim de 300 dias
        let capacity = |g: &Game| -> i32 {
            g.world
                .conn
                .query_row("SELECT s.capacity FROM clubs c JOIN stadiums s ON s.id = c.stadium_id WHERE c.id = ?", [PALMEIRAS], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        let before = capacity(&g);
        assert!(g.board_request("stadium").unwrap().contains("aceitou"));
        assert!(!g.board_requests().unwrap().iter().find(|r| r.key == "stadium").unwrap().available);
        g.date = date::add_days(&g.date, STADIUM_DAYS).unwrap();
        let (mut fresh, mut id) = (Vec::new(), 1000);
        g.stadium_works_day(&mut fresh, &mut id).unwrap();
        assert!(capacity(&g) > before);
        assert!(fresh.iter().any(|n| n.title == "Estádio ampliado"));

        // Camadas jovens
        assert!(g.board_request("youth").unwrap().contains("aceitou"));
        let level: i32 = g.world.conn.query_row("SELECT youth_level FROM board WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(level, 1);

        // Mais tempo, com a confiança baixa: as quedas contam metade
        g.world.conn.execute("UPDATE board SET confidence = 30 WHERE id = 1", []).unwrap();
        let target: i32 = g.world.conn.query_row("SELECT target FROM board WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert!(g.board_request("time").unwrap().contains("mais tempo"));
        let after: i32 = g.world.conn.query_row("SELECT target FROM board WHERE id = 1", [], |r| r.get(0)).unwrap();
        assert!(after > target);
        let (mut fresh, mut id) = (Vec::new(), 1000);
        g.change_confidence(-10, &mut fresh, &mut id).unwrap();
        assert_eq!(g.board_view().unwrap().confidence, 25);
        assert!(g.news().iter().any(|n| n.title == "Pedido à Direcção aceite"));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
