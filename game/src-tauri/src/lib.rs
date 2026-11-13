use std::path::PathBuf;
use std::sync::Mutex;

use nextcm_core::ClubPreview;
use nextcm_core::ContractExtras;
use nextcm_core::FindResult;
use nextcm_core::LeagueNation;
use nextcm_core::LineupCheck;
use nextcm_core::MatchAnalysis;
use nextcm_core::MatchReview;
use nextcm_core::MatchSheet;
use nextcm_core::NewsLink;
use nextcm_core::PressView;
use nextcm_core::ReserveLineup;
use nextcm_core::SquadRoleView;
use nextcm_core::StatusBar;
use nextcm_core::WorldTransfers;
use nextcm_core::{
    BoardConfidence, BoardRequest, BoardView, ClubFinances, ClubListItem, ClubProfile, ClubTactics, CompetitionListItem,
    CompetitionProfile, ContinentRef, ContractReply, ContractTerms, DisciplineOptions, Fixture, GameInfo, Instructions, JobsView, LiveView,
    MatchPreview, NationListItem, NationOption, NationProfile, NewGame, NewsItem, NumberAssign, OfferView, PersonProfile, ReportStaff,
    SaveSummary, ScoutList, SearchQuery, SearchResult, Selection, Session, SquadNumbers, StageView, StandingRow, TransferItem,
    TransferView, World,
};
use nextcm_core::{CareerClub, ContinueStep, ManagerStats};
use nextcm_core::{DealTerms, LoanTerms};
use nextcm_core::{EditorClub, EditorHit, EditorPlayer, EditorProfile, EditorProfileView, EditorValue};
use nextcm_core::{SavedFile, TrainingPlan, TrainingView};
use nextcm_core::{TalkLine, TalkReaction, TalkView};
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
/// O passo da criação de um jogo: não usa a sessão, que o `new_game` tem ocupada.
#[tauri::command]
async fn creation_progress() -> nextcm_core::progress::CreationProgress {
    nextcm_core::progress::creation_progress()
}

#[tauri::command]
async fn squad_more(state: State<'_, AppState>, id: i32, kind: String) -> Result<Vec<nextcm_core::SquadMore>, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    session.squad_more(id, &kind).map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_people(state: State<'_, AppState>, query: SearchQuery) -> Result<SearchResult, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    session.search_people(&query).map_err(|e| e.to_string())
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
session_command!(continue_step(idle: i32) -> ContinueStep);
session_command!(league_table(id: i32) -> Vec<StandingRow>);
session_command!(list_fixtures(id: i32) -> Vec<Fixture>);
session_command!(competition_stages(id: i32) -> Vec<StageView>);
session_command!(club_finances(id: i32) -> ClubFinances);
session_command!(club_transfers(id: i32) -> Vec<TransferItem>);
session_command!(recall_loan(id: i32) -> String);
session_command!(club_moves(id: i32, view: String, season: Option<i32>) -> nextcm_core::ClubMoves);
session_command!(board_view() -> BoardView);
session_command!(board_confidence() -> BoardConfidence);
session_command!(board_requests() -> Vec<BoardRequest>);
session_command!(board_request(key: String) -> String);
session_command!(league_requests() -> Vec<BoardRequest>);
session_command!(discipline_options(id: i32) -> DisciplineOptions);
session_command!(discipline(id: i32, action: usize, reason: String) -> String);
session_command!(press_comment(id: i32, kind: String) -> String);
session_command!(set_reserve(id: i32, reserve: bool) -> String);
session_command!(reserve_lineup() -> ReserveLineup);
session_command!(set_reserve_picks(ids: Vec<i32>) -> ReserveLineup);
session_command!(give_leave(id: i32) -> String);
session_command!(jobs_view(nation: Option<i32>) -> JobsView);
session_command!(apply_job(id: i32) -> String);
session_command!(news_links(id: i32) -> Vec<NewsLink>);
session_command!(set_shortlist(id: i32, on: bool) -> ());
session_command!(league_request(key: String) -> String);
session_command!(squad_numbers() -> SquadNumbers);
session_command!(auto_squad_numbers(numbers: Vec<NumberAssign>) -> Vec<NumberAssign>);
session_command!(set_squad_numbers(numbers: Vec<NumberAssign>) -> SquadNumbers);
session_command!(training_view() -> TrainingView);
session_command!(match_sheet(id: i32) -> MatchSheet);
session_command!(set_training(plan: TrainingPlan) -> TrainingView);
session_command!(transfer_view(id: i32) -> TransferView);
session_command!(make_offer(id: i32, terms: DealTerms) -> TransferView);
session_command!(exchange_candidates(id: i32) -> Vec<i32>);
session_command!(squad_role_view(id: i32) -> SquadRoleView);
session_command!(status_bar(kind: String, id: Option<i32>) -> StatusBar);
session_command!(editor_search(file: String, query: String) -> Vec<EditorHit>);
session_command!(editor_player(file: String, id: i32) -> EditorPlayer);
session_command!(editor_export_csv(file: String, ids: Vec<i32>) -> String);
session_command!(editor_set_attributes(file: String, id: i32, values: Vec<EditorValue>) -> EditorPlayer);
session_command!(editor_search_staff(file: String, query: String) -> Vec<nextcm_core::EditorHit>);
session_command!(editor_staff(file: String, id: i32) -> nextcm_core::EditorStaff);
session_command!(
    editor_set_staff(file: String, id: i32, data: nextcm_core::EditorStaffData, values: Vec<EditorValue>) -> nextcm_core::EditorStaff
);
session_command!(editor_profile(file: String, id: i32) -> EditorProfileView);
session_command!(editor_set_profile(file: String, id: i32, profile: EditorProfile) -> EditorProfileView);
session_command!(editor_search_clubs(file: String, query: String) -> Vec<EditorHit>);
session_command!(editor_squad(file: String, club_id: i32) -> Vec<EditorHit>);
session_command!(editor_commands() -> Vec<nextcm_core::EditorCommand>);
session_command!(editor_command(file: String, ids: Vec<i32>, commands: Vec<String>, first_team: bool) -> usize);
session_command!(editor_club(file: String, id: i32) -> EditorClub);
session_command!(editor_set_club(file: String, club: EditorClub) -> EditorClub);
session_command!(set_squad_role(id: i32, role: String) -> String);
session_command!(offer_to_clubs(id: i32, loan: bool, fee: i64, exclude_rivals: bool) -> String);
session_command!(ask_loan(id: i32, terms: LoanTerms) -> TransferView);
session_command!(exercise_option(id: i32) -> TransferView);
session_command!(lineup_check() -> LineupCheck);
session_command!(press_view(id: i32) -> PressView);
session_command!(answer_press(id: i32, answers: Vec<usize>) -> String);
session_command!(skip_press(id: i32) -> String);
session_command!(set_nickname(id: i32, nickname: String) -> ());
session_command!(set_note(id: i32, text: String) -> ());
session_command!(release_question(id: i32) -> String);
session_command!(release_player(id: i32) -> String);
session_command!(day_competitions() -> Vec<i32>);
session_command!(world_transfers(month: String, page: i64, kind: Option<String>) -> WorldTransfers);
session_command!(resign() -> String);
session_command!(league_nations() -> Vec<LeagueNation>);
session_command!(club_preview(id: i32) -> ClubPreview);
session_command!(talk_view() -> TalkView);
session_command!(give_talk(key: String, tone: String, person: Option<i32>) -> Vec<TalkReaction>);
session_command!(give_talks(tone: String, lines: Vec<TalkLine>) -> Vec<TalkReaction>);
session_command!(retire_manager() -> String);
session_command!(holiday(days: i32) -> GameInfo);
session_command!(manager_career() -> Vec<CareerClub>);
session_command!(manager_stats() -> ManagerStats);
session_command!(last_lineup(id: i32) -> Option<nextcm_core::LastLineup>);
session_command!(squad_attributes(id: i32) -> Vec<nextcm_core::SquadAttributes>);
session_command!(save_as(name: String, overwrite: bool) -> GameInfo);
session_command!(ask_terms(id: i32, terms: DealTerms) -> OfferView);
session_command!(contract_terms(id: i32, role: Option<String>) -> ContractTerms);
session_command!(negotiate(id: i32, role: String, wage: i64, bonus: i64, years: i32, extras: ContractExtras) -> ContractReply);
session_command!(set_listing(id: i32, transfer: bool, loan: bool, asking: Option<i64>) -> TransferView);
session_command!(offer_view(id: i32) -> OfferView);
session_command!(answer_offer(id: i32, key: String, fee: Option<i64>) -> OfferView);
session_command!(scout_player(id: i32) -> String);
session_command!(scout_list(slot: i32) -> ScoutList);
session_command!(report_staff(kind: String) -> Vec<ReportStaff>);
session_command!(staff_report(id: i32, kind: String, staff: Vec<i32>) -> usize);
session_command!(assign_scout(slot: i32, kind: String, target: Option<i32>) -> ScoutList);
session_command!(clear_scout(slot: i32) -> ScoutList);
session_command!(search_clubs(text: String) -> Vec<ClubListItem>);
session_command!(find(text: String) -> FindResult);
session_command!(club_fixtures(id: i32) -> Vec<Fixture>);
session_command!(reserve_fixtures(id: i32) -> Vec<Fixture>);
session_command!(live_match() -> LiveView);
session_command!(match_analysis() -> MatchAnalysis);
session_command!(match_review() -> MatchReview);
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
async fn list_tactics(state: State<'_, AppState>) -> Result<Vec<SavedFile>, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    Ok(session.list_tactics())
}

#[tauri::command]
async fn list_lists(state: State<'_, AppState>) -> Result<Vec<SavedFile>, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    Ok(session.list_lists())
}

#[tauri::command]
async fn save_list(state: State<'_, AppState>, slot: i32, name: String) -> Result<SavedFile, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    session.save_list(slot, &name).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_list(state: State<'_, AppState>, slot: i32, file: String) -> Result<(), String> {
    let mut session = state.session.lock().map_err(|e| e.to_string())?;
    session.load_list(slot, &file).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_tactic(state: State<'_, AppState>, selection: Selection, name: String) -> Result<SavedFile, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    session.save_tactic(&selection, &name).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_tactic(state: State<'_, AppState>, selection: Selection, file: String) -> Result<Selection, String> {
    let session = state.session.lock().map_err(|e| e.to_string())?;
    session.load_tactic(&selection, &file).map_err(|e| e.to_string())
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
            // Também em release: o ficheiro vai para a pasta de logs da app (os relatos do beta)
            app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
            log::info!("NextCM {}", app.package_info().version);
            // No instalador, a base vem como recurso em `data/world.sqlite`
            let bundled = app.path().resource_dir().ok().map(|d| d.join("data").join("world.sqlite")).filter(|p| p.exists());
            let path = std::env::var("NEXTCM_DB")
                .ok()
                .map(PathBuf::from)
                .or(bundled)
                .or_else(world_path)
                .ok_or("world.sqlite não encontrado. Rode `python scripts/extract.py` ou defina NEXTCM_DB.")?;
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
            creation_progress,
            continue_step,
            league_table,
            list_fixtures,
            competition_stages,
            club_finances,
            club_transfers,
            recall_loan,
            club_moves,
            board_view,
            board_confidence,
            board_requests,
            board_request,
            league_requests,
            discipline_options,
            discipline,
            press_comment,
            set_reserve,
            reserve_lineup,
            set_reserve_picks,
            give_leave,
            jobs_view,
            apply_job,
            news_links,
            set_shortlist,
            league_request,
            squad_numbers,
            auto_squad_numbers,
            set_squad_numbers,
            training_view,
            match_sheet,
            set_training,
            transfer_view,
            make_offer,
            exchange_candidates,
            offer_to_clubs,
            squad_role_view,
            status_bar,
            editor_search,
            editor_player,
            editor_export_csv,
            editor_set_attributes,
            editor_search_staff,
            editor_staff,
            editor_set_staff,
            editor_profile,
            editor_set_profile,
            editor_search_clubs,
            editor_squad,
            editor_commands,
            editor_command,
            editor_club,
            editor_set_club,
            set_squad_role,
            ask_loan,
            exercise_option,
            lineup_check,
            press_view,
            answer_press,
            skip_press,
            set_nickname,
            set_note,
            release_question,
            release_player,
            day_competitions,
            world_transfers,
            resign,
            league_nations,
            club_preview,
            talk_view,
            give_talk,
            give_talks,
            retire_manager,
            holiday,
            manager_career,
            manager_stats,
            last_lineup,
            squad_more,
            squad_attributes,
            save_as,
            ask_terms,
            contract_terms,
            negotiate,
            set_listing,
            offer_view,
            answer_offer,
            scout_player,
            scout_list,
            report_staff,
            staff_report,
            assign_scout,
            clear_scout,
            search_clubs,
            find,
            club_fixtures,
            reserve_fixtures,
            preview_tactics,
            set_tactics,
            toggle_player,
            list_tactics,
            list_lists,
            save_list,
            load_list,
            save_tactic,
            load_tactic,
            delete_save,
            match_preview,
            live_match,
            match_analysis,
            match_review,
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
