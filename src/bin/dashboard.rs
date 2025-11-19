use axum::{response::Html, routing::get, Router, Json};
use serde::Serialize;
use redis::AsyncCommands;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(dashboard))
        .route("/data", get(data));
    println!("🧭 Dashboard running at http://localhost:8080/");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Serialize)]
struct PlayerInfo {
    id: u64,
    rating: i32,
    status: String,
    wins: i32,
    losses: i32,
}

#[derive(Serialize)]
struct MatchInfo {
    id: String,
    a: u64,
    b: u64,
}

#[derive(Serialize)]
struct DashboardData {
    players: Vec<PlayerInfo>,
    matches: Vec<MatchInfo>,
}

async fn data() -> Json<DashboardData> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let player_keys: Vec<String> = con.keys("player:*").await.unwrap_or_default();
    let mut players = Vec::new();
    for k in player_keys {
        if let Some(id_part) = k.split(':').nth(1) {
            if let Ok(id) = id_part.parse::<u64>() {
                let rating: i32 = con.hget(&k, "rating").await.unwrap_or(0);
                let status: String = con.hget(&k, "status").await.unwrap_or("unknown".into());
                let wins: i32 = con.hget(&k, "wins").await.unwrap_or(0);
                let losses: i32 = con.hget(&k, "losses").await.unwrap_or(0);
                players.push(PlayerInfo { id, rating, status, wins, losses });
            }
        }
    }

    let match_keys: Vec<String> = con.keys("match:*").await.unwrap_or_default();
    let mut matches = Vec::new();
    for mk in match_keys {
        let a: u64 = con.hget(&mk, "a").await.unwrap_or(0);
        let b: u64 = con.hget(&mk, "b").await.unwrap_or(0);
        matches.push(MatchInfo { id: mk.clone(), a, b });
    }

    Json(DashboardData { players, matches })
}

async fn dashboard() -> Html<String> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();

    let keys: Vec<String> = con.keys("player:*").await.unwrap_or_default();
    let mut html = String::new();

    html.push_str(r#"
        <html>
        <head>
            <style>
                :root {
                  --bg:#0d0f13; --panel:#1a1e24; --panel-border:#262c35; --accent:#e84343;
                  --text:#d9dfe6; --muted:#7a8694; --idle:#3a7bd5; --queued:#f0a202; --inmatch:#6dd96d;
                  --font:'Inter','Segoe UI',Arial,sans-serif;
                }
                body { background:radial-gradient(circle at 20% 20%, #161b23, var(--bg)); color:var(--text); font-family:var(--font); margin:0; padding:24px; }
                h1 { color:var(--accent); letter-spacing:1px; margin:0 0 24px; font-weight:600; }
                .grid { display:flex; gap:24px; flex-wrap:wrap; }
                .panel { background:var(--panel); padding:16px 20px; border-radius:14px; box-shadow:0 4px 12px -4px #000; flex:1 1 360px; border:1px solid var(--panel-border); display:flex; flex-direction:column; }
                .panel h2 { margin:0 0 12px; font-size:18px; font-weight:600; display:flex; justify-content:space-between; align-items:center; }
                ul { list-style:none; padding:0; margin:0; }
                li { margin:4px 0; padding:6px 10px; background:#20262d; border-radius:8px; display:flex; justify-content:space-between; align-items:center; font-size:14px; }
                .list-scroll { max-height:340px; overflow-y:auto; scrollbar-width:thin; }
                .list-scroll::-webkit-scrollbar { width:8px; }
                .list-scroll::-webkit-scrollbar-track { background:#14181e; }
                .list-scroll::-webkit-scrollbar-thumb { background:#2d343c; border-radius:4px; }
                .badge { padding:2px 8px; border-radius:999px; font-size:11px; font-weight:600; letter-spacing:.5px; text-transform:uppercase; }
                .status-idle { background:var(--idle); }
                .status-queued { background:var(--queued); color:#161a1f; }
                .status-inmatch { background:var(--inmatch); color:#0d1215; }
                .count { font-size:12px; font-weight:500; color:var(--muted); }
                footer { margin-top:32px; font-size:11px; color:var(--muted); text-align:center; }
                @media (max-width:800px){ .grid { flex-direction:column; } }
                .fade-enter { animation:fade .35s ease; }
                @keyframes fade { from { opacity:0; transform:translateY(6px);} to { opacity:1; transform:translateY(0);} }
                .delta-up { color:#6dd96d; font-weight:600; margin-left:6px; animation:pulse 1s ease; }
                @keyframes pulse { 0% { transform:translateY(3px); opacity:0;} 50% { opacity:1;} 100% { transform:translateY(0);} }
            </style>
            <link rel="preconnect" href="https://fonts.googleapis.com" />
            <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
            <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap" rel="stylesheet" />
        </head>
        <body>
            <h1>Matchmaking Dashboard</h1>
            <div class="grid">
            <div id="queued" class="panel"><h2><span>Queued Players</span><span class="count" id="queued_count">0</span></h2><ul class="list-scroll" id="queued_list">
    "#);

    for k in keys {
        let rating: i32 = con.hget(&k, "rating").await.unwrap_or(0);
        let status: String = con.hget(&k, "status").await.unwrap_or("unknown".into());
        html.push_str(&format!("<li>{k} → rating {rating}, status {status}</li>"));
    }

    html.push_str("</ul></div>");
    html.push_str("<div id=\"ranked\" class=\"panel\"><h2><span>Ranked (Matched)</span><span class=\"count\" id=\"ranked_count\">0</span></h2><ul class=\"list-scroll\" id=\"ranked_list\"><li>Loading…</li></ul></div>");
    html.push_str("<div id=\"stats\" class=\"panel\"><h2><span>Player Stats (W/L)</span><span class=\"count\" id=\"stats_count\">0</span></h2><ul class=\"list-scroll\" id=\"stats_list\"><li>Loading…</li></ul></div>");
    html.push_str("<div id=\"matches\" class=\"panel\"><h2><span>Ongoing Matches</span><span class=\"count\" id=\"matches_count\">0</span></h2><ul class=\"list-scroll\" id=\"matches_list\"><li>Loading…</li></ul></div>");
    html.push_str("</div><footer>Polling every 2s · Upgrade to WebSockets/SSE for push updates.</footer>");
    html.push_str(r#"
        <script>
    const previousRatings = new Map();
    async function refresh() {
            try {
                const res = await fetch('/data');
                const data = await res.json();
                // Separate sets
                const matchesSet = new Set();
                data.matches.forEach(m => { matchesSet.add(m.a); matchesSet.add(m.b); });
                const queued = data.players.filter(p => p.status === 'queued');
                const ranked = data.players.filter(p => matchesSet.has(p.id));
                // Queued panel
                const qdiv = document.querySelector('#queued_list');
                qdiv.innerHTML = '';
                queued.sort((a,b)=>a.id-b.id).forEach(p => {
                    const li = document.createElement('li');
                    li.textContent = `P${p.id} • ${p.rating}`;
                    const badge = document.createElement('span');
                    badge.className = 'badge status-queued';
                    badge.textContent = 'queued';
                    li.appendChild(badge);
                    qdiv.appendChild(li);
                });
                if (queued.length === 0) { qdiv.innerHTML = '<li>None</li>'; }
                document.getElementById('queued_count').textContent = queued.length;
                // Ranked panel
                const rdiv = document.querySelector('#ranked_list');
                rdiv.innerHTML = '';
                ranked.sort((a,b)=>b.rating - a.rating).forEach((p, idx) => {
                    const li = document.createElement('li');
                    const left = document.createElement('span');
                    let arrow = '';
                    if (previousRatings.has(p.id)) {
                       const prev = previousRatings.get(p.id);
                       if (p.rating > prev) arrow = '<span class="delta-up">▲</span>';
                    }
                    left.innerHTML = `#${idx+1} P${p.id} • ${p.rating} ${arrow}`;
                    const badge = document.createElement('span');
                    badge.className = 'badge status-inmatch';
                    badge.textContent = 'matched';
                    li.appendChild(left);
                    li.appendChild(badge);
                    li.classList.add('fade-enter');
                    rdiv.appendChild(li);
                    previousRatings.set(p.id, p.rating);
                });
                if (ranked.length === 0) { rdiv.innerHTML = '<li>None</li>'; }
                document.getElementById('ranked_count').textContent = ranked.length;
                // Stats panel (all players)
                const sdiv = document.querySelector('#stats_list');
                sdiv.innerHTML = '';
                data.players.sort((a,b)=>b.wins - a.wins || a.losses - b.losses).forEach(p => {
                    const li = document.createElement('li');
                    const ratio = (p.wins + p.losses) > 0 ? ((p.wins/(p.wins+p.losses))*100).toFixed(1)+"%" : "-";
                    li.textContent = `P${p.id} • W:${p.wins} L:${p.losses} (${ratio})`;
                    sdiv.appendChild(li);
                });
                if (data.players.length === 0) { sdiv.innerHTML = '<li>None</li>'; }
                document.getElementById('stats_count').textContent = data.players.length;
                // Matches
                const mdiv = document.querySelector('#matches_list');
                mdiv.innerHTML = '';
                data.matches.forEach(m => {
                    const li = document.createElement('li');
                    li.textContent = `${m.id} → P${m.a} vs P${m.b}`;
                    li.classList.add('fade-enter');
                    mdiv.appendChild(li);
                });
                if (data.matches.length === 0) { mdiv.innerHTML = '<li>None</li>'; }
                document.getElementById('matches_count').textContent = data.matches.length;
            } catch(e) {
                console.error('Refresh failed', e);
            }
        }
        refresh();
        setInterval(refresh, 2000);
        </script>
    </body></html>
    "#);
    Html(html)
}
