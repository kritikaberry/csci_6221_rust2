pub fn expected_score(r1: f64, r2: f64) -> f64 {
    1.0 / (1.0 + 10.0f64.powf((r2 - r1) / 400.0))
}

pub fn update_rating(old: f64, opp: f64, win: bool, k: f64) -> f64 {
    let exp = expected_score(old, opp);
    let score = if win { 1.0 } else { 0.0 };
    old + k * (score - exp)
}
