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

// -------------------------------------------------
// BOT / CLEANUP HELPERS
// -------------------------------------------------
pub async fn mark_as_bot(id: u64) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let key = format!("player:{}", id);
    let _: () = con.hset(&key, "is_bot", 1).await.unwrap_or(());
}

pub async fn is_bot(id: u64) -> bool {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let key = format!("player:{}", id);
    let bot_flag: i32 = con.hget(&key, "is_bot").await.unwrap_or(0);
    bot_flag == 1
}

pub async fn clear_bot_data(id: u64) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    
    // Remove from queue if present
    remove_from_queue(id).await;
    
    // Remove only the bot's player data
    let key = format!("player:{}", id);
    let _: () = con.del(&key).await.unwrap_or(());
    
    tracing::info!("🧹 Bot player data cleared for player {} (matches and sessions preserved)", id);
}

// -------------------------------------------------
// COMPLETED MATCHES (Match History)
// -------------------------------------------------
pub async fn create_completed_match(match_id: u64, player1: u64, player2: u64, winner: u64, loser: u64) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let key = format!("completed_match:{}", match_id);
    let _: () = con.hset(&key, "match_id", match_id).await.unwrap_or(());
    let _: () = con.hset(&key, "player1", player1).await.unwrap_or(());
    let _: () = con.hset(&key, "player2", player2).await.unwrap_or(());
    let _: () = con.hset(&key, "winner", winner).await.unwrap_or(());
    let _: () = con.hset(&key, "loser", loser).await.unwrap_or(());
    
    // Store timestamp
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let _: () = con.hset(&key, "completed_at", timestamp).await.unwrap_or(());

    tracing::info!("📋 COMPLETED MATCH STORED → match_id={} P{} wins over P{}", match_id, winner, loser);
}

pub async fn get_completed_matches() -> Vec<(u64, u64, u64, u64, u64, u64)> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let keys: Vec<String> = con.keys("completed_match:*").await.unwrap_or_default();
    let mut matches = Vec::new();

    for k in keys {
        let match_id: Option<u64> = con.hget(&k, "match_id").await.ok();
        let player1: Option<u64> = con.hget(&k, "player1").await.ok();
        let player2: Option<u64> = con.hget(&k, "player2").await.ok();
        let winner: Option<u64> = con.hget(&k, "winner").await.ok();
        let loser: Option<u64> = con.hget(&k, "loser").await.ok();
        let completed_at: Option<u64> = con.hget(&k, "completed_at").await.ok();

        if let (Some(mid), Some(p1), Some(p2), Some(w), Some(l), Some(timestamp)) = 
            (match_id, player1, player2, winner, loser, completed_at) {
            matches.push((mid, p1, p2, w, l, timestamp));
        }
    }

    // Sort by timestamp descending (most recently completed first)
    matches.sort_by(|a, b| b.5.cmp(&a.5));
    matches
}

// -------------------------------------------------
// GAME SESSIONS (for race game)
// -------------------------------------------------
pub async fn create_game_session(match_id: u64, session_id: String, player1_id: u64, player2_id: u64, player1_slot: u8, player2_slot: u8) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let key = format!("game_session:{}", session_id);
    let _: () = con.hset(&key, "match_id", match_id).await.unwrap_or(());
    let _: () = con.hset(&key, "player1_id", player1_id).await.unwrap_or(());
    let _: () = con.hset(&key, "player2_id", player2_id).await.unwrap_or(());
    let _: () = con.hset(&key, "player1_slot", player1_slot).await.unwrap_or(());
    let _: () = con.hset(&key, "player2_slot", player2_slot).await.unwrap_or(());
    let _: () = con.hset(&key, "status", "waiting").await.unwrap_or(());

    // Also store session_id in match for easy lookup
    let match_key = format!("match:{}", match_id);
    let _: () = con.hset(&match_key, "game_session", &session_id).await.unwrap_or(());

    tracing::info!("🎮 GAME SESSION CREATED → session={} match={} P{} (slot {}) vs P{} (slot {})", 
                   session_id, match_id, player1_id, player1_slot, player2_id, player2_slot);
}

pub async fn get_game_session(session_id: &str) -> Option<(u64, u64, u64, u8, u8)> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let key = format!("game_session:{}", session_id);
    let match_id: Option<u64> = con.hget(&key, "match_id").await.ok();
    let player1_id: Option<u64> = con.hget(&key, "player1_id").await.ok();
    let player2_id: Option<u64> = con.hget(&key, "player2_id").await.ok();
    let player1_slot: Option<u8> = con.hget(&key, "player1_slot").await.ok();
    let player2_slot: Option<u8> = con.hget(&key, "player2_slot").await.ok();

    if let (Some(mid), Some(p1), Some(p2), Some(s1), Some(s2)) = (match_id, player1_id, player2_id, player1_slot, player2_slot) {
        Some((mid, p1, p2, s1, s2))
    } else {
        None
    }
}

pub async fn get_game_session_by_match(match_id: u64) -> Option<String> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let match_key = format!("match:{}", match_id);
    con.hget(&match_key, "game_session").await.ok()
}

pub async fn update_game_session_status(session_id: &str, status: &str) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let key = format!("game_session:{}", session_id);
    let _: () = con.hset(&key, "status", status).await.unwrap_or(());
}

pub async fn get_match_id_by_players(player1: u64, player2: u64) -> Option<u64> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let keys: Vec<String> = con.keys("match:*").await.unwrap_or_default();
    for k in keys {
        let a: u64 = con.hget(&k, "a").await.unwrap_or(0);
        let b: u64 = con.hget(&k, "b").await.unwrap_or(0);
        if (a == player1 && b == player2) || (a == player2 && b == player1) {
            if let Some(match_id_str) = k.strip_prefix("match:") {
                if let Ok(match_id) = match_id_str.parse::<u64>() {
                    return Some(match_id);
                }
            }
        }
    }
    None
}

pub async fn remove_game_session(session_id: &str) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let key = format!("game_session:{}", session_id);
    let _: () = con.del(key).await.unwrap_or(());
    tracing::info!("🗑️ Game session {} removed", session_id);
}

pub async fn cleanup_player_on_disconnect(id: u64) {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    
    // Remove from queue
    remove_from_queue(id).await;
    
    // Check if player is in an ongoing match
    if let Some((opp, _)) = check_match(id).await {
        // Find the match_id
        if let Some(match_id) = get_match_id_by_players(id, opp).await {
            // Remove the match
            remove_match(match_id).await;
            
            // Update opponent's status back to idle and remove from queue
            let opp_key = format!("player:{}", opp);
            let _: () = con.hset(&opp_key, "status", "idle").await.unwrap_or(());
            remove_from_queue(opp).await;
            
            tracing::info!("🔌 Player {} disconnected, match {} (vs P{}) removed", id, match_id, opp);
        }
    }
    
    // Update player status to idle
    let key = format!("player:{}", id);
    let _: () = con.hset(&key, "status", "idle").await.unwrap_or(());
    
    tracing::info!("🧹 Cleaned up disconnected player {}", id);
}
