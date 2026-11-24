//! Process-wide extension state, installed once during init.
//!
//! Handlers run on several threads, so the store sits behind a mutex. Plugins cannot link the
//! host's `SyncMutex`, so this uses `std::sync::Mutex` like the other extensions.
use crate::{
    config::Config,
    runtime::{Incoming, Runtime},
    store::ArenaStore,
};
use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
};
use steel_extension_api::{Operation, PlayerId};

/// File inside the data directory holding the arenas and the lobby.
pub const ARENAS_FILE: &str = "arenas.toml";

/// Everything the arena manager owns.
pub struct ArenaManager {
    pub config: Config,
    store: Mutex<ArenaStore>,
    runtime: Mutex<Runtime<PlayerId, Operation>>,
    /// Deaths, quits and respawns seen by handlers, resolved on the next tick (research R8).
    incoming: Mutex<Vec<Incoming<PlayerId>>>,
    arenas_path: PathBuf,
}

static ARENA_MANAGER: OnceLock<ArenaManager> = OnceLock::new();

impl ArenaManager {
    /// Installs the state once; `data_directory` holds `arenas.toml`.
    pub fn install(
        config: Config,
        store: ArenaStore,
        data_directory: PathBuf,
    ) -> Result<(), String> {
        ARENA_MANAGER
            .set(Self {
                config,
                store: Mutex::new(store),
                runtime: Mutex::new(Runtime::default()),
                incoming: Mutex::new(Vec::new()),
                arenas_path: data_directory.join(ARENAS_FILE),
            })
            .map_err(|_| "arena-manager initialized twice".to_owned())
    }

    /// The installed state, or an error before init.
    pub fn get() -> Result<&'static Self, String> {
        ARENA_MANAGER
            .get()
            .ok_or_else(|| "arena-manager is not initialized".to_owned())
    }

    /// Lock this before [`Self::store`] when both are needed.
    pub fn runtime(&self) -> MutexGuard<'_, Runtime<PlayerId, Operation>> {
        self.runtime.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn push_incoming(&self, item: Incoming<PlayerId>) {
        self.incoming
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(item);
    }

    pub fn take_incoming(&self) -> Vec<Incoming<PlayerId>> {
        std::mem::take(&mut *self.incoming.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// Never hold this while calling the server.
    pub fn store(&self) -> MutexGuard<'_, ArenaStore> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Location of `arenas.toml`.
    pub fn arenas_path(&self) -> &std::path::Path {
        &self.arenas_path
    }
}
