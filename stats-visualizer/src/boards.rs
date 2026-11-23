//! Boards: the leaderboard text, and its hologram kept in step with the stats.
//!
//! Holograms are reached through [`HologramHost`] so the refresh logic runs without a host.

use std::sync::Mutex;

use steel_extension_api::Server;
use steel_extension_api::runtime::{Billboard, HologramId, Position};

use crate::ranking::{RankingEntry, RankingLine, format_ratio, rank};
use crate::state::{BoardList, lock};
use crate::stats::Stats;
use crate::storage::BoardRecord;

/// Shown instead of rank lines while nobody has won the game type.
const NO_WINS_LINE: &str = "no wins yet";
/// Board text always faces the viewer, so it reads from any side.
const BOARD_BILLBOARD: Billboard = Billboard::Center;

/// Creates, updates and removes holograms. `Handle` is the host's [`HologramId`]; it is an
/// associated type only because that id cannot be constructed outside the host crate, so a
/// test host needs its own.
pub trait HologramHost {
    type Handle: Copy;

    /// # Errors
    /// The host's message if the world is not loaded or the position is rejected.
    fn create(&self, world: &str, position: Position, text: &str) -> Result<Self::Handle, String>;

    /// # Errors
    /// The host's message if the hologram no longer exists.
    fn set_text(&self, handle: Self::Handle, text: &str) -> Result<(), String>;

    /// Returns whether the handle was still valid.
    ///
    /// # Errors
    /// The host's message if the removal failed.
    fn remove(&self, handle: Self::Handle) -> Result<bool, String>;
}

impl HologramHost for Server<'_> {
    type Handle = HologramId;

    fn create(&self, world: &str, position: Position, text: &str) -> Result<HologramId, String> {
        self.create_hologram(world.into(), position, text.into(), BOARD_BILLBOARD)
            .match_owned(Ok, |error| Err(error.to_string()))
    }

    fn set_text(&self, handle: HologramId, text: &str) -> Result<(), String> {
        self.set_hologram_text(handle, text.into())
            .match_owned(Ok, |error| Err(error.to_string()))
    }

    fn remove(&self, handle: HologramId) -> Result<bool, String> {
        self.remove_hologram(handle)
            .match_owned(Ok, |error| Err(error.to_string()))
    }
}

/// A stored board plus its runtime hologram. A board without a hologram is inactive; it stays
/// in the saved list and is created on the next refresh.
#[derive(Debug)]
pub struct Board<H = HologramId> {
    pub record: BoardRecord,
    pub hologram: Option<H>,
    /// Lines last sent to the host; an equal ranking skips the host call.
    pub last_lines: Vec<String>,
}

impl<H: Copy> Board<H> {
    #[must_use]
    pub fn new(record: BoardRecord) -> Self {
        Self {
            record,
            hologram: None,
            last_lines: Vec::new(),
        }
    }

    /// Brings the hologram to `lines`: creates it when missing, updates it when the lines
    /// changed, otherwise makes no host call.
    ///
    /// # Errors
    /// The host's message. A failed create leaves the board inactive; a failed update means the
    /// hologram is gone, so it is dropped and the next refresh creates it again.
    pub fn refresh<S: HologramHost<Handle = H>>(
        &mut self,
        lines: &[String],
        host: &S,
    ) -> Result<(), String> {
        let text = lines.join("\n");
        match self.hologram {
            None => {
                let position = Position {
                    x: self.record.x,
                    y: self.record.y,
                    z: self.record.z,
                };
                self.hologram = Some(host.create(&self.record.world, position, &text)?);
            }
            Some(_) if self.last_lines == lines => return Ok(()),
            Some(handle) => {
                if let Err(error) = host.set_text(handle, &text) {
                    self.hologram = None;
                    self.last_lines.clear();
                    return Err(error);
                }
            }
        }
        self.last_lines = lines.to_vec();
        Ok(())
    }

    /// Removes the hologram, leaving the board inactive.
    ///
    /// # Errors
    /// The host's message; the board keeps its hologram then.
    pub fn remove<S: HologramHost<Handle = H>>(&mut self, host: &S) -> Result<(), String> {
        let Some(handle) = self.hologram else {
            return Ok(());
        };
        host.remove(handle)?;
        self.hologram = None;
        self.last_lines.clear();
        Ok(())
    }
}

/// Title, then one line per ranked player, or [`NO_WINS_LINE`] when the ranking is empty.
#[must_use]
pub fn render_lines(game_type: &str, ranking: &[RankingLine]) -> Vec<String> {
    let mut lines = vec![game_type.to_owned()];
    if ranking.is_empty() {
        lines.push(NO_WINS_LINE.to_owned());
    }
    lines.extend(ranking.iter().map(|line| {
        format!(
            "#{} {}  {} wins  {} W/L",
            line.rank,
            line.name,
            line.wins,
            format_ratio(line.ratio)
        )
    }));
    lines
}

/// The top players of `game_type` from the stored records.
#[must_use]
pub fn ranking_for(stats: &Stats, game_type: &str) -> Vec<RankingLine> {
    rank(stats.players().values().filter_map(|player| {
        let record = player.games.get(game_type)?;
        Some(RankingEntry {
            name: &player.name,
            wins: record.wins,
            losses: record.losses,
        })
    }))
}

/// Refreshes the boards of `game_type` and returns the errors for the caller to log.
///
/// The stats lock is released before any host call. The boards lock is held across them: the
/// hologram bindings only touch worlds and never call back into extensions, and the boards
/// must stay consistent with the handles the host returns.
pub fn refresh_game_type<S: HologramHost>(
    boards: &Mutex<BoardList<S::Handle>>,
    stats: &Mutex<Stats>,
    game_type: &str,
    host: &S,
) -> Vec<String> {
    let lines = render_lines(game_type, &ranking_for(&lock(stats), game_type));
    let mut list = lock(boards);
    let mut errors = Vec::new();
    for board in list
        .boards
        .iter_mut()
        .filter(|b| b.record.game_type == game_type)
    {
        if let Err(error) = board.refresh(&lines, host) {
            errors.push(format!("Board {}: {error}", board.record.id));
        }
    }
    errors
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    use steel_extension_api::players::Uuid;

    use super::*;
    use crate::stats::{MatchOutcome, Participant};

    #[derive(Debug, PartialEq)]
    pub(crate) enum Call {
        Create(String, String),
        SetText(u32, String),
        Remove(u32),
    }

    /// Hands out handles 1, 2, ...; `failing` makes the named call return an error.
    #[derive(Default)]
    pub(crate) struct FakeHost {
        pub(crate) calls: RefCell<Vec<Call>>,
        pub(crate) fail_create: bool,
        pub(crate) fail_set_text: bool,
    }

    impl HologramHost for FakeHost {
        type Handle = u32;

        fn create(&self, world: &str, _: Position, text: &str) -> Result<u32, String> {
            if self.fail_create {
                return Err("world not loaded".to_owned());
            }
            let mut calls = self.calls.borrow_mut();
            calls.push(Call::Create(world.to_owned(), text.to_owned()));
            Ok(u32::try_from(calls.len()).unwrap())
        }

        fn set_text(&self, handle: u32, text: &str) -> Result<(), String> {
            if self.fail_set_text {
                return Err("unknown hologram".to_owned());
            }
            self.calls
                .borrow_mut()
                .push(Call::SetText(handle, text.to_owned()));
            Ok(())
        }

        fn remove(&self, handle: u32) -> Result<bool, String> {
            self.calls.borrow_mut().push(Call::Remove(handle));
            Ok(true)
        }
    }

    pub(crate) fn record(id: &str, game_type: &str) -> BoardRecord {
        BoardRecord {
            id: id.to_owned(),
            world: "home:overworld".to_owned(),
            x: 1.0,
            y: 2.0,
            z: 3.0,
            game_type: game_type.to_owned(),
        }
    }

    fn line(rank: u8, name: &str, wins: u32, ratio: f64) -> RankingLine {
        RankingLine {
            rank,
            name: name.to_owned(),
            wins,
            ratio,
        }
    }

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn full_ranking_renders_title_and_five_lines() {
        let ranking: Vec<_> = (1..=5)
            .map(|n| line(n, &format!("p{n}"), u32::from(10 - n), 3.0))
            .collect();
        let rendered = render_lines("skywars", &ranking);
        assert_eq!(rendered.len(), 6);
        assert_eq!(rendered[0], "skywars");
        assert_eq!(rendered[1], "#1 p1  9 wins  3.00 W/L");
        assert_eq!(rendered[5], "#5 p5  5 wins  3.00 W/L");
    }

    #[test]
    fn partial_ranking_renders_only_ranked_players() {
        let rendered = render_lines("skywars", &[line(1, "Ann", 12, 1.5)]);
        assert_eq!(rendered, ["skywars", "#1 Ann  12 wins  1.50 W/L"]);
    }

    #[test]
    fn empty_ranking_renders_the_no_wins_line() {
        assert_eq!(render_lines("skywars", &[]), ["skywars", "no wins yet"]);
    }

    #[test]
    fn ranking_uses_only_the_game_types_records() {
        let mut stats = Stats::default();
        let win = |game_type: &str, n: u8| MatchOutcome {
            game_type: game_type.to_owned(),
            winners: vec![Participant {
                uuid: Uuid { bytes: [n; 16] },
                name: format!("p{n}"),
            }],
            losers: vec![],
        };
        stats.apply(&win("skywars", 1)).unwrap();
        stats.apply(&win("bedwars", 2)).unwrap();
        let ranking = ranking_for(&stats, "skywars");
        assert_eq!(ranking.len(), 1);
        assert_eq!(ranking[0].name, "p1");
    }

    #[test]
    fn first_refresh_creates_and_unchanged_lines_make_no_call() {
        let host = FakeHost::default();
        let mut board = Board::new(record("a", "skywars"));
        let rendered = lines(&["skywars", "no wins yet"]);
        board.refresh(&rendered, &host).unwrap();
        board.refresh(&rendered, &host).unwrap();
        assert_eq!(
            *host.calls.borrow(),
            [Call::Create(
                "home:overworld".to_owned(),
                "skywars\nno wins yet".to_owned()
            )]
        );
    }

    #[test]
    fn changed_lines_make_exactly_one_update() {
        let host = FakeHost::default();
        let mut board = Board::new(record("a", "skywars"));
        board
            .refresh(&lines(&["skywars", "no wins yet"]), &host)
            .unwrap();
        host.calls.borrow_mut().clear();
        board
            .refresh(&lines(&["skywars", "#1 Ann"]), &host)
            .unwrap();
        assert_eq!(
            *host.calls.borrow(),
            [Call::SetText(1, "skywars\n#1 Ann".to_owned())]
        );
    }

    #[test]
    fn failed_create_leaves_the_board_inactive_and_saved() {
        let host = FakeHost {
            fail_create: true,
            ..FakeHost::default()
        };
        let list = Mutex::new(BoardList {
            boards: vec![Board::new(record("a", "skywars"))],
            dirty: false,
        });
        let errors = refresh_game_type(&list, &Mutex::new(Stats::default()), "skywars", &host);
        assert_eq!(errors, ["Board a: world not loaded"]);
        let list = lock(&list);
        assert_eq!(list.boards[0].hologram, None);
        assert_eq!(list.boards[0].last_lines, Vec::<String>::new());
        assert_eq!(list.records(), [record("a", "skywars")]);
    }

    #[test]
    fn failed_update_drops_the_hologram_so_the_next_refresh_recreates_it() {
        let mut host = FakeHost::default();
        let mut board = Board::new(record("a", "skywars"));
        board.refresh(&lines(&["a"]), &host).unwrap();
        host.fail_set_text = true;
        assert!(board.refresh(&lines(&["b"]), &host).is_err());
        assert_eq!(board.hologram, None);
        host.fail_set_text = false;
        board.refresh(&lines(&["b"]), &host).unwrap();
        assert_eq!(board.hologram, Some(2));
    }

    #[test]
    fn only_boards_of_the_game_type_refresh() {
        let host = FakeHost::default();
        let list = Mutex::new(BoardList {
            boards: vec![
                Board::new(record("a", "skywars")),
                Board::new(record("b", "bedwars")),
            ],
            dirty: false,
        });
        let errors = refresh_game_type(&list, &Mutex::new(Stats::default()), "skywars", &host);
        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(host.calls.borrow().len(), 1);
        let list = lock(&list);
        assert!(list.boards[0].hologram.is_some());
        assert!(list.boards[1].hologram.is_none());
    }

    #[test]
    fn remove_clears_the_hologram_once() {
        let host = FakeHost::default();
        let mut board = Board::new(record("a", "skywars"));
        board.refresh(&lines(&["a"]), &host).unwrap();
        board.remove(&host).unwrap();
        board.remove(&host).unwrap();
        assert_eq!(board.hologram, None);
        assert_eq!(host.calls.borrow().last(), Some(&Call::Remove(1)));
        assert_eq!(host.calls.borrow().len(), 2);
    }
}
