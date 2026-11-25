use crate::{api, config, source};
use steel_extension_api::{
    PlayerId, Server,
    data::Value,
    hud::{HudSprites, HudText},
    text::Text,
};

pub const MANA: &str = "magic:state/mana/current";
pub const MAXIMUM: &str = "magic:state/mana/maximum";
pub const PROJECTILE: &str = "magic:state/protection/projectile_until";
pub const PHYSICAL: &str = "magic:state/protection/physical_until";

pub const MANA_SPRITES: HudSprites<'static> =
    HudSprites::new("magic:hud/mana").with_gauge_strip("magic:hud/mana.png");
const MANA_SEGMENTS: u16 = 20;

// The action bar centers its full text width. Vanilla spaces are four GUI pixels
// wide, so this trailing padding moves the visible center 50 pixels toward armor.
const ARMOR_ALIGNMENT_SPACES: usize = 25;

pub fn number(server: &Server<'_>, player: PlayerId, path: &str) -> Result<f64, String> {
    let data = api(server.player_data(player))?;
    let value: Option<Value> = api(data.get(path.into()))?.into();
    value
        .and_then(|v| Option::from(v.as_number()))
        .ok_or_else(|| format!("{path} is missing or nonnumeric"))
}
pub fn initialize(server: &Server<'_>, player: PlayerId) -> Result<(), String> {
    let data = api(server.player_data(player))?;
    api(data.initialize(MAXIMUM.into(), api(Value::number(config()?.maximum_mana))?))?;
    let maximum = number(server, player, MAXIMUM)?;
    if maximum <= 0.0 || !maximum.is_finite() {
        return Err("mana maximum must be finite and positive".into());
    }
    api(data.initialize(MANA.into(), api(Value::number(maximum))?))?;
    Ok(())
}
pub fn publish(server: &Server<'_>, player: PlayerId) -> Result<(), String> {
    let meter = MANA_SPRITES.gauge("player_mana", MANA_SEGMENTS)?;
    let line = |meter| {
        Text::new("Mana: ${player_mana}/${player_max_mana} ")
            .color("aqua")
            .append(meter)
            .append(Text::new(" ".repeat(ARMOR_ALIGNMENT_SPACES)))
            .to_json()
    };
    let template = HudText::new(&line(meter.vanilla().color("blue")))
        .with_resource_pack(&line(meter.packed()));
    api(source()?.set_action_bar(server, player, template))
}
fn update(server: &Server<'_>, player: PlayerId) -> Result<(), String> {
    initialize(server, player)?;
    let data = api(server.player_data(player))?;
    api(data.add_clamped(
        MANA.into(),
        config()?.regeneration,
        0.0,
        number(server, player, MAXIMUM)?,
    ))?;
    publish(server, player)
}
pub fn heartbeat(server: &Server<'_>) -> Result<(), String> {
    let mut errors = Vec::new();
    for player in server.players() {
        if let Err(error) = update(server, player.id) {
            errors.push(format!("{}: {error}", player.name));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
pub fn active(server: &Server<'_>, player: PlayerId, path: &str, now: i64) -> Result<bool, String> {
    let data = api(server.player_data(player))?;
    let value: Option<Value> = api(data.get(path.into()))?.into();
    let Some(value) = value else {
        return Ok(false);
    };
    let expiry: Option<i64> = value.as_integer().into();
    Ok(expiry.ok_or_else(|| format!("{path} is not an integer timestamp"))? > now)
}
