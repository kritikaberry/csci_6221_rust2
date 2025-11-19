pub fn apply_elo(r1: i32, r2: i32, win: bool) -> i32 {
    let k = 32.0;
    let expected = 1.0 / (1.0 + 10_f64.powf((r2 - r1) as f64 / 400.0));
    let score = if win { 1.0 } else { 0.0 };
    (r1 as f64 + k * (score - expected)).round() as i32
}
