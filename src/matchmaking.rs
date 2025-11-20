pub fn hybrid_c_decision(r1: i32, r2: i32) -> bool {
    // Relaxed threshold: allow up to 400 rating difference for easier matches (helps bots match)
    (r1 - r2).abs() < 400
}