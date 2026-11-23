//! Persisted state: game types, signs, kit choices and the open/closed switch.
//!
//! Each file is TOML in the extension data directory, written atomically. A file that cannot be
//! read or fails validation refuses to start the extension, naming the file and the cause.

use std::{
    collections::{BTreeSet, HashMap},
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use steel_extension_api::players::Uuid;

use crate::{
    kits::Kit,
    signs::{Catalog, Sign, SignKind, SignPos, Signs},
};

/// Smallest player count of a game type.
pub(crate) const MIN_PLAYERS: u32 = 2;

const GAME_TYPES_FILE: &str = "gametypes.toml";
const SIGNS_FILE: &str = "signs.toml";
const CHOICES_FILE: &str = "choices.toml";
const STATE_FILE: &str = "state.toml";
const SIGN_KIND_QUEUE: &str = "queue";
const SIGN_KIND_KIT: &str = "kit";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StoreError {
    file: &'static str,
    cause: String,
}

impl StoreError {
    fn new(file: &'static str, cause: impl fmt::Display) -> Self {
        Self {
            file,
            cause: cause.to_string(),
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file, self.cause)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GameType {
    pub name: String,
    /// Players per match (N).
    pub players: u32,
    pub enabled: bool,
    pub kits: Vec<Kit>,
}

/// The kit each player chose per game type.
pub(crate) type Choices = HashMap<(Uuid, String), String>;

#[derive(Debug)]
pub(crate) struct Store {
    dir: PathBuf,
    pub game_types: Vec<GameType>,
    pub signs: Signs,
    pub choices: Choices,
    pub open: bool,
}

impl Catalog for Store {
    fn kits(&self, game_type: &str) -> Option<&[Kit]> {
        self.game_type(game_type).map(|found| found.kits.as_slice())
    }
}

#[derive(Serialize, Deserialize, Default)]
struct GameTypesFile {
    #[serde(default)]
    game_types: Vec<GameTypeEntry>,
}

#[derive(Serialize, Deserialize)]
struct GameTypeEntry {
    name: String,
    players: u32,
    enabled: bool,
    #[serde(default)]
    kits: Vec<KitEntry>,
}

#[derive(Serialize, Deserialize)]
struct KitEntry {
    name: String,
}

#[derive(Serialize, Deserialize, Default)]
struct SignsFile {
    #[serde(default)]
    next_id: u32,
    #[serde(default)]
    signs: Vec<SignEntry>,
}

#[derive(Serialize, Deserialize)]
struct SignEntry {
    id: String,
    world: String,
    x: i32,
    y: i32,
    z: i32,
    kind: String,
    game_type: String,
    #[serde(default)]
    kit: String,
}

#[derive(Serialize, Deserialize, Default)]
struct ChoicesFile {
    #[serde(default)]
    choices: Vec<ChoiceEntry>,
}

#[derive(Serialize, Deserialize)]
struct ChoiceEntry {
    uuid: String,
    game_type: String,
    kit: String,
}

#[derive(Serialize, Deserialize, Default)]
struct StateFile {
    #[serde(default)]
    open: bool,
}

fn read<T: DeserializeOwned + Default>(dir: &Path, file: &'static str) -> Result<T, StoreError> {
    let path = dir.join(file);
    if !path.exists() {
        return Ok(T::default());
    }
    let text = fs::read_to_string(&path).map_err(|error| StoreError::new(file, error))?;
    toml::from_str(&text).map_err(|error| StoreError::new(file, error))
}

/// Atomically replaces the file: write a temporary file, then rename.
fn write<T: Serialize>(dir: &Path, file: &'static str, value: &T) -> Result<(), StoreError> {
    let text = toml::to_string(value).map_err(|error| StoreError::new(file, error))?;
    let path = dir.join(file);
    let temporary = path.with_extension("toml.tmp");
    fs::write(&temporary, text)
        .and_then(|()| fs::rename(&temporary, &path))
        .map_err(|error| StoreError::new(file, error))
}

fn game_types_from(file: GameTypesFile) -> Result<Vec<GameType>, StoreError> {
    let mut names = BTreeSet::new();
    let mut game_types = Vec::new();
    for entry in file.game_types {
        if !names.insert(entry.name.clone()) {
            return Err(StoreError::new(
                GAME_TYPES_FILE,
                format!("duplicate game type {}", entry.name),
            ));
        }
        if entry.players < MIN_PLAYERS {
            let cause = format!(
                "game type {} needs at least {MIN_PLAYERS} players",
                entry.name
            );
            return Err(StoreError::new(GAME_TYPES_FILE, cause));
        }
        let mut kit_names = BTreeSet::new();
        let mut kits = Vec::new();
        for kit in entry.kits {
            if !kit_names.insert(kit.name.clone()) {
                let cause = format!("duplicate kit {}/{}", entry.name, kit.name);
                return Err(StoreError::new(GAME_TYPES_FILE, cause));
            }
            kits.push(Kit { name: kit.name });
        }
        game_types.push(GameType {
            name: entry.name,
            players: entry.players,
            enabled: entry.enabled,
            kits,
        });
    }
    Ok(game_types)
}

fn signs_from(file: SignsFile) -> Result<Signs, StoreError> {
    let mut ids = BTreeSet::new();
    let mut positions = Vec::new();
    let mut signs = Vec::new();
    for entry in file.signs {
        let pos = SignPos {
            world: entry.world,
            x: entry.x,
            y: entry.y,
            z: entry.z,
        };
        if !ids.insert(entry.id.clone()) || positions.contains(&pos) {
            return Err(StoreError::new(
                SIGNS_FILE,
                format!("duplicate sign {}", entry.id),
            ));
        }
        let kind = match (entry.kind.as_str(), entry.kit) {
            (SIGN_KIND_QUEUE, _) => SignKind::Queue {
                game_type: entry.game_type,
            },
            (SIGN_KIND_KIT, kit) if !kit.is_empty() => SignKind::Kit {
                game_type: entry.game_type,
                kit,
            },
            (SIGN_KIND_KIT, _) => {
                return Err(StoreError::new(
                    SIGNS_FILE,
                    format!("kit sign {} has no kit", entry.id),
                ));
            }
            (other, _) => {
                return Err(StoreError::new(
                    SIGNS_FILE,
                    format!("sign {} has unknown kind {other}", entry.id),
                ));
            }
        };
        positions.push(pos.clone());
        signs.push(Sign {
            id: entry.id,
            pos,
            kind,
        });
    }
    Ok(Signs::from_parts(signs, file.next_id))
}

fn choices_from(file: ChoicesFile) -> Result<Choices, StoreError> {
    file.choices
        .into_iter()
        .map(|entry| {
            let uuid = Uuid::parse(&entry.uuid).ok_or_else(|| {
                StoreError::new(CHOICES_FILE, format!("invalid uuid {}", entry.uuid))
            })?;
            Ok(((uuid, entry.game_type), entry.kit))
        })
        .collect()
}

impl Store {
    /// Loads every file; a missing file starts empty.
    pub(crate) fn load(dir: PathBuf) -> Result<Self, StoreError> {
        Ok(Self {
            game_types: game_types_from(read(&dir, GAME_TYPES_FILE)?)?,
            signs: signs_from(read(&dir, SIGNS_FILE)?)?,
            choices: choices_from(read(&dir, CHOICES_FILE)?)?,
            open: read::<StateFile>(&dir, STATE_FILE)?.open,
            dir,
        })
    }

    pub(crate) fn game_type(&self, name: &str) -> Option<&GameType> {
        self.game_types
            .iter()
            .find(|game_type| game_type.name == name)
    }

    pub(crate) fn save_game_types(&self) -> Result<(), StoreError> {
        let game_types = self
            .game_types
            .iter()
            .map(|game_type| GameTypeEntry {
                name: game_type.name.clone(),
                players: game_type.players,
                enabled: game_type.enabled,
                kits: game_type
                    .kits
                    .iter()
                    .map(|kit| KitEntry {
                        name: kit.name.clone(),
                    })
                    .collect(),
            })
            .collect();
        write(&self.dir, GAME_TYPES_FILE, &GameTypesFile { game_types })
    }

    pub(crate) fn save_signs(&self) -> Result<(), StoreError> {
        let signs = self
            .signs
            .list()
            .iter()
            .map(|sign| {
                let (kind, game_type, kit) = match &sign.kind {
                    SignKind::Queue { game_type } => (SIGN_KIND_QUEUE, game_type, String::new()),
                    SignKind::Kit { game_type, kit } => (SIGN_KIND_KIT, game_type, kit.clone()),
                };
                SignEntry {
                    id: sign.id.clone(),
                    world: sign.pos.world.clone(),
                    x: sign.pos.x,
                    y: sign.pos.y,
                    z: sign.pos.z,
                    kind: kind.to_owned(),
                    game_type: game_type.clone(),
                    kit,
                }
            })
            .collect();
        write(
            &self.dir,
            SIGNS_FILE,
            &SignsFile {
                next_id: self.signs.next_id(),
                signs,
            },
        )
    }

    pub(crate) fn save_choices(&self) -> Result<(), StoreError> {
        let mut choices: Vec<ChoiceEntry> = self
            .choices
            .iter()
            .map(|((uuid, game_type), kit)| ChoiceEntry {
                uuid: uuid.to_string(),
                game_type: game_type.clone(),
                kit: kit.clone(),
            })
            .collect();
        choices.sort_by(|a, b| (&a.uuid, &a.game_type).cmp(&(&b.uuid, &b.game_type)));
        write(&self.dir, CHOICES_FILE, &ChoicesFile { choices })
    }

    pub(crate) fn save_state(&self) -> Result<(), StoreError> {
        write(&self.dir, STATE_FILE, &StateFile { open: self.open })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("game-orchestrator-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn everything_survives_a_round_trip() {
        let dir = temp_dir("round-trip");
        let mut store = Store::load(dir.clone()).unwrap();
        assert!(store.game_types.is_empty() && !store.open);
        store.game_types.push(GameType {
            name: "skywars".into(),
            players: 2,
            enabled: false,
            kits: vec![Kit {
                name: "knight".into(),
            }],
        });
        let pos = SignPos {
            world: "lobby:main".into(),
            x: 1,
            y: 2,
            z: 3,
        };
        store.signs.register(
            pos,
            SignKind::Kit {
                game_type: "skywars".into(),
                kit: "knight".into(),
            },
        );
        store.choices.insert(
            (
                Uuid::parse("3f5c3a52-6c35-4d79-9d60-0a8b3c7f12aa").unwrap(),
                "skywars".into(),
            ),
            "knight".into(),
        );
        store.open = true;
        store.save_game_types().unwrap();
        store.save_signs().unwrap();
        store.save_choices().unwrap();
        store.save_state().unwrap();

        let loaded = Store::load(dir).unwrap();
        assert_eq!(loaded.game_types, store.game_types);
        assert_eq!(loaded.signs.list(), store.signs.list());
        assert_eq!(loaded.signs.next_id(), store.signs.next_id());
        assert_eq!(loaded.choices, store.choices);
        assert!(loaded.open);
    }

    #[test]
    fn invalid_files_name_the_file_and_the_cause() {
        for (file, text, cause) in [
            (
                GAME_TYPES_FILE,
                "[[game_types]]\nname='a'\nplayers=2\nenabled=true\n[[game_types]]\nname='a'\nplayers=2\nenabled=true",
                "duplicate game type a",
            ),
            (
                GAME_TYPES_FILE,
                "[[game_types]]\nname='a'\nplayers=1\nenabled=true",
                "at least 2",
            ),
            (
                GAME_TYPES_FILE,
                "[[game_types]]\nname='a'\nplayers=2\nenabled=true\n[[game_types.kits]]\nname='k'\n[[game_types.kits]]\nname='k'",
                "duplicate kit a/k",
            ),
            (
                SIGNS_FILE,
                "[[signs]]\nid='kit-1'\nworld='w'\nx=0\ny=0\nz=0\nkind='kit'\ngame_type='a'",
                "has no kit",
            ),
            (
                CHOICES_FILE,
                "[[choices]]\nuuid='nope'\ngame_type='a'\nkit='k'",
                "invalid uuid",
            ),
            (STATE_FILE, "open = [", ""),
        ] {
            let dir = temp_dir("invalid");
            fs::write(dir.join(file), text).unwrap();
            let message = Store::load(dir).unwrap_err().to_string();
            assert!(
                message.starts_with(file) && message.contains(cause),
                "{message}"
            );
        }
    }
}
