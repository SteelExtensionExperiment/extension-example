# world-cloner-api

Contract crate for the `world-cloner` extension: service key `world-cloner:clone/v1`, the
`WorldCloner` trait, its request types and the `cloner()` lookup. Sibling of `world-cloner`; consumers
depend on it by path (`world-cloner-api = { path = "../world-cloner-api" }`).

Every `Operation` returned by a method must be polled to completion by its caller.

**Prototype:** `TemplateState`, `TemplateStatus`, `CloneTarget` and `CloneSettings` mirror host spec
`005-world-templates` and live in `src/host_prototype.rs`. Replace that module with re-exports from
`steel_extension_api` once the host spec is merged; names and layouts match, so consumers do not change.
