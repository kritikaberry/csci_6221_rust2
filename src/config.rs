#[derive(Clone)]
pub struct AppCfg {
    pub redis_url: String,
    pub default_rating: f64,
}

impl Default for AppCfg {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1/".into(),
            default_rating: 1200.0,
        }
    }
}
