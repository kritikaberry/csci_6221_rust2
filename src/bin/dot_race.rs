use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::thread::sleep;
use std::time::{Duration, Instant};

use macroquad::prelude::*;
use redis::Commands;

// linear interpolate between two Colors
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

#[macroquad::main("Dot Race — 2 Player")]
async fn main() {
    println!("Enter Player ID:");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let id: u64 = input.trim().parse().unwrap();

    loop {
        println!("🎯 Starting matchmaking...");
        if let Err(e) = matchmaking_and_game(id).await {
            println!("❌ ERROR: {e}");
        }

        println!("🔁 Press Enter to queue again...");
        let mut t = String::new();
        let _ = std::io::stdin().read_line(&mut t);
    }
}

async fn matchmaking_and_game(id: u64) -> Result<(), String> {
    let mut stream = TcpStream::connect("127.0.0.1:7500").map_err(|e| format!("connect: {e}"))?;
    stream.set_nonblocking(false).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    stream.write_all(format!("REGISTER {}\n", id).as_bytes()).map_err(|e| format!("write: {e}"))?;
    {
        let mut line = String::new();
        reader.read_line(&mut line).ok();
        if !line.trim().is_empty() { println!("➡ {}", line.trim()); }
    }

    stream.write_all(format!("QUEUE {}\n", id).as_bytes()).map_err(|e| format!("queue: {e}"))?;
    println!("➡ Queued");

    println!("➡ Waiting for match...");
    let opponent_id = loop {
        sleep(Duration::from_millis(700));
        stream.write_all(format!("POLL {}\n", id).as_bytes()).ok();
        let mut line = String::new();
        reader.read_line(&mut line).ok();
        let msg = line.trim();
        if msg.starts_with("MATCH") {
            if let Some(opp) = msg.split_whitespace().nth(1) {
                break opp.parse::<u64>().map_err(|e| format!("parse: {e}"))?;
            }
        }
        print!("."); std::io::stdout().flush().ok();
    };

    let host = id.min(opponent_id);
    let is_host = id == host;
    let winner = run_network_race(id, opponent_id, is_host).await;

    if is_host {
        let win_flag = if winner == id { 1 } else { 0 };
        stream.write_all(format!("RESULT {} {} {}\n", id, opponent_id, win_flag).as_bytes()).ok();
        let mut line = String::new(); reader.read_line(&mut line).ok(); if !line.trim().is_empty() { println!("➡ {}", line.trim()); }
        if let Ok(client) = redis::Client::open("redis://127.0.0.1/") {
            if let Ok(mut rc) = client.get_connection() {
                let r_self: i32 = rc.hget(format!("player:{}", id), "rating").unwrap_or(0);
                let r_opp: i32 = rc.hget(format!("player:{}", opponent_id), "rating").unwrap_or(0);
                println!("⭐ Updated Ratings → P{}: {} | P{}: {}", id, r_self, opponent_id, r_opp);
            }
        }
        println!("🏁 Game Complete. Winner: {}", winner);
    } else {
        if let Ok(client) = redis::Client::open("redis://127.0.0.1/") {
            if let Ok(mut rc) = client.get_connection() {
                let r_self: i32 = rc.hget(format!("player:{}", id), "rating").unwrap_or(0);
                let r_opp: i32 = rc.hget(format!("player:{}", opponent_id), "rating").unwrap_or(0);
                println!("🏁 Match Finished. Winner: {}", winner);
                println!("(Await rating update from host...) Current Ratings: P{}={} P{}={}", id, r_self, opponent_id, r_opp);
            }
        }
    }

    Ok(())
}

async fn run_network_race(my_id: u64, opponent: u64, is_host: bool) -> u64 {
    let mut x1: f32 = 80.0; let mut y1: f32 = screen_height() / 2.0 - 60.0;
    let mut x2: f32 = 80.0; let mut y2: f32 = screen_height() / 2.0 + 60.0;
    let finish_x = screen_width() - 120.0;

    let mut winner: Option<u64> = None;
    let mut trail1: Vec<(f32,f32)> = Vec::new();
    let mut trail2: Vec<(f32,f32)> = Vec::new();

    let mut countdown: f32 = 3.5; let mut started_at: Option<f32> = None;

    let id_a = my_id.min(opponent); let id_b = my_id.max(opponent);
    let key_p1 = format!("race:{}:{}:p1", id_a, id_b);
    let key_p2 = format!("race:{}:{}:p2", id_a, id_b);
    let key_win = format!("race:{}:{}:winner", id_a, id_b);
    let key_input_p1 = format!("race:{}:{}:input:{}", id_a, id_b, id_a);
    let key_input_p2 = format!("race:{}:{}:input:{}", id_a, id_b, id_b);

    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut conn = client.get_connection().unwrap();

    let mut prev_raw_p1: Option<String> = None;
    let mut prev_raw_p2: Option<String> = None;
    let mut last_host_update: Instant = Instant::now();

    loop {
        for i in 0..8 {
            let t = i as f32 / 8.0;
            let c = lerp_color(Color::from_rgba(14,18,22,255), Color::from_rgba(20,24,30,255), t);
            draw_rectangle(0.0, i as f32 * (screen_height()/8.0), screen_width(), screen_height()/8.0 + 1.0, c);
        }

        draw_text(&format!("P{} (YELLOW)  ✦  P{} (RED)", my_id, opponent), 24.0, 48.0, 28.0, Color::new(0.96,0.94,0.9,1.0));

        let is_p1 = my_id == id_a;
        let start_x = 80.0_f32;
        let now = get_time() as f32;
        let elapsed = started_at.map(|s| now - s).unwrap_or(0.0);

        let pct1 = ((x1 - start_x) / (finish_x - start_x)).clamp(0.0, 1.0);
        let pct2 = ((x2 - start_x) / (finish_x - start_x)).clamp(0.0, 1.0);

        let bar_w = screen_width() - 240.0;
        draw_rectangle(120.0, 70.0, bar_w, 8.0, Color::new(0.12,0.14,0.16,1.0));
        draw_rectangle(120.0, 86.0, bar_w, 8.0, Color::new(0.12,0.14,0.16,1.0));
        draw_rectangle(120.0, 70.0, bar_w * pct1, 8.0, Color::new(0.94,0.78,0.16,1.0));
        draw_rectangle(120.0, 86.0, bar_w * pct2, 8.0, Color::new(0.80,0.22,0.22,1.0));
        draw_text(&format!("P{}", id_a), 20.0, 76.0, 16.0, Color::new(0.88,0.88,0.88,1.0));
        draw_text(&format!("P{}", id_b), 20.0, 92.0, 16.0, Color::new(0.88,0.88,0.88,1.0));
        draw_text(&format!("Time: {:.1}s", elapsed), screen_width()-200.0, 84.0, 18.0, Color::new(0.86,0.86,0.86,1.0));

        if is_host {
            if let Ok(raw) = conn.get::<_, String>(&key_input_p1) {
                let parts: Vec<_> = raw.split(',').collect(); if parts.len() == 2 { x1 = parts[0].parse().unwrap_or(x1); y1 = parts[1].parse().unwrap_or(y1); }
            } else if my_id == id_a {
                if is_key_down(KeyCode::W) { y1 -= 4.0; } if is_key_down(KeyCode::S) { y1 += 4.0; } if is_key_down(KeyCode::A) { x1 -= 4.0; } if is_key_down(KeyCode::D) { x1 += 4.0; }
            }

            if let Ok(raw) = conn.get::<_, String>(&key_input_p2) {
                let parts: Vec<_> = raw.split(',').collect(); if parts.len() == 2 { x2 = parts[0].parse().unwrap_or(x2); y2 = parts[1].parse().unwrap_or(y2); }
            } else if my_id == id_b {
                if is_key_down(KeyCode::Up) { y2 -= 4.0; } if is_key_down(KeyCode::Down) { y2 += 4.0; } if is_key_down(KeyCode::Left) { x2 -= 4.0; } if is_key_down(KeyCode::Right) { x2 += 4.0; }
            }

            let raw1 = format!("{},{},{}", id_a, x1, y1);
            let raw2 = format!("{},{},{}", id_b, x2, y2);
            let _ : redis::RedisResult<()> = conn.set(&key_p1, &raw1);
            let _ : redis::RedisResult<()> = conn.set(&key_p2, &raw2);

            if prev_raw_p1.as_deref() != Some(&raw1) || prev_raw_p2.as_deref() != Some(&raw2) {
                prev_raw_p1 = Some(raw1);
                prev_raw_p2 = Some(raw2);
                last_host_update = Instant::now();
            }
        } else {
            if is_p1 {
                if is_key_down(KeyCode::W) { y1 -= 4.0; } if is_key_down(KeyCode::S) { y1 += 4.0; } if is_key_down(KeyCode::A) { x1 -= 4.0; } if is_key_down(KeyCode::D) { x1 += 4.0; }
                let _ : redis::RedisResult<()> = conn.set(&key_input_p1, format!("{},{}", x1, y1));
            } else {
                if is_key_down(KeyCode::Up) { y2 -= 4.0; } if is_key_down(KeyCode::Down) { y2 += 4.0; } if is_key_down(KeyCode::Left) { x2 -= 4.0; } if is_key_down(KeyCode::Right) { x2 += 4.0; }
                let _ : redis::RedisResult<()> = conn.set(&key_input_p2, format!("{},{}", x2, y2));
            }

            let mut host_updated = false;
            if let Ok(raw1) = conn.get::<_, String>(&key_p1) {
                if prev_raw_p1.as_deref() != Some(&raw1) { prev_raw_p1 = Some(raw1.clone()); last_host_update = Instant::now(); host_updated = true; }
                let parts: Vec<_> = raw1.split(',').collect(); if parts.len() == 3 { x1 = parts[1].parse().unwrap_or(x1); y1 = parts[2].parse().unwrap_or(y1); }
            }
            if let Ok(raw2) = conn.get::<_, String>(&key_p2) {
                if prev_raw_p2.as_deref() != Some(&raw2) { prev_raw_p2 = Some(raw2.clone()); last_host_update = Instant::now(); host_updated = true; }
                let parts: Vec<_> = raw2.split(',').collect(); if parts.len() == 3 { x2 = parts[1].parse().unwrap_or(x2); y2 = parts[2].parse().unwrap_or(y2); }
            }

            if !host_updated && last_host_update.elapsed().as_secs() > 3 {
                if is_p1 { x2 += 2.5; } else { x1 += 2.5; }
            }
        }

        x1 = x1.clamp(20.0, screen_width() - 20.0);
        x2 = x2.clamp(20.0, screen_width() - 20.0);
        y1 = y1.clamp(20.0, screen_height() - 20.0);
        y2 = y2.clamp(20.0, screen_height() - 20.0);

        trail1.push((x1,y1)); if trail1.len() > 10 { trail1.remove(0); }
        trail2.push((x2,y2)); if trail2.len() > 10 { trail2.remove(0); }

        for (i, (tx,ty)) in trail1.iter().enumerate() { let a = (i as f32 / trail1.len().max(1) as f32) * 0.6; draw_circle(*tx, *ty, 16.0 - i as f32, Color::new(0.94,0.78,0.16,a)); }
        for (i, (tx,ty)) in trail2.iter().enumerate() { let a = (i as f32 / trail2.len().max(1) as f32) * 0.6; draw_circle(*tx, *ty, 16.0 - i as f32, Color::new(0.80,0.22,0.22,a)); }

        draw_circle(x1, y1, 22.0, Color::new(0.94,0.78,0.16,0.12));
        draw_circle(x2, y2, 22.0, Color::new(0.80,0.22,0.22,0.12));
        draw_circle(x1, y1, 18.0, YELLOW); draw_circle(x2, y2, 18.0, RED);
        draw_text(&format!("P{}", id_a), x1 - 14.0, y1 + 6.0, 20.0, BLACK);
        draw_text(&format!("P{}", id_b), x2 - 14.0, y2 + 6.0, 20.0, BLACK);

        for i in 0..6 { let alpha = 0.08 * (6 - i) as f32; draw_line(finish_x + i as f32, 0.0, finish_x + i as f32, screen_height(), 4.0, Color::new(0.1,0.9,0.3,alpha)); draw_line(finish_x - i as f32, 0.0, finish_x - i as f32, screen_height(), 4.0, Color::new(0.1,0.9,0.3,alpha)); }

        if countdown > 0.0 {
            let n = (countdown).ceil() as i32;
            draw_text(&format!("{}", if n > 0 { n.to_string() } else { "GO".to_string() }), screen_width()/2.0 - 26.0, screen_height()/2.0, 92.0, Color::new(0.98,0.96,0.9,1.0));
            let dt = get_frame_time(); countdown -= dt; if countdown <= 0.0 { started_at = Some(get_time() as f32); }
        } else {
            if is_host {
                if winner.is_none() {
                    if x1 >= finish_x { winner = Some(id_a); }
                    if x2 >= finish_x { winner = Some(id_b); }
                    if let Some(w) = winner { let _ : redis::RedisResult<()> = conn.set(&key_win, w); }
                }
            } else {
                if winner.is_none() {
                    if let Ok(w) = conn.get::<_, u64>(&key_win) { winner = Some(w); }
                }
            }
        }

        if let Some(w) = winner {
            let msg = if w == my_id { "YOU WIN!" } else { "OPPONENT WINS!" };
            for _ in 0..120 { clear_background(BLACK); draw_text(msg, screen_width() / 2.0 - 150.0, screen_height() / 2.0, 60.0, WHITE); next_frame().await; }
            if is_host { let _ : redis::RedisResult<()> = conn.del(&key_p1); let _ : redis::RedisResult<()> = conn.del(&key_p2); let _ : redis::RedisResult<()> = conn.del(&key_win); return w; } else { return w; }
        }

        next_frame().await;
    }
}
