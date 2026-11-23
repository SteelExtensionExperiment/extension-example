//! The Glowing One's player-head skin.
//!
//! Skin: <https://minecraft-heads.com/custom-heads/head/9416e397-fa4a-4661-b34b-f9ecb382fcde>

use steel_extension_api::content::{HeadProfile, ItemAppearance};

/// One skin and its stable profile id (the head's minecraft-heads UUID as vanilla's
/// four signed integers), so clients cache each skin once.
pub struct Skin {
    id: [i32; 4],
    texture: &'static str,
}

pub const GLOWING_ONE: Skin = Skin {
    id: [-1_810_439_273, -95_795_615, -1_286_866_452, -1_283_261_218],
    texture: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvODUyMzgzMmJmNjAyMjM2NjJiMGY3NzlhNjk4NjMwYzFhOWYxODRmNzkwMGM5OTI3MjlhMTMzZjRmMDJkYmY2NCJ9fX0=",
};

impl Skin {
    /// The head item showing this skin, for inventories and models.
    pub fn appearance(&self) -> ItemAppearance {
        ItemAppearance {
            item: "minecraft:player_head".into(),
            model: None.into(),
            profile: Some(HeadProfile {
                id: self.id,
                texture: self.texture.into(),
            })
            .into(),
        }
    }
}
