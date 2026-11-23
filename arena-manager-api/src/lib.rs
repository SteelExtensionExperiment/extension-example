//! Arena manager contract.
//!
//! Calls go down through the [`MATCHES`] service, as with `vault-api`. Results go up through the
//! [`MatchFinished`] and [`MatchFailed`] hook actions, so a listener needs no dependency on
//! arena-manager and no dependency cycle can form.
//!
//! Emit an action only after releasing every lock the emitter holds: dispatch is synchronous and a
//! listener may call back into [`Matches::start_match`].
//!
//! Each [`PlayerEntry`] carries the kit as an [`InventorySnapshot`] (host spec 007): arena-manager
//! restores it with `Server::restore_inventory` when the player is placed, which also applies the
//! snapshot's game mode. It stores no kits.
#![recursion_limit = "256"]

use steel_extension_api::{
    ApiResult, Server,
    hooks::Action,
    inventory_snapshot::InventorySnapshot,
    players::Uuid,
    services,
    stabby::{self, str::Str, string::String as ApiString, vec::Vec as ApiVec},
};

/// Service key of the match contract.
pub const MATCHES: &str = "arena-manager:match/v1";

/// Identifier of one match, unique for the life of the process.
pub type MatchId = u64;

/// How one participant's match ended.
#[stabby::stabby]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Last player standing.
    Won,
    /// Eliminated or left.
    Lost,
    /// No winner.
    Draw,
}

/// A player taking part in a match.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct PlayerEntry {
    /// Profile UUID.
    pub uuid: Uuid,
    /// Inventory and game mode the player receives on arrival, captured with
    /// `Server::inventory_snapshot`.
    pub kit: InventorySnapshot,
}

/// Request to start one match.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct StartRequest {
    /// Game type name, owned by the caller; arena-manager treats it as opaque text.
    pub game_type: ApiString,
    /// Every participant.
    pub players: ApiVec<PlayerEntry>,
}

/// Lifecycle state of a match.
#[stabby::stabby]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchState {
    /// Waiting for the arena world.
    Preparing,
    /// World ready, players being placed.
    Starting,
    /// Match in progress.
    Running,
    /// Winner decided, countdown before returning to the lobby.
    Ending,
}

/// A participant in a running match.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct MatchPlayer {
    /// Profile UUID.
    pub uuid: Uuid,
    /// Whether the player is still in the match.
    pub alive: bool,
}

/// Snapshot of one match.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct MatchInfo {
    /// Match identifier.
    pub id: MatchId,
    /// Game type name.
    pub game_type: ApiString,
    /// Arena name within the game type.
    pub arena: ApiString,
    /// World key of the arena clone, empty until it exists.
    pub world: ApiString,
    /// Current state.
    pub state: MatchState,
    /// Participants.
    pub players: ApiVec<MatchPlayer>,
}

/// Outcome of one participant, in a finished match.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct Participant {
    /// Profile UUID.
    pub uuid: Uuid,
    /// Name at the end of the match.
    pub name: ApiString,
    /// Result for this player.
    pub outcome: Outcome,
}

/// Emitted once when a match finished. `Draw` participants are included.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct MatchFinished {
    /// Match identifier.
    pub id: MatchId,
    /// Game type name.
    pub game_type: ApiString,
    /// Every participant exactly once.
    pub participants: ApiVec<Participant>,
}

impl Action for MatchFinished {
    const TAG: &'static str = "arena-manager:match_finished/v1";
}

/// Emitted once when a match failed before running (clone error, player not ready). Not a result.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct MatchFailed {
    /// Match identifier.
    pub id: MatchId,
    /// Game type name.
    pub game_type: ApiString,
    /// Why the match failed.
    pub message: ApiString,
}

impl Action for MatchFailed {
    const TAG: &'static str = "arena-manager:match_failed/v1";
}

/// Match provider. Calls run on the server tick thread and never block.
#[stabby::stabby(checked)]
pub trait Matches: Send + Sync {
    /// Starts a match, or refuses synchronously: unknown game type, no complete arena for the
    /// player count, a player offline or already in a match, no capacity, template not ready.
    /// The arena world may still be created afterwards; follow progress with [`Matches::matches`].
    extern "C" fn start_match<'a>(
        &self,
        server: Server<'a>,
        request: StartRequest,
    ) -> ApiResult<MatchId>;
    /// Snapshot of every current match.
    extern "C" fn matches(&self, server: Server<'_>) -> ApiVec<MatchInfo>;
    /// Game types that have at least one arena, as saved by arena-manager.
    extern "C" fn game_types(&self, server: Server<'_>) -> ApiVec<ApiString>;
    /// Whether `game_type` has a complete arena for `players` players.
    extern "C" fn can_host(&self, server: Server<'_>, game_type: Str<'_>, players: u32) -> bool;
}

/// Owning, checked match provider object.
pub type DynMatches = stabby::dynptr!(stabby::boxed::Box<dyn Matches + Send + Sync>);

/// The active match provider.
///
/// # Errors
/// Returns an error when no compatible provider is registered.
pub fn matches() -> Result<&'static DynMatches, String> {
    services::get(MATCHES)
}
