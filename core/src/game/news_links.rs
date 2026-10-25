//! Ligações das notícias: os nomes de jogadores, staff e clubes que uma notícia cita, para a
//! tela de Notícias os mostrar como ligações (um clique abre o perfil ou o clube). Calculado
//! na hora, a partir do plantel do treinador, das propostas, das transferências do dia e dos
//! clubes das competições em jogo.

use serde::Serialize;
use ts_rs::TS;

use super::Game;
use crate::Result;

/// Um nome numa notícia: o texto tal como aparece e o que abre.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NewsLink {
    pub text: String,
    /// "person" ou "club"
    pub kind: String,
    pub id: i32,
}

impl Game {
    /// As ligações de uma notícia (as mais compridas primeiro, para "Real Madrid" ganhar a "Real").
    pub fn news_links(&self, news_id: i32) -> Result<Vec<NewsLink>> {
        let Some(n) = self.news.iter().find(|n| n.id == news_id) else { return Ok(Vec::new()) };
        let text = format!("{}\n{}", n.title, n.body);
        // Pessoas candidatas: o plantel e o staff do treinador, as das propostas e as das
        // transferências desse dia
        let people: Vec<(i32, String)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT id, display_name FROM people WHERE club_id = ?1
                 UNION SELECT p.id, p.display_name FROM people p JOIN offers o ON o.person_id = p.id
                 UNION SELECT p.id, p.display_name FROM people p JOIN transfers t ON t.person_id = p.id WHERE t.date = ?2",
            )?
            .query_map(rusqlite::params![self.club_id, n.date], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut links: Vec<NewsLink> = Vec::new();
        for (id, name) in people {
            if name.chars().count() >= 3 && text.contains(&name) {
                links.push(NewsLink { text: name, kind: "person".into(), id });
            }
        }
        for c in self.clubs.values() {
            for name in [&c.name, &c.short_name] {
                if name.chars().count() >= 3 && text.contains(name.as_str()) && !links.iter().any(|l| l.text == *name) {
                    links.push(NewsLink { text: name.clone(), kind: "club".into(), id: c.id });
                }
            }
        }
        links.sort_by(|a, b| b.text.chars().count().cmp(&a.text.chars().count()).then(a.text.cmp(&b.text)));
        Ok(links)
    }
}
