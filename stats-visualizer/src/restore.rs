//! Boards follow their world: the hologram is created when the world loads and the board goes
//! inactive when the world unloads (the host drops the entities itself).
//!
//! Creating a hologram needs a `Server`. `init` and the world events do not provide one, so
//! both schedule a task for the next tick, which does.

use std::sync::Mutex;

use steel_extension_api::events::EventPriority;
use steel_extension_api::{Hooks, scheduler};

use crate::boards::{HologramHost, refresh_boards};
use crate::log_errors;
use crate::state::{self, BoardList, lock};
use crate::stats::Stats;

/// Creates the holograms of the boards in `world`.
pub fn restore_world<S: HologramHost>(
    boards: &Mutex<BoardList<S::Handle>>,
    stats: &Mutex<Stats>,
    host: &S,
    world: &str,
) -> Vec<String> {
    refresh_boards(boards, stats, host, |board| board.record.world == world)
}

/// Creates the holograms of the boards whose world is already loaded.
pub fn restore_loaded<S: HologramHost>(
    boards: &Mutex<BoardList<S::Handle>>,
    stats: &Mutex<Stats>,
    host: &S,
) -> Vec<String> {
    refresh_boards(boards, stats, host, |board| {
        host.world_loaded(&board.record.world)
    })
}

/// Marks the boards in `world` inactive. The host already dropped their holograms, so nothing
/// is removed.
pub fn deactivate_world<H: Copy>(boards: &Mutex<BoardList<H>>, world: &str) {
    for board in &mut lock(boards).boards {
        if board.record.world == world {
            board.deactivate();
        }
    }
}

/// Restores the boards of worlds that loaded before the extension started.
pub fn restore_on_next_tick() {
    scheduler::run_later(1, |server| {
        if let Ok(state) = state::state() {
            log_errors(restore_loaded(&state.boards, &state.stats, server));
        }
    });
}

/// Subscribes to world load and unload in pre-init.
pub fn register(hooks: &mut Hooks<'_>) {
    hooks.on_world_load(EventPriority::Monitor, |event| {
        let world = event.world().key().to_string();
        scheduler::run_later(1, move |server| {
            if let Ok(state) = state::state() {
                log_errors(restore_world(&state.boards, &state.stats, server, &world));
            }
        });
    });
    hooks.on_world_unload(EventPriority::Monitor, |event| {
        if let Ok(state) = state::state() {
            deactivate_world(&state.boards, &event.world());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boards::Board;
    use crate::boards::tests::{Call, FakeHost, record};

    fn in_world(id: &str, world: &str) -> Board<u32> {
        let mut record = record(id, "skywars");
        record.world = world.to_owned();
        Board::new(record)
    }

    fn boards() -> Mutex<BoardList<u32>> {
        Mutex::new(BoardList {
            boards: vec![in_world("a", "lobby:main"), in_world("b", "arena:one")],
            dirty: false,
        })
    }

    #[test]
    fn a_loading_world_restores_only_its_boards() {
        let host = FakeHost::default();
        let boards = boards();
        let errors = restore_world(&boards, &Mutex::new(Stats::default()), &host, "lobby:main");
        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(
            *host.calls.borrow(),
            [Call::Create(
                "lobby:main".to_owned(),
                "skywars\nno wins yet".to_owned()
            )]
        );
        let list = lock(&boards);
        assert!(list.boards[0].hologram.is_some());
        assert!(list.boards[1].hologram.is_none());
    }

    #[test]
    fn startup_restores_only_boards_of_loaded_worlds() {
        let host = FakeHost {
            unloaded: vec!["arena:one"],
            ..FakeHost::default()
        };
        let boards = boards();
        let errors = restore_loaded(&boards, &Mutex::new(Stats::default()), &host);
        assert_eq!(errors, Vec::<String>::new());
        let list = lock(&boards);
        assert!(list.boards[0].hologram.is_some());
        assert!(list.boards[1].hologram.is_none());
        assert_eq!(list.records().len(), 2);
    }

    #[test]
    fn unloading_a_world_deactivates_its_boards_without_host_calls() {
        let host = FakeHost::default();
        let boards = boards();
        let stats = Mutex::new(Stats::default());
        restore_world(&boards, &stats, &host, "lobby:main");
        restore_world(&boards, &stats, &host, "arena:one");
        host.calls.borrow_mut().clear();
        deactivate_world(&boards, "lobby:main");
        assert!(host.calls.borrow().is_empty());
        {
            let list = lock(&boards);
            assert_eq!(list.boards[0].hologram, None);
            assert_eq!(list.boards[0].last_lines, Vec::<String>::new());
            assert!(list.boards[1].hologram.is_some());
        }
        // The world loads again: the board comes back.
        restore_world(&boards, &stats, &host, "lobby:main");
        assert!(lock(&boards).boards[0].hologram.is_some());
    }

    #[test]
    fn a_refused_creation_is_reported_once_across_restores() {
        let host = FakeHost {
            fail_create: true,
            ..FakeHost::default()
        };
        let boards = boards();
        let stats = Mutex::new(Stats::default());
        let first = restore_world(&boards, &stats, &host, "lobby:main");
        let second = restore_world(&boards, &stats, &host, "lobby:main");
        assert_eq!(first, ["Board a: world not loaded"]);
        assert_eq!(second, Vec::<String>::new());
    }
}
