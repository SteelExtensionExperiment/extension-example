//! `extensions/toxic-wasteland/config.toml`. Every value has a default and is validated
//! at startup; an invalid value stops the extension with a message naming the field.

use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub colors: Colors,
    pub sludge: Sludge,
    pub effects: Effects,
    pub spread: Spread,
    pub world: WorldGen,
    pub commands: Commands,
    pub mobs: Mobs,
}

/// Wasteland mob spawning around players.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Mobs {
    pub enabled: bool,
    /// Ticks between spawn attempts.
    pub interval_ticks: u64,
    /// Ghouls kept within 64 blocks of each player.
    pub max_ghouls: u32,
    /// Sludge slimes kept within 64 blocks of each player.
    pub max_slimes: u32,
}

impl Default for Mobs {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ticks: 200,
            max_ghouls: 3,
            max_slimes: 4,
        }
    }
}

/// Biome and sludge colors as `#RRGGBB`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Colors {
    pub water: String,
    pub water_fog: String,
    pub sky: String,
    pub fog: String,
    pub grass: String,
    pub foliage: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Sludge {
    /// Ticks between spread steps (water: 5).
    pub tick_delay: u16,
    /// Level lost per block (water: 1); 2 gives half of water's reach.
    pub drop_off: u8,
    pub converts_to_source: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Effects {
    pub poison_seconds: i32,
    pub poison_amplifier: i32,
    pub contact_damage: f32,
    pub damage_interval_ticks: i64,
}

/// Chances per random tick and neighbor.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Spread {
    pub corrupt_water_chance: f64,
    pub wither_ground_chance: f64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct WorldGen {
    /// Chance that a chunk holds a sludge lake.
    pub lake_chance: f64,
    /// Chance per tree slot (two per chunk) of a dead tree.
    pub tree_chance: f64,
    /// Chance that a chunk shows a uranium ore patch.
    pub uranium_chance: f64,
    pub base_height: i32,
    pub relief: i32,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Commands {
    /// Largest box `/toxic` edits, in blocks.
    pub max_volume: u64,
}

impl Default for Colors {
    fn default() -> Self {
        Self {
            water: "#4CBB17".into(),
            water_fog: "#2E5E0E".into(),
            sky: "#9AA35A".into(),
            fog: "#B4BA7A".into(),
            grass: "#7C8A3A".into(),
            foliage: "#6B7A2E".into(),
        }
    }
}

impl Default for Sludge {
    fn default() -> Self {
        Self {
            tick_delay: 15,
            drop_off: 2,
            converts_to_source: false,
        }
    }
}

impl Default for Effects {
    fn default() -> Self {
        Self {
            poison_seconds: 5,
            poison_amplifier: 0,
            contact_damage: 1.0,
            damage_interval_ticks: 20,
        }
    }
}

impl Default for Spread {
    fn default() -> Self {
        Self {
            corrupt_water_chance: 0.25,
            wither_ground_chance: 0.15,
        }
    }
}

impl Default for WorldGen {
    fn default() -> Self {
        Self {
            lake_chance: 0.3,
            tree_chance: 0.35,
            uranium_chance: 0.25,
            base_height: 70,
            relief: 18,
        }
    }
}

impl Default for Commands {
    fn default() -> Self {
        Self {
            max_volume: 2_000_000,
        }
    }
}

/// Colors resolved to 24-bit RGB values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub water: i32,
    pub water_fog: i32,
    pub sky: i32,
    pub fog: i32,
    pub grass: i32,
    pub foliage: i32,
}

fn color(field: &str, text: &str) -> Result<i32, String> {
    let hex = text
        .strip_prefix('#')
        .filter(|hex| hex.len() == 6)
        .ok_or_else(|| format!("colors.{field}: {text:?} is not a #RRGGBB color"))?;
    i32::from_str_radix(hex, 16)
        .map_err(|_| format!("colors.{field}: {text:?} is not a #RRGGBB color"))
}

fn chance(field: &str, value: f64) -> Result<(), String> {
    if (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(format!("{field}: {value} must be between 0 and 1"))
    }
}

impl Config {
    /// Parses and validates the extension's TOML; an empty file gives the defaults.
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|error| format!("config.toml: {error}"))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        self.palette()?;
        if !(1..=1200).contains(&self.sludge.tick_delay) {
            return Err("sludge.tick_delay: must be between 1 and 1200".into());
        }
        if !(1..=7).contains(&self.sludge.drop_off) {
            return Err("sludge.drop_off: must be between 1 and 7".into());
        }
        let effects = &self.effects;
        if !(0..=3600).contains(&effects.poison_seconds) {
            return Err("effects.poison_seconds: must be between 0 and 3600".into());
        }
        if !(0..=255).contains(&effects.poison_amplifier) {
            return Err("effects.poison_amplifier: must be between 0 and 255".into());
        }
        if !effects.contact_damage.is_finite() || effects.contact_damage < 0.0 {
            return Err("effects.contact_damage: must be a non-negative number".into());
        }
        if !(1..=1200).contains(&effects.damage_interval_ticks) {
            return Err("effects.damage_interval_ticks: must be between 1 and 1200".into());
        }
        chance(
            "spread.corrupt_water_chance",
            self.spread.corrupt_water_chance,
        )?;
        chance(
            "spread.wither_ground_chance",
            self.spread.wither_ground_chance,
        )?;
        chance("world.lake_chance", self.world.lake_chance)?;
        chance("world.tree_chance", self.world.tree_chance)?;
        chance("world.uranium_chance", self.world.uranium_chance)?;
        if !(-32..=300).contains(&self.world.base_height) {
            return Err("world.base_height: must be between -32 and 300".into());
        }
        if !(0..=64).contains(&self.world.relief) {
            return Err("world.relief: must be between 0 and 64".into());
        }
        if !(20..=72_000).contains(&self.mobs.interval_ticks) {
            return Err("mobs.interval_ticks: must be between 20 and 72000".into());
        }
        if self.commands.max_volume == 0 {
            return Err("commands.max_volume: must be at least 1".into());
        }
        Ok(())
    }

    pub fn palette(&self) -> Result<Palette, String> {
        let colors = &self.colors;
        Ok(Palette {
            water: color("water", &colors.water)?,
            water_fog: color("water_fog", &colors.water_fog)?,
            sky: color("sky", &colors.sky)?,
            fog: color("fog", &colors.fog)?,
            grass: color("grass", &colors.grass)?,
            foliage: color("foliage", &colors.foliage)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_gives_defaults() -> Result<(), String> {
        assert_eq!(Config::parse("")?, Config::default());
        Ok(())
    }

    #[test]
    fn colors_parse_as_rgb() -> Result<(), String> {
        let palette = Config::default().palette()?;
        assert_eq!(palette.water, 0x4C_BB17);
        Ok(())
    }

    #[test]
    fn invalid_values_name_their_field() {
        for (text, field) in [
            ("[colors]\nwater = \"green\"", "colors.water"),
            ("[sludge]\ndrop_off = 9", "sludge.drop_off"),
            ("[effects]\ncontact_damage = -1.0", "effects.contact_damage"),
            (
                "[spread]\ncorrupt_water_chance = 2.0",
                "spread.corrupt_water_chance",
            ),
            ("[world]\nlake_chance = -0.5", "world.lake_chance"),
            ("[commands]\nmax_volume = 0", "commands.max_volume"),
        ] {
            let error = Config::parse(text).expect_err(text);
            assert!(error.starts_with(field), "{error}");
        }
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(Config::parse("[sludge]\nspeed = 3").is_err());
    }
}
