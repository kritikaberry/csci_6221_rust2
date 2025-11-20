// player_ui.rs — HUMAN PLAYER WHO LOOPS FOREVER LIKE A BOT
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::{sleep, Duration},
};

#[tokio::main]
async fn main() {
    println!("🎮 Player Client Started (Auto-Match Enabled)");

    // Enter Player ID
    print!("Enter Player ID: ");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let id: u64 = input.trim().parse().expect("Invalid ID");

    // Connect to server
    let mut stream = TcpStream::connect("127.0.0.1:7500")
        .await
        .expect("Cannot connect to matchmaking server");

    let (reader, mut writer) = stream.split();
    let mut lines = BufReader::new(reader).lines();

    // REGISTER
    writer
        .write_all(format!("REGISTER {}\n", id).as_bytes())
        .await
        .unwrap();

    if let Ok(Some(line)) = lines.next_line().await {
        println!("➡ {}", line);
    }

    println!("➡ Auto-queue enabled. Player will continuously play matches.");

    loop {
        // QUEUE
        writer
            .write_all(format!("QUEUE {}\n", id).as_bytes())
            .await
            .unwrap();

        println!("📥 Player {} queued.", id);

        // POLL for match
        loop {
            sleep(Duration::from_millis(1200)).await;

            writer
                .write_all(format!("POLL {}\n", id).as_bytes())
                .await
                .unwrap();

            if let Ok(Some(line)) = lines.next_line().await {
                if line.starts_with("MATCH") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    let opp: u64 = parts[1].parse().unwrap();

                    println!("🔥 MATCH FOUND: {} vs {}", id, opp);

                    // simple pause before accepting match
                    sleep(Duration::from_millis(1500)).await;

                    // random win/lose
                    let win = rand::random::<bool>() as u8;

                    println!(
                        "➡ Result: You {}",
                        if win == 1 { "WIN" } else { "LOSE" }
                    );

                    writer
                        .write_all(format!("RESULT {} {} {}\n", id, opp, win).as_bytes())
                        .await
                        .unwrap();

                    if let Ok(Some(res_line)) = lines.next_line().await {
                        println!("🏆 {}", res_line);
                    }

                    // wait before re-queue
                    sleep(Duration::from_secs(2)).await;

                    break; // go queue again
                }
            }
        }
    }
}
