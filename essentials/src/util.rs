//! Small helpers shared by command and event handlers.
use crate::storage::Loc;
use steel_extension_api::{PlayerId, Server, text};

/// Sends a `&`-formatted message to a player, ignoring players who just left.
pub fn tell(server: &Server<'_>, player: PlayerId, message: &str) {
    let _ = server.send_message(player, text::legacy(message).to_json().as_str().into());
}

/// Broadcasts a `&`-formatted message.
pub fn broadcast(server: &Server<'_>, message: &str) {
    server.broadcast(text::legacy(message).to_json().as_str().into());
}

/// Converts an API result into `Result<T, String>`.
macro_rules! api {
    ($value:expr) => {
        $value.match_owned(Ok, |error| Err(error.to_string()))
    };
}
pub(crate) use api;

/// A player's current location.
pub fn location(server: &Server<'_>, player: PlayerId) -> Result<Loc, String> {
    api!(server.player_location(player)).map(|location| Loc::from(&location))
}

/// Teleports a player to a stored location.
pub fn teleport(server: &Server<'_>, player: PlayerId, target: &Loc) -> Result<(), String> {
    api!(server.teleport_to(player, target.to_api()))
}

/// A player's profile name, or an empty string if they left.
pub fn name(server: &Server<'_>, player: PlayerId) -> String {
    api!(server.player_name(player))
        .map(|name| name.to_string())
        .unwrap_or_default()
}

/// Unix time in seconds.
pub fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

/// Lowercases a home or warp name and checks `[a-z0-9_-]{1,32}`.
pub fn valid_name(name: &str) -> Option<String> {
    let name = name.to_lowercase();
    let valid = (1..=32).contains(&name.len())
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        });
    valid.then_some(name)
}

/// Formats seconds as `1h 2m 3s`.
pub fn duration(seconds: u64) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    match (hours, minutes) {
        (0, 0) => format!("{seconds}s"),
        (0, _) => format!("{minutes}m {seconds}s"),
        _ => format!("{hours}h {minutes}m {seconds}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_normalized_and_validated() {
        assert_eq!(valid_name("Base_1"), Some("base_1".into()));
        assert_eq!(valid_name("no spaces"), None);
        assert_eq!(valid_name(""), None);
        assert_eq!(valid_name(&"a".repeat(33)), None);
    }

    #[test]
    fn durations_are_compact() {
        assert_eq!(duration(5), "5s");
        assert_eq!(duration(65), "1m 5s");
        assert_eq!(duration(3_725), "1h 2m 5s");
    }
}
