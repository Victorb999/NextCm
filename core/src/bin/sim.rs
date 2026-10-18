//! CLI do NextCM para rodar o jogo sem interface (balanceamento e benchmark).
//!
//! `cargo run -p nextcm-core --bin sim -- <id do clube> <dias>`
//!
//! Marco 3.1: cria um jogo temporário no clube, avança os dias e mostra o estado.

use std::path::Path;
use std::time::Instant;

use nextcm_core::{Game, NewGame, World};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let club_id: i32 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(7568);
    let days: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(30);

    let world = World::locate(&std::env::current_dir().unwrap())
        .or_else(|| World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))))
        .expect("data/world.sqlite não encontrado");
    let dir = std::env::temp_dir().join(format!("nextcm-sim-{}", std::process::id()));

    let t = Instant::now();
    let new = NewGame {
        save_name: "sim".into(),
        first_name: "Treinador".into(),
        last_name: "Sim".into(),
        birth_date: None,
        nation_id: None,
        club_id,
    };
    let mut game = Game::create(&world, &dir, &new, 1).expect("não criou o jogo");
    println!("jogo criado em {:?}", t.elapsed());

    let t = Instant::now();
    for _ in 0..days {
        game.advance_day().expect("falhou ao avançar o dia");
    }
    game.save().expect("não gravou");
    let info = game.info().unwrap();
    println!(
        "{} no {}: {} dias em {:?}, data {}, {} notícia(s) por ler",
        info.manager_name, info.club.short_name, days, t.elapsed(), info.date, info.unread_news
    );

    drop(game);
    let _ = std::fs::remove_dir_all(&dir);
}
