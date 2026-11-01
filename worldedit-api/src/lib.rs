//! WorldEdit selection service, used by WorldGuard to define regions.
#![recursion_limit = "256"]

use steel_extension_api::{
    BlockPos,
    players::Uuid,
    stabby::{self, option::Option as StableOption, string::String as ApiString},
};

/// Service key of the selection contract.
pub const SELECTION: &str = "worldedit:selection/v1";

/// An inclusive axis-aligned box of blocks in one world.
#[stabby::stabby]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cuboid {
    /// World key.
    pub world: ApiString,
    /// Minimum corner (inclusive).
    pub min: BlockPos,
    /// Maximum corner (inclusive).
    pub max: BlockPos,
}

impl Cuboid {
    /// Builds the box spanning two corners in any order.
    #[must_use]
    pub fn from_corners(world: &str, a: BlockPos, b: BlockPos) -> Self {
        Self {
            world: world.into(),
            min: BlockPos {
                x: a.x.min(b.x),
                y: a.y.min(b.y),
                z: a.z.min(b.z),
            },
            max: BlockPos {
                x: a.x.max(b.x),
                y: a.y.max(b.y),
                z: a.z.max(b.z),
            },
        }
    }

    /// Number of blocks.
    #[must_use]
    pub fn volume(&self) -> u64 {
        let side = |min: i32, max: i32| u64::from(max.abs_diff(min)) + 1;
        side(self.min.x, self.max.x) * side(self.min.y, self.max.y) * side(self.min.z, self.max.z)
    }

    /// Whether `pos` lies inside.
    #[must_use]
    pub const fn contains(&self, pos: BlockPos) -> bool {
        pos.x >= self.min.x
            && pos.x <= self.max.x
            && pos.y >= self.min.y
            && pos.y <= self.max.y
            && pos.z >= self.min.z
            && pos.z <= self.max.z
    }
}

/// Player selections owned by WorldEdit.
#[stabby::stabby(checked)]
pub trait Selections: Send + Sync {
    /// The player's complete cuboid selection, if both corners are set in one world.
    extern "C" fn selection(&self, player: Uuid) -> StableOption<Cuboid>;
}

/// Owning, checked selection provider object.
pub type DynSelections = stabby::dynptr!(stabby::boxed::Box<dyn Selections + Send + Sync>);

/// The active selection provider.
///
/// # Errors
/// Returns an error when WorldEdit is not installed or incompatible.
pub fn selections() -> Result<&'static DynSelections, String> {
    steel_extension_api::services::get(SELECTION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_normalize_and_volume_counts_inclusive_blocks() {
        let cuboid = Cuboid::from_corners(
            "minecraft:overworld",
            BlockPos { x: 5, y: -1, z: 0 },
            BlockPos { x: -4, y: 2, z: 0 },
        );
        assert_eq!(cuboid.min, BlockPos { x: -4, y: -1, z: 0 });
        assert_eq!(cuboid.volume(), 10 * 4);
        assert!(cuboid.contains(BlockPos { x: 0, y: 0, z: 0 }));
        assert!(!cuboid.contains(BlockPos { x: 0, y: 3, z: 0 }));
    }
}
