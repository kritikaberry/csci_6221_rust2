use redis::AsyncCommands;
use crate::config::AppCfg;
use crate::models::PlayerId;

pub async fn ensure_player(
    con: &mut redis::aio::MultiplexedConnection,
    id: PlayerId,
    game_id: &str,
    cfg: &AppCfg,
) -> redis::RedisResult<(f64, u32)> {
    let key = format!("player:{id}");

    if !con.exists(&key).await? {
        let _: () = con.hset(&key, "rating", cfg.default_rating).await?;
        let _: () = con.hset(&key, "games_played", 0).await?;
        let _: () = con.hset(&key, "status", "idle").await?;
        let _: () = con.hset(&key, "game_id", game_id).await?;
        return Ok((cfg.default_rating, 0));
    }

    let r: f64 = con.hget(&key, "rating").await.unwrap_or(cfg.default_rating);
    let g: u32 = con.hget(&key, "games_played").await.unwrap_or(0);
    Ok((r, g))
}

pub async fn save_player(
    con: &mut redis::aio::MultiplexedConnection,
    id: PlayerId,
    game_id: &str,
    rating: f64,
    games: u32,
) -> redis::RedisResult<()> {
    let key = format!("player:{id}");
    let set_key = format!("game:{game_id}:players_by_rating");

    let _: () = con.hset(&key, "rating", rating).await?;
    let _: () = con.hset(&key, "games_played", games).await?;
    let _: () = con.zadd(&set_key, id.to_string(), rating).await?;

    Ok(())
}

pub async fn set_status(
    con: &mut redis::aio::MultiplexedConnection,
    id: PlayerId,
    status: &str,
) -> redis::RedisResult<()> {
    con.hset(format!("player:{id}"), "status", status).await
}
