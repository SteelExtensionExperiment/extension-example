//! Enchant definitions, costs, proc chances, pickaxe data and sell values, independent of the
//! server so the math is unit tested.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Data key marking the prison pickaxe.
pub const PICKAXE_KEY: &str = "prison-enchants:pickaxe";

/// Every custom enchant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Enchant {
    /// Breaks the whole mine layer of the broken block.
    Jackhammer,
    /// Blows up a sphere around the broken block.
    Explosive,
    /// Clears the entire mine.
    Nuke,
    /// Sends an expanding shell through the mine, tick by tick, with note particles.
    Soundwave,
    /// Haste effect while holding the pickaxe.
    Haste,
    /// Speed effect while holding the pickaxe.
    Speed,
    /// Multiplies sell value.
    Fortune,
}

impl Enchant {
    /// Every enchant, in menu order.
    pub const ALL: [Self; 7] = [
        Self::Jackhammer,
        Self::Explosive,
        Self::Nuke,
        Self::Soundwave,
        Self::Fortune,
        Self::Haste,
        Self::Speed,
    ];

    /// Config and command id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Jackhammer => "jackhammer",
            Self::Explosive => "explosive",
            Self::Nuke => "nuke",
            Self::Soundwave => "soundwave",
            Self::Haste => "haste",
            Self::Speed => "speed",
            Self::Fortune => "fortune",
        }
    }

    /// Parses a config or command id.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|enchant| enchant.id().eq_ignore_ascii_case(id))
    }

    /// Display name with color codes.
    #[must_use]
    pub const fn display(self) -> &'static str {
        match self {
            Self::Jackhammer => "&6&lJackhammer",
            Self::Explosive => "&c&lExplosive",
            Self::Nuke => "&4&lNuke",
            Self::Soundwave => "&d&lSoundwave",
            Self::Haste => "&e&lHaste",
            Self::Speed => "&b&lSpeed",
            Self::Fortune => "&a&lFortune",
        }
    }

    /// Menu icon item.
    #[must_use]
    pub const fn icon(self) -> &'static str {
        match self {
            Self::Jackhammer => "minecraft:anvil",
            Self::Explosive => "minecraft:tnt",
            Self::Nuke => "minecraft:end_crystal",
            Self::Soundwave => "minecraft:note_block",
            Self::Haste => "minecraft:golden_pickaxe",
            Self::Speed => "minecraft:sugar",
            Self::Fortune => "minecraft:diamond",
        }
    }

    /// One-line description for the menu.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Jackhammer => "Breaks the whole mine layer you dig in",
            Self::Explosive => "Blows up the blocks around you",
            Self::Nuke => "Wipes out the entire mine",
            Self::Soundwave => "A wave of sound that shatters everything it reaches",
            Self::Haste => "Permanent haste while held",
            Self::Speed => "Permanent speed while held",
            Self::Fortune => "Multiplies what every block sells for",
        }
    }

    /// Whether the enchant procs randomly on a block break.
    #[must_use]
    pub const fn procs(self) -> bool {
        matches!(
            self,
            Self::Jackhammer | Self::Explosive | Self::Nuke | Self::Soundwave
        )
    }

    /// Item data key of this enchant's level.
    #[must_use]
    pub fn data_key(self) -> String {
        format!("prison-enchants:{}", self.id())
    }

    /// Default tuning.
    #[must_use]
    pub const fn defaults(self) -> EnchantConfig {
        let (max, base_cost, growth, base_chance, chance_per_level) = match self {
            // 1% at level 1, every proc at the max level.
            Self::Jackhammer => (100, 2_000.0, 1.06, 0.01, 0.01),
            Self::Explosive => (50, 1_000.0, 1.08, 0.02, 0.003),
            Self::Nuke => (5, 250_000.0, 3.0, 0.0002, 0.0002),
            Self::Soundwave => (50, 3_000.0, 1.09, 0.005, 0.0015),
            Self::Haste | Self::Speed => (3, 5_000.0, 4.0, 0.0, 0.0),
            Self::Fortune => (10, 1_500.0, 1.6, 0.0, 0.0),
        };
        EnchantConfig {
            max,
            base_cost,
            growth,
            base_chance,
            chance_per_level,
        }
    }
}

/// Tuning of one enchant (`[enchants.<id>]` in the config).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EnchantConfig {
    /// Highest level.
    pub max: u32,
    /// Price of level 1.
    pub base_cost: f64,
    /// Each level costs this many times the previous one.
    pub growth: f64,
    /// Proc chance per block at level 1, 0–1.
    pub base_chance: f64,
    /// Added chance per level above 1.
    pub chance_per_level: f64,
}

impl EnchantConfig {
    /// Price of reaching `level` from `level - 1`.
    #[must_use]
    pub fn cost_of(&self, level: u32) -> f64 {
        let steps = i32::try_from(level.saturating_sub(1)).unwrap_or(i32::MAX);
        (self.base_cost * self.growth.powi(steps)).round()
    }

    /// Total price of going from `from` to `to`.
    #[must_use]
    pub fn cost_between(&self, from: u32, to: u32) -> f64 {
        (from + 1..=to.min(self.max))
            .map(|level| self.cost_of(level))
            .sum()
    }

    /// Proc chance per block at `level`, capped at 1.
    #[must_use]
    pub fn chance(&self, level: u32) -> f64 {
        if level == 0 {
            return 0.0;
        }
        (self.base_chance + self.chance_per_level * f64::from(level - 1)).clamp(0.0, 1.0)
    }
}

/// Plugin config (`extensions/prison-enchants/config.toml`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Per-enchant tuning overrides.
    pub enchants: BTreeMap<String, EnchantConfig>,
    /// Sell value per block key.
    pub values: BTreeMap<String, f64>,
    /// Value of blocks missing from `values`.
    pub default_value: f64,
    /// Sell value multiplier per Fortune level.
    pub fortune_bonus: f64,
}

impl Default for Config {
    fn default() -> Self {
        let values = [
            ("stone", 1.0),
            ("cobblestone", 1.0),
            ("andesite", 1.0),
            ("diorite", 1.0),
            ("granite", 1.0),
            ("coal_ore", 4.0),
            ("copper_ore", 6.0),
            ("iron_ore", 10.0),
            ("lapis_ore", 14.0),
            ("redstone_ore", 14.0),
            ("gold_ore", 25.0),
            ("diamond_ore", 60.0),
            ("emerald_ore", 120.0),
            ("coal_block", 36.0),
            ("iron_block", 90.0),
            ("gold_block", 225.0),
            ("diamond_block", 540.0),
            ("emerald_block", 1_080.0),
        ]
        .into_iter()
        .map(|(key, value)| (format!("minecraft:{key}"), value))
        .collect();
        Self {
            enchants: BTreeMap::new(),
            values,
            default_value: 0.5,
            fortune_bonus: 0.5,
        }
    }
}

impl Config {
    /// Tuning of an enchant, with config overrides applied.
    #[must_use]
    pub fn enchant(&self, enchant: Enchant) -> EnchantConfig {
        self.enchants
            .get(enchant.id())
            .copied()
            .unwrap_or_else(|| enchant.defaults())
    }

    /// Value of one block before Fortune.
    #[must_use]
    pub fn value_of(&self, block: &str) -> f64 {
        self.values
            .get(block)
            .copied()
            .unwrap_or(self.default_value)
    }

    /// Sell price of `counts` (block key, count) with the given Fortune level.
    #[must_use]
    pub fn sell_total<'a>(
        &self,
        counts: impl IntoIterator<Item = (&'a str, u64)>,
        fortune: u32,
    ) -> f64 {
        let multiplier = 1.0 + self.fortune_bonus * f64::from(fortune);
        counts
            .into_iter()
            .map(|(block, count)| {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "money totals are rounded to cents"
                )]
                let count = count as f64;
                self.value_of(block) * count
            })
            .sum::<f64>()
            * multiplier
    }
}

/// Enchant levels of a pickaxe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Levels(BTreeMap<Enchant, u32>);

impl Levels {
    /// Reads levels from item data entries.
    #[must_use]
    pub fn from_data<'a>(data: impl Fn(&str) -> Option<&'a str>) -> Self {
        Self(
            Enchant::ALL
                .into_iter()
                .filter_map(|enchant| {
                    let level = data(&enchant.data_key())?.parse().ok()?;
                    (level > 0).then_some((enchant, level))
                })
                .collect(),
        )
    }

    /// Level of one enchant.
    #[must_use]
    pub fn get(&self, enchant: Enchant) -> u32 {
        self.0.get(&enchant).copied().unwrap_or(0)
    }

    /// Sets one level; 0 removes it.
    pub fn set(&mut self, enchant: Enchant, level: u32) {
        if level == 0 {
            self.0.remove(&enchant);
        } else {
            self.0.insert(enchant, level);
        }
    }

    /// Pickaxe lore lines with color codes.
    #[must_use]
    pub fn lore(&self) -> Vec<String> {
        let mut lines = vec!["&8&m                        ".to_owned()];
        if self.0.is_empty() {
            lines.push("&7No enchants yet".to_owned());
        }
        for enchant in Enchant::ALL {
            let level = self.get(enchant);
            if level > 0 {
                lines.push(format!(
                    "{} &f{}",
                    enchant.display(),
                    roman_or_number(level)
                ));
            }
        }
        lines.push("&8&m                        ".to_owned());
        lines.push("&7Right-click or &e/enchants&7 to upgrade".to_owned());
        lines
    }
}

/// Roman numerals up to 10, digits above.
#[must_use]
pub fn roman_or_number(level: u32) -> String {
    const ROMAN: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    usize::try_from(level)
        .ok()
        .and_then(|level| level.checked_sub(1))
        .and_then(|index| ROMAN.get(index))
        .map_or_else(|| level.to_string(), |roman| (*roman).to_owned())
}

/// Radius of the Explosive sphere at `level`.
#[must_use]
pub fn explosive_radius(level: u32) -> f64 {
    2.0 + f64::from(level.min(50)) / 12.5
}

/// Final radius of the Soundwave at `level`.
#[must_use]
pub fn soundwave_radius(level: u32) -> u32 {
    4 + level.min(50) / 5
}

/// Formats money with thousands separators and two decimals.
#[must_use]
pub fn money(amount: f64) -> String {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "amounts are shown, not computed with; negative values never reach here"
    )]
    let cents = (amount.max(0.0) * 100.0).round() as u64;
    let whole = (cents / 100).to_string();
    let mut grouped = String::new();
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("${grouped}.{:02}", cents % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn costs_grow_geometrically_and_sum_over_ranges() {
        let config = Enchant::Jackhammer.defaults();
        assert!((config.cost_of(1) - 2_000.0).abs() < f64::EPSILON);
        assert!((config.cost_of(2) - 2_120.0).abs() < f64::EPSILON);
        assert!(
            (config.cost_between(0, 2) - 4_120.0).abs() < f64::EPSILON,
            "{}",
            config.cost_between(0, 2)
        );
        assert!(config.cost_between(99, 500) > 0.0, "clamped to max");
        assert!((config.cost_between(100, 101)).abs() < f64::EPSILON);
    }

    #[test]
    fn chances_start_at_base_and_cap_at_one() {
        let config = Enchant::Explosive.defaults();
        assert!(config.chance(0).abs() < f64::EPSILON);
        assert!((config.chance(1) - 0.02).abs() < 1e-12);
        assert!((config.chance(11) - 0.05).abs() < 1e-12);
        let always = EnchantConfig {
            base_chance: 0.9,
            chance_per_level: 0.5,
            ..config
        };
        assert!((always.chance(10) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn jackhammer_always_procs_at_max_level() {
        let config = Enchant::Jackhammer.defaults();
        assert!((config.chance(config.max) - 1.0).abs() < 1e-9);
        assert!((config.chance(1) - 0.01).abs() < 1e-12);
    }

    #[test]
    fn levels_round_trip_through_item_data() {
        let mut levels = Levels::default();
        levels.set(Enchant::Jackhammer, 12);
        levels.set(Enchant::Fortune, 3);
        let data: BTreeMap<String, String> = Enchant::ALL
            .into_iter()
            .map(|enchant| (enchant.data_key(), levels.get(enchant).to_string()))
            .collect();
        let back = Levels::from_data(|key| data.get(key).map(String::as_str));
        assert_eq!(back, levels);
        let lore = back.lore();
        assert!(lore.iter().any(|line| line.contains("Jackhammer &f12")));
        assert!(lore.iter().any(|line| line.contains("Fortune &fIII")));
        assert!(!lore.iter().any(|line| line.contains("Nuke")));
        levels.set(Enchant::Fortune, 0);
        assert_eq!(levels.get(Enchant::Fortune), 0);
    }

    #[test]
    fn sell_totals_apply_values_defaults_and_fortune() {
        let config = Config::default();
        let counts = [
            ("minecraft:stone", 10),
            ("minecraft:diamond_ore", 2),
            ("minecraft:dirt", 4),
        ];
        let base = 10.0 + 120.0 + 2.0;
        assert!((config.sell_total(counts, 0) - base).abs() < 1e-9);
        assert!((config.sell_total(counts, 2) - base * 2.0).abs() < 1e-9);
    }

    #[test]
    fn helpers_format_and_scale() {
        assert_eq!(money(1_234_567.891), "$1,234,567.89");
        assert_eq!(money(5.0), "$5.00");
        assert_eq!(roman_or_number(4), "IV");
        assert_eq!(roman_or_number(42), "42");
        assert_eq!(Enchant::from_id("SoundWave"), Some(Enchant::Soundwave));
        assert!(explosive_radius(50) <= 6.0);
        assert_eq!(soundwave_radius(50), 14);
    }
}
