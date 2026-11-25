//! nextcm-core: dados e regras do NextCM, sem dependência do Tauri.
//!
//! Fase 2: leitura do `world.sqlite` gerado por `scripts/extract.py`.
//! Fase 3: jogo em curso (save, calendário, match engine).

pub mod attributes;
mod club;
mod competition;
pub mod date;
mod editor;
mod editor_search;
mod editor_staff;
mod engine;
pub mod finance;
mod game;
mod league;
mod live;
mod nation;
pub mod nation_names;
mod person;
mod personality;
pub mod progress;
pub mod regulation;
mod search;
mod session;
pub mod squad;
mod tactic_file;
mod tactics;
mod willingness;
mod world;

pub use club::{ClubProfile, CompetitionRef, Honour, SquadPlayer, StaffMember};
pub use competition::{ClubListItem, CompetitionListItem, CompetitionProfile, ContinentRef, HistoryEntry, TableZone};
pub use editor::player as editor_player;
pub use editor::{EditorAttribute, EditorClub, EditorNation, EditorPlayer, EditorProfile, EditorProfileView, EditorValue};
pub use editor::{EditorChanges, save_changes as editor_save};
pub use editor::{EditorCommand, command as editor_command, commands as editor_commands, export_csv as editor_export_csv};
pub use editor::{club as editor_club, profile as editor_profile};
pub use editor_search::{EditorFilter, EditorHit, EditorOptions, options as editor_options, search as editor_search};
pub use editor_search::{search_clubs as editor_search_clubs, squad as editor_squad};
pub use editor_staff::{EditorJob, EditorStaff, EditorStaffData};
pub use editor_staff::{search as editor_search_staff, staff as editor_staff};
pub use engine::{TeamStrength, expected_goals};
pub use finance::{ClubFinances, FinanceRow};
pub use game::ContractExtras;
pub use game::ExchangeCandidate;
pub use game::LineupCheck;
pub use game::MatchSheet;
pub use game::SquadRoleView;
pub use game::StatusBar;
pub use game::WorldTransfers;
pub use game::{AreaStrength, AwardLine, CoachAreas, TrainingPlan, TrainingView};
pub use game::{
    BoardConfidence, BoardRequest, BoardView, ContractReply, ContractTerms, DisciplineOptions, DivisionStats, Game, GameInfo, GroupTable,
    JobNation, JobRow, JobsView, KoRoundView, MatchPreview, MatchRecord, NewGame, NewsItem, NumberAssign, NumberEntry, OfferAction,
    OfferView, ReportStaff, RoleOption, SaveSummary, ScoutFind, ScoutList, SquadNumbers, StageView, StartSeason, TieView, TransferItem,
    TransferView, delete_save, list_saves,
};
pub use game::{CareerClub, LastLineup, ManagerStats, SquadAttributes, SquadMore};
pub use game::{ClubMoves, MoveRow};
pub use game::{ClubPreview, club_preview};
pub use game::{CompetitionStats, PlayerStatRow, RecentXg, TeamStatRow};
pub use game::{DealTerms, LoanTerms};
pub use game::{FriendlyClub, FriendlyDay, FriendlyGame, FriendlyInvite, TourNation};
pub use game::{HolidayGame, HolidayPlan, HolidaySales, HolidayView};
pub use game::{HonourAward, HonourRow, HonoursBoard, ManagerRow, ManagersTable};
pub use game::{KeyMan, MatchAnalysis, MatchReview, Milestone};
pub use game::{LeagueChoice, LeagueMode, LeagueNation, league_nations};
pub use game::{NewsCard, NewsLink, NewsSubRow, NewsSubs};
pub use game::{PressQuestion, PressView};
pub use game::{ReserveLineup, ReserveSpot};
pub use game::{StaffHireView, StaffJobOption};
pub use game::{TalkLine, TalkOption, TalkReaction, TalkView};
pub use league::{
    CHAMPIONS_LEAGUE, COPA_DO_BRASIL, EUROPA_LEAGUE, Fixture, LIBERTADORES, PREMIER_LEAGUE, SERIE_A, SERIE_B, SERIE_C, SERIE_D,
    SUL_AMERICANA, StandingRow,
};
pub use live::{LivePlayer, LiveTeam, LiveView, MAX_SUBS, MatchEvent, ScoreLine, TeamStats};
pub use nation::{NationListItem, NationProfile};
pub use person::{Attributes, CareerEntry, ClubRef, Knowledge, PersonProfile, PlayerDetails, SeasonLine, SquadMove, StaffDetails};
pub use person::{CareerMark, RecordLine};
pub use search::{
    AttributeMin, FindResult, MarketStatus, PersonKind, PersonSummary, Position, PositionTag, SearchQuery, SearchResult, SearchSort, Side,
};
pub use session::{ContinueStep, Session};
pub use squad::PlayerStatus;
pub use tactic_file::{SavedFile, SavedTactic};
pub use tactics::{
    ClubTactics, FORMATIONS, Instructions, Marking, Mentality, PassTo, Passing, PlayerOrder, PlayerOrders, Selection, SetPiece, Tackling,
    TacticPlayer, TacticSlot, Takers,
};
pub use world::{NationOption, World};

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    NotFound(String),
    Io(std::io::Error),
    /// Pedido inválido (ex.: nome de save repetido), com a mensagem para o utilizador.
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Sqlite(e) => write!(f, "erro no banco: {e}"),
            Error::NotFound(what) => write!(f, "não encontrado: {what}"),
            Error::Io(e) => write!(f, "erro de arquivo: {e}"),
            Error::Invalid(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Sqlite(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Uma pasta no %TEMP% para os testes e a CLI que se apaga ao sair, também quando um teste falha
/// (cada cópia do world tem ~86 MB e as que ficavam enchiam o disco). Com um jogo, declare a pasta
/// antes dele: as variáveis largam-se ao contrário, e no Windows não se apaga um save aberto.
#[doc(hidden)]
pub struct TempDir(std::path::PathBuf);

impl TempDir {
    /// A pasta `name` em `%TEMP%/nextcm-tests`, vazia. Um Ctrl+C não chama o `Drop`: uma vez por processo
    /// apagam-se as pastas com mais de um dia (as de outro processo a correr agora mexem-se e ficam).
    pub fn new(name: String) -> TempDir {
        static SWEEP: std::sync::Once = std::sync::Once::new();
        let root = std::env::temp_dir().join("nextcm-tests");
        SWEEP.call_once(|| {
            let day = std::time::Duration::from_secs(24 * 3600);
            for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
                let old = entry.metadata().and_then(|m| m.modified()).is_ok_and(|t| t.elapsed().is_ok_and(|age| age > day));
                if old {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        });
        let path = root.join(name);
        let _ = std::fs::remove_dir_all(&path);
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl std::ops::Deref for TempDir {
    type Target = std::path::PathBuf;
    fn deref(&self) -> &std::path::PathBuf {
        &self.0
    }
}

impl AsRef<std::path::Path> for TempDir {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}
