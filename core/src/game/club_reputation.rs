//! A reputação dos clubes muda com os resultados (pedido de 18/11; até aqui ficava fixa no save,
//! decisão do 3.05). No fim de cada temporada, nas ligas com tabela: o lugar contra o esperado pela
//! reputação, os títulos (liga, copa, continental, pela reputação da competição; as supercopas
//! valem pouco), o acesso e a queda. Quem está perto do máximo sobe devagar, e todos voltam um
//! pouco para a reputação de partida (`base_reputation`), para o mundo não inflacionar.

use std::collections::HashMap;

use rusqlite::params;

use super::{Game, NewsItem};
use crate::Result;
use crate::regulation::{self, Regulation, StageKind};

/// Save 78: a reputação de partida de cada clube (para onde a de hoje volta aos poucos).
pub(super) const BASE_REP_SCHEMA: &str =
    "ALTER TABLE clubs ADD COLUMN base_reputation INTEGER; UPDATE clubs SET base_reputation = reputation;";

/// Pontos por lugar acima (ou abaixo) do esperado, numa liga de 20: 10 lugares acima = +200.
const PLACE_POINTS: f64 = 400.0;
const PROMOTED: f64 = 200.0;
const RELEGATED: f64 = -250.0;
/// O vice de uma copa ou continental leva esta parte do prémio do campeão.
const RUNNER_UP: f64 = 0.35;
/// Uma supercopa (até 2 jogos na temporada) vale esta parte de uma copa.
const SUPER_CUP: f64 = 0.2;
/// O que volta para a reputação de partida em cada temporada.
const DRIFT: f64 = 0.05;
/// A maior mudança numa temporada.
const MAX_STEP: f64 = 700.0;
const MAX_REP: f64 = 10_000.0;

/// O prémio de um título: a liga pela reputação dela, a copa menos, a continental pelo quadrado.
fn title_points(rep: f64, national: bool, league: bool) -> f64 {
    match (national, league) {
        (true, true) => 100.0 + rep * 15.0,
        (true, false) => 30.0 + rep * 6.0,
        _ => rep * rep * 1.2,
    }
}

/// A mudança amortecida: perto do máximo sobe-se devagar, perto do zero desce-se devagar.
fn damped(rep: f64, delta: f64) -> f64 {
    let d = if delta > 0.0 { delta * ((MAX_REP - rep) / 3000.0).clamp(0.1, 1.0) } else { delta * (rep / 3000.0).clamp(0.2, 1.0) };
    d.clamp(-MAX_STEP, MAX_STEP)
}

impl Game {
    /// A reputação dos clubes no fim de uma temporada: `podiums` os campeões e vices desta
    /// temporada, `up` e `down` quem sobe e quem desce.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn club_reputations(
        &mut self,
        regs: &[&'static Regulation],
        year: i32,
        podiums: &[(i32, (i32, Option<i32>, Option<i32>))],
        up: &[i32],
        down: &[i32],
        fresh: &mut Vec<NewsItem>,
        next_id: &mut i32,
    ) -> Result<()> {
        let comp_rep = |id: i32| -> f64 {
            self.world
                .conn
                .query_row("SELECT COALESCE(reputation, 0) FROM competitions WHERE id = ?", [id], |r| r.get::<_, i32>(0))
                .unwrap_or(0) as f64
        };
        let mut delta: HashMap<i32, f64> = HashMap::new();

        // O lugar na liga contra o esperado pela reputação
        for reg in regs.iter().filter(|r| r.managed && r.stages[0].kind == StageKind::League) {
            let table = self.stage_table(reg.competition, year, 0, None);
            if table.is_empty() || table.iter().all(|r| r.played == 0) {
                continue;
            }
            let mut reps: Vec<(i32, i32)> = Vec::new();
            for r in &table {
                let rep: i32 =
                    self.world.conn.query_row("SELECT COALESCE(reputation, 0) FROM clubs WHERE id = ?", [r.club.id], |x| x.get(0))?;
                reps.push((r.club.id, rep));
            }
            reps.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let n = table.len() as f64;
            for r in &table {
                let expected = reps.iter().position(|x| x.0 == r.club.id).unwrap_or(0) as f64 + 1.0;
                *delta.entry(r.club.id).or_default() += (expected - r.position as f64) / n * PLACE_POINTS;
            }
        }
        // Os títulos
        for &(comp, (winner, second, _)) in podiums {
            let Some(reg) = regulation::get(comp) else { continue };
            if super::internationals::is_national_comp(comp) {
                continue;
            }
            let national = self.comp_nations.contains_key(&comp);
            let league = !reg.is_cup();
            let mut pts = title_points(comp_rep(comp), national, league);
            if reg.is_cup() && self.year_matches(comp, year).count() <= 2 {
                pts *= SUPER_CUP;
            }
            *delta.entry(winner).or_default() += pts;
            if reg.is_cup()
                && let Some(r) = second
            {
                *delta.entry(r).or_default() += pts * RUNNER_UP;
            }
        }
        for &id in up {
            *delta.entry(id).or_default() += PROMOTED;
        }
        for &id in down {
            *delta.entry(id).or_default() += RELEGATED;
        }

        let mut ours: Option<(i32, i32)> = None;
        for (id, d) in delta {
            let (rep, base): (i32, Option<i32>) = self.world.conn.query_row(
                "SELECT COALESCE(reputation, 0), base_reputation FROM clubs WHERE id = ? AND is_national_team = 0",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let now = rep as f64;
            let drift = base.map_or(0.0, |b| (b as f64 - now) * DRIFT);
            let next = ((now + damped(now, d) + drift) / 10.0).round() * 10.0;
            let next = next.clamp(0.0, MAX_REP) as i32;
            if next != rep {
                self.world.conn.execute("UPDATE clubs SET reputation = ? WHERE id = ?", params![next, id])?;
            }
            if id == self.club_id && !self.sacked() {
                ours = Some((rep, next));
            }
        }
        if let Some((before, after)) = ours.filter(|(b, a)| (a - b).abs() >= 50) {
            let short = self.club(self.club_id).short_name;
            let (title, how) = if after > before {
                (format!("Reputação do {short} em alta"), "subiu")
            } else {
                (format!("Reputação do {short} em queda"), "caiu")
            };
            self.push_news(
                fresh,
                next_id,
                "board",
                title,
                format!(
                    "Com a temporada que passou, a reputação do {short} {how} de {before} para {after}.\n\n\
                     Isso pesa em quem aceita vir para o clube, nas receitas e no que a Diretoria espera."
                ),
            );
        }
        Ok(())
    }
}

impl Game {
    /// Para a CLI: a média de hoje contra a de partida, quantos mudaram muito, os que mais subiram
    /// e desceram e os 8 maiores.
    pub fn reputation_report(&self) -> Result<Vec<String>> {
        let rows: Vec<(String, i32, i32)> = self
            .world
            .conn
            .prepare(
                "SELECT COALESCE(NULLIF(short_name, ''), name), COALESCE(reputation, 0), base_reputation FROM clubs
                 WHERE is_national_team = 0 AND base_reputation IS NOT NULL AND base_reputation > 0",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let n = rows.len().max(1) as f64;
        let now = rows.iter().map(|r| r.1 as f64).sum::<f64>() / n;
        let base = rows.iter().map(|r| r.2 as f64).sum::<f64>() / n;
        let moved = rows.iter().filter(|r| (r.1 - r.2).abs() >= 500).count();
        let mut by_change = rows.clone();
        by_change.sort_by_key(|r| r.1 - r.2);
        let show = |v: &[(String, i32, i32)]| v.iter().map(|r| format!("{} {}→{}", r.0, r.2, r.1)).collect::<Vec<_>>().join(", ");
        let falls: Vec<_> = by_change.iter().take(5).cloned().collect();
        let rises: Vec<_> = by_change.iter().rev().take(5).cloned().collect();
        let mut top = rows.clone();
        top.sort_by_key(|r| std::cmp::Reverse(r.1));
        let top: Vec<_> = top.into_iter().take(8).collect();
        Ok(vec![
            format!("reputação dos clubes: média {now:.0} (partida {base:.0}), {moved} mudaram 500+"),
            format!("  mais subiram: {}", show(&rises)),
            format!("  mais caíram: {}", show(&falls)),
            format!("  maiores: {}", show(&top)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_and_damping() {
        // A Champions vale mais que a Liga Europa, que vale mais que uma liga grande ganha
        assert!(title_points(20.0, false, false) > title_points(16.0, false, false));
        assert!(title_points(17.0, true, true) > title_points(11.0, true, false));
        // Perto do máximo sobe-se pouco; a mudança tem teto
        assert!(damped(9700.0, 500.0) < damped(5000.0, 500.0));
        assert_eq!(damped(2000.0, 5000.0), MAX_STEP);
        assert!(damped(300.0, -500.0) > -500.0);
    }
}
