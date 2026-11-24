//! The arena worlds: cloned per match, removed once idle, counted toward the cap until fully gone.
//!
//! Every world and domain operation is polled once per call; nothing blocks.
use crate::allocator::{WorldReadiness, WorldView, idle_worlds};
use crate::host::{Host, OperationStatus};
use std::borrow::{Borrow, BorrowMut};
use std::collections::HashSet;

/// Prefix of world domains and key namespaces: `arena<n>`.
const DOMAIN_PREFIX: &str = "arena";
/// Map name of every cloned world: `arena<n>:map`.
const MAP_NAME: &str = "map";

/// What a world is waiting for. The `Retry*` steps repeat a call that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending<O> {
    Clone(O),
    Remove(O),
    RetryRemove,
    Destroy(O),
    RetryDestroy,
}

impl<O> Pending<O> {
    fn readiness(&self) -> WorldReadiness {
        match self {
            Self::Clone(_) => WorldReadiness::Creating,
            Self::Remove(_) | Self::RetryRemove => WorldReadiness::Removing,
            Self::Destroy(_) | Self::RetryDestroy => WorldReadiness::DestroyingDomain,
        }
    }
}

/// A world of this extension and its temporary domain.
#[derive(Debug)]
pub struct ArenaWorld<O> {
    pub(crate) view: WorldView,
    pub(crate) domain: String,
    /// `None` while the world is `Ready`.
    pub(crate) pending: Option<Pending<O>>,
}

impl<O> Borrow<WorldView> for ArenaWorld<O> {
    fn borrow(&self) -> &WorldView {
        &self.view
    }
}

impl<O> BorrowMut<WorldView> for ArenaWorld<O> {
    fn borrow_mut(&mut self) -> &mut WorldView {
        &mut self.view
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorldEvent {
    /// The clone finished; matches waiting on this world can start.
    Ready { key: String },
    /// The clone failed. The world is gone and no longer counts toward the cap.
    CreateFailed {
        key: String,
        game_type: String,
        message: String,
    },
    /// A removal step failed; it is retried on the next poll or cleanup.
    CleanupFailed { key: String, message: String },
}

#[derive(Debug)]
pub struct Worlds<O> {
    pub(crate) worlds: Vec<ArenaWorld<O>>,
    /// Never reused in the process: the host requires a never-used domain name.
    pub(crate) next_domain_id: u64,
}

impl<O> Default for Worlds<O> {
    fn default() -> Self {
        Self {
            worlds: Vec::new(),
            next_domain_id: 0,
        }
    }
}

impl<O: Copy> ArenaWorld<O> {
    fn set(&mut self, pending: Pending<O>) {
        self.view.readiness = pending.readiness();
        self.pending = Some(pending);
    }

    fn cleanup_failed(&self, message: String) -> WorldEvent {
        WorldEvent::CleanupFailed {
            key: self.view.key.clone(),
            message,
        }
    }

    /// The operation, or `None` after pushing `CleanupFailed`.
    fn attempt(&self, result: Result<O, String>, events: &mut Vec<WorldEvent>) -> Option<O> {
        result
            .map_err(|message| events.push(self.cleanup_failed(message)))
            .ok()
    }

    /// Advances the pending step once. Returns `false` once the world is gone.
    fn poll<H: Host<Operation = O>>(&mut self, host: &H, events: &mut Vec<WorldEvent>) -> bool {
        let Some(pending) = self.pending else {
            return true;
        };
        match pending {
            Pending::Clone(operation) => match status(host, operation) {
                OperationStatus::Pending => {}
                OperationStatus::Ready => {
                    self.view.readiness = WorldReadiness::Ready;
                    self.pending = None;
                    events.push(WorldEvent::Ready {
                        key: self.view.key.clone(),
                    });
                }
                OperationStatus::Failed(message) => {
                    events.push(WorldEvent::CreateFailed {
                        key: self.view.key.clone(),
                        game_type: self.view.game_type.clone(),
                        message,
                    });
                    return false;
                }
            },
            Pending::Remove(operation) => match status(host, operation) {
                OperationStatus::Pending => {}
                OperationStatus::Ready => self.destroy_domain(host, events),
                OperationStatus::Failed(message) => {
                    events.push(self.cleanup_failed(message));
                    self.set(Pending::RetryRemove);
                }
            },
            Pending::RetryRemove => {
                let result = host.remove_world(&self.view.key);
                let next = self
                    .attempt(result, events)
                    .map_or(Pending::RetryRemove, Pending::Remove);
                self.set(next);
            }
            Pending::Destroy(operation) => match status(host, operation) {
                OperationStatus::Pending => {}
                OperationStatus::Ready => return false,
                OperationStatus::Failed(message) => {
                    events.push(self.cleanup_failed(message));
                    self.set(Pending::RetryDestroy);
                }
            },
            Pending::RetryDestroy => self.destroy_domain(host, events),
        }
        true
    }

    fn destroy_domain<H: Host<Operation = O>>(&mut self, host: &H, events: &mut Vec<WorldEvent>) {
        let result = host.destroy_temporary_domain(&self.domain);
        let next = self
            .attempt(result, events)
            .map_or(Pending::RetryDestroy, Pending::Destroy);
        self.set(next);
    }
}

/// A lost operation (status read failed) counts as failed.
fn status<H: Host>(host: &H, operation: H::Operation) -> OperationStatus {
    host.operation_status(operation)
        .unwrap_or_else(OperationStatus::Failed)
}

impl<O: Copy> Worlds<O> {
    /// Clones `template` into a new world for `arena` and returns its key.
    ///
    /// The world starts `Creating` with `arena` reserved. A failed call changes nothing but the
    /// counter.
    pub fn request_world<H: Host<Operation = O>>(
        &mut self,
        host: &H,
        template: &str,
        game_type: &str,
        arena: &str,
    ) -> Result<String, String> {
        let domain = format!("{DOMAIN_PREFIX}{}", self.next_domain_id);
        self.next_domain_id += 1;
        let key = format!("{domain}:{MAP_NAME}");
        let operation = host.clone_world(template, &key)?;
        self.worlds.push(ArenaWorld {
            view: WorldView {
                key: key.clone(),
                game_type: game_type.to_owned(),
                readiness: WorldReadiness::Creating,
                busy: HashSet::from([arena.to_owned()]),
            },
            domain,
            pending: Some(Pending::Clone(operation)),
        });
        Ok(key)
    }

    /// Ends `arena`'s match in the world.
    pub fn release_arena(&mut self, world_key: &str, arena: &str) {
        if let Some(world) = self.world_mut(world_key) {
            world.view.busy.remove(arena);
        }
    }

    /// Advances every pending operation by one status read.
    pub fn poll<H: Host<Operation = O>>(&mut self, host: &H) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        self.worlds
            .retain_mut(|world| world.poll(host, &mut events));
        events
    }

    /// Starts removing every idle world.
    pub fn start_cleanup<H: Host<Operation = O>>(&mut self, host: &H) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        let idle: Vec<String> = idle_worlds(&self.worlds)
            .map(|world| world.key.clone())
            .collect();
        for key in idle {
            let Some(world) = self.world_mut(&key) else {
                continue;
            };
            let result = host.remove_world(&key);
            if let Some(operation) = world.attempt(result, &mut events) {
                world.set(Pending::Remove(operation));
            }
        }
        events
    }

    fn world_mut(&mut self, key: &str) -> Option<&mut ArenaWorld<O>> {
        self.worlds.iter_mut().find(|world| world.view.key == key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::fake::{FakeHost, Operation};

    const TEMPLATE: &str = "template";
    const GAME: &str = "duel";
    const ARENA: &str = "a";

    fn operation(worlds: &Worlds<Operation>, index: usize) -> Operation {
        match worlds.worlds[index].pending {
            Some(
                Pending::Clone(operation)
                | Pending::Remove(operation)
                | Pending::Destroy(operation),
            ) => operation,
            other => panic!("no operation in flight: {other:?}"),
        }
    }

    /// Finishes the first world's operation and polls once.
    fn finish_first(
        host: &FakeHost,
        worlds: &mut Worlds<Operation>,
        status: OperationStatus,
    ) -> Vec<WorldEvent> {
        host.finish(operation(worlds, 0), status);
        worlds.poll(host)
    }

    fn ready_world(host: &FakeHost, worlds: &mut Worlds<Operation>) -> String {
        let key = worlds.request_world(host, TEMPLATE, GAME, ARENA).unwrap();
        finish_first(host, worlds, OperationStatus::Ready);
        key
    }

    fn failed(message: &str) -> OperationStatus {
        OperationStatus::Failed(message.to_owned())
    }

    #[test]
    fn counter_is_never_reused_after_the_world_is_gone() {
        let host = FakeHost::default();
        let mut worlds = Worlds::default();
        let first = ready_world(&host, &mut worlds);
        assert_eq!(first, "arena0:map");
        worlds.release_arena(&first, ARENA);
        worlds.start_cleanup(&host);
        finish_first(&host, &mut worlds, OperationStatus::Ready);
        finish_first(&host, &mut worlds, OperationStatus::Ready);
        assert!(worlds.worlds.is_empty());

        let second = worlds.request_world(&host, TEMPLATE, GAME, ARENA).unwrap();
        assert_eq!(second, "arena1:map");
    }

    #[test]
    fn immediate_clone_failure_changes_nothing() {
        let host = FakeHost::default();
        host.fail("clone_world", "no cloner");
        let mut worlds = Worlds::default();
        let result = worlds.request_world(&host, TEMPLATE, GAME, ARENA);
        assert_eq!(result, Err("no cloner".to_owned()));
        assert!(worlds.worlds.is_empty());
    }

    #[test]
    fn async_clone_failure_drops_the_world() {
        let host = FakeHost::default();
        let mut worlds = Worlds::default();
        let key = worlds.request_world(&host, TEMPLATE, GAME, ARENA).unwrap();
        let events = finish_first(&host, &mut worlds, failed("disk full"));
        assert_eq!(
            events,
            [WorldEvent::CreateFailed {
                key,
                game_type: GAME.to_owned(),
                message: "disk full".to_owned()
            }]
        );
        assert!(worlds.worlds.is_empty());
    }

    #[test]
    fn lost_clone_operation_counts_as_failed() {
        let host = FakeHost::default();
        let mut worlds = Worlds::default();
        worlds.request_world(&host, TEMPLATE, GAME, ARENA).unwrap();
        host.state.borrow_mut().operations.clear();
        let events = worlds.poll(&host);
        assert!(matches!(events[..], [WorldEvent::CreateFailed { .. }]));
        assert!(worlds.worlds.is_empty());
    }

    #[test]
    fn clone_becomes_ready_once_finished() {
        let host = FakeHost::default();
        let mut worlds = Worlds::default();
        let key = worlds.request_world(&host, TEMPLATE, GAME, ARENA).unwrap();
        assert_eq!(worlds.worlds[0].view.readiness, WorldReadiness::Creating);
        assert!(worlds.worlds[0].view.busy.contains(ARENA));
        assert_eq!(worlds.poll(&host), []);
        assert_eq!(worlds.worlds[0].view.readiness, WorldReadiness::Creating);

        let events = finish_first(&host, &mut worlds, OperationStatus::Ready);
        assert_eq!(events, [WorldEvent::Ready { key }]);
        assert_eq!(worlds.worlds[0].view.readiness, WorldReadiness::Ready);
    }

    #[test]
    fn busy_world_is_not_cleaned() {
        let host = FakeHost::default();
        let mut worlds = Worlds::default();
        ready_world(&host, &mut worlds);
        assert_eq!(worlds.start_cleanup(&host), []);
        assert_eq!(host.calls(), ["clone_world(template,arena0:map)"]);
        assert_eq!(worlds.worlds[0].view.readiness, WorldReadiness::Ready);
    }

    #[test]
    fn idle_world_is_removed_then_its_domain_destroyed() {
        let host = FakeHost::default();
        let mut worlds = Worlds::default();
        let key = ready_world(&host, &mut worlds);
        worlds.release_arena(&key, ARENA);
        assert_eq!(worlds.start_cleanup(&host), []);
        assert_eq!(worlds.worlds[0].view.readiness, WorldReadiness::Removing);

        // Still counted toward the cap while its operations run.
        assert_eq!(worlds.poll(&host), []);
        assert_eq!(worlds.worlds.len(), 1);
        finish_first(&host, &mut worlds, OperationStatus::Ready);
        assert_eq!(
            worlds.worlds[0].view.readiness,
            WorldReadiness::DestroyingDomain
        );
        finish_first(&host, &mut worlds, OperationStatus::Ready);
        assert!(worlds.worlds.is_empty());
        assert_eq!(
            host.calls(),
            [
                "clone_world(template,arena0:map)",
                "remove_world(arena0:map)",
                "destroy_temporary_domain(arena0)",
            ]
        );
    }

    #[test]
    fn failed_remove_is_retried_and_keeps_counting() {
        let host = FakeHost::default();
        let mut worlds = Worlds::default();
        let key = ready_world(&host, &mut worlds);
        worlds.release_arena(&key, ARENA);
        worlds.start_cleanup(&host);

        let events = finish_first(&host, &mut worlds, failed("busy"));
        assert_eq!(
            events,
            [WorldEvent::CleanupFailed {
                key: key.clone(),
                message: "busy".to_owned()
            }]
        );
        assert_eq!(worlds.worlds.len(), 1);

        host.fail("remove_world", "still failing");
        let events = worlds.poll(&host);
        assert!(matches!(events[..], [WorldEvent::CleanupFailed { .. }]));
        assert_eq!(worlds.worlds[0].pending, Some(Pending::RetryRemove));

        host.state.borrow_mut().failing.clear();
        assert_eq!(worlds.poll(&host), []);
        finish_first(&host, &mut worlds, OperationStatus::Ready);
        finish_first(&host, &mut worlds, OperationStatus::Ready);
        assert!(worlds.worlds.is_empty());
        let removes = host
            .calls()
            .iter()
            .filter(|call| call.starts_with("remove_world"))
            .count();
        assert_eq!(removes, 3);
    }

    #[test]
    fn failed_destroy_is_retried_and_keeps_counting() {
        let host = FakeHost::default();
        let mut worlds = Worlds::default();
        let key = ready_world(&host, &mut worlds);
        worlds.release_arena(&key, ARENA);
        worlds.start_cleanup(&host);
        finish_first(&host, &mut worlds, OperationStatus::Ready);

        let events = finish_first(&host, &mut worlds, failed("in use"));
        assert!(matches!(events[..], [WorldEvent::CleanupFailed { .. }]));
        assert_eq!(worlds.worlds.len(), 1);

        assert_eq!(worlds.poll(&host), []);
        finish_first(&host, &mut worlds, OperationStatus::Ready);
        assert!(worlds.worlds.is_empty());
        let destroys = host
            .calls()
            .iter()
            .filter(|call| call.as_str() == "destroy_temporary_domain(arena0)")
            .count();
        assert_eq!(destroys, 2);
    }

    #[test]
    fn immediate_remove_failure_leaves_the_world_ready_for_retry() {
        let host = FakeHost::default();
        let mut worlds = Worlds::default();
        let key = ready_world(&host, &mut worlds);
        worlds.release_arena(&key, ARENA);
        host.fail("remove_world", "no cloner");
        let events = worlds.start_cleanup(&host);
        assert!(matches!(events[..], [WorldEvent::CleanupFailed { .. }]));
        assert_eq!(worlds.worlds[0].view.readiness, WorldReadiness::Ready);

        host.state.borrow_mut().failing.clear();
        assert_eq!(worlds.start_cleanup(&host), []);
        assert_eq!(worlds.worlds[0].view.readiness, WorldReadiness::Removing);
    }

    #[test]
    fn releasing_the_arena_makes_the_world_idle() {
        let host = FakeHost::default();
        let mut worlds = Worlds::default();
        let key = ready_world(&host, &mut worlds);
        assert_eq!(idle_worlds(&worlds.worlds).count(), 0);
        worlds.release_arena(&key, ARENA);
        assert_eq!(idle_worlds(&worlds.worlds).count(), 1);
    }
}
