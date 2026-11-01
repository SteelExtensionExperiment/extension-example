//! Process-wide plugin state, created during init.
//!
//! Event handlers run on several threads, so data sits behind mutexes. Never hold a lock while
//! calling the server: teleports fire this plugin's own teleport handler, which locks again.

use crate::{
    config::Config,
    messages::Messages,
    storage::{Data, Storage},
};
use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
};
use steel_extension_api::players::Uuid;

/// A pending `/tpa` or `/tpahere` request, keyed by its target.
#[derive(Debug, Clone, Copy)]
pub struct TeleportRequest {
    /// Player who sent the request.
    pub requester: Uuid,
    /// Whether the target should come to the requester (`/tpahere`).
    pub here: bool,
    /// Server tick the request was sent.
    pub sent_tick: u64,
}

/// State that only lives while the server runs.
#[derive(Debug, Default)]
pub struct Runtime {
    /// Pending teleport requests by target.
    pub requests: BTreeMap<Uuid, TeleportRequest>,
    /// Last private message partner, for `/r`.
    pub reply_to: BTreeMap<Uuid, Uuid>,
    /// Last activity tick of each online player.
    pub last_activity: BTreeMap<Uuid, u64>,
    /// Players currently marked AFK.
    pub afk: BTreeMap<Uuid, bool>,
}

/// Everything Essentials owns.
pub struct Essentials {
    /// Parsed configuration.
    pub config: Config,
    /// Message templates.
    pub messages: Messages,
    /// Data directory access.
    pub storage: Storage,
    data: Mutex<Data>,
    runtime: Mutex<Runtime>,
}

static ESSENTIALS: OnceLock<Essentials> = OnceLock::new();

impl Essentials {
    /// Installs the state once.
    pub fn install(config: Config, storage: Storage, data: Data) -> Result<(), String> {
        let messages = Messages::new(config.messages.clone());
        ESSENTIALS
            .set(Self {
                config,
                messages,
                storage,
                data: Mutex::new(data),
                runtime: Mutex::new(Runtime::default()),
            })
            .map_err(|_| "Essentials initialized twice".to_owned())
    }

    /// Persistent data.
    pub fn data(&self) -> MutexGuard<'_, Data> {
        self.data.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runtime-only data.
    pub fn runtime(&self) -> MutexGuard<'_, Runtime> {
        self.runtime.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Formats a configured message.
    pub fn msg(&self, key: &str, args: &[(&str, &str)]) -> String {
        self.messages.format(key, args)
    }

    /// Saves dirty data now.
    pub fn save(&self) {
        let mut data = self.data();
        if data.is_dirty() {
            self.storage.save(&mut data);
        }
    }
}

/// The installed state, or an error before init.
pub fn essentials() -> Result<&'static Essentials, String> {
    ESSENTIALS
        .get()
        .ok_or_else(|| "Essentials is not initialized".to_owned())
}
