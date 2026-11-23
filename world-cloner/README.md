# world-cloner

Steel extension: load saved Steel worlds into memory once and stamp out unlimited RAM-only copies as
runtime worlds, for minigame servers. Exposes the versioned service `world-cloner:clone/v1` (crate
`world-cloner-api`) and `/worldcloner` admin commands.

**Status:** partial. Configuration parsing and validation, and the `/worldcloner` command tree with its argument parsing, are implemented. The service, template loading and command handlers wait for host spec `005-world-templates` (see `plan.md`, "Platform gap"). The spec is in [`specs/001-world-cloner/`](specs/001-world-cloner/spec.md);
the host platform it needs is specified in the Steel checkout as `specs/005-world-templates/`.

## Layout

This extension lives in `extension-example/` inside a Steel checkout, so the crates can reach the API
through `../../steel-extension-api`. The contract crate is a sibling, as with `vault` and `vault-api`:

```text
SteelExtensionMC/
├── steel-extension-api/
├── specs/                      # host platform specs and specs/extensions-workflow.md
└── extension-example/
    ├── world-cloner-api/       # contract crate
    └── world-cloner/           # extension crate
        └── specs/001-world-cloner/
```

Build with the Steel checkout's `rust-toolchain.toml`; Cargo finds it from here.
