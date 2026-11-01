//! Configurable player-facing messages with `&` color codes and `{placeholder}` arguments.

/// Default message for every key; config `[messages]` entries override them.
const DEFAULTS: &[(&str, &str)] = &[
    ("no_permission", "&cYou do not have permission to do that."),
    ("player_only", "&cOnly players can use this command."),
    ("player_not_found", "&cPlayer &4{player}&c is not online."),
    (
        "invalid_name",
        "&cNames may only use a-z, 0-9, _ and -, up to 32 characters.",
    ),
    ("teleporting", "&6Teleporting..."),
    ("home_set", "&6Home &c{home}&6 set."),
    ("home_deleted", "&6Home &c{home}&6 deleted."),
    ("home_missing", "&cYou have no home named &4{home}&c."),
    ("home_limit", "&cYou can only set &4{limit}&c homes."),
    ("homes_none", "&6You have no homes. Use &c/sethome&6."),
    ("homes_list", "&6Homes: &r{homes}"),
    ("warp_set", "&6Warp &c{warp}&6 set."),
    ("warp_deleted", "&6Warp &c{warp}&6 deleted."),
    ("warp_missing", "&cWarp &4{warp}&c does not exist."),
    ("warps_none", "&6There are no warps."),
    ("warps_list", "&6Warps: &r{warps}"),
    ("spawn_set", "&6Spawn set."),
    (
        "spawn_missing",
        "&cThe spawn has not been set. Use &4/setspawn&c.",
    ),
    ("back_none", "&cYou have no previous location."),
    ("tpa_sent", "&6Request sent to &c{player}&6."),
    (
        "tpa_received",
        "&c{player}&6 has requested to teleport to you.",
    ),
    (
        "tpahere_received",
        "&c{player}&6 has requested that you teleport to them.",
    ),
    (
        "tpa_hint",
        "&6Type &c/tpaccept&6 to accept or &c/tpdeny&6 to deny. It expires in &c{seconds}&6 seconds.",
    ),
    ("tpa_none", "&cYou have no pending teleport request."),
    ("tpa_accepted", "&6Teleport request accepted."),
    (
        "tpa_accepted_other",
        "&c{player}&6 accepted your teleport request.",
    ),
    ("tpa_denied", "&6Teleport request denied."),
    (
        "tpa_denied_other",
        "&c{player}&6 denied your teleport request.",
    ),
    ("tpa_self", "&cYou cannot send a request to yourself."),
    ("balance", "&aBalance: &c{amount}"),
    ("balance_other", "&a{player}'s balance: &c{amount}"),
    ("paid", "&a{amount} has been sent to {player}."),
    ("received", "&a{amount} has been received from {player}."),
    ("insufficient_funds", "&cYou do not have enough money."),
    ("invalid_amount", "&cInvalid amount."),
    ("pay_self", "&cYou cannot pay yourself."),
    ("max_money", "&cThis would exceed the balance limit."),
    ("baltop_header", "&6Top balances:"),
    ("baltop_entry", "&7{rank}. &f{player}&7, &c{amount}"),
    ("eco_done", "&a{player}'s balance is now &c{amount}&a."),
    ("god_on", "&6God mode &cenabled&6 for {player}."),
    ("god_off", "&6God mode &cdisabled&6 for {player}."),
    ("fly_on", "&6Flight &cenabled&6 for {player}."),
    ("fly_off", "&6Flight &cdisabled&6 for {player}."),
    ("healed", "&6{player} has been healed."),
    ("fed", "&6{player} is no longer hungry."),
    ("gamemode", "&6Set game mode &c{mode}&6 for {player}."),
    ("nick_set", "&6Your nickname is now &r{nick}&6."),
    ("nick_off", "&6Your nickname was removed."),
    (
        "nick_invalid",
        "&cNicknames must be 1 to 16 characters without spaces.",
    ),
    ("broadcast", "&6[&4Broadcast&6]&a {message}"),
    ("msg_to", "&6[me -> &c{player}&6]&r {message}"),
    ("msg_from", "&6[&c{player}&6 -> me]&r {message}"),
    ("reply_none", "&cYou have nobody to reply to."),
    ("afk_on", "&7* {player} is now AFK."),
    ("afk_off", "&7* {player} is no longer AFK."),
    ("kit_given", "&6Received kit &c{kit}&6."),
    ("kit_missing", "&cKit &4{kit}&c does not exist."),
    (
        "kit_cooldown",
        "&cYou can use that kit again in &4{time}&c.",
    ),
    ("kits_list", "&6Kits: &r{kits}"),
    (
        "kit_dropped",
        "&6Your inventory was full; &c{count}&6 items were dropped.",
    ),
];

/// Message templates resolved against the configuration.
pub struct Messages {
    overrides: std::collections::BTreeMap<String, String>,
}

impl Messages {
    /// Creates templates with configured overrides.
    pub const fn new(overrides: std::collections::BTreeMap<String, String>) -> Self {
        Self { overrides }
    }

    /// Formats `key`, replacing `{name}` with each argument value.
    pub fn format(&self, key: &str, args: &[(&str, &str)]) -> String {
        let template = self
            .overrides
            .get(key)
            .map(String::as_str)
            .or_else(|| {
                DEFAULTS
                    .iter()
                    .find(|(name, _)| *name == key)
                    .map(|(_, text)| *text)
            })
            .unwrap_or(key);
        let mut text = template.to_owned();
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overrides_win_and_placeholders_are_replaced() {
        let mut overrides = std::collections::BTreeMap::new();
        overrides.insert("home_set".to_owned(), "set {home}!".to_owned());
        let messages = Messages::new(overrides);
        assert_eq!(
            messages.format("home_set", &[("home", "base")]),
            "set base!"
        );
        assert_eq!(
            messages.format("home_deleted", &[("home", "a")]),
            "&6Home &ca&6 deleted."
        );
        assert_eq!(messages.format("unknown_key", &[]), "unknown_key");
    }
}
