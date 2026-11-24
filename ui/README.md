# Steel UI

Steel UI renders interactive inventory menus on vanilla clients. Its
`rsx!` declares slot layout, images, and actions. Reusable PNGs and vanilla
fallback items define how widgets look.
The `steel-ui` extension provides the shared renderer.

## Try the demo

Run both commands from the Steel checkout.

```sh
python3 package-extension.py extension-example/ui
python3 package-extension.py extension-example/ui/demo
```

Steel discovers both packages in `extensions/` and loads the renderer first.
Join the server and run `/ui-demo open` to open the Electric Furnace. Its
left column cycles through energy levels. Use the right-side arrows to scroll
the recipes and click a recipe to preview its input and output. Click anywhere
on the orange 4×2 Start cycle button to run the smelting progress animation.
The preview uses display slots and leaves your inventory alone.

Run `/ui-demo reactor` to open the Pulse Reactor. The resource pack draws a
custom machine face over the entire container, with inset item bays and a
separate panel for your inventory. The left energy bar fills continuously and
resets. Press **Run Cycle** to fill the right heat bar and preview a copper ingot.
Players without the pack use the same controls through a standard chest menu
with vanilla item icons.

## Declare a menu

```rust
use steel_ui_api::{Theme, WidgetTexture};
use steel_ui_macros::rsx;

const THEME: Theme = Theme {
    title_color: [0x22, 0x46, 0x4d],
};
const READY: WidgetTexture = WidgetTexture {
    name: "primary",
    fallback: "minecraft:lime_dye",
    png: include_bytes!("ready.png"),
};
const ENERGY_EMPTY: WidgetTexture = WidgetTexture {
    name: "energy-empty",
    fallback: "minecraft:gray_stained_glass_pane",
    png: include_bytes!("energy-empty.png"),
};

rsx! {
    Menu {
        namespace: "my-game",
        id: "lobby",
        title: "Lobby",
        rows: 3,
        theme: THEME,
        Row {
            index: 1,
            Button { column: 4, size: (2, 1), image: READY, text: "Ready", action: Ready }
            Image { column: 0, size: (1, 2), image: ENERGY_EMPTY, live: ENERGY }
        }
    }
}
```

Set one to six rows. Rows and columns start at zero. `size: (width, height)`
is measured in slots and defaults to `(1, 1)`. Rectangles such as `(2, 1)`
and `(2, 2)` are valid if they fit in the nine-column grid and do not overlap
another widget. Every slot of a button triggers the same `action`. `image`
repeats one icon across the region. `tiles` accepts an array of images in
row order with exactly one image per slot.

Placement, size, and click actions are fixed. A widget's item and image are
also fixed by default. Add `live: NAME` to let `open_with` and `update` change
its contents. `slots::NAME` is a slot index for a 1×1 widget and a row-ordered
array for a larger region. List extra images for these updates in `frames`.
Within a menu, slots that use the same texture share one registered item model.

Set `skin: FACE` on a menu to add a `ContainerSkin` constant with an embedded
PNG. The face is 176 pixels wide and 114 + 18 × rows pixels high. The artwork
and interactive slots share the same coordinate system. The first menu slot
starts at (8, 18), with 18 pixels between slots. The generated `render`
function selects the skin for each player before you open the menu. Use this
for seamless fixed artwork across the slot gaps.

## Prepare images

The [image tool](ui-art.py) uses Python and Pillow. `tiles` takes a PNG with
16×16 pixels per slot. It reuses identical tiles and writes a Rust array you
can include with `tiles: include!("art/button.rs")`.

```sh
python3 extension-example/ui/ui-art.py tiles button.png art \
  --name button --size 2x2 --fallback minecraft:lime_dye
```

`gauge` takes a PNG of the full gauge and makes a vertical atlas of frames
from empty to full. Set the fill direction with `--direction up`, `down`,
`left`, or `right`. It requires at least one source pixel per step in the
fill direction. The Reactor selects one frame at a time from this atlas and
uses ordinary item icons in the same live region for players without the pack.

```sh
python3 extension-example/ui/ui-art.py gauge energy-fill-source.png energy-fill.png \
  --steps 15 --direction up
```

## Open and update a menu

To use the macro in your own extension, add `steel-ui-macros` and `steel-ui-api`
as dependencies. Declare `depends: ["steel-ui"]` in `export_extension!`.
Place one `rsx!` declaration in each menu module. It generates
`register_assets` for content registration and `open` or `open_with` for
opening the screen. Register a `MenuHandler` once and match on `action(click)`
in its callback. Pass the returned menu ID to the generated `update` function
to change a live region. The [Furnace](demo/src/furnace.rs) and
[Reactor](demo/src/reactor/layout.rs) show both kinds of regions.

## Validate

```sh
cargo test --manifest-path extension-example/ui/Cargo.toml --workspace
cargo build --manifest-path extension-example/ui/Cargo.toml --workspace
cargo test -p steel-extension-api --features host --test ui -- --ignored
```
