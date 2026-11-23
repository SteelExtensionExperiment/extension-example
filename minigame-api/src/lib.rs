//! Native service contract for temporary lobby and arena rooms.
#![recursion_limit = "256"]

use steel_extension_api::{
    ApiResult, PlayerId, PlayerInfo, Position, Server, WorldSettings, services,
    stabby::{self, slice::Slice, str::Str, string::String as ApiString, vec::Vec as ApiVec},
};

/// Service provided by the Minigame extension.
pub const ROOMS: &str = "minigame:rooms/v1";

/// A temporary world's generator and initial settings.
#[stabby::stabby]
pub struct WorldDefinition {
    pub generator: ApiString,
    pub settings: WorldSettings,
}

/// Configuration for one room with a lobby and an arena in the same temporary domain.
#[stabby::stabby]
pub struct RoomDefinition {
    /// The namespace prefix for the temporary domain.
    pub domain_prefix: ApiString,
    pub capacity: u32,
    pub lobby: WorldDefinition,
    pub arena: WorldDefinition,
}

/// World and membership lifecycle. Game-specific phases belong to the caller.
#[stabby::stabby]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomPhase {
    Idle,
    PreparingLobby,
    Lobby,
    PreparingArena,
    Arena,
    Returning,
    Retiring,
    Closed,
}

/// Current human membership, including pending entries and returns.
#[stabby::stabby]
pub struct Member {
    pub player: PlayerId,
    pub arrived: bool,
    pub leaving: bool,
}

/// Changes produced by one call to `Room::tick`.
#[stabby::stabby]
pub struct RoomUpdate {
    pub phase: RoomPhase,
    /// Players who finished loading the lobby on this tick.
    pub arrivals: ApiVec<PlayerId>,
}

/// A room owned and driven by a minigame from the server thread.
///
/// After opening a room, keep ticking it through `Closed` before releasing it.
#[stabby::stabby(checked)]
pub trait Room: Send + Sync {
    extern "C" fn phase(&self) -> RoomPhase;
    extern "C" fn lobby_key<'a>(&'a self) -> Str<'a>;
    extern "C" fn arena_key<'a>(&'a self) -> Str<'a>;
    extern "C" fn members(&self) -> ApiVec<Member>;
    extern "C" fn bots(&self) -> ApiVec<PlayerId>;
    /// Admit a human and prepare the lobby on the first successful join.
    extern "C" fn join<'a, 'server>(
        &mut self,
        server: &'a Server<'server>,
        player: PlayerId,
    ) -> ApiResult<()>;
    /// Return a human to their original domain.
    extern "C" fn leave(&mut self, player: PlayerId) -> ApiResult<()>;
    /// Advance world operations and membership using the current player snapshot.
    extern "C" fn tick<'a, 'server>(
        &mut self,
        server: &'a Server<'server>,
        players: Slice<'a, PlayerInfo>,
    ) -> ApiResult<RoomUpdate>;
    /// Close admissions and prepare the arena.
    extern "C" fn start_arena<'a, 'server>(&mut self, server: &'a Server<'server>)
    -> ApiResult<()>;
    /// Queue a transfer once and report whether the actor is ready in the arena.
    extern "C" fn enter_arena<'a, 'server>(
        &mut self,
        server: &'a Server<'server>,
        player: &'a PlayerInfo,
    ) -> ApiResult<bool>;
    /// Add a bot in the lobby and include it in automatic room cleanup.
    extern "C" fn spawn_bot<'a, 'server>(
        &mut self,
        server: &'a Server<'server>,
        name: Str<'a>,
        position: Position,
    ) -> ApiResult<PlayerId>;
    extern "C" fn despawn_bot<'a, 'server>(
        &mut self,
        server: &'a Server<'server>,
        player: PlayerId,
    ) -> ApiResult<()>;
    /// Stop admissions and return players before retiring the worlds.
    extern "C" fn close(&mut self);
}

pub type DynRoom = stabby::dynptr!(stabby::boxed::Box<dyn Room + Send + Sync>);

#[stabby::stabby(checked)]
pub trait Rooms: Send + Sync {
    /// Allocate an idle room. Its first join requests a new temporary domain.
    extern "C" fn create(&self, definition: RoomDefinition) -> ApiResult<DynRoom>;
}

pub type DynRooms = stabby::dynptr!(stabby::boxed::Box<dyn Rooms + Send + Sync>);

/// Resolve the installed room provider using Steel's ABI check.
pub fn rooms() -> Result<&'static DynRooms, String> {
    services::get(ROOMS)
}
