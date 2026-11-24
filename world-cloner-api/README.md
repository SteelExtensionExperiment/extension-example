# world-cloner-api

Contract crate for the `world-cloner` extension: service key `world-cloner:clone/v1`, the
`WorldCloner` trait, its request types and the `cloner()` lookup. Sibling of `world-cloner`; consumers
depend on it by path (`world-cloner-api = { path = "../world-cloner-api" }`).

Every `Operation` returned by a method must be polled to completion by its caller.

`TemplateState`, `TemplateStatus`, `CloneTarget` and `CloneSettings` come from `steel_extension_api` (host spec `005-world-templates`) and are re-exported here.
