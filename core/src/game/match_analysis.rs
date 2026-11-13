//! A Análise do Pré-Jogo (marco 3.21), como o "Match Analysis" do FM08: as odds pelos golos
//! esperados do motor, o homem-chave de cada lado, a previsão de um jornal e os marcos dos
//! jogadores. Tudo só para mostrar: sem RNG e sem mexer na partida.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::{Game, MatchPreview};
use crate::Result;

/// O jogador que o jornal aponta como decisivo.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct KeyMan {
    pub id: i32,
    pub name: String,
    pub role: Option<String>,
}

/// Um marco de um jogador neste jogo ("Pode fazer o 100.º jogo pelo clube").
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct Milestone {
    pub id: i32,
    pub name: String,
    pub text: String,
}

/// A Análise do Pré-Jogo.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MatchAnalysis {
    /// Odds decimais: vitória da casa, empate, vitória de fora.
    pub odds: [f64; 3],
    /// 0 a casa, 1 os de fora; nenhum num jogo equilibrado.
    pub favourite: Option<u8>,
    pub key_men: [Option<KeyMan>; 2],
    /// O jornal e a frase dele.
    pub paper: String,
    pub prediction: String,
    pub milestones: [Vec<Milestone>; 2],
    /// O que o Próximo Jogo já mostrava: último confronto, tempo, relvado e ausências.
    pub preview: MatchPreview,
}

/// O Pós-Jogo: o jornal a comparar a previsão com o resultado, os homens-chave e os marcos
/// que se cumpriram.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MatchReview {
    pub paper: String,
    pub analysis: String,
    /// O que o jornal diz do homem-chave de cada lado.
    pub key_men: [Option<String>; 2],
    pub milestones: [Vec<Milestone>; 2],
}

/// A margem das casas de apostas nas odds.
const MARGIN: f64 = 1.08;
/// Marcos por lado, no máximo.
const MILESTONES: usize = 5;

/// P(k golos) de uma Poisson com média `lambda`.
fn poisson_pmf(lambda: f64, k: u32) -> f64 {
    let mut p = (-lambda).exp();
    for i in 1..=k {
        p *= lambda / i as f64;
    }
    p
}

/// Probabilidades de vitória da casa, empate e vitória de fora pelos golos esperados.
pub(super) fn outcome_odds(home: f64, away: f64) -> [f64; 3] {
    let (mut h, mut d, mut a) = (0.0, 0.0, 0.0);
    for i in 0..=10u32 {
        for j in 0..=10u32 {
            let p = poisson_pmf(home, i) * poisson_pmf(away, j);
            match i.cmp(&j) {
                std::cmp::Ordering::Greater => h += p,
                std::cmp::Ordering::Equal => d += p,
                std::cmp::Ordering::Less => a += p,
            }
        }
    }
    let total = h + d + a;
    [h / total, d / total, a / total]
}

/// O jornal desportivo da nação (as das ligas do jogo).
pub(super) fn paper_of(nation: &str) -> &'static str {
    match nation {
        "Brazil" => "Lance!",
        "Portugal" => "A Bola",
        "Spain" => "Marca",
        "Italy" => "La Gazzetta dello Sport",
        "Germany" => "Kicker",
        "France" => "L'Équipe",
        "England" => "The Times",
        "Netherlands" => "De Telegraaf",
        "Argentina" => "Olé",
        "Saudi Arabia" => "Al Riyadiyah",
        _ => "O jornal desportivo",
    }
}

/// A frase do jornal pelas probabilidades.
fn prediction_text(p: [f64; 3], home: &str, away: &str) -> String {
    let [h, _, a] = p;
    if h >= 0.6 {
        format!("O {home} é claro favorito e deve somar os três pontos frente ao {away}.")
    } else if a >= 0.55 {
        format!("O {away} chega como favorito e tem tudo para levar os três pontos.")
    } else if (h - a).abs() < 0.08 {
        "Um jogo muito equilibrado: um empate não seria surpresa nenhuma.".into()
    } else if h > a {
        format!("Ligeira vantagem para o {home}, mas o {away} pode complicar.")
    } else {
        format!("O {away} parte com alguma vantagem, mas o {home} tem argumentos.")
    }
}

/// A frase do Pós-Jogo: o que se previa (pelas probabilidades) contra o resultado.
fn review_text(p: [f64; 3], h: i32, a: i32, home: &str, away: &str) -> String {
    let favourite = if p[0] - p[2] > 0.15 {
        Some((home, away, h - a))
    } else if p[2] - p[0] > 0.15 {
        Some((away, home, a - h))
    } else {
        None
    };
    match favourite {
        Some((fav, other, diff)) if diff >= 3 => {
            format!("Previa a vitória do {fav}, mas não esperava {diff} golos de diferença. Uma noite difícil para o {other}.")
        }
        Some((fav, other, diff)) if diff > 0 => format!("Como se previa, o {fav} ganhou ao {other}, sem grandes surpresas."),
        Some((fav, other, 0)) => format!("Esperava a vitória do {fav}, mas o {other} conseguiu segurar o empate."),
        Some((fav, other, _)) => format!("Ninguém esperava: o {other} ganhou ao {fav}, que era o favorito."),
        None if h == a => "Um jogo equilibrado, como se previa, e um empate justo.".into(),
        None => {
            let (w, l) = if h > a { (home, away) } else { (away, home) };
            format!("Num jogo que se previa equilibrado, o {w} foi melhor e ganhou ao {l}.")
        }
    }
}

impl Game {
    /// A Análise do jogo do treinador no dia de jogo (antes ou durante a partida).
    pub fn match_analysis(&self) -> Result<MatchAnalysis> {
        let day = self.live_day()?;
        let (_, live) = &day.games[day.user];
        let view = self.live_view()?;
        let (xh, xa) = live.expected_now();
        let p = outcome_odds(xh, xa);
        let odds = p.map(|q| ((1.0 / (q * MARGIN)).max(1.01) * 100.0).round() / 100.0);
        let favourite = if p[0] - p[2] > 0.15 {
            Some(0)
        } else if p[2] - p[0] > 0.15 {
            Some(1)
        } else {
            None
        };
        let fixture = &live.fixture;
        let national = super::internationals::is_national_comp(fixture.competition_id);
        let teams = [&view.home, &view.away];
        let mut key_men: [Option<KeyMan>; 2] = [None, None];
        let mut milestones: [Vec<Milestone>; 2] = [Vec::new(), Vec::new()];
        for side in 0..2 {
            let team = teams[side];
            key_men[side] = self.key_man(team)?.map(|pl| KeyMan {
                id: pl.id,
                name: self.person_name(pl.id).unwrap_or_default(),
                role: pl.role.clone(),
            });
            if !national {
                milestones[side] = self.milestones(team.club.id, team.players.iter().chain(&team.bench).map(|p| p.id))?;
            }
        }
        Ok(MatchAnalysis {
            odds,
            favourite,
            key_men,
            paper: self.paper(fixture.home.id)?,
            prediction: prediction_text(p, &fixture.home.short_name, &fixture.away.short_name),
            milestones,
            preview: self.match_preview(fixture.id)?,
        })
    }

    /// O jornal do clube da casa.
    fn paper(&self, home: i32) -> Result<String> {
        let nation: String = self
            .world
            .conn
            .query_row("SELECT COALESCE(n.name, '') FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id WHERE c.id = ?", [home], |r| {
                r.get(0)
            })
            .optional()?
            .unwrap_or_default();
        Ok(paper_of(&nation).to_string())
    }

    /// O homem-chave: o titular de mais reputação (quem entrou do banco não conta).
    fn key_man<'a>(&self, team: &'a crate::LiveTeam) -> Result<Option<&'a crate::LivePlayer>> {
        let mut best: Option<(i32, &crate::LivePlayer)> = None;
        for pl in team.players.iter().chain(&team.out).filter(|p| p.on_minute.is_none()) {
            let rep: i32 = self
                .world
                .conn
                .prepare_cached("SELECT COALESCE(current_reputation, 0) FROM players WHERE person_id = ?")?
                .query_row([pl.id], |r| r.get(0))
                .optional()?
                .unwrap_or(0);
            if best.is_none_or(|(r, _)| rep > r) {
                best = Some((rep, pl));
            }
        }
        Ok(best.map(|(_, p)| p))
    }

    /// Chegou ao clube durante o jogo (o banco não tem a época em curso: quem já lá estava
    /// ao começar não se estreia).
    fn joined_in_game(&self, id: i32) -> Result<bool> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT COALESCE(club_joined, '') > (SELECT created FROM game) FROM people WHERE id = ?")?
            .query_row([id], |r| r.get(0))
            .optional()?
            .unwrap_or(false))
    }

    /// Jogos e golos de um jogador por um clube: as épocas do banco e os jogos gravados.
    fn club_totals(&self, id: i32, club: i32) -> Result<(i64, i64)> {
        let (apps_before, goals_before): (i64, i64) = self.world.conn.query_row(
            "SELECT COALESCE(SUM(apps), 0), COALESCE(SUM(goals), 0) FROM career_history WHERE person_id = ? AND club_id = ?",
            params![id, club],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let (apps_here, goals_here): (i64, i64) = self.world.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(goals), 0) FROM player_matches WHERE person_id = ? AND club = ?",
            params![id, club],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((apps_before + apps_here, goals_before + goals_here))
    }

    /// O Pós-Jogo do jogo do treinador (com o resultado de agora; no fim, o final). Os jogos do
    /// dia só se gravam no Continuar Jogo, então este ainda não conta nos totais.
    pub fn match_review(&self) -> Result<MatchReview> {
        let day = self.live_day()?;
        let (_, live) = &day.games[day.user];
        let view = self.live_view()?;
        let (xh, xa) = live.expected_now();
        let p = outcome_odds(xh, xa);
        let (h, a) = live.score();
        let (h, a) = (h as i32, a as i32);
        let fixture = &live.fixture;
        let (home, away) = (fixture.home.short_name.clone(), fixture.away.short_name.clone());
        let score = format!("{h}-{a}");
        let analysis = review_text(p, h, a, &home, &away);
        let national = super::internationals::is_national_comp(fixture.competition_id);
        let teams = [&view.home, &view.away];
        let mut key_men: [Option<String>; 2] = [None, None];
        let mut milestones: [Vec<Milestone>; 2] = [Vec::new(), Vec::new()];
        for side in 0..2 {
            let team = teams[side];
            let (ours, theirs) = if side == 0 { (h, a) } else { (a, h) };
            let outcome = match ours.cmp(&theirs) {
                std::cmp::Ordering::Greater => "na vitória",
                std::cmp::Ordering::Equal => "no empate",
                std::cmp::Ordering::Less => "na derrota",
            };
            if let Some(k) = self.key_man(team)? {
                let name = self.person_name(k.id)?;
                let rating = k.rating.unwrap_or(6);
                let club = &team.club.short_name;
                let mut text = match rating {
                    8.. => format!("{name} fez um grande jogo (nota {rating}) {outcome} por {score} do {club}."),
                    7 => format!("{name} esteve bem (nota {rating}) {outcome} por {score} do {club}."),
                    6 => format!("{name} fez um jogo normal (nota {rating}) {outcome} por {score} do {club}."),
                    _ => format!("{name} passou ao lado do jogo (nota {rating}) {outcome} por {score} do {club}."),
                };
                if k.goals > 0 {
                    text.push_str(&format!(" Marcou {}.", if k.goals == 1 { "um golo".to_string() } else { format!("{} golos", k.goals) }));
                }
                if k.assists > 0 {
                    let n = if k.assists == 1 { "uma assistência".to_string() } else { format!("{} assistências", k.assists) };
                    text.push_str(&format!(" Fez {n}."));
                }
                if k.red {
                    text.push_str(" Foi expulso.");
                } else if let Some(off) = k.off_minute {
                    text.push_str(&format!(" Saiu aos {off}'."));
                }
                key_men[side] = Some(text);
            }
            if !national {
                for pl in team.players.iter().chain(&team.out) {
                    let (apps, goals) = self.club_totals(pl.id, team.club.id)?;
                    let name = self.person_name(pl.id)?;
                    let next = apps + 1;
                    let now = goals + pl.goals as i64;
                    let text = if apps == 0 && self.joined_in_game(pl.id)? {
                        Some("Estreou-se pelo clube".to_string())
                    } else if next >= 50 && next % 50 == 0 {
                        Some(format!("Fez o {next}.º jogo pelo clube"))
                    } else if now / 25 > goals / 25 && now >= 25 {
                        Some(format!("Chegou aos {} golos pelo clube", now / 25 * 25))
                    } else {
                        None
                    };
                    if let Some(text) = text {
                        milestones[side].push(Milestone { id: pl.id, name, text });
                    }
                    if milestones[side].len() >= MILESTONES {
                        break;
                    }
                }
            }
        }
        Ok(MatchReview { paper: self.paper(fixture.home.id)?, analysis, key_men, milestones })
    }

    /// Os marcos de quem pode jogar por um clube: a estreia, os jogos redondos (50, 100...) e
    /// os golos perto de um número redondo, pelo banco (épocas antes do jogo) e pelos jogos gravados.
    fn milestones(&self, club: i32, ids: impl Iterator<Item = i32>) -> Result<Vec<Milestone>> {
        let mut out = Vec::new();
        for id in ids {
            let (apps, goals) = self.club_totals(id, club)?;
            let name = self.person_name(id)?;
            let next = apps + 1;
            let text = if apps == 0 && self.joined_in_game(id)? {
                Some("Pode estrear-se pelo clube".to_string())
            } else if next >= 50 && next % 50 == 0 {
                Some(format!("Pode fazer o {next}.º jogo pelo clube"))
            } else {
                let target = (goals / 25 + 1) * 25;
                (goals >= 20 && target - goals <= 2).then(|| {
                    let left = target - goals;
                    format!("Está a {left} {} dos {target} pelo clube", if left == 1 { "golo" } else { "golos" })
                })
            };
            if let Some(text) = text {
                out.push(Milestone { id, name, text });
            }
            if out.len() >= MILESTONES {
                break;
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odds_follow_the_expected_goals() {
        let even = outcome_odds(1.3, 1.3);
        assert!((even[0] - even[2]).abs() < 1e-9 && (even.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        let strong = outcome_odds(2.4, 0.7);
        assert!(strong[0] > 0.7 && strong[2] < 0.1);
        assert!(prediction_text(strong, "Palmeiras", "Remo").contains("favorito"));
        assert!(prediction_text(even, "A", "B").contains("equilibrado"));
        assert!(review_text(strong, 5, 0, "Palmeiras", "Remo").contains("5 golos"));
        assert!(review_text(strong, 0, 1, "Palmeiras", "Remo").contains("Ninguém esperava"));
        assert!(review_text(even, 1, 1, "A", "B").contains("equilibrado"));
    }
}
