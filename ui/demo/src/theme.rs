use steel_ui_api::{Theme, WidgetTexture};

pub(super) struct FurnaceTheme {
    pub menu: Theme,
    pub power: WidgetTexture,
    pub socket: WidgetTexture,
    pub flow: WidgetTexture,
    pub recipes: WidgetTexture,
    pub status: WidgetTexture,
    pub up: WidgetTexture,
    pub down: WidgetTexture,
    pub close: WidgetTexture,
    pub energy: [WidgetTexture; 3],
    pub progress: [WidgetTexture; 2],
    pub scroll: [WidgetTexture; 2],
    pub start: [WidgetTexture; 8],
}

const START_BORDER: WidgetTexture = WidgetTexture {
    name: "start-border",
    fallback: "minecraft:orange_stained_glass_pane",
    png: include_bytes!("furnace-assets/start-border.png"),
};
const START_FLAME: WidgetTexture = WidgetTexture {
    name: "start-flame",
    fallback: "minecraft:blaze_powder",
    png: include_bytes!("furnace-assets/start-flame.png"),
};

pub(super) const FURNACE: FurnaceTheme = FurnaceTheme {
    menu: Theme {
        title_color: [0x24, 0x3f, 0x45],
    },
    power: WidgetTexture {
        name: "power",
        fallback: "minecraft:glowstone_dust",
        png: include_bytes!("furnace-assets/power.png"),
    },
    socket: WidgetTexture {
        name: "socket",
        fallback: "minecraft:gray_stained_glass_pane",
        png: include_bytes!("furnace-assets/socket.png"),
    },
    flow: WidgetTexture {
        name: "flow",
        fallback: "minecraft:orange_dye",
        png: include_bytes!("furnace-assets/flow.png"),
    },
    recipes: WidgetTexture {
        name: "recipes",
        fallback: "minecraft:book",
        png: include_bytes!("furnace-assets/recipes.png"),
    },
    status: WidgetTexture {
        name: "status",
        fallback: "minecraft:clock",
        png: include_bytes!("furnace-assets/status.png"),
    },
    up: WidgetTexture {
        name: "scroll-up",
        fallback: "minecraft:feather",
        png: include_bytes!("furnace-assets/scroll-up.png"),
    },
    down: WidgetTexture {
        name: "scroll-down",
        fallback: "minecraft:feather",
        png: include_bytes!("furnace-assets/scroll-down.png"),
    },
    close: WidgetTexture {
        name: "close",
        fallback: "minecraft:barrier",
        png: include_bytes!("furnace-assets/close.png"),
    },
    energy: [
        WidgetTexture {
            name: "energy-off",
            fallback: "minecraft:gray_stained_glass_pane",
            png: include_bytes!("furnace-assets/energy-off.png"),
        },
        WidgetTexture {
            name: "energy-on",
            fallback: "minecraft:cyan_stained_glass_pane",
            png: include_bytes!("furnace-assets/energy-on.png"),
        },
        WidgetTexture {
            name: "energy-pulse",
            fallback: "minecraft:light_blue_stained_glass_pane",
            png: include_bytes!("furnace-assets/energy-pulse.png"),
        },
    ],
    progress: [
        WidgetTexture {
            name: "progress-off",
            fallback: "minecraft:brown_stained_glass_pane",
            png: include_bytes!("furnace-assets/progress-off.png"),
        },
        WidgetTexture {
            name: "progress-on",
            fallback: "minecraft:yellow_stained_glass_pane",
            png: include_bytes!("furnace-assets/progress-on.png"),
        },
    ],
    scroll: [
        WidgetTexture {
            name: "track",
            fallback: "minecraft:black_stained_glass_pane",
            png: include_bytes!("furnace-assets/track.png"),
        },
        WidgetTexture {
            name: "thumb",
            fallback: "minecraft:light_blue_stained_glass_pane",
            png: include_bytes!("furnace-assets/thumb.png"),
        },
    ],
    start: [
        START_BORDER,
        START_FLAME,
        START_FLAME,
        START_BORDER,
        START_BORDER,
        START_BORDER,
        START_BORDER,
        START_BORDER,
    ],
};

pub(super) const LIVE_TEXTURES: &[WidgetTexture] = &[
    FURNACE.energy[1],
    FURNACE.energy[2],
    FURNACE.progress[1],
    FURNACE.scroll[1],
];
