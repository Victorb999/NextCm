//! Palestras (marco 3.14), como no FM08: antes do jogo, ao intervalo e no fim, o treinador
//! fala à equipa e cada jogador reage pela personalidade (Pressão, Jogos Importantes,
//! Determinação, Temperamento, Profissionalismo) e pela situação (favorito ou não, o
//! resultado). A reação mexe na moral e, antes do jogo e ao intervalo, numa motivação só
//! desta partida (±1,5% de rendimento por ponto). O tom (calmo, assertivo, agressivo,
//! motivador) acerta a reação; fala-se à equipa toda ou a um jogador, e ao suplente que
//! acabou de entrar. Sem palestra (a CLI, o adjunto) o jogo é o mesmo: nada aqui gasta o RNG.

use serde::Serialize;
use ts_rs::TS;

use super::Game;
use crate::{Error, Result};

/// Rendimento na partida por ponto de reação.
const MOTIVATION: f64 = 0.015;
/// Diferença de reputação (com a casa) que torna um clube favorito.
const FAVOURITE_REP: i32 = 1000;
const HOME_REP: i32 = 500;

/// Uma frase que o treinador pode dizer.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TalkOption {
    pub key: String,
    pub label: String,
}

/// O painel "Palestra": o momento, as frases e os tons, a frase que o adjunto sugere, a
/// quem se pode falar e se a palestra à equipa já foi dada.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TalkView {
    /// "pre", "half", "full" ou "sub" (a palavra ao suplente que entrou).
    pub moment: String,
    pub title: String,
    pub options: Vec<TalkOption>,
    pub tones: Vec<TalkOption>,
    pub suggested: String,
    /// Os jogadores a quem se pode falar sozinhos (`key` = id).
    pub players: Vec<TalkOption>,
    /// À equipa toda (não no "sub").
    pub team: bool,
    pub done: bool,
}

/// A reação de um jogador.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TalkReaction {
    pub person_id: i32,
    pub name: String,
    /// "Motivado", "Focado", "Sem reação", "Complacente", "Nervoso", "Irritado".
    pub reaction: String,
    /// De -2 a +2.
    pub delta: i32,
}

const PRE: [(&str, &str); 4] = [
    ("expect_win", "Espero uma vitória"),
    ("no_pressure", "Não há pressão, divirtam-se"),
    ("show_worth", "Mostrem o que valem"),
    ("calm", "Joguem com calma e concentração"),
];
const HALF: [(&str, &str); 4] = [
    ("keep_going", "Continuem assim"),
    ("want_more", "Quero mais de vocês"),
    ("disappointed", "Estou desiludido"),
    ("no_pressure", "Não há pressão, divirtam-se"),
];
const FULL: [(&str, &str); 4] = [
    ("proud", "Estou orgulhoso"),
    ("well_done", "Bom trabalho"),
    ("not_happy", "Não estou contente"),
    ("disappointed", "Estou desiludido"),
];

const SUB: [(&str, &str); 3] =
    [("change_game", "Vai lá e muda o jogo"), ("stay_calm", "Entra com calma, faz o simples"), ("show_worth", "Mostra o que vales")];
const TONES: [(&str, &str); 4] = [("calm", "Calmo"), ("assertive", "Assertivo"), ("aggressive", "Agressivo"), ("motivating", "Motivador")];
/// Minutos depois de entrar em que ainda se fala ao suplente.
const SUB_MINUTES: u8 = 5;

/// O que conta de um jogador para a reação.
struct Mind {
    pressure: i32,
    big_games: i32,
    determination: i32,
    temperament: i32,
    professionalism: i32,
}

/// A situação do jogo para quem ouve.
#[derive(Clone, Copy)]
struct Situation {
    /// +1 favorito, 0 equilibrado, -1 sem favoritismo.
    favourite: i32,
    /// Golos a favor menos contra (ao intervalo e no fim).
    lead: i32,
}

fn label(v: i32, nervous: bool) -> &'static str {
    match v {
        2.. => "Motivado",
        1 => "Focado",
        0 => "Sem reação",
        -1 if nervous => "Nervoso",
        -1 => "Complacente",
        _ => "Irritado",
    }
}

/// A reação (−2 a +2) de um jogador a uma frase, e se é de nervos.
fn react(key: &str, moment: &str, m: &Mind, s: Situation) -> (i32, bool) {
    let calm_under_pressure = m.pressure + m.big_games >= 30;
    let nervy = m.pressure + m.big_games < 18;
    let driven = m.determination >= 14;
    let hothead = m.temperament <= 8;
    let pro = m.professionalism >= 13;
    match (moment, key) {
        ("pre", "expect_win") => match s.favourite {
            1 if nervy => (-1, true),
            1 if calm_under_pressure => (2, false),
            1 => (1, false),
            _ if nervy => (-1, true),
            _ if driven => (1, false),
            _ => (0, false),
        },
        ("pre", "no_pressure") | ("half", "no_pressure") => match (s.favourite, s.lead) {
            (_, l) if l < 0 && moment == "half" => (if pro { 0 } else { -1 }, false),
            (1, _) if !pro => (-1, false),
            (1, _) => (0, false),
            _ if nervy => (2, false),
            _ => (1, false),
        },
        ("pre", "show_worth") => match () {
            _ if driven => (2, false),
            _ if hothead => (-1, false),
            _ => (1, false),
        },
        ("pre", "calm") => (if nervy { 1 } else { 0 }, false),
        ("half", "keep_going") => match s.lead {
            1.. => (1, false),
            0 => (0, false),
            _ if pro => (0, false),
            _ => (-1, false),
        },
        ("half", "want_more") => match s.lead {
            ..=0 if driven => (2, false),
            ..=0 => (1, false),
            _ if pro => (1, false),
            _ => (0, false),
        },
        ("half", "disappointed") | ("full", "disappointed") => match s.lead {
            l if l > 0 => (if hothead { -2 } else { -1 }, false),
            _ if hothead => (-2, false),
            _ if driven => (2, false),
            _ => (1, false),
        },
        ("full", "proud") => match s.lead {
            1.. => (2, false),
            0 => (1, false),
            _ if pro => (0, false),
            _ => (-1, false),
        },
        ("full", "well_done") => match s.lead {
            1.. => (1, false),
            0 if s.favourite < 0 => (1, false),
            0 => (0, false),
            _ => (-1, false),
        },
        ("full", "not_happy") => match s.lead {
            1.. => (if hothead { -2 } else { -1 }, false),
            _ if driven => (1, false),
            _ if hothead => (-2, false),
            _ => (0, false),
        },
        ("sub", "change_game") => match () {
            _ if nervy => (-1, true),
            _ if driven || calm_under_pressure => (2, false),
            _ => (1, false),
        },
        ("sub", "stay_calm") => (if nervy { 2 } else { 0 }, false),
        ("sub", "show_worth") => match () {
            _ if driven => (2, false),
            _ if hothead => (-1, false),
            _ => (1, false),
        },
        _ => (0, false),
    }
}

/// O tom acerta a reação: calmo tira os nervos, agressivo puxa por quem tem garra e irrita
/// quem tem pavio curto, motivador acorda quem não reagia; assertivo fica como está.
fn tone(v: i32, nervous: bool, tone: &str, m: &Mind) -> (i32, bool) {
    let v = match tone {
        "calm" if nervous => v + 1,
        "calm" if v >= 2 => 1,
        "aggressive" if m.temperament <= 8 => v - 1,
        "aggressive" if m.determination >= 14 && v > 0 => v + 1,
        "aggressive" if v < 0 => v - 1,
        "motivating" if v == 0 && m.determination < 12 => 1,
        _ => v,
    };
    (v.clamp(-2, 2), nervous && v < 0)
}

impl Game {
    /// O momento da palestra agora: antes do apito, ao intervalo, no fim ou depois de uma
    /// substituição (a palavra ao suplente).
    fn talk_moment(&self) -> Result<&'static str> {
        let day = self.live.as_ref().ok_or_else(|| Error::Invalid("Não há nenhum jogo a decorrer.".into()))?;
        let live = &day.games.get(day.user).ok_or_else(|| Error::Invalid("Não há nenhum jogo a decorrer.".into()))?.1;
        Ok(if live.finished() {
            "full"
        } else if !live.started() {
            "pre"
        } else if live.minute() == 45 {
            "half"
        } else if !self.waiting_subs()?.is_empty() {
            "sub"
        } else {
            return Err(Error::Invalid("Só se fala à equipa antes do jogo, ao intervalo, no fim ou a quem acabou de entrar.".into()));
        })
    }

    /// Os suplentes que entraram há pouco e ainda não ouviram nada.
    fn waiting_subs(&self) -> Result<Vec<(i32, String)>> {
        let day = self.live.as_ref().ok_or_else(|| Error::Invalid("Não há nenhum jogo a decorrer.".into()))?;
        let live = &day.games[day.user].1;
        Ok(live.recent_subs(SUB_MINUTES).into_iter().filter(|(id, _)| !day.talks.contains(&format!("sub:{id}"))).collect())
    }

    /// A situação para o clube do treinador: favoritismo e resultado.
    fn talk_situation(&self) -> Result<Situation> {
        let day = self.live.as_ref().ok_or_else(|| Error::Invalid("Não há nenhum jogo a decorrer.".into()))?;
        let live = &day.games[day.user].1;
        let side = live.user_side().unwrap_or(0);
        let (h, a) = live.score();
        let lead = if side == 0 { h as i32 - a as i32 } else { a as i32 - h as i32 };
        let (home, away) = (live.fixture.home.id, live.fixture.away.id);
        let (ours, theirs) = if side == 0 { (home, away) } else { (away, home) };
        let diff = self.club_rep(ours)? - self.club_rep(theirs)? + if side == 0 { HOME_REP } else { -HOME_REP };
        let favourite = if diff > FAVOURITE_REP {
            1
        } else if diff < -FAVOURITE_REP {
            -1
        } else {
            0
        };
        Ok(Situation { favourite, lead })
    }

    /// O painel "Palestra" agora.
    pub fn talk_view(&self) -> Result<TalkView> {
        let moment = self.talk_moment()?;
        let s = self.talk_situation()?;
        let (title, options): (&str, &[(&str, &str)]) = match moment {
            "pre" => ("Palestra antes do jogo", &PRE),
            "half" => ("Palestra ao intervalo", &HALF),
            "sub" => ("Uma palavra a quem entra", &SUB),
            _ => ("Palestra no fim do jogo", &FULL),
        };
        // O adjunto sugere uma frase, pela situação
        let suggested = match (moment, s.favourite, s.lead) {
            ("sub", _, l) if l < 0 => "change_game",
            ("sub", _, _) => "stay_calm",
            ("pre", 1, _) => "expect_win",
            ("pre", -1, _) => "no_pressure",
            ("pre", _, _) => "show_worth",
            ("half", _, l) if l > 0 => "keep_going",
            ("half", _, _) => "want_more",
            (_, _, l) if l > 0 => "proud",
            (_, _, 0) => "well_done",
            _ => "not_happy",
        };
        let players = if moment == "sub" {
            self.waiting_subs()?
        } else {
            let day = self.live.as_ref().expect("dia de jogo");
            let done = |id: i32| day.talks.contains(&format!("{moment}:{id}"));
            day.games[day.user].1.talk_players().into_iter().filter(|(id, _)| !done(*id)).collect()
        };
        let done = self.live.as_ref().is_some_and(|d| d.talks.iter().any(|t| t == moment));
        let opt = |(k, l): &(&str, &str)| TalkOption { key: (*k).into(), label: (*l).into() };
        Ok(TalkView {
            moment: moment.into(),
            title: title.into(),
            options: options.iter().map(opt).collect(),
            tones: TONES.iter().map(opt).collect(),
            suggested: suggested.into(),
            players: players.into_iter().map(|(id, name)| TalkOption { key: id.to_string(), label: name }).collect(),
            team: moment != "sub",
            done,
        })
    }

    /// Dá a palestra à equipa (`person` = `None`) ou a um jogador: a reação de cada um, a
    /// moral e (antes do jogo, ao intervalo e ao suplente) a motivação nesta partida.
    pub fn give_talk(&mut self, key: &str, tone_key: &str, person: Option<i32>) -> Result<Vec<TalkReaction>> {
        let moment = self.talk_moment()?;
        let options: &[(&str, &str)] = match moment {
            "pre" => &PRE,
            "half" => &HALF,
            "sub" => &SUB,
            _ => &FULL,
        };
        if !options.iter().any(|(k, _)| *k == key) {
            return Err(Error::Invalid("Frase desconhecida.".into()));
        }
        if !TONES.iter().any(|(k, _)| *k == tone_key) {
            return Err(Error::Invalid("Tom desconhecido.".into()));
        }
        let mark = match person {
            Some(id) => format!("{moment}:{id}"),
            None if moment == "sub" => return Err(Error::Invalid("Escolha o jogador que entrou.".into())),
            None => moment.to_string(),
        };
        if self.live.as_ref().is_some_and(|d| d.talks.contains(&mark)) {
            return Err(Error::Invalid(if person.is_some() { "Já falou com ele." } else { "Já falou à equipa neste momento." }.into()));
        }
        let s = self.talk_situation()?;
        let everyone = if moment == "sub" { self.waiting_subs()? } else { self.live_match_mut()?.talk_players() };
        let listeners: Vec<(i32, String)> = everyone.into_iter().filter(|(id, _)| person.is_none_or(|p| p == *id)).collect();
        if listeners.is_empty() {
            return Err(Error::Invalid("Esse jogador não está no jogo.".into()));
        }
        let mut out = Vec::new();
        for (id, name) in listeners {
            let m = self.world.conn.query_row(
                "SELECT COALESCE(p.pressure, 10), COALESCE(pl.important_matches, 10), COALESCE(p.determination, 10),
                        COALESCE(p.temperament, 10), COALESCE(p.professionalism, 10)
                 FROM people p LEFT JOIN players pl ON pl.person_id = p.id WHERE p.id = ?",
                [id],
                |r| {
                    Ok(Mind {
                        pressure: r.get(0)?,
                        big_games: r.get(1)?,
                        determination: r.get(2)?,
                        temperament: r.get(3)?,
                        professionalism: r.get(4)?,
                    })
                },
            )?;
            let (v, nervous) = react(key, moment, &m, s);
            let (v, nervous) = tone(v, nervous, tone_key, &m);
            if moment == "full" {
                self.change_morale(id, v);
            } else {
                self.change_morale(id, v / 2);
                self.live_match_mut()?.motivate(id, 1.0 + MOTIVATION * v as f64);
            }
            out.push(TalkReaction { person_id: id, name, reaction: label(v, nervous).into(), delta: v });
        }
        if let Some(day) = self.live.as_mut() {
            day.talks.push(mark);
        }
        self.unsaved = true;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn talks_before_the_game_and_at_half_time() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-palestra-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Palestra".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
        };
        let mut g = Game::create(&path, &dir, &new, 29).unwrap();
        g.set_watch(true);
        while g.live.is_none() {
            g.advance_to_next_event().unwrap();
        }
        let view = g.talk_view().unwrap();
        assert_eq!((view.moment.as_str(), view.done), ("pre", false));
        let reactions = g.give_talk(&view.suggested, "assertive", None).unwrap();
        assert!(reactions.len() >= 11);
        assert!(g.give_talk(&view.suggested, "calm", None).is_err(), "uma palestra por momento");
        // A um jogador sozinho, depois da equipa
        let one: i32 = view.players[0].key.parse().unwrap();
        assert_eq!(g.give_talk(&view.suggested, "motivating", Some(one)).unwrap().len(), 1);
        assert!(g.give_talk(&view.suggested, "motivating", Some(one)).is_err(), "uma vez a cada jogador");
        assert!(g.talk_view().unwrap().done);
        g.live_step(45).unwrap();
        assert_eq!(g.talk_view().unwrap().moment, "half");
        g.live_step(60).unwrap();
        assert!(g.talk_view().map_or(true, |v| v.moment == "sub"), "a meio da segunda parte só ao suplente que entrou");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reactions_follow_personality_and_situation() {
        let nervy = Mind { pressure: 6, big_games: 7, determination: 10, temperament: 12, professionalism: 12 };
        let leader = Mind { pressure: 16, big_games: 16, determination: 16, temperament: 12, professionalism: 15 };
        let fav = Situation { favourite: 1, lead: 0 };
        assert_eq!(react("expect_win", "pre", &nervy, fav), (-1, true), "o nervoso fica nervoso");
        assert_eq!(react("expect_win", "pre", &leader, fav).0, 2);
        let losing = Situation { favourite: 0, lead: -1 };
        assert_eq!(react("want_more", "half", &leader, losing).0, 2);
        let winning = Situation { favourite: 0, lead: 2 };
        assert!(react("disappointed", "full", &leader, winning).0 < 0, "criticar quem ganhou desmotiva");
        assert_eq!(react("proud", "full", &nervy, winning).0, 2);
        // O tom: calmo tira os nervos, agressivo irrita o pavio curto
        assert_eq!(tone(-1, true, "calm", &nervy), (0, false));
        let hot = Mind { pressure: 10, big_games: 10, determination: 10, temperament: 6, professionalism: 10 };
        assert_eq!(tone(1, false, "aggressive", &hot).0, 0);
    }
}
