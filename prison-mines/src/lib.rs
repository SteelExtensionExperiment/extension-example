//! OP Prison mines: regions defined from a WorldEdit selection, refilled from a weighted block
//! mix on a timer or when enough of them was mined, with a boss bar countdown for nearby
//! players. Resets use the bulk fill path and report their speed.
//!
//! Commands live under `/mine` (alias `/mines`); changes need `prison.mines.admin`. Other
//! plugins reach mines through the `prison:mines/v1` service (see `prison-api`).
#![recursion_limit = "256"]

mod mine;

use mine::{Mine, MineFile, bar_id, parse_mix, threshold_reached, valid_name};
use prison_api::{DynMines, MineInfo, Mines, ResetReport};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Mutex, MutexGuard, OnceLock, PoisonError},
};
use steel_extension_api::{
    ApiResult, BlockPos, Commands, Hooks, PlayerId, ExtensionContext, Position, Server,
    command::{Arg, CommandBuilder, Invocation, argument, literal},
    events::{EventPriority, Location},
    fill::{FillRequest, PatternEntry},
    hud::{BarColor, BarStyle, BossBar},
    log,
    players::Uuid,
    scheduler, services,
    stabby::{
        self, option::Option as StableOption, str::Str, string::String as ApiString,
        vec::Vec as ApiVec,
    },
    text,
};
use worldedit_api::SelectionsDyn as _;

const ADMIN: &str = "prison.mines.admin";
/// Players this close to a mine see its countdown bar.
const BAR_MARGIN: f64 = 24.0;

#[derive(Debug, Default, Clone, Copy)]
struct Runtime {
    mined: u64,
    seconds_left: i64,
}

struct Prison {
    path: PathBuf,
    mines: Mutex<BTreeMap<String, Mine>>,
    runtime: Mutex<BTreeMap<String, Runtime>>,
    /// Players currently shown each mine's bar.
    bars: Mutex<BTreeMap<String, Vec<PlayerId>>>,
}

static PRISON: OnceLock<Prison> = OnceLock::new();

fn prison() -> Result<&'static Prison, String> {
    PRISON
        .get()
        .ok_or_else(|| "prison-mines is not initialized".to_owned())
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Prison {
    fn save(&self, mines: &BTreeMap<String, Mine>) -> Result<(), String> {
        let file = MineFile {
            mines: mines.clone(),
        };
        let text = toml::to_string(&file).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("toml.tmp");
        fs::write(&temporary, text)
            .and_then(|()| fs::rename(&temporary, &self.path))
            .map_err(|error| format!("cannot write {}: {error}", self.path.display()))
    }

    /// Changes one mine and saves the file.
    fn edit(
        &self,
        name: &str,
        change: impl FnOnce(&mut Mine) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut mines = lock(&self.mines);
        let mut updated = mines.clone();
        let mine = updated
            .get_mut(name)
            .ok_or_else(|| format!("No mine named {name}."))?;
        change(mine)?;
        self.save(&updated)?;
        *mines = updated;
        Ok(())
    }

    fn mine(&self, name: &str) -> Result<Mine, String> {
        lock(&self.mines)
            .get(name)
            .cloned()
            .ok_or_else(|| format!("No mine named {name}."))
    }

    fn info(&self, name: &str, mine: &Mine) -> MineInfo {
        let runtime = lock(&self.runtime).get(name).copied().unwrap_or_default();
        MineInfo {
            name: name.into(),
            world: mine.world.as_str().into(),
            volume: mine.volume(),
            mined: runtime.mined,
            seconds_until_reset: if mine.interval == 0 {
                -1
            } else {
                runtime.seconds_left
            },
            min: mine.min_pos(),
            max: mine.max_pos(),
        }
    }

    fn add_mined(&self, name: &str, count: u64) {
        if let Some(runtime) = lock(&self.runtime).get_mut(name) {
            runtime.mined = runtime.mined.saturating_add(count);
        }
    }

    fn mine_at(&self, world: &str, pos: BlockPos) -> Option<(String, Mine)> {
        lock(&self.mines)
            .iter()
            .find(|(_, mine)| mine.world == world && mine.contains(pos))
            .map(|(name, mine)| (name.clone(), mine.clone()))
    }
}

fn pattern(mine: &Mine) -> Result<Vec<PatternEntry>, String> {
    mine.mix
        .iter()
        .map(|entry| {
            Result::from(steel_extension_api::parse_block_state(
                entry.block.as_str().into(),
            ))
            .map(|state| PatternEntry {
                state,
                weight: entry.weight,
            })
            .map_err(|error| format!("{}: {error}", entry.block))
        })
        .collect()
}

#[expect(
    clippy::cast_precision_loss,
    reason = "durations and percentages are shown to players, not computed with"
)]
fn ratio(part: u64, whole: u64) -> f64 {
    part as f64 / whole.max(1) as f64
}

fn millis(micros: u64) -> f64 {
    ratio(micros, 1000)
}

fn tell(server: &Server<'_>, player: PlayerId, message: &str) {
    let _ = server.send_message(player, text::legacy(message).to_json().as_str().into());
}

/// Lifts players out, refills the mine and resets its counters.
fn reset(server: &Server<'_>, name: &str) -> Result<ResetReport, String> {
    let prison = prison()?;
    let mine = prison.mine(name)?;
    let pattern = pattern(&mine)?;
    let (_, top, _) = mine.top_center();
    for player in &server.players() {
        let at = player.position;
        if player.world.as_str() == mine.world && mine.holds_player(at.x, at.y, at.z) {
            let lifted = Position { y: top, ..at };
            let _ = server.teleport(player.id, lifted);
            tell(
                server,
                player.id,
                "&7You were lifted out of the mine before it reset.",
            );
        }
    }
    let report = server.with_world(&mine.world, |world| {
        Result::from(world.fill(FillRequest::cuboid(
            mine.min_pos(),
            mine.max_pos(),
            &pattern,
        )))
        .map_err(|error| error.to_string())
    })??;
    lock(&prison.runtime).insert(
        name.to_owned(),
        Runtime {
            mined: 0,
            seconds_left: i64::from(mine.interval),
        },
    );
    let message = format!(
        "&6Mine &e{name}&6 reset: &e{}&6 blocks in &e{:.1}&6 ms (&e{:.0}&6 blocks/s)",
        report.changed,
        millis(report.micros),
        report.blocks_per_second()
    );
    for player in &server.players() {
        let at = player.position;
        if player.world.as_str() == mine.world && mine.is_near(at.x, at.y, at.z, BAR_MARGIN) {
            tell(server, player.id, &message);
        }
    }
    log::info(format!(
        "mine {name} reset: {} blocks in {} us",
        report.changed, report.micros
    ));
    Ok(ResetReport {
        blocks: report.changed,
        micros: report.micros,
    })
}

fn format_time(seconds: i64) -> String {
    format!("{}:{:02}", seconds.max(0) / 60, seconds.max(0) % 60)
}

/// Shows each mine's countdown to nearby players and removes it from players who left.
fn update_bars(server: &Server<'_>, prison: &Prison, mines: &BTreeMap<String, Mine>) {
    let players = server.players();
    let runtime = lock(&prison.runtime).clone();
    let mut bars = lock(&prison.bars);
    for (name, mine) in mines {
        let state = runtime.get(name).copied().unwrap_or_default();
        let volume = mine.volume().max(1);
        let mined_fraction = ratio(state.mined.min(volume), volume);
        let (title, progress) = if mine.interval > 0 {
            #[expect(
                clippy::cast_precision_loss,
                clippy::cast_possible_truncation,
                reason = "the countdown fraction is shown as a bar"
            )]
            let fraction = (state.seconds_left as f64 / f64::from(mine.interval)) as f32;
            (
                format!(
                    "&6&lMine {name} &7resets in &e{} &8| &7{:.0}% mined",
                    format_time(state.seconds_left),
                    mined_fraction * 100.0
                ),
                fraction,
            )
        } else {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the remaining fraction is shown as a bar"
            )]
            let remaining = (1.0 - mined_fraction) as f32;
            (
                format!("&6&lMine {name} &8| &7{:.0}% mined", mined_fraction * 100.0),
                remaining,
            )
        };
        let bar = BossBar::new(&text::legacy(&title).to_json(), progress, BarColor::Yellow)
            .style(BarStyle::Notched20);
        let id = Uuid {
            bytes: bar_id(name),
        };
        let near: Vec<PlayerId> = players
            .iter()
            .filter(|player| {
                let at = player.position;
                player.world.as_str() == mine.world && mine.is_near(at.x, at.y, at.z, BAR_MARGIN)
            })
            .map(|player| player.id)
            .collect();
        let shown = bars.entry(name.clone()).or_default();
        for player in shown.iter().filter(|player| !near.contains(player)) {
            let _ = server.remove_boss_bar(*player, id);
        }
        for player in &near {
            let _ = server.set_boss_bar(*player, id, bar.clone());
        }
        *shown = near;
    }
    bars.retain(|name, _| mines.contains_key(name));
}

/// Runs once per second: timers, thresholds and bars.
fn tick(server: &Server<'_>) {
    let Ok(prison) = prison() else {
        return;
    };
    let mines = lock(&prison.mines).clone();
    let mut due = Vec::new();
    {
        let mut runtime = lock(&prison.runtime);
        for (name, mine) in &mines {
            let state = runtime.entry(name.clone()).or_insert(Runtime {
                mined: 0,
                seconds_left: i64::from(mine.interval),
            });
            if mine.interval > 0 {
                state.seconds_left -= 1;
            }
            let timed = mine.interval > 0 && state.seconds_left <= 0;
            if timed || threshold_reached(state.mined, mine.volume(), mine.threshold) {
                due.push(name.clone());
            }
        }
    }
    for name in due {
        if let Err(error) = reset(server, &name) {
            log::warn(format!("mine {name} could not reset: {error}"));
            // Retry on the next timer period instead of every second.
            if let Some(state) = lock(&prison.runtime).get_mut(&name) {
                state.seconds_left = mines
                    .get(&name)
                    .map_or(60, |mine| i64::from(mine.interval.max(60)));
            }
        }
    }
    update_bars(server, prison, &mines);
}

fn mine_names() -> Vec<String> {
    prison().map_or_else(
        |_| Vec::new(),
        |prison| lock(&prison.mines).keys().cloned().collect(),
    )
}

fn name_node() -> steel_extension_api::command::Node {
    argument("name", Arg::word()).suggests(|_| mine_names())
}

fn name_arg(ctx: &Invocation<'_, '_>) -> Result<String, String> {
    valid_name(&ctx.string("name")?)
        .ok_or_else(|| "Mine names may use a-z, 0-9, _ and -.".to_owned())
}

fn create(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let prison = prison()?;
    let player = ctx
        .player()
        .map_err(|_| "Only players can create mines.".to_owned())?;
    let name = name_arg(ctx)?;
    let selection: Option<worldedit_api::Cuboid> =
        worldedit_api::selections()?.selection(player.uuid()).into();
    let selection = selection.ok_or("Make a WorldEdit selection first (//wand).")?;
    let mine = Mine::new(selection.world.as_str(), selection.min, selection.max);
    let mut mines = lock(&prison.mines);
    if mines.contains_key(&name) {
        return Err(format!("Mine {name} already exists."));
    }
    if let Some((other, _)) = mines.iter().find(|(_, other)| other.overlaps(&mine)) {
        return Err(format!("The selection overlaps mine {other}."));
    }
    let volume = mine.volume();
    let mut updated = mines.clone();
    updated.insert(name.clone(), mine);
    prison.save(&updated)?;
    *mines = updated;
    ctx.send_legacy(&format!(
        "&6Created mine &e{name}&6 ({volume} blocks). Set its blocks with &e/mine set {name} 60%stone,40%coal_ore&6, then &e/mine reset {name}&6."
    ));
    Ok(1)
}

fn set_mix(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let name = name_arg(ctx)?;
    let mix = parse_mix(&ctx.string("mix")?)?;
    let probe = Mine {
        mix: mix.clone(),
        ..Mine::new(
            "",
            BlockPos { x: 0, y: 0, z: 0 },
            BlockPos { x: 0, y: 0, z: 0 },
        )
    };
    pattern(&probe)?;
    prison()?.edit(&name, |mine| {
        mine.mix = mix;
        Ok(())
    })?;
    let text = prison()?.mine(&name)?.mix_text();
    ctx.send_legacy(&format!("&6Mine &e{name}&6 now uses &e{text}&6."));
    Ok(1)
}

fn set_interval(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let name = name_arg(ctx)?;
    let seconds = u32::try_from(ctx.integer("seconds")?).map_err(|_| "Use 0 or more seconds.")?;
    let prison = prison()?;
    prison.edit(&name, |mine| {
        mine.interval = seconds;
        Ok(())
    })?;
    if let Some(state) = lock(&prison.runtime).get_mut(&name) {
        state.seconds_left = i64::from(seconds);
    }
    ctx.send_legacy(&if seconds == 0 {
        format!("&6Mine &e{name}&6 no longer resets on a timer.")
    } else {
        format!(
            "&6Mine &e{name}&6 resets every &e{}&6.",
            format_time(i64::from(seconds))
        )
    });
    Ok(1)
}

fn set_threshold(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let name = name_arg(ctx)?;
    let percent = u8::try_from(ctx.integer("percent")?).map_err(|_| "Use 0 to 100.")?;
    prison()?.edit(&name, |mine| {
        mine.threshold = percent;
        Ok(())
    })?;
    ctx.send_legacy(&if percent == 0 {
        format!("&6Mine &e{name}&6 no longer resets when mined out.")
    } else {
        format!("&6Mine &e{name}&6 resets when &e{percent}%&6 is mined.")
    });
    Ok(1)
}

fn reset_command(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let name = name_arg(ctx)?;
    let report = reset(&ctx.server(), &name)?;
    if ctx.player().is_err() {
        ctx.send(&format!(
            "Mine {name} reset: {} blocks in {:.1} ms",
            report.blocks,
            millis(report.micros)
        ));
    }
    Ok(1)
}

fn delete(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let prison = prison()?;
    let name = name_arg(ctx)?;
    let mut mines = lock(&prison.mines);
    let mut updated = mines.clone();
    if updated.remove(&name).is_none() {
        return Err(format!("No mine named {name}."));
    }
    prison.save(&updated)?;
    *mines = updated;
    lock(&prison.runtime).remove(&name);
    let id = Uuid {
        bytes: bar_id(&name),
    };
    if let Some(shown) = lock(&prison.bars).remove(&name) {
        for player in shown {
            let _ = ctx.server().remove_boss_bar(player, id);
        }
    }
    ctx.send_legacy(&format!("&6Deleted mine &e{name}&6."));
    Ok(1)
}

fn list(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let mines = lock(&prison()?.mines).clone();
    if mines.is_empty() {
        ctx.send_legacy("&7No mines yet. Create one with &e/mine create <name>&7.");
        return Ok(0);
    }
    ctx.send_legacy(&format!("&6Mines ({}):", mines.len()));
    for (name, mine) in &mines {
        ctx.send_legacy(&format!(
            "&e {name} &7- {} blocks, {}",
            mine.volume(),
            mine.world
        ));
    }
    Ok(i32::try_from(mines.len()).unwrap_or(i32::MAX))
}

fn info(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let prison = prison()?;
    let name = name_arg(ctx)?;
    let mine = prison.mine(&name)?;
    let info = prison.info(&name, &mine);
    let [x1, y1, z1] = mine.min;
    let [x2, y2, z2] = mine.max;
    ctx.send_legacy(&format!("&6Mine &e{name}&6 in &e{}", mine.world));
    ctx.send_legacy(&format!(
        "&7 Box: {x1} {y1} {z1} to {x2} {y2} {z2} ({} blocks)",
        info.volume
    ));
    ctx.send_legacy(&format!("&7 Blocks: &f{}", mine.mix_text()));
    ctx.send_legacy(&format!(
        "&7 Mined: &f{}&7 ({:.1}%)",
        info.mined,
        ratio(info.mined, info.volume) * 100.0
    ));
    ctx.send_legacy(&if mine.interval == 0 {
        "&7 Timer: off".to_owned()
    } else {
        format!(
            "&7 Timer: every {} (next in {})",
            format_time(i64::from(mine.interval)),
            format_time(info.seconds_until_reset)
        )
    });
    ctx.send_legacy(&if mine.threshold == 0 {
        "&7 Threshold: off".to_owned()
    } else {
        format!("&7 Threshold: {}% mined", mine.threshold)
    });
    Ok(1)
}

fn teleport(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let player = ctx
        .player()
        .map_err(|_| "Only players can teleport.".to_owned())?;
    let name = name_arg(ctx)?;
    let mine = prison()?.mine(&name)?;
    let (x, y, z) = mine.top_center();
    Result::from(ctx.server().teleport_to(
        player,
        Location {
            world: mine.world.as_str().into(),
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.0,
        },
    ))
    .map_err(|error| error.to_string())?;
    ctx.send_legacy(&format!("&6Teleported to mine &e{name}&6."));
    Ok(1)
}

fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    commands.register(
        CommandBuilder::new("prison-mines:mine")
            .alias("mines")
            .default_access()
            .then(
                literal("create")
                    .permission(ADMIN)
                    .then(argument("name", Arg::word()).executes(create)),
            )
            .then(
                literal("set")
                    .permission(ADMIN)
                    .then(name_node().then(argument("mix", Arg::greedy()).executes(set_mix))),
            )
            .then(
                literal("interval").permission(ADMIN).then(
                    name_node()
                        .then(argument("seconds", Arg::integer(0, 86_400)).executes(set_interval)),
                ),
            )
            .then(literal("threshold").permission(ADMIN).then(
                name_node().then(argument("percent", Arg::integer(0, 100)).executes(set_threshold)),
            ))
            .then(
                literal("reset")
                    .permission(ADMIN)
                    .then(name_node().executes(reset_command)),
            )
            .then(
                literal("delete")
                    .permission(ADMIN)
                    .then(name_node().executes(delete)),
            )
            .then(literal("list").executes(list))
            .then(literal("info").then(name_node().executes(info)))
            .then(literal("tp").then(name_node().executes(teleport))),
    )
}

/// The `prison:mines/v1` provider.
struct MineService;

impl Mines for MineService {
    extern "C" fn mine_at(&self, world: Str<'_>, pos: BlockPos) -> StableOption<MineInfo> {
        prison()
            .ok()
            .and_then(|prison| {
                prison
                    .mine_at(world.as_str(), pos)
                    .map(|(name, mine)| prison.info(&name, &mine))
            })
            .into()
    }

    extern "C" fn mines(&self) -> ApiVec<MineInfo> {
        let Ok(prison) = prison() else {
            return ApiVec::new();
        };
        let mines = lock(&prison.mines).clone();
        mines
            .iter()
            .map(|(name, mine)| prison.info(name, mine))
            .collect()
    }

    extern "C" fn add_mined(&self, name: Str<'_>, count: u64) {
        if let Ok(prison) = prison() {
            prison.add_mined(name.as_str(), count);
        }
    }

    extern "C" fn reset(&self, server: Server<'_>, name: Str<'_>) -> ApiResult<ResetReport> {
        reset(&server, name.as_str())
            .map_err(ApiString::from)
            .into()
    }
}

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.enable_scheduler();
    hooks.on_commands(register)?;
    hooks.on_block_break_with(EventPriority::Monitor, true, |event| {
        let Ok(prison) = prison() else {
            return;
        };
        let world = event.world().key().to_string();
        if let Some((name, _)) = prison.mine_at(&world, event.pos()) {
            prison.add_mined(&name, 1);
        }
    });
    Ok(())
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let path = PathBuf::from(context.data_directory().as_str()).join("mines.toml");
    let file = match fs::read_to_string(&path) {
        Ok(text) => toml::from_str::<MineFile>(&text)
            .map_err(|error| format!("{} is invalid: {error}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => MineFile::default(),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let runtime = file
        .mines
        .iter()
        .map(|(name, mine)| {
            (
                name.clone(),
                Runtime {
                    mined: 0,
                    seconds_left: i64::from(mine.interval),
                },
            )
        })
        .collect();
    log::info(format!("Loaded {} mines", file.mines.len()));
    PRISON
        .set(Prison {
            path,
            mines: Mutex::new(file.mines),
            runtime: Mutex::new(runtime),
            bars: Mutex::new(BTreeMap::new()),
        })
        .map_err(|_| "prison-mines initialized twice".to_owned())?;
    services::register::<DynMines>(
        &context,
        prison_api::MINES,
        0,
        stabby::boxed::Box::new(MineService).into(),
    )?;
    scheduler::run_repeating(20, 20, tick);
    Ok(())
}

steel_extension_api::export_extension! {
    name: "prison-mines",
    version: "0.1.0",
    depends: ["worldedit"],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
