use steel_extension_api::{
    BlockPos, BlockState, World,
    content::{ItemAppearance, ItemDisplay, MultiblockDisplay},
    is_air,
    menus::StorageAccess,
    stabby,
};

const MIN_EDGE: i32 = 2;
const MAX_EDGE: i32 = 5;
pub(super) const DISPLAY_RADIUS: u8 = (MAX_EDGE - 1) as u8;
const CHEST_SLOTS_PER_CUBIC_BLOCK: u32 = 27;
pub(super) const MAX_CAPACITY: u32 = MAX_EDGE.unsigned_abs().pow(3) * CHEST_SLOTS_PER_CUBIC_BLOCK;

#[derive(Clone, Copy)]
pub(super) struct Parts {
    pub(super) core: BlockState,
    pub(super) wall: BlockState,
}

pub(super) struct Shell {
    shape: Cube,
    pub(super) core_position: BlockPos,
    parts: Parts,
}

struct Cube {
    min: BlockPos,
    edge: i32,
}

impl Shell {
    pub(super) fn find(
        world: &World<'_>,
        part: BlockPos,
        parts: Parts,
    ) -> Result<Self, &'static str> {
        let mut formed = None;
        for edge in MIN_EDGE..=MAX_EDGE {
            for x in 0..edge {
                for y in 0..edge {
                    for z in 0..edge {
                        if !is_boundary(x, y, z, edge) {
                            continue;
                        }
                        let shape = Cube {
                            edge,
                            min: BlockPos {
                                x: part.x - x,
                                y: part.y - y,
                                z: part.z - z,
                            },
                        };
                        let Some(core_position) = shape.find_core(world, parts) else {
                            continue;
                        };
                        if formed.is_some() {
                            return Err("This block belongs to more than one Bulk Chest");
                        }
                        formed = Some(Self {
                            shape,
                            core_position,
                            parts,
                        });
                    }
                }
            }
        }
        formed.ok_or(
            "Build a hollow 2 to 5 block cube from Bulk Chest Walls with one Bulk Chest Core",
        )
    }

    pub(super) fn slots(&self) -> u32 {
        self.shape.edge.unsigned_abs().pow(3) * CHEST_SLOTS_PER_CUBIC_BLOCK
    }

    pub(super) fn display(&self) -> MultiblockDisplay {
        // The vanilla closed chest body spans 14 model pixels per axis.
        // Enclose the block models with a small gap to keep faces from intersecting.
        const MODEL_WIDTH: f32 = 14.0 / 16.0;
        const CLEARANCE: f32 = 0.01;
        let edge = self.shape.edge as f32;
        let scale = (edge + 2.0 * CLEARANCE) / MODEL_WIDTH;
        let min = self.shape.min;
        let core = self.core_position;
        let model = ItemDisplay {
            appearance: ItemAppearance::new("minecraft:chest"),
            translation: [
                (min.x - core.x) as f32 + edge / 2.0,
                (min.y - core.y) as f32 + scale / 2.0 - CLEARANCE,
                (min.z - core.z) as f32 + edge / 2.0,
            ],
            scale: [scale; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            pack_transform: None.into(),
        };
        MultiblockDisplay {
            model,
            parts: self
                .shape
                .blocks()
                .filter_map(|(pos, boundary)| boundary.then_some(pos))
                .collect(),
        }
    }
}

impl Cube {
    fn blocks(&self) -> impl Iterator<Item = (BlockPos, bool)> + '_ {
        (0..self.edge).flat_map(move |x| {
            (0..self.edge).flat_map(move |y| {
                (0..self.edge).map(move |z| {
                    (
                        BlockPos {
                            x: self.min.x + x,
                            y: self.min.y + y,
                            z: self.min.z + z,
                        },
                        is_boundary(x, y, z, self.edge),
                    )
                })
            })
        })
    }

    fn find_core(&self, world: &World<'_>, parts: Parts) -> Option<BlockPos> {
        let mut core = None;
        for (pos, boundary) in self.blocks() {
            let state = world.block(pos).match_owned(Some, |_| None)?;
            if boundary {
                if state == parts.core && core.is_none() {
                    core = Some(pos);
                } else if state != parts.wall {
                    return None;
                }
            } else if !is_air(state) {
                return None;
            }
        }
        core
    }
}

#[stabby::export(callback)]
impl StorageAccess for Shell {
    fn is_valid(&self, world: World<'_>) -> bool {
        self.shape.find_core(&world, self.parts) == Some(self.core_position)
    }
}

const fn is_boundary(x: i32, y: i32, z: i32, edge: i32) -> bool {
    let end = edge - 1;
    x == 0 || x == end || y == 0 || y == end || z == 0 || z == end
}
