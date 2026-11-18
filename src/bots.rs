//! Computer snakes: a port of the Python rules package's bots.py. It must match exactly
//! (same choices, same random numbers drawn in the same order): story bosses are played on
//! the player's own computer, and the server replays a duel in Python to check a win.

use crate::game::{Cell, Dir, Game, DIRS};
use crate::pyrandom::PyRandom;
use std::collections::{HashMap, HashSet, VecDeque};

/// Chance of an aimless (but safe) move.
pub fn wander(level: &str) -> f64 {
    match level {
        "easy" => 0.2,
        "normal" => 0.05,
        "hard" => 0.0,
        "boss1" => 0.12,
        "boss2" => 0.1,
        "boss3" => 0.08,
        "boss4" => 0.06,
        "boss5" => 0.04,
        "boss6" => 0.02,
        "boss7" => 0.01,
        "boss8" => 0.0,
        _ => 0.05,
    }
}

/// How hard it avoids cells another snake's head could move into.
pub fn danger(level: &str) -> i64 {
    match level {
        "easy" => 12,
        "normal" => 25,
        "hard" => 60,
        l if l.starts_with("boss") => 60,
        _ => 25,
    }
}

/// Cells a head can't move into this tick.
fn blocked(game: &Game) -> HashSet<Cell> {
    let mut out: HashSet<Cell> = game.walls.iter().filter(|(_k, w)| game.wall_active(w)).map(|(k, _w)| *k).collect();
    out.extend(game.gates.iter().filter(|g| !g.open).map(|g| (g.x, g.y)));
    out.extend(game.apples.iter().filter(|a| a.locked || a.poison).map(|a| (a.x, a.y)));
    out.extend(game.arrows.keys().copied());
    let cheese = game.has("cheese");
    for s in &game.snakes {
        if s.dead {
            continue;
        }
        let end = if !s.moved && s.grow == 0 { s.body.len().saturating_sub(1) } else { s.body.len() };
        for (i, p) in s.body.iter().enumerate().take(end) {
            if !(cheese && p.hole && i > 0) {
                out.insert((p.x, p.y));
            }
        }
    }
    out
}

/// The direction bot `index` wants to go next, or None to keep going.
pub fn choose(game: &Game, index: usize, rng: &mut PyRandom, level: &str) -> Option<Dir> {
    let s = &game.snakes[index];
    if s.dead || game.over {
        return None;
    }
    let (w, h) = (game.w, game.h);
    let wrap = game.has("borderless");
    let current = s.planned.unwrap_or(s.dir);
    let blocked = blocked(game);
    let step = |c: Cell, d: Dir| -> Option<Cell> {
        let (dx, dy) = d.delta();
        let (x, y) = (c.0 + dx, c.1 + dy);
        if wrap {
            Some((x.rem_euclid(w), y.rem_euclid(h)))
        } else if 0 <= x && x < w && 0 <= y && y < h {
            Some((x, y))
        } else {
            None
        }
    };
    let head = (s.head().x, s.head().y);
    let mut moves: Vec<(Dir, Cell)> = Vec::new();
    for d in DIRS {
        if d == current.opposite() {
            continue;
        }
        if let Some(c) = step(head, d) {
            if !blocked.contains(&c) {
                moves.push((d, c));
            }
        }
    }
    if moves.is_empty() {
        return None;
    }
    // cells another snake's head might move into next: a head-on there knocks both out
    let mut danger_cells: HashSet<Cell> = HashSet::new();
    for (j, o) in game.snakes.iter().enumerate() {
        if j != index && !o.dead {
            for d in DIRS {
                if let Some(c) = step((o.head().x, o.head().y), d) {
                    danger_cells.insert(c);
                }
            }
        }
    }
    // what's worth going for
    let locked: HashSet<i32> = game.apples.iter().filter(|a| a.locked).map(|a| a.group).collect();
    let mut targets: HashSet<Cell> = game.apples.iter().filter(|a| !a.poison && !a.locked).map(|a| (a.x, a.y)).collect();
    targets.extend(game.items.iter().filter(|it| (it.kind == "key" && locked.contains(&it.group)) || it.kind == "shield")
        .map(|it| (it.x, it.y)));
    // shortest path to the nearest target, remembering the first move
    let mut first: HashMap<Cell, Dir> = HashMap::new();
    let mut queue: VecDeque<Cell> = VecDeque::new();
    for &(d, c) in &moves {
        if let std::collections::hash_map::Entry::Vacant(e) = first.entry(c) {
            e.insert(d);
            queue.push_back(c);
        }
    }
    let mut goal_dir = None;
    while let Some(c) = queue.pop_front() {
        if targets.contains(&c) {
            goal_dir = Some(first[&c]);
            break;
        }
        for d in DIRS {
            if let Some(n) = step(c, d) {
                if !blocked.contains(&n) && !first.contains_key(&n) {
                    let f = first[&c];
                    first.insert(n, f);
                    queue.push_back(n);
                }
            }
        }
    }
    let need = s.body.len() as i64 + 2;
    let room = |start: Cell| -> i64 {
        let mut seen: HashSet<Cell> = HashSet::from([start]);
        let mut todo = vec![start];
        while !todo.is_empty() && (seen.len() as i64) < need + 8 {
            let c = todo.pop().unwrap();
            for d in DIRS {
                if let Some(n) = step(c, d) {
                    if !blocked.contains(&n) && !seen.contains(&n) {
                        seen.insert(n);
                        todo.push(n);
                    }
                }
            }
        }
        seen.len() as i64
    };
    let mut scored: Vec<(i64, Dir)> = Vec::new();
    for &(d, c) in &moves {
        let space = room(c);
        let mut score = space.min(need) * 10;
        if Some(d) == goal_dir {
            score += 5;
        }
        if danger_cells.contains(&c) {
            score -= danger(level);
        }
        if d == current {
            score += 1;
        }
        scored.push((score, d));
    }
    if rng.random() < wander(level) {
        let safe: Vec<Dir> = scored.iter().filter(|(sc, _)| *sc >= need * 10 - 5).map(|(_, d)| *d).collect();
        if !safe.is_empty() {
            return Some(*rng.choice(&safe));
        }
    }
    let best = scored.iter().map(|(sc, _)| *sc).max().unwrap();
    let options: Vec<Dir> = scored.iter().filter(|(sc, _)| *sc == best).map(|(_, d)| *d).collect();
    Some(*rng.choice(&options))
}
