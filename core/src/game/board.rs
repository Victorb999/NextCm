//! Direção (marco 3.8, passo 4): a expectativa da época, a confiança no treinador (avisos e
//! despedimento) e os orçamentos de transferências e salarial do clube do treinador. As
//! dívidas dos clubes da IA (vendas forçadas e falência) ficam no mercado e no fim de época.
//! Regras e números no detalhe do 3.8 no ROADMAP.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::{Game, NewsItem};
use crate::person::ClubRef;
use crate::{Result, World};

/// Confiança ao chegar ao clube e os limites dos avisos (0–100; a 0 é despedido).
const START_CONFIDENCE: i32 = 60;
const WARNING_AT: i32 = 35;
const ULTIMATUM_AT: i32 = 15;
/// O orçamento de transferências: metade de (saldo + esta parte das receitas de uma época);
/// as vendas devolvem esta parte do que renderam.
const BUDGET_REVENUE: f64 = 0.2;
const SALES_BACK: f64 = 0.5;
/// Teto da folha salarial (sobre as receitas de uma época) e salário máximo de um jogador
/// (sobre o maior salário do plantel ou sobre a folha permitida).
const WAGE_CAP: f64 = 0.9;
const MAX_WAGE_OVER_TOP: f64 = 1.25;
const MAX_WAGE_SHARE: f64 = 0.1;
/// Saldo negativo (sobre as receitas de uma época) a partir do qual a direção proíbe contratações.
const NO_SIGNINGS_DEBT: f64 = 0.1;
/// Vender um dos jogadores que a direção considera valiosos: confiança e adeptos que se perdem.
const IDOL_CONFIDENCE: i32 = 5;
const IDOL_FANS: i32 = 8;

/// A tela "Confiança da Direcção": os parágrafos do CM.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct BoardConfidence {
    pub club: ClubRef,
    pub paragraphs: Vec<String>,
    /// Satisfação dos adeptos (0–100).
    pub fans: i32,
}

/// A direção do clube do treinador (Finanças & Info).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct BoardView {
    /// "lutar pelo título", "evitar a despromoção"...
    pub goal: String,
    /// Posição que a direção espera na liga.
    pub target: i32,
    pub confidence: i32,
    pub confidence_label: String,
    #[ts(type = "number")]
    pub transfer_budget: i64,
    /// Folha salarial semanal permitida, a atual e o salário máximo de um jogador.
    #[ts(type = "number")]
    pub wage_budget: i64,
    #[ts(type = "number")]
    pub wage_bill: i64,
    #[ts(type = "number")]
    pub max_wage: i64,
    pub sacked: bool,
}

/// O que a direção espera de um clube na divisão dele, pela reputação: a posição alvo e o
/// texto (o mesmo da mensagem de boas-vindas).
pub(super) fn expectation(world: &World, club: i32) -> Result<(i32, &'static str, Option<String>)> {
    let (rank, total, division): (i32, i32, Option<String>) = world.conn.query_row(
        "SELECT (SELECT COUNT(*) FROM clubs o WHERE o.division_id = c.division_id AND o.reputation > c.reputation) + 1,
                (SELECT COUNT(*) FROM clubs o WHERE o.division_id = c.division_id),
                d.name
         FROM clubs c LEFT JOIN competitions d ON d.id = c.division_id WHERE c.id = ?",
        [club],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(if total < 4 {
        (total.max(1), "fazer uma boa época", division)
    } else if rank <= 3 {
        (3, "lutar pelo título", division)
    } else if rank <= total / 2 {
        (total / 2, "terminar na primeira metade da tabela", division)
    } else if rank <= total - 4 {
        (total - 5, "garantir uma posição tranquila a meio da tabela", division)
    } else {
        (total - 4, "evitar a despromoção", division)
    })
}

fn confidence_label(c: i32) -> &'static str {
    match c {
        80.. => "Excelente",
        60..=79 => "Boa",
        40..=59 => "Razoável",
        20..=39 => "Má",
        _ => "Péssima",
    }
}

impl Game {
    /// Cria a direção do clube do treinador (jogo novo, ou save de antes do passo 4).
    pub(super) fn board_setup(&mut self) -> Result<()> {
        let (target, goal, _) = expectation(&self.world, self.club_id)?;
        self.world.conn.execute(
            "INSERT OR REPLACE INTO board (id, goal, target, confidence, transfer_budget, wage_budget, max_wage, warning, sacked)
             VALUES (1, ?, ?, ?, 0, 0, 0, 0, 0)",
            params![goal, target, START_CONFIDENCE],
        )?;
        self.refresh_budgets()
    }

    /// Orçamentos de uma janela, pela situação financeira: metade de (saldo + 20% das receitas
    /// de uma época), nada com dívidas grandes; a folha até 90% das receitas (nunca abaixo da
    /// atual) e um salário máximo por jogador.
    pub(super) fn refresh_budgets(&mut self) -> Result<()> {
        let base = self.club_base(self.club_id)?;
        let (cash, bill, top): (i64, i64, i64) = self.world.conn.query_row(
            "SELECT COALESCE((SELECT cash FROM clubs WHERE id = ?1), 0),
                    COALESCE(SUM(wage), 0), COALESCE(MAX(wage), 0)
             FROM people WHERE club_id = ?1 AND is_player = 1",
            [self.club_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let transfer = if (cash as f64) < -NO_SIGNINGS_DEBT * base {
            0
        } else {
            crate::finance::round_money(((cash as f64 + BUDGET_REVENUE * base) * 0.5).max(0.0))
        };
        let wage_budget = ((base * WAGE_CAP / 52.0) as i64).max(bill * 102 / 100);
        let max_wage = ((top as f64 * MAX_WAGE_OVER_TOP).max(wage_budget as f64 * MAX_WAGE_SHARE)) as i64;
        let round = |v: i64| if v < 10_000 { v / 100 * 100 } else { v / 1000 * 1000 };
        self.world.conn.execute(
            "UPDATE board SET transfer_budget = ?, wage_budget = ?, max_wage = ? WHERE id = 1",
            params![transfer, round(wage_budget), round(max_wage)],
        )?;
        Ok(())
    }

    pub fn board_view(&self) -> Result<BoardView> {
        let (goal, target, confidence, transfer_budget, wage_budget, max_wage, sacked): (String, i32, i32, i64, i64, i64, bool) =
            self.world.conn.query_row(
                "SELECT goal, target, confidence, transfer_budget, wage_budget, max_wage, sacked FROM board WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get::<_, i32>(6)? != 0)),
            )?;
        let wage_bill: i64 = self.world.conn.query_row(
            "SELECT COALESCE(SUM(wage), 0) FROM people WHERE club_id = ? AND is_player = 1",
            [self.club_id],
            |r| r.get(0),
        )?;
        Ok(BoardView {
            goal,
            target,
            confidence,
            confidence_label: confidence_label(confidence).into(),
            transfer_budget,
            wage_budget,
            wage_bill,
            max_wage,
            sacked,
        })
    }

    /// "Confiança da Direcção": o desempenho (pela confiança), as finanças (saldo contra as
    /// receitas), os adeptos e os jogadores que a direção considera valiosos.
    pub fn board_confidence(&self) -> Result<BoardConfidence> {
        let (confidence, fans): (i32, i32) =
            self.world.conn.query_row("SELECT confidence, fans FROM board WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let club = self.club(self.club_id);
        let mood = match confidence {
            85.. => "está deliciada com o seu desempenho como treinador",
            70..=84 => "está muito satisfeita com o seu desempenho como treinador",
            55..=69 => "está satisfeita com o seu desempenho como treinador",
            40..=54 => "considera aceitável o seu desempenho como treinador",
            25..=39 => "está preocupada com o seu desempenho como treinador",
            _ => "está muito descontente com o seu desempenho como treinador",
        };
        let base = self.club_base(self.club_id)?.max(1.0);
        let cash: i64 = self.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [self.club_id], |r| r.get(0))?;
        let money = match cash as f64 / base {
            r if r >= 0.5 => "extremamente segura",
            r if r >= 0.15 => "segura",
            r if r >= 0.0 => "estável",
            r if r >= -NO_SIGNINGS_DEBT => "apertada",
            _ => "preocupante",
        };
        let supporters = match fans {
            75.. => "Estamos satisfeitos por constatar que você conseguiu construir uma relação positiva com os adeptos.",
            50..=74 => "Os adeptos estão contentes com o rumo do clube.",
            30..=49 => "Os adeptos começam a ficar impacientes.",
            _ => "Os adeptos estão descontentes e a pressão sobre o clube é grande.",
        };
        let mut paragraphs =
            vec![format!("A direcção {} {mood}.", club.name), format!("A situação financeira do clube é {money}."), supporters.to_string()];
        let idols: Vec<String> = self.valuable_players()?.iter().map(|&id| self.person_name(id)).collect::<Result<_>>()?;
        match idols.as_slice() {
            [] => {}
            [one] => paragraphs.push(format!("A direcção acredita que {one} é um elemento valioso do plantel.")),
            [a, b, ..] => paragraphs.push(format!("A direcção acredita que {a} e {b} são elementos valiosos do plantel.")),
        }
        Ok(BoardConfidence { club, paragraphs, fans })
    }

    /// Os dois jogadores que a direção considera mais valiosos: os de maior valor no plantel.
    pub(super) fn valuable_players(&self) -> Result<Vec<i32>> {
        let ids: Vec<i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id FROM people p WHERE p.club_id = ? AND p.is_player = 1
                 AND p.id NOT IN (SELECT person_id FROM loans WHERE parent_id <> ?) ORDER BY p.id",
            )?
            .query_map([self.club_id, self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut valued: Vec<(i64, i32)> = Vec::with_capacity(ids.len());
        for id in ids {
            valued.push((self.world.market_value_of(id, &self.date)?.unwrap_or(0), id));
        }
        valued.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        Ok(valued.into_iter().take(2).map(|v| v.1).collect())
    }

    /// Antes de vender um jogador: se for dos valiosos, a direção e os adeptos não gostam.
    /// Devolve a frase para a mensagem da venda.
    pub(super) fn idol_sold(&mut self, person: i32) -> Result<Option<String>> {
        if !self.valuable_players()?.contains(&person) {
            return Ok(None);
        }
        self.world.conn.execute(
            "UPDATE board SET confidence = MAX(1, confidence - ?), fans = MAX(0, fans - ?) WHERE id = 1",
            params![IDOL_CONFIDENCE, IDOL_FANS],
        )?;
        Ok(Some("A direcção e os adeptos não gostaram de ver sair um dos jogadores mais valiosos do plantel.".into()))
    }

    /// Satisfação dos adeptos, de 0 a 100.
    fn change_fans(&self, delta: i32) -> Result<()> {
        self.world.conn.execute("UPDATE board SET fans = MIN(100, MAX(0, fans + ?)) WHERE id = 1", [delta])?;
        Ok(())
    }

    pub fn sacked(&self) -> bool {
        self.world
            .conn
            .query_row("SELECT sacked FROM board WHERE id = 1", [], |r| r.get::<_, i32>(0))
            .optional()
            .ok()
            .flatten()
            .is_some_and(|s| s != 0)
    }

    /// O que sobra do orçamento de transferências.
    pub(super) fn transfer_budget(&self) -> Result<i64> {
        Ok(self.world.conn.query_row("SELECT transfer_budget FROM board WHERE id = 1", [], |r| r.get(0))?)
    }

    /// Gastar (positivo) ou devolver (negativo) orçamento de transferências.
    pub(super) fn spend_budget(&self, amount: i64) -> Result<()> {
        self.world.conn.execute("UPDATE board SET transfer_budget = MAX(0, transfer_budget - ?) WHERE id = 1", [amount])?;
        Ok(())
    }

    /// Uma venda do treinador devolve metade ao orçamento.
    pub(super) fn sale_to_budget(&self, fee: i64) -> Result<()> {
        self.spend_budget(-((fee as f64 * SALES_BACK) as i64))
    }

    /// Mexe na confiança, com os avisos e o despedimento.
    pub(super) fn change_confidence(&mut self, delta: i32, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if delta == 0 || self.sacked() {
            return Ok(());
        }
        // Com "mais tempo" dado pela direção, as quedas contam metade
        let grace: Option<String> = self.world.conn.query_row("SELECT grace_until FROM board WHERE id = 1", [], |r| r.get(0))?;
        let delta = if delta < 0 && grace.is_some_and(|g| g.as_str() > self.date.as_str()) { (delta / 2).min(-1) } else { delta };
        let (old, warning): (i32, i32) =
            self.world.conn.query_row("SELECT confidence, warning FROM board WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let new = (old + delta).clamp(0, 100);
        let mut warning = warning;
        let club = self.club(self.club_id).name;
        if new == 0 {
            self.world.conn.execute("UPDATE board SET confidence = 0, sacked = 1 WHERE id = 1", [])?;
            let body = format!(
                "A Direcção do {club} perdeu a confiança no seu trabalho e decidiu dispensá-lo do cargo de treinador, com efeitos imediatos.\n\nO jogo acabou."
            );
            fresh.push(self.news_item(*next_id, "message", "Despedido".into(), body));
            *next_id += 1;
            self.unsaved = true;
            return Ok(());
        }
        if new < ULTIMATUM_AT && warning < 2 {
            warning = 2;
            let body = format!(
                "A Direcção do {club} está muito descontente com os resultados. Se a situação não mudar depressa, será dispensado."
            );
            fresh.push(self.news_item(*next_id, "message", "Ultimato da Direcção".into(), body));
            *next_id += 1;
        } else if new < WARNING_AT && warning < 1 {
            warning = 1;
            let body = format!("A Direcção do {club} está preocupada com o rumo da equipa e espera melhorias nas próximas semanas.");
            fresh.push(self.news_item(*next_id, "message", "A Direcção está preocupada".into(), body));
            *next_id += 1;
        } else if new >= WARNING_AT + 10 {
            warning = 0;
        }
        self.world.conn.execute("UPDATE board SET confidence = ?, warning = ? WHERE id = 1", params![new, warning])?;
        self.unsaved = true;
        Ok(())
    }

    /// Um jogo do treinador: a vitória sobe 1, a derrota desce 2.
    pub(super) fn board_result(&mut self, goals_for: i32, goals_against: i32, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let delta: i32 = match goals_for.cmp(&goals_against) {
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Less => -2,
        };
        // Os adeptos: uma goleada conta a dobrar
        let fans = if (goals_for - goals_against).abs() >= 3 { 2 * delta.signum() } else { delta.signum() };
        self.change_fans(fans)?;
        self.change_confidence(delta, fresh, next_id)
    }

    /// O dia da direção: orçamentos ao abrir cada janela; no dia 1 de cada mês, a posição na
    /// liga contra a esperada (−6 a +4) e as finanças (saldo negativo −2, dívida grande −4).
    pub(super) fn board_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if self.sacked() {
            return Ok(());
        }
        if matches!(&self.date[5..], "01-01" | "07-01") {
            self.refresh_budgets()?;
            let b = self.board_view()?;
            let body = format!(
                "A Direcção autoriza transferências até {} nesta janela e uma folha salarial até {} por semana (salário máximo de {} por jogador).",
                super::market::money_text(b.transfer_budget),
                super::market::money_text(b.wage_budget),
                super::market::money_text(b.max_wage)
            );
            fresh.push(self.news_item(*next_id, "message", "Orçamentos da Direcção".into(), body));
            *next_id += 1;
        }
        if !self.date.ends_with("-01") {
            return Ok(());
        }
        let mut delta = 0;
        let division: Option<i32> =
            self.world.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [self.club_id], |r| r.get(0))?;
        if let Some(div) = division {
            let table = self.table(div);
            if let Some(row) = table.iter().find(|r| r.club.id == self.club_id).filter(|r| r.played >= 4) {
                let target: i32 = self.world.conn.query_row("SELECT target FROM board WHERE id = 1", [], |r| r.get(0))?;
                delta += (target - row.position).clamp(-6, 4);
                // Os adeptos querem ver a equipa onde a direção a espera (ou acima)
                self.change_fans((target - row.position).clamp(-4, 4))?;
            }
        }
        let base = self.club_base(self.club_id)?;
        let cash: i64 = self.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [self.club_id], |r| r.get(0))?;
        if (cash as f64) < -NO_SIGNINGS_DEBT * base {
            delta -= 4;
            self.world.conn.execute("UPDATE board SET transfer_budget = 0 WHERE id = 1", [])?;
        } else if cash < 0 {
            delta -= 2;
        }
        self.change_confidence(delta, fresh, next_id)
    }

    /// Balanço no fim da época da liga do treinador: subir +20, cumprir o objetivo +12 (o
    /// título +20), falhar por mais de 3 lugares −15, descer −35. Depois, a expectativa nova.
    pub(super) fn board_season_end(
        &mut self,
        position: i32,
        promoted: bool,
        relegated: bool,
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
    ) -> Result<()> {
        if self.sacked() {
            return Ok(());
        }
        let (target, goal): (i32, String) =
            self.world.conn.query_row("SELECT target, goal FROM board WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let (delta, verdict) = if relegated {
            (-35, "A descida de divisão é inaceitável para a Direcção.")
        } else if promoted {
            (20, "A subida de divisão deixou a Direcção muito satisfeita.")
        } else if position == 1 {
            (20, "O título superou todas as expectativas da Direcção.")
        } else if position <= target {
            (12, "A Direcção está satisfeita: o objetivo da época foi cumprido.")
        } else if position > target + 3 {
            (-15, "A Direcção está desiludida: a equipa ficou longe do objetivo.")
        } else {
            (0, "A época ficou perto do que a Direcção esperava.")
        };
        let body = format!("A Direcção esperava {goal} e a equipa terminou em {position}º lugar.\n\n{verdict}");
        fresh.push(self.news_item(*next_id, "message", "Balanço da época".into(), body));
        *next_id += 1;
        self.change_confidence(delta, fresh, next_id)
    }

    /// Época nova na liga do treinador: a expectativa pela divisão onde está agora.
    pub(super) fn board_new_season(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if self.sacked() {
            return Ok(());
        }
        let (target, goal, division) = expectation(&self.world, self.club_id)?;
        self.world.conn.execute("UPDATE board SET goal = ?, target = ? WHERE id = 1", params![goal, target])?;
        let league = division.map(|d| format!(" no {d}")).unwrap_or_default();
        let body = format!("Para a nova época, a Direcção espera que a equipa consiga {goal}{league}.");
        fresh.push(self.news_item(*next_id, "message", "Objectivos da época".into(), body));
        *next_id += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_warnings_and_the_sack() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-direcao-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Direção".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
        };
        let mut g = Game::create(&path, &dir, &new, 5).unwrap();
        let b = g.board_view().unwrap();
        assert_eq!(b.confidence, START_CONFIDENCE);
        assert!(b.transfer_budget > 0 && b.wage_budget >= b.wage_bill && b.max_wage > 0, "{b:?}");
        assert!(g.news().iter().any(|n| n.body.contains(&b.goal)), "a expectativa é a das boas-vindas");

        // As vendas devolvem metade; os gastos descontam
        g.spend_budget(b.transfer_budget / 2).unwrap();
        g.sale_to_budget(1_000_000).unwrap();
        assert_eq!(g.transfer_budget().unwrap(), b.transfer_budget - b.transfer_budget / 2 + 500_000);

        // Confiança da Direcção: os quatro parágrafos; vender um dos valiosos custa confiança e adeptos
        let view = g.board_confidence().unwrap();
        assert_eq!(view.paragraphs.len(), 4, "{:?}", view.paragraphs);
        assert!(view.paragraphs[0].contains(&view.club.name) && view.paragraphs[3].contains("valiosos"));
        let idol = g.valuable_players().unwrap()[0];
        let before = g.board_view().unwrap().confidence;
        assert!(g.idol_sold(idol).unwrap().is_some());
        assert_eq!(g.board_view().unwrap().confidence, before - IDOL_CONFIDENCE);
        assert_eq!(g.board_confidence().unwrap().fans, 60 - IDOL_FANS);
        let modest: i32 = g
            .world
            .conn
            .query_row("SELECT id FROM people WHERE club_id = ? AND is_player = 1 ORDER BY COALESCE(wage, 0) LIMIT 1", [g.club_id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(g.idol_sold(modest).unwrap().is_none());

        // Avisos, ultimato e despedimento
        let (mut fresh, mut next_id) = (Vec::new(), 1000);
        g.change_confidence(-30, &mut fresh, &mut next_id).unwrap();
        assert!(fresh.iter().any(|n| n.title == "A Direcção está preocupada"));
        g.change_confidence(-20, &mut fresh, &mut next_id).unwrap();
        assert!(fresh.iter().any(|n| n.title == "Ultimato da Direcção"));
        assert!(!g.sacked());
        g.change_confidence(-100, &mut fresh, &mut next_id).unwrap();
        assert!(g.sacked() && g.info().unwrap().sacked);
        assert!(fresh.iter().any(|n| n.title == "Despedido"));
        assert!(g.advance_day().is_err(), "o jogo acabou");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
