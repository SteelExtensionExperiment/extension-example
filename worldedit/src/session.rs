//! Per-player selections, clipboards and edit history.
use crate::edit::{ChangeSet, Clipboard};
use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard, PoisonError},
};
use steel_extension_api::{BlockPos, players::Uuid};
use worldedit_api::Cuboid;

/// One selection corner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Corner {
    /// World key.
    pub world: String,
    /// Block position.
    pub pos: BlockPos,
}

/// A player's WorldEdit state.
#[derive(Debug, Default)]
pub struct Session {
    /// First corner.
    pub pos1: Option<Corner>,
    /// Second corner.
    pub pos2: Option<Corner>,
    /// Copied blocks.
    pub clipboard: Option<Clipboard>,
    /// Applied edits, oldest first.
    pub history: Vec<ChangeSet>,
    /// Undone edits, most recently undone last.
    pub redo: Vec<ChangeSet>,
}

impl Session {
    /// The cuboid between both corners, if both are set in the same world.
    pub fn selection(&self) -> Option<Cuboid> {
        let (a, b) = (self.pos1.as_ref()?, self.pos2.as_ref()?);
        (a.world == b.world).then(|| Cuboid::from_corners(&a.world, a.pos, b.pos))
    }

    /// Replaces the selection with `cuboid`.
    pub fn select(&mut self, cuboid: &Cuboid) {
        let world = cuboid.world.to_string();
        self.pos1 = Some(Corner {
            world: world.clone(),
            pos: cuboid.min,
        });
        self.pos2 = Some(Corner {
            world,
            pos: cuboid.max,
        });
    }

    /// Records an edit, dropping the oldest beyond `limit` and clearing redo.
    pub fn remember(&mut self, set: ChangeSet, limit: usize) {
        if set.changes.is_empty() {
            return;
        }
        push_bounded(&mut self.history, set, limit);
        self.redo.clear();
    }
}

/// Appends `item`, dropping the oldest entries beyond `limit`.
pub fn push_bounded<T>(list: &mut Vec<T>, item: T, limit: usize) {
    list.push(item);
    if list.len() > limit {
        let excess = list.len() - limit;
        list.drain(..excess);
    }
}

/// Every session, keyed by player.
#[derive(Default)]
pub struct Sessions(Mutex<BTreeMap<Uuid, Session>>);

impl Sessions {
    /// Locks all sessions.
    pub fn lock(&self) -> MutexGuard<'_, BTreeMap<Uuid, Session>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_history_keeps_the_newest_entries() {
        let mut history = Vec::new();
        for n in 0..5 {
            push_bounded(&mut history, n, 3);
        }
        assert_eq!(history, [2, 3, 4]);
    }

    #[test]
    fn empty_edits_are_not_recorded() {
        let mut session = Session::default();
        session.redo.push(ChangeSet::default());
        session.remember(ChangeSet::default(), 3);
        assert!(session.history.is_empty());
        assert_eq!(session.redo.len(), 1);
    }

    #[test]
    fn selection_requires_both_corners_in_one_world() {
        let mut session = Session {
            pos1: Some(Corner {
                world: "a".into(),
                pos: BlockPos { x: 3, y: 0, z: 0 },
            }),
            ..Session::default()
        };
        assert!(session.selection().is_none());
        session.pos2 = Some(Corner {
            world: "b".into(),
            pos: BlockPos { x: 0, y: 0, z: 0 },
        });
        assert!(session.selection().is_none());
        session.pos2 = Some(Corner {
            world: "a".into(),
            pos: BlockPos { x: 0, y: 5, z: 0 },
        });
        assert_eq!(
            session.selection().map(|cuboid| cuboid.volume()),
            Some(4 * 6)
        );
    }
}
