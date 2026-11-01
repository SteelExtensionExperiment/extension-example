//! OP Prison pickaxe with custom enchants: Jackhammer (whole mine layer), Explosive (sphere),
//! Nuke (whole mine), Soundwave (an expanding shell of note particles that shatters the mine
//! tick by tick), Fortune (sell multiplier), Haste and Speed (effects while held).
//!
//! Everything mined with the pickaxe inside a `prison-mines` mine is sold straight into the
//! Vault economy: block drops are turned off and enchant procs sell the fill histogram. A
//! sidebar shows balance, stats and the current mine's countdown. Upgrades are bought in a
//! chest menu (`/enchants` or right-click with the pickaxe).
#![recursion_limit = "256"]

mod enchants;

use enchants::{
    Config, Enchant, Levels, PICKAXE_KEY, explosive_radius, money, roman_or_number,
    soundwave_radius,
};
use prison_api::{MineInfo, MinesDyn as _};
use std::{
    collections::BTreeMap,
    f64::consts::PI,
    sync::{
        Mutex, MutexGuard, OnceLock, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};
use steel_extension_api::{
    BlockPos, BlockState, Commands, Hooks, PlayerId, ExtensionContext, Position, Server, World,
    command::{Arg, CommandBuilder, Invocation, argument},
    effects::{Effect, Particle, SoundCategory},
    events::{ClickKind, EventPriority, InteractAction},
    fill::{FillMode, FillReport, FillRequest, PatternEntry},
    items::{Glint, ItemSpec},
    log,
    menus::{self, MenuClick, MenuHandler},
    scheduler, text,
};
use vault_api::EconomyDyn as _;

const ADMIN: &str = "prison.enchants.admin";
/// Vanilla Efficiency level on the pickaxe: high enough to break ores instantly even while
/// jumping or sprinting, where vanilla divides mining speed by five.
const EFFICIENCY: u32 = 100;
const MENU_ROWS: i32 = 3;
/// Menu slots of the enchant icons, in [`Enchant::ALL`] order.
const ICON_SLOTS: [i32; 7] = [10, 11, 12, 13, 14, 15, 16];
const INFO_SLOT: i32 = 22;

static CONFIG: OnceLock<Config> = OnceLock::new();
static MENU: OnceLock<MenuHandler> = OnceLock::new();
static STATS: Mutex<BTreeMap<[u8; 16], Stats>> = Mutex::new(BTreeMap::new());
static RANDOM: AtomicU64 = AtomicU64::new(0x2545_F491_4F6C_DD1D);

#[derive(Debug, Default, Clone, Copy)]
struct Stats {
    blocks: u64,
    earned: f64,
}

fn config() -> &'static Config {
    CONFIG.get_or_init(Config::default)
}

fn stats() -> MutexGuard<'static, BTreeMap<[u8; 16], Stats>> {
    STATS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Uniform draw in `0..1` (SplitMix64); procs need no vanilla determinism.
fn roll() -> f64 {
    let mut z = RANDOM
        .fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed)
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    #[expect(
        clippy::cast_precision_loss,
        reason = "53 random bits are exactly representable"
    )]
    let unit = (z >> 11) as f64 / (1_u64 << 53) as f64;
    unit
}

fn legacy(text: &str) -> String {
    text::legacy(text).to_json()
}

fn tell(server: &Server<'_>, player: PlayerId, message: &str) {
    let _ = server.send_message(player, legacy(message).as_str().into());
}

fn action_bar(server: &Server<'_>, player: PlayerId, message: &str) {
    let _ = server.send_action_bar(player, legacy(message).as_str().into());
}

const fn center(pos: BlockPos) -> Position {
    Position {
        x: pos.x as f64 + 0.5,
        y: pos.y as f64 + 0.5,
        z: pos.z as f64 + 0.5,
    }
}

// --- Pickaxe -------------------------------------------------------------------------------

/// The pickaxe item for `levels`, keeping the base item's other data.
fn pickaxe(levels: &Levels) -> ItemSpec {
    let lore: Vec<String> = levels.lore();
    let mut spec = ItemSpec::new("minecraft:diamond_pickaxe")
        .name_legacy("&b&lPrison Pickaxe")
        .lore_legacy(lore.iter().map(String::as_str))
        .glint(Glint::On)
        .unbreakable(true)
        .enchant("minecraft:efficiency", EFFICIENCY)
        .with_data(PICKAXE_KEY, "1");
    for enchant in Enchant::ALL {
        let level = levels.get(enchant);
        if level > 0 {
            spec = spec.with_data(&enchant.data_key(), &level.to_string());
        }
    }
    spec
}

/// The held pickaxe: hotbar slot and enchant levels.
fn held_pickaxe(server: &Server<'_>, player: PlayerId) -> Option<(i32, Levels)> {
    let slot = Result::from(server.held_slot(player)).ok()?;
    let item: ItemSpec = Result::from(server.slot(player, slot)).ok()?;
    item.data(PICKAXE_KEY)?;
    Some((slot, Levels::from_data(|key| item.data(key))))
}

fn store_pickaxe(
    server: &Server<'_>,
    player: PlayerId,
    slot: i32,
    levels: &Levels,
) -> Result<(), String> {
    Result::from(server.set_slot(player, slot, pickaxe(levels))).map_err(|error| error.to_string())
}

// --- Selling -------------------------------------------------------------------------------

/// Pays a player for blocks and updates their stats; returns the amount paid.
fn sell(player: PlayerId, counts: &[(String, u64)], fortune: u32) -> f64 {
    let total = config().sell_total(
        counts.iter().map(|(key, count)| (key.as_str(), *count)),
        fortune,
    );
    let blocks: u64 = counts.iter().map(|(_, count)| count).sum();
    {
        let mut stats = stats();
        let entry = stats.entry(player.uuid().bytes).or_default();
        entry.blocks += blocks;
        entry.earned += total;
    }
    if total > 0.0
        && let Ok(economy) = vault_api::economy()
    {
        let _ = economy.deposit(player.uuid(), total);
    }
    total
}

/// Block keys and counts of a fill histogram.
fn histogram(report: &FillReport) -> Vec<(String, u64)> {
    report
        .replaced
        .iter()
        .map(|entry| {
            (
                steel_extension_api::block_key(entry.state).to_string(),
                entry.count,
            )
        })
        .collect()
}

fn air() -> Result<BlockState, String> {
    Result::from(steel_extension_api::default_block_state(
        "minecraft:air".into(),
    ))
    .map_err(|error| error.to_string())
}

/// Clears blocks with a fill, sells them and reports them to the mine.
fn clear(
    world: &World<'_>,
    player: PlayerId,
    mine: &MineInfo,
    request: FillRequest,
    fortune: u32,
) -> Result<(u64, f64), String> {
    let report = Result::from(world.fill(request.clip(mine.min, mine.max).mode(FillMode::NonAir)))
        .map_err(|error| error.to_string())?;
    if report.changed == 0 {
        return Ok((0, 0.0));
    }
    let paid = sell(player, &histogram(&report), fortune);
    if let Ok(mines) = prison_api::mines() {
        mines.add_mined(mine.name.as_str().into(), report.changed);
    }
    Ok((report.changed, paid))
}

// --- Effects -------------------------------------------------------------------------------

fn particle(world: &World<'_>, particle: Particle) {
    let _ = world.spawn_particle(particle.force(true));
}

fn sound(world: &World<'_>, at: Position, key: &str, volume: f32, pitch: f32) {
    let _ = world.play_sound(key.into(), at, SoundCategory::Players, volume, pitch);
}

/// Dust along the four edges of a horizontal rectangle at height `y`.
fn outline(world: &World<'_>, min: BlockPos, max: BlockPos, y: f64, rgb: u32) {
    let (x0, x1) = (f64::from(min.x), f64::from(max.x) + 1.0);
    let (z0, z1) = (f64::from(min.z), f64::from(max.z) + 1.0);
    let step = ((x1 - x0).max(z1 - z0) / 40.0).max(1.0);
    let mut t = 0.0;
    while t <= (x1 - x0).max(z1 - z0) {
        for (x, z) in [(x0 + t, z0), (x0 + t, z1), (x0, z0 + t), (x1, z0 + t)] {
            if x <= x1 && z <= z1 {
                particle(
                    world,
                    Particle::new("minecraft:dust", Position { x, y, z }, 2).dust(rgb, 2.0),
                );
            }
        }
        t += step;
    }
}

fn jackhammer(
    server: &Server<'_>,
    world: &World<'_>,
    player: PlayerId,
    mine: &MineInfo,
    y: i32,
    fortune: u32,
) -> Result<(), String> {
    let air = air()?;
    let request = FillRequest::layer(
        mine.min,
        mine.max,
        y,
        &[PatternEntry {
            state: air,
            weight: 1,
        }],
    );
    let (count, paid) = clear(world, player, mine, request, fortune)?;
    if count == 0 {
        return Ok(());
    }
    let layer_y = f64::from(y) + 0.5;
    let middle = Position {
        x: (f64::from(mine.min.x) + f64::from(mine.max.x) + 1.0) / 2.0,
        y: layer_y,
        z: (f64::from(mine.min.z) + f64::from(mine.max.z) + 1.0) / 2.0,
    };
    let half_x = f64::midpoint(f64::from(mine.max.x - mine.min.x), 1.0);
    let half_z = f64::midpoint(f64::from(mine.max.z - mine.min.z), 1.0);
    outline(world, mine.min, mine.max, layer_y, 0xFF_AA00);
    particle(
        world,
        Particle::new("minecraft:explosion", middle, 12).spread(half_x / 2.0, 0.2, half_z / 2.0),
    );
    particle(
        world,
        Particle::new("minecraft:firework", middle, 160)
            .spread(half_x / 2.0, 0.3, half_z / 2.0)
            .speed(0.15),
    );
    particle(
        world,
        Particle::new("minecraft:lava", middle, 40).spread(half_x / 2.0, 0.1, half_z / 2.0),
    );
    particle(
        world,
        Particle::new("minecraft:block", middle, 300)
            .block("minecraft:stone")
            .spread(half_x / 2.0, 0.2, half_z / 2.0),
    );
    sound(world, middle, "minecraft:entity.generic.explode", 2.0, 0.7);
    sound(world, middle, "minecraft:block.anvil.land", 1.0, 0.5);
    action_bar(
        server,
        player,
        &format!("&6&lJACKHAMMER! &e{count} blocks &a+{}", money(paid)),
    );
    Ok(())
}

fn explosive(
    server: &Server<'_>,
    world: &World<'_>,
    player: PlayerId,
    mine: &MineInfo,
    pos: BlockPos,
    level: u32,
    fortune: u32,
) -> Result<(), String> {
    let air = air()?;
    let radius = explosive_radius(level);
    let request = FillRequest::sphere(
        pos,
        radius,
        &[PatternEntry {
            state: air,
            weight: 1,
        }],
    );
    let (count, paid) = clear(world, player, mine, request, fortune)?;
    if count == 0 {
        return Ok(());
    }
    let at = center(pos);
    particle(world, Particle::new("minecraft:explosion_emitter", at, 1));
    particle(
        world,
        Particle::new("minecraft:flame", at, 120)
            .spread(radius / 2.0, radius / 2.0, radius / 2.0)
            .speed(0.1),
    );
    particle(
        world,
        Particle::new("minecraft:large_smoke", at, 60)
            .spread(radius / 2.0, radius / 2.0, radius / 2.0)
            .speed(0.05),
    );
    sound(world, at, "minecraft:entity.generic.explode", 2.0, 1.0);
    action_bar(
        server,
        player,
        &format!("&c&lEXPLOSIVE! &e{count} blocks &a+{}", money(paid)),
    );
    Ok(())
}

fn nuke(
    server: &Server<'_>,
    world: &World<'_>,
    player: PlayerId,
    mine: &MineInfo,
    fortune: u32,
) -> Result<(), String> {
    let air = air()?;
    let request = FillRequest::cuboid(
        mine.min,
        mine.max,
        &[PatternEntry {
            state: air,
            weight: 1,
        }],
    );
    let (count, paid) = clear(world, player, mine, request, fortune)?;
    let (min, max) = (mine.min, mine.max);
    let top = f64::from(max.y) + 1.0;
    let middle = Position {
        x: (f64::from(min.x) + f64::from(max.x) + 1.0) / 2.0,
        y: (f64::from(min.y) + f64::from(max.y) + 1.0) / 2.0,
        z: (f64::from(min.z) + f64::from(max.z) + 1.0) / 2.0,
    };
    for (x, z) in [
        (f64::from(min.x), f64::from(min.z)),
        (f64::from(max.x) + 1.0, f64::from(min.z)),
        (f64::from(min.x), f64::from(max.z) + 1.0),
        (f64::from(max.x) + 1.0, f64::from(max.z) + 1.0),
        (middle.x, middle.z),
    ] {
        let _ = world.strike_lightning_effect(Position { x, y: top, z });
    }
    let spread = (
        f64::from(max.x - min.x) / 3.0,
        f64::from(max.y - min.y) / 3.0,
        f64::from(max.z - min.z) / 3.0,
    );
    particle(
        world,
        Particle::new("minecraft:explosion_emitter", middle, 8)
            .spread(spread.0, spread.1, spread.2),
    );
    particle(world, Particle::new("minecraft:flash", middle, 1));
    particle(
        world,
        Particle::new("minecraft:soul_fire_flame", middle, 400)
            .spread(spread.0, spread.1, spread.2)
            .speed(0.2),
    );
    sound(world, middle, "minecraft:entity.wither.spawn", 4.0, 0.6);
    for target in server
        .players()
        .iter()
        .filter(|target| target.world.as_str() == mine.world.as_str())
    {
        let _ = server.send_title(
            target.id,
            legacy("&4&l☢ NUKE ☢").as_str().into(),
            legacy(&format!("&c{count} blocks vaporized"))
                .as_str()
                .into(),
            5,
            40,
            15,
        );
    }
    action_bar(
        server,
        player,
        &format!("&4&lNUKE! &e{count} blocks &a+{}", money(paid)),
    );
    // The empty mine refills shortly after, whatever its own timer says.
    let name = mine.name.to_string();
    scheduler::run_later(60, move |server| {
        if let Ok(mines) = prison_api::mines() {
            let _ = mines.reset(*server, name.as_str().into());
        }
    });
    Ok(())
}

/// Points spread evenly over a sphere (golden spiral).
fn sphere_points(center: Position, radius: f64, count: u32) -> impl Iterator<Item = Position> {
    let golden = PI * (3.0 - 5.0_f64.sqrt());
    (0..count).map(move |index| {
        let t = f64::from(index);
        let y = 1.0 - 2.0 * (t + 0.5) / f64::from(count);
        let ring = (1.0 - y * y).sqrt();
        let angle = golden * t;
        Position {
            x: center.x + radius * ring * angle.cos(),
            y: center.y + radius * y,
            z: center.z + radius * ring * angle.sin(),
        }
    })
}

/// One ring of the Soundwave; schedules the next until `last`.
fn soundwave_step(
    world_key: String,
    player: PlayerId,
    mine: MineInfo,
    origin: BlockPos,
    radius: u32,
    last: u32,
    fortune: u32,
) {
    scheduler::run_later(2, move |server| {
        let result = server.with_world(&world_key, |world| -> Result<(u64, f64), String> {
            let air = air()?;
            let request = FillRequest::shell(
                origin,
                f64::from(radius),
                1.0,
                &[PatternEntry {
                    state: air,
                    weight: 1,
                }],
            );
            let cleared = clear(&world, player, &mine, request, fortune)?;
            let at = center(origin);
            let points = (radius * radius * 6).clamp(12, 90);
            for (index, point) in sphere_points(at, f64::from(radius), points).enumerate() {
                let inside = point.x >= f64::from(mine.min.x)
                    && point.x <= f64::from(mine.max.x) + 1.0
                    && point.y >= f64::from(mine.min.y)
                    && point.y <= f64::from(mine.max.y) + 1.0
                    && point.z >= f64::from(mine.min.z)
                    && point.z <= f64::from(mine.max.z) + 1.0;
                if inside {
                    let pitch = u8::try_from((radius as usize * 3 + index) % 25).unwrap_or(0);
                    particle(&world, Particle::note(point, pitch));
                }
            }
            if radius == 1 {
                particle(&world, Particle::new("minecraft:sonic_boom", at, 1));
                sound(&world, at, "minecraft:entity.warden.sonic_boom", 1.5, 1.2);
            }
            #[expect(
                clippy::cast_precision_loss,
                reason = "radius is at most a few dozen blocks"
            )]
            let rise = radius as f32 / last.max(1) as f32;
            sound(
                &world,
                at,
                "minecraft:block.note_block.bell",
                1.5,
                0.5 + rise * 1.5,
            );
            sound(
                &world,
                at,
                "minecraft:block.note_block.harp",
                1.0,
                0.6 + rise * 1.4,
            );
            Ok(cleared)
        });
        match result {
            Ok(Ok((count, paid))) if count > 0 => {
                action_bar(
                    server,
                    player,
                    &format!("&d&lSOUNDWAVE &7ring {radius}/{last} &a+{}", money(paid)),
                );
            }
            Ok(Err(error)) | Err(error) => {
                log::debug(format!("soundwave stopped: {error}"));
                return;
            }
            Ok(Ok(_)) => {}
        }
        if radius < last {
            soundwave_step(world_key, player, mine, origin, radius + 1, last, fortune);
        }
    });
}

/// A small sparkle on every block broken with the pickaxe.
fn break_sparkle(world: &World<'_>, pos: BlockPos) {
    let at = center(pos);
    let _ = world.spawn_particle(
        Particle::new("minecraft:crit", at, 6)
            .spread(0.3, 0.3, 0.3)
            .speed(0.2),
    );
    let _ = world.spawn_particle(Particle::new("minecraft:wax_on", at, 3).spread(0.3, 0.3, 0.3));
}

fn on_break(
    server: &Server<'_>,
    world: &World<'_>,
    player: PlayerId,
    pos: BlockPos,
    state: BlockState,
) -> bool {
    let Some((_, levels)) = held_pickaxe(server, player) else {
        return false;
    };
    let Ok(mines) = prison_api::mines() else {
        return false;
    };
    let world_key = world.key().to_string();
    let Some(mine): Option<MineInfo> = mines.mine_at(world_key.as_str().into(), pos).into() else {
        return false;
    };
    let fortune = levels.get(Enchant::Fortune);
    let key = steel_extension_api::block_key(state).to_string();
    sell(player, &[(key, 1)], fortune);
    break_sparkle(world, pos);
    let tuning = config();
    for enchant in Enchant::ALL.into_iter().filter(|enchant| enchant.procs()) {
        let level = levels.get(enchant);
        if level == 0 || roll() >= tuning.enchant(enchant).chance(level) {
            continue;
        }
        let (world_key, mine) = (world_key.clone(), mine.clone());
        if enchant == Enchant::Soundwave {
            soundwave_step(
                world_key,
                player,
                mine,
                pos,
                1,
                soundwave_radius(level),
                fortune,
            );
            continue;
        }
        // One tick later, so the break that triggered the proc has finished.
        scheduler::run_later(1, move |server| {
            let result = server.with_world(&world_key, |world| match enchant {
                Enchant::Jackhammer => jackhammer(server, &world, player, &mine, pos.y, fortune),
                Enchant::Explosive => explosive(server, &world, player, &mine, pos, level, fortune),
                Enchant::Nuke => nuke(server, &world, player, &mine, fortune),
                _ => Ok(()),
            });
            if let Ok(Err(error)) | Err(error) = result {
                log::warn(format!("{} failed: {error}", enchant.id()));
            }
        });
    }
    true
}

// --- Menu ----------------------------------------------------------------------------------

fn icon(enchant: Enchant, level: u32) -> ItemSpec {
    let tuning = config().enchant(enchant);
    let mut lore = vec![
        format!("&7{}", enchant.description()),
        String::new(),
        format!("&7Level: &f{} &8/ {}", roman_or_number(level), tuning.max),
    ];
    if enchant.procs() {
        lore.push(format!("&7Chance: &f{:.2}%", tuning.chance(level) * 100.0));
    }
    if level >= tuning.max {
        lore.push("&a&lMAXED".to_owned());
    } else {
        lore.push(format!(
            "&7Next level: &a{}",
            money(tuning.cost_of(level + 1))
        ));
        lore.push(format!(
            "&7+10 levels: &a{}",
            money(tuning.cost_between(level, level + 10))
        ));
        lore.push(String::new());
        lore.push("&eLeft-click &7+1  &eRight-click &7+10".to_owned());
    }
    ItemSpec::new(enchant.icon())
        .name_legacy(&format!(
            "{} &f{}",
            enchant.display(),
            roman_or_number(level)
        ))
        .lore_legacy(lore.iter().map(String::as_str))
        .glint(if level > 0 { Glint::On } else { Glint::Off })
}

fn info_icon(player: PlayerId) -> ItemSpec {
    let balance = vault_api::economy().map_or(0.0, |economy| economy.balance(player.uuid()));
    let stats = stats()
        .get(&player.uuid().bytes)
        .copied()
        .unwrap_or_default();
    ItemSpec::new("minecraft:emerald")
        .name_legacy(&format!("&a&lBalance: {}", money(balance)))
        .lore_legacy([
            format!("&7Blocks mined: &f{}", stats.blocks).as_str(),
            format!("&7Earned: &a{}", money(stats.earned)).as_str(),
        ])
}

fn menu_items(player: PlayerId, levels: &Levels) -> Vec<ItemSpec> {
    let pane = ItemSpec::new("minecraft:black_stained_glass_pane").name_legacy("&r");
    let mut items = vec![pane; usize::try_from(MENU_ROWS * 9).unwrap_or(27)];
    for (slot, enchant) in ICON_SLOTS.into_iter().zip(Enchant::ALL) {
        if let Ok(index) = usize::try_from(slot) {
            items[index] = icon(enchant, levels.get(enchant));
        }
    }
    if let Ok(index) = usize::try_from(INFO_SLOT) {
        items[index] = info_icon(player);
    }
    items
}

fn open_menu(server: &Server<'_>, player: PlayerId) -> Result<(), String> {
    let (_, levels) = held_pickaxe(server, player).ok_or("Hold your prison pickaxe (/pickaxe).")?;
    let handler = *MENU.get().ok_or("menu is not ready")?;
    server.open_menu_with(
        player,
        &legacy("&8Pickaxe Enchants"),
        MENU_ROWS,
        menu_items(player, &levels),
        handler,
    )?;
    let _ = server.play_sound_to(
        player,
        "minecraft:block.enchantment_table.use".into(),
        SoundCategory::Players,
        1.0,
        1.0,
    );
    Ok(())
}

fn buy(server: &Server<'_>, click: MenuClick) -> Result<(), String> {
    let Some(enchant) = ICON_SLOTS
        .iter()
        .position(|slot| *slot == click.slot)
        .map(|index| Enchant::ALL[index])
    else {
        return Ok(());
    };
    let player = click.player;
    let (slot, mut levels) = held_pickaxe(server, player).ok_or("Hold your prison pickaxe.")?;
    let tuning = config().enchant(enchant);
    let current = levels.get(enchant);
    let wanted = match click.kind {
        ClickKind::Pickup if click.button == 1 => current + 10,
        ClickKind::Pickup | ClickKind::QuickMove => current + 1,
        _ => return Ok(()),
    }
    .min(tuning.max);
    if wanted == current {
        return Err(format!("{} is already maxed.", enchant.id()));
    }
    let cost = tuning.cost_between(current, wanted);
    let economy = vault_api::economy().map_err(|_| "No economy is installed.".to_owned())?;
    let receipt = economy.withdraw(player.uuid(), cost);
    if !receipt.is_success() {
        return Err(format!(
            "You need {} (you have {}).",
            money(cost),
            money(economy.balance(player.uuid()))
        ));
    }
    levels.set(enchant, wanted);
    store_pickaxe(server, player, slot, &levels)?;
    let _ = server.set_menu_item(player, click.slot, icon(enchant, wanted));
    let _ = server.set_menu_item(player, INFO_SLOT, info_icon(player));
    let _ = server.play_sound_to(
        player,
        "minecraft:entity.player.levelup".into(),
        SoundCategory::Players,
        1.0,
        1.2,
    );
    tell(
        server,
        player,
        &format!(
            "&aUpgraded {} &ato &f{}&a for &e{}&a.",
            enchant.display(),
            roman_or_number(wanted),
            money(cost)
        ),
    );
    Ok(())
}

// --- Passive effects and sidebar -----------------------------------------------------------

fn sidebar(server: &Server<'_>, player: PlayerId, world: &str, at: Position, levels: &Levels) {
    let balance = vault_api::economy().map_or(0.0, |economy| economy.balance(player.uuid()));
    let stats = stats()
        .get(&player.uuid().bytes)
        .copied()
        .unwrap_or_default();
    let mut lines = vec![
        "&8&m              ".to_owned(),
        format!("&7Balance: &a{}", money(balance)),
        format!("&7Blocks: &f{}", stats.blocks),
        format!("&7Earned: &a{}", money(stats.earned)),
        " ".to_owned(),
    ];
    #[expect(
        clippy::cast_possible_truncation,
        reason = "block coordinates of a player position"
    )]
    let feet = BlockPos {
        x: at.x.floor() as i32,
        y: at.y.floor() as i32 - 1,
        z: at.z.floor() as i32,
    };
    let mine: Option<MineInfo> = prison_api::mines()
        .ok()
        .and_then(|mines| Option::from(mines.mine_at(world.into(), feet)));
    if let Some(mine) = mine {
        lines.push(format!("&7Mine: &e{}", mine.name));
        #[expect(clippy::cast_precision_loss, reason = "a percentage for display")]
        let mined = mine.mined as f64 * 100.0 / mine.volume.max(1) as f64;
        lines.push(format!("&7Mined: &f{mined:.0}%"));
        if mine.seconds_until_reset >= 0 {
            let left = mine.seconds_until_reset;
            lines.push(format!("&7Reset: &e{}:{:02}", left / 60, left % 60));
        }
        lines.push("  ".to_owned());
    }
    for enchant in Enchant::ALL {
        let level = levels.get(enchant);
        if level > 0 {
            lines.push(format!(
                "{} &f{}",
                enchant.display(),
                roman_or_number(level)
            ));
        }
    }
    lines.push("&8&m               ".to_owned());
    let lines = lines
        .iter()
        .map(|line| legacy(line).as_str().into())
        .collect();
    let _ = server.set_sidebar(player, legacy("&6&l⛏ OP PRISON ⛏").as_str().into(), lines);
}

/// Once per second: Haste and Speed while holding the pickaxe, and the sidebar.
fn passive(server: &Server<'_>) {
    for player in &server.players() {
        let Some((_, levels)) = held_pickaxe(server, player.id) else {
            continue;
        };
        for (enchant, effect) in [
            (Enchant::Haste, "minecraft:haste"),
            (Enchant::Speed, "minecraft:speed"),
        ] {
            let level = levels.get(enchant);
            if level > 0 {
                let amplifier = i32::try_from(level - 1).unwrap_or(0);
                let _ = server.add_effect(
                    player.id,
                    Effect::new(effect, 60, amplifier).hide_particles(),
                );
            }
        }
        sidebar(
            server,
            player.id,
            player.world.as_str(),
            player.position,
            &levels,
        );
    }
}

// --- Commands ------------------------------------------------------------------------------

fn give_pickaxe(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = ctx
        .player()
        .map_err(|_| "Only players can hold a pickaxe.".to_owned())?;
    let mut levels = Levels::default();
    levels.set(Enchant::Jackhammer, 1);
    Result::from(ctx.server().give(player, pickaxe(&levels))).map_err(|error| error.to_string())?;
    ctx.send_legacy("&bHere is your &lPrison Pickaxe&b. Right-click it to upgrade.");
    Ok(1)
}

fn enchants_command(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = ctx
        .player()
        .map_err(|_| "Only players can upgrade.".to_owned())?;
    open_menu(&ctx.server(), player)?;
    Ok(1)
}

fn set_level(ctx: &Invocation<'_, '_>, target: Option<PlayerId>) -> Result<i32, String> {
    let enchant = Enchant::from_id(&ctx.string("enchant")?).ok_or("Unknown enchant.")?;
    let level = u32::try_from(ctx.integer("level")?).map_err(|_| "Levels start at 0.")?;
    let player = match target {
        Some(player) => player,
        None => ctx.player()?,
    };
    let server = ctx.server();
    let (slot, mut levels) =
        held_pickaxe(&server, player).ok_or("The player is not holding a prison pickaxe.")?;
    levels.set(enchant, level.min(config().enchant(enchant).max));
    store_pickaxe(&server, player, slot, &levels)?;
    ctx.send_legacy(&format!(
        "&6Set {} &6to &f{}&6.",
        enchant.display(),
        levels.get(enchant)
    ));
    Ok(1)
}

fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    commands.register(
        CommandBuilder::new("prison-enchants:pickaxe")
            .default_access()
            .executes(give_pickaxe),
    )?;
    commands.register(
        CommandBuilder::new("prison-enchants:enchants")
            .alias("enchant")
            .default_access()
            .executes(enchants_command),
    )?;
    let enchant_ids = || {
        Enchant::ALL
            .iter()
            .map(|enchant| enchant.id().to_owned())
            .collect()
    };
    commands.register(
        CommandBuilder::new("prison-enchants:penchant")
            .permission(ADMIN)
            .then(
                argument("enchant", Arg::word())
                    .suggests(move |_| enchant_ids())
                    .then(
                        argument("level", Arg::integer(0, 10_000))
                            .executes(|ctx| set_level(ctx, None))
                            .then(
                                argument("player", Arg::player()).executes(|ctx| {
                                    set_level(ctx, Some(ctx.player_arg("player")?))
                                }),
                            ),
                    ),
            ),
    )
}

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.enable_scheduler();
    hooks.on_commands(register)?;
    let _ = MENU.set(menus::handler(|server, click| {
        if let Err(error) = buy(server, click) {
            tell(server, click.player, &format!("&c{error}"));
            let _ = server.play_sound_to(
                click.player,
                "minecraft:block.note_block.bass".into(),
                SoundCategory::Players,
                1.0,
                0.5,
            );
        }
    }));
    hooks.on_block_break_with(EventPriority::High, true, |event| {
        let (server, world) = (event.server(), event.world());
        if on_break(&server, &world, event.player(), event.pos(), event.state()) {
            event.set_drop_items(false);
        }
    });
    hooks.on_interact_with(EventPriority::Normal, true, |event| {
        if !matches!(
            event.action(),
            InteractAction::RightClickAir | InteractAction::RightClickBlock
        ) {
            return;
        }
        let server = event.server();
        if held_pickaxe(&server, event.player()).is_none() {
            return;
        }
        event.set_cancelled(true);
        if let Err(error) = open_menu(&server, event.player()) {
            tell(&server, event.player(), &format!("&c{error}"));
        }
    });
    Ok(())
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let text = context.config();
    let config = if text.as_str().trim().is_empty() {
        Config::default()
    } else {
        toml::from_str(text.as_str()).map_err(|error| format!("invalid config: {error}"))?
    };
    CONFIG
        .set(config)
        .map_err(|_| "prison-enchants initialized twice".to_owned())?;
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_nanos() % (1_u128 << 64)).unwrap_or(0)
        });
    RANDOM.fetch_xor(seed, Ordering::Relaxed);
    if vault_api::economy().is_err() {
        log::warn("No Vault economy provider: mined blocks are not paid and upgrades are disabled");
    }
    scheduler::run_repeating(20, 20, passive);
    Ok(())
}

steel_extension_api::export_extension! {
    name: "prison-enchants",
    version: "0.1.0",
    depends: ["prison-mines"],
    soft_depends: ["vault", "essentials"],
    pre_init: pre_init,
    init: init,
}
