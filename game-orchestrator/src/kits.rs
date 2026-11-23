//! Kit model and kit choice resolution.

use steel_extension_api::inventory_snapshot::InventorySnapshot;
#[cfg(test)]
use steel_extension_api::{
    inventory_snapshot::INVENTORY_SNAPSHOT_FORMAT, stabby::vec::Vec as ApiVec,
};

/// A named kit of a game type: a snapshot of an administrator's inventory (slots 0-40, selected
/// hotbar slot and game mode), kept as the opaque bytes the host produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Kit {
    pub name: String,
    pub snapshot: InventorySnapshot,
}

impl Kit {
    #[cfg(test)]
    pub(crate) fn named(name: impl Into<String>) -> Self {
        let snapshot = InventorySnapshot {
            format: INVENTORY_SNAPSHOT_FORMAT,
            data: ApiVec::new(),
        };
        Self {
            name: name.into(),
            snapshot,
        }
    }
}

/// Stores `kit`, replacing an earlier kit of the same name; true when it replaced one.
pub(crate) fn save(kits: &mut Vec<Kit>, kit: Kit) -> bool {
    if let Some(existing) = kits.iter_mut().find(|existing| existing.name == kit.name) {
        *existing = kit;
        return true;
    }
    kits.push(kit);
    false
}

/// Removes the kit called `name`, returning it.
pub(crate) fn remove(kits: &mut Vec<Kit>, name: &str) -> Option<Kit> {
    let index = kits.iter().position(|kit| kit.name == name)?;
    Some(kits.remove(index))
}

/// The kit a player gets: their choice while it still exists, else the only kit of a game type
/// with exactly one, else none (the player must choose).
pub(crate) fn resolve<'a>(kits: &'a [Kit], chosen: Option<&str>) -> Option<&'a Kit> {
    let chosen = chosen.and_then(|name| kits.iter().find(|kit| kit.name == name));
    chosen.or(match kits {
        [only] => Some(only),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kits(names: &[&str]) -> Vec<Kit> {
        names
            .iter()
            .map(|name| Kit::named((*name).to_owned()))
            .collect()
    }

    #[test]
    fn choice_must_exist() {
        let kits = kits(&["knight", "archer"]);
        assert_eq!(
            resolve(&kits, Some("archer")).map(|kit| kit.name.as_str()),
            Some("archer")
        );
        assert_eq!(resolve(&kits, Some("mage")), None);
        assert_eq!(resolve(&kits, None), None);
    }

    #[test]
    fn single_kit_is_selected_without_choice_or_after_its_choice_was_removed() {
        let kits = kits(&["knight"]);
        assert_eq!(
            resolve(&kits, None).map(|kit| kit.name.as_str()),
            Some("knight")
        );
        assert_eq!(
            resolve(&kits, Some("removed")).map(|kit| kit.name.as_str()),
            Some("knight")
        );
    }

    #[test]
    fn saving_replaces_a_kit_of_the_same_name() {
        let mut list = kits(&["knight"]);
        let mut changed = Kit::named("knight");
        changed.snapshot.format += 1;
        assert!(save(&mut list, changed.clone()));
        assert_eq!(list, vec![changed]);
        assert!(!save(&mut list, Kit::named("archer")));
        assert_eq!(list.len(), 2);
        assert!(remove(&mut list, "knight").is_some());
        assert!(list.iter().all(|kit| kit.name != "knight"));
    }
}
