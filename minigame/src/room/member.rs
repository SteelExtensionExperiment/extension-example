use std::{collections::HashSet, sync::LazyLock};

use parking_lot::Mutex;
use steel_extension_api::{
    PlayerId, PlayerInfo, Server,
    stabby::{string::String as ApiString, vec::Vec as ApiVec},
};

use super::domain;

static ADMISSIONS: LazyLock<Mutex<HashSet<PlayerId>>> = LazyLock::new(Mutex::default);

pub(super) struct Human {
    pub(super) player: PlayerId,
    pub(super) arrived: bool,
    pub(super) leaving: bool,
    home: String,
    visit_requested: bool,
    return_requested: bool,
}

impl Human {
    pub(super) fn reserve(player: PlayerId, home: &str) -> Result<Self, ApiString> {
        if !ADMISSIONS.lock().insert(player) {
            return Err("You are already joining or playing in another room".into());
        }
        Ok(Self {
            player,
            home: home.to_owned(),
            visit_requested: false,
            arrived: false,
            leaving: false,
            return_requested: false,
        })
    }
}

impl Drop for Human {
    fn drop(&mut self) {
        ADMISSIONS.lock().remove(&self.player);
    }
}

impl Human {
    pub(super) fn update(
        &mut self,
        server: &Server<'_>,
        players: &[PlayerInfo],
        room_domain: &str,
        domain_created: bool,
        arrivals: &mut ApiVec<PlayerId>,
    ) -> Result<bool, ApiString> {
        let id = self.player;
        if !server.is_connected(id) || (self.leaving && !self.visit_requested) {
            return Ok(false);
        }
        let Some(player) = players.iter().find(|player| player.id == id) else {
            return Ok(true);
        };
        if !server.player_ready(id) {
            return Ok(true);
        }
        let in_room = domain(player.world.as_str()) == room_domain;
        if !in_room && (self.arrived || self.leaving) {
            return Ok(false);
        }
        if self.leaving {
            if !self.return_requested {
                server
                    .switch_domain(id, self.home.as_str().into())
                    .match_owned(Ok, Err)?;
                self.return_requested = true;
            }
        } else if !self.visit_requested && domain_created {
            server
                .switch_domain(id, room_domain.into())
                .match_owned(Ok, Err)?;
            self.visit_requested = true;
        } else if in_room && !self.arrived {
            self.arrived = true;
            arrivals.push(id);
        }
        Ok(true)
    }
}
