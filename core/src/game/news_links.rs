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
    /// "person", "club" ou "competition"
    pub kind: String,
    pub id: i32,
}

/// O quadro ao lado do texto da notícia (como no CM): a pessoa ou o clube de que ela fala.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NewsCard {
    /// "player", "staff" ou "club"
    pub kind: String,
    pub id: i32,
    pub name: String,
    /// O clube da pessoa (ou o próprio clube): as cores da faixa.
    pub club: Option<crate::ClubRef>,
    pub nation: Option<String>,
    pub age: Option<i32>,
    /// As siglas da posição ("MA/A DEC") ou a função do staff.
    pub role: Option<String>,
    #[ts(type = "number | null")]
    pub value: Option<i64>,
    /// Jogos, gols e nota média na temporada (jogador).
    pub apps: Option<i32>,
    pub goals: Option<i32>,
    pub rating: Option<f64>,
    /// Clube: a divisão, o lugar na classificação, gols pró e contra e a média de público.
    pub division: Option<String>,
    pub position: Option<i32>,
    pub goals_for: Option<i32>,
    pub goals_against: Option<i32>,
    pub attendance: Option<i32>,
}

impl NewsCard {
    pub(crate) fn empty(kind: &str, id: i32, name: String) -> NewsCard {
        NewsCard {
            kind: kind.into(),
            id,
            name,
            club: None,
            nation: None,
            age: None,
            role: None,
            value: None,
            apps: None,
            goals: None,
            rating: None,
            division: None,
            position: None,
            goals_for: None,
            goals_against: None,
            attendance: None,
        }
    }
}

impl Game {
    /// De quem fala a notícia: a primeira pessoa ou clube citado no título (senão no texto).
    pub fn news_subject(&self, news_id: i32) -> Result<Option<NewsLink>> {
        let Some(n) = self.news.iter().find(|n| n.id == news_id) else { return Ok(None) };
        let links: Vec<NewsLink> =
            self.news_links(news_id)?.into_iter().filter(|l| l.kind != "competition" && l.id != self.manager_id).collect();
        for text in [&n.title, &n.body] {
            // Num empate de posição ganha o nome mais comprido (vêm ordenados assim)
            let first = links.iter().filter_map(|l| text.find(&l.text).map(|at| (at, l))).min_by_key(|&(at, _)| at);
            if let Some((_, l)) = first {
                return Ok(Some(l.clone()));
            }
        }
        Ok(None)
    }

    /// A linha do clube no quadro: divisão, lugar e gols na liga em curso.
    pub fn club_card(&self, id: i32) -> Result<NewsCard> {
        let club = self.club(id);
        let mut card = NewsCard::empty("club", id, club.name.clone());
        let (division, nation, attendance): (Option<i32>, Option<String>, Option<i32>) = self.world.conn.query_row(
            "SELECT c.division_id, n.name, c.avg_attendance FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id WHERE c.id = ?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        card.nation = nation;
        card.attendance = attendance.filter(|&a| a > 0);
        if let Some(d) = division {
            card.division = Some(self.competition_name(d).to_string());
            if let Some(row) = self.table(d).into_iter().find(|r| r.club.id == id) {
                card.position = Some(row.position);
                card.goals_for = Some(row.goals_for);
                card.goals_against = Some(row.goals_against);
            }
        }
        card.club = Some(club);
        Ok(card)
    }

    /// As ligações de uma notícia (as mais compridas primeiro, para "Real Madrid" ganhar a "Real").
    pub fn news_links(&self, news_id: i32) -> Result<Vec<NewsLink>> {
        let Some(n) = self.news.iter().find(|n| n.id == news_id) else { return Ok(Vec::new()) };
        let names = candidates(&format!("{}\n{}", n.title, n.body));
        let conn = &self.world.conn;
        // As pessoas perto do treinador: o plantel e o staff, as das propostas, as das
        // transferências desse dia e as das listas dos olheiros
        let near: HashMap<String, i32> = conn
            .prepare_cached(
                "SELECT id, display_name FROM people WHERE club_id = ?1
                 UNION SELECT p.id, p.display_name FROM people p JOIN offers o ON o.person_id = p.id
                 UNION SELECT p.id, p.display_name FROM people p JOIN transfers t ON t.person_id = p.id WHERE t.date = ?2
                 UNION SELECT p.id, p.display_name FROM people p JOIN scout_finds f ON f.person_id = p.id",
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
        // As competições pelo nome do regulamento ("Liga Europa") e pelo do banco
        let mut comps: HashMap<String, i32> = HashMap::new();
        for r in crate::regulation::all() {
            if let Some(name) = r.name.as_ref().filter(|n| names.contains(*n)) {
                comps.entry(name.clone()).or_insert(r.competition);
            }
        }
        let mut stmt = conn.prepare_cached("SELECT id, name FROM competitions")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i32>(0)?, r.get::<_, Option<String>>(1)?)))? {
            if let (id, Some(name)) = row?
                && names.contains(&name)
            {
                comps.entry(name).or_insert(id);
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
            } else if let Some(&id) = comps.get(&name) {
                Some(("competition", id))
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
