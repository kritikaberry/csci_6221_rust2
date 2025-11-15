use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use redis::AsyncCommands;
use tokio::time::{sleep, Duration};

#[derive(Clone)]
struct Personality {
    queue_delay_min: u64,
    queue_delay_max: u64,
    accept_delay_min: u64,
    accept_delay_max: u64,
    idle_after_match_min: u64,
    idle_after_match_max: u64,
    break_probability: f64,
    rage_quit_probability: f64,
    afk_probability: f64,
}

const PERSONAS: &[Personality] = &[
    // Competitive, fast-reacting
    Personality {
        queue_delay_min: 200,
        queue_delay_max: 800,
        accept_delay_min: 200,
        accept_delay_max: 600,
        idle_after_match_min: 500,
        idle_after_match_max: 2000,
        break_probability: 0.05,
        rage_quit_probability: 0.01,
        afk_probability: 0.02,
    },
    // Casual
    Personality {
        queue_delay_min: 1500,
        queue_delay_max: 4000,
        accept_delay_min: 800,
        accept_delay_max: 1500,
        idle_after_match_min: 2000,
        idle_after_match_max: 5000,
        break_probability: 0.12,
        rage_quit_probability: 0.03,
        afk_probability: 0.05,
    },
    // AFK-prone or distracted
    Personality {
        queue_delay_min: 2000,
        queue_delay_max: 5000,
        accept_delay_min: 3000,
        accept_delay_max: 6000,
        idle_after_match_min: 5000,
        idle_after_match_max: 9000,
        break_probability: 0.20,
        rage_quit_probability: 0.05,
        afk_probability: 0.25,
    },
    // Hyper-aggressive grinder
    Personality {
        queue_delay_min: 50,
        queue_delay_max: 200,
        accept_delay_min: 100,
        accept_delay_max: 200,
        idle_after_match_min: 100,
        idle_after_match_max: 500,
        break_probability: 0.03,
        rage_quit_probability: 0.02,
        afk_probability: 0.01,
    },
];

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let num_bots: usize = args.get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    println!("Launching {} natural behavior bots...", num_bots);

    for i in 1..=num_bots {
        let persona = PERSONAS[rand::thread_rng().gen_range(0..PERSONAS.len())].clone();
        tokio::spawn(async move { run_human_bot(i as u64, persona).await });
    }

    loop {
        sleep(Duration::from_secs(60)).await;
    }
}

async fn run_human_bot(id: u64, persona: Personality) {
    let mut rng = StdRng::from_entropy();
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let rating = gaussian_rating(&mut rng);

    let player_key = format!("player:{}", id);
    let _ : () = con.hset(&player_key, "rating", rating).await.unwrap();
    let _ : () = con.hset(&player_key, "status", "idle").await.unwrap();
    let _ : () = con.hset(&player_key, "game_id", "demo_game").await.unwrap();

    println!("HUMAN BOT {} JOINED (rating {})", id, rating);

    loop {

        // Players randomly need a break
        if rng.gen_bool(persona.break_probability) {
            let break_time = rng.gen_range(5..15);
            println!("BOT {} taking a break ({}s)...", id, break_time);
            sleep(Duration::from_secs(break_time)).await;
        }

        // Rage quit
    if rng.gen_bool(persona.rage_quit_probability) {
            println!("BOT {} rage quit!", id);
            let _: () = con.hset(format!("player:{}", id), "status", "offline").await.unwrap();
            sleep(Duration::from_secs(rng.gen_range(10..30))).await;
        }

        // Queue delay (human reaction)
    sleep(Duration::from_millis(rng.gen_range(persona.queue_delay_min..persona.queue_delay_max))).await;

        // GO QUEUE
        let _: () = con.hset(format!("player:{}", id), "status", "queued").await.unwrap();
        let _: () = con.lpush("queue:demo_game", id.to_string()).await.unwrap();

        // Wait for match
        loop {
            if let Some((opp, match_id)) = check_match(&mut con, id).await {
                
                // AFK scenario
                if rng.gen_bool(persona.afk_probability) {
                    println!("BOT {} is AFK! Match will timeout...", id);
                    sleep(Duration::from_secs(5)).await;
                    let _: () = con.del(match_id.clone()).await.unwrap();
                    break;
                }

                // Human-like accept delay
                sleep(Duration::from_millis(rng.gen_range(persona.accept_delay_min..persona.accept_delay_max))).await;

                // Apply match result
                let win = rng.gen_bool(0.5);
                apply_result(&mut con, id, opp, win).await;

                // Cleanup match
                let _: () = con.del(match_id.clone()).await.unwrap();

                // Idle cooldown
                sleep(Duration::from_millis(rng.gen_range(persona.idle_after_match_min..persona.idle_after_match_max))).await;

                break;
            }

            sleep(Duration::from_millis(200)).await;
        }
    }
}

async fn check_match(con: &mut redis::aio::MultiplexedConnection, id: u64)
    -> Option<(u64, String)>
{
    let keys: Vec<String> = con.keys("match:*").await.unwrap_or(vec![]);

    for k in keys {
        let a: u64 = con.hget(&k, "a").await.unwrap_or(0);
    let b: u64 = con.hget(&k, "b").await.unwrap_or(0);

        if a == id { return Some((b, k)); }
        if b == id { return Some((a, k)); }
    }
    None
}

async fn apply_result(
    con: &mut redis::aio::MultiplexedConnection,
    id: u64,
    opp: u64,
    win: bool
) {
    let key = format!("player:{}", id);
    let opp_key = format!("player:{}", opp);

    let my_rating: i32 = con.hget(&key, "rating").await.unwrap_or(1200);
    let opp_rating: i32 = con.hget(&opp_key, "rating").await.unwrap_or(1200);

    let k = 32;
    let expected = 1.0 / (1.0 + 10_f64.powf((opp_rating - my_rating) as f64 / 400.0));
    let score = if win { 1.0 } else { 0.0 };

    let new_rating = my_rating + (k as f64 * (score - expected)) as i32;

    let _: () = con.hset(&key, "rating", new_rating).await.unwrap();
    let _: () = con.hset(&key, "status", "idle").await.unwrap();
}

fn gaussian_rating(rng: &mut StdRng) -> i32 {
    let mean = 1200.0;
    let stddev = 200.0;

    let rand1: f64 = rng.gen();
    let rand2: f64 = rng.gen();
    let z = (-2.0 * rand1.ln()).sqrt() * (2.0 * std::f64::consts::PI * rand2).cos();

    (mean + z * stddev).round() as i32
}
