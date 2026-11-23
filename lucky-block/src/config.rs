use steel_extension_api::stabby::string::String as ApiString;
use toml::{Table, Value};

pub(crate) fn parse_seed(input: &str) -> Result<Option<u64>, ApiString> {
    let settings: Table = input
        .parse()
        .map_err(|error: toml::de::Error| ApiString::from(error.to_string()))?;
    if settings.keys().any(|key| key != "seed") {
        return Err("lucky-block configuration accepts only 'seed'".into());
    }
    match settings.get("seed") {
        None => Ok(None),
        Some(Value::Integer(seed)) => Ok(Some(*seed as u64)),
        Some(_) => Err("lucky-block 'seed' must be an integer".into()),
    }
}
