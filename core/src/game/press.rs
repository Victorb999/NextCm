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
/// Lugares do topo e do fundo da tabela que tornam um jogo importante.
const TOP: i32 = 4;
const BOTTOM: i32 = 4;

/// Uma resposta e o que ela faz.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Answer {
    label: String,
    /// Moral de todo o plantel.
    #[serde(default)]
    squad: i32,
    /// Elogio (`true`) ou crítica ao jogador da pergunta.
    #[serde(default)]
    praise: Option<bool>,
    /// Moral só do jogador da pergunta.
    #[serde(default)]
    player: i32,
    #[serde(default)]
    board: i32,
    #[serde(default)]
    fans: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Question {
    text: String,
    #[serde(default)]
    person: Option<i32>,
    answers: Vec<Answer>,
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

fn answer(label: &str) -> Answer {
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
                    "Direcção critica ausências".into(),
                    "A Direcção não gostou de ver o treinador faltar às conferências de imprensa antes dos jogos.".into(),
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
        if exists || !self.important(&m)? {
            return Ok(());
        }
        let opp = if m.home_id == self.club_id { m.away_id } else { m.home_id };
        let mut questions = vec![self.opponent_question(opp)?];
        questions.extend(self.form_question()?);
        questions.extend(self.third_question()?);
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
        let (text, answers) = if ours > theirs {
            (
                format!("Vitória por {score} sobre o {opp_name}. Satisfeito com a exibição?"),
                vec![
                    Answer { squad: 1, fans: 1, ..answer("Merecemos ganhar, fomos melhores do princípio ao fim.") },
                    answer("Foi só um jogo. Ainda não ganhámos nada."),
                    Answer { fans: 2, board: -1, ..answer("Hoje calámos muitos críticos.") },
                    answer("Prefiro não falar do jogo."),
                ],
            )
        } else if ours < theirs {
            (
                format!("Derrota por {score} com o {opp_name}. O que falhou?"),
                vec![
                    Answer { squad: 1, board: -1, ..answer("A responsabilidade é minha. Os jogadores deram tudo.") },
                    Answer { squad: -2, board: 1, ..answer("Alguns jogadores não estiveram ao nível que exijo.") },
                    Answer { fans: 1, board: -1, ..answer("O árbitro decidiu o jogo.") },
                    answer("Vamos analisar com calma."),
                ],
            )
        } else {
            (
                format!("Empate a {score} com o {opp_name}. Um ponto ganho ou dois perdidos?"),
                vec![
                    Answer { squad: 1, ..answer("Um bom ponto. A equipa mostrou carácter.") },
                    Answer { squad: -1, board: 1, ..answer("Dois pontos perdidos. Tínhamos de ganhar.") },
                    answer("Não há muito a dizer sobre o jogo."),
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
        if sent_off {
            questions.push(Question {
                text: "Teve um jogador expulso. A arbitragem foi justa?".into(),
                person: None,
                answers: vec![
                    answer("Não comento as decisões do árbitro."),
                    Answer { fans: 1, board: -1, ..answer("Fomos claramente prejudicados.") },
                    Answer { squad: -1, ..answer("O jogador tem de controlar os nervos.") },
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
                format!("Conferência antes do jogo com o {opp}"),
                format!("Os jornalistas esperam-no na conferência de imprensa antes do jogo com o {opp}. Pode ir ou mandar o adjunto."),
            )
        } else {
            (format!("Conferência depois do jogo com o {opp}"), "Os jornalistas querem ouvir o treinador depois do jogo.".to_string())
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

    fn opponent_question(&self, opp: i32) -> Result<Question> {
        let name = self.club(opp).short_name;
        Ok(Question {
            text: format!("Como vê o jogo com o {name}?"),
            person: None,
            answers: vec![
                Answer { squad: 1, fans: 1, ..answer("Vamos ganhar. Somos melhores e vamos prová-lo.") },
                answer(&format!("Será um jogo difícil. Temos de respeitar o {name}.")),
                answer("Prefiro não fazer previsões."),
                Answer { squad: 1, fans: 2, board: -1, ..answer(&format!("O {name} não nos mete medo nenhum.")) },
            ],
        })
    }

    /// Vitórias nos últimos 5 jogos do treinador.
    fn recent_wins(&self) -> (usize, usize) {
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
                text: format!("A equipa ganhou {wins} dos últimos {games} jogos. Está preocupado?"),
                person: None,
                answers: vec![
                    Answer { squad: 1, board: 1, ..answer("Vamos dar a volta a isto. Confio nos meus jogadores.") },
                    answer("Temos de trabalhar mais. Os resultados vão aparecer."),
                    Answer { squad: -2, board: 1, ..answer("Os jogadores têm de fazer muito mais.") },
                    answer("Não vou falar disso."),
                ],
            }
        } else {
            Question {
                text: format!("A equipa ganhou {wins} dos últimos {games} jogos. O que explica a forma da equipa?"),
                person: None,
                answers: vec![
                    Answer { squad: 1, ..answer("O trabalho dos jogadores. Estamos no caminho certo.") },
                    answer("Ainda não ganhámos nada. Há muito a melhorar."),
                    Answer { fans: 1, ..answer("Esta equipa pode chegar muito longe.") },
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
                text: format!("{name} tem estado em grande. Merece mais reconhecimento?"),
                person: Some(person),
                answers: vec![
                    Answer { praise: Some(true), ..answer(&format!("{name} tem sido fantástico. Merece tudo o que lhe acontecer.")) },
                    Answer { player: -1, ..answer("Ainda tem muito a provar.") },
                    answer("Não falo de jogadores individualmente."),
                ],
            }
        } else {
            Question {
                text: format!("{name} tem estado abaixo do esperado. Continua a contar com ele?"),
                person: Some(person),
                answers: vec![
                    Answer { praise: Some(true), ..answer(&format!("{name} tem a minha confiança total.")) },
                    Answer { praise: Some(false), ..answer(&format!("{name} tem de fazer muito mais.")) },
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
                text: format!("Fala-se do interesse do {buyer} em {name}. Vai vendê-lo?"),
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
                text: "Sente-se pressionado pela Direcção?".into(),
                person: None,
                answers: vec![
                    Answer { board: 1, ..answer("Tenho a confiança da Direcção e vou retribuí-la.") },
                    answer("Estou tranquilo. Só penso no próximo jogo."),
                    Answer { board: -2, fans: 1, ..answer("A Direcção sabia o que estava a fazer quando me contratou.") },
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
            .ok_or_else(|| Error::Invalid("Conferência não encontrada.".into()))?;
        let questions: Vec<Question> = serde_json::from_str(&json).unwrap_or_default();
        Ok(PressView {
            id,
            title: "Conferência de Imprensa".into(),
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
            .ok_or_else(|| Error::Invalid("Conferência não encontrada.".into()))?;
        if status != "pending" {
            return Err(Error::Invalid("Esta conferência já acabou.".into()));
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
        for (q, &i) in questions.iter().zip(answers) {
            let a = q.answers.get(i).ok_or_else(|| Error::Invalid("Resposta inválida.".into()))?;
            quotes.push(format!("«{}»", a.label.trim_end_matches('.')));
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
        self.world.conn.execute("UPDATE press SET status = 'done' WHERE id = ?", [id])?;
        let club = self.club(self.club_id).short_name;
        let mut body = format!("Na conferência de imprensa, o treinador do {club} disse: {}.", quotes.join(" "));
        for r in &reactions {
            body = format!("{body}\n\n{r}");
        }
        fresh.push(self.news_item(next_id, "press", format!("O treinador do {club} fala à imprensa"), body.clone()));
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
            return Err(Error::Invalid("Esta conferência já acabou.".into()));
        }
        self.unsaved = true;
        Ok("O adjunto foi à conferência de imprensa e respondeu sem comprometer ninguém.".into())
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
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
