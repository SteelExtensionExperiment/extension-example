//! Runtime state and the rules that tie queues, access, kits, signs and the store together.
//!
//! Everything here is host-free: operations return text for the caller and [`Notice`]s for
//! players who are affected without having typed anything, so the host glue can send them after
//! every lock is released.

use std::collections::HashMap;

use arena_manager_api::MatchId;
use steel_extension_api::{inventory_snapshot::InventorySnapshot, players::Uuid};

use crate::{
    access::{Access, GameAccess, JoinRefusal, TICKS_PER_SECOND},
    config::Config,
    kits::{self, Kit},
    queues::{Formed, Joined, Queues},
    signs::{SignKind, SignPos, validate},
    store::{GameType, MIN_PLAYERS, Store, StoreError},
};

const SECONDS_PER_MINUTE: u64 = 60;

/// A message for a player who did not trigger it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Notice {
    pub to: Uuid,
    pub text: String,
}

fn notices(players: Vec<Uuid>, text: &str) -> Vec<Notice> {
    players
        .into_iter()
        .map(|to| Notice {
            to,
            text: text.to_owned(),
        })
        .collect()
}

fn saved(result: Result<(), StoreError>) -> Result<(), String> {
    result.map_err(|error| format!("cannot save {error}"))
}

/// A match this orchestrator asked for and has not seen end.
struct MatchRef {
    game_type: String,
    players: Vec<Uuid>,
}

/// A request that arena-manager rejected for capacity. Its players are in neither a queue nor a
/// match until a retry succeeds.
pub(crate) struct Pending {
    pub game_type: String,
    pub players: Vec<Uuid>,
}

pub(crate) struct Orchestrator {
    pub store: Store,
    pub config: Config,
    pub access: Access,
    queues: Queues<Uuid>,
    matches: HashMap<MatchId, MatchRef>,
    pending: HashMap<u64, Pending>,
    next_pending: u64,
}

impl Orchestrator {
    pub(crate) fn new(store: Store, config: Config) -> Self {
        let access = Access::new(store.open);
        Self {
            store,
            config,
            access,
            queues: Queues::default(),
            matches: HashMap::new(),
            pending: HashMap::new(),
            next_pending: 0,
        }
    }

    // --- players -----------------------------------------------------------------------

    /// Queues the player. Forming a match from a full queue waits until `start_match` can carry
    /// a kit per player (host spec 007), so a full queue is not consumed yet.
    pub(crate) fn join(&mut self, player: Uuid, game_type: &str) -> Result<String, JoinRefusal> {
        let chosen = self
            .store
            .choices
            .get(&(player, game_type.to_owned()))
            .map(String::as_str);
        let game = self.store.game_type(game_type).map(|found| GameAccess {
            enabled: found.enabled,
            has_valid_kit: kits::resolve(&found.kits, chosen).is_some(),
        });
        self.access
            .check_join(self.in_match(player), game_type, game)?;
        let needed = self
            .store
            .game_type(game_type)
            .map_or(0, |found| found.players);
        match self.queues.join(player, game_type) {
            Joined::AlreadyQueued => Err(JoinRefusal::AlreadyQueued(game_type.to_owned())),
            Joined::Queued {
                position,
                moved_from,
            } => {
                let moved = moved_from.map_or_else(String::new, |from| format!(" (left {from})"));
                Ok(format!(
                    "queued for {game_type} at position {position}, {needed} players needed{moved}"
                ))
            }
        }
    }

    /// Removes the player from their queue; the game type they left.
    pub(crate) fn leave(&mut self, player: Uuid) -> Option<String> {
        self.queues.leave(player)
    }

    /// The player disconnected: they leave their queue and are not re-queued if a match fails.
    /// A request waiting for capacity that holds them is dropped as a whole and the others go
    /// back to the front of the queue.
    pub(crate) fn quit(&mut self, player: Uuid) -> Vec<Notice> {
        self.queues.leave(player);
        for running in self.matches.values_mut() {
            running.players.retain(|member| *member != player);
        }
        let Some(id) = self
            .pending
            .iter()
            .find(|(_, held)| held.players.contains(&player))
            .map(|(id, _)| *id)
        else {
            return Vec::new();
        };
        let Some(mut held) = self.pending.remove(&id) else {
            return Vec::new();
        };
        held.players.retain(|member| *member != player);
        self.queues.push_front(&held.game_type, &held.players);
        notices(
            held.players,
            "a player left before the match started, you are back in the queue",
        )
    }

    fn in_match(&self, player: Uuid) -> bool {
        self.match_of(player).is_some()
            || self
                .pending
                .values()
                .any(|held| held.players.contains(&player))
    }

    fn match_of(&self, player: Uuid) -> Option<(MatchId, &str)> {
        self.matches
            .iter()
            .find(|(_, running)| running.players.contains(&player))
            .map(|(id, running)| (*id, running.game_type.as_str()))
    }

    /// Records a match accepted by arena-manager.
    pub(crate) fn begin_match(&mut self, id: MatchId, game_type: &str, players: Vec<Uuid>) {
        self.matches.insert(
            id,
            MatchRef {
                game_type: game_type.to_owned(),
                players,
            },
        );
    }

    /// Sets the player's kit for a game type. A queued player keeps their place.
    pub(crate) fn choose_kit(
        &mut self,
        player: Uuid,
        game_type: &str,
        kit: &str,
    ) -> Result<String, String> {
        let kind = SignKind::Kit {
            game_type: game_type.to_owned(),
            kit: kit.to_owned(),
        };
        validate(&kind, &self.store).map_err(|error| error.to_string())?;
        self.store
            .choices
            .insert((player, game_type.to_owned()), kit.to_owned());
        saved(self.store.save_choices())?;
        Ok(format!("your kit for {game_type} is {kit}"))
    }

    /// `/queue` for a player.
    pub(crate) fn queue_status(&self, player: Uuid) -> Vec<String> {
        if let Some((id, game_type)) = self.match_of(player) {
            return vec![format!("you are in match #{id} of {game_type}")];
        }
        if self.in_match(player) {
            return vec!["your match is being prepared".to_owned()];
        }
        if let Some((game_type, position)) = self.queues.position(player) {
            let needed = self
                .store
                .game_type(game_type)
                .map_or(0, |found| found.players);
            let kit = self
                .store
                .choices
                .get(&(player, game_type.to_owned()))
                .map_or("none chosen", String::as_str);
            return vec![format!(
                "queued for {game_type}: position {position}, {needed} players needed, kit {kit}"
            )];
        }
        let mut lines = vec!["you are not queued".to_owned()];
        lines.extend(
            self.store
                .game_types
                .iter()
                .filter(|found| found.enabled)
                .map(|found| {
                    format!(
                        "{}: {} queued, {} players needed",
                        found.name,
                        self.queues.queue(&found.name).count(),
                        found.players
                    )
                }),
        );
        lines
    }

    /// A finished match frees its players.
    pub(crate) fn finish(&mut self, id: MatchId) {
        self.matches.remove(&id);
    }

    /// A match failed before it ran: its players are told and re-queued at the front, in order.
    pub(crate) fn fail(&mut self, id: MatchId, message: &str) -> Vec<Notice> {
        let Some(failed) = self.matches.remove(&id) else {
            return Vec::new();
        };
        self.queues.push_front(&failed.game_type, &failed.players);
        notices(
            failed.players,
            &format!("the match failed ({message}), you are back in the queue"),
        )
    }

    // --- signs -------------------------------------------------------------------------

    /// What a click on `pos` does; `None` when no sign is registered there. A sign whose game
    /// type or kit no longer exists answers with the error instead of acting.
    pub(crate) fn click_sign(
        &mut self,
        player: Uuid,
        pos: &SignPos,
    ) -> Option<Result<String, String>> {
        let kind = self.store.signs.at(pos)?.kind.clone();
        let result = match kind {
            SignKind::Queue { game_type } => self
                .join(player, &game_type)
                .map_err(|refusal| refusal.to_string()),
            SignKind::Kit { game_type, kit } => self.choose_kit(player, &game_type, &kit),
        };
        Some(result)
    }

    pub(crate) fn register_sign(&mut self, pos: SignPos, kind: SignKind) -> Result<String, String> {
        let id = self.store.signs.register(pos, kind);
        saved(self.store.save_signs())?;
        Ok(id)
    }

    /// A sign registered at `pos` was edited so that it no longer names this extension.
    pub(crate) fn unregister_sign_at(&mut self, pos: &SignPos) -> Result<Option<String>, String> {
        let Some(id) = self.store.signs.at(pos).map(|sign| sign.id.clone()) else {
            return Ok(None);
        };
        self.remove_sign(&id).map(Some)
    }

    pub(crate) fn sign_lines(&self) -> Vec<String> {
        self.store
            .signs
            .list()
            .iter()
            .map(|sign| {
                let pos = &sign.pos;
                let what = match &sign.kind {
                    SignKind::Queue { game_type } => format!("queue {game_type}"),
                    SignKind::Kit { game_type, kit } => format!("kit {game_type}/{kit}"),
                };
                format!(
                    "{}: {what} at {} {} {} {}",
                    sign.id, pos.world, pos.x, pos.y, pos.z
                )
            })
            .collect()
    }

    pub(crate) fn remove_sign(&mut self, id: &str) -> Result<String, String> {
        self.store
            .signs
            .remove(id)
            .ok_or_else(|| format!("unknown sign {id}"))?;
        saved(self.store.save_signs())?;
        Ok(format!("removed sign {id}"))
    }

    // --- game types --------------------------------------------------------------------

    fn game_type_mut(&mut self, name: &str) -> Result<&mut GameType, String> {
        self.store
            .game_types
            .iter_mut()
            .find(|found| found.name == name)
            .ok_or_else(|| format!("unknown game type {name}"))
    }

    fn check_players(game_type: &str, players: u32) -> Result<(), String> {
        if players < MIN_PLAYERS {
            return Err(format!("{game_type} needs at least {MIN_PLAYERS} players"));
        }
        Ok(())
    }

    pub(crate) fn create_game_type(&mut self, name: &str, players: u32) -> Result<String, String> {
        if self.store.game_type(name).is_some() {
            return Err(format!("game type {name} already exists"));
        }
        Self::check_players(name, players)?;
        self.store.game_types.push(GameType {
            name: name.to_owned(),
            players,
            enabled: true,
            kits: Vec::new(),
        });
        saved(self.store.save_game_types())?;
        Ok(format!("created game type {name} for {players} players"))
    }

    pub(crate) fn set_players(&mut self, name: &str, players: u32) -> Result<String, String> {
        Self::check_players(name, players)?;
        self.game_type_mut(name)?.players = players;
        saved(self.store.save_game_types())?;
        Ok(format!("{name} now needs {players} players"))
    }

    /// Disabling clears the queue; the cleared players get a notice.
    pub(crate) fn set_enabled(
        &mut self,
        name: &str,
        enabled: bool,
    ) -> Result<(String, Vec<Notice>), String> {
        self.game_type_mut(name)?.enabled = enabled;
        saved(self.store.save_game_types())?;
        let cleared = if enabled {
            Vec::new()
        } else {
            self.queues.clear_game_type(name)
        };
        let notice = notices(
            cleared,
            &format!("game type {name} was disabled, you left its queue"),
        );
        Ok((
            format!(
                "game type {name} {}",
                if enabled { "enabled" } else { "disabled" }
            ),
            notice,
        ))
    }

    /// Removes the game type with its kits, signs and choices.
    pub(crate) fn remove_game_type(&mut self, name: &str) -> Result<(String, Vec<Notice>), String> {
        self.game_type_mut(name)?;
        self.store.game_types.retain(|found| found.name != name);
        self.store.signs.remove_game_type(name);
        self.store
            .choices
            .retain(|(_, game_type), _| game_type != name);
        saved(self.store.save_game_types())?;
        saved(self.store.save_signs())?;
        saved(self.store.save_choices())?;
        let notice = notices(
            self.queues.clear_game_type(name),
            &format!("game type {name} was removed, you left its queue"),
        );
        Ok((format!("removed game type {name}"), notice))
    }

    /// Stores the administrator's inventory as the kit, replacing a kit of that name.
    pub(crate) fn save_kit(
        &mut self,
        game_type: &str,
        kit: &str,
        snapshot: InventorySnapshot,
    ) -> Result<String, String> {
        let found = self.game_type_mut(game_type)?;
        let replaced = kits::save(
            &mut found.kits,
            Kit {
                name: kit.to_owned(),
                snapshot,
            },
        );
        saved(self.store.save_game_types())?;
        let verb = if replaced { "replaced" } else { "saved" };
        Ok(format!("{verb} kit {game_type}/{kit}"))
    }

    pub(crate) fn kit_snapshot(
        &self,
        game_type: &str,
        kit: &str,
    ) -> Result<InventorySnapshot, String> {
        let found = self
            .store
            .game_type(game_type)
            .ok_or_else(|| format!("unknown game type {game_type}"))?;
        found
            .kits
            .iter()
            .find(|candidate| candidate.name == kit)
            .map(|candidate| candidate.snapshot.clone())
            .ok_or_else(|| format!("unknown kit {game_type}/{kit}"))
    }

    /// Pops the first N players of every full queue, in game type name order.
    pub(crate) fn form_requests(&mut self) -> Vec<Formed<Uuid>> {
        let store = &self.store;
        self.queues.form(|name| {
            store
                .game_type(name)
                .and_then(|found| usize::try_from(found.players).ok())
        })
    }

    /// Each player's chosen kit snapshot for a match request, in player order. When a player no
    /// longer has a valid kit (it was removed after they queued) they are told and dropped, and
    /// the rest return to the front of the queue.
    pub(crate) fn prepare_request(
        &mut self,
        game_type: &str,
        players: &[Uuid],
    ) -> Result<Vec<(Uuid, InventorySnapshot)>, Vec<Notice>> {
        let Some(found) = self.store.game_type(game_type) else {
            return Err(notices(
                players.to_vec(),
                &format!("game type {game_type} was removed, you left its queue"),
            ));
        };
        let resolved: Vec<(Uuid, Option<InventorySnapshot>)> = players
            .iter()
            .map(|player| {
                let chosen = self
                    .store
                    .choices
                    .get(&(*player, game_type.to_owned()))
                    .map(String::as_str);
                let snapshot = kits::resolve(&found.kits, chosen).map(|kit| kit.snapshot.clone());
                (*player, snapshot)
            })
            .collect();
        if resolved.iter().all(|(_, snapshot)| snapshot.is_some()) {
            return Ok(resolved
                .into_iter()
                .filter_map(|(player, snapshot)| Some((player, snapshot?)))
                .collect());
        }
        let (without, with): (Vec<_>, Vec<_>) = resolved
            .into_iter()
            .partition(|(_, snapshot)| snapshot.is_none());
        let kept: Vec<Uuid> = with.into_iter().map(|(player, _)| player).collect();
        self.queues.push_front(game_type, &kept);
        Err(notices(
            without.into_iter().map(|(player, _)| player).collect(),
            &format!("your kit for {game_type} is gone, choose one again; you left the queue"),
        ))
    }

    /// Holds a request that was rejected for capacity; the id names it for the retry.
    pub(crate) fn hold(&mut self, game_type: &str, players: Vec<Uuid>) -> u64 {
        self.next_pending += 1;
        self.pending.insert(
            self.next_pending,
            Pending {
                game_type: game_type.to_owned(),
                players,
            },
        );
        self.next_pending
    }

    /// A request failed for a reason other than capacity: the players are told and go back to
    /// the front of the queue.
    pub(crate) fn refuse(
        &mut self,
        game_type: &str,
        players: Vec<Uuid>,
        message: &str,
    ) -> Vec<Notice> {
        self.queues.push_front(game_type, &players);
        notices(
            players,
            &format!("the match could not start ({message}), you are back in the queue"),
        )
    }

    /// Takes a held request for its retry; `None` when it was dropped meanwhile.
    pub(crate) fn take_pending(&mut self, id: u64) -> Option<Pending> {
        self.pending.remove(&id)
    }

    /// `/orchestrator match`: checks an on-behalf request and takes the players out of their
    /// queues. It bypasses the open/closed switch, not the disabled flag (O8).
    pub(crate) fn admin_match(
        &mut self,
        game_type: &str,
        players: &[(Uuid, String)],
    ) -> Result<(), String> {
        let found = self
            .store
            .game_type(game_type)
            .ok_or_else(|| format!("unknown game type {game_type}"))?;
        if !found.enabled {
            return Err(JoinRefusal::Disabled(game_type.to_owned()).to_string());
        }
        if usize::try_from(found.players).ok() != Some(players.len()) {
            return Err(format!(
                "{game_type} needs exactly {} players",
                found.players
            ));
        }
        for (index, (player, name)) in players.iter().enumerate() {
            if players[..index]
                .iter()
                .any(|(earlier, _)| earlier == player)
            {
                return Err(format!("{name} is listed twice"));
            }
            if self.in_match(*player) {
                return Err(format!("{name} is in a match"));
            }
            let chosen = self
                .store
                .choices
                .get(&(*player, game_type.to_owned()))
                .map(String::as_str);
            if kits::resolve(&found.kits, chosen).is_none() {
                return Err(format!("{name} has not chosen a kit for {game_type}"));
            }
        }
        for (player, _) in players {
            self.queues.leave(*player);
        }
        Ok(())
    }

    pub(crate) fn remove_kit(&mut self, game_type: &str, kit: &str) -> Result<String, String> {
        let found = self.game_type_mut(game_type)?;
        kits::remove(&mut found.kits, kit)
            .ok_or_else(|| format!("unknown kit {game_type}/{kit}"))?;
        saved(self.store.save_game_types())?;
        Ok(format!("removed kit {game_type}/{kit}"))
    }

    pub(crate) fn game_type_lines(&self) -> Vec<String> {
        self.store
            .game_types
            .iter()
            .map(|found| {
                format!(
                    "{}: {} players, {}, {} kits, {} queued",
                    found.name,
                    found.players,
                    if found.enabled { "enabled" } else { "disabled" },
                    found.kits.len(),
                    self.queues.queue(&found.name).count()
                )
            })
            .collect()
    }

    pub(crate) fn kit_lines(&self, game_type: &str) -> Result<Vec<String>, String> {
        let found = self
            .store
            .game_type(game_type)
            .ok_or_else(|| format!("unknown game type {game_type}"))?;
        Ok(found.kits.iter().map(|kit| kit.name.clone()).collect())
    }

    // --- session -----------------------------------------------------------------------

    /// Opens queueing; the closing tick of a timed session.
    pub(crate) fn open(
        &mut self,
        now: u64,
        hours: Option<u32>,
    ) -> Result<(String, Option<u64>), String> {
        let closes_at = self
            .access
            .open(now, hours, self.config.max_session_hours)
            .map_err(|error| error.to_string())?;
        self.store.open = true;
        saved(self.store.save_state())?;
        let text = hours.map_or_else(
            || "queueing is open until closed".to_owned(),
            |hours| format!("queueing is open for {hours} hours"),
        );
        Ok((text, closes_at))
    }

    /// Closes queueing and clears every queue.
    pub(crate) fn close(&mut self) -> Result<(String, Vec<Notice>), String> {
        self.access.close();
        self.store.open = false;
        saved(self.store.save_state())?;
        let cleared = notices(
            self.queues.clear_all(),
            "queueing was closed, you left the queue",
        );
        Ok(("queueing is closed".to_owned(), cleared))
    }

    /// Closes a timed session that ran out; `None` when it did not.
    pub(crate) fn expire(&mut self, now: u64) -> Option<Vec<Notice>> {
        if !self.access.expire(now) {
            return None;
        }
        self.store.open = false;
        let cleared = notices(
            self.queues.clear_all(),
            "the session ended, you left the queue",
        );
        Some(cleared)
    }

    pub(crate) fn status_lines(&self, now: u64) -> Vec<String> {
        let state = match self.access.ticks_left(now) {
            _ if !self.access.is_open() => "closed".to_owned(),
            Some(ticks) => format!(
                "open, {} minutes left",
                ticks / TICKS_PER_SECOND / SECONDS_PER_MINUTE
            ),
            None => "open".to_owned(),
        };
        let mut lines = vec![format!("queueing is {state}")];
        lines.extend(self.game_type_lines());
        lines.push(format!("{} running matches", self.matches.len()));
        lines
    }

    pub(crate) fn queue_lines(&self, name_of: impl Fn(Uuid) -> String) -> Vec<String> {
        self.store
            .game_types
            .iter()
            .map(|found| {
                let names: Vec<String> = self.queues.queue(&found.name).map(&name_of).collect();
                format!(
                    "{}: {}",
                    found.name,
                    if names.is_empty() {
                        "empty".to_owned()
                    } else {
                        names.join(", ")
                    }
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::*;
    use crate::{kits::Kit, signs::Catalog};

    const ALICE: Uuid = Uuid { bytes: [1; 16] };

    fn orchestrator(name: &str) -> Orchestrator {
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "game-orchestrator-orch-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut orchestrator =
            Orchestrator::new(Store::load(dir).unwrap(), Config::parse(None).unwrap());
        orchestrator.create_game_type("skywars", 2).unwrap();
        orchestrator.store.game_types[0]
            .kits
            .push(Kit::named("knight"));
        orchestrator.store.game_types[0]
            .kits
            .push(Kit::named("archer"));
        orchestrator.open(0, None).unwrap();
        orchestrator
    }

    #[test]
    fn joining_needs_a_kit_unless_the_game_type_has_one() {
        let mut o = orchestrator("kit");
        assert_eq!(
            o.join(ALICE, "skywars"),
            Err(JoinRefusal::NoKit("skywars".into()))
        );
        o.choose_kit(ALICE, "skywars", "knight").unwrap();
        assert!(o.join(ALICE, "skywars").is_ok());
        assert_eq!(
            o.join(ALICE, "skywars"),
            Err(JoinRefusal::AlreadyQueued("skywars".into()))
        );

        o.remove_kit("skywars", "archer").unwrap();
        o.queues.leave(ALICE);
        o.store.choices.clear();
        assert!(
            o.join(ALICE, "skywars").is_ok(),
            "the only kit is selected automatically"
        );
    }

    #[test]
    fn close_and_disable_clear_queues_and_notify() {
        let mut o = orchestrator("clear");
        o.choose_kit(ALICE, "skywars", "knight").unwrap();
        o.join(ALICE, "skywars").unwrap();
        let (_, notices) = o.set_enabled("skywars", false).unwrap();
        assert_eq!(
            notices.iter().map(|notice| notice.to).collect::<Vec<_>>(),
            [ALICE]
        );
        assert_eq!(
            o.join(ALICE, "skywars"),
            Err(JoinRefusal::Disabled("skywars".into()))
        );

        o.set_enabled("skywars", true).unwrap();
        o.join(ALICE, "skywars").unwrap();
        let (_, notices) = o.close().unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(o.join(ALICE, "skywars"), Err(JoinRefusal::Closed));
    }

    #[test]
    fn timed_session_expiry_clears_the_queue_and_persists_closed() {
        let mut o = orchestrator("expire");
        let (_, closes_at) = o.open(0, Some(1)).unwrap();
        o.choose_kit(ALICE, "skywars", "knight").unwrap();
        o.join(ALICE, "skywars").unwrap();
        assert!(o.expire(closes_at.unwrap() - 1).is_none());
        assert_eq!(
            o.expire(closes_at.unwrap()).map(|notices| notices.len()),
            Some(1)
        );
        assert!(!o.store.open);
    }

    #[test]
    fn removing_a_game_type_removes_signs_choices_and_queue() {
        let mut o = orchestrator("remove");
        o.choose_kit(ALICE, "skywars", "knight").unwrap();
        o.join(ALICE, "skywars").unwrap();
        let pos = SignPos {
            world: "w".into(),
            x: 0,
            y: 0,
            z: 0,
        };
        o.register_sign(
            pos.clone(),
            SignKind::Queue {
                game_type: "skywars".into(),
            },
        )
        .unwrap();
        let (_, notices) = o.remove_game_type("skywars").unwrap();
        assert_eq!(notices.len(), 1);
        assert!(o.store.signs.list().is_empty() && o.store.choices.is_empty());
        assert!(o.store.kits("skywars").is_none());
        assert_eq!(o.click_sign(ALICE, &pos), None);
    }

    #[test]
    fn clicking_signs_acts_like_the_commands_and_reports_stale_ones() {
        let mut o = orchestrator("click");
        let kit_pos = SignPos {
            world: "w".into(),
            x: 1,
            y: 0,
            z: 0,
        };
        let queue_pos = SignPos {
            world: "w".into(),
            x: 2,
            y: 0,
            z: 0,
        };
        o.register_sign(
            kit_pos.clone(),
            SignKind::Kit {
                game_type: "skywars".into(),
                kit: "knight".into(),
            },
        )
        .unwrap();
        o.register_sign(
            queue_pos.clone(),
            SignKind::Queue {
                game_type: "skywars".into(),
            },
        )
        .unwrap();
        assert!(o.click_sign(ALICE, &kit_pos).unwrap().is_ok());
        assert!(o.click_sign(ALICE, &queue_pos).unwrap().is_ok());
        o.remove_kit("skywars", "knight").unwrap();
        assert!(
            o.click_sign(ALICE, &kit_pos)
                .unwrap()
                .unwrap_err()
                .contains("unknown kit")
        );
    }

    #[test]
    fn kits_are_saved_loaded_and_resolved_per_player() {
        let mut o = orchestrator("kitsave");
        let mut snapshot = o.kit_snapshot("skywars", "knight").unwrap();
        snapshot.format = 9;
        assert_eq!(
            o.save_kit("skywars", "knight", snapshot.clone()).unwrap(),
            "replaced kit skywars/knight"
        );
        assert_eq!(
            o.save_kit("skywars", "mage", snapshot.clone()).unwrap(),
            "saved kit skywars/mage"
        );
        assert_eq!(o.kit_snapshot("skywars", "mage").unwrap(), snapshot);
        assert!(o.kit_snapshot("skywars", "nope").is_err());

        let bob = Uuid { bytes: [2; 16] };
        o.choose_kit(ALICE, "skywars", "knight").unwrap();
        o.choose_kit(bob, "skywars", "archer").unwrap();
        let request = o.prepare_request("skywars", &[ALICE, bob]).unwrap();
        assert_eq!(request[0].1, snapshot);
        assert_ne!(request[1].1, snapshot);
        assert!(
            o.prepare_request("skywars", &[Uuid { bytes: [3; 16] }])
                .is_err()
        );
    }

    #[test]
    fn a_player_in_a_match_cannot_queue_until_the_result_frees_them() {
        let mut o = orchestrator("match");
        o.choose_kit(ALICE, "skywars", "knight").unwrap();
        o.begin_match(1, "skywars", vec![ALICE]);
        assert_eq!(o.join(ALICE, "skywars"), Err(JoinRefusal::InMatch));
        o.finish(1);
        assert!(o.join(ALICE, "skywars").is_ok());
    }

    #[test]
    fn a_failed_match_requeues_the_players_who_stayed_at_the_front() {
        let mut o = orchestrator("fail");
        let (bob, carol) = (Uuid { bytes: [2; 16] }, Uuid { bytes: [3; 16] });
        for player in [ALICE, bob, carol] {
            o.choose_kit(player, "skywars", "knight").unwrap();
        }
        o.begin_match(7, "skywars", vec![ALICE, bob]);
        o.join(carol, "skywars").unwrap();
        o.quit(bob);
        let notices = o.fail(7, "no arena");
        assert_eq!(
            notices.iter().map(|notice| notice.to).collect::<Vec<_>>(),
            [ALICE]
        );
        assert_eq!(o.queues.position(ALICE), Some(("skywars", 1)));
        assert_eq!(o.queues.position(carol), Some(("skywars", 2)));
        assert_eq!(o.fail(7, "again"), vec![]);
    }

    #[test]
    fn game_type_rules_have_the_contract_messages() {
        let mut o = orchestrator("rules");
        assert_eq!(
            o.create_game_type("skywars", 2).unwrap_err(),
            "game type skywars already exists"
        );
        assert!(
            o.create_game_type("duel", 1)
                .unwrap_err()
                .contains("at least")
        );
        assert_eq!(
            o.set_players("nope", 2).unwrap_err(),
            "unknown game type nope"
        );
    }

    const BOB: Uuid = Uuid { bytes: [2; 16] };
    const CAROL: Uuid = Uuid { bytes: [3; 16] };

    fn queued(o: &mut Orchestrator, players: &[Uuid]) {
        for player in players {
            o.choose_kit(*player, "skywars", "knight").unwrap();
            o.join(*player, "skywars").unwrap();
        }
    }

    #[test]
    fn a_full_queue_forms_one_request_and_leaves_the_rest() {
        let mut o = orchestrator("form");
        queued(&mut o, &[ALICE, BOB, CAROL]);
        let formed = o.form_requests();
        assert_eq!(formed.len(), 1);
        assert_eq!(formed[0].players, vec![ALICE, BOB]);
        assert!(o.queue_status(CAROL)[0].contains("position 1"));
    }

    #[test]
    fn a_player_whose_kit_vanished_is_dropped_and_the_rest_requeue() {
        let mut o = orchestrator("vanished");
        queued(&mut o, &[ALICE, BOB]);
        let formed = o.form_requests().remove(0);
        o.store.choices.remove(&(BOB, "skywars".to_owned()));
        let notices = o.prepare_request("skywars", &formed.players).unwrap_err();
        assert_eq!(
            notices.iter().map(|notice| notice.to).collect::<Vec<_>>(),
            [BOB]
        );
        assert!(o.queue_status(ALICE)[0].contains("position 1"));
    }

    #[test]
    fn a_quit_during_the_capacity_wait_drops_the_request_and_requeues_the_others() {
        let mut o = orchestrator("pending");
        let id = o.hold("skywars", vec![ALICE, BOB]);
        assert_eq!(o.join(ALICE, "skywars"), Err(JoinRefusal::InMatch));
        let notices = o.quit(BOB);
        assert_eq!(
            notices.iter().map(|notice| notice.to).collect::<Vec<_>>(),
            [ALICE]
        );
        assert!(o.take_pending(id).is_none());
        assert!(o.queue_status(ALICE)[0].contains("queued for skywars"));
    }

    #[test]
    fn a_held_request_is_taken_once_for_its_retry() {
        let mut o = orchestrator("retry");
        let id = o.hold("skywars", vec![ALICE, BOB]);
        assert_eq!(
            o.take_pending(id).map(|held| held.players),
            Some(vec![ALICE, BOB])
        );
        assert!(o.take_pending(id).is_none());
    }

    #[test]
    fn on_behalf_matches_need_exact_players_kits_and_an_enabled_game_type() {
        let mut o = orchestrator("admin");
        let named = |players: &[Uuid]| -> Vec<(Uuid, String)> {
            players
                .iter()
                .map(|player| (*player, format!("p{}", player.bytes[0])))
                .collect()
        };
        assert_eq!(
            o.admin_match("skywars", &named(&[ALICE])).unwrap_err(),
            "skywars needs exactly 2 players"
        );
        assert!(
            o.admin_match("skywars", &named(&[ALICE, BOB]))
                .unwrap_err()
                .contains("has not chosen a kit")
        );

        o.choose_kit(ALICE, "skywars", "knight").unwrap();
        o.choose_kit(BOB, "skywars", "knight").unwrap();
        assert!(
            o.admin_match("skywars", &named(&[ALICE, ALICE]))
                .unwrap_err()
                .contains("listed twice")
        );
        o.join(ALICE, "skywars").unwrap();
        o.access.close();
        assert!(
            o.admin_match("skywars", &named(&[ALICE, BOB])).is_ok(),
            "bypasses closed and the queue"
        );
        assert!(o.queue_status(ALICE)[0].contains("not queued"));

        o.set_enabled("skywars", false).unwrap();
        assert!(
            o.admin_match("skywars", &named(&[ALICE, BOB]))
                .unwrap_err()
                .contains("disabled")
        );
    }
}
