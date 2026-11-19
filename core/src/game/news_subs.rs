//! Assinaturas das notícias do mundo, como nas preferências da caixa de entrada do FM: em
//! cada competição, os resultados de impacto, as lesões graves e as transferências grandes;
//! à parte, a Lista Preferencial; e, de cada tema, se a notícia pára o Continuar Jogo. Só se
//! grava o que o treinador mudou: o resto segue o padrão (o que o jogo já mostrava).

use std::collections::HashSet;

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::leagues::LeagueMode;
use super::{Game, internationals};
use crate::regulation;
use crate::{Error, Result};

/// Save 79: as assinaturas mudadas pelo treinador (`competition_id` 0 = a Lista Preferencial,
/// -1 = se o tema pára o Continuar Jogo).
pub(super) const NEWS_SUBS_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS news_subs (
    competition_id INTEGER NOT NULL, topic TEXT NOT NULL, enabled INTEGER NOT NULL,
    PRIMARY KEY (competition_id, topic));";

pub(super) const RESULTS: &str = "results";
pub(super) const INJURIES: &str = "injuries";
pub(super) const TRANSFERS: &str = "transfers";
pub(super) const SHORTLIST: &str = "shortlist";
const TOPICS: [&str; 4] = [RESULTS, INJURIES, TRANSFERS, SHORTLIST];
/// As linhas especiais da tabela.
const SHORTLIST_ROW: i32 = 0;
const STOP_ROW: i32 = -1;

/// Uma competição na grelha das assinaturas.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NewsSubRow {
    pub competition_id: i32,
    pub name: String,
    /// A nação da liga (as continentais não têm).
    pub nation: Option<String>,
    /// O clube do treinador joga nela nesta temporada.
    pub own: bool,
    pub results: bool,
    /// Só nas ligas escolhidas (as de fundo e as copas não têm lesões nem plantéis próprios).
    pub injuries: Option<bool>,
    pub transfers: Option<bool>,
}

/// A tela das assinaturas.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct NewsSubs {
    pub rows: Vec<NewsSubRow>,
    /// Lesões, suspensões e transferências dos jogadores da Lista Preferencial.
    pub shortlist: bool,
    /// Os temas que param o Continuar Jogo ("results", "injuries", "transfers", "shortlist").
    pub stop: Vec<String>,
}

impl Game {
    /// Uma liga escolhida (com plantéis, lesões e mercado).
    fn play_league(&self, comp: i32) -> bool {
        self.comp_nations.contains_key(&comp)
            && regulation::get(comp).is_some_and(|r| !r.is_cup())
            && self.comp_mode(comp) == LeagueMode::Play
    }

    /// As competições da grelha: as do treinador, as ligas escolhidas e as continentais.
    fn news_comps(&self, own: &HashSet<i32>) -> Vec<i32> {
        let mut out: Vec<i32> = own.iter().copied().collect();
        out.sort_unstable();
        for r in regulation::all() {
            let c = r.competition;
            let continental = !self.comp_nations.contains_key(&c) && !internationals::is_national_comp(c);
            if !out.contains(&c) && (self.play_league(c) || continental) {
                out.push(c);
            }
        }
        out
    }

    fn sub_saved(&self, comp: i32, topic: &str) -> Result<Option<bool>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT enabled FROM news_subs WHERE competition_id = ? AND topic = ?")?
            .query_row(params![comp, topic], |r| r.get::<_, i32>(0))
            .optional()?
            .map(|v| v != 0))
    }

    /// Uma assinatura de uma competição; o padrão: os resultados das competições do treinador,
    /// as lesões e as transferências das ligas escolhidas.
    fn sub_on(&self, comp: i32, topic: &str, own: &HashSet<i32>) -> Result<bool> {
        let default = match topic {
            RESULTS => own.contains(&comp),
            _ => self.play_league(comp),
        };
        Ok(self.sub_saved(comp, topic)?.unwrap_or(default))
    }

    /// Assina as lesões ou as transferências desta liga (o padrão não depende das competições
    /// do treinador).
    pub(super) fn league_sub(&self, comp: i32, topic: &str) -> Result<bool> {
        if !self.play_league(comp) {
            return Ok(false);
        }
        Ok(self.sub_saved(comp, topic)?.unwrap_or(true))
    }

    /// As competições que assinam um tema.
    pub(super) fn subscribed(&self, topic: &str) -> Result<HashSet<i32>> {
        let own = self.own_competitions();
        let mut out = HashSet::new();
        for c in self.news_comps(&own) {
            if self.sub_on(c, topic, &own)? {
                out.insert(c);
            }
        }
        Ok(out)
    }

    /// As notícias da Lista Preferencial estão ligadas.
    pub(super) fn shortlist_sub(&self) -> Result<bool> {
        Ok(self.sub_saved(SHORTLIST_ROW, SHORTLIST)?.unwrap_or(true))
    }

    /// As notícias deste tema param o Continuar Jogo (como no CM, por padrão todas).
    pub(super) fn news_stops(&self, topic: &str) -> Result<bool> {
        Ok(self.sub_saved(STOP_ROW, topic)?.unwrap_or(true))
    }

    pub fn news_subs(&self) -> Result<NewsSubs> {
        let own = self.own_competitions();
        let mut rows = Vec::new();
        for c in self.news_comps(&own) {
            let league = self.play_league(c);
            let nation: Option<String> = match self.comp_nations.get(&c) {
                Some(n) => self.world.conn.query_row("SELECT name FROM nations WHERE id = ?", [n], |r| r.get(0)).optional()?,
                None => None,
            };
            rows.push(NewsSubRow {
                competition_id: c,
                name: self.competition_name(c).to_string(),
                nation,
                own: own.contains(&c),
                results: self.sub_on(c, RESULTS, &own)?,
                injuries: if league { Some(self.sub_on(c, INJURIES, &own)?) } else { None },
                transfers: if league { Some(self.sub_on(c, TRANSFERS, &own)?) } else { None },
            });
        }
        // As do treinador primeiro; depois as continentais e as ligas por nação
        rows.sort_by(|a, b| b.own.cmp(&a.own).then(a.nation.cmp(&b.nation)));
        let mut stop = Vec::new();
        for t in TOPICS {
            if self.news_stops(t)? {
                stop.push(t.to_string());
            }
        }
        Ok(NewsSubs { rows, shortlist: self.shortlist_sub()?, stop })
    }

    /// Liga ou desliga uma assinatura: `competition_id` 0 = a Lista Preferencial, -1 = o tema
    /// pára o Continuar Jogo.
    pub fn set_news_sub(&mut self, competition_id: i32, topic: &str, on: bool) -> Result<()> {
        if !TOPICS.contains(&topic) || (competition_id == SHORTLIST_ROW && topic != SHORTLIST) {
            return Err(Error::Invalid("Assinatura desconhecida.".into()));
        }
        self.world.conn.execute(
            "INSERT OR REPLACE INTO news_subs (competition_id, topic, enabled) VALUES (?, ?, ?)",
            params![competition_id, topic, on as i32],
        )?;
        self.unsaved = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::World;
    use crate::game::leagues::LeagueChoice;

    const BRAZIL: i32 = 27;
    const ENGLAND: i32 = 61;
    const PREMIER: i32 = 7;

    #[test]
    fn news_subscriptions() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-assinaturas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let choice = |nation_id, mode| LeagueChoice { nation_id, mode };
        let new = crate::NewGame {
            save_name: "Assinaturas".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: vec![choice(BRAZIL, LeagueMode::Play), choice(ENGLAND, LeagueMode::Play)],
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 5).unwrap();
        let serie_a = crate::league::SERIE_A;
        // O padrão: resultados das competições do treinador, lesões e transferências das ligas escolhidas
        let subs = g.news_subs().unwrap();
        let row = |s: &NewsSubs, c: i32| s.rows.iter().find(|r| r.competition_id == c).cloned().unwrap();
        let a = row(&subs, serie_a);
        assert!(a.own && a.results && a.injuries == Some(true) && a.transfers == Some(true));
        let p = row(&subs, PREMIER);
        assert!(!p.own && !p.results && p.injuries == Some(true));
        assert!(subs.shortlist && subs.stop.len() == TOPICS.len());
        assert!(subs.rows.iter().all(|r| !internationals::is_national_comp(r.competition_id)));
        // Mudar: só fica o que o treinador mexeu
        g.set_news_sub(PREMIER, RESULTS, true).unwrap();
        g.set_news_sub(serie_a, INJURIES, false).unwrap();
        g.set_news_sub(0, SHORTLIST, false).unwrap();
        g.set_news_sub(-1, RESULTS, false).unwrap();
        assert!(g.subscribed(RESULTS).unwrap().contains(&PREMIER));
        assert!(!g.league_sub(serie_a, INJURIES).unwrap() && g.league_sub(PREMIER, INJURIES).unwrap());
        assert!(!g.shortlist_sub().unwrap() && !g.news_stops(RESULTS).unwrap() && g.news_stops(INJURIES).unwrap());
        assert!(g.set_news_sub(0, RESULTS, true).is_err() && g.set_news_sub(serie_a, "golos", true).is_err());
        // Ao carregar, as assinaturas continuam
        g.save().unwrap();
        let file = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&file).unwrap();
        let subs = g.news_subs().unwrap();
        assert!(row(&subs, PREMIER).results && row(&subs, serie_a).injuries == Some(false) && !subs.shortlist);
        assert!(!subs.stop.iter().any(|t| t == RESULTS));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
