use steel_extension_api::{PlayerId, Server, content::ContentRegistry, text::Text};

use crate::RenderedMenu;

// ContainerScreen draws the chest texture at this size and places its title
// at (8, 6). The bitmap font's baseline is seven pixels below the text origin.
const CONTAINER_WIDTH: u32 = 176;
const INVENTORY_AND_FRAME_HEIGHT: u32 = 114;
const SLOT_STRIDE: u32 = 18;
const TITLE_X: i32 = 8;
const TITLE_Y: i32 = 6;
const GLYPH_BASELINE: i32 = 7;
const LEFT_GLYPH: &str = "\u{e000}";
const FACE_GLYPH: &str = "\u{e001}";

/// A full container face drawn through a menu-specific resource-pack font.
pub struct ContainerSkin {
    pub png: &'static [u8],
}

impl ContainerSkin {
    /// Registers the bitmap font and its face in the menu's namespace.
    pub fn register(
        &self,
        registry: &mut ContentRegistry<'_>,
        menu: &str,
        rows: u8,
    ) -> Result<(), String> {
        let (namespace, path) = menu
            .split_once(':')
            .ok_or("menu skin requires a namespace")?;
        let height = INVENTORY_AND_FRAME_HEIGHT + u32::from(rows) * SLOT_STRIDE;
        let decoder = png::Decoder::new(std::io::Cursor::new(self.png))
            .read_info()
            .map_err(|error| format!("invalid menu skin {menu}: {error}"))?;
        let size = decoder.info().size();
        if size != (CONTAINER_WIDTH, height) {
            return Err(format!(
                "menu skin {menu} is {}×{}; expected {CONTAINER_WIDTH}×{height}",
                size.0, size.1,
            ));
        }
        let x = -TITLE_X;
        let ascent = TITLE_Y + GLYPH_BASELINE;
        let font = format!(
            r#"{{"providers":[
                {{"type":"space","advances":{{"{LEFT_GLYPH}":{x}}}}},
                {{"type":"bitmap","file":"{namespace}:{path}.png",
                   "height":{height},"ascent":{ascent},"chars":["{FACE_GLYPH}"]}}
            ]}}"#
        );
        for (path, bytes) in [
            (
                format!("assets/{namespace}/font/{path}.json"),
                font.as_bytes(),
            ),
            (format!("assets/{namespace}/textures/{path}.png"), self.png),
        ] {
            registry
                .add_resource_asset(path.as_str().into(), bytes.into())
                .match_owned(Ok, |error| Err(error.to_string()))?;
        }
        Ok(())
    }

    /// Selects the face for a player who loaded the server pack.
    pub fn apply(
        &self,
        rendered: &mut RenderedMenu,
        server: &Server<'_>,
        player: PlayerId,
        menu: &str,
    ) -> Result<(), String> {
        let packed = server
            .player_uses_resource_pack(player)
            .match_owned(Ok, |error| Err(error.to_string()))?;
        if packed {
            rendered.title = Text::new(format!("{LEFT_GLYPH}{FACE_GLYPH}"))
                .color("#ffffff")
                .font(menu)
                .to_json()
                .into();
        }
        Ok(())
    }
}
