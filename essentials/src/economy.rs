//! Balances in integer cents, `/balance`, `/pay`, `/baltop`, `/eco`, and the Vault economy
//! provider. Amounts use two decimal places, like EssentialsX's default.
use crate::{
    state::essentials,
    util::{name, tell},
};
use steel_extension_api::{
    Commands, PlayerId,
    command::{Arg, CommandBuilder, Invocation, argument, literal},
    players::Uuid,
    stabby::{self, str::Str, string::String as ApiString},
};
use vault_api::{Economy, EconomyResponse};

/// Parses a user-typed amount with at most two decimals into cents.
pub fn parse_amount(text: &str) -> Option<i64> {
    let text = text.trim().replace(',', "");
    let (whole, fraction) = text.split_once('.').unwrap_or((&text, ""));
    if whole.is_empty() && fraction.is_empty() || fraction.len() > 2 {
        return None;
    }
    if !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let whole: i64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let fraction: i64 = format!("{fraction:0<2}").parse().ok()?;
    whole.checked_mul(100)?.checked_add(fraction)
}

/// Converts a non-negative finite amount to cents, rounding to the nearest cent.
#[expect(
    clippy::cast_possible_truncation,
    reason = "amounts are checked against i64 cents before converting"
)]
pub fn to_cents(amount: f64) -> Option<i64> {
    let cents = (amount * 100.0).round();
    (amount.is_finite() && amount >= 0.0 && cents < 9.2e18).then_some(cents as i64)
}

/// Converts cents to the floating-point amount used by Vault.
#[expect(
    clippy::cast_precision_loss,
    reason = "Vault's contract uses f64 amounts; balances stay exact in cents internally"
)]
pub fn to_amount(cents: i64) -> f64 {
    cents as f64 / 100.0
}

/// Formats cents as `$1,234.50`.
pub fn format(cents: i64, symbol: &str) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.unsigned_abs();
    let whole = (cents / 100).to_string();
    let mut grouped = String::new();
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("{sign}{symbol}{grouped}.{:02}", cents % 100)
}

fn format_money(cents: i64) -> String {
    let symbol = essentials().map_or("$", |ess| ess.config.currency_symbol.as_str());
    format(cents, symbol)
}

fn max_cents() -> i64 {
    essentials()
        .ok()
        .and_then(|ess| to_cents(ess.config.max_money))
        .unwrap_or(i64::MAX)
}

/// Creates an account with the starting balance if none exists.
pub fn ensure_account(uuid: Uuid, player_name: &str) -> bool {
    let Ok(ess) = essentials() else {
        return false;
    };
    let starting = to_cents(ess.config.starting_balance).unwrap_or(0);
    let mut data = ess.data();
    if let Some(user) = data.users.get(&uuid)
        && user.name == player_name
    {
        return false;
    }
    let created = !data.users.contains_key(&uuid);
    let user = data.user_mut(uuid);
    if created {
        user.money = starting;
    }
    if !player_name.is_empty() {
        player_name.clone_into(&mut user.name);
    }
    created
}

fn balance(uuid: Uuid) -> i64 {
    essentials()
        .ok()
        .and_then(|ess| ess.data().users.get(&uuid).map(|user| user.money))
        .unwrap_or(0)
}

/// Applies `change` to the balance if the result stays within `0..=max_money`.
fn adjust(uuid: Uuid, change: i64) -> Result<i64, &'static str> {
    let ess = essentials().map_err(|_| "economy unavailable")?;
    let max = max_cents();
    let mut data = ess.data();
    let current = data.users.get(&uuid).ok_or("no account")?.money;
    let next = current.checked_add(change).ok_or("amount too large")?;
    if next < 0 {
        return Err("insufficient funds");
    }
    if next > max {
        return Err("balance limit exceeded");
    }
    data.user_mut(uuid).money = next;
    Ok(next)
}

/// Essentials as the Vault economy provider.
pub struct EssentialsEconomy;

#[stabby::export(callback)]
impl Economy for EssentialsEconomy {
    fn name(&self) -> ApiString {
        "Essentials Economy".into()
    }
    fn fractional_digits(&self) -> i32 {
        2
    }
    fn format(&self, amount: f64) -> ApiString {
        format_money(to_cents(amount).unwrap_or(0)).into()
    }
    fn currency_name_singular(&self) -> ApiString {
        essentials()
            .map_or_else(
                |_| String::new(),
                |ess| ess.config.currency_singular.clone(),
            )
            .into()
    }
    fn currency_name_plural(&self) -> ApiString {
        essentials()
            .map_or_else(|_| String::new(), |ess| ess.config.currency_plural.clone())
            .into()
    }
    fn has_account(&self, player: Uuid) -> bool {
        essentials().is_ok_and(|ess| ess.data().users.contains_key(&player))
    }
    fn create_account(&self, player: Uuid, name: Str<'_>) -> bool {
        ensure_account(player, name.as_str())
    }
    fn balance(&self, player: Uuid) -> f64 {
        to_amount(balance(player))
    }
    fn has(&self, player: Uuid, amount: f64) -> bool {
        to_cents(amount).is_some_and(|cents| balance(player) >= cents)
    }
    fn withdraw(&self, player: Uuid, amount: f64) -> EconomyResponse {
        let Some(cents) = to_cents(amount) else {
            return EconomyResponse::failure(amount, self.balance(player), "invalid amount");
        };
        match adjust(player, -cents) {
            Ok(balance) => EconomyResponse::success(amount, to_amount(balance)),
            Err(error) => EconomyResponse::failure(amount, self.balance(player), error),
        }
    }
    fn deposit(&self, player: Uuid, amount: f64) -> EconomyResponse {
        let Some(cents) = to_cents(amount) else {
            return EconomyResponse::failure(amount, self.balance(player), "invalid amount");
        };
        match adjust(player, cents) {
            Ok(balance) => EconomyResponse::success(amount, to_amount(balance)),
            Err(error) => EconomyResponse::failure(amount, self.balance(player), error),
        }
    }
}

fn amount_arg(ctx: &Invocation<'_, '_>) -> Result<i64, String> {
    let ess = essentials()?;
    let text = ctx.string("amount")?;
    parse_amount(&text)
        .filter(|cents| *cents > 0)
        .ok_or_else(|| ess.msg("invalid_amount", &[]))
}

fn show_balance(ctx: &Invocation<'_, '_>, target: Option<PlayerId>) -> Result<i32, String> {
    let ess = essentials()?;
    let server = ctx.server();
    let (target, own) = match target {
        Some(target) => (target, false),
        None => (ctx.player().map_err(|_| ess.msg("player_only", &[]))?, true),
    };
    ensure_account(target.uuid(), &name(&server, target));
    let amount = format_money(balance(target.uuid()));
    let message = if own {
        ess.msg("balance", &[("amount", &amount)])
    } else {
        ess.msg(
            "balance_other",
            &[("player", &name(&server, target)), ("amount", &amount)],
        )
    };
    ctx.send_legacy(&message);
    Ok(1)
}

fn pay(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let server = ctx.server();
    let sender = ctx.player().map_err(|_| ess.msg("player_only", &[]))?;
    let target = ctx.player_arg("player")?;
    if target.uuid() == sender.uuid() {
        return Err(ess.msg("pay_self", &[]));
    }
    let cents = amount_arg(ctx)?;
    ensure_account(target.uuid(), &name(&server, target));
    {
        let max = max_cents();
        let mut data = ess.data();
        let from = data.users.get(&sender.uuid()).map_or(0, |user| user.money);
        if from < cents {
            return Err(ess.msg("insufficient_funds", &[]));
        }
        let to = data.users.get(&target.uuid()).map_or(0, |user| user.money);
        if to.checked_add(cents).is_none_or(|next| next > max) {
            return Err(ess.msg("max_money", &[]));
        }
        data.user_mut(sender.uuid()).money = from - cents;
        data.user_mut(target.uuid()).money = to + cents;
    }
    let amount = format_money(cents);
    ctx.send_legacy(&ess.msg(
        "paid",
        &[("amount", &amount), ("player", &name(&server, target))],
    ));
    tell(
        &server,
        target,
        &ess.msg(
            "received",
            &[("amount", &amount), ("player", &name(&server, sender))],
        ),
    );
    Ok(1)
}

fn baltop(ctx: &Invocation<'_, '_>) -> Result<i32, String> {
    let ess = essentials()?;
    let mut entries: Vec<(String, i64)> = ess
        .data()
        .users
        .values()
        .map(|user| (user.name.clone(), user.money))
        .collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ctx.send_legacy(&ess.msg("baltop_header", &[]));
    for (rank, (player, money)) in entries.iter().take(ess.config.baltop_entries).enumerate() {
        ctx.send_legacy(&ess.msg(
            "baltop_entry",
            &[
                ("rank", &(rank + 1).to_string()),
                ("player", player),
                ("amount", &format_money(*money)),
            ],
        ));
    }
    Ok(1)
}

#[derive(Clone, Copy)]
enum EcoAction {
    Give,
    Take,
    Set,
}

fn eco(ctx: &Invocation<'_, '_>, action: EcoAction) -> Result<i32, String> {
    let ess = essentials()?;
    let server = ctx.server();
    let target = ctx.player_arg("player")?;
    let text = ctx.string("amount")?;
    let cents = parse_amount(&text).ok_or_else(|| ess.msg("invalid_amount", &[]))?;
    ensure_account(target.uuid(), &name(&server, target));
    let result = match action {
        EcoAction::Give => adjust(target.uuid(), cents),
        EcoAction::Take => {
            let current = balance(target.uuid());
            adjust(target.uuid(), -cents.min(current))
        }
        EcoAction::Set => {
            if cents > max_cents() {
                Err("balance limit exceeded")
            } else {
                let mut data = ess.data();
                data.user_mut(target.uuid()).money = cents;
                Ok(cents)
            }
        }
    };
    let balance = result.map_err(str::to_owned)?;
    ctx.send_legacy(&ess.msg(
        "eco_done",
        &[
            ("player", &name(&server, target)),
            ("amount", &format_money(balance)),
        ],
    ));
    Ok(1)
}

/// Registers the economy commands.
pub fn register(commands: &mut Commands<'_>) -> Result<(), String> {
    commands.register(
        CommandBuilder::new("essentials:balance")
            .alias("bal")
            .alias("money")
            .default_access()
            .executes(|ctx| show_balance(ctx, None))
            .then(
                argument("player", Arg::player())
                    .permission("essentials.balance.others")
                    .executes(|ctx| show_balance(ctx, Some(ctx.player_arg("player")?))),
            ),
    )?;
    commands.register(CommandBuilder::new("essentials:pay").default_access().then(
        argument("player", Arg::player()).then(argument("amount", Arg::word()).executes(pay)),
    ))?;
    commands.register(
        CommandBuilder::new("essentials:baltop")
            .default_access()
            .executes(baltop),
    )?;
    let mut eco = CommandBuilder::new("essentials:eco");
    for (word, action) in [
        ("give", EcoAction::Give),
        ("take", EcoAction::Take),
        ("set", EcoAction::Set),
    ] {
        eco =
            eco.then(literal(word).then(argument("player", Arg::player()).then(
                argument("amount", Arg::word()).executes(move |ctx| self::eco(ctx, action)),
            )));
    }
    commands.register(eco)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amounts_parse_to_exact_cents() {
        assert_eq!(parse_amount("10"), Some(1_000));
        assert_eq!(parse_amount("10.5"), Some(1_050));
        assert_eq!(parse_amount("0.05"), Some(5));
        assert_eq!(parse_amount(".5"), Some(50));
        assert_eq!(parse_amount("1,000.25"), Some(100_025));
        assert_eq!(parse_amount("1.234"), None);
        assert_eq!(parse_amount("-1"), None);
        assert_eq!(parse_amount("abc"), None);
        assert_eq!(parse_amount(""), None);
        assert_eq!(parse_amount("99999999999999999999"), None);
    }

    #[test]
    fn floating_amounts_round_and_reject_invalid_values() {
        assert_eq!(to_cents(1.25), Some(125));
        assert_eq!(to_cents(0.1 + 0.2), Some(30));
        assert_eq!(to_cents(-1.0), None);
        assert_eq!(to_cents(f64::NAN), None);
        assert_eq!(to_cents(f64::INFINITY), None);
    }

    #[test]
    fn formatting_groups_thousands() {
        assert_eq!(format(123_456_789, "$"), "$1,234,567.89");
        assert_eq!(format(5, "$"), "$0.05");
        assert_eq!(format(-150, "€"), "-€1.50");
    }
}
