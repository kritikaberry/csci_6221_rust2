use crate::models::{State, PlayerId, MatchInfo};

pub fn enqueue(state: &mut State, pid: PlayerId, rating: f64, game: &str) {
    let q = state.queues.entry(game.into()).or_default();
    let pos = q.iter().position(|(_, r)| *r > rating).unwrap_or(q.len());
    q.insert(pos, (pid, rating));
}

pub fn try_match(state: &mut State, game: &str) -> Option<MatchInfo> {
    let q = state.queues.get_mut(game)?;
    if q.len() < 2 { return None; }

    let (a, _) = q.pop_front()?;
    let (b, _) = q.pop_front()?;

    state.next_match_id += 1;

    let m = MatchInfo {
        match_id: state.next_match_id,
        game_id: game.into(),
        a, b,
    };

    state.matches.insert(m.match_id, m.clone());
    state.active_matches += 1;
    state.total_matches += 1;

    Some(m)
}
