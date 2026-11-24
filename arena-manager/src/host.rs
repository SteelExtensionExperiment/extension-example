//! The server calls arena-manager makes, behind one trait so the tick logic runs without a host.
//!
//! [`LiveHost`] forwards to `Server` and the `world-cloner` service. The test fake lives in
//! [`fake`].
use steel_extension_api::{
    GameMode, Operation, OperationState, PlayerId, Server, TemplateState,
    events::Location,
    inventory_snapshot::InventorySnapshot,
    players::Uuid,
    stabby::{str::Str, string::String as ApiString},
};
use world_cloner_api::{CloneRequest, CloneSettings, CloneTarget, WorldClonerDyn as _};

/// State of a world operation or domain destruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationStatus {
    Pending,
    Ready,
    Failed(String),
}

/// A configured world-cloner template and its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateInfo {
    pub name: String,
    pub state: TemplateState,
    pub message: String,
}

/// Server access used by the match runtime. Every method returns at once; nothing blocks.
pub trait Host {
    /// Handle of an asynchronous world or domain operation.
    type Operation: Copy + std::fmt::Debug;
    /// Runtime identity of a player.
    type Player: Copy + PartialEq + std::fmt::Debug;

    fn templates(&self) -> Result<Vec<TemplateInfo>, String>;
    /// Clones `template` as `key` into a new temporary domain named by the key's namespace.
    fn clone_world(&self, template: &str, key: &str) -> Result<Self::Operation, String>;
    fn remove_world(&self, key: &str) -> Result<Self::Operation, String>;
    fn destroy_temporary_domain(&self, domain: &str) -> Result<Self::Operation, String>;
    /// Reads an operation. A completed status consumes the result: read it once.
    fn operation_status(&self, operation: Self::Operation) -> Result<OperationStatus, String>;

    fn player_by_uuid(&self, uuid: Uuid) -> Option<Self::Player>;
    fn player_uuid(&self, player: Self::Player) -> Uuid;
    fn player_name(&self, player: Self::Player) -> Result<String, String>;
    fn is_connected(&self, player: Self::Player) -> bool;
    /// Finished relocation and client loading.
    fn player_ready(&self, player: Self::Player) -> bool;
    /// Key of the world the player is in.
    fn player_world(&self, player: Self::Player) -> Result<String, String>;
    fn switch_domain(&self, player: Self::Player, domain: &str) -> Result<(), String>;
    fn select_world(&self, player: Self::Player, key: &str) -> Result<(), String>;
    fn teleport_to(&self, player: Self::Player, destination: &Location) -> Result<(), String>;
    /// Applies the kit's inventory and game mode.
    fn restore_inventory(
        &self,
        player: Self::Player,
        kit: &InventorySnapshot,
    ) -> Result<(), String>;
    fn set_game_mode(&self, player: Self::Player, mode: GameMode) -> Result<(), String>;
    fn send_action_bar(&self, player: Self::Player, text: &str) -> Result<(), String>;
}

/// Turns a host `ApiResult` into a `Result` with an owned message.
macro_rules! api {
    ($value:expr) => {
        $value.match_owned(Ok, |error| Err(error.to_string()))
    };
}

/// [`Host`] backed by the running server and the `world-cloner` service.
pub struct LiveHost<'a, 'b> {
    pub server: &'a Server<'b>,
}

impl Host for LiveHost<'_, '_> {
    type Operation = Operation;
    type Player = PlayerId;

    fn templates(&self) -> Result<Vec<TemplateInfo>, String> {
        let cloner = world_cloner_api::cloner()?;
        Ok(cloner
            .templates(*self.server)
            .iter()
            .map(|info| TemplateInfo {
                name: info.name.to_string(),
                state: info.state,
                message: info.message.to_string(),
            })
            .collect())
    }

    fn clone_world(&self, template: &str, key: &str) -> Result<Operation, String> {
        let request = CloneRequest {
            template: ApiString::from(template),
            key: ApiString::from(key),
            target: CloneTarget::NewTemporaryDomain,
            settings: CloneSettings {
                seed: None.into(),
                spawn: None.into(),
                default_game_mode: None.into(),
                difficulty: None.into(),
            },
        };
        api!(world_cloner_api::cloner()?.clone_world(*self.server, request))
    }

    fn remove_world(&self, key: &str) -> Result<Operation, String> {
        api!(world_cloner_api::cloner()?.remove_world(*self.server, Str::new(key)))
    }

    fn destroy_temporary_domain(&self, domain: &str) -> Result<Operation, String> {
        api!(self.server.destroy_temporary_domain(Str::new(domain)))
    }

    fn operation_status(&self, operation: Operation) -> Result<OperationStatus, String> {
        let status = api!(self.server.operation_status(operation))?;
        Ok(match status.state {
            OperationState::Pending => OperationStatus::Pending,
            OperationState::Ready => OperationStatus::Ready,
            OperationState::Failed => OperationStatus::Failed(status.value.to_string()),
        })
    }

    fn player_uuid(&self, player: PlayerId) -> Uuid {
        player.uuid()
    }

    fn player_by_uuid(&self, uuid: Uuid) -> Option<PlayerId> {
        self.server.player_by_uuid(uuid).into()
    }

    fn player_name(&self, player: Self::Player) -> Result<String, String> {
        api!(self.server.player_name(player)).map(|name| name.to_string())
    }

    fn is_connected(&self, player: Self::Player) -> bool {
        self.server.is_connected(player)
    }

    fn player_ready(&self, player: Self::Player) -> bool {
        self.server.player_ready(player)
    }

    fn player_world(&self, player: Self::Player) -> Result<String, String> {
        api!(self.server.player_location(player)).map(|location| location.world.to_string())
    }

    fn switch_domain(&self, player: Self::Player, domain: &str) -> Result<(), String> {
        api!(self.server.switch_domain(player, Str::new(domain)))
    }

    fn select_world(&self, player: Self::Player, key: &str) -> Result<(), String> {
        api!(self.server.select_world(player, Str::new(key)))
    }

    fn teleport_to(&self, player: Self::Player, destination: &Location) -> Result<(), String> {
        api!(self.server.teleport_to(player, destination.clone()))
    }

    fn restore_inventory(
        &self,
        player: Self::Player,
        kit: &InventorySnapshot,
    ) -> Result<(), String> {
        api!(self.server.restore_inventory(player, kit))
    }

    fn set_game_mode(&self, player: Self::Player, mode: GameMode) -> Result<(), String> {
        api!(self.server.set_game_mode(player, mode))
    }

    fn send_action_bar(&self, player: Self::Player, text: &str) -> Result<(), String> {
        api!(self.server.send_action_bar(player, Str::new(text)))
    }
}

/// Namespace (domain) of a world key `domain:world`.
pub fn domain_of(world_key: &str) -> &str {
    world_key
        .split_once(':')
        .map_or(world_key, |(domain, _)| domain)
}

#[cfg(test)]
pub mod fake {
    //! Scriptable [`Host`] for unit tests. Calls are recorded as `name(args)` strings.
    use super::{Host, OperationStatus, TemplateInfo};
    use std::{
        cell::RefCell,
        collections::{HashMap, HashSet},
    };
    use steel_extension_api::{
        GameMode, TemplateState, events::Location, inventory_snapshot::InventorySnapshot,
        players::Uuid,
    };

    /// Test player: the first byte of its UUID.
    pub type Player = u8;
    /// Test operation handle.
    pub type Operation = u32;

    #[derive(Default)]
    pub struct State {
        next_operation: u32,
        /// Status `operation_status` reports; a terminal status is consumed by the first read.
        pub operations: HashMap<Operation, OperationStatus>,
        pub calls: Vec<String>,
        /// Method name to the error it returns at once.
        pub failing: HashMap<&'static str, String>,
        pub templates: Vec<TemplateInfo>,
        pub online: HashSet<Player>,
        pub ready: HashSet<Player>,
        pub worlds: HashMap<Player, String>,
        pub names: HashMap<Player, String>,
    }

    #[derive(Default)]
    pub struct FakeHost {
        pub state: RefCell<State>,
    }

    impl FakeHost {
        /// An online, ready player standing in `world`.
        pub fn add_player(&self, player: Player, world: &str) {
            let mut state = self.state.borrow_mut();
            state.online.insert(player);
            state.ready.insert(player);
            state.worlds.insert(player, world.to_owned());
            state.names.insert(player, format!("player{player}"));
        }

        pub fn add_template(&self, name: &str, state: TemplateState) {
            self.state.borrow_mut().templates.push(TemplateInfo {
                name: name.to_owned(),
                state,
                message: String::new(),
            });
        }

        pub fn finish(&self, operation: Operation, status: OperationStatus) {
            self.state.borrow_mut().operations.insert(operation, status);
        }

        pub fn fail(&self, method: &'static str, message: &str) {
            self.state
                .borrow_mut()
                .failing
                .insert(method, message.to_owned());
        }

        pub fn calls(&self) -> Vec<String> {
            self.state.borrow().calls.clone()
        }

        fn record(&self, method: &'static str, args: &str) -> Result<(), String> {
            let mut state = self.state.borrow_mut();
            state.calls.push(format!("{method}({args})"));
            state.failing.get(method).map_or(Ok(()), |m| Err(m.clone()))
        }

        fn start_operation(&self, method: &'static str, args: &str) -> Result<Operation, String> {
            self.record(method, args)?;
            let mut state = self.state.borrow_mut();
            state.next_operation += 1;
            let operation = state.next_operation;
            state.operations.insert(operation, OperationStatus::Pending);
            Ok(operation)
        }
    }

    impl Host for FakeHost {
        type Operation = Operation;
        type Player = Player;

        fn templates(&self) -> Result<Vec<TemplateInfo>, String> {
            self.record("templates", "")?;
            Ok(self.state.borrow().templates.clone())
        }

        fn clone_world(&self, template: &str, key: &str) -> Result<Operation, String> {
            self.start_operation("clone_world", &format!("{template},{key}"))
        }

        fn remove_world(&self, key: &str) -> Result<Operation, String> {
            self.start_operation("remove_world", key)
        }

        fn destroy_temporary_domain(&self, domain: &str) -> Result<Operation, String> {
            self.start_operation("destroy_temporary_domain", domain)
        }

        fn operation_status(&self, operation: Operation) -> Result<OperationStatus, String> {
            let mut state = self.state.borrow_mut();
            let status = state
                .operations
                .get(&operation)
                .cloned()
                .ok_or("unknown operation")?;
            if status != OperationStatus::Pending {
                state.operations.remove(&operation);
            }
            Ok(status)
        }

        fn player_by_uuid(&self, uuid: Uuid) -> Option<Player> {
            let player = uuid.bytes[0];
            self.state
                .borrow()
                .online
                .contains(&player)
                .then_some(player)
        }

        fn player_uuid(&self, player: Player) -> Uuid {
            let mut bytes = [0; 16];
            bytes[0] = player;
            Uuid { bytes }
        }

        fn player_name(&self, player: Player) -> Result<String, String> {
            self.state
                .borrow()
                .names
                .get(&player)
                .cloned()
                .ok_or_else(|| "unknown player".to_owned())
        }

        fn is_connected(&self, player: Player) -> bool {
            self.state.borrow().online.contains(&player)
        }

        fn player_ready(&self, player: Player) -> bool {
            self.state.borrow().ready.contains(&player)
        }

        fn player_world(&self, player: Player) -> Result<String, String> {
            self.state
                .borrow()
                .worlds
                .get(&player)
                .cloned()
                .ok_or_else(|| "unknown player".to_owned())
        }

        /// Moves the player to `<domain>:default` and marks it not ready.
        fn switch_domain(&self, player: Player, domain: &str) -> Result<(), String> {
            self.record("switch_domain", &format!("{player},{domain}"))?;
            let mut state = self.state.borrow_mut();
            state.ready.remove(&player);
            state.worlds.insert(player, format!("{domain}:default"));
            Ok(())
        }

        fn select_world(&self, player: Player, key: &str) -> Result<(), String> {
            self.record("select_world", &format!("{player},{key}"))?;
            let mut state = self.state.borrow_mut();
            state.ready.remove(&player);
            state.worlds.insert(player, key.to_owned());
            Ok(())
        }

        fn teleport_to(&self, player: Player, destination: &Location) -> Result<(), String> {
            self.record("teleport_to", &format!("{player},{}", destination.world))?;
            self.state
                .borrow_mut()
                .worlds
                .insert(player, destination.world.to_string());
            Ok(())
        }

        fn restore_inventory(
            &self,
            player: Player,
            _kit: &InventorySnapshot,
        ) -> Result<(), String> {
            self.record("restore_inventory", &player.to_string())
        }

        fn set_game_mode(&self, player: Player, mode: GameMode) -> Result<(), String> {
            self.record("set_game_mode", &format!("{player},{mode:?}"))
        }

        fn send_action_bar(&self, player: Player, text: &str) -> Result<(), String> {
            self.record("send_action_bar", &format!("{player},{text}"))
        }
    }
}
