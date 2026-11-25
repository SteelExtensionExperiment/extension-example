//! The native Steel UI renderer contract and the runtime used by generated menus.
#![recursion_limit = "256"]

use std::collections::BTreeMap;

mod skin;
pub use skin::ContainerSkin;

use steel_extension_api::{
    ApiResult, PlayerId, Server,
    content::{ContentRegistry, ItemAppearance, ItemDefinition},
    items::ItemSpec,
    menus::MenuHandler,
    services,
    stabby::{self, string::String as ApiString, vec::Vec as ApiVec},
    text::Text,
};

pub const RENDERER: &str = "steel-ui:inventory/v2";
pub const COLUMNS: u8 = 9;
pub const MAX_ROWS: u8 = 6;

/// Styling for the whole menu, used as a Rust constant in `rsx!`.
pub struct Theme {
    pub title_color: [u8; 3],
}

/// A reusable widget icon with a vanilla fallback and an embedded PNG.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetTexture {
    pub name: &'static str,
    pub fallback: &'static str,
    pub png: &'static [u8],
}

impl WidgetTexture {
    /// Resource key for this texture in a particular menu.
    pub fn icon(self, menu: &str) -> String {
        format!("{menu}/{}", self.name)
    }

    /// Use this registered texture for a live slot update.
    pub fn item(self, menu: &str, label: &str, tooltip: &str) -> ItemSpec {
        labelled_item(&self.icon(menu), label, tooltip)
    }
}

/// An item with the same label and tooltip in both client pack modes.
pub fn labelled_item(icon: &str, label: &str, tooltip: &str) -> ItemSpec {
    let text = |value: &str| Text::new(value).italic(false).to_json();
    let mut item = ItemSpec::new(icon).name(&text(label));
    if !tooltip.is_empty() {
        item = item.lore([text(tooltip).as_str()]);
    }
    item
}

/// A positioned cell with its registered icon and tooltip.
#[stabby::stabby]
pub struct Cell {
    pub icon: ApiString,
    pub label: ApiString,
    pub tooltip: ApiString,
    pub slot: u8,
}

/// An inventory screen produced by the UI compiler.
#[stabby::stabby]
pub struct Menu {
    pub title: ApiString,
    pub cells: ApiVec<Cell>,
    pub title_color: [u8; 3],
    pub rows: u8,
}

#[stabby::stabby]
pub struct RenderedMenu {
    pub title: ApiString,
    pub items: ApiVec<ItemSpec>,
    pub rows: u8,
}

impl RenderedMenu {
    /// Opens this screen after the caller has applied any initial slot updates.
    pub fn open(
        self,
        server: &Server<'_>,
        player: PlayerId,
        handler: MenuHandler,
    ) -> Result<u64, String> {
        server.open_menu_with(
            player,
            self.title.as_str(),
            i32::from(self.rows),
            self.items,
            handler,
        )
    }
}

#[stabby::stabby(checked)]
pub trait Renderer: Send + Sync {
    extern "C" fn render(&self, menu: Menu) -> ApiResult<RenderedMenu>;
}

pub type DynRenderer = stabby::dynptr!(stabby::boxed::Box<dyn Renderer + Send + Sync>);

/// Resolve the installed renderer during extension init.
pub fn renderer() -> Result<&'static DynRenderer, String> {
    services::get(RENDERER)
}

/// Render a declaration into a title and inventory slots.
pub fn render(menu: Menu) -> Result<RenderedMenu, String> {
    renderer()?
        .render(menu)
        .match_owned(Ok, |error| Err(error.to_string()))
}

/// Open a compiled menu using Steel's read-only inventory and click router.
pub fn open(
    server: &Server<'_>,
    player: PlayerId,
    menu: Menu,
    handler: MenuHandler,
) -> Result<u64, String> {
    render(menu)?.open(server, player, handler)
}

/// Register each distinct widget texture used by a menu.
pub fn register_textures(
    registry: &mut ContentRegistry<'_>,
    menu: &str,
    textures: impl IntoIterator<Item = WidgetTexture>,
) -> Result<(), String> {
    let mut distinct = BTreeMap::new();
    for texture in textures {
        if let Some(existing) = distinct.insert(texture.name, texture)
            && texture != existing
        {
            return Err(format!(
                "different textures share the name {:?} in {menu}",
                texture.name
            ));
        }
    }
    for texture in distinct.into_values() {
        register_icon(registry, &texture.icon(menu), texture.fallback, texture.png)?;
    }
    Ok(())
}

fn register_icon(
    registry: &mut ContentRegistry<'_>,
    key: &str,
    fallback: &str,
    png: &[u8],
) -> Result<(), String> {
    registry
        .register_item_texture(key.into(), png.into())
        .match_owned(Ok, |error| Err(error.to_string()))?;
    registry
        .register_item(ItemDefinition {
            key: key.into(),
            name: " ".into(),
            appearance: ItemAppearance::new(fallback).with_model(key),
            max_stack_size: 1,
        })
        .match_owned(Ok, |error| Err(error.to_string()))
}
