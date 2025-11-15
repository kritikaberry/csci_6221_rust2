use macroquad::prelude::*;
use redis::AsyncCommands;
use tokio::runtime::Runtime;

// -------- ESPORTS COLOR PALETTE --------
const BG: Color      = Color { r: 0.08, g: 0.08, b: 0.10, a: 1.0 };
const PANEL: Color   = Color { r: 0.14, g: 0.14, b: 0.16, a: 1.0 };
const BORDER: Color  = Color { r: 0.85, g: 0.10, b: 0.16, a: 1.0 }; // esports red
const TEXT: Color    = Color { r: 0.82, g: 0.84, b: 0.88, a: 1.0 };
const MUTED: Color   = Color { r: 0.55, g: 0.57, b: 0.62, a: 1.0 };
const SUCCESS: Color = Color { r: 0.20, g: 0.90, b: 0.40, a: 1.0 };

#[derive(Clone)]
enum Screen {
    Login,
    Home,
    Queue,
    MatchFound,
    MatchResult,
}

struct Player {
    id: u64,
    rating: i32,
    status: String,
    game_id: String,
}

#[macroquad::main("ESPORTS PLAYER CLIENT")]
async fn main() {
    // runtime + redis
    let rt = Runtime::new().unwrap();
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();

    // UI state
    let mut screen = Screen::Login;
    let mut player_id = "".to_string();
    let mut my_player = None::<Player>;
    let mut match_opponent = None::<(u64, String)>; 

    loop {
        clear_background(BG);

        match screen {
            Screen::Login => {
                draw_login_screen(&player_id);

                // Typing ID
                if let Some(ch) = get_char_pressed() {
                    if ch.is_numeric() && player_id.len() < 4 {
                        player_id.push(ch);
                    }
                }
                if is_key_pressed(KeyCode::Backspace) {
                    player_id.pop();
                }
                if is_key_pressed(KeyCode::Enter) && !player_id.is_empty() {
                    let id = player_id.parse::<u64>().unwrap();

                    // Register player in Redis
                    rt.block_on(register_player(&client, id));
                    my_player = rt.block_on(fetch_player(&client, id));

                    screen = Screen::Home;
                }
            }

            Screen::Home => {
                draw_home(my_player.as_ref().unwrap());

                if is_key_pressed(KeyCode::Enter) {
                    // Queue player
                    let p = my_player.as_ref().unwrap();
                    rt.block_on(queue_player(&client, p.id, &p.game_id));
                    screen = Screen::Queue;
                }
            }

            Screen::Queue => {
                draw_queue_screen();

                // Check if match created for this player
                let p = my_player.as_ref().unwrap();
                if let Some((opp, game)) = rt.block_on(check_match(&client, p.id)) {
                    match_opponent = Some((opp, game));
                    screen = Screen::MatchFound;
                }

                if is_key_pressed(KeyCode::Escape) {
                    screen = Screen::Home;
                }
            }

            Screen::MatchFound => {
                let (opp_id, game) = match_opponent.clone().unwrap();
                draw_match_found(opp_id, &game);

                if is_key_pressed(KeyCode::Enter) {
                    // simulate result
                    let win = rand::gen_range(0, 2) == 1;
                    rt.block_on(update_match_results(&client, 
                        my_player.as_ref().unwrap().id,
                        opp_id,
                        win,
                    ));
                    my_player = rt.block_on(fetch_player(&client, my_player.as_ref().unwrap().id));
                    screen = Screen::MatchResult;
                }
            }

            Screen::MatchResult => {
                draw_match_result(my_player.as_ref().unwrap());

                if is_key_pressed(KeyCode::Enter) {
                    screen = Screen::Home;
                }
            }
        }

        next_frame().await;
    }
}

// -------------------------------------------------
// REDIS FUNCTIONS
// -------------------------------------------------
async fn register_player(client: &redis::Client, id: u64) {
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let rating = rand::gen_range(1000, 2000);

    let _: () = con.hset_multiple(
        &format!("player:{}", id),
        &[
            ("rating", rating.to_string().as_str()),
            ("status", "idle"),
            ("game_id", "demo_game"),
        ],
    ).await.unwrap();
}

async fn fetch_player(client: &redis::Client, id: u64) -> Option<Player> {
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let key = format!("player:{}", id);

    let exists: bool = redis::cmd("EXISTS").arg(&key).query_async(&mut con).await.unwrap();
    if !exists {
        return None;
    }

    let rating: i32 = con.hget(&key, "rating").await.unwrap();
    let status: String = con.hget(&key, "status").await.unwrap();
    let game: String = con.hget(&key, "game_id").await.unwrap();

    Some(Player { id, rating, status, game_id: game })
}

async fn queue_player(client: &redis::Client, id: u64, game: &str) {
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let _: () = con.hset(format!("player:{}", id), "status", "queued").await.unwrap();
    let _: () = con.lpush(format!("queue:{}", game), id.to_string()).await.unwrap();
}

async fn check_match(client: &redis::Client, my_id: u64)
-> Option<(u64, String)> {
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let m_keys: Vec<String> = con.keys("match:*").await.unwrap_or(vec![]);
    for m in &m_keys {
        let a: u64 = con.hget(m, "a").await.unwrap_or(0);
        let b: u64 = con.hget(m, "b").await.unwrap_or(0);

        if a == my_id {
            let game: String = con.hget(m, "game_id").await.unwrap_or("demo".into());
            return Some((b, game));
        }
        if b == my_id {
            let game: String = con.hget(m, "game_id").await.unwrap_or("demo".into());
            return Some((a, game));
        }
    }
    None
}

async fn update_match_results(client: &redis::Client, my_id: u64, opp: u64, win: bool) {
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let my_key = format!("player:{}", my_id);
    let opp_key = format!("player:{}", opp);

    let my_rating: i32 = con.hget(&my_key, "rating").await.unwrap_or(1200);
    let opp_rating: i32 = con.hget(&opp_key, "rating").await.unwrap_or(1200);

    let k = 32;

    let expected_my = 1.0 / (1.0 + 10_f64.powf((opp_rating - my_rating) as f64 / 400.0));
    let expected_opp = 1.0 - expected_my;

    let score_my = if win { 1.0 } else { 0.0 };
    let score_opp = 1.0 - score_my;

    let new_my = my_rating + (k as f64 * (score_my - expected_my)) as i32;
    let new_opp = opp_rating + (k as f64 * (score_opp - expected_opp)) as i32;

    let _: () = con.hset(&my_key,  "rating", new_my).await.unwrap();
    let _: () = con.hset(&opp_key, "rating", new_opp).await.unwrap();
}


// -------------------------------------------------
// UI DRAWING FUNCTIONS (ESPORTS STYLE)
// -------------------------------------------------
fn draw_login_screen(id: &str) {
    draw_text("PLAYER LOGIN", 100.0, 120.0, 48.0, BORDER);
    draw_panel(80.0, 140.0, 500.0, 200.0);

    draw_text("ENTER PLAYER ID:", 120.0, 200.0, 30.0, TEXT);
    draw_text(id, 120.0, 250.0, 48.0, SUCCESS);

    draw_text("Press ENTER to continue", 120.0, 330.0, 26.0, MUTED);
}

fn draw_home(p: &Player) {
    draw_text("PLAYER HOME", 80.0, 100.0, 42.0, BORDER);
    draw_panel(60.0, 120.0, 560.0, 260.0);

    draw_text(&format!("PLAYER ID:  {}", p.id), 90.0, 180.0, 28.0, TEXT);
    draw_text(&format!("RATING:     {}", p.rating), 90.0, 220.0, 28.0, SUCCESS);
    draw_text(&format!("GAME:       {}", p.game_id), 90.0, 260.0, 28.0, TEXT);
    draw_text(&format!("STATUS:     {}", p.status), 90.0, 300.0, 28.0, TEXT);

    draw_text("[ENTER] QUEUE FOR MATCH", 90.0, 380.0, 32.0, BORDER);
}

fn draw_queue_screen() {
    draw_text("SEARCHING FOR MATCH…", 90.0, 120.0, 40.0, BORDER);
    draw_panel(60.0, 140.0, 560.0, 200.0);

    draw_text("QUEUE STATUS: ACTIVE", 100.0, 200.0, 28.0, SUCCESS);
    draw_text("Press ESC to cancel", 100.0, 260.0, 26.0, MUTED);
}

fn draw_match_found(opp: u64, game: &str) {
    draw_text("MATCH FOUND", 90.0, 120.0, 48.0, BORDER);
    draw_panel(60.0, 140.0, 600.0, 240.0);

    draw_text(&format!("OPPONENT: P{}", opp), 100.0, 220.0, 32.0, TEXT);
    draw_text(&format!("GAME: {}", game), 100.0, 270.0, 32.0, TEXT);

    draw_text("[ENTER] ACCEPT MATCH", 100.0, 340.0, 30.0, SUCCESS);
}

fn draw_match_result(p: &Player) {
    draw_text("MATCH COMPLETE", 80.0, 120.0, 46.0, BORDER);
    draw_panel(60.0, 150.0, 600.0, 240.0);

    draw_text(&format!("NEW RATING: {}", p.rating), 100.0, 240.0, 32.0, SUCCESS);
    draw_text("[ENTER] CONTINUE", 100.0, 330.0, 30.0, MUTED);
}

fn draw_panel(x: f32, y: f32, w: f32, h: f32) {
    draw_rectangle(x, y, w, h, PANEL);
    draw_rectangle_lines(x, y, w, h, 2.0, BORDER);
}
