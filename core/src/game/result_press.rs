//! A imprensa dos resultados expressivos do clube do treinador: uma goleada nas continentais
//! (no jogo ou no agregado do mata-mata), uma goleada sobre um time da metade de cima da liga,
//! o líder folgado que vira favorito ao título e a goleada sofrida.

use super::{Game, NewsItem};
use crate::league::{CHAMPIONS_LEAGUE, EUROPA_LEAGUE, LIBERTADORES, Match, SUL_AMERICANA};
use crate::{Result, date};

/// As competições continentais de clubes.
const CONTINENTAL: [i32; 4] = [CHAMPIONS_LEAGUE, EUROPA_LEAGUE, LIBERTADORES, SUL_AMERICANA];
/// A diferença de gols que faz de um jogo uma goleada (e a do agregado de um mata-mata).
const ROUT: i32 = 4;
const ROUT_AGGREGATE: i32 = 5;
/// Líder com esta vantagem, passado um terço da liga, vira favorito ao título.
const FAVOURITE_LEAD: i32 = 6;
/// Dias até a imprensa voltar a chamá-lo favorito.
const FAVOURITE_AGAIN: i32 = 30;

impl Game {
    /// A notícia da imprensa sobre o jogo `m` do treinador, se o resultado der que falar.
    pub(super) fn result_press(&self, id: i32, m: &Match) -> Result<Option<NewsItem>> {
        let Some((hg, ag)) = m.result else { return Ok(None) };
        let at_home = m.home_id == self.club_id;
        let (gf, ga) = if at_home { (hg as i32, ag as i32) } else { (ag as i32, hg as i32) };
        let us = self.club(self.club_id).short_name;
        let them = self.club(if at_home { m.away_id } else { m.home_id }).short_name;
        let comp = self.fixture(m).competition;
        let press = |title: String, body: String| Some(self.news_item(id, "press", title, body));

        // Nas continentais: a goleada no jogo ou no agregado do mata-mata
        if CONTINENTAL.contains(&m.competition_id) {
            let aggregate = (m.leg == 2)
                .then(|| {
                    self.matches.iter().find(|o| {
                        o.competition_id == m.competition_id && o.stage == m.stage && o.round == m.round && o.tie == m.tie && o.leg == 1
                    })
                })
                .flatten()
                .and_then(|first| {
                    first.result.map(|(h, a)| if first.home_id == self.club_id { (h as i32, a as i32) } else { (a as i32, h as i32) })
                })
                .map(|(f, a)| (gf + f, ga + a));
            let nation = self.club_nation_name(self.club_id)?;
            if let Some((f, a)) = aggregate.filter(|(f, a)| f - a >= ROUT_AGGREGATE) {
                return Ok(press(
                    format!("{us} dá show na {comp}"),
                    format!(
                        "Com o {gf}-{ga} de hoje, o {us} despachou o {them} por {f}-{a} no agregado. A imprensa de {nation} \
                         celebra a campanha, e o {us} já aparece entre os candidatos ao título continental."
                    ),
                ));
            }
            if gf - ga >= ROUT {
                return Ok(press(
                    format!("{us} atropela o {them} na {comp}"),
                    format!(
                        "O {gf}-{ga} sobre o {them} repercute em toda a imprensa de {nation}: uma das maiores goleadas \
                         da temporada na {comp}, e o {us} passa a ser visto como candidato ao título."
                    ),
                ));
            }
        }

        // A goleada sofrida, em qualquer competição
        if ga - gf >= ROUT {
            return Ok(press(
                format!("Vexame: {us} leva {ga} do {them}"),
                format!(
                    "A imprensa não perdoa o {gf}-{ga} diante do {them} pela {comp}. Os torcedores cobram explicações \
                     e o desempenho do time será analisado nos próximos jogos."
                ),
            ));
        }

        // Na liga (pontos corridos): a goleada sobre um time de cima e o líder folgado
        if m.tie.is_some() || m.group.is_some() || gf <= ga {
            return Ok(None);
        }
        let table = self.table_for(m);
        let Some(ours) = table.iter().find(|r| r.club.id == self.club_id) else { return Ok(None) };
        let other = if at_home { m.away_id } else { m.home_id };
        let theirs = table.iter().find(|r| r.club.id == other);
        if gf - ga >= ROUT && theirs.is_some_and(|r| (r.position as usize) <= table.len() / 2) {
            return Ok(press(
                format!("Goleada do {us} sobre o {them} impressiona"),
                format!(
                    "O {gf}-{ga} sobre o {them}, um dos times da parte de cima da tabela, é o assunto da rodada na {comp}. \
                     O {us} chega a {} pontos e é {}º colocado.",
                    ours.points, ours.position
                ),
            ));
        }
        let rounds = (table.len().saturating_sub(1) * 2) as i32;
        let lead = table.get(1).map_or(0, |second| ours.points - second.points);
        let title = format!("{us} surge como favorito ao título");
        let recent = self
            .news
            .iter()
            .any(|n| n.title == title && date::parse(&n.date).zip(date::parse(&self.date)).is_some_and(|(a, b)| b - a < FAVOURITE_AGAIN));
        if ours.position == 1 && lead >= FAVOURITE_LEAD && ours.played * 3 >= rounds && !recent {
            return Ok(press(
                title,
                format!(
                    "Depois da vitória sobre o {them}, o {us} abre {lead} pontos de vantagem na liderança da {comp}. \
                     Para a imprensa, o título passa a ser uma questão de tempo."
                ),
            ));
        }
        Ok(None)
    }

    /// O nome da nação de um clube ("Itália", "Brasil").
    fn club_nation_name(&self, club: i32) -> Result<String> {
        Ok(self.world.conn.query_row(
            "SELECT COALESCE(n.name, '') FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id WHERE c.id = ?",
            [club],
            |r| r.get(0),
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::league::SERIE_A;

    const PALMEIRAS: i32 = 7568;
    const FLAMENGO: i32 = 3675;

    #[test]
    fn big_results_reach_the_press() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-imprensa-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Imprensa".into(),
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
        let g = Game::create(&path, &dir, &new, 3).unwrap();
        let game = |comp: i32, result: (u8, u8)| Match {
            id: 999_999,
            competition_id: comp,
            stage: 0,
            round: 1,
            group: Some(0),
            tie: None,
            leg: 0,
            neutral: false,
            date: g.date.clone(),
            home_id: PALMEIRAS,
            away_id: FLAMENGO,
            result: Some(result),
            pens: None,
        };
        let title = |m: Match| g.result_press(1, &m).unwrap().map(|n| n.title);
        assert_eq!(title(game(LIBERTADORES, (5, 0))).as_deref(), Some("Palmeiras atropela o Flamengo na Copa Libertadores"));
        assert!(title(Match { group: None, ..game(SERIE_A, (0, 4)) }).is_some_and(|t| t.starts_with("Vexame")));
        assert_eq!(title(Match { group: None, ..game(SERIE_A, (1, 0)) }), None, "um 1-0 não é notícia");
        // A aba Prêmios: os prémios gravados da competição, com a temporada e o nome
        let star: i32 =
            g.world.conn.query_row("SELECT id FROM people WHERE club_id = ? AND is_player = 1 LIMIT 1", [PALMEIRAS], |r| r.get(0)).unwrap();
        g.world
            .conn
            .execute(
                "INSERT INTO awards (person_id, date, competition, kind, club, rank, value) VALUES (?, '2025-09-01', ?, 'player_month', ?, 1, 7.8)",
                rusqlite::params![star, SERIE_A, PALMEIRAS],
            )
            .unwrap();
        let awards = g.competition_awards(SERIE_A).unwrap();
        assert!(awards.iter().any(|a| a.person_id == star && a.monthly && a.label == "Jogador do Mês" && a.season == "2025"), "{awards:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
