//! Chat formatting, join/quit messages, `/msg` and `/r`.
use crate::{
    state::{Essentials, essentials},
    util::{name, tell},
};
use steel_extension_api::{
    Commands, Hooks, PlayerId, Server,
    command::{Arg, CommandBuilder, Invocation, argument},
    events::EventPriority,
    players::Uuid,
    text::{self, Text},
};
use vault_api::{ChatDyn as _, PermissionDyn as _};

/// The nickname or profile name shown in chat.
pub fn display_name(ess: &Essentials, uuid: Uuid, fallback: &str) -> String {
    ess.data()
        .users
        .get(&uuid)
        .and_then(|user| user.nickname.clone())
        .unwrap_or_else(|| fallback.to_owned())
}

/// Prefix, suffix and primary group from the Vault chat and permission services, if present.
fn decorations(server: &Server<'_>, uuid: Uuid) -> (String, String, String) {
    let (prefix, suffix) = vault_api::chat().map_or_else(
        |_| (String::new(), String::new()),
        |chat| {
            (
                chat.prefix(*server, uuid).to_string(),
                chat.suffix(*server, uuid).to_string(),
            )
        },
    );
    let group = vault_api::permission().map_or_else(
        |_| String::new(),
        |permission| permission.primary_group(*server, uuid).to_string(),
    );
    (prefix, suffix, group)
}

/// Renders the configured chat format. Placeholders around `{message}` use `&` codes; the
/// message itself is only colored for players with `essentials.chat.color`.
pub fn render(format: &str, values: &[(&str, &str)], message: &str, colored: bool) -> Text {
    let mut format = format.to_owned();
    for (key, value) in values {
        format = format.replace(&format!("{{{key}}}"), value);
    }
    let (before, after) = format.split_once("{message}").unwrap_or((&format, ""));
    let body = if colored {
        text::legacy(message)
    } else {
        Text::new(message)
    };
    Text::new("")
        .append(text::legacy(before))
        .append(body)
        .append(text::legacy(after))
}

fn private_message(
    ctx: &Invocation<'_, '_>,
    target: PlayerId,
    message: &str,
) -> Result<i32, String> {
    let ess = essentials()?;
    let server = ctx.server();
    let sender = ctx.player().ok();
    let sender_name = sender.map_or_else(
        || "Console".to_owned(),
        |id| display_name(ess, id.uuid(), &name(&server, id)),
    );
    let target_name = display_name(ess, target.uuid(), &name(&server, target));
    ctx.send_legacy(&ess.msg("msg_to", &[("player", &target_name), ("message", message)]));
    tell(
        &server,
        target,
        &ess.msg(
            "msg_from",
            &[("player", &sender_name), ("message", message)],
        ),
    );
    if let Some(sender) = sender {
        let mut runtime = ess.runtime();
        runtime.reply_to.insert(sender.uuid(), target.uuid());
        runtime.reply_to.insert(target.uuid(), sender.uuid());
    }
    Ok(1)
}

/// Registers `/msg` and `/r`.
pub fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    commands.register(
        CommandBuilder::new("essentials:msg")
            .alias("tell")
            .alias("w")
            .alias("m")
            .alias("whisper")
            .default_access()
            .then(argument("player", Arg::player()).then(
                argument("message", Arg::greedy()).executes(|ctx| {
                    private_message(ctx, ctx.player_arg("player")?, &ctx.string("message")?)
                }),
            )),
    )?;
    commands.register(
        CommandBuilder::new("essentials:r")
            .alias("reply")
            .default_access()
            .then(argument("message", Arg::greedy()).executes(|ctx| {
                let ess = essentials()?;
                let sender = ctx.player().map_err(|_| ess.msg("player_only", &[]))?;
                let partner = ess.runtime().reply_to.get(&sender.uuid()).copied();
                let target: Option<PlayerId> =
                    partner.and_then(|uuid| ctx.server().player_by_uuid(uuid).into());
                let target = target.ok_or_else(|| ess.msg("reply_none", &[]))?;
                private_message(ctx, target, &ctx.string("message")?)
            })),
    )
}

/// Formats chat and join/quit messages.
pub fn subscribe(hooks: &mut Hooks<'_>) {
    hooks.on_chat_with(EventPriority::High, true, |event| {
        let Ok(ess) = essentials() else {
            return;
        };
        if ess.config.chat_format.is_empty() {
            return;
        }
        let server = event.server();
        let player = event.player();
        let profile = name(&server, player);
        let display = display_name(ess, player.uuid(), &profile);
        let (prefix, suffix, group) = decorations(&server, player.uuid());
        let colored = server.has_permission(player, "essentials.chat.color".into(), false);
        let rendered = render(
            &ess.config.chat_format,
            &[
                ("prefix", &prefix),
                ("suffix", &suffix),
                ("group", &group),
                ("displayname", &display),
                ("name", &profile),
            ],
            &event.message(),
            colored,
        );
        event.set_rendered(rendered.to_json().as_str().into());
    });
    hooks.on_join(EventPriority::Normal, |event| {
        let Ok(ess) = essentials() else {
            return;
        };
        let server = event.server();
        let player = event.player();
        let profile = name(&server, player);
        crate::economy::ensure_account(player.uuid(), &profile);
        ess.runtime()
            .last_activity
            .insert(player.uuid(), server.current_tick());
        if ess.config.join_message.is_empty() {
            return;
        }
        let display = display_name(ess, player.uuid(), &profile);
        let message = ess.config.join_message.replace("{player}", &display);
        event.set_message(text::legacy(&message).to_json().as_str().into());
    });
    hooks.on_quit(EventPriority::Normal, |event| {
        let Ok(ess) = essentials() else {
            return;
        };
        let server = event.server();
        let player = event.player();
        {
            let mut runtime = ess.runtime();
            runtime.requests.remove(&player.uuid());
            runtime.last_activity.remove(&player.uuid());
            runtime.afk.remove(&player.uuid());
        }
        if ess.config.quit_message.is_empty() {
            return;
        }
        let display = display_name(ess, player.uuid(), &name(&server, player));
        let message = ess.config.quit_message.replace("{player}", &display);
        event.set_message(text::legacy(&message).to_json().as_str().into());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_wraps_uncolored_message() {
        let text = render(
            "{prefix}{displayname}&7: &f{message}",
            &[("prefix", "&c[Admin] "), ("displayname", "Steve")],
            "&ahi",
            false,
        );
        assert_eq!(text.plain(), "[Admin] Steve: &ahi");
        let colored = render(
            "{displayname}: {message}",
            &[("displayname", "Steve")],
            "&ahi",
            true,
        );
        assert_eq!(colored.plain(), "Steve: hi");
    }
}
