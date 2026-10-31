# Actions, filters, and hook ordering

This example shows the action/filter system across two independently loaded
extensions. A player runs `/hooks run` to transform a score through a filter
chain, then emit an action announcing the result. The reply shows the result,
the action observer's value and event count, and the most recent server tick.

It demonstrates the `try-plugin` ideas on top of the checked native boundary:

| Part | Responsibility |
| --- | --- |
| [contract](contract/src/lib.rs) | Shared ABI-stable `Score` filter and `Scored` action payloads. This is a library, not an installed extension. |
| [base](base/src/lib.rs) | Register `/hooks run`, add five through a filter, observe `Scored`, and subscribe to built-in server tick and stop actions. |
| [bonus](bonus/src/lib.rs) | Double the score through a filter ordered before the base extension. |

The command starts with `10`. With both extensions, bonus doubles first and base
then adds five, producing `25`. Without ordering constraints, loading base first
would produce `30`. With only base installed, the result is `15`.

## Build and install

From the parent Steel checkout:

```sh
cargo build --manifest-path extension-example/hooks-and-filters/Cargo.toml --release
```

This isolated workspace builds both extension libraries and their shared payload
contract with the API's `host` feature disabled. Its artifacts are under
`extension-example/hooks-and-filters/target/release/`.

Configure base first, then bonus, at the top level of `config/config.toml`
(Linux paths shown):

```toml
[[extensions]]
path = "extension-example/hooks-and-filters/target/release/libsteel_hook_base_extension.so"

[[extensions]]
path = "extension-example/hooks-and-filters/target/release/libsteel_hook_bonus_extension.so"
```

On macOS, use the same names with `.dylib`. On Windows, use
`steel_hook_base_extension.dll` and `steel_hook_bonus_extension.dll` without the
`lib` prefix. Neither extension needs configuration.

Start Steel, connect as a player, and run:

```text
/hooks run
```

The first reply includes `Score: 10 -> 25`, `Action observed: 25`, and
`Scored actions: 1`. Each subsequent command increments the action count. The
server-tick action supplies `Last server tick`; it stops advancing while normal
ticks are frozen. The stop action writes a summary to standard error during
orderly shutdown.

The command permits players by default and respects an explicit deny of
`hookdemo.command.hooks`. `/hookdemo:hooks run` is its namespaced form.

## Follow the hook/filter flow

1. Both extensions subscribe during `pre_init` with `Hooks::add_filter`.
2. Base requests `.after("hook-bonus-example")`; bonus requests
   `.before("hook-base-example")`. These names are the exported descriptor names,
   not Cargo crate names or file paths.
3. Steel resolves ordering before calling extension init. Both constraints
   express the same edge, so their duplication is harmless.
4. The command calls `hooks::apply_filters(&mut Score { value: 10 })` and receives
   the final value. Filters borrow the same payload exclusively, one at a time.
5. It calls `hooks::do_action(&Scored { value })`. Base's action callback observes
   the shared payload and records it; the command then displays the observer's
   value and count.

Base also uses `add_action::<ServerTickAction>` and
`add_action::<ServerStopAction>`. These are host-emitted notifications. This
example does not use the separate `register_server_tick` method, which supplies
a borrowed `Server` handle instead.

The contract crate gives both libraries the same payload definitions and tags:
`hookdemo:score/v1` and `hookdemo:scored/v1`. Steel validates payload ABI reports
before dispatch. The shared contract is compiled into the extensions and does
not need its own server configuration entry.

If you remove bonus and restart, its ordering target is absent and imposes no
constraint; base still works with result `15`. These filters change only this
example's custom score. They do not intercept vanilla damage, chat, or drops.

## Package

Build each installed extension as its own ZIP:

```sh
python3 package-extension.py extension-example/hooks-and-filters/base
python3 package-extension.py extension-example/hooks-and-filters/bonus
```

The output archives are `extensions/steel-hook-base-extension.zip` and
`extensions/steel-hook-bonus-extension.zip`. Steel can discover both automatically.
To retain the deliberately reversed load order, configure those ZIP paths in
the same base-then-bonus order as above. Do not also select their raw libraries.

## Validate native ordering

First build the extension side in debug mode, then build and run the host tests
separately:

```sh
cargo build --manifest-path extension-example/hooks-and-filters/Cargo.toml
cargo test --manifest-path extension-example/hooks-and-filters/host-test/Cargo.toml
```

The host test crate has its own workspace to prevent its `host` feature from
being unified into the extension build. One native test loads base before bonus
and checks that the result is `25`; another loads base alone and checks `15`.
They run in separate test processes because the extension registry freezes once
per process.

See the [example index](../README.MD) and the parent Steel checkout's
`doc/extensions/api.md` for the complete API reference.
