//! Ponte HTTP só para desenvolvimento: expõe os mesmos comandos do Tauri em
//! `POST /invoke/<comando>` (corpo = argumentos em JSON), para abrir a UI no
//! navegador com `npm run dev` (o Vite repassa `/invoke` para cá).
//!
//! `cargo run -p nextcm-core --example dev_bridge` (porta 1421)

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

use nextcm_core::{SearchQuery, World};
use serde_json::Value;

fn call(world: &World, cmd: &str, args: &Value) -> Result<Value, String> {
    let id = || args["id"].as_i64().map(|v| v as i32).ok_or("falta id".to_string());
    let json = |r: Result<Value, serde_json::Error>| r.map_err(|e| e.to_string());
    let e = |e: nextcm_core::Error| e.to_string();
    match cmd {
        "search_people" => {
            let q: SearchQuery = serde_json::from_value(args["query"].clone()).map_err(|e| e.to_string())?;
            json(serde_json::to_value(world.search_people(&q).map_err(e)?))
        }
        "list_nations" => json(serde_json::to_value(world.list_nations().map_err(e)?)),
        "get_person" => json(serde_json::to_value(world.get_person(id()?).map_err(e)?)),
        "get_club" => json(serde_json::to_value(world.get_club(id()?).map_err(e)?)),
        "list_continents" => json(serde_json::to_value(world.list_continents().map_err(e)?)),
        "list_competitions" => json(serde_json::to_value(world.list_competitions().map_err(e)?)),
        "get_competition" => json(serde_json::to_value(world.get_competition(id()?).map_err(e)?)),
        "list_nations_full" => json(serde_json::to_value(world.list_nations_full().map_err(e)?)),
        "get_nation" => json(serde_json::to_value(world.get_nation(id()?).map_err(e)?)),
        _ => Err(format!("comando desconhecido: {cmd}")),
    }
}

fn main() {
    let path = World::locate(&std::env::current_dir().unwrap()).expect("data/world.sqlite não encontrado");
    let world = World::open(&path).expect("não abriu o world.sqlite");
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
        let res = match call(&world, &cmd, &args) {
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
        if let Some((k, v)) = line.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                len = v.trim().parse().unwrap_or(0);
            }
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
    let _ = write!(
        s,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
        out.len()
    );
}
