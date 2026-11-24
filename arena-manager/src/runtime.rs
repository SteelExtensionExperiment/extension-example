//! The match runtime: reserves arenas and worlds, moves players, applies the match rules and
//! cleans up, advanced once per server tick.
//!
//! Generic over the player and operation handles of [`Host`] so it runs against the test fake.
//! Locks: the runtime is locked before the arena store; nothing here calls hook actions, which
//! the caller emits from the returned [`Emission`]s after releasing every lock.
use crate::{
    allocator::{Allocation, allocate},
    config::Config,
    host::Host,
    matches::{Effect, LeaveKind, Match, PlayerEvent, State},
    store::ArenaStore,
    transfer::{Arrival, LobbyReturn, Progress},
    worlds::{WorldEvent, Worlds},
};
use arena_manager_api::{
    MatchFailed, MatchFinished, MatchId, MatchInfo, MatchPlayer, MatchState, Participant,
};
use steel_extension_api::{
    inventory_snapshot::InventorySnapshot, log, players::Uuid, stabby::string::String as ApiString,
};

/// Server tick rate. The host exposes none to extensions.
const TICKS_PER_SECOND: u32 = 20;
/// Idle worlds are checked for removal once per second.
const CLEANUP_INTERVAL_TICKS: u64 = TICKS_PER_SECOND as u64;

/// Something a handler saw; resolved on the next tick (research R8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Incoming<P> {
    Death(P),
    Quit(P),
    Respawn(P),
}

/// A hook action to emit after every lock is released.
#[derive(Debug, Clone)]
pub enum Emission {
    Finished(MatchFinished),
    Failed(MatchFailed),
}

/// Shared inputs of one runtime call.
pub struct Context<'a> {
    pub store: &'a ArenaStore,
    pub config: &'a Config,
}

/// A validated match request: game type and players with their kits, in slot order.
pub struct Request<'a> {
    pub game_type: &'a str,
    pub players: Vec<(Uuid, InventorySnapshot)>,
}

struct Member<P> {
    player: P,
    uuid: Uuid,
    name: String,
    arrival: Arrival<P>,
    placed: bool,
}

struct Record<P> {
    id: MatchId,
    game_type: String,
    arena: String,
    world_key: String,
    rules: Match<P>,
    members: Vec<Member<P>>,
}

impl<P: Copy + PartialEq> Record<P> {
    fn member(&self, player: P) -> Option<&Member<P>> {
        self.members.iter().find(|member| member.player == player)
    }
}

pub struct Runtime<P, O> {
    worlds: Worlds<O>,
    records: Vec<Record<P>>,
    /// Dead players, moved to the lobby once they respawn.
    waiting_respawn: Vec<P>,
    lobby_returns: Vec<LobbyReturn<P>>,
    next_match_id: MatchId,
    ticks: u64,
}

impl<P, O> Default for Runtime<P, O> {
    fn default() -> Self {
        Self {
            worlds: Worlds::default(),
            records: Vec::new(),
            waiting_respawn: Vec::new(),
            lobby_returns: Vec::new(),
            next_match_id: 1,
            ticks: 0,
        }
    }
}

fn timeout_ticks(config: &Config) -> u32 {
    let ticks = config
        .player_ready_timeout_seconds
        .saturating_mul(u64::from(TICKS_PER_SECOND));
    u32::try_from(ticks).unwrap_or(u32::MAX)
}

impl<P: Copy + PartialEq + std::fmt::Debug, O: Copy + std::fmt::Debug> Runtime<P, O> {
    /// Reserves an arena (and a world when none is free) or rejects at once.
    pub fn start_match<H: Host<Player = P, Operation = O>>(
        &mut self,
        host: &H,
        ctx: &Context<'_>,
        request: &Request<'_>,
    ) -> Result<MatchId, String> {
        if ctx.store.lobby.is_none() {
            return Err("no lobby is set; run /arena lobby set".to_owned());
        }
        let players = self.resolve_players(host, request)?;
        let allocation = allocate(
            ctx.store,
            &mut self.worlds.worlds,
            ctx.config.max_worlds,
            request.game_type,
            players.len(),
        )
        .map_err(|rejection| rejection.to_string())?;
        let (arena, world_key, world_ready) = match allocation {
            Allocation::Existing { world, arena } => (arena, world, true),
            Allocation::NewWorld { arena } => {
                let key = self.request_world(host, ctx, request.game_type, &arena)?;
                (arena, key, false)
            }
        };
        let spawns = ctx.store.arena(request.game_type, &arena)?.spawns.clone();
        let delay = u32::try_from(ctx.config.winner_return_delay_seconds).unwrap_or(u32::MAX);
        let mut rules = Match::new(
            players.iter().map(|player| player.0),
            TICKS_PER_SECOND,
            delay,
        )
        .ok_or("the tick rate must be above zero")?;
        if world_ready {
            rules.world_ready();
        }
        let members = players
            .into_iter()
            .zip(spawns)
            .zip(request.players.iter())
            .map(|(((player, uuid, name), spawn), (_, kit))| Member {
                player,
                uuid,
                name,
                arrival: Arrival::new(player, world_key.clone(), spawn, kit.clone()),
                placed: false,
            })
            .collect();
        let id = self.next_match_id;
        self.next_match_id += 1;
        self.records.push(Record {
            id,
            game_type: request.game_type.to_owned(),
            arena,
            world_key,
            rules,
            members,
        });
        Ok(id)
    }

    /// Online, ready, distinct players that are not in a match: handle, uuid and name.
    fn resolve_players<H: Host<Player = P, Operation = O>>(
        &self,
        host: &H,
        request: &Request<'_>,
    ) -> Result<Vec<(P, Uuid, String)>, String> {
        if request.players.is_empty() {
            return Err("a match needs at least one player".to_owned());
        }
        let mut players: Vec<(P, Uuid, String)> = Vec::new();
        for (uuid, _) in &request.players {
            let player = host
                .player_by_uuid(*uuid)
                .ok_or_else(|| format!("player {uuid:?} is not online"))?;
            let name = host.player_name(player)?;
            if players.iter().any(|known| known.0 == player) {
                return Err(format!("player {name} is listed twice"));
            }
            if !host.player_ready(player) {
                return Err(format!("player {name} is not ready"));
            }
            if self
                .records
                .iter()
                .any(|record| record.member(player).is_some())
            {
                return Err(format!("player {name} is already in a match"));
            }
            players.push((player, *uuid, name));
        }
        Ok(players)
    }

    /// Clones the game type's template; the template must be configured and ready.
    fn request_world<H: Host<Player = P, Operation = O>>(
        &mut self,
        host: &H,
        ctx: &Context<'_>,
        game_type: &str,
        arena: &str,
    ) -> Result<String, String> {
        let entry = ctx
            .store
            .game_type(game_type)
            .ok_or_else(|| format!("unknown game type {game_type}"))?;
        let template = entry
            .template
            .as_deref()
            .ok_or_else(|| format!("game type {game_type} has no template; run /arena template"))?;
        let info = host
            .templates()?
            .into_iter()
            .find(|info| info.name == template)
            .ok_or_else(|| format!("unknown template {template}"))?;
        match info.state {
            steel_extension_api::TemplateState::Ready => {}
            steel_extension_api::TemplateState::Loading => {
                return Err(format!("template {template} is still loading"));
            }
            steel_extension_api::TemplateState::Failed => {
                return Err(format!(
                    "template {template} failed to load: {}",
                    info.message
                ));
            }
        }
        self.worlds.request_world(host, template, game_type, arena)
    }

    /// Whether a match is using the arena.
    #[must_use]
    pub fn arena_in_use(&self, game_type: &str, arena: &str) -> bool {
        self.records
            .iter()
            .any(|record| record.game_type == game_type && record.arena == arena)
    }

    /// Snapshot of every current match.
    #[must_use]
    pub fn infos(&self) -> Vec<MatchInfo> {
        self.records
            .iter()
            .map(|record| MatchInfo {
                id: record.id,
                game_type: record.game_type.as_str().into(),
                arena: record.arena.as_str().into(),
                world: match record.rules.state() {
                    State::Preparing => "",
                    _ => record.world_key.as_str(),
                }
                .into(),
                state: match_state(record.rules.state()),
                players: record
                    .members
                    .iter()
                    .map(|member| MatchPlayer {
                        uuid: member.uuid,
                        alive: record.rules.is_alive(member.player),
                    })
                    .collect(),
            })
            .collect()
    }

    /// Advances everything by one server tick and returns the actions to emit.
    pub fn tick<H: Host<Player = P, Operation = O>>(
        &mut self,
        host: &H,
        ctx: &Context<'_>,
        incoming: Vec<Incoming<P>>,
    ) -> Vec<Emission> {
        self.ticks += 1;
        let mut emissions = Vec::new();
        self.apply_world_events(host, ctx, &mut emissions);
        self.apply_incoming(host, ctx, incoming, &mut emissions);
        for index in 0..self.records.len() {
            self.advance_record(host, ctx, index, &mut emissions);
        }
        self.records
            .retain(|record| !matches!(record.rules.state(), State::Finished | State::Failed));
        self.advance_lobby_returns(host, ctx);
        if self.ticks.is_multiple_of(CLEANUP_INTERVAL_TICKS) {
            for event in self.worlds.start_cleanup(host) {
                log_world_event(&event);
            }
        }
        emissions
    }

    fn apply_world_events<H: Host<Player = P, Operation = O>>(
        &mut self,
        host: &H,
        ctx: &Context<'_>,
        emissions: &mut Vec<Emission>,
    ) {
        for event in self.worlds.poll(host) {
            match event {
                WorldEvent::Ready { key } => {
                    for record in self.records.iter_mut().filter(|r| r.world_key == key) {
                        record.rules.world_ready();
                    }
                }
                WorldEvent::CreateFailed { key, message, .. } => {
                    let message = format!("could not create the arena world: {message}");
                    let failing: Vec<usize> = (0..self.records.len())
                        .filter(|&index| self.records[index].world_key == key)
                        .collect();
                    for index in failing {
                        let effects = self.records[index].rules.fail(message.clone());
                        self.apply_effects(host, ctx, index, effects, &[], emissions);
                    }
                }
                event @ WorldEvent::CleanupFailed { .. } => log_world_event(&event),
            }
        }
    }

    fn apply_incoming<H: Host<Player = P, Operation = O>>(
        &mut self,
        host: &H,
        ctx: &Context<'_>,
        incoming: Vec<Incoming<P>>,
        emissions: &mut Vec<Emission>,
    ) {
        let mut events: Vec<(usize, PlayerEvent<P>)> = Vec::new();
        for item in incoming {
            match item {
                Incoming::Respawn(player) => {
                    let Some(position) = self.waiting_respawn.iter().position(|p| *p == player)
                    else {
                        continue;
                    };
                    self.waiting_respawn.swap_remove(position);
                    self.start_lobby_return(ctx, player);
                }
                Incoming::Death(player) | Incoming::Quit(player) => {
                    let kind = if matches!(item, Incoming::Death(_)) {
                        LeaveKind::Death
                    } else {
                        self.waiting_respawn.retain(|p| *p != player);
                        LeaveKind::Quit
                    };
                    let Some(index) = self
                        .records
                        .iter()
                        .position(|record| record.member(player).is_some())
                    else {
                        continue;
                    };
                    events.push((index, PlayerEvent { player, kind }));
                }
            }
        }
        for index in 0..self.records.len() {
            let batch: Vec<PlayerEvent<P>> = events
                .iter()
                .filter(|(event_index, _)| *event_index == index)
                .map(|(_, event)| *event)
                .collect();
            if batch.is_empty() {
                continue;
            }
            let deaths: Vec<P> = batch
                .iter()
                .filter(|event| event.kind == LeaveKind::Death)
                .map(|event| event.player)
                .collect();
            let effects = self.records[index].rules.resolve(&batch);
            self.apply_effects(host, ctx, index, effects, &deaths, emissions);
        }
    }

    /// Places players while `Starting` and runs the countdown while `Ending`.
    fn advance_record<H: Host<Player = P, Operation = O>>(
        &mut self,
        host: &H,
        ctx: &Context<'_>,
        index: usize,
        emissions: &mut Vec<Emission>,
    ) {
        let timeout = timeout_ticks(ctx.config);
        if self.records[index].rules.state() == State::Starting {
            let mut failure = None;
            let record = &mut self.records[index];
            for member in record.members.iter_mut().filter(|member| !member.placed) {
                match member.arrival.advance(host, timeout) {
                    Progress::Pending => {}
                    Progress::Done => {
                        member.placed = true;
                        record.rules.player_placed(member.player);
                    }
                    Progress::Failed(message) => {
                        failure = Some(message);
                        break;
                    }
                }
            }
            if let Some(message) = failure {
                let effects = self.records[index].rules.fail(message);
                self.apply_effects(host, ctx, index, effects, &[], emissions);
                return;
            }
        }
        let effects = self.records[index].rules.tick();
        self.apply_effects(host, ctx, index, effects, &[], emissions);
    }

    fn apply_effects<H: Host<Player = P, Operation = O>>(
        &mut self,
        host: &H,
        ctx: &Context<'_>,
        index: usize,
        effects: Vec<Effect<P>>,
        deaths: &[P],
        emissions: &mut Vec<Emission>,
    ) {
        for effect in effects {
            match effect {
                Effect::SendToLobby(player) => {
                    // A death waits for the respawn; a quit has nothing left to move.
                    if deaths.contains(&player) && !self.waiting_respawn.contains(&player) {
                        self.waiting_respawn.push(player);
                    }
                }
                Effect::Countdown { winner, seconds } => {
                    let text = format!("Returning to the lobby in {seconds}");
                    if let Err(error) = host.send_action_bar(winner, &text) {
                        log::warn(format!("countdown for the winner failed: {error}"));
                    }
                }
                Effect::MoveWinnerToLobby(player) => self.start_lobby_return(ctx, player),
                Effect::ReleaseArena => {
                    let record = &self.records[index];
                    self.worlds.release_arena(&record.world_key, &record.arena);
                }
                Effect::Finished(results) => {
                    emissions.push(Emission::Finished(finished(&self.records[index], &results)));
                }
                Effect::Failed { message } => {
                    let record = &self.records[index];
                    emissions.push(Emission::Failed(MatchFailed {
                        id: record.id,
                        game_type: record.game_type.as_str().into(),
                        message: ApiString::from(message.as_str()),
                    }));
                }
            }
        }
    }

    fn start_lobby_return(&mut self, ctx: &Context<'_>, player: P) {
        match &ctx.store.lobby {
            Some(lobby) => self.lobby_returns.push(LobbyReturn::new(player, lobby)),
            None => log::warn("no lobby is set; a player was not returned"),
        }
    }

    fn advance_lobby_returns<H: Host<Player = P, Operation = O>>(
        &mut self,
        host: &H,
        ctx: &Context<'_>,
    ) {
        let timeout = timeout_ticks(ctx.config);
        self.lobby_returns
            .retain_mut(|lobby_return| match lobby_return.advance(host, timeout) {
                Progress::Pending => true,
                Progress::Done => false,
                Progress::Failed(message) => {
                    log::warn(format!("could not return a player to the lobby: {message}"));
                    false
                }
            });
    }
}

fn finished<P: Copy + PartialEq>(
    record: &Record<P>,
    results: &[(P, arena_manager_api::Outcome)],
) -> MatchFinished {
    MatchFinished {
        id: record.id,
        game_type: record.game_type.as_str().into(),
        participants: results
            .iter()
            .filter_map(|(player, outcome)| {
                let member = record.member(*player)?;
                Some(Participant {
                    uuid: member.uuid,
                    name: member.name.as_str().into(),
                    outcome: *outcome,
                })
            })
            .collect(),
    }
}

fn match_state(state: State) -> MatchState {
    match state {
        State::Preparing => MatchState::Preparing,
        State::Starting => MatchState::Starting,
        State::Running => MatchState::Running,
        State::Ending { .. } | State::Finished | State::Failed => MatchState::Ending,
    }
}

fn log_world_event(event: &WorldEvent) {
    if let WorldEvent::CleanupFailed { key, message } = event {
        log::warn(format!("could not remove arena world {key}: {message}"));
    }
}

#[cfg(test)]
mod tests;
