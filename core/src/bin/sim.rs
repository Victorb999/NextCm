//! CLI do NextCM para rodar o jogo sem interface (balanceamento e benchmark).
//!
//! `cargo run -p nextcm-core --release --bin sim -- [id do clube] [dias|epoca] [seed] [épocas]`
//!
//! Marco 3.2: cria um jogo temporário no clube e avança os dias pedidos ou, com `epoca`
//! (o padrão), até ao fim da Série A; imprime a classificação e os números do match
//! engine. Com mais de uma época (seeds seguidas), mostra a média de pontos e de posição.
//! Marco 3.4: também os números dos lances (remates, cantos, faltas, cartões,
//! substituições e lesões por equipa e jogo).
//! Marco 3.17: `sim -- <clube> treino <seed> [dias]` joga o mesmo ano com todos os programas
//! a Nenhum, Médio e Intensivo e compara a evolução do CA, a condição e as lesões no treino.
//! Passo 3.15c: `sim -- <clube> economia <seed> [meses]` joga desde 1/7/2025 e mostra, no fim
//! de cada mês, as finanças do Bayern, do Hoffenheim, do 1860 e do Viktoria Köln (os prints do
//! CM) e do clube escolhido; no fim, as rubricas da época de cada um.
//! Passo 3.15c (13): `sim -- <clube> fundo <seed>` joga a Premier League 2025/26 a jogar e de
//! fundo (motor rápido) com a mesma seed e compara os golos, as vitórias em casa, o tempo e a
//! classificação.
//! Marco 3.22: `sim -- <clube> carreira <seed> [épocas]` mostra também a economia de cada época
//! (transferências de €100M+, quem mais comprou, saldos e folha dos grandes, fair-play).

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use nextcm_core::{
    Game, LeagueChoice, LeagueMode, NewGame, PREMIER_LEAGUE, SERIE_A, SERIE_B, SERIE_C, StandingRow, TrainingPlan, World, regulation,
};

/// `sim -- <clube> fundo <seed>`: a Premier League a jogar e de fundo, lado a lado.
fn background(world: &Path, club_id: i32, seed: i64) {
    for mode in [LeagueMode::Play, LeagueMode::Background] {
        let dir = std::env::temp_dir().join(format!("nextcm-fundo-{}-{mode:?}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = NewGame {
            save_name: "fundo".into(),
            first_name: "Treinador".into(),
            last_name: "Sim".into(),
            birth_date: None,
            nation_id: None,
            club_id,
            // Brasil a jogar (o clube do treinador), Inglaterra como pedido
            leagues: vec![LeagueChoice { nation_id: 27, mode: LeagueMode::Play }, LeagueChoice { nation_id: 61, mode }],
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut game = Game::create(world, &dir, &new, seed).expect("não criou o jogo");
        let t = Instant::now();
        while game.date() < "2026-06-01" && !game.sacked() {
            game.advance_day().expect("falhou ao avançar o dia");
        }
        let fx: Vec<_> = game.fixtures(PREMIER_LEAGUE).into_iter().filter(|f| f.home_goals.is_some()).collect();
        let n = fx.len().max(1) as f64;
        let goals: i32 = fx.iter().map(|f| f.home_goals.unwrap_or(0) + f.away_goals.unwrap_or(0)).sum();
        let home = fx.iter().filter(|f| f.home_goals > f.away_goals).count() as f64;
        let draws = fx.iter().filter(|f| f.home_goals == f.away_goals).count() as f64;
        println!(
            "{mode:?}: {} jogos, {:.2} golos por jogo, {:.0}% vitórias em casa, {:.0}% empates, {:.1}s até 31/5/2026",
            fx.len(),
            goals as f64 / n,
            100.0 * home / n,
            100.0 * draws / n,
            t.elapsed().as_secs_f64()
        );
        for r in game.table(PREMIER_LEAGUE) {
            println!("  {:>2}. {:<24} {:>3} pts  {:>3}-{:<3}", r.position, r.club.short_name, r.points, r.goals_for, r.goals_against);
        }
        drop(game);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

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
        leagues: Vec::new(),
        start: Default::default(),
        license: 0,
        real_players: true,
        masking: true,
        experience: 0,
        favourite_club: None,
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
        // Despedido, o mundo continua (as médias e o mercado das épocas seguintes contam na mesma)
        let employed = !game.sacked();
        while game.date() < last_day.as_str() {
            game.advance_day().expect("falhou ao avançar o dia");
        }
        if employed && game.sacked() {
            println!("\nO treinador foi despedido ({}); o mundo continua.", game.date());
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
        let b = game.board_view().expect("direção");
        println!("  direção: confiança {} ({}), objetivo {} ({}º)", b.confidence, b.confidence_label, b.goal, b.target);
        println!("  golos por jogo: {}", goals_report.join(", "));
        let (sold, free, loans, released, fees) =
            game.market_totals(&format!("{year}-01-01"), &format!("{}-01-01", year + 1)).expect("mercado");
        println!("  mercado: {sold} transferências (€{:.0}M), {free} livres, {loans} empréstimos, {released} dispensas", fees as f64 / 1e6);
        println!("  Série A: {}", top.join(", "));
        for line in game.economy_report(&format!("{year}-01-01"), &format!("{}-01-01", year + 1)).expect("economia") {
            println!("  {line}");
        }
        // As notícias da época: campeões e o resumo do fim de época
        let fresh: Vec<_> = game.news()[..game.news().len() - before].iter().rev().collect();
        for n in &fresh {
            if n.title.contains(" campeão do ") || n.title.contains(" campeão da ") {
                println!("  {}", n.title);
            }
            if n.title.starts_with("Fim da época") {
                for line in n.body.lines().filter(|l| l.starts_with("Sobem") || l.starts_with("Descem") || l.starts_with("Falência")) {
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

/// `sim -- <clube> treino <seed> [dias]`: o mesmo jogo com os programas todos a uma intensidade.
fn training(world: &Path, club_id: i32, seed: i64, days: u32) {
    for (level, label) in [(0, "Nenhum"), (2, "Médio"), (3, "Intensivo")] {
        let dir = std::env::temp_dir().join(format!("nextcm-treino-{}-{level}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let new = NewGame {
            save_name: "treino".into(),
            first_name: "Treinador".into(),
            last_name: "Sim".into(),
            birth_date: None,
            nation_id: None,
            club_id,
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
        };
        let mut game = Game::create(world, &dir, &new, seed).expect("não criou o jogo");
        let view = game.training_view().expect("treino");
        let programs = view
            .programs
            .into_iter()
            .map(|mut p| {
                (p.fitness, p.tactics, p.shooting, p.technique) = (level, level, level, level);
                p.goalkeeping = if p.name == "GK" { level } else { 0 };
                p
            })
            .collect();
        game.set_training(&TrainingPlan { programs, players: Vec::new(), coaches: None }).expect("programas");
        let ca = |game: &Game| -> HashMap<i32, i32> {
            let club = game.world().get_club(club_id).expect("clube");
            club.players.iter().map(|p| (p.id, p.current_ability.unwrap_or(0))).collect()
        };
        // Soma dos atributos físicos e técnicos do banco (a evolução por área)
        let attrs = |game: &Game| -> HashMap<i32, i32> {
            let keys = ["pace", "acceleration", "stamina", "strength", "technique", "dribbling", "passing", "finishing", "positioning"];
            let club = game.world().get_club(club_id).expect("clube");
            club.players
                .iter()
                .map(|p| {
                    let raw = game.world().get_person(p.id).expect("pessoa").player.map(|pl| pl.raw_attributes).unwrap_or_default();
                    (p.id, keys.iter().map(|k| raw.get(*k).copied().flatten().unwrap_or(0)).sum())
                })
                .collect()
        };
        let before = ca(&game);
        let attrs_before = attrs(&game);
        let news = game.news().len();
        let (mut condition, mut counted) = (0i64, 0i64);
        for _ in 0..days {
            game.advance_day().expect("falhou ao avançar o dia");
            if let Ok(t) = game.club_tactics(club_id) {
                for s in t.starters.iter().filter_map(|s| s.player.as_ref()) {
                    condition += s.condition.unwrap_or(100) as i64;
                    counted += 1;
                }
            }
        }
        let after = ca(&game);
        let attrs_after = attrs(&game);
        let kept: Vec<i32> = before.keys().filter(|id| after.contains_key(id)).copied().collect();
        let young: Vec<i32> = kept.iter().filter(|id| before[id] < 120).copied().collect();
        let avg = |ids: &[i32]| ids.iter().map(|id| (after[id] - before[id]) as f64).sum::<f64>() / ids.len().max(1) as f64;
        let attr_avg =
            |ids: &[i32]| ids.iter().map(|id| (attrs_after[id] - attrs_before[id]) as f64).sum::<f64>() / ids.len().max(1) as f64;
        let veterans: Vec<i32> = kept.iter().filter(|id| before[id] >= 120).copied().collect();
        let injuries = game.news()[..game.news().len() - news].iter().filter(|n| n.title.ends_with("lesionado no treino")).count();
        println!(
            "{label:<10} CA {:+.1} em média ({} jogadores; {:+.1} nos {} com CA < 120), atributos {:+.1} ({:+.1} nos de CA < 120, {:+.1} nos outros), condição do onze {:.0}%, {injuries} lesões no treino",
            avg(&kept),
            kept.len(),
            avg(&young),
            young.len(),
            attr_avg(&kept),
            attr_avg(&young),
            attr_avg(&veterans),
            condition as f64 / counted.max(1) as f64
        );
        drop(game);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// `sim -- <clube> economia <seed> [meses]`: as finanças mês a mês, para comparar com o CM.
fn economy(world: &Path, club_id: i32, seed: i64, months: i64) {
    let dir = std::env::temp_dir().join(format!("nextcm-economia-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let new = NewGame {
        save_name: "economia".into(),
        first_name: "Treinador".into(),
        last_name: "Sim".into(),
        birth_date: None,
        nation_id: None,
        club_id,
        leagues: Vec::new(),
        start: Default::default(),
        license: 0,
        real_players: true,
        masking: true,
        experience: 0,
        favourite_club: None,
    };
    let mut game = Game::create(world, &dir, &new, seed).expect("não criou o jogo");
    let mut clubs = vec![1350, 4590, 10, 10929];
    if !clubs.contains(&club_id) {
        clubs.push(club_id);
    }
    let m = |v: i64| format!("{:>8.2}", v as f64 / 1e6);
    let name = |c: i32| match c {
        1350 => "Bayern".to_string(),
        4590 => "Hoffenheim".to_string(),
        10 => "1860 München".to_string(),
        10929 => "Viktoria Köln".to_string(),
        _ => format!("clube {c}"),
    };
    let show = |game: &Game, title: &str| {
        println!(
            "
{title} ({}): clube | saldo | fundos | mês passado rec/desp | época rec/desp (€M)",
            game.date()
        );
        for &c in &clubs {
            let f = game.club_finances(c).expect("finanças");
            println!(
                "  {:<14} {} {} | {} {} | {} {}",
                name(c),
                m(f.cash),
                m(f.funds),
                m(f.total_income.amounts[1]),
                m(f.total_expenses.amounts[1]),
                m(f.total_income.amounts[2]),
                m(f.total_expenses.amounts[2])
            );
        }
    };
    show(&game, "Início");
    let mut last_season = Vec::new();
    for _ in 0..months {
        let month = game.date()[..7].to_string();
        while game.date().starts_with(month.as_str()) && !game.sacked() {
            // Antes de a época mudar (1/7), guarda as rubricas da que acaba
            if game.date().ends_with("-06-30") {
                last_season = clubs.iter().map(|&c| game.club_finances(c).expect("finanças")).collect();
            }
            game.advance_day().expect("falhou ao avançar o dia");
        }
        show(&game, &format!("Fim de {month}"));
    }
    let books =
        if last_season.is_empty() { clubs.iter().map(|&c| game.club_finances(c).expect("finanças")).collect() } else { last_season };
    for f in books {
        println!(
            "
{} — época {} (€M)",
            name(f.club_id),
            f.season
        );
        for l in f.income.iter().chain(&f.expenses).chain([&f.total_income, &f.total_expenses, &f.result]) {
            if l.amounts[2] != 0 {
                println!("  {:<22} {}", l.label, m(l.amounts[2]));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(2).map(String::as_str) == Some("economia") {
        let world = World::locate(&std::env::current_dir().unwrap())
            .or_else(|| World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))))
            .expect("data/world.sqlite não encontrado");
        let club_id = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(1350);
        let seed = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(1);
        let months = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(12);
        return economy(&world, club_id, seed, months);
    }
    if args.get(2).map(String::as_str) == Some("fundo") {
        let world = World::locate(&std::env::current_dir().unwrap())
            .or_else(|| World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))))
            .expect("data/world.sqlite não encontrado");
        let club_id = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(7568);
        let seed = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(1);
        return background(&world, club_id, seed);
    }
    if args.get(2).map(String::as_str) == Some("treino") {
        let world = World::locate(&std::env::current_dir().unwrap())
            .or_else(|| World::locate(Path::new(env!("CARGO_MANIFEST_DIR"))))
            .expect("data/world.sqlite não encontrado");
        let club_id = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(7568);
        let seed = args.get(3).and_then(|a| a.parse().ok()).unwrap_or(1);
        let days = args.get(4).and_then(|a| a.parse().ok()).unwrap_or(365);
        return training(&world, club_id, seed, days);
    }
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
    let mut xg_total = 0.0f64;
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
            leagues: Vec::new(),
            start: Default::default(),
            license: 0,
            real_players: true,
            masking: true,
            experience: 0,
            favourite_club: None,
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
                xg_total += st.xg;
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
        // Os golos esperados (marco 4.3): por equipa e jogo, a bater com os golos
        println!("golos esperados por equipa e jogo: {:.2}", xg_total / t);
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
