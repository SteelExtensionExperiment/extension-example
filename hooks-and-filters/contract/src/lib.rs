//! Shared payload contracts compiled separately into both extensions and the host test.

use steel_extension_api::{
    hooks::{Action, Filter},
    stabby,
};

/// A custom score transformed by the registered filter chain.
#[stabby::stabby]
pub struct Score {
    /// Current score, updated in place by filters.
    pub value: i32,
}

impl Filter for Score {
    const TAG: &'static str = "hookdemo:score/v1";
}

/// Notification that a score has finished passing through the filter chain.
#[stabby::stabby]
pub struct Scored {
    /// Final score observed by action handlers.
    pub value: i32,
}

impl Action for Scored {
    const TAG: &'static str = "hookdemo:scored/v1";
}
