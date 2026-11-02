//! Player-head skins that give the nuclear blocks their look without a resource pack.
//!
//! - Uranium ore: <https://minecraft-heads.com/custom-heads/head/f96a322e-3a40-4636-807b-50c46d1e95b7>
//! - Nuclear waste barrel: <https://minecraft-heads.com/custom-heads/head/3ee3a969-c3de-492d-944d-b0649a6271d0>

use steel_extension_api::content::{HeadProfile, ItemAppearance, ItemDisplay};

/// One skin and its stable profile id (the head's minecraft-heads UUID as vanilla's
/// four signed integers), so clients cache each skin once.
pub struct Skin {
    id: [i32; 4],
    texture: &'static str,
}

pub const URANIUM_ORE: Skin = Skin {
    id: [-110_480_850, 977_290_806, -2_139_402_044, 1_830_720_951],
    texture: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvOTA2MTRlM2FiZjY0ZDUzNDk2Nzk0Y2Q4YWU2ODU5N2ZjNzI2NmM2MTc5NGJkMWU0OGQ0NTE5ODY4YWUzY2FkMCJ9fX0=",
};

pub const NUCLEAR_WASTE: Skin = Skin {
    id: [
        1_055_107_433,
        -1_008_842_451,
        -1_806_847_900,
        -1_704_824_368,
    ],
    texture: "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvOWM2NDU2OGFkZWJmMjg0NGRmODI0MzMyZGMyN2U0YzY2YThhOWFmNmI0ZDc1Mjc4MmY2MTBkYjYxZDA2NSJ9fX0=",
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

    /// One head enlarged to a full block, with a small margin over the carrier.
    pub fn block_displays(&self) -> Vec<ItemDisplay> {
        // A vanilla head is half a block wide: double it, plus 0.2% to hide the carrier.
        const MODEL_SCALE: f32 = 1.002;
        vec![ItemDisplay {
            appearance: self.appearance(),
            translation: [0.5, 0.5 + MODEL_SCALE * 0.5, 0.5],
            scale: [2.0 * MODEL_SCALE; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
        }]
    }
}
