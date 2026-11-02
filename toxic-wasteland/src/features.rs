//! Wasteland decorations: dead trees, surface uranium deposits, and nuclear waste
//! barrels leaking at the edge of sludge lakes.
//!
//! Every decoration is decided from positional seeded samples and stays inside the
//! chunk that generates it, like the lakes, because terrain generation writes one
//! chunk at a time.

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "chunk-local offsets (0..16), tree heights, and small sample remainders \
              are non-negative and fit their target types"
)]

use crate::config::WorldGen;
use crate::generator::{Column, Lake, Samples, unit};

const TREE_SALT: u64 = 0x7761_7374_655f_7400;
const URANIUM_SALT: u64 = 0x7761_7374_655f_7500;
const BARREL_SALT: u64 = 0x7761_7374_655f_6200;
/// Tree slots tried per chunk.
const TREE_SLOTS: u64 = 2;
/// How far a tree reaches from its trunk: branches (2) plus a lean (1).
const TREE_REACH: i32 = 3;

/// What a decoration block is; the generator maps each kind to a block state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A vertical dead log.
    LogY,
    /// A dead log lying east-west.
    LogX,
    /// A dead log lying north-south.
    LogZ,
    /// Pale hanging moss under a branch.
    Moss,
    Uranium,
    Barrel,
}

/// One decoration block in world coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub kind: Kind,
}

/// Horizontal directions as (dx, dz), with the log axis that points along them.
const DIRECTIONS: [(i32, i32, Kind); 4] = [
    (1, 0, Kind::LogX),
    (-1, 0, Kind::LogX),
    (0, 1, Kind::LogZ),
    (0, -1, Kind::LogZ),
];

/// A chunk's terrain, for placing decorations on it.
pub struct Ground<'a> {
    pub min_x: i32,
    pub min_z: i32,
    pub columns: &'a [[Column; 16]; 16],
    pub lake: Option<Lake>,
}

impl Ground<'_> {
    fn column(&self, x: i32, z: i32) -> Column {
        self.columns[(z - self.min_z) as usize][(x - self.min_x) as usize]
    }

    /// Dry ground away from any lake and its rim.
    fn is_dry(&self, x: i32, z: i32) -> bool {
        self.column(x, z).sludge_top.is_none() && self.lake.is_none_or(|lake| !lake.holds(x, z))
    }

    /// A column `margin` blocks or more from every chunk edge, chosen by `sample`.
    fn pick(&self, sample: u64, margin: i32) -> (i32, i32) {
        let span = (16 - 2 * margin) as u64;
        (
            self.min_x + margin + (sample % span) as i32,
            self.min_z + margin + ((sample >> 16) % span) as i32,
        )
    }
}

/// Every decoration of one chunk.
pub fn decorate(samples: &impl Samples, world: &WorldGen, ground: &Ground<'_>) -> Vec<Placement> {
    let mut placed = Vec::new();
    for slot in 0..TREE_SLOTS {
        dead_tree(samples, world, ground, slot, &mut placed);
    }
    uranium(samples, world, ground, &mut placed);
    barrels(samples, ground, &mut placed);
    placed
}

/// A leafless tree: a leaning trunk, two or three crooked branches, exposed roots,
/// and moss hanging from some branches.
fn dead_tree(
    samples: &impl Samples,
    world: &WorldGen,
    ground: &Ground<'_>,
    slot: u64,
    placed: &mut Vec<Placement>,
) {
    let salt = TREE_SALT + slot * 8;
    if unit(samples.sample(salt, ground.min_x, ground.min_z)) >= world.tree_chance {
        return;
    }
    let shape = samples.sample(salt + 1, ground.min_x, ground.min_z);
    let (x, z) = ground.pick(shape, TREE_REACH);
    if !ground.is_dry(x, z) {
        return;
    }
    let base = ground.column(x, z).ground;
    let height = 5 + (shape >> 32) as i32 % 4;
    let (lean_x, lean_z, _) = DIRECTIONS[((shape >> 36) % 4) as usize];
    let lean_at = 2 + (shape >> 40) as i32 % 2;
    let mut trunk = Vec::with_capacity(height as usize);
    for level in 1..=height {
        // Above `lean_at`, the trunk has slumped one block over.
        let (dx, dz) = if level > lean_at {
            (lean_x, lean_z)
        } else {
            (0, 0)
        };
        trunk.push((x + dx, base + level, z + dz));
        placed.push(Placement {
            x: x + dx,
            y: base + level,
            z: z + dz,
            kind: Kind::LogY,
        });
    }

    // Branches leave the upper trunk in distinct directions, then climb one block.
    let branches = 2 + (shape >> 44) % 2;
    let first = (shape >> 46) % 4;
    for branch in 0..branches {
        let (dx, dz, axis) = DIRECTIONS[((first + branch * 2 + branch / 2) % 4) as usize];
        let (tx, ty, tz) = trunk[(height - 1 - branch as i32).max(0) as usize];
        placed.push(Placement {
            x: tx + dx,
            y: ty,
            z: tz + dz,
            kind: axis,
        });
        placed.push(Placement {
            x: tx + 2 * dx,
            y: ty + 1,
            z: tz + 2 * dz,
            kind: axis,
        });
        if (shape >> (48 + branch)) & 1 == 1 {
            placed.push(Placement {
                x: tx + dx,
                y: ty - 1,
                z: tz + dz,
                kind: Kind::Moss,
            });
        }
    }

    // Roots break through the ground beside the trunk.
    let roots = 1 + (shape >> 52) % 2;
    for root in 0..roots {
        let (dx, dz, axis) = DIRECTIONS[((shape >> 54) as usize + root as usize * 2) % 4];
        placed.push(Placement {
            x: x + dx,
            y: base,
            z: z + dz,
            kind: axis,
        });
    }
}

/// A patch of uranium ore in the surface, visible from afar.
fn uranium(
    samples: &impl Samples,
    world: &WorldGen,
    ground: &Ground<'_>,
    placed: &mut Vec<Placement>,
) {
    if unit(samples.sample(URANIUM_SALT, ground.min_x, ground.min_z)) >= world.uranium_chance {
        return;
    }
    let shape = samples.sample(URANIUM_SALT + 1, ground.min_x, ground.min_z);
    let radius = 1 + (shape >> 32) as i32 % 2;
    let (cx, cz) = ground.pick(shape, radius);
    for x in cx - radius..=cx + radius {
        for z in cz - radius..=cz + radius {
            let (dx, dz) = (x - cx, z - cz);
            // A rough disc: the corners of the square are left out.
            if dx * dx + dz * dz > radius * radius || !ground.is_dry(x, z) {
                continue;
            }
            if unit(samples.sample(URANIUM_SALT + 2, x, z)) < 0.7 {
                placed.push(Placement {
                    x,
                    y: ground.column(x, z).ground,
                    z,
                    kind: Kind::Uranium,
                });
            }
        }
    }
}

/// One to three barrels on the rim of the chunk's sludge lake, where they leaked.
fn barrels(samples: &impl Samples, ground: &Ground<'_>, placed: &mut Vec<Placement>) {
    let Some(lake) = ground.lake else {
        return;
    };
    let mut rim: Vec<(i32, i32)> = Vec::new();
    for z in ground.min_z..ground.min_z + 16 {
        for x in ground.min_x..ground.min_x + 16 {
            if lake.holds(x, z) && !lake.contains(x, z) {
                rim.push((x, z));
            }
        }
    }
    if rim.is_empty() {
        return;
    }
    let count = 1 + samples.sample(BARREL_SALT, ground.min_x, ground.min_z) % 3;
    for index in 0..count {
        let pick = samples.sample(BARREL_SALT + 1 + index, ground.min_x, ground.min_z);
        let (x, z) = rim[(pick % rim.len() as u64) as usize];
        let y = ground.column(x, z).ground + 1;
        if !placed.iter().any(|p| (p.x, p.y, p.z) == (x, y, z)) {
            placed.push(Placement {
                x,
                y,
                z,
                kind: Kind::Barrel,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::generator::{columns, tests::Seeded};

    fn chunk_features(
        world: &WorldGen,
        seed: u64,
        min_x: i32,
        min_z: i32,
    ) -> (Vec<Placement>, [[Column; 16]; 16]) {
        let samples = Seeded(seed);
        let columns = columns(&samples, world, min_x, min_z);
        let ground = Ground {
            min_x,
            min_z,
            columns: &columns,
            lake: Lake::of_chunk(&samples, world, min_x, min_z),
        };
        (decorate(&samples, world, &ground), columns)
    }

    fn always() -> WorldGen {
        WorldGen {
            tree_chance: 1.0,
            uranium_chance: 1.0,
            lake_chance: 0.5,
            ..Config::default().world
        }
    }

    #[test]
    fn decorations_stay_inside_their_chunk() {
        let world = always();
        for chunk in 0..300 {
            let (min_x, min_z) = (chunk * 16 - 2400, chunk * 48);
            let (placed, _) = chunk_features(&world, 7, min_x, min_z);
            for p in placed {
                assert!(
                    (min_x..min_x + 16).contains(&p.x) && (min_z..min_z + 16).contains(&p.z),
                    "{p:?}"
                );
            }
        }
    }

    #[test]
    fn trees_stand_on_dry_ground_and_uranium_is_exposed() {
        let world = always();
        let mut trees = 0;
        for chunk in 0..200 {
            let (min_x, min_z) = (chunk * 16, -chunk * 16);
            let (placed, columns) = chunk_features(&world, 11, min_x, min_z);
            let column = |x: i32, z: i32| columns[(z - min_z) as usize][(x - min_x) as usize];
            let lowest_log = placed
                .iter()
                .filter(|p| p.kind == Kind::LogY)
                .min_by_key(|p| p.y);
            if let Some(log) = lowest_log {
                trees += 1;
                assert_eq!(
                    column(log.x, log.z).ground + 1,
                    log.y,
                    "trunk floats or is buried"
                );
                assert!(column(log.x, log.z).sludge_top.is_none());
            }
            for ore in placed.iter().filter(|p| p.kind == Kind::Uranium) {
                assert_eq!(
                    column(ore.x, ore.z).ground,
                    ore.y,
                    "uranium must be at the surface"
                );
            }
        }
        assert!(trees > 100, "only {trees} trees in 200 chunks");
    }

    #[test]
    fn barrels_sit_on_the_lake_rim() {
        let world = WorldGen {
            lake_chance: 1.0,
            ..always()
        };
        let mut barrels = 0;
        for chunk in 0..100 {
            let (min_x, min_z) = (chunk * 32, chunk * 16);
            let (placed, columns) = chunk_features(&world, 5, min_x, min_z);
            let lake = Lake::of_chunk(&Seeded(5), &world, min_x, min_z).expect("lake chance is 1");
            for barrel in placed.iter().filter(|p| p.kind == Kind::Barrel) {
                barrels += 1;
                assert!(lake.holds(barrel.x, barrel.z) && !lake.contains(barrel.x, barrel.z));
                let column = columns[(barrel.z - min_z) as usize][(barrel.x - min_x) as usize];
                assert_eq!(column.ground + 1, barrel.y);
            }
        }
        assert!(barrels >= 100, "every lake gets at least one barrel");
    }

    #[test]
    fn decorations_are_deterministic_and_modest() {
        let world = always();
        let (first, _) = chunk_features(&world, 3, 160, -80);
        let (second, _) = chunk_features(&world, 3, 160, -80);
        assert_eq!(first, second);
        // Each nuclear block is one client display; keep them few per chunk.
        for chunk in 0..200 {
            let (placed, _) = chunk_features(&world, 13, chunk * 16, 0);
            let displays = placed
                .iter()
                .filter(|p| matches!(p.kind, Kind::Uranium | Kind::Barrel))
                .count();
            assert!(displays <= 16, "{displays} nuclear blocks in one chunk");
        }
    }
}
