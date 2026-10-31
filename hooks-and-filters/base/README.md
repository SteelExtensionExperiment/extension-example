# Base action/filter extension

Registers `/hooks run`, adds five through a `Score` filter, observes the emitted
`Scored` action, and listens to server tick and stop actions. It works alone
(score `15`) or with bonus ordered before it (score `25`).

From the parent Steel checkout, build only this library:

```sh
cargo build --manifest-path extension-example/hooks-and-filters/base/Cargo.toml --release
```

Its Linux artifact is
`extension-example/hooks-and-filters/target/release/libsteel_hook_base_extension.so`.
It requires no configuration. See the [demo guide](../README.md) for installation,
packaging, expected output, and native tests.
