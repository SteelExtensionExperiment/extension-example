use steel_ui_api::{ContainerSkin, Theme, WidgetTexture};
use steel_ui_macros::rsx;

const THEME: Theme = Theme {
    title_color: [0x22, 0x37, 0x40],
};
const FACE: ContainerSkin = ContainerSkin {
    png: include_bytes!("assets/face.png"),
};
pub(super) const METER_OFF: WidgetTexture = WidgetTexture {
    name: "meter-off",
    fallback: "minecraft:gray_stained_glass_pane",
    png: include_bytes!("assets/meter-off.png"),
};
pub(super) const CHARGE: WidgetTexture = WidgetTexture {
    name: "charge",
    fallback: "minecraft:cyan_stained_glass_pane",
    png: include_bytes!("assets/charge.png"),
};
pub(super) const CHARGE_PEAK: WidgetTexture = WidgetTexture {
    name: "charge-peak",
    fallback: "minecraft:light_blue_stained_glass_pane",
    png: include_bytes!("assets/charge-peak.png"),
};
pub(super) const HEAT: WidgetTexture = WidgetTexture {
    name: "heat",
    fallback: "minecraft:orange_stained_glass_pane",
    png: include_bytes!("assets/heat.png"),
};

// The packed face already draws the controls, including the continuous switch.
const RUN: WidgetTexture = WidgetTexture {
    name: "run",
    fallback: "minecraft:lime_dye",
    png: include_bytes!("assets/switch.png"),
};
const CLOSE: WidgetTexture = WidgetTexture {
    name: "close",
    fallback: "minecraft:barrier",
    png: RUN.png,
};
const LIVE_TEXTURES: &[WidgetTexture] = &[CHARGE, CHARGE_PEAK, HEAT];

rsx! {
    Menu {
        id: "reactor",
        title: "Pulse Reactor",
        rows: 4,
        theme: THEME,
        skin: FACE,
        frames: LIVE_TEXTURES,

        Row {
            index: 0,
            Button { column: 7, image: CLOSE, text: "Close", action: Close }
        }
        Row {
            index: 1,
            Label { column: 0, size: (1, 3), live: CHARGE, image: METER_OFF, text: "Energy" }
            Label { column: 2, live: FEED, image: METER_OFF, text: "Feed" }
            Label { column: 4, live: CORE, image: METER_OFF, text: "Reactor core" }
            Label { column: 6, live: OUTPUT, image: METER_OFF, text: "Output" }
            Label { column: 8, size: (1, 3), live: HEAT, image: METER_OFF, text: "Heat" }
        }
        Row {
            index: 2,
            Label { column: 2, live: FLUX, image: METER_OFF, text: "Flux" }
        }
        Row {
            index: 3,
            Button {
                column: 3, size: (3, 1), image: RUN, text: "Run cycle",
                tooltip: "Preview one copper refining cycle.", action: Run,
            }
        }
    }
}
