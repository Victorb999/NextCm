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

use nextcm_core::{Game, NewGame, PREMIER_LEAGUE, SERIE_A, SERIE_B, SERIE_C, StandingRow, World, regulation};

/// `sim -- <clube> carreira <seed> <épocas>`: um só jogo por várias épocas seguidas (marco 3.5).
fn career(world: &Path, club_id: i32, seed: i64, seasons: i64) {
    let dir = std::env::temp_dir().join(format!("nextcm-carreira-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let new = NewGame {
        save_name: "carreira".into(),
        first_name: "Treinador".into(),
        last_name: "Sim".into(),
        birth_date: None,
        nation_id: None,
        club_id,
    };
    let mut game = Game::create(world, &dir, &new, seed).expect("não criou o jogo");
    let divisions = [(SERIE_A, "Série A"), (SERIE_B, "Série B"), (SERIE_C, "Série C"), (PREMIER_LEAGUE, "Premier League")];
    let print_stats = |game: &Game| {
        for (div, name) in divisions {
            let s = game.division_stats(div).expect("estatísticas");
            println!(
                "  {name}: {:>2} clubes, {:>4} jogadores, CA médio {:>5.1} (18 melhores {:>5.1}), idade {:>4.1}, plantéis {}–{}, saldo médio €{:.1}M ({} negativos)",
                s.clubs,
                s.players,
                s.avg_ca,
                s.avg_ca_top,
                s.avg_age,
                s.min_squad,
                s.max_squad,
                s.avg_cash / 1e6,
                s.in_debt
            );
        }
    };
    println!("Início ({}):", game.date());
    print_stats(&game);
    let (mut last_id, mut last_retired, _) = game.career_counts().expect("totais");
    let start = Instant::now();
    for _ in 0..seasons {
        let t = Instant::now();
        let year = game.season();
        let before = game.news().len();
        // Até 31/12, para ver os jogos da época antes de a seguinte os esconder
        let last_day = format!("{year}-12-31");
        while game.date() < last_day.as_str() {
            game.advance_day().expect("falhou ao avançar o dia");
        }
        let mut goals_report = Vec::new();
        for r in regulation::all() {
            let fx: Vec<_> = game.fixtures(r.competition).into_iter().filter(|f| f.home_goals.is_some()).collect();
            if fx.is_empty() {
                continue;
            }
            let goals: i32 = fx.iter().map(|f| f.home_goals.unwrap_or(0) + f.away_goals.unwrap_or(0)).sum();
            let big = fx.iter().filter(|f| (f.home_goals.unwrap_or(0) - f.away_goals.unwrap_or(0)).abs() >= 6).count();
            goals_report.push(format!("{} {:.2} ({big} com 6+)", fx[0].competition, goals as f64 / fx.len() as f64));
        }
        let top: Vec<String> = game.table(SERIE_A).iter().take(6).map(|r| format!("{} {}", r.club.short_name, r.points)).collect();
        game.advance_day().expect("falhou ao avançar o dia");
        println!("\nÉpoca {year} ({:?}):", t.elapsed());
        println!("  golos por jogo: {}", goals_report.join(", "));
        let (sold, free, loans, released, fees) =
            game.market_totals(&format!("{year}-01-01"), &format!("{}-01-01", year + 1)).expect("mercado");
        println!("  mercado: {sold} transferências (€{:.0}M), {free} livres, {loans} empréstimos, {released} dispensas", fees as f64 / 1e6);
        println!("  Série A: {}", top.join(", "));
        // As notícias da época: campeões e o resumo do fim de época
        let fresh: Vec<_> = game.news()[..game.news().len() - before].iter().rev().collect();
        for n in &fresh {
            if n.title.contains(" campeão do ") || n.title.contains(" campeão da ") {
                println!("  {}", n.title);
            }
            if n.title.starts_with("Fim da época") {
                for line in n.body.lines().filter(|l| l.starts_with("Sobem") || l.starts_with("Descem")) {
                    println!("  {line}");
                }
            }
        }
        print_stats(&game);
        let (id, retired, free) = game.career_counts().expect("totais");
        println!(
            "  jogadores novos {} (juniores e plantéis completados), reformas {}, jogadores livres {free}",
            id - last_id,
            retired - last_retired
        );
        (last_id, last_retired) = (id, retired);
    }
    println!("\n{seasons} épocas em {:?}", start.elapsed());
    drop(game);
    let _ = std::fs::remove_dir_all(&dir);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(2).map(String::as_str) == Some("carreira") {
        let world = World::locate(&std::env::current_dir().unwrap())
            .or_else(|| World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))))
            .expect("data/world.sqlite não encontrado");
        let club_id = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(7568);
        let seed = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(1);
        let seasons = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(10);
        return career(&world, club_id, seed, seasons);
    }
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
            per(0),
            per(1),
            per(2),
            per(3),
            per(4),
            per(5),
            per(6),
            per(7)
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
