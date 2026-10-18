use std::path::PathBuf;
use std::sync::Mutex;

use nextcm_core::{
    ClubProfile, ClubTactics, CompetitionListItem, CompetitionProfile, ContinentRef, NationListItem, NationOption, NationProfile,
    PersonProfile, SearchQuery, SearchResult, World};
use tauri::{Manager, State};

/// Banco do mundo, aberto uma vez no início. `rusqlite::Connection` não é `Sync`.
struct AppState {
    world: Mutex<World>,
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
    let world = state.world.lock().map_err(|e| e.to_string())?;
    world.search_people(&query).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_nations(state: State<'_, AppState>) -> Result<Vec<NationOption>, String> {
    let world = state.world.lock().map_err(|e| e.to_string())?;
    world.list_nations().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_person(state: State<'_, AppState>, id: i32) -> Result<PersonProfile, String> {
    let world = state.world.lock().map_err(|e| e.to_string())?;
    world.get_person(id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_club(state: State<'_, AppState>, id: i32) -> Result<ClubProfile, String> {
    let world = state.world.lock().map_err(|e| e.to_string())?;
    world.get_club(id).map_err(|e| e.to_string())
}

/// Gera um comando Tauri que só delega para um método de `World`.
macro_rules! world_command {
    ($name:ident ( $($arg:ident : $ty:ty),* ) -> $out:ty) => {
        #[tauri::command]
        async fn $name(state: State<'_, AppState>, $($arg: $ty),*) -> Result<$out, String> {
            let world = state.world.lock().map_err(|e| e.to_string())?;
            world.$name($($arg),*).map_err(|e| e.to_string())
        }
    };
}

world_command!(list_continents() -> Vec<ContinentRef>);
world_command!(list_competitions() -> Vec<CompetitionListItem>);
world_command!(get_competition(id: i32) -> CompetitionProfile);
world_command!(list_nations_full() -> Vec<NationListItem>);
world_command!(get_nation(id: i32) -> NationProfile);
world_command!(get_club_tactics(id: i32) -> ClubTactics);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      let path = world_path().ok_or(
        "world.sqlite não encontrado. Rode `python scripts/extract.py` ou defina NEXTCM_DB.",
      )?;
      log::info!("abrindo {}", path.display());
      let world = World::open(&path)?;
      app.manage(AppState { world: Mutex::new(world) });
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
    ])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
