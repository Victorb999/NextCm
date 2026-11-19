//! Sorteio da fase de liga como o da UEFA desde 2024/25: os clubes em potes pela reputação
//! (do mais forte ao mais fraco) e cada um com o mesmo número de adversários de cada pote.
//! Com 8 jornadas, 4 potes e 2 adversários de cada (um em casa, outro fora); com 6, 6 potes
//! e 1 de cada. Nunca um da mesma nação (salvo se um pote tiver nacionais demais) e no
//! máximo 2 de cada outra nação. Depois, as jornadas: em cada uma todos jogam, e o menos
//! possível de clubes fica 3 jornadas seguidas em casa ou fora.

use std::collections::HashMap;

use rand_core::RngCore;

use super::competitions::{orient_evenly, shuffle};

/// Um clube no sorteio: a reputação (para o pote) e a nação (ou o continente nas seleções).
#[derive(Debug, Clone, Copy)]
pub(super) struct Entrant {
    pub id: i32,
    pub strength: i32,
    pub nation: Option<i32>,
}

/// Passos da procura dos confrontos de dois potes antes de recomeçar o sorteio.
const PAIR_TRIES: usize = 2000;
/// Sorteios inteiros antes de desistir (o chamador volta ao calendário antigo).
const DRAW_TRIES: usize = 300;
/// Calendários tentados para os mesmos confrontos (fica o com menos sequências de 3).
const SCHEDULE_TRIES: usize = 60;
/// Passos da procura de uma jornada completa antes de recomeçar o calendário.
const MATCHING_STEPS: usize = 20_000;

/// Quantos potes e quantos adversários de cada: `None` quando os números não dão.
fn pots_for(n: usize, matchdays: usize) -> Option<(usize, usize)> {
    let (pots, per_pot) = if matchdays.is_multiple_of(4) { (4, matchdays / 4) } else { (matchdays, 1) };
    if !(1..=2).contains(&per_pot) || pots == 0 || !n.is_multiple_of(pots) {
        return None;
    }
    let size = n / pots;
    // Dentro do pote: com 1 adversário, pares (tamanho par); com 2, um ciclo (pelo menos 3)
    let ok = match per_pot {
        1 => size.is_multiple_of(2),
        _ => size >= 3,
    };
    ok.then_some((pots, per_pot))
}

/// Os jogos de cada jornada (casa, fora), ou `None` se o sorteio não fecha com estes clubes.
pub(super) fn draw(entrants: &[Entrant], matchdays: usize, rng: &mut impl RngCore) -> Option<Vec<Vec<(i32, i32)>>> {
    let n = entrants.len();
    let (pots, per_pot) = pots_for(n, matchdays)?;
    let mut sorted = entrants.to_vec();
    sorted.sort_by_key(|e| (std::cmp::Reverse(e.strength), e.id));
    let size = n / pots;
    let nation: Vec<Option<i32>> = sorted.iter().map(|e| e.nation).collect();
    for t in 0..DRAW_TRIES {
        // Na 2ª metade, a mesma nação passa a valer (só o máximo de 2): um pote com nacionais
        // demais (4 sauditas em 6) não fecha de outra forma
        let strict = t < DRAW_TRIES / 2;
        let Some(mut games) = pairings(&nation, pots, size, per_pot, strict, rng) else { continue };
        if per_pot == 1 {
            // Um adversário por pote: o mando reparte-se no total (metade em casa)
            let mut all: Vec<(i32, i32)> = games.iter().map(|&(h, a)| (h as i32, a as i32)).collect();
            orient_evenly(std::slice::from_mut(&mut all));
            games = all.into_iter().map(|(h, a)| (h as usize, a as usize)).collect();
        }
        if let Some(days) = schedule(n, &games, matchdays, rng) {
            return Some(days.into_iter().map(|d| d.into_iter().map(|(h, a)| (sorted[h].id, sorted[a].id)).collect()).collect());
        }
    }
    None
}

/// Os adversários que cada clube já tem de cada nação.
struct Counts {
    /// Proíbe dois da mesma nação.
    strict: bool,
    nation: Vec<Option<i32>>,
    met: Vec<HashMap<i32, u8>>,
}

impl Counts {
    /// `a` pode jogar com `b`: nações diferentes (se `strict`) e menos de 2 de cada uma.
    fn allows(&self, a: usize, b: usize) -> bool {
        let (na, nb) = (self.nation[a], self.nation[b]);
        if self.strict && na.is_some() && na == nb {
            return false;
        }
        let under = |x: usize, n: Option<i32>| n.is_none_or(|n| self.met[x].get(&n).copied().unwrap_or(0) < 2);
        under(a, nb) && under(b, na)
    }

    fn add(&mut self, a: usize, b: usize, step: i8) {
        for (x, y) in [(a, b), (b, a)] {
            if let Some(n) = self.nation[y] {
                let c = self.met[x].entry(n).or_default();
                *c = (*c as i8 + step) as u8;
            }
        }
    }
}

/// Os confrontos (casa, fora) em índices de `nation`, pote a pote.
fn pairings(
    nation: &[Option<i32>],
    pots: usize,
    size: usize,
    per_pot: usize,
    strict: bool,
    rng: &mut impl RngCore,
) -> Option<Vec<(usize, usize)>> {
    let mut counts = Counts { strict, nation: nation.to_vec(), met: vec![HashMap::new(); nation.len()] };
    let pot = |p: usize| -> Vec<usize> { (p * size..(p + 1) * size).collect() };
    let mut pairs: Vec<(usize, usize)> = (0..pots).flat_map(|a| (a..pots).map(move |b| (a, b))).collect();
    shuffle(&mut pairs, rng);
    let mut games = Vec::new();
    for (pa, pb) in pairs {
        let (mut x, mut y) = (pot(pa), pot(pb));
        shuffle(&mut x, rng);
        shuffle(&mut y, rng);
        let mut search = Search { counts: &mut counts, steps: 0, games: Vec::new() };
        let ok = if per_pot == 1 {
            search.matching(&x, if pa == pb { None } else { Some(&y) }, rng)
        } else {
            search.cycle(&x, if pa == pb { None } else { Some(&y) }, rng)
        };
        if !ok {
            return None;
        }
        games.extend(search.games);
    }
    Some(games)
}

/// A procura (com volta atrás) dos jogos entre dois potes, a respeitar as nações.
struct Search<'a> {
    counts: &'a mut Counts,
    steps: usize,
    games: Vec<(usize, usize)>,
}

impl Search<'_> {
    fn push(&mut self, h: usize, a: usize) -> bool {
        if !self.counts.allows(h, a) {
            return false;
        }
        self.counts.add(h, a, 1);
        self.games.push((h, a));
        true
    }

    fn pop(&mut self) {
        if let Some((h, a)) = self.games.pop() {
            self.counts.add(h, a, -1);
        }
    }

    /// Com 2 adversários: um ciclo por todos (alternando os dois potes), percorrido num sentido,
    /// e cada clube recebe um e visita o outro.
    fn cycle(&mut self, x: &[usize], y: Option<&[usize]>, rng: &mut impl RngCore) -> bool {
        let len = x.len() + y.map_or(0, <[usize]>::len);
        let mut path = vec![x[0]];
        let mut used = vec![x[0]];
        self.extend_cycle(x, y, len, &mut path, &mut used, rng)
    }

    fn extend_cycle(
        &mut self,
        x: &[usize],
        y: Option<&[usize]>,
        len: usize,
        path: &mut Vec<usize>,
        used: &mut Vec<usize>,
        rng: &mut impl RngCore,
    ) -> bool {
        self.steps += 1;
        if self.steps > PAIR_TRIES {
            return false;
        }
        let last = *path.last().unwrap_or(&x[0]);
        if path.len() == len {
            if self.push(last, path[0]) {
                return true;
            }
            return false;
        }
        let from = match y {
            Some(y) if path.len() % 2 == 1 => y,
            _ => x,
        };
        let mut options: Vec<usize> = from.iter().copied().filter(|c| !used.contains(c)).collect();
        shuffle(&mut options, rng);
        for c in options {
            if !self.push(last, c) {
                continue;
            }
            path.push(c);
            used.push(c);
            if self.extend_cycle(x, y, len, path, used, rng) {
                return true;
            }
            path.pop();
            used.pop();
            self.pop();
        }
        false
    }

    /// Com 1 adversário: pares (dentro do pote, ou um de cada pote).
    fn matching(&mut self, x: &[usize], y: Option<&[usize]>, rng: &mut impl RngCore) -> bool {
        self.steps += 1;
        if self.steps > PAIR_TRIES {
            return false;
        }
        let taken: Vec<usize> = self.games.iter().flat_map(|&(h, a)| [h, a]).collect();
        let Some(&first) = x.iter().find(|c| !taken.contains(c)) else { return true };
        let pool = y.unwrap_or(x);
        let mut options: Vec<usize> = pool.iter().copied().filter(|&c| c != first && !taken.contains(&c)).collect();
        shuffle(&mut options, rng);
        for c in options {
            if !self.push(first, c) {
                continue;
            }
            if self.matching(x, y, rng) {
                return true;
            }
            self.pop();
        }
        false
    }
}

/// Reparte os jogos por jornadas em que todos jogam; das tentativas, a com menos clubes 3
/// jornadas seguidas em casa ou fora.
fn schedule(n: usize, games: &[(usize, usize)], matchdays: usize, rng: &mut impl RngCore) -> Option<Vec<Vec<(usize, usize)>>> {
    let mut best: Option<(usize, Vec<Vec<(usize, usize)>>)> = None;
    for _ in 0..SCHEDULE_TRIES {
        let Some(days) = try_schedule(n, games, matchdays, rng) else { continue };
        let streaks = streaks(n, &days);
        if best.as_ref().is_none_or(|b| streaks < b.0) {
            best = Some((streaks, days));
            if streaks == 0 {
                break;
            }
        }
    }
    best.map(|b| b.1)
}

fn try_schedule(n: usize, games: &[(usize, usize)], matchdays: usize, rng: &mut impl RngCore) -> Option<Vec<Vec<(usize, usize)>>> {
    let mut left: Vec<usize> = (0..games.len()).collect();
    let mut days = Vec::with_capacity(matchdays);
    for _ in 0..matchdays {
        shuffle(&mut left, rng);
        let mut matched = vec![usize::MAX; n];
        let mut chosen = Vec::new();
        let mut steps = 0;
        if !perfect_matching(games, &left, &mut matched, &mut chosen, &mut steps) {
            return None;
        }
        left.retain(|g| !chosen.contains(g));
        days.push(chosen.iter().map(|&g| games[g]).collect());
    }
    Some(days)
}

/// Procura (com volta atrás) jogos de `left` que cubram todos os clubes uma vez.
fn perfect_matching(games: &[(usize, usize)], left: &[usize], matched: &mut [usize], chosen: &mut Vec<usize>, steps: &mut usize) -> bool {
    *steps += 1;
    if *steps > MATCHING_STEPS {
        return false;
    }
    // O clube livre com menos jogos possíveis
    let free = |g: &usize| matched[games[*g].0] == usize::MAX && matched[games[*g].1] == usize::MAX;
    let mut options: HashMap<usize, Vec<usize>> = HashMap::new();
    for g in left.iter().filter(|g| free(g)) {
        let (h, a) = games[*g];
        options.entry(h).or_default().push(*g);
        options.entry(a).or_default().push(*g);
    }
    let Some(pick) = (0..matched.len()).filter(|&c| matched[c] == usize::MAX).min_by_key(|c| options.get(c).map_or(0, Vec::len)) else {
        return true;
    };
    for g in options.remove(&pick).unwrap_or_default() {
        let (h, a) = games[g];
        matched[h] = g;
        matched[a] = g;
        chosen.push(g);
        if perfect_matching(games, left, matched, chosen, steps) {
            return true;
        }
        chosen.pop();
        matched[h] = usize::MAX;
        matched[a] = usize::MAX;
    }
    false
}

/// Quantas vezes um clube joga 3 jornadas seguidas em casa (ou fora).
fn streaks(n: usize, days: &[Vec<(usize, usize)>]) -> usize {
    let mut home = vec![Vec::new(); n];
    for d in days {
        for &(h, a) in d {
            home[h].push(true);
            home[a].push(false);
        }
    }
    home.iter().map(|s| s.windows(3).filter(|w| w[0] == w[1] && w[1] == w[2]).count()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;
    use rand_core::SeedableRng;
    use std::collections::HashSet;

    /// Clubes com tantos de cada nação (`nations[i]` da nação `i`).
    fn field(nations: &[usize]) -> Vec<Entrant> {
        let mut out = Vec::new();
        for (nat, &count) in nations.iter().enumerate() {
            for _ in 0..count {
                out.push(Entrant { id: out.len() as i32 + 100, strength: 0, nation: Some(nat as i32) });
            }
        }
        // Reputação a misturar as nações pelos potes
        for (i, e) in out.iter_mut().enumerate() {
            e.strength = ((i * 7919) % 1000) as i32;
        }
        out
    }

    fn check(entrants: &[Entrant], matchdays: usize, seed: u64) {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let days = draw(entrants, matchdays, &mut rng).expect("o sorteio fecha");
        assert_eq!(days.len(), matchdays);
        let (pots, per_pot) = pots_for(entrants.len(), matchdays).unwrap();
        let mut sorted = entrants.to_vec();
        sorted.sort_by_key(|e| (std::cmp::Reverse(e.strength), e.id));
        let size = entrants.len() / pots;
        let pot_of: HashMap<i32, usize> = sorted.iter().enumerate().map(|(i, e)| (e.id, i / size)).collect();
        let nation: HashMap<i32, Option<i32>> = entrants.iter().map(|e| (e.id, e.nation)).collect();
        for d in &days {
            let ids: HashSet<i32> = d.iter().flat_map(|&(h, a)| [h, a]).collect();
            assert_eq!(ids.len(), entrants.len(), "todos jogam em cada jornada");
        }
        for e in entrants {
            let games: Vec<(i32, bool)> = days
                .iter()
                .flatten()
                .filter_map(|&(h, a)| {
                    if h == e.id {
                        Some((a, true))
                    } else if a == e.id {
                        Some((h, false))
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(games.len(), matchdays);
            assert_eq!(games.iter().filter(|g| g.1).count(), matchdays / 2, "metade em casa");
            let rivals: HashSet<i32> = games.iter().map(|g| g.0).collect();
            assert_eq!(rivals.len(), matchdays, "adversários diferentes");
            for p in 0..pots {
                let from: Vec<&(i32, bool)> = games.iter().filter(|g| pot_of[&g.0] == p).collect();
                assert_eq!(from.len(), per_pot, "{per_pot} de cada pote");
                if per_pot == 2 {
                    assert_eq!(from.iter().filter(|g| g.1).count(), 1, "um em casa e outro fora por pote");
                }
            }
            let mut per_nation: HashMap<i32, usize> = HashMap::new();
            for (r, _) in &games {
                assert_ne!(nation[r], e.nation, "nunca da mesma nação");
                *per_nation.entry(nation[r].unwrap()).or_default() += 1;
            }
            assert!(per_nation.values().all(|&c| c <= 2), "no máximo 2 da mesma nação");
        }
    }

    const EUROPE: [usize; 15] = [6, 5, 5, 4, 3, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1];

    #[test]
    fn league_phase_draw_follows_the_pots() {
        for seed in 0..20 {
            // Como na Liga dos Campeões (6 ingleses, 5 espanhóis...), na Conference e na Ásia (3 por nação)
            check(&field(&EUROPE), 8, seed);
            check(&field(&EUROPE), 6, seed);
            check(&field(&[3; 8]), 8, seed);
        }
        assert!(pots_for(35, 8).is_none());
    }
}
