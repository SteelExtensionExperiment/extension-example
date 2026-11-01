//! `/afk` and automatic idle detection.
use crate::{
    state::essentials,
    util::{broadcast, name},
};
use steel_extension_api::{
    Commands, Hooks, PlayerId, Server, command::CommandBuilder, events::EventPriority, scheduler,
};

fn set_afk(server: &Server<'_>, player: PlayerId, afk: bool) {
    let Ok(ess) = essentials() else {
        return;
    };
    let changed = {
        let mut runtime = ess.runtime();
        let previous = runtime.afk.insert(player.uuid(), afk).unwrap_or(false);
        previous != afk
    };
    if !changed {
        return;
    }
    let display = crate::chat::display_name(ess, player.uuid(), &name(server, player));
    let key = if afk { "afk_on" } else { "afk_off" };
    broadcast(server, &ess.msg(key, &[("player", &display)]));
}

/// Records activity and clears AFK.
fn active(server: &Server<'_>, player: PlayerId) {
    let Ok(ess) = essentials() else {
        return;
    };
    let was_afk = {
        let mut runtime = ess.runtime();
        runtime
            .last_activity
            .insert(player.uuid(), server.current_tick());
        runtime.afk.get(&player.uuid()).copied().unwrap_or(false)
    };
    if was_afk {
        set_afk(server, player, false);
    }
}

/// Registers `/afk`.
pub fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    commands.register(
        CommandBuilder::new("essentials:afk")
            .default_access()
            .executes(|ctx| {
                let ess = essentials()?;
                let player = ctx.player().map_err(|_| ess.msg("player_only", &[]))?;
                let server = ctx.server();
                let afk = !ess
                    .runtime()
                    .afk
                    .get(&player.uuid())
                    .copied()
                    .unwrap_or(false);
                if !afk {
                    ess.runtime()
                        .last_activity
                        .insert(player.uuid(), server.current_tick());
                }
                set_afk(&server, player, afk);
                Ok(1)
            }),
    )
}

/// Tracks activity and checks idle players once per second.
pub fn subscribe(hooks: &mut Hooks<'_>) {
    hooks.on_move(EventPriority::Monitor, |event| {
        active(&event.server(), event.player());
    });
    hooks.on_chat(EventPriority::Monitor, |event| {
        active(&event.server(), event.player());
    });
    hooks.on_command(EventPriority::Monitor, |event| {
        if !event.command().to_string().starts_with("afk") {
            active(&event.server(), event.player());
        }
    });
    scheduler::run_repeating(20, 20, |server| {
        let Ok(ess) = essentials() else {
            return;
        };
        let timeout = ess.config.afk_timeout_seconds.saturating_mul(20);
        if timeout == 0 {
            return;
        }
        let now = server.current_tick();
        let idle: Vec<PlayerId> = server
            .players()
            .iter()
            .filter(|info| !info.bot)
            .map(|info| info.id)
            .filter(|player| {
                let runtime = ess.runtime();
                let last = runtime
                    .last_activity
                    .get(&player.uuid())
                    .copied()
                    .unwrap_or(now);
                let afk = runtime.afk.get(&player.uuid()).copied().unwrap_or(false);
                !afk && now.saturating_sub(last) >= timeout
            })
            .collect();
        for player in idle {
            set_afk(server, player, true);
        }
    });
}
