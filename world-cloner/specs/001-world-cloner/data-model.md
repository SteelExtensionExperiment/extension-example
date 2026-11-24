# Data Model: World Cloner extension

Host entities (template, clone, runtime world, `Operation`, the bindings) are defined in the platform
spec `specs/005-world-templates/` in the Steel checkout (`data-model.md`, `contracts/host-api.md`). This
file covers only what this extension adds. Items are **(new)**.

## `world-cloner-api` (new crate)

Types that cross the extension boundary are `#[stabby::stabby]`. `TemplateState`, `CloneTarget`,
`CloneSettings`, `WorldSettings` and `Operation` come from `steel_extension_api::runtime`.

| Item | Shape |
| --- | --- |
| `CLONER` | `"world-cloner:clone/v1"`. |
| `TemplateInfo` | `{ name: ApiString, state: TemplateState, message: ApiString }`; `message` is empty unless Failed. |
| `CloneRequest` | `{ template: ApiString, key: ApiString, target: CloneTarget, settings: CloneSettings }`. Unset fields default to the template; an unset game mode uses the template's configured default. |
| `CreateRequest` | `{ key: ApiString, generator: ApiString, target: CloneTarget, settings: WorldSettings }`. A generated world has no template, so every setting is given. |
| `WorldCloner` | Trait with `templates`, `clone_world`, `create_world`, `remove_world`; see [contracts/service.md](contracts/service.md). |
| `DynWorldCloner`, `cloner()` | The stabby dyn pointer alias and the lookup helper (`services::get`). |

## `world-cloner` (new crate)

### Configuration (`extensions/world-cloner/config.toml`)

```toml
[[templates]]
name = "skywars_a"               # unique; letters, digits, '_' and '-'
path = "templates/skywars_a"     # relative to the extension data directory; must not escape it
default_game_mode = "survival"   # survival | creative | adventure | spectator; default survival

[create]                         # optional; settings for `/worldcloner create`, every key optional
seed = 0                         # i64
spawn = [0, 64, 0]               # [x, y, z] block coordinates
default_game_mode = "survival"   # same set as templates
difficulty = "normal"            # peaceful | easy | normal | hard
```

The `[create]` defaults shown are a plugin-local choice, not vanilla data, and only the admin command
uses them; service callers always pass their own `WorldSettings`.

Validation at `init` (FR-002): unique names, name characters, no absolute paths or `..`, known game
mode, and in `[create]` a 3-element `spawn`, a known game mode and a known difficulty. A failure refuses to start the extension with a message naming the entry. `path` points at the
saved world folder itself (the directory that holds `region/` and `level.toml`), resolved against
`ExtensionContext::data_directory()`.

### Runtime state

| Entity | Fields |
| --- | --- |
| `Cloner` | `config: Config` (`templates`, `create`), `data_dir`, `startup: Vec<(name, Operation)>`, `pending: Vec<PendingOperation>`. |
| `TemplateConfig` | `{ name, path, default_game_mode }`, validated. |
| `CreateConfig` | `{ seed, spawn: [x, y, z], default_game_mode, difficulty }`, validated; built into the `WorldSettings` of `/worldcloner create`. |
| `PendingOperation` | `{ operation: Operation, kind: Clone | Create | Remove, key: String, reply_to: Option<PlayerId> }`. Only commands record one; operations returned through the service belong to the caller. |

The extension keeps no template data: the host registry is the source of truth, read with
`world_template_status` (FR-004). The host key of a template is `world-cloner:<name>`; only this
extension builds it.
