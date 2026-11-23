//! Kit model and kit choice resolution.

/// A named kit of a game type.
///
/// The item payload and game mode are added together with the lossless item snapshot of host
/// spec 007 (spec O1); until then a kit is only its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Kit {
    pub name: String,
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "used by `kit save` once the item snapshot of host spec 007 exists"
    )
)]
pub(crate) struct DuplicateKit;

/// Adds `kit`, refusing a name the game type already has.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "used by `kit save` once the item snapshot of host spec 007 exists"
    )
)]
pub(crate) fn add(kits: &mut Vec<Kit>, kit: Kit) -> Result<(), DuplicateKit> {
    if kits.iter().any(|existing| existing.name == kit.name) {
        return Err(DuplicateKit);
    }
    kits.push(kit);
    Ok(())
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
            .map(|name| Kit {
                name: (*name).to_owned(),
            })
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
    fn duplicate_names_are_rejected() {
        let mut list = kits(&["knight"]);
        assert_eq!(
            add(
                &mut list,
                Kit {
                    name: "knight".into()
                }
            ),
            Err(DuplicateKit)
        );
        assert_eq!(list.len(), 1);
        assert!(remove(&mut list, "knight").is_some());
        assert!(
            add(
                &mut list,
                Kit {
                    name: "knight".into()
                }
            )
            .is_ok()
        );
    }
}
