//! Ball skins from the credited Minecraft-Heads pages; normal client skin downloads suffice.
use steel_extension_api::{
    ApiResult,
    content::{ContentRegistry, HeadProfile, ItemAppearance, ItemDefinition},
    stabby::{self, string::String as ApiString},
};
pub const KEYS: [&str; 3] = [
    "creatures:poke_ball",
    "creatures:super_ball",
    "creatures:hyper_ball",
];
pub const NAMES: [&str; 3] = ["Poké Ball", "Super Ball", "Hyper Ball"];
#[stabby::export(callback)]
pub fn register(mut registry: ContentRegistry<'_>) -> ApiResult<()> {
    let result = (|| -> Result<(), ApiString> {
        // Skin: https://minecraft-heads.com/custom-heads/head/77222-pokeball
        registry.register_item(ItemDefinition {key:"creatures:poke_ball".into(),name:"Poké Ball".into(),max_stack_size:16,appearance:ItemAppearance {item:"minecraft:player_head".into(),model:None.into(),profile:Some(HeadProfile {id:[1411816145, 1284660881, -1836113267, 1600991899],texture:"eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvOGVjYTU3YzQ2NjgxZTIxZmVlZmI1ZGMyNzkwOWVkZTk1YTNjMTE4MzU2NWNmNzQ3NGRjNDlmYzA2NDEzMGUwYyJ9fX0=".into()}).into()}}).match_owned(Ok,Err)?;
        // Skin: https://minecraft-heads.com/custom-heads/head/77224-great-ball
        registry.register_item(ItemDefinition {key:"creatures:super_ball".into(),name:"Super Ball".into(),max_stack_size:16,appearance:ItemAppearance {item:"minecraft:player_head".into(),model:None.into(),profile:Some(HeadProfile {id:[-1074413531, -1076931393, -1389437183, 1927084009],texture:"eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvZjJjYTg5MmI5N2E1YTg0NzI0OTM4Nzk2MTg5ZWJiOWQ0YTU0N2I0Yjk0NTA5YWE3Mjc3ODE3NDcwZDVkNTE0In19fQ==".into()}).into()}}).match_owned(Ok,Err)?;
        // Skin: https://minecraft-heads.com/index.php/custom-heads/head/77223-ultra-ball
        registry.register_item(ItemDefinition {key:"creatures:hyper_ball".into(),name:"Hyper Ball".into(),max_stack_size:16,appearance:ItemAppearance {item:"minecraft:player_head".into(),model:None.into(),profile:Some(HeadProfile {id:[-2087438325, 1604344271, -1078128372, -549452168],texture:"eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvNjQ4YzExNTA5ZTkzZDMwYmIwNGFiNjM5YmUyOTlkOGUyNjUwYmM4YzJlODRlMDg4Njc3Yjk2OWJjOGI1YjcyNSJ9fX0=".into()}).into()}}).match_owned(Ok,Err)?;
        Ok(())
    })();
    result.into()
}
