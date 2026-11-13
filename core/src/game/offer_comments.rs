//! Os Comentários da proposta de transferência (marco 4.0, item 5, como no FM08): o que o agente
//! diz antes de o treinador propor. As posições que o clube vendedor procura (a mesma conta do
//! mercado da IA: menos jogadores do que o mínimo ou um titular fraco), a cláusula de rescisão, o
//! prazo da decisão e os extras que o clube valoriza. Só diz o que o core faz de facto.

use super::deals::Facts;
use super::market::{MIN_ROLE, ROLES, STARTERS, WEAK_STARTER, money_text, role_of};
use super::negotiation::REPLY_DAYS;
use super::{Game, cm_date};
use crate::{Result, date};

/// Os papéis do mercado no plural, como o agente os diz.
const ROLE_NAMES: [&str; ROLES] = ["guarda-redes", "defesas", "médios", "avançados"];
/// Idade até à qual o clube que vende costuma pedir uma percentagem da próxima venda (como em
/// `negotiation::seller_reply`).
const YOUNG: i32 = 23;

impl Game {
    /// As posições que um clube procura: as que têm menos do que o mínimo ou, sem nenhuma, a do
    /// titular mais fraco abaixo do nível (a média dos 11 melhores), sem contar `leaving`.
    pub(super) fn club_needs(&self, club: i32, leaving: Option<i32>) -> Result<Vec<usize>> {
        let rows: Vec<(i32, i32, [i32; 4])> = self
            .world
            .conn
            .prepare_cached(
                "SELECT p.id, COALESCE(pl.current_ability, 1), COALESCE(pl.pos_goalkeeper, 1),
                        MAX(COALESCE(pl.pos_sweeper, 1), COALESCE(pl.pos_defender, 1), COALESCE(pl.pos_wing_back, 1)),
                        MAX(COALESCE(pl.pos_defensive_midfielder, 1), COALESCE(pl.pos_midfielder, 1), COALESCE(pl.pos_attacking_midfielder, 1)),
                        COALESCE(pl.pos_attacker, 1)
                 FROM people p JOIN players pl ON pl.person_id = p.id
                 WHERE p.club_id = ? AND p.is_player = 1 AND p.id NOT IN (SELECT person_id FROM retired)
                 ORDER BY pl.current_ability DESC, p.id",
            )?
            .query_map([club], |r| Ok((r.get(0)?, r.get(1)?, [r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?])))?
            .collect::<rusqlite::Result<_>>()?;
        let mut count = [0usize; ROLES];
        let mut weakest = [0i32; ROLES];
        let (mut top, mut sum) = (0, 0);
        for (_, ca, pos) in rows.iter().filter(|r| Some(r.0) != leaving) {
            let role = role_of(pos[0], pos[1], pos[2], pos[3]);
            count[role] += 1;
            if count[role] == STARTERS[role] {
                weakest[role] = *ca;
            }
            if top < 11 {
                top += 1;
                sum += ca;
            }
        }
        let level = sum / top.max(1);
        // Como o mercado da IA: as posições com menos do que o mínimo ou, sem nenhuma, a do
        // titular mais fraco abaixo do nível
        let short: Vec<usize> = (0..ROLES).filter(|&r| count[r] < MIN_ROLE[r]).collect();
        if !short.is_empty() {
            return Ok(short);
        }
        Ok((0..ROLES).filter(|&r| weakest[r] < level - WEAK_STARTER).min_by_key(|&r| weakest[r]).into_iter().collect())
    }

    /// Os comentários do agente sobre uma proposta por `person` (de outro clube).
    pub(super) fn offer_comments(&self, person: i32, f: &Facts) -> Result<Vec<String>> {
        let Some(club) = f.club else { return Ok(Vec::new()) };
        let seller = self.club(club).short_name;
        let mut out = Vec::new();
        let needs = self.club_needs(club, Some(person))?;
        let names: Vec<&str> = needs.iter().map(|&r| ROLE_NAMES[r]).collect();
        out.push(match names.as_slice() {
            [] => format!("O {seller} tem o plantel composto: não procura reforços."),
            [one] => format!("O {seller} procura {one}: um na troca pode ajudar."),
            [rest @ .., last] => {
                format!("O {seller} procura {} e {last}: um na troca pode ajudar.", rest.join(", "))
            }
        });
        out.push(match self.contract_extras(person)?.and_then(|e| e.min_fee) {
            Some(fee) => format!("Cláusula de rescisão de {}: a esse valor o {seller} não pode recusar.", money_text(fee)),
            None => "Sem cláusula de rescisão: o preço é o que o clube aceitar.".into(),
        });
        let reply = self.reply_on(person, REPLY_DAYS);
        out.push(format!("Proposta hoje: o {seller} responde até {}.", cm_date(&reply)));
        if f.age <= YOUNG {
            out.push(format!("Com {} anos, o {seller} deve pedir uma percentagem da próxima venda.", f.age));
        }
        out.push("Valores por jogos ou internacionalizações contam menos do que à cabeça.".into());
        if let Some(ends) = &f.expires {
            let left = date::parse(ends).unwrap_or(0) - date::parse(&self.date).unwrap_or(0);
            if (0..=super::contracts::PRECONTRACT_DAYS).contains(&(left as i64)) {
                out.push(format!("O contrato acaba a {}: pode propor-lhe um pré-contrato.", cm_date(ends)));
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::{Game, NewGame, World};

    #[test]
    fn comments_on_an_offer() {
        let Some(path) = World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-comentarios-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = NewGame {
            save_name: "Comentários".into(),
            first_name: "A".into(),
            last_name: "B".into(),
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
        let g = Game::create(&path, &dir, &new, 5).unwrap();
        let person: i32 = g
            .world
            .conn
            .query_row(
                "SELECT p.id FROM people p JOIN players pl ON pl.person_id = p.id JOIN clubs c ON c.id = p.club_id
                 WHERE c.division_id = (SELECT division_id FROM clubs WHERE id = 7568) AND c.id <> 7568
                   AND p.id NOT IN (SELECT person_id FROM loans) ORDER BY pl.current_ability DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let view = g.transfer_view(person).unwrap();
        assert!(view.comments.len() >= 4, "{:?}", view.comments);
        assert!(view.comments[0].contains("procura") || view.comments[0].contains("composto"));
        assert!(view.comments.iter().any(|c| c.contains("responde até")));
        // Os do treinador não têm comentários
        let mine: i32 =
            g.world.conn.query_row("SELECT id FROM people WHERE club_id = 7568 AND is_player = 1 LIMIT 1", [], |r| r.get(0)).unwrap();
        assert!(g.transfer_view(mine).unwrap().comments.is_empty());
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
