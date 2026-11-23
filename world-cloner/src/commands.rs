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
}
