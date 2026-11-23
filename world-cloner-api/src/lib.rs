//! World cloner service contract.
//!
//! The `world-cloner` extension registers an implementation under [`CLONER`]; consumers fetch it
//! with [`cloner`] and import `WorldClonerDyn as _` to call the methods.
#![recursion_limit = "256"]

mod host_prototype;

pub use host_prototype::{CloneSettings, CloneTarget, TemplateState, TemplateStatus};

use steel_extension_api::{
    ApiResult, Operation, Server, WorldSettings,
    stabby::{self, str::Str, string::String as ApiString, vec::Vec as ApiVec},
};

/// Service key of the world cloner contract.
pub const CLONER: &str = "world-cloner:clone/v1";

/// A configured template and its current host state.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct TemplateInfo {
    /// Bare template name, as configured (not the host key).
    pub name: ApiString,
    /// Current load state.
    pub state: TemplateState,
    /// Failure diagnostic; empty unless `Failed`.
    pub message: ApiString,
}

/// Clones a template into a new world.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct CloneRequest {
    /// Bare template name.
    pub template: ApiString,
    /// World key `domain:world` of the clone.
    pub key: ApiString,
    /// Domain placement of `key`.
    pub target: CloneTarget,
    /// Overrides; unset fields default to the template, and an unset game mode to its configured one.
    pub settings: CloneSettings,
}

/// Creates a generated world without a template.
#[stabby::stabby]
#[derive(Debug, Clone)]
pub struct CreateRequest {
    /// World key `domain:world`.
    pub key: ApiString,
    /// Registered generator key; one that needs configuration is rejected by the host.
    pub generator: ApiString,
    /// Domain placement of `key`.
    pub target: CloneTarget,
    /// Complete settings; a generated world has no template to default from.
    pub settings: WorldSettings,
}

/// Creates and removes worlds from templates, owned by the `world-cloner` extension.
///
/// Each method either fails immediately (`Err` with a diagnostic) or returns an [`Operation`]
/// that belongs to the caller. Poll it with `Server::operation_status` on later ticks until it is
/// `Ready` or `Failed`; never sleep or spin. The first read after completion consumes the result,
/// and an operation that is never read stays in the host table for the life of the process.
/// World keys are `domain:world`, following the host's identifier rules.
#[stabby::stabby(checked)]
pub trait WorldCloner: Send + Sync {
    /// Every configured template with its current state.
    extern "C" fn templates(&self, server: Server<'_>) -> ApiVec<TemplateInfo>;
    /// Starts cloning a template into the requested world.
    extern "C" fn clone_world(
        &self,
        server: Server<'_>,
        request: CloneRequest,
    ) -> ApiResult<Operation>;
    /// Starts creating a generated world.
    extern "C" fn create_world(
        &self,
        server: Server<'_>,
        request: CreateRequest,
    ) -> ApiResult<Operation>;
    /// Starts removing a runtime world; the host refuses `worlds.toml` worlds.
    extern "C" fn remove_world(&self, server: Server<'_>, key: Str<'_>) -> ApiResult<Operation>;
}

/// Owning, checked world cloner object.
pub type DynWorldCloner = stabby::dynptr!(stabby::boxed::Box<dyn WorldCloner + Send + Sync>);

/// The active world cloner.
///
/// # Errors
/// Returns an error when the `world-cloner` extension is not installed or incompatible.
pub fn cloner() -> Result<&'static DynWorldCloner, String> {
    steel_extension_api::services::get(CLONER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_key_is_stable() {
        assert_eq!(CLONER, "world-cloner:clone/v1");
    }
}
