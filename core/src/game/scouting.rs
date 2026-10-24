//! Conhecimento e olheiros (marco 3.10, passo 1). O clube do treinador só vê os atributos dos
//! jogadores que conhece: os do plantel e os observados (durante um ano) por inteiro; os da
//! mesma nação do clube e os famosos em parte; os outros nada ("-"). Observar um jogador
//! manda um olheiro do staff; o relatório chega em dias, pela distância e pelo olheiro.
//! Sem conhecimento, a direção só autoriza parte do orçamento numa proposta.
//! Regras e números no detalhe do 3.10 no ROADMAP.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use rusqlite::{OptionalExtension, params};

use super::market::money_text;
use super::{Game, NewsItem, age_on, cm_date};
use crate::person::{Knowledge, PersonProfile};
use crate::{Error, Result, date};

/// Dias que dura o conhecimento de um jogador observado.
const KNOWN_DAYS: i32 = 365;
/// Reputação (mundial ou actual) a partir da qual um jogador é conhecido em parte em qualquer
/// lado, e a reputação actual que chega no mesmo continente (escala do banco: 0–200).
const FAMOUS_WORLD: i32 = 150;
const FAMOUS_CONTINENT: i32 = 100;
/// Parte dos atributos tapados no conhecimento parcial.
const PARTIAL_HIDDEN: u64 = 35;
/// Parte do orçamento de transferências que a direção autoriza sem conhecimento e em parte.
const UNKNOWN_BUDGET: f64 = 0.4;
const PARTIAL_BUDGET: f64 = 0.7;
/// Dias até ao relatório: mesma nação, mesmo continente, outro continente (olheiro médio).
const REPORT_DAYS: [f64; 3] = [5.0, 10.0, 18.0];
/// Job dos olheiros em `jobs`.
const SCOUT_JOB: i32 = 9;

/// Onde está um jogador em relação ao clube do treinador.
struct Place {
    club: Option<i32>,
    same_nation: bool,
    same_continent: bool,
    world_rep: i32,
    current_rep: i32,
}

fn hash(seed: i64, person: i32, key: &str) -> u64 {
    let mut h = (seed as u64) ^ (person as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for b in key.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01B3);
    }
    h
}

impl Game {
    fn place(&self, person: i32) -> Result<Option<Place>> {
        let mine: (Option<i32>, Option<i32>) = self.world.conn.query_row(
            "SELECT c.nation_id, n.continent_id FROM clubs c LEFT JOIN nations n ON n.id = c.nation_id WHERE c.id = ?",
            [self.club_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        // A nação do clube dele (sem clube: a dele)
        let row: Option<(Option<i32>, Option<i32>, Option<i32>, i32, i32)> = self
            .world
            .conn
            .query_row(
                "SELECT p.club_id, COALESCE(c.nation_id, p.nation_id), n.continent_id,
                        COALESCE(pl.world_reputation, 0), COALESCE(pl.current_reputation, 0)
                 FROM people p JOIN players pl ON pl.person_id = p.id LEFT JOIN clubs c ON c.id = p.club_id
                 LEFT JOIN nations n ON n.id = COALESCE(c.nation_id, p.nation_id)
                 WHERE p.id = ?",
                [person],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        Ok(row.map(|(club, nation, continent, world_rep, current_rep)| Place {
            club,
            same_nation: nation.is_some() && nation == mine.0,
            same_continent: continent.is_some() && continent == mine.1,
            world_rep,
            current_rep,
        }))
    }

    /// O que o clube do treinador sabe de um jogador (os que não são jogadores: tudo).
    pub fn knowledge(&self, person: i32) -> Result<Knowledge> {
        let Some(p) = self.place(person)? else { return Ok(Knowledge::Known) };
        let lent: bool = self
            .world
            .conn
            .query_row("SELECT 1 FROM loans WHERE person_id = ? AND parent_id = ?", params![person, self.club_id], |_| Ok(true))
            .optional()?
            .is_some();
        let observed: bool = self
            .world
            .conn
            .query_row("SELECT 1 FROM knowledge WHERE person_id = ? AND until >= ?", params![person, self.date], |_| Ok(true))
            .optional()?
            .is_some();
        Ok(if p.club == Some(self.club_id) || lent || observed {
            Knowledge::Known
        } else if p.same_nation || p.world_rep.max(p.current_rep) >= FAMOUS_WORLD || (p.same_continent && p.current_rep >= FAMOUS_CONTINENT)
        {
            Knowledge::Partial
        } else {
            Knowledge::Unknown
        })
    }

    /// Tapa no perfil o que o clube não sabe: os atributos (todos, ou parte fixa por jogador).
    pub fn mask_profile(&self, p: &mut PersonProfile) -> Result<()> {
        let k = self.knowledge(p.id)?;
        p.knowledge = Some(k);
        p.report_on = self.world.conn.query_row("SELECT ready_on FROM scouting WHERE person_id = ?", [p.id], |r| r.get(0)).optional()?;
        if let Some(pl) = &mut p.player {
            match k {
                Knowledge::Known => {}
                Knowledge::Unknown => pl.attributes.values_mut().for_each(|v| *v = None),
                Knowledge::Partial => {
                    for (key, v) in pl.attributes.iter_mut() {
                        if hash(self.seed, p.id, key) % 100 < PARTIAL_HIDDEN {
                            *v = None;
                        }
                    }
                }
            }
            if k != Knowledge::Known {
                pl.raw_attributes.values_mut().for_each(|v| *v = None);
            }
        }
        Ok(())
    }

    /// O orçamento que a direção autoriza numa proposta por este jogador, e a frase do CM
    /// quando ela não o conhece bem.
    pub(super) fn budget_for(&self, person: i32) -> Result<(i64, Option<String>)> {
        let budget = self.transfer_budget()?;
        let (share, why) = match self.knowledge(person)? {
            Knowledge::Known => return Ok((budget, None)),
            Knowledge::Partial => (PARTIAL_BUDGET, "sabe pouco"),
            Knowledge::Unknown => (UNKNOWN_BUDGET, "não sabe nada"),
        };
        let limit = crate::finance::round_money(budget as f64 * share);
        let text = format!("A sua direcção só autorizará transferências até {} porque ela {why} sobre este jogador.", money_text(limit));
        Ok((limit, Some(text)))
    }

    /// O melhor olheiro do clube (Avaliar Jogadores + Avaliar Potencial): id, nome e as duas notas.
    fn best_scout(&self) -> Result<Option<(i32, String, i32, i32)>> {
        Ok(self
            .world
            .conn
            .query_row(
                "SELECT p.id, p.display_name, COALESCE(np.judgement, 10), COALESCE(np.judging_potential, 10)
                 FROM people p LEFT JOIN non_players np ON np.person_id = p.id
                 WHERE p.club_id = ? AND p.club_job = ?
                 ORDER BY COALESCE(np.judgement, 10) + COALESCE(np.judging_potential, 10) DESC, p.id LIMIT 1",
                params![self.club_id, SCOUT_JOB],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?)
    }

    /// "Observar Jogador": o melhor olheiro vai vê-lo; devolve a frase com a data do relatório.
    pub fn scout_player(&mut self, person: i32) -> Result<String> {
        let place = self.place(person)?.ok_or_else(|| Error::Invalid("Só se observam jogadores.".into()))?;
        if place.club == Some(self.club_id) {
            return Err(Error::Invalid("Já conhece os jogadores do seu plantel.".into()));
        }
        if let Some(on) = self
            .world
            .conn
            .query_row("SELECT ready_on FROM scouting WHERE person_id = ?", [person], |r| r.get::<_, String>(0))
            .optional()?
        {
            return Err(Error::Invalid(format!("Já está a ser observado: o relatório chega a {}.", cm_date(&on))));
        }
        let Some((scout, scout_name, judgement, _)) = self.best_scout()? else {
            return Err(Error::Invalid("O clube não tem olheiros.".into()));
        };
        let base = if place.same_nation {
            REPORT_DAYS[0]
        } else if place.same_continent {
            REPORT_DAYS[1]
        } else {
            REPORT_DAYS[2]
        };
        // Um olheiro com Avaliar Jogadores 20 leva 70% do tempo; com 1, 130%
        let days = (base * (1.3 - 0.6 * (judgement.clamp(1, 20) - 1) as f64 / 19.0)).round().max(2.0) as i32;
        let ready = date::add_days(&self.date, days).unwrap_or_default();
        self.world.conn.execute("INSERT INTO scouting (person_id, ready_on, scout_id) VALUES (?, ?, ?)", params![person, ready, scout])?;
        self.unsaved = true;
        let name = self.person_name(person)?;
        Ok(format!("{scout_name} vai observar {name}. O relatório chega a {}.", cm_date(&ready)))
    }

    /// Os relatórios que chegam hoje: o jogador passa a conhecido e chega a mensagem.
    pub(super) fn scouting_day(&mut self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let due: Vec<(i32, Option<i32>)> = self
            .world
            .conn
            .prepare_cached("SELECT person_id, scout_id FROM scouting WHERE ready_on <= ? ORDER BY person_id")?
            .query_map([&self.date], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let until = date::add_days(&self.date, KNOWN_DAYS).unwrap_or_default();
        for (person, scout) in due {
            self.world.conn.execute("DELETE FROM scouting WHERE person_id = ?", [person])?;
            self.world.conn.execute("INSERT OR REPLACE INTO knowledge (person_id, until) VALUES (?, ?)", params![person, until])?;
            let (title, body) = self.scout_report(person, scout)?;
            fresh.push(self.news_item(*next_id, "message", title, body));
            *next_id += 1;
        }
        Ok(())
    }

    /// O relatório: estrelas pelo nível que o olheiro acha que ele tem no plantel (com o erro
    /// do olheiro, fixo por jogador) e uma frase sobre o potencial.
    fn scout_report(&self, person: i32, scout: Option<i32>) -> Result<(String, String)> {
        let (judgement, potential, scout_name): (i32, i32, String) = match scout {
            Some(s) => self.world.conn.query_row(
                "SELECT COALESCE(np.judgement, 10), COALESCE(np.judging_potential, 10), p.display_name
                 FROM people p LEFT JOIN non_players np ON np.person_id = p.id WHERE p.id = ?",
                [s],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?,
            None => (10, 10, "O seu olheiro".into()),
        };
        let (ca, pa, birth, birth_year, club): (i32, i32, Option<String>, Option<i32>, Option<String>) = self.world.conn.query_row(
            "SELECT COALESCE(pl.current_ability, 1), COALESCE(pl.potential_ability, 0), p.birth_date, p.birth_year,
                    COALESCE(NULLIF(c.short_name, ''), c.name)
             FROM people p JOIN players pl ON pl.person_id = p.id LEFT JOIN clubs c ON c.id = p.club_id WHERE p.id = ?",
            [person],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
        let age = age_on(birth.as_deref(), birth_year, &self.date);
        // O nível do plantel: a média dos 11 melhores
        let level: f64 = self.world.conn.query_row(
            "SELECT COALESCE(AVG(ca), 1) FROM (SELECT pl.current_ability AS ca FROM people p JOIN players pl ON pl.person_id = p.id
             WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC LIMIT 11)",
            [self.club_id],
            |r| r.get(0),
        )?;
        // O erro do olheiro: até ±15 de CA (Avaliar Jogadores 1) a ±2 (20), e o mesmo no potencial
        let mut rng = ChaCha8Rng::seed_from_u64(hash(self.seed, person, "relatorio"));
        let mut miss = |skill: i32| {
            let spread = 2.0 + 13.0 * (20 - skill.clamp(1, 20)) as f64 / 19.0;
            ((rng.next_u64() % 2001) as f64 / 1000.0 - 1.0) * spread
        };
        let seen = ca as f64 + miss(judgement);
        let pa = if pa > 0 { pa } else { ca + 10 };
        let room = pa as f64 + miss(potential) - seen;
        let stars = match seen - level {
            d if d >= 10.0 => 5,
            d if d >= 0.0 => 4,
            d if d >= -10.0 => 3,
            d if d >= -20.0 => 2,
            _ => 1,
        };
        let verdict = match stars {
            5 => "Seria um dos melhores do plantel.",
            4 => "Seria titular na equipa.",
            3 => "Seria útil no plantel.",
            2 => "Seria só um suplente.",
            _ => "Não tem nível para o clube.",
        };
        let future = if age <= 23 && room >= 20.0 {
            "Tem muito potencial para crescer."
        } else if age <= 27 && room >= 8.0 {
            "Ainda pode melhorar."
        } else {
            "Já não deve melhorar muito."
        };
        let name = self.person_name(person)?;
        let at = club.map_or("sem clube".to_string(), |c| format!("do {c}"));
        let star_text = "★".repeat(stars) + &"☆".repeat(5 - stars);
        Ok((
            format!("Relatório sobre {name}"),
            format!("{scout_name} observou {name} ({at}, {age} anos).\n\nAvaliação: {star_text}\n{verdict} {future}"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;
    const FLAMENGO: i32 = 3675;

    #[test]
    fn scouts_reveal_unknown_players() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-olheiros-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Olheiros".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
        };
        let mut g = Game::create(&path, &dir, &new, 9).unwrap();
        let first = |sql: &str| g.world.conn.query_row(sql, [], |r| r.get::<_, i32>(0)).unwrap();
        let mine = first("SELECT id FROM people WHERE club_id = 7568 AND is_player = 1 LIMIT 1");
        let rival = first(&format!("SELECT id FROM people WHERE club_id = {FLAMENGO} AND is_player = 1 LIMIT 1"));
        // Um jogador modesto de um clube da Ásia
        let far = first(
            "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id JOIN clubs c ON c.id = p.club_id
             JOIN nations n ON n.id = c.nation_id WHERE n.continent_id = 1 AND COALESCE(pl.world_reputation, 0) < 50 AND COALESCE(pl.current_reputation, 0) < 50 ORDER BY p.id LIMIT 1",
        );
        assert_eq!(g.knowledge(mine).unwrap(), Knowledge::Known);
        assert_eq!(g.knowledge(rival).unwrap(), Knowledge::Partial, "mesma nação");
        assert_eq!(g.knowledge(far).unwrap(), Knowledge::Unknown);

        // O perfil chega tapado: tudo "-" no desconhecido, parte no parcial
        let masked = |g: &Game, id: i32| {
            let mut p = g.world.get_person(id).unwrap();
            g.mask_profile(&mut p).unwrap();
            let attrs = &p.player.as_ref().unwrap().attributes;
            (attrs.values().filter(|v| v.is_none()).count(), attrs.len())
        };
        let (hidden, total) = masked(&g, far);
        assert_eq!(hidden, total);
        let (hidden, total) = masked(&g, rival);
        let some = g.world.get_person(rival).unwrap().player.unwrap().attributes.values().filter(|v| v.is_none()).count();
        assert!(hidden > some && hidden < total, "{hidden} de {total}");

        // A direção limita a proposta
        let (limit, note) = g.budget_for(far).unwrap();
        assert!(limit < g.transfer_budget().unwrap() || g.transfer_budget().unwrap() == 0);
        assert!(note.unwrap().contains("não sabe nada"));

        // Observar: o relatório chega e ele passa a conhecido
        let text = g.scout_player(far).unwrap();
        assert!(text.contains("relatório chega"), "{text}");
        assert!(g.scout_player(far).is_err(), "já está a ser observado");
        assert!(g.scout_player(mine).is_err());
        let ready: String = g.world.conn.query_row("SELECT ready_on FROM scouting WHERE person_id = ?", [far], |r| r.get(0)).unwrap();
        g.date = ready;
        let (mut fresh, mut id) = (Vec::new(), 1000);
        g.scouting_day(&mut fresh, &mut id).unwrap();
        assert_eq!(g.knowledge(far).unwrap(), Knowledge::Known);
        assert!(fresh.iter().any(|n| n.title.starts_with("Relatório sobre") && n.body.contains('★')));
        assert_eq!(g.budget_for(far).unwrap().1, None);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
