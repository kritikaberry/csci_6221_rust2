use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::{sleep, Duration},
};

#[tokio::main]
async fn main() {
    println!("🎮 Terminal Player Client Started");

    // Enter Player ID
    print!("Enter Player ID: ");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let id: u64 = match input.trim().parse() {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Invalid ID.");
            return;
        }
    };

    // Connect to matchmaking TCP server
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
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[0] == "REGISTERED" {
            println!("➡ Registered with rating {}", parts[1]);
        } else {
            println!("➡ {}", line);
        }
    }

    // Queue for match
    println!("➡ Press Enter to queue…");
    let mut tmp = String::new();
    let _ = std::io::stdin().read_line(&mut tmp);

    writer
        .write_all(format!("QUEUE {}\n", id).as_bytes())
        .await
        .unwrap();
    println!("➡ You are queued.");

    // Auto-poll loop
    println!("➡ Searching for match…");

    loop {
        // add small jitter to avoid clients polling at exact same time
        let jitter_ms = 700 + (rand::random::<u16>() % 400) as u64;
        sleep(Duration::from_millis(jitter_ms)).await;

        writer
            .write_all(format!("POLL {}\n", id).as_bytes())
            .await
            .unwrap();

    if let Ok(Some(line)) = lines.next_line().await {
            if line.starts_with("MATCH") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let opp: u64 = parts[1].parse().unwrap();

                println!("\n🔥 MATCH FOUND! Opponent: {}", opp);

                // Simulate accepting match
                sleep(Duration::from_secs(2)).await;

                // For now randomly win/lose
                let win = rand::random::<bool>() as u8;
                println!(
                    "➡ Sending result… You {}",
                    if win == 1 { "WIN" } else { "LOSE" }
                );

                writer
                    .write_all(format!("RESULT {} {} {}\n", id, opp, win).as_bytes())
                    .await
                    .unwrap();

                if let Ok(Some(res_line)) = lines.next_line().await {
                    println!("➡ {}", res_line); // RESULT_OK new_rating
                }

                println!("🏁 Match complete. Exiting.");
                break;
            } else if line.starts_with("NO_MATCH") {
                // keep searching
            } else if line.starts_with("ERR") {
                eprintln!("Server error.");
                break;
            }
        }

        print!(".");
        let _ = std::io::Write::flush(&mut std::io::stdout());
    }
}
