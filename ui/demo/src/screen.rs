use steel_extension_api::{items::Glint, menus::MenuItemUpdate, stabby::vec::Vec as ApiVec};
use steel_ui_api::{WidgetTexture, labelled_item};

use crate::{furnace::slots, theme::FURNACE};

pub(super) const TICKS_PER_FRAME: u64 = 5;
const FRAMES_PER_PROGRESS_CELL: usize = 3;
const ENERGY_SLOTS: [i32; 4] = [
    slots::ENERGY_BOTTOM,
    slots::ENERGY_THIRD,
    slots::ENERGY_SECOND,
    slots::ENERGY_TOP,
];
const PROGRESS_SLOTS: [i32; 4] = [
    slots::PROGRESS_FIRST,
    slots::PROGRESS_SECOND,
    slots::PROGRESS_THIRD,
    slots::PROGRESS_LAST,
];
const RECIPE_SLOTS: [i32; 3] = [
    slots::RECIPE_FIRST,
    slots::RECIPE_SECOND,
    slots::RECIPE_LAST,
];
const TRACK_SLOTS: [i32; 3] = [slots::TRACK_TOP, slots::TRACK_MIDDLE, slots::TRACK_BOTTOM];
const MAX_SCROLL: usize = RECIPES.len() - RECIPE_SLOTS.len();
const CYCLE_FRAMES: usize = PROGRESS_SLOTS.len() * FRAMES_PER_PROGRESS_CELL;

struct Recipe {
    name: &'static str,
    input: &'static str,
    output: &'static str,
}

const RECIPES: [Recipe; 6] = [
    Recipe {
        name: "Iron ingot",
        input: "minecraft:raw_iron",
        output: "minecraft:iron_ingot",
    },
    Recipe {
        name: "Gold ingot",
        input: "minecraft:raw_gold",
        output: "minecraft:gold_ingot",
    },
    Recipe {
        name: "Copper ingot",
        input: "minecraft:raw_copper",
        output: "minecraft:copper_ingot",
    },
    Recipe {
        name: "Glass",
        input: "minecraft:sand",
        output: "minecraft:glass",
    },
    Recipe {
        name: "Stone",
        input: "minecraft:cobblestone",
        output: "minecraft:stone",
    },
    Recipe {
        name: "Brick",
        input: "minecraft:clay_ball",
        output: "minecraft:brick",
    },
];

#[derive(Clone, Copy)]
enum Cycle {
    Idle,
    Running(usize),
    Complete,
}

impl Cycle {
    const fn progress(self) -> usize {
        match self {
            Self::Idle => 0,
            Self::Running(frame) => frame / FRAMES_PER_PROGRESS_CELL,
            Self::Complete => PROGRESS_SLOTS.len(),
        }
    }
}

pub(super) struct Screen {
    assets: &'static str,
    charge: usize,
    offset: usize,
    selected: usize,
    cycle: Cycle,
}

impl Screen {
    pub(super) const fn new(assets: &'static str) -> Self {
        Self {
            assets,
            charge: 0,
            offset: 0,
            selected: 0,
            cycle: Cycle::Idle,
        }
    }

    pub(super) fn initial(&self) -> ApiVec<MenuItemUpdate> {
        let mut updates = ApiVec::new();
        self.recipe_list(&mut updates);
        self.recipe_preview(&mut updates);
        self.energy_header(&mut updates);
        updates
    }

    pub(super) fn advance(&mut self) -> ApiVec<MenuItemUpdate> {
        let mut updates = ApiVec::new();
        let previous = self.charge;
        self.charge = (self.charge + 1) % (ENERGY_SLOTS.len() + 1);
        self.energy_header(&mut updates);
        for (index, slot) in ENERGY_SLOTS.into_iter().enumerate() {
            let next = energy_texture(self.charge, index);
            if next != energy_texture(previous, index) {
                updates.push(self.textured_slot(slot, next, "Energy cell", ""));
            }
        }
        if let Cycle::Running(frame) = self.cycle {
            let previous = self.cycle.progress();
            let next = frame + 1;
            self.cycle = if next >= CYCLE_FRAMES {
                Cycle::Complete
            } else {
                Cycle::Running(next)
            };
            self.progress_change(previous, &mut updates);
            if matches!(self.cycle, Cycle::Complete) {
                updates.push(self.status());
            }
        }
        updates
    }

    pub(super) fn scroll(&mut self, down: bool) -> ApiVec<MenuItemUpdate> {
        let previous = self.offset;
        self.offset = if down {
            (previous + 1).min(MAX_SCROLL)
        } else {
            previous.saturating_sub(1)
        };
        let mut updates = ApiVec::new();
        if self.offset != previous {
            self.recipe_list(&mut updates);
        }
        updates
    }

    pub(super) fn select(&mut self, row: usize) -> ApiVec<MenuItemUpdate> {
        let selected = self.offset + row;
        let mut updates = ApiVec::new();
        if selected == self.selected {
            return updates;
        }
        let previous = self.cycle.progress();
        self.selected = selected;
        self.cycle = Cycle::Idle;
        self.recipe_list(&mut updates);
        self.recipe_preview(&mut updates);
        self.progress_change(previous, &mut updates);
        updates
    }

    pub(super) fn start(&mut self) -> ApiVec<MenuItemUpdate> {
        let mut updates = ApiVec::new();
        if !matches!(self.cycle, Cycle::Running(_)) {
            let previous = self.cycle.progress();
            self.cycle = Cycle::Running(0);
            self.progress_change(previous, &mut updates);
            updates.push(self.status());
        }
        updates
    }

    fn energy_header(&self, updates: &mut ApiVec<MenuItemUpdate>) {
        updates.push(self.textured_slot(
            slots::ENERGY_HEADER,
            FURNACE.power,
            "Energy",
            &format!(
                "{} / {} cells charged · Looping demo",
                self.charge,
                ENERGY_SLOTS.len()
            ),
        ));
    }

    fn recipe_list(&self, updates: &mut ApiVec<MenuItemUpdate>) {
        for (row, slot) in RECIPE_SLOTS.into_iter().enumerate() {
            let index = self.offset + row;
            let recipe = &RECIPES[index];
            let selected = index == self.selected;
            let item = labelled_item(
                recipe.output,
                recipe.name,
                if selected {
                    "Selected recipe"
                } else {
                    "Click to select this recipe"
                },
            )
            .glint(if selected { Glint::On } else { Glint::Off });
            updates.push(MenuItemUpdate { item, slot });
        }
        let thumb = self.offset * (TRACK_SLOTS.len() - 1) / MAX_SCROLL;
        for (row, slot) in TRACK_SLOTS.into_iter().enumerate() {
            let active = row == thumb;
            updates.push(self.textured_slot(
                slot,
                FURNACE.scroll[usize::from(active)],
                if active { "Recipe list position" } else { " " },
                if active {
                    "Use the arrows above and below to scroll."
                } else {
                    ""
                },
            ));
        }
    }

    fn recipe_preview(&self, updates: &mut ApiVec<MenuItemUpdate>) {
        let recipe = &RECIPES[self.selected];
        for (slot, key, label) in [
            (slots::INPUT, recipe.input, "Recipe input"),
            (slots::OUTPUT, recipe.output, "Recipe output"),
        ] {
            updates.push(MenuItemUpdate {
                item: labelled_item(key, label, recipe.name),
                slot,
            });
        }
        updates.push(self.status());
    }

    fn progress_change(&self, previous: usize, updates: &mut ApiVec<MenuItemUpdate>) {
        let lit = self.cycle.progress();
        for (index, slot) in PROGRESS_SLOTS.into_iter().enumerate() {
            let active = index < lit;
            if active != (index < previous) {
                updates.push(self.textured_slot(
                    slot,
                    FURNACE.progress[usize::from(active)],
                    "Smelting progress",
                    "",
                ));
            }
        }
    }

    fn status(&self) -> MenuItemUpdate {
        let (name, tip) = match self.cycle {
            Cycle::Idle => ("Idle", "Select a recipe, then start the cycle."),
            Cycle::Running(_) => ("Smelting preview", RECIPES[self.selected].name),
            Cycle::Complete => ("Cycle complete", RECIPES[self.selected].name),
        };
        self.textured_slot(slots::STATUS, FURNACE.status, name, tip)
    }

    fn textured_slot(
        &self,
        slot: i32,
        texture: WidgetTexture,
        label: &str,
        tooltip: &str,
    ) -> MenuItemUpdate {
        MenuItemUpdate {
            item: texture.item(self.assets, label, tooltip),
            slot,
        }
    }
}

fn energy_texture(charge: usize, index: usize) -> WidgetTexture {
    if index >= charge {
        FURNACE.energy[0]
    } else if index + 1 == charge {
        FURNACE.energy[2]
    } else {
        FURNACE.energy[1]
    }
}
