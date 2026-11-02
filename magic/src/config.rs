use serde::Deserialize;

/// Authored spell balancing, independent of vanilla combat constants.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub maximum_mana: f64,
    pub regeneration: f64,
    pub costs: [f64; 9],
    pub damage: [f32; 7],
    pub projectile_speed: f32,
    pub terrain_ticks: u32,
    pub protection_ticks: u32,
    pub protection_multiplier: f32,
    /// Vanilla has no general physical-damage tag; this example defaults to melee.
    pub physical_causes: Vec<String>,
    pub physical_tags: Vec<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            maximum_mana: 100.0,
            regeneration: 4.0,
            costs: [12.0, 10.0, 8.0, 18.0, 22.0, 20.0, 35.0, 25.0, 30.0],
            damage: [6.0, 3.0, 5.0, 2.0, 0.0, 0.0, 8.0],
            projectile_speed: 1.4,
            terrain_ticks: 200,
            protection_ticks: 300,
            protection_multiplier: 0.25,
            physical_causes: [
                "minecraft:mob_attack",
                "minecraft:mob_attack_no_aggro",
                "minecraft:sting",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            physical_tags: vec!["minecraft:is_player_attack".into()],
        }
    }
}
impl Config {
    pub fn parse(input: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(input).map_err(|e| e.to_string())?;
        if !config.maximum_mana.is_finite()
            || config.maximum_mana <= 0.0
            || !config.regeneration.is_finite()
            || config.regeneration < 0.0
            || config.costs.iter().any(|v| !v.is_finite() || *v < 0.0)
            || config.damage.iter().any(|v| !v.is_finite() || *v < 0.0)
            || !config.projectile_speed.is_finite()
            || config.projectile_speed <= 0.0
            || config.terrain_ticks == 0
            || config.protection_ticks == 0
            || !config.protection_multiplier.is_finite()
            || !(0.0..=1.0).contains(&config.protection_multiplier)
        {
            return Err("invalid magic balancing configuration".into());
        }
        Ok(config)
    }
}
