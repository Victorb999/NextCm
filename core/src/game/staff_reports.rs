//! Relatórios do staff sobre um jogador do plantel (marco 3.10, passo 3): menu Acção do perfil,
//! "Pedir Relatório Médico" (fisioterapeutas) e "Pedir Relatório Preparador Físico"
//! (preparadores e o adjunto). Cada um escolhido manda a sua mensagem na hora; a opinião é mais
//! certa quanto melhor for quem a escreve. Regras no detalhe do 3.10 no ROADMAP.

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use serde::Serialize;
use ts_rs::TS;

use super::{Game, cm_date};
use crate::{Error, Result};

/// Jobs em `jobs`: adjunto, preparador, fisioterapeuta e os jogadores que também o são.
const ASSISTANT: i32 = 6;
const COACH: [i32; 2] = [8, 13];
const PHYSIO: [i32; 2] = [10, 14];

/// Os atributos de que um preparador fala, com o nome do CM.
const STRENGTHS: [(&str, &str); 24] = [
    ("acceleration", "aceleração"),
    ("agility", "agilidade"),
    ("anticipation", "antecipação"),
    ("bravery", "bravura"),
    ("heading", "jogo aéreo"),
    ("vision", "criatividade"),
    ("crossing", "cruzamentos"),
    ("decisions", "decisões"),
    ("tackling", "desarme"),
    ("determination", "determinação"),
    ("finishing", "finalização"),
    ("dribbling", "finta"),
    ("strength", "força"),
    ("work_rate", "trabalho"),
    ("leadership", "liderança"),
    ("flair", "instinto"),
    ("marking", "marcação"),
    ("passing", "passe"),
    ("positioning", "posicionamento"),
    ("long_shots", "remates de longe"),
    ("stamina", "resistência"),
    ("off_the_ball", "movimentação sem bola"),
    ("technique", "técnica"),
    ("pace", "velocidade"),
];
const KEEPER: [(&str, &str); 4] =
    [("handling", "jogo de mãos"), ("reflexes", "reflexos"), ("positioning", "posicionamento"), ("decisions", "decisões")];

/// Quem do staff pode fazer um relatório.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ReportStaff {
    pub id: i32,
    pub name: String,
    pub job: String,
}

/// "Sobrenome, Nome" → "Sobrenome" (o nome comum fica como está).
fn surname(list_name: &str) -> &str {
    list_name.split(',').next().unwrap_or(list_name).trim()
}

impl Game {
    /// O staff que pode fazer o relatório: "medical" (fisioterapeutas) ou "fitness" (adjunto e
    /// preparadores), o melhor no ofício primeiro.
    pub fn report_staff(&self, kind: &str) -> Result<Vec<ReportStaff>> {
        let (jobs, skill) = match kind {
            "medical" => (PHYSIO.to_vec(), "np.physiotherapy"),
            "fitness" => ([vec![ASSISTANT], COACH.to_vec()].concat(), "np.coaching"),
            _ => return Err(Error::Invalid("Relatório desconhecido.".into())),
        };
        let list = jobs.iter().map(|j| j.to_string()).collect::<Vec<_>>().join(", ");
        Ok(self
            .world
            .conn
            .prepare(&format!(
                "SELECT p.id, p.display_name, j.name FROM people p JOIN jobs j ON j.id = p.club_job
                 LEFT JOIN non_players np ON np.person_id = p.id
                 WHERE p.club_id = ? AND p.club_job IN ({list}) ORDER BY p.club_job = {ASSISTANT} DESC, COALESCE({skill}, 10) DESC, p.id"
            ))?
            .query_map([self.club_id], |r| Ok(ReportStaff { id: r.get(0)?, name: r.get(1)?, job: r.get(2)? }))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Pede os relatórios sobre um jogador do plantel a quem foi escolhido; as mensagens chegam
    /// já. Devolve quantas chegaram.
    pub fn staff_report(&mut self, person: i32, kind: &str, staff: &[i32]) -> Result<usize> {
        let club: Option<i32> = self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [person], |r| r.get(0))?;
        if club != Some(self.club_id) {
            return Err(Error::Invalid("Só se pedem relatórios sobre os jogadores do plantel.".into()));
        }
        let allowed: Vec<i32> = self.report_staff(kind)?.iter().map(|s| s.id).collect();
        let chosen: Vec<i32> = staff.iter().copied().filter(|s| allowed.contains(s)).collect();
        if chosen.is_empty() {
            return Err(Error::Invalid("Escolha quem faz o relatório.".into()));
        }
        let p = self.world.get_person(person)?;
        let pl = p.player.as_ref().ok_or_else(|| Error::Invalid("Só jogadores.".into()))?;
        let short = surname(&p.list_name).to_string();
        for &s in &chosen {
            let (name, list_name, physio, coaching, judgement): (String, String, i32, i32, i32) = self.world.conn.query_row(
                "SELECT p.display_name, p.list_name, COALESCE(np.physiotherapy, 10), COALESCE(np.coaching, 10), COALESCE(np.judgement, 10)
                 FROM people p LEFT JOIN non_players np ON np.person_id = p.id WHERE p.id = ?",
                [s],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )?;
            let who = surname(&list_name).to_string();
            // Um sorteio fixo por quem escreve, jogador e dia
            let day = crate::date::parse(&self.date).unwrap_or(0) as u64;
            let mut rng = ChaCha8Rng::seed_from_u64((self.seed as u64) ^ (s as u64) << 20 ^ (person as u64) << 40 ^ day);
            let (title, body) = if kind == "medical" {
                (format!("{who} faz ponto da situação de {short}"), self.medical_text(person, &name, &p.name, physio, &mut rng)?)
            } else {
                let skill = (coaching + judgement) / 2;
                (
                    format!("{who} apresenta relatório sobre {short}"),
                    self.fitness_text(person, &name, &p.name, &pl.position_long, &pl.attributes, skill, &mut rng)?,
                )
            };
            self.post_news(title, body, None);
        }
        self.unsaved = true;
        Ok(chosen.len())
    }

    /// O médico: a lesão, o cansaço e a Propensão a Lesões (vista com o erro do médico).
    fn medical_text(&self, person: i32, physio: &str, name: &str, skill: i32, rng: &mut ChaCha8Rng) -> Result<String> {
        let st = self.squads.state(person);
        if let (Some(injury), Some(until)) = (&st.injury, &st.injury_until) {
            return Ok(format!(
                "{physio} informa que {name} está lesionado ({}) e só deve voltar a {}.",
                injury.to_lowercase(),
                cm_date(until)
            ));
        }
        let prone: i32 =
            self.world.conn.query_row("SELECT COALESCE(injury_proneness, 10) FROM players WHERE person_id = ?", [person], |r| r.get(0))?;
        // Erro de ±6 (Fisioterapia 1) a ±0 (20)
        let spread = (20 - skill.clamp(1, 20)) as f64 * 6.0 / 19.0;
        let seen = prone as f64 + ((rng.next_u64() % 2001) as f64 / 1000.0 - 1.0) * spread;
        let mut text = if seen >= 15.0 {
            format!("{physio} alerta no seu relatório que {name} tem tendência para se lesionar e aconselha a poupá-lo em jogos seguidos.")
        } else if seen >= 11.0 {
            format!("{physio} refere no seu relatório que {name} está bem fisicamente, mas convém vigiá-lo.")
        } else {
            format!("{physio} mencionou no seu relatório que não tem qualquer tipo de preocupação sobre a situação física de {name}.")
        };
        if st.condition < 80 {
            text.push_str(&format!("\n\nAcrescenta que {name} está cansado ({}% de condição) e precisa de descanso.", st.condition));
        }
        Ok(text)
    }

    /// O preparador: a opinião sobre o lugar no plantel (pelo CA, com o erro de quem escreve),
    /// os pontos fortes e, se houver, um ponto fraco.
    #[allow(clippy::too_many_arguments)]
    fn fitness_text(
        &self,
        person: i32,
        coach: &str,
        name: &str,
        position: &str,
        attributes: &crate::Attributes,
        skill: i32,
        rng: &mut ChaCha8Rng,
    ) -> Result<String> {
        let cas: Vec<(i32, i32)> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, COALESCE(pl.current_ability, 1) FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1",
            )?
            .query_map([self.club_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let ca = cas.iter().find(|c| c.0 == person).map_or(1, |c| c.1);
        // Erro de ±20 de CA (1) a ±2 (20)
        let spread = 2.0 + 18.0 * (20 - skill.clamp(1, 20)) as f64 / 19.0;
        let seen = ca as f64 + ((rng.next_u64() % 2001) as f64 / 1000.0 - 1.0) * spread;
        let rank = cas.iter().filter(|c| c.0 != person && c.1 as f64 > seen).count();
        let (opening, role) = match rank {
            0..=2 => ("é um grande admirador das qualidades de", "um elemento chave do plantel actual"),
            3..=10 => ("aprecia as qualidades de", "um titular no plantel actual"),
            11..=17 => ("vê com bons olhos", "uma boa opção para o plantel"),
            _ => ("tem dúvidas sobre", "alguém com pouco lugar no plantel actual"),
        };
        let pos = position.split(['/', '(']).next().unwrap_or(position).trim().to_lowercase();
        let keeper = position.starts_with("Guarda-Redes");
        let mut ranked: Vec<(&str, i32)> = (if keeper { &KEEPER[..] } else { &STRENGTHS[..] })
            .iter()
            .filter_map(|(k, label)| attributes.get(*k).copied().flatten().map(|v| (*label, v)))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        let mut text = format!("{coach} {opening} {name}.\n\nEle vê o {pos} como {role}.");
        if ranked.len() >= 2 && ranked[0].1 >= 13 {
            text.push_str(&format!(
                "\n\nO relatório também referencia favoravelmente {name} em termos de {} e ainda {}.",
                ranked[0].0, ranked[1].0
            ));
        }
        if let Some(weak) = ranked.iter().rev().find(|r| r.1 <= 8) {
            text.push_str(&format!(" Aponta, no entanto, que tem de melhorar a {}.", weak.0));
        }
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn staff_write_reports_on_the_squad() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-relatorios-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Relatórios".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
            leagues: Vec::new(),
            start: Default::default(),
        };
        let mut g = Game::create(&path, &dir, &new, 13).unwrap();
        let star: i32 = g
            .world
            .conn
            .query_row(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND p.is_player = 1
                 ORDER BY pl.current_ability DESC LIMIT 1",
                [PALMEIRAS],
                |r| r.get(0),
            )
            .unwrap();
        let physios = g.report_staff("medical").unwrap();
        let coaches = g.report_staff("fitness").unwrap();
        assert!(!physios.is_empty() && coaches.len() >= 2, "{physios:?} {coaches:?}");
        assert!(g.staff_report(star, "medical", &[]).is_err());
        let other: i32 = g
            .world
            .conn
            .query_row("SELECT id FROM people WHERE club_id <> ? AND is_player = 1 LIMIT 1", [PALMEIRAS], |r| r.get(0))
            .unwrap();
        assert!(g.staff_report(other, "medical", &[physios[0].id]).is_err(), "só do plantel");

        let before = g.news().len();
        assert_eq!(g.staff_report(star, "medical", &[physios[0].id]).unwrap(), 1);
        let ids: Vec<i32> = coaches.iter().map(|c| c.id).collect();
        assert_eq!(g.staff_report(star, "fitness", &ids).unwrap(), coaches.len());
        assert_eq!(g.news().len(), before + 1 + coaches.len());
        let medical = g.news().iter().find(|n| n.title.contains("faz ponto da situação")).unwrap();
        assert!(medical.body.contains("relatório"), "{}", medical.body);
        let fitness = g.news().iter().find(|n| n.title.contains("apresenta relatório")).unwrap();
        // O melhor do plantel: o preparador acha-o pelo menos titular
        assert!(fitness.body.contains("elemento chave") || fitness.body.contains("titular"), "{}", fitness.body);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
