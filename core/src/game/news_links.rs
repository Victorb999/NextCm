//! Ligações das notícias: os nomes de jogadores, staff e clubes que uma notícia cita, para a
//! tela de Notícias os mostrar como ligações (um clique abre o perfil ou o clube). Calculado
//! na hora: cada sequência de palavras do texto que comece por maiúscula é procurada nos
//! nomes da base de dados. Num nome repetido ganha quem está perto do treinador (o plantel,
//! as propostas, as transferências do dia), depois o clube e, por fim, o jogador mais capaz.

use std::collections::{HashMap, HashSet};

use rusqlite::OptionalExtension;
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
        let names = candidates(&format!("{}\n{}", n.title, n.body));
        let conn = &self.world.conn;
        // As pessoas perto do treinador: o plantel e o staff, as das propostas e as das
        // transferências desse dia
        let near: HashMap<String, i32> = conn
            .prepare_cached(
                "SELECT id, display_name FROM people WHERE club_id = ?1
                 UNION SELECT p.id, p.display_name FROM people p JOIN offers o ON o.person_id = p.id
                 UNION SELECT p.id, p.display_name FROM people p JOIN transfers t ON t.person_id = p.id WHERE t.date = ?2",
            )?
            .query_map(rusqlite::params![self.club_id, n.date], |r| Ok((r.get::<_, String>(1)?, r.get(0)?)))?
            .collect::<rusqlite::Result<_>>()?;
        // Os clubes pelo nome e pelo nome curto (num nome repetido, o de mais reputação)
        let mut clubs: HashMap<String, (i32, i32)> = HashMap::new();
        let mut stmt = conn.prepare_cached("SELECT id, name, short_name, COALESCE(reputation, 0) FROM clubs")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, i32>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, i32>(3)?))
        })?;
        for row in rows {
            let (id, name, short, rep) = row?;
            for name in [name, short].into_iter().flatten() {
                if names.contains(&name) {
                    let e = clubs.entry(name).or_insert((id, rep));
                    if rep > e.1 {
                        *e = (id, rep);
                    }
                }
            }
        }
        // O resto: qualquer pessoa com esse nome (o índice é sem maiúsculas; o 2.º "=" exige-as)
        let mut anyone = conn.prepare_cached(
            "SELECT id FROM people WHERE display_name = ?1 COLLATE NOCASE AND display_name = ?1
             ORDER BY ability DESC LIMIT 1",
        )?;
        let mut links: Vec<NewsLink> = Vec::new();
        for name in names {
            let link = if let Some(&id) = near.get(&name) {
                Some(("person", id))
            } else if let Some(&(id, _)) = clubs.get(&name) {
                Some(("club", id))
            } else {
                anyone.query_row([&name], |r| r.get(0)).optional()?.map(|id| ("person", id))
            };
            if let Some((kind, id)) = link {
                links.push(NewsLink { text: name, kind: kind.into(), id });
            }
        }
        links.sort_by(|a, b| b.text.chars().count().cmp(&a.text.chars().count()).then(a.text.cmp(&b.text)));
        Ok(links)
    }
}

/// Os nomes que o texto pode citar: de uma a cinco palavras seguidas, a primeira com
/// maiúscula e com pelo menos 3 letras ao todo. Pontuação corta o nome; o ponto final
/// também, menos nas abreviaturas curtas ("Jr."). Os parênteses também, mas um curto no fim
/// fica no nome ("Central Córdoba (SdE)").
fn candidates(text: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for part in text.split(['\n', ',', ';', ':', '"', '!', '?', '«', '»']) {
        let words: Vec<&str> = part.split_whitespace().collect();
        for start in 0..words.len() {
            let first = words[start].trim_start_matches('(');
            if !first.chars().next().is_some_and(char::is_uppercase) {
                continue;
            }
            let mut name = String::new();
            for (k, &raw) in words.iter().skip(start).take(5).enumerate() {
                if k > 0 && raw.starts_with('(') {
                    if raw.ends_with(')') && raw.chars().count() <= 8 {
                        out.insert(format!("{name} {raw}"));
                    }
                    break;
                }
                let raw = if k == 0 { first } else { raw };
                let (word, dot) = match raw.strip_suffix('.') {
                    Some(w) if w.chars().count() > 2 => (w, true),
                    _ => (raw, false),
                };
                let (word, stop) = match word.strip_suffix(')') {
                    Some(w) => (w, true),
                    None => (word, dot),
                };
                if !name.is_empty() {
                    name.push(' ');
                }
                name.push_str(word);
                if name.chars().filter(|c| c.is_alphabetic()).count() >= 3 {
                    out.insert(name.clone());
                }
                if stop {
                    break;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::candidates;

    #[test]
    fn nomes_candidatos() {
        let c = candidates("O Real Madrid contratou Kevin De Bruyne. Acordo com o Benfica, diz Silva Jr.");
        for name in ["Real Madrid", "Real", "Kevin De Bruyne", "De Bruyne", "Benfica", "Silva Jr.", "Acordo com o Benfica"] {
            assert!(c.contains(name), "{name}");
        }
        // Não passa do ponto final nem começa em minúscula
        assert!(!c.contains("Kevin De Bruyne Acordo"));
        assert!(!c.contains("contratou Kevin"));
        // Parênteses: o clube entre eles sem os parênteses, e o "(SdE)" curto no fim do nome
        let c = candidates("Wallace Yan (Bragantino) emprestado ao Central Córdoba (SdE)\nIván Román (Atlético Mineiro) sai");
        for name in ["Wallace Yan", "Bragantino", "Central Córdoba (SdE)", "Atlético Mineiro"] {
            assert!(c.contains(name), "{name}");
        }
        assert!(!c.contains("Wallace Yan (Bragantino)") && !c.contains("Atlético Mineiro) sai"));
    }
}
