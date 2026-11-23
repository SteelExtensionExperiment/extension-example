mod member;

use std::collections::HashSet;

use member::Human;
use minigame_api::{Member, Room, RoomDefinition, RoomPhase, RoomUpdate, WorldDefinition};
use steel_extension_api::{
    ApiResult, Operation, OperationState, PlayerId, PlayerInfo, Position, Server,
    stabby::{self, slice::Slice, str::Str, string::String as ApiString, vec::Vec as ApiVec},
};

#[derive(Clone, Copy)]
enum State {
    Idle,
    PreparingLobby(Operation),
    Lobby,
    PreparingArena(Operation),
    Arena,
    Returning(Option<Operation>),
    Retiring(Operation),
    Closed,
}

pub(super) struct GameRoom {
    definition: RoomDefinition,
    domain: String,
    lobby: String,
    arena: String,
    state: State,
    domain_created: bool,
    members: Vec<Human>,
    bots: Vec<PlayerId>,
    arena_transfers: HashSet<PlayerId>,
}

impl GameRoom {
    pub(super) fn new(domain: String, definition: RoomDefinition) -> Self {
        Self {
            lobby: format!("{domain}:lobby"),
            arena: format!("{domain}:arena"),
            domain,
            definition,
            state: State::Idle,
            domain_created: false,
            members: Vec::new(),
            bots: Vec::new(),
            arena_transfers: HashSet::new(),
        }
    }

    fn join_player(&mut self, server: &Server<'_>, player: PlayerId) -> Result<(), ApiString> {
        if self.members.iter().any(|member| member.player == player) {
            return Err("You are already in this room".into());
        }
        if !matches!(
            self.state,
            State::Idle | State::PreparingLobby(_) | State::Lobby
        ) {
            return Err("This room is no longer accepting players".into());
        }
        if self.members.len() >= self.definition.capacity as usize {
            return Err("This room is full".into());
        }
        let players = server.players();
        let current = players
            .iter()
            .find(|info| info.id == player)
            .ok_or_else(|| ApiString::from("Player is changing worlds; try again once loaded"))?;
        let home = domain(current.world.as_str());
        if server.domain_is_temporary(home.into()) {
            return Err("Leave your current temporary game before joining this room".into());
        }
        if !server.player_ready(player) {
            return Err("Wait until your current world has loaded".into());
        }
        let member = Human::reserve(player, home)?;
        if matches!(self.state, State::Idle) {
            self.state = State::PreparingLobby(Self::create_world(
                server,
                &self.lobby,
                &self.definition.lobby,
                true,
            )?);
        }
        self.members.push(member);
        Ok(())
    }

    fn create_world(
        server: &Server<'_>,
        key: &str,
        definition: &WorldDefinition,
        primary: bool,
    ) -> Result<Operation, ApiString> {
        server
            .create_temporary_world(
                key.into(),
                definition.generator.as_str().into(),
                definition.settings,
                primary,
            )
            .into()
    }

    fn update_members(
        &mut self,
        server: &Server<'_>,
        players: &[PlayerInfo],
    ) -> Result<ApiVec<PlayerId>, ApiString> {
        let mut arrivals = ApiVec::new();
        let mut result = Ok(());
        let domain_name = &self.domain;
        let domain_created = self.domain_created;
        self.members.retain_mut(|member| {
            if result.is_err() {
                return true;
            }
            match member.update(server, players, domain_name, domain_created, &mut arrivals) {
                Ok(keep) => keep,
                Err(error) => {
                    result = Err(error);
                    true
                }
            }
        });
        result.map(|()| arrivals)
    }

    fn advance(&mut self, server: &Server<'_>) -> Result<(), ApiString> {
        match self.state {
            State::PreparingLobby(operation) => {
                if let Some(result) = Self::poll(server, operation)? {
                    self.finish_creation(result)?;
                    self.state = State::Lobby;
                    self.domain_created = true;
                }
            }
            State::PreparingArena(operation) => {
                if let Some(result) = Self::poll(server, operation)? {
                    self.finish_creation(result)?;
                    self.state = State::Arena;
                }
            }
            State::Returning(creation) => {
                while let Some(&bot) = self.bots.last() {
                    self.remove_bot(server, bot)?;
                }
                if let Some(operation) = creation {
                    if let Some(result) = Self::poll(server, operation)? {
                        self.state = State::Returning(None);
                        result?;
                        self.domain_created = true;
                    }
                    return Ok(());
                }
                if self.members.is_empty() {
                    self.state = if self.domain_created {
                        State::Retiring(
                            server
                                .destroy_temporary_domain(self.domain.as_str().into())
                                .match_owned(Ok, Err)?,
                        )
                    } else {
                        State::Closed
                    };
                }
            }
            State::Retiring(operation) => {
                if let Some(result) = Self::poll(server, operation)? {
                    self.state = State::Returning(None);
                    result?;
                    self.state = State::Closed;
                }
            }
            State::Idle | State::Lobby | State::Arena | State::Closed => {}
        }
        Ok(())
    }

    fn finish_creation(&mut self, result: Result<(), ApiString>) -> Result<(), ApiString> {
        if result.is_err() {
            self.return_players(None);
        }
        result
    }

    fn return_players(&mut self, creation: Option<Operation>) {
        for member in &mut self.members {
            member.leaving = true;
        }
        self.state = State::Returning(creation);
    }

    /// Keep a completed failure distinct from a failed status lookup. The host
    /// consumes completed operations, so their handles must be cleared even on error.
    fn poll(
        server: &Server<'_>,
        operation: Operation,
    ) -> Result<Option<Result<(), ApiString>>, ApiString> {
        let status = server.operation_status(operation).match_owned(Ok, Err)?;
        Ok(match status.state {
            OperationState::Pending => None,
            OperationState::Ready => Some(Ok(())),
            OperationState::Failed => Some(Err(status.value)),
        })
    }

    fn move_to_arena(
        &mut self,
        server: &Server<'_>,
        player: &PlayerInfo,
    ) -> Result<bool, ApiString> {
        if !matches!(self.state, State::Arena) {
            return Err("The arena is not ready for transfers".into());
        }
        let id = player.id;
        if !self.bots.contains(&id)
            && !self
                .members
                .iter()
                .any(|member| member.player == id && !member.leaving)
        {
            return Err("Player is not in this room".into());
        }
        if !server.player_ready(id) {
            return Ok(false);
        }
        if player.world.as_str() == self.arena {
            return Ok(true);
        }
        if !self.arena_transfers.contains(&id) {
            server
                .select_world(id, self.arena.as_str().into())
                .match_owned(Ok, Err)?;
            self.arena_transfers.insert(id);
        }
        Ok(false)
    }

    fn add_bot(
        &mut self,
        server: &Server<'_>,
        name: Str<'_>,
        position: Position,
    ) -> Result<PlayerId, ApiString> {
        if !matches!(self.state, State::Lobby) {
            return Err("The lobby is not ready for bots".into());
        }
        let humans = self.members.iter().filter(|member| !member.leaving).count();
        if humans + self.bots.len() >= self.definition.capacity as usize {
            return Err("This room is full".into());
        }
        let bot = server
            .spawn_bot(self.lobby.as_str().into(), name, position)
            .match_owned(Ok, Err)?;
        self.bots.push(bot);
        Ok(bot)
    }

    fn remove_bot(&mut self, server: &Server<'_>, player: PlayerId) -> Result<(), ApiString> {
        let index = self
            .bots
            .iter()
            .position(|&bot| bot == player)
            .ok_or_else(|| ApiString::from("Bot is not in this room"))?;
        if server.is_connected(player) {
            server.despawn_bot(player).match_owned(Ok, Err)?;
        }
        self.bots.remove(index);
        self.arena_transfers.remove(&player);
        Ok(())
    }
}

#[stabby::export(callback)]
impl Room for GameRoom {
    fn phase(&self) -> RoomPhase {
        match self.state {
            State::Idle => RoomPhase::Idle,
            State::PreparingLobby(_) => RoomPhase::PreparingLobby,
            State::Lobby => RoomPhase::Lobby,
            State::PreparingArena(_) => RoomPhase::PreparingArena,
            State::Arena => RoomPhase::Arena,
            State::Returning(_) => RoomPhase::Returning,
            State::Retiring(_) => RoomPhase::Retiring,
            State::Closed => RoomPhase::Closed,
        }
    }

    fn lobby_key<'a>(&'a self) -> Str<'a> {
        self.lobby.as_str().into()
    }

    fn arena_key<'a>(&'a self) -> Str<'a> {
        self.arena.as_str().into()
    }

    fn members(&self) -> ApiVec<Member> {
        self.members
            .iter()
            .map(|member| Member {
                player: member.player,
                arrived: member.arrived,
                leaving: member.leaving,
            })
            .collect()
    }

    fn bots(&self) -> ApiVec<PlayerId> {
        self.bots.iter().copied().collect()
    }

    fn join(&mut self, server: &Server<'_>, player: PlayerId) -> ApiResult<()> {
        self.join_player(server, player).into()
    }

    fn leave(&mut self, player: PlayerId) -> ApiResult<()> {
        let member = self
            .members
            .iter_mut()
            .find(|member| member.player == player)
            .ok_or_else(|| ApiString::from("You are not in this room"));
        member.map(|member| member.leaving = true).into()
    }

    fn tick(
        &mut self,
        server: &Server<'_>,
        players: Slice<'_, PlayerInfo>,
    ) -> ApiResult<RoomUpdate> {
        let result: Result<RoomUpdate, ApiString> = (|| {
            let arrivals = self.update_members(server, players.as_slice())?;
            self.bots.retain(|&bot| server.is_connected(bot));
            if !matches!(self.state, State::Idle)
                && self.members.iter().all(|member| member.leaving)
            {
                self.close();
            }
            self.advance(server)?;
            Ok(RoomUpdate {
                phase: self.phase(),
                arrivals,
            })
        })();
        result.into()
    }

    fn start_arena(&mut self, server: &Server<'_>) -> ApiResult<()> {
        if !matches!(self.state, State::Lobby) {
            return Err(ApiString::from("The lobby is not ready to start")).into();
        }
        Self::create_world(server, &self.arena, &self.definition.arena, false)
            .map(|operation| self.state = State::PreparingArena(operation))
            .into()
    }

    fn enter_arena(&mut self, server: &Server<'_>, player: &PlayerInfo) -> ApiResult<bool> {
        self.move_to_arena(server, player).into()
    }

    fn spawn_bot(
        &mut self,
        server: &Server<'_>,
        name: Str<'_>,
        position: Position,
    ) -> ApiResult<PlayerId> {
        self.add_bot(server, name, position).into()
    }

    fn despawn_bot(&mut self, server: &Server<'_>, player: PlayerId) -> ApiResult<()> {
        self.remove_bot(server, player).into()
    }

    fn close(&mut self) {
        let creation = match self.state {
            State::Returning(_) | State::Retiring(_) | State::Closed => return,
            State::PreparingLobby(operation) | State::PreparingArena(operation) => Some(operation),
            State::Idle | State::Lobby | State::Arena => None,
        };
        self.return_players(creation);
    }
}

fn domain(world: &str) -> &str {
    world.split_once(':').map_or(world, |(domain, _)| domain)
}
