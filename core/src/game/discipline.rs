//! Gestão do plantel (marco 3.12, passo 1): "Disciplinar Jogador" e elogiar ou criticar um
//! jogador na imprensa, pelo menu Acção do perfil. As razões de um castigo só existem quando
//! aconteceram (expulsões e más exibições ficam guardadas em `player_log` no fim de cada jogo);
//! a reação depende da justiça do castigo e da personalidade. Regras no detalhe do 3.12.

use rusqlite::params;
use serde::Serialize;
use ts_rs::TS;

use super::Game;
use super::board_meeting::BoardRequest;
use super::market::money_text;
use crate::{Error, Result, date};

/// Castigos, do mais leve ao mais pesado: texto e semanas de salário de multa.
pub const ACTIONS: [(&str, i64); 3] =
    [("Aviso Oficial", 0), ("Multa de uma semana de salário", 1), ("Multa de duas semanas de salário", 2)];

/// As razões do CM (a primeira está sempre disponível).
const REASONS: [(&str, &str); 5] = [
    ("none", "Sem Razões"),
    ("unprofessional", "Comportamento Pouco Profissional"),
    ("sent_off", "Expulsão em jogo anterior"),
    ("violent", "Comportamento Violento"),
    ("poor", "Fraca Exibição"),
];

/// Dias em que uma expulsão ou uma má exibição ainda justificam um castigo.
const RECENT_DAYS: i32 = 14;
/// Nota (1–10) abaixo da qual uma exibição é fraca.
const POOR_RATING: f64 = 5.5;
/// Agressividade + Maldade a partir da qual uma expulsão é comportamento violento.
const VIOLENT_TEMPER: i32 = 28;
/// Dias entre dois castigos e entre dois comentários na imprensa sobre o mesmo jogador.
const DISCIPLINE_DAYS: i32 = 7;
const PRESS_DAYS: i32 = 21;

/// O diálogo "Disciplinar Jogador".
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DisciplineOptions {
    pub actions: Vec<String>,
    pub reasons: Vec<BoardRequest>,
}

/// O que interessa da personalidade para as reações.
struct Mind {
    professionalism: i32,
    temperament: i32,
    determination: i32,
    pressure: i32,
}

impl Game {
    /// No fim de um jogo do treinador: expulsões e más exibições, que podem justificar castigos.
    pub(super) fn log_match(&self, sent_off: &[i32], ratings: &[(i32, f64)]) -> Result<()> {
        let mut insert = self.world.conn.prepare_cached("INSERT INTO player_log (person_id, kind, date) VALUES (?, ?, ?)")?;
        for &id in sent_off {
            let temper: i32 = self.world.conn.query_row(
                "SELECT COALESCE(aggression, 10) + COALESCE(dirtiness, 10) FROM players WHERE person_id = ?",
                [id],
                |r| r.get(0),
            )?;
            insert.execute(params![id, if temper >= VIOLENT_TEMPER { "violent" } else { "sent_off" }, self.date])?;
        }
        for &(id, rating) in ratings {
            if rating < POOR_RATING {
                insert.execute(params![id, "poor", self.date])?;
            }
        }
        Ok(())
    }

    fn logged_since(&self, person: i32, kinds: &[&str], days: i32) -> Result<bool> {
        let since = date::add_days(&self.date, -days).unwrap_or_default();
        let list = kinds.iter().map(|k| format!("'{k}'")).collect::<Vec<_>>().join(", ");
        Ok(self.world.conn.query_row(
            &format!("SELECT COUNT(*) FROM player_log WHERE person_id = ? AND kind IN ({list}) AND date > ?"),
            params![person, since],
            |r| r.get::<_, i64>(0),
        )? > 0)
    }

    fn mind(&self, person: i32) -> Result<Mind> {
        Ok(self.world.conn.query_row(
            "SELECT COALESCE(professionalism, 10), COALESCE(temperament, 10), COALESCE(determination, 10), COALESCE(pressure, 10)
             FROM people WHERE id = ?",
            [person],
            |r| Ok(Mind { professionalism: r.get(0)?, temperament: r.get(1)?, determination: r.get(2)?, pressure: r.get(3)? }),
        )?)
    }

    fn own_player(&self, person: i32) -> Result<()> {
        let club: Option<i32> =
            self.world.conn.query_row("SELECT club_id FROM people WHERE id = ? AND is_player = 1", [person], |r| r.get(0))?;
        if club != Some(self.club_id) {
            return Err(Error::Invalid("Só para os jogadores do seu plantel.".into()));
        }
        Ok(())
    }

    /// Os castigos e as razões de "Disciplinar Jogador" (as que não aconteceram ficam apagadas).
    pub fn discipline_options(&self, person: i32) -> Result<DisciplineOptions> {
        self.own_player(person)?;
        let unhappy: bool =
            self.world.conn.query_row("SELECT COUNT(*) FROM unhappy WHERE person_id = ?", [person], |r| r.get::<_, i64>(0))? > 0;
        let mut reasons = Vec::new();
        for (key, label) in REASONS {
            let ok = match key {
                "none" => true,
                "unprofessional" => unhappy,
                "sent_off" => self.logged_since(person, &["sent_off", "violent"], RECENT_DAYS)?,
                "violent" => self.logged_since(person, &["violent"], RECENT_DAYS)?,
                _ => self.logged_since(person, &["poor"], RECENT_DAYS)?,
            };
            reasons.push(BoardRequest {
                key: key.into(),
                label: label.into(),
                available: ok,
                reason: (!ok).then(|| "Não aconteceu nada que o justifique.".to_string()),
            });
        }
        Ok(DisciplineOptions { actions: ACTIONS.iter().map(|a| a.0.to_string()).collect(), reasons })
    }

    /// Aplica um castigo; devolve a reação do jogador (que chega também como mensagem).
    pub fn discipline(&mut self, person: i32, action: usize, reason: &str) -> Result<String> {
        let options = self.discipline_options(person)?;
        let (label, weeks) = *ACTIONS.get(action).ok_or_else(|| Error::Invalid("Castigo desconhecido.".into()))?;
        let why = options.reasons.iter().find(|r| r.key == reason).ok_or_else(|| Error::Invalid("Razão desconhecida.".into()))?;
        if !why.available {
            return Err(Error::Invalid("Essa razão não se aplica a este jogador.".into()));
        }
        if self.logged_since(person, &["discipline"], DISCIPLINE_DAYS)? {
            return Err(Error::Invalid("Já o castigou há poucos dias.".into()));
        }
        let name = self.person_name(person)?;
        let m = self.mind(person)?;
        // A multa entra nas receitas do clube
        let wage: i64 = self.world.conn.query_row("SELECT COALESCE(wage, 0) FROM people WHERE id = ?", [person], |r| r.get(0))?;
        let fine = wage * weeks;
        if fine > 0 {
            self.book(self.club_id, "player_fines", fine)?;
        }
        let fair = reason != "none";
        let (delta, reaction) = if !fair {
            self.add_unhappy(person, "unfair")?;
            (-4 - weeks as i32, format!("{name} não percebe o castigo e sente que foi tratado injustamente."))
        } else if m.professionalism >= 15 {
            (1, format!("{name} aceitou o castigo e promete dar a volta por cima."))
        } else if m.temperament <= 7 {
            self.add_unhappy(person, "unfair")?;
            (-3 - weeks as i32, format!("{name} reagiu mal ao castigo e discutiu com o treinador."))
        } else {
            (-1, format!("{name} aceitou o castigo sem comentários."))
        };
        self.change_morale(person, delta);
        self.world
            .conn
            .execute("INSERT INTO player_log (person_id, kind, date) VALUES (?, 'discipline', ?)", params![person, self.date])?;
        let fined = if fine > 0 { format!(" A multa de {} entra nas contas do clube.", money_text(fine)) } else { String::new() };
        let body = format!("{label} a {name} ({}).{fined}\n\n{reaction}", REASONS.iter().find(|r| r.0 == reason).map_or("", |r| r.1));
        self.post_news_as("press", format!("{name} castigado"), body);
        self.unsaved = true;
        Ok(reaction)
    }

    /// Elogiar ("praise") ou criticar ("criticise") um jogador na imprensa; devolve a reação.
    pub fn press_comment(&mut self, person: i32, kind: &str) -> Result<String> {
        self.own_player(person)?;
        if kind != "praise" && kind != "criticise" {
            return Err(Error::Invalid("Comentário desconhecido.".into()));
        }
        if self.logged_since(person, &["praise", "criticise"], PRESS_DAYS)? {
            return Err(Error::Invalid("Já falou dele à imprensa há pouco tempo.".into()));
        }
        let (title, reaction) = self.press_reaction(person, kind == "praise")?;
        self.world.conn.execute("INSERT INTO player_log (person_id, kind, date) VALUES (?, ?, ?)", params![person, kind, self.date])?;
        self.post_news_as("press", title, reaction.clone());
        self.unsaved = true;
        Ok(reaction)
    }

    /// O efeito de um elogio (ou de uma crítica) em público, pela personalidade: muda a moral
    /// e devolve o título e a reação para a mensagem. Também nas conferências de imprensa.
    pub(super) fn press_reaction(&mut self, person: i32, praise: bool) -> Result<(String, String)> {
        let name = self.person_name(person)?;
        let m = self.mind(person)?;
        let st = self.squads.state(person);
        let (delta, title, reaction) = if praise {
            if st.morale >= 18 {
                (0, format!("{name} elogiado"), format!("{name} agradeceu o elogio, mas já anda nas nuvens: arrisca-se a facilitar."))
            } else {
                // Vale mais para quem está em baixo ou joga pouco
                let boost = if st.morale <= 8 || st.missed >= 3 { 4 } else { 2 };
                (boost, format!("{name} elogiado"), format!("{name} ficou contente com os elogios do treinador na imprensa."))
            }
        } else if m.determination + m.professionalism >= 30 {
            (1, format!("{name} criticado"), format!("{name} respondeu bem às críticas do treinador e promete trabalhar mais."))
        } else if m.temperament <= 7 || m.pressure <= 7 {
            self.add_unhappy(person, "manager")?;
            (-4, format!("{name} criticado"), format!("{name} não gostou das críticas públicas do treinador e perdeu a confiança nele."))
        } else {
            (-2, format!("{name} criticado"), format!("{name} ficou abalado com as críticas do treinador."))
        };
        self.change_morale(person, delta);
        Ok((title, reaction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PALMEIRAS: i32 = 7568;

    #[test]
    fn discipline_and_the_press() {
        let Some(path) = crate::World::locate(std::path::Path::new(env!("CARGO_MANIFEST_DIR"))) else { return };
        let dir = std::env::temp_dir().join(format!("nextcm-disciplina-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = crate::NewGame {
            save_name: "Disciplina".into(),
            first_name: "Victor".into(),
            last_name: "Araujo".into(),
            birth_date: None,
            nation_id: None,
            club_id: PALMEIRAS,
        };
        let mut g = Game::create(&path, &dir, &new, 23).unwrap();
        let ids: Vec<i32> = g
            .world
            .conn
            .prepare("SELECT id FROM people WHERE club_id = ? AND is_player = 1 ORDER BY id LIMIT 4")
            .unwrap()
            .query_map([PALMEIRAS], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let (calm, hot, pro, other) = (ids[0], ids[1], ids[2], ids[3]);
        let set = |g: &Game, id: i32, sql: &str| g.world.conn.execute(&format!("UPDATE people SET {sql} WHERE id = ?"), [id]).unwrap();
        set(&g, hot, "professionalism = 5, temperament = 4, determination = 5, pressure = 5");
        set(&g, pro, "professionalism = 18, temperament = 15, determination = 18, pressure = 15");
        set(&g, calm, "professionalism = 10, temperament = 12, determination = 10, pressure = 12");

        // Sem nada para castigar, só "Sem Razões"
        let opts = g.discipline_options(hot).unwrap();
        assert_eq!(opts.actions.len(), 3);
        assert!(opts.reasons[0].available && opts.reasons.iter().skip(1).all(|r| !r.available));
        assert!(g.discipline(hot, 0, "sent_off").is_err());

        // Uma expulsão e uma má exibição no último jogo
        g.log_match(&[hot, pro], &[(calm, 4.8)]).unwrap();
        assert!(g.discipline_options(pro).unwrap().reasons.iter().find(|r| r.key == "sent_off").unwrap().available);
        assert!(g.discipline_options(calm).unwrap().reasons.iter().find(|r| r.key == "poor").unwrap().available);

        // O profissional aceita e até motiva; o de cabeça quente revolta-se; a multa entra nas contas
        let morale = |g: &Game, id: i32| g.squads.state(id).morale;
        let (p0, h0) = (morale(&g, pro), morale(&g, hot));
        let cash_of = |g: &Game| {
            g.world.conn.query_row("SELECT COALESCE(cash, 0) FROM clubs WHERE id = ?", [PALMEIRAS], |r| r.get::<_, i64>(0)).unwrap()
        };
        let cash = cash_of(&g);
        assert!(g.discipline(pro, 1, "sent_off").unwrap().contains("aceitou"));
        assert!(morale(&g, pro) > p0);
        assert!(cash_of(&g) >= cash);
        g.discipline(hot, 0, "sent_off").unwrap();
        assert!(morale(&g, hot) < h0);
        assert!(g.unhappy_reasons(hot).unwrap().iter().any(|r| r.contains("injustamente")));
        assert!(g.discipline(hot, 0, "none").is_err(), "um castigo de cada vez");

        // Castigar sem razão revolta sempre
        let c0 = morale(&g, calm);
        g.discipline(calm, 0, "none").unwrap();
        assert!(morale(&g, calm) <= c0 - 4);

        // Imprensa: elogio sobe a moral; crítica ao determinado motiva, ao frágil abala
        let o0 = morale(&g, other);
        g.press_comment(other, "praise").unwrap();
        assert!(morale(&g, other) > o0);
        assert!(g.press_comment(other, "criticise").is_err(), "um comentário de cada vez");
        g.date = date::add_days(&g.date, PRESS_DAYS + 1).unwrap();
        let p1 = morale(&g, pro);
        assert!(g.press_comment(pro, "criticise").unwrap().contains("respondeu bem"));
        assert!(morale(&g, pro) > p1);
        g.press_comment(hot, "criticise").unwrap();
        assert!(g.unhappy_reasons(hot).unwrap().iter().any(|r| r.contains("confiança no treinador")));
        assert!(g.news().iter().any(|n| n.title.ends_with("criticado")));
        drop(g);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
