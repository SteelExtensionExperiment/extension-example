//! Plugin configuration from the server's `[plugins.config]` table.
use serde::Deserialize;
use std::collections::BTreeMap;

/// Essentials settings. Every field has an EssentialsX-like default.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Chat line format. Placeholders: `{prefix}`, `{suffix}`, `{displayname}`, `{name}`,
    /// `{group}`, `{message}`. Empty keeps vanilla chat.
    pub chat_format: String,
    /// Join broadcast with `{player}`; empty keeps the vanilla message.
    pub join_message: String,
    /// Leave broadcast with `{player}`; empty keeps the vanilla message.
    pub quit_message: String,
    /// Balance of new accounts.
    pub starting_balance: f64,
    /// Largest allowed balance.
    pub max_money: f64,
    /// Symbol placed before formatted amounts.
    pub currency_symbol: String,
    /// Currency name for one unit.
    pub currency_singular: String,
    /// Currency name for several units.
    pub currency_plural: String,
    /// Homes allowed without the `essentials:homes` permission metadata.
    pub default_home_limit: i64,
    /// Seconds before a teleport request expires.
    pub tpa_timeout_seconds: u64,
    /// Idle seconds before a player is marked AFK; 0 disables automatic AFK.
    pub afk_timeout_seconds: u64,
    /// Entries shown by `/baltop`.
    pub baltop_entries: usize,
    /// Kits by name.
    pub kits: BTreeMap<String, Kit>,
    /// Message overrides by key; see `messages.rs` for the keys and defaults.
    pub messages: BTreeMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut kits = BTreeMap::new();
        kits.insert(
            "starter".to_owned(),
            Kit {
                items: vec![
                    "minecraft:stone_sword 1".to_owned(),
                    "minecraft:bread 16".to_owned(),
                ],
                cooldown_seconds: 86_400,
                default_access: true,
            },
        );
        Self {
            chat_format: "{prefix}&f{displayname}{suffix}&7: &f{message}".to_owned(),
            join_message: "&e{player} joined the game".to_owned(),
            quit_message: "&e{player} left the game".to_owned(),
            starting_balance: 100.0,
            max_money: 10_000_000_000_000.0,
            currency_symbol: "$".to_owned(),
            currency_singular: "Dollar".to_owned(),
            currency_plural: "Dollars".to_owned(),
            default_home_limit: 3,
            tpa_timeout_seconds: 120,
            afk_timeout_seconds: 300,
            baltop_entries: 10,
            kits,
            messages: BTreeMap::new(),
        }
    }
}

/// A kit of items with a per-player cooldown.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kit {
    /// Entries `"namespace:item count"`; the count defaults to 1.
    pub items: Vec<String>,
    /// Seconds between uses; 0 means no cooldown.
    #[serde(default)]
    pub cooldown_seconds: u64,
    /// Whether players need no `essentials.kits.<name>` grant to use it.
    #[serde(default = "default_true")]
    pub default_access: bool,
}

const fn default_true() -> bool {
    true
}

impl Config {
    /// Checks values that serde cannot.
    pub fn validate(&self) -> Result<(), String> {
        if !self.starting_balance.is_finite() || self.starting_balance < 0.0 {
            return Err("starting_balance must be a non-negative number".into());
        }
        if !self.max_money.is_finite() || self.max_money < self.starting_balance {
            return Err("max_money must be at least starting_balance".into());
        }
        for (name, kit) in &self.kits {
            for entry in &kit.items {
                parse_kit_item(entry).map_err(|error| format!("kit {name}: {error}"))?;
            }
        }
        Ok(())
    }
}

/// Parses `"namespace:item count"` into the key and count.
pub fn parse_kit_item(entry: &str) -> Result<(String, i32), String> {
    let mut parts = entry.split_whitespace();
    let item = parts.next().ok_or("empty kit item")?;
    let count = match parts.next() {
        Some(count) => count
            .parse::<i32>()
            .ok()
            .filter(|count| *count > 0)
            .ok_or_else(|| format!("invalid count in {entry:?}"))?,
        None => 1,
    };
    if parts.next().is_some() {
        return Err(format!("unexpected text in {entry:?}"));
    }
    let item = if item.contains(':') {
        item.to_owned()
    } else {
        format!("minecraft:{item}")
    };
    Ok((item, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kit_items_default_namespace_and_count() {
        assert_eq!(parse_kit_item("bread"), Ok(("minecraft:bread".into(), 1)));
        assert_eq!(parse_kit_item("mymod:gem 5"), Ok(("mymod:gem".into(), 5)));
        assert!(parse_kit_item("bread 0").is_err());
        assert!(parse_kit_item("bread 2 extra").is_err());
    }

    #[test]
    fn default_config_is_valid_and_parses_overrides() {
        assert!(Config::default().validate().is_ok());
        let config: Config =
            toml::from_str("starting_balance = 5.0\n[messages]\nhome_set = \"ok\"")
                .unwrap_or_default();
        assert!((config.starting_balance - 5.0).abs() < f64::EPSILON);
        assert_eq!(
            config.messages.get("home_set").map(String::as_str),
            Some("ok")
        );
    }
}
