//! Extension configuration: the list of world templates and their validation.

use std::collections::HashSet;
use std::path::{Component, Path};

use serde::Deserialize;

/// Game mode for players visiting a cloned world for the first time.
///
/// Local mirror of the host `GameMode`, which has no serde support; the caller maps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "survival" => Some(Self::Survival),
            "creative" => Some(Self::Creative),
            "adventure" => Some(Self::Adventure),
            "spectator" => Some(Self::Spectator),
            _ => None,
        }
    }
}

/// One validated template entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateConfig {
    /// Unique name; ASCII letters, digits, `_` and `-`.
    pub name: String,
    /// Saved world folder, relative to the extension data directory, never leaving it.
    pub path: String,
    /// Game mode for first-time visitors.
    pub default_game_mode: GameMode,
}

/// Validated extension configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    pub templates: Vec<TemplateConfig>,
}

#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RawConfig {
    templates: Vec<RawTemplate>,
}

// Missing fields default to empty so validation can report them against the entry.
#[derive(Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RawTemplate {
    name: String,
    path: String,
    default_game_mode: Option<String>,
}

impl Config {
    /// Parses and validates the configuration TOML. An empty document is valid.
    ///
    /// # Errors
    /// Returns a message naming the offending entry (by index and name) for duplicate or
    /// malformed names, absolute or escaping paths and unknown game modes.
    pub fn parse(toml: &str) -> Result<Self, String> {
        let raw: RawConfig =
            toml::from_str(toml).map_err(|e| format!("invalid world-cloner config: {e}"))?;
        let mut seen = HashSet::new();
        let mut templates = Vec::with_capacity(raw.templates.len());
        for (index, entry) in raw.templates.into_iter().enumerate() {
            let label = format!("templates[{index}] (name \"{}\")", entry.name);
            validate_name(&entry.name).map_err(|e| format!("{label}: {e}"))?;
            if !seen.insert(entry.name.clone()) {
                return Err(format!("{label}: duplicate template name"));
            }
            validate_path(&entry.path).map_err(|e| format!("{label}: {e}"))?;
            let default_game_mode = match entry.default_game_mode.as_deref() {
                None => GameMode::default(),
                Some(mode) => GameMode::from_name(mode).ok_or_else(|| {
                    format!(
                        "{label}: unknown default_game_mode \"{mode}\" \
                         (expected survival, creative, adventure or spectator)"
                    )
                })?,
            };
            templates.push(TemplateConfig {
                name: entry.name,
                path: entry.path,
                default_game_mode,
            });
        }
        Ok(Self { templates })
    }
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name is empty".to_owned());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("name may only contain ASCII letters, digits, '_' and '-'".to_owned());
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("path is empty".to_owned());
    }
    let path = Path::new(path);
    if path.is_absolute() || path.has_root() {
        return Err("path must be relative to the extension data directory".to_owned());
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("path must not contain '..'".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, path: &str) -> String {
        format!("[[templates]]\nname = \"{name}\"\npath = \"{path}\"\n")
    }

    fn assert_names_entry(toml: &str, index: usize, name: &str) {
        let err = Config::parse(toml).expect_err("config should be rejected");
        assert!(
            err.contains(&format!("templates[{index}] (name \"{name}\")")),
            "message does not name the entry: {err}"
        );
    }

    #[test]
    fn empty_config_is_valid() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn duplicate_names_name_the_second_entry() {
        let toml = format!("{}{}", entry("a", "x"), entry("a", "y"));
        assert_names_entry(&toml, 1, "a");
    }

    #[test]
    fn bad_name_characters_and_empty_name() {
        assert_names_entry(&entry("bad name", "x"), 0, "bad name");
        assert_names_entry(&entry("ä", "x"), 0, "ä");
        assert_names_entry(&entry("", "x"), 0, "");
    }

    #[test]
    fn absolute_and_empty_paths() {
        assert_names_entry(&entry("a", "/etc/world"), 0, "a");
        assert_names_entry(&entry("a", ""), 0, "a");
    }

    #[test]
    fn parent_dir_paths_leading_and_embedded() {
        assert_names_entry(&entry("a", "../world"), 0, "a");
        assert_names_entry(
            &format!("{}{}", entry("ok", "x"), entry("b", "a/../b")),
            1,
            "b",
        );
    }

    #[test]
    fn dotted_names_are_not_parent_dir() {
        assert!(Config::parse(&entry("a", "maps/..hidden/w..d")).is_ok());
    }

    #[test]
    fn unknown_game_mode_names_the_entry() {
        let toml = format!("{}default_game_mode = \"hardcore\"\n", entry("a", "x"));
        assert_names_entry(&toml, 0, "a");
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(Config::parse("bogus = 1").is_err());
        assert!(Config::parse(&format!("{}bogus = 1\n", entry("a", "x"))).is_err());
    }

    #[test]
    fn game_mode_defaults_to_survival() {
        let config = Config::parse(&entry("a", "x")).unwrap();
        assert_eq!(config.templates[0].default_game_mode, GameMode::Survival);
    }

    #[test]
    fn valid_config_round_trip() {
        let toml = "[[templates]]\nname = \"skywars_a\"\npath = \"templates/skywars_a\"\n\
                    default_game_mode = \"adventure\"\n\n\
                    [[templates]]\nname = \"b-2\"\npath = \"b\"\n";
        let config = Config::parse(toml).unwrap();
        assert_eq!(
            config.templates,
            vec![
                TemplateConfig {
                    name: "skywars_a".to_owned(),
                    path: "templates/skywars_a".to_owned(),
                    default_game_mode: GameMode::Adventure,
                },
                TemplateConfig {
                    name: "b-2".to_owned(),
                    path: "b".to_owned(),
                    default_game_mode: GameMode::Survival,
                },
            ]
        );
    }
}
