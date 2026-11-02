//! The `toxic_wasteland:wasteland` terrain generator: barren hills of coarse dirt and
//! gravel over stone, in the wasteland biome, dotted with sludge lakes.
//!
//! Heights come from three octaves of value noise over positional seeded samples, so a
//! seed always produces the same world regardless of chunk order. Each lake is decided
//! by its own chunk and stays inside it, because terrain generation writes one chunk.

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "chunk-local offsets (0..16), small sample remainders, and noise bounded by \
              `relief` all fit their target types"
)]

use steel_extension_api::{BlockChange, BlockPos, BlockState, Terrain, stabby};

use crate::{Materials, config::WorldGen};

const HEIGHT_SALT: u64 = 0x7761_7374_655f_6800;
const LAKE_SALT: u64 = 0x7761_7374_655f_6c00;
/// Octave lattice spacings in blocks and their weights.
const OCTAVES: [(i32, f64); 3] = [(64, 0.6), (24, 0.3), (8, 0.1)];
/// Depth of the coarse dirt and gravel layer above stone.
const SOIL_DEPTH: i32 = 4;

/// Positional samples from the world seed.
pub trait Samples {
    fn sample(&self, salt: u64, x: i32, z: i32) -> u64;
}

impl Samples for Terrain<'_> {
    fn sample(&self, salt: u64, x: i32, z: i32) -> u64 {
        self.random_at(salt, BlockPos { x, y: 0, z })
    }
}

fn unit(sample: u64) -> f64 {
    // The top 53 bits map exactly onto [0, 1).
    (sample >> 11) as f64 / (1_u64 << 53) as f64
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise in [-1, 1] for one octave.
fn octave(samples: &impl Samples, octave: usize, spacing: i32, x: i32, z: i32) -> f64 {
    let salt = HEIGHT_SALT + octave as u64;
    let (gx, gz) = (x.div_euclid(spacing), z.div_euclid(spacing));
    let tx = smooth(f64::from(x.rem_euclid(spacing)) / f64::from(spacing));
    let tz = smooth(f64::from(z.rem_euclid(spacing)) / f64::from(spacing));
    let corner = |dx: i32, dz: i32| unit(samples.sample(salt, gx + dx, gz + dz)) * 2.0 - 1.0;
    let north = corner(0, 0) + (corner(1, 0) - corner(0, 0)) * tx;
    let south = corner(0, 1) + (corner(1, 1) - corner(0, 1)) * tx;
    north + (south - north) * tz
}

/// Surface height of a column.
pub fn height(samples: &impl Samples, world: &WorldGen, x: i32, z: i32) -> i32 {
    let noise: f64 = OCTAVES
        .iter()
        .enumerate()
        .map(|(index, (spacing, weight))| octave(samples, index, *spacing, x, z) * weight)
        .sum();
    world.base_height + (noise * f64::from(world.relief)).round() as i32
}

/// A chunk-local sludge basin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lake {
    pub center_x: i32,
    pub center_z: i32,
    pub radius: i32,
    pub depth: i32,
}

impl Lake {
    /// The lake of the chunk with minimum corner `(min_x, min_z)`, if it has one.
    pub fn of_chunk(
        samples: &impl Samples,
        world: &WorldGen,
        min_x: i32,
        min_z: i32,
    ) -> Option<Self> {
        let roll = samples.sample(LAKE_SALT, min_x, min_z);
        if unit(roll) >= world.lake_chance {
            return None;
        }
        let shape = samples.sample(LAKE_SALT + 1, min_x, min_z);
        let radius = 3 + (shape % 3) as i32;
        // Keep the basin and the ring around it inside the chunk: the center lies in
        // `radius + 1 ..= 14 - radius` of the chunk.
        let span = (13 - 2 * radius) as u64;
        Some(Self {
            center_x: min_x + radius + 1 + ((shape >> 8) % (span + 1)) as i32,
            center_z: min_z + radius + 1 + ((shape >> 16) % (span + 1)) as i32,
            radius,
            depth: 2 + ((shape >> 24) % 3) as i32,
        })
    }

    fn distance_sq(&self, x: i32, z: i32) -> i32 {
        let (dx, dz) = (x - self.center_x, z - self.center_z);
        dx * dx + dz * dz
    }

    /// Whether a column belongs to the basin.
    pub fn contains(&self, x: i32, z: i32) -> bool {
        self.distance_sq(x, z) <= self.radius * self.radius
    }

    /// Whether a column belongs to the basin or the ring that holds it in.
    fn holds(&self, x: i32, z: i32) -> bool {
        let rim = self.radius + 1;
        self.distance_sq(x, z) <= rim * rim
    }

    /// Depth below the surface at a basin column, deepest in the middle.
    fn depth_at(&self, x: i32, z: i32) -> i32 {
        let edge = f64::from(self.distance_sq(x, z)) / f64::from(self.radius * self.radius);
        1 + (f64::from(self.depth - 1) * (1.0 - edge)).round() as i32
    }
}

/// One column's materials from the bottom up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    /// Highest solid block.
    pub ground: i32,
    /// Highest sludge block, above `ground`, when the column is in a lake.
    pub sludge_top: Option<i32>,
}

/// Columns of a chunk, indexed `[z][x]` from its minimum corner.
pub fn columns(
    samples: &impl Samples,
    world: &WorldGen,
    min_x: i32,
    min_z: i32,
) -> [[Column; 16]; 16] {
    let mut heights = [[0; 16]; 16];
    for (dz, row) in heights.iter_mut().enumerate() {
        for (dx, h) in row.iter_mut().enumerate() {
            *h = height(samples, world, min_x + dx as i32, min_z + dz as i32);
        }
    }
    let lake = Lake::of_chunk(samples, world, min_x, min_z);
    // The surface sits one block below the lowest column of the basin and its ring,
    // so every lake is contained by terrain.
    let surface = lake.map(|lake| {
        let mut lowest = i32::MAX;
        for (dz, row) in heights.iter().enumerate() {
            for (dx, h) in row.iter().enumerate() {
                if lake.holds(min_x + dx as i32, min_z + dz as i32) {
                    lowest = lowest.min(*h);
                }
            }
        }
        lowest - 1
    });
    let mut columns = [[Column {
        ground: 0,
        sludge_top: None,
    }; 16]; 16];
    for (dz, row) in columns.iter_mut().enumerate() {
        for (dx, column) in row.iter_mut().enumerate() {
            let (x, z) = (min_x + dx as i32, min_z + dz as i32);
            let ground = heights[dz][dx];
            *column = match (lake, surface) {
                (Some(lake), Some(surface)) if lake.contains(x, z) => Column {
                    ground: ground.min(surface - lake.depth_at(x, z)),
                    sludge_top: Some(surface),
                },
                _ => Column {
                    ground,
                    sludge_top: None,
                },
            };
        }
    }
    columns
}

/// Material of a solid block at `y` in a column whose ground is `ground`.
fn solid(materials: &Materials, x: i32, y: i32, z: i32, ground: i32) -> BlockState {
    if y <= ground - SOIL_DEPTH {
        return materials.stone;
    }
    // A fixed pattern breaks up the soil without consuming random samples.
    if (x * 7 + z * 13 + y * 3).rem_euclid(5) == 0 {
        materials.gravel
    } else {
        materials.coarse_dirt
    }
}

#[stabby::export(callback)]
pub fn generate(terrain: Terrain<'_>) {
    let config = crate::config();
    let materials = crate::materials();
    let (min_x, min_z, min_y) = (terrain.min_x(), terrain.min_z(), terrain.min_y());
    let columns = columns(&terrain, &config.world, min_x, min_z);
    let mut changes = Vec::with_capacity(16 * 16 * 96);
    for (dz, row) in columns.iter().enumerate() {
        for (dx, column) in row.iter().enumerate() {
            let (x, z) = (min_x + dx as i32, min_z + dz as i32);
            changes.push(BlockChange {
                pos: BlockPos { x, y: min_y, z },
                state: materials.bedrock,
            });
            for y in min_y + 1..=column.ground {
                changes.push(BlockChange {
                    pos: BlockPos { x, y, z },
                    state: solid(materials, x, y, z, column.ground),
                });
            }
            if let Some(top) = column.sludge_top {
                for y in column.ground + 1..=top {
                    changes.push(BlockChange {
                        pos: BlockPos { x, y, z },
                        state: materials.sludge[0],
                    });
                }
            }
        }
    }
    if let Err(error) = terrain
        .write_blocks(changes.as_slice().into())
        .match_owned(Ok, Err)
    {
        panic!("wasteland generation produced an invalid terrain batch: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SplitMix64` over (seed, salt, x, z), standing in for the host's sampler.
    struct Seeded(u64);

    impl Samples for Seeded {
        fn sample(&self, salt: u64, x: i32, z: i32) -> u64 {
            let mut v =
                self.0 ^ salt ^ (u64::from(x.cast_unsigned()) << 32) ^ u64::from(z.cast_unsigned());
            v = v.wrapping_add(0x9E37_79B9_7F4A_7C15);
            v = (v ^ (v >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            v = (v ^ (v >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            v ^ (v >> 31)
        }
    }

    fn world() -> WorldGen {
        crate::config::Config::default().world
    }

    #[test]
    fn heights_stay_within_relief_and_are_deterministic() {
        let world = world();
        for x in -100..100 {
            let h = height(&Seeded(1), &world, x, x * 3);
            assert!(
                (world.base_height - world.relief..=world.base_height + world.relief).contains(&h)
            );
            assert_eq!(h, height(&Seeded(1), &world, x, x * 3));
        }
    }

    #[test]
    fn lakes_stay_inside_their_chunk_with_a_rim() {
        let world = WorldGen {
            lake_chance: 1.0,
            ..world()
        };
        for chunk in 0..200 {
            let (min_x, min_z) = (chunk * 16, -chunk * 16);
            let lake = Lake::of_chunk(&Seeded(9), &world, min_x, min_z).expect("lake chance is 1");
            let rim = lake.radius + 1;
            assert!(lake.center_x - rim >= min_x && lake.center_x + rim < min_x + 16);
            assert!(lake.center_z - rim >= min_z && lake.center_z + rim < min_z + 16);
        }
    }

    #[test]
    fn every_lake_column_is_held_in_by_higher_ground() {
        let world = WorldGen {
            lake_chance: 1.0,
            ..world()
        };
        for chunk in 0..100 {
            let (min_x, min_z) = (chunk * 16, chunk * 32);
            let columns = columns(&Seeded(3), &world, min_x, min_z);
            let lake = Lake::of_chunk(&Seeded(3), &world, min_x, min_z).expect("lake chance is 1");
            for dz in 0..16 {
                for dx in 0..16 {
                    let (x, z) = (min_x + dx as i32, min_z + dz as i32);
                    let column = columns[dz][dx];
                    let Some(top) = column.sludge_top else {
                        if lake.holds(x, z) {
                            assert!(
                                column.ground > top_of(&columns),
                                "ring column below the surface"
                            );
                        }
                        continue;
                    };
                    assert!(column.ground < top);
                }
            }
        }
    }

    fn top_of(columns: &[[Column; 16]; 16]) -> i32 {
        columns
            .iter()
            .flatten()
            .find_map(|column| column.sludge_top)
            .unwrap_or(i32::MIN)
    }

    #[test]
    fn lake_coverage_matches_the_default_chance() {
        let world = world();
        let mut sludge_columns = 0;
        let chunks = 32 * 32;
        for cx in 0..32 {
            for cz in 0..32 {
                let columns = columns(&Seeded(42), &world, cx * 16, cz * 16);
                sludge_columns += columns
                    .iter()
                    .flatten()
                    .filter(|c| c.sludge_top.is_some())
                    .count();
            }
        }
        let coverage = sludge_columns as f64 / f64::from(chunks * 256);
        assert!((0.02..=0.15).contains(&coverage), "coverage {coverage}");
    }
}
