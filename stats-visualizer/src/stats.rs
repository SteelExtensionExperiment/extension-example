//! Player stats and result application.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use steel_extension_api::players::Uuid;

/// Wins and losses of one player in one game type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameRecord {
    pub wins: u32,
    pub losses: u32,
}

/// All counters of one player, with the last name seen.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerRecord {
    pub name: String,
    pub games: BTreeMap<String, GameRecord>,
}

/// A player taking part in a finished game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Participant {
    pub uuid: Uuid,
    pub name: String,
}

/// A finished game, reduced to what stats need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchOutcome {
    pub game_type: String,
    pub winners: Vec<Participant>,
    pub losers: Vec<Participant>,
}

/// A result that cannot be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultError {
    /// The UUID appears more than once among winners and losers.
    DuplicateParticipant(Uuid),
}

impl fmt::Display for ResultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateParticipant(uuid) => {
                write!(f, "player {uuid} appears more than once in the result")
            }
        }
    }
}

impl std::error::Error for ResultError {}

/// Player records plus the flag telling the autosave that they changed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stats {
    players: HashMap<Uuid, PlayerRecord>,
    dirty: bool,
}

impl Stats {
    /// Builds stats from loaded records; nothing needs saving yet.
    #[must_use]
    pub fn from_players(players: HashMap<Uuid, PlayerRecord>) -> Self {
        Self {
            players,
            dirty: false,
        }
    }

    #[must_use]
    pub fn players(&self) -> &HashMap<Uuid, PlayerRecord> {
        &self.players
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Adds one win per winner and one loss per loser. A result naming any player twice is
    /// rejected as a whole and changes nothing.
    ///
    /// # Errors
    /// [`ResultError::DuplicateParticipant`] if a UUID appears twice among winners and losers.
    pub fn apply(&mut self, result: &MatchOutcome) -> Result<(), ResultError> {
        let mut seen = HashSet::new();
        for participant in result.winners.iter().chain(&result.losers) {
            if !seen.insert(participant.uuid) {
                return Err(ResultError::DuplicateParticipant(participant.uuid));
            }
        }

        for (participants, is_win) in [(&result.winners, true), (&result.losers, false)] {
            for participant in participants {
                let record = self.players.entry(participant.uuid).or_default();
                record.name.clone_from(&participant.name);
                let game = record.games.entry(result.game_type.clone()).or_default();
                let counter = if is_win {
                    &mut game.wins
                } else {
                    &mut game.losses
                };
                *counter = counter.saturating_add(1);
            }
        }
        self.dirty = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u8) -> Uuid {
        Uuid { bytes: [n; 16] }
    }

    fn player(n: u8, name: &str) -> Participant {
        Participant {
            uuid: uuid(n),
            name: name.to_owned(),
        }
    }

    fn outcome(winners: Vec<Participant>, losers: Vec<Participant>) -> MatchOutcome {
        MatchOutcome {
            game_type: "bedwars".to_owned(),
            winners,
            losers,
        }
    }

    fn game(stats: &Stats, n: u8) -> GameRecord {
        stats.players()[&uuid(n)].games["bedwars"]
    }

    #[test]
    fn adds_wins_and_losses_per_participant() {
        let mut stats = Stats::default();
        stats
            .apply(&outcome(
                vec![player(1, "a"), player(2, "b")],
                vec![player(3, "c")],
            ))
            .unwrap();
        stats
            .apply(&outcome(vec![player(1, "a")], vec![player(2, "b")]))
            .unwrap();

        assert_eq!(game(&stats, 1), GameRecord { wins: 2, losses: 0 });
        assert_eq!(game(&stats, 2), GameRecord { wins: 1, losses: 1 });
        assert_eq!(game(&stats, 3), GameRecord { wins: 0, losses: 1 });
        assert!(stats.is_dirty());
    }

    #[test]
    fn replaces_stored_name() {
        let mut stats = Stats::default();
        stats.apply(&outcome(vec![player(1, "old")], vec![])).unwrap();
        stats.apply(&outcome(vec![player(1, "new")], vec![])).unwrap();
        assert_eq!(stats.players()[&uuid(1)].name, "new");
    }

    #[test]
    fn counters_saturate() {
        let mut players = HashMap::new();
        players.insert(
            uuid(1),
            PlayerRecord {
                name: "a".to_owned(),
                games: BTreeMap::from([(
                    "bedwars".to_owned(),
                    GameRecord {
                        wins: u32::MAX,
                        losses: u32::MAX,
                    },
                )]),
            },
        );
        let mut stats = Stats::from_players(players);
        assert!(!stats.is_dirty());

        stats.apply(&outcome(vec![player(1, "a")], vec![])).unwrap();
        stats.apply(&outcome(vec![], vec![player(1, "a")])).unwrap();
        assert_eq!(
            game(&stats, 1),
            GameRecord {
                wins: u32::MAX,
                losses: u32::MAX
            }
        );
    }

    #[test]
    fn duplicate_uuid_rejects_whole_result() {
        let same_group = outcome(vec![player(1, "a"), player(2, "b"), player(1, "a")], vec![]);
        let both_groups = outcome(vec![player(1, "a"), player(2, "b")], vec![player(2, "b")]);
        for result in [same_group, both_groups] {
            let mut stats = Stats::default();
            let before = stats.clone();
            let err = stats.apply(&result).unwrap_err();
            assert!(matches!(err, ResultError::DuplicateParticipant(_)));
            assert_eq!(stats, before);
            assert!(!stats.is_dirty());
        }
    }

    #[test]
    fn rejection_keeps_existing_dirty_state_and_data() {
        let mut stats = Stats::default();
        stats.apply(&outcome(vec![player(1, "a")], vec![])).unwrap();
        stats.clear_dirty();
        let before = stats.clone();

        let bad = outcome(vec![player(1, "x"), player(3, "c")], vec![player(3, "c")]);
        assert!(stats.apply(&bad).is_err());
        assert_eq!(stats, before);
        assert!(!stats.is_dirty());
    }
}
