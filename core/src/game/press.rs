//! Conferências de imprensa (marco 3.12, passo 4), como no FM: antes dos jogos importantes
//! (clássicos, eliminatórias, luta pelo título ou contra a descida) e depois de cada jogo do
//! treinador. Três perguntas tiradas da situação (o adversário, a forma, um jogador em
//! destaque ou em baixo, um rumor de transferência, a pressão da direção, o resultado, a
//! arbitragem) e respostas com tom; cada resposta mexe na moral, na confiança da direção e
//! nos adeptos. Mandar o adjunto não mexe em nada, mas faltar a três conferências antes dos
//! jogos seguidas irrita a direção. As perguntas ficam gravadas (`press`) ao criar a conferência.

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Game, NewsItem};
use crate::date;
use crate::{Error, Result};

/// Dias para responder a uma conferência depois do jogo.
const POST_DAYS: i32 = 2;
/// Conferências antes dos jogos faltadas seguidas até a direção se queixar.
const SKIPS_TO_COMPLAIN: usize = 3;
/// Quanto rende a mais o adversário que o treinador provocou antes do jogo.
pub(super) const PROVOKED_BOOST: f64 = 0.04;
/// Lugares do topo e do fundo da tabela que tornam um jogo importante.
const TOP: i32 = 4;
const BOTTOM: i32 = 4;

/// Uma resposta e o que ela faz.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(super) struct Answer {
    pub(super) label: String,
    /// Moral de todo o plantel.
    #[serde(default)]
    pub(super) squad: i32,
    /// Elogio (`true`) ou crítica ao jogador da pergunta.
    #[serde(default)]
    pub(super) praise: Option<bool>,
    /// Moral só do jogador da pergunta.
    #[serde(default)]
    pub(super) player: i32,
    #[serde(default)]
    pub(super) board: i32,
    #[serde(default)]
    pub(super) fans: i32,
    /// Provoca o adversário: ele vem mais motivado para o jogo.
    #[serde(default)]
    pub(super) provoke: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Question {
    pub(super) text: String,
    #[serde(default)]
    pub(super) person: Option<i32>,
    pub(super) answers: Vec<Answer>,
}

/// Uma conferência para a tela: as perguntas e as respostas possíveis.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PressView {
    pub id: i32,
    pub title: String,
    pub intro: String,
    pub questions: Vec<PressQuestion>,
    /// "pending", "done", "skipped".
    pub status: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PressQuestion {
    pub text: String,
    pub answers: Vec<String>,
}

pub(super) fn answer(label: &str) -> Answer {
    Answer { label: label.into(), ..Default::default() }
}

impl Game {
    /// No avanço do dia: as conferências por responder que passaram do prazo (o adjunto foi
    /// no lugar do treinador) e a conferência antes do jogo de amanhã, se for importante.
    pub(super) fn press_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let expired: Vec<(i32, String)> = self
            .world
            .conn
            .prepare_cached("SELECT id, kind FROM press WHERE status = 'pending' AND deadline < ? ORDER BY id")?
            .query_map([&self.date], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, kind) in expired {
            self.world.conn.execute("UPDATE press SET status = 'missed' WHERE id = ?", [id])?;
            if kind == "pre" && self.missed_in_a_row()? % SKIPS_TO_COMPLAIN == 0 {
                self.change_confidence(-1, fresh, next_id)?;
                let n = self.news_item(
                    *next_id,
                    "press",
                    "Diretoria critica ausências".into(),
                    "A Diretoria não gostou de ver o treinador faltando às entrevistas coletivas antes dos jogos.".into(),
                );
                fresh.push(n);
                *next_id += 1;
            }
        }
        let tomorrow = date::add_days(&self.date, 1).unwrap_or_default();
        let Some(m) = self
            .matches
            .iter()
            .find(|m| m.date == tomorrow && m.result.is_none() && (m.home_id == self.club_id || m.away_id == self.club_id))
            .cloned()
        else {
            return Ok(());
        };
        let exists: bool =
            self.world.conn.query_row("SELECT COUNT(*) FROM press WHERE fixture = ? AND kind = 'pre'", [m.id], |r| r.get::<_, i64>(0))? > 0;
        if exists || m.competition_id == super::friendlies::CLUB_FRIENDLY || !self.important(&m)? {
            return Ok(());
        }
        let opp = if m.home_id == self.club_id { m.away_id } else { m.home_id };
        let mut rng = self.press_rng(m.id, 1);
        let mut questions = vec![self.opponent_question(opp, &mut rng)?];
        // Os temas do momento (o clássico, a disputa do título, as críticas...) e, para completar,
        // o treinador do outro lado, a forma e a terceira de sempre
        questions.extend(self.topical_questions(&m, &mut rng)?.into_iter().take(2));
        if questions.len() < 3 {
            questions.extend(self.rival_coach_question(opp, m.id)?);
        }
        if questions.len() < 3 {
            questions.extend(self.form_question()?);
        }
        if questions.len() < 3 {
            questions.extend(self.third_question()?);
        }
        questions.truncate(3);
        self.open_press("pre", m.id, &tomorrow, &questions, fresh, next_id, &self.club(opp).short_name)
    }

    /// Depois do jogo do treinador: a conferência sobre o resultado.
    /// `ratings`: as notas dos jogadores do treinador no jogo.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn post_match_press(
        &mut self,
        fixture: i32,
        opp: i32,
        (ours, theirs): (i32, i32),
        sent_off: bool,
        ratings: &[(i32, f64)],
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
    ) -> Result<()> {
        let opp_name = self.club(opp).short_name;
        let score = format!("{ours}-{theirs}");
        let mut rng = self.press_rng(fixture, 2);
        let say =
            |rng: &mut _, options: &[&str]| super::press_topics::pick(rng, options).replace("{score}", &score).replace("{opp}", &opp_name);
        let (text, answers) = if ours > theirs {
            (
                say(
                    &mut rng,
                    &[
                        "Vitória por {score} sobre o {opp}. Satisfeito com a exibição?",
                        "{score} sobre o {opp}. Foi a melhor exibição do time na temporada?",
                        "O time venceu o {opp} por {score}. O que mais te agradou?",
                    ],
                ),
                vec![
                    Answer { squad: 1, fans: 1, ..answer("Merecemos ganhar, fomos melhores do começo ao fim.") },
                    answer("Foi só um jogo. Ainda não ganhamos nada."),
                    Answer { fans: 2, board: -1, ..answer("Hoje calamos a boca de muita gente.") },
                    answer("Prefiro não falar do jogo."),
                ],
            )
        } else if ours < theirs {
            (
                say(
                    &mut rng,
                    &[
                        "Derrota por {score} para o {opp}. O que deu errado?",
                        "O {opp} ganhou por {score}. Onde o jogo foi perdido?",
                        "Uma derrota dura, {score} para o {opp}. Os torcedores merecem uma explicação?",
                    ],
                ),
                vec![
                    Answer { squad: 1, board: -1, ..answer("A responsabilidade é minha. Os jogadores deram tudo.") },
                    Answer { squad: -2, board: 1, ..answer("Alguns jogadores não renderam o que eu cobro.") },
                    Answer { fans: 1, board: -1, ..answer("O árbitro decidiu o jogo.") },
                    answer("Vamos analisar com calma."),
                ],
            )
        } else {
            (
                say(
                    &mut rng,
                    &["Empate em {score} com o {opp}. Um ponto ganho ou dois perdidos?", "{score} com o {opp}. O empate foi justo?"],
                ),
                vec![
                    Answer { squad: 1, ..answer("Um bom ponto. O time mostrou caráter.") },
                    Answer { squad: -1, board: 1, ..answer("Dois pontos perdidos. A gente tinha que ganhar.") },
                    answer("Não tem muito o que falar do jogo."),
                ],
            )
        };
        let mut questions = vec![Question { text, person: None, answers }];
        // O melhor em campo, senão o pior
        let best = ratings.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1));
        let worst = ratings.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1));
        let focus = best.filter(|b| b.1 >= 7.0).or(worst);
        if let Some((p, r)) = focus
            && let Some(q) = self.player_question(p, r)?
        {
            questions.push(q);
        }
        // Um tema do momento, se houver (as continentais e o clássico já passaram)
        if !sent_off
            && let Some(m) = self.matches.iter().find(|m| m.id == fixture).cloned()
            && let Some(q) = self.topical_questions(&m, &mut rng)?.into_iter().next()
        {
            questions.push(q);
        }
        if sent_off {
            questions.push(Question {
                text: "Um jogador foi expulso. A arbitragem foi justa?".into(),
                person: None,
                answers: vec![
                    answer("Não comento as decisões do árbitro."),
                    Answer { fans: 1, board: -1, ..answer("Fomos claramente prejudicados.") },
                    Answer { squad: -1, ..answer("O jogador tem que controlar os nervos.") },
                ],
            });
        } else {
            questions.extend(self.third_question()?);
        }
        questions.truncate(3);
        let deadline = date::add_days(&self.date, POST_DAYS).unwrap_or_default();
        self.open_press("post", fixture, &deadline, &questions, fresh, next_id, &opp_name)
    }

    /// Grava a conferência e a mensagem com os botões (`news.link` = "press:<id>").
    #[allow(clippy::too_many_arguments)]
    fn open_press(
        &mut self,
        kind: &str,
        fixture: i32,
        deadline: &str,
        questions: &[Question],
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
        opp: &str,
    ) -> Result<()> {
        let json = serde_json::to_string(questions).unwrap_or_default();
        self.world.conn.execute(
            "INSERT INTO press (kind, fixture, date, deadline, questions, status) VALUES (?, ?, ?, ?, ?, 'pending')",
            params![kind, fixture, self.date, deadline, json],
        )?;
        let id = self.world.conn.last_insert_rowid();
        let (title, body) = if kind == "pre" {
            (
                format!("Entrevista coletiva antes do jogo com o {opp}"),
                format!("Os jornalistas esperam você na entrevista coletiva antes do jogo com o {opp}. Você pode ir ou mandar o auxiliar."),
            )
        } else {
            (
                format!("Entrevista coletiva depois do jogo com o {opp}"),
                "Os jornalistas querem ouvir o treinador depois do jogo.".to_string(),
            )
        };
        let mut n = self.news_item(*next_id, "press", title, body);
        n.link = Some(format!("press:{id}"));
        fresh.push(n);
        *next_id += 1;
        self.unsaved = true;
        Ok(())
    }

    /// Jogo importante: clássico (rivais), eliminatória, os dois no topo da tabela ou o
    /// treinador nos lugares do fundo.
    fn important(&self, m: &crate::league::Match) -> Result<bool> {
        let opp = if m.home_id == self.club_id { m.away_id } else { m.home_id };
        let rivals: [Option<i32>; 3] =
            self.world.conn.query_row("SELECT rival1_id, rival2_id, rival3_id FROM clubs WHERE id = ?", [self.club_id], |r| {
                Ok([r.get(0)?, r.get(1)?, r.get(2)?])
            })?;
        if rivals.contains(&Some(opp)) || m.tie.is_some() {
            return Ok(true);
        }
        let table = self.table(m.competition_id);
        let pos = |club: i32| table.iter().find(|r| r.club.id == club).map(|r| r.position);
        let n = table.len() as i32;
        Ok(match (pos(self.club_id), pos(opp)) {
            (Some(a), Some(b)) => (a <= TOP && b <= TOP) || a > n - BOTTOM,
            _ => false,
        })
    }

    /// Conferências antes dos jogos faltadas seguidas (as mais recentes).
    fn missed_in_a_row(&self) -> Result<usize> {
        let statuses: Vec<String> = self
            .world
            .conn
            .prepare_cached("SELECT status FROM press WHERE kind = 'pre' AND status <> 'pending' ORDER BY id DESC")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(statuses.iter().take_while(|s| s.as_str() != "done").count())
    }

    fn opponent_question(&self, opp: i32, rng: &mut impl rand_core::RngCore) -> Result<Question> {
        let name = self.club(opp).short_name;
        Ok(Question {
            text: super::press_topics::pick(
                rng,
                &[
                    "Como você vê o jogo com o {opp}?",
                    "O que você espera do {opp} amanhã?",
                    "O {opp} chega embalado. Como parar esse time?",
                    "Qual ponto forte do {opp} mais te preocupa?",
                ],
            )
            .replace("{opp}", &name),
            person: None,
            answers: vec![
                Answer { squad: 1, fans: 1, ..answer("Vamos ganhar. Somos melhores e vamos provar isso em campo.") },
                answer(&format!("Vai ser um jogo difícil. Temos que respeitar o {name}.")),
                answer("Prefiro não fazer previsões."),
                Answer { squad: 1, fans: 2, board: -1, provoke: true, ..answer(&format!("O {name} não mete medo na gente.")) },
            ],
        })
    }

    /// O treinador do adversário (num jogo par, pela seed): elogiar, fugir ou provocar.
    fn rival_coach_question(&self, opp: i32, fixture: i32) -> Result<Option<Question>> {
        let Some((_, coach)) = self.coach_of(opp)? else { return Ok(None) };
        if fixture % 2 != 0 {
            return Ok(None);
        }
        let name = self.club(opp).short_name;
        Ok(Some(Question {
            text: format!("O que você acha do trabalho de {coach} no {name}?"),
            person: None,
            answers: vec![
                answer(&format!("{coach} é um grande treinador. Temos muito respeito pelo {name}.")),
                answer("Não comento o trabalho dos colegas."),
                Answer { squad: 1, fans: 1, provoke: true, ..answer(&format!("Não entendo como {coach} ainda não foi demitido.")) },
            ],
        }))
    }

    /// O treinador provocou o adversário na conferência antes deste jogo.
    pub(super) fn provoked(&self, fixture: i32) -> Result<bool> {
        Ok(self
            .world
            .conn
            .query_row("SELECT provoked FROM press WHERE fixture = ? AND kind = 'pre' AND status = 'done'", [fixture], |r| r.get(0))
            .optional()?
            .unwrap_or(false))
    }

    /// Vitórias nos últimos 5 jogos do treinador.
    pub(super) fn recent_wins(&self) -> (usize, usize) {
        let mut games: Vec<&crate::league::Match> =
            self.matches.iter().filter(|m| m.result.is_some() && (m.home_id == self.club_id || m.away_id == self.club_id)).collect();
        games.sort_by(|a, b| b.date.cmp(&a.date));
        let last: Vec<_> = games.into_iter().take(5).collect();
        let wins = last
            .iter()
            .filter(|m| {
                let (h, a) = m.result.unwrap_or((0, 0));
                if m.home_id == self.club_id { h > a } else { a > h }
            })
            .count();
        (wins, last.len())
    }

    fn form_question(&self) -> Result<Option<Question>> {
        let (wins, games) = self.recent_wins();
        if games < 3 {
            return Ok(None);
        }
        Ok(Some(if wins <= 1 {
            Question {
                text: format!("O time ganhou {wins} dos últimos {games} jogos. Você está preocupado?"),
                person: None,
                answers: vec![
                    Answer { squad: 1, board: 1, ..answer("Vamos virar essa situação. Confio nos meus jogadores.") },
                    answer("Temos que trabalhar mais. Os resultados vão aparecer."),
                    Answer { squad: -2, board: 1, ..answer("Os jogadores precisam fazer muito mais.") },
                    answer("Não vou falar disso."),
                ],
            }
        } else {
            Question {
                text: format!("O time ganhou {wins} dos últimos {games} jogos. O que explica essa boa fase?"),
                person: None,
                answers: vec![
                    Answer { squad: 1, ..answer("O trabalho dos jogadores. Estamos no caminho certo.") },
                    answer("Ainda não ganhamos nada. Tem muita coisa para melhorar."),
                    Answer { fans: 1, ..answer("Este time pode chegar muito longe.") },
                ],
            }
        }))
    }

    /// Um jogador em destaque (nota 7 ou mais) ou em baixo (abaixo de 6).
    fn player_question(&self, person: i32, rating: f64) -> Result<Option<Question>> {
        let best = rating >= 7.0;
        if !best && rating >= 6.0 {
            return Ok(None);
        }
        let name = self.person_name(person)?;
        Ok(Some(if best {
            Question {
                text: format!("{name} vive grande fase. Merece mais reconhecimento?"),
                person: Some(person),
                answers: vec![
                    Answer { praise: Some(true), ..answer(&format!("{name} está jogando demais. Merece tudo de bom que vier.")) },
                    Answer { player: -1, ..answer("Ele ainda tem muito a provar.") },
                    answer("Não falo de jogadores individualmente."),
                ],
            }
        } else {
            Question {
                text: format!("{name} está abaixo do esperado. Você continua contando com ele?"),
                person: Some(person),
                answers: vec![
                    Answer { praise: Some(true), ..answer(&format!("{name} tem toda a minha confiança.")) },
                    Answer { praise: Some(false), ..answer(&format!("{name} precisa fazer muito mais.")) },
                    answer("Não falo de jogadores individualmente."),
                ],
            }
        }))
    }

    /// A terceira pergunta: um rumor de transferência, a pressão da direção ou um jogador.
    fn third_question(&self) -> Result<Option<Question>> {
        let rumour: Option<(i32, i32)> = self
            .world
            .conn
            .query_row(
                "SELECT person_id, buyer FROM offers WHERE seller = ? AND kind = 'transfer' AND status IN ('pending', 'raised', 'countered')
                 ORDER BY fee DESC LIMIT 1",
                [self.club_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((person, buyer)) = rumour {
            let name = self.person_name(person)?;
            let buyer = self.club(buyer).short_name;
            return Ok(Some(Question {
                text: format!("Dizem que o {buyer} quer {name}. Você vai vender?"),
                person: Some(person),
                answers: vec![
                    Answer { player: 1, fans: 1, ..answer(&format!("{name} não está à venda.")) },
                    Answer { player: -1, fans: -1, ..answer("No futebol tudo tem um preço.") },
                    answer("Não comento rumores."),
                ],
            }));
        }
        let confidence: i32 = self.world.conn.query_row("SELECT confidence FROM board WHERE id = 1", [], |r| r.get(0)).unwrap_or(50);
        if confidence < 40 {
            return Ok(Some(Question {
                text: "Você se sente pressionado pela Diretoria?".into(),
                person: None,
                answers: vec![
                    Answer { board: 1, ..answer("Tenho a confiança da Diretoria e vou retribuir.") },
                    answer("Estou tranquilo. Só penso no próximo jogo."),
                    Answer { board: -2, fans: 1, ..answer("A Diretoria sabia o que estava fazendo quando me contratou.") },
                ],
            }));
        }
        // Senão, um jogador com más exibições no último mês
        let poor: Option<i32> = self
            .world
            .conn
            .query_row(
                "SELECT l.person_id FROM player_log l JOIN people p ON p.id = l.person_id
                 WHERE l.kind = 'poor' AND p.club_id = ? AND l.date >= ? GROUP BY l.person_id HAVING COUNT(*) >= 2
                 ORDER BY COUNT(*) DESC, l.person_id LIMIT 1",
                params![self.club_id, date::add_days(&self.date, -30).unwrap_or_default()],
                |r| r.get(0),
            )
            .optional()?;
        match poor {
            Some(p) => self.player_question(p, 5.0),
            None => Ok(None),
        }
    }

    /// A conferência para a tela.
    pub fn press_view(&self, id: i32) -> Result<PressView> {
        let (kind, json, status): (String, String, String) = self
            .world
            .conn
            .query_row("SELECT kind, questions, status FROM press WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?
            .ok_or_else(|| Error::Invalid("Entrevista não encontrada.".into()))?;
        let questions: Vec<Question> = serde_json::from_str(&json).unwrap_or_default();
        Ok(PressView {
            id,
            title: "Entrevista Coletiva".into(),
            intro: if kind == "pre" {
                "Antes do jogo, os jornalistas querem saber:".into()
            } else {
                "Depois do jogo, os jornalistas querem saber:".into()
            },
            questions: questions
                .iter()
                .map(|q| PressQuestion { text: q.text.clone(), answers: q.answers.iter().map(|a| a.label.clone()).collect() })
                .collect(),
            status,
        })
    }

    /// As respostas do treinador (uma por pergunta): aplica os efeitos e põe as frases nas
    /// notícias. Devolve o resumo para a tela.
    pub fn answer_press(&mut self, id: i32, answers: &[usize]) -> Result<String> {
        let (json, status): (String, String) = self
            .world
            .conn
            .query_row("SELECT questions, status FROM press WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?
            .ok_or_else(|| Error::Invalid("Entrevista não encontrada.".into()))?;
        if status != "pending" {
            return Err(Error::Invalid("Essa entrevista já acabou.".into()));
        }
        let questions: Vec<Question> = serde_json::from_str(&json).unwrap_or_default();
        if answers.len() != questions.len() {
            return Err(Error::Invalid("Responda a todas as perguntas.".into()));
        }
        let mut next_id = self.news.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let mut fresh = Vec::new();
        let mut quotes = Vec::new();
        let mut reactions = Vec::new();
        let squad: Vec<i32> = self.squads.states.keys().copied().filter(|&p| self.club_of(p) == Some(self.club_id)).collect();
        let mut provoked = false;
        for (q, &i) in questions.iter().zip(answers) {
            let a = q.answers.get(i).ok_or_else(|| Error::Invalid("Resposta inválida.".into()))?;
            provoked |= a.provoke;
            quotes.push(format!("\"{}\"", a.label));
            if a.squad != 0 {
                for &p in &squad {
                    self.change_morale(p, a.squad);
                }
            }
            if let Some(p) = q.person {
                if let Some(praise) = a.praise {
                    let (_, reaction) = self.press_reaction(p, praise)?;
                    reactions.push(reaction);
                }
                if a.player != 0 {
                    self.change_morale(p, a.player);
                }
            }
            if a.board != 0 {
                self.change_confidence(a.board, &mut fresh, &mut next_id)?;
            }
            if a.fans != 0 {
                self.world.conn.execute("UPDATE board SET fans = MIN(100, MAX(0, fans + ?)) WHERE id = 1", [a.fans])?;
            }
        }
        self.world.conn.execute("UPDATE press SET status = 'done', provoked = ? WHERE id = ?", params![provoked, id])?;
        let club = self.club(self.club_id).short_name;
        let mut body = format!("Na entrevista coletiva, o treinador do {club} disse: {}", quotes.join(" "));
        if provoked {
            body = format!(
                "{body}

As palavras do treinador caíram mal no adversário, que promete dar uma resposta em campo."
            );
        }
        for r in &reactions {
            body = format!("{body}\n\n{r}");
        }
        fresh.push(self.news_item(next_id, "press", format!("Treinador do {club} dá entrevista coletiva"), body.clone()));
        self.fresh_ids(&mut fresh);
        for n in fresh {
            self.news.insert(0, n);
        }
        self.unsaved = true;
        Ok(body)
    }

    /// "Mandar o Adjunto": responde por ele, sem efeitos.
    pub fn skip_press(&mut self, id: i32) -> Result<String> {
        let n = self.world.conn.execute("UPDATE press SET status = 'skipped' WHERE id = ? AND status = 'pending'", [id])?;
        if n == 0 {
            return Err(Error::Invalid("Essa entrevista já acabou.".into()));
        }
        self.unsaved = true;
        Ok("O auxiliar foi à entrevista coletiva e respondeu sem comprometer ninguém.".into())
    }

    fn club_of(&self, person: i32) -> Option<i32> {
        self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [person], |r| r.get(0)).ok().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_conference_after_the_match() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-imprensa-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Imprensa".into(),
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
        let mut g = Game::create(&path, &dir, &new, 21).unwrap();
        let first = g.matches.iter().filter(|m| m.home_id == 7568 || m.away_id == 7568).map(|m| m.date.clone()).min().unwrap();
        while g.date.as_str() <= first.as_str() {
            g.advance_day().unwrap();
        }
        let id: i32 = g.world.conn.query_row("SELECT id FROM press WHERE kind = 'post' ORDER BY id LIMIT 1", [], |r| r.get(0)).unwrap();
        assert!(g.news().iter().any(|n| n.link.as_deref() == Some(&format!("press:{id}"))));
        let view = g.press_view(id).unwrap();
        assert!(!view.questions.is_empty() && view.questions.iter().all(|q| q.answers.len() >= 3));
        assert!(g.answer_press(id, &[0]).is_err() || view.questions.len() == 1, "uma resposta por pergunta");
        let answers = vec![0; view.questions.len()];
        let text = g.answer_press(id, &answers).unwrap();
        assert!(text.contains("disse"));
        assert_eq!(g.press_view(id).unwrap().status, "done");
        assert!(g.answer_press(id, &answers).is_err(), "só uma vez");

        // Antes do próximo jogo: provocar o treinador adversário motiva o adversário
        let next = g.matches.iter().find(|m| m.result.is_none() && (m.home_id == 7568 || m.away_id == 7568)).unwrap().clone();
        let opp = if next.home_id == 7568 { next.away_id } else { next.home_id };
        let q = g.rival_coach_question(opp, 2).unwrap().expect("o adversário tem treinador");
        assert!(q.answers.iter().any(|a| a.provoke));
        let (mut fresh, mut nid) = (Vec::new(), 9000);
        let deadline = g.date.clone();
        g.open_press("pre", next.id, &deadline, &[q], &mut fresh, &mut nid, "rival").unwrap();
        let pre: i32 = g.world.conn.query_row("SELECT MAX(id) FROM press", [], |r| r.get(0)).unwrap();
        assert!(!g.provoked(next.id).unwrap());
        let text = g.answer_press(pre, &[2]).unwrap();
        assert!(text.contains("resposta em campo"), "{text}");
        assert!(g.provoked(next.id).unwrap());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
