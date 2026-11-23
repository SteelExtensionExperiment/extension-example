//! Match results from the orchestrator into stats.

use std::sync::Mutex;

use game_orchestrator_api::{MatchResult, Outcome};
use steel_extension_api::{Hooks, log, scheduler, stabby};

use crate::game_types::{GameTypes, OrchestratorGames, check_game_type};
use crate::state::{self, lock};
use crate::stats::{MatchOutcome, Participant, Stats};

/// Maps the orchestrator result. A `Draw` participant is ignored: neither a win nor a loss.
fn to_match_outcome(result: &MatchResult) -> MatchOutcome {
    let mut outcome = MatchOutcome {
        game_type: result.game_type.to_string(),
        winners: Vec::new(),
        losers: Vec::new(),
    };
    for participant in &result.participants {
        let target = match participant.outcome {
            Outcome::Won => &mut outcome.winners,
            Outcome::Lost => &mut outcome.losers,
            Outcome::Draw => continue,
        };
        target.push(Participant {
            uuid: participant.uuid,
            name: participant.name.to_string(),
        });
    }
    outcome
}

/// Applies a result if its game type exists. Returns the reason when it is rejected, with
/// nothing changed; the caller logs it, so no host call happens under the stats lock.
fn record_result(
    source: &impl GameTypes,
    stats: &Mutex<Stats>,
    outcome: &MatchOutcome,
) -> Result<(), String> {
    check_game_type(source, &outcome.game_type)
        .map_err(|error| format!("Ignored result: {error}"))?;
    lock(stats)
        .apply(outcome)
        .map_err(|error| format!("Rejected {} result: {error}", outcome.game_type))
}

#[stabby::export(callback)]
fn on_match_result(result: &MatchResult) {
    let outcome = to_match_outcome(result);
    // Validating the game type needs a `Server`, which the handler does not get. A scheduler
    // task runs on the tick thread with one, on the next tick.
    scheduler::run_later(1, move |server| {
        let Ok(state) = state::state() else {
            log::error("Dropped a match result: stats visualizer is not initialized");
            return;
        };
        if let Err(error) = record_result(&OrchestratorGames(*server), &state.stats, &outcome) {
            log::error(error);
        }
        // Refreshing the boards of this game type hooks in here once boards exist (T006).
    });
}

/// Subscribes to match results in pre-init.
///
/// # Errors
/// A message if the host refuses the subscription.
pub fn register(hooks: &mut Hooks<'_>) -> Result<(), String> {
    hooks
        .add_action::<MatchResult>(on_match_result)
        .register()
        .match_owned(Ok, |error| Err(error.to_string()))
}

#[cfg(test)]
mod tests {
    use steel_extension_api::players::Uuid;
    use steel_extension_api::stabby::{string::String as ApiString, vec::Vec as ApiVec};

    use super::*;
    use crate::game_types::tests::FakeGames;

    fn uuid(n: u8) -> Uuid {
        Uuid { bytes: [n; 16] }
    }

    fn api_participant(n: u8, outcome: Outcome) -> game_orchestrator_api::Participant {
        game_orchestrator_api::Participant {
            uuid: uuid(n),
            name: ApiString::from(format!("p{n}").as_str()),
            outcome,
        }
    }

    fn result(game_type: &str, participants: Vec<(u8, Outcome)>) -> MatchResult {
        let mut list = ApiVec::new();
        for (n, outcome) in participants {
            list.push(api_participant(n, outcome));
        }
        MatchResult {
            game_type: ApiString::from(game_type),
            participants: list,
        }
    }

    fn bedwars_win(winner: u8) -> MatchOutcome {
        to_match_outcome(&result("bedwars", vec![(winner, Outcome::Won)]))
    }

    #[test]
    fn mapping_splits_outcomes_and_skips_draws() {
        let outcome = to_match_outcome(&result(
            "bedwars",
            vec![(1, Outcome::Won), (2, Outcome::Lost), (3, Outcome::Draw)],
        ));
        assert_eq!(outcome.game_type, "bedwars");
        assert_eq!(outcome.winners.len(), 1);
        assert_eq!(outcome.winners[0].name, "p1");
        assert_eq!(outcome.losers.len(), 1);
        assert_eq!(outcome.losers[0].uuid, uuid(2));
    }

    #[test]
    fn listed_type_is_recorded() {
        let stats = Mutex::new(Stats::default());
        let source = FakeGames(Some(vec!["bedwars"]));
        assert_eq!(record_result(&source, &stats, &bedwars_win(1)), Ok(()));
        assert_eq!(lock(&stats).players()[&uuid(1)].games["bedwars"].wins, 1);
    }

    #[test]
    fn unlisted_type_changes_nothing_and_is_reported() {
        let stats = Mutex::new(Stats::default());
        let source = FakeGames(Some(vec!["spleef"]));
        let error = record_result(&source, &stats, &bedwars_win(1)).unwrap_err();
        assert!(error.contains("unknown game type bedwars"), "{error}");
        assert!(lock(&stats).players().is_empty());
        assert!(!lock(&stats).is_dirty());
    }

    #[test]
    fn duplicate_uuid_changes_nothing_and_is_reported() {
        let stats = Mutex::new(Stats::default());
        let source = FakeGames(Some(vec!["bedwars"]));
        let outcome = to_match_outcome(&result(
            "bedwars",
            vec![(1, Outcome::Won), (1, Outcome::Lost)],
        ));
        let error = record_result(&source, &stats, &outcome).unwrap_err();
        assert!(error.contains("more than once"), "{error}");
        assert!(lock(&stats).players().is_empty());
    }
}
