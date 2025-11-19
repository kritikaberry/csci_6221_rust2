// bots.rs — SLOW + REALISTIC TCP BOT CLIENT
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

    println!("🤖 Launching {bots} slow TCP bots...");

    for i in 1..=bots {
        tokio::spawn(async move { run_bot(i as u64).await });
    }

    loop { sleep(Duration::from_secs(60)).await; }
}

async fn run_bot(id: u64) {
    let mut rng = StdRng::from_entropy();

    // connect
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

    loop {
        // 🟡 slow natural delay before bots queue again (5–12 seconds)
        let wait = rng.gen_range(5000..12000);
        sleep(Duration::from_millis(wait)).await;

        // QUEUE
        writer.write_all(format!("QUEUE {}\n", id).as_bytes()).await.unwrap();
        println!("📥 BOT {id} queued (waited {wait}ms)");

        // 🟦 POLLING LOOP
        loop {
            // realistic human polling speed (1.5s – 3s)
            let poll_delay = rng.gen_range(1500..3000);
            sleep(Duration::from_millis(poll_delay)).await;

            writer.write_all(format!("POLL {}\n", id).as_bytes()).await.unwrap();

            if let Ok(Some(line)) = lines.next_line().await {
                if line.starts_with("MATCH") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    let opp: u64 = parts[1].parse().unwrap();

                    println!("🎯 BOT {id} matched vs BOT {opp}");

                    // simulate “thinking” before accepting (1–2.5 sec)
                    let think_time = rng.gen_range(1000..2500);
                    sleep(Duration::from_millis(think_time)).await;

                    // random win/lose
                    let win = rng.gen_bool(0.5) as u8;

                    writer.write_all(format!("RESULT {} {} {}\n", id, opp, win).as_bytes())
                        .await
                        .unwrap();

                    if let Ok(Some(res)) = lines.next_line().await {
                        println!("🏆 BOT {id} result: {res}");
                    }

                    // after match, bot stays idle 3–6 seconds before queuing again
                    let idle = rng.gen_range(3000..6000);
                    sleep(Duration::from_millis(idle)).await;

                    break; // go back to queue cycle
                }
            }
        }
    }
}
