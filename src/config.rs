#[derive(Clone)]
pub struct AppCfg {
    pub tcp_addr: String,
}

impl Default for AppCfg {
    fn default() -> Self {
        Self {
            tcp_addr: "0.0.0.0:7500".into(),
        }
    }
}
