//! The shared renderer for declarative vanilla inventory menus.
#![recursion_limit = "256"]

use steel_extension_api::{
    ApiResult, ExtensionContext, Hooks,
    items::ItemSpec,
    services,
    stabby::{self, boxed::Box as ApiBox, string::String as ApiString},
    text::Text,
};
use steel_ui_api::{
    COLUMNS, DynRenderer, MAX_ROWS, Menu, RENDERER, RenderedMenu, Renderer, labelled_item,
};

struct InventoryRenderer;

#[stabby::export(callback)]
impl Renderer for InventoryRenderer {
    fn render(&self, menu: Menu) -> ApiResult<RenderedMenu> {
        render(&menu).into()
    }
}

fn render(menu: &Menu) -> Result<RenderedMenu, ApiString> {
    if !(1..=MAX_ROWS).contains(&menu.rows) {
        return Err(format!("UI menus require 1–{MAX_ROWS} rows").into());
    }
    let mut items = vec![ItemSpec::empty(); usize::from(menu.rows) * usize::from(COLUMNS)];
    for cell in &menu.cells {
        let slot = items
            .get_mut(usize::from(cell.slot))
            .ok_or_else(|| ApiString::from(format!("UI slot {} is outside the menu", cell.slot)))?;
        if !slot.is_empty() {
            return Err(format!("UI slot {} is occupied twice", cell.slot).into());
        }
        if cell.icon.as_str().is_empty() || cell.icon.as_str() == "minecraft:air" {
            return Err(format!("UI slot {} requires a visible icon", cell.slot).into());
        }
        *slot = labelled_item(
            cell.icon.as_str(),
            cell.label.as_str(),
            cell.tooltip.as_str(),
        );
    }
    let [red, green, blue] = menu.title_color;
    Ok(RenderedMenu {
        title: Text::new(menu.title.as_str())
            .color(format!("#{red:02x}{green:02x}{blue:02x}"))
            .italic(false)
            .to_json()
            .into(),
        rows: menu.rows,
        items: items.into_iter().collect(),
    })
}

fn pre_init(_hooks: Hooks<'_>) -> Result<(), String> {
    Ok(())
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let renderer: DynRenderer = ApiBox::new(InventoryRenderer).into();
    services::register(&context, RENDERER, 0, renderer)
}

steel_extension_api::export_extension!("steel-ui", pre_init, init);
