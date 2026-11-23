//! Template loading at startup (FR-003): started on the first tick, each result reported once.

use std::fmt;

use crate::host::{HostWorlds, OpStatus};
use crate::service::Cloner;

/// Outcome of loading one template, for the caller to log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartupEvent {
    Ready { template: String },
    Failed { template: String, message: String },
}

impl fmt::Display for StartupEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready { template } => write!(f, "template {template} is ready"),
            Self::Failed { template, message } => {
                write!(f, "template {template} failed to load: {message}")
            }
        }
    }
}

/// Starts every configured template on the first tick and polls the loads on later ticks.
pub struct Startup<H: HostWorlds> {
    started: bool,
    loading: Vec<(String, H::Operation)>,
}

impl<H: HostWorlds> Default for Startup<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: HostWorlds> Startup<H> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            started: false,
            loading: Vec::new(),
        }
    }

    /// Runs once per server tick. The first call starts the loads and reports only immediate
    /// refusals; later calls poll each pending load until it completes.
    pub fn tick(&mut self, host: &H, cloner: &Cloner) -> Vec<StartupEvent> {
        if self.started {
            return self.poll(host);
        }
        self.started = true;
        let mut events = Vec::new();
        for template in cloner.template_configs() {
            let started = cloner
                .template_path(template)
                .and_then(|path| host.load_template(&Cloner::host_key(&template.name), &path));
            match started {
                Ok(operation) => self.loading.push((template.name.clone(), operation)),
                Err(message) => {
                    events.push(StartupEvent::Failed {
                        template: template.name.clone(),
                        message,
                    });
                }
            }
        }
        events
    }

    fn poll(&mut self, host: &H) -> Vec<StartupEvent> {
        let mut events = Vec::new();
        for (template, operation) in std::mem::take(&mut self.loading) {
            match OpStatus::finished(host.operation_status(operation)) {
                None => self.loading.push((template, operation)),
                Some(Ok(_)) => events.push(StartupEvent::Ready { template }),
                Some(Err(message)) => events.push(StartupEvent::Failed { template, message }),
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::config::Config;
    use crate::host::fake::{Call, FakeHost};

    fn cloner() -> Cloner {
        let toml = "[[templates]]\nname = \"a\"\npath = \"t/a\"\n\
                    [[templates]]\nname = \"b\"\npath = \"t/b\"\n";
        Cloner::new(Config::parse(toml).unwrap(), PathBuf::from("/data"))
    }

    fn failed(template: &str, message: &str) -> StartupEvent {
        StartupEvent::Failed {
            template: template.to_owned(),
            message: message.to_owned(),
        }
    }

    #[test]
    fn loads_start_on_the_first_tick_only() {
        let (host, cloner, mut startup) = (FakeHost::default(), cloner(), Startup::new());
        assert_eq!(startup.tick(&host, &cloner), Vec::new());
        assert_eq!(startup.tick(&host, &cloner), Vec::new());
        let calls = host.calls.borrow();
        let [Call::Load { key: a, path }, Call::Load { key: b, .. }] = calls.as_slice() else {
            panic!("expected two loads, got {calls:?}");
        };
        assert_eq!(
            (a.as_str(), b.as_str()),
            ("world-cloner:a", "world-cloner:b")
        );
        assert_eq!(path, "/data/t/a");
    }

    #[test]
    fn each_result_is_reported_once_and_never_polled_again() {
        let (host, cloner, mut startup) = (FakeHost::default(), cloner(), Startup::new());
        startup.tick(&host, &cloner);
        host.finish(0, Ok(OpStatus::Ready("world-cloner:a".to_owned())));
        host.finish(1, Ok(OpStatus::Failed("no region folder".to_owned())));
        assert_eq!(
            startup.tick(&host, &cloner),
            [
                StartupEvent::Ready {
                    template: "a".to_owned()
                },
                failed("b", "no region folder")
            ]
        );
        host.polls.borrow_mut().clear();
        assert_eq!(startup.tick(&host, &cloner), Vec::new());
        assert!(host.polls.borrow().is_empty());
    }

    #[test]
    fn pending_loads_stay_pending() {
        let (host, cloner, mut startup) = (FakeHost::default(), cloner(), Startup::new());
        startup.tick(&host, &cloner);
        host.finish(1, Ok(OpStatus::Ready(String::new())));
        assert_eq!(startup.tick(&host, &cloner).len(), 1);
        host.polls.borrow_mut().clear();
        assert_eq!(startup.tick(&host, &cloner), Vec::new());
        assert_eq!(*host.polls.borrow(), [0]);
    }

    #[test]
    fn an_immediate_refusal_does_not_stop_the_others() {
        let (host, cloner, mut startup) = (FakeHost::default(), cloner(), Startup::new());
        host.refused
            .borrow_mut()
            .insert("world-cloner:a".to_owned(), "not a directory".to_owned());
        assert_eq!(
            startup.tick(&host, &cloner),
            [failed("a", "not a directory")]
        );
        host.finish(0, Ok(OpStatus::Ready(String::new())));
        assert_eq!(
            startup.tick(&host, &cloner),
            [StartupEvent::Ready {
                template: "b".to_owned()
            }]
        );
    }

    #[test]
    fn a_lost_operation_is_reported_as_failed() {
        let (host, cloner, mut startup) = (FakeHost::default(), cloner(), Startup::new());
        startup.tick(&host, &cloner);
        host.finish(0, Err("unknown operation".to_owned()));
        assert_eq!(
            startup.tick(&host, &cloner),
            [failed("a", "unknown operation")]
        );
    }
}
