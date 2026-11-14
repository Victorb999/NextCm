//! Polimento do perfil e do plantel (marco 3.13, passo 1): os jogos de cada jogador ficam
//! gravados (`player_matches`) para a Forma (as últimas 5 notas) e as estatísticas da época
//! por competição; a alcunha e a nota do treinador; Dispensar um jogador com a indemnização
//! do resto do contrato, como no CM.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use std::collections::BTreeMap;

use super::market::money_text;
use super::{Game, cm_date};
use crate::live::PlayerLine;
use crate::person::{HonourLine, InjuryLine, PersonProfile, SeasonLine, SeasonRecord, StatLine, SuspensionLine};
use crate::regulation::{self, SeasonKind, StageKind};
use crate::squad::Event;
use crate::{Error, Result, date};

/// Um jogador na ficha da partida (as "Classificações Jogadores" do CM).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct SheetLine {
    pub id: i32,
    pub name: String,
    pub number: Option<i32>,
    pub rating: f64,
    pub goals: i32,
    pub assists: i32,
    /// Homem do jogo.
    pub mom: bool,
    /// Minuto em que entrou (0 = titular) e em que saiu; `None` em jogos de antes do save 34.
    pub on: Option<i32>,
    pub off: Option<i32>,
    pub yellow: bool,
    pub red: bool,
    /// Golos esperados (0 nos jogos de antes do save 62 e nas reservas).
    pub xg: f64,
}

/// A ficha de uma partida jogada: os dois lados, titulares primeiro e depois quem entrou.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MatchSheet {
    pub fixture: crate::Fixture,
    pub home: Vec<SheetLine>,
    pub away: Vec<SheetLine>,
}

/// Pontos fortes que o adjunto nota (atributo, frase) e os pontos fracos (atributo, o que
/// tem de melhorar), para a frase do Perfil Geral.
const STRENGTHS: [(&str, &str); 14] = [
    ("pace", "rápido"),
    ("dribbling", "forte no drible"),
    ("finishing", "um finalizador perigoso"),
    ("passing", "tem um passe de qualidade"),
    ("vision", "vê o jogo como poucos"),
    ("tackling", "forte no desarme"),
    ("marking", "um marcador seguro"),
    ("heading", "forte no jogo aéreo"),
    ("crossing", "cruza muito bem"),
    ("long_shots", "remata bem de longe"),
    ("technique", "muito evoluído tecnicamente"),
    ("stamina", "incansável"),
    ("strength", "fisicamente muito forte"),
    ("positioning", "sempre bem posicionado"),
];
const KEEPER_STRENGTHS: [(&str, &str); 4] = [
    ("reflexes", "tem grandes reflexos"),
    ("handling", "tem mãos seguras"),
    ("one_on_ones", "forte no um contra um"),
    ("positioning", "sempre bem posicionado entre os postes"),
];
const WEAKNESSES: [(&str, &str); 8] = [
    ("pace", "a velocidade"),
    ("stamina", "a resistência"),
    ("passing", "o passe"),
    ("tackling", "o desarme"),
    ("heading", "o jogo aéreo"),
    ("finishing", "a finalização"),
    ("decisions", "as decisões"),
    ("strength", "a força"),
];
/// Valor a partir do qual um atributo é um ponto forte, e até ao qual é um ponto fraco.
const STRONG: i32 = 15;
const WEAK: i32 = 8;

/// A frase do adjunto no Perfil Geral, pelos atributos que o clube conhece e pela posição
/// ("Rápido e forte no drible, joga sobretudo na direita."); nada sem atributos conhecidos.
fn assistant_note(p: &PersonProfile) -> Option<String> {
    let pl = p.player.as_ref()?;
    let get = |k: &str| pl.attributes.get(k).copied().flatten();
    let keeper = pl.positions.iter().any(|t| t.role == "GR");
    let list: &[(&str, &str)] = if keeper { &KEEPER_STRENGTHS } else { &STRENGTHS };
    let mut strong: Vec<(i32, &str)> = list.iter().filter_map(|(k, text)| get(k).filter(|v| *v >= STRONG).map(|v| (v, *text))).collect();
    strong.sort_by_key(|s| std::cmp::Reverse(s.0));
    if pl.attributes.values().filter(|v| v.is_some()).count() < 10 {
        return None;
    }
    let mut out = match strong.as_slice() {
        [] => "Um jogador sem nada que se destaque".to_string(),
        [(_, a)] => capitalise(a),
        [(_, a), (_, b), ..] => format!("{} e {b}", capitalise(a)),
    };
    // A posição em que mais joga, pelo lado
    let side = pl.positions.iter().find(|t| t.natural).and_then(|t| t.side.as_deref()).map(|s| match s {
        "D" => "na direita",
        "E" => "na esquerda",
        _ => "pelo meio",
    });
    if let (Some(side), false) = (side, keeper) {
        out.push_str(&format!(", joga sobretudo {side}"));
    }
    if !keeper && let Some((_, what)) = WEAKNESSES.iter().filter_map(|(k, w)| get(k).filter(|v| *v <= WEAK).map(|v| (v, *w))).min() {
        out.push_str(&format!("; tem de melhorar {what}"));
    }
    out.push('.');
    Some(out)
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

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

/// Os golos esperados de cada jogador em cada jogo (save 62, marco 4.3).
pub(super) const XG_SCHEMA: &str = "
ALTER TABLE player_matches ADD COLUMN xg REAL NOT NULL DEFAULT 0;
ALTER TABLE friendly_apps ADD COLUMN xg REAL NOT NULL DEFAULT 0;
ALTER TABLE intl_apps ADD COLUMN xg REAL NOT NULL DEFAULT 0;
";

/// Golos esperados com 2 casas, como ficam gravados.
pub(super) fn xg2(v: f64) -> f64 {
    (v.max(0.0) * 100.0).round() / 100.0
}

impl Game {
    /// No fim do dia: a linha de cada jogador que jogou (nota, golos, assistências, passes,
    /// cortes, dribles, remates); o homem do jogo é a melhor nota dos dois lados.
    pub(super) fn record_match(&self, fixture: i32, competition: i32, clubs: [i32; 2], lines: [Vec<PlayerLine>; 2]) -> Result<()> {
        let mom = lines.iter().flatten().max_by(|a, b| a.rating.total_cmp(&b.rating)).map(|l| l.id);
        // Os amigáveis ficam à parte ("Não Competitivos") e não pagam prémios
        let friendly = competition == super::friendlies::CLUB_FRIENDLY;
        // Os prémios por golo, assistência e jogo sem sofrer golos dos contratos do treinador
        for side in 0..2 {
            let conceded: u32 = lines[1 - side].iter().map(|l| l.goals as u32).sum();
            if !friendly {
                self.pay_match_bonuses(clubs[side], &lines[side], conceded)?;
            }
        }
        let table = if friendly { "friendly_apps" } else { "player_matches" };
        let mut insert = self.world.conn.prepare_cached(&format!(
            "INSERT INTO {table} (person_id, fixture, date, competition, club, rating, goals, assists, sub, mom,
                                  passes, passes_ok, tackles, dribbles, shots, on_target, minute_on, minute_off, cards, xg)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        ))?;
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
                    l.stats.on_target,
                    l.on,
                    l.off,
                    if l.red { 2 } else { l.yellow as i32 },
                    xg2(l.stats.xg)
                ])?;
            }
        }
        Ok(())
    }

    /// A ficha de uma partida jogada, pelos jogos gravados de cada jogador.
    pub fn match_sheet(&self, fixture: i32) -> Result<MatchSheet> {
        // Os jogos das reservas têm o id negativo (o do jogo do campeonato de que saem)
        if fixture < 0 {
            return self.reserve_sheet(-fixture);
        }
        let m = self.matches.iter().find(|m| m.id == fixture).ok_or_else(|| Error::NotFound(format!("jogo {fixture}")))?;
        if m.result.is_none() {
            return Err(Error::Invalid("Este jogo ainda não se jogou.".into()));
        }
        type Row = (i32, i32, String, Option<i32>, f64, i32, i32, bool, Option<i32>, Option<i32>, i32, bool, f64);
        // Os jogos de selecções ficam em `intl_apps`, com a selecção no lugar do clube
        let national = super::internationals::is_national_comp(m.competition_id);
        let (table, side) = if national {
            ("intl_apps", "team")
        } else if m.competition_id == super::friendlies::CLUB_FRIENDLY {
            ("friendly_apps", "club")
        } else {
            ("player_matches", "club")
        };
        let sql = format!(
            "SELECT m.person_id, m.{side}, COALESCE((SELECT nickname FROM nicknames n WHERE n.person_id = p.id), p.list_name),
                    pl.squad_number, ROUND(m.rating, 1), m.goals, m.assists, m.mom, m.minute_on, m.minute_off, m.cards, m.sub, m.xg
             FROM {table} m JOIN people p ON p.id = m.person_id LEFT JOIN players pl ON pl.person_id = p.id
             WHERE m.fixture = ? ORDER BY m.sub, m.rowid"
        );
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare_cached(&sql)?
            .query_map([fixture], |r| {
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
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        let (mut home, mut away) = (Vec::new(), Vec::new());
        for (id, club, name, number, rating, goals, assists, mom, on, off, cards, sub, xg) in rows {
            let line = SheetLine {
                id,
                name,
                // O número do clube não é o da selecção
                number: number.filter(|_| !national),
                rating,
                goals,
                assists,
                mom,
                on: on.or((!sub).then_some(0)),
                off,
                yellow: cards == 1,
                red: cards == 2,
                xg,
            };
            if club == m.home_id { home.push(line) } else { away.push(line) }
        }
        Ok(MatchSheet { fixture: self.fixture(m), home, away })
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

    /// Primeiro dia da época do clube (civil ou europeia), para as estatísticas da época.
    pub(super) fn season_start(&self, club: Option<i32>) -> Result<String> {
        let kind = club.map(|c| self.kind_of(c)).transpose()?.unwrap_or(SeasonKind::European);
        let year: i32 = self.date[0..4].parse().unwrap_or(0);
        Ok(match kind {
            SeasonKind::Calendar => format!("{year}-01-01"),
            SeasonKind::European | SeasonKind::International if &self.date[5..] >= "07-01" => format!("{year}-07-01"),
            SeasonKind::European | SeasonKind::International => format!("{}-07-01", year - 1),
        })
    }

    /// As últimas notas de um jogador (a Forma), da mais antiga à mais recente.
    pub(super) fn form_of(&self, person: i32) -> Result<Vec<f64>> {
        let mut form: Vec<f64> = self
            .world
            .conn
            .prepare_cached("SELECT rating FROM player_matches WHERE person_id = ? ORDER BY date DESC, fixture DESC LIMIT ?")?
            .query_map(params![person, FORM_GAMES as i64], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        form.reverse();
        Ok(form)
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
        p.form = self.form_of(p.id)?;
        let start = self.season_start(p.club.as_ref().map(|c| c.id))?;
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
        self.fill_marks(p)?;
        self.fill_personal(p)?;
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
                    transfer: None,
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
            // O rótulo pela divisão do clube ("2024/25" numa liga europeia)
            let division = c.club.as_ref().map(|x| self.division_of(x.id)).transpose()?.flatten();
            out.push(SeasonRecord {
                label: division.map_or(year.to_string(), |d| regulation::season_label(d, year)),
                year,
                club: c.club.clone(),
                on_loan: c.on_loan,
                stats: StatLine { apps: c.apps, goals: c.goals, ..Default::default() },
                by_kind: Vec::new(),
                transfer: None,
            });
        }
        // A época em curso no clube atual aparece sempre, mesmo sem jogos (como no CM)
        if let Some(club) = &p.club
            && let Some(division) = self.division_of(club.id)?
        {
            let year = regulation::season_of(division, &self.date);
            if !out.iter().any(|s| s.year == year && s.club.as_ref().is_some_and(|x| x.id == club.id)) {
                let mut by_kind: Vec<(String, StatLine)> = KINDS.iter().map(|k| (k.to_string(), StatLine::default())).collect();
                by_kind.push(("Jogos Oficiais".into(), StatLine::default()));
                out.insert(
                    0,
                    SeasonRecord {
                        label: regulation::season_label(division, year),
                        year,
                        club: Some(club.clone()),
                        on_loan,
                        stats: StatLine::default(),
                        by_kind,
                        transfer: None,
                    },
                );
            }
        }
        // Os jogos das reservas e os amigáveis: "Não Competitivos", à cabeça e fora dos Jogos
        // Oficiais (como no CM)
        let reserve: Vec<(i32, String, i32, i32, f64)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT club, date, goals, assists, rating FROM reserve_apps WHERE person_id = ?1
                 UNION ALL SELECT club, date, goals, assists, rating FROM friendly_apps WHERE person_id = ?1 ORDER BY 2",
            )?
            .query_map([p.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut friendly: BTreeMap<(i32, i32), Totals> = BTreeMap::new();
        for (club, day, goals, assists, rating) in reserve {
            let Some(division) = self.division_of(club)? else { continue };
            let t = friendly.entry((regulation::season_of(division, &day), club)).or_default();
            t.add(false, false, goals, assists, 0, 0, 0, 0, 0, 0, rating);
        }
        for ((year, club), t) in friendly {
            if let Some(s) =
                out.iter_mut().find(|s| s.year == year && s.club.as_ref().is_some_and(|c| c.id == club) && !s.by_kind.is_empty())
            {
                s.by_kind.insert(0, ("Não Competitivos".into(), t.line()));
            }
        }
        // Os jogos pelas selecções: "Internacionais" na época do clube em que estava, fora dos
        // Jogos Oficiais do clube
        let intl: Vec<(Option<i32>, String, i32, i32, f64)> = self
            .world
            .conn
            .prepare_cached("SELECT club, date, goals, assists, rating FROM intl_apps WHERE person_id = ? ORDER BY date")?
            .query_map([p.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let mut caps: BTreeMap<(i32, i32), Totals> = BTreeMap::new();
        for (club, day, goals, assists, rating) in intl {
            let Some(club) = club else { continue };
            let Some(division) = self.division_of(club)? else { continue };
            let t = caps.entry((regulation::season_of(division, &day), club)).or_default();
            t.add(false, false, goals, assists, 0, 0, 0, 0, 0, 0, rating);
        }
        for ((year, club), t) in caps {
            if let Some(s) =
                out.iter_mut().find(|s| s.year == year && s.club.as_ref().is_some_and(|c| c.id == club) && !s.by_kind.is_empty())
            {
                s.by_kind.push(("Internacionais".into(), t.line()));
            }
        }
        // Como chegou a cada clube durante o jogo: a transferência na linha da época
        let moves: Vec<(i32, String, String, i64)> = self
            .world
            .conn
            .prepare_cached("SELECT to_club, date, kind, fee FROM transfers WHERE person_id = ? AND to_club IS NOT NULL ORDER BY date")?
            .query_map([p.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (club, day, kind, fee) in moves {
            let Some(division) = self.division_of(club)? else { continue };
            let year = regulation::season_of(division, &day);
            let text = match kind.as_str() {
                "loan" => "Empréstimo".to_string(),
                "transfer" if fee > 0 => money_text(fee),
                "return" => continue,
                _ => "Livre".to_string(),
            };
            if let Some(s) = out.iter_mut().find(|s| s.year == year && s.club.as_ref().is_some_and(|c| c.id == club)) {
                s.transfer = Some(text);
            }
        }
        p.seasons = out;
        p.assistant_note = assistant_note(p);
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
    pub(super) fn comp_kind(&self, comp: i32) -> &'static str {
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
    fn the_assistant_sums_up_a_player() {
        assert_eq!(capitalise("rápido"), "Rápido");
    }

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
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
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
        // A frase do adjunto (um jogador do plantel: o clube conhece-o)
        let note = p.assistant_note.clone().unwrap();
        assert!(note.ends_with('.') && note.chars().next().unwrap().is_uppercase(), "{note}");
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
