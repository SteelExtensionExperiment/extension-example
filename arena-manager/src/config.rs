//! `config.toml` settings.
use serde::Deserialize;

const DEFAULT_MAX_WORLDS: usize = 4;
const DEFAULT_WINNER_RETURN_DELAY_SECONDS: u64 = 10;
const DEFAULT_PLAYER_READY_TIMEOUT_SECONDS: u64 = 15;

/// Arena manager settings. Every key is optional.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Most arena worlds existing or being created at once. At least 1.
    pub max_worlds: usize,
    /// Countdown before the last player standing returns to the lobby. Above 0.
    pub winner_return_delay_seconds: u64,
    /// How long to wait for a player to become ready after a domain switch. Above 0.
    pub player_ready_timeout_seconds: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_worlds: DEFAULT_MAX_WORLDS,
            winner_return_delay_seconds: DEFAULT_WINNER_RETURN_DELAY_SECONDS,
            player_ready_timeout_seconds: DEFAULT_PLAYER_READY_TIMEOUT_SECONDS,
        }
    }
}

impl Config {
    /// Parses and validates the file contents.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        config.validate()?;
        Ok(config)
    }

    /// Checks values that serde cannot.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_worlds < 1 {
            return Err("max_worlds must be at least 1".into());
        }
        if self.winner_return_delay_seconds == 0 {
            return Err("winner_return_delay_seconds must be greater than 0".into());
        }
        if self.player_ready_timeout_seconds == 0 {
            return Err("player_ready_timeout_seconds must be greater than 0".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults() {
        assert_eq!(Config::from_toml(""), Ok(Config::default()));
    }

    #[test]
    fn invalid_values_name_their_key() {
        for key in [
            "max_worlds",
            "winner_return_delay_seconds",
            "player_ready_timeout_seconds",
        ] {
            let error = Config::from_toml(&format!("{key} = 0")).unwrap_err();
            assert!(error.contains(key), "{error}");
        }
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let error = Config::from_toml("max_world = 2").unwrap_err();
        assert!(error.contains("max_world"), "{error}");
    }
}
