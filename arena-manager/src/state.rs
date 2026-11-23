//! Process-wide extension state, installed once during init.
//!
//! Handlers run on several threads, so the store sits behind a mutex. Plugins cannot link the
//! host's `SyncMutex`, so this uses `std::sync::Mutex` like the other extensions.
use crate::{config::Config, store::ArenaStore};
use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
};

/// File inside the data directory holding the arenas and the lobby.
pub const ARENAS_FILE: &str = "arenas.toml";

/// Everything the arena manager owns.
pub struct ArenaManager {
    pub config: Config,
    store: Mutex<ArenaStore>,
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

    /// Never hold this while calling the server.
    pub fn store(&self) -> MutexGuard<'_, ArenaStore> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Location of `arenas.toml`.
    pub fn arenas_path(&self) -> &std::path::Path {
        &self.arenas_path
    }
}
