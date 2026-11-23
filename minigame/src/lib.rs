//! Shared room management for minigame extensions.
#![recursion_limit = "256"]

mod room;

use std::sync::atomic::{AtomicU64, Ordering};

use minigame_api::{DynRoom, DynRooms, ROOMS, RoomDefinition, Rooms};
use steel_extension_api::{
    ApiResult, ExtensionContext, Hooks, services,
    stabby::{self, boxed::Box as ApiBox, string::String as ApiString},
};

struct RoomProvider(AtomicU64);

#[stabby::export(callback)]
impl Rooms for RoomProvider {
    fn create(&self, definition: RoomDefinition) -> ApiResult<DynRoom> {
        let result: Result<DynRoom, ApiString> = (|| {
            if definition.capacity == 0 {
                return Err("room capacity must be positive".into());
            }
            let prefix = definition.domain_prefix.as_str();
            if prefix.is_empty() {
                return Err("room domain prefix must be nonempty".into());
            }
            let previous = self
                .0
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .map_err(|_| ApiString::from("minigame room identities exhausted"))?;
            let domain = format!("{prefix}_{}", previous + 1);
            Ok(ApiBox::new(room::GameRoom::new(domain, definition)).into())
        })();
        result.into()
    }
}

fn pre_init(_hooks: Hooks<'_>) -> Result<(), String> {
    Ok(())
}

fn init(context: ExtensionContext<'_>) -> Result<(), String> {
    let provider: DynRooms = ApiBox::new(RoomProvider(AtomicU64::new(0))).into();
    services::register(&context, ROOMS, 0, provider)
}

steel_extension_api::export_extension!("minigame", pre_init, init);
