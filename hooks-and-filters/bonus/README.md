# Bonus filter extension

Adds a doubling filter for the shared `Score` payload, ordered before
`hook-base-example`. It supplies no command or world behavior; use the base
extension's `/hooks run` command to emit the shared filter and action.

From the parent Steel checkout, build only this library:

```sh
cargo build --manifest-path extension-example/hooks-and-filters/bonus/Cargo.toml --release
```

Its Linux artifact is
`extension-example/hooks-and-filters/target/release/libsteel_hook_bonus_extension.so`.
It requires no configuration. See the [demo guide](../README.md) for installation,
packaging, expected output, and native tests.
