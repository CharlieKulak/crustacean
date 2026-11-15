//! The game simulation: a line-by-line port of OpenSnake's `game.py`.
//!
//! It must behave exactly like the Python game, random numbers included: the
//! server checks every speedrun by replaying it in Python. Anything that uses
//! the random number generator has to call it in the same order with the same
//! arguments. The golden tests (tests/golden.rs) replay runs recorded in Python
//! and compare every cell.

use crate::pyrandom::PyRandom;
use indexmap::IndexMap;
use std::collections::{BTreeSet, HashMap, HashSet};

pub type Cell = (i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// In Python's dict order (it matters for anything that iterates over them).
pub const DIRS: [Dir; 4] = [Dir::Up, Dir::Down, Dir::Left, Dir::Right];

impl Dir {
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::Up => (0, -1),
            Dir::Down => (0, 1),
            Dir::Left => (-1, 0),
            Dir::Right => (1, 0),
        }
    }
    pub fn opposite(self) -> Dir {
        match self {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Dir::Up => "up",
            Dir::Down => "down",
            Dir::Left => "left",
            Dir::Right => "right",
        }
    }
    pub fn parse(s: &str) -> Option<Dir> {
        match s {
            "up" => Some(Dir::Up),
            "down" => Some(Dir::Down),
            "left" => Some(Dir::Left),
            "right" => Some(Dir::Right),
            _ => None,
        }
    }
}

pub const FRUITS: [&str; 7] = ["apple", "orange", "cherry", "grape", "strawberry", "lemon", "banana"];

pub fn dir_between(a: Cell, b: Cell) -> Dir {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dx.abs() >= dy.abs() {
        if dx > 0 {
            Dir::Right
        } else {
            Dir::Left
        }
    } else if dy > 0 {
        Dir::Down
    } else {
        Dir::Up
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seg {
    pub x: i32,
    pub y: i32,
    pub hole: bool,
}

impl Seg {
    pub fn new(x: i32, y: i32) -> Seg {
        Seg { x, y, hole: false }
    }
}

#[derive(Clone, Debug)]
pub struct Snake {
    pub body: Vec<Seg>,
    pub dir: Dir,
    pub queue: Vec<Dir>,
    pub planned: Option<Dir>,
    pub grow: i32,
    pub shield: i32,
    pub dead: bool,
    pub hole_counter: i32,
    pub no_interp: bool,
    pub moved: bool,
    pub team: i32,
    pub score: i32,
    pub name: String,
}

impl Snake {
    fn new(body: Vec<Seg>, dir: Dir) -> Snake {
        Snake {
            body,
            dir,
            queue: Vec::new(),
            planned: None,
            grow: 0,
            shield: 0,
            dead: false,
            hole_counter: 0,
            no_interp: false,
            moved: false,
            team: 0,
            score: 0,
            name: String::new(),
        }
    }
    pub fn head(&self) -> Seg {
        self.body[0]
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Apple {
    pub x: i32,
    pub y: i32,
    pub fruit: String,
    pub group: i32,
    pub poison: bool,
    pub locked: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub x: i32,
    pub y: i32,
    pub kind: String, // "key" | "shield"
    pub group: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Wall {
    pub x: i32,
    pub y: i32,
    pub kind: String,     // "wall" | "statue"
    pub dim: Option<i32>, // dimension mode: the dimension it's solid in
}

#[derive(Clone, Debug, PartialEq)]
pub struct Gate {
    pub x: i32,
    pub y: i32,
    pub period: i32,
    pub phase: i32,
    pub open: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub kind: String,
    pub x: i32,
    pub y: i32,
    pub fruit: String,
    pub snake: i32,
}

impl Event {
    fn new(kind: &str) -> Event {
        Event { kind: kind.into(), x: 0, y: 0, fruit: String::new(), snake: -1 }
    }
    fn at(kind: &str, x: i32, y: i32) -> Event {
        Event { kind: kind.into(), x, y, fruit: String::new(), snake: -1 }
    }
    fn fruit(kind: &str, x: i32, y: i32, fruit: &str) -> Event {
        Event { kind: kind.into(), x, y, fruit: fruit.into(), snake: -1 }
    }
}

type Ok<'a> = &'a dyn Fn(&Game, i32, i32) -> bool;

#[derive(Clone)]
pub struct Game {
    pub w: i32,
    pub h: i32,
    pub teams: Option<Vec<i32>>,
    pub goal: i32,
    pub modes: BTreeSet<String>,
    pub apple_count: i32,
    pub fruit: String,
    pub rng: PyRandom,

    pub tick: i64,
    pub score: i32,
    pub over: bool,
    pub won: bool,
    pub started: bool,
    pub events: Vec<Event>,
    pub winner: Option<i32>,
    pub next_group: i32,
    pub dim: i32,

    pub walls: IndexMap<Cell, Wall>,
    pub apples: Vec<Apple>,
    /// How many fruit are on each cell (a quick lookup; the list above is the real thing).
    /// Code outside this file that changes `apples` calls `reindex_apples` after.
    pub apple_cells: HashMap<Cell, u32>,
    pub items: Vec<Item>,
    pub targets: Vec<Cell>,
    pub mines: HashSet<Cell>,
    pub revealed: HashSet<Cell>,
    pub arrows: IndexMap<Cell, Dir>,
    pub gates: Vec<Gate>,
    pub snakes: Vec<Snake>,
    /// Every snake's cells before the last step (for drawing in between).
    pub prev: Vec<Vec<Cell>>,
}

impl Game {
    pub fn new(w: i32, h: i32, modes: &[&str], apple_count: i32, fruit: &str, rng: PyRandom) -> Game {
        Game::with_teams(w, h, modes, apple_count, fruit, rng, None, 0)
    }

    /// `teams` turns on versus: one snake per entry, the entry being its team (0 or 1).
    #[allow(clippy::too_many_arguments)]
    pub fn with_teams(w: i32, h: i32, modes: &[&str], apple_count: i32, fruit: &str, rng: PyRandom,
                      teams: Option<Vec<i32>>, goal: i32) -> Game {
        let mut modes: BTreeSet<String> = modes.iter().map(|m| m.to_string()).collect();
        if teams.as_ref().is_some_and(|t| !t.is_empty()) {
            modes.remove("yinyang");
        }
        let mut g = Game {
            w,
            h,
            teams: teams.filter(|t| !t.is_empty()),
            goal,
            modes,
            apple_count,
            fruit: fruit.to_string(),
            rng,
            tick: 0,
            score: 0,
            over: false,
            won: false,
            started: false,
            events: Vec::new(),
            winner: None,
            next_group: 1,
            dim: 0,
            walls: IndexMap::new(),
            apples: Vec::new(),
            apple_cells: HashMap::new(),
            items: Vec::new(),
            targets: Vec::new(),
            mines: HashSet::new(),
            revealed: HashSet::new(),
            arrows: IndexMap::new(),
            gates: Vec::new(),
            snakes: Vec::new(),
            prev: Vec::new(),
        };
        g.reset();
        g
    }

    pub fn has(&self, mode: &str) -> bool {
        self.modes.contains(mode)
    }

    pub fn versus(&self) -> bool {
        self.teams.is_some()
    }

    pub fn reset(&mut self) {
        let (w, h) = (self.w, self.h);
        let mid_y = h.div_euclid(2);
        let start_x = 3.max(w.div_euclid(4));
        let yin = self.has("yinyang");
        self.tick = 0;
        self.score = 0;
        self.over = false;
        self.won = false;
        self.started = false;
        self.events.clear();
        self.winner = None;
        self.next_group = 1;
        self.dim = 0;
        self.walls.clear();
        self.apples.clear();
        self.apple_cells.clear();
        self.items.clear();
        self.targets.clear();
        self.mines.clear();
        self.revealed.clear();
        self.arrows.clear();
        self.gates.clear();

        let row = if yin { 1.max(mid_y - 2) } else { mid_y };
        if self.versus() {
            self.snakes = self.spawn_teams();
        } else {
            self.snakes = vec![make_snake(start_x, row, Dir::Right)];
            if yin {
                self.snakes.push(make_snake(w - 1 - start_x, h - 1 - row, Dir::Left));
            }
        }
        self.prev = self.snapshot();

        if !self.versus() && !yin && !self.has("sokoban") && !self.has("portal") {
            // First fruit straight ahead, but not miles away on huge boards.
            let i = self.add_apple((w - 1 - start_x).min(start_x + 8), mid_y, None, None, false);
            let group = self.apples[i].group;
            self.maybe_lock(group);
        }
        self.fill_apples();
        if self.has("gate") {
            self.spawn_gate();
        }
        if self.has("arrow") {
            self.spawn_arrow();
        }
        if self.has("minesweeper") {
            for _ in 0..3 {
                self.spawn_mine();
            }
        }
        for s in &self.snakes {
            for p in &s.body {
                self.revealed.insert((p.x, p.y));
            }
        }
    }

    fn spawn_teams(&self) -> Vec<Snake> {
        let teams = self.teams.clone().unwrap_or_default();
        let mut snakes: Vec<Option<Snake>> = vec![None; teams.len()];
        let x = 3.max(self.w.div_euclid(6)).min(self.w.div_euclid(2) - 2);
        for (team, (col, d)) in [(x, Dir::Right), (self.w - 1 - x, Dir::Left)].into_iter().enumerate() {
            let members: Vec<usize> =
                teams.iter().enumerate().filter(|(_, t)| **t == team as i32).map(|(i, _)| i).collect();
            for (j, &i) in members.iter().enumerate() {
                let y = py_round((j as f64 + 1.0) * self.h as f64 / (members.len() as f64 + 1.0)) as i32;
                let y = (self.h - 1).min(0.max(if team == 0 { y } else { self.h - 1 - y }));
                let mut sn = make_snake(col, y, d);
                sn.team = team as i32;
                snakes[i] = Some(sn);
            }
        }
        snakes.into_iter().map(|s| s.expect("every snake has a team")).collect()
    }

    pub fn snapshot(&self) -> Vec<Vec<Cell>> {
        self.snakes.iter().map(|s| s.body.iter().map(|p| (p.x, p.y)).collect()).collect()
    }

    // ---- input

    /// Queue a turn for one snake (versus: each player steers their own).
    pub fn input_for(&mut self, index: usize, d: Dir) {
        if self.over || index >= self.snakes.len() {
            return;
        }
        let s = &mut self.snakes[index];
        if s.dead {
            return;
        }
        let last = s.queue.last().copied().unwrap_or(s.planned.unwrap_or(s.dir));
        if d != last && d != last.opposite() && s.queue.len() < 3 {
            s.queue.push(d);
        }
    }

    pub fn input(&mut self, d: Dir) {
        if self.over {
            return;
        }
        self.started = true;
        for (i, s) in self.snakes.iter_mut().enumerate() {
            let want = if i == 1 { d.opposite() } else { d }; // the yin-yang twin is point-mirrored
            let last = s.queue.last().copied().unwrap_or(s.planned.unwrap_or(s.dir));
            if want == last || want == last.opposite() || s.queue.len() >= 3 {
                continue;
            }
            s.queue.push(want);
        }
    }

    // ---- queries

    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        0 <= x && x < self.w && 0 <= y && y < self.h
    }

    pub fn apple_at(&self, x: i32, y: i32) -> Option<usize> {
        if !self.apple_cells.contains_key(&(x, y)) {
            return None; // (nothing there: no need to look through every fruit)
        }
        self.apples.iter().position(|a| a.x == x && a.y == y)
    }

    fn index_add(&mut self, c: Cell) {
        *self.apple_cells.entry(c).or_insert(0) += 1;
    }

    fn index_remove(&mut self, c: Cell) {
        if let Some(n) = self.apple_cells.get_mut(&c) {
            *n -= 1;
            if *n == 0 {
                self.apple_cells.remove(&c);
            }
        }
    }

    fn move_apple(&mut self, i: usize, x: i32, y: i32) {
        let (ox, oy) = (self.apples[i].x, self.apples[i].y);
        self.index_remove((ox, oy));
        self.apples[i].x = x;
        self.apples[i].y = y;
        self.index_add((x, y));
    }

    /// Rebuild the fruit lookup after changing `apples` directly.
    pub fn reindex_apples(&mut self) {
        self.apple_cells.clear();
        for i in 0..self.apples.len() {
            let c = (self.apples[i].x, self.apples[i].y);
            self.index_add(c);
        }
    }

    pub fn item_at(&self, x: i32, y: i32) -> Option<usize> {
        self.items.iter().position(|it| it.x == x && it.y == y)
    }

    pub fn gate_at(&self, x: i32, y: i32) -> Option<usize> {
        self.gates.iter().position(|g| g.x == x && g.y == y)
    }

    pub fn snake_at(&self, x: i32, y: i32) -> bool {
        self.snakes.iter().any(|s| s.body.iter().any(|p| p.x == x && p.y == y))
    }

    pub fn wall_active(&self, wall: &Wall) -> bool {
        wall.dim.is_none() || wall.dim == Some(self.dim)
    }

    /// What would stop a head entering (x, y), or None if passable.
    pub fn solid_at(&self, x: i32, y: i32) -> Option<&'static str> {
        if let Some(wall) = self.walls.get(&(x, y)) {
            if self.wall_active(wall) {
                return Some(if wall.kind == "statue" { "statue" } else { "wall" });
            }
        }
        if let Some(g) = self.gate_at(x, y) {
            if !self.gates[g].open {
                return Some("gate");
            }
        }
        if let Some(a) = self.apple_at(x, y) {
            if self.apples[a].locked {
                return Some("lock");
            }
        }
        if self.mines.contains(&(x, y)) {
            return Some("mine");
        }
        let cheese = self.has("cheese");
        for s in &self.snakes {
            if s.dead && self.versus() {
                continue; // knocked-out snakes are off the board
            }
            // A tail that will move away this tick does not block.
            let end = if !s.moved && s.grow == 0 { s.body.len() - 1 } else { s.body.len() };
            for (i, p) in s.body.iter().enumerate().take(end) {
                if p.x == x && p.y == y && !(cheese && p.hole && i > 0) {
                    return Some("snake");
                }
            }
        }
        None
    }

    pub fn is_free(&self, x: i32, y: i32) -> bool {
        let k = (x, y);
        self.in_bounds(x, y)
            && !self.walls.contains_key(&k)
            && self.gate_at(x, y).is_none()
            && self.apple_at(x, y).is_none()
            && self.item_at(x, y).is_none()
            && !self.targets.contains(&k)
            && !self.mines.contains(&k)
            && !self.arrows.contains_key(&k)
            && !self.snake_at(x, y)
    }

    pub fn head_dist(&self, x: i32, y: i32) -> i32 {
        self.snakes.iter().map(|s| (s.head().x - x).abs() + (s.head().y - y).abs()).min().unwrap_or(i32::MAX)
    }

    /// A uniformly random free cell passing `ok`, or None if there is none.
    pub fn random_free(&mut self, ok: Ok) -> Option<Cell> {
        let mut taken: HashSet<Cell> = HashSet::new();
        for s in &self.snakes {
            for p in &s.body {
                taken.insert((p.x, p.y));
            }
        }
        taken.extend(self.items.iter().map(|it| (it.x, it.y)));
        taken.extend(self.gates.iter().map(|g| (g.x, g.y)));
        taken.extend(self.targets.iter().copied());
        let free = |g: &Game, x: i32, y: i32| {
            let k = (x, y);
            !taken.contains(&k) && !g.apple_cells.contains_key(&k) && !g.walls.contains_key(&k) && !g.mines.contains(&k)
                && !g.arrows.contains_key(&k)
        };
        for _ in 0..200 {
            let x = self.rng.randrange(self.w as usize) as i32;
            let y = self.rng.randrange(self.h as usize) as i32;
            if free(self, x, y) && ok(self, x, y) {
                return Some((x, y));
            }
        }
        let mut cells = Vec::new();
        for y in 0..self.h {
            for x in 0..self.w {
                if free(self, x, y) && ok(self, x, y) {
                    cells.push((x, y));
                }
            }
        }
        if cells.is_empty() {
            None
        } else {
            Some(*self.rng.choice(&cells))
        }
    }

    /// Cells right in front of a head are kept clear of new hazards.
    pub fn in_path(&self, x: i32, y: i32) -> bool {
        self.snakes.iter().any(|s| {
            let (dx, dy) = s.dir.delta();
            (1..4).any(|i| (s.head().x + dx * i, s.head().y + dy * i) == (x, y))
        })
    }

    /// With obstacles on the board, is a fruit here safe to eat? (See game.py.)
    pub fn escapable(&self, x: i32, y: i32) -> bool {
        if self.walls.is_empty() && self.gates.is_empty() {
            return true;
        }
        let wrap = self.has("borderless");
        let mut blocked: HashSet<Cell> = self.walls.keys().copied().collect();
        blocked.extend(self.gates.iter().map(|g| (g.x, g.y)));
        blocked.extend(self.mines.iter().copied());
        for s in &self.snakes {
            blocked.extend(s.body.iter().map(|p| (p.x, p.y)));
        }
        let open_neighbours = |cx: i32, cy: i32| -> Vec<Cell> {
            let mut out = Vec::new();
            for d in DIRS {
                let (dx, dy) = d.delta();
                let (mut nx, mut ny) = (cx + dx, cy + dy);
                if wrap {
                    nx = nx.rem_euclid(self.w);
                    ny = ny.rem_euclid(self.h);
                } else if !self.in_bounds(nx, ny) {
                    continue;
                }
                if !blocked.contains(&(nx, ny)) {
                    out.push((nx, ny));
                }
            }
            out
        };
        if open_neighbours(x, y).len() < 2 {
            return false; // a dead end: in and no way out
        }
        let longest = self.snakes.iter().map(|s| s.body.len()).max().unwrap_or(0) as i32;
        let need = 80.min(longest + 2) as usize;
        let mut seen: HashSet<Cell> = HashSet::from([(x, y)]);
        let mut todo = vec![(x, y)];
        while !todo.is_empty() && seen.len() < need {
            let (cx, cy) = todo.pop().unwrap();
            for n in open_neighbours(cx, cy) {
                if seen.insert(n) {
                    todo.push(n);
                }
            }
        }
        seen.len() >= need
    }

    fn hazard_cell(&mut self, extra: Ok) -> Option<Cell> {
        let first = self.random_free(&|g, x, y| g.head_dist(x, y) >= 3 && !g.in_path(x, y) && extra(g, x, y));
        first.or_else(|| self.random_free(&|g, x, y| g.head_dist(x, y) >= 2 && extra(g, x, y)))
    }

    // ---- spawning

    fn pick_fruit(&mut self) -> String {
        if self.fruit == "mixed" {
            self.rng.choice(&FRUITS).to_string()
        } else {
            self.fruit.clone()
        }
    }

    /// Adds an apple and returns its index.
    fn add_apple(&mut self, x: i32, y: i32, group: Option<i32>, fruit: Option<String>, poison: bool) -> usize {
        let group = group.unwrap_or_else(|| {
            let g = self.next_group;
            self.next_group += 1;
            g
        });
        let fruit = match fruit {
            Some(f) => f,
            None => self.pick_fruit(),
        };
        self.apples.push(Apple { x, y, fruit, group, poison, locked: false });
        self.index_add((x, y));
        self.apples.len() - 1
    }

    fn spawn_group(&mut self) -> bool {
        let interior = |g: &Game, x: i32, y: i32| 0 < x && x < g.w - 1 && 0 < y && y < g.h - 1;
        if self.has("sokoban") {
            let Some(a) = self.random_free(&|g, x, y| interior(g, x, y) && g.head_dist(x, y) >= 2) else {
                return false;
            };
            let apple = self.add_apple(a.0, a.1, None, None, false);
            if let Some(t) = self.random_free(&|g, x, y| interior(g, x, y) && (x, y) != a) {
                self.targets.push(t);
            }
            let group = self.apples[apple].group;
            self.maybe_lock(group);
            return true;
        }
        // A safe spot if there is one (see escapable), else anywhere free.
        let a = self
            .random_free(&|g, x, y| g.head_dist(x, y) >= 2 && g.escapable(x, y))
            .or_else(|| self.random_free(&|g, x, y| g.head_dist(x, y) >= 2));
        let Some(a) = a else { return false };
        let apple = self.add_apple(a.0, a.1, None, None, false);
        let group = self.apples[apple].group;
        if self.has("portal") {
            let far = move |g: &Game, x: i32, y: i32| g.head_dist(x, y) >= 2 && (x - a.0).abs() + (y - a.1).abs() >= 4;
            let b = self.random_free(&|g, x, y| far(g, x, y) && g.escapable(x, y)).or_else(|| self.random_free(&far));
            let Some(b) = b else {
                if let Some(a) = self.apples.pop() {
                    self.index_remove((a.x, a.y));
                }
                return false;
            };
            let fruit = self.apples[apple].fruit.clone();
            self.add_apple(b.0, b.1, Some(group), Some(fruit), false);
        }
        self.maybe_lock(group);
        true
    }

    fn maybe_lock(&mut self, group: i32) {
        if !self.has("key") {
            return;
        }
        let Some(k) = self.random_free(&|g, x, y| g.head_dist(x, y) >= 2) else { return };
        for a in self.apples.iter_mut() {
            if a.group == group {
                a.locked = true;
            }
        }
        self.items.push(Item { x: k.0, y: k.1, kind: "key".into(), group });
    }

    pub fn fill_apples(&mut self) {
        // (counted once, then kept up to date: each new group is one more, so lots of fruit stays quick)
        let mut good = self.apples.iter().filter(|a| !a.poison).map(|a| a.group).collect::<HashSet<_>>().len() as i32;
        let mut guard = 0;
        let mut limit = self.apple_count * 4 + 50; // stops when the board has no room left
        while good < self.apple_count && guard < limit {
            guard += 1;
            if !self.spawn_group() {
                break;
            }
            good += 1;
        }
        if self.has("poison") {
            limit += self.apple_count * 2;
            let mut poison = self.apples.iter().filter(|a| a.poison).count() as i32;
            while poison < self.apple_count && guard < limit {
                guard += 1;
                let Some(p) = self.random_free(&|g, x, y| g.head_dist(x, y) >= 2) else { break };
                self.add_apple(p.0, p.1, None, None, true);
                poison += 1;
            }
        }
        if !self.apples.iter().any(|a| !a.poison) {
            self.win();
        }
    }

    pub fn spawn_wall(&mut self, dim: Option<i32>) {
        if let Some(c) = self.hazard_cell(&|_, _, _| true) {
            self.walls.insert(c, Wall { x: c.0, y: c.1, kind: "wall".into(), dim });
        }
    }

    pub fn spawn_mine(&mut self) {
        if let Some(c) = self.hazard_cell(&|g, x, y| !g.revealed.contains(&(x, y))) {
            self.mines.insert(c);
        }
    }

    pub fn spawn_gate(&mut self) {
        let Some(c) = self.hazard_cell(&|_, _, _| true) else { return };
        let period = 16 + self.rng.randrange(3) as i32 * 4;
        let phase = self.rng.randrange(period as usize) as i32;
        self.gates.push(Gate { x: c.0, y: c.1, period, phase, open: true });
        self.update_gates();
    }

    pub fn spawn_arrow(&mut self) {
        let Some(c) = self.hazard_cell(&|_, _, _| true) else { return };
        // Never point an arrow straight into the edge right next to it.
        let ok: Vec<Dir> = DIRS
            .into_iter()
            .filter(|d| {
                let (dx, dy) = d.delta();
                self.in_bounds(c.0 + dx * 2, c.1 + dy * 2)
            })
            .collect();
        let d = if ok.is_empty() { Dir::Right } else { *self.rng.choice(&ok) };
        self.arrows.insert(c, d);
    }

    pub fn spawn_shield(&mut self) {
        if self.items.iter().any(|it| it.kind == "shield") {
            return;
        }
        if let Some(c) = self.random_free(&|g, x, y| g.head_dist(x, y) >= 3) {
            self.items.push(Item { x: c.0, y: c.1, kind: "shield".into(), group: 0 });
        }
    }

    pub fn update_gates(&mut self) {
        let tick = self.tick;
        for i in 0..self.gates.len() {
            let g = &self.gates[i];
            let should_open = ((tick + g.phase as i64).rem_euclid(g.period as i64) as f64) < g.period as f64 / 2.0;
            // A gate never shuts on top of the snake.
            let open = should_open || self.snake_at(g.x, g.y);
            self.gates[i].open = open;
        }
    }

    // ---- simulation

    /// Lock in each snake's next turn (halfway between steps, for drawing corners).
    pub fn plan(&mut self) {
        for s in self.snakes.iter_mut() {
            if s.planned.is_none() && !s.queue.is_empty() {
                s.planned = Some(s.queue.remove(0));
            }
        }
    }

    pub fn step(&mut self) {
        if self.over || !self.started {
            return;
        }
        self.tick += 1;
        self.prev = self.snapshot();
        for s in self.snakes.iter_mut() {
            s.moved = false;
            s.no_interp = false;
        }
        let versus = self.versus();
        let mut targets: Vec<(usize, Cell)> = Vec::new();
        if versus {
            for i in 0..self.snakes.len() {
                if !self.snakes[i].dead {
                    turn(&mut self.snakes[i]);
                    targets.push((i, self.next_cell(i)));
                }
            }
        }
        for i in 0..self.snakes.len() {
            if self.snakes[i].dead && versus {
                continue;
            }
            if !versus {
                turn(&mut self.snakes[i]);
            }
            self.move_snake(i);
            self.snakes[i].moved = true;
            if self.over {
                return;
            }
        }
        if versus {
            self.head_on(&targets);
            self.check_versus_end();
            if self.over {
                return;
            }
        }
        if self.has("magnet") && self.tick % 2 == 0 {
            self.magnetize();
        }
        if self.has("sokoban") {
            self.check_targets();
        }
        if self.has("gate") {
            self.update_gates();
        }
        if self.has("shield") && self.tick % 60 == 0 {
            self.spawn_shield();
        }
    }

    fn next_cell(&self, i: usize) -> Cell {
        let s = &self.snakes[i];
        let (dx, dy) = s.dir.delta();
        let (x, y) = (s.head().x + dx, s.head().y + dy);
        if self.has("borderless") {
            self.wrap(x, y)
        } else {
            (x, y)
        }
    }

    fn wrap(&self, x: i32, y: i32) -> Cell {
        (x.rem_euclid(self.w), y.rem_euclid(self.h))
    }

    fn move_snake(&mut self, i: usize) {
        let (dx, dy) = self.snakes[i].dir.delta();
        let head = self.snakes[i].head();
        let (mut nx, mut ny) = (head.x + dx, head.y + dy);
        if !self.in_bounds(nx, ny) {
            if self.has("borderless") {
                (nx, ny) = self.wrap(nx, ny);
            } else {
                return self.collide(i, "edge");
            }
        }
        if self.has("sokoban") {
            if let Some(b) = self.apple_at(nx, ny) {
                if !self.apples[b].locked && !self.apples[b].poison {
                    let (mut px, mut py) = (nx + dx, ny + dy);
                    if !self.in_bounds(px, py) && self.has("borderless") {
                        (px, py) = self.wrap(px, py);
                    }
                    let blocked = !self.in_bounds(px, py)
                        || self.solid_at(px, py).is_some()
                        || self.apple_at(px, py).is_some()
                        || self.snake_at(px, py)
                        || self.item_at(px, py).is_some();
                    if blocked {
                        return self.collide(i, "push");
                    }
                    self.move_apple(b, px, py);
                    self.events.push(Event::at("push", px, py));
                }
            }
        }
        if let Some(hit) = self.solid_at(nx, ny) {
            if self.snakes[i].shield > 0 && hit != "lock" {
                self.snakes[i].shield -= 1;
                self.walls.shift_remove(&(nx, ny));
                self.mines.remove(&(nx, ny));
                self.events.push(Event::at("shield", nx, ny));
            } else {
                return self.collide(i, hit);
            }
        }
        let cheese = self.has("cheese");
        let s = &mut self.snakes[i];
        let hole = cheese && s.hole_counter.rem_euclid(5) >= 3;
        s.hole_counter += 1;
        s.body.insert(0, Seg { x: nx, y: ny, hole });
        if s.grow > 0 {
            s.grow -= 1;
        } else {
            s.body.pop();
        }
        self.revealed.insert((nx, ny));

        if let Some(&d) = self.arrows.get(&(nx, ny)) {
            let s = &mut self.snakes[i];
            s.dir = d;
            s.queue.clear();
            s.planned = None;
            self.events.push(Event::at("arrow", nx, ny));
        }
        if let Some(it) = self.item_at(nx, ny) {
            let item = self.items.remove(it);
            if item.kind == "key" {
                for a in self.apples.iter_mut() {
                    if a.group == item.group {
                        a.locked = false;
                    }
                }
                self.events.push(Event::at("unlock", nx, ny));
            } else {
                self.snakes[i].shield += 1;
                self.events.push(Event::at("shieldup", nx, ny));
            }
        }
        if let Some(a) = self.apple_at(nx, ny) {
            if !self.has("sokoban") || self.apples[a].poison {
                self.eat(i, a);
            }
        }
    }

    fn collide(&mut self, i: usize, what: &str) {
        self.snakes[i].dead = true;
        if self.versus() {
            let h = self.snakes[i].head();
            self.events.push(Event { kind: "die".into(), x: h.x, y: h.y, fruit: what.into(), snake: i as i32 });
            return;
        }
        self.over = true;
        self.events.push(Event { kind: "die".into(), x: 0, y: 0, fruit: what.into(), snake: -1 });
    }

    // ---- versus

    fn head_on(&mut self, targets: &[(usize, Cell)]) {
        let mut cells: IndexMap<Cell, Vec<usize>> = IndexMap::new();
        for &(i, cell) in targets {
            cells.entry(cell).or_default().push(i);
        }
        for group in cells.values() {
            if group.len() > 1 {
                for &i in group {
                    if !self.snakes[i].dead {
                        self.snakes[i].dead = true;
                        let h = self.snakes[i].head();
                        self.events.push(Event {
                            kind: "die".into(),
                            x: h.x,
                            y: h.y,
                            fruit: "head-on".into(),
                            snake: i as i32,
                        });
                    }
                }
            }
        }
    }

    pub fn team_score(&self, team: i32) -> i32 {
        self.snakes.iter().filter(|s| s.team == team).map(|s| s.score).sum()
    }

    fn check_versus_end(&mut self) {
        let alive: BTreeSet<i32> = self.snakes.iter().filter(|s| !s.dead).map(|s| s.team).collect();
        let reached: Vec<i32> = [0, 1].into_iter().filter(|&t| self.goal > 0 && self.team_score(t) >= self.goal).collect();
        if !reached.is_empty() {
            self.winner = if reached.len() == 1 { Some(reached[0]) } else { None };
        } else if alive.len() <= 1 {
            self.winner = alive.iter().next().copied();
        } else {
            return;
        }
        self.over = true;
        self.events.push(Event::new(if self.winner.is_some() { "win" } else { "draw" }));
    }

    fn eat(&mut self, i: usize, a: usize) {
        let apple = self.apples.remove(a);
        self.index_remove((apple.x, apple.y));
        if apple.poison {
            self.events.push(Event::fruit("poison", apple.x, apple.y, &apple.fruit));
            self.collide(i, "poison");
            return;
        }
        self.events.push(Event::fruit("eat", apple.x, apple.y, &apple.fruit));
        if self.has("portal") {
            if let Some(p) = self.apples.iter().position(|b| b.group == apple.group) {
                let partner = self.apples.remove(p);
                self.index_remove((partner.x, partner.y));
                self.snakes[i].body[0] = Seg::new(partner.x, partner.y);
                self.revealed.insert((partner.x, partner.y));
                self.events.push(Event::at("portal", partner.x, partner.y));
            }
        }
        self.scored(i);
    }

    fn scored(&mut self, i: usize) {
        self.score += 1;
        self.snakes[i].score += 1;
        // Only the eater grows, except in Yin Yang where both snakes share everything.
        if self.has("yinyang") {
            for t in self.snakes.iter_mut() {
                t.grow += 1;
            }
        } else {
            self.snakes[i].grow += 1;
        }
        if self.has("statue") && self.snakes[i].body.len() > 3 {
            let rest: Vec<Seg> = self.snakes[i].body[3..].to_vec();
            for p in rest {
                self.walls.insert((p.x, p.y), Wall { x: p.x, y: p.y, kind: "statue".into(), dim: None });
            }
            self.snakes[i].body.truncate(3);
            self.snakes[i].no_interp = true;
        }
        if self.has("twin") {
            let s = &mut self.snakes[i];
            s.body.reverse();
            s.dir = if s.body.len() > 1 {
                dir_between((s.body[1].x, s.body[1].y), (s.body[0].x, s.body[0].y))
            } else {
                s.dir.opposite()
            };
            s.queue.clear();
            s.planned = None;
            s.no_interp = true;
        }
        if self.has("wall") {
            self.spawn_wall(None);
        }
        if self.has("dimension") {
            self.dim = 1 - self.dim;
            let d = self.rng.randrange(2) as i32;
            self.spawn_wall(Some(d));
            self.events.push(Event::new("dimension"));
        }
        if self.has("minesweeper") {
            self.spawn_mine();
        }
        if self.has("gate") && self.score % 2 == 0 {
            self.spawn_gate();
        }
        if self.has("arrow") {
            self.spawn_arrow();
        }
        if self.has("shield") && self.score % 4 == 0 {
            self.spawn_shield();
        }
        self.fill_apples();
        if self.has("poison") {
            // n good + n poison always; eating one swaps which half is which.
            for a in self.apples.iter_mut() {
                a.poison = !a.poison;
            }
        }
    }

    fn check_targets(&mut self) {
        // (Python loops over a copy of the list: apples added by scoring aren't looked at)
        let snapshot = self.apples.clone();
        for a in snapshot {
            if let Some(t) = self.targets.iter().position(|&c| c == (a.x, a.y)) {
                self.targets.remove(t);
                if let Some(idx) = self.apples.iter().position(|b| *b == a) {
                    let gone = self.apples.remove(idx);
                    self.index_remove((gone.x, gone.y));
                }
                self.events.push(Event::fruit("eat", a.x, a.y, &a.fruit));
                self.scored(0);
            }
        }
        // An apple shoved into a corner can never be scored; replace it.
        if self.has("borderless") {
            return;
        }
        let snapshot = self.apples.clone();
        for a in snapshot {
            if (a.x == 0 || a.x == self.w - 1) && (a.y == 0 || a.y == self.h - 1) {
                if let Some(idx) = self.apples.iter().position(|b| *b == a) {
                    let gone = self.apples.remove(idx);
                    self.index_remove((gone.x, gone.y));
                    if !self.targets.is_empty() {
                        self.targets.remove(0);
                    }
                    self.events.push(Event::at("poof", a.x, a.y));
                    self.fill_apples();
                }
            }
        }
    }

    fn magnetize(&mut self) {
        let snapshot = self.apples.clone();
        for a in snapshot {
            if a.locked || a.poison {
                continue;
            }
            // (the apple as it is now: an earlier one may have moved, but not this one)
            let Some(ai) = self.apples.iter().position(|b| *b == a) else { continue };
            let mut best: Option<(usize, i32)> = None;
            for (si, s) in self.snakes.iter().enumerate() {
                let dist = (s.head().x - a.x).abs().max((s.head().y - a.y).abs());
                if 0 < dist && dist <= 3 && best.is_none_or(|b| dist < b.1) {
                    best = Some((si, dist));
                }
            }
            let Some((si, _)) = best else { continue };
            let head = self.snakes[si].head();
            let sx = (head.x > a.x) as i32 - (head.x < a.x) as i32;
            let sy = (head.y > a.y) as i32 - (head.y < a.y) as i32;
            let steps = if (head.x - a.x).abs() >= (head.y - a.y).abs() {
                [(sx, 0), (0, sy)]
            } else {
                [(0, sy), (sx, 0)]
            };
            for (mx, my) in steps {
                if mx == 0 && my == 0 {
                    continue;
                }
                let (tx, ty) = (a.x + mx, a.y + my);
                if (tx, ty) == (head.x, head.y) {
                    if self.has("sokoban") {
                        continue;
                    }
                    self.move_apple(ai, tx, ty);
                    self.eat(si, ai);
                    return; // apple list changed
                }
                if self.is_free(tx, ty) {
                    self.move_apple(ai, tx, ty);
                    break;
                }
            }
        }
    }

    pub fn win(&mut self) {
        if self.over {
            return;
        }
        if self.versus() {
            let (a, b) = (self.team_score(0), self.team_score(1));
            self.winner = if a == b { None } else if a > b { Some(0) } else { Some(1) };
            self.over = true;
            self.events.push(Event::new(if self.winner.is_some() { "win" } else { "draw" }));
            return;
        }
        self.won = true;
        self.over = true;
        self.events.push(Event::new("win"));
    }

    /// Mines in the 8 cells around (x, y), for minesweeper hints.
    pub fn mine_count(&self, x: i32, y: i32) -> i32 {
        let mut n = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if (dx != 0 || dy != 0) && self.mines.contains(&(x + dx, y + dy)) {
                    n += 1;
                }
            }
        }
        n
    }

    /// Where each snake's cells are, by cell (for quick lookups when drawing).
    pub fn occupancy(&self) -> HashMap<Cell, usize> {
        let mut m = HashMap::new();
        for (i, s) in self.snakes.iter().enumerate() {
            for p in &s.body {
                m.insert((p.x, p.y), i);
            }
        }
        m
    }
}

fn turn(s: &mut Snake) {
    if let Some(p) = s.planned.take() {
        s.dir = p;
    } else if !s.queue.is_empty() {
        s.dir = s.queue.remove(0);
    }
}

fn make_snake(x: i32, y: i32, d: Dir) -> Snake {
    let (dx, dy) = d.delta();
    Snake::new((0..4).map(|i| Seg::new(x - dx * i, y - dy * i)).collect(), d)
}

/// Python's round(): halves go to the even neighbour.
pub fn py_round(v: f64) -> f64 {
    v.round_ties_even()
}
