//! Vault service contracts.
//!
//! Like Bukkit's Vault, this crate only defines interfaces. Providers register an
//! implementation with [`steel_extension_api::services::register`] under one of the keys below;
//! consumers fetch the active provider with [`economy`], [`permission`] or [`chat`].
//!
//! Differences from Vault: players are identified by UUID only (no `OfflinePlayer` or name
//! overloads), there are no per-world overloads, and banks are not part of the contract.
#![recursion_limit = "256"]

use steel_extension_api::{
    Server,
    players::Uuid,
    stabby::{self, str::Str, string::String as ApiString, vec::Vec as ApiVec},
};

/// Service key of the economy contract.
pub const ECONOMY: &str = "vault:economy/v1";
/// Service key of the permission contract.
pub const PERMISSION: &str = "vault:permission/v1";
/// Service key of the chat metadata contract.
pub const CHAT: &str = "vault:chat/v1";

/// Outcome kind of an economy transaction.
#[stabby::stabby]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseKind {
    /// The transaction was applied.
    Success,
    /// The transaction was refused; see `error`.
    Failure,
    /// The provider does not support this operation.
    NotImplemented,
}

/// Result of an economy transaction.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct EconomyResponse {
    /// Reason for a failure; empty on success.
    pub error: ApiString,
    /// Amount requested.
    pub amount: f64,
    /// Balance after the transaction.
    pub balance: f64,
    /// Outcome.
    pub kind: ResponseKind,
}

impl EconomyResponse {
    /// A successful transaction.
    #[must_use]
    pub fn success(amount: f64, balance: f64) -> Self {
        Self {
            error: ApiString::default(),
            amount,
            balance,
            kind: ResponseKind::Success,
        }
    }

    /// A refused transaction.
    #[must_use]
    pub fn failure(amount: f64, balance: f64, error: &str) -> Self {
        Self {
            error: error.into(),
            amount,
            balance,
            kind: ResponseKind::Failure,
        }
    }

    /// Whether the transaction was applied.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.kind == ResponseKind::Success
    }
}

/// An economy provider. All methods may be called from any thread.
#[stabby::stabby(checked)]
pub trait Economy: Send + Sync {
    /// Provider name, such as `Essentials Economy`.
    extern "C" fn name(&self) -> ApiString;
    /// Digits after the decimal point that amounts are rounded to.
    extern "C" fn fractional_digits(&self) -> i32;
    /// Formats an amount with the currency symbol or name.
    extern "C" fn format(&self, amount: f64) -> ApiString;
    /// Currency name for one unit.
    extern "C" fn currency_name_singular(&self) -> ApiString;
    /// Currency name for several units.
    extern "C" fn currency_name_plural(&self) -> ApiString;
    /// Whether the player has an account.
    extern "C" fn has_account(&self, player: Uuid) -> bool;
    /// Creates an account with the starting balance; returns whether one was created.
    extern "C" fn create_account(&self, player: Uuid, name: Str<'_>) -> bool;
    /// Current balance; zero without an account.
    extern "C" fn balance(&self, player: Uuid) -> f64;
    /// Whether the balance covers `amount`.
    extern "C" fn has(&self, player: Uuid, amount: f64) -> bool;
    /// Removes `amount` (must be non-negative).
    extern "C" fn withdraw(&self, player: Uuid, amount: f64) -> EconomyResponse;
    /// Adds `amount` (must be non-negative).
    extern "C" fn deposit(&self, player: Uuid, amount: f64) -> EconomyResponse;
}

/// Owning, checked economy provider object.
pub type DynEconomy = stabby::dynptr!(stabby::boxed::Box<dyn Economy + Send + Sync>);

/// A permission provider for online players.
///
/// Methods borrow the caller's [`Server`] handle because permission state lives on the host;
/// every plugin callback that needs a permission check already holds one.
#[stabby::stabby(checked)]
pub trait Permission: Send + Sync {
    /// Provider name.
    extern "C" fn name(&self) -> ApiString;
    /// Whether groups are supported.
    extern "C" fn has_group_support(&self) -> bool;
    /// Whether the online player holds `node`, applying `default` when unset.
    extern "C" fn has(
        &self,
        server: Server<'_>,
        player: Uuid,
        node: Str<'_>,
        default: bool,
    ) -> bool;
    /// Groups of the online player, highest priority first.
    extern "C" fn player_groups(&self, server: Server<'_>, player: Uuid) -> ApiVec<ApiString>;
    /// Highest-priority group, or empty.
    extern "C" fn primary_group(&self, server: Server<'_>, player: Uuid) -> ApiString;
    /// Every configured group.
    extern "C" fn groups(&self, server: Server<'_>) -> ApiVec<ApiString>;
    /// Adds the player to a group; returns `false` when unsupported or refused.
    extern "C" fn add_group(&self, server: Server<'_>, player: Uuid, group: Str<'_>) -> bool;
    /// Removes the player from a group; returns `false` when unsupported or refused.
    extern "C" fn remove_group(&self, server: Server<'_>, player: Uuid, group: Str<'_>) -> bool;
}

/// Owning, checked permission provider object.
pub type DynPermission = stabby::dynptr!(stabby::boxed::Box<dyn Permission + Send + Sync>);

/// A chat metadata provider for online players. Methods borrow the caller's [`Server`].
#[stabby::stabby(checked)]
pub trait Chat: Send + Sync {
    /// Provider name.
    extern "C" fn name(&self) -> ApiString;
    /// Chat prefix with `&` color codes, or empty.
    extern "C" fn prefix(&self, server: Server<'_>, player: Uuid) -> ApiString;
    /// Chat suffix with `&` color codes, or empty.
    extern "C" fn suffix(&self, server: Server<'_>, player: Uuid) -> ApiString;
    /// A metadata option as text, or empty when unset.
    extern "C" fn option_string(&self, server: Server<'_>, player: Uuid, key: Str<'_>)
    -> ApiString;
    /// A metadata option as an integer, or `default` when unset or not numeric.
    extern "C" fn option_integer(
        &self,
        server: Server<'_>,
        player: Uuid,
        key: Str<'_>,
        default: i64,
    ) -> i64;
}

/// Owning, checked chat provider object.
pub type DynChat = stabby::dynptr!(stabby::boxed::Box<dyn Chat + Send + Sync>);

/// The active economy provider.
///
/// # Errors
/// Returns an error when no compatible provider is registered.
pub fn economy() -> Result<&'static DynEconomy, String> {
    steel_extension_api::services::get(ECONOMY)
}

/// The active permission provider.
///
/// # Errors
/// Returns an error when no compatible provider is registered.
pub fn permission() -> Result<&'static DynPermission, String> {
    steel_extension_api::services::get(PERMISSION)
}

/// The active chat provider.
///
/// # Errors
/// Returns an error when no compatible provider is registered.
pub fn chat() -> Result<&'static DynChat, String> {
    steel_extension_api::services::get(CHAT)
}
