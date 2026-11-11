//! Vontades no mercado (marco 3.24): se o clube aceita vender e se o jogador aceita ir. As
//! mesmas contas para a IA e para as propostas do treinador. Nada é proibido: um indispensável,
//! um acabado de chegar ou uma venda ao rival saem muito mais caros e raros (o Figo foi do
//! Barcelona para o Real). Só as contas, puras e testadas; quem as usa lê os dados.

/// Como o clube que vende vê o jogador.
#[derive(Debug, Clone, Copy, Default)]
pub struct Sale {
    /// Na lista de transferências do clube: quer vendê-lo.
    pub listed: bool,
    /// Pediu para sair (ou está insatisfeito): o clube quer resolver.
    pub unhappy: bool,
    /// Um dos 11 melhores do plantel.
    pub starter: bool,
    /// Vale uma parte grande do plantel (`market::STAR_SHARE`): indispensável.
    pub star: bool,
    /// Chegou há pouco ao clube.
    pub recent: bool,
    /// O comprador é rival do clube.
    pub rival: bool,
}

/// Porque é que o clube resiste à venda (o texto da recusa, como no CM).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Holdout {
    Star,
    Recent,
    Rival,
}

impl Holdout {
    pub fn text(self) -> &'static str {
        match self {
            Holdout::Star => "O clube considera-o um elemento indispensável no elenco.",
            Holdout::Recent => "O clube acabou de o comprar e olha-o como um importante jogador para o futuro.",
            Holdout::Rival => "O clube não quer reforçar um rival.",
        }
    }
}

/// O que o clube pede (× o valor de mercado) e a probabilidade de aceitar vender pagando isso.
/// Listado: 90% e quase sempre (ao rival, +20%). Pediu para sair: 15% de desconto e o
/// "indispensável" pesa metade. Titular: +40%. Indispensável: +15% e 35% (raro, não caro: com
/// +60% os negócios de €100M+ triplicavam na CLI); acabado de chegar:
/// +25% e 30%; rival: o dobro e 30%. As resistências multiplicam-se; nunca abaixo de 5%.
pub fn sale_terms(s: &Sale) -> (f64, f64) {
    if s.listed {
        return if s.rival { (0.9 * 1.2, 0.85) } else { (0.9, 0.97) };
    }
    let mut price = if s.starter { 1.4 } else { 1.0 };
    let mut chance: f64 = 1.0;
    if s.star {
        price *= if s.unhappy { 1.05 } else { 1.15 };
        chance *= if s.unhappy { 0.65 } else { 0.35 };
    }
    if s.recent {
        price *= 1.25;
        chance *= 0.3;
    }
    if s.rival {
        price *= 2.0;
        chance *= 0.3;
    }
    if s.unhappy {
        price *= 0.85;
        chance = (chance * 1.5).min(1.0);
    }
    (price, chance.max(0.05))
}

/// A resistência principal do clube (para o texto da recusa), a mais forte primeiro.
pub fn holdout(s: &Sale) -> Option<Holdout> {
    if s.listed {
        return s.rival.then_some(Holdout::Rival);
    }
    if s.rival {
        Some(Holdout::Rival)
    } else if s.star && !s.unhappy {
        Some(Holdout::Star)
    } else if s.recent {
        Some(Holdout::Recent)
    } else {
        None
    }
}

/// Uma proposta acima do pedido vence a resistência mais vezes: a probabilidade cresce com o
/// que se oferece a mais (o dobro do pedido chega quase sempre).
pub fn sale_chance_for(base: f64, fee: f64, asking: f64) -> f64 {
    if asking <= 0.0 {
        return 1.0;
    }
    let over = (fee / asking - 1.0).max(0.0);
    (base + (1.0 - base) * over.min(1.0)).clamp(0.0, 1.0)
}

/// Probabilidade de o jogador aceitar mudar-se para o rival do seu clube: de 60% (Lealdade 1)
/// a 5% (20); insatisfeito ou a pedir para sair, +25%. Para os outros clubes, 1.
pub fn rival_move_chance(loyalty: f64, unhappy: bool) -> f64 {
    let base = 0.05 + 0.55 * (20.0 - loyalty.clamp(1.0, 20.0)) / 19.0;
    (base + if unhappy { 0.25 } else { 0.0 }).min(1.0)
}

/// O salário que pede para ir para o rival: +30%.
pub const RIVAL_WAGE: f64 = 1.3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_and_unhappy_are_easier() {
        let normal = Sale::default();
        assert_eq!(sale_terms(&normal), (1.0, 1.0));
        let listed = Sale { listed: true, star: true, recent: true, ..normal };
        assert_eq!(sale_terms(&listed), (0.9, 0.97), "listado: o clube quer vender, nem o indispensável conta");
        assert_eq!(holdout(&listed), None);
        let (p, c) = sale_terms(&Sale { listed: true, rival: true, ..normal });
        assert!((p - 1.08).abs() < 1e-9 && c > 0.8, "listado ao rival: pouco mais caro");
        // O insatisfeito sai mais barato e o indispensável pesa menos
        let star = Sale { star: true, starter: true, ..normal };
        let upset = Sale { unhappy: true, ..star };
        assert!(sale_terms(&upset).0 < sale_terms(&star).0 && sale_terms(&upset).1 > sale_terms(&star).1);
        assert_eq!(holdout(&star), Some(Holdout::Star));
        assert_eq!(holdout(&upset), None);
    }

    #[test]
    fn rival_and_stars_are_hard_not_impossible() {
        let figo = Sale { star: true, starter: true, rival: true, ..Default::default() };
        let (price, chance) = sale_terms(&figo);
        assert!(price > 3.0, "{price}");
        assert!((0.05..0.2).contains(&chance), "{chance}");
        assert_eq!(holdout(&figo), Some(Holdout::Rival));
        // Pagar muito acima do pedido abre a porta
        assert!(sale_chance_for(chance, 2.0, 1.0) > 0.99);
        assert!((sale_chance_for(chance, 1.0, 1.0) - chance).abs() < 1e-9);
        assert!(sale_chance_for(0.3, 1.5, 1.0) > 0.6);
        // O jogador: o leal quase nunca vai para o rival; o insatisfeito vai mais
        assert!((rival_move_chance(20.0, false) - 0.05).abs() < 1e-9);
        assert!((rival_move_chance(1.0, false) - 0.6).abs() < 1e-9);
        assert!(rival_move_chance(10.0, true) > rival_move_chance(10.0, false));
    }
}
