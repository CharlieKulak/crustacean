//! Speedruns: categories, the in-game timer, run files and replays (from `speedrun.py`).
//!
//! Times are in-game time: the sum of the tick lengths of every move made. A run
//! is a seed plus the inputs and the tick each was pressed on, so the server can
//! replay it in Python and get exactly the same time.

use crate::game::{Dir, Game};
use crate::pyrandom::PyRandom;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};

pub const RUN_FORMAT: i64 = 1;
pub const CUSTOM_PREFIX: &str = "custom-";
pub const WEEKLY_PREFIX: &str = "weekly-";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Category {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub desc: String,
    pub w: i32,
    pub h: i32,
    #[serde(default)]
    pub modes: Vec<String>,
    /// Fruit to eat; None = fill the board.
    pub goal: Option<i32>,
    #[serde(default = "one")]
    pub fruit_count: i32,
    #[serde(default = "default_tick")]
    pub tick_ms: f64,
    #[serde(default)]
    pub accel: f64,
    #[serde(default = "default_min_tick")]
    pub min_tick_ms: f64,
    #[serde(default = "apple")]
    pub fruit: String,
    #[serde(skip)]
    pub custom: bool,
    #[serde(skip)]
    pub weekly: bool,
}

fn one() -> i32 {
    1
}
fn default_tick() -> f64 {
    130.0
}
fn default_min_tick() -> f64 {
    45.0
}
fn apple() -> String {
    "apple".into()
}

impl Category {
    pub fn ms_for(&self, score: i32) -> f64 {
        self.min_tick_ms.max(self.tick_ms - self.accel * score as f64)
    }

    pub fn goal_text(&self) -> String {
        match self.goal {
            None => "Fill the board".into(),
            Some(g) => format!("Eat {g}"),
        }
    }

    /// A custom category from its rules (goal 0 or None = fill the board; accel: ms faster per
    /// fruit eaten, for Accelerate, or 0).
    #[allow(clippy::too_many_arguments)]
    pub fn custom(w: i32, h: i32, modes: &[String], goal: Option<i32>, fruit_count: i32, tick_ms: f64,
                  fruit: &str, name: &str, accel: f64) -> Category {
        let mut modes: Vec<String> = modes.to_vec();
        modes.sort();
        let goal = goal.filter(|g| *g != 0);
        let tick_ms = (tick_ms * 10.0).round_ties_even() / 10.0;
        let mut c = Category {
            id: String::new(),
            name: if name.is_empty() { "Custom".into() } else { name.chars().take(40).collect() },
            desc: String::new(),
            w,
            h,
            modes,
            goal,
            fruit_count,
            tick_ms,
            accel: (accel.clamp(0.0, 50.0) * 10.0).round_ties_even() / 10.0,
            min_tick_ms: 45.0,
            fruit: fruit.into(),
            custom: true,
            weekly: false,
        };
        c.id = format!("{CUSTOM_PREFIX}{}", &hex(&Sha1::digest(c.rules_json().as_bytes()))[..10]);
        c
    }

    /// The rules exactly as Python's `json.dumps(rules, sort_keys=True, separators=(",", ":"))`
    /// writes them: their hash is the category's id.
    pub fn rules_json(&self) -> String {
        let modes: Vec<String> = self.modes.iter().map(|m| serde_json::to_string(m).unwrap()).collect();
        let goal = self.goal.map_or("null".to_string(), |g| g.to_string());
        // (Accelerate is only in the rules when it's on, like the server's: other ids stay the same)
        let accel = if self.accel > 0.0 { format!("\"accel\":{},", py_float(self.accel)) } else { String::new() };
        format!(
            "{{{accel}\"fruit\":{},\"fruit_count\":{},\"goal\":{},\"h\":{},\"modes\":[{}],\"tick_ms\":{},\"w\":{}}}",
            serde_json::to_string(&self.fruit).unwrap(),
            self.fruit_count,
            goal,
            self.h,
            modes.join(","),
            py_float(self.tick_ms),
            self.w
        )
    }

    /// The rules as saved in run files (`category_def` in Python).
    pub fn def(&self) -> Value {
        let mut d = json!({"id": self.id, "name": self.name, "w": self.w, "h": self.h, "modes": self.modes,
                           "goal": self.goal, "fruit_count": self.fruit_count, "tick_ms": self.tick_ms, "fruit": self.fruit});
        if self.accel > 0.0 {
            d["accel"] = json!(self.accel);
        }
        d
    }

    pub fn from_def(d: &Value) -> Option<Category> {
        let modes: Vec<String> = d.get("modes")?.as_array()?.iter().filter_map(|m| m.as_str().map(String::from)).collect();
        let goal = d.get("goal").and_then(|g| g.as_i64()).map(|g| g as i32);
        let c = Category::custom(
            d.get("w")?.as_i64()? as i32,
            d.get("h")?.as_i64()? as i32,
            &modes,
            goal,
            d.get("fruit_count").and_then(|f| f.as_i64()).unwrap_or(1) as i32,
            d.get("tick_ms")?.as_f64()?,
            d.get("fruit").and_then(|f| f.as_str()).unwrap_or("apple"),
            d.get("name").and_then(|n| n.as_str()).unwrap_or(""),
            d.get("accel").and_then(|a| a.as_f64()).unwrap_or(0.0),
        );
        if !(5..=1000).contains(&c.w) || !(5..=1000).contains(&c.h) || c.tick_ms <= 0.0 || c.fruit_count < 1 {
            return None;
        }
        Some(c)
    }

    /// A weekly or community category from the server's description of it (its id stays).
    pub fn from_server(id: &str, name: &str, desc: &str, d: &Value) -> Option<Category> {
        let mut c = Category::from_def(d)?;
        c.id = id.into();
        c.name = name.into();
        c.desc = desc.into();
        c.custom = id.starts_with(CUSTOM_PREFIX);
        c.weekly = id.starts_with(WEEKLY_PREFIX);
        Some(c)
    }

    pub fn mode_refs(&self) -> Vec<&str> {
        self.modes.iter().map(|m| m.as_str()).collect()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A float the way Python's repr writes it (for the values categories use).
pub fn py_float(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e16 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

/// 12345 -> "0:12.345".
pub fn format_time(ms: f64) -> String {
    let ms = ms.round_ties_even() as i64;
    let (minutes, rest) = (ms / 60000, ms % 60000);
    format!("{}:{:02}.{:03}", minutes, rest / 1000, rest % 1000)
}

/// Signed difference: "-0.412" (ahead) or "+1.050" (behind).
pub fn format_delta(ms: f64) -> String {
    let sign = if ms < 0.0 { "-" } else { "+" };
    let a = ms.abs().round_ties_even() as i64;
    if a >= 60000 {
        format!("{sign}{}", format_time(a as f64))
    } else {
        format!("{sign}{}.{:03}", a / 1000, a % 1000)
    }
}

/// One speedrun attempt: a seeded game plus timer, splits and input log.
pub struct Run {
    pub category: Category,
    pub seed: i64,
    pub game: Game,
    pub inputs: Vec<(i64, Dir)>,
    pub elapsed: f64,
    pub splits: Vec<f64>,
    pub finished: bool,
}

impl Run {
    pub fn new(category: Category, seed: i64) -> Run {
        let game = Game::new(category.w, category.h, &category.mode_refs(), category.fruit_count, &category.fruit,
                             PyRandom::new(seed));
        Run { category, seed, game, inputs: Vec::new(), elapsed: 0.0, splits: Vec::new(), finished: false }
    }

    pub fn tick_ms(&self) -> f64 {
        self.category.ms_for(self.game.score)
    }

    pub fn input(&mut self, d: Dir) {
        if self.game.over {
            return;
        }
        self.inputs.push((self.game.tick, d));
        self.game.input(d);
    }

    pub fn step(&mut self) {
        let g = &mut self.game;
        if g.over || !g.started {
            return;
        }
        let ms = self.category.ms_for(g.score);
        let (tick, score) = (g.tick, g.score);
        g.step();
        if g.tick == tick {
            return;
        }
        self.elapsed += ms;
        for _ in 0..(g.score - score) {
            self.splits.push(self.elapsed);
        }
        let reached = self.category.goal.is_some_and(|goal| g.score >= goal);
        if reached || (g.won && !g.snakes.iter().any(|s| s.dead)) {
            self.finished = true;
            if !g.over {
                g.over = true;
                g.won = true;
                g.events.push(crate::game::Event {
                    kind: "win".into(),
                    x: 0,
                    y: 0,
                    fruit: String::new(),
                    snake: -1,
                });
            }
        }
    }

    pub fn time_ms(&self) -> i64 {
        self.elapsed.round_ties_even() as i64
    }

    /// The run file the server checks (`Run.to_json` in Python).
    pub fn to_json(&self, player: &str, game_version: &str, date: &str) -> Value {
        let mut out = json!({
            "format": RUN_FORMAT,
            "game": "opensnake",
            "game_version": game_version,
            "category": self.category.id,
            "seed": self.seed,
            "time_ms": self.time_ms(),
            "time": format_time(self.elapsed),
            "splits_ms": self.splits.iter().map(|s| s.round_ties_even() as i64).collect::<Vec<_>>(),
            "inputs": self.inputs.iter().map(|(t, d)| json!([t, d.name()])).collect::<Vec<_>>(),
            "player": player,
            "date": date,
        });
        if self.category.custom {
            out["category_def"] = self.category.def();
        }
        out
    }

    /// Re-run a saved run from its seed and inputs.
    pub fn replay(category: Category, seed: i64, inputs: &[(i64, Dir)], max_ticks: i64) -> Run {
        let mut run = Run::new(category, seed);
        let mut i = 0;
        while !run.game.over && run.game.tick < max_ticks {
            while i < inputs.len() && inputs[i].0 <= run.game.tick {
                run.input(inputs[i].1);
                i += 1;
            }
            if !run.game.started {
                break;
            }
            run.step();
            run.game.events.clear();
        }
        run
    }
}
