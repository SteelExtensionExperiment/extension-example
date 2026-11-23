//! Extension configuration from `config.toml`.

use serde::Deserialize;

/// Default autosave period: 1200 ticks, 60 seconds at 20 ticks per second.
const DEFAULT_AUTOSAVE_INTERVAL_TICKS: u64 = 1200;

/// Stats visualizer settings. Every field has a default.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Server ticks between saves of changed data; must be at least 1.
    pub autosave_interval_ticks: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            autosave_interval_ticks: DEFAULT_AUTOSAVE_INTERVAL_TICKS,
        }
    }
}

impl Config {
    /// Parses `text` and validates the result.
    ///
    /// # Errors
    /// A message if the TOML is invalid, has unknown keys, or fails [`Config::validate`].
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        config.validate()?;
        Ok(config)
    }

    /// # Errors
    /// A message if `autosave_interval_ticks` is 0.
    pub fn validate(&self) -> Result<(), String> {
        if self.autosave_interval_ticks == 0 {
            return Err("autosave_interval_ticks must be at least 1".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_uses_defaults() {
        let config = Config::parse("").unwrap();
        assert_eq!(config.autosave_interval_ticks, 1200);
    }

    #[test]
    fn reads_interval() {
        let config = Config::parse("autosave_interval_ticks = 200").unwrap();
        assert_eq!(config.autosave_interval_ticks, 200);
    }

    #[test]
    fn rejects_zero_interval() {
        let error = Config::parse("autosave_interval_ticks = 0").unwrap_err();
        assert!(error.contains("autosave_interval_ticks"), "{error}");
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(Config::parse("autosave = 5").is_err());
    }
}
