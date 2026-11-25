//! O que já está decidido numa liga antes do fim: campeão, vagas (continentais, acesso) e
//! rebaixamento garantidos pela matemática (os pontos que cada um ainda pode fazer). A tabela
//! marca-os (`StandingRow::clinched`) e o treinador recebe a mensagem uma vez: a do seu clube e a
//! do campeão da sua liga.

use super::{Game, NewsItem};
use crate::league::StandingRow;
use crate::regulation;

/// Pontos por vitória.
const WIN: i32 = 3;

impl Game {
    /// Marca em `rows` (a classificação de pontos corridos de `comp`) o que cada clube já
    /// garantiu: "champion", o tipo da faixa ("continental", "promotion"...) ou "relegation".
    /// Empates nos pontos contam como possíveis (sem garantir nada pelos critérios de desempate).
    pub(super) fn mark_clinched(&self, comp: i32, rows: &mut [StandingRow]) {
        let year = self.current_season(comp);
        let left = |club: i32| -> i32 {
            self.year_matches(comp, year).filter(|m| m.stage == 0 && m.result.is_none() && (m.home_id == club || m.away_id == club)).count()
                as i32
        };
        let max: Vec<i32> = rows.iter().map(|r| r.points + WIN * left(r.club.id)).collect();
        let total = rows.len();
        // Ainda há jogos? Com a liga acabada a tabela já diz tudo (as faixas pintadas)
        if max.iter().zip(rows.iter()).all(|(m, r)| *m == r.points) {
            return;
        }
        let zones = regulation::zones(comp);
        for i in 0..total {
            // Quantos ainda podem chegar aos pontos dele, e quantos já estão garantidamente à frente
            let can_reach = (0..total).filter(|&j| j != i && max[j] >= rows[i].points).count();
            let surely_ahead = (0..total).filter(|&j| j != i && rows[j].points > max[i]).count();
            let top = |n: usize| can_reach < n;
            let mut mark = None;
            if top(1) {
                mark = Some("champion".to_string());
            } else {
                // A melhor faixa de cima garantida (as faixas vêm pela ordem das vagas)
                let mut best: Option<&regulation::Zone> = None;
                for z in zones.iter().filter(|z| !z.from_bottom && z.from == 1) {
                    if top(z.to) && best.is_none_or(|b| z.to < b.to) {
                        best = Some(z);
                    }
                }
                if let Some(z) = best {
                    mark = Some(z.kind.to_string());
                }
                for z in zones.iter().filter(|z| z.from_bottom && z.kind == "relegation") {
                    if surely_ahead >= total.saturating_sub(z.to) {
                        mark = Some("relegation".into());
                    }
                }
            }
            rows[i].clinched = mark;
        }
    }

    /// As mensagens do que ficou decidido hoje na liga do treinador: o que o clube dele garantiu
    /// e o campeão. Cada uma só uma vez por temporada.
    pub(super) fn clinch_news(&self, fresh: &mut Vec<NewsItem>, next_id: &mut i32) {
        if self.sacked() {
            return;
        }
        let Ok(Some(comp)) = self.division_of(self.club_id) else { return };
        let mut rows = self.table(comp);
        if rows.is_empty() {
            return;
        }
        self.mark_clinched(comp, &mut rows);
        let name = self.competition_name(comp).to_string();
        let year = self.current_season(comp);
        let rounds_left = |club: i32| -> i32 {
            self.year_matches(comp, year).filter(|m| m.stage == 0 && m.result.is_none() && (m.home_id == club || m.away_id == club)).count()
                as i32
        };
        let season_start = self.season_start(Some(self.club_id)).unwrap_or_default();
        let zones = regulation::zones(comp);
        let mut out: Vec<(String, String)> = Vec::new();
        for r in &rows {
            let Some(mark) = r.clinched.as_deref() else { continue };
            let mine = r.club.id == self.club_id;
            let club = &r.club.short_name;
            let left = rounds_left(r.club.id);
            let early =
                if left == 1 { "com uma rodada de antecedência".to_string() } else { format!("com {left} rodadas de antecedência") };
            match (mark, mine) {
                ("champion", true) => out.push((
                    format!("{club} é campeão da {name}!"),
                    format!("O {club} conquistou a {name} {early}, com {} pontos. A festa toma conta da cidade.", r.points),
                )),
                ("champion", false) => out.push((
                    format!("{club} é campeão da {name}"),
                    format!("O {club} garantiu matematicamente o título da {name} {early}."),
                )),
                ("relegation", true) => out.push((
                    format!("{club} está rebaixado"),
                    format!("Não há mais como escapar: o {club} vai disputar a próxima temporada numa divisão abaixo."),
                )),
                (kind, true) => {
                    let label =
                        zones.iter().find(|z| z.kind == kind).and_then(|z| z.competition).map(|c| self.competition_name(c).to_string());
                    let (title, body) = match (kind, label) {
                        ("promotion", _) => (format!("{club} garante o acesso"), format!("O {club} garantiu o acesso {early}.")),
                        (_, Some(l)) => (
                            format!("{club} garante vaga na {l}"),
                            format!("O {club} não sai mais da zona de classificação para a {l} ({early})."),
                        ),
                        _ => continue,
                    };
                    out.push((title, body));
                }
                _ => {}
            }
        }
        for (title, body) in out {
            let said = self.news.iter().chain(fresh.iter()).any(|n| n.title == title && n.date >= season_start);
            if !said {
                fresh.push(self.news_item(*next_id, "message", title, body));
                *next_id += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::league::SERIE_A;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn clinched_title_is_marked_and_announced_once() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = crate::TempDir::new(format!("nextcm-garantido-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Garantido".into(),
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
        let mut g = Game::create(&path, &dir, &new, 3).unwrap();
        // O Palmeiras ganha tudo e os outros empatam, com as 4 últimas rodadas por jogar
        let year = g.current_season(SERIE_A);
        let last = g.year_matches(SERIE_A, year).map(|m| m.round).max().unwrap();
        let ids: Vec<i32> = g.year_matches(SERIE_A, year).map(|m| m.id).collect();
        for m in g.matches.iter_mut().filter(|m| ids.contains(&m.id)) {
            m.result = if m.round > last - 4 {
                None
            } else if m.home_id == PALMEIRAS {
                Some((2, 0))
            } else if m.away_id == PALMEIRAS {
                Some((0, 2))
            } else {
                Some((1, 1))
            };
        }
        let rows = g.table(SERIE_A);
        let ours = rows.iter().find(|r| r.club.id == PALMEIRAS).unwrap();
        assert_eq!(ours.clinched.as_deref(), Some("champion"));
        assert!(rows.iter().filter(|r| r.club.id != PALMEIRAS).all(|r| r.clinched.as_deref() != Some("champion")));
        let (mut fresh, mut id) = (Vec::new(), 1);
        g.clinch_news(&mut fresh, &mut id);
        assert!(
            fresh.iter().any(|n| n.title.starts_with("Palmeiras é campeão")),
            "{:?}",
            fresh.iter().map(|n| &n.title).collect::<Vec<_>>()
        );
        // Só uma vez por temporada
        g.news.extend(fresh);
        let (mut again, mut id) = (Vec::new(), 100);
        g.clinch_news(&mut again, &mut id);
        assert!(again.iter().all(|n| !n.title.starts_with("Palmeiras é campeão")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod bench_tests {
    use crate::game::Game;

    const PALMEIRAS: i32 = 7568;

    /// Com o GR suplente lesionado e outro só nas reservas, o adjunto não passa de 9 suplentes
    /// (antes punha 9 e mais o GR das reservas, e nenhuma troca era aceite).
    #[test]
    fn assistant_bench_never_passes_nine_with_the_backup_keeper_out() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = crate::TempDir::new(format!("nextcm-banco-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Banco".into(),
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
        let mut g = Game::create(&path, &dir, &new, 3).unwrap();
        let keepers: Vec<i32> = g
            .world
            .conn
            .prepare(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id WHERE p.club_id = ? AND pl.pos_goalkeeper >= 15
                 ORDER BY pl.current_ability DESC",
            )
            .unwrap()
            .query_map([PALMEIRAS], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(keepers.len() >= 3, "{keepers:?}");
        // O titular joga; o último vai para as reservas; os do meio estão lesionados
        let last = *keepers.last().unwrap();
        for &k in &keepers[1..keepers.len() - 1] {
            g.squads.states.entry(k).or_default().injury = Some("Lesão no joelho".into());
        }
        g.squads.states.entry(last).or_default().reserve = true;
        let t = g.club_tactics(PALMEIRAS).unwrap();
        assert!(t.bench.len() <= crate::tactics::BENCH, "{} suplentes", t.bench.len());
        assert!(t.bench.iter().any(|p| p.id == last), "o GR das reservas vai para o banco");
        // E a escolha passa na validação (as trocas voltam a ser aceites)
        assert!(g.set_selection(t.selection()).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
