//! Calls to arena-manager: building the request with a kit per player, the capacity retry and
//! the handling of rejections. Runs on the tick thread; nothing here waits.

use arena_manager_api::{MatchesDyn as _, PlayerEntry, StartRequest};
use steel_extension_api::{Server, log, players::Uuid, scheduler};

use crate::host::{self, deliver, tell};

/// Arena-manager rejects a request for capacity with this text (its spec D7). The contract has no
/// typed cause, so the retry decision depends on this wording.
const CAPACITY_REJECTION: &str = "no capacity";

/// Forms matches from every full queue and submits them in game type order.
pub(crate) fn pump(server: &Server<'_>) {
    let formed = match host::with(crate::orchestrator::Orchestrator::form_requests) {
        Ok(formed) => formed,
        Err(error) => {
            log::error(error);
            return;
        }
    };
    for request in formed {
        submit(server, &request.game_type, request.players);
    }
}

/// Where a request ended up, decided after the state lock is released.
enum Outcome {
    Started(u64),
    /// No capacity: wait and retry.
    Busy,
    /// Players are told and go back to the front of the queue.
    Refused(String),
}

/// Asks arena-manager for a match. `players` are out of every queue and match when this runs.
pub(crate) fn submit(server: &Server<'_>, game_type: &str, players: Vec<Uuid>) {
    let prepared = host::with(|orchestrator| orchestrator.prepare_request(game_type, &players));
    let entries = match prepared {
        Ok(Ok(entries)) => entries,
        Ok(Err(notices)) => {
            deliver(server, notices);
            return;
        }
        Err(error) => {
            log::error(error);
            return;
        }
    };
    let outcome = match arena_manager_api::matches() {
        Err(_) => Outcome::Refused("arena manager not available".to_owned()),
        Ok(arena) => {
            let request = StartRequest {
                game_type: game_type.into(),
                players: entries
                    .into_iter()
                    .map(|(uuid, kit)| PlayerEntry { uuid, kit })
                    .collect(),
            };
            arena
                .start_match(*server, request)
                .match_owned(Outcome::Started, |message| {
                    let message = message.to_string();
                    if message.contains(CAPACITY_REJECTION) {
                        Outcome::Busy
                    } else {
                        Outcome::Refused(message)
                    }
                })
        }
    };
    settle(server, game_type, players, outcome);
}

fn settle(server: &Server<'_>, game_type: &str, players: Vec<Uuid>, outcome: Outcome) {
    match outcome {
        Outcome::Started(id) => {
            let announced = players.clone();
            let _ = host::with(|orchestrator| orchestrator.begin_match(id, game_type, players));
            for player in announced {
                tell(server, player, &format!("&amatch found for {game_type}"));
            }
        }
        Outcome::Busy => hold_and_retry(server, game_type, players),
        Outcome::Refused(message) => {
            let notices =
                host::with(|orchestrator| orchestrator.refuse(game_type, players, &message));
            match notices {
                Ok(notices) => deliver(server, notices),
                Err(error) => log::error(error),
            }
        }
    }
}

fn hold_and_retry(server: &Server<'_>, game_type: &str, players: Vec<Uuid>) {
    let first = players.clone();
    let held = host::with(|orchestrator| {
        let interval = u64::from(orchestrator.config.retry_interval_ticks);
        (orchestrator.hold(game_type, players), interval)
    });
    let Ok((id, interval)) = held else {
        return;
    };
    for player in first {
        tell(
            server,
            player,
            "&eall arenas are busy, you will start when one is free",
        );
    }
    scheduler::run_later(interval, move |server| retry(server, id));
}

fn retry(server: &Server<'_>, id: u64) {
    let Ok(Some(held)) = host::with(|orchestrator| orchestrator.take_pending(id)) else {
        return;
    };
    submit(server, &held.game_type, held.players);
}
