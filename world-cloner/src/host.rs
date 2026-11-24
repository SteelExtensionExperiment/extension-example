//! The seam between the extension logic and the host `Server`.
//!
//! The logic is written against [`HostWorlds`] so it can be tested with a fake; `Operation` and
//! `PlayerId` cannot be constructed outside the host API crate, hence the associated types.

use steel_extension_api::WorldSettings;
use world_cloner_api::{CloneSettings, CloneTarget, TemplateStatus};

/// Result of polling a host operation once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpStatus {
    Pending,
    /// Completed; carries the host's result value (a world or template key).
    Ready(String),
    /// Failed; carries the host's diagnostic.
    Failed(String),
}

impl OpStatus {
    /// Folds a poll result into `None` while pending, else the final outcome. A poll error means
    /// the host no longer knows the operation, so it is reported as a failure.
    #[must_use]
    pub fn finished(polled: Result<Self, String>) -> Option<Result<String, String>> {
        match polled {
            Ok(Self::Pending) => None,
            Ok(Self::Ready(value)) => Some(Ok(value)),
            Ok(Self::Failed(message)) | Err(message) => Some(Err(message)),
        }
    }
}

/// The host world-template and operation calls this extension uses.
pub trait HostWorlds {
    /// Handle to a host operation.
    type Operation: Copy;
    /// Handle to a player.
    type Player: Copy;

    /// Host registry state of a template key; `None` while the host has no entry.
    fn template_status(&self, key: &str) -> Option<TemplateStatus>;

    /// Starts loading the template folder at the absolute `path` under `key`.
    ///
    /// # Errors
    /// The host's immediate refusal.
    fn load_template(&self, key: &str, path: &str) -> Result<Self::Operation, String>;

    /// Starts cloning the template `template_key` as the world `key`.
    ///
    /// # Errors
    /// The host's immediate refusal.
    fn clone_world(
        &self,
        template_key: &str,
        key: &str,
        target: CloneTarget,
        settings: CloneSettings,
    ) -> Result<Self::Operation, String>;

    /// Starts creating a generated world.
    ///
    /// # Errors
    /// The host's immediate refusal.
    fn create_world(
        &self,
        key: &str,
        generator: &str,
        target: CloneTarget,
        settings: WorldSettings,
    ) -> Result<Self::Operation, String>;

    /// Starts removing a runtime world.
    ///
    /// # Errors
    /// The host's immediate refusal.
    fn remove_world(&self, key: &str) -> Result<Self::Operation, String>;

    /// Polls an operation; the first poll after completion consumes the result.
    ///
    /// # Errors
    /// The host no longer knows the operation.
    fn operation_status(&self, operation: Self::Operation) -> Result<OpStatus, String>;

    /// Sends plain chat to a player. Best effort: the player may have left.
    fn message(&self, player: Self::Player, text: &str);
}

#[cfg(test)]
pub(crate) mod fake {
    use std::cell::{Cell, RefCell};
    use std::collections::{HashMap, VecDeque};

    use super::{CloneSettings, CloneTarget, HostWorlds, OpStatus, TemplateStatus, WorldSettings};

    /// One recorded host call.
    #[derive(Debug)]
    pub enum Call {
        Load {
            key: String,
            path: String,
        },
        Clone {
            template_key: String,
            key: String,
            target: CloneTarget,
            settings: CloneSettings,
        },
        Create {
            key: String,
            generator: String,
            target: CloneTarget,
            settings: WorldSettings,
        },
        Remove {
            key: String,
        },
    }

    /// Records calls; operations are `u32` ids that stay pending until a result is scripted.
    #[derive(Default)]
    pub struct FakeHost {
        pub statuses: RefCell<HashMap<String, TemplateStatus>>,
        pub calls: RefCell<Vec<Call>>,
        pub messages: RefCell<Vec<(u32, String)>>,
        /// Keys whose start call is refused immediately, with the diagnostic.
        pub refused: RefCell<HashMap<String, String>>,
        /// Scripted poll results per operation id; an empty script polls as pending.
        pub script: RefCell<HashMap<u32, VecDeque<Result<OpStatus, String>>>>,
        pub polls: RefCell<Vec<u32>>,
        next: Cell<u32>,
    }

    impl FakeHost {
        fn start(&self, key: &str, call: Call) -> Result<u32, String> {
            if let Some(message) = self.refused.borrow().get(key) {
                return Err(message.clone());
            }
            self.calls.borrow_mut().push(call);
            let id = self.next.get();
            self.next.set(id + 1);
            Ok(id)
        }

        pub fn finish(&self, operation: u32, result: Result<OpStatus, String>) {
            self.script
                .borrow_mut()
                .entry(operation)
                .or_default()
                .push_back(result);
        }
    }

    impl HostWorlds for FakeHost {
        type Operation = u32;
        type Player = u32;

        fn template_status(&self, key: &str) -> Option<TemplateStatus> {
            // `TemplateStatus` is a host type without `Clone`; rebuild it from its public fields.
            self.statuses
                .borrow()
                .get(key)
                .map(|status| TemplateStatus {
                    state: status.state,
                    message: status.message.as_str().into(),
                    chunks: status.chunks,
                })
        }

        fn load_template(&self, key: &str, path: &str) -> Result<u32, String> {
            self.start(
                key,
                Call::Load {
                    key: key.to_owned(),
                    path: path.to_owned(),
                },
            )
        }

        fn clone_world(
            &self,
            template_key: &str,
            key: &str,
            target: CloneTarget,
            settings: CloneSettings,
        ) -> Result<u32, String> {
            let call = Call::Clone {
                template_key: template_key.to_owned(),
                key: key.to_owned(),
                target,
                settings,
            };
            self.start(key, call)
        }

        fn create_world(
            &self,
            key: &str,
            generator: &str,
            target: CloneTarget,
            settings: WorldSettings,
        ) -> Result<u32, String> {
            let call = Call::Create {
                key: key.to_owned(),
                generator: generator.to_owned(),
                target,
                settings,
            };
            self.start(key, call)
        }

        fn remove_world(&self, key: &str) -> Result<u32, String> {
            self.start(
                key,
                Call::Remove {
                    key: key.to_owned(),
                },
            )
        }

        fn operation_status(&self, operation: u32) -> Result<OpStatus, String> {
            self.polls.borrow_mut().push(operation);
            self.script
                .borrow_mut()
                .get_mut(&operation)
                .and_then(VecDeque::pop_front)
                .unwrap_or(Ok(OpStatus::Pending))
        }

        fn message(&self, player: u32, text: &str) {
            self.messages.borrow_mut().push((player, text.to_owned()));
        }
    }
}
