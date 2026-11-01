//! Cuboid regions, block changes with history, and clipboards.
//!
//! Edits run synchronously on the caller's thread like vanilla `/fill`. Every chunk an edit
//! touches must already be loaded; the check happens before the first write so an edit is never
//! applied partially. Positions outside the world's height are skipped.
// TODO: block entity contents (chests, signs) are not copied or restored by undo.
use steel_extension_api::{BlockPos, BlockState, BlockUpdateFlags, World, block_key};
use worldedit_api::Cuboid;

/// One axis direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// +Y
    Up,
    /// -Y
    Down,
    /// -Z
    North,
    /// +Z
    South,
    /// +X
    East,
    /// -X
    West,
}

impl Direction {
    /// Parses a direction name or its first letter.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.to_lowercase().as_str() {
            "up" | "u" => Self::Up,
            "down" | "d" => Self::Down,
            "north" | "n" => Self::North,
            "south" | "s" => Self::South,
            "east" | "e" => Self::East,
            "west" | "w" => Self::West,
            _ => return None,
        })
    }

    /// The direction a player looks, from vanilla yaw (0 = south) and pitch.
    pub fn facing(yaw: f32, pitch: f32) -> Self {
        if pitch < -67.5 {
            return Self::Up;
        }
        if pitch > 67.5 {
            return Self::Down;
        }
        let yaw = yaw.rem_euclid(360.0);
        if !(45.0..315.0).contains(&yaw) {
            Self::South
        } else if yaw < 135.0 {
            Self::West
        } else if yaw < 225.0 {
            Self::North
        } else {
            Self::East
        }
    }

    const fn offset(self) -> (i32, i32, i32) {
        match self {
            Self::Up => (0, 1, 0),
            Self::Down => (0, -1, 0),
            Self::North => (0, 0, -1),
            Self::South => (0, 0, 1),
            Self::East => (1, 0, 0),
            Self::West => (-1, 0, 0),
        }
    }
}

/// Moves the face of `cuboid` facing `direction` outward by `amount`.
pub fn expand(cuboid: &Cuboid, amount: i32, direction: Direction) -> Cuboid {
    let mut result = cuboid.clone();
    let (x, y, z) = direction.offset();
    let face = |min: &mut i32, max: &mut i32, sign: i32| {
        if sign > 0 {
            *max = max.saturating_add(amount);
        } else if sign < 0 {
            *min = min.saturating_sub(amount);
        }
    };
    face(&mut result.min.x, &mut result.max.x, x);
    face(&mut result.min.y, &mut result.max.y, y);
    face(&mut result.min.z, &mut result.max.z, z);
    result
}

/// Moves the face opposite `direction` inward by `amount`; fails if the box would vanish.
pub fn contract(cuboid: &Cuboid, amount: i32, direction: Direction) -> Result<Cuboid, String> {
    let mut result = cuboid.clone();
    let (x, y, z) = direction.offset();
    let face = |min: &mut i32, max: &mut i32, sign: i32| {
        if sign > 0 {
            *min = min.saturating_add(amount);
        } else if sign < 0 {
            *max = max.saturating_sub(amount);
        }
    };
    face(&mut result.min.x, &mut result.max.x, x);
    face(&mut result.min.y, &mut result.max.y, y);
    face(&mut result.min.z, &mut result.max.z, z);
    if result.min.x > result.max.x || result.min.y > result.max.y || result.min.z > result.max.z {
        return Err("the selection cannot contract that far".into());
    }
    Ok(result)
}

/// Every block of the cuboid, in x, z, y order.
pub fn positions(cuboid: &Cuboid) -> impl Iterator<Item = BlockPos> + '_ {
    (cuboid.min.y..=cuboid.max.y).flat_map(move |y| {
        (cuboid.min.z..=cuboid.max.z)
            .flat_map(move |z| (cuboid.min.x..=cuboid.max.x).map(move |x| BlockPos { x, y, z }))
    })
}

/// Blocks on the four vertical sides of the cuboid.
pub fn walls(cuboid: &Cuboid) -> impl Iterator<Item = BlockPos> + '_ {
    positions(cuboid).filter(move |pos| {
        pos.x == cuboid.min.x
            || pos.x == cuboid.max.x
            || pos.z == cuboid.min.z
            || pos.z == cuboid.max.z
    })
}

/// One applied block change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    /// Changed block.
    pub pos: BlockPos,
    /// State before the edit.
    pub before: BlockState,
    /// State after the edit.
    pub after: BlockState,
}

/// Changes made by one command, undoable as a unit.
#[derive(Debug, Clone, Default)]
pub struct ChangeSet {
    /// World key.
    pub world: String,
    /// Applied changes in application order.
    pub changes: Vec<Change>,
}

/// Fails unless every chunk overlapping `min..=max` horizontally is loaded.
pub fn ensure_loaded(world: &World<'_>, min: BlockPos, max: BlockPos) -> Result<(), String> {
    for chunk_x in min.x.div_euclid(16)..=max.x.div_euclid(16) {
        for chunk_z in min.z.div_euclid(16)..=max.z.div_euclid(16) {
            if !world.is_chunk_loaded(chunk_x, chunk_z) {
                return Err(format!(
                    "chunk {chunk_x}, {chunk_z} is not loaded; move closer and try again"
                ));
            }
        }
    }
    Ok(())
}

/// Bounding box of a set of positions.
fn bounds(positions: &[BlockPos]) -> Option<(BlockPos, BlockPos)> {
    let first = *positions.first()?;
    Some(positions.iter().fold((first, first), |(min, max), pos| {
        (
            BlockPos {
                x: min.x.min(pos.x),
                y: min.y.min(pos.y),
                z: min.z.min(pos.z),
            },
            BlockPos {
                x: max.x.max(pos.x),
                y: max.y.max(pos.y),
                z: max.z.max(pos.z),
            },
        )
    }))
}

fn read(world: &World<'_>, pos: BlockPos) -> Result<BlockState, String> {
    world
        .block(pos)
        .match_owned(Ok, |error| Err(error.to_string()))
}

fn write(world: &World<'_>, pos: BlockPos, state: BlockState) -> Result<(), String> {
    world
        .set_block_with_flags(pos, state, BlockUpdateFlags::ALL)
        .match_owned(Ok, |error| Err(error.to_string()))
}

/// Applies `edits` after checking the block `limit` and loaded chunks. `choose` maps each
/// position and its current state to the new state, or `None` to leave it.
pub fn apply(
    world: &World<'_>,
    targets: Vec<BlockPos>,
    limit: u64,
    mut choose: impl FnMut(BlockPos, BlockState) -> Option<BlockState>,
) -> Result<ChangeSet, String> {
    let (min_y, max_y) = (world.min_y(), world.max_y());
    let targets: Vec<BlockPos> = targets
        .into_iter()
        .filter(|pos| (min_y..=max_y).contains(&pos.y))
        .collect();
    if u64::try_from(targets.len()).unwrap_or(u64::MAX) > limit {
        return Err(format!(
            "this edit changes {} blocks, more than the limit of {limit}",
            targets.len()
        ));
    }
    let Some((min, max)) = bounds(&targets) else {
        return Ok(ChangeSet {
            world: world.key().to_string(),
            changes: Vec::new(),
        });
    };
    ensure_loaded(world, min, max)?;
    let mut set = ChangeSet {
        world: world.key().to_string(),
        changes: Vec::new(),
    };
    for pos in targets {
        let before = read(world, pos)?;
        let Some(after) = choose(pos, before) else {
            continue;
        };
        if after == before {
            continue;
        }
        write(world, pos, after)?;
        set.changes.push(Change { pos, before, after });
    }
    Ok(set)
}

/// Restores every `before` state, newest first.
pub fn undo(world: &World<'_>, set: &ChangeSet) -> Result<(), String> {
    check_set_loaded(world, set)?;
    for change in set.changes.iter().rev() {
        write(world, change.pos, change.before)?;
    }
    Ok(())
}

/// Reapplies every `after` state, oldest first.
pub fn redo(world: &World<'_>, set: &ChangeSet) -> Result<(), String> {
    check_set_loaded(world, set)?;
    for change in &set.changes {
        write(world, change.pos, change.after)?;
    }
    Ok(())
}

fn check_set_loaded(world: &World<'_>, set: &ChangeSet) -> Result<(), String> {
    if world.key().as_str() != set.world {
        return Err(format!("that edit was made in {}", set.world));
    }
    let positions: Vec<BlockPos> = set.changes.iter().map(|change| change.pos).collect();
    match bounds(&positions) {
        Some((min, max)) => ensure_loaded(world, min, max),
        None => Ok(()),
    }
}

/// Copied blocks relative to the copy origin.
#[derive(Debug, Clone, Default)]
pub struct Clipboard {
    /// Offsets from the origin and their states.
    pub blocks: Vec<(BlockPos, BlockState)>,
}

/// Offsets `pos` by `-origin`.
pub const fn relative(pos: BlockPos, origin: BlockPos) -> BlockPos {
    BlockPos {
        x: pos.x - origin.x,
        y: pos.y - origin.y,
        z: pos.z - origin.z,
    }
}

/// Offsets `offset` by `origin`.
pub const fn absolute(offset: BlockPos, origin: BlockPos) -> BlockPos {
    BlockPos {
        x: offset.x + origin.x,
        y: offset.y + origin.y,
        z: offset.z + origin.z,
    }
}

/// Copies the cuboid relative to `origin`.
pub fn copy(
    world: &World<'_>,
    cuboid: &Cuboid,
    origin: BlockPos,
    limit: u64,
) -> Result<Clipboard, String> {
    if cuboid.volume() > limit {
        return Err(format!(
            "the selection has {} blocks, more than the limit of {limit}",
            cuboid.volume()
        ));
    }
    ensure_loaded(world, cuboid.min, cuboid.max)?;
    let (min_y, max_y) = (world.min_y(), world.max_y());
    let mut clipboard = Clipboard::default();
    for pos in positions(cuboid).filter(|pos| (min_y..=max_y).contains(&pos.y)) {
        clipboard
            .blocks
            .push((relative(pos, origin), read(world, pos)?));
    }
    Ok(clipboard)
}

/// Counts blocks of the cuboid accepted by `accept(state, block key)`.
pub fn count(
    world: &World<'_>,
    cuboid: &Cuboid,
    limit: u64,
    accept: impl Fn(BlockState, &str) -> bool,
) -> Result<u64, String> {
    if cuboid.volume() > limit {
        return Err(format!(
            "the selection has {} blocks, more than the limit of {limit}",
            cuboid.volume()
        ));
    }
    ensure_loaded(world, cuboid.min, cuboid.max)?;
    let (min_y, max_y) = (world.min_y(), world.max_y());
    let mut matches = 0;
    for pos in positions(cuboid).filter(|pos| (min_y..=max_y).contains(&pos.y)) {
        let state = read(world, pos)?;
        if accept(state, block_key(state).as_str()) {
            matches += 1;
        }
    }
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cuboid(min: (i32, i32, i32), max: (i32, i32, i32)) -> Cuboid {
        Cuboid {
            world: "w".into(),
            min: BlockPos {
                x: min.0,
                y: min.1,
                z: min.2,
            },
            max: BlockPos {
                x: max.0,
                y: max.1,
                z: max.2,
            },
        }
    }

    #[test]
    fn expand_and_contract_move_the_right_faces() {
        let base = cuboid((0, 0, 0), (9, 9, 9));
        assert_eq!(
            expand(&base, 5, Direction::Up),
            cuboid((0, 0, 0), (9, 14, 9))
        );
        assert_eq!(
            expand(&base, 5, Direction::North),
            cuboid((0, 0, -5), (9, 9, 9))
        );
        assert_eq!(
            contract(&base, 3, Direction::Up),
            Ok(cuboid((0, 3, 0), (9, 9, 9)))
        );
        assert_eq!(
            contract(&base, 3, Direction::West),
            Ok(cuboid((0, 0, 0), (6, 9, 9)))
        );
        assert!(contract(&base, 10, Direction::East).is_err());
    }

    #[test]
    fn positions_and_walls_cover_expected_blocks() {
        let area = cuboid((0, 0, 0), (2, 1, 2));
        assert_eq!(positions(&area).count(), 18);
        // 3x3 footprint minus the center column, two layers.
        assert_eq!(walls(&area).count(), 16);
    }

    #[test]
    fn facing_follows_vanilla_yaw() {
        assert_eq!(Direction::facing(0.0, 0.0), Direction::South);
        assert_eq!(Direction::facing(90.0, 0.0), Direction::West);
        assert_eq!(Direction::facing(180.0, 0.0), Direction::North);
        assert_eq!(Direction::facing(-90.0, 0.0), Direction::East);
        assert_eq!(Direction::facing(10.0, -80.0), Direction::Up);
    }

    #[test]
    fn clipboard_offsets_round_trip() {
        let origin = BlockPos {
            x: 10,
            y: 64,
            z: -5,
        };
        let pos = BlockPos {
            x: 12,
            y: 60,
            z: -9,
        };
        let offset = relative(pos, origin);
        let target = BlockPos {
            x: 100,
            y: 70,
            z: 100,
        };
        assert_eq!(absolute(offset, origin), pos);
        assert_eq!(
            absolute(offset, target),
            BlockPos {
                x: 102,
                y: 66,
                z: 96
            }
        );
    }
}
