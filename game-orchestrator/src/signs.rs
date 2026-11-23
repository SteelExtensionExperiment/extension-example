//! Sign text parsing and the registry of registered signs.

#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "needs the world of a sign-edit event, which the host does not expose"
    )
)]

use std::fmt;

use crate::kits::Kit;

const QUEUE_HEADER: &str = "[Queue]";
const KIT_HEADER: &str = "[Kit]";
const QUEUE_ID_PREFIX: &str = "queue";
const KIT_ID_PREFIX: &str = "kit";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SignKind {
    Queue { game_type: String },
    Kit { game_type: String, kit: String },
}

impl SignKind {
    fn id_prefix(&self) -> &'static str {
        match self {
            Self::Queue { .. } => QUEUE_ID_PREFIX,
            Self::Kit { .. } => KIT_ID_PREFIX,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SignPos {
    pub world: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SignError {
    MissingGameType,
    MissingKit,
    UnknownGameType(String),
    UnknownKit { game_type: String, kit: String },
}

impl fmt::Display for SignError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingGameType => f.write_str("line 2 must name a game type"),
            Self::MissingKit => f.write_str("line 3 must name a kit"),
            Self::UnknownGameType(name) => write!(f, "unknown game type {name}"),
            Self::UnknownKit { game_type, kit } => write!(f, "unknown kit {game_type}/{kit}"),
        }
    }
}

/// The stored game types, as far as signs need them.
pub(crate) trait Catalog {
    /// Kits of the game type, or `None` when it does not exist.
    fn kits(&self, game_type: &str) -> Option<&[Kit]>;
}

/// Checks that the game type and kit a sign names still exist. Used when a sign is written and
/// when a registered sign is clicked, so a stale sign is reported instead of acted on.
pub(crate) fn validate(kind: &SignKind, catalog: &impl Catalog) -> Result<(), SignError> {
    let (game_type, kit) = match kind {
        SignKind::Queue { game_type } => (game_type, None),
        SignKind::Kit { game_type, kit } => (game_type, Some(kit)),
    };
    let kits = catalog
        .kits(game_type)
        .ok_or_else(|| SignError::UnknownGameType(game_type.clone()))?;
    match kit {
        Some(kit) if !kits.iter().any(|known| known.name == *kit) => Err(SignError::UnknownKit {
            game_type: game_type.clone(),
            kit: kit.clone(),
        }),
        _ => Ok(()),
    }
}

/// Parses the first lines of an edited sign. `Ok(None)` when line 1 is not one of our headers,
/// so other signs are left alone.
pub(crate) fn parse(
    lines: [&str; 4],
    catalog: &impl Catalog,
) -> Result<Option<SignKind>, SignError> {
    let [header, game_type, kit, _] = lines.map(str::trim);
    let game_type = (!game_type.is_empty()).then(|| game_type.to_owned());
    let kind = if header.eq_ignore_ascii_case(QUEUE_HEADER) {
        SignKind::Queue {
            game_type: game_type.ok_or(SignError::MissingGameType)?,
        }
    } else if header.eq_ignore_ascii_case(KIT_HEADER) {
        SignKind::Kit {
            game_type: game_type.ok_or(SignError::MissingGameType)?,
            kit: (!kit.is_empty())
                .then(|| kit.to_owned())
                .ok_or(SignError::MissingKit)?,
        }
    } else {
        return Ok(None);
    };
    validate(&kind, catalog)?;
    Ok(Some(kind))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sign {
    pub id: String,
    pub pos: SignPos,
    pub kind: SignKind,
}

/// Registered signs. `next_id` only grows and is persisted with the signs, so an id is never
/// reused, not even after a restart.
#[derive(Debug, Default)]
pub(crate) struct Signs {
    signs: Vec<Sign>,
    next_id: u32,
}

impl Signs {
    pub(crate) fn from_parts(signs: Vec<Sign>, next_id: u32) -> Self {
        Self { signs, next_id }
    }

    pub(crate) fn next_id(&self) -> u32 {
        self.next_id
    }

    pub(crate) fn list(&self) -> &[Sign] {
        &self.signs
    }

    pub(crate) fn at(&self, pos: &SignPos) -> Option<&Sign> {
        self.signs.iter().find(|sign| sign.pos == *pos)
    }

    /// Registers a sign, replacing any sign already registered at the position. Returns its id.
    pub(crate) fn register(&mut self, pos: SignPos, kind: SignKind) -> String {
        self.signs.retain(|sign| sign.pos != pos);
        self.next_id += 1;
        let id = format!("{}-{}", kind.id_prefix(), self.next_id);
        self.signs.push(Sign {
            id: id.clone(),
            pos,
            kind,
        });
        id
    }

    pub(crate) fn remove(&mut self, id: &str) -> Option<Sign> {
        let index = self.signs.iter().position(|sign| sign.id == id)?;
        Some(self.signs.remove(index))
    }

    /// Drops every sign naming `game_type`; returns how many.
    pub(crate) fn remove_game_type(&mut self, game_type: &str) -> usize {
        let before = self.signs.len();
        self.signs.retain(|sign| match &sign.kind {
            SignKind::Queue { game_type: named }
            | SignKind::Kit {
                game_type: named, ..
            } => named != game_type,
        });
        before - self.signs.len()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    impl Catalog for HashMap<String, Vec<Kit>> {
        fn kits(&self, game_type: &str) -> Option<&[Kit]> {
            self.get(game_type).map(Vec::as_slice)
        }
    }

    fn catalog() -> HashMap<String, Vec<Kit>> {
        HashMap::from([(
            "skywars".to_owned(),
            vec![Kit {
                name: "knight".into(),
            }],
        )])
    }

    fn pos(x: i32) -> SignPos {
        SignPos {
            world: "lobby:main".into(),
            x,
            y: 65,
            z: 0,
        }
    }

    #[test]
    fn parses_queue_and_kit_signs_with_any_header_case() {
        let catalog = catalog();
        assert_eq!(
            parse(["[queue]", "skywars", "", ""], &catalog),
            Ok(Some(SignKind::Queue {
                game_type: "skywars".into()
            }))
        );
        assert_eq!(
            parse([" [KIT] ", "skywars", "knight", ""], &catalog),
            Ok(Some(SignKind::Kit {
                game_type: "skywars".into(),
                kit: "knight".into()
            }))
        );
        assert_eq!(parse(["hello", "skywars", "", ""], &catalog), Ok(None));
    }

    #[test]
    fn rejects_missing_and_unknown_names() {
        let catalog = catalog();
        assert_eq!(
            parse(["[Queue]", "", "", ""], &catalog),
            Err(SignError::MissingGameType)
        );
        assert_eq!(
            parse(["[Kit]", "skywars", "", ""], &catalog),
            Err(SignError::MissingKit)
        );
        assert_eq!(
            parse(["[Queue]", "bedwars", "", ""], &catalog),
            Err(SignError::UnknownGameType("bedwars".into()))
        );
        assert_eq!(
            parse(["[Kit]", "skywars", "mage", ""], &catalog),
            Err(SignError::UnknownKit {
                game_type: "skywars".into(),
                kit: "mage".into()
            })
        );
    }

    #[test]
    fn a_registered_sign_whose_kit_was_removed_is_stale() {
        let mut catalog = catalog();
        let kind = SignKind::Kit {
            game_type: "skywars".into(),
            kit: "knight".into(),
        };
        assert_eq!(validate(&kind, &catalog), Ok(()));
        catalog.get_mut("skywars").unwrap().clear();
        assert!(matches!(
            validate(&kind, &catalog),
            Err(SignError::UnknownKit { .. })
        ));
    }

    #[test]
    fn ids_are_never_reused_and_a_position_holds_one_sign() {
        let mut signs = Signs::default();
        let queue = SignKind::Queue {
            game_type: "skywars".into(),
        };
        let first = signs.register(pos(0), queue.clone());
        assert_eq!(first, "queue-1");
        assert!(signs.remove(&first).is_some());
        assert_eq!(signs.register(pos(0), queue.clone()), "queue-2");
        assert_eq!(signs.register(pos(0), queue), "queue-3");
        assert_eq!(signs.list().len(), 1);
        assert_eq!(
            signs.at(&pos(0)).map(|sign| sign.id.as_str()),
            Some("queue-3")
        );
    }

    #[test]
    fn removing_a_game_type_drops_its_signs() {
        let mut signs = Signs::default();
        signs.register(
            pos(0),
            SignKind::Queue {
                game_type: "skywars".into(),
            },
        );
        signs.register(
            pos(1),
            SignKind::Kit {
                game_type: "skywars".into(),
                kit: "knight".into(),
            },
        );
        signs.register(
            pos(2),
            SignKind::Queue {
                game_type: "other".into(),
            },
        );
        assert_eq!(signs.remove_game_type("skywars"), 2);
        assert_eq!(signs.list().len(), 1);
    }
}
