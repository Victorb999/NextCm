//! Insatisfação dos jogadores do treinador e pedidos para sair (depois do 3.9). Às segundas os
//! motivos entram e saem pelo que aconteceu, pesam na moral e, com o tempo, o jogador pede para
//! ser posto na lista de transferências (uma proposta `kind = 'request'`, com os botões na
//! mensagem). Regras e números no ROADMAP ("Listas da IA, insatisfação e pedidos para sair").

use rusqlite::{OptionalExtension, params};

use super::{Game, NewsItem, age_on};
use crate::{Result, date, personality};

/// Motivos de insatisfação: chave gravada em `unhappy.reason` e o texto do CM.
pub const REASONS: [(&str, &str); 7] = [
    ("starter", "Sente que deve ser titular"),
    ("contract", "Quer negociar um novo contrato"),
    ("bigger", "Quer mudar para um clube maior"),
    ("blocked", "Frustrado por não o deixarem sair"),
    ("homesick", "Tem saudades de casa"),
    ("unfair", "Sente que foi tratado injustamente"),
    ("manager", "Perdeu a confiança no treinador"),
];

/// Jogos seguidos sem jogar (podendo) para um dos 11 melhores se sentir titular a mais.
const STARTER_MISSED: i32 = 4;
/// Dias de contrato que faltam para um dos 15 melhores querer renovar.
const CONTRACT_DAYS: i32 = 365;
/// Dias que duram a desilusão de não ir para um clube maior, a de não o deixarem sair, o
/// castigo injusto e a perda de confiança no treinador.
const GRUDGE_DAYS: i32 = 120;
/// Adaptabilidade até à qual um estrangeiro acabado de chegar tem saudades de casa.
const HOMESICK_ADAPTABILITY: i32 = 7;
/// Moral máxima com motivos: 12 menos isto por motivo (no mínimo 4); desce 2 por semana até lá.
const MORALE_PER_REASON: i32 = 3;
/// Dias de insatisfação até pedir para sair, e dias que espera pela resposta.
const REQUEST_AFTER: i32 = 28;
const REQUEST_DAYS: i32 = 7;
/// Moral que perde quando o treinador recusa o pedido (ou o põe na lista sem ele pedir).
const REFUSED_MORALE: i32 = 3;
pub(super) const LISTED_MORALE: i32 = 2;

pub fn reason_text(key: &str) -> &'static str {
    REASONS.iter().find(|r| r.0 == key).map_or("Está insatisfeito", |r| r.1)
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
            .prepare_cached("SELECT reason FROM unhappy WHERE person_id = ? ORDER BY since, reason")?
            .query_map([person], |r| r.get::<_, String>(0))?
            .map(|r| r.map(|k| reason_text(&k).to_string()))
            .collect::<rusqlite::Result<_>>()?)
    }

    pub(super) fn change_morale(&mut self, person: i32, delta: i32) {
        let s = self.squads.states.entry(person).or_default();
        s.morale = (s.morale + delta).clamp(1, 20);
    }

    fn squad_unhappy(&self) -> Result<Vec<Unhappy>> {
        type Row = (i32, Option<String>, Option<i32>, Option<String>, Option<String>, [i32; 3], bool, bool, bool);
        let rows: Vec<Row> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, p.birth_date, p.birth_year, p.contract_expires, p.club_joined,
                        COALESCE(p.ambition, 10), COALESCE(p.loyalty, 10), COALESCE(p.adaptability, 10),
                        COALESCE(p.nation_id <> c.nation_id AND COALESCE(p.second_nation_id, 0) <> c.nation_id
                          AND EXISTS (SELECT 1 FROM transfers t WHERE t.date = p.club_joined AND t.person_id = p.id AND t.to_club = p.club_id), 0),
                        EXISTS (SELECT 1 FROM transfer_list l WHERE l.person_id = p.id AND l.transfer = 1),
                        EXISTS (SELECT 1 FROM loans o WHERE o.person_id = p.id AND o.parent_id <> p.club_id)
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
                ))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows
            .into_iter()
            .enumerate()
            .map(|(rank, (id, birth, birth_year, expires, joined, [ambition, loyalty, adaptability], moved_abroad, listed, loaned_in))| {
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
                    listed,
                    loaned_in,
                }
            })
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
            .execute("DELETE FROM unhappy WHERE reason IN ('bigger', 'blocked', 'unfair', 'manager') AND since < ?", [&grudge])?;
        let squad = self.squad_unhappy()?;
        for p in &squad {
            let missed = self.squads.state(p.id).missed;
            let mut new = Vec::new();
            // Titular sem jogar
            if p.rank < 11 && p.age >= 20 && missed >= STARTER_MISSED {
                if self.add_unhappy(p.id, "starter")? {
                    new.push("starter");
                }
            } else if missed == 0 {
                self.remove_unhappy(p.id, "starter")?;
            }
            // Contrato a acabar
            let left = p.expires.as_deref().and_then(date::parse).map_or(0, |e| e - today);
            if p.rank < 15 && p.age < 33 && !p.loaned_in && left < CONTRACT_DAYS {
                if self.add_unhappy(p.id, "contract")? {
                    new.push("contract");
                }
            } else {
                self.remove_unhappy(p.id, "contract")?;
            }
            // Saudades de casa
            let days = p.joined.as_deref().and_then(date::parse).map_or(i32::MAX, |j| today - j);
            if p.moved_abroad
                && p.adaptability <= HOMESICK_ADAPTABILITY
                && (days as f64) < personality::adaptation_days(p.adaptability as f64)
            {
                if self.add_unhappy(p.id, "homesick")? {
                    new.push("homesick");
                }
            } else {
                self.remove_unhappy(p.id, "homesick")?;
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
        // A moral desce até ao teto dos insatisfeitos
        let counts: Vec<(i32, i32)> = self
            .world
            .conn
            .prepare_cached("SELECT person_id, COUNT(*) FROM unhappy GROUP BY person_id ORDER BY person_id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, n) in counts {
            let cap = (12 - MORALE_PER_REASON * n).max(4);
            let s = self.squads.states.entry(id).or_default();
            if s.morale > cap {
                s.morale = (s.morale - 2).max(cap);
            }
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
            let Some(reason) = reasons.iter().find(|r| matches!(r.as_str(), "starter" | "bigger" | "contract")) else { continue };
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
}
