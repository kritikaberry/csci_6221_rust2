// bots.rs — HIGHLY REALISTIC SLOW TCP BOT CLIENT
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::{sleep, Duration},
};
use rand::{Rng, rngs::StdRng, SeedableRng};

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
    let mut stream = TcpStream::connect("127.0.0.1:7500")
        .await
        .expect("BOT cannot connect to matchmaking server");

    let (reader, mut writer) = stream.split();
    let mut lines = BufReader::new(reader).lines();

    // REGISTER
    writer.write_all(format!("REGISTER {}\n", id).as_bytes()).await.unwrap();
    if let Ok(Some(line)) = lines.next_line().await {
        println!("🤖 BOT {id}: {line}");
    }

    // --- MAIN LOOP ---
    let mut matches_played = 0;

    loop {
        // 🟡 bots wait realistically before queueing
        let wait = rng.gen_range(queue_min..queue_max);
        sleep(Duration::from_millis(wait)).await;

        writer.write_all(format!("QUEUE {}\n", id).as_bytes()).await.unwrap();
        println!("📥 BOT {id} queued (waited {}ms)", wait);

        // --- POLL LOOP ---
        loop {
            // realistic polling timing
            let poll_delay = rng.gen_range(1200..2500);
            sleep(Duration::from_millis(poll_delay)).await;

            writer.write_all(format!("POLL {}\n", id).as_bytes()).await.unwrap();

            if let Ok(Some(line)) = lines.next_line().await {
                if line.starts_with("MATCH") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    let opp: u64 = parts[1].parse().unwrap();

                    println!("🎯 BOT {id} matched vs BOT {}", opp);

                    // simulate reaction time
                    let reaction = rng.gen_range(800..2000);
                    sleep(Duration::from_millis(reaction)).await;

                    // random win/lose
                    let win = rng.gen_bool(0.5) as u8;

                    writer
                        .write_all(format!("RESULT {} {} {}\n", id, opp, win).as_bytes())
                        .await
                        .unwrap();

                    if let Ok(Some(res)) = lines.next_line().await {
                        println!("🏆 BOT {id}: {res}");
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
        }
    }
}
