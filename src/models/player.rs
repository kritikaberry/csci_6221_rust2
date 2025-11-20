use redis::ToRedisArgs;
use redis_macros::{FromRedisValue, ToRedisArgs};
use uuid::Uuid;
use crate::error::PlayerError;
use serde::{Deserialize, Serialize};

#[derive(Eq, Hash, PartialEq, Clone, Copy, Serialize, Deserialize, ToRedisArgs, FromRedisValue)]
pub struct PlayerId(Uuid);

impl PlayerId {
    pub fn new() -> PlayerId {
        let pid = Uuid::new_v4();
        PlayerId(pid)
    }
}
#[derive(Serialize, Deserialize, ToRedisArgs, FromRedisValue)]
pub struct Mmr(i32);

impl Mmr {
    pub fn new() -> Mmr {
        Mmr(1200)
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

#[derive(Serialize, Deserialize, ToRedisArgs, FromRedisValue)]
pub struct Player {
    pid: PlayerId,
}

impl Player {
    pub fn new() -> Player {
        let pid = PlayerId::new();
        let mmr = Mmr::new();

        Player {
            pid,
        }
    }

    pub fn pid(&self) -> &PlayerId {
        &self.pid
    }
}