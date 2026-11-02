//! `/toxic paint | pour | drain` over a `WorldEdit` selection or two corners.

use steel_extension_api::{
    BlockPos, Commands,
    command::{Arg, CommandBuilder, Invocation, Node, argument, literal},
    fill::ReplaceFilter,
};
use worldedit_api::SelectionsDyn as _;

use crate::content::BIOME;

const ADMIN: &str = "toxic_wasteland.admin";

/// An inclusive box in one world.
struct Region {
    world: String,
    min: BlockPos,
    max: BlockPos,
}

impl Region {
    fn new(world: String, a: BlockPos, b: BlockPos) -> Self {
        Self {
            world,
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

    fn volume(&self) -> u64 {
        let side = |a: i32, b: i32| u64::from(b.abs_diff(a)) + 1;
        side(self.min.x, self.max.x) * side(self.min.y, self.max.y) * side(self.min.z, self.max.z)
    }

    /// The executor's corners when given, otherwise their `WorldEdit` selection.
    fn resolve(ctx: &Invocation<'_, '_>, corners: bool) -> Result<Self, String> {
        let region = if corners {
            Self::new(
                ctx.world().key().to_string(),
                ctx.block_pos("from")?,
                ctx.block_pos("to")?,
            )
        } else {
            let selections = worldedit_api::selections()
                .map_err(|_| "Make a WorldEdit selection or give two corners.".to_owned())?;
            let player = ctx
                .player()
                .map_err(|_| "Make a WorldEdit selection or give two corners.".to_owned())?;
            let selection: Option<worldedit_api::Cuboid> =
                selections.selection(player.uuid()).into();
            let selection = selection.ok_or("Make a WorldEdit selection or give two corners.")?;
            Self::new(selection.world.to_string(), selection.min, selection.max)
        };
        let limit = crate::config().commands.max_volume;
        let volume = region.volume();
        if volume > limit {
            return Err(format!("Region has {volume} blocks; the limit is {limit}."));
        }
        Ok(region)
    }
}

fn paint(ctx: &Invocation<'_, '_>, corners: bool) -> Result<i32, String> {
    let region = Region::resolve(ctx, corners)?;
    let biome = ctx.string("biome")?;
    let cells = ctx.server().with_world(&region.world, |world| {
        Result::from(world.set_biomes(region.min, region.max, biome.as_str().into()))
            .map_err(|error| error.to_string())
    })??;
    ctx.send_legacy(&format!(
        "&aPainted &e{cells}&a biome cells as &e{biome}&a."
    ));
    Ok(i32::try_from(cells).unwrap_or(i32::MAX))
}

fn pour(ctx: &Invocation<'_, '_>, corners: bool) -> Result<i32, String> {
    let region = Region::resolve(ctx, corners)?;
    let materials = crate::materials();
    let filter = ReplaceFilter {
        states: steel_extension_api::stabby::vec::Vec::new(),
        replaceable: true,
    };
    let changed = ctx.server().with_world(&region.world, |world| {
        let report = Result::from(world.replace_blocks(
            region.min,
            region.max,
            filter.clone(),
            materials.sludge[0],
        ))
        .map_err(|error| error.to_string())?;
        // Bulk writes skip fluid updates; wake the border so the sludge spreads.
        for pos in border(&region) {
            let _ = world.schedule_fluid_tick(pos);
        }
        Ok::<_, String>(report.changed)
    })??;
    ctx.send_legacy(&format!("&aPoured &e{changed}&a sludge blocks."));
    Ok(i32::try_from(changed).unwrap_or(i32::MAX))
}

fn drain(ctx: &Invocation<'_, '_>, corners: bool) -> Result<i32, String> {
    let region = Region::resolve(ctx, corners)?;
    let materials = crate::materials();
    let filter = ReplaceFilter {
        states: materials.sludge.iter().copied().collect(),
        replaceable: false,
    };
    let changed = ctx.server().with_world(&region.world, |world| {
        Result::from(world.replace_blocks(region.min, region.max, filter.clone(), materials.air))
            .map(|report| report.changed)
            .map_err(|error| error.to_string())
    })??;
    ctx.send_legacy(&format!("&aDrained &e{changed}&a sludge blocks."));
    Ok(i32::try_from(changed).unwrap_or(i32::MAX))
}

/// Outer faces of a region, where poured sludge meets its surroundings.
fn border(region: &Region) -> impl Iterator<Item = BlockPos> + '_ {
    let (min, max) = (region.min, region.max);
    (min.x..=max.x).flat_map(move |x| {
        (min.y..=max.y).flat_map(move |y| {
            (min.z..=max.z).filter_map(move |z| {
                let edge = x == min.x || x == max.x || y == min.y || z == min.z || z == max.z;
                edge.then_some(BlockPos { x, y, z })
            })
        })
    })
}

/// `<from> <to>` followed by `tail`, or `tail` alone for the `WorldEdit` selection.
fn with_region(tail: impl Fn(bool) -> Node) -> Vec<Node> {
    vec![
        argument("from", Arg::block_pos()).then(argument("to", Arg::block_pos()).then(tail(true))),
        tail(false),
    ]
}

pub fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    let biome = |corners: bool| {
        argument("biome", Arg::greedy())
            .suggests(|_| vec![BIOME.to_owned(), "minecraft:plains".to_owned()])
            .executes(move |ctx| paint(ctx, corners))
    };
    let mut paint_node = literal("paint");
    for node in with_region(biome) {
        paint_node = paint_node.then(node);
    }
    let pour_node = literal("pour").executes(|ctx| pour(ctx, false)).then(
        argument("from", Arg::block_pos())
            .then(argument("to", Arg::block_pos()).executes(|ctx| pour(ctx, true))),
    );
    let drain_node = literal("drain").executes(|ctx| drain(ctx, false)).then(
        argument("from", Arg::block_pos())
            .then(argument("to", Arg::block_pos()).executes(|ctx| drain(ctx, true))),
    );
    commands.register(
        CommandBuilder::new("toxic-wasteland:toxic")
            .permission(ADMIN)
            .then(paint_node)
            .then(pour_node)
            .then(drain_node),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_volume_counts_inclusive_corners() {
        let region = Region::new(
            String::new(),
            BlockPos { x: 3, y: 0, z: 3 },
            BlockPos { x: 0, y: 1, z: 0 },
        );
        assert_eq!(region.volume(), 4 * 2 * 4);
    }

    #[test]
    fn border_skips_the_inside_and_the_top() {
        let region = Region::new(
            String::new(),
            BlockPos { x: 0, y: 0, z: 0 },
            BlockPos { x: 2, y: 2, z: 2 },
        );
        let border: Vec<_> = border(&region).collect();
        assert!(!border.contains(&BlockPos { x: 1, y: 1, z: 1 }));
        assert!(!border.contains(&BlockPos { x: 1, y: 2, z: 1 }));
        assert_eq!(border.len(), 27 - 1 - 1);
    }
}
