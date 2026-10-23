use std::path::PathBuf;
use std::sync::Mutex;

use nextcm_core::{
    ClubFinances, ClubProfile, ClubTactics, CompetitionListItem, CompetitionProfile, ContinentRef, ContractReply, ContractTerms, Fixture,
    GameInfo, Instructions, LiveView, MatchPreview, NationListItem, NationOption, NationProfile, NewGame, NewsItem, OfferView,
    PersonProfile, SaveSummary, SearchQuery, SearchResult, Selection, Session, StageView, StandingRow, TransferItem, TransferView, World,
};
use tauri::{Manager, State};

/// Mundo base e jogo aberto. `rusqlite::Connection` não é `Sync`, daí o Mutex.
struct AppState {
    session: Mutex<Session>,
}

/// Onde está o world.sqlite: variável NEXTCM_DB, senão `data/world.sqlite`
/// subindo a partir do executável ou da pasta atual.
fn world_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("NEXTCM_DB") {
        return Some(PathBuf::from(p));
    }
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    World::locate(&exe_dir).or_else(|| World::locate(&std::env::current_dir().ok()?))
}

// Comandos `async` rodam fora da thread da janela, então a interface nunca trava.
#[tauri::command]
async fn search_people(state: State<'_, AppState>, query: SearchQuery) -> Result<SearchResult, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    session.db().search_people(&query).map_err(|e| e.to_string())
}

/// Gera um comando Tauri que só delega para um método de `World` (no banco do jogo aberto).
macro_rules! world_command {
    ($name:ident ( $($arg:ident : $ty:ty),* ) -> $out:ty) => {
        #[tauri::command]
        async fn $name(state: State<'_, AppState>, $($arg: $ty),*) -> Result<$out, String> {
            let session = state.session.lock().map_err(|e| e.to_string())?;
            session.db().$name($($arg),*).map_err(|e| e.to_string())
        }
    };
}

world_command!(list_nations() -> Vec<NationOption>);
world_command!(list_continents() -> Vec<ContinentRef>);
world_command!(list_competitions() -> Vec<CompetitionListItem>);
world_command!(get_competition(id: i32) -> CompetitionProfile);
world_command!(list_nations_full() -> Vec<NationListItem>);
world_command!(get_nation(id: i32) -> NationProfile);

/// Gera um comando Tauri que delega para um método de `Session` (jogo em curso).
macro_rules! session_command {
    ($name:ident ( $($arg:ident : $ty:ty),* ) -> $out:ty) => {
        #[tauri::command]
        async fn $name(state: State<'_, AppState>, $($arg: $ty),*) -> Result<$out, String> {
            #[allow(unused_mut)]
            let mut session = state.session.lock().map_err(|e| e.to_string())?;
            session.$name($($arg),*).map_err(|e| e.to_string())
        }
    };
}

session_command!(get_person(id: i32) -> PersonProfile);
session_command!(get_club(id: i32) -> ClubProfile);
session_command!(get_club_tactics(id: i32) -> ClubTactics);
session_command!(game_info() -> Option<GameInfo>);
session_command!(match_preview(id: i32) -> MatchPreview);
session_command!(list_saves() -> Vec<SaveSummary>);
session_command!(save_game() -> GameInfo);
session_command!(advance_day() -> GameInfo);
session_command!(list_news() -> Vec<NewsItem>);
session_command!(read_news(id: i32) -> GameInfo);
session_command!(continue_game() -> GameInfo);
session_command!(league_table(id: i32) -> Vec<StandingRow>);
session_command!(list_fixtures(id: i32) -> Vec<Fixture>);
session_command!(competition_stages(id: i32) -> Vec<StageView>);
session_command!(club_finances(id: i32) -> ClubFinances);
session_command!(club_transfers(id: i32) -> Vec<TransferItem>);
session_command!(transfer_view(id: i32) -> TransferView);
session_command!(make_offer(id: i32, fee: i64) -> TransferView);
session_command!(ask_loan(id: i32) -> TransferView);
session_command!(contract_terms(id: i32, role: Option<String>) -> ContractTerms);
session_command!(negotiate(id: i32, role: String, wage: i64, bonus: i64, years: i32) -> ContractReply);
session_command!(set_listing(id: i32, transfer: bool, loan: bool, asking: Option<i64>) -> TransferView);
session_command!(offer_view(id: i32) -> OfferView);
session_command!(answer_offer(id: i32, key: String, fee: Option<i64>) -> OfferView);
session_command!(club_fixtures(id: i32) -> Vec<Fixture>);
session_command!(live_match() -> LiveView);
session_command!(live_step(minute: i32) -> LiveView);
session_command!(live_substitute(out: i32, inn: i32) -> LiveView);
session_command!(live_swap(a: i32, b: i32) -> LiveView);
session_command!(live_instructions(instructions: Instructions) -> LiveView);
session_command!(live_finish() -> GameInfo);
session_command!(live_tactics(side: i32) -> ClubTactics);

#[tauri::command]
async fn live_preview_tactics(state: State<'_, AppState>, selection: Selection) -> Result<ClubTactics, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    session.live_preview_tactics(&selection).map_err(|e| e.to_string())
}

#[tauri::command]
async fn live_set_tactics(state: State<'_, AppState>, selection: Selection) -> Result<LiveView, String> {
    let mut session = state.session.lock().map_err(|e| e.to_string())?;
    session.live_set_tactics(selection).map_err(|e| e.to_string())
}

#[tauri::command]
async fn new_game(state: State<'_, AppState>, game: NewGame) -> Result<GameInfo, String> {
    let mut session = state.session.lock().map_err(|e| e.to_string())?;
    session.new_game(&game).map_err(|e| e.to_string())
}

#[tauri::command]
async fn preview_tactics(state: State<'_, AppState>, selection: Selection) -> Result<ClubTactics, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    session.preview_tactics(&selection).map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_player(state: State<'_, AppState>, selection: Selection, player: i32) -> Result<ClubTactics, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    session.toggle_player(&selection, player).map_err(|e| e.to_string())
}

#[tauri::command]
async fn set_tactics(state: State<'_, AppState>, selection: Selection) -> Result<ClubTactics, String> {
    let mut session = state.session.lock().map_err(|e| e.to_string())?;
    session.set_tactics(selection).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_game(state: State<'_, AppState>, file: String) -> Result<GameInfo, String> {
    let mut session = state.session.lock().map_err(|e| e.to_string())?;
    session.load_game(&file).map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_save(state: State<'_, AppState>, file: String) -> Result<(), String> {
    let mut session = state.session.lock().map_err(|e| e.to_string())?;
    session.delete_save(&file).map_err(|e| e.to_string())
}

#[tauri::command]
async fn close_game(state: State<'_, AppState>) -> Result<(), String> {
    state.session.lock().map_err(|e| e.to_string())?.close_game();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
            }
            let path = world_path().ok_or("world.sqlite não encontrado. Rode `python scripts/extract.py` ou defina NEXTCM_DB.")?;
            let saves = app.path().app_data_dir()?.join("saves");
            log::info!("abrindo {} (saves em {})", path.display(), saves.display());
            let session = Session::open(&path, &saves)?;
            app.manage(AppState { session: Mutex::new(session) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search_people,
            list_nations,
            get_person,
            get_club,
            get_club_tactics,
            list_continents,
            list_competitions,
            get_competition,
            list_nations_full,
            get_nation,
            game_info,
            list_saves,
            new_game,
            load_game,
            save_game,
            close_game,
            advance_day,
            list_news,
            read_news,
            continue_game,
            league_table,
            list_fixtures,
            competition_stages,
            club_finances,
            club_transfers,
            transfer_view,
            make_offer,
            ask_loan,
            contract_terms,
            negotiate,
            set_listing,
            offer_view,
            answer_offer,
            club_fixtures,
            preview_tactics,
            set_tactics,
            toggle_player,
            delete_save,
            match_preview,
            live_match,
            live_step,
            live_substitute,
            live_swap,
            live_instructions,
            live_finish,
            live_tactics,
            live_preview_tactics,
            live_set_tactics,
        ])
        .run(tauri::generate_context!())
        .expect("error while building tauri application");
}
