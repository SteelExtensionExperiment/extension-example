//! Block patterns (`stone`, `50%stone,50%dirt`) and masks (`stone,oak_log[axis=x]`).
//!
//! Parsing is split into syntax (pure, testable) and resolution through the host block
//! registry. A mask entry without properties matches every state of that block; with
//! properties it matches that exact state.
use steel_extension_api::{BlockState, block_key, parse_block_state};

/// One `weight%block` entry before resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightedEntry {
    /// Relative weight (percentages need not sum to 100).
    pub weight: f64,
    /// Block state text.
    pub block: String,
}

/// Splits a pattern into weighted entries.
pub fn parse_pattern_syntax(input: &str) -> Result<Vec<WeightedEntry>, String> {
    let mut entries = Vec::new();
    for part in split_top_level(input) {
        let part = part.trim();
        if part.is_empty() {
            return Err("empty pattern entry".into());
        }
        let (weight, block) = match part.split_once('%') {
            Some((weight, block)) => {
                let weight: f64 = weight
                    .parse()
                    .map_err(|_| format!("invalid weight in {part:?}"))?;
                if !weight.is_finite() || weight <= 0.0 {
                    return Err(format!("weight must be positive in {part:?}"));
                }
                (weight, block)
            }
            None => (1.0, part),
        };
        if block.is_empty() {
            return Err(format!("missing block in {part:?}"));
        }
        entries.push(WeightedEntry {
            weight,
            block: block.to_owned(),
        });
    }
    if entries.is_empty() {
        return Err("empty pattern".into());
    }
    Ok(entries)
}

/// Splits on commas outside `[...]` property lists.
fn split_top_level(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0_i32;
    let mut start = 0;
    for (index, character) in input.char_indices() {
        match character {
            '[' => depth += 1,
            ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&input[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&input[start..]);
    parts
}

/// A resolved weighted block pattern.
#[derive(Debug, Clone)]
pub struct Pattern {
    entries: Vec<(f64, BlockState)>,
    total: f64,
}

impl Pattern {
    /// Resolves pattern text through the block registry.
    pub fn parse(input: &str) -> Result<Self, String> {
        let entries = parse_pattern_syntax(input)?
            .into_iter()
            .map(|entry| {
                let state = parse_block_state(entry.block.as_str().into())
                    .match_owned(Ok, |error| Err(format!("{}: {error}", entry.block)))?;
                Ok((entry.weight, state))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self::from_states(entries))
    }

    /// Builds a pattern from resolved weights.
    pub fn from_states(entries: Vec<(f64, BlockState)>) -> Self {
        let total = entries.iter().map(|(weight, _)| weight).sum();
        Self { entries, total }
    }

    /// Picks a state using `roll` in `0.0..1.0`; `None` only for an empty pattern.
    pub fn pick(&self, roll: f64) -> Option<BlockState> {
        let mut target = roll * self.total;
        for (weight, state) in &self.entries {
            if target < *weight {
                return Some(*state);
            }
            target -= weight;
        }
        self.entries.last().map(|(_, state)| *state)
    }
}

/// Which states a mask entry accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
enum MaskEntry {
    /// Any state of a block key.
    Block(String),
    /// One exact state.
    State(BlockState),
}

/// A set of accepted block states.
#[derive(Debug, Clone)]
pub struct Mask {
    entries: Vec<MaskEntry>,
}

impl Mask {
    /// Resolves mask text through the block registry.
    pub fn parse(input: &str) -> Result<Self, String> {
        let entries = split_top_level(input)
            .into_iter()
            .map(|part| {
                let part = part.trim();
                let state = parse_block_state(part.into())
                    .match_owned(Ok, |error| Err(format!("{part}: {error}")))?;
                Ok(if part.contains('[') {
                    MaskEntry::State(state)
                } else {
                    MaskEntry::Block(block_key(state).to_string())
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self { entries })
    }

    /// Whether `state` is accepted. `key` is the state's block key.
    pub fn matches(&self, state: BlockState, key: &str) -> bool {
        self.entries.iter().any(|entry| match entry {
            MaskEntry::Block(block) => block == key,
            MaskEntry::State(expected) => *expected == state,
        })
    }
}

/// Small xorshift generator for pattern rolls; edits need no reproducible randomness.
pub struct Rng(u64);

impl Rng {
    /// Seeds from the clock.
    pub fn new() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9E37_79B9_7F4A_7C15, |time| {
                time.as_secs() ^ u64::from(time.subsec_nanos()).rotate_left(32)
            });
        Self(seed | 1)
    }

    /// Next value in `0.0..1.0`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "53 random mantissa bits are taken on purpose"
    )]
    pub fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1_u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_and_properties_split_correctly() {
        let entries = parse_pattern_syntax("50%stone, 25%oak_log[axis=x,foo=y],dirt");
        assert_eq!(
            entries,
            Ok(vec![
                WeightedEntry {
                    weight: 50.0,
                    block: "stone".into()
                },
                WeightedEntry {
                    weight: 25.0,
                    block: "oak_log[axis=x,foo=y]".into()
                },
                WeightedEntry {
                    weight: 1.0,
                    block: "dirt".into()
                },
            ])
        );
    }

    #[test]
    fn invalid_patterns_are_rejected() {
        assert!(parse_pattern_syntax("").is_err());
        assert!(parse_pattern_syntax("stone,,dirt").is_err());
        assert!(parse_pattern_syntax("0%stone").is_err());
        assert!(parse_pattern_syntax("x%stone").is_err());
        assert!(parse_pattern_syntax("5%").is_err());
    }

    #[test]
    fn rng_stays_in_unit_interval() {
        let mut rng = Rng(42);
        for _ in 0..1000 {
            let value = rng.next_f64();
            assert!((0.0..1.0).contains(&value));
        }
    }
}
