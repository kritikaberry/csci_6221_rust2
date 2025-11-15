use tokio::time::{sleep, Duration};
use redis::AsyncCommands;
use rand::Rng;

#[tokio::main]
async fn main() {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    println!("Matchmaking server started...");

    loop {
        let q = "queue:demo_game";
        let players: Vec<String> = con.lrange(q, 0, -1).await.unwrap_or(vec![]);

        if players.len() >= 2 {
            let p1: String = con.rpop(q, None).await.unwrap();
            let p2: String = con.rpop(q, None).await.unwrap();

            let a = p1.parse::<u64>().unwrap();
            let b = p2.parse::<u64>().unwrap();

            let match_id = format!("match:{}", rand::thread_rng().gen::<u32>());

            // Set match hash fields individually to avoid temporary string slice lifetimes.
            let _ : () = con.hset(&match_id, "a", a).await.unwrap();
            let _ : () = con.hset(&match_id, "b", b).await.unwrap();
            let _ : () = con.hset(&match_id, "game_id", "demo_game").await.unwrap();

            let _ : () = con.hset(format!("player:{}", a), "status", "in_match").await.unwrap();
            let _ : () = con.hset(format!("player:{}", b), "status", "in_match").await.unwrap();

            println!("MATCH CREATED → {} vs {}", a, b);
        }

        sleep(Duration::from_millis(500)).await;
    }
}
