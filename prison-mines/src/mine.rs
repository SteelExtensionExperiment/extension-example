//! Mine definitions, block mixes and reset rules, independent of the server.

use serde::{Deserialize, Serialize};
use steel_extension_api::BlockPos;

/// One block of a mix with its relative weight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MixEntry {
    /// Block state, such as `minecraft:diamond_ore`.
    pub block: String,
    /// Relative weight (the `60` of `60%stone`).
    pub weight: u32,
}

/// A stored mine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mine {
    /// World key.
    pub world: String,
    /// Minimum corner `[x, y, z]` (inclusive).
    pub min: [i32; 3],
    /// Maximum corner `[x, y, z]` (inclusive).
    pub max: [i32; 3],
    /// Block mix.
    pub mix: Vec<MixEntry>,
    /// Seconds between timed resets; 0 disables the timer.
    #[serde(default)]
    pub interval: u32,
    /// Percentage mined that triggers a reset; 0 disables it.
    #[serde(default)]
    pub threshold: u8,
}

/// The `mines.toml` file.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct MineFile {
    /// Mines by name.
    #[serde(default)]
    pub mines: std::collections::BTreeMap<String, Mine>,
}

impl Mine {
    /// A mine over the box between two corners, with a plain stone mix and a 5 minute timer.
    #[must_use]
    pub fn new(world: &str, a: BlockPos, b: BlockPos) -> Self {
        Self {
            world: world.to_owned(),
            min: [a.x.min(b.x), a.y.min(b.y), a.z.min(b.z)],
            max: [a.x.max(b.x), a.y.max(b.y), a.z.max(b.z)],
            mix: vec![MixEntry {
                block: "minecraft:stone".into(),
                weight: 100,
            }],
            interval: 300,
            threshold: 0,
        }
    }

    /// Minimum corner.
    #[must_use]
    pub const fn min_pos(&self) -> BlockPos {
        BlockPos {
            x: self.min[0],
            y: self.min[1],
            z: self.min[2],
        }
    }

    /// Maximum corner.
    #[must_use]
    pub const fn max_pos(&self) -> BlockPos {
        BlockPos {
            x: self.max[0],
            y: self.max[1],
            z: self.max[2],
        }
    }

    /// Number of blocks.
    #[must_use]
    pub fn volume(&self) -> u64 {
        (0..3)
            .map(|axis| u64::from(self.max[axis].abs_diff(self.min[axis])) + 1)
            .product()
    }

    /// Whether a block lies inside.
    #[must_use]
    pub const fn contains(&self, pos: BlockPos) -> bool {
        pos.x >= self.min[0]
            && pos.x <= self.max[0]
            && pos.y >= self.min[1]
            && pos.y <= self.max[1]
            && pos.z >= self.min[2]
            && pos.z <= self.max[2]
    }

    /// Whether a player standing at `(x, y, z)` is in the mine or on its floor-to-top column,
    /// and so must be lifted out before a reset.
    #[must_use]
    pub fn holds_player(&self, x: f64, y: f64, z: f64) -> bool {
        let inside = |value: f64, min: i32, max: i32| {
            value >= f64::from(min) && value < f64::from(max) + 1.0
        };
        inside(x, self.min[0], self.max[0])
            && inside(z, self.min[2], self.max[2])
            && y >= f64::from(self.min[1]) - 1.0
            && y < f64::from(self.max[1]) + 1.0
    }

    /// Whether `pos` is within `margin` blocks of the mine (for the countdown bar).
    #[must_use]
    pub fn is_near(&self, x: f64, y: f64, z: f64, margin: f64) -> bool {
        let near = |value: f64, min: i32, max: i32| {
            value >= f64::from(min) - margin && value <= f64::from(max) + 1.0 + margin
        };
        near(x, self.min[0], self.max[0])
            && near(y, self.min[1], self.max[1])
            && near(z, self.min[2], self.max[2])
    }

    /// Whether two mines in the same world share a block.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.world == other.world
            && (0..3)
                .all(|axis| self.min[axis] <= other.max[axis] && other.min[axis] <= self.max[axis])
    }

    /// Where `/mine tp` puts a player: the middle of the top face.
    #[must_use]
    pub fn top_center(&self) -> (f64, f64, f64) {
        let middle =
            |axis: usize| (f64::from(self.min[axis]) + f64::from(self.max[axis]) + 1.0) / 2.0;
        (middle(0), f64::from(self.max[1]) + 1.0, middle(2))
    }

    /// The mix as percentages, formatted like `/mine set` accepts it.
    #[must_use]
    pub fn mix_text(&self) -> String {
        let total: u32 = self.mix.iter().map(|entry| entry.weight).sum();
        self.mix
            .iter()
            .map(|entry| {
                let name = entry
                    .block
                    .strip_prefix("minecraft:")
                    .unwrap_or(&entry.block);
                let percent = f64::from(entry.weight) * 100.0 / f64::from(total.max(1));
                let percent = format!("{percent:.2}");
                let percent = percent.trim_end_matches('0').trim_end_matches('.');
                format!("{percent}%{name}")
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Parses a mix such as `60%stone,30%coal_ore,10%diamond_ore`. A bare block has weight 1;
/// weights are relative and need not add up to 100. Keys without a namespace get
/// `minecraft:`.
///
/// # Errors
/// Returns a message naming the first invalid entry.
pub fn parse_mix(text: &str) -> Result<Vec<MixEntry>, String> {
    let mut mix = Vec::new();
    for part in text
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let (weight, block) = match part.split_once('%') {
            Some((weight, block)) => {
                let weight: f64 = weight
                    .trim()
                    .parse()
                    .map_err(|_| format!("{part:?}: the weight must be a number"))?;
                if !(weight > 0.0 && weight.is_finite()) {
                    return Err(format!("{part:?}: the weight must be positive"));
                }
                // Hundredths keep `0.5%` precise while weights stay integers.
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "weights are positive and checked below u32::MAX / 100"
                )]
                let scaled = (weight.min(1_000_000.0) * 100.0).round() as u32;
                (scaled.max(1), block.trim())
            }
            None => (100, part),
        };
        if block.is_empty() {
            return Err(format!("{part:?}: missing block"));
        }
        let block = if block.contains(':') {
            block.to_owned()
        } else {
            format!("minecraft:{block}")
        };
        mix.push(MixEntry { block, weight });
    }
    if mix.is_empty() {
        return Err("the mix is empty".into());
    }
    Ok(normalize(mix))
}

/// Divides every weight by their greatest common divisor (`60%,30%,10%` is stored as 6, 3, 1).
fn normalize(mut mix: Vec<MixEntry>) -> Vec<MixEntry> {
    const fn gcd(a: u32, b: u32) -> u32 {
        if b == 0 { a } else { gcd(b, a % b) }
    }
    let divisor = mix
        .iter()
        .fold(0, |acc, entry| gcd(acc, entry.weight))
        .max(1);
    for entry in &mut mix {
        entry.weight /= divisor;
    }
    mix
}

/// Whether a mine with `mined` of `volume` blocks broken passes its reset threshold.
#[must_use]
pub fn threshold_reached(mined: u64, volume: u64, threshold: u8) -> bool {
    threshold > 0
        && volume > 0
        && mined.saturating_mul(100) >= volume.saturating_mul(u64::from(threshold))
}

/// A stable boss bar id per mine name (FNV-1a over the name, twice).
#[must_use]
pub fn bar_id(name: &str) -> [u8; 16] {
    let hash = |seed: u64| {
        name.bytes().fold(seed, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
    };
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&hash(0xcbf2_9ce4_8422_2325).to_be_bytes());
    bytes[8..].copy_from_slice(&hash(0x6c62_272e_07bb_0142).to_be_bytes());
    bytes
}

/// Mine names: 1–32 of `a-z`, `0-9`, `_` and `-`.
#[must_use]
pub fn valid_name(name: &str) -> Option<String> {
    let name = name.to_ascii_lowercase();
    (!name.is_empty()
        && name.len() <= 32
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        }))
    .then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos { x, y, z }
    }

    #[test]
    fn mixes_parse_percentages_bare_blocks_and_namespaces() {
        let Ok(mix) = parse_mix("60%stone, 30%coal_ore,10%minecraft:diamond_ore") else {
            panic!("valid mix");
        };
        assert_eq!(mix.len(), 3);
        assert_eq!(
            mix[0],
            MixEntry {
                block: "minecraft:stone".into(),
                weight: 6
            }
        );
        assert_eq!(mix[2].block, "minecraft:diamond_ore");
        let Ok(half) = parse_mix("99.5%stone,0.5%emerald_ore") else {
            panic!("fractional mix");
        };
        assert_eq!(half[0].weight, 199);
        assert_eq!(half[1].weight, 1);
        let Ok(bare) = parse_mix("stone,gold_ore") else {
            panic!("bare mix");
        };
        assert_eq!(bare[0].weight, bare[1].weight);
        for bad in ["", "abc%stone", "-5%stone", "10%", " , "] {
            assert!(parse_mix(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn mix_text_round_trips_through_the_parser() {
        let mut mine = Mine::new("home:overworld", pos(0, 0, 0), pos(1, 1, 1));
        let Ok(mix) = parse_mix("60%stone,40%iron_ore") else {
            panic!("valid mix");
        };
        mine.mix = mix;
        assert_eq!(mine.mix_text(), "60%stone,40%iron_ore");
        assert_eq!(parse_mix(&mine.mix_text()), Ok(mine.mix.clone()));
    }

    #[test]
    fn geometry_volume_overlap_and_player_lift_zone() {
        let mine = Mine::new("w", pos(10, 60, 10), pos(0, 40, 0));
        assert_eq!(mine.min, [0, 40, 0]);
        assert_eq!(mine.volume(), 11 * 21 * 11);
        assert!(mine.contains(pos(10, 60, 10)));
        assert!(!mine.contains(pos(11, 60, 10)));
        assert!(mine.holds_player(10.9, 55.0, 0.0));
        assert!(mine.holds_player(5.0, 39.0, 5.0), "standing on the floor");
        assert!(
            !mine.holds_player(5.0, 61.0, 5.0),
            "standing on top is safe"
        );
        assert!(mine.overlaps(&Mine::new("w", pos(10, 60, 10), pos(20, 70, 20))));
        assert!(!mine.overlaps(&Mine::new("w", pos(11, 40, 0), pos(20, 70, 20))));
        assert!(!mine.overlaps(&Mine::new("other", pos(0, 40, 0), pos(5, 50, 5))));
        assert_eq!(mine.top_center(), (5.5, 61.0, 5.5));
    }

    #[test]
    fn thresholds_trigger_at_the_configured_percentage() {
        assert!(!threshold_reached(49, 100, 50));
        assert!(threshold_reached(50, 100, 50));
        assert!(!threshold_reached(100, 100, 0), "0 disables");
        assert!(!threshold_reached(0, 0, 10));
        assert!(threshold_reached(u64::MAX, u64::MAX, 100));
    }

    #[test]
    fn mines_persist_as_toml() {
        let mut file = MineFile::default();
        let mut mine = Mine::new("home:overworld", pos(-5, 30, -5), pos(5, 50, 5));
        mine.threshold = 40;
        file.mines.insert("a".into(), mine);
        let Ok(text) = toml::to_string(&file) else {
            panic!("serializable");
        };
        let Ok(back) = toml::from_str::<MineFile>(&text) else {
            panic!("readable: {text}");
        };
        assert_eq!(back.mines, file.mines);
    }

    #[test]
    fn names_and_bar_ids() {
        assert_eq!(valid_name("Mine_A"), Some("mine_a".into()));
        assert_eq!(valid_name("bad name"), None);
        assert_ne!(bar_id("a"), bar_id("b"));
        assert_eq!(bar_id("a"), bar_id("a"));
    }
}
