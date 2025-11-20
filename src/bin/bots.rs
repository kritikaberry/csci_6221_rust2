// bots.rs — HIGHLY REALISTIC SLOW TCP BOT CLIENT
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::{sleep, Duration},
};
use rand::{Rng, rngs::StdRng, SeedableRng};

// Note: We can't directly import pong_game here due to circular dependency
// Instead, we'll spawn a separate process or use a different approach
// For now, bots will wait for game completion and submit results

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bots: usize = args.get(1).and_then(|x| x.parse().ok()).unwrap_or(10);

    println!("🤖 Launching {bots} realistic TCP bots...");

    for i in 1..=bots {
        tokio::spawn(async move { run_bot(i as u64).await });
    }

    loop { sleep(Duration::from_secs(60)).await; }
}

async fn run_bot(id: u64) {
    let mut rng = StdRng::from_entropy();

    // BOT PERSONALITY → DIFFERENT SPEED PROFILES
    let personality = rng.gen_range(1..=4);

    let mut queue_min = match personality {
        1 => 4000,  // fast player
        2 => 6000,  // normal
        3 => 9000,  // slower
        _ => 12000, // very slow
    };

    let mut queue_max = queue_min + 5000;

    // CONNECT TO SERVER
    let result = TcpStream::connect("127.0.0.1:7500").await;
    let mut stream = match result {
        Ok(s) => s,
        Err(e) => {
            eprintln!("BOT {id} cannot connect: {}", e);
            csci_6221_rust2::storage::clear_bot_data(id).await;
            return;
        }
    };

    let (reader, mut writer) = stream.split();
    let mut lines = BufReader::new(reader).lines();

    // REGISTER
    if writer.write_all(format!("REGISTER {}\n", id).as_bytes()).await.is_err() {
        eprintln!("BOT {id} failed to register (connection error)");
        csci_6221_rust2::storage::cleanup_player_on_disconnect(id).await;
        return;
    }
    
    if let Ok(Some(line)) = lines.next_line().await {
        println!("🤖 BOT {id}: {line}");
    } else {
        eprintln!("BOT {id} failed to receive registration response (connection error)");
        csci_6221_rust2::storage::cleanup_player_on_disconnect(id).await;
        return;
    }
    
    // Mark as bot in Redis
    csci_6221_rust2::storage::mark_as_bot(id).await;

    // --- MAIN LOOP ---
    let mut matches_played = 0;

    loop {
        // 🟡 bots wait realistically before queueing
        let wait = rng.gen_range(queue_min..queue_max);
        sleep(Duration::from_millis(wait)).await;

        // Check connection before queueing
        if writer.write_all(format!("QUEUE {}\n", id).as_bytes()).await.is_err() {
            eprintln!("BOT {id} connection died during QUEUE");
            csci_6221_rust2::storage::cleanup_player_on_disconnect(id).await;
            return;
        }
        println!("📥 BOT {id} queued (waited {}ms)", wait);

        // --- POLL LOOP ---
        loop {
            // realistic polling timing
            let poll_delay = rng.gen_range(1200..2500);
            sleep(Duration::from_millis(poll_delay)).await;

            // Check connection before polling
            if writer.write_all(format!("POLL {}\n", id).as_bytes()).await.is_err() {
                eprintln!("BOT {id} connection died during POLL");
                csci_6221_rust2::storage::cleanup_player_on_disconnect(id).await;
                return;
            }

            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.starts_with("MATCH") {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        let opp: u64 = parts[1].parse().unwrap();
                        
                        // Check if session ID is provided (for game sessions)
                        let session_id = if parts.len() >= 4 && parts[2] == "SESSION" {
                            Some(parts[3].to_string())
                        } else {
                            None
                        };
                        
                        let opp_is_bot = csci_6221_rust2::storage::is_bot(opp).await;
                        if opp_is_bot {
                            println!("🎯 BOT {id} matched vs BOT {}", opp);
                        } else {
                            println!("🎯 BOT {id} matched vs PLAYER {}", opp);
                        }

                        // If there's a game session, bots should actually play the game
                        // Note: This is generic - games can implement their own bot clients
                        // The bot client should be launched by the game itself, not by the core matchmaking system
                        if let Some(session) = &session_id {
                            println!("🎮 BOT {id} matched with game session: {}", session);
                            println!("   Note: Game-specific bot clients should handle actual gameplay");
                            
                            // Wait for game to complete (game client will handle result submission)
                            // Wait up to 2 minutes for game to finish
                            let max_wait = Duration::from_secs(120);
                            let start = std::time::Instant::now();
                            
                            while start.elapsed() < max_wait {
                                // Check if match is still ongoing
                                if let Some(_mid) = csci_6221_rust2::storage::get_match_id_by_players(id, opp).await {
                                    // Match still exists, game might still be running
                                    sleep(Duration::from_millis(1000)).await;
                                } else {
                                    // Match completed (moved to completed matches)
                                    println!("✅ BOT {id}: Game completed");
                                    break;
                                }
                            }
                            
                            // Game should be completed by now (result submitted by game client)
                            // Continue to queue again
                            break;
                        } else {
                            // No game session, submit random result (legacy behavior)
                            let reaction = rng.gen_range(800..2000);
                            sleep(Duration::from_millis(reaction)).await;

                            // random win/lose
                            let win = rng.gen_bool(0.5) as u8;

                            // Check connection before sending result
                            if writer.write_all(format!("RESULT {} {} {}\n", id, opp, win).as_bytes()).await.is_err() {
                                eprintln!("BOT {id} connection died during RESULT");
                                csci_6221_rust2::storage::cleanup_player_on_disconnect(id).await;
                                return;
                            }

                            match lines.next_line().await {
                                Ok(Some(res)) => println!("🏆 BOT {id}: {res}"),
                                Ok(None) | Err(_) => {
                                    eprintln!("BOT {id} connection died after RESULT");
                                    csci_6221_rust2::storage::cleanup_player_on_disconnect(id).await;
                                    return;
                                }
                            }
                        }

                        matches_played += 1;

                        // 🛌 fatigue system: bots gradually slow down over time
                        if matches_played % 5 == 0 {
                            let fatigue = rng.gen_range(3000..7000);
                            println!("😴 BOT {id} resting for {fatigue}ms (fatigue)");
                            sleep(Duration::from_millis(fatigue)).await;

                            queue_min += 500; // becomes slower
                            queue_max += 800;
                        }

                        // idle time after match
                        let idle = rng.gen_range(2500..6000);
                        sleep(Duration::from_millis(idle)).await;

                        break; // back to queue
                    }
                }
                Ok(None) => {
                    // Connection closed
                    eprintln!("BOT {id} connection closed by server");
                    csci_6221_rust2::storage::cleanup_player_on_disconnect(id).await;
                    return;
                }
                Err(_) => {
                    // Connection error
                    eprintln!("BOT {id} connection error during POLL");
                    csci_6221_rust2::storage::cleanup_player_on_disconnect(id).await;
                    return;
                }
            }
        }
    }
    
    // Cleanup on exit (if we ever get here)
    csci_6221_rust2::storage::cleanup_player_on_disconnect(id).await;
}
