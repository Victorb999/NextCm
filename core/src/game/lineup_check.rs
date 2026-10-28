//! Antes do dia de jogo, como no CM: sem onze escolhido o jogo não avança ("Escolha Equipa"),
//! e com batedores fora do onze ou suplentes a menos pede confirmação ("Confirmar Por Favor").
//! Só quando o treinador escolhe a equipa (com o adjunto a escolher não há nada a confirmar).

use serde::Serialize;
use ts_rs::TS;

use super::Game;
use crate::Result;
use crate::date;
use crate::tactics::BENCH;

/// O que o Continuar Jogo tem de mostrar antes de entrar no dia de jogo.
#[derive(Debug, Clone, Default, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct LineupCheck {
    /// A equipa não pode jogar assim (o jogo não avança).
    pub problem: Option<String>,
    /// Avisos para confirmar ("Avançar para o jogo?").
    pub warnings: Vec<String>,
}

const NUMBERS: [&str; 12] = ["nenhum", "um", "dois", "três", "quatro", "cinco", "seis", "sete", "oito", "nove", "dez", "onze"];

fn number(n: usize) -> String {
    NUMBERS.get(n).map_or_else(|| n.to_string(), |s| s.to_string())
}

/// "a, b, e c", como na frase do CM.
fn names(list: &[String]) -> String {
    match list {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{}, e {last}", rest.join(", ")),
    }
}

impl Game {
    /// O treinador joga amanhã com a equipa escolhida por ele: o que falta ou o que confirmar.
    pub fn lineup_check(&self) -> Result<LineupCheck> {
        let tomorrow = date::add_days(&self.date, 1).unwrap_or_default();
        let plays = self.matches.iter().any(|m| m.date == tomorrow && (m.home_id == self.club_id || m.away_id == self.club_id));
        let Some(sel) = self.selection.as_ref().filter(|s| !s.auto) else { return Ok(LineupCheck::default()) };
        if !plays || self.sacked() || self.live.is_some() {
            return Ok(LineupCheck::default());
        }
        let chosen = sel.starters.iter().flatten().count();
        let needed = sel.starters.len().max(11);
        if chosen < needed {
            let start = if chosen == 0 {
                "Não escolheu nenhum jogador de campo.".to_string()
            } else {
                format!("Só escolheu {} jogadores de campo.", number(chosen))
            };
            let problem = format!("{start} Tem de escolher onze jogadores de campo, e pode convocar até vinte jogadores.");
            return Ok(LineupCheck { problem: Some(problem), warnings: Vec::new() });
        }
        let mut warnings = Vec::new();
        let k = &sel.takers;
        let mut outside: Vec<i32> =
            [k.penalty, k.free_kick, k.corner, k.playmaker].into_iter().flatten().filter(|id| !sel.starters.contains(&Some(*id))).collect();
        outside.dedup();
        if !outside.is_empty() {
            let list: Vec<String> = outside.iter().map(|&id| self.person_name(id)).collect::<Result<_>>()?;
            let verb = if list.len() == 1 { "foi escolhido" } else { "foram escolhidos" };
            let parts = if list.len() == 1 { "não faz parte" } else { "não fazem parte" };
            warnings.push(format!("{} {verb} como marcadores de bolas paradas, mas {parts} do onze inicial.", names(&list)));
        }
        if let Some(c) = k.captain.filter(|c| !sel.starters.contains(&Some(*c))) {
            warnings.push(format!("O capitão, {}, não faz parte do onze inicial: o adjunto escolhe outro.", self.person_name(c)?));
        }
        if sel.bench.len() < BENCH {
            warnings.push(match sel.bench.len() {
                0 => "Não escolheu nenhum substituto.".to_string(),
                1 => format!("Apenas escolheu um substituto entre {} possíveis.", number(BENCH)),
                n => format!("Apenas escolheu {} substitutos entre {} possíveis.", number(n), number(BENCH)),
            });
        }
        Ok(LineupCheck { problem: None, warnings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_team_is_refused_and_short_bench_asks() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-escolha-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Escolha".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: 7568,
            leagues: Vec::new(),
        };
        let mut g = Game::create(&path, &dir, &new, 13).unwrap();
        // Véspera do primeiro jogo do Palmeiras
        let first = g.matches.iter().filter(|m| m.home_id == 7568 || m.away_id == 7568).map(|m| m.date.clone()).min().unwrap();
        g.date = date::add_days(&first, -1).unwrap();
        assert_eq!(g.lineup_check().unwrap().problem, None, "o adjunto escolhe: nada a dizer");
        let mut sel = g.club_tactics(7568).unwrap().selection();
        sel.auto = false;
        let full = sel.clone();
        sel.starters.iter_mut().for_each(|s| *s = None);
        sel.bench.clear();
        g.selection = Some(sel);
        assert!(g.lineup_check().unwrap().problem.unwrap().starts_with("Não escolheu nenhum jogador de campo"));
        let mut short = full.clone();
        short.bench.truncate(7);
        g.selection = Some(short);
        let c = g.lineup_check().unwrap();
        assert!(c.problem.is_none() && c.warnings == vec!["Apenas escolheu sete substitutos entre nove possíveis.".to_string()]);
        g.selection = Some(full);
        assert!(g.lineup_check().unwrap().warnings.is_empty());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
