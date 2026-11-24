//! Expand declarative inventory menus into Rust.

mod element;
mod syntax;

use std::collections::BTreeMap;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::quote;
use syn::{Error, Ident, parse_macro_input};

use syntax::{Menu, ResolvedMenu};

#[cfg(test)]
mod tests;

/// Declare an inventory menu with typed Rust theme and texture constants.
///
/// The declaration generates asset registration, rendering, and typed click actions.
#[proc_macro]
pub fn rsx(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Menu);
    input
        .resolve()
        .map(|menu| rust(&menu))
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn rust(menu: &ResolvedMenu) -> Tokens {
    let id = &menu.id;
    let title = &menu.title;
    let theme = &menu.theme;
    let frames = menu
        .frames
        .as_ref()
        .map_or_else(|| quote!(&[]), |frames| quote!(#frames));
    let rows = menu.rows;
    let asset_key = Ident::new("__STEEL_UI_ASSET_KEY", Span::mixed_site());
    let theme_const = Ident::new("__STEEL_UI_THEME", Span::mixed_site());
    let textures_const = Ident::new("__STEEL_UI_TEXTURES", Span::mixed_site());
    let frames_const = Ident::new("__STEEL_UI_FRAMES", Span::mixed_site());
    let (skin_const, register_skin, render_body) = match &menu.skin {
        Some(skin) => {
            let name = Ident::new("__STEEL_UI_SKIN", Span::mixed_site());
            (
                quote!(const #name: ::steel_ui_api::ContainerSkin = #skin;),
                quote!(#name.register(registry, &key, #rows)?;),
                quote! {
                    let mut rendered = ::steel_ui_api::render(menu()?)?;
                    #name.apply(&mut rendered, server, player, asset_key()?)?;
                    Ok(rendered)
                },
            )
        }
        None => (
            Tokens::new(),
            Tokens::new(),
            quote! {
                let _ = (server, player);
                ::steel_ui_api::render(menu()?)
            },
        ),
    };
    let textures = menu.cells.iter().map(|cell| &cell.texture);
    let cells = menu.cells.iter().enumerate().map(|(index, cell)| {
        let slot = cell.slot;
        let label = &cell.text;
        let tooltip = &cell.tooltip;
        quote! {
            ::steel_ui_api::Cell {
                slot: #slot,
                icon: #textures_const[#index].icon(key).into(),
                label: #label.into(),
                tooltip: #tooltip.into(),
            }
        }
    });
    let mut actions = BTreeMap::new();
    let click_arms = menu
        .cells
        .iter()
        .filter_map(|cell| {
            let slot = i32::from(cell.slot);
            let action = cell.action.as_ref()?;
            actions.insert(action.to_string(), action);
            Some(quote! { #slot => Some(Action::#action), })
        })
        .collect::<Vec<_>>();
    let variants = actions.values();
    let named_slots = menu.cells.iter().filter_map(|cell| {
        let region = cell.name.as_ref()?;
        let name = &region.name;
        let anchor = i32::from(cell.slot);
        let count = usize::from(region.width) * usize::from(region.height);
        if count == 1 {
            return Some(quote! { pub const #name: i32 = #anchor; });
        }
        let slots = (0..i32::from(region.height)).flat_map(|row| {
            (0..i32::from(region.width))
                .map(move |column| anchor + row * i32::from(steel_ui_api::COLUMNS) + column)
        });
        Some(quote! { pub const #name: [i32; #count] = [#(#slots,)*]; })
    });
    let live_slots = menu
        .cells
        .iter()
        .filter(|cell| cell.live)
        .map(|cell| i32::from(cell.slot));
    quote! {
        const #theme_const: ::steel_ui_api::Theme = #theme;
        const #textures_const: &[::steel_ui_api::WidgetTexture] = &[#(#textures,)*];
        const #frames_const: &[::steel_ui_api::WidgetTexture] = #frames;
        const LIVE_SLOTS: &[i32] = &[#(#live_slots,)*];
        #skin_const

        static #asset_key: ::std::sync::OnceLock<String> = ::std::sync::OnceLock::new();

        /// Extension-owned key used to register the assets of this menu.
        pub fn asset_key() -> Result<&'static str, String> {
            #asset_key.get().map(String::as_str)
                .ok_or_else(|| format!("menu {:?} assets have not been registered", #id))
        }

        pub mod slots {
            #(#named_slots)*
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Action {
            #(#variants,)*
        }

        pub fn register_assets(
            registry: &mut ::steel_extension_api::content::ContentRegistry<'_>,
        ) -> Result<(), String> {
            let key = registry.claim_menu_assets(#id.into())
                .match_owned(Ok, |error| Err(error.to_string()))?
                .to_string();
            #register_skin
            ::steel_ui_api::register_textures(
                registry,
                &key,
                #textures_const.iter().chain(#frames_const).copied(),
            )?;
            #asset_key.set(key).map_err(|key| format!("menu assets already registered for {key}"))
        }

        pub fn render(
            server: &::steel_extension_api::Server<'_>,
            player: ::steel_extension_api::PlayerId,
        ) -> Result<::steel_ui_api::RenderedMenu, String> {
            #render_body
        }

        pub fn menu() -> Result<::steel_ui_api::Menu, String> {
            let key = asset_key()?;
            Ok(::steel_ui_api::Menu {
                title: #title.into(),
                title_color: #theme_const.title_color,
                rows: #rows,
                cells: vec![#(#cells,)*].into_iter().collect(),
            })
        }

        pub fn open(
            server: &::steel_extension_api::Server<'_>,
            player: ::steel_extension_api::PlayerId,
            handler: ::steel_extension_api::menus::MenuHandler,
        ) -> Result<u64, String> {
            render(server, player)?.open(server, player, handler)
        }

        /// Open with values for the declared live regions.
        pub fn open_with(
            server: &::steel_extension_api::Server<'_>,
            player: ::steel_extension_api::PlayerId,
            handler: ::steel_extension_api::menus::MenuHandler,
            updates: impl IntoIterator<Item = ::steel_extension_api::menus::MenuItemUpdate>,
        ) -> Result<u64, String> {
            let mut rendered = render(server, player)?;
            for update in updates {
                let slot = live_slot(update.slot)?;
                let item = rendered.items.get_mut(slot)
                    .ok_or_else(|| format!("rendered menu is missing live slot {slot}"))?;
                *item = update.item;
            }
            rendered.open(server, player, handler)
        }

        /// Change the items in declared live regions.
        pub fn update(
            server: &::steel_extension_api::Server<'_>,
            player: ::steel_extension_api::PlayerId,
            menu: u64,
            updates: ::steel_extension_api::stabby::vec::Vec<::steel_extension_api::menus::MenuItemUpdate>,
        ) -> Result<bool, String> {
            for update in &updates {
                live_slot(update.slot)?;
            }
            server.update_menu(player, menu, updates).match_owned(Ok, |error| Err(error.to_string()))
        }

        fn live_slot(slot: i32) -> Result<usize, String> {
            if !LIVE_SLOTS.contains(&slot) {
                return Err(format!("UI slot {slot} is fixed"));
            }
            Ok(slot as usize)
        }

        pub fn action(click: ::steel_extension_api::menus::MenuClick) -> Option<Action> {
            if click.kind != ::steel_extension_api::events::ClickKind::Pickup
                || !matches!(click.button, 0 | 1)
            {
                return None;
            }
            match click.slot {
                #(#click_arms)*
                _ => None,
            }
        }
    }
}
