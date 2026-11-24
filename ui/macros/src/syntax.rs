use std::collections::BTreeSet;
use steel_ui_api::{COLUMNS, MAX_ROWS};
use syn::{
    Error, Expr, Ident, LitInt, LitStr,
    parse::{Parse, ParseStream},
    parse_quote,
};

use super::element::{Element, SlotSize};

pub(super) struct Menu {
    namespace: LitStr,
    id: LitStr,
    title: LitStr,
    rows: LitInt,
    theme: Expr,
    skin: Option<Expr>,
    frames: Option<Expr>,
    children: Vec<Row>,
}

struct Row {
    index: LitInt,
    widgets: Vec<Widget>,
}

struct Widget {
    column: LitInt,
    textures: WidgetTextures,
    size: Option<SlotSize>,
    name: Option<Ident>,
    text: Option<LitStr>,
    tooltip: Option<LitStr>,
    action: Option<Ident>,
}

enum WidgetTextures {
    Uniform(Expr),
    Tiles(Expr),
}

impl Parse for Menu {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse::<Ident>()?;
        if name != "Menu" {
            return Err(Error::new_spanned(name, "expected Menu"));
        }
        let mut element = Element::parse_body(name, input)?;
        Ok(Self {
            namespace: element.required("namespace")?,
            id: element.required("id")?,
            title: element.required("title")?,
            rows: element.required("rows")?,
            theme: element.required("theme")?,
            skin: element.optional("skin")?,
            frames: element.optional("frames")?,
            children: element
                .finish()?
                .into_iter()
                .map(Row::parse)
                .collect::<syn::Result<_>>()?,
        })
    }
}

impl Row {
    fn parse(mut element: Element) -> syn::Result<Self> {
        if element.name != "Row" {
            return Err(Error::new_spanned(
                element.name,
                "Menu only accepts Row children",
            ));
        }
        let index = element.required("index")?;
        let widgets = element
            .finish()?
            .into_iter()
            .map(Widget::parse)
            .collect::<syn::Result<_>>()?;
        Ok(Self { index, widgets })
    }
}

impl Widget {
    fn parse(mut element: Element) -> syn::Result<Self> {
        let (text, tooltip, action) = match element.name.to_string().as_str() {
            "Button" => {
                let action = element.required::<Ident>("action")?;
                if !action
                    .to_string()
                    .starts_with(|ch: char| ch.is_ascii_uppercase())
                {
                    return Err(Error::new_spanned(
                        action,
                        "button action must start with an uppercase letter",
                    ));
                }
                (
                    Some(element.required("text")?),
                    element.optional("tooltip")?,
                    Some(action),
                )
            }
            "Label" => (
                Some(element.required("text")?),
                element.optional("tooltip")?,
                None,
            ),
            "Image" => (None, None, None),
            _ => {
                return Err(Error::new_spanned(
                    element.name,
                    "Row accepts Button, Label, and Image",
                ));
            }
        };
        let column = element.required("column")?;
        let size = element.optional("size")?;
        let name = element.optional("live")?;
        let textures = match (element.optional("image")?, element.optional("tiles")?) {
            (Some(image), None) => WidgetTextures::Uniform(image),
            (None, Some(tiles)) => WidgetTextures::Tiles(tiles),
            _ => {
                return Err(Error::new_spanned(
                    &element.name,
                    "expected exactly one of image or tiles",
                ));
            }
        };
        if let Some(child) = element.finish()?.first() {
            return Err(Error::new_spanned(
                &child.name,
                "widgets cannot contain children",
            ));
        }
        Ok(Self {
            column,
            textures,
            size,
            name,
            text,
            tooltip,
            action,
        })
    }
}

pub(super) struct ResolvedMenu {
    pub namespace: String,
    pub id: String,
    pub title: String,
    pub theme: Expr,
    pub skin: Option<Expr>,
    pub frames: Option<Expr>,
    pub rows: u8,
    pub cells: Vec<Cell>,
}

pub(super) struct Cell {
    pub slot: u8,
    pub texture: Expr,
    pub text: String,
    pub tooltip: String,
    pub action: Option<Ident>,
    pub name: Option<NamedSlots>,
    pub live: bool,
}

pub(super) struct NamedSlots {
    pub name: Ident,
    pub width: u8,
    pub height: u8,
}

impl Menu {
    pub(super) fn resolve(self) -> syn::Result<ResolvedMenu> {
        let namespace = self.namespace.value();
        if !valid_segment(&namespace) || namespace == "minecraft" {
            return Err(Error::new_spanned(
                &self.namespace,
                "Menu namespace must be an extension namespace",
            ));
        }
        let id = self.id.value();
        if !valid_segment(&id) {
            return Err(Error::new_spanned(
                &self.id,
                "Menu id must be a nonempty lowercase resource path segment",
            ));
        }
        let title = self.title.value();
        if title.trim().is_empty() {
            return Err(Error::new_spanned(
                &self.title,
                "Menu title must be nonempty",
            ));
        }
        let rows = self.rows.base10_parse::<u8>()?;
        if !(1..=MAX_ROWS).contains(&rows) {
            return Err(Error::new_spanned(
                &self.rows,
                format!("Menu rows must be between 1 and {MAX_ROWS}"),
            ));
        }
        let mut slots = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut cells = Vec::new();
        for row in self.children {
            let index = row.index.base10_parse::<u8>()?;
            if index >= rows {
                return Err(Error::new_spanned(&row.index, "Row is outside the menu"));
            }
            for widget in row.widgets {
                let column = widget.column.span();
                for cell in widget.resolve(index, rows)? {
                    if let Some(region) = &cell.name
                        && !names.insert(region.name.to_string())
                    {
                        return Err(Error::new_spanned(&region.name, "duplicate live slot name"));
                    }
                    if !slots.insert(cell.slot) {
                        return Err(Error::new(
                            column,
                            format!("Slot {} is occupied twice", cell.slot),
                        ));
                    }
                    cells.push(cell);
                }
            }
        }
        if cells.is_empty() {
            return Err(Error::new_spanned(
                &self.rows,
                "Menu must declare at least one widget",
            ));
        }
        Ok(ResolvedMenu {
            namespace,
            id,
            title,
            theme: self.theme,
            skin: self.skin,
            frames: self.frames,
            rows,
            cells,
        })
    }
}

impl Widget {
    fn resolve(self, row: u8, rows: u8) -> syn::Result<Vec<Cell>> {
        let column = self.column.base10_parse::<u8>()?;
        if column >= COLUMNS {
            return Err(Error::new_spanned(
                &self.column,
                "Column is outside the menu",
            ));
        }
        let (width, height) = match self.size {
            Some(size) => (
                span(&size.width, COLUMNS - column, "width")?,
                span(&size.height, rows - row, "height")?,
            ),
            None => (1, 1),
        };
        let text = if let Some(literal) = self.text {
            let value = literal.value();
            if value.trim().is_empty() {
                return Err(Error::new_spanned(
                    literal,
                    "Labels and buttons require text",
                ));
            }
            value
        } else {
            " ".into()
        };
        let tooltip = self
            .tooltip
            .map_or_else(String::new, |tooltip| tooltip.value());
        let count = usize::from(width) * usize::from(height);
        let mut cells = Vec::with_capacity(count);
        for y in 0..height {
            for x in 0..width {
                let texture = match &self.textures {
                    WidgetTextures::Uniform(texture) => texture.clone(),
                    WidgetTextures::Tiles(tiles) => {
                        let index = usize::from(y) * usize::from(width) + usize::from(x);
                        parse_quote! {{
                            let row_tiles: [::steel_ui_api::WidgetTexture; #count] = #tiles;
                            row_tiles[#index]
                        }}
                    }
                };
                cells.push(Cell {
                    slot: (row + y) * COLUMNS + column + x,
                    texture,
                    text: text.clone(),
                    tooltip: tooltip.clone(),
                    action: self.action.clone(),
                    live: self.name.is_some(),
                    name: if x == 0 && y == 0 {
                        self.name.clone().map(|name| NamedSlots {
                            name,
                            width,
                            height,
                        })
                    } else {
                        None
                    },
                });
            }
        }
        Ok(cells)
    }
}

fn span(literal: &LitInt, available: u8, dimension: &str) -> syn::Result<u8> {
    let size = literal.base10_parse()?;
    if !(1..=available).contains(&size) {
        return Err(Error::new_spanned(
            literal,
            format!("Widget {dimension} must be between 1 and {available} here"),
        ));
    }
    Ok(size)
}

pub(super) fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}
