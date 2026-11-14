//! Equipa de reservas (marco 3.12, passo 2, e passo 3.19.2): no menu Acção, Despromover para
//! Reservas e Promover ao Plantel Principal. Os das reservas (`Availability::reserve`) ficam
//! fora das escolhas do adjunto, mas o treinador pode relacioná-los; a despromoção mexe na
//! moral, mais em quem se acha titular. Como no CM, depende da equipa B do clube
//! ([`ReserveMode`]): a "II" (a Amateure do Bayern) faz de reservas e a "B" (Barça B, Benfica B)
//! é um clube à parte, para onde se relega e de onde se promove.
//! Regras em docs/roadmap/3.12-empregos-e-gestao-do-plantel.md e 3.19.2-reservas-e-equipas-b.md.

use rand_core::RngCore;

use rusqlite::OptionalExtension;

use super::Game;
use super::regen::{ATT, DEF, GK, MID, NewPlayer};
use crate::{Error, Result};

/// Jogadores que a equipa principal mantém, no mínimo.
const MIN_FIRST_TEAM: usize = 16;
/// Ao chegar a um clube, o plantel principal fica com estes (os melhores guarda-redes e os
/// melhores dos outros, por CA) e o resto vai para as reservas, como no CM.
const FIRST_TEAM_KEEPERS: usize = 2;
const FIRST_TEAM_SIZE: usize = 22;
/// Jogadores que as reservas têm ao começar: os que faltam são jogadores de enchimento do
/// clube (a roxo), como no CM, por funções (GR, D, M, A).
const RESERVE_SQUAD: [usize; 4] = [2, 5, 5, 4];
/// Moral que perde quem desce (o dobro se for dos 11 melhores) e que ganha quem sobe.
const DEMOTED_MORALE: i32 = 2;
const PROMOTED_MORALE: i32 = 1;

/// Como funcionam as reservas do clube do treinador.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ReserveMode {
    /// Sem equipa B: as reservas são os marcados (`reserve`) do próprio plantel.
    Flag,
    /// Com uma "II" numa liga que o jogo não joga (o Bayern II): ela é a equipa de reservas. Os
    /// jogadores dela passam ao clube, nas reservas, e o ecrã das reservas tem o nome dela.
    Second(i32),
    /// Com uma "B" (ou uma "II" numa liga jogada): clube à parte, sem reservas; relegar e
    /// promover mudam o jogador de clube.
    BTeam(i32),
}

impl Game {
    /// As reservas de um clube, pela equipa B dele.
    pub(super) fn reserve_mode_of(&self, club: i32) -> Result<ReserveMode> {
        Ok(match self.world.b_team(club)? {
            None => ReserveMode::Flag,
            Some(b) if b.second && !self.matches.iter().any(|m| m.competition_id == b.division) => ReserveMode::Second(b.club),
            Some(b) => ReserveMode::BTeam(b.club),
        })
    }

    pub(super) fn reserve_mode(&self) -> Result<ReserveMode> {
        self.reserve_mode_of(self.club_id)
    }

    /// O nome do ecrã das reservas do treinador ("Reservas Manchester United", ou o da II, como
    /// no CM); `None` com uma equipa B, que não tem reservas.
    pub(super) fn reserves_name(&self) -> Result<Option<String>> {
        Ok(match self.reserve_mode()? {
            ReserveMode::Flag => Some(format!("Reservas {}", self.club(self.club_id).name)),
            ReserveMode::Second(b) => Some(self.club(b).name),
            ReserveMode::BTeam(_) => None,
        })
    }

    /// A acção do menu Acção que muda o jogador de plantel (chave e texto), se houver.
    pub(super) fn squad_move(&self, person: i32) -> Result<Option<(&'static str, &'static str)>> {
        if self.is_ghost(person) || self.sacked() {
            return Ok(None);
        }
        let club: Option<i32> = self
            .world
            .conn
            .query_row("SELECT club_id FROM people WHERE id = ? AND is_player = 1", [person], |r| r.get(0))
            .optional()?
            .flatten();
        let Some(club) = club else { return Ok(None) };
        Ok(match self.reserve_mode()? {
            ReserveMode::BTeam(b) if club == b && self.loan_of(person)?.is_none() => Some(("promote", "Promover à Equipa 'A'")),
            ReserveMode::BTeam(_) if club == self.club_id && self.loan_of(person)?.is_none() => Some(("demote", "Relegar para Equipa 'B'")),
            ReserveMode::BTeam(_) => None,
            _ if club != self.club_id => None,
            _ if self.squads.state(person).reserve => Some(("promote", "Promover ao Plantel Principal")),
            _ => Some(("demote", "Despromover para Reservas")),
        })
    }

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

    /// Ao chegar ao clube, como no CM: com uma "II", o plantel dela passa às reservas; depois o
    /// plantel principal fica com [`FIRST_TEAM_SIZE`] (os [`FIRST_TEAM_KEEPERS`] melhores GR e
    /// os melhores dos outros) e o resto desce, de qualquer idade. As funções que ficam por
    /// preencher nas reservas ([`RESERVE_SQUAD`]) recebem jogadores de enchimento. Com uma "B"
    /// não há reservas nem corte.
    pub(super) fn start_reserves(&mut self) -> Result<()> {
        let absorbed = match self.reserve_mode()? {
            ReserveMode::BTeam(_) => return Ok(()),
            ReserveMode::Second(b) => self.absorb_second_team(b)?,
            ReserveMode::Flag => Vec::new(),
        };
        let (mut keepers, mut others) = (0, 0);
        for id in self.own_squad()? {
            if absorbed.contains(&id) || self.squads.state(id).reserve {
                continue;
            }
            let keep = if self.is_keeper(id)? {
                keepers += 1;
                keepers <= FIRST_TEAM_KEEPERS
            } else {
                others += 1;
                others <= FIRST_TEAM_SIZE - FIRST_TEAM_KEEPERS
            };
            if !keep {
                self.squads.states.entry(id).or_default().reserve = true;
            }
        }
        self.reserve_ghosts()?;
        self.unsaved = true;
        Ok(())
    }

    /// Saves de antes do passo 3.19.2: com uma "B", os despromovidos voltam ao plantel principal
    /// (os de enchimento vão para a B); com uma "II", o plantel dela passa às reservas. Sem
    /// equipa B fica como estava (sem novo corte a meio da época).
    pub(super) fn migrate_reserves(&mut self) -> Result<()> {
        match self.reserve_mode()? {
            ReserveMode::Flag => return Ok(()),
            ReserveMode::Second(b) => {
                self.absorb_second_team(b)?;
            }
            ReserveMode::BTeam(b) => {
                for id in self.own_squad()? {
                    if !self.squads.state(id).reserve {
                        continue;
                    }
                    self.squads.states.entry(id).or_default().reserve = false;
                    if self.is_ghost(id) {
                        self.world.conn.execute("UPDATE people SET club_id = ? WHERE id = ?", [b, id])?;
                    }
                }
                self.world.conn.execute("DELETE FROM reserve_picks", [])?;
                super::numbers::fix_squad_numbers(&self.world, &[self.club_id, b], self.seed, None)?;
                self.clean_selection()?;
                self.refresh_live()?;
            }
        }
        self.unsaved = true;
        Ok(())
    }

    /// Os jogadores da "II" passam ao clube do treinador, nas reservas: os emprestados a ela
    /// também (a azul, como no CM; os do próprio clube deixam de estar emprestados), e os que
    /// ela emprestou passam a ser do clube. Devolve-os.
    fn absorb_second_team(&mut self, second: i32) -> Result<Vec<i32>> {
        let ids: Vec<i32> = self
            .world
            .conn
            .prepare("SELECT id FROM people WHERE club_id = ? AND is_player = 1")?
            .query_map([second], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for &id in &ids {
            self.world.conn.execute("UPDATE people SET club_id = ? WHERE id = ?", [self.club_id, id])?;
            self.squads.states.entry(id).or_default().reserve = true;
        }
        self.world.conn.execute("UPDATE loans SET parent_id = ? WHERE parent_id = ?", [self.club_id, second])?;
        self.world.conn.execute(
            "DELETE FROM loans WHERE parent_id = ?1 AND person_id IN (SELECT id FROM people WHERE club_id = ?1)",
            [self.club_id],
        )?;
        if !ids.is_empty() {
            super::numbers::fix_squad_numbers(&self.world, &[self.club_id], self.seed, None)?;
            self.refresh_live()?;
        }
        Ok(ids)
    }

    /// Completa as reservas do treinador com jogadores de enchimento, como o CM faz.
    fn reserve_ghosts(&mut self) -> Result<()> {
        let positions = [GK, DEF, MID, ATT];
        let mut have = [0usize; 4];
        for id in self.own_squad()? {
            if !self.squads.state(id).reserve {
                continue;
            }
            let notes: [i32; 4] = self.world.conn.query_row(
                "SELECT pos_goalkeeper, pos_defender, pos_midfielder, pos_attacker FROM players WHERE person_id = ?",
                [id],
                |r| {
                    Ok([
                        r.get::<_, Option<i32>>(0)?.unwrap_or(0),
                        r.get::<_, Option<i32>>(1)?.unwrap_or(0),
                        r.get::<_, Option<i32>>(2)?.unwrap_or(0),
                        r.get::<_, Option<i32>>(3)?.unwrap_or(0),
                    ])
                },
            )?;
            // Conta na função em que é melhor
            let best = (0..4).max_by_key(|&i| (notes[i], std::cmp::Reverse(i))).unwrap_or(2);
            have[best] += 1;
        }
        let need: Vec<_> = (0..4).flat_map(|i| std::iter::repeat_n(positions[i], RESERVE_SQUAD[i].saturating_sub(have[i]))).collect();
        if need.is_empty() {
            return Ok(());
        }
        let year = self.season();
        let level = self.ghost_ability(self.club_id)?;
        let nation: Option<i32> = self.world.conn.query_row("SELECT nation_id FROM clubs WHERE id = ?", [self.club_id], |r| r.get(0))?;
        let mut rng = self.season_rng(0x0005_E7E5);
        let mut factory = self.player_factory()?;
        let mut created = false;
        for position in need {
            let ca = (level * (0.85 + 0.3 * crate::engine::uniform(&mut rng))).round().clamp(30.0, 170.0) as i32;
            let p = NewPlayer {
                club: self.club_id,
                nation,
                birth_year: year - 18 - (rng.next_u32() % 6) as i32,
                ca,
                pa: ca + 5 + (rng.next_u32() % 25) as i32,
                position,
                contract_until: format!("{}-06-30", year + 2),
                template: None,
            };
            if let Some((id, _)) = self.create_player(&mut factory, &p, &mut rng)? {
                self.world.conn.execute("INSERT OR IGNORE INTO ghosts (person_id) VALUES (?)", [id])?;
                self.squads.states.entry(id).or_default().reserve = true;
                created = true;
            }
        }
        super::numbers::fix_squad_numbers(&self.world, &[self.club_id], self.seed, None)?;
        // Os novos entram no estado do plantel já (sem isto, até à época seguinte, ficavam
        // fora das Reservas e apareciam no plantel principal)
        if created {
            self.refresh_live()?;
        }
        Ok(())
    }

    /// Despromove (`reserve = true`) ou promove um jogador; devolve a frase para a tela. Com
    /// uma equipa B, relega-o para ela ou promove-o de lá (muda de clube, com o mesmo contrato).
    pub fn set_reserve(&mut self, person: i32, reserve: bool) -> Result<String> {
        if self.is_ghost(person) {
            return Err(Error::Invalid(super::deals::GHOST_ONLY.into()));
        }
        let squad = self.own_squad()?;
        if let ReserveMode::BTeam(b) = self.reserve_mode()? {
            return self.move_to_b_team(person, b, reserve, &squad);
        }
        let Some(rank) = squad.iter().position(|&id| id == person) else {
            return Err(Error::Invalid("Só para os jogadores do seu plantel.".into()));
        };
        if self.squads.state(person).reserve == reserve {
            return Err(Error::Invalid(if reserve { "Já está nas reservas." } else { "Já está no plantel principal." }.into()));
        }
        let name = self.person_name(person)?;
        if reserve {
            self.check_first_team(&squad)?;
        }
        self.squads.states.entry(person).or_default().reserve = reserve;
        let text = if reserve {
            self.demoted_morale(person, rank);
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

    fn check_first_team(&self, squad: &[i32]) -> Result<()> {
        let first_team = squad.iter().filter(|id| !self.squads.state(**id).reserve).count();
        if first_team <= MIN_FIRST_TEAM {
            return Err(Error::Invalid(format!("O plantel principal precisa de pelo menos {MIN_FIRST_TEAM} jogadores.")));
        }
        Ok(())
    }

    /// Quem é dos 11 melhores acha-se titular: custa-lhe mais descer.
    fn demoted_morale(&mut self, person: i32, rank: usize) {
        let lost = if rank < 11 { 2 * DEMOTED_MORALE } else { DEMOTED_MORALE };
        self.change_morale(person, -lost);
    }

    /// Relegar para a equipa B (`down`) ou promover de lá à equipa A.
    fn move_to_b_team(&mut self, person: i32, b_team: i32, down: bool, squad: &[i32]) -> Result<String> {
        let club: Option<i32> =
            self.world.conn.query_row("SELECT club_id FROM people WHERE id = ?", [person], |r| r.get(0)).optional()?.flatten();
        let (from, to) = if down { (self.club_id, b_team) } else { (b_team, self.club_id) };
        if club != Some(from) {
            return Err(Error::Invalid(
                if down { "Só para os jogadores do seu plantel." } else { "Só para os jogadores da equipa B." }.into(),
            ));
        }
        if self.loan_of(person)?.is_some() {
            return Err(Error::Invalid("Está emprestado: não pode mudar de equipa.".into()));
        }
        if down {
            self.check_first_team(squad)?;
        }
        let name = self.person_name(person)?;
        self.world.conn.execute("UPDATE people SET club_id = ? WHERE id = ?", [to, person])?;
        self.squads.states.entry(person).or_default().reserve = false;
        let text = if down {
            let rank = squad.iter().position(|&id| id == person).unwrap_or(squad.len());
            self.demoted_morale(person, rank);
            format!("{name} foi relegado para a equipa B.")
        } else {
            self.change_morale(person, PROMOTED_MORALE);
            format!("{name} foi promovido à equipa A.")
        };
        super::numbers::fix_squad_numbers(&self.world, &[from, to], self.seed, None)?;
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
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut g = Game::create(&path, &dir, &new, 31).unwrap();
        let squad = g.own_squad().unwrap();
        assert_eq!(g.reserve_mode().unwrap(), ReserveMode::Flag);
        // O corte do CM: 22 no plantel principal, com 2 guarda-redes; o resto nas reservas
        let first: Vec<i32> = squad.iter().copied().filter(|id| !g.squads.state(*id).reserve).collect();
        assert!(first.len() <= FIRST_TEAM_SIZE && first.len() >= MIN_FIRST_TEAM, "{} no principal", first.len());
        assert!(first.iter().filter(|id| g.is_keeper(**id).unwrap()).count() <= FIRST_TEAM_KEEPERS);
        // As reservas começam com pelo menos 16, completadas com jogadores de enchimento
        let reserves: Vec<i32> = squad.iter().copied().filter(|id| g.squads.state(*id).reserve).collect();
        assert!(reserves.len() >= RESERVE_SQUAD.iter().sum::<usize>(), "{} nas reservas", reserves.len());
        assert!(reserves.iter().any(|id| g.is_ghost(*id)));
        assert!(squad.iter().filter(|id| g.is_ghost(**id)).all(|id| g.squads.state(*id).reserve));
        // Os de enchimento têm estado já (eram esquecidos até à época seguinte e apareciam no
        // plantel principal) e não se podem promover
        let ghost = *reserves.iter().find(|id| g.is_ghost(**id)).unwrap();
        assert!(g.player_status(ghost).is_some_and(|s| s.reserve));
        let mut club = g.world.get_club(PALMEIRAS).unwrap();
        g.fill_squad(&mut club).unwrap();
        assert!(club.players.iter().filter(|p| p.ghost).all(|p| p.reserve), "os de enchimento ficam nas Reservas");
        assert!(g.set_reserve(ghost, false).is_err());
        assert!(g.squad_move(ghost).unwrap().is_none(), "o de enchimento não sobe");
        // Relacionar sem promover: uma reserva (e um de enchimento) no onze, pelo treinador
        let mut sel = g.club_tactics(PALMEIRAS).unwrap().selection();
        let reserve = *reserves.iter().find(|id| !g.is_ghost(**id)).unwrap();
        sel.starters[5] = Some(reserve);
        sel.starters[6] = Some(ghost);
        sel.bench.retain(|id| *id != reserve && *id != ghost);
        sel.auto = false;
        let t = g.set_selection(sel).unwrap();
        let picked: Vec<_> = t.starters.iter().filter_map(|s| s.player.as_ref()).filter(|p| p.reserve).map(|p| p.id).collect();
        assert!(picked.contains(&reserve) && picked.contains(&ghost));
        assert!(g.squads.state(reserve).reserve, "continua nas reservas");
        g.clean_selection().unwrap();
        assert!(g.selection.as_ref().unwrap().starters.contains(&Some(reserve)), "a escolha fica");
        // O onze das reservas não o usa
        let comp = g.division_of(PALMEIRAS).unwrap().unwrap();
        let today = g.date.clone();
        let r = g.reserve_team(PALMEIRAS, comp, &today).unwrap();
        assert!(r.starters.iter().filter_map(|s| s.player.as_ref()).all(|p| p.id != reserve));
        g.selection = None;
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

    fn start(club: i32, name: &str) -> Option<(Game, std::path::PathBuf)> {
        let path = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))?;
        let dir = std::env::temp_dir().join(format!("nextcm-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: name.into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: club,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        Some((Game::create(&path, &dir, &new, 31).unwrap(), dir))
    }

    fn players_of(conn: &rusqlite::Connection, club: i32) -> Vec<i32> {
        conn.prepare("SELECT id FROM people WHERE club_id = ? AND is_player = 1 AND id NOT IN (SELECT person_id FROM loans)")
            .unwrap()
            .query_map([club], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn second_team_is_the_reserves() {
        const BAYERN: i32 = 1350;
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let world = crate::World::open(&path).unwrap();
        let b = world.b_team(BAYERN).unwrap().unwrap();
        assert!(b.second);
        let amateure: Vec<i32> = world
            .conn
            .prepare("SELECT id FROM people WHERE club_id = ? AND is_player = 1")
            .unwrap()
            .query_map([b.club], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        drop(world);
        assert!(!amateure.is_empty());
        let (mut g, dir) = start(BAYERN, "reservas-bayern").unwrap();
        // A Regionalliga não é jogada: a Amateure faz de reservas, com o nome dela
        assert_eq!(g.reserve_mode().unwrap(), ReserveMode::Second(b.club));
        assert_eq!(g.reserves_name().unwrap(), Some(g.club(b.club).name));
        let squad = g.own_squad().unwrap();
        for id in &amateure {
            // Os que ela emprestou ficam emprestados, agora pelo Bayern
            let lent = g.loan_of(*id).unwrap().is_some_and(|l| l.0 == BAYERN);
            assert!(lent || (squad.contains(id) && g.squads.state(*id).reserve), "{id} nas reservas do Bayern");
        }
        // Despromover e promover entre o Bayern e a Amateure
        let id = *amateure.iter().find(|id| squad.contains(id)).unwrap();
        assert_eq!(g.squad_move(id).unwrap().map(|m| m.0), Some("promote"));
        g.set_reserve(id, false).unwrap();
        assert!(!g.squads.state(id).reserve);
        // As reservas jogam
        assert!(!g.reserve_fixtures(BAYERN).unwrap().is_empty());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn b_team_is_another_club() {
        const BENFICA: i32 = 1427;
        let Some((mut g, dir)) = start(BENFICA, "reservas-benfica") else { return };
        let ReserveMode::BTeam(b) = g.reserve_mode().unwrap() else { panic!("o Benfica tem a equipa B") };
        // Sem reservas, sem corte e sem jogos das reservas
        let squad = g.own_squad().unwrap();
        assert!(squad.iter().all(|id| !g.squads.state(*id).reserve && !g.is_ghost(*id)));
        assert!(g.reserves_name().unwrap().is_none());
        assert!(g.reserve_fixtures(BENFICA).unwrap().is_empty());
        // Relegar para a B muda o clube, com o mesmo contrato; e volta
        let id = *squad.iter().rev().find(|id| g.loan_of(**id).unwrap().is_none()).unwrap();
        let contract = |g: &Game| -> Option<String> {
            g.world.conn.query_row("SELECT contract_expires FROM people WHERE id = ?", [id], |r| r.get(0)).unwrap()
        };
        let before = contract(&g);
        assert_eq!(g.squad_move(id).unwrap(), Some(("demote", "Relegar para Equipa 'B'")));
        assert!(g.set_reserve(id, true).unwrap().contains("equipa B"));
        assert!(players_of(&g.world.conn, b).contains(&id) && !g.own_squad().unwrap().contains(&id));
        assert_eq!(contract(&g), before);
        // Os da B não se relacionam
        let mut sel = g.club_tactics(BENFICA).unwrap().selection();
        sel.starters[5] = Some(id);
        sel.auto = false;
        assert!(g.set_selection(sel).is_err());
        assert_eq!(g.squad_move(id).unwrap(), Some(("promote", "Promover à Equipa 'A'")));
        assert!(g.set_reserve(id, false).unwrap().contains("equipa A"));
        assert!(g.own_squad().unwrap().contains(&id));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
