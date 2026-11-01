//! Regions, flags and the rules that decide what a player may do at a position.
//!
//! Simplified from WorldGuard 7 (documented differences):
//! - Regions are cuboids only; there are no region groups on flags.
//! - Build-type flags (`build`, `block-break`, `block-place`, `interact`, `chest-access`) only
//!   restrict non-members: owners and members of an applicable region of the winning priority
//!   are always allowed. Without an explicit value, non-members are denied inside any region.
//! - Among the highest-priority regions that set a flag (directly or through parents), `deny`
//!   wins over `allow`. The world's `__global__` region supplies values outside regions and
//!   for flags no region sets.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use steel_extension_api::{BlockPos, players::Uuid};

/// Id of the per-world region holding default flags.
pub const GLOBAL: &str = "__global__";

/// How a flag value is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagKind {
    /// `allow` or `deny`, restricting non-members only.
    Build,
    /// `allow` or `deny`, applying to everyone.
    State,
    /// `allow` or `deny` for non-members crossing a region boundary.
    Boundary,
    /// Free text with `&` color codes.
    Text,
}

/// Every supported flag with its kind.
pub const FLAGS: &[(&str, FlagKind)] = &[
    ("build", FlagKind::Build),
    ("block-break", FlagKind::Build),
    ("block-place", FlagKind::Build),
    ("interact", FlagKind::Build),
    ("chest-access", FlagKind::Build),
    ("pvp", FlagKind::State),
    ("mob-damage", FlagKind::State),
    ("invincible", FlagKind::State),
    ("entry", FlagKind::Boundary),
    ("exit", FlagKind::Boundary),
    ("greeting", FlagKind::Text),
    ("farewell", FlagKind::Text),
];

/// The kind of a flag name, if supported.
pub fn flag_kind(flag: &str) -> Option<FlagKind> {
    FLAGS
        .iter()
        .find(|(name, _)| *name == flag)
        .map(|(_, kind)| *kind)
}

/// Normalizes a flag value for storage, rejecting invalid state values.
pub fn parse_flag_value(flag: &str, value: &str) -> Result<String, String> {
    let kind = flag_kind(flag).ok_or_else(|| format!("Unknown flag {flag}."))?;
    if kind == FlagKind::Text {
        return Ok(value.to_owned());
    }
    match value.to_lowercase().as_str() {
        "allow" | "true" | "yes" => Ok("allow".into()),
        "deny" | "false" | "no" => Ok("deny".into()),
        _ => Err(format!("Flag {flag} takes allow or deny.")),
    }
}

/// Lowercases and validates a region id.
pub fn valid_id(id: &str) -> Option<String> {
    let id = id.to_lowercase();
    let valid = !id.is_empty()
        && id.len() <= 64
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
        });
    (valid && id != GLOBAL).then_some(id)
}

/// One protected cuboid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Region {
    /// Minimum corner `[x, y, z]`.
    pub min: [i32; 3],
    /// Maximum corner `[x, y, z]`.
    pub max: [i32; 3],
    /// Higher priorities override lower ones.
    #[serde(default)]
    pub priority: i32,
    /// Region whose flags and members are inherited.
    #[serde(default)]
    pub parent: Option<String>,
    /// Owner UUIDs mapped to their last known names.
    #[serde(default)]
    pub owners: BTreeMap<String, String>,
    /// Member UUIDs mapped to their last known names.
    #[serde(default)]
    pub members: BTreeMap<String, String>,
    /// Flag values by name.
    #[serde(default)]
    pub flags: BTreeMap<String, String>,
}

impl Region {
    /// A new region spanning two corners.
    pub fn new(min: BlockPos, max: BlockPos) -> Self {
        Self {
            min: [min.x, min.y, min.z],
            max: [max.x, max.y, max.z],
            priority: 0,
            parent: None,
            owners: BTreeMap::new(),
            members: BTreeMap::new(),
            flags: BTreeMap::new(),
        }
    }

    /// Whether `pos` lies inside.
    pub const fn contains(&self, pos: BlockPos) -> bool {
        pos.x >= self.min[0]
            && pos.x <= self.max[0]
            && pos.y >= self.min[1]
            && pos.y <= self.max[1]
            && pos.z >= self.min[2]
            && pos.z <= self.max[2]
    }
}

/// Regions of one world plus its global flags.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldRegions {
    /// Flags of the `__global__` region.
    #[serde(default)]
    pub global: BTreeMap<String, String>,
    /// Regions by id.
    #[serde(default)]
    pub regions: BTreeMap<String, Region>,
}

/// What an action is checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Breaking a block.
    Break,
    /// Placing a block.
    Place,
    /// Using a door, button, lever or similar.
    Interact,
    /// Opening a container.
    ChestAccess,
}

impl Action {
    /// Flags consulted in order; the first one with a value decides.
    const fn flags(self) -> &'static [&'static str] {
        match self {
            Self::Break => &["block-break", "build"],
            Self::Place => &["block-place", "build"],
            Self::Interact => &["interact", "build"],
            Self::ChestAccess => &["chest-access", "interact", "build"],
        }
    }
}

impl WorldRegions {
    /// Ids of regions containing `pos`, highest priority first.
    pub fn at(&self, pos: BlockPos) -> Vec<&str> {
        let mut ids: Vec<&str> = self
            .regions
            .iter()
            .filter(|(_, region)| region.contains(pos))
            .map(|(id, _)| id.as_str())
            .collect();
        ids.sort_by_key(|id| std::cmp::Reverse(self.regions[*id].priority));
        ids
    }

    /// Ids of the region and its ancestors, nearest first; stops at cycles or missing parents.
    fn lineage_ids(&self, id: &str) -> Vec<&str> {
        let mut chain: Vec<&str> = Vec::new();
        let mut current = self.regions.get_key_value(id);
        while let Some((key, region)) = current {
            if chain.contains(&key.as_str()) {
                break;
            }
            chain.push(key);
            current = region
                .parent
                .as_deref()
                .and_then(|parent| self.regions.get_key_value(parent));
        }
        chain
    }

    /// The region and its ancestors, nearest first.
    fn lineage(&self, id: &str) -> Vec<&Region> {
        self.lineage_ids(id)
            .into_iter()
            .filter_map(|id| self.regions.get(id))
            .collect()
    }

    /// A region's own or inherited flag value.
    pub fn effective_flag(&self, id: &str, flag: &str) -> Option<&str> {
        self.lineage(id)
            .into_iter()
            .find_map(|region| region.flags.get(flag).map(String::as_str))
    }

    /// Whether `player` owns or is a member of the region or one of its ancestors.
    pub fn is_member(&self, id: &str, player: Uuid) -> bool {
        let key = player.to_string();
        self.lineage(id)
            .into_iter()
            .any(|region| region.owners.contains_key(&key) || region.members.contains_key(&key))
    }

    /// Whether `player` owns the region or one of its ancestors.
    pub fn is_owner(&self, id: &str, player: Uuid) -> bool {
        let key = player.to_string();
        self.lineage(id)
            .into_iter()
            .any(|region| region.owners.contains_key(&key))
    }

    /// Resolves a state flag among `ids` (highest priority first), then the global region.
    fn query_state(&self, ids: &[&str], flag: &str) -> Option<bool> {
        let mut index = 0;
        while index < ids.len() {
            let priority = self.regions[ids[index]].priority;
            let group: Vec<&str> = ids[index..]
                .iter()
                .take_while(|id| self.regions[**id].priority == priority)
                .copied()
                .collect();
            index += group.len();
            let values: Vec<&str> = group
                .iter()
                .filter_map(|id| self.effective_flag(id, flag))
                .collect();
            if !values.is_empty() {
                return Some(!values.contains(&"deny"));
            }
        }
        self.global.get(flag).map(|value| value != "deny")
    }

    /// The highest priority among `ids` and the regions sharing it.
    fn top_group<'a>(&self, ids: &[&'a str]) -> Vec<&'a str> {
        let Some(first) = ids.first() else {
            return Vec::new();
        };
        let priority = self.regions[*first].priority;
        ids.iter()
            .take_while(|id| self.regions[**id].priority == priority)
            .copied()
            .collect()
    }

    /// Whether `player` may perform `action` at `pos`.
    pub fn allows(&self, pos: BlockPos, player: Uuid, action: Action) -> bool {
        let ids = self.at(pos);
        if self
            .top_group(&ids)
            .iter()
            .any(|id| self.is_member(id, player))
        {
            return true;
        }
        for flag in action.flags() {
            if let Some(value) = self.query_state(&ids, flag) {
                return value;
            }
        }
        ids.is_empty()
    }

    /// A flag applying to everyone at `pos`, or `default`.
    pub fn state(&self, pos: BlockPos, flag: &str, default: bool) -> bool {
        self.query_state(&self.at(pos), flag).unwrap_or(default)
    }

    /// Whether a non-member may cross into (`entry`) or out of (`exit`) region `id`.
    pub fn may_cross(&self, id: &str, player: Uuid, flag: &str) -> bool {
        self.is_member(id, player) || self.effective_flag(id, flag) != Some("deny")
    }

    /// Sets a region's parent after checking it exists and creates no cycle.
    pub fn set_parent(&mut self, id: &str, parent: Option<&str>) -> Result<(), String> {
        if let Some(parent) = parent {
            if !self.regions.contains_key(parent) {
                return Err(format!("Region {parent} does not exist."));
            }
            if self.lineage_ids(parent).contains(&id) {
                return Err("That would create a parent cycle.".into());
            }
        }
        let region = self
            .regions
            .get_mut(id)
            .ok_or_else(|| format!("Region {id} does not exist."))?;
        region.parent = parent.map(str::to_owned);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: Uuid = Uuid { bytes: [1; 16] };
    const BOB: Uuid = Uuid { bytes: [2; 16] };

    fn pos(x: i32, y: i32, z: i32) -> BlockPos {
        BlockPos { x, y, z }
    }

    fn region(min: i32, max: i32, priority: i32) -> Region {
        let mut region = Region::new(pos(min, min, min), pos(max, max, max));
        region.priority = priority;
        region
    }

    #[test]
    fn non_members_cannot_build_in_regions_but_can_outside() {
        let mut world = WorldRegions::default();
        let mut spawn = region(0, 10, 0);
        spawn.members.insert(ALICE.to_string(), "Alice".into());
        world.regions.insert("spawn".into(), spawn);
        assert!(world.allows(pos(5, 5, 5), ALICE, Action::Break));
        assert!(!world.allows(pos(5, 5, 5), BOB, Action::Break));
        assert!(world.allows(pos(50, 5, 5), BOB, Action::Break));
    }

    #[test]
    fn higher_priority_allow_overrides_lower_deny() {
        let mut world = WorldRegions::default();
        let mut low = region(0, 20, 0);
        low.flags.insert("build".into(), "deny".into());
        let mut high = region(5, 10, 10);
        high.flags.insert("build".into(), "allow".into());
        world.regions.insert("low".into(), low);
        world.regions.insert("high".into(), high);
        assert!(world.allows(pos(7, 7, 7), BOB, Action::Place));
        assert!(!world.allows(pos(15, 15, 15), BOB, Action::Place));
    }

    #[test]
    fn deny_wins_at_equal_priority_and_specific_flags_beat_build() {
        let mut world = WorldRegions::default();
        let mut a = region(0, 10, 0);
        a.flags.insert("pvp".into(), "allow".into());
        a.flags.insert("build".into(), "allow".into());
        a.flags.insert("block-break".into(), "deny".into());
        let mut b = region(0, 10, 0);
        b.flags.insert("pvp".into(), "deny".into());
        world.regions.insert("a".into(), a);
        world.regions.insert("b".into(), b);
        assert!(!world.state(pos(1, 1, 1), "pvp", true));
        assert!(!world.allows(pos(1, 1, 1), BOB, Action::Break));
        assert!(world.allows(pos(1, 1, 1), BOB, Action::Place));
    }

    #[test]
    fn children_inherit_flags_and_members_from_parents() {
        let mut world = WorldRegions::default();
        let mut parent = region(-100, 100, 0);
        parent.flags.insert("greeting".into(), "hi".into());
        parent.members.insert(ALICE.to_string(), "Alice".into());
        world.regions.insert("parent".into(), parent);
        world.regions.insert("child".into(), region(0, 5, 5));
        assert_eq!(world.set_parent("child", Some("parent")), Ok(()));
        assert_eq!(world.effective_flag("child", "greeting"), Some("hi"));
        assert!(world.allows(pos(1, 1, 1), ALICE, Action::Interact));
        assert!(world.set_parent("parent", Some("child")).is_err());
    }

    #[test]
    fn global_flags_apply_outside_regions() {
        let mut world = WorldRegions::default();
        world.global.insert("pvp".into(), "deny".into());
        world.global.insert("build".into(), "deny".into());
        assert!(!world.state(pos(0, 0, 0), "pvp", true));
        assert!(!world.allows(pos(0, 0, 0), BOB, Action::Break));
        assert!(world.state(pos(0, 0, 0), "mob-damage", true));
    }

    #[test]
    fn entry_and_exit_only_restrict_non_members() {
        let mut world = WorldRegions::default();
        let mut vault = region(0, 10, 0);
        vault.flags.insert("entry".into(), "deny".into());
        vault.owners.insert(ALICE.to_string(), "Alice".into());
        world.regions.insert("vault".into(), vault);
        assert!(world.may_cross("vault", ALICE, "entry"));
        assert!(!world.may_cross("vault", BOB, "entry"));
        assert!(world.may_cross("vault", BOB, "exit"));
    }

    #[test]
    fn flag_values_and_ids_are_validated() {
        assert_eq!(parse_flag_value("pvp", "DENY"), Ok("deny".into()));
        assert!(parse_flag_value("pvp", "maybe").is_err());
        assert!(parse_flag_value("nope", "allow").is_err());
        assert_eq!(
            parse_flag_value("greeting", "&aHi there"),
            Ok("&aHi there".into())
        );
        assert_eq!(valid_id("Spawn_1"), Some("spawn_1".into()));
        assert_eq!(valid_id(GLOBAL), None);
        assert_eq!(valid_id("bad id"), None);
    }

    #[test]
    fn files_round_trip() {
        let mut world = WorldRegions::default();
        let mut spawn = region(0, 10, 3);
        spawn.flags.insert("pvp".into(), "deny".into());
        world.regions.insert("spawn".into(), spawn);
        world.global.insert("build".into(), "allow".into());
        let text = toml::to_string(&world).unwrap_or_default();
        let loaded: WorldRegions = toml::from_str(&text).unwrap_or_default();
        assert_eq!(loaded, world);
    }
}
