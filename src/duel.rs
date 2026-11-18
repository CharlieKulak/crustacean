//! Story-mode boss duels, played on the player's own computer: a port of the Python rules
//! package's duel.py (the `Duel` there). It must match exactly: the server replays the
//! duel from its seed and the player's turns to check a win before paying for it.

use crate::bots;
use crate::game::{Cell, Dir, Game, Wall, DIRS};
use crate::pyrandom::PyRandom;
use indexmap::IndexMap;
use serde_json::{json, Value};

pub const YOU: usize = 0;
pub const BOSS: usize = 1;
const WARN_SECONDS: f64 = 2.0;
const FORCE_DROP_SECONDS: f64 = 4.0;
const MAX_DROP: i64 = 8;
const SUDDEN_DEATH_SECONDS: f64 = 150.0;
const SUDDEN_DEATH_EVERY: f64 = 3.0;
const SIGNATURES: [&str; 7] = ["pebbles", "venom", "scramble", "icewall", "firestorm", "eclipse", "overclock"];
const FOG_SECONDS: f64 = 6.0;
const OVERCLOCK_SECONDS: f64 = 8.0;
const VENOM_SECONDS: f64 = 6.0;
const ARROW_SECONDS: f64 = 8.0;
const ICE_SECONDS: f64 = 7.0;
const FIRE_SECONDS: f64 = 4.0;

fn sloppy(level: &str) -> f64 {
    match level {
        "boss1" => 0.45,
        "boss2" => 0.38,
        "boss3" => 0.32,
        "boss4" => 0.26,
        "boss5" => 0.2,
        "boss6" => 0.14,
        "boss7" => 0.08,
        _ => 0.0,
    }
}

/// Junk sent by a combo of this many fruit: 1, 2, 4, 6, 9, 12, 16, then 4 more each.
pub fn chain_power(chain: i64) -> i64 {
    const TABLE: [i64; 8] = [0, 1, 2, 4, 6, 9, 12, 16];
    if (chain as usize) < TABLE.len() { TABLE[chain as usize] } else { 16 + 4 * (chain - 7) }
}

/// Python's round(): halves go to the even number.
fn py_round(x: f64) -> i64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 && (r as i64) % 2 != 0 {
        (r - x.signum()) as i64
    } else {
        r as i64
    }
}

/// A boss, from the catalogue's story data.
#[derive(Clone)]
pub struct BossDef {
    pub num: usize,
    pub w: i32,
    pub h: i32,
    pub apples: i32,
    pub tick_ms: f64,
    pub combo_window: i64,
    pub attack: String,
    pub attack_every: f64,
    pub bot: String,
}

impl BossDef {
    pub fn from_catalog(num: usize) -> Option<BossDef> {
        let b = &crate::cosmetics::catalog().story["chapters"][num.checked_sub(1)?]["boss"];
        Some(BossDef {
            num,
            w: b["w"].as_i64()? as i32,
            h: b["h"].as_i64()? as i32,
            apples: b["apples"].as_i64()? as i32,
            tick_ms: b["tick_ms"].as_f64()?,
            combo_window: b["combo_window"].as_i64()?,
            attack: b["attack"].as_str()?.to_string(),
            attack_every: b["attack_every"].as_f64()?,
            bot: format!("boss{num}"),
        })
    }
}

pub struct Side {
    pub game: Game,
    pub chain: i64,
    pub chain_left: i64,
    /// [amount, drop at tick, force at tick]
    pub incoming: Vec<[i64; 3]>,
    pub temp: IndexMap<Cell, i64>,
    pub arrows: IndexMap<Cell, i64>,
    pub venom: Vec<(Cell, i64)>,
    pub fog_until: i64,
    pub overclock_until: i64,
    pub best_chain: i64,
    pub sent: i64,
}

impl Side {
    pub fn pending(&self) -> i64 {
        self.incoming.iter().map(|x| x[0]).sum()
    }
}

pub struct Duel {
    pub boss: BossDef,
    pub sides: Vec<Side>,
    pub rng: PyRandom,
    pub bot_rng: PyRandom,
    pub tick: i64,
    pub per_second: f64,
    pub next_sudden: i64,
    pub next_signature: i64,
    pub signature_i: usize,
    pub over: bool,
    pub winner: Option<usize>,
    /// This move's events, for the animations: [kind, side, amount, extra].
    pub events: Vec<Value>,
}

impl Duel {
    pub fn new(boss: BossDef, seed: i64) -> Duel {
        let mut root = PyRandom::new(seed);
        let mut sides = Vec::new();
        for _ in 0..2 {
            let g = Game::new(boss.w, boss.h, &[], boss.apples, "apple", PyRandom::new(root.getrandbits(32) as i64));
            sides.push(Side { game: g, chain: 0, chain_left: 0, incoming: Vec::new(), temp: IndexMap::new(),
                              arrows: IndexMap::new(), venom: Vec::new(), fog_until: 0, overclock_until: 0, best_chain: 0,
                              sent: 0 });
        }
        let rng = PyRandom::new(root.getrandbits(32) as i64);
        let bot_rng = PyRandom::new(root.getrandbits(32) as i64);
        let per_second = 1000.0 / boss.tick_ms;
        let mut d = Duel { boss, sides, rng, bot_rng, tick: 0, per_second, next_sudden: 0, next_signature: 0, signature_i: 0,
                           over: false, winner: None, events: Vec::new() };
        d.next_sudden = d.secs(SUDDEN_DEATH_SECONDS);
        d.next_signature = d.secs(d.boss.attack_every);
        d
    }

    pub fn secs(&self, s: f64) -> i64 {
        py_round(s * self.per_second).max(1)
    }

    /// How long this move lasts, as a share of the normal speed (Overclock: faster).
    pub fn speed(&self) -> f64 {
        if self.sides[YOU].overclock_until > self.tick { 0.75 } else { 1.0 }
    }

    pub fn tick_ms(&self) -> f64 {
        self.boss.tick_ms * self.speed()
    }

    pub fn start(&mut self) {
        for s in &mut self.sides {
            s.game.started = true;
        }
    }

    pub fn you(&mut self) -> &mut Game {
        &mut self.sides[YOU].game
    }

    fn alive(&self) -> Vec<usize> {
        (0..self.sides.len()).filter(|&i| !self.sides[i].game.over).collect()
    }

    fn steer_boss(&mut self) {
        let level = self.boss.bot.clone();
        let g = &self.sides[BOSS].game;
        if g.over {
            return;
        }
        let s = &g.snakes[0];
        let d = bots::choose(g, 0, &mut self.bot_rng, &level);
        let (dx, dy) = s.planned.unwrap_or(s.dir).delta();
        let (hx, hy) = (s.head().x + dx, s.head().y + dy);
        let safe_ahead = g.in_bounds(hx, hy) && g.solid_at(hx, hy).is_none();
        if let Some(d) = d {
            let skip = safe_ahead && self.bot_rng.random() < sloppy(&level);
            if !skip {
                self.sides[BOSS].game.input_for(0, d);
            }
        }
    }

    // ---- a move -------------------------------------------------------------------------

    pub fn step(&mut self) {
        if self.over {
            return;
        }
        self.tick += 1;
        self.events.clear();
        self.steer_boss();
        for i in 0..2 {
            self.move_side(i);
        }
        for i in 0..2 {
            self.combo(i);
            self.drop_junk(i);
            self.expire(i);
        }
        if self.alive().len() > 1 && self.tick >= self.next_signature {
            self.signature();
            self.next_signature = self.tick + self.secs(self.boss.attack_every.max(6.0));
        }
        if self.tick >= self.next_sudden {
            self.next_sudden = self.tick + self.secs(SUDDEN_DEATH_EVERY);
            if self.tick == self.secs(SUDDEN_DEATH_SECONDS) {
                self.events.push(json!(["sudden", -1, 0]));
            }
            for i in 0..2 {
                self.place_junk(i, 2, 0.0);
            }
        }
        self.check_end();
    }

    fn move_side(&mut self, i: usize) {
        let window = self.boss.combo_window;
        let side = &mut self.sides[i];
        if side.game.over {
            return;
        }
        let score = side.game.score;
        side.game.step();
        let eaten = (side.game.score - score) as i64;
        if eaten != 0 {
            side.chain = if side.chain_left > 0 { side.chain + eaten } else { eaten };
            side.chain_left = window;
            side.best_chain = side.best_chain.max(side.chain);
            let chain = side.chain;
            self.events.push(json!(["chain", i, chain]));
        }
    }

    fn combo(&mut self, i: usize) {
        let side = &mut self.sides[i];
        if side.chain_left > 0 {
            side.chain_left -= 1;
            if side.chain_left == 0 && side.chain != 0 {
                let power = chain_power(side.chain);
                self.attack(i, power);
                self.sides[i].chain = 0;
            }
        }
    }

    fn target(&self, i: usize) -> Option<usize> {
        let n = self.sides.len() as i64;
        self.alive().into_iter().filter(|&j| j != i)
            .min_by_key(|&j| (self.sides[j].pending(), (j as i64 - i as i64).rem_euclid(n)))
    }

    pub fn attack(&mut self, i: usize, mut power: i64) {
        let to = self.target(i);
        let mut cancelled = 0;
        {
            let side = &mut self.sides[i];
            while power > 0 && !side.incoming.is_empty() {
                let take = power.min(side.incoming[0][0]);
                side.incoming[0][0] -= take;
                power -= take;
                cancelled += take;
                if side.incoming[0][0] == 0 {
                    side.incoming.remove(0);
                }
            }
        }
        if let (true, Some(to)) = (power > 0, to) {
            let (warn, force) = (self.tick + self.secs(WARN_SECONDS), self.tick + self.secs(FORCE_DROP_SECONDS));
            self.sides[to].incoming.push([power, warn, force]);
            self.sides[i].sent += power;
        }
        let sent = if to.is_some() { power } else { 0 };
        self.events.push(json!(["attack", i, sent, cancelled, to.map_or(-1, |t| t as i64)]));
    }

    fn drop_junk(&mut self, i: usize) {
        let tick = self.tick;
        let side = &self.sides[i];
        if side.incoming.is_empty() || side.game.over {
            return;
        }
        let [amount, drop_at, force_at] = side.incoming[0];
        if tick >= force_at || (tick >= drop_at && side.chain_left == 0) {
            let n = amount.min(MAX_DROP);
            let placed = self.place_junk(i, n, 0.0);
            let side = &mut self.sides[i];
            side.incoming[0][0] -= n;
            if side.incoming[0][0] <= 0 {
                side.incoming.remove(0);
            }
            if placed > 0 {
                self.events.push(json!(["junk", i, placed]));
            }
        }
    }

    /// Cells new junk may land on: not near the head, not just ahead of it.
    fn ok_cell(g: &Game) -> Box<dyn Fn(i32, i32) -> bool> {
        let s = &g.snakes[0];
        let (hx, hy) = (s.head().x, s.head().y);
        let (dx, dy) = s.planned.unwrap_or(s.dir).delta();
        let ahead: Vec<Cell> = (1..5).map(|k| (hx + dx * k, hy + dy * k)).collect();
        Box::new(move |x, y| (x - hx).abs() + (y - hy).abs() > 3 && !ahead.contains(&(x, y)))
    }

    fn free_cell(rng: &mut PyRandom, g: &Game, ok: &dyn Fn(i32, i32) -> bool) -> Option<Cell> {
        let mut cells = Vec::new();
        for y in 0..g.h {
            for x in 0..g.w {
                if g.is_free(x, y) && ok(x, y) {
                    cells.push((x, y));
                }
            }
        }
        if cells.is_empty() { None } else { Some(*rng.choice(&cells)) }
    }

    fn place_junk(&mut self, i: usize, n: i64, temp_seconds: f64) -> i64 {
        if self.sides[i].game.over {
            return 0;
        }
        let until = self.tick + self.secs(temp_seconds);
        let ok = Duel::ok_cell(&self.sides[i].game);
        let mut placed = 0;
        for _ in 0..n {
            let Some(c) = Duel::free_cell(&mut self.rng, &self.sides[i].game, &ok) else { break };
            self.sides[i].game.walls.insert(c, Wall { x: c.0, y: c.1, kind: "wall".into(), dim: None });
            if temp_seconds != 0.0 {
                self.sides[i].temp.insert(c, until);
            }
            placed += 1;
        }
        placed
    }

    fn expire(&mut self, i: usize) {
        let tick = self.tick;
        let side = &mut self.sides[i];
        let melted: Vec<Cell> = side.temp.iter().filter(|(_, u)| tick >= **u).map(|(c, _)| *c).collect();
        for c in melted {
            side.game.walls.shift_remove(&c);
            side.temp.shift_remove(&c);
        }
        let gone: Vec<Cell> = side.arrows.iter().filter(|(_, u)| tick >= **u).map(|(c, _)| *c).collect();
        for c in gone {
            side.game.arrows.shift_remove(&c);
            side.arrows.shift_remove(&c);
        }
        let mut keep = Vec::new();
        for &(c, until) in &side.venom {
            let a = side.game.apple_at(c.0, c.1);
            if tick >= until {
                if let Some(a) = a {
                    side.game.apples[a].poison = false;
                }
            } else if a.is_some() {
                keep.push((c, until));
            }
        }
        side.venom = keep;
    }

    // ---- the boss's signature attacks ---------------------------------------------------

    fn signature(&mut self) {
        let mut kind = self.boss.attack.clone();
        if kind == "chaos" {
            kind = SIGNATURES[self.signature_i % SIGNATURES.len()].to_string();
            self.signature_i += 1;
        }
        self.events.push(json!(["signature", BOSS, 0, kind]));
        let tick = self.tick;
        match kind.as_str() {
            "pebbles" => {
                let at = tick + self.secs(1.0);
                self.sides[YOU].incoming.push([3, at, at]);
            }
            "venom" => {
                let until = tick + self.secs(VENOM_SECONDS);
                let side = &mut self.sides[YOU];
                let total = side.game.apples.len();
                let pick: Vec<usize> = (0..total).filter(|&k| !side.game.apples[k].poison).take(1.max(total.saturating_sub(1)))
                    .collect();
                for k in pick {
                    side.game.apples[k].poison = true;
                    let a = &side.game.apples[k];
                    side.venom.push(((a.x, a.y), until));
                }
            }
            "scramble" => {
                let until = tick + self.secs(ARROW_SECONDS);
                let ok = Duel::ok_cell(&self.sides[YOU].game);
                for _ in 0..4 {
                    if let Some(c) = Duel::free_cell(&mut self.rng, &self.sides[YOU].game, &ok) {
                        let d: Dir = *self.rng.choice(&DIRS);
                        self.sides[YOU].game.arrows.insert(c, d);
                        self.sides[YOU].arrows.insert(c, until);
                    }
                }
            }
            "icewall" => {
                let until = tick + self.secs(ICE_SECONDS);
                let ok = Duel::ok_cell(&self.sides[YOU].game);
                let (w, h) = (self.sides[YOU].game.w, self.sides[YOU].game.h);
                let cells: Vec<Cell> = if self.rng.random() < 0.5 {
                    let y = self.rng.randrange(h as usize) as i32;
                    (0..w).map(|x| (x, y)).collect()
                } else {
                    let x = self.rng.randrange(w as usize) as i32;
                    (0..h).map(|y| (x, y)).collect()
                };
                let gap = self.rng.randrange(cells.len() - 2);
                let side = &mut self.sides[YOU];
                for (k, &c) in cells.iter().enumerate() {
                    if (gap <= k && k < gap + 3) || !side.game.is_free(c.0, c.1) || !ok(c.0, c.1) {
                        continue;
                    }
                    side.game.walls.insert(c, Wall { x: c.0, y: c.1, kind: "wall".into(), dim: None });
                    side.temp.insert(c, until);
                }
            }
            "firestorm" => {
                self.place_junk(YOU, 6, FIRE_SECONDS);
            }
            "eclipse" => self.sides[YOU].fog_until = tick + self.secs(FOG_SECONDS),
            "overclock" => self.sides[YOU].overclock_until = tick + self.secs(OVERCLOCK_SECONDS),
            _ => {}
        }
    }

    // ---- the end ------------------------------------------------------------------------

    fn check_end(&mut self) {
        let (you, boss) = (&self.sides[YOU].game, &self.sides[BOSS].game);
        if you.over && !you.won {
            self.winner = Some(BOSS); // (crashing on the same move as the boss still loses)
        } else if you.won || boss.over {
            self.winner = Some(if you.won || !boss.won { YOU } else { BOSS });
        } else {
            return;
        }
        self.over = true;
        let w = self.winner.unwrap();
        self.events.push(json!(["over", w, 0]));
    }

    pub fn forfeit(&mut self) {
        if !self.over {
            self.over = true;
            self.winner = Some(BOSS);
        }
    }

    /// The duel's extras for one side, as the server's duel state has them (for drawing).
    pub fn side_extras(&self, i: usize) -> Value {
        let s = &self.sides[i];
        let t = self.tick;
        json!({
            "chain": s.chain, "chain_left": s.chain_left,
            "incoming": s.incoming.iter().map(|x| json!([x[0], (x[1] - t).max(0)])).collect::<Vec<_>>(),
            "fog": (s.fog_until - t).max(0), "overclock": (s.overclock_until - t).max(0),
            "best_chain": s.best_chain, "sent": s.sent, "out": s.game.over,
        })
    }
}
