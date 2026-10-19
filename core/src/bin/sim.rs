//! CLI do NextCM para rodar o jogo sem interface (balanceamento e benchmark).
//!
//! `cargo run -p nextcm-core --release --bin sim -- [id do clube] [dias|epoca] [seed] [épocas]`
//!
//! Marco 3.2: cria um jogo temporário no clube e avança os dias pedidos ou, com `epoca`
//! (o padrão), até ao fim da Série A; imprime a classificação e os números do match
//! engine. Com mais de uma época (seeds seguidas), mostra a média de pontos e de posição.
//! Marco 3.4: também os números dos lances (remates, cantos, faltas, cartões,
//! substituições e lesões por equipa e jogo).

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use nextcm_core::{Game, NewGame, SERIE_A, StandingRow, World};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let club_id: i32 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(7568);
    let days: Option<u32> = args.get(2).and_then(|a| a.parse().ok());
    let seed: i64 = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(1);
    let seasons: i64 = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(1).max(1);

    let world = World::locate(&std::env::current_dir().unwrap())
        .or_else(|| World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))))
        .expect("data/world.sqlite não encontrado");
    let dir = std::env::temp_dir().join(format!("nextcm-sim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);

    // id do clube → (nome, soma dos pontos, soma das posições, títulos)
    let mut totals: HashMap<i32, (String, i32, i32, i32)> = HashMap::new();
    let (mut goals, mut games, mut home_wins, mut draws) = (0, 0, 0, 0);
    // remates, à baliza, cantos, faltas, amarelos, vermelhos, substituições, lesões (somas por equipa)
    let mut lances = [0i64; 8];
    let mut teams = 0i64;

    for s in 0..seasons {
        let t = Instant::now();
        let new = NewGame {
            save_name: format!("sim {s}"),
            first_name: "Treinador".into(),
            last_name: "Sim".into(),
            birth_date: None,
            nation_id: None,
            club_id,
        };
        let mut game = Game::create(&world, &dir, &new, seed + s).expect("não criou o jogo");
        let created = t.elapsed();

        let t = Instant::now();
        let mut played_days = 0;
        match days {
            Some(n) => {
                for _ in 0..n {
                    game.advance_day().expect("falhou ao avançar o dia");
                }
                played_days = n;
            }
            None => {
                while game.fixtures(SERIE_A).iter().any(|f| f.home_goals.is_none()) {
                    game.advance_day().expect("falhou ao avançar o dia");
                    played_days += 1;
                }
            }
        }
        let elapsed = t.elapsed();

        for f in game.fixtures(SERIE_A) {
            let (Some(h), Some(a)) = (f.home_goals, f.away_goals) else { continue };
            goals += h + a;
            games += 1;
            home_wins += (h > a) as i32;
            draws += (h == a) as i32;
        }
        for r in game.records() {
            for (st, subs) in r.stats.iter().zip(r.subs) {
                for (k, v) in [st.shots, st.on_target, st.corners, st.fouls, st.yellows, st.reds, subs].into_iter().enumerate() {
                    lances[k] += v as i64;
                }
                teams += 1;
            }
            lances[7] += r.injuries as i64;
        }
        let table = game.table(SERIE_A);
        for r in &table {
            let e = totals.entry(r.club.id).or_insert((r.club.short_name.clone(), 0, 0, 0));
            e.1 += r.points;
            e.2 += r.position;
            e.3 += (r.position == 1) as i32;
        }

        if seasons == 1 {
            let info = game.info().unwrap();
            println!("jogo criado em {created:?}; {played_days} dias em {elapsed:?}, data {}", info.date);
            print_table(&table);
            let news = game.news();
            println!("\n{} notícias; a última: {}", news.len(), news.first().map_or("-", |n| n.title.as_str()));
        } else {
            let champion = table.first().map_or("-", |r| r.club.short_name.as_str());
            println!("seed {:>4}: campeão {champion:<22} ({played_days} dias em {elapsed:?})", seed + s);
        }
        drop(game);
        let _ = std::fs::remove_dir_all(&dir);
    }

    if games > 0 {
        let g = games as f64;
        println!(
            "\n{games} jogos: {:.2} golos por jogo, vitórias em casa {:.0}%, empates {:.0}%, vitórias fora {:.0}%",
            goals as f64 / g,
            100.0 * home_wins as f64 / g,
            100.0 * draws as f64 / g,
            100.0 * (games - home_wins - draws) as f64 / g
        );
    }
    if teams > 0 {
        let t = teams as f64;
        let per = |k: usize| lances[k] as f64 / t;
        println!(
            "por equipa e jogo: {:.1} remates ({:.1} à baliza), {:.1} cantos, {:.1} faltas, {:.2} amarelos, {:.3} vermelhos, {:.1} substituições, {:.2} lesões",
            per(0), per(1), per(2), per(3), per(4), per(5), per(6), per(7)
        );
    }
    if seasons > 1 {
        let mut rows: Vec<_> = totals.into_values().collect();
        rows.sort_by_key(|r| r.2);
        println!("\nMédia em {seasons} épocas:\n{:<24} {:>6} {:>6} {:>7}", "Clube", "Pos", "P", "Títulos");
        for (name, pts, pos, titles) in rows {
            let n = seasons as f64;
            println!("{name:<24} {:>6.1} {:>6.1} {titles:>7}", pos as f64 / n, pts as f64 / n);
        }
    }
}

fn print_table(table: &[StandingRow]) {
    println!("\n{:>3}  {:<24} {:>3} {:>3} {:>3} {:>3} {:>3} {:>3} {:>4}", "", "Clube", "J", "V", "E", "D", "M", "S", "P");
    for r in table {
        println!(
            "{:>2}º  {:<24} {:>3} {:>3} {:>3} {:>3} {:>3} {:>3} {:>4}",
            r.position, r.club.short_name, r.played, r.won, r.drawn, r.lost, r.goals_for, r.goals_against, r.points
        );
    }
}
