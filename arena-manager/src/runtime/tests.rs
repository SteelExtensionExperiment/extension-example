use super::*;
use crate::{
    host::{OperationStatus, fake::FakeHost},
    store::{Lobby, LobbyGameMode},
};
use arena_manager_api::Outcome;
use steel_extension_api::TemplateState;

type TestRuntime = Runtime<u8, u32>;

const GAME: &str = "duel";
const TEMPLATE: &str = "duel_map";
const ARENA: &str = "a";
const LOBBY_WORLD: &str = "lobby:main";
const FIRST_WORLD: &str = "arena0:map";

fn store() -> ArenaStore {
    let mut store = ArenaStore::default();
    store.create_arena(GAME, ARENA).unwrap();
    store.set_template(GAME, TEMPLATE).unwrap();
    for slot in 1..=2 {
        let spawn = crate::store::Spawn {
            slot,
            x: f64::from(slot),
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        store.set_spawn(GAME, ARENA, spawn).unwrap();
    }
    store.set_lobby(Lobby {
        world: LOBBY_WORLD.to_owned(),
        x: 0.5,
        y: 70.0,
        z: 0.5,
        yaw: 0.0,
        pitch: 0.0,
        game_mode: LobbyGameMode::Adventure,
    });
    store
}

fn config(max_worlds: usize) -> Config {
    Config {
        max_worlds,
        winner_return_delay_seconds: 1,
        ..Config::default()
    }
}

fn host() -> FakeHost {
    let host = FakeHost::default();
    host.add_template(TEMPLATE, TemplateState::Ready);
    for player in 1..=4 {
        host.add_player(player, LOBBY_WORLD);
    }
    host
}

fn uuid_bytes(player: u8) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[0] = player;
    bytes
}

fn request(_host: &FakeHost, players: &[u8]) -> Request<'static> {
    Request {
        game_type: GAME,
        players: players
            .iter()
            .map(|&player| {
                let kit = InventorySnapshot {
                    format: 1,
                    data: steel_extension_api::stabby::vec::Vec::new(),
                };
                (
                    Uuid {
                        bytes: uuid_bytes(player),
                    },
                    kit,
                )
            })
            .collect(),
    }
}

fn ctx<'a>(store: &'a ArenaStore, config: &'a Config) -> Context<'a> {
    Context { store, config }
}

fn ready_all(host: &FakeHost) {
    host.state.borrow_mut().ready.extend(1..=4);
}

/// Starts a two-player match in a fresh world and ticks until it is running.
fn running_match(
    runtime: &mut TestRuntime,
    host: &FakeHost,
    store: &ArenaStore,
    config: &Config,
) -> MatchId {
    let id = runtime
        .start_match(host, &ctx(store, config), &request(host, &[1, 2]))
        .unwrap();
    host.finish(1, OperationStatus::Ready);
    runtime.tick(host, &ctx(store, config), vec![]);
    ready_all(host);
    runtime.tick(host, &ctx(store, config), vec![]);
    assert_eq!(runtime.infos()[0].state, MatchState::Running);
    id
}

#[test]
fn full_match_returns_players_and_reports_one_result() {
    let (store, config, host) = (store(), config(1), host());
    let mut runtime = TestRuntime::default();
    let id = running_match(&mut runtime, &host, &store, &config);
    assert!(
        host.calls()
            .contains(&format!("clone_world({TEMPLATE},{FIRST_WORLD})"))
    );

    let emissions = runtime.tick(&host, &ctx(&store, &config), vec![Incoming::Death(1)]);
    assert!(emissions.is_empty());
    assert!(
        host.calls()
            .contains(&"send_action_bar(2,Returning to the lobby in 1)".to_owned())
    );
    runtime.tick(&host, &ctx(&store, &config), vec![Incoming::Respawn(1)]);
    assert!(
        host.calls()
            .contains(&format!("select_world(1,{LOBBY_WORLD})"))
    );

    let mut emitted = Vec::new();
    for _ in 0..TICKS_PER_SECOND {
        emitted.extend(runtime.tick(&host, &ctx(&store, &config), vec![]));
    }
    let [Emission::Finished(result)] = &emitted[..] else {
        panic!("expected one result, got {emitted:?}");
    };
    let outcomes: Vec<_> = result.participants.iter().map(|p| p.outcome).collect();
    assert_eq!(result.id, id);
    assert_eq!(outcomes, [Outcome::Lost, Outcome::Won]);
    assert!(runtime.infos().is_empty());
    assert!(
        host.calls()
            .contains(&format!("select_world(2,{LOBBY_WORLD})"))
    );
}

#[test]
fn idle_world_is_removed_after_the_match() {
    let (store, config, host) = (store(), config(1), host());
    let mut runtime = TestRuntime::default();
    running_match(&mut runtime, &host, &store, &config);
    runtime.tick(&host, &ctx(&store, &config), vec![Incoming::Death(1)]);
    for _ in 0..(TICKS_PER_SECOND * 2) {
        runtime.tick(&host, &ctx(&store, &config), vec![]);
    }
    assert!(
        host.calls()
            .contains(&format!("remove_world({FIRST_WORLD})"))
    );
}

#[test]
fn clone_failure_reports_the_match_as_failed_and_frees_the_cap() {
    let (store, config, host) = (store(), config(1), host());
    let mut runtime = TestRuntime::default();
    runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 2]))
        .unwrap();
    host.finish(1, OperationStatus::Failed("out of disk".to_owned()));
    let emissions = runtime.tick(&host, &ctx(&store, &config), vec![]);
    let [Emission::Failed(failed)] = &emissions[..] else {
        panic!("expected one failure, got {emissions:?}");
    };
    assert!(failed.message.to_string().contains("out of disk"));
    assert!(runtime.infos().is_empty());
    assert!(
        runtime
            .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 2]))
            .is_ok()
    );
}

#[test]
fn request_beyond_the_world_limit_is_rejected() {
    let (store, config, host) = (store(), config(1), host());
    let mut runtime = TestRuntime::default();
    running_match(&mut runtime, &host, &store, &config);
    let error = runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[3, 4]))
        .unwrap_err();
    assert_eq!(error, "no capacity");
}

#[test]
fn death_before_the_match_runs_cancels_it() {
    let (store, config, host) = (store(), config(1), host());
    let mut runtime = TestRuntime::default();
    runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 2]))
        .unwrap();
    let emissions = runtime.tick(&host, &ctx(&store, &config), vec![Incoming::Quit(2)]);
    assert!(matches!(&emissions[..], [Emission::Failed(_)]));
}

#[test]
fn requests_are_rejected_with_a_cause() {
    let (store, config, host) = (store(), config(4), host());
    let mut runtime = TestRuntime::default();

    let mut no_lobby = store.clone();
    no_lobby.lobby = None;
    let error = runtime
        .start_match(&host, &ctx(&no_lobby, &config), &request(&host, &[1, 2]))
        .unwrap_err();
    assert!(error.contains("lobby"), "{error}");

    let error = runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 9]))
        .unwrap_err();
    assert!(error.contains("not online"), "{error}");

    let error = runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 1]))
        .unwrap_err();
    assert!(error.contains("twice"), "{error}");

    host.state.borrow_mut().ready.remove(&2);
    let error = runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 2]))
        .unwrap_err();
    assert!(error.contains("not ready"), "{error}");
    host.state.borrow_mut().ready.insert(2);

    let error = runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 2, 3]))
        .unwrap_err();
    assert!(error.contains("no complete arena"), "{error}");

    runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 2]))
        .unwrap();
    let error = runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[2, 3]))
        .unwrap_err();
    assert!(error.contains("already in a match"), "{error}");
}

#[test]
fn template_must_be_ready() {
    let (store, config) = (store(), config(4));
    let mut runtime = TestRuntime::default();
    let host = FakeHost::default();
    for player in 1..=2 {
        host.add_player(player, LOBBY_WORLD);
    }
    let error = runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 2]))
        .unwrap_err();
    assert!(error.contains("unknown template"), "{error}");
    host.add_template(TEMPLATE, TemplateState::Loading);
    let error = runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 2]))
        .unwrap_err();
    assert!(error.contains("still loading"), "{error}");
    assert!(runtime.infos().is_empty());
}

#[test]
fn an_arena_with_a_match_is_in_use() {
    let (store, config, host) = (store(), config(1), host());
    let mut runtime = TestRuntime::default();
    assert!(!runtime.arena_in_use(GAME, ARENA));
    runtime
        .start_match(&host, &ctx(&store, &config), &request(&host, &[1, 2]))
        .unwrap();
    assert!(runtime.arena_in_use(GAME, ARENA));
    assert!(!runtime.arena_in_use(GAME, "other"));
}
