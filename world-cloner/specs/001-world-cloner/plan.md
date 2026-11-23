# Implementation Plan: World Cloner extension

**Branch**: `001-world-cloner` | **Date**: 2026-11-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-world-cloner/spec.md`

## Summary

Build the `world-cloner` extension and its `world-cloner-api` contract crate on the host platform from
`specs/005-world-templates/` (Steel checkout). The extension reads its configuration, starts template
loading on the first tick, exposes the versioned service `world-cloner:clone/v1`, and adds the
`/worldcloner` admin commands. It holds no world or template data of its own.

## Technical Context

**Language/Version**: Rust (the Steel checkout's `rust-toolchain.toml`, nightly with `-Zthreads=0`; found by
Cargo because this extension sits inside the checkout)

**Primary Dependencies**: `steel-extension-api` (path `../../steel-extension-api`, without the `host`
feature), `serde`, `toml`. No new crates.

**Storage**: the extension's `config.toml` only. Template folders are read by the host and never written.

**Testing**: unit tests beside the code (FR-010) and the manual check in [quickstart.md](quickstart.md).
The native test location is open (O1).

**Target Platform**: Linux server, vanilla Java clients.

**Project Type**: native extension (cdylib) plus a contract crate.

**Performance Goals**: none beyond the host's; this extension does no per-tick work except polling a few
operations.

**Constraints**:
- No async, no threads: world work runs on the host.
- No panics in production paths; every failure is a message to the caller.

**Scale/Scope**: a handful of templates; commands used rarely.

## Constitution Check

| Principle | Status | How |
| --- | --- | --- |
| I. Stable Native Boundary | Pass, with one unproven item | The contract uses stabby types under a versioned key. A service method returning the host `Operation` has no precedent; T006 builds it and the first consumer proves it. |
| II. Modding-Grade Extensibility | Pass | Other extensions use only the service; the host keys stay private to this extension. |
| III. Vanilla Parity First | Pass | No gameplay logic here; clone behavior is the host's. |
| IV. Foundation Rigor | Pass | The extension implements the full declared contract; unimplemented platform pieces block tasks, they are not stubbed. |
| V. Explicit Failure | Pass | Every call returns a message on failure; invalid configuration refuses to start. |
| VI. Concurrency & Dependencies | Pass | No threads, no async, no new crates. |
| VII. Verified Behavior | Pass, open point | Unit tests plus a manual check; the native test is decision O1. |

No violations.

## Project Structure

### Documentation (this feature)

```text
specs/001-world-cloner/
├── plan.md
├── spec.md
├── research.md
├── data-model.md
├── quickstart.md
├── tasks.md
├── contracts/
│   ├── service.md
│   └── commands.md
└── checklists/
    └── requirements.md
```

### Source Code

```text
world-cloner-api/
├── Cargo.toml
└── src/lib.rs                # NEW contract: CLONER, request types, WorldCloner trait

world-cloner/
├── Cargo.toml
├── extension.toml
├── clippy.toml
└── src/
    ├── lib.rs                # NEW export_extension!, init, tick
    ├── config.rs             # NEW configuration and validation
    ├── startup.rs            # NEW first-tick loading and result logging
    ├── service.rs            # NEW service implementation
    └── commands.rs           # NEW /worldcloner and the pending-operation reporter
```

**Structure decision.** One module per responsibility, none large. The host calls sit behind a small
seam so the mapping, defaults and failure messages can be unit-tested without a server.

## Layout requirement

This extension sits in `extension-example/` inside a Steel checkout, as `SteelExtensionMC/extension-example/world-cloner/`
(crate and this spec) with its contract crate beside it as `extension-example/world-cloner-api/`, like `vault` and
`vault-api`. Its crates reach the API through `../../steel-extension-api`. Other extensions depend on
`world-cloner-api` by the sibling path `../world-cloner-api`.

## Platform dependencies

| This task | Waits for (Steel `specs/005-world-templates/tasks.md`) |
| --- | --- |
| T004, T005, T006 | T012 (types, load, status, clone bindings), T007 (registry) |
| T007 | T015 (`create_world` binding) |
| T008 | T018 (`remove_world` binding) |

## Complexity Tracking

Empty.
