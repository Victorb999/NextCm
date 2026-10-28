//! Reunião com a Federação (marco 3.11, passo 3): "Especifique as questões que deseja discutir
//! com a Liga". Queixas sobre a arbitragem (depois de um jogo com expulsões ou muitos cartões) e
//! pedir o adiamento de um jogo (com muitos lesionados ou jogos seguidos). As opções que não
//! fazem sentido agora aparecem apagadas. Regras e números no detalhe do 3.11 no ROADMAP.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use rusqlite::{OptionalExtension, params};

use super::board_meeting::BoardRequest;
use super::market::money_text;
use super::{Game, cm_date};
use crate::{Error, Result, date, finance};

/// Dias depois do jogo em que ainda se pode reclamar da arbitragem.
const COMPLAINT_DAYS: i32 = 7;
/// Amarelos num jogo que já dão para reclamar.
const COMPLAINT_YELLOWS: usize = 4;
/// Multa por declarações (parte das receitas de uma época).
const FINE_SHARE: f64 = 0.002;
/// Adiamento: o jogo tem de ser nos próximos dias; lesionados ou jogos em 7 dias que o
/// justificam; dias para a nova data; dias entre pedidos; probabilidade de a liga aceitar.
const POSTPONE_AHEAD: i32 = 7;
const POSTPONE_INJURED: usize = 5;
const POSTPONE_BUSY: usize = 3;
const POSTPONE_GAP: i32 = 2;
const POSTPONE_COOLDOWN: i32 = 60;
const POSTPONE_YES: f64 = 0.25;

/// O que o jogo do treinador deixou para uma queixa (guardado em `league_issues`).
pub(super) struct Grievance {
    pub fixture: i32,
    pub sent_off: Option<i32>,
    pub yellows: usize,
    pub competition: i32,
}

impl Game {
    /// Depois de um jogo do treinador: guarda o que dá para reclamar.
    pub(super) fn note_grievance(&self, g: &Grievance) -> Result<()> {
        if g.sent_off.is_none() && g.yellows < COMPLAINT_YELLOWS {
            return Ok(());
        }
        self.world.conn.execute(
            "INSERT OR REPLACE INTO league_issues (fixture_id, date, competition_id, sent_off, yellows, used) VALUES (?, ?, ?, ?, ?, 0)",
            params![g.fixture, self.date, g.competition, g.sent_off, g.yellows as i32],
        )?;
        Ok(())
    }

    /// O último jogo com motivo de queixa ainda por usar: (jogo, competição, expulso).
    fn open_grievance(&self) -> Result<Option<(i32, i32, Option<i32>)>> {
        let since = date::add_days(&self.date, -COMPLAINT_DAYS).unwrap_or_default();
        Ok(self
            .world
            .conn
            .query_row(
                "SELECT fixture_id, competition_id, sent_off FROM league_issues WHERE used = 0 AND date >= ? ORDER BY date DESC LIMIT 1",
                [since],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?)
    }

    /// O próximo jogo de liga (pontos corridos) do treinador nos próximos dias: índice em `matches`.
    fn postponable(&self) -> Option<usize> {
        let until = date::add_days(&self.date, POSTPONE_AHEAD)?;
        (0..self.matches.len())
            .filter(|&i| {
                let m = &self.matches[i];
                m.result.is_none()
                    && m.tie.is_none()
                    && m.date.as_str() > self.date.as_str()
                    && m.date <= until
                    && (m.home_id == self.club_id || m.away_id == self.club_id)
            })
            .min_by(|&a, &b| self.matches[a].date.cmp(&self.matches[b].date))
    }

    /// Lesionados do plantel e jogos do clube nos próximos 7 dias.
    fn congestion(&self) -> (usize, usize) {
        let injured =
            self.squads.traits.iter().filter(|(id, t)| t.club_id == self.club_id && self.squads.state(**id).injury.is_some()).count();
        let until = date::add_days(&self.date, 7).unwrap_or_default();
        let busy = self
            .matches
            .iter()
            .filter(|m| {
                m.result.is_none()
                    && m.date.as_str() >= self.date.as_str()
                    && m.date <= until
                    && (m.home_id == self.club_id || m.away_id == self.club_id)
            })
            .count();
        (injured, busy)
    }

    /// As questões da reunião, com o que dá para pedir agora.
    pub fn league_requests(&self) -> Result<Vec<BoardRequest>> {
        let referee = match self.open_grievance()? {
            Some(_) => None,
            None => Some("Só depois de um jogo com expulsões ou muitos cartões (até 7 dias depois).".to_string()),
        };
        let last: Option<String> =
            self.world.conn.query_row("SELECT last FROM board_requests WHERE kind = 'postpone'", [], |r| r.get(0)).optional()?;
        let (injured, busy) = self.congestion();
        let postpone =
            if let Some(until) = last.and_then(|d| date::add_days(&d, POSTPONE_COOLDOWN)).filter(|u| u.as_str() > self.date.as_str()) {
                Some(format!("A Liga já recusou um pedido há pouco: volte a pedir depois de {}.", cm_date(&until)))
            } else if self.postponable().is_none() {
                Some("Não tem nenhum jogo do campeonato nos próximos dias.".to_string())
            } else if injured < POSTPONE_INJURED && busy < POSTPONE_BUSY {
                Some("A Liga só adia jogos com muitos lesionados ou jogos seguidos.".to_string())
            } else {
                None
            };
        Ok(vec![
            BoardRequest {
                key: "referee".into(),
                label: "Queixas sobre a arbitragem".into(),
                available: referee.is_none(),
                reason: referee,
            },
            BoardRequest {
                key: "postpone".into(),
                label: "Pedir adiamento do jogo".into(),
                available: postpone.is_none(),
                reason: postpone,
            },
        ])
    }

    /// Leva uma questão à Liga; devolve a resposta (que chega também como mensagem).
    pub fn league_request(&mut self, key: &str) -> Result<String> {
        let req =
            self.league_requests()?.into_iter().find(|r| r.key == key).ok_or_else(|| Error::Invalid("Questão desconhecida.".into()))?;
        if !req.available {
            return Err(Error::Invalid(req.reason.unwrap_or_default()));
        }
        let (accepted, text) = if key == "referee" {
            let (fixture, competition, sent_off) = self.open_grievance()?.ok_or_else(|| Error::Invalid("Sem queixa.".into()))?;
            self.world.conn.execute("UPDATE league_issues SET used = 1 WHERE fixture_id = ?", [fixture])?;
            let mut rng = ChaCha8Rng::seed_from_u64((self.seed as u64) ^ 0xA2B1_7E00 ^ fixture as u64);
            let roll = (rng.next_u64() % 100) as i32;
            match (roll, sent_off) {
                (0..=24, Some(player)) => {
                    // O castigo da expulsão é anulado
                    let name = self.person_name(player)?;
                    if let Some(k) = self.squads.states.entry(player).or_default().cards.get_mut(&competition) {
                        k.suspended = (k.suspended - 1).max(0);
                    }
                    (true, format!("A Liga analisou o jogo e deu-lhe razão: o castigo de {name} foi anulado."))
                }
                (0..=69, _) => (false, "A Liga tomou nota da sua queixa sobre a arbitragem, mas não vai tomar nenhuma medida.".to_string()),
                _ => {
                    let fine = finance::round_money(FINE_SHARE * self.club_base(self.club_id)?).max(1_000);
                    self.book(self.club_id, "fines", fine)?;
                    (false, format!("A Liga não gostou das suas declarações sobre a arbitragem e multou o clube em {}.", money_text(fine)))
                }
            }
        } else {
            let i = self.postponable().ok_or_else(|| Error::Invalid("Sem jogo.".into()))?;
            self.world.conn.execute("INSERT OR REPLACE INTO board_requests (kind, last) VALUES ('postpone', ?)", [&self.date])?;
            let m = self.matches[i].clone();
            let mut rng = ChaCha8Rng::seed_from_u64((self.seed as u64) ^ 0x9057_9013 ^ m.id as u64);
            let yes = (rng.next_u64() % 1000) as f64 / 1000.0 < POSTPONE_YES;
            let opponent = self.club(if m.home_id == self.club_id { m.away_id } else { m.home_id }).short_name;
            match self.free_date(&m).filter(|_| yes) {
                Some(new) => {
                    self.matches[i].date = new.clone();
                    (true, format!("A Liga aceitou adiar o jogo com o {opponent}: passa de {} para {}.", cm_date(&m.date), cm_date(&new)))
                }
                None => (false, format!("A Liga recusou adiar o jogo com o {opponent} de {}.", cm_date(&m.date))),
            }
        };
        let title = if accepted { "Pedido à Liga aceite" } else { "Resposta da Liga" };
        self.post_news(title.into(), format!("{}\n\n{text}", req.label), None);
        self.unsaved = true;
        Ok(text)
    }

    /// Uma data nova para um jogo adiado: a primeira, entre 7 e 28 dias depois, em que nenhum dos
    /// dois clubes joga a 2 dias de distância, e antes do último jogo da competição.
    fn free_date(&self, m: &crate::league::Match) -> Option<String> {
        let last = self.matches.iter().filter(|x| x.competition_id == m.competition_id).map(|x| x.date.clone()).max()?;
        (7..=28).find_map(|d| {
            let day = date::add_days(&m.date, d)?;
            if day >= last {
                return None;
            }
            let near = |x: &crate::league::Match| {
                let gap = date::parse(&x.date).zip(date::parse(&day)).map_or(i32::MAX, |(a, b)| (a - b).abs());
                x.id != m.id && gap <= POSTPONE_GAP && [x.home_id, x.away_id].iter().any(|c| *c == m.home_id || *c == m.away_id)
            };
            (!self.matches.iter().any(near)).then_some(day)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn requests_to_the_league() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-liga-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Liga".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
            leagues: Vec::new(),
            start: Default::default(),
        };
        let mut g = Game::create(&path, &dir, &new, 19).unwrap();
        // Sem motivo, as duas questões ficam apagadas
        let reqs = g.league_requests().unwrap();
        assert!(reqs.iter().all(|r| !r.available), "{reqs:?}");
        assert!(g.league_request("referee").is_err());

        // Uma expulsão no último jogo: a queixa abre e só se faz uma vez
        let player: i32 =
            g.world.conn.query_row("SELECT id FROM people WHERE club_id = ? AND is_player = 1 LIMIT 1", [PALMEIRAS], |r| r.get(0)).unwrap();
        g.note_grievance(&Grievance { fixture: 1, sent_off: Some(player), yellows: 2, competition: 1 }).unwrap();
        assert!(g.league_requests().unwrap()[0].available);
        let answer = g.league_request("referee").unwrap();
        assert!(answer.contains("Liga"), "{answer}");
        assert!(!g.league_requests().unwrap()[0].available);

        // Adiamento: com 5 lesionados e um jogo de liga nos próximos dias
        let next = g
            .matches
            .iter()
            .filter(|m| m.tie.is_none() && (m.home_id == PALMEIRAS || m.away_id == PALMEIRAS))
            .map(|m| m.date.clone())
            .min()
            .unwrap();
        g.date = date::add_days(&next, -3).unwrap();
        let ids: Vec<i32> = g.squads.traits.iter().filter(|(_, t)| t.club_id == PALMEIRAS).map(|(id, _)| *id).take(5).collect();
        for id in ids {
            g.squads.states.entry(id).or_default().injury = Some("Entorse no tornozelo".into());
        }
        assert!(g.league_requests().unwrap()[1].available, "{:?}", g.league_requests().unwrap()[1]);
        let answer = g.league_request("postpone").unwrap();
        assert!(answer.contains("adiar"), "{answer}");
        if answer.contains("aceitou") {
            assert!(!g.matches.iter().any(|m| m.date == next && (m.home_id == PALMEIRAS || m.away_id == PALMEIRAS) && m.tie.is_none()));
        }
        assert!(!g.league_requests().unwrap()[1].available, "não volta a pedir já");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
