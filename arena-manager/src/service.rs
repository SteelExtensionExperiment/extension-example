//! The `arena-manager:match/v1` service and the per-tick driver.
use crate::{
    host::LiveHost,
    runtime::{Context, Emission, Request},
    state::ArenaManager,
    store::ArenaStore,
};
use arena_manager_api::{MatchId, MatchInfo, Matches, StartRequest};
use steel_extension_api::{
    ApiResult, Server,
    hooks::do_action,
    log,
    stabby::{self, str::Str, string::String as ApiString, vec::Vec as ApiVec},
};

/// The service object registered in `init`.
pub struct MatchService;

/// Game types with at least one arena.
fn game_types(store: &ArenaStore) -> impl Iterator<Item = &str> {
    store
        .game_types
        .iter()
        .filter(|entry| !entry.arenas.is_empty())
        .map(|entry| entry.name.as_str())
}

/// Whether `game_type` has a complete arena with exactly `players` slots.
fn can_host(store: &ArenaStore, game_type: &str, players: usize) -> bool {
    store.game_type(game_type).is_some_and(|entry| {
        entry
            .arenas
            .iter()
            .any(|arena| arena.is_complete() && arena.slot_count() == players)
    })
}

#[stabby::export(callback)]
impl Matches for MatchService {
    fn start_match<'a>(&self, server: Server<'a>, request: StartRequest) -> ApiResult<MatchId> {
        let start = || {
            let manager = ArenaManager::get()?;
            let players = request
                .players
                .iter()
                .map(|entry| (entry.uuid, entry.kit.clone()))
                .collect();
            let request = Request {
                game_type: request.game_type.as_str(),
                players,
            };
            let mut runtime = manager.runtime();
            let store = manager.store();
            let context = Context {
                store: &store,
                config: &manager.config,
            };
            runtime.start_match(&LiveHost { server: &server }, &context, &request)
        };
        start().map_err(ApiString::from).into()
    }

    fn matches(&self, _server: Server<'_>) -> ApiVec<MatchInfo> {
        ArenaManager::get().map_or_else(
            |_| ApiVec::new(),
            |manager| manager.runtime().infos().into_iter().collect(),
        )
    }

    fn game_types(&self, _server: Server<'_>) -> ApiVec<ApiString> {
        ArenaManager::get().map_or_else(
            |_| ApiVec::new(),
            |manager| game_types(&manager.store()).map(ApiString::from).collect(),
        )
    }

    fn can_host(&self, _server: Server<'_>, game_type: Str<'_>, players: u32) -> bool {
        let Ok(players) = usize::try_from(players) else {
            return false;
        };
        ArenaManager::get()
            .is_ok_and(|manager| can_host(&manager.store(), game_type.as_str(), players))
    }
}

/// Advances the runtime by one server tick, then emits its actions with no lock held.
pub fn on_tick(server: &Server<'_>, _tick: u64) {
    let Ok(manager) = ArenaManager::get() else {
        return;
    };
    let incoming = manager.take_incoming();
    let emissions = {
        let mut runtime = manager.runtime();
        let store = manager.store();
        let context = Context {
            store: &store,
            config: &manager.config,
        };
        runtime.tick(&LiveHost { server }, &context, incoming)
    };
    for emission in emissions {
        let result: Result<(), ApiString> = match &emission {
            Emission::Finished(finished) => do_action(finished).into(),
            Emission::Failed(failed) => do_action(failed).into(),
        };
        if let Err(error) = result {
            log::error(format!("match action failed: {error}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Spawn;

    fn spawn(slot: u32) -> Spawn {
        Spawn {
            slot,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        }
    }

    #[test]
    fn hosting_needs_a_complete_arena_with_that_many_slots() {
        let mut store = ArenaStore::default();
        store.create_arena("duel", "a").unwrap();
        assert!(!can_host(&store, "duel", 2));
        for slot in 1..=2 {
            store.set_spawn("duel", "a", spawn(slot)).unwrap();
        }
        assert!(can_host(&store, "duel", 2));
        assert!(!can_host(&store, "duel", 3));
        assert!(!can_host(&store, "other", 2));
        store.create_arena("empty", "x").unwrap();
        assert_eq!(game_types(&store).count(), 2);
    }
}
