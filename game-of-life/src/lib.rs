//! Runs Conway's Game of Life on one configured X/Z board.
mod life;

use serde::Deserialize;
use std::sync::OnceLock;
use steel_extension_api::{
    ApiResult, BlockState, Hooks, default_block_state,
    stabby::{str::Str, string::String as ApiString},
};

struct Materials {
    alive: BlockState,
    dead: BlockState,
}

struct Settings {
    materials: Materials,
    life: life::LifeConfig,
}

static SETTINGS: OnceLock<Settings> = OnceLock::new();

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    life: life::LifeConfig,
}

fn pre_init(mut hooks: Hooks<'_>) -> ApiResult<()> {
    hooks.register_world_tick(life::tick);
    Ok(()).into()
}

fn init(config: Str<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        let config: Config =
            toml::from_str(config.as_str()).map_err(|error| ApiString::from(error.to_string()))?;
        config.life.validate().map_err(ApiString::from)?;
        let materials = Materials {
            alive: default_block_state("minecraft:black_concrete".into()).match_owned(Ok, Err)?,
            dead: default_block_state("minecraft:white_concrete".into()).match_owned(Ok, Err)?,
        };
        SETTINGS
            .set(Settings {
                materials,
                life: config.life,
            })
            .map_err(|_| ApiString::from("Game of Life extension was initialized twice"))?;
        Ok(())
    })();
    result.into()
}

fn settings() -> &'static Settings {
    match SETTINGS.get() {
        Some(settings) => settings,
        None => panic!("Game of Life tick invoked before successful initialization"),
    }
}

steel_extension_api::export_extension!("game-of-life-example", pre_init, init);
