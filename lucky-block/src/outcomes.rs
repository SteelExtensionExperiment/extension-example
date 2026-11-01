//! Example-specific outcomes; these are independent of vanilla loot tables.

use steel_extension_api::{
    BlockPos, BlockState, World,
    block_behavior::{
        BlockBehavior, OriginalBlockBehavior,
        types::{BlockEntityBorrow, ItemStack, ItemStackRef, Player},
    },
    default_block_state,
    stabby::{self, option::Option as StableOption, string::String as ApiString},
};
use steel_utils::{
    locks::SyncMutex,
    random::{Random, xoroshiro::Xoroshiro},
};

const REWARDS: [(&str, i32); 6] = [
    ("minecraft:diamond", 3),
    ("minecraft:emerald", 5),
    ("minecraft:iron_ingot", 8),
    ("minecraft:gold_ingot", 8),
    ("minecraft:bread", 8),
    ("minecraft:coal", 16),
];

pub(crate) struct LuckyBlock {
    original: OriginalBlockBehavior,
    random: SyncMutex<Xoroshiro>,
    air: [BlockState; 3],
    cobweb: BlockState,
    obsidian: BlockState,
}

impl LuckyBlock {
    pub(crate) fn new(original: OriginalBlockBehavior, config: &str) -> Result<Self, ApiString> {
        let settings: toml::Table = config
            .parse()
            .map_err(|error: toml::de::Error| ApiString::from(error.to_string()))?;
        if settings.keys().any(|key| key != "seed") {
            return Err("lucky-block configuration only accepts 'seed'".into());
        }
        let seed = match settings.get("seed") {
            None => rand::random(),
            Some(toml::Value::Integer(seed)) => *seed as u64,
            Some(_) => return Err("lucky-block 'seed' must be an integer".into()),
        };
        // Resolve and validate every reward before installing the behavior.
        for (key, count) in REWARDS {
            ItemStack::from_key(key.into(), count).match_owned(Ok, Err)?;
        }
        let state = |key: &str| default_block_state(key.into()).match_owned(Ok, Err);
        Ok(Self {
            original,
            random: SyncMutex::new(Xoroshiro::from_seed(seed)),
            air: [
                state("minecraft:air")?,
                state("minecraft:cave_air")?,
                state("minecraft:void_air")?,
            ],
            cobweb: state("minecraft:cobweb")?,
            obsidian: state("minecraft:obsidian")?,
        })
    }

    fn activate(&self, world: World<'_>, pos: BlockPos, miner: BlockPos) -> Result<(), ApiString> {
        // Release the RNG lock before calling the host: block updates can invoke callbacks.
        let outcome = self.random.lock().next_i32_bounded(8);
        if let Some(&(key, count)) = REWARDS.get(outcome as usize) {
            let stack = ItemStack::from_key(key.into(), count).match_owned(Ok, Err)?;
            world.pop_resource(pos, stack).match_owned(Ok, Err)?;
            return Ok(());
        }
        let blocks = if outcome == 6 {
            Trap::Snare
        } else {
            Trap::Cage
        }
        .blocks(miner);
        // Check the entire footprint before changing any blocks.
        let mut changes = Vec::new();
        for (pos, material) in blocks {
            let current = world.block(pos).match_owned(Ok, Err)?;
            if self.air.contains(&current) {
                changes.push((
                    pos,
                    match material {
                        Material::Cobweb => self.cobweb,
                        Material::Obsidian => self.obsidian,
                    },
                ));
            }
        }
        for (pos, state) in changes {
            world.set_block(pos, state).match_owned(Ok, Err)?;
        }
        Ok(())
    }
}

#[stabby::export(callback)]
impl BlockBehavior for LuckyBlock {
    fn original(&self) -> OriginalBlockBehavior {
        self.original
    }

    fn player_destroy(
        &self,
        world: World<'_>,
        player: Player<'_>,
        pos: BlockPos,
        _state: BlockState,
        _block_entity: StableOption<BlockEntityBorrow<'_>>,
        _tool: ItemStackRef<'_>,
    ) {
        // The host invokes this after successful survival mining, never for creative breaks.
        let miner = player.block_position();
        if let Err(error) = self.activate(world, pos, miner) {
            eprintln!("Lucky block at {pos:?} could not apply its outcome: {error}");
        }
    }
}

enum Material {
    Cobweb,
    Obsidian,
}

enum Trap {
    Snare,
    Cage,
}

impl Trap {
    fn blocks(&self, center: BlockPos) -> Vec<(BlockPos, Material)> {
        let mut blocks = Vec::new();
        for y in 0..=2 {
            for z in -1..=1 {
                for x in -1..=1 {
                    let material = match self {
                        Self::Snare if y < 2 => Material::Cobweb,
                        Self::Cage if x != 0 || z != 0 || y == 2 => Material::Obsidian,
                        _ => continue,
                    };
                    blocks.push((
                        BlockPos {
                            x: center.x + x,
                            y: center.y + y,
                            z: center.z + z,
                        },
                        material,
                    ));
                }
            }
        }
        blocks
    }
}
