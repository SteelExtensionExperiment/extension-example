# Research: World Cloner extension

File:line references are relative to the Steel checkout (`SteelExtensionMC/`) and were checked on
2026-11-23 (`master` at `55a3fb29`). Re-verify before implementing. The host side (templates, storage,
worlds, removal) is researched in `specs/005-world-templates/research.md` in the Steel checkout.

## R1. Services and the extension boundary

**Finding.**

- A contract is a `#[stabby::stabby(checked)]` trait in a small crate, registered with
  `services::register::<Dyn…>(&context, key, priority, value)` and fetched with
  `services::get::<Dyn…>(key)`; versions are part of the key (`steel-extension-api/src/services.rs:1-109`).
  Pattern: `extension-example/worldedit-api/src/lib.rs:11-79`, provider `worldedit/src/lib.rs:415-477`,
  consumer `worldguard/src/lib.rs:29,177,782`.
- Methods may take a borrowed `Server<'_>` (`prison-api/src/lib.rs:71`, `vault-api/src/lib.rs:130-132`).
- `Operation` is a stabby `Copy` struct whose id is `pub(crate)` (`runtime/types.rs:40-65`); callers
  poll `Server::operation_status`, and the first reader after completion consumes the result
  (`steel-core/src/server/temporary/mod.rs:178-193`, one server-wide table, not per extension).
- Dependencies are declared in the long `export_extension!` form (`steel-extension-api/src/lib.rs:221-264`),
  not in `extension.toml` (`host/package.rs:15-26`, `deny_unknown_fields`).
- Config is `extensions/<name>/config.toml` (`host/mod.rs:315-329`) or `[extensions.config]`;
  `data_directory()` is an existing absolute directory (`bindings.rs:158-162`).
- Commands: `CommandBuilder` with literals and arguments, permissions per node
  (`command/builder.rs:139-255`); pattern `extension-example/worldedit/src/lib.rs:359-386,428-429`.
  `Server::message` sends plain system chat to a player (`doc/extensions/api.md:461-486`).
- Lifecycle: `pre_init` subscribes, `init` has the frozen registries and `ExtensionContext`, the
  server-tick callback `register_server_tick` runs every tick (`runtime/bindings.rs:41-54`).

**Decision.**

- Service methods return the host `Operation` directly, so the caller polls it with its own `Server`.
  **No existing contract returns an `Operation`; this is unverified by a compile.** Task T006 builds it,
  and the first consumer proves it before anything else depends on it.
- Because results are consumed once and unread operations stay in the table forever, the contract
  documents that every returned `Operation` must be polled to completion.
- `CreateRequest` carries a full `WorldSettings` (a generated world has no template to fall back on),
  while `CloneRequest` carries the optional `CloneSettings` that default to the template.
- The extension keeps no template data (FR-004): `world_template_status` is read when listing and when
  validating a request.
- Startup loading starts on the first server tick, not in `init`, because world operations need a live
  `Server`. Operations started there are polled on later ticks and consumed once, to log the result.

## R2. Test strategy (open point O1)

**Finding.**

- Unit tests sit beside code and need no host.
- Native tests live in `steel-extension-api/tests/*.rs` with helpers in `tests/common/mod.rs`:
  `library(name)` resolves `../extension-example/<crate>/target/debug/` (`:222-233`) with one special
  case for the in-tree `event_probe`, `load_extensions` (`:284-332`), `start` (`:345-421`).
  `tests/spleef.rs:216-260` loads one library by explicit path and drives temporary worlds.
- That harness is a test module inside `steel-extension-api`, not an importable library, so code outside
  `steel-extension-api` cannot reuse it. This extension now sits at the path `library(name)` searches
  (`extension-example/world-cloner/`), so a test file inside `steel-extension-api/tests/` can load it by name.
- The host platform has its own native test with a probe extension kept in the Steel repo
  (`specs/005-world-templates/tasks.md` T001, T019). It covers the host bindings but not this
  extension's configuration, service or commands.

**Options for this extension's native test** (decision needed, O1):

1. A test file in the Steel checkout (`steel-extension-api/tests/`) that loads this extension's library
   (`../extension-example/world-cloner/target/debug/libworld_cloner.so`). Reuses the harness. Whether it
   is committed is part of the decision: the examples are a separate repository from the Steel checkout.
2. A dev-dependency in this extension on `steel-extension-api` with the `host` feature and a small
   harness copied here. Self-contained, but duplicates `tests/common` and builds the whole host in this
   crate's tests.
3. No native test for this extension: unit tests for configuration, request mapping and command syntax, the
   platform's probe test for host behavior, and the manual check in [quickstart.md](quickstart.md).

**Decided 2026-11-24: option 3.** tasks T003, T006 and T009 carry the unit tests.

## R3. Risks and open points

1. **Operation leaks** (R1): unread operations stay in the host table forever, so the contract and the
   command reporter must poll what they receive.
2. **Contract compile unknown** (R1): proven first by T006.
3. **Platform timing**: the extension cannot compile against bindings that are not merged. The task
   order in [tasks.md](tasks.md) starts with what does not need them.
4. **O1** (R2): native test location.
