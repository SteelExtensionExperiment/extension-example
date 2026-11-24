//! Process-wide state and the dirty-file save decision.
//!
//! Extensions cannot link the host lock wrappers, so this uses `std` mutexes and recovers a
//! poisoned lock (the data is plain counters and records, valid after a panicked writer).

use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use steel_extension_api::runtime::HologramId;

use crate::boards::Board;
use crate::stats::Stats;
use crate::storage::{BoardRecord, Storage};

/// The runtime boards and whether the saved list differs from them.
#[derive(Debug)]
pub struct BoardList<H = HologramId> {
    pub boards: Vec<Board<H>>,
    pub dirty: bool,
}

impl<H: Copy> BoardList<H> {
    /// Loaded records become inactive boards: no hologram until the first refresh.
    #[must_use]
    pub fn from_records(records: Vec<BoardRecord>) -> Self {
        Self {
            boards: records.into_iter().map(Board::new).collect(),
            dirty: false,
        }
    }

    /// The stored form of every board.
    #[must_use]
    pub fn records(&self) -> Vec<BoardRecord> {
        self.boards
            .iter()
            .map(|board| board.record.clone())
            .collect()
    }
}

/// Where the data files are written; a trait so the save decision is testable without disk.
pub trait DataStore {
    /// # Errors
    /// A message describing why the stats file could not be written.
    fn save_stats(&self, stats: &Stats) -> Result<(), String>;

    /// # Errors
    /// A message describing why the boards file could not be written.
    fn save_boards(&self, boards: &[BoardRecord]) -> Result<(), String>;
}

impl DataStore for Storage {
    fn save_stats(&self, stats: &Stats) -> Result<(), String> {
        Storage::save_stats(self, stats)
    }

    fn save_boards(&self, boards: &[BoardRecord]) -> Result<(), String> {
        Storage::save_boards(self, boards)
    }
}

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Writes the files whose data changed, or both when `force` is set (shutdown). A dirty flag
/// is cleared only after its write succeeded. Returns the write errors for the caller to log;
/// this never logs, so no host call happens under a lock.
pub fn save_changed<S: DataStore, H: Copy>(
    store: &S,
    stats: &Mutex<Stats>,
    boards: &Mutex<BoardList<H>>,
    force: bool,
) -> Vec<String> {
    let mut errors = Vec::new();
    {
        let mut stats = lock(stats);
        if force || stats.is_dirty() {
            match store.save_stats(&stats) {
                Ok(()) => stats.clear_dirty(),
                Err(error) => errors.push(error),
            }
        }
    }
    let mut boards = lock(boards);
    if force || boards.dirty {
        match store.save_boards(&boards.records()) {
            Ok(()) => boards.dirty = false,
            Err(error) => errors.push(error),
        }
    }
    errors
}

/// Everything the extension owns.
pub struct State {
    pub storage: Storage,
    pub stats: Mutex<Stats>,
    pub boards: Mutex<BoardList>,
}

static STATE: OnceLock<State> = OnceLock::new();

impl State {
    /// Installs the state once.
    ///
    /// # Errors
    /// A message if the state is already installed.
    pub fn install(storage: Storage, stats: Stats, boards: Vec<BoardRecord>) -> Result<(), String> {
        STATE
            .set(Self {
                storage,
                stats: Mutex::new(stats),
                boards: Mutex::new(BoardList::from_records(boards)),
            })
            .map_err(|_| "stats-visualizer initialized twice".to_owned())
    }

    /// Saves changed files, or both with `force`; returns the errors to log.
    pub fn save(&self, force: bool) -> Vec<String> {
        save_changed(&self.storage, &self.stats, &self.boards, force)
    }
}

/// The installed state, or an error before init.
///
/// # Errors
/// A message if `init` has not installed the state.
pub fn state() -> Result<&'static State, String> {
    STATE
        .get()
        .ok_or_else(|| "stats-visualizer is not initialized".to_owned())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use steel_extension_api::players::Uuid;

    use super::*;
    use crate::stats::{MatchOutcome, Participant};

    /// Records which files were written; a file listed in `failing` returns an error.
    #[derive(Default)]
    struct FakeStore {
        writes: RefCell<Vec<&'static str>>,
        failing: Vec<&'static str>,
    }

    impl FakeStore {
        fn write(&self, file: &'static str) -> Result<(), String> {
            self.writes.borrow_mut().push(file);
            if self.failing.contains(&file) {
                return Err(format!("{file} failed"));
            }
            Ok(())
        }
    }

    impl DataStore for FakeStore {
        fn save_stats(&self, _: &Stats) -> Result<(), String> {
            self.write("stats")
        }

        fn save_boards(&self, _: &[BoardRecord]) -> Result<(), String> {
            self.write("boards")
        }
    }

    fn dirty_stats() -> Stats {
        let mut stats = Stats::default();
        stats
            .apply(&MatchOutcome {
                game_type: "bedwars".to_owned(),
                winners: vec![Participant {
                    uuid: Uuid { bytes: [1; 16] },
                    name: "a".to_owned(),
                }],
                losers: vec![],
            })
            .unwrap();
        stats
    }

    fn board_list(dirty: bool) -> Mutex<BoardList> {
        Mutex::new(BoardList {
            boards: vec![],
            dirty,
        })
    }

    #[test]
    fn nothing_dirty_writes_nothing() {
        let store = FakeStore::default();
        let errors = save_changed(
            &store,
            &Mutex::new(Stats::default()),
            &board_list(false),
            false,
        );
        assert_eq!(errors, Vec::<String>::new());
        assert!(store.writes.borrow().is_empty());
    }

    #[test]
    fn dirty_stats_write_only_stats_and_clear_flag() {
        let store = FakeStore::default();
        let stats = Mutex::new(dirty_stats());
        let boards = board_list(false);
        let errors = save_changed(&store, &stats, &boards, false);
        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(*store.writes.borrow(), ["stats"]);
        assert!(!stats.lock().unwrap().is_dirty());
    }

    #[test]
    fn dirty_boards_write_only_boards_and_clear_flag() {
        let store = FakeStore::default();
        let boards = board_list(true);
        let errors = save_changed(&store, &Mutex::new(Stats::default()), &boards, false);
        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(*store.writes.borrow(), ["boards"]);
        assert!(!boards.lock().unwrap().dirty);
    }

    #[test]
    fn failed_write_keeps_flag_and_reports_error() {
        let store = FakeStore {
            failing: vec!["stats"],
            ..FakeStore::default()
        };
        let stats = Mutex::new(dirty_stats());
        let boards = board_list(true);
        let errors = save_changed(&store, &stats, &boards, false);
        assert_eq!(errors, ["stats failed"]);
        assert!(stats.lock().unwrap().is_dirty());
        // The other file is still attempted and cleared.
        assert!(!boards.lock().unwrap().dirty);
    }

    #[test]
    fn force_writes_both_even_when_clean() {
        let store = FakeStore::default();
        let errors = save_changed(
            &store,
            &Mutex::new(Stats::default()),
            &board_list(false),
            true,
        );
        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(*store.writes.borrow(), ["stats", "boards"]);
    }
}
