//! What other extensions see: the `games` service, and arena-manager results turned into
//! `MatchResult` actions.

use arena_manager_api::{MatchFailed, MatchFinished};
use game_orchestrator_api::{Games, MatchResult, Participant};
use steel_extension_api::{
    Server, hooks, log, scheduler,
    stabby::{self, vec::Vec as ApiVec},
};

use crate::host::{self, deliver};

pub(crate) struct OrchestratorGames;

#[stabby::export(callback)]
impl Games for OrchestratorGames {
    extern "C" fn game_types(&self, _server: Server<'_>) -> ApiVec<stabby::string::String> {
        host::with(|orchestrator| {
            orchestrator
                .store
                .game_types
                .iter()
                .map(|found| found.name.as_str().into())
                .collect()
        })
        .unwrap_or_default()
    }
}

/// Frees the players and re-emits the result for consumers such as stats-visualizer. The state
/// lock is released before the action is dispatched, as the listeners run synchronously.
pub(crate) extern "C" fn on_match_finished(finished: &MatchFinished) {
    if let Err(error) = host::with(|orchestrator| orchestrator.finish(finished.id)) {
        log::error(error);
        return;
    }
    let result = MatchResult {
        game_type: finished.game_type.clone(),
        participants: finished
            .participants
            .iter()
            .map(|participant| Participant {
                uuid: participant.uuid,
                name: participant.name.clone(),
                outcome: participant.outcome,
            })
            .collect(),
    };
    hooks::do_action(&result).match_owned(
        |()| (),
        |error| log::error(format!("cannot emit the match result: {error}")),
    );
}

/// Tells and re-queues the players; no result is emitted. Action handlers have no server, so
/// the messages go out on the next tick.
pub(crate) extern "C" fn on_match_failed(failed: &MatchFailed) {
    let message = failed.message.to_string();
    let notices = match host::with(|orchestrator| orchestrator.fail(failed.id, &message)) {
        Ok(notices) => notices,
        Err(error) => {
            log::error(error);
            return;
        }
    };
    scheduler::run_later(1, move |server| deliver(server, notices));
}
