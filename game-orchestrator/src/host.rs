//! Shared runtime state and the host calls that deliver messages.

use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use steel_extension_api::{Server, players::Uuid, text};

use crate::orchestrator::{Notice, Orchestrator};

static STATE: OnceLock<Mutex<Orchestrator>> = OnceLock::new();

const NOT_READY: &str = "the game orchestrator is not ready";

pub(crate) fn install(orchestrator: Orchestrator) -> Result<(), String> {
    STATE
        .set(Mutex::new(orchestrator))
        .map_err(|_| "the game orchestrator is already initialised".to_owned())
}

fn lock() -> Result<MutexGuard<'static, Orchestrator>, String> {
    let state = STATE.get().ok_or_else(|| NOT_READY.to_owned())?;
    Ok(state.lock().unwrap_or_else(PoisonError::into_inner))
}

/// Runs `action` on the state. The lock is released before this returns, so callers send
/// messages and emit actions only afterwards.
pub(crate) fn with<R>(action: impl FnOnce(&mut Orchestrator) -> R) -> Result<R, String> {
    Ok(action(&mut *lock()?))
}

/// Sends legacy-formatted text (`&` colour codes) to an online player; offline players are
/// skipped.
pub(crate) fn tell(server: &Server<'_>, player: Uuid, message: &str) {
    let Some(id) = Option::from(server.player_by_uuid(player)) else {
        return;
    };
    let _ = server.send_message(id, text::legacy(message).to_json().as_str().into());
}

pub(crate) fn deliver(server: &Server<'_>, notices: Vec<Notice>) {
    for notice in notices {
        tell(server, notice.to, &format!("&e{}", notice.text));
    }
}
