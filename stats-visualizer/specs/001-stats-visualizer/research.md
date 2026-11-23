# Research: Stats Visualizer extension

File:line references are relative to the Steel checkout (`SteelExtensionMC/`) and were checked on
2026-11-23 (`master` at `55a3fb29`). Re-verify before implementing. The hologram capability is researched
in `specs/006-text-holograms/research.md`.

## R1. Host facilities this extension uses

**Finding.**

- **Shutdown:** `Hooks::on_shutdown(|server| ...)` (`steel-extension-api/src/events/handlers.rs:432`)
  and `register_shutdown` (`steel-extension-api/src/bindings.rs:121-125`). The callback runs once on
  clean stop, before worlds are saved; callbacks run in reverse initialization order.
- **Periodic work:** `hooks.enable_scheduler()` in `pre_init`, then `scheduler::run_repeating(delay,
  period, job)` (`steel-extension-api/src/scheduler.rs:77-87`). Precedent:
  `extension-example/essentials/src/lib.rs:27,44-51` (dirty flag, save every N ticks, save again on
  shutdown).
- **Atomic write:** temporary file then rename, `extension-example/essentials/src/storage.rs:236-244`;
  corrupt file moved to `*.broken`, `storage.rs:225-233`.
- **Data directory and config:** `ExtensionContext::data_directory()` and `config()`
  (`steel-extension-api/src/bindings.rs:154-161`).
- **World load/unload events:** `on_world_load` / `on_world_unload`
  (`steel-extension-api/src/events/handlers.rs:378-388`, `events/more.rs:98,329`). Used to bring boards
  back when their world appears.
- **Players:** `player_name`, `player_by_uuid` (`steel-extension-api/src/players.rs:123-139`), `Uuid`
  display/parse (`players.rs:21,35`). Results carry UUID and name, so no lookup is required.
- **Commands:** `CommandBuilder` with literals, arguments, per-node permissions and completion
  (`steel-extension-api/src/command/builder.rs`), `Command::reply` (`runtime/bindings.rs:90`).
- **Services and dependencies:** `steel-extension-api/src/services.rs`; dependencies are declared in the
  long `export_extension!` form (`steel-extension-api/src/lib.rs:221-264`), not in `extension.toml`
  (`host/package.rs:15-26`).
- **Text:** every text API accepts plain text or a vanilla JSON component (`steel-extension-api/src/text.rs:1-6`);
  the `Text` builder gives colors and styles.

**Platform gap (decision D3).** There is no extension binding to create or update floating text.
`World::spawn_entity` returns only a UUID and cannot set text
(`steel-extension-api/src/entity_behavior.rs:68-75`); `World::remove_entity` takes an entity id, not the
UUID (`steel-extension-api/src/worlds.rs:107`); a text display spawned that way would be saved into chunk
data and duplicated after a restart. The host already has the entity: `TextDisplayEntity`
(`steel-core/src/entity/entities/objects/technical/display/text_display.rs`). Exposing it to extensions
without persistence is host work, specified in `specs/006-text-holograms/`.

**Decision.** This extension uses only the hologram bindings of spec 006 and keeps no entity ids.

## R2. The orchestrator contract

**Finding.** `game-orchestrator/` contains only a README: no spec, no API crate. Nothing can be
verified against code.

**Decision.** Record the minimum needed in
[contracts/required-orchestrator.md](contracts/required-orchestrator.md) as a proposal. Every task that
touches the orchestrator is blocked on its spec (O3). The extension holds no copy of the orchestrator's
types; it uses the sibling path `../game-orchestrator-api` once that crate exists.

## R3. Test strategy (open point O1)

**Finding.** Same situation as the world cloner (`world-cloner/specs/001-world-cloner/research.md` R2):
the native-test helper `steel-extension-api/tests/common/mod.rs` is not importable from outside
`steel-extension-api`, and it resolves libraries at `../extension-example/<crate>/target/debug/`, where
this extension now sits.

**Decision.** Until O1 is decided, unit tests carry the logic. Ranking, ratio, result application,
storage round trip, corrupt-file handling and command syntax need no host. Host calls (hologram create,
update, remove; orchestrator lookups) sit behind small traits so tests use fakes. The manual check is
[quickstart.md](quickstart.md).

## R4. Risks and open points

1. **Two upstream dependencies** (spec 006 and the orchestrator contract) are both unbuilt. Tasks that need
   neither (T001–T005: ranking, storage, config) can start now and carry real value.
2. **Hologram lifecycle** (spec 006): boards must survive chunk unloads and world loads. This extension
   assumes the host owns that and exposes create/update/remove only; if spec 006 changes that split,
   `boards.rs` changes.
3. **Synchronous autosave** (O2): file size grows with the player count; a few thousand records are tens of
   kilobytes, but a large network may want a job-system write later.
4. **Draws** are not representable in the proposed result (R2, open question 3).
5. **Vanilla:** this extension has no gameplay logic; vanilla parity is a host concern (spec 006).
