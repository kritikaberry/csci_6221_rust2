use redis::ToRedisArgs;
use uuid::Uuid;
use crate::error::PlayerError;
use serde::{Deserialize, Serialize};

#[derive(Eq, Hash, PartialEq, Clone, Copy, Serialize, Deserialize)]
pub struct PlayerId(Uuid);

impl ToRedisArgs for PlayerId {

}

impl PlayerId {
    pub fn new() -> PlayerId {
        let pid = Uuid::new_v4();
        PlayerId(pid)
    }
}
pub struct Mmr(i32);

impl Mmr {
    pub fn new() -> Mmr {
        Mmr(0)
    }
}
pub struct PlayerUsername(String);

impl TryFrom<String> for PlayerUsername {
    type Error = PlayerError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(PlayerError::EmptyPlayerUsername);
        }
        Ok(PlayerUsername(value)) 
    }
}
pub struct Player {
    pid: PlayerId,
    mmr: Mmr,
}

impl ToRedisArgs for Player {
    fn write_redis_args<W>(&self, out: &mut W)
        where
            W: ?Sized + redis::RedisWrite {
        
    }
}

impl Player {
    pub fn new() -> Player {
        let pid = PlayerId::new();
        let mmr = Mmr::new();

        Player {
            pid,
            mmr
        }
    }

    pub fn pid(&self) -> &PlayerId {
        &self.pid
    }

    pub fn mmr(&self) -> &Mmr {
        &self.mmr
    }
}