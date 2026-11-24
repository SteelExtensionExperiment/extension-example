use steel_extension_api::{
    content::{ContentRegistry, ItemAppearance, ItemDefinition},
    items::ItemSpec,
    menus::MenuItemUpdate,
    stabby::vec::Vec as ApiVec,
};
use steel_ui_api::{WidgetTexture, labelled_item};

use super::layout::{CHARGE, CHARGE_PEAK, HEAT, METER_OFF, slots};

pub(super) const LEVELS: usize = 15;
const TRACK_HEIGHT: i32 = 45;
const MODEL_TOP: i32 = 32;
const MODEL_BOTTOM: i32 = MODEL_TOP - TRACK_HEIGHT;
const MODEL_UV_SIZE: f32 = 16.0;

pub(super) const ENERGY_GAUGE: Gauge = Gauge {
    name: "energy-fill",
    label: "Energy",
    atlas: include_bytes!("assets/energy-fill.png"),
    slots: slots::CHARGE,
    filled: CHARGE,
    peak: CHARGE_PEAK,
};
pub(super) const HEAT_GAUGE: Gauge = Gauge {
    name: "heat-fill",
    label: "Heat",
    atlas: include_bytes!("assets/heat-fill.png"),
    slots: slots::HEAT,
    filled: HEAT,
    peak: HEAT,
};

pub(super) struct Gauge {
    name: &'static str,
    label: &'static str,
    atlas: &'static [u8],
    slots: [i32; 3],
    filled: WidgetTexture,
    peak: WidgetTexture,
}

impl Gauge {
    pub(super) fn register(
        &self,
        registry: &mut ContentRegistry<'_>,
        assets: &str,
    ) -> Result<(), String> {
        let (namespace, path) = assets.split_once(':').ok_or("missing reactor namespace")?;
        let texture = format!("{path}/{}", self.name);
        registry
            .add_resource_asset(
                format!("assets/{namespace}/textures/item/{texture}.png")
                    .as_str()
                    .into(),
                self.atlas.into(),
            )
            .match_owned(Ok, |error| Err(error.to_string()))?;

        for level in 1..=LEVELS {
            let frame = format!("{texture}-{level}");
            let model_key = format!("{namespace}:{frame}");
            let definition = format!(
                r#"{{"model":{{"type":"minecraft:model","model":"{namespace}:item/{frame}"}},"oversized_in_gui":true}}"#
            );

            // The mesh keeps its bounds while each level selects one atlas frame.
            let frame_height = MODEL_UV_SIZE / (LEVELS + 1) as f32;
            let top = level as f32 * frame_height;
            let bottom = (level + 1) as f32 * frame_height;
            // When centered on the middle slot, the quad spans y=37 through y=81.
            let model = format!(
                r##"{{
                    "gui_light":"front",
                    "textures":{{"fill":"{namespace}:item/{texture}","particle":"#fill"}},
                    "display":{{"gui":{{"translation":[0,1,0]}}}},
                    "elements":[{{
                        "from":[5,{MODEL_BOTTOM},7.5],"to":[11,{MODEL_TOP},8.5],
                        "faces":{{
                            "north":{{"texture":"#fill","uv":[16,{top},0,{bottom}]}},
                            "south":{{"texture":"#fill","uv":[0,{top},16,{bottom}]}}
                        }}
                    }}]
                }}"##
            );
            for (path, json) in [
                (format!("assets/{namespace}/items/{frame}.json"), definition),
                (
                    format!("assets/{namespace}/models/item/{frame}.json"),
                    model,
                ),
            ] {
                registry
                    .add_resource_asset(path.as_str().into(), json.as_bytes().into())
                    .match_owned(Ok, |error| Err(error.to_string()))?;
            }
            registry
                .register_item(ItemDefinition {
                    key: model_key.as_str().into(),
                    name: self.label.into(),
                    appearance: ItemAppearance::new(self.filled.fallback).with_model(&model_key),
                    max_stack_size: 1,
                })
                .match_owned(Ok, |error| Err(error.to_string()))?;
        }
        Ok(())
    }

    pub(super) fn clear(&self) -> impl Iterator<Item = MenuItemUpdate> {
        self.slots.into_iter().map(|slot| MenuItemUpdate {
            slot,
            item: ItemSpec::empty(),
        })
    }

    pub(super) fn update(
        &self,
        assets: &str,
        updates: &mut ApiVec<MenuItemUpdate>,
        from: usize,
        to: usize,
        packed: bool,
    ) {
        if from == to {
            return;
        }
        if packed {
            let item = if to == 0 {
                ItemSpec::empty()
            } else {
                let key = format!("{assets}/{}-{to}", self.name);
                labelled_item(&key, self.label, "")
            };
            updates.push(MenuItemUpdate {
                slot: self.slots[1],
                item,
            });
            return;
        }
        let previous = from * self.slots.len() / LEVELS;
        let filled = to * self.slots.len() / LEVELS;
        for (index, slot) in self.slots.into_iter().rev().enumerate() {
            let texture = self.segment(filled, index);
            if texture != self.segment(previous, index) {
                updates.push(MenuItemUpdate {
                    slot,
                    item: texture.item(assets, self.label, ""),
                });
            }
        }
    }

    fn segment(&self, filled: usize, index: usize) -> WidgetTexture {
        if index >= filled {
            METER_OFF
        } else if index + 1 == filled {
            self.peak
        } else {
            self.filled
        }
    }
}
