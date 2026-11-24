//! `config.toml`: global settings. The file is optional; missing means defaults.

use std::fmt;

use serde::Deserialize;

/// File name, used in error messages.
pub(crate) const FILE_NAME: &str = "config.toml";
pub(crate) const DEFAULT_RETRY_INTERVAL_TICKS: u32 = 40;
pub(crate) const DEFAULT_MAX_SESSION_HOURS: u32 = 24;

const KEY_RETRY_INTERVAL_TICKS: &str = "retry_interval_ticks";
const KEY_MAX_SESSION_HOURS: &str = "max_session_hours";
const MINIMUM: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Config {
    /// Ticks between retries of a request rejected for capacity.
    pub retry_interval_ticks: u32,
    /// Upper bound for `/orchestrator open <hours>`.
    pub max_session_hours: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ConfigError {
    Parse(String),
    Invalid { key: &'static str, value: i64 },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(cause) => write!(f, "{FILE_NAME}: {cause}"),
            Self::Invalid { key, value } => {
                write!(
                    f,
                    "{FILE_NAME}: {key} must be at least {MINIMUM} (got {value})"
                )
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    retry_interval_ticks: Option<i64>,
    max_session_hours: Option<i64>,
}

fn at_least_minimum(
    key: &'static str,
    value: Option<i64>,
    default: u32,
) -> Result<u32, ConfigError> {
    let Some(value) = value else {
        return Ok(default);
    };
    u32::try_from(value)
        .ok()
        .filter(|checked| *checked >= MINIMUM)
        .ok_or(ConfigError::Invalid { key, value })
}

impl Config {
    /// Parses the file text; `None` (file missing) gives the defaults.
    pub(crate) fn parse(text: Option<&str>) -> Result<Self, ConfigError> {
        let raw = match text {
            None => Raw {
                retry_interval_ticks: None,
                max_session_hours: None,
            },
            Some(text) => {
                toml::from_str(text).map_err(|error| ConfigError::Parse(error.to_string()))?
            }
        };
        Ok(Self {
            retry_interval_ticks: at_least_minimum(
                KEY_RETRY_INTERVAL_TICKS,
                raw.retry_interval_ticks,
                DEFAULT_RETRY_INTERVAL_TICKS,
            )?,
            max_session_hours: at_least_minimum(
                KEY_MAX_SESSION_HOURS,
                raw.max_session_hours,
                DEFAULT_MAX_SESSION_HOURS,
            )?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULTS: Config = Config {
        retry_interval_ticks: DEFAULT_RETRY_INTERVAL_TICKS,
        max_session_hours: DEFAULT_MAX_SESSION_HOURS,
    };

    #[test]
    fn missing_or_empty_file_gives_defaults() {
        assert_eq!(Config::parse(None), Ok(DEFAULTS));
        assert_eq!(Config::parse(Some("")), Ok(DEFAULTS));
    }

    #[test]
    fn values_are_read() {
        let config =
            Config::parse(Some("retry_interval_ticks = 5\nmax_session_hours = 2")).unwrap();
        assert_eq!(
            config,
            Config {
                retry_interval_ticks: 5,
                max_session_hours: 2
            }
        );
    }

    #[test]
    fn invalid_value_names_its_key() {
        for (text, key) in [
            ("retry_interval_ticks = 0", KEY_RETRY_INTERVAL_TICKS),
            ("max_session_hours = -3", KEY_MAX_SESSION_HOURS),
            ("max_session_hours = 99999999999", KEY_MAX_SESSION_HOURS),
        ] {
            let error = Config::parse(Some(text)).unwrap_err();
            assert!(error.to_string().contains(key), "{error}");
        }
    }

    #[test]
    fn unknown_key_and_bad_syntax_are_errors_naming_the_file() {
        for text in ["unknown = 1", "retry_interval_ticks ="] {
            let error = Config::parse(Some(text)).unwrap_err();
            assert!(error.to_string().starts_with(FILE_NAME), "{error}");
        }
    }
}
