use tokio::{
    net::{TcpListener, TcpStream},
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
};
use rand::Rng;
use tracing::info;

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

    while let Ok(Some(line)) = lines.next_line().await {
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() { continue; }

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

                push_to_queue(id).await;

                info!("📥 Player {} added to queue", id);

                writer.write_all(b"QUEUED\n").await.unwrap();
            }

            // -------------------------------------------------
            // POLL (Option-1: full scan for best match)
            // -------------------------------------------------
            "POLL" => {
                let id = parts[1].parse().unwrap();

                // 1. Already matched?
                if let Some((opp, _)) = check_match(id).await {
                    info!("🎯 MATCH FOUND for Player {} → Opponent {}", id, opp);
                    writer.write_all(format!("MATCH {}\n", opp).as_bytes()).await.unwrap();
                    continue;
                }

                // 2. Try to match THIS player (correct logic)
                let my_rating = get_rating(id).await;
                let mut q = get_full_queue().await;

                // remove myself from local view
                q.retain(|&x| x != id);

                for opp in q {
                    let r2 = get_rating(opp).await;

                    if hybrid_c_decision(my_rating, r2) {
                        // remove both from queue
                        remove_from_queue(id).await;
                        remove_from_queue(opp).await;

                        let mid = rand::thread_rng().gen_range(10000..99999);

                        create_match(id, opp, mid).await;

                        info!("🔥 MATCH {} vs {} | match_id {}", id, opp, mid);


                        writer.write_all(format!("MATCH {}\n", opp).as_bytes()).await.unwrap();
                        continue;
                    }
                }

                // 3. No match
                writer.write_all(b"NO_MATCH\n").await.unwrap();
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

                // Elo for both sides
                let winner_new = apply_elo(get_rating(winner).await, get_rating(loser).await, true);
                let loser_new = apply_elo(get_rating(loser).await, get_rating(winner).await, false);
                update_rating(winner, winner_new).await;
                update_rating(loser, loser_new).await;

                // Persist win/loss counters
                record_result(winner, loser).await;

                info!("🏆 RESULT: winner=P{} (new {}) loser=P{} (new {})", winner, winner_new, loser, loser_new);
                writer.write_all(format!("RESULT_OK {} {}\n", winner_new, loser_new).as_bytes()).await.unwrap();
            }

            _ => {
                writer.write_all(b"ERR\n").await.unwrap();
            }
        }
    }
}
