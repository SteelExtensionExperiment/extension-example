use crate::{
    rust,
    syntax::{Menu, ResolvedMenu},
};

const DEMO: &str = include_str!("../../demo/src/furnace.rs");

fn resolve(source: &str) -> Result<ResolvedMenu, String> {
    let file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let declaration = file
        .items
        .into_iter()
        .find_map(|item| match item {
            syn::Item::Macro(item) if item.mac.path.is_ident("rsx") => Some(item.mac.tokens),
            _ => None,
        })
        .ok_or("missing rsx! declaration")?;
    syn::parse2::<Menu>(declaration)
        .and_then(Menu::resolve)
        .map_err(|error| error.to_string())
}

#[test]
fn overlapping_and_out_of_bounds_buttons_fail_with_diagnostics() {
    let duplicate = DEMO.replace(
        "column: 7, live: RECIPE_LAST",
        "column: 5, live: RECIPE_LAST",
    );
    assert!(resolve(&duplicate).is_err_and(|error| error.contains("Slot 32 is occupied twice")));
    let too_wide = DEMO.replace("size: (4, 2)", "size: (8, 2)");
    assert!(resolve(&too_wide).is_err_and(|error| error.contains("Widget width")));
    let unknown = DEMO.replace(
        "tiles: FURNACE.start",
        "tiles: FURNACE.start, colour: FURNACE.power",
    );
    assert!(resolve(&unknown).is_err_and(|error| error.contains("unknown Button property")));
}

#[test]
fn generated_menu_uses_rust_theme_values_and_typed_actions()
-> Result<(), Box<dyn std::error::Error>> {
    let menu = resolve(DEMO)?;
    let button_slots: Vec<_> = menu
        .cells
        .iter()
        .filter(|cell| cell.action.as_ref().is_some_and(|action| action == "Start"))
        .map(|cell| cell.slot)
        .collect();
    assert_eq!(button_slots, [29, 30, 31, 32, 38, 39, 40, 41]);
    let code = rust(&menu);
    let parsed = syn::parse2::<syn::File>(code.clone())?;
    assert!(parsed.items.iter().any(|item| {
        matches!(item, syn::Item::Enum(action) if action.variants.iter().any(|variant| variant.ident == "Start"))
    }));
    let code_text = code.to_string();
    assert!(code_text.contains("FURNACE . menu"));
    assert!(code_text.contains("FURNACE . start"));
    assert!(code_text.contains("pub const RECIPE_LAST"));
    Ok(())
}

#[test]
fn image_sizes_reserve_rectangles_and_only_live_regions_can_update()
-> Result<(), Box<dyn std::error::Error>> {
    let menu = resolve(
        r#"
        rsx! { Menu {
            namespace: "demo", id: "images", title: "Images", rows: 2, theme: THEME,
            Row {
                index: 0,
                Image { column: 1, size: (2, 2), live: GAUGE, tiles: FRAMES }
                Image { column: 5, size: (2, 1), image: FACE }
                Image { column: 8, image: FACE }
            }
        } }
    "#,
    )?;
    let (live, fixed): (Vec<_>, Vec<_>) = menu.cells.iter().partition(|cell| cell.live);
    assert_eq!(
        live.iter().map(|cell| cell.slot).collect::<Vec<_>>(),
        [1, 2, 10, 11]
    );
    assert_eq!(
        fixed.iter().map(|cell| cell.slot).collect::<Vec<_>>(),
        [5, 6, 8]
    );
    let generated = rust(&menu).to_string();
    assert!(generated.contains("GAUGE : [i32 ; 4usize] = [1i32 , 2i32 , 10i32 , 11i32"));
    assert!(generated.contains("LIVE_SLOTS : & [i32] = & [1i32 , 2i32 , 10i32 , 11i32"));
    Ok(())
}
