use axum::{response::{Html, Redirect}, routing::{get, post}, Router, Json, extract::{Path, Query}, http::StatusCode};
use serde::{Serialize, Deserialize};
use redis::AsyncCommands;
use std::collections::HashMap;
use rand::Rng;

#[tokio::main]
async fn main() {
    // Get server IP for sharing
    let local_ip = get_local_ip().unwrap_or_else(|| "localhost".to_string());
    
    let app = Router::new()
        .route("/", get(dashboard))
        .route("/data", get(data))
        .route("/queue", post(queue_player))
        .route("/player/:id/status", get(player_status))
        .route("/play/:session_id", get(play_game))
        .route("/view/:session_id", get(view_game));
    
    println!("🧭 Dashboard running at http://{}:8080/", local_ip);
    println!("🌐 Share this URL with other devices on your network!");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok()?.ip().to_string().into()
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
    session_id: Option<String>,
}

#[derive(Serialize)]
struct CompletedMatchInfo {
    match_id: u64,
    player1: u64,
    player2: u64,
    winner: u64,
    loser: u64,
}

#[derive(Serialize)]
struct DashboardData {
    players: Vec<PlayerInfo>,
    matches: Vec<MatchInfo>,
    completed_matches: Vec<CompletedMatchInfo>,
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
        let session_id: Option<String> = con.hget(&mk, "game_session").await.ok();
        matches.push(MatchInfo { id: mk.clone(), a, b, session_id });
    }

    // Get completed matches (already sorted by timestamp, most recent first)
    let completed = csci_6221_rust2::storage::get_completed_matches().await;
    let completed_matches: Vec<CompletedMatchInfo> = completed
        .into_iter()
        .map(|(mid, p1, p2, w, l, _timestamp)| CompletedMatchInfo {
            match_id: mid,
            player1: p1,
            player2: p2,
            winner: w,
            loser: l,
        })
        .collect();

    Json(DashboardData { players, matches, completed_matches })
}

#[derive(Deserialize)]
struct QueueRequest {
    player_id: u64,
}

async fn queue_player(Json(payload): Json<QueueRequest>) -> Result<Json<HashMap<String, String>>, StatusCode> {
    // Check if player is already in a match
    if let Some((opp, _)) = csci_6221_rust2::storage::check_match(payload.player_id).await {
        let mut response = HashMap::new();
        response.insert("status".to_string(), "already_in_match".to_string());
        response.insert("opponent".to_string(), opp.to_string());
        response.insert("message".to_string(), format!("Player {} is already in a match with P{}", payload.player_id, opp));
        return Ok(Json(response));
    }

    // Check player status
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let key = format!("player:{}", payload.player_id);
    let status: String = con.hget(&key, "status").await.unwrap_or("idle".into());
    
    if status == "inmatch" {
        let mut response = HashMap::new();
        response.insert("status".to_string(), "already_in_match".to_string());
        response.insert("message".to_string(), "Player is already in a match".to_string());
        return Ok(Json(response));
    }

    // Register player if not already registered (with random rating)
    if status == "unknown" || !csci_6221_rust2::storage::get_rating(payload.player_id).await > 0 {
        let rating = rand::thread_rng().gen_range(1100..1900);
        csci_6221_rust2::storage::register_player(payload.player_id, rating).await;
    }

    // Add player to queue directly (no TCP connection needed)
    csci_6221_rust2::storage::push_to_queue(payload.player_id).await;
    
    // Verify player is actually in the queue
    let queue = csci_6221_rust2::storage::get_full_queue().await;
    if !queue.contains(&payload.player_id) {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    let mut response = HashMap::new();
    response.insert("status".to_string(), "queued".to_string());
    response.insert("player_id".to_string(), payload.player_id.to_string());
    response.insert("message".to_string(), "Successfully added to queue".to_string());
    
    Ok(Json(response))
}

async fn player_status(Path(id): Path<u64>) -> Json<HashMap<String, String>> {
    let mut response = HashMap::new();
    
    // 1. Check if player is already matched
    if let Some((opp, _)) = csci_6221_rust2::storage::check_match(id).await {
        if let Some(match_id) = csci_6221_rust2::storage::get_match_id_by_players(id, opp).await {
            if let Some(session_id) = csci_6221_rust2::storage::get_game_session_by_match(match_id).await {
                let server_ip = get_local_ip().unwrap_or_else(|| "localhost".to_string());
                response.insert("status".to_string(), "matched".to_string());
                response.insert("opponent".to_string(), opp.to_string());
                response.insert("session_id".to_string(), session_id.clone());
                response.insert("game_url".to_string(), format!("http://{}:8889/play/{}?player_id={}&player_name=Player{}", server_ip, session_id, id, id));
                return Json(response);
            } else {
                response.insert("status".to_string(), "matched".to_string());
                response.insert("opponent".to_string(), opp.to_string());
                return Json(response);
            }
        }
    }
    
    // 2. Try to match this player (active matchmaking)
    // Bots can only match with bots, real players only with real players
    let is_me_bot = csci_6221_rust2::storage::is_bot(id).await;
    let my_rating = csci_6221_rust2::storage::get_rating(id).await;
    let mut q = csci_6221_rust2::storage::get_full_queue().await;
    
    // Remove myself from local view
    q.retain(|&x| x != id);
    
    // Filter queue: bots match with bots, real players match with real players
    // Also exclude players already in a match
    let mut candidates = Vec::new();
    for player_id in q {
        let is_opp_bot = csci_6221_rust2::storage::is_bot(player_id).await;
        // Only match if both are bots OR both are real players
        if is_me_bot == is_opp_bot {
            // Check if opponent is already in a match
            let client = redis::Client::open("redis://127.0.0.1/").unwrap();
            let mut con = client.get_multiplexed_async_connection().await.unwrap();
            let key = format!("player:{}", player_id);
            let opp_status: String = con.hget(&key, "status").await.unwrap_or("idle".into());
            // Only include if not already in a match
            if opp_status != "inmatch" {
                candidates.push(player_id);
            }
        }
    }
    
    // Try to match with candidates
    for opp in candidates {
        let r2 = csci_6221_rust2::storage::get_rating(opp).await;
        
        if csci_6221_rust2::matchmaking::hybrid_c_decision(my_rating, r2) {
            // Remove both from queue
            csci_6221_rust2::storage::remove_from_queue(id).await;
            csci_6221_rust2::storage::remove_from_queue(opp).await;
            
            let mid = rand::thread_rng().gen_range(10000..99999);
            
            csci_6221_rust2::storage::create_match(id, opp, mid).await;
            
            // Create game session for race game
            let session_id = format!("race_{}", mid);
            // Randomly assign slots (1 or 2)
            let (p1_slot, p2_slot) = if rand::random::<bool>() {
                (1, 2)
            } else {
                (2, 1)
            };
            // Randomly assign which player gets which slot
            let (p1_id, p2_id) = if rand::random::<bool>() {
                (id, opp)
            } else {
                (opp, id)
            };
            
            csci_6221_rust2::storage::create_game_session(mid, session_id.clone(), p1_id, p2_id, p1_slot, p2_slot).await;
            
            let server_ip = get_local_ip().unwrap_or_else(|| "localhost".to_string());
            response.insert("status".to_string(), "matched".to_string());
            response.insert("opponent".to_string(), opp.to_string());
            response.insert("session_id".to_string(), session_id.clone());
            response.insert("game_url".to_string(), format!("http://{}:8889/play/{}?player_id={}&player_name=Player{}", server_ip, session_id, id, id));
            return Json(response);
        }
    }
    
    // 3. No match found
    response.insert("status".to_string(), "queued".to_string());
    Json(response)
}

async fn play_game(Path(session_id): Path<String>, Query(params): Query<HashMap<String, String>>) -> Result<Redirect, StatusCode> {
    // Get player_id and player_name from query params, or try to get from game session
    let player_id = params.get("player_id").and_then(|s| s.parse::<u64>().ok());
    let player_name = params.get("player_name").map(|s| s.clone());
    
    // If not provided, try to get from game session
    let (final_player_id, final_player_name) = if let Some((_, p1_id, p2_id, _, _)) = csci_6221_rust2::storage::get_game_session(&session_id).await {
        // Use the first player ID if player_id not provided
        let pid = player_id.unwrap_or(p1_id);
        let pname = player_name.unwrap_or_else(|| format!("Player{}", pid));
        (pid, pname)
    } else {
        // Fallback if session not found
        let pid = player_id.unwrap_or(0);
        let pname = player_name.unwrap_or_else(|| "Player".to_string());
        (pid, pname)
    };
    
    // Get server IP for redirect
    let server_ip = get_local_ip().unwrap_or_else(|| "localhost".to_string());
    let redirect_url = format!("http://{}:8889/play/{}?player_id={}&player_name={}", 
                              server_ip, session_id, final_player_id, urlencoding::encode(&final_player_name));
    
    Ok(Redirect::temporary(&redirect_url))
}

async fn view_game(Path(session_id): Path<String>) -> Result<Redirect, StatusCode> {
    // Get server IP for redirect
    let server_ip = get_local_ip().unwrap_or_else(|| "localhost".to_string());
    let redirect_url = format!("http://{}:8889/view/{}", server_ip, session_id);
    
    Ok(Redirect::temporary(&redirect_url))
}

async fn dashboard() -> Html<String> {
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
                li { margin:4px 0; padding:6px 10px; background:#20262d; border-radius:8px; display:flex; justify-content:space-between; align-items:center; font-size:14px; transition: opacity .25s ease, transform .25s ease, background-color .25s ease; }
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
                .fade-enter { opacity:0; transform:translateY(6px); }
                .fade-out { opacity:0; transform:translateY(-6px); }
                @keyframes fade { from { opacity:0; transform:translateY(6px);} to { opacity:1; transform:translateY(0);} }
                .delta-up { color:#6dd96d; font-weight:600; margin-left:6px; animation:pulse 1s ease; }
                .delta-down { color:var(--accent); font-weight:600; margin-left:6px; animation:pulse 1s ease; }
                @keyframes pulse { 0% { transform:translateY(3px); opacity:0;} 50% { opacity:1;} 100% { transform:translateY(0);} }
                /* Ranking bars */
                .rank-row { display:flex; justify-content:space-between; align-items:center; gap:12px; }
                .rank-bar { position:relative; height:6px; background:#2a3139; border-radius:999px; overflow:hidden; margin-top:6px; }
                .rank-fill { height:100%; width:0%; background:linear-gradient(90deg, #3ddc84, #6dd96d); box-shadow:0 0 8px rgba(109,217,109,.35) inset; transition: width .4s ease; }
                /* Make bar width responsive to container */
                #ranked_list li { flex-direction:column; align-items:stretch; }
            </style>
            <link rel="preconnect" href="https://fonts.googleapis.com" />
            <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
            <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600&display=swap" rel="stylesheet" />
        </head>
        <body>
            <!-- Player header removed (dashboard is read-only lobby) -->
            <h1>Game Lobby</h1>
          <!-- Player ID input removed: dashboard now allows joining the queue without pre-setting an ID.
              The queue button will prompt for an ID if one isn't stored in sessionStorage. -->
                <!-- Queue section removed — this page is now a read-only lobby overview. -->
            <div class="grid">
                <div id="queued" class="panel">
                    <h2><span>Queued Players</span><span class="count" id="queued_count">0</span></h2>
                    <ul class="list-scroll" id="queued_list"><li class="empty">Loading…</li></ul>
                </div>
                <div id="ranked" class="panel">
                    <h2><span>Rankings</span><span class="count" id="ranked_count">0</span></h2>
                    <ul class="list-scroll" id="ranked_list"><li class="empty">Loading…</li></ul>
                </div>
                <div id="stats" class="panel">
                    <h2><span>Player Stats (W/L)</span><span class="count" id="stats_count">0</span></h2>
                    <ul class="list-scroll" id="stats_list"><li class="empty">Loading…</li></ul>
                </div>
                <div id="matches" class="panel">
                    <h2><span>Ongoing Matches</span><span class="count" id="matches_count">0</span></h2>
                    <ul class="list-scroll" id="matches_list"><li class="empty">Loading…</li></ul>
                </div>
                <div id="completed" class="panel">
                    <h2><span>Match History</span><span class="count" id="completed_count">0</span></h2>
                    <ul class="list-scroll" id="completed_list"><li class="empty">Loading…</li></ul>
                </div>
            </div>
            <footer>Polling every 2s · Upgrade to WebSockets/SSE for push updates.</footer>
            <script>
            // Smooth, minimal DOM updates
            const previousRatings = new Map();

            function raf(fn){ requestAnimationFrame(fn); }

            function setIfChanged(li, html){
                if (li.dataset.content !== html) {
                    li.innerHTML = html;
                    li.dataset.content = html;
                }
            }

                function syncList(listEl, items, keyFn, renderFn, options={}){
                    const existing = new Map(Array.from(listEl.children).map(li => [li.dataset.key, li]));
                    const seen = new Set();
                    const useFlip = !!options.flip;
                    const firstRects = new Map();
                    if (useFlip) {
                        existing.forEach((li, key) => firstRects.set(key, li.getBoundingClientRect()));
                    }
                    items.forEach((item, idx) => {
                        const key = String(keyFn(item, idx));
                        let li = existing.get(key);
                        if (!li) {
                            li = document.createElement('li');
                            li.dataset.key = key;
                            li.classList.add('fade-enter');
                            listEl.appendChild(li);
                            raf(() => li.classList.remove('fade-enter'));
                        }
                        renderFn(li, item, idx);
                        seen.add(key);
                        if (listEl.children[idx] !== li) {
                            listEl.insertBefore(li, listEl.children[idx] || null);
                        }
                    });
                    // FLIP animate moves
                    if (useFlip) {
                        Array.from(listEl.children).forEach(li => {
                            const key = li.dataset.key;
                            const first = firstRects.get(key);
                            if (!first) return;
                            const last = li.getBoundingClientRect();
                            const dx = first.left - last.left;
                            const dy = first.top - last.top;
                            if (dx || dy) {
                                li.style.transform = `translate(${dx}px, ${dy}px)`;
                                li.style.transition = 'transform 0s';
                                requestAnimationFrame(() => {
                                    li.style.transform = '';
                                    li.style.transition = 'transform .25s ease, opacity .25s ease';
                                });
                            }
                        });
                    }
                    // Remove stale
                    for (const [key, li] of existing) {
                        if (!seen.has(key)) {
                            li.classList.add('fade-out');
                            setTimeout(() => li.remove(), 260);
                        }
                    }
                }

            async function refresh() {
                try {
                    const res = await fetch('/data');
                    const data = await res.json();

                    // Compute sets
                    const queued = data.players.filter(p => p.status === 'queued').sort((a,b)=>a.id-b.id);
                    // Ranked players: those who have played matches (have wins or losses)
                    // Rank by rating (which is based on completed match history)
                    const ranked = data.players
                        .filter(p => p.wins > 0 || p.losses > 0)
                        .sort((a,b) => {
                            // Primary sort: rating (higher is better)
                            if (b.rating !== a.rating) return b.rating - a.rating;
                            // Secondary sort: win rate
                            const aRate = (a.wins + a.losses) > 0 ? a.wins / (a.wins + a.losses) : 0;
                            const bRate = (b.wins + b.losses) > 0 ? b.wins / (b.wins + b.losses) : 0;
                            if (bRate !== aRate) return bRate - aRate;
                            // Tertiary sort: total wins
                            return b.wins - a.wins;
                        });

                    // Queued panel
                    const qdiv = document.getElementById('queued_list');
                    if (queued.length === 0) {
                        qdiv.innerHTML = '<li class="empty">None</li>';
                    } else {
                        syncList(qdiv, queued, p => `q-${p.id}`, (li, p) => {
                            const content = `P${p.id} • ${p.rating} <span class="badge status-queued">queued</span>`;
                            setIfChanged(li, content);
                        });
                    }
                    document.getElementById('queued_count').textContent = queued.length;

                    // Ranked panel
                            const rdiv = document.getElementById('ranked_list');
                    if (ranked.length === 0) {
                        rdiv.innerHTML = '<li class="empty">None</li>';
                    } else {
                                // Normalize rating for bar width
                                const maxRating = ranked[0]?.rating || 1;
                                const minRating = ranked[ranked.length-1]?.rating || 0;
                                const span = Math.max(1, maxRating - minRating);
                                syncList(rdiv, ranked, (p, idx) => `r-${p.id}`, (li, p, idx) => {
                                    let arrow = '';
                                    if (previousRatings.has(p.id)) {
                                        const prev = previousRatings.get(p.id);
                                        if (p.rating > prev) arrow = '<span class="delta-up">▲</span>';
                                        else if (p.rating < prev) arrow = '<span class="delta-down">▼</span>';
                                    }
                                    // Create structure once, then update text + bar width
                                    if (!li.dataset.rendered) {
                                        li.innerHTML = `
                                            <div class="rank-row">
                                                <span class="rank-left"></span>
                                                <span class="badge status-inmatch">ranked</span>
                                            </div>
                                            <div class="rank-bar"><div class="rank-fill"></div></div>
                                        `;
                                        li.dataset.rendered = '1';
                                    }
                                    const leftEl = li.querySelector('.rank-left');
                                    const fillEl = li.querySelector('.rank-fill');
                                    const leftText = `#${idx+1} P${p.id} • ${p.rating} ${arrow}`;
                                    if (leftEl && leftEl.innerHTML !== leftText) leftEl.innerHTML = leftText;
                                    const pct = Math.round(((p.rating - minRating) / span) * 100);
                                    if (fillEl) fillEl.style.width = pct + '%';
                                    previousRatings.set(p.id, p.rating);
                                }, { flip: true });
                    }
                    document.getElementById('ranked_count').textContent = ranked.length;

                    // Stats panel
                    const sdiv = document.getElementById('stats_list');
                    const playersSorted = [...data.players].sort((a,b)=>b.wins - a.wins || a.losses - b.losses);
                    if (playersSorted.length === 0) {
                        sdiv.innerHTML = '<li class="empty">None</li>';
                    } else {
                        syncList(sdiv, playersSorted, p => `s-${p.id}`, (li, p) => {
                            const ratio = (p.wins + p.losses) > 0 ? ((p.wins/(p.wins+p.losses))*100).toFixed(1)+"%" : "-";
                            const content = `P${p.id} • W:${p.wins} L:${p.losses} (${ratio})`;
                            setIfChanged(li, content);
                        });
                    }
                    document.getElementById('stats_count').textContent = playersSorted.length;

                    // Matches
                    const mdiv = document.getElementById('matches_list');
                    if (data.matches.length === 0) {
                        mdiv.innerHTML = '<li class="empty">None</li>';
                    } else {
                        syncList(mdiv, data.matches, m => m.id, (li, m) => {
                            const matchId = m.id.replace('match:', '');
                            let buttons = '';
                            if (m.session_id) {
                                // Get server hostname (same host as dashboard, different port)
                                const serverHost = window.location.hostname;
                                const raceWebPort = 8889;
                                const p1Url = `http://${serverHost}:${raceWebPort}/play/${m.session_id}?player_id=${m.a}&player_name=Player${m.a}`;
                                const p2Url = `http://${serverHost}:${raceWebPort}/play/${m.session_id}?player_id=${m.b}&player_name=Player${m.b}`;
                                const viewUrl = `http://${serverHost}:${raceWebPort}/view/${m.session_id}`;
                                buttons = `
                                    <div style="display:flex; gap:8px; margin-top:8px; flex-wrap:wrap;">
                                        <a href="${p1Url}" 
                                           style="background:#3a7bd5; border:none; color:white; padding:6px 12px; border-radius:6px; cursor:pointer; font-size:12px; font-weight:500; text-decoration:none; display:inline-block;">
                                            P${m.a} Enter Game
                                        </a>
                                        <a href="${p2Url}" 
                                           style="background:#3a7bd5; border:none; color:white; padding:6px 12px; border-radius:6px; cursor:pointer; font-size:12px; font-weight:500; text-decoration:none; display:inline-block;">
                                            P${m.b} Enter Game
                                        </a>
                                        <a href="${viewUrl}" 
                                           style="background:#7a8694; border:none; color:white; padding:6px 12px; border-radius:6px; cursor:pointer; font-size:12px; font-weight:500; text-decoration:none; display:inline-block;">
                                            👁️ View
                                        </a>
                                    </div>
                                `;
                            }
                            const content = `
                                <div>
                                    <div>${m.id} → P${m.a} vs P${m.b}</div>
                                    ${buttons}
                                </div>
                            `;
                            setIfChanged(li, content);
                        });
                    }
                    document.getElementById('matches_count').textContent = data.matches.length;

                    // Completed Matches
                    const cdiv = document.getElementById('completed_list');
                    if (data.completed_matches.length === 0) {
                        cdiv.innerHTML = '<li class="empty">None</li>';
                    } else {
                        syncList(cdiv, data.completed_matches, m => `c-${m.match_id}`, (li, m) => {
                            const content = `Match #${m.match_id} → <strong>P${m.winner}</strong> wins over P${m.loser}`;
                            setIfChanged(li, content);
                        });
                    }
                    document.getElementById('completed_count').textContent = data.completed_matches.length;
                } catch (e) {
                    console.error('Refresh failed', e);
                }
            }
            function viewGame(sessionId) {
                window.location.href = `/view/${sessionId}`;
            }
            
            // Dashboard is read-only; no player header or leave action.
            let currentPlayerId = null;
            try { const savedId = sessionStorage.getItem('player_id'); if (savedId) currentPlayerId = savedId; } catch(e) { /* ignore */ }
            
            refresh();
            setInterval(refresh, 2000);
            </script>
    </body></html>
    "#);
    Html(html)
}