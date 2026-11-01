//! Persistent user, warp and spawn data as TOML files in the plugin data directory.
//!
//! Layout mirrors EssentialsX: `userdata/<uuid>.toml`, `warps.toml` and `spawn.toml`.
//! Writes go to a temporary file that is renamed over the old one, so a crash never leaves a
//! half-written file. Unreadable files are renamed to `*.broken` and reported, never overwritten.

use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};
use steel_extension_api::{events::Location, log, players::Uuid};

/// A stored world position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Loc {
    /// World key.
    pub world: String,
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
    /// Z coordinate.
    pub z: f64,
    /// Yaw in degrees.
    #[serde(default)]
    pub yaw: f32,
    /// Pitch in degrees.
    #[serde(default)]
    pub pitch: f32,
}

impl From<&Location> for Loc {
    fn from(location: &Location) -> Self {
        Self {
            world: location.world.to_string(),
            x: location.x,
            y: location.y,
            z: location.z,
            yaw: location.yaw,
            pitch: location.pitch,
        }
    }
}

impl Loc {
    /// The API form of this location.
    #[must_use]
    pub fn to_api(&self) -> Location {
        Location {
            world: self.world.as_str().into(),
            x: self.x,
            y: self.y,
            z: self.z,
            yaw: self.yaw,
            pitch: self.pitch,
        }
    }
}

/// One player's persistent data.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct User {
    /// Last known profile name.
    pub name: String,
    /// Nickname with `&` codes.
    pub nickname: Option<String>,
    /// Balance in cents.
    pub money: i64,
    /// Whether god mode is on.
    pub god: bool,
    /// Homes by lowercase name.
    pub homes: BTreeMap<String, Loc>,
    /// Location before the last teleport or death, for `/back`.
    pub last_location: Option<Loc>,
    /// Unix seconds of each kit's last use.
    pub kit_cooldowns: BTreeMap<String, u64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct WarpsFile {
    #[serde(default)]
    warps: BTreeMap<String, Loc>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SpawnFile {
    spawn: Option<Loc>,
}

/// All loaded data and what changed since the last save.
#[derive(Debug, Default)]
pub struct Data {
    /// Users by UUID.
    pub users: BTreeMap<Uuid, User>,
    /// Warps by lowercase name.
    pub warps: BTreeMap<String, Loc>,
    /// Server spawn.
    pub spawn: Option<Loc>,
    dirty_users: BTreeSet<Uuid>,
    warps_dirty: bool,
    spawn_dirty: bool,
}

impl Data {
    /// Mutable access to a user, marking it for saving.
    pub fn user_mut(&mut self, uuid: Uuid) -> &mut User {
        self.dirty_users.insert(uuid);
        self.users.entry(uuid).or_default()
    }

    /// Marks warps for saving.
    pub const fn touch_warps(&mut self) {
        self.warps_dirty = true;
    }

    /// Marks the spawn for saving.
    pub const fn touch_spawn(&mut self) {
        self.spawn_dirty = true;
    }

    /// Whether anything needs saving.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        !self.dirty_users.is_empty() || self.warps_dirty || self.spawn_dirty
    }
}

/// Reads and writes the data directory.
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    /// Uses `root` as the data directory.
    #[must_use]
    pub const fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn users_dir(&self) -> PathBuf {
        self.root.join("userdata")
    }

    /// Loads everything; unreadable files are set aside and reported.
    pub fn load(&self) -> Result<Data, String> {
        let mut data = Data::default();
        let users = self.users_dir();
        fs::create_dir_all(&users)
            .map_err(|error| format!("cannot create {}: {error}", users.display()))?;
        let entries = fs::read_dir(&users)
            .map_err(|error| format!("cannot read {}: {error}", users.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "toml") {
                continue;
            }
            let Some(uuid) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(Uuid::parse)
            else {
                continue;
            };
            if let Some(user) = read::<User>(&path) {
                data.users.insert(uuid, user);
            }
        }
        if let Some(file) = read::<WarpsFile>(&self.root.join("warps.toml")) {
            data.warps = file.warps;
        }
        if let Some(file) = read::<SpawnFile>(&self.root.join("spawn.toml")) {
            data.spawn = file.spawn;
        }
        Ok(data)
    }

    /// Writes everything marked dirty. Failed writes stay dirty for the next attempt.
    pub fn save(&self, data: &mut Data) {
        let dirty = std::mem::take(&mut data.dirty_users);
        for uuid in dirty {
            let Some(user) = data.users.get(&uuid) else {
                continue;
            };
            let path = self.users_dir().join(format!("{uuid}.toml"));
            if let Err(error) = write(&path, user) {
                log::error(error);
                data.dirty_users.insert(uuid);
            }
        }
        if data.warps_dirty {
            let file = WarpsFile {
                warps: data.warps.clone(),
            };
            data.warps_dirty = write(&self.root.join("warps.toml"), &file)
                .map_err(log::error)
                .is_err();
        }
        if data.spawn_dirty {
            let file = SpawnFile {
                spawn: data.spawn.clone(),
            };
            data.spawn_dirty = write(&self.root.join("spawn.toml"), &file)
                .map_err(log::error)
                .is_err();
        }
    }
}

/// Reads a TOML file; a missing file is `None`, an unreadable one is renamed and `None`.
fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            log::error(format!("cannot read {}: {error}", path.display()));
            return None;
        }
    };
    match toml::from_str(&text) {
        Ok(value) => Some(value),
        Err(error) => {
            let broken = path.with_extension("toml.broken");
            log::error(format!(
                "{} is invalid ({error}); moved to {}",
                path.display(),
                broken.display()
            ));
            let _ = fs::rename(path, broken);
            None
        }
    }
}

/// Atomically replaces `path` with `value` serialized as TOML.
fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let text = toml::to_string(value)
        .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, text)
        .and_then(|()| fs::rename(&temporary, path))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("essentials-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn loc(x: f64) -> Loc {
        Loc {
            world: "minecraft:overworld".into(),
            x,
            y: 64.0,
            z: -3.5,
            yaw: 90.0,
            pitch: 0.0,
        }
    }

    #[test]
    fn data_round_trips_through_files() -> Result<(), String> {
        let storage = Storage::new(temp_dir("round-trip"));
        let mut data = storage.load()?;
        let uuid = Uuid { bytes: [7; 16] };
        let user = data.user_mut(uuid);
        user.name = "Steve".into();
        user.money = 12_345;
        user.homes.insert("base".into(), loc(1.0));
        user.kit_cooldowns.insert("starter".into(), 42);
        data.warps.insert("shop".into(), loc(2.0));
        data.touch_warps();
        data.spawn = Some(loc(3.0));
        data.touch_spawn();
        storage.save(&mut data);
        assert!(!data.is_dirty());

        let loaded = storage.load()?;
        assert_eq!(loaded.users.get(&uuid), data.users.get(&uuid));
        assert_eq!(loaded.warps, data.warps);
        assert_eq!(loaded.spawn, data.spawn);
        Ok(())
    }

    #[test]
    fn invalid_files_are_set_aside_not_overwritten() -> Result<(), String> {
        let root = temp_dir("broken");
        let storage = Storage::new(root.clone());
        storage.load()?;
        let uuid = Uuid { bytes: [1; 16] };
        let path = root.join("userdata").join(format!("{uuid}.toml"));
        fs::write(&path, "money = \"lots\"").map_err(|e| e.to_string())?;
        let data = storage.load()?;
        assert!(!data.users.contains_key(&uuid));
        assert!(
            root.join("userdata")
                .join(format!("{uuid}.toml.broken"))
                .exists()
        );
        Ok(())
    }
}
