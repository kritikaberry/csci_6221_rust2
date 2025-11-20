use redis::AsyncCommands;
use std::time::{SystemTime, UNIX_EPOCH};

const QUEUE: &str = "queue:demo";

// -------------------------------------------------
// PLAYER INFO
// -------------------------------------------------
pub async fn register_player(id: u64, rating: i32) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let key = format!("player:{id}");
    let _: () = con.hset(&key, "rating", rating).await.unwrap();
    let _: () = con.hset(&key, "status", "idle").await.unwrap();
    let _: () = con.hset(&key, "wins", 0).await.unwrap();
    let _: () = con.hset(&key, "losses", 0).await.unwrap();
}

pub async fn get_rating(id: u64) -> i32 {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let key = format!("player:{id}");
    con.hget(&key, "rating").await.unwrap_or(1200)
}

pub async fn update_rating(id: u64, new_rating: i32) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let key = format!("player:{id}");

    let _: () = con.hset(&key, "rating", new_rating).await.unwrap();
    let _: () = con.hset(&key, "status", "idle").await.unwrap();
}

// -------------------------------------------------
// QUEUE OPS — FIFO (RPUSH / LPOP)
// -------------------------------------------------
pub async fn push_to_queue(id: u64) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let _: () = con.rpush(QUEUE, id).await.unwrap();
    let _: () = con.hset(format!("player:{id}"), "status", "queued").await.unwrap();
}

pub async fn pop_queue() -> Option<u64> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    con.lpop(QUEUE, None).await.unwrap_or(None)
}

pub async fn remove_from_queue(id: u64) -> bool {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let removed: i64 = con.lrem(QUEUE, 1, id).await.unwrap_or(0);
    removed > 0
}

pub async fn get_full_queue() -> Vec<u64> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    con.lrange(QUEUE, 0, -1).await.unwrap_or_default()
}

// -------------------------------------------------
// MATCHES
// -------------------------------------------------
pub async fn create_match(a: u64, b: u64, match_id: u64) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let key = format!("match:{match_id}");
    let _: () = con.hset(&key, "a", a).await.unwrap();
    let _: () = con.hset(&key, "b", b).await.unwrap();
    let _: () = con.hset(&key, "game_id", "demo").await.unwrap();
    // mark players as in a match
    let _ : () = con.hset(format!("player:{}", a), "status", "inmatch").await.unwrap();
    let _ : () = con.hset(format!("player:{}", b), "status", "inmatch").await.unwrap();
    // record start timestamp (unix seconds)
    let started = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let _ : () = con.hset(&key, "started_at", started).await.unwrap();

    tracing::info!("🟢 MATCH CREATED → id={match_id}, P{a} vs P{b}");
}

pub async fn check_match(id: u64) -> Option<(u64, String)> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let keys: Vec<String> = con.keys("match:*").await.unwrap_or_default();

    for k in keys {
        let a: u64 = con.hget(&k, "a").await.unwrap_or(0);
        let b: u64 = con.hget(&k, "b").await.unwrap_or(0);

        if a == id { return Some((b, "demo".into())); }
        if b == id { return Some((a, "demo".into())); }
    }
    None
}

pub async fn remove_match(match_id: u64) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let key = format!("match:{match_id}");
    let _: () = con.del(key).await.unwrap_or(());
}

// -------------------------------------------------
// RESULTS (WIN / LOSS TRACKING)
// -------------------------------------------------
pub async fn record_result(winner: u64, loser: u64) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let wkey = format!("player:{winner}");
    let lkey = format!("player:{loser}");

    let _: () = con.hincr(&wkey, "wins", 1).await.unwrap();
    let _: () = con.hincr(&lkey, "losses", 1).await.unwrap();
    // Mark winner back to idle (could be queued again manually)
    let _: () = con.hset(&wkey, "status", "idle").await.unwrap();
    let _: () = con.hset(&lkey, "status", "idle").await.unwrap();

    tracing::info!("🏁 RESULT STORED → winner=P{} loser=P{}", winner, loser);
}
