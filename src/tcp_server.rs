use tokio::{
    net::{TcpListener, TcpStream},
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};
use rand::Rng;
use tracing::info;
use redis::AsyncCommands;

use crate::{
    storage::*,
    elo::apply_elo,
    matchmaking::hybrid_c_decision,
};

pub async fn start_tcp_server(addr: &str) {
    let listener = TcpListener::bind(addr).await.unwrap();

    info!("🚀 Matchmaking Server Running @ {}", addr);
    info!("📡 Waiting for clients...");

    loop {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move { handle_client(socket).await });
    }
}

async fn handle_client(mut socket: TcpStream) {
    let (reader, mut writer) = socket.split();
    let mut lines = BufReader::new(reader).lines();
    let mut player_id: Option<u64> = None;

    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.is_empty() { continue; }

                // Track player ID from various commands
                match parts[0] {
                    "REGISTER" | "QUEUE" | "POLL" | "RESULT" | "VIEW" => {
                        if parts.len() > 1 {
                            if let Ok(id) = parts[1].parse::<u64>() {
                                player_id = Some(id);
                            }
                        }
                    }
                    _ => {}
                }

                match parts[0] {

            // -------------------------------------------------
            // REGISTER
            // -------------------------------------------------
            "REGISTER" => {
                let id = parts[1].parse().unwrap();
                let rating = rand::thread_rng().gen_range(1100..1900);

                register_player(id, rating).await;

                info!("🆕 REGISTERED Player {} (rating {})", id, rating);

                writer.write_all(format!("REGISTERED {}\n", rating).as_bytes()).await.unwrap();
            }

            // -------------------------------------------------
            // QUEUE
            // -------------------------------------------------
            "QUEUE" => {
                let id = parts[1].parse().unwrap();

                // Check if player is already in a match
                if let Some((opp, _)) = check_match(id).await {
                    info!("⚠️ Player {} is already in a match with P{}", id, opp);
                    writer.write_all(b"ALREADY_IN_MATCH\n").await.unwrap();
                    continue;
                }

                // Check player status
                let client = redis::Client::open("redis://127.0.0.1/").unwrap();
                let mut con = client.get_multiplexed_async_connection().await.unwrap();
                let key = format!("player:{}", id);
                let status: String = con.hget(&key, "status").await.unwrap_or("idle".into());
                
                if status == "inmatch" {
                    info!("⚠️ Player {} is already in a match (status: inmatch)", id);
                    writer.write_all(b"ALREADY_IN_MATCH\n").await.unwrap();
                    continue;
                }

                if push_to_queue(id).await {
                    if is_bot(id).await {
                        info!("🤖 Bot {} added to queue (bot-only matching)", id);
                    } else {
                        info!("📥 Player {} added to queue", id);
                    }
                } else {
                    info!("⚠️ Player {} cannot queue: already in a match", id);
                    writer.write_all(b"ALREADY_IN_MATCH\n").await.unwrap();
                }

                // Provide dashboard link
                writer.write_all(b"QUEUED\n").await.unwrap();
                writer.write_all(b"DASHBOARD http://localhost:8080/\n").await.unwrap();
            }

            // -------------------------------------------------
            // POLL (Option-1: full scan for best match)
            // -------------------------------------------------
            "POLL" => {
                let id = parts[1].parse().unwrap();

                // 1. Already matched?
                if let Some((opp, _)) = check_match(id).await {
                    info!("🎯 MATCH FOUND for Player {} → Opponent {}", id, opp);
                    // Try to get session from match
                    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
                    let mut con = client.get_multiplexed_async_connection().await.unwrap();
                    let keys: Vec<String> = con.keys("match:*").await.unwrap_or_default();
                    for k in keys {
                        let a: u64 = con.hget(&k, "a").await.unwrap_or(0);
                        let b: u64 = con.hget(&k, "b").await.unwrap_or(0);
                        if (a == id && b == opp) || (a == opp && b == id) {
                            let match_id_str = k.strip_prefix("match:").unwrap_or("");
                            if let Ok(match_id) = match_id_str.parse::<u64>() {
                                if let Some(session_id) = get_game_session_by_match(match_id).await {
                                    writer.write_all(format!("MATCH {} SESSION {}\n", opp, session_id).as_bytes()).await.unwrap();
                                } else {
                                    writer.write_all(format!("MATCH {}\n", opp).as_bytes()).await.unwrap();
                                }
                            } else {
                                writer.write_all(format!("MATCH {}\n", opp).as_bytes()).await.unwrap();
                            }
                            continue;
                        }
                    }
                    writer.write_all(format!("MATCH {}\n", opp).as_bytes()).await.unwrap();
                    continue;
                }

                // 2. Try to match THIS player (correct logic)
                // Bots can only match with bots, real players only with real players
                let is_me_bot = is_bot(id).await;
                
                let my_rating = get_rating(id).await;
                let mut q = get_full_queue().await;

                // remove myself from local view
                q.retain(|&x| x != id);
                
                // Filter queue: bots match with bots, real players match with real players
                // Also exclude players already in a match (both status and actual match check)
                let mut candidates = Vec::new();
                for player_id in q {
                    let is_opp_bot = is_bot(player_id).await;
                    // Only match if both are bots OR both are real players
                    if is_me_bot == is_opp_bot {
                        // Check if opponent is already in a match (both status and actual match check)
                        let is_in_match = check_match(player_id).await.is_some();
                        if is_in_match {
                            // Remove from queue if they're in a match but still in queue (cleanup)
                            remove_from_queue(player_id).await;
                            continue;
                        }
                        
                        // Also check status
                        let opp_status: String = {
                            let client = redis::Client::open("redis://127.0.0.1/").unwrap();
                            let mut con = client.get_multiplexed_async_connection().await.unwrap();
                            let key = format!("player:{}", player_id);
                            con.hget(&key, "status").await.unwrap_or("idle".into())
                        };
                        // Only include if not already in a match
                        if opp_status != "inmatch" {
                            candidates.push(player_id);
                        } else {
                            // Status says inmatch but still in queue - remove from queue as cleanup
                            remove_from_queue(player_id).await;
                        }
                    }
                }

                for opp in candidates {
                    let r2 = get_rating(opp).await;

                    if hybrid_c_decision(my_rating, r2) {
                        // Double-check both players are still available (not matched in the meantime)
                        let my_match = check_match(id).await;
                        let opp_match = check_match(opp).await;
                        if my_match.is_some() || opp_match.is_some() {
                            // One of them got matched elsewhere, skip
                            continue;
                        }
                        
                        // remove both from queue (create_match will also do this, but do it here for safety)
                        remove_from_queue(id).await;
                        remove_from_queue(opp).await;

                        let mid = rand::thread_rng().gen_range(10000..99999);

                        create_match(id, opp, mid).await;

                        // Create generic game session (game-agnostic)
                        // Session ID format: game_<match_id> (games can use their own prefix)
                        let session_id = format!("game_{}", mid);
                        // Randomly assign slots (1 or 2)
                        let (p1_slot, p2_slot) = if rand::random::<bool>() {
                            (1, 2)
                        } else {
                            (2, 1)
                        };
                        // Randomly assign which player gets which slot
                        let (p1_id, p2_id) = if rand::random::<bool>() {
                            (id, opp)
                        } else {
                            (opp, id)
                        };
                        
                        create_game_session(mid, session_id.clone(), p1_id, p2_id, p1_slot, p2_slot).await;

                        if is_me_bot {
                            info!("🤖 BOT MATCH {} vs {} | match_id {} | session {}", id, opp, mid, session_id);
                        } else {
                            info!("🔥 PLAYER MATCH {} vs {} | match_id {} | session {}", id, opp, mid, session_id);
                        }

                        // Send match with game session link
                        writer.write_all(format!("MATCH {} SESSION {}\n", opp, session_id).as_bytes()).await.unwrap();
                        continue;
                    }
                }

                // 3. No match
                writer.write_all(b"NO_MATCH\n").await.unwrap();
            }

            // -------------------------------------------------
            // VIEW (for viewers to get session info)
            // -------------------------------------------------
            "VIEW" => {
                if parts.len() < 2 {
                    writer.write_all(b"ERR Usage: VIEW <match_id>\n").await.unwrap();
                    continue;
                }
                
                let match_id_str = parts[1];
                let match_id: u64 = match match_id_str.parse() {
                    Ok(id) => id,
                    Err(_) => {
                        writer.write_all(b"ERR Invalid match_id\n").await.unwrap();
                        continue;
                    }
                };
                
                // Get session ID from match
                if let Some(session_id) = get_game_session_by_match(match_id).await {
                    info!("👁️ Viewer requested session {} for match {}", session_id, match_id);
                    writer.write_all(format!("SESSION {}\n", session_id).as_bytes()).await.unwrap();
                    // Generic viewer command - games should provide their own viewer client
                    writer.write_all(format!("VIEW_SESSION {}\n", session_id).as_bytes()).await.unwrap();
                } else {
                    writer.write_all(b"ERR No session found for this match\n").await.unwrap();
                }
            }

            // -------------------------------------------------
            // RESULT
            // -------------------------------------------------
            "RESULT" => {
                let id = parts[1].parse().unwrap();
                let opp = parts[2].parse().unwrap();
                let win = parts[3] == "1";

                // Winner/loser
                let (winner, loser) = if win { (id, opp) } else { (opp, id) };

                // Use complete_match_with_result to ensure all stats are updated together
                // This ensures ratings, wins, losses are only updated when matches complete
                // Rankings are based solely on match history
                let winner_rating_before = get_rating(winner).await;
                let loser_rating_before = get_rating(loser).await;
                complete_match_with_result(winner, loser).await;
                let winner_rating_after = get_rating(winner).await;
                let loser_rating_after = get_rating(loser).await;
                
                writer.write_all(format!("RESULT_OK {} {}\n", winner_rating_after, loser_rating_after).as_bytes()).await.unwrap();
                info!("🏆 RESULT: P{} wins over P{} | Ratings: {}→{} (winner), {}→{} (loser) | Stats updated from match history", 
                    winner, loser, winner_rating_before, winner_rating_after, loser_rating_before, loser_rating_after);
            }

            _ => {
                writer.write_all(b"ERR\n").await.unwrap();
            }
                }
            }
            Ok(None) => {
                // Connection closed by client
                break;
            }
            Err(_) => {
                // Error reading from connection
                break;
            }
        }
    }

    // Cleanup on disconnect
    if let Some(id) = player_id {
        cleanup_player_on_disconnect(id).await;
        info!("🔌 Client disconnected, cleaned up player {}", id);
    }
}
