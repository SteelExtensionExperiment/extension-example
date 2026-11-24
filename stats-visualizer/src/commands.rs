//! `/statsboard place|remove|list`.
//!
//! The decisions live in [`place`], [`remove`] and [`list`], which take their sources through
//! [`Env`] so they run without a host. The closures at the bottom only read arguments and send
//! feedback.

use std::sync::Mutex;

use steel_extension_api::command::{
    Arg, CommandBuilder, Invocation, SenderKind, argument, literal,
};
use steel_extension_api::runtime::Position;
use steel_extension_api::{Commands, Server};

use crate::boards::{Board, HologramHost, ranking_for, render_lines};
use crate::game_types::{GameTypes, OrchestratorGames, check_game_type, unknown_type_boards};
use crate::log_errors;
use crate::state::{self, BoardList, DataStore, lock, save_changed};
use crate::stats::Stats;
use crate::storage::{BoardRecord, Storage};

const PERMISSION_PLACE: &str = "statsboard.place";
const PERMISSION_REMOVE: &str = "statsboard.remove";
const PERMISSION_LIST: &str = "statsboard.list";

/// Digits shown for a board coordinate in `list`.
const COORDINATE_DECIMALS: usize = 2;

const CONSOLE_NEEDS_COORDINATES: &str = "the console must give <x> <y> <z> <world>";

/// Everything the commands read and change.
pub struct Env<'a, S: HologramHost, G, D> {
    pub host: &'a S,
    pub games: &'a G,
    pub store: &'a D,
    pub stats: &'a Mutex<Stats>,
    pub boards: &'a Mutex<BoardList<S::Handle>>,
}

/// Where a player stands; the default placement of `place`.
pub struct Spot {
    pub world: String,
    pub position: Position,
}

pub struct PlaceRequest<'a> {
    pub id: &'a str,
    pub game_type: &'a str,
    /// `None` places at the sender's position.
    pub position: Option<Position>,
    /// `None` uses the sender's world.
    pub world: Option<&'a str>,
}

/// The feedback of a changed board plus the save errors for the caller to log.
#[derive(Debug, PartialEq, Eq)]
pub struct Done {
    pub message: String,
    pub errors: Vec<String>,
}

/// An id is letters, digits, `_` and `-`; ASCII, like an unquoted command word.
fn check_id(id: &str) -> Result<(), String> {
    let valid = !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        return Ok(());
    }
    Err(format!(
        "invalid board id {id}: use letters, digits, _ and -"
    ))
}

fn placement(
    sender: Option<&Spot>,
    request: &PlaceRequest<'_>,
) -> Result<(String, Position), String> {
    match (request.position, request.world, sender) {
        (Some(position), Some(world), _) => Ok((world.to_owned(), position)),
        (Some(position), None, Some(spot)) => Ok((spot.world.clone(), position)),
        (None, _, Some(spot)) => Ok((spot.world.clone(), spot.position)),
        _ => Err(CONSOLE_NEEDS_COORDINATES.to_owned()),
    }
}

fn save<S: HologramHost, G, D: DataStore>(env: &Env<'_, S, G, D>) -> Vec<String> {
    save_changed(env.store, env.stats, env.boards, false)
}

/// Places a board, creates its hologram and saves the boards.
///
/// # Errors
/// The contract's message for an invalid or used id, an unknown game type or world, a console
/// without coordinates, or the host's message if it refuses the hologram. Nothing is saved then.
pub fn place<S: HologramHost, G: GameTypes, D: DataStore>(
    env: &Env<'_, S, G, D>,
    sender: Option<&Spot>,
    request: &PlaceRequest<'_>,
) -> Result<Done, String> {
    check_id(request.id)?;
    check_game_type(env.games, request.game_type)?;
    let (world, position) = placement(sender, request)?;
    if !env.host.world_loaded(&world) {
        return Err(format!("unknown world {world}"));
    }
    let lines = render_lines(
        request.game_type,
        &ranking_for(&lock(env.stats), request.game_type),
    );
    let mut board = Board::new(BoardRecord {
        id: request.id.to_owned(),
        world,
        x: position.x,
        y: position.y,
        z: position.z,
        game_type: request.game_type.to_owned(),
    });
    {
        let mut list = lock(env.boards);
        if list.boards.iter().any(|b| b.record.id == request.id) {
            return Err(format!("board {} already exists", request.id));
        }
        board.refresh(&lines, env.host)?;
        list.boards.push(board);
        list.dirty = true;
    }
    Ok(Done {
        message: format!("Board {} placed", request.id),
        errors: save(env),
    })
}

/// Removes a board, its hologram and its saved entry.
///
/// # Errors
/// `unknown board <id>`, or the host's message if the hologram cannot be removed (the board is
/// kept then).
pub fn remove<S: HologramHost, G, D: DataStore>(
    env: &Env<'_, S, G, D>,
    id: &str,
) -> Result<Done, String> {
    {
        let mut list = lock(env.boards);
        let index = list
            .boards
            .iter()
            .position(|board| board.record.id == id)
            .ok_or_else(|| format!("unknown board {id}"))?;
        list.boards[index].remove(env.host)?;
        list.boards.remove(index);
        list.dirty = true;
    }
    Ok(Done {
        message: format!("Board {id} removed"),
        errors: save(env),
    })
}

/// One line per board. Boards are marked `unknown game type` only while the orchestrator
/// answers; without it nothing is known to be missing.
pub fn list<S: HologramHost, G: GameTypes, D>(env: &Env<'_, S, G, D>) -> Vec<String> {
    let known = env.games.game_types().ok();
    let list = lock(env.boards);
    let records = list.records();
    let unknown = known
        .as_ref()
        .map_or_else(Vec::new, |known| unknown_type_boards(&records, known));
    list.boards
        .iter()
        .map(|board| {
            let record = &board.record;
            let mut notes = Vec::new();
            if board.hologram.is_none() {
                notes.push("inactive");
            }
            if unknown.contains(&record.id.as_str()) {
                notes.push("unknown game type");
            }
            let mut line = format!(
                "{}: {} in {} at {:.COORDINATE_DECIMALS$} {:.COORDINATE_DECIMALS$} {:.COORDINATE_DECIMALS$}",
                record.id, record.game_type, record.world, record.x, record.y, record.z
            );
            if !notes.is_empty() {
                line = format!("{line} ({})", notes.join(", "));
            }
            line
        })
        .collect()
}

fn with_env<R>(
    ctx: &Invocation<'_, '_>,
    run: impl FnOnce(&Env<'_, Server<'_>, OrchestratorGames<'_>, Storage>) -> R,
) -> Result<R, String> {
    let state = state::state()?;
    let server = ctx.server();
    let games = OrchestratorGames(ctx.server());
    Ok(run(&Env {
        host: &server,
        games: &games,
        store: &state.storage,
        stats: &state.stats,
        boards: &state.boards,
    }))
}

fn finish(ctx: &Invocation<'_, '_>, done: Result<Done, String>) -> Result<i32, String> {
    let done = done?;
    log_errors(done.errors);
    ctx.send(&done.message);
    Ok(1)
}

fn run_place(ctx: &Invocation<'_, '_>, located: bool, in_world: bool) -> Result<i32, String> {
    let id = ctx.string("id")?;
    let game_type = ctx.string("gametype")?;
    let position = if located {
        Some(Position {
            x: ctx.double("x")?,
            y: ctx.double("y")?,
            z: ctx.double("z")?,
        })
    } else {
        None
    };
    let world = if in_world {
        Some(ctx.string("world")?)
    } else {
        None
    };
    let spot = (ctx.sender() == SenderKind::Player).then(|| Spot {
        world: ctx.world().key().to_string(),
        position: ctx.position(),
    });
    let request = PlaceRequest {
        id: &id,
        game_type: &game_type,
        position,
        world: world.as_deref(),
    };
    finish(
        ctx,
        with_env(ctx, |env| place(env, spot.as_ref(), &request))?,
    )
}

fn run_list(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    #[expect(
        clippy::redundant_closure,
        reason = "`list` itself does not satisfy the higher-ranked `Env` lifetime"
    )]
    let lines = with_env(ctx, |env| list(env))?;
    if lines.is_empty() {
        ctx.send("No boards placed");
        return Ok(0);
    }
    ctx.send(&lines.join("\n"));
    Ok(i32::try_from(lines.len()).unwrap_or(i32::MAX))
}

fn board_ids() -> Vec<String> {
    let Ok(state) = state::state() else {
        return Vec::new();
    };
    lock(&state.boards)
        .boards
        .iter()
        .map(|board| board.record.id.clone())
        .collect()
}

/// Registers `/statsboard`. Each subcommand carries its own permission node.
///
/// # Errors
/// The host's message if it refuses the command tree.
pub fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    let coordinate = |name: &str| argument(name, Arg::double(f64::MIN, f64::MAX));
    let place = literal("place").permission(PERMISSION_PLACE).then(
        argument("id", Arg::word()).then(
            argument("gametype", Arg::word())
                .executes(|ctx| run_place(ctx, false, false))
                .then(
                    coordinate("x").then(
                        coordinate("y").then(
                            coordinate("z")
                                .executes(|ctx| run_place(ctx, true, false))
                                .then(
                                    argument("world", Arg::word())
                                        .executes(|ctx| run_place(ctx, true, true)),
                                ),
                        ),
                    ),
                ),
        ),
    );
    let remove = literal("remove").permission(PERMISSION_REMOVE).then(
        argument("id", Arg::word())
            .suggests(|_| board_ids())
            .executes(|ctx| {
                let id = ctx.string("id")?;
                finish(ctx, with_env(ctx, |env| remove(env, &id))?)
            }),
    );
    let list = literal("list")
        .permission(PERMISSION_LIST)
        .executes(run_list);
    commands.register(
        CommandBuilder::new("statsboard:statsboard")
            .default_access()
            .then(place)
            .then(remove)
            .then(list),
    )
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::boards::tests::{Call, FakeHost, record};
    use crate::game_types::tests::FakeGames;

    /// Remembers the board ids of every write.
    #[derive(Default)]
    struct FakeStore {
        saved: RefCell<Vec<Vec<String>>>,
    }

    impl DataStore for FakeStore {
        fn save_stats(&self, _: &Stats) -> Result<(), String> {
            Ok(())
        }

        fn save_boards(&self, boards: &[BoardRecord]) -> Result<(), String> {
            self.saved
                .borrow_mut()
                .push(boards.iter().map(|board| board.id.clone()).collect());
            Ok(())
        }
    }

    struct World {
        host: FakeHost,
        games: FakeGames,
        store: FakeStore,
        stats: Mutex<Stats>,
        boards: Mutex<BoardList<u32>>,
    }

    impl World {
        fn new() -> Self {
            Self {
                host: FakeHost::default(),
                games: FakeGames(Some(vec!["skywars", "bedwars"])),
                store: FakeStore::default(),
                stats: Mutex::new(Stats::default()),
                boards: Mutex::new(BoardList {
                    boards: vec![],
                    dirty: false,
                }),
            }
        }

        fn env(&self) -> Env<'_, FakeHost, FakeGames, FakeStore> {
            Env {
                host: &self.host,
                games: &self.games,
                store: &self.store,
                stats: &self.stats,
                boards: &self.boards,
            }
        }

        fn place(&self, sender: Option<&Spot>, request: &PlaceRequest<'_>) -> Result<Done, String> {
            place(&self.env(), sender, request)
        }
    }

    const AT: Position = Position {
        x: 1.5,
        y: 70.0,
        z: -3.0,
    };

    fn spot() -> Spot {
        Spot {
            world: "lobby:main".to_owned(),
            position: AT,
        }
    }

    fn request<'a>(id: &'a str, game_type: &'a str) -> PlaceRequest<'a> {
        PlaceRequest {
            id,
            game_type,
            position: None,
            world: None,
        }
    }

    #[test]
    fn ids_allow_letters_digits_underscore_and_dash_only() {
        for good in ["a", "Lobby_1", "sky-wars", "007"] {
            assert_eq!(check_id(good), Ok(()), "{good}");
        }
        for bad in ["", "a b", "a:b", "a.b", "ü", "a/b"] {
            assert!(check_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_player_places_at_their_spot_and_the_board_is_saved() {
        let world = World::new();
        let done = world
            .place(Some(&spot()), &request("lobby", "skywars"))
            .unwrap();
        assert_eq!(
            done,
            Done {
                message: "Board lobby placed".to_owned(),
                errors: vec![]
            }
        );
        assert_eq!(
            *world.host.calls.borrow(),
            [Call::Create(
                "lobby:main".to_owned(),
                "skywars\nno wins yet".to_owned()
            )]
        );
        let list = lock(&world.boards);
        assert_eq!(list.boards[0].hologram, Some(1));
        assert_eq!(
            (list.boards[0].record.x, list.boards[0].record.z),
            (1.5, -3.0)
        );
        assert_eq!(*world.store.saved.borrow(), [vec!["lobby".to_owned()]]);
    }

    #[test]
    fn coordinates_default_to_the_senders_world_and_the_console_must_give_a_world() {
        let world = World::new();
        let at = |world: Option<&'static str>| PlaceRequest {
            position: Some(AT),
            world,
            ..request("a", "skywars")
        };
        world.place(Some(&spot()), &at(None)).unwrap();
        assert_eq!(lock(&world.boards).boards[0].record.world, "lobby:main");
        let mut other = at(Some("arena:one"));
        other.id = "b";
        world.place(None, &other).unwrap();
        assert_eq!(lock(&world.boards).boards[1].record.world, "arena:one");
        let mut console = at(None);
        console.id = "c";
        assert_eq!(
            world.place(None, &console).unwrap_err(),
            CONSOLE_NEEDS_COORDINATES
        );
    }

    #[test]
    fn the_console_without_coordinates_is_refused() {
        let world = World::new();
        assert_eq!(
            world.place(None, &request("a", "skywars")).unwrap_err(),
            CONSOLE_NEEDS_COORDINATES
        );
        assert!(world.store.saved.borrow().is_empty());
    }

    #[test]
    fn refusals_change_nothing() {
        let world = World::new();
        world
            .place(Some(&spot()), &request("a", "skywars"))
            .unwrap();
        let saved = world.store.saved.borrow().len();
        let calls = world.host.calls.borrow().len();
        let refuse = |id, game_type| world.place(Some(&spot()), &request(id, game_type));
        assert_eq!(
            refuse("a", "skywars").unwrap_err(),
            "board a already exists"
        );
        assert_eq!(refuse("b", "tnt").unwrap_err(), "unknown game type tnt");
        assert!(refuse("b c", "skywars").is_err());
        assert_eq!(lock(&world.boards).boards.len(), 1);
        assert_eq!(world.store.saved.borrow().len(), saved);
        assert_eq!(world.host.calls.borrow().len(), calls);
    }

    #[test]
    fn an_unloaded_world_is_unknown() {
        let mut world = World::new();
        world.host.unloaded = vec!["arena:one"];
        let request = PlaceRequest {
            position: Some(AT),
            world: Some("arena:one"),
            ..request("a", "skywars")
        };
        assert_eq!(
            world.place(None, &request).unwrap_err(),
            "unknown world arena:one"
        );
        assert!(lock(&world.boards).boards.is_empty());
    }

    #[test]
    fn a_hologram_refused_by_the_host_keeps_its_message_and_saves_nothing() {
        let mut world = World::new();
        world.host.fail_create = true;
        assert_eq!(
            world
                .place(Some(&spot()), &request("a", "skywars"))
                .unwrap_err(),
            "world not loaded"
        );
        assert!(lock(&world.boards).boards.is_empty());
        assert!(world.store.saved.borrow().is_empty());
    }

    #[test]
    fn remove_drops_the_hologram_and_saves() {
        let world = World::new();
        world
            .place(Some(&spot()), &request("a", "skywars"))
            .unwrap();
        let done = remove(&world.env(), "a").unwrap();
        assert_eq!(done.message, "Board a removed");
        assert_eq!(world.host.calls.borrow().last(), Some(&Call::Remove(1)));
        assert!(lock(&world.boards).boards.is_empty());
        assert_eq!(
            world.store.saved.borrow().last(),
            Some(&Vec::<String>::new())
        );
    }

    #[test]
    fn removing_an_unknown_id_fails_without_saving() {
        let world = World::new();
        assert_eq!(
            remove(&world.env(), "nope").unwrap_err(),
            "unknown board nope"
        );
        assert!(world.store.saved.borrow().is_empty());
    }

    #[test]
    fn list_marks_inactive_boards_and_vanished_game_types() {
        let mut world = World::new();
        world
            .place(Some(&spot()), &request("live", "skywars"))
            .unwrap();
        {
            let mut list = lock(&world.boards);
            list.boards.push(Board::new(record("sleeping", "skywars")));
            list.boards.push(Board::new(record("old", "gone")));
        }
        assert_eq!(
            list(&world.env()),
            [
                "live: skywars in lobby:main at 1.50 70.00 -3.00",
                "sleeping: skywars in home:overworld at 1.00 2.00 3.00 (inactive)",
                "old: gone in home:overworld at 1.00 2.00 3.00 (inactive, unknown game type)",
            ]
        );
        world.games = FakeGames(None);
        assert!(!list(&world.env())[2].contains("unknown game type"));
    }
}
