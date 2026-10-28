//! Equipa de reservas (marco 3.12, passo 2): no menu Acção, Despromover para Reservas e
//! Promover ao Plantel Principal. Os das reservas saem da escolha da equipa principal
//! (`Availability::reserve`), e a despromoção mexe na moral, mais em quem se acha titular.
//! Regras no detalhe do 3.12 no ROADMAP.

use super::Game;
use crate::{Error, Result};

/// Jogadores que a equipa principal mantém, no mínimo.
const MIN_FIRST_TEAM: usize = 16;
/// Ao chegar a um clube, os jovens (até esta idade) de reputação baixa que não estão entre os
/// melhores do plantel começam nas reservas, como no CM; os juniores novos também.
const YOUTH_AGE: i32 = 19;
const YOUTH_REP: i32 = 50;
const FIRST_TEAM_CORE: usize = 22;
/// Moral que perde quem desce (o dobro se for dos 11 melhores) e que ganha quem sobe.
const DEMOTED_MORALE: i32 = 2;
const PROMOTED_MORALE: i32 = 1;

impl Game {
    /// Os jogadores do plantel do treinador, do melhor para o pior (CA).
    fn own_squad(&self) -> Result<Vec<i32>> {
        Ok(self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1 ORDER BY pl.current_ability DESC, p.id",
            )?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Os jovens de reputação baixa fora dos [`FIRST_TEAM_CORE`] melhores vão para as reservas.
    pub(super) fn youth_to_reserves(&mut self) -> Result<()> {
        let squad = self.own_squad()?;
        for &id in squad.iter().skip(FIRST_TEAM_CORE) {
            let (birth, birth_year, rep): (Option<String>, Option<i32>, i32) = self.world.conn.query_row(
                "SELECT p.birth_date, p.birth_year, COALESCE(pl.current_reputation, 0) FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.id = ?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            if super::age_on(birth.as_deref(), birth_year, &self.date) <= YOUTH_AGE && rep < YOUTH_REP {
                self.squads.states.entry(id).or_default().reserve = true;
            }
        }
        self.unsaved = true;
        Ok(())
    }

    /// Despromove (`reserve = true`) ou promove um jogador; devolve a frase para a tela.
    pub fn set_reserve(&mut self, person: i32, reserve: bool) -> Result<String> {
        let squad = self.own_squad()?;
        let Some(rank) = squad.iter().position(|&id| id == person) else {
            return Err(Error::Invalid("Só para os jogadores do seu plantel.".into()));
        };
        if self.squads.state(person).reserve == reserve {
            return Err(Error::Invalid(if reserve { "Já está nas reservas." } else { "Já está no plantel principal." }.into()));
        }
        let name = self.person_name(person)?;
        if reserve {
            let first_team = squad.iter().filter(|id| !self.squads.state(**id).reserve).count();
            if first_team <= MIN_FIRST_TEAM {
                return Err(Error::Invalid(format!("O plantel principal precisa de pelo menos {MIN_FIRST_TEAM} jogadores.")));
            }
        }
        self.squads.states.entry(person).or_default().reserve = reserve;
        let text = if reserve {
            // Quem é dos 11 melhores acha-se titular: custa-lhe mais
            let lost = if rank < 11 { 2 * DEMOTED_MORALE } else { DEMOTED_MORALE };
            self.change_morale(person, -lost);
            format!("{name} foi despromovido para a equipa de reservas.")
        } else {
            self.change_morale(person, PROMOTED_MORALE);
            format!("{name} foi promovido ao plantel principal.")
        };
        self.clean_selection()?;
        self.refresh_live()?;
        self.unsaved = true;
        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn reserves_leave_the_first_team() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-reservas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Reservas".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
            leagues: Vec::new(),
            start: Default::default(),
        };
        let mut g = Game::create(&path, &dir, &new, 31).unwrap();
        let squad = g.own_squad().unwrap();
        let star = squad[0];
        let in_team = |g: &Game, id: i32| {
            let t = g.club_tactics(PALMEIRAS).unwrap();
            t.starters.iter().filter_map(|s| s.player.as_ref()).chain(&t.bench).any(|p| p.id == id)
        };
        assert!(in_team(&g, star));
        let before = g.squads.state(star).morale;
        assert!(g.set_reserve(star, true).unwrap().contains("reservas"));
        assert!(!in_team(&g, star), "fora da equipa principal");
        assert_eq!(g.squads.state(star).morale, (before - 2 * DEMOTED_MORALE).max(1));
        assert!(g.player_status(star).unwrap().reserve);
        assert!(g.set_reserve(star, true).is_err());
        // Volta: entra outra vez nas contas do adjunto
        g.set_reserve(star, false).unwrap();
        assert!(in_team(&g, star));
        // Não deixa o plantel principal com menos de 16
        let mut refused = false;
        for &id in squad.iter().rev() {
            // Os jovens começam já nas reservas
            if g.squads.state(id).reserve {
                continue;
            }
            if g.set_reserve(id, true).is_err() {
                refused = true;
                break;
            }
        }
        assert!(refused);
        assert_eq!(squad.iter().filter(|id| !g.squads.state(**id).reserve).count(), MIN_FIRST_TEAM);
        // Grava e carrega: as reservas ficam
        let reserved = *squad.last().unwrap();
        g.save().unwrap();
        let file = g.path().to_path_buf();
        drop(g);
        let g = Game::load(&file).unwrap();
        assert!(g.squads.state(reserved).reserve);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
