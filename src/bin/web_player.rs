use axum::{
    routing::{get, post},
    extract::{Path, Form},
    Router,
    response::Html,
    Json,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(dashboard_home))
        .route("/player/:id", get(player_page))
        .route("/queue", post(queue_player))
        .route("/match_status/:id", get(match_status));

    println!("Web Dashboard running on http://localhost:8080/");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ==============================
// PAGE WRAPPER
// ==============================

fn page(title: &str, body: String) -> Html<String> {
    Html(format!(
        r#"
        <html>
        <head>
            <title>{}</title>
            <meta http-equiv="refresh" content="1">
            <style>
                body {{
                    font-family: Arial, sans-serif;
                    background: #101014;
                    color: #e5e5e5;
                    margin: 0;
                    padding: 0;
                }}

                .container {{
                    display: flex;
                    height: 100vh;
                }}

                /* LEFT SIDEBAR */
                .left {{
                    width: 30%;
                    background: #16161c;
                    padding: 20px;
                    overflow-y: auto;
                    border-right: 2px solid #222;
                }}

                /* RIGHT CONTENT */
                .right {{
                    width: 70%;
                    padding: 20px;
                    overflow-y: auto;
                }}

                /* PANELS */
                .panel {{
                    background: #1e1e26;
                    padding: 15px;
                    border-radius: 10px;
                    margin-bottom: 20px;
                    border: 1px solid #2a2a34;
                }}

                h1, h2 {{
                    color: #ff4d4d;
                }}

                a {{
                    color: #7dbcff;
                    text-decoration: none;
                }}

                a:hover {{
                    text-decoration: underline;
                }}

                .scroll {{
                    max-height: 70vh;
                    overflow-y: auto;
                }}

                /* Rating Colors */
                .rating-low {{ color: #a0a0a0; }}
                .rating-mid {{ color: #7abaff; }}
                .rating-good {{ color: #48d97e; }}
                .rating-high {{ color: #ffd54f; }}
                .rating-top {{ color: #ff6961; }}
            </style>
        </head>
        <body>
            {}
        </body>
        </html>
        "#,
        title,
        body
    ))
}

// ==============================
// REDIS HELPERS
// ==============================

async fn get_players(con: &mut redis::aio::MultiplexedConnection) -> Vec<(String, i32)> {
    let keys: Vec<String> = con.keys("player:*").await.unwrap_or_default();
    let mut list = vec![];

    for k in keys {
        let r: i32 = con.hget(&k, "rating").await.unwrap_or(0);
        list.push((k, r));
    }

    list.sort_by_key(|(_, r)| -(*r));
    list
}

async fn get_queue(con: &mut redis::aio::MultiplexedConnection) -> Vec<String> {
    con.lrange("queue:demo_game", 0, -1).await.unwrap_or_default()
}

async fn get_matches(con: &mut redis::aio::MultiplexedConnection) -> Vec<(String, u64, u64)> {
    let keys: Vec<String> = con.keys("match:*").await.unwrap_or_default();
    let mut list = vec![];

    for k in keys {
        let a: u64 = con.hget(&k, "a").await.unwrap_or(0);
        let b: u64 = con.hget(&k, "b").await.unwrap_or(0);
        list.push((k, a, b));
    }

    list
}

// ==============================
// QUEUE PLAYER
// ==============================

#[derive(Deserialize)]
struct QueueForm { player_id: u64 }

async fn queue_player(Form(f): Form<QueueForm>) -> Html<String> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let _: () = con.rpush("queue:demo_game", f.player_id).await.unwrap();

    Html("Queued".into())
}

// ==============================
// MATCH STATUS
// ==============================

#[derive(Serialize)]
struct MatchStatus {
    status: String,
    opponent: Option<u64>,
    match_id: Option<String>,
}

async fn match_status(Path(id): Path<u64>) -> Json<MatchStatus> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let keys: Vec<String> = con.keys("match:*").await.unwrap_or_default();

    for k in keys {
        let a: u64 = con.hget(&k, "a").await.unwrap_or(0);
        let b: u64 = con.hget(&k, "b").await.unwrap_or(0);

        if a == id {
            return Json(MatchStatus { status: "found".into(), opponent: Some(b), match_id: Some(k) });
        }
        if b == id {
            return Json(MatchStatus { status: "found".into(), opponent: Some(a), match_id: Some(k) });
        }
    }

    Json(MatchStatus { status: "searching".into(), opponent: None, match_id: None })
}

// ==============================
// RATING COLOR
// ==============================

fn rating_class(r: i32) -> &'static str {
    match r {
        r if r <= 1000 => "rating-low",
        r if r <= 1400 => "rating-mid",
        r if r <= 1800 => "rating-good",
        r if r <= 2200 => "rating-high",
        _ => "rating-top",
    }
}

// ==============================
// MAIN DASHBOARD PAGE
// ==============================

async fn dashboard_home() -> Html<String> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let players = get_players(&mut con).await;
    let queue = get_queue(&mut con).await;
    let matches = get_matches(&mut con).await;

    let mut html = String::new();

    html.push_str("<div class='container'>");

    // LEFT SECTION
    html.push_str("<div class='left'>");

    // QUEUE PANEL
    html.push_str("<div class='panel'><h2>Queue</h2>");
    for q in &queue {
        html.push_str(&format!("<div>Player {}</div>", q));
    }
    html.push_str("</div>");

    // MATCHES PANEL
    html.push_str("<div class='panel'><h2>Active Matches</h2>");
    for (_, a, b) in &matches {
        html.push_str(&format!("<div>ID{} vs ID{}</div>", a, b));
    }
    html.push_str("</div>");

    html.push_str("</div>"); // left end

    // RIGHT SECTION
    html.push_str("<div class='right'>");
    html.push_str("<div class='panel'><h1>Players Online</h1></div>");

    html.push_str("<div class='panel scroll'>");
    for (key, rating) in &players {
        let id = key.replace("player:", "");
        let class = rating_class(*rating);

        html.push_str(&format!(
            "<div><a href='/player/{id}' class='{class}'>Player {id} — {rating}</a></div>"
        ));
    }
    html.push_str("</div></div>"); // right end

    html.push_str("</div>"); // container end

    page("Dashboard", html)
}

// ==============================
// PLAYER PAGE
// ==============================

fn elo_svg(points: &[i32]) -> String {
    if points.len() < 2 { return "".into(); }
    let w = 300.0;
    let h = 120.0;

    let min = *points.iter().min().unwrap() as f32;
    let max = *points.iter().max().unwrap() as f32;
    let range = (max - min).max(1.0);

    let mut path = format!("M 0 {}", h - ((points[0] as f32 - min) / range * h));

    for (i, r) in points.iter().enumerate() {
        let x = (i as f32 / (points.len() - 1) as f32) * w;
        let y = h - ((*r as f32 - min) / range * h);
        path.push_str(&format!(" L {} {}", x, y));
    }

    format!(
        "<svg width='300' height='120'>
            <path d='{path}' stroke='#ff4d4d' stroke-width='2' fill='none'/>
        </svg>"
    )
}

async fn player_page(Path(id): Path<u64>) -> Html<String> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let key = format!("player:{id}");
    let rating: i32 = con.hget(&key, "rating").await.unwrap_or(1200);

    let mut history = vec![];
    for i in 0..20 {
        history.push(rating - (20 - i) * 3);
    }

    let graph = elo_svg(&history);

    let body = format!(
        r#"
        <div class='panel'>
            <h1>Player {}</h1>
            <h2>Rating: <span class="{}">{}</span></h2>
        </div>

        <div class='panel'>
            <h2>ELO Trend</h2>
            {}
        </div>

        <a href="/">← Back</a>
        "#,
        id,
        rating_class(rating),
        rating,
        graph
    );

    page(&format!("Player {}", id), body)
}
