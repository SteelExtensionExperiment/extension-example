//! Arenas and lobby persisted in `arenas.toml`.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// First spawn slot; slots are numbered from 1.
const FIRST_SLOT: u32 = 1;
/// Fewest spawn slots an arena needs to be complete.
const MIN_COMPLETE_SLOTS: usize = 2;

/// Game mode players get when they return to the lobby.
///
/// Defined here because `steel_extension_api::GameMode` has no serde or string parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LobbyGameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl From<LobbyGameMode> for steel_extension_api::GameMode {
    fn from(mode: LobbyGameMode) -> Self {
        match mode {
            LobbyGameMode::Survival => Self::Survival,
            LobbyGameMode::Creative => Self::Creative,
            LobbyGameMode::Adventure => Self::Adventure,
            LobbyGameMode::Spectator => Self::Spectator,
        }
    }
}

/// Where players return after a match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lobby {
    /// World key at the time of `/arena lobby set`.
    pub world: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub game_mode: LobbyGameMode,
}

/// A spawn point; coordinates only, since every world cloned for the game type is identical.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spawn {
    /// Starts at 1.
    pub slot: u32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

/// Named set of spawn points for one game type. Spawns are kept sorted by slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arena {
    /// Unique within the game type.
    pub name: String,
    #[serde(default)]
    pub spawns: Vec<Spawn>,
}

impl Arena {
    pub const fn slot_count(&self) -> usize {
        self.spawns.len()
    }

    /// Slots are contiguous from 1 and number at least 2. Derived, never stored.
    pub fn is_complete(&self) -> bool {
        self.spawns.len() >= MIN_COMPLETE_SLOTS
            && self
                .spawns
                .iter()
                .zip(FIRST_SLOT..)
                .all(|(spawn, slot)| spawn.slot == slot)
    }
}

/// A game type with its arenas and the world-cloner template it clones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameTypeEntry {
    pub name: String,
    /// Unset until `/arena template`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(default)]
    pub arenas: Vec<Arena>,
}

/// Contents of `arenas.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ArenaStore {
    /// Absent until `/arena lobby set`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lobby: Option<Lobby>,
    pub game_types: Vec<GameTypeEntry>,
}

fn unknown_arena(game_type: &str, arena: &str) -> String {
    format!("unknown arena {game_type}/{arena}")
}

impl ArenaStore {
    /// Reads `path`; a missing file is an empty store.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
        };
        let mut store: Self = toml::from_str(&text)
            .map_err(|error| format!("{} is invalid: {error}", path.display()))?;
        store
            .validate()
            .map_err(|error| format!("{} is invalid: {error}", path.display()))?;
        Ok(store)
    }

    /// Writes a temporary file next to `path` and renames it over the old one.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string(self)
            .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
        let temporary = path.with_extension("toml.tmp");
        let write = || {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&temporary, &text)?;
            fs::rename(&temporary, path)
        };
        write().map_err(|error| format!("cannot write {}: {error}", path.display()))
    }

    /// Checks uniqueness and slots, and sorts spawns by slot.
    fn validate(&mut self) -> Result<(), String> {
        let mut game_types = HashSet::new();
        for game_type in &mut self.game_types {
            if !game_types.insert(game_type.name.clone()) {
                return Err(format!("duplicate game type {}", game_type.name));
            }
            let mut arenas = HashSet::new();
            for arena in &mut game_type.arenas {
                let id = format!("{}/{}", game_type.name, arena.name);
                if !arenas.insert(arena.name.clone()) {
                    return Err(format!("duplicate arena {id}"));
                }
                arena.spawns.sort_by_key(|spawn| spawn.slot);
                if arena.spawns.iter().any(|spawn| spawn.slot < FIRST_SLOT) {
                    return Err(format!("arena {id} has a slot below {FIRST_SLOT}"));
                }
                if let Some(pair) = arena
                    .spawns
                    .windows(2)
                    .find(|pair| pair[0].slot == pair[1].slot)
                {
                    return Err(format!("arena {id} has duplicate slot {}", pair[0].slot));
                }
            }
        }
        Ok(())
    }

    pub fn game_type(&self, name: &str) -> Option<&GameTypeEntry> {
        self.game_types
            .iter()
            .find(|game_type| game_type.name == name)
    }

    pub fn arena(&self, game_type: &str, arena: &str) -> Result<&Arena, String> {
        self.game_type(game_type)
            .and_then(|entry| {
                entry
                    .arenas
                    .iter()
                    .find(|candidate| candidate.name == arena)
            })
            .ok_or_else(|| unknown_arena(game_type, arena))
    }

    fn arena_mut(&mut self, game_type: &str, arena: &str) -> Result<&mut Arena, String> {
        self.game_types
            .iter_mut()
            .find(|entry| entry.name == game_type)
            .and_then(|entry| {
                entry
                    .arenas
                    .iter_mut()
                    .find(|candidate| candidate.name == arena)
            })
            .ok_or_else(|| unknown_arena(game_type, arena))
    }

    /// Creates an empty arena, adding the game type when it is new.
    pub fn create_arena(&mut self, game_type: &str, arena: &str) -> Result<(), String> {
        if self.arena(game_type, arena).is_ok() {
            return Err(format!("arena {arena} already exists for {game_type}"));
        }
        let index = self
            .game_types
            .iter()
            .position(|entry| entry.name == game_type)
            .unwrap_or_else(|| {
                self.game_types.push(GameTypeEntry {
                    name: game_type.to_owned(),
                    template: None,
                    arenas: Vec::new(),
                });
                self.game_types.len() - 1
            });
        self.game_types[index].arenas.push(Arena {
            name: arena.to_owned(),
            spawns: Vec::new(),
        });
        Ok(())
    }

    /// Removes the arena; the game type stays, since it holds the template.
    pub fn remove_arena(&mut self, game_type: &str, arena: &str) -> Result<(), String> {
        self.arena(game_type, arena)?;
        if let Some(entry) = self
            .game_types
            .iter_mut()
            .find(|entry| entry.name == game_type)
        {
            entry.arenas.retain(|candidate| candidate.name != arena);
        }
        Ok(())
    }

    /// Stores `spawn`, replacing any spawn with the same slot.
    pub fn set_spawn(&mut self, game_type: &str, arena: &str, spawn: Spawn) -> Result<(), String> {
        if spawn.slot < FIRST_SLOT {
            return Err(format!("slot must be {FIRST_SLOT} or higher"));
        }
        let arena = self.arena_mut(game_type, arena)?;
        arena.spawns.retain(|existing| existing.slot != spawn.slot);
        arena.spawns.push(spawn);
        arena.spawns.sort_by_key(|existing| existing.slot);
        Ok(())
    }

    pub fn remove_spawn(&mut self, game_type: &str, arena: &str, slot: u32) -> Result<(), String> {
        let found = self.arena_mut(game_type, arena)?;
        let before = found.spawns.len();
        found.spawns.retain(|spawn| spawn.slot != slot);
        if found.spawns.len() == before {
            return Err(format!("arena {game_type}/{arena} has no spawn {slot}"));
        }
        Ok(())
    }

    pub fn set_template(&mut self, game_type: &str, template: &str) -> Result<(), String> {
        let entry = self
            .game_types
            .iter_mut()
            .find(|entry| entry.name == game_type)
            .ok_or_else(|| format!("unknown game type {game_type}"))?;
        entry.template = Some(template.to_owned());
        Ok(())
    }

    pub fn set_lobby(&mut self, lobby: Lobby) {
        self.lobby = Some(lobby);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("arena-manager-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn spawn(slot: u32) -> Spawn {
        Spawn {
            slot,
            x: 10.5,
            y: 70.0,
            z: -3.5,
            yaw: 90.0,
            pitch: 0.0,
        }
    }

    fn arena_with_slots(slots: &[u32]) -> Result<ArenaStore, String> {
        let mut store = ArenaStore::default();
        store.create_arena("skywars", "north")?;
        for slot in slots {
            store.set_spawn("skywars", "north", spawn(*slot))?;
        }
        Ok(store)
    }

    fn load_text(name: &str, text: &str) -> Result<ArenaStore, String> {
        let dir = temp_dir(name);
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let path = dir.join("arenas.toml");
        fs::write(&path, text).map_err(|error| error.to_string())?;
        ArenaStore::load(&path)
    }

    #[test]
    fn round_trips_through_file() -> Result<(), String> {
        let mut store = arena_with_slots(&[2, 1])?;
        store.create_arena("skywars", "south")?;
        store.create_arena("spleef", "pit")?;
        store.set_template("skywars", "skywars_a")?;
        store.set_lobby(Lobby {
            world: "lobby:main".into(),
            x: 0.5,
            y: 64.0,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
            game_mode: LobbyGameMode::Adventure,
        });

        let path = temp_dir("round-trip").join("arenas.toml");
        store.save(&path)?;
        assert!(!path.with_extension("toml.tmp").exists());
        let loaded = ArenaStore::load(&path)?;
        assert_eq!(loaded, store);
        assert_eq!(loaded.arena("skywars", "north")?.spawns[0].slot, 1);
        Ok(())
    }

    #[test]
    fn missing_file_is_empty_store() -> Result<(), String> {
        let store = ArenaStore::load(&temp_dir("missing").join("arenas.toml"))?;
        assert_eq!(store, ArenaStore::default());
        Ok(())
    }

    #[test]
    fn duplicates_are_rejected_on_load() {
        let game_type = "[[game_types]]\nname = \"skywars\"\n";
        let arena = "[[game_types.arenas]]\nname = \"north\"\n";
        let spawn = "[[game_types.arenas.spawns]]\nslot = 1\nx = 0.0\ny = 0.0\nz = 0.0\nyaw = 0.0\npitch = 0.0\n";
        let cases = [
            (
                "dup-type",
                format!("{game_type}{game_type}"),
                "duplicate game type skywars",
            ),
            (
                "dup-arena",
                format!("{game_type}{arena}{arena}"),
                "duplicate arena skywars/north",
            ),
            (
                "dup-slot",
                format!("{game_type}{arena}{spawn}{spawn}"),
                "duplicate slot 1",
            ),
        ];
        for (name, text, expected) in cases {
            let error = load_text(name, &text).unwrap_err();
            assert!(
                error.contains(expected) && error.contains("arenas.toml"),
                "{error}"
            );
        }
    }

    #[test]
    fn invalid_game_mode_and_slot_zero_are_rejected() {
        let lobby = "[lobby]\nworld = \"a:b\"\nx = 0.0\ny = 0.0\nz = 0.0\nyaw = 0.0\npitch = 0.0\ngame_mode = \"hardcore\"\n";
        let error = load_text("bad-mode", lobby).unwrap_err();
        assert!(
            error.contains("arenas.toml") && error.contains("hardcore"),
            "{error}"
        );

        let error = arena_with_slots(&[0]).unwrap_err();
        assert!(error.contains("slot"), "{error}");
    }

    #[test]
    fn completeness_needs_contiguous_slots_and_at_least_two() -> Result<(), String> {
        for (slots, complete) in [
            (&[1, 2][..], true),
            (&[1, 2, 3], true),
            (&[1, 3], false),
            (&[1], false),
            (&[2, 3], false),
        ] {
            let store = arena_with_slots(slots)?;
            let arena = store.arena("skywars", "north")?;
            assert_eq!(arena.is_complete(), complete, "{slots:?}");
            assert_eq!(arena.slot_count(), slots.len());
        }
        Ok(())
    }

    #[test]
    fn filling_a_gap_completes_the_arena() -> Result<(), String> {
        let mut store = arena_with_slots(&[1, 3])?;
        store.set_spawn("skywars", "north", spawn(2))?;
        assert!(store.arena("skywars", "north")?.is_complete());
        store.remove_spawn("skywars", "north", 2)?;
        assert!(!store.arena("skywars", "north")?.is_complete());
        Ok(())
    }

    #[test]
    fn mutation_errors_match_the_command_contract() -> Result<(), String> {
        let mut store = arena_with_slots(&[1])?;
        assert_eq!(
            store.create_arena("skywars", "north"),
            Err("arena north already exists for skywars".into())
        );
        assert_eq!(
            store.remove_arena("skywars", "west"),
            Err("unknown arena skywars/west".into())
        );
        assert_eq!(
            store.set_spawn("spleef", "pit", spawn(1)),
            Err("unknown arena spleef/pit".into())
        );
        assert_eq!(
            store.remove_spawn("skywars", "north", 4),
            Err("arena skywars/north has no spawn 4".into())
        );
        store.remove_arena("skywars", "north")?;
        assert!(store.game_type("skywars").is_some());
        Ok(())
    }

    #[test]
    fn corrupt_file_names_file_and_cause() {
        let error = load_text("corrupt", "game_types = [").unwrap_err();
        assert!(error.contains("arenas.toml"), "{error}");
        assert!(error.contains("invalid"), "{error}");
        assert!(error.contains("expected"), "{error}");
    }

    #[test]
    fn save_replaces_existing_file() -> Result<(), String> {
        let path = temp_dir("replace").join("arenas.toml");
        arena_with_slots(&[1])?.save(&path)?;
        let second = arena_with_slots(&[1, 2])?;
        second.save(&path)?;
        assert_eq!(ArenaStore::load(&path)?, second);
        assert!(!path.with_extension("toml.tmp").exists());
        Ok(())
    }
}
