//! Polimento do perfil e do plantel (marco 3.13, passo 1): os jogos de cada jogador ficam
//! gravados (`player_matches`) para a Forma (as últimas 5 notas) e as estatísticas da época
//! por competição; a alcunha e a nota do treinador; Dispensar um jogador com a indemnização
//! do resto do contrato, como no CM.

use rusqlite::{OptionalExtension, params};

use std::collections::BTreeMap;

use super::market::money_text;
use super::{Game, cm_date};
use crate::live::PlayerLine;
use crate::person::{HonourLine, InjuryLine, PersonProfile, SeasonLine, SeasonRecord, StatLine, SuspensionLine};
use crate::regulation::{self, SeasonKind, StageKind};
use crate::squad::Event;
use crate::{Error, Result, date};

/// Notas na Forma do perfil.
const FORM_GAMES: usize = 5;

/// Os tipos de competição do Historial.
const KINDS: [&str; 3] = ["Liga", "Taça", "Continentais"];

/// Somas de um conjunto de jogos, para fazer uma `StatLine`.
#[derive(Debug, Clone, Default)]
struct Totals {
    apps: i32,
    subs: i32,
    mom: i32,
    goals: i32,
    assists: i32,
    passes: i32,
    passes_ok: i32,
    tackles: i32,
    dribbles: i32,
    shots: i32,
    on_target: i32,
    rating: f64,
}

impl Totals {
    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        sub: bool,
        mom: bool,
        goals: i32,
        assists: i32,
        passes: i32,
        ok: i32,
        tackles: i32,
        dribbles: i32,
        shots: i32,
        on: i32,
        rating: f64,
    ) {
        self.apps += 1;
        self.subs += sub as i32;
        self.mom += mom as i32;
        self.goals += goals;
        self.assists += assists;
        self.passes += passes;
        self.passes_ok += ok;
        self.tackles += tackles;
        self.dribbles += dribbles;
        self.shots += shots;
        self.on_target += on;
        self.rating += rating;
    }

    fn merge(&mut self, o: &Totals) {
        self.apps += o.apps;
        self.subs += o.subs;
        self.mom += o.mom;
        self.goals += o.goals;
        self.assists += o.assists;
        self.passes += o.passes;
        self.passes_ok += o.passes_ok;
        self.tackles += o.tackles;
        self.dribbles += o.dribbles;
        self.shots += o.shots;
        self.on_target += o.on_target;
        self.rating += o.rating;
    }

    fn line(&self) -> StatLine {
        let per_game = |v: i32| (self.apps > 0).then(|| (v as f64 / self.apps as f64 * 10.0).round() / 10.0);
        let pct = |a: i32, b: i32| (b > 0).then(|| (100.0 * a as f64 / b as f64).round() as i32);
        StatLine {
            apps: self.apps,
            subs: self.subs,
            goals: self.goals,
            assists: self.assists,
            mom: self.mom,
            pass_pct: pct(self.passes_ok, self.passes),
            tackles: per_game(self.tackles),
            dribbles: per_game(self.dribbles),
            shot_pct: pct(self.on_target, self.shots),
            rating: (self.apps > 0).then(|| (self.rating / self.apps as f64 * 100.0).round() / 100.0),
        }
    }
}

impl Game {
    /// No fim do dia: a linha de cada jogador que jogou (nota, golos, assistências, passes,
    /// cortes, dribles, remates); o homem do jogo é a melhor nota dos dois lados.
    pub(super) fn record_match(&self, fixture: i32, competition: i32, clubs: [i32; 2], lines: [Vec<PlayerLine>; 2]) -> Result<()> {
        let mom = lines.iter().flatten().max_by(|a, b| a.rating.total_cmp(&b.rating)).map(|l| l.id);
        let mut insert = self.world.conn.prepare_cached(
            "INSERT INTO player_matches (person_id, fixture, date, competition, club, rating, goals, assists, sub, mom,
                                         passes, passes_ok, tackles, dribbles, shots, on_target)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )?;
        for (side, players) in lines.iter().enumerate() {
            for l in players {
                insert.execute(params![
                    l.id,
                    fixture,
                    self.date,
                    competition,
                    clubs[side],
                    (l.rating * 10.0).round() / 10.0,
                    l.goals,
                    l.assists,
                    l.sub,
                    mom == Some(l.id),
                    l.stats.passes,
                    l.stats.passes_ok,
                    l.stats.tackles,
                    l.stats.dribbles,
                    l.stats.shots,
                    l.stats.on_target
                ])?;
            }
        }
        Ok(())
    }

    /// Lesões e suspensões de um jogo para o Historial dos jogadores.
    pub(super) fn log_events(&self, events: &[Event], competition: i32) -> Result<()> {
        for e in events {
            match e {
                Event::Injured { player, injury, days, .. } => {
                    self.world.conn.execute(
                        "INSERT INTO injury_log (person_id, date, injury, kind, days) VALUES (?, ?, ?, 'Jogo', ?)",
                        params![player, self.date, injury, days],
                    )?;
                }
                Event::Suspended { player, red, .. } => {
                    self.world.conn.execute(
                        "INSERT INTO suspension_log (person_id, date, competition, games, red) VALUES (?, ?, ?, 1, ?)",
                        params![player, self.date, competition, red],
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Uma competição acabou: o título (e a final perdida) no currículo de quem está no plantel.
    pub(super) fn record_honours(&self, competition: i32) -> Result<()> {
        let year = self.current_season(competition);
        let Some((winner, second, _)) = self.podium(competition, year) else { return Ok(()) };
        let reg = regulation::get(competition);
        // Uma final (eliminatória no fim) tem vencedor e finalista vencido; uma liga, campeão
        let knockout = reg.is_some_and(|r| {
            let last = self.year_matches(competition, year).map(|m| m.stage).max().unwrap_or(0) as usize;
            r.stages.get(last).is_some_and(|s| s.kind == StageKind::Knockout)
        });
        let mut winners = vec![(winner, if knockout { "winner" } else { "champion" })];
        if knockout && let Some(s) = second {
            winners.push((s, "runner_up"));
        }
        for (club, kind) in winners {
            self.world.conn.execute(
                "INSERT INTO player_honours (person_id, date, club, competition, kind)
                 SELECT id, ?, ?, ?, ? FROM people WHERE club_id = ? AND is_player = 1",
                params![self.date, club, competition, kind, club],
            )?;
        }
        Ok(())
    }

    /// Forma, estatísticas da época, alcunha, nota e o custo de dispensar (jogador do treinador).
    pub fn fill_record(&self, p: &mut PersonProfile) -> Result<()> {
        p.nickname = self.world.conn.query_row("SELECT nickname FROM nicknames WHERE person_id = ?", [p.id], |r| r.get(0)).optional()?;
        if let Some(n) = &p.nickname {
            p.name = n.clone();
        }
        p.note = self.world.conn.query_row("SELECT text FROM notes WHERE person_id = ?", [p.id], |r| r.get(0)).optional()?;
        if p.player.is_none() {
            return Ok(());
        }
        p.form = self
            .world
            .conn
            .prepare_cached("SELECT rating FROM player_matches WHERE person_id = ? ORDER BY date DESC, fixture DESC LIMIT ?")?
            .query_map(params![p.id, FORM_GAMES as i64], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        p.form.reverse();
        // A época do clube onde joga (civil ou europeia)
        let kind = p.club.as_ref().map(|c| self.kind_of(c.id)).transpose()?.unwrap_or(SeasonKind::European);
        let year: i32 = self.date[0..4].parse().unwrap_or(0);
        let start = match kind {
            SeasonKind::Calendar => format!("{year}-01-01"),
            SeasonKind::European if &self.date[5..] >= "07-01" => format!("{year}-07-01"),
            SeasonKind::European => format!("{}-07-01", year - 1),
        };
        let rows: Vec<(i32, i32, i32, i32, f64)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT competition, COUNT(*), SUM(goals), SUM(assists), AVG(rating) FROM player_matches
                 WHERE person_id = ? AND date >= ? GROUP BY competition ORDER BY COUNT(*) DESC, competition",
            )?
            .query_map(params![p.id, start], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        p.season = rows
            .into_iter()
            .map(|(c, apps, goals, assists, rating)| SeasonLine {
                competition: self.competition_name(c).to_string(),
                apps,
                goals,
                assists,
                rating: (rating * 100.0).round() / 100.0,
            })
            .collect();
        if p.club.as_ref().is_some_and(|c| c.id == self.club_id) {
            p.release_cost = Some(self.release_cost(p.id)?);
        }
        self.fill_history(p)
    }

    /// Carreira por época com as estatísticas, lesões, suspensões e títulos.
    fn fill_history(&self, p: &mut PersonProfile) -> Result<()> {
        type Row = (i32, i32, String, bool, bool, i32, i32, i32, i32, i32, i32, i32, i32, f64);
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare_cached(
                "SELECT club, competition, date, sub, mom, goals, assists, passes, passes_ok, tackles, dribbles, shots, on_target, rating
                 FROM player_matches WHERE person_id = ? ORDER BY date",
            )?
            .query_map([p.id], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    r.get(13)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        // Por época (a da competição) e clube; dentro, por tipo de competição
        let mut seasons: BTreeMap<(i32, i32), (String, BTreeMap<&'static str, Totals>)> = BTreeMap::new();
        for r in &rows {
            let (club, comp) = (r.0, r.1);
            let year = regulation::season_of(comp, &r.2);
            let entry = seasons.entry((year, club)).or_insert_with(|| (regulation::season_label(comp, year), BTreeMap::new()));
            let t = entry.1.entry(self.comp_kind(comp)).or_default();
            t.add(r.3, r.4, r.5, r.6, r.7, r.8, r.9, r.10, r.11, r.12, r.13);
        }
        // Emprestado: o clube atual, com o empréstimo em curso
        let on_loan: bool =
            self.world.conn.query_row("SELECT COUNT(*) FROM loans WHERE person_id = ?", [p.id], |r| r.get::<_, i64>(0))? > 0;
        let latest = rows.last().map(|r| (regulation::season_of(r.1, &r.2), r.0));
        let mut out: Vec<SeasonRecord> = seasons
            .into_iter()
            .rev()
            .map(|((year, club), (label, kinds))| {
                let mut total = Totals::default();
                for t in kinds.values() {
                    total.merge(t);
                }
                let mut by_kind: Vec<(String, StatLine)> =
                    KINDS.iter().map(|k| (k.to_string(), kinds.get(k).map(Totals::line).unwrap_or_default())).collect();
                by_kind.push(("Jogos Oficiais".into(), total.line()));
                SeasonRecord {
                    label,
                    year,
                    club: Some(self.club(club)),
                    on_loan: on_loan && latest == Some((year, club)) && p.club.as_ref().is_some_and(|c| c.id == club),
                    stats: total.line(),
                    by_kind,
                }
            })
            .collect();
        // As épocas do patch: só jogos e golos; a do ano em que o jogo começou, no mesmo clube,
        // soma-se à do jogo (a primeira metade da época civil)
        let first = out.iter().map(|s| s.year).min();
        for c in &p.career {
            let Some(year) = c.year else { continue };
            if first.is_some_and(|f| year > f) {
                continue;
            }
            if let Some(s) = out.iter_mut().find(|s| s.year == year && s.club.as_ref().map(|x| x.id) == c.club.as_ref().map(|x| x.id)) {
                s.stats.apps += c.apps;
                s.stats.goals += c.goals;
                continue;
            }
            out.push(SeasonRecord {
                label: year.to_string(),
                year,
                club: c.club.clone(),
                on_loan: c.on_loan,
                stats: StatLine { apps: c.apps, goals: c.goals, ..Default::default() },
                by_kind: Vec::new(),
            });
        }
        p.seasons = out;
        p.injuries = self
            .world
            .conn
            .prepare_cached("SELECT date, injury, kind, days FROM injury_log WHERE person_id = ? ORDER BY date DESC")?
            .query_map([p.id], |r| Ok(InjuryLine { date: r.get(0)?, injury: r.get(1)?, kind: r.get(2)?, days: r.get(3)? }))?
            .collect::<rusqlite::Result<_>>()?;
        let suspensions: Vec<(String, i32, i32, bool)> = self
            .world
            .conn
            .prepare_cached("SELECT date, competition, games, red FROM suspension_log WHERE person_id = ? ORDER BY date DESC")?
            .query_map([p.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        p.suspensions = suspensions
            .into_iter()
            .map(|(date, comp, games, red)| SuspensionLine {
                date,
                text: format!("Suspensão de {games} jogo{} ({})", if games > 1 { "s" } else { "" }, self.competition_name(comp)),
                reason: if red { "Cartão vermelho".into() } else { format!("{} cartões amarelos", crate::squad::YELLOWS_FOR_BAN) },
            })
            .collect();
        let honours: Vec<(String, i32, i32, String)> = self
            .world
            .conn
            .prepare_cached("SELECT date, club, competition, kind FROM player_honours WHERE person_id = ? ORDER BY date DESC")?
            .query_map([p.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        p.honours = honours
            .into_iter()
            .map(|(date, club, comp, kind)| {
                let what = match kind.as_str() {
                    "champion" => "Campeão",
                    "winner" => "Vencedor",
                    _ => "Finalista vencido",
                };
                HonourLine { date, club: Some(self.club(club)), text: format!("{what} {}", self.competition_name(comp)) }
            })
            .collect();
        Ok(())
    }

    /// Liga, Taça ou Continentais, como no Historial do CM.
    fn comp_kind(&self, comp: i32) -> &'static str {
        let nation: Option<i32> =
            self.world.conn.query_row("SELECT nation_id FROM competitions WHERE id = ?", [comp], |r| r.get(0)).unwrap_or(None);
        if nation.is_none() {
            return KINDS[2];
        }
        match regulation::get(comp).and_then(|r| r.stages.first()) {
            Some(st) if st.kind == StageKind::League && !st.entrants.estadual => KINDS[0],
            _ => KINDS[1],
        }
    }

    /// "Definir Alcunha" (vazia apaga).
    pub fn set_nickname(&mut self, person: i32, nickname: &str) -> Result<()> {
        let nickname = nickname.trim();
        if nickname.chars().count() > 30 {
            return Err(Error::Invalid("A alcunha tem no máximo 30 letras.".into()));
        }
        if nickname.is_empty() {
            self.world.conn.execute("DELETE FROM nicknames WHERE person_id = ?", [person])?;
        } else {
            self.world.conn.execute("INSERT OR REPLACE INTO nicknames (person_id, nickname) VALUES (?, ?)", params![person, nickname])?;
        }
        self.unsaved = true;
        Ok(())
    }

    /// "Criar Nota Treinador" (vazia apaga).
    pub fn set_note(&mut self, person: i32, text: &str) -> Result<()> {
        let text = text.trim();
        if text.is_empty() {
            self.world.conn.execute("DELETE FROM notes WHERE person_id = ?", [person])?;
        } else {
            self.world.conn.execute("INSERT OR REPLACE INTO notes (person_id, text) VALUES (?, ?)", params![person, text])?;
        }
        self.unsaved = true;
        Ok(())
    }

    /// A indemnização para dispensar: o salário até ao fim do contrato.
    fn release_cost(&self, person: i32) -> Result<i64> {
        let (wage, expires): (Option<i64>, Option<String>) =
            self.world
                .conn
                .query_row("SELECT wage, contract_expires FROM people WHERE id = ?", [person], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let days = expires.as_deref().and_then(date::parse).zip(date::parse(&self.date)).map_or(0, |(e, t)| (e - t).max(0));
        Ok(crate::finance::round_money(wage.unwrap_or(0) as f64 * days as f64 / 7.0))
    }

    /// "Dispensar": o jogador fica livre e o clube paga o resto do contrato.
    pub fn release_player(&mut self, person: i32) -> Result<String> {
        let club: Option<i32> = self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [person], |r| r.get(0))?;
        if club != Some(self.club_id) || self.loan_of(person)?.is_some() {
            return Err(Error::Invalid("Só se dispensam jogadores do seu plantel.".into()));
        }
        let cost = self.release_cost(person)?;
        let name = self.person_name(person)?;
        self.book(self.club_id, "wages", cost)?;
        self.world.conn.execute("UPDATE people SET club_id = NULL, club_job = NULL, contract_expires = NULL WHERE id = ?", [person])?;
        self.number_after_move(person, None)?;
        self.log_transfer(person, Some(self.club_id), None, 0, "release")?;
        for table in ["transfer_list", "squad_roles", "shortlist"] {
            self.world.conn.execute(&format!("DELETE FROM {table} WHERE person_id = ?"), [person])?;
        }
        self.squad_changed()?;
        let text = if cost > 0 {
            format!("{name} foi dispensado. O clube pagou-lhe {} de indemnização pelo resto do contrato.", money_text(cost))
        } else {
            format!("{name} foi dispensado.")
        };
        self.post_news_as("contract", format!("{name} dispensado"), text.clone());
        Ok(text)
    }

    /// A pergunta do CM antes de dispensar.
    pub fn release_question(&self, person: i32) -> Result<String> {
        let name = self.person_name(person)?;
        let cost = self.release_cost(person)?;
        let until: Option<String> =
            self.world.conn.query_row("SELECT contract_expires FROM people WHERE id = ?", [person], |r| r.get(0))?;
        let club = self.club(self.club_id).short_name;
        Ok(if cost > 0 {
            format!(
                "Dispensar {name}? O {club} terá de o indemnizar em {} (contrato até {}).",
                money_text(cost),
                until.as_deref().map(cm_date).unwrap_or_default()
            )
        } else {
            format!("Dispensar {name}?")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_nickname_and_release() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-perfil-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Perfil".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
            start: Default::default(),
        };
        let mut g = Game::create(&path, &dir, &new, 17).unwrap();
        let first = g.matches.iter().filter(|m| m.home_id == 7568 || m.away_id == 7568).map(|m| m.date.clone()).min().unwrap();
        while g.date.as_str() <= first.as_str() {
            g.advance_day().unwrap();
        }
        let played: i32 =
            g.world.conn.query_row("SELECT person_id FROM player_matches WHERE club = 7568 LIMIT 1", [], |r| r.get(0)).unwrap();
        let mut p = g.world.get_person(played).unwrap();
        g.fill_record(&mut p).unwrap();
        assert_eq!(p.form.len(), 1);
        assert_eq!(p.season.iter().map(|s| s.apps).sum::<i32>(), 1);
        // O Historial: a época com passes, cortes e nota (o jogo não foi visto: as contas à parte)
        let season = &p.seasons[0];
        assert_eq!(season.stats.apps, 1);
        assert!(season.stats.pass_pct.is_some() && season.stats.rating.is_some());
        assert_eq!(season.by_kind.last().map(|k| k.0.as_str()), Some("Jogos Oficiais"));
        let passes: i64 = g.world.conn.query_row("SELECT SUM(passes) FROM player_matches", [], |r| r.get(0)).unwrap();
        let moms: i64 = g.world.conn.query_row("SELECT COUNT(DISTINCT fixture) - SUM(mom) FROM player_matches", [], |r| r.get(0)).unwrap();
        assert!(passes > 0);
        assert_eq!(moms, 0, "um homem do jogo por jogo");
        g.set_nickname(played, "Mágico").unwrap();
        g.set_note(played, "Bom no jogo aéreo").unwrap();
        g.fill_record(&mut p).unwrap();
        assert_eq!((p.name.as_str(), p.note.as_deref()), ("Mágico", Some("Bom no jogo aéreo")));
        // Dispensar: fica livre e o clube paga o resto do contrato
        let cost = p.release_cost.unwrap();
        let cash = g.cash_of(7568).unwrap();
        assert!(g.release_question(played).unwrap().starts_with("Dispensar"));
        g.release_player(played).unwrap();
        assert_eq!(g.facts(played).unwrap().unwrap().club, None);
        assert_eq!(g.cash_of(7568).unwrap(), cash - cost);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
