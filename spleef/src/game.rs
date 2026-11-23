use crate::{ai, api, map};
use minigame_api::{
    DynRoom, DynRooms, RoomDefinition, RoomDyn, RoomDynMut, RoomPhase, RoomsDyn, WorldDefinition,
};
use serde::Deserialize;
use steel_extension_api::{
    BlockPos, Difficulty, GameMode, PlayerId, PlayerInfo, Position, Server, WorldSettings,
};

const TICKS_PER_SECOND: u64 = 20;
const TRANSFER_TIMEOUT_TICKS: u64 = 30 * TICKS_PER_SECOND;

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub lobby_seconds: u64,
    pub round_seconds: u64,
    pub ending_seconds: u64,
    pub players: usize,
    pub seed: i64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            lobby_seconds: 20,
            round_seconds: 180,
            ending_seconds: 5,
            players: 4,
            seed: 0,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if !(2..=8).contains(&self.players) {
            return Err("Spleef players must be between 2 and 8".into());
        }
        if !(1..=300).contains(&self.lobby_seconds)
            || !(10..=1800).contains(&self.round_seconds)
            || !(1..=30).contains(&self.ending_seconds)
        {
            return Err("Spleef timers are outside their supported bounds".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Phase {
    Waiting { deadline: Option<u64> },
    Transferring { deadline: u64 },
    Playing { deadline: u64 },
    Ending { deadline: u64 },
}

struct Contestant {
    id: PlayerId,
    bot: bool,
    ready: bool,
    alive: bool,
}

pub struct Game {
    config: Config,
    provider: &'static DynRooms,
    round: Option<Round>,
}

struct Round {
    room: DynRoom,
    phase: Phase,
    contestants: Vec<Contestant>,
}

impl Game {
    pub fn new(config: Config, provider: &'static DynRooms) -> Self {
        Self {
            config,
            provider,
            round: None,
        }
    }

    pub fn status(&self) -> String {
        let (phase, humans) = self.round.as_ref().map_or(("ready", 0), |round| {
            let members = round.room.members();
            (
                round.phase_label(),
                members.iter().filter(|member| !member.leaving).count(),
            )
        });
        format!(
            "Spleef: {phase}; {humans}/{} human players. /spleef join | leave | status",
            self.config.players
        )
    }

    pub fn join(&mut self, server: &Server<'_>, id: PlayerId) -> Result<(), String> {
        if let Some(round) = &mut self.round {
            return api!(round.room.join(server, id));
        }

        let world = |generator: &str, default_game_mode| WorldDefinition {
            generator: generator.into(),
            settings: WorldSettings {
                seed: self.config.seed,
                spawn: spawn(),
                default_game_mode,
                difficulty: Difficulty::Peaceful,
            },
        };
        let mut room = api!(
            self.provider.create(RoomDefinition {
                domain_prefix: "spleef".into(),
                capacity: self
                    .config
                    .players
                    .try_into()
                    .map_err(|_| "Spleef capacity is too large")?,
                lobby: world("spleef:lobby", GameMode::Adventure),
                arena: world("spleef:arena", GameMode::Survival),
            })
        )?;
        api!(room.join(server, id))?;
        self.round = Some(Round {
            room,
            phase: Phase::Waiting { deadline: None },
            contestants: Vec::new(),
        });
        Ok(())
    }

    pub fn leave(&mut self, id: PlayerId) -> Result<(), String> {
        let round = self.round.as_mut().ok_or("You are not in Spleef")?;
        api!(round.room.leave(id))?;
        if let Some(contestant) = round.contestants.iter_mut().find(|actor| actor.id == id) {
            contestant.alive = false;
        }
        Ok(())
    }

    pub fn begin_return(&mut self) {
        if let Some(round) = &mut self.round {
            round.room.close();
        }
    }

    pub fn tick(&mut self, server: &Server<'_>, tick: u64) -> Result<(), String> {
        if let Some(round) = &mut self.round
            && round.tick(&self.config, server, tick)?
        {
            self.round = None;
        }
        Ok(())
    }
}

impl Round {
    fn phase_label(&self) -> &'static str {
        match self.room.phase() {
            RoomPhase::Idle | RoomPhase::Closed => "ready",
            RoomPhase::PreparingLobby => "preparing the lobby",
            RoomPhase::Lobby => "waiting in the lobby",
            RoomPhase::PreparingArena => "building the arena",
            RoomPhase::Arena => match self.phase {
                Phase::Waiting { .. } | Phase::Transferring { .. } => "entering the arena",
                Phase::Playing { .. } => "playing",
                Phase::Ending { .. } => "round finished",
            },
            RoomPhase::Returning => "returning players",
            RoomPhase::Retiring => "cleaning up",
        }
    }

    fn tick(&mut self, config: &Config, server: &Server<'_>, tick: u64) -> Result<bool, String> {
        let players = server.players();
        let players = players.as_slice();
        let update = api!(self.room.tick(server, players.into()))?;
        for &id in &update.arrivals {
            api!(server.set_game_mode(id, GameMode::Adventure))?;
            api!(server.clear_inventory(id))?;
            api!(server.message(id, "Welcome to Spleef! Dig snow under your opponents; falling eliminates you. /spleef leave restores your normal world.".into()))?;
        }
        match update.phase {
            RoomPhase::Lobby => self.tick_lobby(config, server, tick)?,
            RoomPhase::Arena => self.tick_arena(config, server, players, tick)?,
            RoomPhase::Closed => return Ok(true),
            _ => {}
        }
        Ok(false)
    }

    fn tick_lobby(
        &mut self,
        config: &Config,
        server: &Server<'_>,
        tick: u64,
    ) -> Result<(), String> {
        let Phase::Waiting { mut deadline } = self.phase else {
            return Err("Spleef was in the lobby after the round started".into());
        };
        let members = self.room.members();
        if members
            .iter()
            .any(|member| member.arrived && !member.leaving)
        {
            let humans = members.iter().filter(|member| !member.leaving).count();
            self.fill_bots(server, config.players - humans)?;
            if deadline.is_none() {
                deadline = Some(tick + config.lobby_seconds * TICKS_PER_SECOND);
                self.announce(
                    server,
                    &format!(
                        "Spleef starts in {} seconds. Empty places are filled with bots.",
                        config.lobby_seconds
                    ),
                )?;
            }
        }
        self.phase = Phase::Waiting { deadline };
        if deadline.is_some_and(|end| tick >= end) {
            api!(self.room.start_arena(server))?;
        }
        Ok(())
    }

    fn fill_bots(&mut self, server: &Server<'_>, desired: usize) -> Result<(), String> {
        let bots = self.room.bots();
        for &id in bots.iter().skip(desired).rev() {
            api!(self.room.despawn_bot(server, id))?;
        }
        for number in bots.len() + 1..=desired {
            let name = format!("SpleefBot{number}");
            let id = api!(self.room.spawn_bot(
                server,
                name.as_str().into(),
                start_position(number)
            ))?;
            api!(server.set_game_mode(id, GameMode::Adventure))?;
        }
        Ok(())
    }

    fn tick_arena(
        &mut self,
        config: &Config,
        server: &Server<'_>,
        players: &[PlayerInfo],
        tick: u64,
    ) -> Result<(), String> {
        match self.phase {
            Phase::Waiting { .. } => {
                let members = self.room.members();
                let bots = self.room.bots();
                self.contestants = members
                    .iter()
                    .filter(|member| !member.leaving)
                    .map(|member| (member.player, false))
                    .chain(bots.iter().map(|&id| (id, true)))
                    .map(|(id, bot)| Contestant {
                        id,
                        bot,
                        ready: false,
                        alive: true,
                    })
                    .collect();
                self.phase = Phase::Transferring {
                    deadline: tick + TRANSFER_TIMEOUT_TICKS,
                };
            }
            Phase::Transferring { deadline } => {
                if self.transfer_contestants(server, players)? {
                    self.announce(server, "Spleef begins! Last player on the snow wins.")?;
                    self.phase = Phase::Playing {
                        deadline: tick + config.round_seconds * TICKS_PER_SECOND,
                    };
                } else if tick >= deadline {
                    self.announce(
                        server,
                        "Spleef cancelled: players did not finish loading the arena.",
                    )?;
                    self.room.close();
                }
            }
            Phase::Playing { deadline } => {
                self.play(config, server, players, tick)?;
                if tick >= deadline && matches!(self.phase, Phase::Playing { .. }) {
                    self.announce(server, "Time is up! Spleef ended in a draw.")?;
                    self.phase = Phase::Ending {
                        deadline: tick + config.ending_seconds * TICKS_PER_SECOND,
                    };
                }
            }
            Phase::Ending { deadline } => {
                if tick >= deadline {
                    self.room.close();
                }
            }
        }
        Ok(())
    }

    fn transfer_contestants(
        &mut self,
        server: &Server<'_>,
        players: &[PlayerInfo],
    ) -> Result<bool, String> {
        let members = self.room.members();
        for (index, actor) in self.contestants.iter_mut().enumerate() {
            if !actor.alive {
                continue;
            }
            if !server.is_connected(actor.id)
                || (!actor.bot
                    && !members
                        .iter()
                        .any(|member| member.player == actor.id && !member.leaving))
            {
                actor.alive = false;
                continue;
            }
            let Some(player) = info(players, actor.id) else {
                continue;
            };
            if !actor.ready && api!(self.room.enter_arena(server, player))? {
                api!(server.teleport(actor.id, start_position(index)))?;
                api!(server.clear_inventory(actor.id))?;
                api!(server.equip_item(actor.id, "minecraft:diamond_shovel".into()))?;
                api!(server.set_game_mode(actor.id, GameMode::Survival))?;
                actor.ready = true;
            }
        }
        Ok(self
            .contestants
            .iter()
            .all(|actor| !actor.alive || actor.ready))
    }

    fn announce(&self, server: &Server<'_>, message: &str) -> Result<(), String> {
        for member in &self.room.members() {
            if server.is_connected(member.player) {
                api!(server.message(member.player, message.into()))?;
            }
        }
        Ok(())
    }

    fn play(
        &mut self,
        config: &Config,
        server: &Server<'_>,
        players: &[PlayerInfo],
        tick: u64,
    ) -> Result<(), String> {
        let arena = self.room.arena_key();
        let arena = arena.as_str();
        for actor in &mut self.contestants {
            if !actor.alive {
                continue;
            }
            let player = info(players, actor.id);
            let lost = player.is_none_or(|p| {
                p.world.as_str() != arena
                    || p.position.y < f64::from(map::FLOOR - 8)
                    || p.health <= 0.0
            });
            if !lost {
                continue;
            }
            actor.alive = false;
            if let Some(player) = player
                && player.world.as_str() == arena
                && player.health > 0.0
            {
                api!(server.set_game_mode(actor.id, GameMode::Spectator))?;
                api!(server.teleport(
                    actor.id,
                    Position {
                        x: 0.5,
                        y: f64::from(map::FLOOR + 12),
                        z: 0.5
                    }
                ))?;
                if !actor.bot {
                    api!(
                        server.message(
                            actor.id,
                            "Eliminated! You can watch until the round ends or use /spleef leave."
                                .into()
                        )
                    )?;
                }
            }
        }
        let alive: Vec<_> = self
            .contestants
            .iter()
            .filter(|p| p.alive)
            .filter_map(|p| info(players, p.id))
            .collect();
        for actor in self.contestants.iter().filter(|p| p.alive && p.bot) {
            if let Some(player) = info(players, actor.id) {
                ai::step(server, arena, player, &alive, tick)?;
            }
        }
        if alive.len() <= 1 {
            let text = alive.first().map_or_else(
                || "Spleef ended in a draw.".to_owned(),
                |p| format!("{} wins Spleef!", p.name),
            );
            self.announce(server, &text)?;
            self.phase = Phase::Ending {
                deadline: tick + config.ending_seconds * TICKS_PER_SECOND,
            };
        }
        Ok(())
    }
}
fn info(players: &[PlayerInfo], id: PlayerId) -> Option<&PlayerInfo> {
    players.iter().find(|p| p.id == id)
}
fn spawn() -> BlockPos {
    BlockPos {
        x: 0,
        y: map::FLOOR + 1,
        z: 0,
    }
}
fn start_position(index: usize) -> Position {
    const POSITIONS: [(f64, f64); 8] = [
        (-7.5, -7.5),
        (7.5, 7.5),
        (-7.5, 7.5),
        (7.5, -7.5),
        (0.5, -9.5),
        (0.5, 9.5),
        (-9.5, 0.5),
        (9.5, 0.5),
    ];
    let (x, z) = POSITIONS[index % POSITIONS.len()];
    Position {
        x,
        y: f64::from(map::FLOOR + 1),
        z,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_spawn_slots_are_on_the_authored_floor_with_room_for_walls() {
        for i in 0..8 {
            let p = start_position(i);
            assert!(
                p.x.abs() < f64::from(map::RADIUS - 1) && p.z.abs() < f64::from(map::RADIUS - 1)
            );
        }
    }
}
