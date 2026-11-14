//! Direção (marco 3.8, passo 4): a expectativa da época, a confiança no treinador (avisos e
//! despedimento) e os orçamentos de transferências e salarial do clube do treinador. As
//! dívidas dos clubes da IA (vendas forçadas e falência) ficam no mercado e no fim de época.
//! Regras e números em docs/roadmap/3.08-mercado.md.

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
/// Os Fundos Transferências seguem o saldo, como no CM: ao chegar, o saldo arredondado para
/// baixo (`budget_reserve` guarda a diferença), e depois o saldo menos essa reserva, euro a euro,
/// revisto no dia 1 de cada mês (no Bayern do CM a diferença fica em €27M de junho a junho).
/// As vendas voltam inteiras aos fundos. Do saldo ao chegar, a direção só dá metade do que passa
/// de `CASH_KEPT` das receitas, e adianta `SEASON_FUNDS` das receitas da época (3.22): o Real
/// fica com ~€225M, o United com ~€155M, como as janelas de hoje.
const SALES_BACK: f64 = 1.0;
pub(super) const SEASON_FUNDS: f64 = 0.17;
const CASH_KEPT: f64 = 0.1;
const CASH_GIVEN: f64 = 0.5;
/// O salário máximo de um jogador (sobre o maior salário do plantel ou sobre a folha permitida);
/// a folha pode crescer `WAGE_GROWTH` sobre a de hoje, ou chegar a `WAGE_TARGET` das receitas (um
/// clube que sobe), mas nunca passa de `finance::WAGE_SHARE` (o teto da UEFA); se já passou, fica
/// a de hoje: só se contrata vendendo. O United de 2025 paga 47% das receitas, o Real ~40%.
const WAGE_GROWTH: f64 = 1.1;
const WAGE_TARGET: f64 = 0.5;
const MAX_WAGE_OVER_TOP: f64 = 1.25;
const MAX_WAGE_SHARE: f64 = 0.1;
/// O salário máximo por função (como no CM, a direção limita cada papel), sobre a folha
/// permitida, na ordem de `SQUAD_ROLES`: indispensável (o `max_wage`, que é este ou o maior
/// salário do plantel +25%), importante, rotação, apoio, perspectiva futura, jovem com valor.
const ROLE_WAGE_SHARE: [f64; 6] = [MAX_WAGE_SHARE, 0.07, 0.045, 0.03, 0.025, 0.015];
/// Saldo negativo (sobre as receitas de uma época) a partir do qual a direção proíbe contratações.
const NO_SIGNINGS_DEBT: f64 = 0.1;
/// Equilibrar orçamentos: €1 a mais por semana na folha custa isto nos fundos (um ano de salário).
pub(super) const SHIFT_WEEKS: i64 = 52;
/// Save 68: o que o treinador passou dos fundos para a folha (por semana; negativo ao contrário).
pub(super) const SHIFT_SCHEMA: &str = "ALTER TABLE board ADD COLUMN wage_shift INTEGER NOT NULL DEFAULT 0;";
/// Vender um dos jogadores que a direção considera valiosos: confiança e adeptos que se perdem.
const IDOL_CONFIDENCE: i32 = 5;
const IDOL_FANS: i32 = 8;

/// A tela "Confiança da Direcção": os parágrafos do CM.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct BoardConfidence {
    pub club: ClubRef,
    /// As frases da direção, como no CM, cada uma com o assunto e o nível.
    pub lines: Vec<BoardLine>,
    /// Satisfação dos adeptos (0–100).
    pub fans: i32,
    /// "lutar pelo título" e a divisão, o orçamento de transferências e a folha salarial.
    pub goal: String,
    pub division: Option<String>,
    #[ts(type = "number")]
    pub transfer_budget: i64,
    #[ts(type = "number")]
    pub wage_bill: i64,
    #[ts(type = "number")]
    pub wage_budget: i64,
}

/// Uma frase da Confiança da Direcção.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct BoardLine {
    /// "performance", "finances", "fans" ou "players".
    pub kind: String,
    /// "Desempenho da equipa", "Situação financeira"...
    pub label: String,
    pub text: String,
    /// Nível de 0 a 100 (sem nível nos jogadores).
    pub level: Option<i32>,
    /// Os jogadores valiosos (para as ligações).
    pub players: Vec<(i32, String)>,
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
    /// Equilibrar orçamentos (como no FM): o que passou dos fundos de transferências para a folha
    /// semanal (negativo: da folha para os fundos), e quanto pode ir de um lado para o outro.
    #[ts(type = "number")]
    pub wage_shift: i64,
    #[ts(type = "number")]
    pub shift_min: i64,
    #[ts(type = "number")]
    pub shift_max: i64,
    /// Quanto custa nos fundos €1 a mais por semana na folha (a pré-visualização da barra).
    #[ts(type = "number")]
    pub shift_weeks: i64,
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

/// Fundos arredondados para baixo como no CM: a €1M acima de €10M, a €500K acima de €1M,
/// a €10K abaixo.
pub(super) fn round_funds(v: i64) -> i64 {
    let step = match v {
        10_000_000.. => 1_000_000,
        1_000_000.. => 500_000,
        _ => 10_000,
    };
    if v <= 0 { 0 } else { v / step * step }
}

/// Os fundos ao chegar: o saldo arredondado mais largo (o Viktoria Köln do CM tem ~€723K e
/// €500K de fundos; a diferença fica para sempre).
fn start_funds(cash: i64) -> i64 {
    let step = match cash {
        10_000_000.. => 1_000_000,
        500_000.. => 250_000,
        _ => 10_000,
    };
    cash / step * step
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
        let reserve = self.season_reserve()?;
        self.world.conn.execute(
            "INSERT OR REPLACE INTO board (id, goal, target, confidence, transfer_budget, wage_budget, max_wage, warning, sacked, budget_reserve)
             VALUES (1, ?, ?, ?, 0, 0, 0, 0, 0, ?)",
            params![goal, target, START_CONFIDENCE, reserve],
        )?;
        self.refresh_budgets()
    }

    /// A parte do saldo que a direção guarda numa época: tudo menos metade do que passa de
    /// `CASH_KEPT` das receitas (revista ao chegar e no começo de cada época).
    fn season_reserve(&self) -> Result<i64> {
        let cash: i64 = self.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [self.club_id], |r| r.get(0))?;
        let free = (CASH_GIVEN * (cash as f64 - CASH_KEPT * self.club_base(self.club_id)?)).max(0.0) as i64;
        Ok(if cash > 0 { cash - start_funds(free) } else { cash })
    }

    /// Os fundos de hoje: o saldo menos a reserva, mais a parte das receitas da época que a direção
    /// adianta ([`SEASON_FUNDS`]), arredondado como no CM; nada com dívidas grandes ou proibido
    /// de contratar pelo fair-play.
    fn funds_now(&self) -> Result<i64> {
        let base = self.club_base(self.club_id)?;
        let (cash, reserve): (i64, i64) = self.world.conn.query_row(
            "SELECT COALESCE((SELECT cash FROM clubs WHERE id = ?), 0), budget_reserve FROM board WHERE id = 1",
            [self.club_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if (cash as f64) < -NO_SIGNINGS_DEBT * base || self.fair_play_banned(self.club_id)? {
            return Ok(0);
        }
        let funds = (cash - reserve + (SEASON_FUNDS * base) as i64).min(self.fair_play_spend(self.club_id)? as i64);
        Ok(round_funds((funds - self.wage_shift()? * SHIFT_WEEKS).max(0)))
    }

    /// O que o treinador passou dos fundos para a folha semanal (negativo: ao contrário).
    fn wage_shift(&self) -> Result<i64> {
        Ok(self.world.conn.query_row("SELECT wage_shift FROM board WHERE id = 1", [], |r| r.get(0)).optional()?.unwrap_or(0))
    }

    /// A folha permitida pela direção, sem a troca do treinador.
    fn wage_allowed(&self, bill: i64) -> Result<i64> {
        let base = self.club_base(self.club_id)?;
        let grow = (bill as f64 * WAGE_GROWTH).max(base * WAGE_TARGET / 52.0);
        Ok((grow.min(base * crate::finance::WAGE_SHARE / 52.0) as i64).max(bill))
    }

    fn wage_bill(&self) -> Result<i64> {
        Ok(self.world.conn.query_row(
            "SELECT COALESCE(SUM(wage), 0) FROM people WHERE club_id = ? AND is_player = 1",
            [self.club_id],
            |r| r.get(0),
        )?)
    }

    /// Os limites da troca (por semana): para a folha, até aos fundos de hoje; para os fundos,
    /// só a folga da folha (a folha permitida nunca fica abaixo da de hoje).
    fn shift_limits(&self) -> Result<(i64, i64)> {
        let shift = self.wage_shift()?;
        let bill = self.wage_bill()?;
        let funds = self.transfer_budget()?;
        let max = shift + funds / SHIFT_WEEKS;
        let min = (bill - self.wage_allowed(bill)?).min(shift);
        Ok((min, max))
    }

    /// Equilibrar orçamentos (como no FM): o treinador escolhe quanto da folha semanal vem dos
    /// fundos de transferências (€1 por semana custa €52 nos fundos), a qualquer momento, sem a
    /// folha permitida ficar abaixo da de hoje nem os fundos abaixo de zero. Dura até à nova época.
    pub fn set_wage_shift(&mut self, shift: i64) -> Result<BoardView> {
        if self.sacked() {
            return Err(crate::Error::Invalid("Sem clube, não há orçamentos.".into()));
        }
        let (min, max) = self.shift_limits()?;
        let shift = shift.clamp(min, max);
        let spent = (shift - self.wage_shift()?) * SHIFT_WEEKS;
        self.world.conn.execute("UPDATE board SET wage_shift = ? WHERE id = 1", [shift])?;
        // Os fundos mudam já (o que se gastou este mês continua gasto)
        self.spend_budget(spent)?;
        self.refresh_wage_budget()?;
        self.unsaved = true;
        self.board_view()
    }

    /// A folha permitida e o salário máximo, com a troca do treinador.
    fn refresh_wage_budget(&mut self) -> Result<()> {
        let bill = self.wage_bill()?;
        let top: i64 = self.world.conn.query_row(
            "SELECT COALESCE(MAX(wage), 0) FROM people WHERE club_id = ? AND is_player = 1",
            [self.club_id],
            |r| r.get(0),
        )?;
        let round = |v: i64| if v < 10_000 { v / 100 * 100 } else { v / 1000 * 1000 };
        // Arredondada para baixo, mas nunca abaixo da folha de hoje
        let wage_budget = round(self.wage_allowed(bill)? + self.wage_shift()?).max(bill);
        let max_wage = ((top as f64 * MAX_WAGE_OVER_TOP).max(wage_budget as f64 * MAX_WAGE_SHARE)) as i64;
        self.world.conn.execute("UPDATE board SET wage_budget = ?, max_wage = ? WHERE id = 1", params![wage_budget, round(max_wage)])?;
        Ok(())
    }

    /// O salário semanal máximo que a direção autoriza numa função (`role`, de `SQUAD_ROLES`):
    /// o do indispensável é o `max_wage`; os outros, uma parte da folha permitida, nunca acima dele.
    pub(super) fn role_max_wage(&self, role: usize) -> Result<i64> {
        let b = self.board_view()?;
        if role == 0 {
            return Ok(b.max_wage);
        }
        let share = ROLE_WAGE_SHARE.get(role).copied().unwrap_or(MAX_WAGE_SHARE);
        let v = (b.wage_budget as f64 * share) as i64;
        let v = if v < 10_000 { v / 100 * 100 } else { v / 1000 * 1000 };
        Ok(v.min(b.max_wage))
    }

    /// Dia 1 de cada mês: os fundos voltam a ser o saldo menos a reserva.
    pub(super) fn sync_funds(&mut self) -> Result<()> {
        let funds = self.funds_now()?;
        self.world.conn.execute("UPDATE board SET transfer_budget = ? WHERE id = 1", [funds])?;
        Ok(())
    }

    /// Mais fundos dados pela direção (reunião): entram já e ficam (a reserva desce).
    pub(super) fn add_funds(&self, amount: i64) -> Result<()> {
        self.spend_budget(-amount)?;
        self.world.conn.execute("UPDATE board SET budget_reserve = budget_reserve - ? WHERE id = 1", [amount])?;
        Ok(())
    }

    /// Orçamentos de uma janela, pela situação financeira: os fundos, a folha permitida e um
    /// salário máximo por jogador.
    pub(super) fn refresh_budgets(&mut self) -> Result<()> {
        let transfer = self.funds_now()?;
        self.world.conn.execute("UPDATE board SET transfer_budget = ? WHERE id = 1", [transfer])?;
        self.refresh_wage_budget()
    }

    pub fn board_view(&self) -> Result<BoardView> {
        let (goal, target, confidence, transfer_budget, wage_budget, max_wage, sacked): (String, i32, i32, i64, i64, i64, bool) =
            self.world.conn.query_row(
                "SELECT goal, target, confidence, transfer_budget, wage_budget, max_wage, sacked FROM board WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get::<_, i32>(6)? != 0)),
            )?;
        let wage_bill = self.wage_bill()?;
        let (shift_min, shift_max) = if sacked { (0, 0) } else { self.shift_limits()? };
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
            wage_shift: self.wage_shift()?,
            shift_min,
            shift_max,
            shift_weeks: SHIFT_WEEKS,
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
        let line = |kind: &str, label: &str, text: String, level: Option<i32>| BoardLine {
            kind: kind.into(),
            label: label.into(),
            text,
            level,
            players: Vec::new(),
        };
        // As finanças em 0–100: metade com saldo zero, cheio com meia época de receitas em caixa
        let money_level = (50.0 + 100.0 * cash as f64 / base).clamp(0.0, 100.0) as i32;
        let mut lines = vec![
            line("performance", "Desempenho da equipa", format!("A direcção {} {mood}.", club.name), Some(confidence)),
            line("finances", "Situação financeira", format!("A situação financeira do clube é {money}."), Some(money_level)),
            line("fans", "Relação com os adeptos", supporters.to_string(), Some(fans)),
        ];
        let idols: Vec<(i32, String)> =
            self.valuable_players()?.iter().map(|&id| Ok((id, self.person_name(id)?))).collect::<Result<_>>()?;
        let text = match idols.as_slice() {
            [] => None,
            [(_, one)] => Some(format!("A direcção acredita que {one} é um elemento valioso do plantel.")),
            [(_, a), (_, b), ..] => Some(format!("A direcção acredita que {a} e {b} são elementos valiosos do plantel.")),
        };
        if let Some(text) = text {
            lines.push(BoardLine { players: idols, ..line("players", "Jogadores-chave", text, None) });
        }
        let (_, goal, division) = expectation(&self.world, self.club_id)?;
        let (transfer_budget, wage_budget): (i64, i64) =
            self.world.conn.query_row("SELECT transfer_budget, wage_budget FROM board WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let wage_bill: i64 = self.world.conn.query_row(
            "SELECT COALESCE(SUM(wage), 0) FROM people WHERE club_id = ? AND is_player = 1",
            [self.club_id],
            |r| r.get(0),
        )?;
        Ok(BoardConfidence { club, lines, fans, goal: goal.into(), division, transfer_budget, wage_bill, wage_budget })
    }

    /// Os dois jogadores que a direção considera mais valiosos: os de maior valor de mercado.
    pub(super) fn valuable_players(&self) -> Result<Vec<i32>> {
        Ok(self.most_valuable(self.club_id)?.into_iter().take(2).collect())
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
    pub(super) fn change_fans(&self, delta: i32) -> Result<()> {
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
        if delta == 0 || self.sacked() || self.prestart {
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
            // Fica sem clube: o lugar vago, a reputação a descer, e procura outro emprego
            self.world.conn.execute("UPDATE people SET club_id = NULL, club_job = NULL WHERE id = ?", [self.manager_id])?;
            self.log_staff_move(self.manager_id, Some(self.club_id), None)?;
            self.world.conn.execute("INSERT OR REPLACE INTO vacancies (club_id, since) VALUES (?, ?)", params![self.club_id, self.date])?;
            self.change_manager_rep(super::jobs::REP_SACKED)?;
            self.career_leave()?;
            let body = format!(
                "A Direcção do {club} perdeu a confiança no seu trabalho e decidiu dispensá-lo do cargo de treinador, com efeitos imediatos.\n\nProcure um novo clube em Informação de Empregos."
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
        self.sync_funds()?;
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
        let (delta, verdict): (i32, &str) = if relegated {
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
        self.change_manager_rep(super::jobs::REP_SEASON * delta.signum())?;
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
        let reserve = self.season_reserve()?;
        self.world.conn.execute(
            "UPDATE board SET goal = ?, target = ?, budget_reserve = ?, wage_shift = 0 WHERE id = 1",
            params![goal, target, reserve],
        )?;
        self.sync_funds()?;
        self.refresh_wage_budget()?;
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
    fn missing_board_is_recreated_on_load() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-sem-direcao-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Sem direção".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 1669,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 29).unwrap();
        // Um save que ficou sem a linha da direção (aconteceu num jogo do Botafogo)
        g.world.conn.execute("DELETE FROM board", []).unwrap();
        g.save().unwrap();
        let file = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&file).unwrap();
        assert_eq!(g.board_view().unwrap().confidence, START_CONFIDENCE);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

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
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 5).unwrap();
        let b = g.board_view().unwrap();
        assert_eq!(b.confidence, START_CONFIDENCE);
        assert!(b.transfer_budget > 0 && b.wage_budget >= b.wage_bill && b.max_wage > 0, "{b:?}");
        // A folha pode crescer ~10% (ou chegar a metade das receitas), sem passar dos 70%
        let base = g.club_base(g.club_id).unwrap();
        assert!(b.wage_budget as f64 >= b.wage_bill as f64 * 1.09, "{b:?}");
        assert!(b.wage_budget as f64 <= (base * crate::finance::WAGE_SHARE / 52.0).max(b.wage_bill as f64), "{b:?}");
        assert!(g.news().iter().any(|n| n.body.contains(&b.goal)), "a expectativa é a das boas-vindas");
        // Equilibrar orçamentos: €1 por semana a mais na folha custa €52 nos fundos; a folha
        // permitida nunca fica abaixo da de hoje
        let shift = (b.shift_max / 2).max(1000);
        let moved = g.set_wage_shift(shift).unwrap();
        assert_eq!(moved.wage_shift, shift);
        assert!((moved.transfer_budget - (b.transfer_budget - shift * SHIFT_WEEKS)).abs() <= 1000, "{moved:?}");
        assert!(moved.wage_budget >= b.wage_budget + shift - 1000, "{moved:?}");
        let back = g.set_wage_shift(i64::MIN / 2).unwrap();
        assert!(back.wage_budget >= back.wage_bill && back.wage_shift == back.shift_min, "{back:?}");
        assert!(back.shift_min == 0 || back.transfer_budget > b.transfer_budget, "{back:?} {b:?}");
        g.set_wage_shift(0).unwrap();

        // Os fundos são metade do saldo acima de 10% das receitas mais 17% delas; as vendas voltam
        // inteiras e os gastos descontam
        let fin = g.club_finances(g.club_id).unwrap();
        let free = (CASH_GIVEN * (fin.cash as f64 - CASH_KEPT * base)).max(0.0);
        let expected = (free + SEASON_FUNDS * base) as i64;
        assert!(b.transfer_budget <= expected && b.transfer_budget >= expected - 2_000_000, "{} {expected}", b.transfer_budget);
        g.spend_budget(b.transfer_budget / 2).unwrap();
        g.sale_to_budget(1_000_000).unwrap();
        assert_eq!(g.transfer_budget().unwrap(), b.transfer_budget - b.transfer_budget / 2 + 1_000_000);

        // Confiança da Direcção: os quatro parágrafos; vender um dos valiosos custa confiança e adeptos
        let view = g.board_confidence().unwrap();
        assert_eq!(view.lines.len(), 4, "{:?}", view.lines);
        assert!(view.lines[0].text.contains(&view.club.name) && view.lines[3].text.contains("valiosos"));
        assert_eq!(view.lines[3].players.len(), 2);
        assert!(view.lines[..3].iter().all(|l| l.level.is_some_and(|v| (0..=100).contains(&v))));
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
        assert!(g.advance_day().is_ok(), "o jogo continua sem clube");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
