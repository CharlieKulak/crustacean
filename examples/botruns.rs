//! Plays speedruns with a simple bot and writes the run files, to check them with
//! the Python game: `cargo run --release --example botruns -- out.json [count]`.

use crustacean::game::{DIRS, Dir};
use crustacean::pyrandom::PyRandom;
use crustacean::speedrun::{Category, Run};
use serde_json::Value;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or("runs.json".into());
    let count: usize = args.get(2).and_then(|c| c.parse().ok()).unwrap_or(200);
    let catalog: Value = serde_json::from_str(include_str!("../assets/catalog.json")).unwrap();
    let only = std::env::var("ONLY").ok();
    let mut cats: Vec<Category> = catalog["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| serde_json::from_value(c.clone()).unwrap())
        .collect();
    let all: Vec<String> = ["wall", "portal", "cheese", "borderless", "twin", "yinyang", "key", "sokoban",
                            "poison", "dimension", "minesweeper", "statue", "light", "shield", "arrow", "magnet",
                            "gate"].iter().map(|s| s.to_string()).collect();
    if let Some(o) = &only {
        cats.retain(|c| o.split(',').any(|x| x == c.id));
    }
    let mut pick = PyRandom::new(99);
    let only_builtin = only.is_some();
    for n in 0..if only_builtin { 0 } else { 12 } {
        // custom categories with random mixes of modes
        let mut modes: Vec<String> = Vec::new();
        for _ in 0..(1 + n % 3) {
            let m = pick.choice(&all).clone();
            let clash = [("sokoban", "portal"), ("sokoban", "poison"), ("portal", "poison")]
                .iter()
                .any(|(a, b)| (m == *a && modes.iter().any(|x| x == b)) || (m == *b && modes.iter().any(|x| x == a)));
            if !clash && !modes.contains(&m) {
                modes.push(m);
            }
        }
        let fruit = if n % 4 == 0 { "mixed" } else { "apple" };
        cats.push(Category::custom(10 + n % 5, 9 + n % 4, &modes, Some(6 + n % 5), 1 + n % 3,
                                   100.0 + 7.5 * n as f64, fruit, "", if n % 3 == 0 { 4.0 } else { 0.0 }));
    }
    let mut runs = Vec::new();
    let mut rng = PyRandom::new(2024);
    for i in 0..count {
        let cat = cats[i % cats.len()].clone();
        let seed = rng.randrange(1 << 31) as i64;
        let mut run = Run::new(cat, seed);
        run.input(Dir::Up);
        while !run.game.over && run.game.tick < 3000 {
            let g = &run.game;
            let s = &g.snakes[0];
            let head = s.head();
            let mut want: Vec<Dir> = Vec::new();
            let target = g.apples.iter().filter(|a| !a.poison && !a.locked).map(|a| (a.x, a.y))
                .chain(g.items.iter().filter(|it| it.kind == "key").map(|it| (it.x, it.y)))
                .min_by_key(|&(x, y)| (x - head.x).abs() + (y - head.y).abs());
            if let Some((tx, ty)) = target {
                if tx > head.x { want.push(Dir::Right) }
                if tx < head.x { want.push(Dir::Left) }
                if ty > head.y { want.push(Dir::Down) }
                if ty < head.y { want.push(Dir::Up) }
            }
            let mut shuffled = DIRS.to_vec();
            for k in (1..4).rev() {
                let j = rng.randrange(k + 1);
                shuffled.swap(k, j);
            }
            want.extend(shuffled);
            let mut turn = None;
            for d in want {
                if d == s.dir.opposite() { continue }
                let (dx, dy) = d.delta();
                let (nx, ny) = (head.x + dx, head.y + dy);
                if g.in_bounds(nx, ny) && g.solid_at(nx, ny).is_none() {
                    if d != s.dir { turn = Some(d) }
                    break;
                }
            }
            if let Some(d) = turn { run.input(d) }
            run.step();
            run.game.events.clear();
        }
        runs.push(serde_json::json!({"finished": run.finished,
                                     "file": run.to_json("rustbot", "rust", "2026-11-13T00:00:00Z")}));
    }
    std::fs::write(&out, serde_json::to_string(&runs).unwrap()).unwrap();
    let done = runs.iter().filter(|r| r["finished"].as_bool().unwrap()).count();
    println!("wrote {} runs ({} finished) to {}", runs.len(), done, out);
}
