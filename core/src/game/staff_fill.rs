//! Os atributos de staff que o banco traz vazios. No patch, ~73% do staff não tem atributos
//! (a Roma: o departamento médico, quase todos os preparadores e olheiros); o CM gera-os ao
//! começar o jogo e o perfil mostra-os. Aqui saem do CA e da função: os da função perto do
//! nível dele (CA/10), os outros abaixo; os de estilo (atacar, pressionar...) e a
//! personalidade à volta do meio. Cada pessoa tem o seu sorteio (a seed do jogo e o id).

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};

use super::Game;
use crate::Result;
use crate::attributes::{STAFF_RELATIVE, displayed, initial_intrinsic};

/// Os atributos de staff (`non_players`) que se geram.
const SKILLS: [&str; 21] = [
    "attacking",
    "business",
    "coaching",
    "coaching_gks",
    "coaching_technique",
    "directness",
    "discipline",
    "free_roles",
    "interference",
    "judgement",
    "judging_potential",
    "man_handling",
    "marking",
    "motivating",
    "offside",
    "patience",
    "physiotherapy",
    "pressing",
    "resources",
    "tactics",
    "youngsters",
];
/// Os de estilo de jogo (preferências, não qualidade): à volta do meio.
const STYLE: [&str; 6] = ["attacking", "directness", "free_roles", "marking", "offside", "pressing"];
/// A personalidade (`people`), quando falta.
const MENTAL: [&str; 8] =
    ["adaptability", "ambition", "determination", "loyalty", "pressure", "professionalism", "sportsmanship", "temperament"];
/// CA de quem não tem (o banco traz 0 em alguns).
const DEFAULT_CA: i32 = 80;

/// Os atributos de cada função (o cargo do CM): perto do nível; os outros ficam abaixo.
fn focus(job: i32) -> &'static [&'static str] {
    match job {
        // Presidente, directores
        1..=4 => &["business", "resources", "interference", "patience", "judgement"],
        // Treinador, adjunto, interino, jogador-treinador
        5 | 6 | 12 | 15 => &["coaching", "tactics", "motivating", "man_handling", "coaching_technique", "discipline", "judgement"],
        // Treinador da equipa B
        7 => &["youngsters", "coaching", "coaching_technique", "judging_potential", "motivating"],
        // Preparador e jogador-preparador
        8 | 13 => &["coaching", "coaching_technique", "motivating", "youngsters", "discipline", "judgement"],
        // Olheiro
        9 => &["judgement", "judging_potential"],
        // Fisioterapeuta
        10 | 14 => &["physiotherapy", "motivating"],
        _ => &["coaching", "motivating"],
    }
}

/// Um número de `lo` a `hi` (inclusive).
fn roll(rng: &mut ChaCha8Rng, lo: i32, hi: i32) -> i32 {
    lo + (rng.next_u32() % (hi - lo + 1) as u32) as i32
}

/// O valor do banco que um atributo relativo precisa para o perfil mostrar `shown` com este CA.
fn relative_raw(shown: i32, ca: i32) -> i32 {
    (1..=20).min_by_key(|&v| ((displayed(initial_intrinsic(v, ca), ca, false) - shown).abs(), v)).unwrap_or(10)
}

impl Game {
    /// Preenche os atributos vazios do staff (todos os do banco, não os que já têm valor).
    pub(super) fn fill_staff_attributes(&self) -> Result<()> {
        let conn = &self.world.conn;
        let skills = SKILLS;
        let cols = skills.join(", ");
        let sql = format!(
            "SELECT n.person_id, COALESCE(p.club_job, 0), COALESCE(n.current_ability, 0), {cols}, {}
             FROM non_players n JOIN people p ON p.id = n.person_id
             WHERE {} ORDER BY n.person_id",
            MENTAL.iter().map(|m| format!("p.{m}")).collect::<Vec<_>>().join(", "),
            skills
                .iter()
                .map(|k| format!("n.{k} IS NULL"))
                .chain(MENTAL.iter().map(|m| format!("p.{m} IS NULL")))
                .collect::<Vec<_>>()
                .join(" OR ")
        );
        type Row = (i32, i32, i32, Vec<Option<i32>>);
        let rows: Vec<Row> = conn
            .prepare(&sql)?
            .query_map([], |r| {
                let n = skills.len() + MENTAL.len();
                let mut v = Vec::with_capacity(n);
                for i in 0..n {
                    v.push(r.get::<_, Option<i32>>(3 + i)?);
                }
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, v))
            })?
            .collect::<rusqlite::Result<_>>()?;
        for (id, job, ca, values) in rows {
            let mut rng = ChaCha8Rng::seed_from_u64((self.seed as u64) ^ (id as u64).wrapping_mul(0x5DEE_CE66_D1CE_4E5B) ^ 0x57AF_F11E);
            let ca = if ca > 0 { ca } else { DEFAULT_CA };
            let level = (ca / 10).clamp(4, 18);
            let main = focus(job);
            let mut sets: Vec<String> = Vec::new();
            let mut args: Vec<i32> = Vec::new();
            for (i, key) in skills.iter().enumerate() {
                if values[i].is_some() {
                    continue;
                }
                let shown = if STYLE.contains(key) {
                    roll(&mut rng, 5, 15)
                } else if main.contains(key) {
                    (level + roll(&mut rng, -2, 3)).clamp(1, 20)
                } else {
                    (level - 5 + roll(&mut rng, -3, 3)).clamp(1, 20)
                };
                let raw = if STAFF_RELATIVE.contains(key) { relative_raw(shown, ca) } else { shown };
                sets.push(format!("{key} = ?"));
                args.push(raw);
            }
            if !sets.is_empty() {
                args.push(id);
                conn.execute(
                    &format!("UPDATE non_players SET {} WHERE person_id = ?", sets.join(", ")),
                    rusqlite::params_from_iter(&args),
                )?;
            }
            let mut mental_sets: Vec<String> = Vec::new();
            let mut mental_args: Vec<i32> = Vec::new();
            for (j, key) in MENTAL.iter().enumerate() {
                if values[skills.len() + j].is_none() {
                    mental_sets.push(format!("{key} = ?"));
                    mental_args.push(roll(&mut rng, 6, 15));
                }
            }
            if !mental_sets.is_empty() {
                mental_args.push(id);
                conn.execute(
                    &format!("UPDATE people SET {} WHERE id = ?", mental_sets.join(", ")),
                    rusqlite::params_from_iter(&mental_args),
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_values_invert_the_display() {
        // Com CA 190 o perfil mostra ~10 + v/2: para mostrar 16 o banco precisa de ~15
        let v = relative_raw(16, 190);
        assert_eq!(displayed(initial_intrinsic(v, 190), 190, false), 16);
        assert!(focus(10).contains(&"physiotherapy") && focus(9).contains(&"judgement"));
    }

    #[test]
    fn roma_staff_get_attributes() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-staff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Staff".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 8609,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let g = Game::create(&path, &dir, &new, 4).unwrap();
        let empty: i64 = g
            .world
            .conn
            .query_row(
                "SELECT COUNT(*) FROM non_players n JOIN people p ON p.id = n.person_id WHERE p.club_id = 8609
                   AND (n.coaching IS NULL OR n.physiotherapy IS NULL OR n.judgement IS NULL OR p.determination IS NULL)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(empty, 0);
        // O fisioterapeuta Cardini (CA 150) é bom no que faz
        let physio: i32 =
            g.world.conn.query_row("SELECT physiotherapy FROM non_players WHERE person_id = 19408", [], |r| r.get(0)).unwrap();
        let shown = displayed(initial_intrinsic(physio, 150), 150, false);
        assert!(shown >= 12, "{shown}");
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
