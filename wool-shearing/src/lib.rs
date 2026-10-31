//! Replaces wool interactions while delegating every other block behavior.
#![recursion_limit = "512"]

use steel_extension_api::{
    ApiResult, BlockPos, BlockState, Hooks, Registry, World,
    block_behavior::{
        BlockBehavior, InteractionResult, OriginalBlockBehavior,
        types::{BlockHit, InteractionHand, Inventory, ItemStack, Player},
    },
    blocks_in_tag, default_block_state,
    stabby::{self, str::Str, string::String as ApiString},
};

fn pre_init(mut hooks: Hooks<'_>) -> ApiResult<()> {
    hooks.register_block_registration(register_blocks)
}

#[stabby::export(callback)]
fn register_blocks(mut registry: Registry<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        let wood = default_block_state("minecraft:oak_wood".into()).match_owned(Ok, Err)?;
        for block in blocks_in_tag("minecraft:wool".into())
            .match_owned(Ok, Err)?
            .iter()
        {
            let original = OriginalBlockBehavior::get(&registry, block.as_str().into())
                .match_owned(Ok, Err)?;
            registry
                .replace_block_behavior(
                    block.as_str().into(),
                    stabby::boxed::Box::new(ShearedWool { original, wood }).into(),
                )
                .match_owned(Ok, Err)?;
        }
        Ok(())
    })();
    result.into()
}

// All state is resolved during block registration; no runtime initialization is needed.
fn init(_config: Str<'_>) -> ApiResult<()> {
    Ok(()).into()
}

struct ShearedWool {
    original: OriginalBlockBehavior,
    wood: BlockState,
}

#[stabby::export(callback)]
impl BlockBehavior for ShearedWool {
    fn original(&self) -> OriginalBlockBehavior {
        self.original
    }

    fn use_item_on(
        &self,
        state: BlockState,
        world: World<'_>,
        pos: BlockPos,
        player: Player<'_>,
        hand: InteractionHand,
        hit_result: BlockHit<'_>,
        mut inv: Inventory<'_>,
    ) -> InteractionResult {
        if !inv.held_item_is("minecraft:shears".into()) {
            return self
                .original
                .use_item_on(state, world, pos, player, hand, hit_result, inv);
        }
        let wool = match ItemStack::from_block(state).match_owned(Ok, Err) {
            Ok(wool) => wool,
            Err(error) => {
                eprintln!("Wool interaction could not create its drop: {error}");
                return self
                    .original
                    .use_item_on(state, world, pos, player, hand, hit_result, inv);
            }
        };
        if let Err(error) = world.set_block(pos, self.wood).match_owned(Ok, Err) {
            eprintln!("Wool interaction could not replace the block: {error}");
            return self
                .original
                .use_item_on(state, world, pos, player, hand, hit_result, inv);
        }
        if let Err(error) = inv
            .damage_item(1, player.has_infinite_materials())
            .match_owned(Ok, Err)
        {
            panic!("valid shears damage failed after replacing wool: {error}");
        }
        if let Err(error) = world.pop_resource(pos, wool).match_owned(Ok, Err) {
            eprintln!("Wool interaction could not drop its item: {error}");
        }
        InteractionResult::Success
    }
}

steel_extension_api::export_extension!("wool-shearing-example", pre_init, init);
