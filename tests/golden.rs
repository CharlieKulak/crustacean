//! The Rust game must match the Python one exactly: the server replays every
//! speedrun in Python. assets/golden.json is made by the Python repo's
//! tools/export_catalog.py.

use crustacean::game::Dir;
use crustacean::pyrandom::PyRandom;
use crustacean::speedrun::{Category, Run};
use serde_json::Value;

fn golden() -> Value {
    serde_json::from_str(include_str!("../assets/golden.json")).unwrap()
}

#[test]
fn random_numbers_match_python() {
    for case in golden()["rng"].as_array().unwrap() {
        let seed = case["seed"].as_i64().unwrap();
        let mut r = PyRandom::new(seed);
        let u32s: Vec<u64> = (0..8).map(|_| r.getrandbits(32)).collect();
        let want: Vec<u64> = case["u32"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        assert_eq!(u32s, want, "getrandbits, seed {seed}");
        let ns = [1u64, 2, 3, 7, 10, 17, 100, 1000, 12345, 1 << 31];
        let got: Vec<u64> = ns.iter().map(|&n| r.randbelow(n)).collect();
        let want: Vec<u64> = case["randrange"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        assert_eq!(got, want, "randrange, seed {seed}");
        for v in case["random"].as_array().unwrap() {
            assert_eq!(r.random(), v.as_f64().unwrap(), "random, seed {seed}");
        }
        let letters = ["a", "b", "c", "d", "e"];
        for v in case["choice"].as_array().unwrap() {
            assert_eq!(*r.choice(&letters), v.as_str().unwrap(), "choice, seed {seed}");
        }
    }
}

fn category(d: &Value, id: &str) -> Category {
    let mut c: Category = serde_json::from_value(d.clone()).unwrap();
    c.id = id.into();
    c.custom = id.starts_with("custom-");
    c
}

#[test]
fn recorded_runs_replay_exactly() {
    let mut failures = Vec::new();
    let runs = golden()["runs"].as_array().unwrap().clone();
    for r in &runs {
        let id = r["id"].as_str().unwrap();
        let seed = r["seed"].as_i64().unwrap();
        let cat = category(&r["category"], id);
        if cat.custom {
            // (the id is a hash of the rules: it has to come out the same)
            let again = Category::custom(cat.w, cat.h, &cat.modes, cat.goal, cat.fruit_count, cat.tick_ms,
                                         &cat.fruit, "", cat.accel);
            assert_eq!(again.id, id, "custom id for {}", cat.rules_json());
        }
        let inputs: Vec<(i64, Dir)> = r["inputs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| (i[0].as_i64().unwrap(), Dir::parse(i[1].as_str().unwrap()).unwrap()))
            .collect();
        let run = Run::replay(cat, seed, &inputs, 2_000_000);
        let st = &r["state"];
        let mut problems = Vec::new();
        if run.elapsed != r["elapsed"].as_f64().unwrap() {
            problems.push(format!("elapsed {} vs {}", run.elapsed, r["elapsed"]));
        }
        if run.finished != r["finished"].as_bool().unwrap() {
            problems.push(format!("finished {} vs {}", run.finished, r["finished"]));
        }
        if run.game.tick != st["tick"].as_i64().unwrap() {
            problems.push(format!("tick {} vs {}", run.game.tick, st["tick"]));
        }
        let snakes: Vec<Vec<(i64, i64, i64)>> = run
            .game
            .snakes
            .iter()
            .map(|s| s.body.iter().map(|p| (p.x as i64, p.y as i64, p.hole as i64)).collect())
            .collect();
        let want: Vec<Vec<(i64, i64, i64)>> = st["snakes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| {
                s["b"].as_array().unwrap().iter().map(|p| {
                    (p[0].as_i64().unwrap(), p[1].as_i64().unwrap(), p[2].as_i64().unwrap())
                }).collect()
            })
            .collect();
        if snakes != want {
            problems.push("snake bodies differ".into());
        }
        let apples: Vec<(i64, i64, String, i64, bool, bool)> = run
            .game
            .apples
            .iter()
            .map(|a| (a.x as i64, a.y as i64, a.fruit.clone(), a.group as i64, a.poison, a.locked))
            .collect();
        let want: Vec<(i64, i64, String, i64, bool, bool)> = st["apples"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| (a[0].as_i64().unwrap(), a[1].as_i64().unwrap(), a[2].as_str().unwrap().to_string(),
                      a[3].as_i64().unwrap(), a[4].as_i64().unwrap() != 0, a[5].as_i64().unwrap() != 0))
            .collect();
        if apples != want {
            problems.push(format!("apples {apples:?} vs {want:?}"));
        }
        let walls: Vec<(i64, i64, String, Option<i64>)> = run
            .game
            .walls
            .values()
            .map(|w| (w.x as i64, w.y as i64, w.kind.clone(), w.dim.map(|d| d as i64)))
            .collect();
        let want: Vec<(i64, i64, String, Option<i64>)> = st["walls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|w| (w[0].as_i64().unwrap(), w[1].as_i64().unwrap(), w[2].as_str().unwrap().to_string(),
                      w[3].as_i64()))
            .collect();
        if walls != want {
            problems.push("walls differ".into());
        }
        if !problems.is_empty() {
            failures.push(format!("{id} seed {seed}: {}", problems.join("; ")));
        }
    }
    assert!(failures.is_empty(), "{} of {} runs differ:\n{}", failures.len(), runs.len(), failures.join("\n"));
}

/// Watching a run (the game's own loop: recorded turns fed in as their moves come up, with
/// `plan` locking turns in half way through a cell) must end exactly where the record did.
#[test]
fn watching_a_run_ends_where_it_did() {
    for r in golden()["runs"].as_array().unwrap() {
        let id = r["id"].as_str().unwrap();
        let cat = category(&r["category"], id);
        let inputs: Vec<(i64, Dir)> = r["inputs"].as_array().unwrap().iter()
            .map(|i| (i[0].as_i64().unwrap(), Dir::parse(i[1].as_str().unwrap()).unwrap())).collect();
        let mut run = Run::new(cat, r["seed"].as_i64().unwrap());
        let mut next = 0;
        for _ in 0..200_000 {
            while next < inputs.len() && inputs[next].0 <= run.game.tick {
                run.input(inputs[next].1);
                next += 1;
            }
            if run.game.over || !run.game.started {
                break;
            }
            run.game.plan();
            run.step();
        }
        assert_eq!(run.elapsed, r["elapsed"].as_f64().unwrap(), "{id}");
        assert_eq!(run.finished, r["finished"].as_bool().unwrap(), "{id}");
    }
}

/// Boss duels are played on the player's computer and replayed by the server in Python to
/// check a win: the Rust duel (and the boss's AI) must come out exactly the same.
#[test]
fn boss_duels_replay_exactly() {
    use crustacean::duel::{BossDef, Duel};
    let mut failures = Vec::new();
    for case in golden()["duels"].as_array().unwrap() {
        let num = case["boss"].as_u64().unwrap() as usize;
        let seed = case["seed"].as_i64().unwrap();
        let mut d = Duel::new(BossDef::from_catalog(num).unwrap(), seed);
        d.start();
        let inputs: Vec<(i64, Dir)> = case["inputs"].as_array().unwrap().iter()
            .map(|i| (i[0].as_i64().unwrap(), Dir::parse(i[1].as_str().unwrap()).unwrap())).collect();
        let mut next = 0;
        while !d.over && d.tick < 6000 {
            while next < inputs.len() && inputs[next].0 <= d.tick {
                let dir = inputs[next].1;
                d.you().input_for(0, dir);
                next += 1;
            }
            d.step();
            for s in &mut d.sides {
                s.game.events.clear();
            }
        }
        let mut problems = Vec::new();
        if d.tick != case["tick"].as_i64().unwrap() {
            problems.push(format!("tick {} vs {}", d.tick, case["tick"]));
        }
        if d.winner.map(|w| w as i64) != case["winner"].as_i64() {
            problems.push(format!("winner {:?} vs {}", d.winner, case["winner"]));
        }
        for (i, want) in case["sides"].as_array().unwrap().iter().enumerate() {
            let s = &d.sides[i];
            let body: Vec<(i64, i64)> = s.game.snakes[0].body.iter().map(|p| (p.x as i64, p.y as i64)).collect();
            let wbody: Vec<(i64, i64)> = want["body"].as_array().unwrap().iter().map(|p| (p[0].as_i64().unwrap(), p[1].as_i64().unwrap())).collect();
            if body != wbody {
                problems.push(format!("side {i} body differs"));
            }
            let mut walls: Vec<(i64, i64)> = s.game.walls.keys().map(|&(x, y)| (x as i64, y as i64)).collect();
            walls.sort();
            let wwalls: Vec<(i64, i64)> = want["walls"].as_array().unwrap().iter().map(|p| (p[0].as_i64().unwrap(), p[1].as_i64().unwrap())).collect();
            if walls != wwalls {
                problems.push(format!("side {i} walls differ ({} vs {})", walls.len(), wwalls.len()));
            }
            let apples: Vec<(i64, i64, i64)> = s.game.apples.iter().map(|a| (a.x as i64, a.y as i64, a.poison as i64)).collect();
            let wapples: Vec<(i64, i64, i64)> = want["apples"].as_array().unwrap().iter()
                .map(|a| (a[0].as_i64().unwrap(), a[1].as_i64().unwrap(), a[2].as_i64().unwrap())).collect();
            if apples != wapples {
                problems.push(format!("side {i} apples differ"));
            }
            let mut arrows: Vec<String> = s.game.arrows.iter().map(|(&(x, y), d)| format!("{x},{y},{}", d.name())).collect();
            arrows.sort();
            let mut warrows: Vec<String> = want["arrows"].as_array().unwrap().iter()
                .map(|a| format!("{},{},{}", a[0], a[1], a[2].as_str().unwrap())).collect();
            warrows.sort();
            if arrows != warrows {
                problems.push(format!("side {i} arrows differ"));
            }
            if s.sent != want["sent"].as_i64().unwrap() || s.best_chain != want["best_chain"].as_i64().unwrap() {
                problems.push(format!("side {i} sent/best chain {}/{} vs {}/{}", s.sent, s.best_chain, want["sent"], want["best_chain"]));
            }
        }
        if !problems.is_empty() {
            failures.push(format!("boss {num} seed {seed}: {}", problems.join("; ")));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
