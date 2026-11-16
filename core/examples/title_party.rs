//! A festa de um título já ganho num save antigo (as notícias da torcida e da Diretoria).
//! Uso: `cargo run -p nextcm-core --release --example title_party -- <save.sqlite> <competição> <data>`
//! Feche o jogo antes: o jogo aberto grava as notícias da memória por cima.

use nextcm_core::Game;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, path, comp, date] = args.as_slice() else {
        eprintln!("uso: title_party <save.sqlite> <competição> <data AAAA-MM-DD>");
        std::process::exit(2);
    };
    let mut game = Game::load(std::path::Path::new(path)).expect("não abriu o save");
    let news = game.celebrate_title(comp.parse().expect("competição"), date).expect("festa");
    if news.is_empty() {
        eprintln!("o clube do treinador não é o campeão desta competição nesta temporada");
        std::process::exit(1);
    }
    for n in &news {
        println!("{} | {}\n{}\n", n.date, n.title, n.body);
    }
    game.save().expect("não gravou");
}
