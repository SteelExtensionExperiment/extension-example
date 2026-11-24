//! `/worldcloner` command tree and the parsing that turns its arguments into a [`Request`].
//!
//! The tree only parses; the caller-supplied handler performs the operation.
//!
//! Keys and generators contain `:`, which Brigadier's unquoted words and phrases stop at, and the
//! `world` argument only resolves loaded worlds. The arguments that follow a template are therefore
//! read as one greedy argument and split on whitespace here, so `<new|existing>` is validated by
//! [`TargetKind`] and not offered as a literal node.

use std::str::FromStr;
use std::sync::Arc;

use steel_extension_api::command::{Arg, CommandBuilder, Invocation, argument, literal};
use world_cloner_api::{CloneRequest, CloneSettings, CloneTarget, TemplateState};

use crate::host::{HostWorlds, OpStatus};
use crate::service::Cloner;

const ROOT_KEY: &str = "worldcloner:worldcloner";
const KEY_SEPARATOR: char = ':';
const TEMPLATE_ARG: &str = "template";
const ARGS_ARG: &str = "args";

/// Whether a new temporary domain is created for the world or it joins a loaded one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    New,
    Existing,
}

impl FromStr for TargetKind {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "new" => Ok(Self::New),
            "existing" => Ok(Self::Existing),
            other => Err(format!("expected new or existing, found {other}")),
        }
    }
}

/// A parsed `/worldcloner` subcommand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Templates,
    Clone {
        template: String,
        key: String,
        target: TargetKind,
    },
    Create {
        generator: String,
        key: String,
        target: TargetKind,
    },
    Remove {
        key: String,
    },
}

/// Mirrors the characters the host accepts in an `Identifier` namespace.
fn is_namespace_char(char: char) -> bool {
    matches!(char, '_' | '-' | '.') || char.is_ascii_lowercase() || char.is_ascii_digit()
}

/// Validates a `domain:world` key. Unlike the host's `Identifier` parser, the domain is required
/// and neither part may be empty, so a bare name cannot silently fall into `minecraft`.
///
/// # Errors
///
/// Returns the message to show the sender when the input is malformed.
pub fn parse_key(text: &str) -> Result<String, String> {
    let (domain, world) = text
        .split_once(KEY_SEPARATOR)
        .ok_or_else(|| format!("key {text} must be domain{KEY_SEPARATOR}world"))?;
    if domain.is_empty() || world.is_empty() {
        return Err(format!("key {text} must be domain{KEY_SEPARATOR}world"));
    }
    if domain == ".." || !domain.chars().all(is_namespace_char) {
        return Err(format!("invalid domain in key {text}"));
    }
    if !world
        .chars()
        .all(|char| is_namespace_char(char) || char == '/')
    {
        return Err(format!("invalid world name in key {text}"));
    }
    Ok(text.to_owned())
}

/// Splits `rest` into exactly `N` whitespace-separated tokens.
fn tokens<'a, const N: usize>(rest: &'a str, usage: &str) -> Result<[&'a str; N], String> {
    rest.split_whitespace()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| format!("usage: {usage}"))
}

/// Parses the `<key> <new|existing>` part of `clone <template> ...`.
///
/// # Errors
///
/// Returns the message to show the sender when the input is malformed.
pub fn parse_clone(template: &str, rest: &str) -> Result<Request, String> {
    let [key, target] = tokens(rest, "/worldcloner clone <template> <key> <new|existing>")?;
    Ok(Request::Clone {
        template: template.to_owned(),
        key: parse_key(key)?,
        target: target.parse()?,
    })
}

/// Parses the `<generator> <key> <new|existing>` part of `create ...`.
///
/// # Errors
///
/// Returns the message to show the sender when the input is malformed.
pub fn parse_create(rest: &str) -> Result<Request, String> {
    let [generator, key, target] =
        tokens(rest, "/worldcloner create <generator> <key> <new|existing>")?;
    Ok(Request::Create {
        generator: generator.to_owned(),
        key: parse_key(key)?,
        target: target.parse()?,
    })
}

/// Parses the `<key>` part of `remove ...`.
///
/// # Errors
///
/// Returns the message to show the sender when the input is malformed.
pub fn parse_remove(rest: &str) -> Result<Request, String> {
    let [key] = tokens(rest, "/worldcloner remove <key>")?;
    Ok(Request::Remove {
        key: parse_key(key)?,
    })
}

/// Builds `/worldcloner`. Each subcommand requires `worldcloner.<name>`; the root is open by
/// default but has no executor, so it grants nothing by itself.
///
/// `template_names` supplies the completions for `<template>`; `handler` runs a parsed request.
pub fn tree<H, T>(handler: H, template_names: T) -> CommandBuilder
where
    H: Fn(&Invocation<'_, '_>, Request) -> Result<i32, String> + Send + Sync + 'static,
    T: Fn() -> Vec<String> + Send + Sync + 'static,
{
    let handler = Arc::new(handler);
    let templates = Arc::clone(&handler);
    let clone = Arc::clone(&handler);
    let create = Arc::clone(&handler);
    let remove = handler;
    CommandBuilder::new(ROOT_KEY)
        .default_access()
        .then(
            literal("templates")
                .permission("worldcloner.templates")
                .executes(move |ctx| templates(ctx, Request::Templates)),
        )
        .then(
            literal("clone").permission("worldcloner.clone").then(
                argument(TEMPLATE_ARG, Arg::word())
                    .suggests(move |_| template_names())
                    .then(argument(ARGS_ARG, Arg::greedy()).executes(move |ctx| {
                        let request =
                            parse_clone(&ctx.string(TEMPLATE_ARG)?, &ctx.string(ARGS_ARG)?)?;
                        clone(ctx, request)
                    })),
            ),
        )
        .then(
            literal("create").permission("worldcloner.create").then(
                argument(ARGS_ARG, Arg::greedy())
                    .executes(move |ctx| create(ctx, parse_create(&ctx.string(ARGS_ARG)?)?)),
            ),
        )
        .then(
            literal("remove").permission("worldcloner.remove").then(
                argument(ARGS_ARG, Arg::greedy())
                    .executes(move |ctx| remove(ctx, parse_remove(&ctx.string(ARGS_ARG)?)?)),
            ),
        )
}

impl From<TargetKind> for CloneTarget {
    fn from(kind: TargetKind) -> Self {
        match kind {
            TargetKind::New => Self::NewTemporaryDomain,
            TargetKind::Existing => Self::ExistingDomain,
        }
    }
}

/// What a tracked operation does to its world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Clone,
    Create,
    Remove,
}

impl OperationKind {
    const fn gerund(self) -> &'static str {
        match self {
            Self::Clone => "Cloning",
            Self::Create => "Creating",
            Self::Remove => "Removing",
        }
    }

    fn progress(self, key: &str) -> String {
        format!("{} world `{key}`...", self.gerund())
    }

    fn outcome(self, key: &str, result: &Result<String, String>) -> String {
        match (result, self) {
            (Ok(_), Self::Remove) => format!("World `{key}` removed"),
            (Ok(_), _) => format!("World `{key}` is ready"),
            (Err(diagnostic), _) => format!("{} world `{key}` failed: {diagnostic}", self.gerund()),
        }
    }
}

/// An operation started by a command, kept until its result can be reported.
pub struct PendingOperation<O, P> {
    pub operation: O,
    pub kind: OperationKind,
    pub key: String,
    /// The player to message; `None` when the sender is not a player.
    pub reply_to: Option<P>,
}

/// Reports the final result of command-started operations to the sender and the log.
///
/// Generic over the operation and player handles only: the host is a short-lived borrow handed to
/// each call.
pub struct Reporter<O, P> {
    pending: Vec<PendingOperation<O, P>>,
}

impl<O, P> Default for Reporter<O, P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<O, P> Reporter<O, P> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    pub fn track(&mut self, pending: PendingOperation<O, P>) {
        self.pending.push(pending);
    }
}

impl<O: Copy, P: Copy> Reporter<O, P> {
    /// Polls each tracked operation once, messages the sender of each one that finished, drops
    /// it, and returns one log line per finished operation.
    pub fn poll<H: HostWorlds<Operation = O, Player = P>>(&mut self, host: &H) -> Vec<String> {
        let mut lines = Vec::new();
        for pending in std::mem::take(&mut self.pending) {
            let Some(result) = OpStatus::finished(host.operation_status(pending.operation)) else {
                self.pending.push(pending);
                continue;
            };
            let line = pending.kind.outcome(&pending.key, &result);
            if let Some(player) = pending.reply_to {
                host.message(player, &line);
            }
            lines.push(line);
        }
        lines
    }
}

fn state_name(state: TemplateState) -> &'static str {
    match state {
        TemplateState::Loading => "loading",
        TemplateState::Ready => "ready",
        TemplateState::Failed => "failed",
    }
}

/// `/worldcloner templates`: one line per template with its state, and the message when Failed.
#[must_use]
pub fn list_templates<H: HostWorlds>(host: &H, cloner: &Cloner) -> String {
    let templates = cloner.templates(host);
    if templates.is_empty() {
        return "No templates configured".to_owned();
    }
    let lines: Vec<String> = templates
        .iter()
        .map(|info| match info.state {
            TemplateState::Failed => format!("{}: failed - {}", info.name, info.message),
            state => format!("{}: {}", info.name, state_name(state)),
        })
        .collect();
    lines.join("\n")
}

/// `/worldcloner clone`: starts the clone and tracks it for reporting. Returns the immediate
/// feedback.
///
/// # Errors
/// The service's or host's immediate refusal, unchanged.
pub fn start_clone<H: HostWorlds>(
    host: &H,
    cloner: &Cloner,
    reporter: &mut Reporter<H::Operation, H::Player>,
    template: &str,
    key: &str,
    target: TargetKind,
    sender: Option<H::Player>,
) -> Result<String, String> {
    let request = CloneRequest {
        template: template.into(),
        key: key.into(),
        target: target.into(),
        settings: CloneSettings {
            seed: None.into(),
            spawn: None.into(),
            default_game_mode: None.into(),
            difficulty: None.into(),
        },
    };
    let operation = cloner.clone_world(host, request)?;
    reporter.track(PendingOperation {
        operation,
        kind: OperationKind::Clone,
        key: key.to_owned(),
        reply_to: sender,
    });
    Ok(OperationKind::Clone.progress(key))
}

/// `/worldcloner create`: starts the creation with the configured `[create]` settings and tracks
/// it for reporting. Returns the immediate feedback.
///
/// # Errors
/// The host's immediate refusal, unchanged.
pub fn start_create<H: HostWorlds>(
    host: &H,
    cloner: &Cloner,
    reporter: &mut Reporter<H::Operation, H::Player>,
    generator: &str,
    key: &str,
    target: TargetKind,
    sender: Option<H::Player>,
) -> Result<String, String> {
    let operation =
        cloner.create_world(host, cloner.create_request(generator, key, target.into()))?;
    reporter.track(PendingOperation {
        operation,
        kind: OperationKind::Create,
        key: key.to_owned(),
        reply_to: sender,
    });
    Ok(OperationKind::Create.progress(key))
}

/// Runs a parsed [`Request`] and returns the immediate feedback.
///
/// # Errors
/// The refusal of the called operation, unchanged.
pub fn execute<H: HostWorlds>(
    host: &H,
    cloner: &Cloner,
    reporter: &mut Reporter<H::Operation, H::Player>,
    request: Request,
    sender: Option<H::Player>,
) -> Result<String, String> {
    match request {
        Request::Templates => Ok(list_templates(host, cloner)),
        Request::Clone {
            template,
            key,
            target,
        } => start_clone(host, cloner, reporter, &template, &key, target, sender),
        Request::Create {
            generator,
            key,
            target,
        } => start_create(host, cloner, reporter, &generator, &key, target, sender),
        Request::Remove { key } => start_remove(host, cloner, reporter, &key, sender),
    }
}

/// `/worldcloner remove`: starts the removal and tracks it for reporting. Returns the immediate
/// feedback.
///
/// # Errors
/// The host's immediate refusal, unchanged.
pub fn start_remove<H: HostWorlds>(
    host: &H,
    cloner: &Cloner,
    reporter: &mut Reporter<H::Operation, H::Player>,
    key: &str,
    sender: Option<H::Player>,
) -> Result<String, String> {
    let operation = cloner.remove_world(host, key)?;
    reporter.track(PendingOperation {
        operation,
        kind: OperationKind::Remove,
        key: key.to_owned(),
        reply_to: sender,
    });
    Ok(OperationKind::Remove.progress(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_need_a_nonempty_domain_and_world() {
        assert_eq!(parse_key("game_3:map"), Ok("game_3:map".to_owned()));
        assert_eq!(parse_key("arena:maps/one"), Ok("arena:maps/one".to_owned()));
        for bad in [
            "map", ":map", "game:", "a:b:c", "Game:map", "..:map", "g:ma p", "g/x:map",
        ] {
            assert!(parse_key(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn clone_reads_key_and_target_after_the_template() {
        assert_eq!(
            parse_clone("lobby", "game_3:map  existing "),
            Ok(Request::Clone {
                template: "lobby".to_owned(),
                key: "game_3:map".to_owned(),
                target: TargetKind::Existing,
            })
        );
    }

    #[test]
    fn create_reads_a_namespaced_generator() {
        assert_eq!(
            parse_create("example:strange game_3:map new"),
            Ok(Request::Create {
                generator: "example:strange".to_owned(),
                key: "game_3:map".to_owned(),
                target: TargetKind::New,
            })
        );
    }

    #[test]
    fn wrong_token_count_or_target_reports_usage_or_the_bad_word() {
        assert!(
            parse_clone("lobby", "game_3:map")
                .unwrap_err()
                .starts_with("usage:")
        );
        assert!(parse_clone("lobby", "game_3:map new extra").is_err());
        assert!(parse_create("example:strange game_3:map").is_err());
        assert!(parse_remove("").is_err());
        assert!(parse_remove("a:b c:d").is_err());
        assert_eq!(
            parse_clone("lobby", "game_3:map maybe"),
            Err("expected new or existing, found maybe".to_owned())
        );
    }

    #[test]
    fn remove_validates_its_key() {
        assert_eq!(
            parse_remove("game_3:map"),
            Ok(Request::Remove {
                key: "game_3:map".to_owned()
            })
        );
        assert!(parse_remove("map").is_err());
    }

    mod execution {
        use std::path::PathBuf;

        use world_cloner_api::TemplateStatus;

        use super::*;
        use crate::config::Config;
        use crate::host::fake::{Call, FakeHost};

        fn cloner() -> Cloner {
            let toml = "[[templates]]\nname = \"ready\"\npath = \"r\"\n\
                        [[templates]]\nname = \"broken\"\npath = \"b\"\n\
                        [[templates]]\nname = \"new\"\npath = \"n\"\n";
            Cloner::new(Config::parse(toml).unwrap(), PathBuf::from("/data"))
        }

        fn host() -> FakeHost {
            let host = FakeHost::default();
            let status = |state, message: &str| TemplateStatus {
                state,
                message: message.into(),
                chunks: 0,
            };
            let mut statuses = host.statuses.borrow_mut();
            statuses.insert(
                "world-cloner:ready".to_owned(),
                status(TemplateState::Ready, ""),
            );
            statuses.insert(
                "world-cloner:broken".to_owned(),
                status(TemplateState::Failed, "no region"),
            );
            drop(statuses);
            host
        }

        fn track(
            reporter: &mut Reporter<u32, u32>,
            operation: u32,
            kind: OperationKind,
            player: Option<u32>,
        ) {
            reporter.track(PendingOperation {
                operation,
                kind,
                key: "a:w".to_owned(),
                reply_to: player,
            });
        }

        #[test]
        fn success_messages_the_player_and_returns_a_log_line() {
            let (host, mut reporter) = (FakeHost::default(), Reporter::new());
            track(&mut reporter, 0, OperationKind::Clone, Some(9));
            host.finish(0, Ok(OpStatus::Ready("a:w".to_owned())));
            assert_eq!(reporter.poll(&host), ["World `a:w` is ready"]);
            assert_eq!(
                *host.messages.borrow(),
                [(9, "World `a:w` is ready".to_owned())]
            );
        }

        #[test]
        fn failure_carries_the_diagnostic_and_a_lost_operation_counts_as_failure() {
            let (host, mut reporter) = (FakeHost::default(), Reporter::new());
            track(&mut reporter, 0, OperationKind::Create, Some(1));
            track(&mut reporter, 1, OperationKind::Remove, Some(1));
            host.finish(0, Ok(OpStatus::Failed("generator missing".to_owned())));
            host.finish(1, Err("unknown operation".to_owned()));
            assert_eq!(
                reporter.poll(&host),
                [
                    "Creating world `a:w` failed: generator missing",
                    "Removing world `a:w` failed: unknown operation"
                ]
            );
            assert_eq!(host.messages.borrow().len(), 2);
        }

        #[test]
        fn no_player_means_log_only_and_removal_has_its_own_wording() {
            let (host, mut reporter) = (FakeHost::default(), Reporter::new());
            track(&mut reporter, 0, OperationKind::Remove, None);
            host.finish(0, Ok(OpStatus::Ready("a:w".to_owned())));
            assert_eq!(reporter.poll(&host), ["World `a:w` removed"]);
            assert!(host.messages.borrow().is_empty());
        }

        #[test]
        fn pending_is_kept_and_finished_is_dropped() {
            let (host, mut reporter) = (FakeHost::default(), Reporter::new());
            track(&mut reporter, 0, OperationKind::Clone, None);
            track(&mut reporter, 1, OperationKind::Clone, None);
            host.finish(1, Ok(OpStatus::Ready(String::new())));
            assert_eq!(reporter.poll(&host).len(), 1);
            host.finish(0, Ok(OpStatus::Ready(String::new())));
            assert_eq!(reporter.poll(&host).len(), 1);
            host.polls.borrow_mut().clear();
            assert_eq!(reporter.poll(&host), Vec::<String>::new());
            assert!(host.polls.borrow().is_empty());
        }

        #[test]
        fn templates_lists_one_line_each_with_the_failure_message() {
            assert_eq!(
                list_templates(&host(), &cloner()),
                "ready: ready\nbroken: failed - no region\nnew: loading"
            );
            let empty = Cloner::new(Config::default(), PathBuf::new());
            assert_eq!(list_templates(&host(), &empty), "No templates configured");
        }

        #[test]
        fn clone_starts_tracks_and_reports_to_the_sender() {
            let (host, cloner, mut reporter) = (host(), cloner(), Reporter::new());
            let feedback = start_clone(
                &host,
                &cloner,
                &mut reporter,
                "ready",
                "g:m",
                TargetKind::Existing,
                Some(5),
            );
            assert_eq!(feedback, Ok("Cloning world `g:m`...".to_owned()));
            assert!(matches!(
                host.calls.borrow().as_slice(),
                [Call::Clone {
                    target: CloneTarget::ExistingDomain,
                    ..
                }]
            ));
            host.finish(0, Ok(OpStatus::Ready("g:m".to_owned())));
            assert_eq!(reporter.poll(&host), ["World `g:m` is ready"]);
            assert_eq!(host.messages.borrow()[0].0, 5);
        }

        #[test]
        fn immediate_refusals_are_returned_and_not_tracked() {
            let (host, cloner, mut reporter) = (host(), cloner(), Reporter::new());
            let clone = |reporter: &mut Reporter<u32, u32>, template| {
                start_clone(
                    &host,
                    &cloner,
                    reporter,
                    template,
                    "g:m",
                    TargetKind::New,
                    None,
                )
            };
            assert_eq!(
                clone(&mut reporter, "nope"),
                Err("unknown template nope".to_owned())
            );
            assert_eq!(clone(&mut reporter, "broken"), Err("no region".to_owned()));
            assert_eq!(
                clone(&mut reporter, "new"),
                Err("template is still loading".to_owned())
            );
            host.refused
                .borrow_mut()
                .insert("g:w".to_owned(), "not removable".to_owned());
            let removed = start_remove(&host, &cloner, &mut reporter, "g:w", None);
            assert_eq!(removed, Err("not removable".to_owned()));
            assert_eq!(reporter.poll(&host), Vec::<String>::new());
            assert!(host.polls.borrow().is_empty());
        }

        #[test]
        fn create_tracks_and_passes_the_configured_settings() {
            let toml = "[create]\nseed = 7\nspawn = [1, 2, 3]\n\
                        default_game_mode = \"spectator\"\ndifficulty = \"peaceful\"\n";
            let cloner = Cloner::new(Config::parse(toml).unwrap(), PathBuf::new());
            let (host, mut reporter) = (host(), Reporter::new());
            let feedback = start_create(
                &host,
                &cloner,
                &mut reporter,
                "example:strange",
                "g:m",
                TargetKind::New,
                Some(4),
            );
            assert_eq!(feedback, Ok("Creating world `g:m`...".to_owned()));
            let calls = host.calls.borrow();
            let [
                Call::Create {
                    key,
                    generator,
                    target,
                    settings,
                },
            ] = calls.as_slice()
            else {
                panic!("expected one create call, got {calls:?}");
            };
            assert_eq!(
                (key.as_str(), generator.as_str()),
                ("g:m", "example:strange")
            );
            assert_eq!(*target, CloneTarget::NewTemporaryDomain);
            assert_eq!(
                (
                    settings.seed,
                    settings.spawn.x,
                    settings.spawn.y,
                    settings.spawn.z
                ),
                (7, 1, 2, 3)
            );
            assert!(matches!(
                settings.default_game_mode,
                steel_extension_api::GameMode::Spectator
            ));
            assert!(matches!(
                settings.difficulty,
                steel_extension_api::Difficulty::Peaceful
            ));
            drop(calls);
            host.finish(0, Ok(OpStatus::Ready("g:m".to_owned())));
            assert_eq!(reporter.poll(&host), ["World `g:m` is ready"]);
            assert_eq!(host.messages.borrow()[0].0, 4);
        }

        #[test]
        fn create_refusal_is_returned_and_not_tracked() {
            let (host, cloner, mut reporter) = (host(), cloner(), Reporter::new());
            host.refused
                .borrow_mut()
                .insert("g:m".to_owned(), "key in use".to_owned());
            let result = start_create(
                &host,
                &cloner,
                &mut reporter,
                "x:y",
                "g:m",
                TargetKind::New,
                None,
            );
            assert_eq!(result, Err("key in use".to_owned()));
            assert_eq!(reporter.poll(&host), Vec::<String>::new());
            assert!(host.polls.borrow().is_empty());
        }

        #[test]
        fn execute_dispatches_every_request() {
            let (host, cloner, mut reporter) = (host(), cloner(), Reporter::new());
            let mut run = |request| execute(&host, &cloner, &mut reporter, request, None);
            let key = || "g:m".to_owned();
            let target = TargetKind::New;
            assert_eq!(
                run(Request::Templates),
                Ok("ready: ready\nbroken: failed - no region\nnew: loading".to_owned())
            );
            assert_eq!(
                run(Request::Clone {
                    template: "ready".to_owned(),
                    key: key(),
                    target
                }),
                Ok("Cloning world `g:m`...".to_owned())
            );
            assert_eq!(
                run(Request::Create {
                    generator: "x:y".to_owned(),
                    key: key(),
                    target
                }),
                Ok("Creating world `g:m`...".to_owned())
            );
            assert_eq!(
                run(Request::Remove { key: key() }),
                Ok("Removing world `g:m`...".to_owned())
            );
            assert!(matches!(
                host.calls.borrow().as_slice(),
                [Call::Clone { .. }, Call::Create { .. }, Call::Remove { .. }]
            ));
        }

        #[test]
        fn remove_starts_and_tracks() {
            let (host, cloner, mut reporter) = (host(), cloner(), Reporter::new());
            let feedback = start_remove(&host, &cloner, &mut reporter, "g:m", Some(2));
            assert_eq!(feedback, Ok("Removing world `g:m`...".to_owned()));
            host.finish(0, Ok(OpStatus::Ready("g:m".to_owned())));
            assert_eq!(reporter.poll(&host), ["World `g:m` removed"]);
        }
    }
}
