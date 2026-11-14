//! What the game remembers between sessions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub server: String,
    pub token: Option<String>,
    pub user: Option<String>,
    pub guest_name: String,
    pub view3d: bool,
    pub volume: f32,
    pub fullscreen: bool,
    // single-player
    pub board_w: i32,
    pub board_h: i32,
    pub speed: f32,
    pub modes: Vec<String>,
    pub fruit: String,
    pub fruit_count: i32,
    pub best: HashMap<String, i32>,
    // speedruns: personal bests (time in ms) by category
    pub pbs: HashMap<String, i64>,
    /// The texture pack in use ("" = none).
    pub pack: String,
    // versus hosting
    pub vs_size: String,
    pub vs_speed: f32,
    pub vs_apples: i32,
    pub vs_goal: i32,
    pub vs_modes: Vec<String>,
    pub vs_ranked: bool,
    pub vs_public: bool,
    pub seen_intro: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            server: String::new(),
            token: None,
            user: None,
            guest_name: String::new(),
            view3d: false,
            volume: 0.8,
            fullscreen: false,
            board_w: 17,
            board_h: 15,
            speed: 0.5,
            modes: Vec::new(),
            fruit: "apple".into(),
            fruit_count: 1,
            best: HashMap::new(),
            pbs: HashMap::new(),
            pack: String::new(),
            vs_size: "medium".into(),
            vs_speed: 0.5,
            vs_apples: 3,
            vs_goal: 0,
            vs_modes: Vec::new(),
            vs_ranked: true,
            vs_public: true,
            seen_intro: false,
        }
    }
}

impl Settings {
    pub fn load() -> Settings {
        crate::net::storage_get()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            crate::net::storage_set(&s);
        }
    }
}

/// Milliseconds per cell for a speed slider position (modes.speed_ms in Python).
pub fn speed_ms(value: f32) -> f64 {
    const TICKS: [(f64, f64); 5] = [(0.0, 260.0), (0.25, 170.0), (0.5, 130.0), (0.75, 85.0), (1.0, 45.0)];
    let v = (value as f64).clamp(0.0, 1.0);
    for w in TICKS.windows(2) {
        let ((p0, m0), (p1, m1)) = (w[0], w[1]);
        if v <= p1 {
            return m0 + (m1 - m0) * (v - p0) / (p1 - p0);
        }
    }
    45.0
}

pub fn speed_label(value: f32) -> &'static str {
    match (value * 4.0).round() as i32 {
        0 => "Slowest",
        1 => "Slow",
        2 => "Default",
        3 => "Fast",
        _ => "Fastest",
    }
}
