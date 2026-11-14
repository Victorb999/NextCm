//! O Novo Jogo como no FM08 (marco 4.0, itens 1, 2 e 4): a ficha de cada clube antes de o
//! escolher, a experiência passada do treinador e as primeiras mensagens do jogo novo.

use rusqlite::{OptionalExtension, params};
use serde::Serialize;
use ts_rs::TS;

use super::{Game, NewsItem, board, market::money_text, match_analysis::paper_of};
use crate::person::ClubRef;
use crate::world::World;
use crate::{Result, finance};

/// A experiência passada do treinador, como no FM08: o nível e a reputação que dá a mais.
pub const EXPERIENCES: [(&str, i32); 5] = [
    ("Nenhuma", 0),
    ("Jogador amador", 250),
    ("Jogador semi-profissional", 500),
    ("Jogador profissional", 1000),
    ("Jogador internacional", 1500),
];

/// Jogadores que contam para a força de um plantel na previsão dos media.
const PREDICTION_SQUAD: i32 = 16;

/// A ficha de um clube no Novo Jogo (como a do "Select Team" do FM08).
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ClubPreview {
    pub club: ClubRef,
    /// As cores do equipamento alternativo.
    pub away_fg: Option<String>,
    pub away_bg: Option<String>,
    pub division: Option<String>,
    /// "Mundial", "Continental", "Nacional", "Regional" ou "Local".
    pub reputation: String,
    /// "Ricas", "Saudáveis", "Estáveis", "Apertadas" ou "Com dívidas".
    pub finances: String,
    #[ts(type = "number")]
    pub cash: i64,
    /// "Allianz Parque, São Paulo".
    pub stadium: Option<String>,
    /// "43 713 lugares, todos sentados, com aquecimento subterrâneo".
    pub capacity: Option<String>,
    /// As instalações de treino, com as frases do CM.
    pub facilities: Option<String>,
    /// "1º, Campeonato Brasileiro Série A".
    pub last_season: Option<String>,
    /// O lugar que os media prevêem na divisão e quantos clubes ela tem.
    pub prediction: Option<i32>,
    pub clubs_in_division: i32,
    /// O que a direcção espera ("lutar pelo título").
    pub board_goal: String,
    /// Os três melhores jogadores pelo CA.
    pub key_players: Vec<(i32, String)>,
}

fn rep_level(rep: i32) -> &'static str {
    match rep {
        8000.. => "Mundial",
        6500.. => "Continental",
        5000.. => "Nacional",
        3000.. => "Regional",
        _ => "Local",
    }
}

/// As finanças pelo saldo face às receitas de uma época.
fn finances_label(cash: i64, revenue: f64) -> &'static str {
    let r = cash as f64 / revenue.max(1.0);
    match r {
        _ if cash < 0 => "Com dívidas",
        r if r >= 1.0 => "Ricas",
        r if r >= 0.4 => "Saudáveis",
        r if r >= 0.1 => "Estáveis",
        _ => "Apertadas",
    }
}

fn thousands(v: i32) -> String {
    let s = v.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// A força de cada clube de uma divisão (a média do CA dos 16 melhores), do mais forte ao mais
/// fraco: a previsão dos media.
fn division_order(world: &World, division: i32) -> Result<Vec<i32>> {
    Ok(world
        .conn
        .prepare_cached(
            "WITH ranked AS (
                SELECT p.club_id, pl.current_ability AS ca,
                       ROW_NUMBER() OVER (PARTITION BY p.club_id ORDER BY pl.current_ability DESC, p.id) AS rn
                FROM people p JOIN players pl ON pl.person_id = p.id JOIN clubs c ON c.id = p.club_id
                WHERE c.division_id = ?1 AND p.is_player = 1)
             SELECT c.id FROM clubs c LEFT JOIN ranked r ON r.club_id = c.id AND r.rn <= ?2
             WHERE c.division_id = ?1 GROUP BY c.id ORDER BY AVG(COALESCE(r.ca, 0)) DESC, c.reputation DESC, c.id",
        )?
        .query_map(params![division, PREDICTION_SQUAD], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

/// O lugar previsto de um clube na divisão dele e quantos clubes ela tem.
pub(super) fn media_prediction(world: &World, club: i32) -> Result<Option<(i32, i32, Vec<i32>)>> {
    let division: Option<i32> =
        world.conn.query_row("SELECT division_id FROM clubs WHERE id = ?", [club], |r| r.get(0)).optional()?.flatten();
    let Some(division) = division else { return Ok(None) };
    let order = division_order(world, division)?;
    let place = order.iter().position(|&c| c == club).map(|i| i as i32 + 1);
    Ok(place.map(|p| (p, order.len() as i32, order)))
}

/// A ficha de um clube para o Novo Jogo, antes de o jogo existir.
pub fn club_preview(world: &World, id: i32) -> Result<ClubPreview> {
    let profile = world.get_club(id)?;
    let (away_fg, away_bg, division_id): (Option<String>, Option<String>, Option<i32>) =
        world
            .conn
            .query_row("SELECT away_fg, away_bg, division_id FROM clubs WHERE id = ?", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    let rep = profile.reputation.unwrap_or(0);
    // O saldo do 1.º dia, como no Game::create
    let revenue = finance::club_revenue(id, rep, division_id);
    let cash = finance::start_cash(profile.cash.unwrap_or(0).into(), revenue);
    let stadium = profile.stadium.as_ref().map(|s| match &profile.stadium_city {
        Some(city) => format!("{s}, {city}"),
        None => s.clone(),
    });
    let capacity = profile.stadium_capacity.filter(|&c| c > 0).map(|c| {
        let mut text = format!("{} lugares", thousands(c));
        if profile.stadium_seated.is_some_and(|s| s >= c) {
            text.push_str(", todos sentados");
        }
        if profile.undersoil_heating {
            text.push_str(", com aquecimento subterrâneo");
        }
        if profile.retractable_roof {
            text.push_str(", telhado retráctil");
        }
        text
    });
    let last_season = match (profile.last_position, &profile.last_division) {
        (Some(p), Some(d)) if p > 0 => Some(format!("{p}º, {}", d.name)),
        _ => None,
    };
    let (prediction, clubs_in_division) = match media_prediction(world, id)? {
        Some((p, n, _)) => (Some(p), n),
        None => (None, 0),
    };
    let (_, goal, _) = board::expectation(world, id)?;
    let mut best: Vec<&crate::SquadPlayer> = profile.players.iter().collect();
    best.sort_by_key(|p| (std::cmp::Reverse(p.current_ability.unwrap_or(0)), p.id));
    Ok(ClubPreview {
        club: profile.club.clone(),
        away_fg,
        away_bg,
        division: profile.division.as_ref().map(|d| d.name.clone()),
        reputation: rep_level(rep).into(),
        finances: finances_label(cash, revenue).into(),
        cash,
        stadium,
        capacity,
        facilities: profile.facilities.clone(),
        last_season,
        prediction,
        clubs_in_division,
        board_goal: goal.into(),
        key_players: best
            .iter()
            .take(3)
            .map(|p| {
                let name: String = world
                    .conn
                    .query_row("SELECT display_name FROM people WHERE id = ?", [p.id], |r| r.get(0))
                    .unwrap_or_else(|_| p.list_name.clone());
                (p.id, name)
            })
            .collect(),
    })
}

/// Os sectores do relatório do plantel: o nome, quantos contam no onze e a coluna da posição.
const SECTORS: [(&str, &str, usize, &str); 4] = [
    ("a baliza", "um guarda-redes", 1, "pl.pos_goalkeeper"),
    ("a defesa", "um defesa", 4, "MAX(COALESCE(pl.pos_sweeper, 1), COALESCE(pl.pos_defender, 1), COALESCE(pl.pos_wing_back, 1))"),
    (
        "o meio-campo",
        "um médio",
        4,
        "MAX(COALESCE(pl.pos_defensive_midfielder, 1), COALESCE(pl.pos_midfielder, 1), COALESCE(pl.pos_attacking_midfielder, 1))",
    ),
    ("o ataque", "um avançado", 2, "pl.pos_attacker"),
];

impl Game {
    /// A experiência passada (0 Nenhuma a 4 Internacional): fica gravada e soma à reputação.
    pub(super) fn set_experience(&self, level: i32) -> Result<()> {
        let level = level.clamp(0, EXPERIENCES.len() as i32 - 1);
        self.manager_rep()?;
        self.world.conn.execute("UPDATE manager_info SET experience = ? WHERE id = 1", [level])?;
        self.change_manager_rep(EXPERIENCES[level as usize].1)?;
        Ok(())
    }

    /// O clube do coração do treinador (nas preferências, como as dos jogadores).
    pub(super) fn set_favourite_club(&self, club: Option<i32>) -> Result<()> {
        let Some(club) = club else { return Ok(()) };
        self.world.conn.execute(
            "INSERT INTO preferences (person_id, kind, target_id, liked) VALUES (?, 'club', ?, 1)",
            params![self.manager_id, club],
        )?;
        Ok(())
    }

    fn assistant_name(&self) -> Result<Option<String>> {
        Ok(self
            .world
            .conn
            .query_row(
                "SELECT display_name FROM people WHERE club_id = ? AND club_job = 6 AND id <> ? ORDER BY id LIMIT 1",
                params![self.club_id, self.manager_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// As primeiras mensagens do jogo novo, como no FM08: o contrato e os orçamentos da
    /// direcção, a previsão dos media e o relatório do plantel pelo adjunto.
    pub(super) fn first_messages(&self, favourite: Option<i32>) -> Result<Vec<NewsItem>> {
        let mut fresh = Vec::new();
        let mut next_id = self.news.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let club = self.club(self.club_id);
        let us = club.short_name.clone();

        // A direcção: o contrato e os orçamentos
        let (expires, wage): (Option<String>, Option<i64>) =
            self.world
                .conn
                .query_row("SELECT contract_expires, wage FROM people WHERE id = ?", [self.manager_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let (transfer, wages): (i64, i64) =
            self.world.conn.query_row("SELECT transfer_budget, wage_budget FROM board WHERE id = 1", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut body = format!("A Direcção do {} apresenta as condições do seu contrato e os orçamentos para a época.\n\n", club.name);
        if let Some(e) = &expires {
            body += &format!("O contrato vai até {}", super::cm_date(e));
            if let Some(w) = wage.filter(|&w| w > 0) {
                body += &format!(", com um salário de {} por semana", money_text(w));
            }
            body += ".\n";
        }
        body += &format!(
            "Para contratações há {} disponíveis, e a folha salarial pode ir até {} por semana.",
            money_text(transfer),
            money_text(wages)
        );
        if favourite == Some(self.club_id) {
            body += &format!("\n\nOs adeptos receberam bem a notícia: o novo treinador é adepto do {} desde criança.", club.short_name);
        }
        self.push_news(&mut fresh, &mut next_id, "message", "Contrato e orçamentos".into(), body);

        // Os media: o lugar previsto e os favoritos
        if let Some((place, total, order)) = media_prediction(&self.world, self.club_id)? {
            let nation: Option<String> = self
                .world
                .conn
                .query_row("SELECT n.name FROM clubs c JOIN nations n ON n.id = c.nation_id WHERE c.id = ?", [self.club_id], |r| r.get(0))
                .optional()?;
            let paper = paper_of(nation.as_deref().unwrap_or(""));
            let division: Option<String> = self
                .world
                .conn
                .query_row("SELECT d.name FROM clubs c JOIN competitions d ON d.id = c.division_id WHERE c.id = ?", [self.club_id], |r| {
                    r.get(0)
                })
                .optional()?;
            let league = division.map(|d| format!(" no {d}")).unwrap_or_default();
            let favourites: Vec<String> = order.iter().take(3).map(|&c| self.club(c).short_name).collect();
            let verdict = match place {
                1 => "o favorito ao título".to_string(),
                p if p <= 3 => "um dos candidatos ao título".to_string(),
                p if p > total - 4 => "um dos candidatos à descida".to_string(),
                _ => "uma equipa para o meio da tabela".to_string(),
            };
            let body = format!(
                "O {paper} aponta o {us} para o {place}º lugar{league}: é {verdict}.\n\nOs favoritos do jornal: {}.",
                super::season::join_names(&favourites)
            );
            self.push_news(&mut fresh, &mut next_id, "message", format!("Previsão dos media: {us} em {place}º"), body);
        }

        // O adjunto: o relatório do plantel
        if let Some(text) = self.squad_report()? {
            let who = self.assistant_name()?.unwrap_or_else(|| "O adjunto".into());
            self.push_news(&mut fresh, &mut next_id, "message", "Relatório do plantel".into(), format!("{text}\n\n{who}"));
        }
        Ok(fresh)
    }

    /// Os titulares de cada sector de um clube (os de 15+ na posição, sem repetir) e a média do
    /// CA deles; um sector sem gente que chegue conta os que faltam com CA 60.
    fn sectors_of(&self, club: i32) -> Result<Vec<(f64, Vec<String>)>> {
        let mut out = Vec::new();
        let mut used: Vec<i32> = Vec::new();
        for (_, _, n, col) in SECTORS {
            let rows: Vec<(i32, String, i32)> = self
                .world
                .conn
                .prepare_cached(&format!(
                    "SELECT p.id, p.display_name, COALESCE(pl.current_ability, 0) FROM people p JOIN players pl ON pl.person_id = p.id
                     WHERE p.club_id = ? AND p.is_player = 1 AND COALESCE({col}, 1) >= 15
                       AND p.id NOT IN (SELECT person_id FROM ghosts)
                     ORDER BY pl.current_ability DESC, p.id LIMIT 8"
                ))?
                .query_map([club], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<rusqlite::Result<_>>()?;
            let picked: Vec<&(i32, String, i32)> = rows.iter().filter(|r| !used.contains(&r.0)).take(n).collect();
            used.extend(picked.iter().map(|r| r.0));
            let sum: f64 = picked.iter().map(|r| r.2 as f64).sum::<f64>() + (n - picked.len()) as f64 * 60.0;
            out.push((sum / n as f64, picked.iter().map(|r| r.1.clone()).collect()));
        }
        Ok(out)
    }

    /// O relatório do plantel pelo adjunto: os melhores, o sector mais forte e o mais fraco
    /// (cada sector face à média do mesmo sector nos clubes da divisão) e os jovens a seguir.
    fn squad_report(&self) -> Result<Option<String>> {
        let ours = self.sectors_of(self.club_id)?;
        let rivals: Vec<i32> = self
            .world
            .conn
            .prepare("SELECT id FROM clubs WHERE division_id = (SELECT division_id FROM clubs WHERE id = ?1) AND id <> ?1")?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut average = [0.0; SECTORS.len()];
        for &c in &rivals {
            for (i, (v, _)) in self.sectors_of(c)?.iter().enumerate() {
                average[i] += v / rivals.len() as f64;
            }
        }
        // A diferença para a média da divisão (sem rivais, para a média do próprio onze)
        let base = ours.iter().map(|s| s.0).sum::<f64>() / ours.len() as f64;
        let diff: Vec<f64> = ours.iter().enumerate().map(|(i, s)| s.0 - if rivals.is_empty() { base } else { average[i] }).collect();
        let order = |a: &usize, b: &usize| diff[*a].total_cmp(&diff[*b]);
        let filled: Vec<usize> = (0..ours.len()).filter(|&i| !ours[i].1.is_empty()).collect();
        if filled.len() < 2 {
            return Ok(None);
        }
        let strong = filled.iter().copied().max_by(order).unwrap();
        let weak = (0..ours.len()).min_by(order).unwrap();
        let best: Vec<String> = self
            .world
            .conn
            .prepare(
                "SELECT p.display_name FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1 AND p.id NOT IN (SELECT person_id FROM ghosts)
                 ORDER BY pl.current_ability DESC, p.id LIMIT 3",
            )?
            .query_map([self.club_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let young: Vec<String> = self
            .world
            .conn
            .prepare(
                "SELECT p.display_name FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ?1 AND p.is_player = 1 AND p.birth_date > ?2 AND p.id NOT IN (SELECT person_id FROM ghosts)
                   AND COALESCE(pl.potential_ability, 0) >= COALESCE(pl.current_ability, 0) + 20
                 ORDER BY pl.potential_ability DESC, p.id LIMIT 2",
            )?
            .query_map(params![self.club_id, crate::date::add_days(&self.date, -21 * 365).unwrap_or_default()], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let us = self.club(self.club_id).short_name;
        let mut text =
            format!("Estive a ver o plantel do {us}. Os jogadores mais importantes são {}.\n\n", super::season::join_names(&best));
        let (strong_name, _, _, _) = SECTORS[strong];
        text += &format!("O nosso ponto forte é {strong_name} ({}).", super::season::join_names(&ours[strong].1));
        if weak != strong {
            let (weak_name, one, _, _) = SECTORS[weak];
            text += &format!(" O sector mais fraco é {weak_name}: recomendo contratar {one} se a direcção o permitir.");
        }
        if !young.is_empty() {
            text += &format!("\n\nVale a pena acompanhar {}: são jovens com muito potencial.", super::season::join_names(&young));
        }
        Ok(Some(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(finances_label(-5, 100.0), "Com dívidas");
        assert_eq!(finances_label(150, 100.0), "Ricas");
        assert_eq!(finances_label(5, 100.0), "Apertadas");
        assert_eq!(thousands(43713), "43 713");
        assert_eq!(thousands(950), "950");
    }

    #[test]
    fn preview_of_palmeiras() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let w = World::open(&path).unwrap();
        let p = club_preview(&w, 7568).unwrap();
        assert_eq!(p.club.short_name, "Palmeiras");
        assert!(p.stadium.is_some() && p.capacity.is_some());
        assert!(p.prediction.is_some_and(|x| x >= 1 && x <= p.clubs_in_division), "{p:?}");
        assert_eq!(p.key_players.len(), 3);
        assert!(!p.board_goal.is_empty());
    }

    #[test]
    fn first_messages_experience_and_favourite_club() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-newgame-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Novo".into(),
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
            experience: 3,
            favourite_club: Some(7568),
        };
        let g = Game::create(&path, &dir, &new, 7).unwrap();
        let titles: Vec<&str> = g.news.iter().map(|n| n.title.as_str()).collect();
        assert!(titles.contains(&"Contrato e orçamentos"), "{titles:?}");
        assert!(titles.iter().any(|t| t.starts_with("Previsão dos media")), "{titles:?}");
        assert!(titles.contains(&"Relatório do plantel"), "{titles:?}");
        let contract = g.news.iter().find(|n| n.title == "Contrato e orçamentos").unwrap();
        assert!(contract.body.contains("desde criança"), "{}", contract.body);
        let club_rep: i32 = g.world.conn.query_row("SELECT reputation FROM clubs WHERE id = 7568", [], |r| r.get(0)).unwrap();
        assert_eq!(g.manager_rep().unwrap(), club_rep + EXPERIENCES[3].1);
        let liked: i32 = g
            .world
            .conn
            .query_row("SELECT liked FROM preferences WHERE person_id = ? AND kind = 'club' AND target_id = 7568", [g.manager_id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(liked, 1);
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
