//! Open/closed switch, timed session and the join check.

use std::fmt;

const TICKS_PER_SECOND: u64 = 20;
const SECONDS_PER_HOUR: u64 = 3600;
const TICKS_PER_HOUR: u64 = TICKS_PER_SECOND * SECONDS_PER_HOUR;

/// What the join check needs to know about a game type.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GameAccess {
    pub enabled: bool,
    /// The player has a valid kit (chosen, or the only one).
    pub has_valid_kit: bool,
}

/// Why a join is refused; `Display` is the message of the commands contract.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum JoinRefusal {
    InMatch,
    Closed,
    UnknownGameType(String),
    Disabled(String),
    NoKit(String),
    /// Not part of the access check: raised by the queue when the join changes nothing.
    AlreadyQueued(String),
}

impl fmt::Display for JoinRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InMatch => f.write_str("you are in a match"),
            Self::Closed => f.write_str("queueing is closed"),
            Self::UnknownGameType(name) => write!(f, "unknown game type {name}"),
            Self::Disabled(name) => write!(f, "game type {name} is disabled"),
            Self::NoKit(name) => write!(f, "choose a kit for {name} at a kit sign first"),
            Self::AlreadyQueued(name) => write!(f, "you are already queued for {name}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct InvalidHours {
    pub max: u32,
}

impl fmt::Display for InvalidHours {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hours must be between 1 and {}", self.max)
    }
}

#[derive(Debug, Default)]
pub(crate) struct Access {
    open: bool,
    /// Tick at which a timed session closes; `None` stays open until `close`.
    closes_at_tick: Option<u64>,
}

impl Access {
    pub(crate) fn new(open: bool) -> Self {
        Self {
            open,
            closes_at_tick: None,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        self.open
    }

    /// Opens queueing, for `hours` if given. Returns the closing tick of a timed session.
    pub(crate) fn open(
        &mut self,
        now: u64,
        hours: Option<u32>,
        max_hours: u32,
    ) -> Result<Option<u64>, InvalidHours> {
        let closes_at_tick = match hours {
            None => None,
            Some(hours) if (1..=max_hours).contains(&hours) => {
                Some(now + u64::from(hours) * TICKS_PER_HOUR)
            }
            Some(_) => return Err(InvalidHours { max: max_hours }),
        };
        self.open = true;
        self.closes_at_tick = closes_at_tick;
        Ok(closes_at_tick)
    }

    pub(crate) fn close(&mut self) {
        self.open = false;
        self.closes_at_tick = None;
    }

    /// Closes a timed session whose time has come; true when it just closed.
    pub(crate) fn expire(&mut self, now: u64) -> bool {
        if !self.open || self.closes_at_tick.is_none_or(|at| now < at) {
            return false;
        }
        self.close();
        true
    }

    pub(crate) fn ticks_left(&self, now: u64) -> Option<u64> {
        self.closes_at_tick.map(|at| at.saturating_sub(now))
    }

    /// Refusal order: in a match, closed, unknown, disabled, no kit.
    pub(crate) fn check_join(
        &self,
        in_match: bool,
        game_type: &str,
        game: Option<GameAccess>,
    ) -> Result<(), JoinRefusal> {
        if in_match {
            return Err(JoinRefusal::InMatch);
        }
        if !self.open {
            return Err(JoinRefusal::Closed);
        }
        let game = game.ok_or_else(|| JoinRefusal::UnknownGameType(game_type.to_owned()))?;
        if !game.enabled {
            return Err(JoinRefusal::Disabled(game_type.to_owned()));
        }
        if !game.has_valid_kit {
            return Err(JoinRefusal::NoKit(game_type.to_owned()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OK: GameAccess = GameAccess {
        enabled: true,
        has_valid_kit: true,
    };

    #[test]
    fn refusals_follow_the_documented_order() {
        let closed = Access::new(false);
        let open = Access::new(true);
        let no_kit_disabled = GameAccess {
            enabled: false,
            has_valid_kit: false,
        };
        assert_eq!(
            closed.check_join(true, "a", Some(OK)),
            Err(JoinRefusal::InMatch)
        );
        assert_eq!(
            closed.check_join(false, "a", None),
            Err(JoinRefusal::Closed)
        );
        assert_eq!(
            open.check_join(false, "a", None),
            Err(JoinRefusal::UnknownGameType("a".into()))
        );
        assert_eq!(
            open.check_join(false, "a", Some(no_kit_disabled)),
            Err(JoinRefusal::Disabled("a".into()))
        );
        let no_kit = GameAccess {
            enabled: true,
            has_valid_kit: false,
        };
        assert_eq!(
            open.check_join(false, "a", Some(no_kit)),
            Err(JoinRefusal::NoKit("a".into()))
        );
        assert_eq!(open.check_join(false, "a", Some(OK)), Ok(()));
    }

    #[test]
    fn timed_session_expires_and_untimed_does_not() {
        let mut access = Access::new(false);
        let closes = access.open(100, Some(2), 24).unwrap();
        assert_eq!(closes, Some(100 + 2 * TICKS_PER_HOUR));
        assert!(!access.expire(closes.unwrap() - 1));
        assert!(access.expire(closes.unwrap()));
        assert!(!access.is_open());

        access.open(0, None, 24).unwrap();
        assert!(!access.expire(u64::MAX));
        assert!(access.is_open());
    }

    #[test]
    fn hours_outside_the_bound_are_rejected_without_changing_state() {
        let mut access = Access::new(false);
        assert_eq!(access.open(0, Some(0), 24), Err(InvalidHours { max: 24 }));
        assert_eq!(access.open(0, Some(25), 24), Err(InvalidHours { max: 24 }));
        assert!(!access.is_open());
    }
}
