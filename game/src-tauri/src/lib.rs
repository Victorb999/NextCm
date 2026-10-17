use std::path::PathBuf;
use std::sync::Mutex;

use nextcm_core::{NationOption, SearchQuery, SearchResult, World};
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
    .invoke_handler(tauri::generate_handler![search_people, list_nations])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
