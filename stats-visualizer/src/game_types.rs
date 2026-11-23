//! Game type validation against the orchestrator's list.
//!
//! The functions here take a [`GameTypes`] source so they run without a host. T009 (the
//! `/statsboard` command) calls [`check_game_type`] in `place`, [`complete_game_types`] for the
//! `<gametype>` argument and [`unknown_type_boards`] to mark boards in `list`.

use game_orchestrator_api::GamesDyn as _;
use steel_extension_api::Server;

use crate::storage::BoardRecord;

/// Names of the game types that currently exist.
pub trait GameTypes {
    /// # Errors
    /// A message if the game type provider is unavailable.
    fn game_types(&self) -> Result<Vec<String>, String>;
}

/// The orchestrator service. It needs a [`Server`], which only tick-thread callbacks such as
/// scheduler tasks and commands receive.
pub struct OrchestratorGames<'a>(pub Server<'a>);

impl GameTypes for OrchestratorGames<'_> {
    fn game_types(&self) -> Result<Vec<String>, String> {
        let games = game_orchestrator_api::games()?;
        Ok(games
            .game_types(self.0)
            .iter()
            .map(ToString::to_string)
            .collect())
    }
}

/// # Errors
/// `unknown game type <name>` if the orchestrator has no such type, or the provider's error.
pub fn check_game_type(source: &impl GameTypes, name: &str) -> Result<(), String> {
    if source.game_types()?.iter().any(|known| known == name) {
        return Ok(());
    }
    Err(format!("unknown game type {name}"))
}

/// Game types starting with `prefix`. A missing provider completes nothing.
#[must_use]
pub fn complete_game_types(source: &impl GameTypes, prefix: &str) -> Vec<String> {
    let mut types = source.game_types().unwrap_or_default();
    types.retain(|known| known.starts_with(prefix));
    types
}

/// Ids of the boards whose game type is not in `known`.
#[must_use]
pub fn unknown_type_boards<'a>(boards: &'a [BoardRecord], known: &[String]) -> Vec<&'a str> {
    boards
        .iter()
        .filter(|board| !known.contains(&board.game_type))
        .map(|board| board.id.as_str())
        .collect()
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A fixed list, or a provider that is down when `None`.
    pub struct FakeGames(pub Option<Vec<&'static str>>);

    impl GameTypes for FakeGames {
        fn game_types(&self) -> Result<Vec<String>, String> {
            let types = self.0.as_ref().ok_or("no game provider")?;
            Ok(types.iter().map(|name| (*name).to_owned()).collect())
        }
    }

    fn board(id: &str, game_type: &str) -> BoardRecord {
        BoardRecord {
            id: id.to_owned(),
            world: "minecraft:overworld".to_owned(),
            x: 0.0,
            y: 0.0,
            z: 0.0,
            game_type: game_type.to_owned(),
        }
    }

    #[test]
    fn known_type_is_accepted_and_unknown_rejected() {
        let source = FakeGames(Some(vec!["bedwars", "spleef"]));
        assert_eq!(check_game_type(&source, "spleef"), Ok(()));
        assert_eq!(
            check_game_type(&source, "tnt"),
            Err("unknown game type tnt".to_owned())
        );
    }

    #[test]
    fn completion_filters_by_prefix() {
        let source = FakeGames(Some(vec!["bedwars", "spleef", "skywars"]));
        assert_eq!(complete_game_types(&source, "s"), ["spleef", "skywars"]);
        assert_eq!(complete_game_types(&source, "").len(), 3);
        assert_eq!(
            complete_game_types(&FakeGames(None), ""),
            Vec::<String>::new()
        );
    }

    #[test]
    fn boards_of_vanished_types_are_found() {
        let boards = [
            board("a", "bedwars"),
            board("b", "gone"),
            board("c", "gone"),
        ];
        let known = vec!["bedwars".to_owned()];
        assert_eq!(unknown_type_boards(&boards, &known), ["b", "c"]);
    }
}
