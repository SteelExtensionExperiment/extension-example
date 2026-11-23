//! Game orchestrator contract.
//!
//! The orchestrator owns game types and kits. It listens to arena-manager's
//! `MatchFinished` action and re-emits [`MatchResult`] so consumers such as stats-visualizer link
//! only this crate. Emit only after releasing every lock; dispatch is synchronous.
#![recursion_limit = "256"]

use steel_extension_api::{
    Server,
    hooks::Action,
    players::Uuid,
    services,
    stabby::{self, string::String as ApiString, vec::Vec as ApiVec},
};

pub use arena_manager_api::Outcome;

/// Service key of the game contract.
pub const GAMES: &str = "game-orchestrator:games/v1";

/// One participant of a finished match.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct Participant {
    /// Profile UUID.
    pub uuid: Uuid,
    /// Latest known name.
    pub name: ApiString,
    /// Result for this player.
    pub outcome: Outcome,
}

/// Emitted once per finished match, with every participant once. Never emitted for failed matches.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Game type name, stable across restarts.
    pub game_type: ApiString,
    /// Every participant.
    pub participants: ApiVec<Participant>,
}

impl Action for MatchResult {
    const TAG: &'static str = "game-orchestrator:match_result/v1";
}

/// Game type provider. Calls run on the server tick thread and never block.
#[stabby::stabby(checked)]
pub trait Games: Send + Sync {
    /// Names of every game type, enabled or not, as saved by the orchestrator.
    extern "C" fn game_types(&self, server: Server<'_>) -> ApiVec<ApiString>;
}

/// Owning, checked game provider object.
pub type DynGames = stabby::dynptr!(stabby::boxed::Box<dyn Games + Send + Sync>);

/// The active game provider.
///
/// # Errors
/// Returns an error when no compatible provider is registered.
pub fn games() -> Result<&'static DynGames, String> {
    services::get(GAMES)
}
