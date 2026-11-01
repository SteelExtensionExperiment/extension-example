//! Prison mine service contract (`prison:mines/v1`), provided by `prison-mines`.
//!
//! Enchant and economy plugins use it to find the mine a block belongs to, report blocks they
//! broke in bulk (so threshold resets stay accurate) and trigger resets.
#![recursion_limit = "256"]

use steel_extension_api::{
    ApiResult, BlockPos, Server,
    stabby::{
        self, option::Option as StableOption, str::Str, string::String as ApiString,
        vec::Vec as ApiVec,
    },
};

/// Service key of the mine contract.
pub const MINES: &str = "prison:mines/v1";

/// A mine as seen by other plugins.
#[stabby::stabby]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MineInfo {
    /// Mine name.
    pub name: ApiString,
    /// World key.
    pub world: ApiString,
    /// Blocks in the mine.
    pub volume: u64,
    /// Blocks mined since the last reset.
    pub mined: u64,
    /// Seconds until the timed reset; negative when the mine has no timer.
    pub seconds_until_reset: i64,
    /// Minimum corner (inclusive).
    pub min: BlockPos,
    /// Maximum corner (inclusive).
    pub max: BlockPos,
}

impl MineInfo {
    /// Whether `pos` lies inside the mine.
    #[must_use]
    pub const fn contains(&self, pos: BlockPos) -> bool {
        pos.x >= self.min.x
            && pos.x <= self.max.x
            && pos.y >= self.min.y
            && pos.y <= self.max.y
            && pos.z >= self.min.z
            && pos.z <= self.max.z
    }
}

/// Outcome of a reset.
#[stabby::stabby]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResetReport {
    /// Blocks written.
    pub blocks: u64,
    /// Write time in microseconds.
    pub micros: u64,
}

/// Mine provider. Methods taking a [`Server`] must be called on the server thread.
#[stabby::stabby(checked)]
pub trait Mines: Send + Sync {
    /// The mine containing `pos` in `world`, if any.
    extern "C" fn mine_at(&self, world: Str<'_>, pos: BlockPos) -> StableOption<MineInfo>;
    /// Every mine.
    extern "C" fn mines(&self) -> ApiVec<MineInfo>;
    /// Adds blocks broken by something other than a normal block break (enchants, explosions).
    extern "C" fn add_mined(&self, name: Str<'_>, count: u64);
    /// Lifts players out and refills the mine now.
    extern "C" fn reset(&self, server: Server<'_>, name: Str<'_>) -> ApiResult<ResetReport>;
}

/// Owning, checked mine provider object.
pub type DynMines = stabby::dynptr!(stabby::boxed::Box<dyn Mines + Send + Sync>);

/// The active mine provider.
///
/// # Errors
/// Returns an error when `prison-mines` is not installed or incompatible.
pub fn mines() -> Result<&'static DynMines, String> {
    steel_extension_api::services::get(MINES)
}
