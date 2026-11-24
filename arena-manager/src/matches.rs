//! Match state machine. Pure: the caller feeds events and executes the returned [`Effect`]s.
//!
//! ```text
//! Preparing -> Starting -> Running -> Ending -> Finished
//!      \___________\___________________________/-> Failed (before Running)
//! ```
//!
//! Deaths and quits are resolved in batches drained once per server tick (research R8), so
//! "simultaneous" means "same batch" and the outcome does not depend on handler order.

use std::num::NonZeroU32;

use arena_manager_api::Outcome;

/// Why a player left the match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaveKind {
    Death,
    Quit,
}

/// A queued death or disconnect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerEvent<K> {
    pub player: K,
    pub kind: LeaveKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Preparing,
    Starting,
    Running,
    /// Winner decided; counting down to the move to the lobby.
    Ending {
        remaining_ticks: u32,
    },
    Finished,
    Failed,
}

/// Work the caller must carry out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect<K> {
    /// Move a dead or disconnected player to the lobby now.
    SendToLobby(K),
    /// Show the winner the seconds left before the move to the lobby.
    Countdown { winner: K, seconds: u32 },
    /// Countdown elapsed: move the winner to the lobby.
    MoveWinnerToLobby(K),
    /// Nothing is left to play: the arena can be reused.
    ReleaseArena,
    /// Final result, emitted exactly once, one entry per participant in join order.
    Finished(Vec<(K, Outcome)>),
    /// The match was cancelled before it ran; no outcomes. Emitted exactly once.
    Failed { message: String },
}

#[derive(Debug)]
struct Participant<K> {
    key: K,
    placed: bool,
    alive: bool,
    outcome: Option<Outcome>,
}

#[derive(Debug)]
pub struct Match<K> {
    state: State,
    players: Vec<Participant<K>>,
    ticks_per_second: NonZeroU32,
    winner_return_delay_seconds: u32,
}

impl<K: Copy + PartialEq> Match<K> {
    /// A match in `Preparing`. `None` if `ticks_per_second` is zero.
    #[must_use]
    pub fn new(
        players: impl IntoIterator<Item = K>,
        ticks_per_second: u32,
        winner_return_delay_seconds: u32,
    ) -> Option<Self> {
        Some(Self {
            state: State::Preparing,
            players: players
                .into_iter()
                .map(|key| Participant {
                    key,
                    placed: false,
                    alive: true,
                    outcome: None,
                })
                .collect(),
            ticks_per_second: NonZeroU32::new(ticks_per_second)?,
            winner_return_delay_seconds,
        })
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    /// Whether `player` is still in the match.
    #[must_use]
    pub fn is_alive(&self, player: K) -> bool {
        self.players.iter().any(|p| p.key == player && p.alive)
    }

    /// The arena world exists: `Preparing` -> `Starting`.
    pub fn world_ready(&mut self) {
        if self.state == State::Preparing {
            self.state = State::Starting;
        }
    }

    /// A player stands at their spawn. When all are placed, `Starting` -> `Running`.
    pub fn player_placed(&mut self, player: K) {
        if self.state != State::Starting {
            return;
        }
        if let Some(participant) = self.players.iter_mut().find(|p| p.key == player) {
            participant.placed = true;
        }
        if self.players.iter().all(|p| p.placed) {
            self.state = State::Running;
        }
    }

    /// Cancels a match that has not started running (clone failed, player not ready).
    pub fn fail(&mut self, message: impl Into<String>) -> Vec<Effect<K>> {
        if !matches!(self.state, State::Preparing | State::Starting) {
            return Vec::new();
        }
        self.state = State::Failed;
        vec![
            Effect::Failed {
                message: message.into(),
            },
            Effect::ReleaseArena,
        ]
    }

    /// Resolves one drained batch of deaths and quits.
    ///
    /// Before `Running` any event of a participant cancels the match. Once a winner is decided
    /// further events are ignored: the outcomes are final.
    pub fn resolve(&mut self, events: &[PlayerEvent<K>]) -> Vec<Effect<K>> {
        match self.state {
            State::Preparing | State::Starting => {
                if events.iter().any(|e| self.is_participant(e.player)) {
                    return self.fail("a player died or left before the match started");
                }
                Vec::new()
            }
            State::Running => self.resolve_running(events),
            State::Ending { .. } | State::Finished | State::Failed => Vec::new(),
        }
    }

    /// One server tick has passed.
    pub fn tick(&mut self) -> Vec<Effect<K>> {
        let State::Ending { remaining_ticks } = self.state else {
            return Vec::new();
        };
        let remaining_ticks = remaining_ticks.saturating_sub(1);
        self.state = State::Ending { remaining_ticks };
        let mut effects = Vec::new();
        if remaining_ticks == 0 {
            self.finish(&mut effects);
            return effects;
        }
        if let (Some(winner), true) = (
            self.winner(),
            remaining_ticks.is_multiple_of(self.ticks_per_second.get()),
        ) {
            effects.push(Effect::Countdown {
                winner,
                seconds: remaining_ticks / self.ticks_per_second,
            });
        }
        effects
    }

    fn is_participant(&self, player: K) -> bool {
        self.players.iter().any(|p| p.key == player)
    }

    fn winner(&self) -> Option<K> {
        self.players
            .iter()
            .find(|p| p.outcome == Some(Outcome::Won))
            .map(|p| p.key)
    }

    fn resolve_running(&mut self, events: &[PlayerEvent<K>]) -> Vec<Effect<K>> {
        let mut effects = Vec::new();
        let mut died = Vec::new();
        for event in events {
            let Some(participant) = self
                .players
                .iter_mut()
                .find(|p| p.key == event.player && p.alive)
            else {
                continue;
            };
            participant.alive = false;
            participant.outcome = Some(Outcome::Lost);
            if event.kind == LeaveKind::Death {
                died.push(event.player);
            }
            effects.push(Effect::SendToLobby(event.player));
        }
        if effects.is_empty() {
            return effects;
        }

        let mut survivors = self.players.iter().filter(|p| p.alive).map(|p| p.key);
        let (first, second) = (survivors.next(), survivors.next());
        match (first, second) {
            (Some(_), Some(_)) => {}
            (Some(winner), None) => self.begin_ending(winner, &mut effects),
            // Everyone left in this batch: those who died together draw, quitters stay lost.
            (None, _) => {
                for participant in self.players.iter_mut().filter(|p| died.contains(&p.key)) {
                    participant.outcome = Some(Outcome::Draw);
                }
                self.finish(&mut effects);
            }
        }
        effects
    }

    fn begin_ending(&mut self, winner: K, effects: &mut Vec<Effect<K>>) {
        if let Some(participant) = self.players.iter_mut().find(|p| p.key == winner) {
            participant.outcome = Some(Outcome::Won);
        }
        let remaining_ticks = self
            .winner_return_delay_seconds
            .saturating_mul(self.ticks_per_second.get());
        self.state = State::Ending { remaining_ticks };
        if remaining_ticks == 0 {
            self.finish(effects);
            return;
        }
        effects.push(Effect::Countdown {
            winner,
            seconds: self.winner_return_delay_seconds,
        });
    }

    /// Moves the winner (if any), releases the arena and emits the result.
    fn finish(&mut self, effects: &mut Vec<Effect<K>>) {
        self.state = State::Finished;
        if let Some(winner) = self.winner() {
            effects.push(Effect::MoveWinnerToLobby(winner));
        }
        effects.push(Effect::ReleaseArena);
        effects.push(Effect::Finished(
            self.players
                .iter()
                .filter_map(|p| p.outcome.map(|outcome| (p.key, outcome)))
                .collect(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TPS: u32 = 4;
    const DELAY_SECONDS: u32 = 2;

    fn death(player: u8) -> PlayerEvent<u8> {
        PlayerEvent {
            player,
            kind: LeaveKind::Death,
        }
    }

    fn quit(player: u8) -> PlayerEvent<u8> {
        PlayerEvent {
            player,
            kind: LeaveKind::Quit,
        }
    }

    fn running(players: &[u8], delay_seconds: u32) -> Match<u8> {
        let mut m = Match::new(players.iter().copied(), TPS, delay_seconds).unwrap();
        m.world_ready();
        for &p in players {
            m.player_placed(p);
        }
        assert_eq!(m.state(), State::Running);
        m
    }

    fn finished_result(effects: &[Effect<u8>]) -> Option<&Vec<(u8, Outcome)>> {
        effects.iter().find_map(|e| match e {
            Effect::Finished(result) => Some(result),
            _ => None,
        })
    }

    /// Ticks until the match is no longer `Ending`, collecting every effect.
    fn run_countdown(m: &mut Match<u8>) -> Vec<Effect<u8>> {
        let mut all = Vec::new();
        while matches!(m.state(), State::Ending { .. }) {
            all.extend(m.tick());
        }
        all
    }

    #[test]
    fn rejects_zero_tick_rate() {
        assert!(Match::new([1u8, 2], 0, DELAY_SECONDS).is_none());
    }

    #[test]
    fn starts_running_only_when_everyone_is_placed() {
        let mut m = Match::new([1u8, 2], TPS, DELAY_SECONDS).unwrap();
        m.player_placed(1);
        assert_eq!(m.state(), State::Preparing);
        m.world_ready();
        m.player_placed(1);
        assert_eq!(m.state(), State::Starting);
        m.player_placed(2);
        assert_eq!(m.state(), State::Running);
    }

    #[test]
    fn one_death_in_two_players_ends_with_won_and_lost() {
        let mut m = running(&[1, 2], DELAY_SECONDS);
        let effects = m.resolve(&[death(1)]);
        assert_eq!(
            effects,
            [
                Effect::SendToLobby(1),
                Effect::Countdown {
                    winner: 2,
                    seconds: DELAY_SECONDS
                }
            ]
        );
        let rest = run_countdown(&mut m);
        assert_eq!(
            finished_result(&rest),
            Some(&vec![(1, Outcome::Lost), (2, Outcome::Won)])
        );
        assert_eq!(m.state(), State::Finished);
    }

    #[test]
    fn simultaneous_death_in_one_batch_is_a_draw_without_countdown() {
        let mut m = running(&[1, 2], DELAY_SECONDS);
        let effects = m.resolve(&[death(1), death(2)]);
        assert_eq!(m.state(), State::Finished);
        assert!(effects.contains(&Effect::ReleaseArena));
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::Countdown { .. } | Effect::MoveWinnerToLobby(_)))
        );
        assert_eq!(
            finished_result(&effects),
            Some(&vec![(1, Outcome::Draw), (2, Outcome::Draw)])
        );
    }

    #[test]
    fn deaths_in_separate_batches_are_not_a_draw() {
        let mut m = running(&[1, 2], DELAY_SECONDS);
        m.resolve(&[death(1)]);
        let rest = m.resolve(&[death(2)]);
        assert!(rest.is_empty(), "winner is already decided");
        assert_eq!(
            finished_result(&run_countdown(&mut m)),
            Some(&vec![(1, Outcome::Lost), (2, Outcome::Won)])
        );
    }

    #[test]
    fn last_two_dying_together_draw_but_earlier_death_stays_lost() {
        let mut m = running(&[1, 2, 3], DELAY_SECONDS);
        assert_eq!(m.resolve(&[death(1)]), [Effect::SendToLobby(1)]);
        let effects = m.resolve(&[death(2), death(3)]);
        assert_eq!(
            finished_result(&effects),
            Some(&vec![
                (1, Outcome::Lost),
                (2, Outcome::Draw),
                (3, Outcome::Draw)
            ])
        );
    }

    #[test]
    fn quit_counts_as_loss_and_leaves_the_other_as_winner() {
        let mut m = running(&[1, 2], DELAY_SECONDS);
        m.resolve(&[quit(1)]);
        assert_eq!(
            finished_result(&run_countdown(&mut m)),
            Some(&vec![(1, Outcome::Lost), (2, Outcome::Won)])
        );
    }

    #[test]
    fn quits_never_become_a_draw() {
        let mut m = running(&[1, 2], DELAY_SECONDS);
        let effects = m.resolve(&[quit(1), quit(2)]);
        assert_eq!(
            finished_result(&effects),
            Some(&vec![(1, Outcome::Lost), (2, Outcome::Lost)])
        );
        let mut m = running(&[1, 2], DELAY_SECONDS);
        let effects = m.resolve(&[death(1), quit(2)]);
        assert_eq!(
            finished_result(&effects),
            Some(&vec![(1, Outcome::Draw), (2, Outcome::Lost)])
        );
    }

    #[test]
    fn three_players_last_standing_wins_and_dead_return_at_once() {
        let mut m = running(&[1, 2, 3], DELAY_SECONDS);
        assert_eq!(m.resolve(&[death(2)]), [Effect::SendToLobby(2)]);
        assert_eq!(m.state(), State::Running);
        let effects = m.resolve(&[quit(1)]);
        assert_eq!(effects[0], Effect::SendToLobby(1));
        assert!(matches!(
            effects[1],
            Effect::Countdown {
                winner: 3,
                seconds: DELAY_SECONDS
            }
        ));
        assert_eq!(
            finished_result(&run_countdown(&mut m)),
            Some(&vec![
                (1, Outcome::Lost),
                (2, Outcome::Lost),
                (3, Outcome::Won)
            ])
        );
    }

    #[test]
    fn death_or_quit_before_running_cancels_without_outcomes() {
        for kind in [LeaveKind::Death, LeaveKind::Quit] {
            let event = PlayerEvent { player: 1, kind };
            let mut preparing = Match::new([1u8, 2], TPS, DELAY_SECONDS).unwrap();
            let mut starting = Match::new([1u8, 2], TPS, DELAY_SECONDS).unwrap();
            starting.world_ready();
            for m in [&mut preparing, &mut starting] {
                let effects = m.resolve(&[event]);
                assert_eq!(m.state(), State::Failed);
                assert!(matches!(effects[0], Effect::Failed { .. }));
                assert!(effects.contains(&Effect::ReleaseArena));
                assert!(finished_result(&effects).is_none());
            }
        }
    }

    #[test]
    fn strangers_before_running_do_not_cancel() {
        let mut m = Match::new([1u8, 2], TPS, DELAY_SECONDS).unwrap();
        assert_eq!(m.resolve(&[death(9)]), []);
        assert_eq!(m.state(), State::Preparing);
    }

    #[test]
    fn countdown_shows_each_second_and_arena_is_released_only_when_it_elapses() {
        let mut m = running(&[1, 2], DELAY_SECONDS);
        let start = m.resolve(&[death(1)]);
        assert!(!start.contains(&Effect::ReleaseArena));

        let mut shown = Vec::new();
        for tick in 1..=(DELAY_SECONDS * TPS) {
            let effects = m.tick();
            let released = effects.contains(&Effect::ReleaseArena);
            if tick < DELAY_SECONDS * TPS {
                assert!(!released, "released early at tick {tick}");
                shown.extend(effects.into_iter().filter_map(|e| match e {
                    Effect::Countdown { seconds, .. } => Some(seconds),
                    _ => None,
                }));
            } else {
                assert!(released);
                assert_eq!(
                    effects,
                    [
                        Effect::MoveWinnerToLobby(2),
                        Effect::ReleaseArena,
                        Effect::Finished(vec![(1, Outcome::Lost), (2, Outcome::Won)])
                    ]
                );
            }
        }
        assert_eq!(shown, [1], "the initial effect showed 2");
        assert_eq!(m.state(), State::Finished);
    }

    #[test]
    fn zero_delay_moves_the_winner_and_releases_at_once() {
        let mut m = running(&[1, 2], 0);
        let effects = m.resolve(&[death(1)]);
        assert_eq!(m.state(), State::Finished);
        assert!(effects.contains(&Effect::MoveWinnerToLobby(2)));
        assert!(effects.contains(&Effect::ReleaseArena));
    }

    #[test]
    fn result_is_emitted_exactly_once_and_later_events_are_ignored() {
        let mut m = running(&[1, 2], DELAY_SECONDS);
        let mut all = m.resolve(&[death(1), death(1), death(9)]);
        all.extend(run_countdown(&mut m));
        all.extend(m.resolve(&[death(2), quit(1)]));
        all.extend(m.tick());
        all.extend(m.fail("late"));
        assert_eq!(
            all.iter()
                .filter(|e| matches!(e, Effect::Finished(_)))
                .count(),
            1
        );
        assert_eq!(
            all.iter().filter(|e| **e == Effect::ReleaseArena).count(),
            1
        );
        assert!(!all.iter().any(|e| matches!(e, Effect::Failed { .. })));
    }

    #[test]
    fn failure_is_reported_once() {
        let mut m = Match::new([1u8, 2], TPS, DELAY_SECONDS).unwrap();
        assert!(matches!(m.fail("clone failed")[0], Effect::Failed { .. }));
        assert_eq!(m.fail("again"), []);
        assert_eq!(m.resolve(&[death(1)]), []);
        assert_eq!(m.state(), State::Failed);
    }
}
