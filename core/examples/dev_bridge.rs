//! Ponte HTTP só para desenvolvimento: expõe os mesmos comandos do Tauri em
//! `POST /invoke/<comando>` (corpo = argumentos em JSON), para abrir a UI no
//! navegador com `npm run dev` (o Vite repassa `/invoke` para cá).
//!
//! `cargo run -p nextcm-core --example dev_bridge` (porta 1421)

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

use nextcm_core::{Instructions, NewGame, NumberAssign, SearchQuery, Selection, Session, World};
use serde_json::Value;

fn call(session: &mut Session, cmd: &str, args: &Value) -> Result<Value, String> {
    let id = || args["id"].as_i64().map(|v| v as i32).ok_or("falta id".to_string());
    let e = |e: nextcm_core::Error| e.to_string();
    let out = |v: Result<Value, serde_json::Error>| v.map_err(|e| e.to_string());
    let world = session.db();
    match cmd {
        "search_people" => {
            let q: SearchQuery = serde_json::from_value(args["query"].clone()).map_err(|e| e.to_string())?;
            out(serde_json::to_value(session.search_people(&q).map_err(e)?))
        }
        "list_nations" => out(serde_json::to_value(world.list_nations().map_err(e)?)),
        "get_person" => out(serde_json::to_value(session.get_person(id()?).map_err(e)?)),
        "get_club" => out(serde_json::to_value(session.get_club(id()?).map_err(e)?)),
        "get_club_tactics" => out(serde_json::to_value(session.get_club_tactics(id()?).map_err(e)?)),
        "preview_tactics" => {
            let sel: Selection = serde_json::from_value(args["selection"].clone()).map_err(|e| e.to_string())?;
            out(serde_json::to_value(session.preview_tactics(&sel).map_err(e)?))
        }
        "toggle_player" => {
            let sel: Selection = serde_json::from_value(args["selection"].clone()).map_err(|e| e.to_string())?;
            let player = args["player"].as_i64().ok_or("falta player")? as i32;
            out(serde_json::to_value(session.toggle_player(&sel, player).map_err(e)?))
        }
        "set_tactics" => {
            let sel: Selection = serde_json::from_value(args["selection"].clone()).map_err(|e| e.to_string())?;
            out(serde_json::to_value(session.set_tactics(sel).map_err(e)?))
        }
        "list_continents" => out(serde_json::to_value(world.list_continents().map_err(e)?)),
        "list_competitions" => out(serde_json::to_value(world.list_competitions().map_err(e)?)),
        "get_competition" => out(serde_json::to_value(world.get_competition(id()?).map_err(e)?)),
        "list_nations_full" => out(serde_json::to_value(world.list_nations_full().map_err(e)?)),
        "get_nation" => out(serde_json::to_value(world.get_nation(id()?).map_err(e)?)),
        "match_preview" => out(serde_json::to_value(session.match_preview(id()?).map_err(e)?)),
        "game_info" => out(serde_json::to_value(session.game_info().map_err(e)?)),
        "list_saves" => out(serde_json::to_value(session.list_saves().map_err(e)?)),
        "new_game" => {
            let g: NewGame = serde_json::from_value(args["game"].clone()).map_err(|e| e.to_string())?;
            out(serde_json::to_value(session.new_game(&g).map_err(e)?))
        }
        "load_game" => {
            let file = args["file"].as_str().ok_or("falta file")?;
            out(serde_json::to_value(session.load_game(file).map_err(e)?))
        }
        "delete_save" => {
            let file = args["file"].as_str().ok_or("falta file")?;
            session.delete_save(file).map_err(e)?;
            Ok(Value::Null)
        }
        "save_game" => out(serde_json::to_value(session.save_game().map_err(e)?)),
        "close_game" => {
            session.close_game();
            Ok(Value::Null)
        }
        "advance_day" => out(serde_json::to_value(session.advance_day().map_err(e)?)),
        "list_news" => out(serde_json::to_value(session.list_news().map_err(e)?)),
        "read_news" => out(serde_json::to_value(session.read_news(id()?).map_err(e)?)),
        "continue_game" => out(serde_json::to_value(session.continue_game().map_err(e)?)),
        "league_table" => out(serde_json::to_value(session.league_table(id()?).map_err(e)?)),
        "list_fixtures" => out(serde_json::to_value(session.list_fixtures(id()?).map_err(e)?)),
        "competition_stages" => out(serde_json::to_value(session.competition_stages(id()?).map_err(e)?)),
        "club_finances" => out(serde_json::to_value(session.club_finances(id()?).map_err(e)?)),
        "squad_numbers" => out(serde_json::to_value(session.squad_numbers().map_err(e)?)),
        "auto_squad_numbers" | "set_squad_numbers" => {
            let numbers: Vec<NumberAssign> = serde_json::from_value(args["numbers"].clone()).map_err(|e| e.to_string())?;
            if cmd == "auto_squad_numbers" {
                out(serde_json::to_value(session.auto_squad_numbers(numbers).map_err(e)?))
            } else {
                out(serde_json::to_value(session.set_squad_numbers(numbers).map_err(e)?))
            }
        }
        "board_view" => out(serde_json::to_value(session.board_view().map_err(e)?)),
        "board_confidence" => out(serde_json::to_value(session.board_confidence().map_err(e)?)),
        "board_requests" => out(serde_json::to_value(session.board_requests().map_err(e)?)),
        "league_requests" => out(serde_json::to_value(session.league_requests().map_err(e)?)),
        "discipline_options" => out(serde_json::to_value(session.discipline_options(id()?).map_err(e)?)),
        "discipline" => {
            let action = args["action"].as_u64().ok_or("falta action")? as usize;
            let reason = args["reason"].as_str().ok_or("falta reason")?.to_string();
            out(serde_json::to_value(session.discipline(id()?, action, reason).map_err(e)?))
        }
        "jobs_view" => {
            let nation = args["nation"].as_i64().map(|v| v as i32);
            out(serde_json::to_value(session.jobs_view(nation).map_err(e)?))
        }
        "apply_job" => out(serde_json::to_value(session.apply_job(id()?).map_err(e)?)),
        "set_reserve" => {
            let reserve = args["reserve"].as_bool().ok_or("falta reserve")?;
            out(serde_json::to_value(session.set_reserve(id()?, reserve).map_err(e)?))
        }
        "press_comment" => {
            let kind = args["kind"].as_str().ok_or("falta kind")?.to_string();
            out(serde_json::to_value(session.press_comment(id()?, kind).map_err(e)?))
        }
        "league_request" => {
            let key = args["key"].as_str().ok_or("falta key")?.to_string();
            out(serde_json::to_value(session.league_request(key).map_err(e)?))
        }
        "board_request" => {
            let key = args["key"].as_str().ok_or("falta key")?.to_string();
            out(serde_json::to_value(session.board_request(key).map_err(e)?))
        }
        "transfer_view" => out(serde_json::to_value(session.transfer_view(id()?).map_err(e)?)),
        "make_offer" => {
            let fee = args["fee"].as_i64().ok_or("falta fee")?;
            out(serde_json::to_value(session.make_offer(id()?, fee).map_err(e)?))
        }
        "ask_loan" => out(serde_json::to_value(session.ask_loan(id()?).map_err(e)?)),
        "contract_terms" => {
            let role = args["role"].as_str().map(str::to_string);
            out(serde_json::to_value(session.contract_terms(id()?, role).map_err(e)?))
        }
        "negotiate" => {
            let role = args["role"].as_str().ok_or("falta role")?.to_string();
            let (wage, bonus) = (args["wage"].as_i64().ok_or("falta wage")?, args["bonus"].as_i64().unwrap_or(0));
            let years = args["years"].as_i64().ok_or("falta years")? as i32;
            out(serde_json::to_value(session.negotiate(id()?, role, wage, bonus, years).map_err(e)?))
        }
        "set_listing" => {
            let (transfer, loan) = (args["transfer"].as_bool().unwrap_or(false), args["loan"].as_bool().unwrap_or(false));
            out(serde_json::to_value(session.set_listing(id()?, transfer, loan, args["asking"].as_i64()).map_err(e)?))
        }
        "offer_view" => out(serde_json::to_value(session.offer_view(id()?).map_err(e)?)),
        "scout_player" => out(serde_json::to_value(session.scout_player(id()?).map_err(e)?)),
        "report_staff" => {
            let kind = args["kind"].as_str().ok_or("falta kind")?.to_string();
            out(serde_json::to_value(session.report_staff(kind).map_err(e)?))
        }
        "staff_report" => {
            let kind = args["kind"].as_str().ok_or("falta kind")?.to_string();
            let staff: Vec<i32> = serde_json::from_value(args["staff"].clone()).map_err(|e| e.to_string())?;
            out(serde_json::to_value(session.staff_report(id()?, kind, staff).map_err(e)?))
        }
        "scout_list" => {
            let slot = args["slot"].as_i64().ok_or("falta slot")? as i32;
            out(serde_json::to_value(session.scout_list(slot).map_err(e)?))
        }
        "assign_scout" => {
            let slot = args["slot"].as_i64().ok_or("falta slot")? as i32;
            let kind = args["kind"].as_str().ok_or("falta kind")?.to_string();
            let target = args["target"].as_i64().map(|v| v as i32);
            out(serde_json::to_value(session.assign_scout(slot, kind, target).map_err(e)?))
        }
        "clear_scout" => {
            let slot = args["slot"].as_i64().ok_or("falta slot")? as i32;
            out(serde_json::to_value(session.clear_scout(slot).map_err(e)?))
        }
        "search_clubs" => {
            let text = args["text"].as_str().unwrap_or("").to_string();
            out(serde_json::to_value(session.search_clubs(text).map_err(e)?))
        }
        "answer_offer" => {
            let key = args["key"].as_str().ok_or("falta key")?.to_string();
            out(serde_json::to_value(session.answer_offer(id()?, key, args["fee"].as_i64()).map_err(e)?))
        }
        "club_transfers" => out(serde_json::to_value(session.club_transfers(id()?).map_err(e)?)),
        "club_fixtures" => out(serde_json::to_value(session.club_fixtures(id()?).map_err(e)?)),
        "live_match" => out(serde_json::to_value(session.live_match().map_err(e)?)),
        "live_step" => {
            let minute = args["minute"].as_i64().ok_or("falta minute")? as i32;
            out(serde_json::to_value(session.live_step(minute).map_err(e)?))
        }
        "live_substitute" => {
            let (o, i) = (args["out"].as_i64().ok_or("falta out")?, args["inn"].as_i64().ok_or("falta inn")?);
            out(serde_json::to_value(session.live_substitute(o as i32, i as i32).map_err(e)?))
        }
        "live_swap" => {
            let (a, b) = (args["a"].as_i64().ok_or("falta a")?, args["b"].as_i64().ok_or("falta b")?);
            out(serde_json::to_value(session.live_swap(a as i32, b as i32).map_err(e)?))
        }
        "live_instructions" => {
            let i: Instructions = serde_json::from_value(args["instructions"].clone()).map_err(|e| e.to_string())?;
            out(serde_json::to_value(session.live_instructions(i).map_err(e)?))
        }
        "live_tactics" => {
            let side = args["side"].as_i64().ok_or("falta side")? as i32;
            out(serde_json::to_value(session.live_tactics(side).map_err(e)?))
        }
        "live_preview_tactics" => {
            let sel: Selection = serde_json::from_value(args["selection"].clone()).map_err(|e| e.to_string())?;
            out(serde_json::to_value(session.live_preview_tactics(&sel).map_err(e)?))
        }
        "live_set_tactics" => {
            let sel: Selection = serde_json::from_value(args["selection"].clone()).map_err(|e| e.to_string())?;
            out(serde_json::to_value(session.live_set_tactics(sel).map_err(e)?))
        }
        "live_finish" => out(serde_json::to_value(session.live_finish().map_err(e)?)),
        _ => Err(format!("comando desconhecido: {cmd}")),
    }
}

fn main() {
    let path = World::locate(&std::env::current_dir().unwrap()).expect("data/world.sqlite não encontrado");
    // Saves de dev ficam em data/saves (fora do git, como o world.sqlite)
    let saves = path.parent().unwrap().join("saves");
    let mut session = Session::open(&path, &saves).expect("não abriu o world.sqlite");
    let listener = TcpListener::bind("127.0.0.1:1421").expect("porta 1421 ocupada");
    println!("dev_bridge em http://127.0.0.1:1421 ({})", path.display());

    // Cada conexão é lida numa thread (o navegador abre conexões ociosas que
    // travariam um loop simples); os comandos rodam aqui, pois `World` não é `Sync`.
    let (tx, rx) = mpsc::channel::<Job>();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            thread::spawn(move || handle(stream, tx));
        }
    });
    for (cmd, args, reply) in rx {
        let res = match call(&mut session, &cmd, &args) {
            Ok(v) => ("200 OK", v.to_string()),
            Err(msg) => ("500 Internal Server Error", Value::String(msg).to_string()),
        };
        let _ = reply.send(res);
    }
}

type Job = (String, Value, mpsc::Sender<(&'static str, String)>);

fn handle(stream: TcpStream, tx: mpsc::Sender<Job>) {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut len = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':')
            && k.eq_ignore_ascii_case("content-length")
        {
            len = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; len];
    let _ = reader.read_exact(&mut body);

    let path = request_line.split_whitespace().nth(1).unwrap_or("");
    let cmd = path.strip_prefix("/invoke/").unwrap_or("").to_string();
    let args: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let (reply_tx, reply_rx) = mpsc::channel();
    if tx.send((cmd, args, reply_tx)).is_err() {
        return;
    }
    let Ok((status, out)) = reply_rx.recv() else { return };
    let mut s = &stream;
    let _ =
        write!(s, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
}
