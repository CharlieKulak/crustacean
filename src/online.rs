//! The OpenSnake server: accounts, snake points, the store, the market, the
//! battle pass, daily challenges, friends, messages, speedrun records and
//! versus matches.
//!
//! Requests are tagged with what they were for; the replies fill in the caches
//! below, which the screens draw from.

use crate::game::{Apple, Dir, Game, Item, Seg, Snake, Wall};
use crate::net::{self, NetEvent};
use crate::pyrandom::PyRandom;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

pub const DEFAULT_SERVER: &str = "https://snake.charliekulak.com";

#[derive(Clone, Debug, PartialEq)]
pub enum Req {
    Info,
    Login { create: bool },
    Me,
    Records,
    Leaderboard(String),
    SubmitRun,
    Round,
    Weekly,
    Categories,
    Vote(String),
    Rename,
    Rankings,
    Pass,
    BuyPass,
    ClaimPass(i64),
    Buy(String),
    OpenCrate(String),
    Market,
    MarketList,
    MarketCancel,
    MarketBuy,
    Daily,
    ClaimDaily,
    SetProfile,
    Picture,
    Profile(String),
    Users,
    Friends,
    FriendAction,
    Chats,
    Conversation(String),
    SendDm,
    Matches,
    GetPicture(String),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tone {
    Good,
    Bad,
    Info,
}

/// What the screens should know about right away (a toast, a sound, a reveal).
#[derive(Clone, Debug)]
pub enum Notice {
    Say(String, Tone),
    CrateOpened(Value),
    RunResult(Value),
    Reward(Value),
    MatchStarted,
    MatchOver(Value),
    Lobby,
    LeftMatch,
    Invite(Value),
    Dm(Value),
    Sound(&'static str),
}

/// A versus match being played (the server runs it; we draw it and predict our turns).
pub struct Match {
    pub game: Game,
    pub you: usize,
    pub tick_ms: f64,
    pub countdown: Option<i64>,
    pub result: Option<Value>,
    pub reward: Option<Value>,
    /// Turns sent and not yet made by the server: (direction, sent at ms, the move it's for).
    pub pending: Vec<(Dir, f64, i64)>,
    /// Time since the latest state arrived (for drawing in between).
    pub acc: f64,
}

pub struct Online {
    pub server: String,
    pub token: Option<String>,
    pub user: Option<String>,
    pub me: Option<Value>,
    pub server_ok: Option<bool>,
    pub site: String,
    pending: HashMap<u32, Req>,
    pub busy: HashSet<String>,
    pub notices: Vec<Notice>,

    // caches
    pub records: HashMap<String, Value>,
    pub leaderboards: HashMap<String, Value>,
    pub weekly: Option<Value>,
    pub community: Option<Value>,
    /// The last community search asked for ("/api/categories?q=...&sort=...").
    pub community_path: String,
    pub rankings: Option<Value>,
    pub pass: Option<Value>,
    pub market: Option<Value>,
    pub daily: Option<Value>,
    pub friends: Option<Value>,
    pub chats: Option<Value>,
    pub conversation: Option<(String, Vec<Value>)>,
    pub users: Option<Value>,
    pub profiles: HashMap<String, Value>,
    pub matches: Option<Value>,
    pub pictures: HashMap<String, Option<String>>, // name -> base64 PNG (None while loading)

    // versus
    ws: Option<u32>,
    ws_open: bool,
    outbox: Vec<String>,
    pub my_id: Option<i64>,
    pub room: Option<Value>,
    pub game: Option<Match>,
    pub ping_ms: Option<f64>,
    last_ping: f64,
    last_hello: f64,
    pub invite: Option<(Value, f64)>,
}

impl Online {
    pub fn new(server: String, token: Option<String>, user: Option<String>) -> Online {
        Online {
            server,
            token,
            user,
            me: None,
            server_ok: None,
            site: String::new(),
            pending: HashMap::new(),
            busy: HashSet::new(),
            notices: Vec::new(),
            records: HashMap::new(),
            leaderboards: HashMap::new(),
            weekly: None,
            community: None,
            community_path: String::new(),
            rankings: None,
            pass: None,
            market: None,
            daily: None,
            friends: None,
            chats: None,
            conversation: None,
            users: None,
            profiles: HashMap::new(),
            matches: None,
            pictures: HashMap::new(),
            ws: None,
            ws_open: false,
            outbox: Vec::new(),
            my_id: None,
            room: None,
            game: None,
            ping_ms: None,
            last_ping: 0.0,
            last_hello: -1e9,
            invite: None,
        }
    }

    pub fn logged_in(&self) -> bool {
        self.token.is_some()
    }

    pub fn points(&self) -> i64 {
        self.me.as_ref().and_then(|m| m["points"].as_i64()).unwrap_or(0)
    }

    pub fn copies(&self, item: &str) -> i64 {
        let Some(me) = &self.me else { return 0 };
        if let Some(n) = me["counts"].get(item).and_then(|v| v.as_i64()) {
            return n;
        }
        me["inventory"].as_array().map_or(0, |a| a.iter().any(|x| x.as_str() == Some(item)) as i64)
    }

    pub fn owns(&self, item: &str) -> bool {
        self.copies(item) > 0
    }

    pub fn equipped_skin(&self) -> String {
        self.me.as_ref().and_then(|m| m["skin"].as_str().map(String::from)).unwrap_or_default()
    }

    fn say(&mut self, text: impl Into<String>, tone: Tone) {
        self.notices.push(Notice::Say(text.into(), tone));
    }

    // ---- requests

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.server.trim_end_matches('/'), path)
    }

    pub fn get(&mut self, path: &str, tag: Req) {
        let key = format!("{tag:?}");
        if self.busy.contains(&key) && !matches!(tag, Req::GetPicture(_)) {
            return; // (already asked)
        }
        let id = net::http("GET", &self.url(path), None, self.token.as_deref());
        self.busy.insert(key);
        self.pending.insert(id, tag);
    }

    pub fn post(&mut self, path: &str, body: Value, tag: Req) {
        let id = net::http("POST", &self.url(path), Some(body.to_string()), self.token.as_deref());
        self.busy.insert(format!("{tag:?}"));
        self.pending.insert(id, tag);
    }

    pub fn is_busy(&self, tag: &Req) -> bool {
        self.busy.contains(&format!("{tag:?}"))
    }

    pub fn check_server(&mut self) {
        self.get("/api/info", Req::Info);
        self.get("/api/records", Req::Records);
        if self.logged_in() {
            self.refresh_me();
            self.hello();
        }
    }

    pub fn refresh_me(&mut self) {
        if self.logged_in() {
            self.get("/api/me", Req::Me);
        }
    }

    pub fn login(&mut self, name: &str, password: &str, create: bool) {
        let path = if create { "/api/register" } else { "/api/login" };
        self.post(path, json!({"username": name, "password": password}), Req::Login { create });
    }

    pub fn logout(&mut self) {
        if self.logged_in() {
            let id = net::http("POST", &self.url("/api/logout"), Some("{}".into()), self.token.as_deref());
            let _ = id;
        }
        self.token = None;
        self.user = None;
        self.me = None;
        self.pass = None;
        self.daily = None;
        self.friends = None;
        self.chats = None;
        self.close_socket();
    }

    pub fn picture(&mut self, name: &str) -> Option<String> {
        if let Some(p) = self.pictures.get(name) {
            return p.clone();
        }
        self.pictures.insert(name.to_string(), None);
        self.get(&format!("/api/pictures/{}", urlencode(name)), Req::GetPicture(name.to_string()));
        None
    }

    // ---- every frame

    pub fn poll(&mut self, now_ms: f64) {
        let events: Vec<NetEvent> = net::poll().into_iter().collect();
        self.handle(&events, now_ms);
    }

    /// Deal with what's arrived (events for other connections are ignored).
    pub fn handle(&mut self, events: &[NetEvent], now_ms: f64) {
        for ev in events.iter().cloned() {
            match ev {
                NetEvent::Http { id, status, body } => {
                    let Some(tag) = self.pending.remove(&id) else { continue };
                    self.busy.remove(&format!("{tag:?}"));
                    let data: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                    self.on_reply(tag, status, data);
                }
                NetEvent::WsOpen { id } if Some(id) == self.ws => {
                    self.ws_open = true;
                    if let Some(t) = self.token.clone() {
                        // (log in with a message, not in the URL, so the token stays out of server logs)
                        net::ws_send(id, &json!({"t": "auth", "token": t}).to_string());
                    }
                    for m in std::mem::take(&mut self.outbox) {
                        net::ws_send(id, &m);
                    }
                }
                NetEvent::WsMessage { id, text } if Some(id) == self.ws => {
                    if let Ok(m) = serde_json::from_str::<Value>(&text) {
                        self.on_message(m, now_ms);
                    }
                }
                NetEvent::WsClosed { id } if Some(id) == self.ws => {
                    let lost = self.room.is_some();
                    self.ws = None;
                    self.ws_open = false;
                    self.room = None;
                    if lost {
                        self.game = None;
                        self.say("Disconnected from the match.", Tone::Bad);
                        self.notices.push(Notice::LeftMatch);
                    }
                }
                NetEvent::Picked { data } if self.logged_in() => {
                    self.upload_picture(&data);
                }
                _ => {}
            }
        }
        if self.ws_open && now_ms - self.last_ping > 2000.0 {
            self.last_ping = now_ms;
            self.ws_send_now(json!({"t": "ping", "at": now_ms}));
        }
        if self.logged_in() && self.ws.is_none() && now_ms - self.last_hello > 15000.0 {
            self.last_hello = now_ms;
            self.hello(); // (re)connect, so friends see you online
        }
        if let Some((_, until)) = &self.invite {
            if now_ms > *until {
                self.invite = None;
            }
        }
    }

    fn on_reply(&mut self, tag: Req, status: u16, d: Value) {
        let ok = (200..300).contains(&status);
        let err = d["error"].as_str().unwrap_or(if status == 0 { "Couldn't reach the server." } else { "Something went wrong." }).to_string();
        if status == 401 && self.logged_in() && !matches!(tag, Req::Login { .. }) {
            // our login ran out (or was logged out elsewhere)
            self.token = None;
            self.user = None;
            self.me = None;
            self.say("You've been logged out. Log in again.", Tone::Bad);
            return;
        }
        match tag {
            Req::Info => {
                self.server_ok = Some(ok);
                if ok {
                    self.site = d["site"].as_str().unwrap_or("").to_string();
                }
            }
            Req::Login { create } => {
                if ok {
                    self.token = d["token"].as_str().map(String::from);
                    self.user = d["username"].as_str().map(String::from);
                    let who = self.user.clone().unwrap_or_default();
                    self.say(if create { format!("Welcome, {who}! Your account is ready.") } else { format!("Logged in as {who}.") }, Tone::Good);
                    self.close_socket();
                    self.refresh_me();
                    self.hello();
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::Me => {
                if ok {
                    self.me = Some(d);
                }
            }
            Req::Records => {
                if let Some(m) = d["records"].as_object() {
                    self.records = m.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                }
            }
            Req::Leaderboard(cat) => {
                if ok {
                    self.leaderboards.insert(cat, d);
                }
            }
            Req::SubmitRun => {
                self.notices.push(Notice::RunResult(if ok { d.clone() } else { json!({"error": err}) }));
                if ok {
                    self.refresh_me();
                    self.get("/api/records", Req::Records);
                    self.daily = None;
                }
            }
            Req::Round => {
                if ok {
                    if let Some(me) = self.me.as_mut() {
                        me["points"] = d["points"].clone();
                    }
                    let earned = d["earned"].as_i64().unwrap_or(0);
                    if earned > 0 {
                        self.say(format!("+{} snake points", crate::cosmetics::format_points(earned)), Tone::Good);
                    }
                    self.daily = None;
                }
            }
            Req::Weekly => self.weekly = ok.then_some(d),
            Req::Categories => self.community = ok.then_some(d),
            Req::Vote(id) => {
                if ok {
                    self.set_votes(&id, d["votes"].as_i64().unwrap_or(0), d["voted"].as_bool().unwrap_or(false));
                } else {
                    self.say(err, Tone::Bad);
                    let path = self.community_path.clone();
                    if !path.is_empty() {
                        self.get(&path, Req::Categories); // (put the real count back)
                    }
                }
            }
            Req::Rename => {
                if ok {
                    self.say("Renamed.", Tone::Good);
                    self.get("/api/categories", Req::Categories);
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::Rankings => self.rankings = ok.then_some(d),
            Req::Pass => self.pass = ok.then_some(d),
            Req::BuyPass => {
                if ok {
                    self.say("Battle pass unlocked! Claim your rewards.", Tone::Good);
                    self.notices.push(Notice::Sound("coin"));
                    self.pass = Some(d["pass"].clone());
                    self.refresh_me();
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::ClaimPass(_) => {
                if ok {
                    if let Some(p) = d.get("pass") {
                        self.pass = Some(p.clone());
                    }
                    let tier = &d["tier"];
                    if tier["kind"] == "points" {
                        self.say(format!("+{} snake points", crate::cosmetics::format_points(tier["amount"].as_i64().unwrap_or(0))), Tone::Good);
                        self.notices.push(Notice::Sound("coin"));
                    } else {
                        self.notices.push(Notice::CrateOpened(d.clone()));
                    }
                    self.refresh_me();
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::Buy(item) => {
                if ok {
                    let name = crate::cosmetics::catalog().item_name(&item);
                    self.say(format!("Bought {name}!"), Tone::Good);
                    self.notices.push(Notice::Sound("coin"));
                    self.refresh_me();
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::OpenCrate(_) => {
                if ok {
                    self.notices.push(Notice::CrateOpened(d));
                    self.refresh_me();
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::Market => self.market = ok.then_some(d),
            Req::MarketList | Req::MarketCancel => {
                if ok {
                    self.say(if tag == Req::MarketList { "Listed on the market." } else { "Taken off the market: it's back in your collection." }, Tone::Good);
                    self.get("/api/market", Req::Market);
                    self.refresh_me();
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::MarketBuy => {
                if ok {
                    self.say(format!("Bought {}!", d["name"].as_str().unwrap_or("it")), Tone::Good);
                    self.notices.push(Notice::Sound("coin"));
                    self.get("/api/market", Req::Market);
                    self.refresh_me();
                } else {
                    self.say(err, Tone::Bad);
                    self.get("/api/market", Req::Market);
                }
            }
            Req::Daily => self.daily = ok.then_some(d),
            Req::ClaimDaily => {
                if ok {
                    self.say("Challenge reward claimed!", Tone::Good);
                    self.notices.push(Notice::Sound("coin"));
                    let mut d = d;
                    if let Some(me) = self.me.as_mut() {
                        me["points"] = d["points"].take();
                    }
                    self.daily = Some(d);
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::SetProfile | Req::Picture => {
                if ok {
                    self.say("Profile saved.", Tone::Good);
                    if let Some(u) = self.user.clone() {
                        self.pictures.remove(&u);
                        self.profiles.remove(&u);
                    }
                    self.refresh_me();
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::Profile(name) => {
                if ok {
                    self.profiles.insert(name, d);
                }
            }
            Req::Users => self.users = ok.then_some(d),
            Req::Friends => self.friends = ok.then_some(d),
            Req::FriendAction => {
                if ok {
                    self.get("/api/friends", Req::Friends);
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::Chats => self.chats = ok.then_some(d),
            Req::Conversation(name) => {
                if ok {
                    let msgs = d["messages"].as_array().cloned().unwrap_or_default();
                    self.conversation = Some((name, msgs));
                }
            }
            Req::SendDm => {
                if ok {
                    if let Some((_, msgs)) = self.conversation.as_mut() {
                        msgs.push(d);
                    }
                } else {
                    self.say(err, Tone::Bad);
                }
            }
            Req::Matches => self.matches = ok.then_some(d),
            Req::GetPicture(name) => {
                let png = d["png"].as_str().map(String::from);
                self.pictures.insert(name, png.or(Some(String::new())));
            }
        }
    }

    /// Upvote a community category (or take it back), showing it straight away.
    pub fn vote(&mut self, id: &str, up: bool) {
        let votes = self.community_entry(id).and_then(|c| c["votes"].as_i64()).unwrap_or(0);
        self.set_votes(id, (votes + if up { 1 } else { -1 }).max(0), up);
        self.post("/api/categories/vote", json!({"id": id, "up": up}), Req::Vote(id.to_string()));
    }

    fn community_entry(&self, id: &str) -> Option<&Value> {
        self.community.as_ref()?["categories"].as_array()?.iter().find(|c| c["id"].as_str() == Some(id))
    }

    fn set_votes(&mut self, id: &str, votes: i64, voted: bool) {
        let Some(list) = self.community.as_mut().and_then(|c| c["categories"].as_array_mut()) else { return };
        if let Some(c) = list.iter_mut().find(|c| c["id"].as_str() == Some(id)) {
            c["votes"] = json!(votes);
            c["voted"] = json!(voted);
        }
    }

    pub fn upload_picture(&mut self, b64: &str) {
        if !self.logged_in() {
            return;
        }
        self.post("/api/profile/picture", json!({"png": b64}), Req::Picture);
    }

    // ---- the match socket

    fn ws_url(&self) -> String {
        let s = self.server.trim_end_matches('/');
        let s = if let Some(rest) = s.strip_prefix("https://") {
            format!("wss://{rest}")
        } else if let Some(rest) = s.strip_prefix("http://") {
            format!("ws://{rest}")
        } else {
            format!("ws://{s}")
        };
        format!("{s}/ws")
    }

    fn ensure_socket(&mut self, guest_name: &str) {
        if self.ws.is_none() {
            let url = if self.logged_in() || guest_name.is_empty() {
                self.ws_url()
            } else {
                format!("{}?name={}", self.ws_url(), urlencode(guest_name))
            };
            self.ws = Some(net::ws_open(&url));
            self.ws_open = false;
            self.my_id = None;
        }
    }

    fn ws_send_now(&mut self, m: Value) {
        if let (Some(id), true) = (self.ws, self.ws_open) {
            net::ws_send(id, &m.to_string());
        }
    }

    pub fn send(&mut self, m: Value) {
        self.ensure_socket("");
        match (self.ws, self.ws_open) {
            (Some(id), true) => net::ws_send(id, &m.to_string()),
            _ => self.outbox.push(m.to_string()),
        }
    }

    pub fn send_as(&mut self, guest: &str, m: Value) {
        self.ensure_socket(guest);
        self.send(m);
    }

    pub fn hello(&mut self) {
        self.send(json!({"t": "hello"}));
    }

    pub fn close_socket(&mut self) {
        if let Some(id) = self.ws.take() {
            net::ws_close(id);
        }
        self.ws_open = false;
        self.room = None;
        self.game = None;
    }

    pub fn leave_match(&mut self) {
        self.send(json!({"t": "leave"}));
        self.room = None;
        self.game = None;
    }

    pub fn is_host(&self) -> bool {
        match (&self.room, self.my_id) {
            (Some(r), Some(me)) => r["host"].as_i64() == Some(me),
            _ => false,
        }
    }

    fn on_message(&mut self, m: Value, now_ms: f64) {
        match m["t"].as_str().unwrap_or("") {
            "welcome" => self.my_id = m["id"].as_i64(),
            "error" => {
                let msg = m["msg"].as_str().unwrap_or("Error").to_string();
                self.say(msg, Tone::Bad);
            }
            "room" => {
                let back_to_lobby = m["state"] == "lobby" && self.room.as_ref().is_none_or(|r| r["state"] != "lobby");
                self.room = Some(m);
                if back_to_lobby {
                    self.game = None;
                    self.notices.push(Notice::Lobby);
                }
            }
            "start" => {
                let mut g = Game::new(8, 5, &[], 1, "apple", PyRandom::new(0));
                apply_state(&mut g, &m["state"]);
                g.prev = g.snapshot();
                self.game = Some(Match {
                    game: g,
                    you: m["you"].as_u64().unwrap_or(0) as usize,
                    tick_ms: m["tick_ms"].as_f64().unwrap_or(130.0),
                    countdown: Some(3),
                    result: None,
                    reward: None,
                    pending: Vec::new(),
                    acc: 0.0,
                });
                self.notices.push(Notice::MatchStarted);
            }
            "countdown" => {
                if let Some(g) = self.game.as_mut() {
                    let n = m["n"].as_i64().unwrap_or(0);
                    g.countdown = if n > 0 { Some(n) } else { None };
                    self.notices.push(Notice::Sound(if n > 0 { "tick" } else { "start" }));
                }
            }
            "state" => {
                if let Some(mt) = self.game.as_mut() {
                    apply_state(&mut mt.game, &m["s"]);
                    // carry over lateness so the snakes catch up smoothly (see online.py)
                    mt.acc = (mt.acc - mt.tick_ms).max(0.0).min(mt.tick_ms * 0.5);
                    predict(mt, now_ms);
                }
            }
            "over" => {
                if let Some(mt) = self.game.as_mut() {
                    mt.result = Some(m.clone());
                }
                self.notices.push(Notice::MatchOver(m));
            }
            "reward" => {
                if let Some(me) = self.me.as_mut() {
                    me["points"] = m["balance"].clone();
                    me["rp"] = m["rp"].clone();
                    me["rank"] = m["rank"].clone();
                }
                if let Some(mt) = self.game.as_mut() {
                    mt.reward = Some(m.clone());
                }
                self.notices.push(Notice::Reward(m));
                self.daily = None;
            }
            "pong" => {
                if let Some(at) = m["at"].as_f64() {
                    let rtt = (now_ms - at).max(0.0);
                    self.ping_ms = Some(self.ping_ms.map_or(rtt, |p| 0.7 * p + 0.3 * rtt));
                }
            }
            "invite" => {
                self.invite = Some((m.clone(), now_ms + 30000.0));
                self.notices.push(Notice::Invite(m));
            }
            "invited" => {
                let who = m["name"].as_str().unwrap_or("them").to_string();
                self.say(format!("Invite sent to {who}."), Tone::Good);
            }
            "friends" => self.get("/api/friends", Req::Friends),
            "dm" => {
                let from = m["from"].as_str().unwrap_or("").to_string();
                if let Some((with, msgs)) = self.conversation.as_mut() {
                    if *with == from || m["to"].as_str() == Some(with.as_str()) {
                        msgs.push(m.clone());
                    }
                }
                self.chats = None;
                self.notices.push(Notice::Dm(m));
            }
            "sold" => {
                let name = m["name"].as_str().unwrap_or("something").to_string();
                let earned = m["earned"].as_i64().unwrap_or(0);
                self.say(format!("{} bought your {name}: +{} snake points", m["buyer"].as_str().unwrap_or("Someone"),
                                 crate::cosmetics::format_points(earned)), Tone::Good);
                self.notices.push(Notice::Sound("coin"));
                self.refresh_me();
            }
            _ => {}
        }
    }

    // ---- versus turns (ported from online.py)

    pub const LAG_MARGIN_MS: f64 = 20.0;

    /// How many moves from now a turn pressed now can reach the server in time for.
    pub fn moves_ahead(&self) -> i64 {
        let Some(mt) = &self.game else { return 1 };
        let rtt = self.ping_ms.unwrap_or(120.0);
        let tick = mt.tick_ms.max(1.0);
        ((mt.acc + rtt + Self::LAG_MARGIN_MS) / tick).floor() as i64 + 1
    }

    pub fn turn(&mut self, d: Dir, now_ms: f64) -> bool {
        let ahead = self.moves_ahead();
        let Some(mt) = self.game.as_mut() else { return false };
        let g = &mt.game;
        let Some(me) = g.snakes.get(mt.you) else { return false };
        if me.dead || g.over || mt.countdown.is_some() {
            return false;
        }
        let last = mt.pending.last().map(|p| p.0).unwrap_or(me.dir);
        if d == last || d == last.opposite() || mt.pending.len() >= 2 {
            return false;
        }
        let step = match mt.pending.last() {
            Some(p) => p.2 + 1,
            None => g.tick + ahead,
        };
        mt.pending.push((d, now_ms, step));
        predict(mt, now_ms);
        self.send(json!({"t": "input", "d": d.name(), "step": step}));
        true
    }
}

/// Show our own turn on our snake as soon as the server will make it.
fn predict(mt: &mut Match, now_ms: f64) {
    let tick = mt.game.tick;
    let tick_ms = mt.tick_ms;
    let Some(me) = mt.game.snakes.get_mut(mt.you) else { return };
    if me.dead {
        mt.pending.clear();
        return;
    }
    mt.pending.retain(|&(_, at, step)| step > tick && now_ms - at < 3.0 * tick_ms + 250.0);
    if let Some(&(d, _, step)) = mt.pending.first() {
        if step == tick + 1 {
            me.planned = Some(d);
        }
    }
}

/// Load a server state into a game (the previous one becomes `prev`, for drawing in between).
pub fn apply_state(g: &mut Game, st: &Value) {
    g.prev = g.snapshot();
    let i = |v: &Value| v.as_i64().unwrap_or(0) as i32;
    g.w = i(&st["w"]).max(1);
    g.h = i(&st["h"]).max(1);
    g.tick = st["tick"].as_i64().unwrap_or(0);
    g.over = st["over"].as_bool().unwrap_or(false);
    g.winner = st["winner"].as_i64().map(|w| w as i32);
    g.started = st["started"].as_bool().unwrap_or(false);
    g.goal = i(&st["goal"]);
    g.dim = i(&st["dim"]);
    g.modes = st["modes"].as_array().map(|a| a.iter().filter_map(|m| m.as_str().map(String::from)).collect()).unwrap_or_default();
    let old_names: Vec<String> = g.snakes.iter().map(|s| s.name.clone()).collect();
    let mut snakes = Vec::new();
    for d in st["snakes"].as_array().cloned().unwrap_or_default() {
        let body: Vec<Seg> = d["b"]
            .as_array()
            .map(|b| b.iter().map(|p| Seg { x: i(&p[0]), y: i(&p[1]), hole: p[2].as_i64().unwrap_or(0) != 0 }).collect())
            .unwrap_or_default();
        if body.is_empty() {
            continue;
        }
        snakes.push(Snake {
            body,
            dir: Dir::parse(d["d"].as_str().unwrap_or("right")).unwrap_or(Dir::Right),
            queue: Vec::new(),
            planned: None,
            grow: 0,
            shield: i(&d["shield"]),
            dead: d["dead"].as_bool().unwrap_or(false),
            hole_counter: 0,
            no_interp: d["ni"].as_bool().unwrap_or(false),
            moved: false,
            team: i(&d["team"]),
            score: i(&d["score"]),
            name: d["name"].as_str().unwrap_or("").to_string(),
        });
    }
    if snakes.len() != g.snakes.len() {
        g.prev = snakes.iter().map(|s| s.body.iter().map(|p| (p.x, p.y)).collect()).collect();
    }
    let _ = old_names;
    g.teams = Some(snakes.iter().map(|s| s.team).collect());
    g.snakes = snakes;
    g.apples = st["apples"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|x| Apple {
                    x: i(&x[0]),
                    y: i(&x[1]),
                    fruit: x[2].as_str().unwrap_or("apple").to_string(),
                    group: i(&x[3]),
                    poison: x[4].as_i64().unwrap_or(0) != 0,
                    locked: x[5].as_i64().unwrap_or(0) != 0,
                })
                .collect()
        })
        .unwrap_or_default();
    g.walls = st["walls"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|w| ((i(&w[0]), i(&w[1])), Wall { x: i(&w[0]), y: i(&w[1]), kind: w[2].as_str().unwrap_or("wall").to_string(), dim: w[3].as_i64().map(|d| d as i32) }))
                .collect()
        })
        .unwrap_or_default();
    g.items = st["items"]
        .as_array()
        .map(|a| a.iter().map(|x| Item { x: i(&x[0]), y: i(&x[1]), kind: x[2].as_str().unwrap_or("key").to_string(), group: i(&x[3]) }).collect())
        .unwrap_or_default();
    for e in st["events"].as_array().cloned().unwrap_or_default() {
        g.events.push(crate::game::Event {
            kind: e[0].as_str().unwrap_or("").to_string(),
            x: i(&e[1]),
            y: i(&e[2]),
            fruit: e[3].as_str().unwrap_or("").to_string(),
            snake: e[4].as_i64().unwrap_or(-1) as i32,
        });
    }
}

pub fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
