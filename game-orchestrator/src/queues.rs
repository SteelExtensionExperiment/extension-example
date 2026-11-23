//! One queue per game type, with the rule that a player is in at most one queue.

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    hash::Hash,
};

/// What `join` did.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Joined {
    /// Queued at the 1-based `position`; `moved_from` is the queue the player left, if any.
    Queued {
        position: usize,
        moved_from: Option<String>,
    },
    /// Already in this queue; nothing changed.
    AlreadyQueued,
}

/// Players taken from the front of one queue for one match request.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "called once start_match can carry a kit per player (host spec 007)"
    )
)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Formed<P> {
    pub game_type: String,
    pub players: Vec<P>,
}

/// Both maps live in one struct so they cannot disagree.
#[derive(Debug)]
pub(crate) struct Queues<P> {
    /// Name order makes formation order deterministic.
    by_game_type: BTreeMap<String, VecDeque<P>>,
    of_player: HashMap<P, String>,
}

impl<P> Default for Queues<P> {
    fn default() -> Self {
        Self {
            by_game_type: BTreeMap::new(),
            of_player: HashMap::new(),
        }
    }
}

impl<P: Copy + Eq + Hash> Queues<P> {
    pub(crate) fn join(&mut self, player: P, game_type: &str) -> Joined {
        if self
            .of_player
            .get(&player)
            .is_some_and(|current| current == game_type)
        {
            return Joined::AlreadyQueued;
        }
        let moved_from = self.leave(player);
        let queue = self.by_game_type.entry(game_type.to_owned()).or_default();
        queue.push_back(player);
        self.of_player.insert(player, game_type.to_owned());
        Joined::Queued {
            position: queue.len(),
            moved_from,
        }
    }

    /// Removes the player from their queue (leave, quit), returning its game type.
    pub(crate) fn leave(&mut self, player: P) -> Option<String> {
        let game_type = self.of_player.remove(&player)?;
        if let Some(queue) = self.by_game_type.get_mut(&game_type) {
            queue.retain(|queued| *queued != player);
            if queue.is_empty() {
                self.by_game_type.remove(&game_type);
            }
        }
        Some(game_type)
    }

    /// Puts `players` back at the front of the queue in their original order. A player already
    /// queued elsewhere is moved, so the one-queue rule holds.
    pub(crate) fn push_front(&mut self, game_type: &str, players: &[P]) {
        for player in players {
            self.leave(*player);
        }
        let queue = self.by_game_type.entry(game_type.to_owned()).or_default();
        for player in players.iter().rev() {
            queue.push_front(*player);
            self.of_player.insert(*player, game_type.to_owned());
        }
    }

    pub(crate) fn position(&self, player: P) -> Option<(&str, usize)> {
        let game_type = self.of_player.get(&player)?;
        let index = self
            .by_game_type
            .get(game_type)?
            .iter()
            .position(|queued| *queued == player)?;
        Some((game_type, index + 1))
    }

    pub(crate) fn queue(&self, game_type: &str) -> impl Iterator<Item = P> + '_ {
        self.by_game_type
            .get(game_type)
            .into_iter()
            .flatten()
            .copied()
    }

    /// Empties one queue, returning its players in order.
    pub(crate) fn clear_game_type(&mut self, game_type: &str) -> Vec<P> {
        let Some(queue) = self.by_game_type.remove(game_type) else {
            return Vec::new();
        };
        for player in &queue {
            self.of_player.remove(player);
        }
        queue.into()
    }

    /// Empties every queue, returning all players.
    pub(crate) fn clear_all(&mut self) -> Vec<P> {
        self.of_player.clear();
        std::mem::take(&mut self.by_game_type)
            .into_values()
            .flatten()
            .collect()
    }

    /// Pops the first N players of every queue that holds at least N, in game type name order.
    /// `players_needed` gives N per game type; `None` or 0 leaves that queue alone.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "called once start_match can carry a kit per player (host spec 007)"
        )
    )]
    pub(crate) fn form(
        &mut self,
        players_needed: impl Fn(&str) -> Option<usize>,
    ) -> Vec<Formed<P>> {
        let mut formed = Vec::new();
        for (game_type, queue) in &mut self.by_game_type {
            let Some(needed) = players_needed(game_type).filter(|needed| *needed > 0) else {
                continue;
            };
            while queue.len() >= needed {
                let players: Vec<P> = queue.drain(..needed).collect();
                for player in &players {
                    self.of_player.remove(player);
                }
                formed.push(Formed {
                    game_type: game_type.clone(),
                    players,
                });
            }
        }
        self.by_game_type.retain(|_, queue| !queue.is_empty());
        formed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn needs(n: usize) -> impl Fn(&str) -> Option<usize> {
        move |_| Some(n)
    }

    #[test]
    fn join_moves_between_queues_and_never_duplicates() {
        let mut queues = Queues::default();
        assert_eq!(
            queues.join(1, "a"),
            Joined::Queued {
                position: 1,
                moved_from: None
            }
        );
        assert_eq!(queues.join(1, "a"), Joined::AlreadyQueued);
        assert_eq!(
            queues.join(1, "b"),
            Joined::Queued {
                position: 1,
                moved_from: Some("a".into())
            }
        );
        assert_eq!(queues.queue("a").count(), 0);
        assert_eq!(queues.position(1), Some(("b", 1)));
    }

    #[test]
    fn formation_takes_first_n_and_keeps_the_rest_in_order() {
        let mut queues = Queues::default();
        for player in 1..=5 {
            queues.join(player, "a");
        }
        let formed = queues.form(needs(2));
        assert_eq!(
            formed,
            vec![
                Formed {
                    game_type: "a".into(),
                    players: vec![1, 2]
                },
                Formed {
                    game_type: "a".into(),
                    players: vec![3, 4]
                },
            ]
        );
        assert_eq!(queues.queue("a").collect::<Vec<_>>(), vec![5]);
        assert_eq!(queues.position(1), None);
    }

    #[test]
    fn two_game_types_complete_in_name_order() {
        let mut queues = Queues::default();
        queues.join(1, "b");
        queues.join(2, "a");
        queues.join(3, "b");
        queues.join(4, "a");
        let order: Vec<_> = queues
            .form(needs(2))
            .into_iter()
            .map(|formed| formed.game_type)
            .collect();
        assert_eq!(order, ["a", "b"]);
    }

    #[test]
    fn requeue_at_front_keeps_original_order() {
        let mut queues = Queues::default();
        queues.join(9, "a");
        queues.push_front("a", &[1, 2]);
        assert_eq!(queues.queue("a").collect::<Vec<_>>(), vec![1, 2, 9]);
        assert_eq!(queues.position(9), Some(("a", 3)));
    }

    #[test]
    fn changed_player_count_is_applied_on_next_formation() {
        let mut queues = Queues::default();
        for player in 1..=3 {
            queues.join(player, "a");
        }
        assert_eq!(queues.form(needs(4)), vec![]);
        assert_eq!(queues.form(needs(3)).len(), 1);
    }

    #[test]
    fn quit_and_clear_remove_players() {
        let mut queues = Queues::default();
        queues.join(1, "a");
        queues.join(2, "a");
        queues.join(3, "b");
        assert_eq!(queues.leave(1), Some("a".into()));
        assert_eq!(queues.clear_game_type("a"), vec![2]);
        assert_eq!(queues.position(2), None);
        assert_eq!(queues.clear_all(), vec![3]);
    }
}
