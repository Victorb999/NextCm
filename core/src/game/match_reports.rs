//! A partida jogada como o CM a mostra depois (Visão Geral, Estatísticas, Zonas de Ação,
//! Relatório, os Dados de cada time e as Classificações Jogadores): o `LiveView` final de cada
//! jogo, sem o relato lance a lance, os outros jogos e a classificação, comprimido (save 74).
//! Os jogos das reservas (motor rápido) guardam só a escalação, os gols e as notas.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use rand_core::RngCore;
use rusqlite::{OptionalExtension, params};

use super::Game;
use crate::engine::{self, uniform};
use crate::league::Fixture;
use crate::live::{LivePlayer, LiveTeam, LiveView, MatchEvent, PlayerStats, TeamStats};
use crate::tactics::ClubTactics;
use crate::{Error, Result};

/// Save 74: as partidas guardadas.
pub(super) const REPORTS_SCHEMA: &str = "CREATE TABLE match_reports (fixture INTEGER PRIMARY KEY, data BLOB NOT NULL);";

/// Os lances que ficam no Relatório da partida (o resto do relato não se guarda).
const KEY_EVENTS: [&str; 8] = ["start", "goal", "yellow", "red", "injury", "sub", "half", "end"];

impl Game {
    /// Guarda a partida acabada (os jogos das reservas com o id negativo, como na ficha).
    pub(super) fn store_report(&self, fixture: i32, mut v: LiveView) -> Result<()> {
        v.others.clear();
        v.table.clear();
        v.events.retain(|e| KEY_EVENTS.contains(&e.kind.as_str()));
        for e in &mut v.events {
            e.lines.clear();
        }
        let json = serde_json::to_vec(&v).map_err(|e| Error::Invalid(format!("partida {fixture}: {e}")))?;
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
        enc.write_all(&json)?;
        let data = enc.finish()?;
        self.world.conn.execute("INSERT OR REPLACE INTO match_reports (fixture, data) VALUES (?, ?)", params![fixture, data])?;
        Ok(())
    }

    /// A partida guardada, se houver (os jogos de antes do save 74 não a têm).
    pub(super) fn match_report(&self, fixture: i32) -> Result<Option<LiveView>> {
        let data: Option<Vec<u8>> =
            self.world.conn.query_row("SELECT data FROM match_reports WHERE fixture = ?", [fixture], |r| r.get(0)).optional()?;
        let Some(data) = data else { return Ok(None) };
        let mut json = Vec::new();
        DeflateDecoder::new(data.as_slice()).read_to_end(&mut json)?;
        Ok(serde_json::from_slice(&json).ok())
    }

    /// A partida das reservas para a ficha: as duas escalações (com o lugar no campo), as notas e
    /// os gols. Os minutos dos gols saem de um sorteio à parte, que não mexe no resultado.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn reserve_match_view(
        &self,
        fixture: Fixture,
        teams: &[ClubTactics; 2],
        goals: [i32; 2],
        ratings: &[(i32, f64, i32, i32)],
    ) -> LiveView {
        let mut rng = engine::match_rng(self.seed ^ 0x0E5E_4E70, fixture.id);
        let player = |p: &crate::tactics::TacticPlayer, role: Option<(String, i32, i32)>| {
            let rated = ratings.iter().find(|r| r.0 == p.id);
            LivePlayer {
                id: p.id,
                list_name: p.list_name.clone(),
                squad_number: p.squad_number,
                role: role.as_ref().map(|r| r.0.clone()),
                position: p.position.clone(),
                condition: 100,
                rating: rated.map(|r| r.1.round() as i32),
                goals: rated.map_or(0, |r| r.2),
                assists: rated.map_or(0, |r| r.3),
                yellow: false,
                red: false,
                injured: false,
                fit: None,
                on_minute: None,
                off_minute: None,
                x: role.as_ref().map(|r| r.1),
                y: role.as_ref().map(|r| r.2),
                stats: PlayerStats::default(),
            }
        };
        let team = |t: &ClubTactics| LiveTeam {
            club: self.club(t.club_id),
            formation: t.formation.clone(),
            instructions: t.instructions,
            players: t.starters.iter().filter_map(|s| s.player.as_ref().map(|p| player(p, Some((s.role.clone(), s.x, s.y))))).collect(),
            bench: t.bench.iter().map(|p| player(p, None)).collect(),
            out: Vec::new(),
            subs_left: 0,
            stats: TeamStats::default(),
        };
        let home = team(&teams[0]);
        let away = team(&teams[1]);
        // Os gols: um minuto ao acaso para cada um, pela ordem; o marcador, quem marcou (os de
        // enchimento não têm nome)
        let mut events = Vec::new();
        for (side, t) in [&home, &away].into_iter().enumerate() {
            let mut scorers: Vec<&LivePlayer> = t.players.iter().flat_map(|p| std::iter::repeat_n(p, p.goals.max(0) as usize)).collect();
            for _ in 0..goals[side] {
                let minute = 1 + (rng.next_u32() % 90) as i32;
                let who = (!scorers.is_empty()).then(|| scorers.remove((uniform(&mut rng) * scorers.len() as f64) as usize));
                events.push(MatchEvent {
                    minute,
                    kind: "goal".into(),
                    side: Some(side as i32),
                    player_id: who.map(|p| p.id),
                    text: match who {
                        Some(p) => format!("Gol de {}", first_last(&p.list_name)),
                        None => format!("Gol do {}", t.club.short_name),
                    },
                    lines: Vec::new(),
                });
            }
        }
        events.sort_by_key(|e| e.minute);
        LiveView {
            fixture,
            minute: 90,
            half_time: false,
            finished: true,
            home_goals: goals[0],
            away_goals: goals[1],
            user_side: None,
            home,
            away,
            events,
            others: Vec::new(),
            recent_possession: 50,
            zones: [0, 0, 0],
            zone_possession: [50, 50, 50],
            useful_time: 0,
            venue: None,
            city: None,
            weather: String::new(),
            referee: String::new(),
            attendance: 0,
            table: Vec::new(),
        }
    }
}

/// "Sobrenome, Nome" → "Nome Sobrenome".
fn first_last(list_name: &str) -> String {
    match list_name.split_once(", ") {
        Some((last, first)) => format!("{first} {last}"),
        None => list_name.to_string(),
    }
}
