use super::{api, balls, source};
use steel_extension_api::{
    PlayerId, Server, data::Value, hud::Source, items::ItemSpec, stabby::vec::Vec,
};

pub const PARTY: usize = 6;
const SIDEBAR_CREATURES: usize = 3;
pub const THROW_AFTER: &str = "creatures:state/throw_after";

/// Vanilla can send entity-interaction followed by item-use for one click.
/// Reserve an authored use interval before either callback changes inventory.
pub fn reserve_throw(
    server: &Server<'_>,
    player: PlayerId,
    now: i64,
    ticks: u32,
) -> Result<bool, String> {
    let data = api(server.player_data(player))?;
    api(data.initialize(THROW_AFTER.into(), Value::integer(now)))?;
    if now < integer(server, player, THROW_AFTER)? {
        return Ok(false);
    }
    let ready = now
        .checked_add(i64::from(ticks))
        .ok_or("ball use deadline overflow")?;
    api(data.set(THROW_AFTER.into(), Value::integer(ready)))?;
    Ok(true)
}
pub fn path(slot: usize) -> String {
    format!("creatures:state/party/{slot}")
}
pub fn read(server: &Server<'_>, player: PlayerId, path: &str) -> Result<Option<Value>, String> {
    let data = api(server.player_data(player))?;
    Ok(api(data.get(path.into()))?.into())
}
pub fn integer(server: &Server<'_>, player: PlayerId, path: &str) -> Result<i64, String> {
    let value = read(server, player, path)?.ok_or_else(|| format!("missing {path}"))?;
    Option::from(value.as_integer()).ok_or_else(|| format!("{path} is not an integer"))
}
pub fn label(server: &Server<'_>, player: PlayerId, slot: usize) -> Result<String, String> {
    text(server, player, &format!("{}/label", path(slot)))
}
pub fn text(server: &Server<'_>, player: PlayerId, path: &str) -> Result<String, String> {
    let value = read(server, player, path)?.ok_or_else(|| format!("missing {path}"))?;
    let value: Option<steel_extension_api::stabby::string::String> = value.as_text().into();
    value
        .map(|s| s.to_string())
        .ok_or_else(|| format!("{path} is not text"))
}
pub fn token(server: &Server<'_>, player: PlayerId, slot: usize) -> Result<ItemSpec, String> {
    let grade = usize::try_from(integer(server, player, &format!("{}/grade", path(slot)))?)
        .map_err(|e| e.to_string())?;
    let key = balls::KEYS.get(grade).ok_or("invalid ball grade")?;
    let identity = format!("{}/control", path(slot));
    if read(server, player, &identity)?.is_none() {
        let data = api(server.player_data(player))?;
        api(data.initialize(
            identity.as_str().into(),
            Value::text(rand::random::<u128>().to_string().as_str().into()),
        ))?;
    }
    Ok(ItemSpec::new(key)
        .name(&format!(
            "{} · {}",
            balls::NAMES[grade],
            label(server, player, slot)?
        ))
        .with_data("creatures:slot", &slot.to_string())
        .with_data("creatures:owner", &player.uuid().to_string())
        .with_data("creatures:control", &text(server, player, &identity)?))
}
pub fn validate_ball(
    server: &Server<'_>,
    player: PlayerId,
    slot: usize,
    item: &ItemSpec,
) -> Result<(), String> {
    let outdated = || {
        "this ball no longer controls this creature; get its current ball from /party".to_owned()
    };
    let value = read(server, player, &format!("{}/control", path(slot)))?.ok_or_else(outdated)?;
    let control: Option<steel_extension_api::stabby::string::String> = value.as_text().into();
    let control = control.ok_or("creature control identity is not text")?;
    if item.data("creatures:owner") != Some(player.uuid().to_string().as_str())
        || item.data("creatures:control") != Some(control.as_str())
    {
        return Err(outdated());
    }
    Ok(())
}
pub fn bind(source: Source) -> Result<(), String> {
    for slot in 0..PARTY {
        api(source.bind(
            format!("creature_{}", slot + 1).as_str().into(),
            format!("{}/label", path(slot)).as_str().into(),
        ))?;
        api(source.bind(
            format!("creature_{}_health", slot + 1).as_str().into(),
            format!("{}/health", path(slot)).as_str().into(),
        ))?;
        api(source.bind(
            format!("creature_{}_max_health", slot + 1).as_str().into(),
            format!("{}/health/maximum", path(slot)).as_str().into(),
        ))?;
    }
    Ok(())
}
pub fn hud(server: &Server<'_>, player: PlayerId) -> Result<(), String> {
    let deployed = api(server.stored_entity_instances(player))?;
    let mut lines = Vec::new();
    lines.push(r#"{"text":"Your team","color":"gold","bold":true}"#.into());
    let mut shown = 0;
    for slot in 0..PARTY {
        if read(server, player, &path(slot))?.is_none() {
            continue;
        }
        if shown == SIDEBAR_CREATURES {
            break;
        }
        shown += 1;
        let grade = usize::try_from(integer(server, player, &format!("{}/grade", path(slot)))?)
            .map_err(|error| error.to_string())?;
        let ball = balls::NAMES.get(grade).ok_or("invalid ball grade")?;
        let (status, color) = if deployed
            .iter()
            .any(|entry| entry.path.as_str() == path(slot))
        {
            ("Deployed", "aqua")
        } else {
            ("In ball", "gray")
        };
        let index = slot + 1;
        lines.push(format!(r#"{{"text":"{index}. ${{creature_{index}}}","color":"gold"}}"#).into());
        lines.push(
            format!(r#"{{"text":"HP: ${{creature_{index}_health}}/${{creature_{index}_max_health}}","color":"green"}}"#).into(),
        );
        lines.push(format!(r#"{{"text":"{ball} · {status}","color":"{color}"}}"#).into());
    }
    api(source()?.set_sidebar(server, player, "".into(), lines))
}
fn update(server: &Server<'_>, player: PlayerId) -> Result<(), String> {
    api(server.synchronize_stored_entities(player))?;
    let instances = api(server.stored_entity_instances(player))?;
    let live: Vec<_> = instances.iter().map(|entry| entry.instance).collect();
    super::battle::retain_instances(player, &live);
    for deployment in instances {
        if super::battle::completed(player, deployment.instance, deployment.entity.identity()) {
            api(server.return_entity(player, deployment.path.as_str().into()))?;
            api(server.message(
                player,
                "Battle ended; your creature returned to its ball.".into(),
            ))?;
        }
    }
    hud(server, player)
}
pub fn heartbeat(server: &Server<'_>) -> Result<(), String> {
    let mut errors = Vec::new();
    let players = server.players();
    super::battle::retain_owners(
        &players
            .iter()
            .map(|player| player.id.uuid())
            .collect::<Vec<_>>(),
    );
    for player in players {
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
