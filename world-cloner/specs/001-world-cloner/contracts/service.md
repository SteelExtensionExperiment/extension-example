# Contract: service `world-cloner:clone/v1`

Provided by the `world-cloner` extension, defined in the `world-cloner-api` crate, consumed by other
extensions (the arena manager and game orchestrator are the planned ones). It wraps the host bindings in
`specs/005-world-templates/contracts/host-api.md` in the Steel checkout.

## Trait

```text
pub const CLONER: &str = "world-cloner:clone/v1";

#[stabby::stabby(checked)]
pub trait WorldCloner: Send + Sync {
    extern "C" fn templates(&self, server: Server<'_>) -> ApiVec<TemplateInfo>;
    extern "C" fn clone_world(&self, server: Server<'_>, request: CloneRequest) -> ApiResult<Operation>;
    extern "C" fn create_world(&self, server: Server<'_>, request: CreateRequest) -> ApiResult<Operation>;
    extern "C" fn remove_world(&self, server: Server<'_>, key: Str<'_>) -> ApiResult<Operation>;
}

pub type DynWorldCloner = stabby::dynptr!(Box<dyn WorldCloner + Send + Sync>);
pub fn cloner() -> Result<&'static DynWorldCloner, String>;   // services::get
```

Exact signatures follow `extension-example/worldedit-api/src/lib.rs:64-79` and
`extension-example/vault-api/src/lib.rs:130-132`; names and shapes are final, spelling may change to
satisfy stabby. Request types are in [../data-model.md](../data-model.md).

## Methods

| Method | Behavior |
| --- | --- |
| `templates` | One `TemplateInfo` per configured template, with its current host state and failure message. |
| `clone_world` | Maps `request.template` to the host key `world-cloner:<name>`, fills the template's `default_game_mode` when `settings` leaves it unset, and calls the host `clone_world`. Fails immediately with the template's message when it is Failed, "template is still loading" when Loading, and `unknown template <name>` when not configured. |
| `create_world` | Calls the host `create_world` with the request's full `WorldSettings`. A generator that needs configuration, such as `steel:empty`, is rejected by the host with its message. |
| `remove_world` | Calls the host `remove_world`. The host refuses worlds from `worlds.toml`, worlds being created or removed, and shared clock owners. |

## Rules for callers

- Every returned `Operation` belongs to the caller. Poll it with `Server::operation_status` on later
  ticks until `Ready` or `Failed`. The first read after completion consumes the result; an operation never
  read stays in the host table for the life of the process.
- `OperationStatus::value` is empty while pending, the world key on success, or a diagnostic on failure.
- Never sleep or spin waiting for an operation; return to the tick loop.
- Consumers declare `depends: ["world-cloner"]` (or `soft_depends`) in `export_extension!`, import
  `world_cloner_api::WorldClonerDyn as _`, and store returned `Operation`s in their own state.

## Keys and naming

- Template names in the service are bare names (`skywars_a`); only this extension builds
  `world-cloner:<name>` host keys.
- World keys are `domain:world`. The domain part follows the host's identifier rules
  (`steel-utils/src/types/identifier.rs:95-108`).

## Versioning

`/v1` changes only for incompatible layout changes. Adding a method is a new key (`/v2`). A published
contract changes only with the user's approval (`specs/extensions-workflow.md` in the Steel
checkout), because other extensions build against it.

## Extension lifecycle

| Stage | What the extension does |
| --- | --- |
| `pre_init` | Subscribes to commands and the server tick. |
| `init` | Parses and validates `config.toml`; registers the service with `services::register`. |
| first server tick | Calls `load_world_template` for every configured template; remembers each `Operation`. |
| later ticks | Polls startup operations once to log the result; polls command operations and reports to the sender. |

Declared in the long `export_extension!` form with `name: "world-cloner"`, `depends: []`.
