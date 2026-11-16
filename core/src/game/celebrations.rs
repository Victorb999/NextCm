//! A festa de um título: no clube do treinador, a notícia da torcida na rua (com o nome do
//! artilheiro nos cânticos) e a mensagem de parabéns da Diretoria (que sobe a confiança nas
//! copas; a liga conta no balanço da temporada). Nos títulos dos outros, uma linha da festa
//! na notícia do campeão. Os textos variam por título sem gastar o RNG do jogo.

use rusqlite::OptionalExtension;

use super::{Game, NewsItem, internationals};
use crate::Result;

/// Confiança da Diretoria e satisfação da torcida por uma copa nacional e por uma continental
/// (a liga já sobe +20 no balanço da temporada; aí só a torcida).
const CUP_BOOST: (i32, i32) = (8, 8);
const CONTINENTAL_BOOST: (i32, i32) = (15, 12);
const LEAGUE_FANS: i32 = 10;

/// Que título foi.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Title {
    League,
    Cup,
    Continental,
}

/// O que se sabe da campanha para contar a festa.
struct Campaign {
    club: String,
    short: String,
    city: Option<String>,
    stadium: Option<String>,
    /// O artilheiro do clube na competição e os gols.
    scorer: Option<(String, i32)>,
    /// O melhor pelas notas (com pelo menos 3 jogos), se não for o artilheiro.
    star: Option<String>,
    /// A final: o adversário e os autores dos gols do campeão nela.
    final_against: Option<String>,
    final_scorers: Vec<String>,
}

/// Uma das `n` variantes, fixa para um título (competição, ano, clube): o misturador do
/// splitmix64, para dois títulos do mesmo clube na mesma temporada não saírem iguais.
fn pick(n: usize, comp: i32, year: i32, club: i32, salt: u64) -> usize {
    let mut h = (comp as u64) << 40 ^ (year as u64) << 20 ^ club as u64 ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for _ in 0..2 {
        h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        h ^= h >> 31;
    }
    (h % n as u64) as usize
}

/// O nome da cidade em português, quando o do banco (em inglês) tem outro.
fn city_pt(name: &str) -> &str {
    match name {
        "Rome" => "Roma",
        "Milan" => "Milão",
        "Turin" => "Turim",
        "Naples" => "Nápoles",
        "Florence" => "Florença",
        "Genoa" => "Gênova",
        "Venice" => "Veneza",
        "Munich" => "Munique",
        "Cologne" => "Colônia",
        "Hamburg" => "Hamburgo",
        "London" => "Londres",
        "Edinburgh" => "Edimburgo",
        "Lisbon" => "Lisboa",
        "Oporto" => "Porto",
        "Seville" => "Sevilha",
        "Zaragoza" => "Saragoça",
        "La Coruna" | "A Coruña" | "La Coruña" => "Corunha",
        "Valencia" => "Valência",
        "Marseille" => "Marselha",
        "Brussels" => "Bruxelas",
        "Antwerp" => "Antuérpia",
        "Rotterdam" => "Roterdã",
        "The Hague" => "Haia",
        "Geneva" => "Genebra",
        "Zurich" => "Zurique",
        "Basel" => "Basileia",
        "Vienna" => "Viena",
        "Prague" => "Praga",
        "Warsaw" => "Varsóvia",
        "Copenhagen" => "Copenhague",
        "Stockholm" => "Estocolmo",
        "Gothenburg" => "Gotemburgo",
        "Athens" => "Atenas",
        "Istanbul" => "Istambul",
        "Moscow" => "Moscou",
        "St. Petersburg" | "Saint Petersburg" => "São Petersburgo",
        "Belgrade" => "Belgrado",
        "Bucharest" => "Bucareste",
        "Mexico City" => "Cidade do México",
        _ => name,
    }
}

/// "A", "A e B", "A, B e C".
fn joined(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} e {last}", rest.join(", ")),
    }
}

impl Game {
    /// A notícia da festa da torcida e a mensagem da Diretoria, se o campeão de `comp` é o
    /// clube do treinador.
    pub(super) fn title_party(&mut self, comp: i32, fresh: &mut Vec<NewsItem>, next_id: &mut i32) -> Result<()> {
        if internationals::is_national_comp(comp) || self.sacked() {
            return Ok(());
        }
        let year = self.current_season(comp);
        let Some((winner, ..)) = self.podium(comp, year) else { return Ok(()) };
        if winner != self.club_id {
            return Ok(());
        }
        let title = self.title_kind(comp, year)?;
        let c = self.campaign(comp, year, winner)?;
        let label = crate::regulation::season_label(comp, year);
        let the_comp = self.the_competition(comp);
        let of_comp = self.of_competition(comp);
        let manager = self.person_name(self.manager_id)?;
        // Os títulos já festejados nos últimos 300 dias: cada um roda as variantes, para dois
        // títulos na mesma temporada não contarem a festa com as mesmas frases
        let since = crate::date::parse(&self.date).unwrap_or(0) - 300;
        let earlier = self
            .news
            .iter()
            .chain(fresh.iter())
            .filter(|n| n.kind == "competition" && n.title.starts_with("Diretoria ") && crate::date::parse(&n.date).unwrap_or(0) >= since)
            .count();
        let v = |n: usize, salt: u64| (pick(n, 0, year, winner, salt) + earlier) % n;

        // A torcida
        let headline = match (v(4, 1), &c.city) {
            (1, Some(city)) => format!("Festa nas ruas de {city}"),
            (2, Some(city)) => format!("{}: noite de festa em {city}", c.short),
            (3, _) => format!("Torcida do {} faz a festa", c.short),
            _ => format!("Torcedores celebram conquista {of_comp}"),
        };
        let mut body = match (v(3, 2), &c.city, &c.stadium) {
            (0, Some(city), _) => format!(
                "Milhares de torcedores do {} tomaram as ruas de {city} logo depois do apito final. Bandeiras, sinalizadores e buzinaço seguiram madrugada adentro para celebrar a conquista {of_comp} {label}.",
                c.short
            ),
            (1, _, Some(stadium)) => format!(
                "A torcida do {} lotou os arredores do {stadium} para receber o elenco com a taça, e a cantoria ainda continuava nos bares em volta do estádio quando o dia nasceu.",
                c.short
            ),
            (_, Some(city), _) => format!(
                "Fogos de artifício iluminaram o céu de {city}: a torcida do {} saiu de casa para festejar {the_comp} {label} e a praça mais movimentada da cidade virou um mar de bandeiras.",
                c.short
            ),
            _ => {
                format!("A torcida do {} saiu à rua para festejar {the_comp} {label}, com bandeiras e cânticos até de madrugada.", c.short)
            }
        };
        if let Some(opp) = &c.final_against {
            body += &match (v(2, 3), c.final_scorers.as_slice()) {
                (_, []) => format!(" A vitória na final sobre o {opp} ainda era o assunto em cada esquina."),
                (0, scorers) => format!(" Os gols de {} na final contra o {opp} foram revistos no telão vezes sem conta.", joined(scorers)),
                (_, scorers) => format!(
                    " Na final contra o {opp}, {} {} decidir.",
                    joined(scorers),
                    if scorers.len() == 1 { "ajudou a" } else { "ajudaram a" }
                ),
            };
        }
        body += "\n\n";
        body += &match (&c.scorer, v(3, 4)) {
            (Some((name, goals)), 0) => {
                format!(
                    "O nome mais gritado foi o de {name}, artilheiro do time na campanha com {goals} gol{}.",
                    if *goals == 1 { "" } else { "s" }
                )
            }
            (Some((name, goals)), 1) => format!(
                "\"Ô, ô, ô, {name} é terror!\": o artilheiro da campanha ({goals} gol{}) foi carregado nos ombros dos torcedores no desembarque do elenco.",
                if *goals == 1 { "" } else { "s" }
            ),
            (Some((name, _)), _) => format!("Entre os cânticos, um coro não parava: o nome de {name}, o goleador do time no torneio."),
            (None, _) => "Os jogadores foram aplaudidos um a um na saída do estádio.".to_string(),
        };
        if let Some(star) = &c.star {
            body += &format!(" {star}, o melhor do time nas notas, também teve direito a coro próprio.");
        }
        // Como no CM: a torcida canta em coro o nome do treinador
        body += &match v(3, 5) {
            0 => format!(" E houve música para o treinador: \"{manager}, {manager}!\" ecoou até o ônibus partir."),
            1 => format!(" A torcida entoou em coro o nome do treinador {manager}, que teve de ir à varanda da sede para agradecer."),
            _ => format!(" O coro para {manager} não parou a noite inteira, e numa das faixas lia-se: \"Fica, professor!\""),
        };
        body += &match (title, v(2, 6)) {
            (Title::League, 0) => "\n\nO elenco deve desfilar em carro aberto pela cidade nos próximos dias, com a taça.".to_string(),
            (Title::Continental, _) => {
                format!("\n\nPara muitos torcedores, foi a noite mais importante da história recente do {}.", c.short)
            }
            (_, 0) => "\n\nA taça vai ficar exposta na sede do clube para quem quiser tirar uma foto.".to_string(),
            _ => "\n\nNas redes sociais, as fotos da festa somavam milhões de visualizações antes do amanhecer.".to_string(),
        };
        fresh.push(self.news_item(*next_id, "competition", headline, body));
        *next_id += 1;

        // A Diretoria
        let quote = match (title, v(2, 7)) {
            (Title::League, 0) => {
                "Este título é fruto de uma temporada inteira de trabalho sério. A torcida esperou por isso, e você entregou.".to_string()
            }
            (Title::League, _) => format!(
                "Ser campeão {of_comp} é o maior objetivo do {} todos os anos. Hoje o clube inteiro tem orgulho do seu trabalho.",
                c.short
            ),
            (Title::Cup, 0) => {
                "Uma taça é sempre uma taça, e esta vai ficar na nossa sala de troféus. Que seja a primeira de muitas.".to_string()
            }
            (Title::Cup, _) => "Copa se ganha com coragem nos jogos grandes, e o time mostrou isso do primeiro ao último jogo.".to_string(),
            (Title::Continental, 0) => {
                format!("Levar o nome do {} por todo o continente e voltar com a taça é algo que fica para a história.", c.short)
            }
            (Title::Continental, _) => {
                "Conquistas como esta mudam o tamanho de um clube. O mundo inteiro está olhando para nós.".to_string()
            }
        };
        let effect = match title {
            Title::League => "A Diretoria vai levar a conquista em conta no balanço da temporada.",
            Title::Cup | Title::Continental => "A confiança da Diretoria no seu trabalho subiu.",
        };
        // Há quanto tempo o treinador está no clube
        let joined: Option<String> = self
            .world
            .conn
            .query_row(
                "SELECT joined FROM manager_history WHERE club_id = ? AND left IS NULL ORDER BY joined DESC LIMIT 1",
                [self.club_id],
                |r| r.get(0),
            )
            .optional()?;
        // Dias entre a chegada e o início desta edição da competição (a pré-temporada ainda conta)
        let start = self.year_matches(comp, year).map(|m| m.date.clone()).min().unwrap_or_else(|| self.date.clone());
        let days = joined.and_then(|j| Some(crate::date::parse(&start)? - crate::date::parse(&j)?)).unwrap_or(0);
        let tenure = if days < 200 {
            format!(
                "Ao chegar ao título logo na primeira temporada no clube, {manager} provou que é um dos melhores técnicos do seu escalão."
            )
        } else {
            format!(
                "Na sua {}ª temporada no clube, {manager} vê o trabalho recompensado, e a Diretoria não esconde que acertou ao apostar nele.",
                1 + (days + 165) / 365
            )
        };
        let body = format!(
            "A Diretoria do {} declarou estar radiante com a conquista {of_comp} {label}, e o presidente fez questão de cumprimentar pessoalmente {manager}:\n\n\"{quote}\"\n\n{tenure}\n\n{effect}",
            c.club
        );
        let headline = match v(2, 9) {
            0 => format!("Diretoria radiante com a conquista {of_comp}"),
            _ => format!("Diretoria dá os parabéns pelo título {of_comp}"),
        };
        fresh.push(self.news_item(*next_id, "competition", headline, body));
        *next_id += 1;

        let (confidence, fans) = match title {
            Title::League => (0, LEAGUE_FANS),
            Title::Cup => CUP_BOOST,
            Title::Continental => CONTINENTAL_BOOST,
        };
        self.change_fans(fans)?;
        self.change_confidence(confidence, fresh, next_id)
    }

    /// A festa de um título já ganho, num save de antes desta notícia existir: as notícias saem
    /// com a data `date` (a do título), sem mexer na data do jogo. Ferramenta de dev
    /// (`examples/title_party.rs`).
    pub fn celebrate_title(&mut self, comp: i32, date: &str) -> Result<Vec<NewsItem>> {
        let today = std::mem::replace(&mut self.date, date.to_string());
        let mut fresh = Vec::new();
        let mut next_id = self.news.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let done = self.title_party(comp, &mut fresh, &mut next_id);
        self.date = today;
        done?;
        for n in &fresh {
            self.news.push(n.clone());
        }
        self.news.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
        self.unsaved = true;
        Ok(fresh)
    }

    /// A linha da festa na notícia do campeão de outro clube ("" sem cidade conhecida).
    pub(super) fn party_line(&self, comp: i32, year: i32, winner: i32) -> Result<String> {
        if internationals::is_national_comp(comp) {
            return Ok(String::new());
        }
        let c = self.campaign(comp, year, winner)?;
        let Some(city) = &c.city else { return Ok(String::new()) };
        Ok(match (&c.scorer, pick(3, comp, year, winner, 8)) {
            (Some((name, _)), 0) => format!("\n\nEm {city}, a torcida festejou noite adentro, gritando o nome de {name}."),
            (_, 1) => format!("\n\nBuzinaço e fogos em {city}: a torcida do {} saiu à rua para comemorar.", c.short),
            _ => format!("\n\nA festa da torcida do {} tomou conta de {city}.", c.short),
        })
    }

    /// Liga (a divisão do clube), copa continental (clubes de mais de uma nação) ou copa.
    fn title_kind(&self, comp: i32, year: i32) -> Result<Title> {
        if self.division_of(self.club_id)? == Some(comp) {
            return Ok(Title::League);
        }
        let clubs: Vec<String> = self.year_matches(comp, year).flat_map(|m| [m.home_id, m.away_id]).map(|c| c.to_string()).collect();
        if clubs.is_empty() {
            return Ok(Title::Cup);
        }
        let nations: i32 = self.world.conn.query_row(
            &format!("SELECT COUNT(DISTINCT nation_id) FROM clubs WHERE id IN ({})", clubs.join(",")),
            [],
            |r| r.get(0),
        )?;
        Ok(if nations > 1 { Title::Continental } else { Title::Cup })
    }

    /// Os dados da campanha de `club` em `comp` na época `year`.
    fn campaign(&self, comp: i32, year: i32, club: i32) -> Result<Campaign> {
        let r = self.club(club);
        let (stadium, city): (Option<String>, Option<String>) = self
            .world
            .conn
            .query_row(
                "SELECT s.name, ci.name FROM clubs c LEFT JOIN stadiums s ON s.id = c.stadium_id
                 LEFT JOIN cities ci ON ci.id = s.city_id WHERE c.id = ?",
                [club],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?
            .unwrap_or((None, None));
        let games: Vec<&crate::league::Match> =
            self.year_matches(comp, year).filter(|m| m.result.is_some() && (m.home_id == club || m.away_id == club)).collect();
        let ids: Vec<String> = games.iter().map(|m| m.id.to_string()).collect();
        let (mut scorer, mut star) = (None, None);
        if !ids.is_empty() {
            let list = ids.join(",");
            let top: Option<(i32, i32)> = self
                .world
                .conn
                .query_row(
                    &format!(
                        "SELECT person_id, SUM(goals) g FROM player_matches WHERE club = ? AND fixture IN ({list})
                         GROUP BY person_id HAVING g > 0 ORDER BY g DESC, person_id LIMIT 1"
                    ),
                    [club],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let best: Option<i32> = self
                .world
                .conn
                .query_row(
                    &format!(
                        "SELECT person_id FROM player_matches WHERE club = ? AND fixture IN ({list})
                         GROUP BY person_id HAVING COUNT(*) >= 3 ORDER BY AVG(rating) DESC, person_id LIMIT 1"
                    ),
                    [club],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some((id, goals)) = top {
                scorer = Some((self.person_name(id)?, goals));
            }
            if let Some(id) = best.filter(|&b| top.is_none_or(|t| t.0 != b)) {
                star = Some(self.person_name(id)?);
            }
        }
        // A final: o último jogo de uma eliminatória (com outro clube na mesma ronda, não é final)
        let (mut final_against, mut final_scorers) = (None, Vec::new());
        if let Some(last) = games.iter().max_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)))
            && last.tie.is_some()
            && !self.year_matches(comp, year).any(|m| m.stage == last.stage && m.round == last.round && m.tie != last.tie)
        {
            let opp = if last.home_id == club { last.away_id } else { last.home_id };
            final_against = Some(self.club(opp).short_name);
            let mut stmt = self.world.conn.prepare(
                "SELECT person_id FROM player_matches WHERE club = ? AND fixture = ? AND goals > 0 ORDER BY goals DESC, person_id",
            )?;
            let who: Vec<i32> = stmt.query_map([club, last.id], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            for id in who {
                final_scorers.push(self.person_name(id)?);
            }
        }
        let city = city.map(|c| city_pt(&c).to_string());
        Ok(Campaign { club: r.name, short: r.short_name, city, stadium, scorer, star, final_against, final_scorers })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_and_names() {
        assert_eq!(joined(&["A".into(), "B".into(), "C".into()]), "A, B e C");
        assert_eq!(joined(&["A".into()]), "A");
        assert_eq!((city_pt("Rome"), city_pt("São Paulo")), ("Roma", "São Paulo"));
        assert_eq!(pick(3, 1, 2026, 7568, 1), pick(3, 1, 2026, 7568, 1), "o mesmo título, o mesmo texto");
        let spread: std::collections::HashSet<usize> = (0..40).map(|c| pick(3, c, 2026, 7568, 1)).collect();
        assert_eq!(spread.len(), 3, "as variantes aparecem todas");
    }
}
