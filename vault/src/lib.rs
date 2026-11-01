//! Vault for Steel: permission and chat providers backed by Steel's permission groups, plus
//! `/vault-info`. The economy contract is provided by another plugin, such as Essentials.
//!
//! Chat prefixes and suffixes come from permission metadata keys `vault:prefix` and
//! `vault:suffix` (configure them per group in `groups.toml`).
#![recursion_limit = "256"]

use steel_extension_api::{
    Hooks, PlayerId, ExtensionContext, Server,
    command::CommandBuilder,
    log,
    players::{MetadataKind, MetadataValue, Uuid},
    services,
    stabby::{self, str::Str, string::String as ApiString, vec::Vec as ApiVec},
};
use vault_api::{CHAT, Chat, DynChat, DynPermission, ECONOMY, PERMISSION, Permission};

/// Metadata key read for chat prefixes.
pub const PREFIX_KEY: &str = "vault:prefix";
/// Metadata key read for chat suffixes.
pub const SUFFIX_KEY: &str = "vault:suffix";

fn online(server: &Server<'_>, player: Uuid) -> Option<PlayerId> {
    server.player_by_uuid(player).into()
}

fn metadata_text(server: &Server<'_>, player: Uuid, key: &str) -> String {
    let Some(id) = online(server, player) else {
        return String::new();
    };
    let value: Option<MetadataValue> = server
        .permission_metadata(id, key.into())
        .match_owned(Option::from, |_| None);
    value.map_or_else(String::new, |value| value.text.to_string())
}

struct SteelPermission;

#[stabby::export(callback)]
impl Permission for SteelPermission {
    fn name(&self) -> ApiString {
        "Steel".into()
    }

    fn has_group_support(&self) -> bool {
        true
    }

    fn has(&self, server: Server<'_>, player: Uuid, node: Str<'_>, default: bool) -> bool {
        online(&server, player).is_some_and(|id| server.has_permission(id, node, default))
    }

    fn player_groups(&self, server: Server<'_>, player: Uuid) -> ApiVec<ApiString> {
        online(&server, player)
            .and_then(|id| server.permission_groups(id).match_owned(Some, |_| None))
            .unwrap_or_default()
    }

    fn primary_group(&self, server: Server<'_>, player: Uuid) -> ApiString {
        self.player_groups(server, player)
            .iter()
            .next()
            .cloned()
            .unwrap_or_default()
    }

    fn groups(&self, server: Server<'_>) -> ApiVec<ApiString> {
        server.group_names()
    }

    // TODO: Steel exposes no runtime group membership mutation to plugins yet; Vault callers
    // receive `false` exactly like a provider without write support.
    fn add_group(&self, _server: Server<'_>, _player: Uuid, _group: Str<'_>) -> bool {
        false
    }

    fn remove_group(&self, _server: Server<'_>, _player: Uuid, _group: Str<'_>) -> bool {
        false
    }
}

struct SteelChat;

#[stabby::export(callback)]
impl Chat for SteelChat {
    fn name(&self) -> ApiString {
        "Steel".into()
    }

    fn prefix(&self, server: Server<'_>, player: Uuid) -> ApiString {
        metadata_text(&server, player, PREFIX_KEY).into()
    }

    fn suffix(&self, server: Server<'_>, player: Uuid) -> ApiString {
        metadata_text(&server, player, SUFFIX_KEY).into()
    }

    fn option_string(&self, server: Server<'_>, player: Uuid, key: Str<'_>) -> ApiString {
        metadata_text(&server, player, key.as_str()).into()
    }

    fn option_integer(&self, server: Server<'_>, player: Uuid, key: Str<'_>, default: i64) -> i64 {
        let Some(id) = online(&server, player) else {
            return default;
        };
        let value: Option<MetadataValue> = server
            .permission_metadata(id, key)
            .match_owned(Option::from, |_| None);
        match value {
            Some(value) if value.kind == MetadataKind::Integer => value.integer,
            Some(value) => value.text.as_str().parse().unwrap_or(default),
            None => default,
        }
    }
}

fn describe(key: &str) -> String {
    let providers = services::providers(key);
    if providers.is_empty() {
        return format!("&7{key}: &cnone");
    }
    let names: Vec<String> = providers
        .iter()
        .map(|info| format!("{} (priority {})", info.provider, info.priority))
        .collect();
    format!("&7{key}: &a{}", names.join("&7, &a"))
}

fn pre_init(mut hooks: Hooks<'_>) -> Result<(), String> {
    hooks.on_commands(|commands| {
        commands.register(CommandBuilder::new("vault:vault-info").executes(|ctx| {
            ctx.send_legacy("&6Vault for Steel");
            for key in [ECONOMY, PERMISSION, CHAT] {
                ctx.send_legacy(&describe(key));
            }
            Ok(1)
        }))
    })
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    services::register::<DynPermission>(
        &context,
        PERMISSION,
        0,
        stabby::boxed::Box::new(SteelPermission).into(),
    )?;
    services::register::<DynChat>(&context, CHAT, 0, stabby::boxed::Box::new(SteelChat).into())?;
    log::info("Registered Steel permission and chat providers");
    Ok(())
}

steel_extension_api::export_extension! {
    name: "vault",
    version: "0.1.0",
    depends: [],
    soft_depends: [],
    pre_init: pre_init,
    init: init,
}
