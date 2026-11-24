//! Persistence of stats and boards as TOML files in the extension directory.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use steel_extension_api::players::Uuid;

use crate::stats::{GameRecord, PlayerRecord, Stats};

const STATS_FILE: &str = "stats.toml";
const BOARDS_FILE: &str = "boards.toml";
const BROKEN_SUFFIX: &str = ".broken";
const TEMPORARY_SUFFIX: &str = ".tmp";

/// A board as stored on disk; the runtime board with its hologram handle lives in `boards`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardRecord {
    pub id: String,
    pub world: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub game_type: String,
}

/// Result of a load. `error` is set when the file could not be used (the caller logs it,
/// since logging needs the host); `value` is then the empty state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded<T> {
    pub value: T,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Default)]
struct StatsFile {
    #[serde(default)]
    players: Vec<PlayerEntry>,
}

#[derive(Serialize, Deserialize)]
struct PlayerEntry {
    uuid: String,
    name: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    games: BTreeMap<String, GameEntry>,
}

#[derive(Serialize, Deserialize)]
struct GameEntry {
    wins: u32,
    losses: u32,
}

#[derive(Serialize, Deserialize, Default)]
struct BoardsFile {
    #[serde(default)]
    boards: Vec<BoardRecord>,
}

impl StatsFile {
    fn from_stats(stats: &Stats) -> Self {
        let mut players: Vec<PlayerEntry> = stats
            .players()
            .iter()
            .map(|(uuid, record)| PlayerEntry {
                uuid: uuid.to_string(),
                name: record.name.clone(),
                games: record
                    .games
                    .iter()
                    .filter(|(_, game)| game.wins != 0 || game.losses != 0)
                    .map(|(game_type, game)| {
                        (
                            game_type.clone(),
                            GameEntry {
                                wins: game.wins,
                                losses: game.losses,
                            },
                        )
                    })
                    .collect(),
            })
            .collect();
        players.sort_by(|a, b| a.uuid.cmp(&b.uuid));
        Self { players }
    }

    fn into_stats(self) -> Result<Stats, String> {
        let mut players = HashMap::new();
        for entry in self.players {
            let uuid = Uuid::parse(&entry.uuid)
                .ok_or_else(|| format!("invalid player uuid {:?}", entry.uuid))?;
            let record = PlayerRecord {
                name: entry.name,
                games: entry
                    .games
                    .into_iter()
                    .map(|(game_type, game)| {
                        (
                            game_type,
                            GameRecord {
                                wins: game.wins,
                                losses: game.losses,
                            },
                        )
                    })
                    .collect(),
            };
            if players.insert(uuid, record).is_some() {
                return Err(format!("duplicate player uuid {uuid}"));
            }
        }
        Ok(Stats::from_players(players))
    }
}

/// Reads and writes the extension's data files in one directory.
#[derive(Debug, Clone)]
pub struct Storage {
    directory: PathBuf,
}

impl Storage {
    #[must_use]
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    #[must_use]
    pub fn load_stats(&self) -> Loaded<Stats> {
        load(&self.directory.join(STATS_FILE), StatsFile::into_stats)
    }

    #[must_use]
    pub fn load_boards(&self) -> Loaded<Vec<BoardRecord>> {
        load(&self.directory.join(BOARDS_FILE), |file: BoardsFile| {
            Ok(file.boards)
        })
    }

    /// # Errors
    /// A message if the file cannot be serialized or atomically replaced.
    pub fn save_stats(&self, stats: &Stats) -> Result<(), String> {
        self.save(STATS_FILE, &StatsFile::from_stats(stats))
    }

    /// # Errors
    /// A message if the file cannot be serialized or atomically replaced.
    pub fn save_boards(&self, boards: &[BoardRecord]) -> Result<(), String> {
        let file = BoardsFile {
            boards: boards.to_vec(),
        };
        self.save(BOARDS_FILE, &file)
    }

    /// Writes a temporary file next to the target, then renames it over the target.
    fn save<T: Serialize>(&self, name: &str, value: &T) -> Result<(), String> {
        let path = self.directory.join(name);
        let text = toml::to_string(value)
            .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
        let temporary = self.directory.join(format!("{name}{TEMPORARY_SUFFIX}"));
        fs::create_dir_all(&self.directory)
            .and_then(|()| fs::write(&temporary, text))
            .and_then(|()| fs::rename(&temporary, &path))
            .map_err(|error| {
                let _ = fs::remove_file(&temporary);
                format!("cannot write {}: {error}", path.display())
            })
    }
}

/// A missing file is an empty state. An invalid one is moved to `<name>.broken` and treated as
/// empty; any other read failure leaves the file untouched and is treated as empty.
fn load<F, T>(path: &Path, convert: impl FnOnce(F) -> Result<T, String>) -> Loaded<T>
where
    F: DeserializeOwned,
    T: Default,
{
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Loaded {
                value: T::default(),
                error: None,
            };
        }
        Err(error) if error.kind() == ErrorKind::InvalidData => {
            return Loaded {
                value: T::default(),
                error: Some(move_aside(path, "is not valid UTF-8")),
            };
        }
        Err(error) => {
            return Loaded {
                value: T::default(),
                error: Some(format!("cannot read {}: {error}", path.display())),
            };
        }
    };
    let parsed = toml::from_str(&text)
        .map_err(|error| error.to_string())
        .and_then(convert);
    match parsed {
        Ok(value) => Loaded { value, error: None },
        Err(reason) => Loaded {
            value: T::default(),
            error: Some(move_aside(path, &format!("is invalid ({reason})"))),
        },
    }
}

/// Renames `path` to `<name>.broken` and describes what happened.
fn move_aside(path: &Path, problem: &str) -> String {
    let mut broken = path.as_os_str().to_owned();
    broken.push(BROKEN_SUFFIX);
    let broken = PathBuf::from(broken);
    match fs::rename(path, &broken) {
        Ok(()) => format!(
            "{} {problem}; moved to {}",
            path.display(),
            broken.display()
        ),
        Err(error) => format!(
            "{} {problem}; could not move it to {}: {error}",
            path.display(),
            broken.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unique directory per test, removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("stats-visualizer-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn uuid(n: u8) -> Uuid {
        Uuid { bytes: [n; 16] }
    }

    fn sample_stats() -> Stats {
        let alex = PlayerRecord {
            name: "Alex".to_owned(),
            games: BTreeMap::from([
                (
                    "skywars".to_owned(),
                    GameRecord {
                        wins: 12,
                        losses: 4,
                    },
                ),
                ("spleef".to_owned(), GameRecord { wins: 0, losses: 9 }),
            ]),
        };
        let steve = PlayerRecord {
            name: "Steve".to_owned(),
            games: BTreeMap::new(),
        };
        Stats::from_players(HashMap::from([(uuid(2), alex), (uuid(1), steve)]))
    }

    fn board(id: &str) -> BoardRecord {
        BoardRecord {
            id: id.to_owned(),
            world: "lobby:main".to_owned(),
            x: 12.5,
            y: 71.0,
            z: -3.5,
            game_type: "skywars".to_owned(),
        }
    }

    #[test]
    fn stats_round_trip() {
        let dir = TempDir::new("stats-round-trip");
        let storage = Storage::new(dir.0.clone());
        let stats = sample_stats();
        storage.save_stats(&stats).unwrap();

        let loaded = storage.load_stats();
        assert_eq!(loaded.error, None);
        assert_eq!(loaded.value, stats);
        assert!(!loaded.value.is_dirty());
    }

    #[test]
    fn boards_round_trip() {
        let dir = TempDir::new("boards-round-trip");
        let storage = Storage::new(dir.0.clone());
        let boards = vec![board("lobby_skywars"), board("lobby_spleef")];
        storage.save_boards(&boards).unwrap();

        let loaded = storage.load_boards();
        assert_eq!(loaded.error, None);
        assert_eq!(loaded.value, boards);
    }

    #[test]
    fn missing_files_are_empty_without_error() {
        let dir = TempDir::new("missing");
        let storage = Storage::new(dir.0.join("not-created"));
        let stats = storage.load_stats();
        let boards = storage.load_boards();
        assert_eq!(stats.error, None);
        assert!(stats.value.players().is_empty());
        assert_eq!(boards.error, None);
        assert_eq!(boards.value, []);
    }

    #[test]
    fn invalid_files_are_moved_aside_with_bytes_preserved() {
        let cases: [(&str, &[u8]); 3] = [
            ("not toml", b"[[players\nuuid = "),
            (
                "bad uuid",
                b"[[players]]\nuuid = \"nope\"\nname = \"Alex\"\n",
            ),
            ("non utf8", &[0xff, 0xfe, 0x00, 0x80]),
        ];
        for (label, bytes) in cases {
            let dir = TempDir::new(&format!("invalid-{}", label.replace(' ', "-")));
            let path = dir.0.join(STATS_FILE);
            fs::write(&path, bytes).unwrap();

            let loaded = Storage::new(dir.0.clone()).load_stats();

            assert!(loaded.value.players().is_empty(), "{label}");
            assert!(loaded.error.is_some(), "{label}");
            assert!(!path.exists(), "{label}");
            assert_eq!(
                fs::read(dir.0.join("stats.toml.broken")).unwrap(),
                bytes,
                "{label}"
            );
        }
    }

    #[test]
    fn failed_rename_returns_error_and_keeps_target() {
        let dir = TempDir::new("failed-rename");
        let target = dir.0.join(STATS_FILE);
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep"), b"data").unwrap();

        let result = Storage::new(dir.0.clone()).save_stats(&sample_stats());

        assert!(result.is_err());
        assert_eq!(fs::read(target.join("keep")).unwrap(), b"data");
        assert!(!dir.0.join("stats.toml.tmp").exists());
    }

    #[test]
    fn zero_count_games_are_not_written_but_player_is() {
        let dir = TempDir::new("zero-games");
        let storage = Storage::new(dir.0.clone());
        let zeroed = PlayerRecord {
            name: "Steve".to_owned(),
            games: BTreeMap::from([("empty_game".to_owned(), GameRecord::default())]),
        };
        let stats = Stats::from_players(HashMap::from([(uuid(1), zeroed)]));
        storage.save_stats(&stats).unwrap();

        let text = fs::read_to_string(dir.0.join(STATS_FILE)).unwrap();
        assert!(!text.contains("empty_game"));
        assert!(text.contains("Steve"));
        let loaded = storage.load_stats().value;
        assert!(loaded.players()[&uuid(1)].games.is_empty());
    }

    #[test]
    fn players_are_written_sorted_by_uuid() {
        let dir = TempDir::new("sorted");
        let storage = Storage::new(dir.0.clone());
        storage.save_stats(&sample_stats()).unwrap();

        let text = fs::read_to_string(dir.0.join(STATS_FILE)).unwrap();
        let first = text.find(&uuid(1).to_string()).unwrap();
        let second = text.find(&uuid(2).to_string()).unwrap();
        assert!(first < second);
    }
}
