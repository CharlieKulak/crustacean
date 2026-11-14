//! Plays an online match between two clients against a local server and checks the
//! replies (no window): `cargo run --example protocol_check -- http://127.0.0.1:8765 TOKEN_A TOKEN_B`.

use opensnake::game::Dir;
use opensnake::online::{Notice, Online, Req};
use serde_json::json;
use std::time::{Duration, Instant};

fn now(t0: Instant) -> f64 {
    t0.elapsed().as_secs_f64() * 1000.0
}

fn pump(clients: &mut [&mut Online], t0: Instant, ms: u64) -> Vec<Vec<Notice>> {
    let end = Instant::now() + Duration::from_millis(ms);
    let mut out = vec![Vec::new(); clients.len()];
    while Instant::now() < end {
        // (one process, two players: both see every event and keep their own)
        let events: Vec<_> = opensnake::net::poll().into_iter().collect();
        for (i, c) in clients.iter_mut().enumerate() {
            c.handle(&events, now(t0));
            out[i].extend(c.notices.drain(..));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (server, ta, tb) = (a[1].clone(), a[2].clone(), a[3].clone());
    let t0 = Instant::now();
    let mut host = Online::new(server.clone(), Some(ta), Some("snakefan".into()));
    let mut guest = Online::new(server.clone(), Some(tb), Some("gentastic".into()));
    host.refresh_me();
    host.get("/api/daily", Req::Daily);
    host.get("/api/market", Req::Market);
    pump(&mut [&mut host, &mut guest], t0, 600);
    let me = host.me.clone().expect("me");
    println!("me: {} points, counts for {} items, best rank {}", me["points"], me["counts"].as_object().map_or(0, |m| m.len()), me["best_rank"]);
    println!("daily: {}", host.daily.as_ref().map_or("none".into(), |d| format!("{} challenges", d["challenges"].as_array().map_or(0, |a| a.len()))));
    let listings = host.market.as_ref().and_then(|m| m["listings"].as_array().cloned()).unwrap_or_default();
    println!("market: {} listings", listings.len());
    if let Some(l) = listings.first() {
        host.post("/api/market/buy", json!({"id": l["id"]}), Req::MarketBuy);
    }
    let n = pump(&mut [&mut host, &mut guest], t0, 800);
    println!("after buying: {:?}", n[0].iter().filter_map(|x| if let Notice::Say(s, _) = x { Some(s.clone()) } else { None }).collect::<Vec<_>>());
    println!("seller heard: {:?}", n[1].iter().filter_map(|x| if let Notice::Say(s, _) = x { Some(s.clone()) } else { None }).collect::<Vec<_>>());

    // an unranked match
    host.send(json!({"t": "host", "ranked": false, "public": true, "settings": {"size": "small", "speed": 1.0, "goal": 3}}));
    pump(&mut [&mut host, &mut guest], t0, 700);
    let room = host.room.clone().expect("room");
    println!("room {} ranked={} host={}", room["code"], room["ranked"], host.is_host());
    guest.send(json!({"t": "join", "code": room["code"]}));
    pump(&mut [&mut host, &mut guest], t0, 500);
    println!("players: {}", guest.room.as_ref().map_or(0, |r| r["players"].as_array().map_or(0, |p| p.len())));
    host.send(json!({"t": "start"}));
    let mut turns = 0;
    let mut over = false;
    let mut rewards = Vec::new();
    let dirs = [Dir::Up, Dir::Left, Dir::Down, Dir::Right];
    for k in 0..400 {
        let n = pump(&mut [&mut host, &mut guest], t0, 50);
        for (i, ns) in n.iter().enumerate() {
            for x in ns {
                match x {
                    Notice::MatchOver(_) => over = true,
                    Notice::Reward(r) => rewards.push((i, r.clone())),
                    _ => {}
                }
            }
        }
        if k % 4 == 0 && host.game.is_some() && host.turn(dirs[(k / 4) % 4], now(t0)) {
            turns += 1;
        }
        if over && rewards.len() >= 2 {
            break;
        }
    }
    println!("ping {:?} ms, turns sent {turns}, over {over}", host.ping_ms.map(|p| p.round()));
    for (i, r) in &rewards {
        println!("reward for {}: points {} ranked {} rp_change {}", if *i == 0 { "host" } else { "guest" }, r["points"], r["ranked"], r["rp_change"]);
    }
    let ok = over && rewards.len() == 2 && rewards.iter().all(|(_, r)| r["ranked"] == false && r["rp_change"] == 0);
    println!("{}", if ok { "PROTOCOL OK" } else { "PROTOCOL FAILED" });
    std::process::exit(if ok { 0 } else { 1 });
}
