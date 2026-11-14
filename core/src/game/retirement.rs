//! Reformas e regens (marco 4.5). A reforma tem vários motivos: a idade (deslocada pelo nível:
//! o craque joga mais anos, o guarda-redes dois a mais), o contrato (com contrato em vigor
//! raramente para; sem clube, muito mais), os minutos, as lesões e a personalidade. A meio da
//! época os que vão parar anunciam-no (`retiring`); o treinador pode tentar segurá-los com uma
//! renovação. No fim da época saem: uns ficam no futebol como staff (no clube ou livres), os
//! outros saem do jogo (`retired`). Cada reformado dá origem a um jogador novo da mesma nação
//! (16–18 anos, a posição e o perfil dele, o potencial ligado ao que ele foi), como no CM.
//! Tudo fica em `retirements` (idade, clube, motivo, se virou staff).

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use rusqlite::{OptionalExtension, params};

use super::season::{age_on, join_names};
use super::{Game, NewsItem, regen};
use crate::Result;
use crate::engine::uniform;
use crate::regulation::SeasonKind;

/// As reformas e os anúncios (save 63).
pub(super) const RETIREMENT_SCHEMA: &str = "
CREATE TABLE retirements (
    person_id INTEGER PRIMARY KEY,
    date TEXT NOT NULL,
    age INTEGER NOT NULL,
    club_id INTEGER,
    reason TEXT NOT NULL,
    staff INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE retiring (
    person_id INTEGER PRIMARY KEY,
    announced TEXT NOT NULL,
    reason TEXT NOT NULL
);
";

/// A reputação mundial (0–200) a partir da qual a reforma de um jogador de outro clube é notícia.
const FAMOUS: i32 = 120;
/// Chance de ficar no futebol como staff, antes dos atributos (liderança, determinação, fama).
const STAFF_BASE: f64 = 0.12;
/// Dos que viram staff, os que ficam no clube onde jogavam (os outros ficam livres).
const STAFF_STAYS: f64 = 0.4;
/// Chance de adiar a reforma quando o treinador lhe propõe renovar (antes da personalidade).
const STAYS_ON: f64 = 0.4;
/// Um livre só deixa herdeiro se foi um profissional a sério (CA deste nível); quem pára num
/// clube das divisões geridas deixa sempre. Sem isto, os milhares de livres fracos enchem os plantéis.
const HEIR_CA: i32 = 100;
/// Plantel (todos os jogadores) a partir do qual o clube não recebe herdeiros.
const HEIR_SQUAD: i32 = 36;

/// O que pesa na decisão de um jogador.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Career {
    pub age: i32,
    pub goalkeeper: bool,
    pub ca: i32,
    /// Dias até o contrato acabar (`None` sem clube).
    pub contract_days: Option<i32>,
    /// Jogos na época.
    pub apps: i32,
    /// Dias de lesão que faltam.
    pub injured_days: i32,
    pub injury_proneness: i32,
    pub professionalism: i32,
    pub ambition: i32,
}

/// Chance de terminar a carreira no fim desta época e o motivo principal.
pub(super) fn retire_chance(c: &Career) -> (f64, &'static str) {
    // A idade efectiva: o guarda-redes dura dois anos mais; o craque (CA 160) mais um, o fraco
    // (CA 80) um a menos
    let shift = if c.goalkeeper { 2.0 } else { 0.0 } + ((c.ca - 120) as f64 / 40.0).clamp(-1.5, 2.0);
    let age = c.age as f64 - shift;
    const CURVE: [(f64, f64); 10] = [
        (31.0, 0.0),
        (32.0, 0.03),
        (33.0, 0.06),
        (34.0, 0.12),
        (35.0, 0.22),
        (36.0, 0.38),
        (37.0, 0.55),
        (38.0, 0.72),
        (39.0, 0.86),
        (40.0, 0.97),
    ];
    let base = if age <= CURVE[0].0 {
        0.0
    } else if age >= CURVE[CURVE.len() - 1].0 {
        CURVE[CURVE.len() - 1].1
    } else {
        let i = CURVE.iter().position(|&(a, _)| a > age).unwrap_or(1);
        let ((a0, p0), (a1, p1)) = (CURVE[i - 1], CURVE[i]);
        p0 + (p1 - p0) * (age - a0) / (a1 - a0)
    };
    if base <= 0.0 {
        return (0.0, "idade");
    }
    let mut chance = base;
    let mut reason = ("idade", 1.0);
    let mut weigh = |factor: f64, why: &'static str| {
        chance *= factor;
        if factor > reason.1 {
            reason = (why, factor);
        }
    };
    match c.contract_days {
        None => weigh(2.0, "sem clube"),
        Some(d) if d > 300 => weigh(0.35, "contrato"),
        Some(d) if d <= 30 => weigh(1.3, "fim do contrato"),
        _ => {}
    }
    if c.contract_days.is_some() {
        if c.apps < 5 {
            weigh(1.5, "falta de minutos");
        } else if c.apps >= 15 {
            weigh(0.75, "minutos");
        }
    }
    if c.injured_days > 90 {
        weigh(1.8, "lesão");
    }
    if c.injury_proneness >= 15 {
        weigh(1.25, "lesões");
    }
    let drive = (c.professionalism + c.ambition) as f64 / 2.0;
    if drive >= 15.0 {
        weigh(0.8, "vontade");
    } else if drive <= 7.0 {
        weigh(1.25, "vontade");
    }
    (chance.min(1.0), reason.0)
}

/// O texto do motivo nas notícias.
fn reason_text(reason: &str) -> &'static str {
    match reason {
        "sem clube" => "sem clube e sem propostas",
        "fim do contrato" => "no fim do contrato",
        "falta de minutos" => "por já quase não jogar",
        "lesão" | "lesões" => "por causa das lesões",
        _ => "pela idade",
    }
}

impl Game {
    fn career_of(&self, id: i32, club: Option<i32>, contract: Option<&str>) -> Result<Career> {
        let (birth, birth_year, ca, gk, prone, prof, amb): (Option<String>, Option<i32>, i32, i32, i32, i32, i32) =
            self.world.conn.query_row(
                "SELECT p.birth_date, p.birth_year, COALESCE(pl.current_ability, 1), COALESCE(pl.pos_goalkeeper, 0),
                        COALESCE(pl.injury_proneness, 10), COALESCE(p.professionalism, 10), COALESCE(p.ambition, 10)
                 FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.id = ?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )?;
        let today = crate::date::parse(&self.date).unwrap_or(0);
        let state = self.squads.state(id);
        let injured_days = state.injury_until.as_deref().and_then(crate::date::parse).map_or(0, |d| (d - today).max(0));
        let contract_days = club.map(|_| contract.and_then(crate::date::parse).map_or(365, |d| d - today));
        Ok(Career {
            age: age_on(birth.as_deref(), birth_year, &self.date),
            goalkeeper: gk >= 15,
            ca,
            contract_days,
            apps: state.apps,
            injured_days,
            injury_proneness: prone,
            professionalism: prof,
            ambition: amb,
        })
    }

    /// Jogadores com 31 anos ou mais nas divisões geridas deste tipo de época (`free`: também
    /// os livres; os das nações com divisões de calendário civil no fim dessa época, todos os
    /// outros no fim da europeia), que ainda não anunciaram a reforma.
    fn veterans(&self, kind: SeasonKind, free: bool) -> Result<Vec<(i32, String, Option<i32>, Option<String>)>> {
        let managed = self.managed_sql(Some(kind));
        let year: i32 = self.date[0..4].parse().unwrap_or(0);
        let calendar = self.managed_sql(Some(SeasonKind::Calendar));
        let civil = format!("(SELECT DISTINCT nation_id FROM clubs WHERE division_id IN ({calendar}))");
        let free_sql = match (free, kind) {
            (false, _) => String::new(),
            (true, SeasonKind::Calendar) => format!("OR (p.club_id IS NULL AND p.nation_id IN {civil})"),
            (true, _) => format!("OR (p.club_id IS NULL AND (p.nation_id IS NULL OR p.nation_id NOT IN {civil}))"),
        };
        let sql = format!(
            "SELECT p.id, p.display_name, p.club_id, p.contract_expires FROM people p
             WHERE p.is_player = 1 AND p.id NOT IN (SELECT person_id FROM retired) AND p.id NOT IN (SELECT person_id FROM retiring)
               AND COALESCE(p.birth_year, 0) <= ?1 - 31 AND p.id <> ?2
               AND (p.club_id IN (SELECT id FROM clubs WHERE division_id IN ({managed})) {free_sql})
             ORDER BY p.id"
        );
        Ok(self
            .world
            .conn
            .prepare(&sql)?
            .query_map(params![year, self.manager_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// A meio da época (1/2 na Europa, 1/8 no Brasil): os veteranos dos clubes que decidem parar
    /// no fim dela anunciam-no. Ao treinador, os do plantel; a todos, os famosos.
    pub(super) fn announce_retirements(&mut self, kind: SeasonKind, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let mut rng = self.season_rng(0x4E71);
        let (mut ours, mut famous) = (Vec::new(), Vec::new());
        for (id, name, club, contract) in self.veterans(kind, false)? {
            let career = self.career_of(id, club, contract.as_deref())?;
            let (chance, reason) = retire_chance(&career);
            if chance <= 0.0 || uniform(&mut rng) >= chance {
                continue;
            }
            self.world.conn.execute(
                "INSERT OR IGNORE INTO retiring (person_id, announced, reason) VALUES (?, ?, ?)",
                params![id, self.date, reason],
            )?;
            if club == Some(self.club_id) {
                ours.push((name, reason));
            } else if self.world_rep(id)? >= FAMOUS {
                famous.push(format!("{name} ({}, {} anos)", club.map(|c| self.club(c).short_name).unwrap_or_default(), career.age));
            }
        }
        for (name, reason) in ours {
            let body = format!(
                "{name} anunciou que vai terminar a carreira no fim desta época, {}.\n\nSe quiser que ele continue, \
                 pode propor-lhe uma renovação: talvez mude de ideias.",
                reason_text(reason)
            );
            self.push_news(fresh, next_id, "message", format!("{name} vai terminar a carreira"), body);
        }
        if !famous.is_empty() {
            let body = format!("Anunciaram que vão terminar a carreira no fim desta época: {}.", join_names(&famous));
            self.push_news(fresh, next_id, "jobs", "Despedidas anunciadas".into(), body);
        }
        self.unsaved = true;
        Ok(())
    }

    fn world_rep(&self, id: i32) -> Result<i32> {
        Ok(self
            .world
            .conn
            .query_row("SELECT COALESCE(world_reputation, 0) FROM players WHERE person_id = ?", [id], |r| r.get(0))
            .optional()?
            .unwrap_or(0))
    }

    /// As reformas de um período e a população, para a CLI `sim` (4.5): quantas, a idade média,
    /// os motivos, as que viraram staff, e os jogadores nos clubes geridos e livres.
    pub fn retirement_report(&self, from: &str, to: &str) -> Result<String> {
        let conn = &self.world.conn;
        let (count, age, staff): (i64, f64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(AVG(age), 0), COALESCE(SUM(staff), 0) FROM retirements WHERE date >= ? AND date < ?",
            params![from, to],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let reasons: Vec<String> = conn
            .prepare("SELECT reason, COUNT(*) FROM retirements WHERE date >= ? AND date < ? GROUP BY reason ORDER BY COUNT(*) DESC")?
            .query_map(params![from, to], |r| Ok(format!("{} {}", r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let managed = self.managed_sql(None);
        let (players, free): (i64, i64) = conn.query_row(
            &format!(
                "SELECT (SELECT COUNT(*) FROM people WHERE is_player = 1 AND club_id IN (SELECT id FROM clubs WHERE division_id IN ({managed}))),
                        (SELECT COUNT(*) FROM people WHERE is_player = 1 AND club_id IS NULL AND id NOT IN (SELECT person_id FROM retired))"
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(format!(
            "reformas {count} (idade média {age:.1}, {staff} viraram staff; {}), jogadores nos clubes geridos {players}, livres {free}",
            reasons.join(", ")
        ))
    }

    /// O jogador anunciou que vai parar? (o motivo)
    pub(super) fn retiring(&self, id: i32) -> Result<Option<String>> {
        Ok(self.world.conn.prepare_cached("SELECT reason FROM retiring WHERE person_id = ?")?.query_row([id], |r| r.get(0)).optional()?)
    }

    /// O treinador propõe renovar a quem anunciou a reforma: ele aceita continuar ou não (sempre
    /// a mesma resposta para o mesmo jogador). `true` se adia a reforma.
    pub(super) fn stays_on(&self, id: i32) -> Result<bool> {
        let (prof, amb): (i32, i32) = self.world.conn.query_row(
            "SELECT COALESCE(professionalism, 10), COALESCE(ambition, 10) FROM people WHERE id = ?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let chance = STAYS_ON * (1.0 + (prof + amb - 20) as f64 / 20.0);
        Ok(uniform(&mut self.deal_rng(id, 0x5747)) < chance)
    }

    /// Ele adiou a reforma (renovou com o treinador).
    pub(super) fn cancel_retirement(&self, id: i32) -> Result<()> {
        self.world.conn.execute("DELETE FROM retiring WHERE person_id = ?", [id])?;
        Ok(())
    }

    /// Fim da época: param os que anunciaram e alguns dos livres das nações destas divisões.
    /// Uns ficam como staff, os outros saem do futebol; cada um deixa um regen.
    pub(super) fn retirements(&mut self, kind: SeasonKind, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        let mut rng = self.season_rng(0x0E7E);
        let mut leaving: Vec<(i32, String, Option<i32>, String)> = Vec::new();
        // Os que anunciaram, se a época do clube deles é esta (ou estão sem clube)
        let announced: Vec<(i32, String, Option<i32>, String)> = self
            .world
            .conn
            .prepare("SELECT p.id, p.display_name, p.club_id, r.reason FROM retiring r JOIN people p ON p.id = r.person_id ORDER BY p.id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<rusqlite::Result<_>>()?;
        for (id, name, club, reason) in announced {
            if club.is_none() || self.kind_of(club.unwrap_or(0))? == kind {
                leaving.push((id, name, club, reason));
            }
        }
        // Os livres que ninguém quis
        for (id, name, club, contract) in self.veterans(kind, true)? {
            if club.is_some() {
                continue;
            }
            let (chance, reason) = retire_chance(&self.career_of(id, None, contract.as_deref())?);
            if chance > 0.0 && uniform(&mut rng) < chance {
                leaving.push((id, name, None, reason.to_string()));
            }
        }
        // Os jogadores de enchimento que ficaram sem clube saem do jogo (só servem ao clube deles)
        if kind == SeasonKind::European {
            self.world.conn.execute(
                "INSERT OR IGNORE INTO retired (person_id, date)
                 SELECT g.person_id, ? FROM ghosts g JOIN people p ON p.id = g.person_id WHERE p.club_id IS NULL",
                [&self.date],
            )?;
        }
        let mut factory = self.player_factory()?;
        let (mut ours, mut famous, mut heirs) = (Vec::new(), Vec::new(), Vec::new());
        for (id, name, club, reason) in leaving {
            let career = self.career_of(id, club, None)?;
            let staff = self.retire_player(id, club, &reason, career.age, &mut rng)?;
            if let Some(heir) = self.create_heir(&mut factory, id, club.is_some(), &mut rng)?
                && heir.1 == self.club_id
            {
                heirs.push(heir);
            }
            if club == Some(self.club_id) {
                ours.push(match &staff {
                    Some(job) if job.1 => format!("{name} (fica no clube como {})", job.0),
                    _ => name,
                });
            } else if self.world_rep(id)? >= FAMOUS {
                famous.push(name);
            }
        }
        self.refresh_live()?;
        if !ours.is_empty() {
            let verb = if ours.len() == 1 { "terminou a carreira" } else { "terminaram a carreira" };
            self.push_news(fresh, next_id, "message", "Reformas".into(), format!("{} {verb} no fim desta época.", join_names(&ours)));
        }
        if !famous.is_empty() {
            let verb = if famous.len() == 1 { "terminou a carreira" } else { "terminaram a carreira" };
            self.push_news(fresh, next_id, "jobs", "Fim de carreira".into(), format!("{} {verb}.", join_names(&famous)));
        }
        if !heirs.is_empty() {
            for (id, _, _) in &heirs {
                self.squads.states.entry(*id).or_default().reserve = true;
            }
            let names: Vec<String> = heirs.into_iter().map(|h| h.2).collect();
            let body = format!(
                "Chegou ao clube um jovem talento: {}.\n\nComeça na equipa de reservas, com contrato de três anos.",
                join_names(&names)
            );
            self.push_news(fresh, next_id, "message", "Novo talento".into(), body);
        }
        self.unsaved = true;
        Ok(())
    }

    /// Tira um jogador do futebol, ou passa-o a staff. Devolve a função e se ficou no clube.
    fn retire_player(
        &self,
        id: i32,
        club: Option<i32>,
        reason: &str,
        age: i32,
        rng: &mut ChaCha8Rng,
    ) -> Result<Option<(&'static str, bool)>> {
        let conn = &self.world.conn;
        conn.execute("DELETE FROM retiring WHERE person_id = ?", [id])?;
        let (staff_already, determination, leadership, rep): (bool, i32, i32, i32) = conn.query_row(
            "SELECT p.is_non_player = 1, COALESCE(p.determination, 10), COALESCE(pl.leadership, 10), COALESCE(pl.world_reputation, 0)
             FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.id = ?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
        let chance = STAFF_BASE
            + if staff_already { 0.25 } else { 0.0 }
            + if determination >= 15 { 0.1 } else { 0.0 }
            + if leadership >= 15 { 0.1 } else { 0.0 }
            + if rep >= 100 { 0.1 } else { 0.0 };
        let becomes_staff = uniform(rng) < chance.min(0.7);
        let job = if becomes_staff {
            self.make_staff(id)?;
            let stays = club.is_some() && uniform(rng) < STAFF_STAYS;
            let (job_id, label) = if self.coach_minded(id)? { (6, "Auxiliar Técnico") } else { (9, "Olheiro") };
            let year: i32 = self.date[0..4].parse().unwrap_or(0);
            let conn = &self.world.conn;
            if stays {
                conn.execute(
                    "UPDATE people SET is_player = 0, club_job = ?, club_joined = ?, contract_expires = ?, wage = wage / 5 WHERE id = ?",
                    params![job_id, self.date, format!("{}-06-30", year + 2), id],
                )?;
            } else {
                conn.execute(
                    "UPDATE people SET is_player = 0, club_id = NULL, club_job = NULL, contract_expires = NULL, wage = 0 WHERE id = ?",
                    [id],
                )?;
            }
            Some((label, stays))
        } else {
            let conn = &self.world.conn;
            conn.execute("INSERT OR IGNORE INTO retired (person_id, date) VALUES (?, ?)", params![id, self.date])?;
            conn.execute("UPDATE people SET club_id = NULL, club_job = NULL, contract_expires = NULL WHERE id = ?", [id])?;
            None
        };
        self.world.conn.execute(
            "INSERT OR REPLACE INTO retirements (person_id, date, age, club_id, reason, staff) VALUES (?, ?, ?, ?, ?, ?)",
            params![id, self.date, age, club, reason, job.is_some()],
        )?;
        Ok(job)
    }

    /// Mais jeito para treinar (táctica e liderança) do que para observar (visão e antecipação)?
    fn coach_minded(&self, id: i32) -> Result<bool> {
        Ok(self.world.conn.query_row(
            "SELECT COALESCE(coaching, 0) + COALESCE(tactics, 0) >= COALESCE(judgement, 0) + COALESCE(judging_potential, 0)
             FROM non_players WHERE person_id = ?",
            [id],
            |r| r.get(0),
        )?)
    }

    /// Dá ao reformado a ficha de staff, se ainda não tem: os atributos saem dos de jogador
    /// (a leitura do jogo, a liderança, a técnica), na escala 1–20.
    pub(super) fn make_staff(&self, id: i32) -> Result<()> {
        let conn = &self.world.conn;
        let has: bool = conn.query_row("SELECT COUNT(*) > 0 FROM non_players WHERE person_id = ?", [id], |r| r.get(0))?;
        conn.execute("UPDATE people SET is_non_player = 1 WHERE id = ?", [id])?;
        if has {
            return Ok(());
        }
        conn.execute(
            "INSERT INTO non_players (person_id, current_ability, potential_ability, home_reputation, current_reputation, world_reputation,
                                      attacking, business, coaching, coaching_gks, coaching_technique, directness, discipline, free_roles,
                                      interference, judgement, judging_potential, man_handling, marking, motivating, offside, patience,
                                      physiotherapy, pressing, resources, tactics, youngsters, preferred_formation)
             SELECT pl.person_id,
                    MIN(200, MAX(20, (pl.decisions + pl.anticipation + pl.leadership + pl.teamwork) * 2 + pl.current_ability / 4)),
                    MIN(200, MAX(40, (pl.decisions + pl.anticipation + pl.leadership + pl.teamwork) * 2 + pl.current_ability / 4 + 30)),
                    pl.home_reputation / 2, pl.current_reputation / 2, pl.world_reputation / 2,
                    MIN(20, MAX(1, (pl.flair + pl.off_the_ball) / 2)), 8,
                    MIN(20, MAX(1, (pl.teamwork + pl.leadership + pl.decisions) / 3)),
                    CASE WHEN pl.pos_goalkeeper >= 15 THEN MIN(20, MAX(1, (pl.handling + pl.reflexes) / 2)) ELSE 4 END,
                    MIN(20, MAX(1, pl.technique)), 10, MIN(20, MAX(1, p.professionalism)), MIN(20, MAX(1, pl.flair)),
                    10, MIN(20, MAX(1, (pl.vision + pl.anticipation) / 2)), MIN(20, MAX(1, (pl.vision + pl.decisions) / 2)),
                    MIN(20, MAX(1, (pl.leadership + p.determination) / 2)), MIN(20, MAX(1, pl.marking)),
                    MIN(20, MAX(1, pl.leadership)), MIN(20, MAX(1, pl.positioning)), 10, 3,
                    MIN(20, MAX(1, pl.work_rate)), 10, MIN(20, MAX(1, (pl.decisions + pl.positioning + pl.anticipation) / 3)), 10, NULL
             FROM players pl JOIN people p ON p.id = pl.person_id WHERE pl.person_id = ?",
            [id],
        )?;
        Ok(())
    }

    /// O regen de um reformado: um jogador de 16 a 18 anos da mesma nação, com o perfil dele (é o
    /// modelo dos atributos) e um potencial perto do que ele foi, num clube gerido da nação
    /// (os de mais reputação, mais vezes). Devolve o id, o clube e o nome com a posição.
    fn create_heir(
        &self,
        factory: &mut regen::Factory,
        id: i32,
        had_club: bool,
        rng: &mut ChaCha8Rng,
    ) -> Result<Option<(i32, i32, String)>> {
        let (nation, ca, pa, gk, def, mid, att): (Option<i32>, i32, i32, i32, i32, i32, i32) = self.world.conn.query_row(
            "SELECT p.nation_id, COALESCE(pl.current_ability, 1), COALESCE(pl.potential_ability, 0), COALESCE(pl.pos_goalkeeper, 0),
                    COALESCE(pl.pos_defender, 0), COALESCE(pl.pos_midfielder, 0), COALESCE(pl.pos_attacker, 0)
             FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.id = ?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
        )?;
        let Some(nation) = nation else { return Ok(None) };
        if !had_club && ca < HEIR_CA {
            return Ok(None);
        }
        let sql = format!(
            "SELECT c.id, COALESCE(c.reputation, 0) FROM clubs c WHERE c.nation_id = ?1 AND c.division_id IN ({})
               AND (SELECT COUNT(*) FROM people p WHERE p.club_id = c.id AND p.is_player = 1) < ?2 ORDER BY c.id",
            self.managed_sql(None)
        );
        let clubs: Vec<(i32, i32)> = self
            .world
            .conn
            .prepare(&sql)?
            .query_map(params![nation, HEIR_SQUAD], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        if clubs.is_empty() {
            return Ok(None);
        }
        let weights: Vec<f64> = clubs.iter().map(|&(_, rep)| 1.0 + (rep as f64 / 2000.0).powi(2)).collect();
        let mut pick = uniform(rng) * weights.iter().sum::<f64>();
        let mut club = clubs[0].0;
        for (&(c, _), w) in clubs.iter().zip(&weights) {
            if pick < *w {
                club = c;
                break;
            }
            pick -= w;
        }
        let best = gk.max(def).max(mid).max(att);
        let position = if gk == best {
            regen::GK
        } else if def == best {
            regen::DEF
        } else if mid == best {
            regen::MID
        } else {
            regen::ATT
        };
        let year: i32 = self.date[0..4].parse().unwrap_or(0);
        let age = 16 + (rng.next_u32() % 3) as i32;
        let new_ca = 30 + (rng.next_u32() % 26) as i32 + (age - 16) * 8;
        let anchor = ca.max(pa) as f64;
        let new_pa = ((anchor * (0.7 + 0.35 * uniform(rng))).round() as i32).clamp(new_ca + 10, 195);
        let p = regen::NewPlayer {
            club,
            nation: Some(nation),
            birth_year: year - age,
            ca: new_ca,
            pa: new_pa,
            position,
            contract_until: format!("{}-12-31", year + 3),
            template: Some(id),
        };
        Ok(self.create_player(factory, &p, rng)?.map(|(heir, name)| (heir, club, format!("{name} ({age} anos, {})", position.1))))
    }

    /// A safra dos juniores de um clube num ano: −1 fraca, 0 normal, 1 geração de ouro. A
    /// mesma no aviso de fevereiro e na chegada de março.
    pub(super) fn vintage(&self, club: i32, year: i32) -> i32 {
        let mut rng =
            ChaCha8Rng::seed_from_u64((self.seed as u64) ^ ((year as u64) << 32) ^ (club as u64).wrapping_mul(0x9E37_79B9) ^ 0x5AF2A);
        match uniform(&mut rng) {
            r if r < 0.06 => 1,
            r if r < 0.26 => -1,
            _ => 0,
        }
    }

    /// 1 de fevereiro: o responsável pela formação avisa como vem a safra do clube.
    pub(super) fn vintage_news(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if self.sacked() {
            return Ok(());
        }
        let year: i32 = self.date[0..4].parse().unwrap_or(0);
        let body = match self.vintage(self.club_id, year) {
            1 => {
                "O responsável pela formação está entusiasmado: a geração de juniores que chega em março promete muito, \
                  com vários jogadores de grande potencial."
            }
            -1 => "O responsável pela formação avisa que a geração de juniores que chega em março é fraca: não espere grandes talentos.",
            _ => "O responsável pela formação diz que a geração de juniores que chega em março é normal.",
        };
        self.push_news(fresh, next_id, "message", "Os próximos juniores".into(), body.into());
        Ok(())
    }

    /// A força da formação de cada nação: o CA médio dos 30 melhores jogadores dela, como pontos
    /// de potencial (Brasil e França ~+12, Malta ~−6).
    pub(super) fn nation_talent(&self) -> Result<std::collections::HashMap<i32, f64>> {
        let rows: Vec<(i32, f64)> = self
            .world
            .conn
            .prepare(
                "SELECT nation_id, AVG(ca) FROM (
                    SELECT p.nation_id, pl.current_ability AS ca,
                           ROW_NUMBER() OVER (PARTITION BY p.nation_id ORDER BY pl.current_ability DESC) AS n
                    FROM people p JOIN players pl ON pl.person_id = p.id
                    WHERE p.is_player = 1 AND p.nation_id IS NOT NULL AND p.id NOT IN (SELECT person_id FROM retired))
                 WHERE n <= 30 GROUP BY nation_id",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows.into_iter().map(|(n, ca)| (n, ((ca - 130.0) / 3.0).clamp(-8.0, 14.0))).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NewGame, World};

    #[test]
    fn a_veteran_announces_retires_and_leaves_an_heir() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-reformas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = NewGame {
            save_name: "Reformas".into(),
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
        let mut g = Game::create(&path, &dir, &new, 8).unwrap();
        let year: i32 = g.date[0..4].parse().unwrap();
        // O mais velho do plantel pode ser Jogador/Treinador: ganha ficha de staff e treina
        let old: i32 = g
            .world
            .conn
            .query_row("SELECT id FROM people WHERE club_id = 7568 AND is_player = 1 ORDER BY birth_year, id LIMIT 1", [], |r| r.get(0))
            .unwrap();
        g.world.conn.execute("UPDATE people SET birth_year = ?, birth_date = NULL WHERE id = ?", params![year - 34, old]).unwrap();
        let view = g.squad_role_view(old).unwrap();
        assert_eq!(view.jobs.len(), 3);
        assert!(g.set_squad_role(old, "job:12").unwrap().starts_with("Jogador/Treinador"));
        let (job, staff): (i32, bool) = g
            .world
            .conn
            .query_row(
                "SELECT club_job, (SELECT COUNT(*) > 0 FROM non_players WHERE person_id = id) FROM people WHERE id = ?",
                [old],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(job == 12 && staff);
        let profile = g.world.get_person(old).unwrap();
        assert!(profile.staff.is_some() && profile.player.is_some(), "as duas fichas");
        // Um jovem não pode
        let young: i32 = g
            .world
            .conn
            .query_row("SELECT id FROM people WHERE club_id = 7568 AND is_player = 1 ORDER BY birth_year DESC, id LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(g.squad_role_view(young).unwrap().jobs.is_empty() || g.set_squad_role(young, "job:13").is_ok());
        // Um jogador de campo do plantel com 40 anos e o contrato a acabar
        let id: i32 = g
            .world
            .conn
            .query_row(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = 7568 AND p.is_player = 1 AND pl.pos_goalkeeper < 15 ORDER BY pl.current_ability LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let soon = crate::date::add_days(&g.date, 20).unwrap();
        g.world
            .conn
            .execute("UPDATE people SET birth_year = ?, birth_date = NULL, contract_expires = ? WHERE id = ?", params![year - 41, soon, id])
            .unwrap();
        let (mut fresh, mut next_id) = (Vec::new(), 1000);
        g.announce_retirements(SeasonKind::Calendar, &mut fresh, &mut next_id).unwrap();
        assert!(g.retiring(id).unwrap().is_some(), "anunciou");
        assert!(g.player_status(id).unwrap().retiring);
        assert!(fresh.iter().any(|n| n.title.ends_with("vai terminar a carreira")));
        let before: i32 = g.world.conn.query_row("SELECT MAX(id) FROM people", [], |r| r.get(0)).unwrap();
        g.retirements(SeasonKind::Calendar, &mut fresh, &mut next_id).unwrap();
        let (age, staff): (i32, bool) =
            g.world.conn.query_row("SELECT age, staff FROM retirements WHERE person_id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert!(age >= 40);
        assert!(g.retiring(id).unwrap().is_none());
        let player: bool =
            g.world.conn.query_row("SELECT is_player = 1 AND club_id IS NOT NULL FROM people WHERE id = ?", [id], |r| r.get(0)).unwrap();
        assert!(!player, "já não joga");
        if staff {
            let has: bool = g.world.conn.query_row("SELECT COUNT(*) > 0 FROM non_players WHERE person_id = ?", [id], |r| r.get(0)).unwrap();
            assert!(has, "o staff tem ficha");
        }
        // O regen: brasileiro, 16 a 18 anos, criado agora
        let (nation, born): (Option<i32>, i32) = g
            .world
            .conn
            .query_row("SELECT nation_id, birth_year FROM people WHERE id > ? ORDER BY id LIMIT 1", [before], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        let theirs: Option<i32> = g.world.conn.query_row("SELECT nation_id FROM people WHERE id = ?", [id], |r| r.get(0)).unwrap();
        assert_eq!(nation, theirs);
        assert!((year - 18..=year - 16).contains(&born));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_vintage_is_the_same_in_february_and_march() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-safra-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = NewGame {
            save_name: "Safra".into(),
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
        let g = Game::create(&path, &dir, &new, 8).unwrap();
        let all: Vec<i32> = (0..400).map(|c| g.vintage(c, 2027)).collect();
        assert_eq!(all, (0..400).map(|c| g.vintage(c, 2027)).collect::<Vec<_>>());
        let golden = all.iter().filter(|&&v| v == 1).count();
        let weak = all.iter().filter(|&&v| v == -1).count();
        assert!((8..45).contains(&golden) && (50..120).contains(&weak), "{golden} {weak}");
        let talent = g.nation_talent().unwrap();
        // O Brasil forma melhor do que Malta
        let brazil: i32 =
            g.world.conn.query_row("SELECT id FROM nations WHERE name = 'Brasil' OR name = 'Brazil'", [], |r| r.get(0)).unwrap();
        assert!(talent[&brazil] > 5.0, "{}", talent[&brazil]);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn veteran(age: i32) -> Career {
        Career {
            age,
            ca: 120,
            contract_days: Some(100),
            apps: 10,
            professionalism: 10,
            ambition: 10,
            injury_proneness: 10,
            ..Default::default()
        }
    }

    #[test]
    fn age_level_contract_and_minutes_weigh() {
        assert_eq!(retire_chance(&veteran(30)).0, 0.0);
        assert!(retire_chance(&veteran(36)).0 > retire_chance(&veteran(34)).0);
        // O craque e o guarda-redes duram mais
        let star = Career { ca: 170, ..veteran(35) };
        let keeper = Career { goalkeeper: true, ..veteran(35) };
        assert!(retire_chance(&star).0 < retire_chance(&veteran(35)).0);
        assert!(retire_chance(&keeper).0 < retire_chance(&veteran(35)).0);
        // Com contrato longo raramente para; sem clube, muito mais
        let signed = Career { contract_days: Some(700), ..veteran(35) };
        let free = Career { contract_days: None, ..veteran(35) };
        assert!(retire_chance(&signed).0 < 0.5 * retire_chance(&veteran(35)).0);
        let (p, why) = retire_chance(&free);
        assert!(p > retire_chance(&veteran(35)).0 && why == "sem clube");
        // Quem já não joga para mais cedo
        let (p, why) = retire_chance(&Career { apps: 1, ..veteran(35) });
        assert!(p > retire_chance(&veteran(35)).0 && why == "falta de minutos");
        assert_eq!(retire_chance(&Career { contract_days: None, ..veteran(45) }).0, 1.0);
    }
}
