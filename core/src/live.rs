//! Match Engine v2 (marco 3.4): o jogo minuto a minuto, com lances. A cada minuto, os
//! golos esperados do motor v1 (`engine::expected_goals`, com a força do que está em campo
//! naquele minuto) viram remates; cada remate é golo, defesa do guarda-redes ou falhado.
//! Também há faltas, cartões (dois amarelos = vermelho), lesões, cansaço, substituições
//! do adjunto e notas dos jogadores. Tudo com o RNG da partida: mesma seed = mesmo jogo.
//!
//! O jogo do treinador corre por partes (`play_until`), e entre elas ele pode substituir
//! e mudar as instruções; sem mexer em nada, o resultado é o mesmo que jogá-lo de seguida.
//!
//! No jogo que o treinador vê (`detailed`), um segundo RNG faz o "enchimento" das telas
//! do CM: passes, cortes, cabeceamentos, lançamentos, foras de jogo, zonas de acção e o
//! relato dos minutos sem lances. Esse RNG nunca toca no resultado.

use rand_chacha::ChaCha8Rng;
use rand_core::RngCore;
use serde::Serialize;
use ts_rs::TS;

use crate::engine::{SlotRating, TeamStrength, condition_factor, expected_goals, expected_goals_neutral, morale_factor, sector, uniform};
use crate::league::Fixture;
use crate::person::ClubRef;
use crate::personality;
use crate::squad::Side;
use crate::tactics::{
    Availability, ClubTactics, Instructions, PlayerOrders, Selection, SquadState, Tackling, TacticPlayer, fit_for_role, orders_of,
};
use crate::{Error, Result, World, date};

/// Substituições por jogo (regra actual do Brasileirão).
pub const MAX_SUBS: i32 = 5;
/// Remates de uma equipa média por jogo (mais para quem tem mais golos esperados).
const SHOTS_PER_90: f64 = 12.0;
/// Fração dos remates que vão à baliza (golos incluídos).
const ON_TARGET: f64 = 0.36;
const FOULS_PER_90: f64 = 13.0;
/// Amarelos e vermelhos directos por equipa e jogo (antes da agressividade e do Desarme).
const YELLOWS_PER_90: f64 = 1.9;
const REDS_PER_90: f64 = 0.025;
/// Cantos que não vêm de remates.
const CORNERS_PER_90: f64 = 1.5;
/// Quem ganha por 3 ou mais tira o pé: os seus golos esperados caem para 65%. Segura as
/// goleadas de 6+ entre equipas desiguais e quase não mexe na média de golos.
const LEAD_EASE: (u8, f64) = (3, 0.65);
/// Minutos em que o adjunto pensa numa substituição (46 = ao intervalo).
const AI_SUB_MINUTES: [u8; 5] = [46, 58, 66, 74, 82];
/// Substituições tácticas do adjunto (as outras ficam para lesões).
const AI_TACTICAL_SUBS: i32 = 3;

/// Atributos (1–20 do banco) que contam nos lances.
#[derive(Debug, Clone)]
struct Profile {
    /// Nome curto para o relato (o "nome comum" ou o apelido).
    name: String,
    pos: [Option<i32>; 12],
    finishing: f64,
    long_shots: f64,
    heading: f64,
    passing: f64,
    vision: f64,
    stamina: f64,
    injury_proneness: f64,
    tackling: f64,
    jumping: f64,
    /// Agressividade + Maldade.
    temper: f64,
    /// Consistência: quanto o rendimento varia de jogo para jogo.
    consistency: f64,
    /// Jogos Importantes + Pressão: como reage nos jogos decisivos.
    nerve: f64,
    determination: f64,
    versatility: f64,
    /// Peso nos vermelhos directos (Temperamento).
    hothead: f64,
    /// Adaptação de um estrangeiro acabado de chegar ao clube (1 = adaptado).
    settle: f64,
}

/// Atributos de um jogador para um jogo no dia `date`.
fn load_profile(world: &World, id: i32, date: &str) -> Result<Profile> {
    let mut stmt = world.conn.prepare_cached(
        "SELECT COALESCE(NULLIF(p.common_name, ''), NULLIF(p.second_name, ''), p.display_name),
                pl.pos_goalkeeper, pl.pos_sweeper, pl.pos_defender, pl.pos_defensive_midfielder,
                pl.pos_midfielder, pl.pos_attacking_midfielder, pl.pos_attacker, pl.pos_wing_back,
                pl.pos_right_side, pl.pos_left_side, pl.pos_central, pl.pos_free_role,
                pl.finishing, pl.long_shots, pl.heading, pl.passing, pl.vision, pl.stamina,
                pl.injury_proneness, pl.aggression, pl.dirtiness, pl.tackling, pl.jumping,
                pl.consistency, pl.important_matches, p.pressure, p.determination,
                pl.versatility, p.sportsmanship, p.temperament, p.adaptability, p.club_joined,
                -- Estrangeiro que chegou ao clube durante o jogo (o banco traz datas de entrada fictícias)
                p.nation_id <> c.nation_id AND COALESCE(p.second_nation_id, 0) <> c.nation_id
                  AND EXISTS (SELECT 1 FROM transfers t WHERE t.date = p.club_joined AND t.person_id = p.id AND t.to_club = p.club_id)
         FROM people p JOIN players pl ON pl.person_id = p.id LEFT JOIN clubs c ON c.id = p.club_id WHERE p.id = ?",
    )?;
    let today = date::parse(date).unwrap_or(0);
    Ok(stmt.query_row([id], |r| {
        let mut pos = [None; 12];
        for (i, slot) in pos.iter_mut().enumerate() {
            *slot = r.get(1 + i)?;
        }
        let v = |i: usize| r.get::<_, Option<i32>>(i).map(|v| v.unwrap_or(10).clamp(1, 20) as f64);
        let settle = match (r.get::<_, Option<bool>>(33)?.unwrap_or(false), r.get::<_, Option<String>>(32)?) {
            (true, Some(joined)) => {
                let days = today - date::parse(&joined).unwrap_or(today);
                personality::adaptation_factor(days, v(31)?)
            }
            _ => 1.0,
        };
        Ok(Profile {
            name: r.get(0)?,
            pos,
            finishing: v(13)?,
            long_shots: v(14)?,
            heading: v(15)?,
            passing: v(16)?,
            vision: v(17)?,
            stamina: v(18)?,
            injury_proneness: v(19)?,
            temper: (v(20)? + v(21)?) * personality::sportsmanship_factor(v(29)?),
            tackling: v(22)?,
            jumping: v(23)?,
            // Vazios no patch (o CM gera-os ao começar): contam como 10
            consistency: v(24)?,
            nerve: (v(25)? + v(26)?) / 2.0,
            determination: v(27)?,
            versatility: v(28)?,
            hothead: personality::temperament_red_factor(v(30)?),
            settle,
        })
    })?)
}

#[derive(Debug, Clone)]
struct MatchPlayer {
    info: TacticPlayer,
    prof: Profile,
    /// Função (índice em `tactics.starters`) onde está agora; `None` fora de campo.
    slot: Option<usize>,
    /// Nota (1–20) na função onde está.
    fit: i32,
    condition: f64,
    morale: i32,
    /// Minuto em que entrou (0 = titular); `None` = suplente por usar.
    entered: Option<u8>,
    left: Option<u8>,
    yellow: bool,
    red: bool,
    injured: bool,
    goals: u8,
    assists: u8,
    /// Nota pelos lances (sem o resultado, que conta só na exibição).
    rating: f64,
    stats: PlayerStats,
    /// Forma do dia (Consistência) e o efeito de um jogo decisivo (Jogos Importantes e Pressão).
    form: f64,
}

/// Números de um jogador no jogo (a tela "Dados" do CM).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStats {
    pub passes: i32,
    pub passes_ok: i32,
    pub tackles: i32,
    pub tackles_won: i32,
    pub headers: i32,
    pub headers_won: i32,
    pub shots: i32,
    pub on_target: i32,
}

impl MatchPlayer {
    fn value(&self) -> f64 {
        value(&self.info, &self.prof, self.fit, self.condition, self.morale) * self.form
    }
}

/// Forma do dia: rendimento à volta de 100% com desvio de 2% (Consistência 20) a 6% (1),
/// entre 85% e 115%.
fn day_form(consistency: f64, rng: &mut impl RngCore) -> f64 {
    let sd = 0.02 + 0.04 * (20.0 - consistency.clamp(1.0, 20.0)) / 19.0;
    // Normal pelo método de Box-Muller
    let (u1, u2) = (uniform(rng).max(1e-12), uniform(rng));
    let z = (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos();
    (1.0 + sd * z).clamp(0.85, 1.15)
}

/// Num jogo decisivo: de -4% (Jogos Importantes e Pressão a 1) a +4% (a 20).
fn nerve_factor(nerve: f64) -> f64 {
    1.0 + 0.04 * (nerve.clamp(1.0, 20.0) - 10.5) / 9.5
}

/// O que rodeia um jogo além das equipas: decisivo ou não, a torcida e o peso da camisa.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Occasion {
    /// Eliminatória ou clássico (rivais).
    pub big: bool,
    /// Golos esperados da casa × isto (estádio: ±3,75% entre 10 mil e 70 mil lugares).
    pub crowd: f64,
    /// Reputação da casa menos a de fora, de -1 a 1: ±3% nos golos esperados de cada lado.
    pub shirt: f64,
}

impl Default for Occasion {
    fn default() -> Self {
        Occasion { big: false, crowd: 1.0, shirt: 0.0 }
    }
}

impl Occasion {
    /// A partir da lotação do estádio da casa e das reputações (0–10 000 no banco).
    pub fn new(big: bool, neutral: bool, capacity: Option<i32>, home_rep: i32, away_rep: i32) -> Occasion {
        let crowd = if neutral {
            1.0
        } else {
            let fill = (capacity.unwrap_or(15_000) as f64 / 40_000.0).clamp(0.25, 1.75);
            1.0 + 0.05 * (fill - 1.0)
        };
        let shirt = ((home_rep - away_rep) as f64 / 4000.0).clamp(-1.0, 1.0);
        Occasion { big, crowd, shirt }
    }
}

/// Golos esperados × isto para quem está a perder: de -4% (Determinação 1) a +4% (20).
fn determination_factor(determination: f64) -> f64 {
    1.0 + 0.04 * (determination.clamp(1.0, 20.0) - 10.5) / 9.5
}

/// Quanto um jogador rende numa função com uma nota e uma condição (a Versatilidade conta
/// fora da posição, a adaptação nos estrangeiros acabados de chegar).
fn value(p: &TacticPlayer, prof: &Profile, fit: i32, condition: f64, morale: i32) -> f64 {
    p.current_ability.unwrap_or(1) as f64
        * personality::versatility_fit(fit, prof.versatility)
        * condition_factor(condition)
        * morale_factor(morale)
        * prof.settle
}

/// Estatísticas de uma equipa no jogo.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct TeamStats {
    /// Posse de bola em % (as duas somam 100).
    pub possession: i32,
    pub shots: i32,
    pub on_target: i32,
    pub corners: i32,
    pub fouls: i32,
    pub yellows: i32,
    pub reds: i32,
    // Só no jogo que o treinador vê (a zero nos outros)
    pub throw_ins: i32,
    pub offsides: i32,
    pub passes: i32,
    pub passes_ok: i32,
    pub tackles: i32,
    pub tackles_won: i32,
    pub headers: i32,
    pub headers_won: i32,
}

#[derive(Debug, Clone)]
struct Team {
    club: ClubRef,
    /// Funções, setas, formação e instruções (os jogadores estão em `players`).
    tactics: ClubTactics,
    /// Titulares primeiro, depois os suplentes.
    players: Vec<MatchPlayer>,
    subs: i32,
    tactical_subs: i32,
    /// O adjunto faz as substituições tácticas (as das lesões faz sempre).
    ai: bool,
    stats: TeamStats,
    /// Soma da posse minuto a minuto (fração de 0 a 1).
    possession: f64,
}

impl Team {
    fn new(world: &World, club: ClubRef, tactics: ClubTactics, ai: bool, date: &str) -> Result<Team> {
        let mut players = Vec::new();
        let state = |p: &TacticPlayer| (p.condition.unwrap_or(100) as f64, p.morale.unwrap_or(12));
        for (s, slot) in tactics.starters.iter().enumerate() {
            if let Some(p) = &slot.player {
                let (condition, morale) = state(p);
                players.push(MatchPlayer {
                    info: p.clone(),
                    prof: load_profile(world, p.id, date)?,
                    slot: Some(s),
                    fit: p.fit.unwrap_or(10),
                    condition,
                    morale,
                    entered: Some(0),
                    left: None,
                    yellow: false,
                    red: false,
                    injured: false,
                    goals: 0,
                    assists: 0,
                    rating: 0.0,
                    stats: PlayerStats::default(),
                    form: 1.0,
                });
            }
        }
        for p in &tactics.bench {
            let (condition, morale) = state(p);
            players.push(MatchPlayer {
                info: p.clone(),
                prof: load_profile(world, p.id, date)?,
                slot: None,
                fit: 1,
                condition,
                morale,
                entered: None,
                left: None,
                yellow: false,
                red: false,
                injured: false,
                goals: 0,
                assists: 0,
                rating: 0.0,
                stats: PlayerStats::default(),
                form: 1.0,
            });
        }
        Ok(Team { club, tactics, players, subs: 0, tactical_subs: 0, ai, stats: TeamStats::default(), possession: 0.0 })
    }

    fn role(&self, slot: usize) -> &str {
        &self.tactics.starters[slot].role
    }

    /// Quem está em campo, pela ordem das funções.
    fn on_pitch(&self) -> Vec<usize> {
        let mut out: Vec<usize> = (0..self.players.len()).filter(|&i| self.players[i].slot.is_some()).collect();
        out.sort_by_key(|&i| self.players[i].slot);
        out
    }

    fn strength(&self) -> TeamStrength {
        let on = self.on_pitch();
        let slots: Vec<SlotRating> = on
            .iter()
            .map(|&i| {
                let p = &self.players[i];
                let s = &self.tactics.starters[p.slot.unwrap_or(0)];
                SlotRating { role: &s.role, y: s.y, run: s.run, rating: p.value() }
            })
            .collect();
        let missing = self.tactics.starters.len().saturating_sub(on.len());
        TeamStrength::from_slots(&slots, missing, &self.tactics.instructions)
    }

    fn goalkeeper(&self) -> Option<usize> {
        self.on_pitch().into_iter().find(|&i| self.players[i].slot.is_some_and(|s| sector(self.role(s)) == 0))
    }

    /// Sorteia alguém em campo com o peso dado (função, jogador).
    fn pick(&self, rng: &mut ChaCha8Rng, exclude: Option<usize>, weight: impl Fn(&str, &MatchPlayer) -> f64) -> Option<usize> {
        let cands: Vec<(usize, f64)> = self
            .on_pitch()
            .into_iter()
            .filter(|&i| Some(i) != exclude)
            .map(|i| (i, weight(self.role(self.players[i].slot.unwrap_or(0)), &self.players[i])))
            .filter(|&(_, w)| w > 0.0)
            .collect();
        let total: f64 = cands.iter().map(|c| c.1).sum();
        if total <= 0.0 {
            return None;
        }
        let mut x = uniform(rng) * total;
        for &(i, w) in &cands {
            if x < w {
                return Some(i);
            }
            x -= w;
        }
        cands.last().map(|c| c.0)
    }

    /// As instruções individuais de um jogador do jogo.
    fn orders(&self, i: usize) -> PlayerOrders {
        orders_of(&self.tactics.orders, self.players[i].info.id)
    }

    /// Quem bate (penáltis, livres, cantos): o escolhido nas Tácticas, se estiver em campo;
    /// senão o adjunto escolhe o melhor jogador de campo para isso.
    fn taker(&self, chosen: Option<i32>, attr: impl Fn(&Profile) -> f64) -> Option<usize> {
        let on = self.on_pitch();
        if let Some(i) = on.iter().copied().find(|&i| Some(self.players[i].info.id) == chosen) {
            return Some(i);
        }
        on.into_iter()
            .filter(|&i| self.players[i].slot.is_some_and(|s| sector(self.role(s)) != 0))
            .max_by(|&a, &b| attr(&self.players[a].prof).total_cmp(&attr(&self.players[b].prof)).then(b.cmp(&a)))
    }

    /// Suplentes por usar.
    fn bench(&self) -> Vec<usize> {
        (0..self.players.len()).filter(|&i| self.players[i].entered.is_none()).collect()
    }

    /// O melhor suplente para uma função e quanto renderia lá.
    fn best_sub(&self, slot: usize) -> Option<(usize, f64)> {
        let role = self.role(slot);
        self.bench()
            .into_iter()
            .map(|i| {
                let p = &self.players[i];
                (i, value(&p.info, &p.prof, fit_for_role(&p.prof.pos, role), p.condition, p.morale))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Troca `out` (em campo) por `inn` (no banco) na mesma função.
    fn substitute(&mut self, out: usize, inn: usize, minute: u8, rating: f64) {
        let slot = self.players[out].slot.take();
        self.players[out].left = Some(minute);
        let role = slot.map(|s| self.role(s).to_string()).unwrap_or_default();
        let p = &mut self.players[inn];
        p.slot = slot;
        p.fit = fit_for_role(&p.prof.pos, &role);
        p.entered = Some(minute);
        p.rating = rating;
        self.subs += 1;
    }
}

/// Um lance do relato.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct MatchEvent {
    pub minute: i32,
    /// "start", "goal", "save", "miss", "yellow", "red", "injury", "sub", "tactics", "half", "end"
    pub kind: String,
    /// 0 = casa, 1 = fora; `None` nos avisos do árbitro.
    pub side: Option<i32>,
    pub player_id: Option<i32>,
    /// Resumo numa frase (notícias, testes).
    pub text: String,
    /// O lance contado passo a passo, como no relato do CM.
    pub lines: Vec<String>,
}

/// Um jogador na tela da partida.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LivePlayer {
    pub id: i32,
    /// "Sobrenome, Nome", como nas listas.
    pub list_name: String,
    pub squad_number: Option<i32>,
    /// Função em campo ("D C"); `None` no banco ou depois de sair.
    pub role: Option<String>,
    pub position: String,
    pub condition: i32,
    /// Nota de 1 a 10 (só de quem já jogou).
    pub rating: Option<i32>,
    pub goals: i32,
    pub assists: i32,
    pub yellow: bool,
    pub red: bool,
    pub injured: bool,
    /// Nota (1–20) na função onde está; fora da posição abaixo de 15.
    pub fit: Option<i32>,
    pub on_minute: Option<i32>,
    pub off_minute: Option<i32>,
    /// Lugar no campo (0–100) de quem está em campo.
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub stats: PlayerStats,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LiveTeam {
    pub club: ClubRef,
    pub formation: String,
    pub instructions: Instructions,
    /// Em campo, pela ordem das funções.
    pub players: Vec<LivePlayer>,
    /// Suplentes por usar.
    pub bench: Vec<LivePlayer>,
    /// Quem já saiu (substituído, expulso ou lesionado).
    pub out: Vec<LivePlayer>,
    pub subs_left: i32,
    pub stats: TeamStats,
}

/// Outro jogo do dia, com o resultado no minuto em que vai o jogo do treinador.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ScoreLine {
    pub fixture: Fixture,
    pub home_goals: i32,
    pub away_goals: i32,
}

/// Tudo o que a tela da partida mostra.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LiveView {
    pub fixture: Fixture,
    pub minute: i32,
    /// No intervalo (45' jogados, segunda parte por começar).
    pub half_time: bool,
    pub finished: bool,
    pub home_goals: i32,
    pub away_goals: i32,
    /// Lado do treinador (0 casa, 1 fora).
    pub user_side: Option<i32>,
    pub home: LiveTeam,
    pub away: LiveTeam,
    pub events: Vec<MatchEvent>,
    pub others: Vec<ScoreLine>,
    /// Posse da equipa da casa nos últimos 5 minutos (%).
    pub recent_possession: i32,
    /// Onde andou a bola (%): terço defensivo da casa, meio-campo, terço defensivo de fora.
    pub zones: [i32; 3],
    /// Posse da casa em cada terço (%).
    pub zone_possession: [i32; 3],
    /// Tempo útil de jogo, em segundos.
    pub useful_time: i32,
    // Preenchidos pelo jogo (`Game::live_view`)
    pub venue: Option<String>,
    pub city: Option<String>,
    pub weather: String,
    pub referee: String,
    pub attendance: i32,
    /// Classificação com os resultados do dia no minuto em que vai o jogo.
    pub table: Vec<crate::league::StandingRow>,
}

/// Uma partida a decorrer.
#[derive(Debug, Clone)]
pub struct LiveMatch {
    pub fixture: Fixture,
    teams: [Team; 2],
    user: Option<usize>,
    minute: u8,
    score: [u8; 2],
    events: Vec<MatchEvent>,
    /// (lado, minuto, jogador) de cada golo.
    goals: Vec<(usize, u8, i32)>,
    rng: ChaCha8Rng,
    /// RNG do enchimento (só no jogo que o treinador vê).
    detail: Option<ChaCha8Rng>,
    /// Posse da casa em cada minuto.
    ball: Vec<f64>,
    zones: [f64; 3],
    zone_home: [f64; 3],
    useful: f64,
    occasion: Occasion,
}

/// Escolhe uma variante do texto sem gastar o RNG (o relato não muda o jogo).
fn variant(n: usize, minute: u8, id: i32) -> usize {
    (minute as usize * 31 + id.unsigned_abs() as usize * 7) % n
}

impl LiveMatch {
    /// `user`: o lado do treinador; `user_ai`: o adjunto também faz as substituições dele.
    pub fn new(
        world: &World,
        fixture: Fixture,
        home: ClubTactics,
        away: ClubTactics,
        user: Option<usize>,
        user_ai: bool,
        rng: ChaCha8Rng,
    ) -> Result<LiveMatch> {
        let ai = |side| user != Some(side) || user_ai;
        let teams = [
            Team::new(world, fixture.home.clone(), home, ai(0), &fixture.date)?,
            Team::new(world, fixture.away.clone(), away, ai(1), &fixture.date)?,
        ];
        let mut m = LiveMatch {
            fixture,
            teams,
            user,
            minute: 0,
            score: [0, 0],
            events: Vec::new(),
            goals: Vec::new(),
            rng,
            detail: None,
            ball: Vec::new(),
            zones: [0.0; 3],
            zone_home: [0.0; 3],
            useful: 0.0,
            occasion: Occasion::default(),
        };
        // Forma do dia de cada titular: a nota parte de 6 ± 0,4
        for side in 0..2 {
            for i in 0..m.teams[side].players.len() {
                if m.teams[side].players[i].entered.is_some() {
                    m.teams[side].players[i].rating = m.base_rating();
                }
            }
        }
        // E o rendimento do dia de cada jogador (titulares e suplentes), pela Consistência
        for side in 0..2 {
            for i in 0..m.teams[side].players.len() {
                let c = m.teams[side].players[i].prof.consistency;
                m.teams[side].players[i].form = day_form(c, &mut m.rng);
            }
        }
        Ok(m)
    }

    /// Jogo decisivo, torcida e peso da camisa (sem isto, um jogo como os do 3.4).
    pub fn with_occasion(mut self, occasion: Occasion) -> LiveMatch {
        if occasion.big {
            for team in &mut self.teams {
                for p in &mut team.players {
                    p.form *= nerve_factor(p.prof.nerve);
                }
            }
        }
        self.occasion = occasion;
        self
    }

    /// Liga o enchimento das telas (o jogo que o treinador vê), com um RNG só dele.
    pub fn detailed(mut self, rng: ChaCha8Rng) -> LiveMatch {
        self.detail = Some(rng);
        self
    }

    fn base_rating(&mut self) -> f64 {
        6.0 + (uniform(&mut self.rng) - 0.5) * 0.8
    }

    /// A bola já rolou (antes do apito ainda se grava o jogo).
    pub fn started(&self) -> bool {
        self.minute > 0
    }

    pub fn finished(&self) -> bool {
        self.minute >= 90
    }

    pub fn user_side(&self) -> Option<usize> {
        self.user
    }

    pub fn score(&self) -> (u8, u8) {
        (self.score[0], self.score[1])
    }

    fn short(&self, side: usize) -> &str {
        &self.teams[side].club.short_name
    }

    fn name(&self, side: usize, i: usize) -> String {
        self.teams[side].players[i].prof.name.clone()
    }

    fn push(&mut self, kind: &str, side: Option<usize>, player: Option<i32>, text: String) {
        let lines = vec![text.clone()];
        self.push_lines(kind, side, player, text, lines);
    }

    fn push_lines(&mut self, kind: &str, side: Option<usize>, player: Option<i32>, text: String, lines: Vec<String>) {
        self.events.push(MatchEvent {
            minute: self.minute as i32,
            kind: kind.into(),
            side: side.map(|s| s as i32),
            player_id: player,
            text,
            lines,
        });
    }

    /// Um colega de `i` em campo para contar a jogada (sem gastar o RNG).
    fn mate(&self, side: usize, i: usize) -> String {
        let on: Vec<usize> = self.teams[side]
            .on_pitch()
            .into_iter()
            .filter(|&o| o != i && self.teams[side].players[o].slot.is_some_and(|s| sector(self.teams[side].role(s)) != 0))
            .collect();
        if on.is_empty() {
            return self.short(side).to_string();
        }
        self.name(side, on[variant(on.len(), self.minute, self.teams[side].players[i].info.id + 3)])
    }

    /// Joga até ao minuto `to` (no máximo 90).
    pub fn play_until(&mut self, to: u8) {
        while self.minute < to.min(90) {
            self.play_minute();
        }
    }

    fn play_minute(&mut self) {
        self.minute += 1;
        let m = self.minute;
        if m == 1 {
            let text = format!("Começa o jogo entre o {} e o {}.", self.short(0), self.short(1));
            self.push("start", None, None, text);
        }
        if m == 46 {
            self.push("start", None, None, "Começa a segunda parte.".into());
        }
        if AI_SUB_MINUTES.contains(&m) {
            for side in 0..2 {
                self.ai_change(side);
            }
        }

        let st = [self.teams[0].strength(), self.teams[1].strength()];
        let (xh, xa) = if self.fixture.neutral { expected_goals_neutral(&st[0], &st[1]) } else { expected_goals(&st[0], &st[1]) };
        let mid = st[0].midfield / (st[0].midfield + st[1].midfield);
        let home_ball = (0.5 + (mid - 0.5) * 1.5).clamp(0.3, 0.7);
        self.teams[0].possession += home_ball;
        self.teams[1].possession += 1.0 - home_ball;
        self.ball.push(home_ball);
        let before = self.events.len();
        // Torcida e peso da camisa
        let o = self.occasion;
        let (xh, xa) = (xh * o.crowd * (1.0 + 0.03 * o.shirt), xa * (1.0 - 0.03 * o.shirt));
        for (side, xg) in [(0, xh), (1, xa)] {
            let ease = if self.score[side] >= self.score[1 - side] + LEAD_EASE.0 { LEAD_EASE.1 } else { 1.0 };
            // Quem está a perder vai à luta conforme a Determinação de quem está em campo
            let chase = if self.score[side] < self.score[1 - side] {
                let on = self.teams[side].on_pitch();
                let det = on.iter().map(|&i| self.teams[side].players[i].prof.determination).sum::<f64>() / on.len().max(1) as f64;
                determination_factor(det)
            } else {
                1.0
            };
            self.attack(side, xg * ease * chase);
        }
        for side in 0..2 {
            self.discipline(side);
            self.injuries(side);
            self.tire(side);
        }
        if let Some(mut r) = self.detail.take() {
            self.flavour(&mut r, home_ball, st[0].attack / (st[0].attack + st[1].defence), self.events.len() == before);
            self.detail = Some(r);
        }

        if m == 45 {
            let text = format!("Intervalo: {} {}-{} {}.", self.short(0), self.score[0], self.score[1], self.short(1));
            self.push("half", None, None, text);
        }
        if m == 90 {
            self.full_time();
        }
    }

    /// Um minuto de ataque de um lado: talvez um remate, e o remate talvez golo.
    fn attack(&mut self, side: usize, xg: f64) {
        let shots90 = (SHOTS_PER_90 * (xg / 1.17).sqrt()).clamp(4.0, 30.0);
        let other = 1 - side;
        if uniform(&mut self.rng) < CORNERS_PER_90 / 90.0 * (shots90 / SHOTS_PER_90) {
            self.teams[side].stats.corners += 1;
        }
        if uniform(&mut self.rng) >= shots90 / 90.0 {
            return;
        }
        let p_goal = (xg / shots90).min(0.6);
        // Instruções individuais e o organizador (que remata menos e passa mais)
        let orders = self.teams[side].tactics.orders.clone();
        let playmaker = self.teams[side].tactics.takers.playmaker;
        let Some(shooter) = self.teams[side].pick(&mut self.rng, None, |role, p| {
            let w = match role.split(' ').next().unwrap_or("") {
                "A" => 3.2,
                "MA" => 2.0,
                "M" => 1.0,
                "MD" | "AL" => 0.6,
                "D" => 0.35,
                "L" => 0.25,
                _ => 0.0,
            };
            let maker = if playmaker == Some(p.info.id) { 0.9 } else { 1.0 };
            w * (4.0 + p.prof.finishing + 0.5 * p.prof.long_shots + 0.3 * p.prof.heading)
                * orders_of(&orders, p.info.id).shot_weight()
                * maker
        }) else {
            return;
        };
        self.teams[side].stats.shots += 1;
        self.teams[side].players[shooter].stats.shots += 1;
        let roll = uniform(&mut self.rng);
        let name = self.name(side, shooter);
        let id = self.teams[side].players[shooter].info.id;
        let m = self.minute;
        let header =
            self.teams[side].players[shooter].prof.heading > self.teams[side].players[shooter].prof.finishing && variant(2, m, id) == 0;
        let action = if header {
            format!("{name} cabeceou")
        } else {
            match variant(5, m, id + 1) {
                0 => format!("{name} arriscou o chute"),
                1 => format!("{name} rematou com o pé direito"),
                2 => format!("{name} rematou de primeira"),
                3 => format!("{name} tentou de longe"),
                _ => format!("{name} rematou com o pé esquerdo"),
            }
        };
        if roll < p_goal {
            // Alguns golos são de bola parada (sem gastar o RNG da partida): o penálti e o
            // livre directo são do batedor; o canto dá a assistência ao marcador de cantos
            let club_id = self.teams[side].club.id;
            let takers = self.teams[side].tactics.takers;
            let set_piece = match variant(14, m, club_id) {
                0 => self.teams[side].taker(takers.penalty, |p| p.finishing + p.nerve).map(|t| (t, true)),
                1 => self.teams[side].taker(takers.free_kick, |p| p.long_shots + p.finishing).map(|t| (t, false)),
                _ => None,
            };
            let (shooter, name, id, action) = match set_piece {
                Some((t, penalty)) => {
                    let t_name = self.name(side, t);
                    let players = &mut self.teams[side].players;
                    players[shooter].stats.shots -= 1;
                    players[t].stats.shots += 1;
                    let action = if penalty { format!("{t_name} bate o penálti") } else { format!("{t_name} bate o livre directo") };
                    (t, t_name, players[t].info.id, action)
                }
                None => (shooter, name, id, action),
            };
            self.score[side] += 1;
            self.goals.push((side, m, id));
            let t = &mut self.teams[side];
            t.stats.on_target += 1;
            t.players[shooter].goals += 1;
            t.players[shooter].rating += 1.0;
            // 70% dos golos têm assistência
            let assist = if uniform(&mut self.rng) < 0.7 {
                self.teams[side].pick(&mut self.rng, Some(shooter), |role, p| {
                    let w = match role.split(' ').next().unwrap_or("") {
                        "MA" => 2.5,
                        "M" => 2.0,
                        "AL" => 1.6,
                        "A" => 1.4,
                        "MD" => 1.0,
                        "D" => 0.6,
                        "L" => 0.5,
                        _ => 0.1,
                    };
                    let maker = if playmaker == Some(p.info.id) { 1.5 } else { 1.0 };
                    w * (4.0 + p.prof.passing + p.prof.vision) * orders_of(&orders, p.info.id).assist_weight() * maker
                })
            } else {
                None
            };
            let corner = assist.is_some() && set_piece.is_none() && variant(6, m, club_id + 1) == 0;
            let assist = match (set_piece, corner) {
                (Some(_), _) => None,
                (None, true) => self.teams[side].taker(takers.corner, |p| p.passing + p.vision).filter(|&c| c != shooter).or(assist),
                _ => assist,
            };
            if let Some(a) = assist {
                self.teams[side].players[a].assists += 1;
                self.teams[side].players[a].rating += 0.6;
            }
            for i in self.teams[other].on_pitch() {
                let t = &mut self.teams[other];
                match t.players[i].slot.map(|s| sector(&t.tactics.starters[s].role)) {
                    Some(0) => t.players[i].rating -= 0.4,
                    Some(1) => t.players[i].rating -= 0.15,
                    _ => {}
                }
            }
            let score = format!("{}-{}", self.score[0], self.score[1]);
            let club = self.short(side).to_string();
            let outcome = match variant(4, m, id) {
                0 => format!("GOLO! {name} marca para o {club}! ({score})"),
                1 => format!("GOLO! Remate certeiro de {name}! {score}"),
                2 => format!("GOLO! {name} não perdoa e faz o {score}!"),
                _ => format!("GOLO do {club}! {name} atira para o fundo da baliza! ({score})"),
            };
            let mut text = outcome.clone();
            let buildup = match (set_piece, assist) {
                (Some((_, true)), _) => {
                    text += " De penálti.";
                    format!("Penálti para o {club}!")
                }
                (Some(_), _) => {
                    text += " De livre directo.";
                    format!("Livre perigoso para o {club} à entrada da área")
                }
                (None, Some(a)) if corner => {
                    let p = self.name(side, a);
                    text += &format!(" Canto de {p}.");
                    format!("{p} bate o canto para a área")
                }
                (None, Some(a)) => {
                    let p = self.name(side, a);
                    text += &format!(" Assistência de {p}.");
                    match variant(4, m, id + 2) {
                        0 => format!("{p} procurou {name} com uma bola na área"),
                        1 => format!("{p} cruzou para a área"),
                        2 => format!("{p} tocou para {name} na entrada da área"),
                        _ => format!("{p} lançou {name} em velocidade"),
                    }
                }
                (None, None) => match variant(3, m, id + 2) {
                    0 => format!("{name} aproveitou a sobra"),
                    1 => format!("{name} ganhou a bola no meio-campo e avançou"),
                    _ => format!("{name} fugiu à marcação"),
                },
            };
            self.push_lines("goal", Some(side), Some(id), text, vec![buildup, action, outcome]);
        } else if roll < ON_TARGET.max(p_goal + 0.1) {
            self.teams[side].stats.on_target += 1;
            self.teams[side].players[shooter].stats.on_target += 1;
            self.teams[side].players[shooter].rating += 0.1;
            let gk = self.teams[other].goalkeeper();
            let keeper = gk.map(|g| self.name(other, g)).unwrap_or_else(|| "a defesa".into());
            if let Some(g) = gk {
                self.teams[other].players[g].rating += 0.25;
            }
            if uniform(&mut self.rng) < 0.3 {
                self.teams[side].stats.corners += 1;
            }
            let text = match variant(4, m, id) {
                0 => format!("{name} remata... defende {keeper}!"),
                1 => format!("Grande defesa de {keeper} ao remate de {name}!"),
                2 => format!("Remate de {name}, {keeper} agarra sem problemas."),
                _ => format!("{name} obriga {keeper} a uma defesa difícil."),
            };
            let outcome = match variant(4, m, id + 5) {
                0 => format!("{keeper} defendeu bem"),
                1 => format!("{keeper} segurou com firmeza"),
                2 => format!("{keeper} defendeu, mas a bola continuou em jogo"),
                _ => format!("Grande defesa de {keeper}!"),
            };
            let buildup = format!("{} tocou para {name}", self.mate(side, shooter));
            self.push_lines("save", Some(side), Some(id), text, vec![buildup, action, outcome]);
        } else {
            if uniform(&mut self.rng) < 0.25 {
                self.teams[side].stats.corners += 1;
            }
            let text = match variant(4, m, id) {
                0 => format!("{name} remata por cima da barra."),
                1 => format!("{name} atira ao lado."),
                2 => format!("Remate de {name} bloqueado pela defesa."),
                _ => format!("Boa jogada do {}, mas {name} falha o alvo.", self.short(side)),
            };
            let outcome = match variant(4, m, id + 5) {
                0 => "Mas passou por cima do travessão".to_string(),
                1 => "A bola saiu ao lado".to_string(),
                2 => "A defesa bloqueou o remate".to_string(),
                _ => "Rasteiro, ao lado do poste".to_string(),
            };
            let buildup = match variant(3, m, id + 2) {
                0 => format!("{} procurou {name} na área", self.mate(side, shooter)),
                1 => format!("{name} recebeu de costas para a baliza"),
                _ => format!("Boa jogada do {}", self.short(side)),
            };
            self.push_lines("miss", Some(side), Some(id), text, vec![buildup, action, outcome]);
        }
    }

    /// Faltas e cartões: mais com Desarme Duro e jogadores agressivos.
    fn discipline(&mut self, side: usize) {
        let t = &self.teams[side];
        let on = t.on_pitch();
        if on.is_empty() {
            return;
        }
        let tackle: f64 = match t.tactics.instructions.tackling {
            Tackling::Normal => 1.0,
            Tackling::Soft => 0.6,
            Tackling::Hard => 1.4,
        };
        // Jogadores mais agressivos que a média (~21) e menos desportivos dão mais cartões; os
        // de cabeça quente (Temperamento) mais vermelhos directos
        let temper = on.iter().map(|&i| t.players[i].prof.temper).sum::<f64>() / on.len() as f64 / 21.0;
        let hothead = on.iter().map(|&i| t.players[i].prof.hothead).sum::<f64>() / on.len() as f64;
        if uniform(&mut self.rng) < FOULS_PER_90 * tackle.sqrt() / 90.0 {
            self.teams[side].stats.fouls += 1;
        }
        let orders = self.teams[side].tactics.orders.clone();
        let team_tackling = self.teams[side].tactics.instructions.tackling;
        let booking = |role: &str, p: &MatchPlayer| {
            p.prof.temper * if sector(role) == 0 { 0.3 } else { 1.0 } * orders_of(&orders, p.info.id).booking_weight(team_tackling)
        };
        if uniform(&mut self.rng) < YELLOWS_PER_90 * tackle * temper / 90.0
            && let Some(i) = self.teams[side].pick(&mut self.rng, None, booking)
        {
            self.teams[side].stats.fouls += 1;
            let name = self.name(side, i);
            let id = self.teams[side].players[i].info.id;
            // Quem já tem amarelo tem cuidado: metade das vezes escapa ao segundo
            let booked = self.teams[side].players[i].yellow;
            if booked && uniform(&mut self.rng) < 0.5 {
            } else if booked {
                self.teams[side].stats.yellows += 1;
                let text = format!("Segundo amarelo para {name}! O {} fica reduzido a dez.", self.short(side));
                self.push("red", Some(side), Some(id), text);
                self.send_off(side, i);
            } else {
                let p = &mut self.teams[side].players[i];
                p.yellow = true;
                p.rating -= 0.3;
                self.teams[side].stats.yellows += 1;
                let text = match variant(3, self.minute, id) {
                    0 => format!("Cartão amarelo para {name} ({}).", self.short(side)),
                    1 => format!("Entrada dura de {name}, que vê o amarelo."),
                    _ => format!("O árbitro mostra o amarelo a {name}."),
                };
                let lines = vec![format!("{name} entrou duro"), format!("Cartão amarelo para {name}")];
                self.push_lines("yellow", Some(side), Some(id), text, lines);
            }
        }
        let red = |role: &str, p: &MatchPlayer| booking(role, p) * p.prof.hothead;
        if uniform(&mut self.rng) < REDS_PER_90 * tackle * temper * hothead / 90.0
            && let Some(i) = self.teams[side].pick(&mut self.rng, None, red)
        {
            self.teams[side].stats.fouls += 1;
            let name = self.name(side, i);
            let id = self.teams[side].players[i].info.id;
            let text = format!("Cartão vermelho directo para {name}! O {} fica com dez.", self.short(side));
            self.push("red", Some(side), Some(id), text);
            self.send_off(side, i);
        }
    }

    /// Expulsão; sem guarda-redes, o adjunto põe o do banco (no lugar de um jogador de
    /// campo) ou manda um jogador de campo para a baliza.
    fn send_off(&mut self, side: usize, i: usize) {
        let m = self.minute;
        let t = &mut self.teams[side];
        t.stats.reds += 1;
        let slot = t.players[i].slot.take();
        let p = &mut t.players[i];
        p.red = true;
        p.left = Some(m);
        p.rating -= 1.5;
        let Some(slot) = slot.filter(|&s| sector(t.role(s)) == 0) else { return };
        let keeper = t.bench().into_iter().find(|&b| fit_for_role(&t.players[b].prof.pos, "GR") >= 10);
        let outfield = t.on_pitch();
        if let (Some(gk), true) = (keeper, t.subs < MAX_SUBS) {
            // Sai o jogador de campo que menos rende
            let Some(&out) = outfield.iter().min_by(|&&a, &&b| t.players[a].value().total_cmp(&t.players[b].value())) else {
                return;
            };
            let r = self.base_rating();
            let t = &mut self.teams[side];
            t.substitute(out, gk, m, r);
            t.players[gk].slot = Some(slot);
            t.players[gk].fit = fit_for_role(&t.players[gk].prof.pos, "GR");
            let text = format!(
                "Substituição no {}: sai {}, entra o guarda-redes {}.",
                self.short(side),
                self.name(side, out),
                self.name(side, gk)
            );
            let id = self.teams[side].players[gk].info.id;
            self.push("sub", Some(side), Some(id), text);
        } else if let Some(&g) = outfield.iter().max_by_key(|&&o| fit_for_role(&t.players[o].prof.pos, "GR")) {
            t.players[g].slot = Some(slot);
            t.players[g].fit = fit_for_role(&t.players[g].prof.pos, "GR");
            let text = format!("{} vai para a baliza.", self.name(side, g));
            self.push("tactics", Some(side), None, text);
        }
    }

    /// Lesões: 0,6% (Propensão 1) a 1,8% (20) por jogador e jogo inteiro.
    fn injuries(&mut self, side: usize) {
        for i in self.teams[side].on_pitch() {
            let ip = self.teams[side].players[i].prof.injury_proneness;
            let chance = (0.006 + 0.012 * (ip - 1.0) / 19.0) / 90.0;
            if uniform(&mut self.rng) >= chance || self.teams[side].players[i].injured {
                continue;
            }
            self.teams[side].players[i].injured = true;
            let name = self.name(side, i);
            let id = self.teams[side].players[i].info.id;
            let lines = vec![format!("{name} fica caído no relvado"), format!("{name} está lesionado e não pode continuar")];
            self.push_lines("injury", Some(side), Some(id), format!("{name} está lesionado e não pode continuar."), lines);
            let slot = self.teams[side].players[i].slot.unwrap_or(0);
            match self.teams[side].best_sub(slot) {
                Some((sub, _)) if self.teams[side].subs < MAX_SUBS => self.make_sub(side, i, sub),
                // Sem substituições, fica em campo a coxear
                _ => {
                    let p = &mut self.teams[side].players[i];
                    p.condition = p.condition.min(30.0);
                }
            }
        }
    }

    /// Cansaço: 20% (Resistência 20) a 28% (1) num jogo inteiro, mais 3% com Pressão.
    fn tire(&mut self, side: usize) {
        let team_pressing = self.teams[side].tactics.instructions.pressing;
        let pressing = if team_pressing { 3.0 } else { 0.0 };
        for i in self.teams[side].on_pitch() {
            let extra = self.teams[side].orders(i).extra_drain(team_pressing);
            let p = &mut self.teams[side].players[i];
            let drain = 28.0 - (p.prof.stamina - 1.0) * 8.0 / 19.0 + pressing + extra;
            p.condition = (p.condition - drain / 90.0).max(0.0);
        }
    }

    fn make_sub(&mut self, side: usize, out: usize, inn: usize) {
        let r = self.base_rating();
        let m = self.minute;
        self.teams[side].substitute(out, inn, m, r);
        let text = format!("Substituição no {}: sai {}, entra {}.", self.short(side), self.name(side, out), self.name(side, inn));
        let id = self.teams[side].players[inn].info.id;
        self.push("sub", Some(side), Some(id), text);
    }

    /// O adjunto troca o jogador que mais ganha com a troca (o cansado por um fresco).
    fn ai_change(&mut self, side: usize) {
        let t = &self.teams[side];
        if !t.ai || t.tactical_subs >= AI_TACTICAL_SUBS || t.subs >= MAX_SUBS - 1 {
            return;
        }
        let mut best: Option<(usize, usize, f64)> = None;
        for i in t.on_pitch() {
            let slot = t.players[i].slot.unwrap_or(0);
            if sector(t.role(slot)) == 0 {
                continue;
            }
            let cur = t.players[i].value();
            if let Some((sub, val)) = t.best_sub(slot) {
                let gain = (val - cur) / cur;
                if gain > 0.01 && best.is_none_or(|b| gain > b.2) {
                    best = Some((i, sub, gain));
                }
            }
        }
        if let Some((out, inn, _)) = best {
            self.teams[side].tactical_subs += 1;
            self.make_sub(side, out, inn);
        }
    }

    fn full_time(&mut self) {
        let text = format!("Final do jogo: {} {}-{} {}.", self.short(0), self.score[0], self.score[1], self.short(1));
        self.push("end", None, None, text);
        let best = (0..2)
            .flat_map(|s| (0..self.teams[s].players.len()).map(move |i| (s, i)))
            .filter(|&(s, i)| self.teams[s].players[i].entered.is_some())
            .max_by(|a, b| self.final_rating(a.0, a.1).total_cmp(&self.final_rating(b.0, b.1)));
        if let Some((s, i)) = best {
            let text = format!("Melhor em campo: {} ({}).", self.name(s, i), self.short(s));
            let id = self.teams[s].players[i].info.id;
            self.push("end", Some(s), Some(id), text);
        }
    }

    /// Nota com o resultado: vitória +0,3, derrota −0,3; sem sofrer, +0,4 ao GR e defesas.
    fn final_rating(&self, side: usize, i: usize) -> f64 {
        let p = &self.teams[side].players[i];
        let (gf, ga) = (self.score[side], self.score[1 - side]);
        let mut r = p.rating + 0.3 * (gf.cmp(&ga) as i32) as f64;
        let back = p.slot.is_some_and(|s| sector(self.teams[side].role(s)) <= 1);
        if ga == 0 && back && self.minute >= 60 {
            r += 0.4;
        }
        r.clamp(1.0, 10.0)
    }

    // ---- O treinador mexe na equipa (só no lado dele) ----

    fn user_team(&self) -> Result<usize> {
        if self.finished() {
            return Err(Error::Invalid("O jogo já acabou.".into()));
        }
        self.user.ok_or_else(|| Error::Invalid("Este jogo não é da sua equipa.".into()))
    }

    /// O treinador mexeu: a partir daqui o adjunto só substitui lesionados (como nas
    /// Tácticas, onde mexer na equipa tira a escolha ao adjunto).
    fn user_takes_over(&mut self) -> Result<usize> {
        let side = self.user_team()?;
        self.teams[side].ai = false;
        Ok(side)
    }

    /// Substituição do treinador: `out` em campo, `inn` no banco.
    pub fn substitute(&mut self, out: i32, inn: i32) -> Result<()> {
        let side = self.user_team()?;
        let t = &self.teams[side];
        if t.subs >= MAX_SUBS {
            return Err(Error::Invalid(format!("Já fez as {MAX_SUBS} substituições.")));
        }
        let find = |id: i32| t.players.iter().position(|p| p.info.id == id);
        let out =
            find(out).filter(|&i| t.players[i].slot.is_some()).ok_or_else(|| Error::Invalid("Esse jogador não está em campo.".into()))?;
        let inn = find(inn)
            .filter(|&i| t.players[i].entered.is_none())
            .ok_or_else(|| Error::Invalid("Esse jogador não está no banco.".into()))?;
        self.user_takes_over()?;
        if self.minute == 0 {
            // Antes do pontapé de saída é só uma troca no onze: não conta como substituição
            let t = &mut self.teams[side];
            let slot = t.players[out].slot.take();
            t.players[out].entered = None;
            let r = t.players[out].rating;
            let role = slot.map(|s| t.role(s).to_string()).unwrap_or_default();
            let p = &mut t.players[inn];
            p.slot = slot;
            p.fit = fit_for_role(&p.prof.pos, &role);
            p.entered = Some(0);
            p.rating = r;
            return Ok(());
        }
        self.make_sub(side, out, inn);
        Ok(())
    }

    /// Troca as funções de dois jogadores em campo.
    pub fn swap(&mut self, a: i32, b: i32) -> Result<()> {
        let side = self.user_takes_over()?;
        let t = &mut self.teams[side];
        let find = |id: i32| t.players.iter().position(|p| p.info.id == id && p.slot.is_some());
        let (Some(a), Some(b)) = (find(a), find(b)) else {
            return Err(Error::Invalid("Os dois jogadores têm de estar em campo.".into()));
        };
        let (sa, sb) = (t.players[a].slot, t.players[b].slot);
        t.players[a].slot = sb;
        t.players[b].slot = sa;
        for i in [a, b] {
            let role = t.players[i].slot.map(|s| t.role(s).to_string()).unwrap_or_default();
            t.players[i].fit = fit_for_role(&t.players[i].prof.pos, &role);
        }
        Ok(())
    }

    /// Novas instruções à equipa do treinador.
    pub fn set_instructions(&mut self, instructions: Instructions) -> Result<()> {
        let side = self.user_team()?;
        if self.teams[side].tactics.instructions != instructions {
            self.user_takes_over()?;
            self.teams[side].tactics.instructions = instructions;
            let text = format!("O {} muda as instruções à equipa.", self.short(side));
            self.push("tactics", Some(side), None, text);
        }
        Ok(())
    }

    // ---- Tácticas a meio do jogo (a mesma escolha da tela de Tácticas) ----

    /// A escolha que está em campo agora: formação, quem está em cada função (`None` onde
    /// falta alguém, por expulsão), suplentes por usar, posições, setas e instruções.
    pub fn selection(&self, side: usize) -> Selection {
        let t = &self.teams[side];
        let mut starters = vec![None; t.tactics.starters.len()];
        for p in &t.players {
            if let Some(s) = p.slot {
                starters[s] = Some(p.info.id);
            }
        }
        Selection {
            formation: t.tactics.formation_index,
            starters,
            bench: t.bench().into_iter().map(|i| t.players[i].info.id).collect(),
            instructions: t.tactics.instructions,
            auto: false,
            positions: if t.tactics.custom { t.tactics.starters.iter().map(|s| [s.x, s.y]).collect() } else { Vec::new() },
            runs: t.tactics.starters.iter().map(|s| s.run).collect(),
            takers: t.tactics.takers,
            orders: t.tactics.orders.clone(),
        }
    }

    /// Condição, moral e lesão de quem está no jogo, para as notas e as cores das Tácticas.
    fn squad_state(&self, side: usize) -> SquadState {
        self.teams[side]
            .players
            .iter()
            .map(|p| {
                let a = Availability {
                    condition: p.condition.round() as i32,
                    morale: p.morale,
                    injured: p.injured,
                    suspended: false,
                    reserve: false,
                };
                (p.info.id, a)
            })
            .collect()
    }

    /// Quem entra e quem sai com esta escolha; recusa o que não se pode fazer a meio do jogo.
    fn changes(&self, side: usize, sel: &Selection) -> Result<(Vec<i32>, Vec<i32>)> {
        let t = &self.teams[side];
        let on: Vec<i32> = t.on_pitch().into_iter().map(|i| t.players[i].info.id).collect();
        let bench: Vec<i32> = t.bench().into_iter().map(|i| t.players[i].info.id).collect();
        let new_on: Vec<i32> = sel.starters.iter().flatten().copied().collect();
        if let Some(&p) = new_on.iter().find(|p| !on.contains(p) && !bench.contains(p)) {
            let gone = t.players.iter().any(|q| q.info.id == p && q.left.is_some());
            return Err(Error::Invalid(
                if gone { "Quem já saiu não pode voltar a jogar." } else { "Esse jogador não está no banco." }.into(),
            ));
        }
        if new_on.len() != on.len() {
            return Err(Error::Invalid(format!("Tem de ter {} jogadores em campo.", on.len())));
        }
        let entering: Vec<i32> = new_on.iter().filter(|p| !on.contains(p)).copied().collect();
        let leaving: Vec<i32> = on.iter().filter(|p| !new_on.contains(p)).copied().collect();
        if self.minute > 0 && t.subs + entering.len() as i32 > MAX_SUBS {
            return Err(Error::Invalid(format!("Só pode fazer {MAX_SUBS} substituições (faltam {}).", MAX_SUBS - t.subs)));
        }
        Ok((entering, leaving))
    }

    /// As Tácticas de um lado como a tela mostra (funções, notas, condição); com `sel`, como
    /// ficariam com essa escolha (sem mexer no jogo).
    pub fn tactics(&self, world: &World, side: usize, sel: Option<&Selection>) -> Result<ClubTactics> {
        let own = self.selection(side);
        let sel = sel.unwrap_or(&own);
        if sel != &own {
            self.changes(side, sel)?;
        }
        let mut t = world.tactics_from_selection(self.teams[side].club.id, sel, Some(&self.squad_state(side)))?;
        t.reserves.clear();
        t.editable = self.user == Some(side) && !self.finished();
        t.auto = false;
        Ok(t)
    }

    /// O treinador confirma as Tácticas (Ok): substituições, trocas de função, formação,
    /// posições, setas e instruções, tudo de uma vez.
    pub fn apply_selection(&mut self, world: &World, sel: &Selection) -> Result<()> {
        let side = self.user_team()?;
        let (entering, leaving) = self.changes(side, sel)?;
        let new = self.tactics(world, side, Some(sel))?;
        let before = self.selection(side);
        self.user_takes_over()?;
        let m = self.minute;
        let short = self.short(side).to_string();
        // Quem sai: a meio do jogo sai de vez; antes do apito volta ao banco
        let index = |t: &Team, id: i32| t.players.iter().position(|p| p.info.id == id).expect("jogador do jogo");
        for &id in &leaving {
            let t = &mut self.teams[side];
            let i = index(t, id);
            t.players[i].slot = None;
            if m == 0 {
                t.players[i].entered = None;
            } else {
                t.players[i].left = Some(m);
            }
        }
        for &id in &entering {
            let r = self.base_rating();
            let t = &mut self.teams[side];
            let i = index(t, id);
            t.players[i].entered = Some(m);
            t.players[i].rating = r;
            if m > 0 {
                t.subs += 1;
            }
        }
        // Funções novas: a ordem das funções pode ter mudado com a formação
        let t = &mut self.teams[side];
        for p in &mut t.players {
            if p.slot.is_some() || entering.contains(&p.info.id) {
                p.slot = None;
            }
        }
        for (s, slot) in new.starters.iter().enumerate() {
            if let Some(tp) = &slot.player {
                let i = index(t, tp.id);
                t.players[i].slot = Some(s);
                t.players[i].fit = tp.fit.unwrap_or(1);
            }
        }
        t.tactics = new;
        if m > 0 {
            for (out, inn) in leaving.iter().zip(&entering) {
                let (o, n) = (index(&self.teams[side], *out), index(&self.teams[side], *inn));
                let text = format!("Substituição no {short}: sai {}, entra {}.", self.name(side, o), self.name(side, n));
                self.push("sub", Some(side), Some(*inn), text);
            }
            let after = self.selection(side);
            let shape = |s: &Selection| (s.formation, s.positions.clone(), s.runs.clone(), s.instructions);
            if shape(&before) != shape(&after) {
                self.push("tactics", Some(side), None, format!("O {short} muda a táctica."));
            }
        }
        Ok(())
    }

    // ---- Depois do jogo ----

    /// O que o jogo deixou em cada clube (para condição, moral, lesões e castigos).
    pub fn sides(&self) -> [Side; 2] {
        [0, 1].map(|s| {
            let t = &self.teams[s];
            let played = t.players.iter().filter(|p| p.entered.is_some()).map(|p| (p.info.id, p.condition.round() as i32)).collect();
            Side {
                club: t.club.id,
                played,
                yellows: t.players.iter().filter(|p| p.yellow && !p.red).map(|p| p.info.id).collect(),
                sent_off: t.players.iter().filter(|p| p.red).map(|p| p.info.id).collect(),
                injured: t.players.iter().filter(|p| p.injured).map(|p| p.info.id).collect(),
                goals_for: self.score[s],
                goals_against: self.score[1 - s],
            }
        })
    }

    /// Notas finais (1–10) de quem jogou num lado: (id, nota).
    pub fn ratings(&self, side: usize) -> Vec<(i32, f64)> {
        let t = &self.teams[side];
        (0..t.players.len())
            .filter(|&i| t.players[i].entered.is_some())
            .map(|i| (t.players[i].info.id, self.final_rating(side, i)))
            .collect()
    }

    /// Marcadores de um lado: (nome, minuto).
    pub fn scorers(&self, side: usize) -> Vec<(String, u8)> {
        self.goals
            .iter()
            .filter(|g| g.0 == side)
            .map(|&(_, m, id)| {
                let name = self.teams[side].players.iter().find(|p| p.info.id == id).map_or_else(String::new, |p| p.prof.name.clone());
                (name, m)
            })
            .collect()
    }

    /// Golos até um minuto (para os outros jogos do dia).
    pub fn score_at(&self, minute: u8) -> (i32, i32) {
        let n = |side| self.goals.iter().filter(|g| g.0 == side && g.1 <= minute).count() as i32;
        (n(0), n(1))
    }

    pub fn stats(&self) -> [TeamStats; 2] {
        [0, 1].map(|s| self.team_stats(s))
    }

    pub fn subs_made(&self) -> [i32; 2] {
        [self.teams[0].subs, self.teams[1].subs]
    }

    fn team_stats(&self, side: usize) -> TeamStats {
        let total = self.teams[0].possession + self.teams[1].possession;
        let possession = if total > 0.0 { (100.0 * self.teams[side].possession / total).round() as i32 } else { 50 };
        TeamStats { possession, ..self.teams[side].stats }
    }

    /// A partida para a tela; `others`: os outros jogos do dia.
    pub fn view(&self, others: &[&LiveMatch]) -> LiveView {
        let player = |side: usize, i: usize| {
            let t = &self.teams[side];
            let p = &t.players[i];
            LivePlayer {
                id: p.info.id,
                list_name: p.info.list_name.clone(),
                squad_number: p.info.squad_number,
                role: p.slot.map(|s| t.role(s).to_string()),
                position: p.info.position.clone(),
                condition: p.condition.round() as i32,
                rating: (p.entered.is_some() && self.minute > 0).then(|| self.final_rating(side, i).round() as i32),
                goals: p.goals as i32,
                assists: p.assists as i32,
                yellow: p.yellow,
                red: p.red,
                injured: p.injured,
                fit: p.slot.map(|_| p.fit),
                on_minute: p.entered.filter(|&m| m > 0).map(i32::from),
                off_minute: p.left.map(i32::from),
                x: p.slot.map(|s| t.tactics.starters[s].x),
                y: p.slot.map(|s| t.tactics.starters[s].y),
                stats: p.stats,
            }
        };
        let team = |side: usize| {
            let t = &self.teams[side];
            LiveTeam {
                club: t.club.clone(),
                formation: t.tactics.formation.clone(),
                instructions: t.tactics.instructions,
                players: t.on_pitch().into_iter().map(|i| player(side, i)).collect(),
                bench: t.bench().into_iter().map(|i| player(side, i)).collect(),
                out: (0..t.players.len()).filter(|&i| t.players[i].left.is_some()).map(|i| player(side, i)).collect(),
                subs_left: MAX_SUBS - t.subs,
                stats: self.team_stats(side),
            }
        };
        LiveView {
            fixture: self.fixture.clone(),
            minute: self.minute as i32,
            half_time: self.minute == 45,
            finished: self.finished(),
            home_goals: self.score[0] as i32,
            away_goals: self.score[1] as i32,
            user_side: self.user.map(|s| s as i32),
            home: team(0),
            away: team(1),
            events: self.events.clone(),
            others: others
                .iter()
                .map(|o| {
                    let (h, a) = o.score_at(self.minute);
                    ScoreLine { fixture: o.fixture.clone(), home_goals: h, away_goals: a }
                })
                .collect(),
            recent_possession: {
                let last = &self.ball[self.ball.len().saturating_sub(5)..];
                if last.is_empty() { 50 } else { (100.0 * last.iter().sum::<f64>() / last.len() as f64).round() as i32 }
            },
            zones: {
                let total: f64 = self.zones.iter().sum();
                self.zones.map(|z| if total > 0.0 { (100.0 * z / total).round() as i32 } else { 0 })
            },
            zone_possession: [0, 1, 2]
                .map(|i| if self.zones[i] > 0.0 { (100.0 * self.zone_home[i] / self.zones[i]).round() as i32 } else { 50 }),
            useful_time: self.useful.round() as i32,
            venue: None,
            city: None,
            weather: String::new(),
            referee: String::new(),
            attendance: 0,
            table: Vec::new(),
        }
    }
}

/// Sorteio ponderado; `None` sem candidatos.
fn weighted(r: &mut ChaCha8Rng, cands: &[(usize, f64)]) -> Option<usize> {
    let total: f64 = cands.iter().map(|c| c.1).sum();
    if total <= 0.0 {
        return None;
    }
    let mut x = uniform(r) * total;
    for &(i, w) in cands {
        if x < w {
            return Some(i);
        }
        x -= w;
    }
    cands.last().map(|c| c.0)
}

/// Poisson pelo método de Knuth (λ pequeno).
fn poisson(lambda: f64, r: &mut ChaCha8Rng) -> u32 {
    let limit = (-lambda).exp();
    let (mut k, mut p) = (0, uniform(r));
    while p > limit && k < 20 {
        k += 1;
        p *= uniform(r);
    }
    k
}

/// Peso de cada função num tipo de lance.
fn role_weight(role: &str, table: [f64; 8]) -> f64 {
    // GR, L, D, AL, MD, M, MA, A
    let i = match role.split(' ').next().unwrap_or("") {
        "GR" => 0,
        "L" => 1,
        "D" => 2,
        "AL" => 3,
        "MD" => 4,
        "M" => 5,
        "MA" => 6,
        _ => 7,
    };
    table[i]
}

const PASSERS: [f64; 8] = [0.5, 1.0, 1.0, 1.1, 1.4, 1.5, 1.2, 0.7];
const TACKLERS: [f64; 8] = [0.0, 1.5, 1.5, 1.2, 1.6, 1.0, 0.5, 0.3];
const HEADERS: [f64; 8] = [0.1, 1.5, 1.5, 0.6, 1.0, 0.7, 0.5, 1.4];

impl LiveMatch {
    /// Quem está em campo com o peso de um tipo de lance.
    fn weights(&self, side: usize, table: [f64; 8], attr: impl Fn(&Profile) -> f64) -> Vec<(usize, f64)> {
        let t = &self.teams[side];
        t.on_pitch()
            .into_iter()
            .map(|i| (i, role_weight(t.role(t.players[i].slot.unwrap_or(0)), table) * attr(&t.players[i].prof)))
            .collect()
    }

    /// O enchimento de um minuto no jogo que o treinador vê: passes, cortes, cabeceamentos,
    /// lançamentos, foras de jogo, zonas de acção, tempo útil e, nos minutos sem lances,
    /// uma jogada no relato. Tudo com `r`, nunca com o RNG do resultado.
    fn flavour(&mut self, r: &mut ChaCha8Rng, home_ball: f64, home_push: f64, quiet: bool) {
        // Onde andou a bola: quem tem mais posse e mais ataque empurra o jogo para a frente
        let push = (home_ball - 0.5) * 0.5 + (home_push - 0.5) * 0.4;
        let left = (0.3 - push).clamp(0.1, 0.55);
        let right = (0.3 + push).clamp(0.1, 0.55);
        let zones = [left, 1.0 - left - right, right];
        let zone_home = [home_ball - 0.12, home_ball, home_ball + 0.12].map(|v| v.clamp(0.1, 0.9));
        for i in 0..3 {
            self.zones[i] += zones[i];
            self.zone_home[i] += zones[i] * zone_home[i];
        }
        self.useful += 33.0 + uniform(r) * 15.0;

        for side in 0..2 {
            let share = if side == 0 { home_ball } else { 1.0 - home_ball };
            let pressed = self.teams[1 - side].tactics.instructions.pressing;
            // Passes: ~4,6 por minuto a 50% de posse
            let passers = self.weights(side, PASSERS, |p| 6.0 + p.passing + p.vision);
            for _ in 0..poisson(9.2 * share, r) {
                let Some(i) = weighted(r, &passers) else { break };
                let p = &self.teams[side].players[i].prof;
                // ~78% de passes certos num médio de Passe 12
                let chance = (0.47 + 0.018 * p.passing + 0.008 * p.vision - if pressed { 0.04 } else { 0.0 }).clamp(0.45, 0.9);
                let ok = uniform(r) < chance;
                let t = &mut self.teams[side];
                t.players[i].stats.passes += 1;
                t.stats.passes += 1;
                if ok {
                    t.players[i].stats.passes_ok += 1;
                    t.stats.passes_ok += 1;
                }
            }
            // Cortes: quem não tem a bola tenta recuperá-la
            let tacklers = self.weights(side, TACKLERS, |p| 5.0 + p.tackling);
            for _ in 0..poisson(0.44 * (1.0 - share), r) {
                let Some(i) = weighted(r, &tacklers) else { break };
                let won = uniform(r) < (0.45 + 0.02 * (self.teams[side].players[i].prof.tackling - 10.0)).clamp(0.25, 0.75);
                let t = &mut self.teams[side];
                t.players[i].stats.tackles += 1;
                t.stats.tackles += 1;
                if won {
                    t.players[i].stats.tackles_won += 1;
                    t.stats.tackles_won += 1;
                }
            }
            if uniform(r) < 0.24 {
                self.teams[side].stats.throw_ins += 1;
            }
            if uniform(r) < 0.05 * share {
                self.teams[side].stats.offsides += 1;
            }
        }

        // Bolas pelo ar: um jogador de cada lado, ganha quem cabeceia e salta melhor
        let aerial = |p: &Profile| p.heading + p.jumping;
        let (h, a) = (self.weights(0, HEADERS, |p| aerial(p)), self.weights(1, HEADERS, |p| aerial(p)));
        for _ in 0..poisson(0.3, r) {
            let (Some(i), Some(j)) = (weighted(r, &h), weighted(r, &a)) else { break };
            let (hi, hj) = (aerial(&self.teams[0].players[i].prof), aerial(&self.teams[1].players[j].prof));
            let home_wins = uniform(r) < hi / (hi + hj);
            for (side, k, won) in [(0, i, home_wins), (1, j, !home_wins)] {
                let t = &mut self.teams[side];
                t.players[k].stats.headers += 1;
                t.stats.headers += 1;
                if won {
                    t.players[k].stats.headers_won += 1;
                    t.stats.headers_won += 1;
                }
            }
        }

        if quiet && uniform(r) < 0.75 {
            let side = if uniform(r) < home_ball { 0 } else { 1 };
            let passers = self.weights(side, PASSERS, |p| 6.0 + p.passing);
            let (Some(i), Some(j)) = (weighted(r, &passers), weighted(r, &passers)) else { return };
            let (p, q) = (self.name(side, i), self.name(side, j));
            let club = self.short(side).to_string();
            let gk = self.teams[side].goalkeeper().map(|g| self.name(side, g)).unwrap_or_else(|| p.clone());
            let other_gk = self.teams[1 - side].goalkeeper().map(|g| self.name(1 - side, g)).unwrap_or_else(|| "o guarda-redes".into());
            let line = match (uniform(r) * 10.0) as usize {
                0 => format!("{p} conduz a bola pelo meio-campo"),
                1 if i != j => format!("{p} abre o jogo para {q}"),
                2 if i != j => format!("{p} tenta o passe longo para {q}"),
                3 => format!("{gk} repôs a bola em jogo"),
                4 => format!("{p} cobra o lançamento lateral"),
                5 => format!("{p} recupera a bola"),
                6 => format!("Troca de passes do {club} no meio-campo"),
                7 => format!("{p} tenta o drible mas perde a bola"),
                8 => format!("{p} cruza, mas {other_gk} sai bem e agarra"),
                _ => format!("O {club} tenta chegar à área pela {}", if uniform(r) < 0.5 { "direita" } else { "esquerda" }),
            };
            self.push_lines("play", Some(side), None, line.clone(), vec![line]);
        }
    }
}

#[cfg(test)]
mod tests {
    use rand_core::SeedableRng;

    use super::*;

    #[test]
    fn day_form_spreads_with_low_consistency() {
        let spread = |c: f64| {
            let mut rng = ChaCha8Rng::seed_from_u64(9);
            let v: Vec<f64> = (0..4000).map(|_| day_form(c, &mut rng)).collect();
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let sd = (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
            assert!((mean - 1.0).abs() < 0.005, "média {mean}");
            assert!(v.iter().all(|x| (0.85..=1.15).contains(x)));
            sd
        };
        let (steady, erratic) = (spread(20.0), spread(1.0));
        assert!((steady - 0.02).abs() < 0.004 && (erratic - 0.06).abs() < 0.006, "{steady} {erratic}");
    }

    #[test]
    fn occasion_crowd_shirt_and_nerve() {
        // Campo neutro: sem torcida; o peso da camisa conta na mesma
        let neutral = Occasion::new(true, true, Some(80_000), 9000, 5000);
        assert_eq!(neutral.crowd, 1.0);
        assert_eq!(neutral.shirt, 1.0);
        // Estádio grande empurra mais do que um pequeno, sempre perto de 100%
        let big = Occasion::new(false, false, Some(70_000), 6000, 6000);
        let small = Occasion::new(false, false, Some(8_000), 6000, 6000);
        assert!(big.crowd > 1.03 && small.crowd < 0.97 && big.crowd <= 1.0375 && small.crowd >= 0.9625);
        assert_eq!(big.shirt, 0.0);
        assert!((Occasion::new(false, false, None, 3000, 5000).shirt + 0.5).abs() < 1e-9);
        // Jogos decisivos e Determinação: ±4% nos extremos, nada a meio
        assert!((nerve_factor(20.0) - 1.04).abs() < 1e-9 && (nerve_factor(1.0) - 0.96).abs() < 1e-9);
        assert!((nerve_factor(10.5) - 1.0).abs() < 1e-9 && (determination_factor(10.5) - 1.0).abs() < 1e-9);
    }
}
