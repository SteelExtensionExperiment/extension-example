use steel_ui_macros::rsx;

use super::theme::{FURNACE, LIVE_TEXTURES};

rsx! {
    Menu {
        id: "furnace",
        title: "Electric Furnace",
        rows: 6,
        theme: FURNACE.menu,
        frames: LIVE_TEXTURES,

        Row {
            index: 0,
            Label {
                column: 0, live: ENERGY_HEADER, image: FURNACE.power,
                text: "Energy", tooltip: "Charge rises and resets in a loop.",
            }
            Label { column: 2, image: FURNACE.socket, text: "Feed chamber" }
            Label { column: 5, image: FURNACE.socket, text: "Output chamber" }
            Label { column: 7, image: FURNACE.recipes, text: "Recipes", tooltip: "Select a recipe below." }
            Button { column: 8, image: FURNACE.up, text: "Scroll up", action: ScrollUp }
        }
        Row {
            index: 1,
            Label { column: 0, live: ENERGY_TOP, image: FURNACE.energy[0], text: "Energy cell" }
            Label { column: 2, live: INPUT, image: FURNACE.socket, text: "Recipe input" }
            Label { column: 4, image: FURNACE.flow, text: "Smelting" }
            Label { column: 5, live: OUTPUT, image: FURNACE.socket, text: "Recipe output" }
            Button { column: 7, live: RECIPE_FIRST, image: FURNACE.recipes, text: "Recipe", action: RecipeFirst }
            Image { column: 8, live: TRACK_TOP, image: FURNACE.scroll[0] }
        }
        Row {
            index: 2,
            Label { column: 0, live: ENERGY_SECOND, image: FURNACE.energy[0], text: "Energy cell" }
            Image { column: 2, live: PROGRESS_FIRST, image: FURNACE.progress[0] }
            Image { column: 3, live: PROGRESS_SECOND, image: FURNACE.progress[0] }
            Image { column: 4, live: PROGRESS_THIRD, image: FURNACE.progress[0] }
            Image { column: 5, live: PROGRESS_LAST, image: FURNACE.progress[0] }
            Button { column: 7, live: RECIPE_SECOND, image: FURNACE.recipes, text: "Recipe", action: RecipeSecond }
            Image { column: 8, live: TRACK_MIDDLE, image: FURNACE.scroll[0] }
        }
        Row {
            index: 3,
            Label { column: 0, live: ENERGY_THIRD, image: FURNACE.energy[0], text: "Energy cell" }
            Button {
                column: 2, size: (4, 2), tiles: FURNACE.start,
                text: "Start cycle", tooltip: "Run the selected recipe preview.", action: Start,
            }
            Button { column: 7, live: RECIPE_LAST, image: FURNACE.recipes, text: "Recipe", action: RecipeLast }
            Image { column: 8, live: TRACK_BOTTOM, image: FURNACE.scroll[0] }
        }
        Row {
            index: 4,
            Label { column: 0, live: ENERGY_BOTTOM, image: FURNACE.energy[0], text: "Energy cell" }
            Button { column: 8, image: FURNACE.down, text: "Scroll down", action: ScrollDown }
        }
        Row {
            index: 5,
            Label {
                column: 2, live: STATUS, image: FURNACE.status,
                text: "Idle", tooltip: "Select a recipe, then start the cycle.",
            }
            Button { column: 8, image: FURNACE.close, text: "Close", action: Close }
        }
    }
}
