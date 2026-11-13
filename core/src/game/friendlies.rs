//! Amigáveis de clubes (marco 4.0, item 8), como no CM 01/02: no Calendário do clube, o
//! "Marcar" tem Amistoso em Casa, Amistoso Fora e Excursão (o Torneio fica de fora). Escolhem-se
//! as datas (as livres: sem jogo a menos de 2 dias, nem jogo oficial a menos de 3) e depois o
//! clube: no amistoso, na página dele ("Convidar Clube"); na excursão, de uma lista do país
//! escolhido, um por data. Os outros clubes também convidam (uma mensagem com Aceitar/Recusar).
//!
//! Os amigáveis são jogos normais da competição 72 (a "Friendly" do banco): jogam-se ao vivo e
//! cansam, mas não mexem na direcção, nos adeptos, nos prémios nem na imprensa, e os jogos de
//! cada jogador ficam em `friendly_apps` ("Não Competitivos" no Historial, fora da Forma e das
//! estatísticas da época). Na excursão, `round` guarda a nação (0 nos outros).

use std::collections::HashMap;

use rand_core::RngCore;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Game, NewsItem, cm_date};
use crate::league::{Fixture, Match};
use crate::person::ClubRef;
use crate::{Error, Result, date};

pub(super) use crate::regulation::CLUB_FRIENDLY;

/// A competição em português, os jogos de cada jogador e os convites (save 60).
pub(super) const FRIENDLIES_SCHEMA: &str = "
UPDATE competitions SET name = 'Amigáveis', short_name = 'Amigáveis', abbreviation = 'AMG' WHERE id = 72;
CREATE TABLE friendly_apps AS SELECT * FROM player_matches WHERE 0;
CREATE INDEX friendly_apps_person ON friendly_apps (person_id, date);
CREATE INDEX friendly_apps_fixture ON friendly_apps (fixture);
CREATE TABLE friendly_invites (id INTEGER PRIMARY KEY, club INTEGER NOT NULL, date TEXT NOT NULL, at_home INTEGER NOT NULL,
                               until TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending');
";

/// Quantos dias para a frente se podem marcar (as 20 páginas do CM).
const RANGE_DAYS: i32 = 340;
/// O primeiro dia que se pode marcar (depois de amanhã).
const MIN_AHEAD: i32 = 2;
/// Dias livres à volta de qualquer jogo do clube (menos do que isto) e de um jogo oficial.
const GAP: i32 = 2;
const OFFICIAL_GAP: i32 = 3;
/// O adversário não pode jogar a menos de tantos dias.
const OPPONENT_GAP: i32 = 2;
/// Só clubes com plantel para um jogo.
const MIN_PLAYERS: i32 = 16;
/// Jogos de uma excursão.
const TOUR_MAX: usize = 5;
/// Um clube com mais do que esta parte da reputação do nosso não vem jogar ao nosso estádio.
const BIG_CLUB: f64 = 1.6;
/// Convites: um em cada tantos dias, até tantos amigáveis (marcados ou por responder) nos
/// próximos 60 dias, para uma data entre 14 e 45 dias, longe de jogos oficiais.
const INVITE_ODDS: u64 = 12;
const INVITE_MAX: usize = 4;
const INVITE_WINDOW: i32 = 60;
const INVITE_FROM: i32 = 14;
const INVITE_TO: i32 = 45;
const INVITE_OFFICIAL_GAP: i32 = 6;
/// A resposta ao convite: até tantos dias antes do jogo.
const ANSWER_BEFORE: i32 = 3;

/// Um dia da tela "Escolher Datas": o jogo marcado, livre, indisponível ou escolhido.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FriendlyDay {
    pub date: String,
    pub fixture: Option<Fixture>,
    /// "match", "free", "busy" ou "chosen".
    pub status: String,
}

/// Um país para a excursão, com o continente ("EUR", "SAM"...).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TourNation {
    pub id: i32,
    pub name: String,
    pub continent: String,
}

/// Um clube que pode jogar nas datas escolhidas, com a divisão ("A-L").
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FriendlyClub {
    pub club: ClubRef,
    pub nation: String,
    pub division: String,
}

/// Um jogo a marcar: a data e o adversário.
#[derive(Debug, Clone, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FriendlyGame {
    pub date: String,
    pub club: i32,
}

/// O convite de outro clube para um amigável.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FriendlyInvite {
    pub id: i32,
    pub club: ClubRef,
    pub date: String,
    /// No estádio do treinador.
    pub at_home: bool,
    /// Até quando se responde.
    pub until: String,
    /// "pending", "accepted", "declined" ou "expired".
    pub status: String,
}

const CONTINENTS: [&str; 6] = ["AFR", "ASI", "EUR", "NAM", "OCE", "SAM"];

impl Game {
    /// Só o treinador de um clube marca amigáveis.
    fn friendly_owner(&self) -> Result<()> {
        if self.sacked() {
            return Err(Error::Invalid("Sem clube, não há amigáveis para marcar.".into()));
        }
        if self.is_national_team(self.club_id) {
            return Err(Error::Invalid("As selecções não marcam amigáveis de clubes.".into()));
        }
        Ok(())
    }

    /// Os dias (desde 1970) dos jogos de cada clube, e se o jogo é oficial.
    fn match_days(&self) -> HashMap<i32, Vec<(i32, bool)>> {
        let mut out: HashMap<i32, Vec<(i32, bool)>> = HashMap::new();
        for m in &self.matches {
            let Some(d) = date::parse(&m.date) else { continue };
            let official = m.competition_id != CLUB_FRIENDLY;
            for c in [m.home_id, m.away_id] {
                out.entry(c).or_default().push((d, official));
            }
        }
        out
    }

    /// O clube do treinador pode jogar um amigável no dia `d` (com os já escolhidos).
    fn day_free(&self, mine: &[(i32, bool)], d: i32, chosen: &[i32]) -> bool {
        let today = date::parse(&self.date).unwrap_or(0);
        d >= today + MIN_AHEAD
            && mine.iter().all(|&(x, official)| (x - d).abs() >= if official { OFFICIAL_GAP } else { GAP })
            && chosen.iter().all(|&c| (c - d).abs() >= GAP)
    }

    /// O adversário não joga perto do dia `d`.
    fn opponent_free(days: &HashMap<i32, Vec<(i32, bool)>>, club: i32, d: i32) -> bool {
        days.get(&club).is_none_or(|v| v.iter().all(|&(x, _)| (x - d).abs() >= OPPONENT_GAP))
    }

    /// "Escolher Datas": os próximos dias com o jogo marcado, livres, indisponíveis ou escolhidos.
    pub fn friendly_dates(&self, chosen: &[String]) -> Result<Vec<FriendlyDay>> {
        self.friendly_owner()?;
        let today = date::parse(&self.date).unwrap_or(0);
        let chosen: Vec<i32> = chosen.iter().filter_map(|c| date::parse(c)).collect();
        let days = self.match_days();
        let mine = days.get(&self.club_id).cloned().unwrap_or_default();
        let mut fixtures: HashMap<String, Fixture> = HashMap::new();
        for m in self.matches.iter().filter(|m| m.home_id == self.club_id || m.away_id == self.club_id) {
            fixtures.entry(m.date.clone()).or_insert_with(|| self.fixture(m));
        }
        let mut out = Vec::new();
        for d in today + 1..=today + RANGE_DAYS {
            let iso = date::format(d);
            let fixture = fixtures.remove(&iso);
            let status = if fixture.is_some() {
                "match"
            } else if chosen.contains(&d) {
                "chosen"
            } else if self.day_free(&mine, d, &chosen) {
                "free"
            } else {
                "busy"
            };
            out.push(FriendlyDay { date: iso, fixture, status: status.into() });
        }
        Ok(out)
    }

    /// "Escolher País" da excursão: os países com clubes para jogar (menos o do treinador).
    pub fn tour_nations(&self) -> Result<Vec<TourNation>> {
        self.friendly_owner()?;
        let own: Option<i32> = self.world.conn.query_row("SELECT nation_id FROM clubs WHERE id = ?", [self.club_id], |r| r.get(0))?;
        let rows: Vec<(i32, String, Option<i32>)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT n.id, n.name, n.continent_id FROM nations n
                 WHERE n.id IS NOT ?1 AND EXISTS (SELECT 1 FROM clubs c WHERE c.nation_id = n.id AND c.is_national_team = 0
                     AND (SELECT COUNT(*) FROM people p WHERE p.club_id = c.id AND p.is_player = 1) >= ?2)
                 ORDER BY n.name",
            )?
            .query_map(params![own, MIN_PLAYERS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .map(|(id, name, c)| TourNation {
                id,
                name,
                continent: c.and_then(|c| CONTINENTS.get(c as usize)).copied().unwrap_or("").into(),
            })
            .collect())
    }

    /// "Escolher Clube": os clubes de uma nação que podem jogar em todas as datas escolhidas.
    pub fn friendly_clubs(&self, nation: i32, dates: &[String]) -> Result<Vec<FriendlyClub>> {
        self.friendly_owner()?;
        let days = self.match_days();
        let wanted: Vec<i32> = dates.iter().filter_map(|d| date::parse(d)).collect();
        let rows: Vec<(i32, String, Option<String>)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT c.id, n.name, d.abbreviation FROM clubs c JOIN nations n ON n.id = c.nation_id
                 LEFT JOIN competitions d ON d.id = c.division_id
                 WHERE c.nation_id = ?1 AND c.is_national_team = 0 AND c.id <> ?2
                   AND (SELECT COUNT(*) FROM people p WHERE p.club_id = c.id AND p.is_player = 1) >= ?3
                 ORDER BY c.short_name",
            )?
            .query_map(params![nation, self.club_id, MIN_PLAYERS], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .filter(|(id, ..)| wanted.iter().all(|&d| Self::opponent_free(&days, *id, d)))
            .map(|(id, nation, division)| FriendlyClub { club: self.club(id), nation, division: division.unwrap_or_default() })
            .collect())
    }

    /// Um clube com plantel para um amigável.
    fn friendly_opponent(&self, club: i32) -> Result<()> {
        let (national, players): (bool, i32) = self
            .world
            .conn
            .query_row(
                "SELECT c.is_national_team <> 0, (SELECT COUNT(*) FROM people p WHERE p.club_id = c.id AND p.is_player = 1)
                 FROM clubs c WHERE c.id = ?",
                [club],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("clube {club}")))?;
        if club == self.club_id || national {
            return Err(Error::Invalid("Esse clube não pode jogar um amigável com o seu.".into()));
        }
        if players < MIN_PLAYERS {
            return Err(Error::Invalid(format!("O {} não tem plantel para um amigável.", self.club(club).short_name)));
        }
        Ok(())
    }

    fn reputation(&self, club: i32) -> Result<i32> {
        Ok(self.world.conn.query_row("SELECT COALESCE(reputation, 0) FROM clubs WHERE id = ?", [club], |r| r.get(0))?)
    }

    /// Marca amigáveis: `kind` "home" ou "away" (um jogo, com o clube convidado, que pode
    /// recusar) ou "tour" (até 5 jogos, todos no país da excursão, fora). Devolve os jogos.
    pub fn arrange_friendlies(&mut self, kind: &str, games: &[FriendlyGame]) -> Result<Vec<Fixture>> {
        self.friendly_owner()?;
        let tour = kind == "tour";
        match kind {
            "home" | "away" if games.len() == 1 => {}
            "tour" if (1..=TOUR_MAX).contains(&games.len()) => {}
            "home" | "away" => return Err(Error::Invalid("Um amistoso é um jogo.".into())),
            "tour" => return Err(Error::Invalid(format!("Uma excursão tem de 1 a {TOUR_MAX} jogos."))),
            _ => return Err(Error::Invalid("Tipo de amigável desconhecido.".into())),
        }
        let days = self.match_days();
        let mine = days.get(&self.club_id).cloned().unwrap_or_default();
        let mut games = games.to_vec();
        games.sort_by(|a, b| a.date.cmp(&b.date));
        let mut chosen: Vec<i32> = Vec::new();
        let mut nation: Option<i32> = None;
        for g in &games {
            let d = date::parse(&g.date).ok_or_else(|| Error::Invalid("Data inválida.".into()))?;
            if !self.day_free(&mine, d, &chosen) {
                return Err(Error::Invalid(format!("O {} não pode jogar a {}.", self.club(self.club_id).short_name, cm_date(&g.date))));
            }
            chosen.push(d);
            self.friendly_opponent(g.club)?;
            let name = self.club(g.club).short_name;
            if !Self::opponent_free(&days, g.club, d) {
                return Err(Error::Invalid(format!("O {name} recusou o convite: já tem um jogo perto de {}.", cm_date(&g.date))));
            }
            if tour {
                let n: Option<i32> = self.world.conn.query_row("SELECT nation_id FROM clubs WHERE id = ?", [g.club], |r| r.get(0))?;
                if nation.is_some_and(|x| Some(x) != n) {
                    return Err(Error::Invalid("Os jogos de uma excursão são todos no mesmo país.".into()));
                }
                nation = n;
            } else if kind == "home" && self.reputation(g.club)? as f64 > BIG_CLUB * self.reputation(self.club_id)?.max(1) as f64 {
                return Err(Error::Invalid(format!(
                    "O {name} recusou o convite: não tem interesse em jogar no estádio do {}.",
                    self.club(self.club_id).short_name
                )));
            }
        }
        let mut out = Vec::new();
        for g in &games {
            let (home, away) = if kind == "home" { (self.club_id, g.club) } else { (g.club, self.club_id) };
            out.push(self.add_friendly(&g.date, home, away, nation.filter(|_| tour).unwrap_or(0))?);
        }
        Ok(out)
    }

    /// Junta um amigável ao calendário (e o adversário aos clubes vivos, se ainda não era).
    fn add_friendly(&mut self, day: &str, home: i32, away: i32, tour: i32) -> Result<Fixture> {
        for club in [home, away] {
            self.ensure_live(club)?;
        }
        let m = Match {
            id: self.next_match_id(),
            competition_id: CLUB_FRIENDLY,
            round: tour,
            date: day.to_string(),
            home_id: home,
            away_id: away,
            ..Default::default()
        };
        let f = self.fixture(&m);
        self.matches.push(m);
        self.unsaved = true;
        Ok(f)
    }

    /// Os jogadores de um clube que ainda não jogava no jogo (uma excursão ao estrangeiro).
    fn ensure_live(&mut self, club: i32) -> Result<()> {
        if self.squads.traits.values().any(|t| t.club_id == club) {
            return Ok(());
        }
        let traits = crate::squad::Squads::load_traits(&self.world, &[club])?;
        self.squads.traits.extend(traits);
        super::numbers::fix_squad_numbers(&self.world, &[club], self.seed, Some(self.club_id))?;
        self.clubs.extend(super::club_refs(&self.world, &[club])?);
        Ok(())
    }

    /// "Cancelar": desmarca um amigável do treinador que ainda não se jogou (nem é hoje).
    pub fn cancel_friendly(&mut self, fixture: i32) -> Result<()> {
        let i = self.matches.iter().position(|m| m.id == fixture).ok_or_else(|| Error::NotFound(format!("jogo {fixture}")))?;
        let m = &self.matches[i];
        if m.competition_id != CLUB_FRIENDLY || (m.home_id != self.club_id && m.away_id != self.club_id) {
            return Err(Error::Invalid("Só se cancelam os amigáveis do clube.".into()));
        }
        if m.result.is_some() || m.date <= self.date {
            return Err(Error::Invalid("Esse amigável já não se pode cancelar.".into()));
        }
        self.matches.remove(i);
        self.unsaved = true;
        Ok(())
    }

    /// O nome do amigável na coluna da competição: "Excursão Australia" ou "Amigável".
    pub(super) fn friendly_name(&self, m: &Match) -> String {
        if m.round <= 0 {
            return "Amigável".into();
        }
        let nation: Option<String> =
            self.world.conn.query_row("SELECT name FROM nations WHERE id = ?", [m.round], |r| r.get(0)).optional().ok().flatten();
        nation.map_or("Excursão".into(), |n| format!("Excursão {n}"))
    }

    /// De manhã: saem os amigáveis que chocam com um jogo oficial marcado depois (ou que já não
    /// são do clube do treinador), os convites por responder expiram e pode chegar um novo.
    pub(super) fn friendlies_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let days = self.match_days();
        let official = |club: i32, d: i32| days.get(&club).is_some_and(|v| v.iter().any(|&(x, o)| o && (x - d).abs() < GAP));
        let sacked = self.sacked();
        let own = self.club_id;
        let mut dropped: Vec<Match> = Vec::new();
        self.matches.retain(|m| {
            if m.competition_id != CLUB_FRIENDLY || m.result.is_some() {
                return true;
            }
            let ours = !sacked && (m.home_id == own || m.away_id == own);
            let d = date::parse(&m.date).unwrap_or(0);
            let keep = ours && !official(m.home_id, d) && !official(m.away_id, d);
            if !keep && ours {
                dropped.push(m.clone());
            }
            keep
        });
        for m in dropped {
            let other = self.club(if m.home_id == self.club_id { m.away_id } else { m.home_id }).short_name;
            let n = self.news_item(
                *next_id,
                "message",
                "Amigável cancelado".into(),
                format!("O amigável com o {other} a {} foi cancelado: há um jogo oficial nessa altura.", cm_date(&m.date)),
            );
            fresh.push(n);
            *next_id += 1;
        }
        self.world.conn.execute("UPDATE friendly_invites SET status = 'expired' WHERE status = 'pending' AND until < ?", [&self.date])?;
        if !sacked && !self.is_national_team(self.club_id) {
            self.new_invite(&days, fresh, next_id)?;
        }
        Ok(())
    }

    /// Um clube convida o do treinador (de vez em quando, para uma data longe de jogos oficiais).
    fn new_invite(&mut self, days: &HashMap<i32, Vec<(i32, bool)>>, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let today = date::parse(&self.date).unwrap_or(0);
        let mut rng = self.comp_rng(CLUB_FRIENDLY, self.season(), today as u64);
        if !rng.next_u64().is_multiple_of(INVITE_ODDS) {
            return Ok(());
        }
        let pending: Vec<i32> = self
            .world
            .conn
            .prepare_cached("SELECT date FROM friendly_invites WHERE status = 'pending'")?
            .query_map([], |r| r.get::<_, String>(0))?
            .filter_map(|d| d.ok().and_then(|d| date::parse(&d)))
            .collect();
        let mine = days.get(&self.club_id).cloned().unwrap_or_default();
        let booked = mine.iter().filter(|&&(x, o)| !o && x >= today && x <= today + INVITE_WINDOW).count() + pending.len();
        if booked >= INVITE_MAX {
            return Ok(());
        }
        let quiet = |d: i32| mine.iter().all(|&(x, o)| !o || (x - d).abs() >= INVITE_OFFICIAL_GAP);
        let candidates: Vec<i32> =
            (today + INVITE_FROM..=today + INVITE_TO).filter(|&d| self.day_free(&mine, d, &pending) && quiet(d)).collect();
        if candidates.is_empty() {
            return Ok(());
        }
        let d = candidates[(rng.next_u64() % candidates.len() as u64) as usize];
        // Um clube do mesmo país, de reputação parecida ou menor, livre nesse dia
        let ours = self.reputation(self.club_id)?;
        let clubs: Vec<i32> = self
            .world
            .conn
            .prepare_cached(
                "SELECT c.id FROM clubs c WHERE c.nation_id = (SELECT nation_id FROM clubs WHERE id = ?1) AND c.id <> ?1
                   AND c.is_national_team = 0 AND COALESCE(c.reputation, 0) BETWEEN ?2 AND ?3
                   AND (SELECT COUNT(*) FROM people p WHERE p.club_id = c.id AND p.is_player = 1) >= ?4
                 ORDER BY c.id",
            )?
            .query_map(params![self.club_id, ours * 2 / 5, ours * 11 / 10, MIN_PLAYERS], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<i32>>>()?
            .into_iter()
            .filter(|&c| Self::opponent_free(days, c, d))
            .collect();
        if clubs.is_empty() {
            return Ok(());
        }
        let club = clubs[(rng.next_u64() % clubs.len() as u64) as usize];
        let at_home = rng.next_u64().is_multiple_of(2);
        let day = date::format(d);
        let until = date::format(d - ANSWER_BEFORE);
        self.world
            .conn
            .execute("INSERT INTO friendly_invites (club, date, at_home, until) VALUES (?, ?, ?, ?)", params![club, day, at_home, until])?;
        let id = self.world.conn.last_insert_rowid() as i32;
        let (them, us) = (self.club(club).short_name, self.club(self.club_id).short_name);
        let place = if at_home { format!("no estádio do {us}") } else { format!("no estádio do {them}") };
        let mut n = self.news_item(
            *next_id,
            "message",
            format!("Convite do {them}"),
            format!("O {them} convida o {us} para um jogo amigável {place}, a {}.\n\nResponda até {}.", cm_date(&day), cm_date(&until)),
        );
        n.link = Some(format!("friendly:{id}"));
        fresh.push(n);
        *next_id += 1;
        Ok(())
    }

    pub fn friendly_invite(&self, id: i32) -> Result<FriendlyInvite> {
        let (club, day, at_home, until, status): (i32, String, bool, String, String) = self
            .world
            .conn
            .query_row("SELECT club, date, at_home, until, status FROM friendly_invites WHERE id = ?", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .optional()?
            .ok_or_else(|| Error::NotFound(format!("convite {id}")))?;
        Ok(FriendlyInvite { id, club: self.club(club), date: day, at_home, until, status })
    }

    /// Aceitar ou recusar o convite: aceite, o jogo entra no calendário (se a data ainda estiver livre).
    pub fn answer_friendly_invite(&mut self, id: i32, accept: bool) -> Result<FriendlyInvite> {
        let inv = self.friendly_invite(id)?;
        if inv.status != "pending" {
            return Err(Error::Invalid("Este convite já não está à espera de resposta.".into()));
        }
        if accept {
            self.friendly_owner()?;
            let days = self.match_days();
            let mine = days.get(&self.club_id).cloned().unwrap_or_default();
            let d = date::parse(&inv.date).unwrap_or(0);
            if !self.day_free(&mine, d, &[]) || !Self::opponent_free(&days, inv.club.id, d) {
                return Err(Error::Invalid(format!("Já não há data livre a {} para este amigável.", cm_date(&inv.date))));
            }
            let (home, away) = if inv.at_home { (self.club_id, inv.club.id) } else { (inv.club.id, self.club_id) };
            self.add_friendly(&inv.date, home, away, 0)?;
        }
        self.world
            .conn
            .execute("UPDATE friendly_invites SET status = ? WHERE id = ?", params![if accept { "accepted" } else { "declined" }, id])?;
        self.unsaved = true;
        self.friendly_invite(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewGame, World};

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn arrange_play_and_cancel_friendlies() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-amigaveis-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = NewGame {
            save_name: "Amigáveis".into(),
            first_name: "A".into(),
            last_name: "B".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: false,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 8).unwrap();
        let brazil: i32 = g.world.conn.query_row("SELECT nation_id FROM clubs WHERE id = ?", [PALMEIRAS], |r| r.get(0)).unwrap();

        // As datas: nada antes de depois de amanhã; à volta de um jogo, indisponível
        let days = g.friendly_dates(&[]).unwrap();
        assert!(days.iter().take(1).all(|d| d.status != "free"));
        let free: Vec<&FriendlyDay> = days.iter().filter(|d| d.status == "free").collect();
        assert!(!free.is_empty());
        let first = free[0].date.clone();
        let chosen = g.friendly_dates(std::slice::from_ref(&first)).unwrap();
        let next = date::add_days(&first, 1).unwrap();
        assert_eq!(chosen.iter().find(|d| d.date == first).unwrap().status, "chosen");
        assert_ne!(chosen.iter().find(|d| d.date == next).unwrap().status, "free", "o dia a seguir fica indisponível");

        // Um amistoso em casa com um clube brasileiro mais pequeno, livre nesse dia
        let clubs = g.friendly_clubs(brazil, std::slice::from_ref(&first)).unwrap();
        let ours = g.reputation(PALMEIRAS).unwrap();
        let small = clubs.iter().find(|c| g.reputation(c.club.id).unwrap() < ours).expect("um clube mais pequeno").club.id;
        let made = g.arrange_friendlies("home", &[FriendlyGame { date: first.clone(), club: small }]).unwrap();
        assert_eq!(made[0].competition, "Amigável");
        assert_eq!(made[0].home.id, PALMEIRAS);
        // A mesma data já não serve; dois jogos num amistoso também não
        assert!(g.arrange_friendlies("away", &[FriendlyGame { date: first.clone(), club: small }]).is_err());
        assert!(g.arrange_friendlies("away", &[]).is_err());

        // Uma excursão à Austrália: dois jogos fora, um clube por data
        let australia = g.tour_nations().unwrap().into_iter().find(|n| n.name == "Australia").unwrap();
        assert_eq!(australia.continent, "ASI", "a Austrália joga na Ásia, como no CM");
        let free: Vec<String> = g.friendly_dates(&[]).unwrap().into_iter().filter(|d| d.status == "free").map(|d| d.date).collect();
        let a = free[0].clone();
        let b = free.iter().find(|d| date::parse(d).unwrap() - date::parse(&a).unwrap() >= GAP).unwrap().clone();
        let aussies = g.friendly_clubs(australia.id, &[a.clone(), b.clone()]).unwrap();
        assert!(aussies.len() >= 2);
        let tour = g
            .arrange_friendlies(
                "tour",
                &[FriendlyGame { date: a.clone(), club: aussies[0].club.id }, FriendlyGame { date: b.clone(), club: aussies[1].club.id }],
            )
            .unwrap();
        assert_eq!(tour.len(), 2);
        assert!(tour.iter().all(|f| f.competition == "Excursão Australia" && f.away.id == PALMEIRAS));
        let fixtures = g.club_fixtures(PALMEIRAS);
        assert_eq!(fixtures.iter().filter(|f| f.competition_id == CLUB_FRIENDLY).count(), 3);

        // Cancelar o último jogo da excursão
        let last = tour[1].id;
        g.cancel_friendly(last).unwrap();
        assert!(!g.matches.iter().any(|m| m.id == last));
        let official = g.matches.iter().find(|m| m.competition_id != CLUB_FRIENDLY && m.home_id == PALMEIRAS).unwrap().id;
        assert!(g.cancel_friendly(official).is_err(), "só os amigáveis");

        // Um convite aceite entra no calendário
        let free = g.friendly_dates(&[]).unwrap().into_iter().find(|d| d.status == "free").unwrap().date;
        let other = g.friendly_clubs(brazil, std::slice::from_ref(&free)).unwrap()[0].club.id;
        g.world
            .conn
            .execute("INSERT INTO friendly_invites (club, date, at_home, until) VALUES (?, ?, 0, ?)", params![other, free, free])
            .unwrap();
        let id = g.world.conn.last_insert_rowid() as i32;
        let inv = g.answer_friendly_invite(id, true).unwrap();
        assert_eq!(inv.status, "accepted");
        assert!(g.matches.iter().any(|m| m.competition_id == CLUB_FRIENDLY && m.home_id == other && m.date == free));
        assert!(g.answer_friendly_invite(id, false).is_err());

        // Joga o amistoso: o jogo fica em friendly_apps ("Não Competitivos"), não nos oficiais
        let fixture = made[0].id;
        while g.matches.iter().any(|m| m.id == fixture && m.result.is_none()) {
            g.advance_day().unwrap();
        }
        let count = |table: &str| -> i64 {
            g.world.conn.query_row(&format!("SELECT COUNT(*) FROM {table} WHERE fixture = ?"), [fixture], |r| r.get(0)).unwrap()
        };
        assert!(count("friendly_apps") >= 22);
        assert_eq!(count("player_matches"), 0);
        let sheet = g.match_sheet(fixture).unwrap();
        assert!(!sheet.home.is_empty());
        let player: i32 = g
            .world
            .conn
            .query_row("SELECT person_id FROM friendly_apps WHERE fixture = ? AND club = ? LIMIT 1", params![fixture, PALMEIRAS], |r| {
                r.get(0)
            })
            .unwrap();
        let mut p = g.world.get_person(player).unwrap();
        g.fill_record(&mut p).unwrap();
        let kinds = &p.seasons[0].by_kind;
        assert_eq!(kinds[0].0, "Não Competitivos");
        assert!(kinds[0].1.apps >= 1);
        assert!(g.news.iter().any(|n| n.body.starts_with("Amigável.")));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
