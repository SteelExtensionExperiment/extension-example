//! Command-driven action/filter demonstration with built-in tick and stop notifications.

use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use steel_extension_api::{
    ApiResult, Command, Commands, Hooks,
    hooks::{ServerStopAction, ServerTickAction, apply_filters, do_action},
    stabby::{self, str::Str, string::String as ApiString},
};
use steel_hook_example_contract::{Score, Scored};

static LAST_TICK: AtomicU64 = AtomicU64::new(0);
static LAST_SCORE: AtomicI32 = AtomicI32::new(0);
static SCORED_EVENTS: AtomicU64 = AtomicU64::new(0);

fn pre_init(mut hooks: Hooks<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        hooks
            .register_commands(register_commands)
            .match_owned(Ok, Err)?;
        hooks
            .add_filter::<Score>(add_five)
            .after("hook-bonus-example")
            .register()
            .match_owned(Ok, Err)?;
        hooks
            .add_action::<Scored>(observe_score)
            .register()
            .match_owned(Ok, Err)?;
        hooks
            .add_action::<ServerTickAction>(observe_tick)
            .register()
            .match_owned(Ok, Err)?;
        hooks
            .add_action::<ServerStopAction>(observe_stop)
            .register()
            .match_owned(Ok, Err)?;
        Ok(())
    })();
    result.into()
}

#[stabby::export(callback)]
fn register_commands(mut commands: Commands<'_>) -> ApiResult<()> {
    commands.register_player_command(
        "hookdemo:hooks".into(),
        [Str::new("run")].as_slice().into(),
        run,
    )
}

#[stabby::export(callback)]
fn add_five(score: &mut Score) {
    score.value = score.value.saturating_add(5);
}

#[stabby::export(callback)]
fn observe_score(event: &Scored) {
    LAST_SCORE.store(event.value, Ordering::Relaxed);
    SCORED_EVENTS.fetch_add(1, Ordering::Relaxed);
}

#[stabby::export(callback)]
fn observe_tick(event: &ServerTickAction) {
    LAST_TICK.store(event.tick_count, Ordering::Relaxed);
}

#[stabby::export(callback)]
fn observe_stop(_event: &ServerStopAction) {
    eprintln!(
        "Hook demo stopped after {} scored actions",
        SCORED_EVENTS.load(Ordering::Relaxed)
    );
}

#[stabby::export(callback)]
fn run(command: Command<'_>, _action: Str<'_>) -> ApiResult<i32> {
    let result = (|| -> Result<i32, ApiString> {
        let mut score = Score { value: 10 };
        apply_filters(&mut score).match_owned(Ok, Err)?;
        do_action(&Scored { value: score.value }).match_owned(Ok, Err)?;
        let message = format!(
            "Score: 10 -> {}. Action observed: {}. Scored actions: {}. Last server tick: {}.",
            score.value,
            LAST_SCORE.load(Ordering::Relaxed),
            SCORED_EVENTS.load(Ordering::Relaxed),
            LAST_TICK.load(Ordering::Relaxed),
        );
        command.reply(message.as_str().into());
        Ok(score.value)
    })();
    result.into()
}

// All subscriptions are installed during pre-init; the demo needs no configuration.
fn init(_config: Str<'_>) -> ApiResult<()> {
    Ok(()).into()
}

steel_extension_api::export_extension!("hook-base-example", pre_init, init);
