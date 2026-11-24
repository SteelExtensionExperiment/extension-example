//! The `/arena` command tree. Handlers read the sender, then call plain functions that map
//! to store calls and build the reply, so they run without a host.
use crate::{
    host::{Host as _, LiveHost},
    state::{ARENAS_FILE, ArenaManager},
    store::{ArenaStore, Lobby, LobbyGameMode, Spawn},
};
use arena_manager_api::{MatchInfo, MatchState};
use std::{fmt::Write as _, path::Path};
use steel_extension_api::{
    Commands,
    command::{Arg, CommandBuilder, Invocation, Node, argument, literal},
    events::Location,
};

const PERMISSION_CREATE: &str = "arena.create";
const PERMISSION_REMOVE: &str = "arena.remove";
const PERMISSION_SPAWN: &str = "arena.spawn";
const PERMISSION_TEMPLATE: &str = "arena.template";
const PERMISSION_LIST: &str = "arena.list";
const PERMISSION_LOBBY: &str = "arena.lobby";
const PERMISSION_MATCHES: &str = "arena.matches";

const PLAYER_ONLY: &str = "this command needs a player";

const LOBBY_GAME_MODES: [(&str, LobbyGameMode); 4] = [
    ("survival", LobbyGameMode::Survival),
    ("creative", LobbyGameMode::Creative),
    ("adventure", LobbyGameMode::Adventure),
    ("spectator", LobbyGameMode::Spectator),
];

/// Applies `change`, then saves the store at once; the reply says so.
fn commit(
    store: &mut ArenaStore,
    path: &Path,
    change: impl FnOnce(&mut ArenaStore) -> Result<String, String>,
) -> Result<String, String> {
    let reply = change(store)?;
    store
        .save(path)
        .map_err(|error| format!("{reply}, but it is not saved: {error}"))?;
    Ok(format!("{reply} (saved to {ARENAS_FILE})"))
}

fn create_arena(store: &mut ArenaStore, game_type: &str, arena: &str) -> Result<String, String> {
    store.create_arena(game_type, arena)?;
    Ok(format!("created arena {game_type}/{arena}"))
}

fn remove_arena(store: &mut ArenaStore, game_type: &str, arena: &str) -> Result<String, String> {
    store.remove_arena(game_type, arena)?;
    Ok(format!("removed arena {game_type}/{arena}"))
}

fn set_spawn(
    store: &mut ArenaStore,
    game_type: &str,
    arena: &str,
    spawn: Spawn,
) -> Result<String, String> {
    let slot = spawn.slot;
    store.set_spawn(game_type, arena, spawn)?;
    Ok(format!("set spawn {slot} of {game_type}/{arena}"))
}

fn remove_spawn(
    store: &mut ArenaStore,
    game_type: &str,
    arena: &str,
    slot: u32,
) -> Result<String, String> {
    store.remove_spawn(game_type, arena, slot)?;
    Ok(format!("removed spawn {slot} of {game_type}/{arena}"))
}

fn set_template(store: &mut ArenaStore, game_type: &str, template: &str) -> Result<String, String> {
    store.set_template(game_type, template)?;
    Ok(format!("{game_type} now clones template {template}"))
}

fn set_lobby(store: &mut ArenaStore, lobby: Lobby) -> String {
    let reply = format!(
        "lobby set to {} in {} with game mode {}",
        position_text(lobby.x, lobby.y, lobby.z),
        lobby.world,
        game_mode_name(lobby.game_mode),
    );
    store.set_lobby(lobby);
    reply
}

fn game_mode_name(mode: LobbyGameMode) -> &'static str {
    LOBBY_GAME_MODES
        .iter()
        .find(|(_, candidate)| *candidate == mode)
        .map_or("", |(name, _)| name)
}

fn position_text(x: f64, y: f64, z: f64) -> String {
    format!("{x:.2} {y:.2} {z:.2}")
}

fn list(store: &ArenaStore) -> String {
    if store.game_types.is_empty() {
        return "no game types".to_owned();
    }
    let mut text = String::new();
    for game_type in &store.game_types {
        let template = game_type.template.as_deref().unwrap_or("none");
        let _ = writeln!(text, "{} (template: {template})", game_type.name);
        for arena in &game_type.arenas {
            let state = if arena.is_complete() {
                "complete"
            } else {
                "incomplete"
            };
            let _ = writeln!(
                text,
                "  {}: slots {}, {state}",
                arena.name,
                arena.slot_count()
            );
        }
    }
    text.truncate(text.trim_end().len());
    text
}

fn match_state_name(state: MatchState) -> &'static str {
    match state {
        MatchState::Preparing => "preparing",
        MatchState::Starting => "starting",
        MatchState::Running => "running",
        MatchState::Ending => "ending",
    }
}

fn matches_text(infos: &[MatchInfo]) -> String {
    if infos.is_empty() {
        return "no matches".to_owned();
    }
    let lines: Vec<String> = infos
        .iter()
        .map(|info| {
            let alive = info.players.iter().filter(|player| player.alive).count();
            let world = if info.world.is_empty() {
                "no world yet"
            } else {
                info.world.as_str()
            };
            format!(
                "#{} {}/{} in {world}: {}, {alive}/{} alive",
                info.id,
                info.game_type,
                info.arena,
                match_state_name(info.state),
                info.players.len(),
            )
        })
        .collect();
    lines.join("\n")
}

fn list_spawns(store: &ArenaStore, game_type: &str, arena: &str) -> Result<String, String> {
    let found = store.arena(game_type, arena)?;
    if found.spawns.is_empty() {
        return Ok(format!("{game_type}/{arena} has no spawns"));
    }
    let mut text = format!("{game_type}/{arena} spawns:");
    for spawn in &found.spawns {
        let _ = write!(
            text,
            "\n  {}: {} yaw {:.1} pitch {:.1}",
            spawn.slot,
            position_text(spawn.x, spawn.y, spawn.z),
            spawn.yaw,
            spawn.pitch,
        );
    }
    Ok(text)
}

/// Converts an ABI result into a `Result` with an owned message.
macro_rules! api {
    ($value:expr) => {
        $value.match_owned(Ok, |error| Err(error.to_string()))
    };
}

/// The sending player's location; non-players get [`PLAYER_ONLY`].
fn sender_location(ctx: &Invocation<'_, '_>) -> Result<Location, String> {
    let player = ctx.player().map_err(|_| PLAYER_ONLY.to_owned())?;
    api!(ctx.server().player_location(player))
}

fn slot(ctx: &Invocation<'_, '_>) -> Result<u32, String> {
    u32::try_from(ctx.integer("slot")?).map_err(|_| "slot must be 1 or higher".to_owned())
}

/// Runs `change` on the shared store, saves, and replies.
fn mutate(
    ctx: &Invocation<'_, '_>,
    change: impl FnOnce(&mut ArenaStore) -> Result<String, String>,
) -> Result<i32, String> {
    let manager = ArenaManager::get()?;
    let reply = commit(&mut manager.store(), manager.arenas_path(), change)?;
    ctx.send(&reply);
    Ok(1)
}

fn read(
    ctx: &Invocation<'_, '_>,
    view: impl FnOnce(&ArenaStore) -> Result<String, String>,
) -> Result<i32, String> {
    let reply = view(&ArenaManager::get()?.store())?;
    ctx.send(&reply);
    Ok(1)
}

fn arena_names(ctx: &Invocation<'_, '_>) -> Result<(String, String), String> {
    Ok((ctx.string("gametype")?, ctx.string("arena")?))
}

fn spawn_set(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let here = sender_location(ctx)?;
    let (game_type, arena) = arena_names(ctx)?;
    let spawn = Spawn {
        slot: slot(ctx)?,
        x: here.x,
        y: here.y,
        z: here.z,
        yaw: here.yaw,
        pitch: here.pitch,
    };
    mutate(ctx, |store| set_spawn(store, &game_type, &arena, spawn))
}

fn lobby_set(ctx: &Invocation<'_, '_>, game_mode: LobbyGameMode) -> Result<i32, String> {
    let here = sender_location(ctx)?;
    let lobby = Lobby {
        world: here.world.to_string(),
        x: here.x,
        y: here.y,
        z: here.z,
        yaw: here.yaw,
        pitch: here.pitch,
        game_mode,
    };
    mutate(ctx, |store| Ok(set_lobby(store, lobby)))
}

fn game_type_arg() -> Node {
    argument("gametype", Arg::word())
}

fn arena_arg() -> Node {
    argument("arena", Arg::word())
}

fn slot_arg() -> Node {
    argument("slot", Arg::integer(1, i32::MAX))
}

pub fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    let mut lobby_set_node = literal("set").permission(PERMISSION_LOBBY);
    for (name, mode) in LOBBY_GAME_MODES {
        lobby_set_node =
            lobby_set_node.then(literal(name).executes(move |ctx| lobby_set(ctx, mode)));
    }
    let create = literal("create")
        .permission(PERMISSION_CREATE)
        .then(game_type_arg().then(arena_arg().executes(|ctx| {
            let (game_type, arena) = arena_names(ctx)?;
            mutate(ctx, |store| create_arena(store, &game_type, &arena))
        })));
    let remove = literal("remove")
        .permission(PERMISSION_REMOVE)
        .then(game_type_arg().then(arena_arg().executes(|ctx| {
            let (game_type, arena) = arena_names(ctx)?;
            let manager = ArenaManager::get()?;
            // Held until the change is saved, so a match cannot start in between.
            let runtime = manager.runtime();
            if runtime.arena_in_use(&game_type, &arena) {
                return Err(format!("arena {game_type}/{arena} has a running match"));
            }
            mutate(ctx, |store| remove_arena(store, &game_type, &arena))
        })));
    let spawn = literal("spawn")
        .then(
            literal("set")
                .permission(PERMISSION_SPAWN)
                .then(game_type_arg().then(arena_arg().then(slot_arg().executes(spawn_set)))),
        )
        .then(
            literal("remove")
                .permission(PERMISSION_SPAWN)
                .then(
                    game_type_arg().then(arena_arg().then(slot_arg().executes(|ctx| {
                        let (game_type, arena) = arena_names(ctx)?;
                        let slot = slot(ctx)?;
                        mutate(ctx, |store| remove_spawn(store, &game_type, &arena, slot))
                    }))),
                ),
        )
        .then(
            literal("list")
                .permission(PERMISSION_LIST)
                .then(game_type_arg().then(arena_arg().executes(|ctx| {
                    let (game_type, arena) = arena_names(ctx)?;
                    read(ctx, |store| list_spawns(store, &game_type, &arena))
                }))),
        );
    let template = literal("template")
        .permission(PERMISSION_TEMPLATE)
        .then(
            game_type_arg().then(argument("template", Arg::word()).executes(|ctx| {
                let game_type = ctx.string("gametype")?;
                let template = ctx.string("template")?;
                let server = ctx.server();
                let known = LiveHost { server: &server }.templates()?;
                if !known.iter().any(|info| info.name == template) {
                    return Err(format!("unknown template {template}"));
                }
                mutate(ctx, |store| set_template(store, &game_type, &template))
            })),
        );
    let matches = literal("matches")
        .permission(PERMISSION_MATCHES)
        .executes(|ctx| {
            let infos = ArenaManager::get()?.runtime().infos();
            ctx.send(&matches_text(&infos));
            Ok(1)
        });
    let list = literal("list")
        .permission(PERMISSION_LIST)
        .executes(|ctx| read(ctx, |store| Ok(list(store))));
    commands.register(
        CommandBuilder::new("arena:arena")
            .default_access()
            .then(create)
            .then(remove)
            .then(spawn)
            .then(template)
            .then(list)
            .then(matches)
            .then(literal("lobby").then(lobby_set_node)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir()
            .join(format!(
                "arena-manager-commands-{name}-{}",
                std::process::id()
            ))
            .join(ARENAS_FILE)
    }

    fn spawn(slot: u32) -> Spawn {
        Spawn {
            slot,
            x: 1.5,
            y: 64.0,
            z: -2.0,
            yaw: 90.0,
            pitch: 10.0,
        }
    }

    #[test]
    fn mutation_is_saved_at_once_and_reply_says_so() -> Result<(), String> {
        let path = path("commit");
        let mut store = ArenaStore::default();
        let reply = commit(&mut store, &path, |store| {
            create_arena(store, "skywars", "north")
        })?;
        assert_eq!(reply, "created arena skywars/north (saved to arenas.toml)");
        assert_eq!(ArenaStore::load(&path)?, store);
        Ok(())
    }

    #[test]
    fn failed_change_is_not_saved() {
        let path = path("failed");
        let mut store = ArenaStore::default();
        let error = commit(&mut store, &path, |store| {
            remove_arena(store, "skywars", "north")
        })
        .unwrap_err();
        assert_eq!(error, "unknown arena skywars/north");
        assert!(!path.exists());
    }

    #[test]
    fn save_failure_is_reported() -> Result<(), String> {
        let path = path("unwritable");
        let directory = path.parent().ok_or("no parent")?;
        fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        // A directory in place of the file makes the final rename fail.
        fs::create_dir_all(&path).map_err(|error| error.to_string())?;
        let mut store = ArenaStore::default();
        let error = commit(&mut store, &path, |store| {
            create_arena(store, "skywars", "north")
        })
        .unwrap_err();
        assert!(
            error.contains("not saved") && error.contains("arenas.toml"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn contract_error_messages() -> Result<(), String> {
        let mut store = ArenaStore::default();
        create_arena(&mut store, "skywars", "north")?;
        assert_eq!(
            create_arena(&mut store, "skywars", "north"),
            Err("arena north already exists for skywars".into())
        );
        let unknown = Err("unknown arena skywars/south".to_owned());
        assert_eq!(remove_arena(&mut store, "skywars", "south"), unknown);
        assert_eq!(set_spawn(&mut store, "skywars", "south", spawn(1)), unknown);
        assert_eq!(list_spawns(&store, "skywars", "south"), unknown);
        Ok(())
    }

    #[test]
    fn spawn_set_replaces_and_gap_leaves_arena_incomplete() -> Result<(), String> {
        let mut store = ArenaStore::default();
        create_arena(&mut store, "skywars", "north")?;
        set_spawn(&mut store, "skywars", "north", spawn(1))?;
        set_spawn(&mut store, "skywars", "north", spawn(3))?;
        assert!(list(&store).contains("north: slots 2, incomplete"));
        set_spawn(&mut store, "skywars", "north", spawn(2))?;
        set_spawn(&mut store, "skywars", "north", spawn(3))?;
        assert!(list(&store).contains("north: slots 3, complete"));
        remove_spawn(&mut store, "skywars", "north", 3)?;
        assert!(list(&store).contains("north: slots 2, complete"));
        Ok(())
    }

    #[test]
    fn listings() -> Result<(), String> {
        let mut store = ArenaStore::default();
        assert_eq!(list(&store), "no game types");
        create_arena(&mut store, "skywars", "north")?;
        assert_eq!(
            list(&store),
            "skywars (template: none)\n  north: slots 0, incomplete"
        );
        set_template(&mut store, "skywars", "skywars_a")?;
        assert!(list(&store).starts_with("skywars (template: skywars_a)"));
        assert_eq!(
            list_spawns(&store, "skywars", "north")?,
            "skywars/north has no spawns"
        );
        set_spawn(&mut store, "skywars", "north", spawn(1))?;
        assert_eq!(
            list_spawns(&store, "skywars", "north")?,
            "skywars/north spawns:\n  1: 1.50 64.00 -2.00 yaw 90.0 pitch 10.0"
        );
        assert_eq!(
            set_template(&mut store, "spleef", "pit"),
            Err("unknown game type spleef".into())
        );
        Ok(())
    }

    #[test]
    fn lobby_set_reports_world_and_game_mode() {
        let mut store = ArenaStore::default();
        let reply = set_lobby(
            &mut store,
            Lobby {
                world: "lobby:main".into(),
                x: 0.5,
                y: 64.0,
                z: 0.5,
                yaw: 0.0,
                pitch: 0.0,
                game_mode: LobbyGameMode::Adventure,
            },
        );
        assert_eq!(
            reply,
            "lobby set to 0.50 64.00 0.50 in lobby:main with game mode adventure"
        );
        assert!(store.lobby.is_some());
    }

    #[test]
    fn matches_listing_shows_state_world_and_alive_players() {
        use steel_extension_api::players::Uuid;
        let player = |alive| arena_manager_api::MatchPlayer {
            uuid: Uuid::default(),
            alive,
        };
        let info = |world: &str| MatchInfo {
            id: 7,
            game_type: "duel".into(),
            arena: "a".into(),
            world: world.into(),
            state: MatchState::Running,
            players: [player(true), player(false)].into_iter().collect(),
        };
        assert_eq!(matches_text(&[]), "no matches");
        assert_eq!(
            matches_text(&[info("arena0:map")]),
            "#7 duel/a in arena0:map: running, 1/2 alive"
        );
        assert!(matches_text(&[info("")]).contains("no world yet"));
    }
}
