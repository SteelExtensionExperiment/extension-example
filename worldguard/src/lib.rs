//! WorldGuard core features for Steel: cuboid regions defined from WorldEdit selections,
//! owners and members, priorities, parents, the `__global__` region, and the flags `build`,
//! `block-break`, `block-place`, `interact`, `chest-access`, `pvp`, `mob-damage`, `invincible`,
//! `entry`, `exit`, `greeting` and `farewell`. See `region.rs` for the resolution rules.
//!
//! Commands live under `/rg` (aliases `/region`, `/regions`). Changes need
//! `worldguard.region.<subcommand>`; owners may manage their regions' members. Everything is
//! bypassed with `worldguard.region.bypass`.
// TODO: polygon regions, region groups on flags and the rest of WorldGuard's flag list.
#![recursion_limit = "256"]

mod region;

use region::{Action, GLOBAL, Region, WorldRegions, flag_kind, parse_flag_value, valid_id};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
};
use steel_extension_api::{
    BlockPos, Commands, Hooks, PlayerId, ExtensionContext, Server,
    command::{Arg, CommandBuilder, Invocation, argument, literal},
    events::{EventPriority, InteractAction},
    log,
    players::Uuid,
    text,
};
use worldedit_api::SelectionsDyn as _;

/// On-disk form of one world's regions.
#[derive(serde::Serialize, serde::Deserialize)]
struct RegionFile {
    world: String,
    #[serde(default)]
    global: BTreeMap<String, String>,
    #[serde(default)]
    regions: BTreeMap<String, Region>,
}

struct Guard {
    root: PathBuf,
    worlds: Mutex<BTreeMap<String, WorldRegions>>,
    /// Worlds whose region file could not be read; never overwritten.
    locked: BTreeSet<String>,
    containers: BTreeSet<String>,
    interactive: BTreeSet<String>,
}

static GUARD: OnceLock<Guard> = OnceLock::new();

fn guard() -> Result<&'static Guard, String> {
    GUARD
        .get()
        .ok_or_else(|| "WorldGuard is not initialized".to_owned())
}

impl Guard {
    fn worlds(&self) -> MutexGuard<'_, BTreeMap<String, WorldRegions>> {
        self.worlds.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn file(&self, world: &str) -> PathBuf {
        self.root
            .join(format!("{}.toml", world.replace([':', '/'], "_")))
    }

    /// Writes one world's regions atomically.
    fn save(&self, world: &str, regions: &WorldRegions) -> Result<(), String> {
        if self.locked.contains(world) {
            return Err(format!(
                "Regions of {world} were not loaded because their file is invalid; fix it first."
            ));
        }
        let path = self.file(world);
        let file = RegionFile {
            world: world.to_owned(),
            global: regions.global.clone(),
            regions: regions.regions.clone(),
        };
        let text = toml::to_string(&file).map_err(|error| error.to_string())?;
        let temporary = path.with_extension("toml.tmp");
        fs::write(&temporary, text)
            .and_then(|()| fs::rename(&temporary, &path))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    }

    /// Edits one world's regions and saves them.
    fn edit<R>(
        &self,
        world: &str,
        change: impl FnOnce(&mut WorldRegions) -> Result<R, String>,
    ) -> Result<R, String> {
        let mut worlds = self.worlds();
        let regions = worlds.entry(world.to_owned()).or_default();
        let mut updated = regions.clone();
        let result = change(&mut updated)?;
        self.save(world, &updated)?;
        *regions = updated;
        Ok(result)
    }

    fn read<R>(&self, world: &str, query: impl FnOnce(&WorldRegions) -> R) -> R {
        let worlds = self.worlds();
        let empty = WorldRegions::default();
        query(worlds.get(world).unwrap_or(&empty))
    }
}

/// Loads every `*.toml` region file. Unreadable files are reported and their world is
/// locked so the file is never overwritten.
fn load(root: &Path) -> Result<(BTreeMap<String, WorldRegions>, BTreeSet<String>), String> {
    #[derive(serde::Deserialize)]
    struct Header {
        world: String,
    }
    fs::create_dir_all(root)
        .map_err(|error| format!("cannot create {}: {error}", root.display()))?;
    let mut worlds = BTreeMap::new();
    let mut locked = BTreeSet::new();
    let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
        match toml::from_str::<RegionFile>(&text) {
            Ok(file) => {
                worlds.insert(
                    file.world,
                    WorldRegions {
                        global: file.global,
                        regions: file.regions,
                    },
                );
            }
            Err(error) => {
                log::error(format!(
                    "{} is invalid and will not be modified: {error}",
                    path.display()
                ));
                if let Ok(header) = toml::from_str::<Header>(&text) {
                    locked.insert(header.world);
                }
            }
        }
    }
    Ok((worlds, locked))
}

fn tell(server: &Server<'_>, player: PlayerId, message: &str) {
    let _ = server.send_message(player, text::legacy(message).to_json().as_str().into());
}

fn bypass(server: &Server<'_>, player: PlayerId) -> bool {
    server.has_permission(player, "worldguard.region.bypass".into(), false)
}

fn player(ctx: &Invocation<'_, '_>) -> Result<PlayerId, String> {
    ctx.player()
        .map_err(|_| "Only players can use this command.".to_owned())
}

fn world_key(ctx: &Invocation<'_, '_>) -> String {
    ctx.world().key().to_string()
}

fn id_arg(ctx: &Invocation<'_, '_>) -> Result<String, String> {
    let id = ctx.string("id")?;
    if id.eq_ignore_ascii_case(GLOBAL) {
        return Ok(GLOBAL.to_owned());
    }
    valid_id(&id).ok_or_else(|| "Region ids may use a-z, 0-9, _ and -.".to_owned())
}

fn selection(ctx: &Invocation<'_, '_>, player: PlayerId) -> Result<worldedit_api::Cuboid, String> {
    let selections = worldedit_api::selections()?;
    let cuboid: Option<worldedit_api::Cuboid> = selections.selection(player.uuid()).into();
    let cuboid = cuboid.ok_or("Make a WorldEdit selection first (//wand).")?;
    if cuboid.world.as_str() != world_key(ctx) {
        return Err("Your selection is in another world.".into());
    }
    Ok(cuboid)
}

fn define(ctx: &Invocation<'_, '_>, redefine: bool) -> Result<i32, String> {
    let guard = guard()?;
    let player = player(ctx)?;
    let id = id_arg(ctx)?;
    if id == GLOBAL {
        return Err("The global region has no bounds.".into());
    }
    let cuboid = selection(ctx, player)?;
    let name = ctx
        .server()
        .player_name(player)
        .match_owned(|name| name.to_string(), |_| String::new());
    guard.edit(&world_key(ctx), |world| {
        match (world.regions.get_mut(&id), redefine) {
            (Some(_), false) => Err(format!("Region {id} already exists.")),
            (None, true) => Err(format!("Region {id} does not exist.")),
            (Some(region), true) => {
                region.min = [cuboid.min.x, cuboid.min.y, cuboid.min.z];
                region.max = [cuboid.max.x, cuboid.max.y, cuboid.max.z];
                Ok(())
            }
            (None, false) => {
                let mut region = Region::new(cuboid.min, cuboid.max);
                region.owners.insert(player.uuid().to_string(), name);
                world.regions.insert(id.clone(), region);
                Ok(())
            }
        }
    })?;
    let verb = if redefine { "updated" } else { "defined" };
    ctx.send_legacy(&format!(
        "&eRegion &6{id}&e {verb} ({} blocks).",
        cuboid.volume()
    ));
    Ok(1)
}

fn remove(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let guard = guard()?;
    let id = id_arg(ctx)?;
    guard.edit(&world_key(ctx), |world| {
        world
            .regions
            .remove(&id)
            .ok_or_else(|| format!("Region {id} does not exist."))?;
        for region in world.regions.values_mut() {
            if region.parent.as_deref() == Some(id.as_str()) {
                region.parent = None;
            }
        }
        Ok(())
    })?;
    ctx.send_legacy(&format!("&eRegion &6{id}&e removed."));
    Ok(1)
}

fn list(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let guard = guard()?;
    let ids: Vec<String> = guard.read(&world_key(ctx), |world| {
        world.regions.keys().cloned().collect()
    });
    if ids.is_empty() {
        ctx.send_legacy("&eNo regions in this world.");
    } else {
        ctx.send_legacy(&format!("&eRegions: &6{}", ids.join("&e, &6")));
    }
    Ok(1)
}

fn describe(world: &WorldRegions, id: &str) -> Vec<String> {
    let names = |map: &BTreeMap<String, String>| {
        if map.is_empty() {
            "(none)".to_owned()
        } else {
            map.values().cloned().collect::<Vec<_>>().join(", ")
        }
    };
    if id == GLOBAL {
        return vec![format!("&eRegion &6{GLOBAL}&e flags: &f{:?}", world.global)];
    }
    let Some(region) = world.regions.get(id) else {
        return vec![format!("&cRegion {id} does not exist.")];
    };
    vec![
        format!("&eRegion &6{id}&e (priority {})", region.priority),
        format!(
            "&eBounds: &f({}, {}, {}) -> ({}, {}, {})",
            region.min[0],
            region.min[1],
            region.min[2],
            region.max[0],
            region.max[1],
            region.max[2]
        ),
        format!(
            "&eParent: &f{}",
            region.parent.as_deref().unwrap_or("(none)")
        ),
        format!("&eOwners: &f{}", names(&region.owners)),
        format!("&eMembers: &f{}", names(&region.members)),
        format!(
            "&eFlags: &f{}",
            region
                .flags
                .iter()
                .map(|(flag, value)| format!("{flag}={value}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ]
}

fn info(ctx: &Invocation<'_, '_>, explicit: bool) -> Result<i32, String> {
    let guard = guard()?;
    let lines = guard.read(&world_key(ctx), |world| {
        if explicit {
            return id_arg(ctx).map(|id| describe(world, &id));
        }
        let ids = world.at(ctx.position().block());
        if ids.is_empty() {
            return Ok(vec!["&eNo regions here.".to_owned()]);
        }
        Ok(ids.into_iter().flat_map(|id| describe(world, id)).collect())
    })?;
    for line in lines {
        ctx.send_legacy(&line);
    }
    Ok(1)
}

#[derive(Clone, Copy)]
enum Membership {
    AddOwner,
    RemoveOwner,
    AddMember,
    RemoveMember,
}

fn membership(ctx: &Invocation<'_, '_>, change: Membership) -> Result<i32, String> {
    let guard = guard()?;
    let id = id_arg(ctx)?;
    let server = ctx.server();
    let target = ctx.player_arg("player")?;
    let target_name = server
        .player_name(target)
        .match_owned(|name| name.to_string(), |_| String::new());
    let world = world_key(ctx);
    let node = match change {
        Membership::AddOwner => "worldguard.region.addowner",
        Membership::RemoveOwner => "worldguard.region.removeowner",
        Membership::AddMember => "worldguard.region.addmember",
        Membership::RemoveMember => "worldguard.region.removemember",
    };
    let is_owner = ctx
        .player()
        .is_ok_and(|player| guard.read(&world, |regions| regions.is_owner(&id, player.uuid())));
    let owner_may = is_owner && matches!(change, Membership::AddMember | Membership::RemoveMember);
    if !owner_may && !ctx.has_permission(node, false) {
        return Err("You do not have permission to change that region.".into());
    }
    guard.edit(&world, |regions| {
        let region = regions
            .regions
            .get_mut(&id)
            .ok_or_else(|| format!("Region {id} does not exist."))?;
        let key = target.uuid().to_string();
        match change {
            Membership::AddOwner => region.owners.insert(key, target_name.clone()),
            Membership::RemoveOwner => region.owners.remove(&key),
            Membership::AddMember => region.members.insert(key, target_name.clone()),
            Membership::RemoveMember => region.members.remove(&key),
        };
        Ok(())
    })?;
    ctx.send_legacy(&format!("&eRegion &6{id}&e updated for {target_name}."));
    Ok(1)
}

fn flag(ctx: &Invocation<'_, '_>, with_value: bool) -> Result<i32, String> {
    let guard = guard()?;
    let id = id_arg(ctx)?;
    let flag = ctx.string("flag")?.to_lowercase();
    flag_kind(&flag).ok_or_else(|| format!("Unknown flag {flag}."))?;
    let value = if with_value {
        Some(parse_flag_value(&flag, &ctx.string("value")?)?)
    } else {
        None
    };
    guard.edit(&world_key(ctx), |world| {
        let flags = if id == GLOBAL {
            &mut world.global
        } else {
            &mut world
                .regions
                .get_mut(&id)
                .ok_or_else(|| format!("Region {id} does not exist."))?
                .flags
        };
        match &value {
            Some(value) => flags.insert(flag.clone(), value.clone()),
            None => flags.remove(&flag),
        };
        Ok(())
    })?;
    match value {
        Some(value) => ctx.send_legacy(&format!("&eFlag &6{flag}&e of &6{id}&e set to &f{value}")),
        None => ctx.send_legacy(&format!("&eFlag &6{flag}&e of &6{id}&e removed.")),
    }
    Ok(1)
}

fn set_priority(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let guard = guard()?;
    let id = id_arg(ctx)?;
    let priority = ctx.integer("priority")?;
    guard.edit(&world_key(ctx), |world| {
        world
            .regions
            .get_mut(&id)
            .map(|region| region.priority = priority)
            .ok_or_else(|| format!("Region {id} does not exist."))
    })?;
    ctx.send_legacy(&format!("&ePriority of &6{id}&e set to {priority}."));
    Ok(1)
}

fn set_parent(ctx: &Invocation<'_, '_>, with_parent: bool) -> Result<i32, String> {
    let guard = guard()?;
    let id = id_arg(ctx)?;
    let parent = if with_parent {
        Some(valid_id(&ctx.string("parent")?).ok_or("Invalid parent id.")?)
    } else {
        None
    };
    guard.edit(&world_key(ctx), |world| {
        world.set_parent(&id, parent.as_deref())
    })?;
    ctx.send_legacy(&format!(
        "&eParent of &6{id}&e set to {}.",
        parent.as_deref().unwrap_or("(none)")
    ));
    Ok(1)
}

fn region_ids() -> Vec<String> {
    guard().map_or_else(
        |_| Vec::new(),
        |guard| {
            guard
                .worlds()
                .values()
                .flat_map(|world| world.regions.keys().cloned())
                .chain(std::iter::once(GLOBAL.to_owned()))
                .collect()
        },
    )
}

fn id_node() -> steel_extension_api::command::Node {
    argument("id", Arg::word()).suggests(|_| region_ids())
}

fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    let membership_node =
        |name: &'static str, change: Membership| {
            literal(name).then(id_node().then(
                argument("player", Arg::player()).executes(move |ctx| membership(ctx, change)),
            ))
        };
    commands.register(
        CommandBuilder::new("worldguard:rg")
            .alias("region")
            .alias("regions")
            .default_access()
            .then(
                literal("define")
                    .permission("worldguard.region.define")
                    .then(id_node().executes(|ctx| define(ctx, false))),
            )
            .then(
                literal("redefine")
                    .permission("worldguard.region.redefine")
                    .then(id_node().executes(|ctx| define(ctx, true))),
            )
            .then(
                literal("remove")
                    .permission("worldguard.region.remove")
                    .then(id_node().executes(remove)),
            )
            .then(literal("list").executes(list))
            .then(
                literal("info")
                    .executes(|ctx| info(ctx, false))
                    .then(id_node().executes(|ctx| info(ctx, true))),
            )
            .then(membership_node("addowner", Membership::AddOwner))
            .then(membership_node("removeowner", Membership::RemoveOwner))
            .then(membership_node("addmember", Membership::AddMember))
            .then(membership_node("removemember", Membership::RemoveMember))
            .then(
                literal("flag").permission("worldguard.region.flag").then(
                    id_node().then(
                        argument("flag", Arg::word())
                            .suggests(|_| {
                                region::FLAGS
                                    .iter()
                                    .map(|(name, _)| (*name).to_owned())
                                    .collect()
                            })
                            .executes(|ctx| flag(ctx, false))
                            .then(argument("value", Arg::greedy()).executes(|ctx| flag(ctx, true))),
                    ),
                ),
            )
            .then(
                literal("setpriority")
                    .permission("worldguard.region.setpriority")
                    .then(
                        id_node().then(
                            argument("priority", Arg::integer(i32::MIN, i32::MAX))
                                .executes(set_priority),
                        ),
                    ),
            )
            .then(
                literal("setparent")
                    .permission("worldguard.region.setparent")
                    .then(
                        id_node().executes(|ctx| set_parent(ctx, false)).then(
                            argument("parent", Arg::word())
                                .suggests(|_| region_ids())
                                .executes(|ctx| set_parent(ctx, true)),
                        ),
                    ),
            ),
    )
}

/// Checks a build-type action and tells the player when it is denied.
fn deny_action(
    server: &Server<'_>,
    player: PlayerId,
    world: &str,
    pos: BlockPos,
    action: Action,
) -> bool {
    let Ok(guard) = guard() else {
        return false;
    };
    if guard.read(world, |regions| regions.allows(pos, player.uuid(), action))
        || bypass(server, player)
    {
        return false;
    }
    let what = match action {
        Action::Break => "break that block",
        Action::Place => "place that block",
        Action::Interact => "use that",
        Action::ChestAccess => "open that",
    };
    tell(
        server,
        player,
        &format!("&c&lHey!&7 Sorry, but you can't {what} here."),
    );
    true
}

fn block_kind(guard: &Guard, key: &str) -> Option<Action> {
    if guard.containers.contains(key) {
        Some(Action::ChestAccess)
    } else if guard.interactive.contains(key) {
        Some(Action::Interact)
    } else {
        None
    }
}

fn subscribe(hooks: &mut Hooks<'_>) {
    hooks.on_block_break_with(EventPriority::Normal, true, |event| {
        let world = event.world().key().to_string();
        if deny_action(
            &event.server(),
            event.player(),
            &world,
            event.pos(),
            Action::Break,
        ) {
            event.set_cancelled(true);
        }
    });
    hooks.on_block_place_with(EventPriority::Normal, true, |event| {
        let world = event.world().key().to_string();
        if deny_action(
            &event.server(),
            event.player(),
            &world,
            event.pos(),
            Action::Place,
        ) {
            event.set_cancelled(true);
        }
    });
    hooks.on_interact_with(EventPriority::Normal, true, |event| {
        if event.action() != InteractAction::RightClickBlock {
            return;
        }
        let (Ok(guard), Some(pos)) = (guard(), Option::<BlockPos>::from(event.pos())) else {
            return;
        };
        let world = event.world();
        let Ok(state) = world.block(pos).match_owned(Ok, Err) else {
            return;
        };
        let Some(action) = block_kind(guard, steel_extension_api::block_key(state).as_str()) else {
            return;
        };
        let key = world.key().to_string();
        if deny_action(&event.server(), event.player(), &key, pos, action) {
            event.set_cancelled(true);
        }
    });
    hooks.on_damage_with(EventPriority::Normal, true, |event| {
        let Ok(guard) = guard() else {
            return;
        };
        let victim = event.victim();
        if Option::<PlayerId>::from(victim.player).is_none() {
            return;
        }
        let world = event.world().to_string();
        let pos = victim.position.block();
        let attacker: Option<steel_extension_api::events::EntityInfo> = event.attacker().into();
        let attacker_is_player =
            attacker.map(|info| Option::<PlayerId>::from(info.player).is_some());
        let cancel = guard.read(&world, |regions| {
            if regions.state(pos, "invincible", false) {
                return true;
            }
            match attacker_is_player {
                Some(true) => !regions.state(pos, "pvp", true),
                Some(false) => !regions.state(pos, "mob-damage", true),
                None => false,
            }
        });
        if cancel {
            event.set_cancelled(true);
        }
    });
    hooks.on_move_with(EventPriority::Normal, true, |event| {
        let Ok(guard) = guard() else {
            return;
        };
        let server = event.server();
        let player = event.player();
        let world = event.world().to_string();
        let (from, to) = (event.from().block(), event.to().block());
        let (blocked, greetings) = guard.read(&world, |regions| {
            let before: BTreeSet<&str> = regions.at(from).into_iter().collect();
            let after: BTreeSet<&str> = regions.at(to).into_iter().collect();
            let entered: Vec<&str> = after.difference(&before).copied().collect();
            let exited: Vec<&str> = before.difference(&after).copied().collect();
            let uuid: Uuid = player.uuid();
            if let Some(id) = entered
                .iter()
                .find(|id| !regions.may_cross(id, uuid, "entry"))
            {
                return (
                    Some(format!("&c&lHey!&7 You are not permitted to enter {id}.")),
                    Vec::new(),
                );
            }
            if let Some(id) = exited
                .iter()
                .find(|id| !regions.may_cross(id, uuid, "exit"))
            {
                return (
                    Some(format!("&c&lHey!&7 You are not permitted to leave {id}.")),
                    Vec::new(),
                );
            }
            let messages = entered
                .iter()
                .filter_map(|id| regions.effective_flag(id, "greeting"))
                .chain(
                    exited
                        .iter()
                        .filter_map(|id| regions.effective_flag(id, "farewell")),
                )
                .map(str::to_owned)
                .collect();
            (None, messages)
        });
        if let Some(message) = blocked
            && !bypass(&server, player)
        {
            event.set_cancelled(true);
            tell(&server, player, &message);
            return;
        }
        for message in greetings {
            tell(&server, player, &message);
        }
    });
}

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.on_commands(register)?;
    subscribe(&mut hooks);
    Ok(())
}

/// Collects every block in the given tags plus explicit keys.
fn block_set(tags: &[&str], extra: &[&str]) -> BTreeSet<String> {
    let mut set: BTreeSet<String> = extra.iter().map(|key| format!("minecraft:{key}")).collect();
    for tag in tags {
        match steel_extension_api::blocks_in_tag((*tag).into())
            .match_owned(Ok, |error| Err(error.to_string()))
        {
            Ok(blocks) => set.extend(blocks.iter().map(ToString::to_string)),
            Err(error) => log::warn(format!("block tag {tag}: {error}")),
        }
    }
    set
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let root = PathBuf::from(context.data_directory().as_str()).join("regions");
    let (worlds, locked) = load(&root)?;
    let count: usize = worlds.values().map(|world| world.regions.len()).sum();
    let containers = block_set(
        &["minecraft:shulker_boxes"],
        &[
            "chest",
            "trapped_chest",
            "barrel",
            "furnace",
            "blast_furnace",
            "smoker",
            "hopper",
            "dispenser",
            "dropper",
            "brewing_stand",
            "crafter",
            "chiseled_bookshelf",
            "decorated_pot",
        ],
    );
    let interactive = block_set(
        &[
            "minecraft:doors",
            "minecraft:trapdoors",
            "minecraft:fence_gates",
            "minecraft:buttons",
            "minecraft:beds",
            "minecraft:anvil",
        ],
        &[
            "lever",
            "repeater",
            "comparator",
            "note_block",
            "daylight_detector",
            "jukebox",
            "cake",
            "bell",
            "crafting_table",
            "enchanting_table",
            "loom",
            "stonecutter",
            "grindstone",
            "cartography_table",
            "smithing_table",
            "lectern",
            "beacon",
            "respawn_anchor",
            "ender_chest",
        ],
    );
    GUARD
        .set(Guard {
            root,
            worlds: Mutex::new(worlds),
            locked,
            containers,
            interactive,
        })
        .map_err(|_| "WorldGuard initialized twice".to_owned())?;
    log::info(format!("Loaded {count} regions"));
    Ok(())
}

steel_extension_api::export_extension! {
    name: "worldguard",
    version: "0.1.0",
    depends: ["worldedit"],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
