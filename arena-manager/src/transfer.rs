//! Tick-driven step machines that move one player into a match world and back to the lobby.
//!
//! Relocation is queued by the host, so each machine is advanced once per server tick and never
//! blocks. A call does at most one transition. The caller owns the timeout and passes it in
//! ticks (`player_ready_timeout_seconds` times the ticks per second).
use crate::{
    host::{Host, domain_of},
    store::{Lobby, Spawn},
};
use steel_extension_api::{
    GameMode, events::Location, inventory_snapshot::InventorySnapshot,
    stabby::string::String as ApiString,
};

/// Result of one [`Arrival::advance`] or [`LobbyReturn::advance`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    Pending,
    Done,
    Failed(String),
}

/// Player name for messages, falling back to the identity when the host cannot resolve it.
fn display_name<H: Host>(host: &H, player: H::Player) -> String {
    host.player_name(player)
        .unwrap_or_else(|_| format!("{player:?}"))
}

/// Counts one tick of waiting; true once `timeout_ticks` ticks have passed.
fn timed_out(waited: &mut u32, timeout_ticks: u32) -> bool {
    *waited += 1;
    *waited >= timeout_ticks
}

#[derive(Debug)]
enum ArrivalStep {
    Switch,
    WaitReady,
}

/// Moves a player into a match world: switch domain, wait until ready, teleport, restore the kit.
#[derive(Debug)]
pub struct Arrival<P> {
    player: P,
    world_key: String,
    spawn: Spawn,
    kit: InventorySnapshot,
    step: ArrivalStep,
    waited: u32,
}

impl<P: Copy> Arrival<P> {
    pub fn new(player: P, world_key: String, spawn: Spawn, kit: InventorySnapshot) -> Self {
        Self {
            player,
            world_key,
            spawn,
            kit,
            step: ArrivalStep::Switch,
            waited: 0,
        }
    }

    pub fn advance<H: Host<Player = P>>(&mut self, host: &H, timeout_ticks: u32) -> Progress {
        if !host.is_connected(self.player) {
            return Progress::Failed(format!(
                "{} left before the match started",
                display_name(host, self.player)
            ));
        }
        match self.step {
            ArrivalStep::Switch => self.switch(host),
            ArrivalStep::WaitReady => self.wait_ready(host, timeout_ticks),
        }
    }

    fn switch<H: Host<Player = P>>(&mut self, host: &H) -> Progress {
        let domain = domain_of(&self.world_key);
        let current = match host.player_world(self.player) {
            Ok(world) => world,
            Err(error) => return Progress::Failed(error),
        };
        if domain_of(&current) != domain
            && let Err(error) = host.switch_domain(self.player, domain)
        {
            return Progress::Failed(error);
        }
        self.step = ArrivalStep::WaitReady;
        Progress::Pending
    }

    fn wait_ready<H: Host<Player = P>>(&mut self, host: &H, timeout_ticks: u32) -> Progress {
        if !host.player_ready(self.player) {
            if timed_out(&mut self.waited, timeout_ticks) {
                return Progress::Failed(format!(
                    "{} was not ready after {timeout_ticks} ticks",
                    display_name(host, self.player)
                ));
            }
            return Progress::Pending;
        }
        let destination = Location {
            world: ApiString::from(self.world_key.as_str()),
            x: self.spawn.x,
            y: self.spawn.y,
            z: self.spawn.z,
            yaw: self.spawn.yaw,
            pitch: self.spawn.pitch,
        };
        let result = host
            .teleport_to(self.player, &destination)
            .and_then(|()| host.restore_inventory(self.player, &self.kit));
        match result {
            Ok(()) => Progress::Done,
            Err(error) => Progress::Failed(error),
        }
    }
}

#[derive(Debug)]
enum LobbyStep {
    SelectWorld { last_error: String },
    WaitReady,
}

/// Moves a player to the lobby: select the lobby world, wait until ready, teleport, set the
/// game mode. The caller starts it only for players that are alive.
#[derive(Debug)]
pub struct LobbyReturn<P> {
    player: P,
    world: String,
    destination: Location,
    game_mode: GameMode,
    step: LobbyStep,
    waited: u32,
}

impl<P: Copy> LobbyReturn<P> {
    pub fn new(player: P, lobby: &Lobby) -> Self {
        Self {
            player,
            world: lobby.world.clone(),
            destination: Location {
                world: ApiString::from(lobby.world.as_str()),
                x: lobby.x,
                y: lobby.y,
                z: lobby.z,
                yaw: lobby.yaw,
                pitch: lobby.pitch,
            },
            game_mode: lobby.game_mode.into(),
            step: LobbyStep::SelectWorld {
                last_error: String::new(),
            },
            waited: 0,
        }
    }

    pub fn advance<H: Host<Player = P>>(&mut self, host: &H, timeout_ticks: u32) -> Progress {
        if !host.is_connected(self.player) {
            return Progress::Done;
        }
        match &mut self.step {
            LobbyStep::SelectWorld { last_error } => {
                // The player may still be mid-relocation, so an error is retried until timeout.
                let Err(error) = host.select_world(self.player, &self.world) else {
                    self.step = LobbyStep::WaitReady;
                    self.waited = 0;
                    return Progress::Pending;
                };
                *last_error = error;
                if timed_out(&mut self.waited, timeout_ticks) {
                    return Progress::Failed(last_error.clone());
                }
                Progress::Pending
            }
            LobbyStep::WaitReady => self.wait_ready(host, timeout_ticks),
        }
    }

    fn wait_ready<H: Host<Player = P>>(&mut self, host: &H, timeout_ticks: u32) -> Progress {
        if !host.player_ready(self.player) {
            if timed_out(&mut self.waited, timeout_ticks) {
                return Progress::Failed(format!(
                    "{} was not ready after {timeout_ticks} ticks",
                    display_name(host, self.player)
                ));
            }
            return Progress::Pending;
        }
        let result = host
            .teleport_to(self.player, &self.destination)
            .and_then(|()| host.set_game_mode(self.player, self.game_mode));
        match result {
            Ok(()) => Progress::Done,
            Err(error) => Progress::Failed(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{host::fake::FakeHost, store::LobbyGameMode};

    const PLAYER: u8 = 1;
    const TIMEOUT: u32 = 3;
    const KIT_FORMAT: u32 = 1;

    fn arrival() -> Arrival<u8> {
        let spawn = Spawn {
            slot: 1,
            x: 1.0,
            y: 64.0,
            z: 2.0,
            yaw: 90.0,
            pitch: 0.0,
        };
        let kit = InventorySnapshot {
            format: KIT_FORMAT,
            data: steel_extension_api::stabby::vec::Vec::default(),
        };
        Arrival::new(PLAYER, "match1:map".to_owned(), spawn, kit)
    }

    fn lobby() -> Lobby {
        Lobby {
            world: "lobby:main".to_owned(),
            x: 0.5,
            y: 70.0,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
            game_mode: LobbyGameMode::Adventure,
        }
    }

    #[test]
    fn arrival_in_same_domain_skips_switch() {
        let host = FakeHost::default();
        host.add_player(PLAYER, "match1:other");
        let mut arrival = arrival();
        assert_eq!(arrival.advance(&host, TIMEOUT), Progress::Pending);
        assert_eq!(arrival.advance(&host, TIMEOUT), Progress::Done);
        assert_eq!(
            host.calls(),
            ["teleport_to(1,match1:map)", "restore_inventory(1)"]
        );
    }

    #[test]
    fn arrival_switches_waits_then_teleports_and_restores() {
        let host = FakeHost::default();
        host.add_player(PLAYER, "lobby:main");
        let mut arrival = arrival();
        assert_eq!(arrival.advance(&host, TIMEOUT), Progress::Pending);
        assert_eq!(arrival.advance(&host, TIMEOUT), Progress::Pending);
        assert_eq!(host.calls(), ["switch_domain(1,match1)"]);
        host.state.borrow_mut().ready.insert(PLAYER);
        assert_eq!(arrival.advance(&host, TIMEOUT), Progress::Done);
        assert_eq!(
            host.calls(),
            [
                "switch_domain(1,match1)",
                "teleport_to(1,match1:map)",
                "restore_inventory(1)"
            ]
        );
    }

    #[test]
    fn arrival_times_out_naming_the_player() {
        let host = FakeHost::default();
        host.add_player(PLAYER, "lobby:main");
        let mut arrival = arrival();
        assert_eq!(arrival.advance(&host, TIMEOUT), Progress::Pending);
        assert_eq!(arrival.advance(&host, TIMEOUT), Progress::Pending);
        assert_eq!(arrival.advance(&host, TIMEOUT), Progress::Pending);
        let Progress::Failed(message) = arrival.advance(&host, TIMEOUT) else {
            panic!("expected a timeout");
        };
        assert!(message.contains("player1"), "{message}");
    }

    #[test]
    fn arrival_fails_when_player_disconnects_while_waiting() {
        let host = FakeHost::default();
        host.add_player(PLAYER, "lobby:main");
        let mut arrival = arrival();
        arrival.advance(&host, TIMEOUT);
        host.state.borrow_mut().online.remove(&PLAYER);
        assert_eq!(
            arrival.advance(&host, TIMEOUT),
            Progress::Failed("player1 left before the match started".to_owned())
        );
    }

    #[test]
    fn arrival_fails_on_teleport_error_without_restoring() {
        let host = FakeHost::default();
        host.add_player(PLAYER, "match1:other");
        host.fail("teleport_to", "chunk not loaded");
        let mut arrival = arrival();
        arrival.advance(&host, TIMEOUT);
        assert_eq!(
            arrival.advance(&host, TIMEOUT),
            Progress::Failed("chunk not loaded".to_owned())
        );
        assert_eq!(host.calls(), ["teleport_to(1,match1:map)"]);
    }

    #[test]
    fn lobby_return_retries_select_world_after_error() {
        let host = FakeHost::default();
        host.add_player(PLAYER, "match1:map");
        host.fail("select_world", "still relocating");
        let mut back = LobbyReturn::new(PLAYER, &lobby());
        assert_eq!(back.advance(&host, TIMEOUT), Progress::Pending);
        host.state.borrow_mut().failing.clear();
        assert_eq!(back.advance(&host, TIMEOUT), Progress::Pending);
        host.state.borrow_mut().ready.insert(PLAYER);
        assert_eq!(back.advance(&host, TIMEOUT), Progress::Done);
        assert_eq!(
            host.calls(),
            [
                "select_world(1,lobby:main)",
                "select_world(1,lobby:main)",
                "teleport_to(1,lobby:main)",
                "set_game_mode(1,Adventure)"
            ]
        );
    }

    #[test]
    fn lobby_return_gives_up_with_last_error() {
        let host = FakeHost::default();
        host.add_player(PLAYER, "match1:map");
        host.fail("select_world", "still relocating");
        let mut back = LobbyReturn::new(PLAYER, &lobby());
        assert_eq!(back.advance(&host, 2), Progress::Pending);
        assert_eq!(
            back.advance(&host, 2),
            Progress::Failed("still relocating".to_owned())
        );
    }

    #[test]
    fn lobby_return_of_offline_player_is_done_without_calls() {
        let host = FakeHost::default();
        let mut back = LobbyReturn::new(PLAYER, &lobby());
        assert_eq!(back.advance(&host, TIMEOUT), Progress::Done);
        assert_eq!(host.calls().len(), 0);
    }

    #[test]
    fn lobby_return_waits_for_ready_before_teleport() {
        let host = FakeHost::default();
        host.add_player(PLAYER, "match1:map");
        let mut back = LobbyReturn::new(PLAYER, &lobby());
        assert_eq!(back.advance(&host, TIMEOUT), Progress::Pending);
        assert_eq!(back.advance(&host, TIMEOUT), Progress::Pending);
        assert_eq!(host.calls(), ["select_world(1,lobby:main)"]);
        host.state.borrow_mut().ready.insert(PLAYER);
        assert_eq!(back.advance(&host, TIMEOUT), Progress::Done);
        assert_eq!(
            host.calls(),
            [
                "select_world(1,lobby:main)",
                "teleport_to(1,lobby:main)",
                "set_game_mode(1,Adventure)"
            ]
        );
    }
}
