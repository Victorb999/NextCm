//! Insatisfação dos jogadores do treinador e pedidos para sair (depois do 3.9). Às segundas os
//! motivos entram e saem pelo que aconteceu, pesam na moral e, com o tempo, o jogador pede para
//! ser posto na lista de transferências (uma proposta `kind = 'request'`, com os botões na
//! mensagem). Regras e números em docs/roadmap/3.09b-insatisfacao-e-pedidos-para-sair.md.
//!
//! Marco 3.19 (itens 4.2 e 4.3): os jovens até aos 23 anos aceitam as reservas e ficam
//! contentes; aos 24, nas reservas, querem ver o futuro esclarecido ("Fut"). Os estrangeiros
//! com saudades de casa podem ter duas semanas de folga; sem ela, acabam por faltar sem
//! autorização ("Fit"). As ausências ficam em `absences`.

use rusqlite::{OptionalExtension, params};

use super::{Game, NewsItem, age_on};
use crate::engine::{match_rng, uniform};
use crate::{Error, Result, date, personality};

/// Folgas e faltas sem autorização (save 41): uma linha por jogador, a última. Uma folga fica
/// gravada depois de acabar: as saudades não voltam no mesmo clube.
pub(super) const ABSENCES_SCHEMA: &str =
    "CREATE TABLE absences (person_id INTEGER PRIMARY KEY, kind TEXT NOT NULL, since TEXT NOT NULL, until TEXT NOT NULL);";

/// Motivos de insatisfação: chave gravada em `unhappy.reason` e o texto do CM.
pub const REASONS: [(&str, &str); 13] = [
    ("starter", "Sente que deve ser titular"),
    ("contract", "Quer negociar um novo contrato"),
    ("bigger", "Quer mudar para um clube maior"),
    ("blocked", "Frustrado por não o deixarem sair"),
    ("homesick", "Tem saudades de casa"),
    ("unfair", "Sente que foi tratado injustamente"),
    ("manager", "Perdeu a confiança no treinador"),
    ("training", "Infeliz com o treino"),
    ("future", "Deseja ver o seu futuro esclarecido"),
    ("vented", "Desabafou os seus sentimentos publicamente"),
    ("subbed", "Descontente por ter sido substituído"),
    ("position", "Quer jogar numa posição diferente"),
    ("teammate", "Não se dá com um colega de equipa"),
];

/// Queixas sobre o clube (o grupo "Club Unhappiness" do CM): chave com o prefixo `club_`.
pub const CLUB_REASONS: [(&str, &str); 5] = [
    ("club_depth", "Acha que o clube precisa de mais profundidade no plantel"),
    ("club_value", "Acha irrealista a avaliação que o clube faz dele"),
    ("club_facilities", "Descontente com as instalações de treino do clube"),
    ("club_ambition", "Duvida das ambições do clube"),
    ("club_results", "Insatisfeito com os resultados do clube"),
];

/// Os motivos pessoais de uma queixa antiga que levam o jogador a desabafar na imprensa.
const GRIEVANCES: &str = "'starter', 'contract', 'bigger', 'blocked', 'unfair', 'manager', 'future'";
/// Dias de queixa até desabafar na imprensa (só quem tem mau feitio).
const VENT_AFTER: i32 = 21;
const VENT_TEMPERAMENT: i32 = 8;
/// Probabilidade semanal de não se dar com um colega (quem tem pouco espírito desportivo ou mau feitio).
const TEAMMATE_CHANCE: f64 = 0.04;
/// Nota (1–20) na função abaixo da qual o jogador sente que não é a posição dele, e a partir da
/// qual está à vontade.
const POSITION_BAD: i32 = 10;
const POSITION_OK: i32 = 13;
/// Minuto até ao qual sair substituído a jogar bem (nota 7 ou mais) magoa um ambicioso.
const SUBBED_BEFORE: u8 = 76;
const SUBBED_RATING: f64 = 7.0;
/// Menos de tantos jogadores no plantel principal (ou menos de 2 guarda-redes): falta profundidade.
const MIN_DEPTH: usize = 18;
/// Pede o preço vezes o valor: a avaliação do clube é irrealista.
const UNREALISTIC_PRICE: f64 = 1.5;
/// Instalações de treino (1 a 20) até às quais os melhores se queixam.
const POOR_FACILITIES: i32 = 6;
/// Reputação do jogador acima da do clube a partir da qual duvida das ambições do clube.
const REPUTATION_GAP: i32 = 1500;
/// Lugares abaixo do objectivo da Direcção a partir dos quais se queixa dos resultados.
const BEHIND_TARGET: i32 = 4;

/// Jogos seguidos sem jogar (podendo) para um dos 11 melhores se sentir titular a mais.
const STARTER_MISSED: i32 = 4;
/// Dias de contrato que faltam para um dos 15 melhores querer renovar.
const CONTRACT_DAYS: i32 = 365;
/// Os dias de contrato a partir dos quais quer renovar, pelo lugar no plantel: os 5 melhores
/// a um ano, até ao 11.º a 300 dias e os outros a 240; a ambição adianta e a lealdade atrasa
/// (8 dias por ponto acima de 10), entre 120 dias e um ano. Assim não se queixam todos no
/// mesmo dia (os contratos acabam quase todos a 30/6).
fn contract_days(rank: usize, ambition: i32, loyalty: i32) -> i32 {
    let base = match rank {
        0..5 => CONTRACT_DAYS,
        5..11 => 300,
        _ => 240,
    };
    (base + 8 * (ambition - 10).max(0) - 8 * (loyalty - 10).max(0)).clamp(120, CONTRACT_DAYS)
}
/// Dias de um treinador novo no clube em que ninguém começa a pedir contrato.
const NEW_MANAGER_GRACE: i32 = 60;
/// Dias que duram a desilusão de não ir para um clube maior, a de não o deixarem sair, o
/// castigo injusto e a perda de confiança no treinador.
const GRUDGE_DAYS: i32 = 120;
/// Adaptabilidade até à qual um estrangeiro acabado de chegar (ou um jovem estrangeiro do
/// plantel) tem saudades de casa.
const HOMESICK_ADAPTABILITY: i32 = 7;
/// Idade até à qual um estrangeiro do plantel, mesmo sem ter chegado durante o jogo, conta como
/// jovem longe de casa; e até à qual um jogador aceita as reservas sem se queixar.
pub(super) const YOUNG_AGE: i32 = 23;
/// As saudades: só até esta idade, depois dos primeiros dias no clube e em quem ainda não
/// jogou estas épocas fora do seu país.
const HOMESICK_MAX_AGE: i32 = 30;
const HOMESICK_SETTLE: i32 = 14;
const HOMESICK_ABROAD: i64 = 2;
/// Dias de folga para matar as saudades.
pub(super) const LEAVE_DAYS: i32 = 14;
/// Com saudades há este tempo sem folga, pode faltar sem autorização (por segunda-feira).
const AWOL_AFTER: i32 = 28;
const AWOL_CHANCE: f64 = 0.25;
/// Dias que fica fora quando falta (de 7 a 14).
const AWOL_DAYS: i32 = 7;
/// Moral que ganha com a folga.
const LEAVE_MORALE: i32 = 2;
/// Moral máxima com motivos: 12 menos isto por motivo (no mínimo 4); desce 2 por semana até lá.
const MORALE_PER_REASON: i32 = 3;
/// Dias de insatisfação até pedir para sair, e dias que espera pela resposta.
const REQUEST_AFTER: i32 = 28;
const REQUEST_DAYS: i32 = 7;
/// Moral que perde quando o treinador recusa o pedido (ou o põe na lista sem ele pedir).
const REFUSED_MORALE: i32 = 3;
pub(super) const LISTED_MORALE: i32 = 2;

pub fn reason_text(key: &str) -> &'static str {
    REASONS.iter().chain(&CLUB_REASONS).find(|r| r.0 == key).map_or("Está insatisfeito", |r| r.1)
}

/// O que o clube é hoje, para as queixas sobre ele.
struct ClubMood {
    main: usize,
    keepers: usize,
    training: i32,
    reputation: i32,
    /// Lugares abaixo do objectivo da Direcção na liga (0 sem jogos que cheguem).
    behind: i32,
}

/// O que o treinador sabe de um jogador do plantel para os motivos.
struct Unhappy {
    id: i32,
    rank: usize,
    age: i32,
    expires: Option<String>,
    joined: Option<String>,
    ambition: i32,
    loyalty: i32,
    adaptability: i32,
    /// Estrangeiro que chegou ao clube durante o jogo.
    moved_abroad: bool,
    /// Estrangeiro (não é da nação do clube).
    foreign: bool,
    listed: bool,
    loaned_in: bool,
}

impl Game {
    /// Junta um motivo (se ainda não o tiver); devolve se é novo.
    pub(super) fn add_unhappy(&self, person: i32, reason: &str) -> Result<bool> {
        Ok(self
            .world
            .conn
            .execute("INSERT OR IGNORE INTO unhappy (person_id, reason, since) VALUES (?, ?, ?)", params![person, reason, self.date])?
            > 0)
    }

    pub(super) fn remove_unhappy(&self, person: i32, reason: &str) -> Result<()> {
        self.world.conn.execute("DELETE FROM unhappy WHERE person_id = ? AND reason = ?", params![person, reason])?;
        Ok(())
    }

    /// Os motivos de um jogador, pelo texto do CM (para o perfil).
    pub fn unhappy_reasons(&self, person: i32) -> Result<Vec<String>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT reason FROM unhappy WHERE person_id = ? AND substr(reason, 1, 5) <> 'club_' ORDER BY since, reason")?
            .query_map([person], |r| r.get::<_, String>(0))?
            .map(|r| r.map(|k| reason_text(&k).to_string()))
            .collect::<rusqlite::Result<_>>()?)
    }

    /// As queixas sobre o clube de um jogador (o "Club Unhappiness" do CM).
    pub fn club_unhappy_reasons(&self, person: i32) -> Result<Vec<String>> {
        Ok(self
            .world
            .conn
            .prepare_cached("SELECT reason FROM unhappy WHERE person_id = ? AND substr(reason, 1, 5) = 'club_' ORDER BY since, reason")?
            .query_map([person], |r| r.get::<_, String>(0))?
            .map(|r| r.map(|k| reason_text(&k).to_string()))
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Põe ou tira um motivo; devolve se é novo.
    fn set_unhappy(&self, person: i32, reason: &str, on: bool) -> Result<bool> {
        if on {
            return self.add_unhappy(person, reason);
        }
        self.remove_unhappy(person, reason)?;
        Ok(false)
    }

    /// Mau feitio, espírito desportivo e reputação do jogador, para as queixas.
    fn mood_traits(&self, person: i32) -> Result<(i32, i32, i32)> {
        Ok(self
            .world
            .conn
            .prepare_cached(
                "SELECT COALESCE(p.temperament, 10), COALESCE(p.sportsmanship, 10), COALESCE(pl.current_reputation, 0)
                 FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.id = ?",
            )?
            .query_row([person], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?)
    }

    /// O clube hoje: plantel principal, guarda-redes, instalações, reputação e a tabela face ao
    /// objectivo da Direcção.
    fn club_mood(&self, squad: &[Unhappy]) -> Result<ClubMood> {
        let main: Vec<i32> = squad.iter().filter(|p| !self.squads.state(p.id).reserve).map(|p| p.id).collect();
        let mut keepers = 0;
        for &id in &main {
            keepers += usize::from(self.is_keeper(id)?);
        }
        let (training, reputation, division): (Option<i32>, Option<i32>, Option<i32>) =
            self.world.conn.query_row("SELECT training, reputation, division_id FROM clubs WHERE id = ?", [self.club_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })?;
        let target: Option<i32> = self.world.conn.query_row("SELECT target FROM board WHERE id = 1", [], |r| r.get(0)).optional()?;
        let behind = match (division, target) {
            (Some(div), Some(target)) => {
                self.table(div).iter().find(|r| r.club.id == self.club_id && r.played >= 8).map_or(0, |r| (r.position - target).max(0))
            }
            _ => 0,
        };
        Ok(ClubMood { main: main.len(), keepers, training: training.unwrap_or(0), reputation: reputation.unwrap_or(0), behind })
    }

    /// Desabafo na imprensa, não se dar com um colega e as queixas do clube de um jogador.
    /// Devolve as queixas do clube que ele passou a ter hoje.
    fn mood_reasons(&self, p: &Unhappy, mood: &ClubMood, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<Vec<&'static str>> {
        let (temperament, sportsmanship, reputation) = self.mood_traits(p.id)?;
        // Queixa antiga e mau feitio: vai à imprensa
        let old = date::add_days(&self.date, -VENT_AFTER).unwrap_or_default();
        let stale: bool = self
            .world
            .conn
            .query_row(
                &format!("SELECT 1 FROM unhappy WHERE person_id = ? AND reason IN ({GRIEVANCES}) AND since <= ?"),
                params![p.id, old],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if self.set_unhappy(p.id, "vented", stale && temperament <= VENT_TEMPERAMENT)? {
            let name = self.person_name(p.id)?;
            fresh.push(self.news_item(
                *next_id,
                "press",
                format!("{name} desabafa na imprensa"),
                format!("{name}, farto de esperar, deu a sua opinião aos jornais: a situação dele no clube está por resolver."),
            ));
            *next_id += 1;
            self.change_fans(-1)?;
        }
        // Um colega com quem não se dá (por semana, sorteado; passa com o tempo)
        if p.rank < 15 && (sportsmanship <= 8 || temperament <= 6) {
            let mut rng = match_rng(self.seed ^ 0xA_402, p.id ^ date::parse(&self.date).unwrap_or(0));
            if uniform(&mut rng) < TEAMMATE_CHANCE {
                self.add_unhappy(p.id, "teammate")?;
            }
        }
        // As queixas do clube
        let asking: Option<Option<i64>> = self
            .world
            .conn
            .query_row("SELECT asking FROM transfer_list WHERE person_id = ? AND transfer = 1", [p.id], |r| r.get(0))
            .optional()?;
        let unrealistic = match asking {
            Some(Some(asking)) => {
                self.world.market_value_of(p.id, &self.date)?.is_some_and(|v| asking as f64 > UNREALISTIC_PRICE * v as f64)
            }
            _ => false,
        };
        let conditions = [
            ("club_depth", p.rank < 14 && (mood.main < MIN_DEPTH || mood.keepers < 2)),
            ("club_value", unrealistic),
            ("club_facilities", p.rank < 11 && p.ambition >= 11 && (1..=POOR_FACILITIES).contains(&mood.training)),
            ("club_ambition", p.rank < 8 && p.ambition >= 14 && reputation > mood.reputation + REPUTATION_GAP),
            ("club_results", p.rank < 14 && p.ambition >= 10 && mood.behind >= BEHIND_TARGET),
        ];
        let mut new = Vec::new();
        for (key, on) in conditions {
            if self.set_unhappy(p.id, key, on)? {
                new.push(key);
            }
        }
        Ok(new)
    }

    /// No fim de um jogo do treinador: quem saiu substituído a jogar bem e quem jogou fora da
    /// posição (ou voltou a estar à vontade).
    pub(super) fn match_moods(&self, lines: &[crate::live::PlayerLine]) -> Result<()> {
        let mine: Vec<i32> = self
            .world
            .conn
            .prepare_cached("SELECT id FROM people WHERE club_id = ? AND is_player = 1")?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for l in lines.iter().filter(|l| !l.sub && mine.contains(&l.id)) {
            let ambition: i32 =
                self.world.conn.query_row("SELECT COALESCE(ambition, 10) FROM people WHERE id = ?", [l.id], |r| r.get(0))?;
            let early = l.off.is_some_and(|m| m < SUBBED_BEFORE) && !l.injured && !l.red;
            if early && l.rating >= SUBBED_RATING && ambition >= 12 {
                self.add_unhappy(l.id, "subbed")?;
            }
            if l.fit < POSITION_BAD {
                self.add_unhappy(l.id, "position")?;
            } else if l.fit >= POSITION_OK {
                self.remove_unhappy(l.id, "position")?;
            }
        }
        Ok(())
    }

    pub(super) fn change_morale(&mut self, person: i32, delta: i32) {
        let s = self.squads.states.entry(person).or_default();
        s.morale = (s.morale + delta).clamp(1, 20);
    }

    /// Um jovem "com valor" ou de "excelente perspectiva": a função no clube é uma das de jovem
    /// ou, sem função definida, tem até 23 anos (o estatuto de jovem que o CM dá por defeito).
    /// Com uma função de adulto (apoio, rotação...), já não aceita as reservas.
    pub(super) fn valued_youth(&self, person: i32, age: i32) -> Result<bool> {
        let role: Option<String> = self
            .world
            .conn
            .prepare_cached("SELECT role FROM squad_roles WHERE person_id = ?")?
            .query_row([person], |r| r.get(0))
            .optional()?;
        Ok(match role.as_deref() {
            Some("prospect" | "youngster") => true,
            Some(_) => false,
            None => age <= YOUNG_AGE,
        })
    }

    /// Nas reservas, até aos 23 anos e com valor: aceita lá estar e está contente.
    pub(super) fn content_in_reserves(&self, person: i32, age: i32) -> Result<bool> {
        Ok(self.squads.state(person).reserve && age <= YOUNG_AGE && self.valued_youth(person, age)?)
    }

    /// Épocas que um jogador passou em clubes fora do seu país (pelo historial do banco).
    fn seasons_abroad(&self, person: i32) -> Result<i64> {
        Ok(self
            .world
            .conn
            .prepare_cached(
                "SELECT COUNT(DISTINCT h.year) FROM career_history h JOIN clubs c ON c.id = h.club_id JOIN people p ON p.id = h.person_id
             WHERE h.person_id = ? AND c.nation_id IS NOT NULL AND c.nation_id <> p.nation_id
               AND COALESCE(p.second_nation_id, 0) <> c.nation_id",
            )?
            .query_row([person], |r| r.get(0))?)
    }

    fn squad_unhappy(&self) -> Result<Vec<Unhappy>> {
        type Row = (i32, Option<String>, Option<i32>, Option<String>, Option<String>, [i32; 3], bool, bool, bool, bool);
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, p.birth_date, p.birth_year, p.contract_expires, p.club_joined,
                        COALESCE(p.ambition, 10), COALESCE(p.loyalty, 10), COALESCE(p.adaptability, 10),
                        COALESCE(p.nation_id <> c.nation_id AND COALESCE(p.second_nation_id, 0) <> c.nation_id
                          AND EXISTS (SELECT 1 FROM transfers t WHERE t.date = p.club_joined AND t.person_id = p.id AND t.to_club = p.club_id), 0),
                        EXISTS (SELECT 1 FROM transfer_list l WHERE l.person_id = p.id AND l.transfer = 1),
                        EXISTS (SELECT 1 FROM loans o WHERE o.person_id = p.id AND o.parent_id <> p.club_id),
                        COALESCE(p.nation_id <> c.nation_id AND COALESCE(p.second_nation_id, 0) <> c.nation_id, 0)
                 FROM people p JOIN players pl ON pl.person_id = p.id LEFT JOIN clubs c ON c.id = p.club_id
                 WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC, p.id",
            )?
            .query_map([self.club_id], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    [r.get(5)?, r.get(6)?, r.get(7)?],
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .enumerate()
            .map(
                |(
                    rank,
                    (id, birth, birth_year, expires, joined, [ambition, loyalty, adaptability], moved_abroad, listed, loaned_in, foreign),
                )| {
                    Unhappy {
                        id,
                        rank,
                        age: age_on(birth.as_deref(), birth_year, &self.date),
                        expires,
                        joined,
                        ambition,
                        loyalty,
                        adaptability,
                        moved_abroad,
                        foreign,
                        listed,
                        loaned_in,
                    }
                },
            )
            .collect())
    }

    /// Às segundas: motivos novos e resolvidos, a moral e os pedidos para sair.
    pub(super) fn unhappy_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if date::weekday(&self.date) != Some(1) {
            return Ok(());
        }
        // Quem saiu do clube leva os motivos com ele
        self.world.conn.execute("DELETE FROM unhappy WHERE person_id NOT IN (SELECT id FROM people WHERE club_id = ?)", [self.club_id])?;
        let today = date::parse(&self.date).unwrap_or(0);
        let grudge = date::add_days(&self.date, -GRUDGE_DAYS).unwrap_or_default();
        self.world
            .conn
            .execute("DELETE FROM unhappy WHERE reason IN ('bigger', 'blocked', 'unfair', 'manager', 'subbed', 'teammate', 'position') AND since < ?", [&grudge])?;
        let squad = self.squad_unhappy()?;
        // Há quanto tempo o treinador está no clube
        let manager_days: i32 = self
            .world
            .conn
            .query_row("SELECT club_joined FROM people WHERE id = ?", [self.manager_id], |r| r.get::<_, Option<String>>(0))?
            .as_deref()
            .and_then(date::parse)
            .map_or(i32::MAX, |j| today - j);
        let mood = self.club_mood(&squad)?;
        // As queixas novas do clube, agrupadas por motivo (uma notícia por motivo, não por jogador)
        let mut club_new: std::collections::BTreeMap<&'static str, Vec<i32>> = Default::default();
        for p in &squad {
            let missed = self.squads.state(p.id).missed;
            let reserve = self.squads.state(p.id).reserve;
            let mut new = Vec::new();
            // Titular sem jogar (os jovens com valor aceitam as reservas)
            let young_reserve = self.content_in_reserves(p.id, p.age)?;
            if p.rank < 11 && p.age >= 20 && missed >= STARTER_MISSED && !young_reserve {
                if self.add_unhappy(p.id, "starter")? {
                    new.push("starter");
                }
            } else if missed == 0 || young_reserve {
                self.remove_unhappy(p.id, "starter")?;
            }
            // Nas reservas com 24 anos ou mais (ou mais novo sem ser um jovem com valor): quer
            // ver o futuro esclarecido
            // Na lista (de transferências ou de empréstimos) o futuro já está esclarecido
            if reserve && !young_reserve && !self.on_any_list(p.id)? {
                if self.add_unhappy(p.id, "future")? {
                    let name = self.person_name(p.id)?;
                    fresh.push(self.news_item(
                        *next_id,
                        "press",
                        format!("{name} quer saber o seu futuro"),
                        format!(
                            "{name}, com {} anos e ainda nas reservas, deseja ver o seu futuro esclarecido. Promova-o ao \
                             plantel principal, empreste-o ou ponha-o na lista de transferências.",
                            p.age
                        ),
                    ));
                    *next_id += 1;
                }
            } else {
                self.remove_unhappy(p.id, "future")?;
            }
            // Contrato a acabar (o treinador acabado de chegar tem uns meses; com a renovação em
            // conversa, espera)
            let left = p.expires.as_deref().and_then(date::parse).map_or(0, |e| e - today);
            let wants = p.rank < 15 && p.age < 33 && !p.loaned_in && left < contract_days(p.rank, p.ambition, p.loyalty);
            if wants && (manager_days < NEW_MANAGER_GRACE || self.open_offer(p.id)?.is_some()) {
                // Ainda não se queixa (o motivo que já tinha fica)
            } else if wants {
                if self.add_unhappy(p.id, "contract")? {
                    new.push("contract");
                }
            } else {
                self.remove_unhappy(p.id, "contract")?;
            }
            // Saudades de casa: o estrangeiro que chegou, ou o jovem estrangeiro, ainda por se
            // adaptar; depois de uma folga não voltam
            // (os primeiros dias ainda não; os mais velhos e quem já jogou fora não sentem)
            let days = p.joined.as_deref().and_then(date::parse).map_or(i32::MAX, |j| today - j);
            let away = p.moved_abroad || (p.foreign && p.age <= YOUNG_AGE);
            if away
                && p.adaptability <= HOMESICK_ADAPTABILITY
                && p.age <= HOMESICK_MAX_AGE
                && days >= HOMESICK_SETTLE
                && (days as f64) < personality::adaptation_days(p.adaptability as f64)
                && !self.had_leave(p.id)?
                && self.seasons_abroad(p.id)? < HOMESICK_ABROAD
            {
                if self.add_unhappy(p.id, "homesick")? {
                    new.push("homesick");
                }
            } else {
                self.remove_unhappy(p.id, "homesick")?;
            }
            // O programa de treino (marco 3.17)
            if self.training_grievance(p.id)? {
                if self.add_unhappy(p.id, "training")? {
                    new.push("training");
                }
            } else {
                self.remove_unhappy(p.id, "training")?;
            }
            // Desabafos, colegas e as queixas do clube
            for key in self.mood_reasons(p, &mood, fresh, next_id)? {
                club_new.entry(key).or_default().push(p.id);
            }
            // Já está na lista: deixam-no sair
            if p.listed {
                self.remove_unhappy(p.id, "blocked")?;
            }
            if !new.is_empty() {
                let name = self.person_name(p.id)?;
                let why: Vec<&str> = new.iter().map(|k| reason_text(k)).collect();
                fresh.push(self.news_item(
                    *next_id,
                    "press",
                    format!("{name} insatisfeito"),
                    format!("{name} está insatisfeito: {}.", why.join("; ").to_lowercase()),
                ));
                *next_id += 1;
            }
        }
        for (key, ids) in club_new {
            let names: Vec<String> = ids.iter().take(3).map(|&id| self.person_name(id)).collect::<Result<_>>()?;
            let more = ids.len().saturating_sub(3);
            let who = match (names.as_slice(), more) {
                ([one], 0) => one.clone(),
                (list, 0) => format!("{} e {}", list[..list.len() - 1].join(", "), list[list.len() - 1]),
                (list, n) => format!("{} e mais {n}", list.join(", ")),
            };
            fresh.push(self.news_item(
                *next_id,
                "press",
                "Jogadores descontentes com o clube".into(),
                format!("{who}: {}.", reason_text(key).to_lowercase()),
            ));
            *next_id += 1;
        }
        // A moral desce até ao teto dos insatisfeitos (as queixas do clube pesam um terço)
        let counts: Vec<(i32, i32)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT person_id, SUM(CASE WHEN substr(reason, 1, 5) = 'club_' THEN 1 ELSE 3 END) FROM unhappy
                 GROUP BY person_id ORDER BY person_id",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, weight) in counts {
            let cap = (12 - MORALE_PER_REASON * weight / 3).max(4);
            let s = self.squads.states.entry(id).or_default();
            if s.morale > cap {
                s.morale = (s.morale - 2).max(cap);
            }
        }
        // Os jovens das reservas sem motivos estão contentes: a moral sobe até à inicial
        let mut young: Vec<i32> = Vec::new();
        for p in &squad {
            if self.content_in_reserves(p.id, p.age)? {
                young.push(p.id);
            }
        }
        for id in young {
            if self.unhappy_reasons(id)?.is_empty() {
                let s = self.squads.states.entry(id).or_default();
                if s.morale < 12 {
                    s.morale += 1;
                }
            }
        }
        // Com saudades há semanas e sem folga: pode faltar sem autorização
        let homesick_since = date::add_days(&self.date, -AWOL_AFTER).unwrap_or_default();
        for p in &squad {
            let long: bool = self
                .world
                .conn
                .query_row(
                    "SELECT 1 FROM unhappy WHERE person_id = ? AND reason = 'homesick' AND since <= ?",
                    params![p.id, homesick_since],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !long || self.absence(p.id)?.is_some() {
                continue;
            }
            let mut rng = match_rng(self.seed ^ 0xA_401, p.id ^ today);
            if uniform(&mut rng) >= AWOL_CHANCE {
                continue;
            }
            let days = AWOL_DAYS + (uniform(&mut rng) * (AWOL_DAYS + 1) as f64) as i32;
            let until = date::add_days(&self.date, days).unwrap_or_default();
            self.world.conn.execute(
                "INSERT OR REPLACE INTO absences (person_id, kind, since, until) VALUES (?, 'awol', ?, ?)",
                params![p.id, self.date, until],
            )?;
            self.clean_selection()?;
            let name = self.person_name(p.id)?;
            fresh.push(self.news_item(
                *next_id,
                "press",
                format!("{name} ausente sem permissão"),
                format!(
                    "{name} não apareceu no treino e não deu notícias: terá voltado para o seu país, com saudades de casa. \
                     Deverá voltar por volta de {}. Para o evitar, pode dar-lhe uma folga (Acção → Dar Folga).",
                    super::cm_date(&until)
                ),
            ));
            *next_id += 1;
        }
        // Pedidos para sair
        let since = date::add_days(&self.date, -REQUEST_AFTER).unwrap_or_default();
        for p in &squad {
            if p.listed || p.loaned_in || !(p.ambition >= 12 || p.loyalty <= 10) {
                continue;
            }
            let reasons: Vec<String> = self
                .world
                .conn
                .prepare_cached("SELECT reason FROM unhappy WHERE person_id = ? AND since <= ? ORDER BY since, reason")?
                .query_map(params![p.id, since], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            let blocked: bool = self
                .world
                .conn
                .query_row("SELECT 1 FROM unhappy WHERE person_id = ? AND reason = 'blocked'", [p.id], |r| r.get::<_, i32>(0))
                .optional()?
                .is_some();
            let Some(reason) =
                reasons.iter().find(|r| matches!(r.as_str(), "starter" | "bigger" | "contract" | "future" | "club_ambition"))
            else {
                continue;
            };
            if blocked || self.open_offer(p.id)?.is_some() {
                continue;
            }
            let until = date::add_days(&self.date, REQUEST_DAYS).unwrap_or_default();
            self.world.conn.execute(
                "INSERT INTO offers (kind, person_id, buyer, seller, fee, status, answer_on, created) VALUES ('request', ?, ?, ?, 0, 'pending', ?, ?)",
                params![p.id, self.club_id, self.club_id, until, self.date],
            )?;
            let offer = self.world.conn.last_insert_rowid() as i32;
            let name = self.person_name(p.id)?;
            let mut n = self.news_item(
                *next_id,
                "deal",
                format!("{name} pede para sair"),
                format!(
                    "{name} pede para ser posto na lista de transferências ({}). Responda até {}.",
                    reason_text(reason).to_lowercase(),
                    super::cm_date(&until)
                ),
            );
            n.offer_id = Some(offer);
            fresh.push(n);
            *next_id += 1;
        }
        Ok(())
    }

    /// A ausência em curso de um jogador do plantel: ("leave" ou "awol", até quando).
    pub fn absence(&self, person: i32) -> Result<Option<(String, String)>> {
        Ok(self
            .world
            .conn
            .query_row("SELECT kind, until FROM absences WHERE person_id = ? AND until > ?", params![person, self.date], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?)
    }

    /// Os jogadores fora (de folga ou ausentes sem permissão) hoje.
    pub(super) fn absent_players(&self) -> Vec<i32> {
        let rows = || -> Result<Vec<i32>> {
            Ok(self
                .world
                .conn
                .prepare_cached("SELECT person_id FROM absences WHERE until > ?")?
                .query_map([&self.date], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?)
        };
        rows().unwrap_or_default()
    }

    fn had_leave(&self, person: i32) -> Result<bool> {
        Ok(self
            .world
            .conn
            .query_row("SELECT 1 FROM absences WHERE person_id = ? AND kind = 'leave'", [person], |_| Ok(()))
            .optional()?
            .is_some())
    }

    /// Pode ter folga: tem saudades de casa (ou faltou por isso) e não está já de folga.
    pub fn can_give_leave(&self, person: i32) -> Result<bool> {
        let homesick = self.unhappy_reasons(person)?.iter().any(|r| r == reason_text("homesick"));
        let away = self.absence(person)?;
        Ok((homesick || away.as_ref().is_some_and(|a| a.0 == "awol")) && !away.is_some_and(|a| a.0 == "leave"))
    }

    /// "Dar Folga": duas semanas no seu país; as saudades passam e não voltam neste clube.
    pub fn give_leave(&mut self, person: i32) -> Result<String> {
        let mine: bool =
            self.world.conn.query_row("SELECT club_id = ? FROM people WHERE id = ?", params![self.club_id, person], |r| r.get(0))?;
        if !mine || self.sacked() {
            return Err(Error::Invalid("Só para os jogadores do seu plantel.".into()));
        }
        if !self.can_give_leave(person)? {
            return Err(Error::Invalid("Só se dá folga a quem tem saudades de casa.".into()));
        }
        let until = date::add_days(&self.date, LEAVE_DAYS).unwrap_or_default();
        self.world.conn.execute(
            "INSERT OR REPLACE INTO absences (person_id, kind, since, until) VALUES (?, 'leave', ?, ?)",
            params![person, self.date, until],
        )?;
        self.remove_unhappy(person, "homesick")?;
        self.change_morale(person, LEAVE_MORALE);
        self.clean_selection()?;
        self.refresh_live()?;
        self.unsaved = true;
        let name = self.person_name(person)?;
        Ok(format!("{name} vai passar duas semanas no seu país. Volta a {}.", super::cm_date(&until)))
    }

    /// O treinador recusa (ou não responde a) um pedido para sair.
    pub(super) fn refuse_request(&mut self, person: i32) -> Result<()> {
        self.add_unhappy(person, "blocked")?;
        self.change_morale(person, -REFUSED_MORALE);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn veterans_abroad_are_not_homesick() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = crate::World::open(&path).unwrap();
        // O Kovacic (Croácia) jogou anos em Espanha, Inglaterra e Itália
        let kovacic: i32 =
            w.conn.query_row("SELECT id FROM people WHERE display_name = 'Mateo Kovacic' AND is_player = 1", [], |r| r.get(0)).unwrap();
        let abroad: i64 = w
            .conn
            .query_row(
                "SELECT COUNT(DISTINCT h.year) FROM career_history h JOIN clubs c ON c.id = h.club_id JOIN people p ON p.id = h.person_id
                 WHERE h.person_id = ? AND c.nation_id <> p.nation_id",
                [kovacic],
                |r| r.get(0),
            )
            .unwrap();
        assert!(abroad >= HOMESICK_ABROAD, "{abroad} épocas fora");
    }

    #[test]
    fn contract_complaints_are_spread() {
        // Os 5 melhores a um ano; os outros mais tarde; a lealdade atrasa, a ambição adianta
        assert_eq!(contract_days(0, 10, 10), 365);
        assert_eq!(contract_days(7, 10, 10), 300);
        assert_eq!(contract_days(12, 10, 10), 240);
        assert_eq!(contract_days(12, 10, 20), 160);
        assert_eq!(contract_days(12, 20, 10), 320);
        assert_eq!(contract_days(0, 20, 10), 365, "nunca mais de um ano");
        assert_eq!(contract_days(14, 1, 20), 160);
    }

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn unhappy_players_ask_to_leave() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-insatisfeitos-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Insatisfeitos".into(),
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
        };
        let mut g = Game::create(&path, &dir, &new, 7).unwrap();
        let squad = g.squad_unhappy().unwrap();
        let star = squad.iter().find(|p| p.rank < 11 && p.age >= 20 && !p.loaned_in).unwrap().id;
        g.world.conn.execute("UPDATE people SET ambition = 18, contract_expires = '2029-06-30' WHERE id = ?", [star]).unwrap();
        g.squads.states.entry(star).or_default().missed = STARTER_MISSED;
        let monday = |g: &mut Game, d: &str| {
            g.date = d.into();
            let (mut fresh, mut id) = (Vec::new(), 1000);
            g.unhappy_day(&mut fresh, &mut id).unwrap();
            fresh
        };

        // Um titular que não joga fica insatisfeito, e a moral desce
        let before = g.squads.state(star).morale;
        let news = monday(&mut g, "2025-07-07");
        assert_eq!(g.unhappy_reasons(star).unwrap(), vec![reason_text("starter").to_string()]);
        assert!(news.iter().any(|n| n.title.contains("insatisfeito")));
        assert!(g.squads.state(star).morale < before);
        assert!(g.player_status(star).unwrap().unhappy.len() == 1);

        // Ao fim de 4 semanas pede para sair; recusar deixa-o frustrado
        let news = monday(&mut g, "2025-08-04");
        let request = news.iter().find(|n| n.title.contains("pede para sair")).and_then(|n| n.offer_id).expect("pedido");
        let view = g.answer_offer(request, "reject", None).unwrap();
        assert_eq!(view.status, "rejected");
        assert!(g.unhappy_reasons(star).unwrap().contains(&reason_text("blocked").to_string()));
        assert!(monday(&mut g, "2025-08-11").iter().all(|n| !n.title.contains("pede para sair")), "frustrado não volta a pedir já");

        // Jogar resolve o motivo do titular
        g.squads.states.entry(star).or_default().missed = 0;
        monday(&mut g, "2025-08-18");
        assert!(!g.unhappy_reasons(star).unwrap().contains(&reason_text("starter").to_string()));

        // Pôr na lista a pedido: fica "listado a pedido" e sem o "não o deixam sair"
        g.world.conn.execute("UPDATE unhappy SET since = '2025-01-01' WHERE person_id = ?", [star]).unwrap();
        g.world.conn.execute("DELETE FROM unhappy WHERE person_id = ? AND reason = 'blocked'", [star]).unwrap();
        g.add_unhappy(star, "bigger").unwrap();
        g.world.conn.execute("UPDATE unhappy SET since = '2025-08-01' WHERE person_id = ?", [star]).unwrap();
        let news = monday(&mut g, "2025-09-01");
        let request = news.iter().find(|n| n.title.contains("pede para sair")).and_then(|n| n.offer_id).expect("novo pedido");
        assert_eq!(g.answer_offer(request, "accept", None).unwrap().status, "done");
        let requested: i32 = g
            .world
            .conn
            .query_row("SELECT requested FROM transfer_list WHERE person_id = ? AND transfer = 1", [star], |r| r.get(0))
            .unwrap();
        assert_eq!(requested, 1);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reserves_homesickness_and_leave() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-saudades-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Saudades".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: false,
        };
        let mut g = Game::create(&path, &dir, &new, 11).unwrap();
        let squad = g.squad_unhappy().unwrap();
        let monday = |g: &mut Game, d: &str| {
            g.date = d.into();
            let (mut fresh, mut id) = (Vec::new(), 1000);
            g.unhappy_day(&mut fresh, &mut id).unwrap();
            fresh
        };

        // Um jovem das reservas não se queixa de não jogar e está contente
        let young = squad.iter().find(|p| p.rank < 11 && !p.loaned_in && !p.foreign).unwrap().id;
        g.world.conn.execute("UPDATE people SET birth_date = '2004-01-01', birth_year = 2004 WHERE id = ?", [young]).unwrap();
        g.set_reserve(young, true).unwrap();
        g.squads.states.entry(young).or_default().missed = STARTER_MISSED;
        monday(&mut g, "2025-07-07");
        assert!(g.unhappy_reasons(young).unwrap().is_empty());
        let future = g.transfer_view(young).unwrap().future;
        assert!(future.iter().any(|f| f == "Contente com situação no clube"), "{future:?}");

        // Com 23 e a função "apoio à primeira equipa", já não é um "jovem com valor": quer ver o
        // futuro esclarecido; com a função "jovem com valor" (ou sem função), aceita as reservas
        g.world.conn.execute("UPDATE people SET birth_date = '2002-01-01', birth_year = 2002 WHERE id = ?", [young]).unwrap();
        g.world.conn.execute("INSERT OR REPLACE INTO squad_roles (person_id, role) VALUES (?, 'backup')", [young]).unwrap();
        monday(&mut g, "2025-07-07");
        assert!(g.unhappy_reasons(young).unwrap().contains(&reason_text("future").to_string()));
        g.world.conn.execute("INSERT OR REPLACE INTO squad_roles (person_id, role) VALUES (?, 'youngster')", [young]).unwrap();
        monday(&mut g, "2025-07-07");
        assert!(g.unhappy_reasons(young).unwrap().is_empty());
        g.world.conn.execute("DELETE FROM squad_roles WHERE person_id = ?", [young]).unwrap();

        // Aos 24, nas reservas, quer ver o futuro esclarecido: notícia, "Fut" e o Futuro
        g.world.conn.execute("UPDATE people SET birth_date = '2001-01-01', birth_year = 2001 WHERE id = ?", [young]).unwrap();
        let news = monday(&mut g, "2025-07-14");
        assert!(news.iter().any(|n| n.title.contains("quer saber o seu futuro")));
        assert!(g.unhappy_reasons(young).unwrap().contains(&reason_text("future").to_string()));
        let mut club = g.world.get_club(PALMEIRAS).unwrap();
        g.fill_squad(&mut club).unwrap();
        assert_eq!(club.players.iter().find(|p| p.id == young).unwrap().info.as_deref(), Some("Fut"));
        // Promovido, passa
        g.set_reserve(young, false).unwrap();
        monday(&mut g, "2025-07-21");
        assert!(!g.unhappy_reasons(young).unwrap().contains(&reason_text("future").to_string()));

        // Um jovem estrangeiro pouco adaptável tem saudades; sem folga, acaba por faltar
        let foreign = squad.iter().find(|p| p.foreign && !p.loaned_in).map(|p| p.id).unwrap_or(young);
        g.world
            .conn
            .execute(
                "UPDATE people SET birth_date = '2005-01-01', birth_year = 2005, adaptability = 2, club_joined = '2025-07-01',
                        nation_id = CASE WHEN nation_id = (SELECT nation_id FROM clubs WHERE id = ?1) THEN 1 ELSE nation_id END WHERE id = ?2",
                params![PALMEIRAS, foreign],
            )
            .unwrap();
        // A primeira vez fora do país (sem épocas no estrangeiro no historial)
        g.world.conn.execute("DELETE FROM career_history WHERE person_id = ?", [foreign]).unwrap();
        monday(&mut g, "2025-07-28");
        assert!(g.unhappy_reasons(foreign).unwrap().contains(&reason_text("homesick").to_string()));
        assert!(g.can_give_leave(foreign).unwrap());
        let future = g.transfer_view(foreign).unwrap().future;
        assert!(future.iter().any(|f| f == "Está a ter dificuldades em adaptar-se ao estilo de vida Brasileiro"), "{future:?}");
        let mut awol = false;
        for d in ["2025-08-25", "2025-09-01", "2025-09-08", "2025-09-15", "2025-09-22", "2025-09-29", "2025-10-06", "2025-10-13"] {
            monday(&mut g, d);
            if g.absence(foreign).unwrap().is_some_and(|a| a.0 == "awol") {
                awol = true;
                break;
            }
        }
        assert!(awol, "com saudades há semanas, falta sem autorização");
        assert!(g.club_state(PALMEIRAS, None)[&foreign].injured, "ausente não pode jogar");

        // A folga: duas semanas fora, as saudades passam e não voltam
        assert!(g.give_leave(foreign).unwrap().contains("duas semanas"));
        assert_eq!(g.absence(foreign).unwrap().map(|a| a.0).as_deref(), Some("leave"));
        assert!(!g.unhappy_reasons(foreign).unwrap().contains(&reason_text("homesick").to_string()));
        assert!(g.give_leave(foreign).is_err(), "já está de folga");
        let back = date::add_days(&g.date, LEAVE_DAYS + 7).unwrap();
        monday(&mut g, &back);
        assert!(g.absence(foreign).unwrap().is_none());
        assert!(!g.unhappy_reasons(foreign).unwrap().contains(&reason_text("homesick").to_string()));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn club_and_personal_moods() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-queixas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Queixas".into(),
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
        };
        let mut g = Game::create(&path, &dir, &new, 7).unwrap();
        let squad = g.squad_unhappy().unwrap();
        let star = squad.iter().find(|p| p.rank < 8 && !p.loaned_in).unwrap().id;
        let monday = |g: &mut Game, d: &str| {
            g.date = d.into();
            let (mut fresh, mut id) = (Vec::new(), 1000);
            g.unhappy_day(&mut fresh, &mut id).unwrap();
            fresh
        };

        // Instalações fracas e um jogador ambicioso: queixa do clube, à parte das pessoais, e uma
        // só notícia por motivo
        g.world.conn.execute("UPDATE clubs SET training = 3 WHERE id = ?", [PALMEIRAS]).unwrap();
        g.world.conn.execute("UPDATE people SET ambition = 16, temperament = 4 WHERE id = ?", [star]).unwrap();
        let news = monday(&mut g, "2025-07-07");
        assert!(g.club_unhappy_reasons(star).unwrap().contains(&reason_text("club_facilities").to_string()));
        assert!(g.unhappy_reasons(star).unwrap().is_empty(), "as queixas do clube não são pessoais");
        assert_eq!(g.player_status(star).unwrap().club_unhappy.len(), g.club_unhappy_reasons(star).unwrap().len());
        assert_eq!(news.iter().filter(|n| n.title == "Jogadores descontentes com o clube" && n.body.contains("instalações")).count(), 1);
        // Instalações boas: a queixa passa
        g.world.conn.execute("UPDATE clubs SET training = 15 WHERE id = ?", [PALMEIRAS]).unwrap();
        monday(&mut g, "2025-07-14");
        assert!(!g.club_unhappy_reasons(star).unwrap().contains(&reason_text("club_facilities").to_string()));

        // Falta de profundidade: poucos no plantel principal
        let others: Vec<i32> = squad.iter().skip(16).map(|p| p.id).collect();
        for id in &others {
            g.squads.states.entry(*id).or_default().reserve = true;
        }
        monday(&mut g, "2025-07-21");
        assert!(g.club_unhappy_reasons(star).unwrap().contains(&reason_text("club_depth").to_string()));
        for id in &others {
            g.squads.states.entry(*id).or_default().reserve = false;
        }
        monday(&mut g, "2025-07-28");
        assert!(!g.club_unhappy_reasons(star).unwrap().contains(&reason_text("club_depth").to_string()));

        // Preço irrealista na lista de transferências
        g.world
            .conn
            .execute("INSERT OR REPLACE INTO transfer_list (person_id, transfer, loan, asking) VALUES (?, 1, 0, 900000000)", [star])
            .unwrap();
        monday(&mut g, "2025-08-04");
        assert!(g.club_unhappy_reasons(star).unwrap().contains(&reason_text("club_value").to_string()));

        // Queixa antiga e mau feitio: desabafa na imprensa
        g.add_unhappy(star, "bigger").unwrap();
        g.world.conn.execute("UPDATE unhappy SET since = '2025-07-01' WHERE person_id = ? AND reason = 'bigger'", [star]).unwrap();
        let news = monday(&mut g, "2025-08-11");
        assert!(g.unhappy_reasons(star).unwrap().contains(&reason_text("vented").to_string()));
        assert!(news.iter().any(|n| n.title.contains("desabafa")));

        // Ao fim do jogo: saiu substituído cedo a jogar bem; jogou fora da posição
        let line = |id: i32, fit: i32, off: Option<u8>, rating: f64| crate::live::PlayerLine {
            id,
            rating,
            goals: 0,
            assists: 0,
            sub: false,
            stats: Default::default(),
            on: 0,
            off,
            yellow: false,
            red: false,
            injured: false,
            fit,
        };
        let other = squad.iter().find(|p| p.id != star && !p.loaned_in).unwrap().id;
        g.world.conn.execute("UPDATE people SET ambition = 15 WHERE id = ?", [other]).unwrap();
        g.match_moods(&[line(other, 6, Some(60), 7.5)]).unwrap();
        let reasons = g.unhappy_reasons(other).unwrap();
        assert!(reasons.contains(&reason_text("subbed").to_string()));
        assert!(reasons.contains(&reason_text("position").to_string()));
        // À vontade na posição e a acabar o jogo: a queixa da posição passa
        g.match_moods(&[line(other, 15, None, 6.0)]).unwrap();
        assert!(!g.unhappy_reasons(other).unwrap().contains(&reason_text("position").to_string()));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
