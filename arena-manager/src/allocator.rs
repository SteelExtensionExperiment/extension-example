//! Picks the arena and world for a match request. Pure: the caller creates worlds and applies
//! the result.
use crate::store::ArenaStore;
use std::borrow::{Borrow, BorrowMut};
use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldReadiness {
    Creating,
    Ready,
    Removing,
    DestroyingDomain,
}

/// What the allocator needs to know about an `ArenaWorld`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldView {
    pub key: String,
    pub game_type: String,
    pub readiness: WorldReadiness,
    /// Names of arenas that have a match in this world.
    pub busy: HashSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Allocation {
    /// The arena is already marked busy in `world`.
    Existing { world: String, arena: String },
    /// The caller creates a world and marks `arena` busy in it.
    NewWorld { arena: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    UnknownGameType(String),
    NoArena {
        game_type: String,
        player_count: usize,
    },
    NoCapacity,
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownGameType(name) => write!(f, "unknown game type {name}"),
            Self::NoArena {
                game_type,
                player_count,
            } => write!(
                f,
                "no complete arena for {game_type} with {player_count} players"
            ),
            Self::NoCapacity => f.write_str("no capacity"),
        }
    }
}

/// Reserves a free complete arena whose slot count equals `player_count`.
///
/// Only `Ready` worlds are reused. Every world counts toward `max_worlds`, whatever its state.
pub fn allocate<W: BorrowMut<WorldView>>(
    store: &ArenaStore,
    worlds: &mut [W],
    max_worlds: usize,
    game_type: &str,
    player_count: usize,
) -> Result<Allocation, Rejection> {
    let entry = store
        .game_type(game_type)
        .ok_or_else(|| Rejection::UnknownGameType(game_type.to_owned()))?;
    let candidates: Vec<&str> = entry
        .arenas
        .iter()
        .filter(|arena| arena.is_complete() && arena.slot_count() == player_count)
        .map(|arena| arena.name.as_str())
        .collect();
    let Some(&first) = candidates.first() else {
        return Err(Rejection::NoArena {
            game_type: game_type.to_owned(),
            player_count,
        });
    };

    let ready_worlds =
        worlds
            .iter_mut()
            .map(BorrowMut::borrow_mut)
            .filter(|world: &&mut WorldView| {
                world.readiness == WorldReadiness::Ready && world.game_type == game_type
            });
    for world in ready_worlds {
        let Some(&arena) = candidates
            .iter()
            .find(|arena| !world.busy.contains(**arena))
        else {
            continue;
        };
        world.busy.insert(arena.to_owned());
        return Ok(Allocation::Existing {
            world: world.key.clone(),
            arena: arena.to_owned(),
        });
    }

    if worlds.len() < max_worlds {
        return Ok(Allocation::NewWorld {
            arena: first.to_owned(),
        });
    }
    Err(Rejection::NoCapacity)
}

/// Ready worlds without a match: candidates for cleanup.
pub fn idle_worlds<W: Borrow<WorldView>>(worlds: &[W]) -> impl Iterator<Item = &WorldView> {
    worlds
        .iter()
        .map(Borrow::borrow)
        .filter(|world| world.readiness == WorldReadiness::Ready && world.busy.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{Arena, GameTypeEntry, Spawn};

    const GAME: &str = "duel";

    fn arena(name: &str, slots: u32) -> Arena {
        Arena {
            name: name.to_owned(),
            spawns: (1..=slots)
                .map(|slot| Spawn {
                    slot,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    yaw: 0.0,
                    pitch: 0.0,
                })
                .collect(),
        }
    }

    fn store(arenas: Vec<Arena>) -> ArenaStore {
        ArenaStore {
            lobby: None,
            game_types: vec![GameTypeEntry {
                name: GAME.to_owned(),
                template: None,
                arenas,
            }],
        }
    }

    fn world(key: &str, game_type: &str, readiness: WorldReadiness, busy: &[&str]) -> WorldView {
        WorldView {
            key: key.to_owned(),
            game_type: game_type.to_owned(),
            readiness,
            busy: busy.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    fn no_worlds() -> Vec<WorldView> {
        Vec::new()
    }

    fn existing(world: &str, arena: &str) -> Allocation {
        Allocation::Existing {
            world: world.to_owned(),
            arena: arena.to_owned(),
        }
    }

    fn new_world(arena: &str) -> Allocation {
        Allocation::NewWorld {
            arena: arena.to_owned(),
        }
    }

    #[test]
    fn free_arena_in_ready_world_is_reused_and_reserved() {
        let store = store(vec![arena("a", 2), arena("b", 2)]);
        let mut worlds = [world("w1", GAME, WorldReadiness::Ready, &["a"])];
        let result = allocate(&store, &mut worlds, 1, GAME, 2);
        assert_eq!(result, Ok(existing("w1", "b")));
        assert!(worlds[0].busy.contains("b"));
    }

    #[test]
    fn cap_counts_worlds_in_every_state() {
        let store = store(vec![arena("a", 2)]);
        assert_eq!(
            allocate(&store, &mut no_worlds(), 1, GAME, 2),
            Ok(new_world("a"))
        );
        for readiness in [
            WorldReadiness::Creating,
            WorldReadiness::Removing,
            WorldReadiness::DestroyingDomain,
        ] {
            let mut worlds = [world("w1", GAME, readiness, &[])];
            assert_eq!(
                allocate(&store, &mut worlds, 1, GAME, 2),
                Err(Rejection::NoCapacity)
            );
        }
    }

    #[test]
    fn last_arena_goes_to_one_request_only() {
        let store = store(vec![arena("a", 2)]);
        let mut worlds = [world("w1", GAME, WorldReadiness::Ready, &[])];
        assert_eq!(
            allocate(&store, &mut worlds, 2, GAME, 2),
            Ok(existing("w1", "a"))
        );
        // Below the cap the second request gets a new world; at the cap it is rejected.
        assert_eq!(
            allocate(&store, &mut worlds, 2, GAME, 2),
            Ok(new_world("a"))
        );
        assert_eq!(
            allocate(&store, &mut worlds, 1, GAME, 2),
            Err(Rejection::NoCapacity)
        );
    }

    #[test]
    fn incomplete_arena_is_ignored() {
        let mut gap = arena("gap", 3);
        gap.spawns.remove(1);
        let store = store(vec![arena("one", 1), gap, arena("ok", 2)]);
        assert_eq!(
            allocate(&store, &mut no_worlds(), 1, GAME, 2),
            Ok(new_world("ok"))
        );
        let only_incomplete = self::store(vec![arena("one", 1)]);
        assert_eq!(
            allocate(&only_incomplete, &mut no_worlds(), 1, GAME, 1),
            Err(Rejection::NoArena {
                game_type: GAME.to_owned(),
                player_count: 1
            })
        );
    }

    #[test]
    fn slot_count_must_equal_player_count() {
        let store = store(vec![arena("a", 4)]);
        let error = allocate(&store, &mut no_worlds(), 1, GAME, 2).unwrap_err();
        assert_eq!(
            error,
            Rejection::NoArena {
                game_type: GAME.to_owned(),
                player_count: 2
            }
        );
        assert_eq!(
            error.to_string(),
            "no complete arena for duel with 2 players"
        );
        assert_eq!(
            allocate(&store, &mut no_worlds(), 1, GAME, 4),
            Ok(new_world("a"))
        );
    }

    #[test]
    fn unknown_game_type_is_rejected() {
        let store = store(vec![arena("a", 2)]);
        let error = allocate(&store, &mut no_worlds(), 1, "ctf", 2).unwrap_err();
        assert_eq!(error, Rejection::UnknownGameType("ctf".to_owned()));
        assert_eq!(error.to_string(), "unknown game type ctf");
    }

    #[test]
    fn world_of_another_game_type_is_not_used() {
        let store = store(vec![arena("a", 2)]);
        let mut worlds = [world("w1", "ctf", WorldReadiness::Ready, &[])];
        assert_eq!(
            allocate(&store, &mut worlds, 2, GAME, 2),
            Ok(new_world("a"))
        );
        assert!(worlds[0].busy.is_empty());
    }

    #[test]
    fn creating_world_is_not_reused() {
        let store = store(vec![arena("a", 2)]);
        let mut worlds = [world("w1", GAME, WorldReadiness::Creating, &[])];
        assert_eq!(
            allocate(&store, &mut worlds, 2, GAME, 2),
            Ok(new_world("a"))
        );
    }

    #[test]
    fn idle_worlds_are_ready_and_empty() {
        let worlds = [
            world("idle", GAME, WorldReadiness::Ready, &[]),
            world("busy", GAME, WorldReadiness::Ready, &["a"]),
            world("creating", GAME, WorldReadiness::Creating, &[]),
        ];
        let keys: Vec<&str> = idle_worlds(&worlds)
            .map(|world| world.key.as_str())
            .collect();
        assert_eq!(keys, ["idle"]);
    }
}
