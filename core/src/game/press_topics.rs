//! Os temas das conferências de imprensa tirados da situação do clube, para não serem sempre
//! as mesmas perguntas: a disputa do título, a luta contra o rebaixamento, o clássico, os
//! elogios e as críticas ao treinador, o jogador insatisfeito, o titular lesionado, o reforço
//! que acabou de chegar e as continentais. Cada conferência sorteia (pela partida) entre os que
//! se aplicam, e as perguntas de sempre têm várias formas de as fazer.

use rand_core::RngCore;
use rusqlite::{OptionalExtension, params};

use super::Game;
use super::press::{Answer, Question, answer};
use crate::engine;
use crate::league::{CHAMPIONS_LEAGUE, EUROPA_LEAGUE, LIBERTADORES, Match, SUL_AMERICANA};
use crate::{Result, date, regulation};

/// Pontos de distância que fazem uma disputa (do título, da permanência).
const RACE_GAP: i32 = 4;
/// Lugares do fundo que contam como luta contra o rebaixamento (com a distância acima).
const DANGER: i32 = 4;

/// Uma das formas de dizer a mesma coisa, pelo sorteio da conferência.
pub(super) fn pick<'a>(rng: &mut impl RngCore, options: &[&'a str]) -> &'a str {
    options[(rng.next_u32() as usize) % options.len()]
}

impl Game {
    /// O sorteio das perguntas de uma conferência (o mesmo ao reabrir o jogo).
    pub(super) fn press_rng(&self, fixture: i32, kind: i64) -> rand_chacha::ChaCha8Rng {
        engine::match_rng(self.seed ^ 0x9E55_0000 ^ kind, fixture)
    }

    /// Os temas que se aplicam agora ao clube do treinador, para o jogo `m` (antes ou depois).
    pub(super) fn topical_questions(&self, m: &Match, rng: &mut impl RngCore) -> Result<Vec<Question>> {
        let mut out = Vec::new();
        let us = self.club(self.club_id).short_name;
        let opp = if m.home_id == self.club_id { m.away_id } else { m.home_id };
        let opp_name = self.club(opp).short_name;

        // O clássico
        let rivals: [Option<i32>; 3] =
            self.world.conn.query_row("SELECT rival1_id, rival2_id, rival3_id FROM clubs WHERE id = ?", [self.club_id], |r| {
                Ok([r.get(0)?, r.get(1)?, r.get(2)?])
            })?;
        if rivals.contains(&Some(opp)) && m.result.is_none() {
            out.push(Question {
                text: pick(
                    rng,
                    &[
                        "É o clássico com o {opp}. O que este jogo significa para a cidade?",
                        "A torcida só fala do jogo com o {opp}. Vale mais do que três pontos?",
                        "Clássico é jogo à parte. Como você preparou o time para enfrentar o {opp}?",
                    ],
                )
                .replace("{opp}", &opp_name),
                person: None,
                answers: vec![
                    Answer { squad: 1, fans: 2, ..answer("É o jogo do ano para os nossos torcedores. Vamos dar tudo por eles.") },
                    answer("São três pontos como os outros. Tratamos este jogo com a mesma seriedade."),
                    Answer {
                        fans: 2,
                        board: -1,
                        provoke: true,
                        ..answer(&format!("A cidade é nossa, e no fim do jogo o {opp_name} vai lembrar disso."))
                    },
                ],
            });
        }

        // A tabela da liga: a disputa do título e a luta contra o rebaixamento
        if let Ok(Some(comp)) = self.division_of(self.club_id) {
            let table = self.table(comp);
            let n = table.len() as i32;
            if let Some(ours) = table.iter().find(|r| r.club.id == self.club_id).filter(|r| r.played >= 8) {
                let leader = &table[0];
                let second = table.get(1);
                let chaser = if ours.position == 1 { second } else { Some(leader) };
                if let Some(other) = chaser.filter(|o| ours.position <= 2 && (ours.points - o.points).abs() <= RACE_GAP) {
                    let name = &other.club.short_name;
                    out.push(Question {
                        text: pick(
                            rng,
                            &[
                                "A disputa do título com o {other} está ponto a ponto. Quem chega na frente?",
                                "O {other} não larga o {us} na briga pelo título. Você sente a pressão?",
                                "Muitos dizem que o título se decide entre {us} e {other}. Você concorda?",
                            ],
                        )
                        .replace("{other}", name)
                        .replace("{us}", &us),
                        person: None,
                        answers: vec![
                            Answer {
                                squad: 1, fans: 1, ..answer("Dependemos só de nós. Se ganharmos os nossos jogos, o título é nosso.")
                            },
                            answer(&format!("O {name} tem um grande time. Vai ser decidido nos detalhes.")),
                            Answer { fans: 2, board: -1, provoke: true, ..answer(&format!("A pressão está toda do lado do {name}.")) },
                            answer("Não olho para a tabela. Só para o próximo jogo."),
                        ],
                    });
                }
                let safety = table.get((n - DANGER - 1).max(0) as usize).map_or(0, |r| r.points);
                if ours.position > n - DANGER || ours.points - safety <= 2 {
                    out.push(Question {
                        text: pick(
                            rng,
                            &[
                                "O {us} está na zona de perigo. Você acredita que o time escapa?",
                                "Os torcedores temem o rebaixamento. O que você diz a eles?",
                                "A luta para não cair vai até o fim. O time tem estrutura para isso?",
                            ],
                        )
                        .replace("{us}", &us),
                        person: None,
                        answers: vec![
                            Answer { squad: 1, fans: 1, ..answer("Vamos sair dessa. Este grupo tem caráter.") },
                            Answer { board: 1, ..answer("Sabemos a situação. Cada jogo agora é uma final.") },
                            Answer { squad: -1, board: 1, ..answer("Alguns jogadores precisam entender o que está em jogo.") },
                        ],
                    });
                }
            }
        }

        // Elogios ou críticas ao treinador, pela forma
        let (wins, games) = self.recent_wins();
        if games >= 5 && wins >= 4 {
            out.push(Question {
                text: pick(
                    rng,
                    &[
                        "Tem gente que já coloca você entre os melhores treinadores do país. Como você recebe os elogios?",
                        "O time joga um futebol que encanta. Qual é o segredo?",
                        "Comentaristas falam de você para a seleção um dia. Você pensa nisso?",
                    ],
                )
                .into(),
                person: None,
                answers: vec![
                    Answer { squad: 1, ..answer("O mérito é dos jogadores. Eu só dou uma ajuda.") },
                    answer("Elogios não ganham jogos. Seguimos trabalhando."),
                    Answer { fans: 1, board: 1, ..answer("Fico feliz. É o resultado de muito trabalho nesta casa.") },
                ],
            });
        } else if games >= 5 && wins == 0 {
            out.push(Question {
                text: pick(
                    rng,
                    &[
                        "A torcida começa a pedir a sua saída. Como responde?",
                        "Cinco jogos sem vencer. O seu trabalho está ameaçado?",
                        "Os críticos dizem que o time perdeu o rumo. Você ainda tem as respostas?",
                    ],
                )
                .into(),
                person: None,
                answers: vec![
                    Answer { board: 1, ..answer("Entendo a frustração. A resposta vai ser dada em campo.") },
                    Answer { squad: 1, ..answer("Confio nos meus jogadores e no meu trabalho.") },
                    Answer { fans: -1, board: -1, ..answer("Quem me critica não vê o que fazemos no dia a dia.") },
                ],
            });
        }

        // As continentais
        if [CHAMPIONS_LEAGUE, EUROPA_LEAGUE, LIBERTADORES, SUL_AMERICANA].contains(&m.competition_id) && m.result.is_none() {
            let comp = self.fixture(m).competition;
            out.push(Question {
                text: pick(
                    rng,
                    &[
                        "Que chances você dá ao {us} na {comp} desta temporada?",
                        "O clube sonha alto na {comp}. É realista pensar no título?",
                    ],
                )
                .replace("{us}", &us)
                .replace("{comp}", &comp),
                person: None,
                answers: vec![
                    Answer { squad: 1, fans: 1, ..answer("Viemos para ganhar. Não tem outro objetivo.") },
                    Answer { board: 1, ..answer("Jogo a jogo. Tem times muito fortes nesta competição.") },
                    answer("A liga é a prioridade, mas vamos dar o máximo."),
                ],
            });
        }

        // O mata-mata antes do jogo: jogo único, ida ou volta (com o placar da ida)
        if m.result.is_none()
            && let Some(q) = self.knockout_preview(m, &us, &opp_name, rng)
        {
            out.push(q);
        }

        // O jogador insatisfeito do plantel
        let unhappy: Option<i32> = self
            .world
            .conn
            .query_row(
                "SELECT u.person_id FROM unhappy u JOIN people p ON p.id = u.person_id WHERE p.club_id = ?
                 ORDER BY u.since, u.person_id LIMIT 1",
                [self.club_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(p) = unhappy {
            let name = self.person_name(p)?;
            out.push(Question {
                text: pick(
                    rng,
                    &[
                        "Dizem que {name} anda descontente no clube. Como está essa situação?",
                        "{name} não parece feliz. Existe risco de ele sair?",
                    ],
                )
                .replace("{name}", &name),
                person: Some(p),
                answers: vec![
                    Answer { player: 1, ..answer(&format!("{name} é importante para nós. Vamos conversar e resolver.")) },
                    Answer { player: -1, board: 1, ..answer("Ninguém é maior do que o clube.") },
                    answer("Assuntos internos ficam dentro do vestiário."),
                ],
            });
        }

        // O titular lesionado
        let keys = self.key_players(self.club_id)?;
        if let Some(&p) = keys.iter().find(|id| self.squads.state(**id).injury.is_some()) {
            let name = self.person_name(p)?;
            out.push(Question {
                text: pick(
                    rng,
                    &["Como o time vai lidar com a ausência de {name}?", "Sem {name}, o time perde muito. Quem assume a responsabilidade?"],
                )
                .replace("{name}", &name),
                person: Some(p),
                answers: vec![
                    Answer { squad: 1, ..answer("Temos um grupo forte. Quem entrar vai dar conta.") },
                    answer(&format!("Perder {name} dói, mas não podemos nos esconder atrás disso.")),
                ],
            });
        }

        // O reforço que acabou de chegar
        let signing: Option<i32> = self
            .world
            .conn
            .query_row(
                "SELECT person_id FROM transfers WHERE to_club = ? AND kind IN ('transfer', 'free', 'loan') AND date >= ?
                 ORDER BY fee DESC, id DESC LIMIT 1",
                params![self.club_id, date::add_days(&self.date, -21).unwrap_or_default()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(p) = signing {
            let name = self.person_name(p)?;
            out.push(Question {
                text: pick(
                    rng,
                    &[
                        "{name} acabou de chegar. Já está pronto para jogar?",
                        "Os torcedores estão animados com {name}. O que ele traz ao time?",
                    ],
                )
                .replace("{name}", &name),
                person: Some(p),
                answers: vec![
                    Answer { player: 1, fans: 1, ..answer(&format!("{name} é exatamente o que procurávamos. Vai fazer a diferença.")) },
                    answer("Precisa de tempo para se adaptar. Não vamos queimar etapas."),
                ],
            });
        }

        // A ordem dos temas é a do sorteio
        for i in (1..out.len()).rev() {
            let j = (rng.next_u32() as usize) % (i + 1);
            out.swap(i, j);
        }
        Ok(out)
    }

    /// A ida de uma eliminatória já jogada: os gols a favor menos os contra do treinador.
    fn first_leg_lead(&self, m: &Match) -> Option<i32> {
        let year = regulation::season_of(m.competition_id, &m.date);
        let games = self.tie_games(m.competition_id, year, m.stage as usize, m.round, m.tie?);
        let first = games.iter().find(|g| g.leg == 1)?;
        let (h, a) = first.result?;
        let (h, a) = (h as i32, a as i32);
        Some(if first.home_id == self.club_id { h - a } else { a - h })
    }

    /// Antes de um jogo de mata-mata: a pressão do jogo único, a ida ou a volta com o placar da ida.
    fn knockout_preview(&self, m: &Match, us: &str, opp: &str, rng: &mut impl RngCore) -> Option<Question> {
        m.tie?;
        let goals = |n: i32| if n.abs() == 1 { "um gol".to_string() } else { format!("{} gols", n.abs()) };
        let (texts, answers): (&[&str], Vec<Answer>) = match (m.leg, self.first_leg_lead(m)) {
            (2, Some(lead)) if lead > 0 => (
                &[
                    "O {us} leva {gols} de vantagem para a volta. Dá para administrar?",
                    "Com {gols} de frente, o {us} joga pelo empate?",
                    "Vantagem de {gols} sobre o {opp}. Existe o risco de relaxar?",
                ],
                vec![
                    Answer { board: 1, ..answer("Não tem nada decidido. Vamos jogar para ganhar.") },
                    Answer { squad: 1, fans: 1, ..answer("Fizemos o mais difícil. Agora é confirmar.") },
                    Answer { fans: 1, provoke: true, ..answer(&format!("O {opp} vai precisar de um milagre.")) },
                ],
            ),
            (2, Some(lead)) if lead < 0 => (
                &[
                    "O {us} precisa tirar {gols} de desvantagem. Você acredita na virada?",
                    "Perdendo por {gols} no agregado, o que o time precisa fazer contra o {opp}?",
                    "A torcida ainda acredita na classificação. E você?",
                ],
                vec![
                    Answer { squad: 1, fans: 2, ..answer("Acredito, e muito. Com a torcida do nosso lado, tudo é possível.") },
                    answer("Vai ser difícil, mas já vimos viradas maiores."),
                    Answer { board: 1, ..answer("Temos que ser pacientes. Um gol cedo muda tudo.") },
                ],
            ),
            (2, _) => (
                &[
                    "Tudo igual na eliminatória com o {opp}. Quem tem mais nervos?",
                    "A vaga se decide nos 90 minutos de amanhã. Como o time está?",
                    "Empate no agregado. O time está preparado até para os pênaltis?",
                ],
                vec![
                    Answer { squad: 1, ..answer("Estamos prontos. Treinamos tudo, até os pênaltis.") },
                    answer("Vai ser decidido nos detalhes. Quem errar menos passa."),
                    Answer { fans: 1, provoke: true, ..answer(&format!("A pressão é toda do {opp}.")) },
                ],
            ),
            (1, _) => (
                &[
                    "Primeiro jogo da eliminatória com o {opp}. Um empate já serve?",
                    "Como o time vai encarar a ida contra o {opp}?",
                    "Na ida, vale mais atacar ou não sofrer gols?",
                ],
                vec![
                    Answer { squad: 1, fans: 1, ..answer("Queremos resolver já na ida. Vamos para cima.") },
                    answer("São 180 minutos. Não dá para decidir nada amanhã, mas dá para perder."),
                    Answer { board: 1, ..answer("Não sofrer gols é tão importante quanto marcar.") },
                ],
            ),
            _ => (
                &[
                    "Jogo único contra o {opp}, sem margem para erro. Como o time encara o mata-mata?",
                    "Quem perder está fora. O time aguenta essa pressão?",
                    "Uma noite pode decidir a temporada. O que você diz aos jogadores?",
                ],
                vec![
                    Answer { squad: 1, fans: 1, ..answer("É para esses jogos que estamos aqui. O time está pronto.") },
                    answer("Mata-mata não tem favorito. Vamos respeitar o adversário."),
                    Answer { squad: -1, board: 1, ..answer("Quem não aguentar a pressão não vai jogar.") },
                ],
            ),
        };
        let lead = self.first_leg_lead(m).unwrap_or(0);
        Some(Question {
            text: pick(rng, texts).replace("{us}", us).replace("{opp}", opp).replace("{gols}", &goals(lead)),
            person: None,
            answers,
        })
    }

    /// Depois de um jogo de mata-mata, a pergunta do resultado: a ida, a classificação ou a
    /// eliminação (também nos pênaltis). `None` fora do mata-mata.
    pub(super) fn knockout_result(&self, m: &Match, (ours, theirs): (i32, i32), rng: &mut impl RngCore) -> Option<Question> {
        m.tie?;
        let us = self.club(self.club_id).short_name;
        let opp = self.club(if m.home_id == self.club_id { m.away_id } else { m.home_id }).short_name;
        let (texts, answers): (&[&str], Vec<Answer>) = if m.leg == 1 {
            match ours.cmp(&theirs) {
                std::cmp::Ordering::Greater => (
                    &[
                        "Vitória por {score} na ida. O {opp} ainda pode reagir na volta?",
                        "{score} sobre o {opp} no primeiro jogo. A vaga está encaminhada?",
                        "O {us} sai na frente na eliminatória. Que vantagem é essa?",
                    ],
                    vec![
                        Answer { squad: 1, fans: 1, ..answer("Fizemos metade do trabalho. Agora é terminar na volta.") },
                        answer(&format!("Nada está decidido. O {opp} vai vir com tudo.")),
                        Answer { fans: 1, provoke: true, ..answer(&format!("Se o {opp} jogar como hoje, não passa.")) },
                    ],
                ),
                std::cmp::Ordering::Less => (
                    &[
                        "Derrota por {score} na ida. Dá para virar na volta?",
                        "O {opp} venceu o primeiro jogo por {score}. A eliminatória ficou difícil?",
                        "O {us} vai precisar de uma virada. O que tem que mudar?",
                    ],
                    vec![
                        Answer { squad: 1, fans: 1, ..answer("Dá para virar. Acredito neste grupo.") },
                        answer("Vamos precisar de um jogo perfeito na volta."),
                        Answer { squad: -1, board: 1, ..answer("Erramos demais hoje. Se não melhorarmos, estamos fora.") },
                    ],
                ),
                std::cmp::Ordering::Equal => (
                    &["{score} na ida com o {opp}. Fica tudo para a volta?", "Empate no primeiro jogo. O resultado foi bom para o {us}?"],
                    vec![
                        answer("Resultado aberto. Vamos decidir na volta."),
                        Answer { squad: 1, ..answer("O time está vivo na disputa. Saio confiante.") },
                        Answer { squad: -1, board: 1, ..answer("Podíamos ter resolvido mais hoje.") },
                    ],
                ),
            }
        } else {
            let year = regulation::season_of(m.competition_id, &m.date);
            let games = self.tie_games(m.competition_id, year, m.stage as usize, m.round, m.tie?);
            let pens = games.last().is_some_and(|g| g.pens.is_some());
            let through = Self::tie_winner(&games)? == self.club_id;
            match (through, pens) {
                (true, true) => (
                    &[
                        "Classificação nos pênaltis contra o {opp}. Sofrimento até o fim?",
                        "O {us} passou nos pênaltis. O goleiro foi o herói?",
                    ],
                    vec![
                        Answer { squad: 1, ..answer("Pênalti também se treina. Estávamos preparados.") },
                        answer("Sofremos mais do que devíamos. Temos que matar o jogo antes."),
                        Answer { fans: 1, ..answer("Este grupo não desiste nunca. Os torcedores merecem.") },
                    ],
                ),
                (true, false) => (
                    &[
                        "O {us} passou pelo {opp}. Qual foi o segredo da classificação?",
                        "Vaga garantida contra o {opp}. Até onde o time pode ir?",
                        "{score} e o {us} segue em frente. Dá para sonhar com o título?",
                    ],
                    vec![
                        Answer { squad: 1, fans: 1, ..answer("O grupo mereceu. Agora é pensar na próxima fase.") },
                        Answer { fans: 2, board: -1, ..answer("Este time pode ganhar a competição.") },
                        answer("Passamos, mas ainda temos muito a melhorar."),
                    ],
                ),
                (false, true) => (
                    &[
                        "Eliminado nos pênaltis pelo {opp}. Dá para culpar a sorte?",
                        "O {us} caiu nos pênaltis. O que você diz aos jogadores que erraram?",
                    ],
                    vec![
                        Answer { squad: 1, ..answer("Pênalti é loteria. Não dá para cobrar ninguém.") },
                        answer("Não foi nos pênaltis que perdemos. Foi antes, quando não matamos o jogo."),
                        Answer { squad: -1, board: 1, ..answer("Quem bate pênalti tem que ter frieza. Faltou.") },
                    ],
                ),
                (false, false) => (
                    &[
                        "O {us} está eliminado pelo {opp}. Qual o tamanho da decepção?",
                        "Fim da linha na competição. O que faltou contra o {opp}?",
                        "A eliminação para o {opp} mancha a temporada?",
                    ],
                    vec![
                        Answer { squad: 1, board: -1, ..answer("A responsabilidade é minha. Os jogadores deram tudo.") },
                        Answer { squad: -2, board: 1, ..answer("Faltou atitude de alguns jogadores.") },
                        answer("Vamos levantar a cabeça e focar no campeonato."),
                    ],
                ),
            }
        };
        let text = pick(rng, texts).replace("{us}", &us).replace("{opp}", &opp).replace("{score}", &format!("{ours}-{theirs}"));
        Some(Question { text, person: None, answers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::league::SERIE_A;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn press_questions_follow_the_situation_and_vary() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = crate::TempDir::new(format!("nextcm-temas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Temas".into(),
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
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 3).unwrap();
        let today = g.date.clone();
        let rival: i32 = g.world.conn.query_row("SELECT rival1_id FROM clubs WHERE id = ?", [PALMEIRAS], |r| r.get(0)).unwrap();
        let rival_name = g.club(rival).short_name;
        let game = |id: i32| Match {
            id,
            competition_id: SERIE_A,
            stage: 0,
            round: 1,
            group: None,
            tie: None,
            leg: 0,
            neutral: false,
            date: today.clone(),
            home_id: PALMEIRAS,
            away_id: rival,
            result: None,
            pens: None,
            extra_time: false,
        };
        // No clássico pergunta-se pelo clássico
        let mut rng = g.press_rng(1, 1);
        let topics = g.topical_questions(&game(1), &mut rng).unwrap();
        assert!(
            topics.iter().any(|q| q.text.contains(&rival_name) && q.answers.iter().any(|a| a.provoke)),
            "{:?}",
            topics.iter().map(|q| &q.text).collect::<Vec<_>>()
        );
        // A mesma pergunta de várias formas, conforme a partida
        let texts: std::collections::HashSet<String> = (0..12)
            .map(|id| {
                let mut rng = g.press_rng(id, 1);
                pick(&mut rng, &["a", "b", "c", "d"]).to_string()
            })
            .collect();
        assert!(texts.len() >= 3, "{texts:?}");

        // Mata-mata: nunca se fala de pontos; nos pênaltis, a classificação nos pênaltis
        let ko = |id: i32, leg: u8, result: (u8, u8), pens: Option<(u8, u8)>| Match {
            tie: Some(0),
            stage: 9,
            leg,
            result: Some(result),
            pens,
            ..game(id)
        };
        let first_leg = ko(900_001, 1, (1, 1), None);
        let q = g.knockout_result(&first_leg, (1, 1), &mut rng).expect("ida");
        assert!(!q.text.contains("ponto") && q.answers.iter().all(|a| !a.label.contains("ponto")), "{}", q.text);
        let decider = ko(900_002, 0, (1, 1), Some((4, 2)));
        g.matches.push(decider.clone());
        let q = g.knockout_result(&decider, (1, 1), &mut rng).expect("jogo único");
        assert!(q.text.contains("pênaltis") && !q.text.contains("ponto"), "{}", q.text);
        assert!(g.knockout_result(&game(900_003), (1, 1), &mut rng).is_none(), "fora do mata-mata");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
