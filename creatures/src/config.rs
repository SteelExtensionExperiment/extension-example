use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub base_chance: [f64; 3],
    pub weakened_bonus: f64,
    pub battle_damage: f32,
    pub projectile_speed: f32,
    pub throw_cooldown_ticks: u32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            base_chance: [0.25, 0.45, 0.7],
            weakened_bonus: 0.4,
            battle_damage: 3.0,
            projectile_speed: 1.5,
            throw_cooldown_ticks: 4,
        }
    }
}
impl Config {
    pub fn parse(input: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(input).map_err(|e| e.to_string())?;
        if config
            .base_chance
            .iter()
            .any(|p| !p.is_finite() || !(0.0..=1.0).contains(p))
            || config.base_chance.windows(2).any(|p| p[0] > p[1])
            || !config.weakened_bonus.is_finite()
            || !(0.0..=1.0).contains(&config.weakened_bonus)
            || !config.battle_damage.is_finite()
            || config.battle_damage <= 0.0
            || !config.projectile_speed.is_finite()
            || config.projectile_speed <= 0.0
            || config.throw_cooldown_ticks == 0
        {
            return Err("invalid creature balancing configuration".into());
        }
        Ok(config)
    }
    pub fn chance(&self, grade: usize, health: f32, maximum: f32) -> Result<f64, String> {
        if grade >= 3
            || !health.is_finite()
            || !maximum.is_finite()
            || maximum <= 0.0
            || health <= 0.0
        {
            return Err("invalid capture grade or target vitals".into());
        }
        Ok((self.base_chance[grade]
            + self.weakened_bonus * (1.0 - f64::from(health / maximum).clamp(0.0, 1.0)))
        .clamp(0.0, 1.0))
    }
}
