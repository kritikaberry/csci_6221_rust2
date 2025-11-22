use redis::{RedisError, aio::MultiplexedConnection, AsyncCommands};
use crate::Mmr;
use crate::PlayerId;

pub struct DatabaseHandler {
    db_con: MultiplexedConnection,

}

const SORTED_PLAYERS: &str = "sorted_players";

impl Clone for DatabaseHandler {
    fn clone(&self) -> Self {
        DatabaseHandler { db_con: self.db_con.clone() }
    }
}

impl DatabaseHandler {

    pub async fn new(db_url: &str) -> Result<DatabaseHandler, RedisError> {
        let client = redis::Client::open(db_url)?; 
        let db_con = client.get_multiplexed_async_connection().await?;

        Ok(DatabaseHandler {
            db_con,
        })
    }

    pub async fn add_player(&mut self, pid: PlayerId) -> Result<(), RedisError> {
        let _: () = self.db_con.zadd(SORTED_PLAYERS, pid, Mmr::new()).await?;
        Ok(())
    }
}