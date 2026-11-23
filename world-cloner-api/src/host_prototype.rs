//! PROTOTYPE ONLY: mirrors host spec 005 `runtime/types.rs`; replace this module with re-exports
//! from `steel_extension_api` when host 005 is merged. Same names and layouts so the swap changes
//! no downstream code.

use steel_extension_api::{
    BlockPos, Difficulty, GameMode,
    stabby::{self, option::Option as StableOption, string::String as ApiString},
};

/// Load state of a world template.
#[stabby::stabby]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateState {
    /// The template is still being read.
    Loading,
    /// The template can be cloned.
    Ready,
    /// The template could not be loaded; see the status message.
    Failed,
}

/// Status of one world template in the host registry.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct TemplateStatus {
    /// Current state.
    pub state: TemplateState,
    /// Failure diagnostic; empty unless `Failed`.
    pub message: ApiString,
    /// Chunks held by the template.
    pub chunks: u64,
}

/// Where a cloned or created world is placed.
#[stabby::stabby]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloneTarget {
    /// The key's namespace is a new temporary domain that has never been used.
    NewTemporaryDomain,
    /// The key's namespace is a loaded domain that is not closing.
    ExistingDomain,
}

/// Per-clone overrides; `None` keeps the template's value.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct CloneSettings {
    /// World seed.
    pub seed: StableOption<i64>,
    /// Spawn position.
    pub spawn: StableOption<BlockPos>,
    /// Game mode for first-time visitors; `None` uses the server default.
    pub default_game_mode: StableOption<GameMode>,
    /// Initial difficulty.
    pub difficulty: StableOption<Difficulty>,
}
