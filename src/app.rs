//! The game: screens, navigation and playing.

use crate::audio::Sfx;
use crate::cosmetics::{catalog, default_skin, format_points, team_skin, Skin};
use crate::game::{Dir, Game};
use crate::online::{Notice, Online, Req, Tone, DEFAULT_SERVER};
use crate::pyrandom::PyRandom;
use crate::render::{draw_board, BoardColors, Effects, View};
use crate::render3d::draw_board3d;
use crate::settings::{speed_ms, Settings};
use crate::speedrun::{format_time, Category, Run};
use crate::ui::*;
use macroquad::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;

pub const TURN_CELL_MS: f64 = 85.0;
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Screen {
    Menu,
    Setup,
    Play,
    Speedrun,
    Online,
    Lobby,
    Versus,
    Profile,
    Store,
    Pass,
    Daily,
    Leaderboard,
    Packs,
    Settings,
}

/// A game being played on this computer (a round or a speedrun).
pub struct PlayState {
    pub run: Option<Run>,
    pub game: Game,
    pub acc: f64,
    pub paused: bool,
    pub over_at: Option<f64>,
    pub reported: bool,
    pub new_best: bool,
    pub result: Option<Value>,
    pub origin: Screen,
    pub pb_before: Option<i64>,
}

impl PlayState {
    pub fn game(&self) -> &Game {
        match &self.run {
            Some(r) => &r.game,
            None => &self.game,
        }
    }
    pub fn game_mut(&mut self) -> &mut Game {
        match &mut self.run {
            Some(r) => &mut r.game,
            None => &mut self.game,
        }
    }
}

/// The loot box opening (or a battle pass reward) being shown.
pub struct Reveal {
    pub data: Value,
    pub started: f64,
    pub reel: Vec<String>,
    pub crate_id: String,
}

pub struct App {
    pub ui: Ui,
    pub settings: Settings,
    pub online: Online,
    pub sfx: Sfx,
    pub fx: Effects,
    pub screen: Screen,
    pub entered: f64,
    pub play: Option<PlayState>,
    pub tabs: HashMap<Screen, usize>,
    pub fields: HashMap<&'static str, String>,
    pub reveal: Option<Reveal>,
    pub selected: Option<String>,
    pub selling: Option<String>,
    pub viewing: Option<String>,
    pub swipe: Option<Vec2>,
    pub shot: Option<(String, String, i32)>,
    pub quit: bool,
    pub intro: f64,
    /// (screenshots: hold the game half way between two moves)
    pub frozen: bool,
}

impl App {
    pub async fn new() -> App {
        let regular = load_ttf_font_from_bytes(include_bytes!("../assets/fonts/Nunito-SemiBold.ttf")).unwrap();
        let bold = load_ttf_font_from_bytes(include_bytes!("../assets/fonts/Nunito-ExtraBold.ttf")).unwrap();
        let mut settings = Settings::load();
        // On the web, the game talks to the site it was loaded from.
        if let Some(origin) = crate::net::page_origin() {
            settings.server = origin;
        }
        if settings.server.is_empty() {
            settings.server = DEFAULT_SERVER.into();
        }
        let mut online = Online::new(settings.server.clone(), settings.token.clone(), settings.user.clone());
        online.check_server();
        let mut sfx = Sfx::load().await;
        sfx.volume = settings.volume;
        let shot = std::env::var("OPENSNAKE_SHOT").ok().and_then(|s| {
            let mut p = s.splitn(2, ':');
            Some((p.next()?.to_string(), p.next()?.to_string(), 0))
        });
        let mut app = App {
            ui: Ui::new(regular, bold),
            settings,
            online,
            sfx,
            fx: Effects::default(),
            screen: Screen::Menu,
            entered: 0.0,
            play: None,
            tabs: HashMap::new(),
            fields: HashMap::new(),
            reveal: None,
            selected: None,
            selling: None,
            viewing: None,
            swipe: None,
            shot,
            quit: false,
            intro: get_time(),
            frozen: false,
        };
        if let Some((name, _, _)) = app.shot.clone() {
            app.open_named(&name);
        }
        app
    }

    pub fn now_ms(&self) -> f64 {
        self.ui.time * 1000.0
    }

    pub fn field(&mut self, key: &'static str) -> &mut String {
        self.fields.entry(key).or_default()
    }

    pub fn tab(&self, s: Screen) -> usize {
        *self.tabs.get(&s).unwrap_or(&0)
    }

    pub fn save(&mut self) {
        self.settings.token = self.online.token.clone();
        self.settings.user = self.online.user.clone();
        self.settings.server = self.online.server.clone();
        self.settings.save();
    }

    pub fn go(&mut self, s: Screen) {
        if self.screen != s {
            self.screen = s;
            self.entered = self.ui.time;
            self.ui.focus = None;
            self.sfx.play("click");
            self.on_enter(s);
        }
    }

    pub fn set_tab(&mut self, s: Screen, t: usize) {
        if self.tab(s) != t {
            self.tabs.insert(s, t);
            self.entered = self.ui.time - 0.15;
            self.ui.focus = None;
            self.sfx.play("click");
            self.on_enter(s);
        }
    }

    /// Fetch what a screen shows.
    pub fn on_enter(&mut self, s: Screen) {
        let o = &mut self.online;
        match s {
            Screen::Speedrun => {
                o.get("/api/records", Req::Records);
                o.get("/api/weekly", Req::Weekly);
                o.community_path.clear(); // (fetched again, with the search, by the Community tab)
            }
            Screen::Leaderboard => o.get("/api/rankings", Req::Rankings),
            Screen::Pass if o.logged_in() => o.get("/api/battlepass", Req::Pass),
            Screen::Daily if o.logged_in() => o.get("/api/daily", Req::Daily),
            Screen::Store => {
                o.refresh_me();
                o.get("/api/market", Req::Market);
            }
            Screen::Profile if o.logged_in() => {
                o.refresh_me();
                o.get("/api/friends", Req::Friends);
                o.get("/api/messages", Req::Chats);
            }
            Screen::Online => o.get("/api/matches", Req::Matches),
            _ => {}
        }
    }

    /// For screenshots: open a screen by name.
    pub fn open_named(&mut self, name: &str) {
        let (screen, tab) = match name {
            "menu" => (Screen::Menu, 0),
            "setup" => (Screen::Setup, 0),
            "speedrun" => (Screen::Speedrun, 0),
            "weekly" => (Screen::Speedrun, 1),
            "community" => (Screen::Speedrun, 2),
            "community_search" => {
                self.fields.insert("comm_q", "gentastic".into());
                (Screen::Speedrun, 2)
            }
            "online" => (Screen::Online, 0),
            "profile" => (Screen::Profile, 0),
            "friends" => (Screen::Profile, 1),
            "messages" => (Screen::Profile, 2),
            "store" => (Screen::Store, 0),
            "crates" => (Screen::Store, 1),
            "market" => (Screen::Store, 2),
            "collection" => (Screen::Store, 3),
            "pass" => (Screen::Pass, 0),
            "daily" => (Screen::Daily, 0),
            "leaderboard" => (Screen::Leaderboard, 0),
            "packs" => (Screen::Packs, 0),
            "settings" => (Screen::Settings, 0),
            "play" | "play3d" => {
                self.settings.view3d = name == "play3d";
                self.settings.modes = vec!["wall".into(), "key".into(), "shield".into()];
                self.settings.fruit_count = 3;
                self.start_round();
                for d in [Dir::Up, Dir::Right] {
                    if let Some(p) = self.play.as_mut() {
                        p.game_mut().input(d);
                    }
                }
                return;
            }
            "reel" | "reveal" => {
                // a loot box opening, part way through (or done)
                self.screen = Screen::Store;
                self.tabs.insert(Screen::Store, 1);
                let item = "skin:theworld".to_string();
                let reel = crate::screens_shop::make_reel(&item, "royal");
                let back = if name == "reveal" { 6.0 } else { 3.0 };
                self.reveal = Some(Reveal { data: json!({"item": item, "new": true}), started: get_time() - back + 1.5, reel,
                                            crate_id: "royal".into() });
                return;
            }
            "smooth" | "smooth3d" => {
                // a long snake winding round corners, frozen between two moves
                self.settings.view3d = name == "smooth3d";
                self.settings.modes = vec![];
                self.settings.board_w = 17;
                self.settings.board_h = 15;
                self.start_round();
                let path: Vec<(i32, i32)> = vec![(9, 4), (8, 4), (7, 4), (6, 4), (5, 4), (4, 4), (3, 4), (3, 5), (3, 6), (3, 7),
                    (4, 7), (5, 7), (6, 7), (7, 7), (8, 7), (9, 7), (10, 7), (10, 8), (10, 9), (10, 10), (9, 10), (8, 10),
                    (7, 10), (6, 10), (5, 10), (4, 10), (3, 10)];
                if let Some(p) = self.play.as_mut() {
                    let g = p.game_mut();
                    g.started = true;
                    g.snakes[0].body = path[1..].iter().map(|&(x, y)| crate::game::Seg::new(x, y)).collect();
                    g.snakes[0].body.insert(0, crate::game::Seg::new(path[0].0, path[0].1));
                    g.snakes[0].dir = Dir::Right;
                    // where it was before this move: one cell further back, tail included
                    let mut prev: Vec<(i32, i32)> = path[1..].to_vec();
                    prev.push((2, 10));
                    g.prev = vec![prev];
                    g.snakes[0].body.pop();
                    g.snakes[0].body.insert(0, crate::game::Seg::new(10, 4));
                    let mut pv = path.clone();
                    pv.pop();
                    g.prev = vec![pv];
                }
                self.frozen = true;
                return;
            }
            "run" => {
                let cat = catalog().categories[0].clone();
                self.start_run(cat, Screen::Speedrun);
                return;
            }
            _ => (Screen::Menu, 0),
        };
        self.screen = screen;
        self.tabs.insert(screen, tab);
        self.on_enter(screen);
    }

    // ---- the frame

    pub fn frame(&mut self) {
        self.ui.begin();
        let now = self.now_ms();
        self.online.poll(now);
        self.handle_notices();
        self.update();
        clear_background(BG_BOTTOM);
        self.ui.background();
        let enter = ((self.ui.time - self.entered) as f32 * 5.0).min(1.0);
        match self.screen {
            Screen::Menu => self.menu(),
            Screen::Setup => self.setup_screen(),
            Screen::Play => self.play_screen(),
            Screen::Speedrun => self.speedrun_screen(),
            Screen::Online => self.online_screen(),
            Screen::Lobby => self.lobby_screen(),
            Screen::Versus => self.versus_screen(),
            Screen::Profile => self.profile_screen(),
            Screen::Store => self.store_screen(),
            Screen::Pass => self.pass_screen(),
            Screen::Daily => self.daily_screen(),
            Screen::Leaderboard => self.leaderboard_screen(),
            Screen::Packs => self.packs_screen(),
            Screen::Settings => self.settings_screen(),
        }
        // a quick fade as a new screen comes in
        if enter < 1.0 {
            draw_rectangle(0.0, 0.0, self.ui.w, self.ui.h, alpha(BG_BOTTOM, (1.0 - enter) * 0.6));
        }
        self.overlays();
        self.ui.end();
        use macroquad::miniquad::{window::set_mouse_cursor, CursorIcon};
        set_mouse_cursor(if self.ui.hand { CursorIcon::Pointer } else { CursorIcon::Default });
        self.screenshot();
    }

    fn screenshot(&mut self) {
        let Some((name, path, n)) = self.shot.as_mut() else { return };
        *n += 1;
        if *n == std::env::var("OPENSNAKE_SHOT_FRAMES").ok().and_then(|f| f.parse().ok()).unwrap_or(90) {
            let _ = name;
            let mut img = get_screen_data();
            for px in img.bytes.chunks_mut(4) {
                px[3] = 255; // (what's on screen is opaque, whatever the alpha channel says)
            }
            img.export_png(path);
            self.quit = true;
        }
    }

    fn handle_notices(&mut self) {
        let notices = std::mem::take(&mut self.online.notices);
        for n in notices {
            match n {
                Notice::Say(text, tone) => {
                    let c = match tone {
                        Tone::Good => ACCENT,
                        Tone::Bad => C_RED,
                        Tone::Info => C_BLUE,
                    };
                    self.ui.toast(text, c);
                }
                Notice::Sound(s) => self.sfx.play(s),
                Notice::CrateOpened(d) => {
                    let item = d["item"].as_str().unwrap_or("").to_string();
                    let crate_id = d["tier"]["crate"].as_str().or(d["crate"].as_str()).unwrap_or("").to_string();
                    let reel = crate::screens_shop::make_reel(&item, &crate_id);
                    self.reveal = Some(Reveal { data: d, started: self.ui.time, reel, crate_id });
                }
                Notice::RunResult(d) => {
                    if let Some(p) = self.play.as_mut() {
                        p.result = Some(d.clone());
                    }
                    if d["wr"].as_bool() == Some(true) {
                        self.sfx.play("reveal");
                    }
                    if let Some(e) = d["error"].as_str() {
                        self.ui.toast(e.to_string(), C_RED);
                    } else if d["valid"] == false {
                        self.ui.toast(d["message"].as_str().unwrap_or("The server didn't accept the run.").to_string(), C_RED);
                    }
                }
                Notice::Reward(m) => {
                    let pts = m["points"].as_i64().unwrap_or(0);
                    let mut text = format!("+{} snake points", format_points(pts));
                    if m["ranked"].as_bool() == Some(true) {
                        let ch = m["rp_change"].as_i64().unwrap_or(0);
                        text += &format!("   {} ({}{} RP)", m["rank"].as_str().unwrap_or(""), if ch >= 0 { "+" } else { "" }, ch);
                    }
                    self.ui.toast(text, C_GOLD);
                    for up in m["rank_ups"].as_array().cloned().unwrap_or_default() {
                        let rewards: Vec<String> = up["rewards"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
                        self.ui.toast(format!("New rank: {}! {}", up["rank"].as_str().unwrap_or(""), rewards.join(", ")), C_PURPLE);
                        self.sfx.play("reveal");
                    }
                }
                Notice::MatchStarted => {
                    self.fx = Effects::default();
                    self.go(Screen::Versus);
                }
                Notice::MatchOver(m) => {
                    let mine = self.online.game.as_ref().and_then(|mt| mt.game.snakes.get(mt.you).map(|s| s.team));
                    let won = mine.is_some() && m["winner"].as_i64().map(|w| w as i32) == mine;
                    self.sfx.play(if won { "win" } else { "die" });
                }
                Notice::Lobby => {
                    if self.screen == Screen::Versus || self.screen == Screen::Online {
                        self.go(Screen::Lobby);
                    }
                }
                Notice::LeftMatch => {
                    if matches!(self.screen, Screen::Lobby | Screen::Versus) {
                        self.go(Screen::Online);
                    }
                }
                Notice::Invite(m) => {
                    self.sfx.play("start");
                    let _ = m;
                }
                Notice::Dm(m) => {
                    let from = m["from"].as_str().unwrap_or("").to_string();
                    let me = self.online.user.clone().unwrap_or_default();
                    let reading = self.screen == Screen::Profile && self.tab(Screen::Profile) == 2
                        && self.online.conversation.as_ref().is_some_and(|(w, _)| *w == from);
                    if from != me && !reading {
                        self.ui.toast(format!("{from}: {}", m["body"].as_str().unwrap_or("")), C_BLUE);
                        self.sfx.play("unlock");
                    }
                }
            }
        }
        // remember a new login
        if self.settings.token != self.online.token {
            self.save();
        }
    }

    // ---- playing

    pub fn start_round(&mut self) {
        let s = &self.settings;
        let modes: Vec<&str> = s.modes.iter().map(|m| m.as_str()).collect();
        let seed = (rand::rand() as i64) & 0x7fff_ffff;
        let game = Game::new(s.board_w, s.board_h, &modes, s.fruit_count, &s.fruit, PyRandom::new(seed));
        self.play = Some(PlayState { run: None, game, acc: 0.0, paused: false, over_at: None, reported: false,
                                     new_best: false, result: None, origin: Screen::Setup, pb_before: None });
        self.fx = Effects::default();
        self.go(Screen::Play);
    }

    pub fn start_run(&mut self, cat: Category, origin: Screen) {
        let seed = (rand::rand() as i64) & 0x7fff_ffff;
        let pb = self.settings.pbs.get(&cat.id).copied();
        let run = Run::new(cat, seed);
        let game = run.game.clone();
        self.play = Some(PlayState { run: Some(run), game, acc: 0.0, paused: false, over_at: None, reported: false,
                                     new_best: false, result: None, origin, pb_before: pb });
        self.fx = Effects::default();
        self.go(Screen::Play);
    }

    pub fn restart(&mut self) {
        let Some(p) = &self.play else { return };
        let origin = p.origin;
        match &p.run {
            Some(r) => {
                let cat = r.category.clone();
                self.start_run(cat, origin);
            }
            None => self.start_round(),
        }
        self.screen = Screen::Play;
    }

    fn tick_ms(&self) -> f64 {
        match &self.play {
            Some(p) => match &p.run {
                Some(r) => r.tick_ms(),
                None => speed_ms(self.settings.speed),
            },
            None => 130.0,
        }
    }

    pub fn steer(&mut self, d: Dir) {
        match self.screen {
            Screen::Play => {
                let Some(p) = self.play.as_mut() else { return };
                if p.paused || p.game().over {
                    return;
                }
                let before: usize = p.game().snakes.iter().map(|s| s.queue.len()).sum();
                match &mut p.run {
                    Some(r) => r.input(d),
                    None => p.game.input(d),
                }
                let after: usize = p.game().snakes.iter().map(|s| s.queue.len()).sum();
                if after > before {
                    self.sfx.play("turn");
                }
            }
            Screen::Versus => {
                let now = self.now_ms();
                if self.online.turn(d, now) {
                    self.sfx.play("turn");
                }
            }
            _ => {}
        }
    }

    fn read_controls(&mut self) {
        if self.ui.focus.is_some() {
            return;
        }
        for k in self.ui.keys.clone() {
            let d = match k {
                KeyCode::Up | KeyCode::W => Some(Dir::Up),
                KeyCode::Down | KeyCode::S => Some(Dir::Down),
                KeyCode::Left | KeyCode::A => Some(Dir::Left),
                KeyCode::Right | KeyCode::D => Some(Dir::Right),
                _ => None,
            };
            if let Some(d) = d {
                self.steer(d);
            }
        }
        // swipes (touch screens)
        if self.ui.clicked && self.swipe.is_none() {
            self.swipe = Some(self.ui.mouse);
        }
        if let Some(start) = self.swipe {
            let moved = self.ui.mouse - start;
            if moved.length() > 28.0 * self.ui.u {
                let d = if moved.x.abs() > moved.y.abs() {
                    if moved.x > 0.0 { Dir::Right } else { Dir::Left }
                } else if moved.y > 0.0 {
                    Dir::Down
                } else {
                    Dir::Up
                };
                self.steer(d);
                self.swipe = if self.ui.down { Some(self.ui.mouse) } else { None };
            } else if !self.ui.down {
                self.swipe = None;
            }
        }
    }

    fn update(&mut self) {
        let dt = self.ui.dt as f64 * 1000.0;
        self.fx.update(self.ui.dt);
        match self.screen {
            Screen::Play => {
                self.read_controls();
                let ms = self.tick_ms();
                let Some(p) = self.play.as_mut() else { return };
                if self.frozen {
                    p.acc = ms * 0.5;
                    return;
                }
                let g = p.game();
                if p.paused || !g.started || g.over {
                    self.drain();
                    return;
                }
                // While a turn waits, glide through the cell at the Fast speed (see the Python game).
                let waiting = g.snakes.iter().any(|s| !s.queue.is_empty() || s.planned.is_some());
                let boost = if ms > TURN_CELL_MS && waiting { ms / TURN_CELL_MS } else { 1.0 };
                p.acc += dt * boost;
                while p.acc >= ms {
                    p.acc -= ms;
                    match &mut p.run {
                        Some(r) => r.step(),
                        None => p.game.step(),
                    }
                    if p.game().over {
                        break;
                    }
                }
                if p.acc >= ms / 2.0 {
                    p.game_mut().plan(); // the head is entering its cell: lock the turn so it can curve
                }
                self.drain();
            }
            Screen::Versus => {
                self.read_controls();
                if let Some(mt) = self.online.game.as_mut() {
                    mt.acc = (mt.acc + dt).min(mt.tick_ms * 1.5);
                }
                self.drain();
            }
            _ => {}
        }
    }

    /// Sounds and effects for what just happened in the game.
    fn drain(&mut self) {
        let versus = self.screen == Screen::Versus;
        let events = if versus {
            match self.online.game.as_mut() {
                Some(mt) => std::mem::take(&mut mt.game.events),
                None => return,
            }
        } else {
            match self.play.as_mut() {
                Some(p) => std::mem::take(&mut p.game_mut().events),
                None => return,
            }
        };
        let you = self.online.game.as_ref().map(|m| m.you as i32);
        let mut finished = false;
        for ev in events {
            self.sfx.play(&ev.kind);
            match ev.kind.as_str() {
                "eat" => self.fx.burst(ev.x, ev.y, crate::render::fruit_color(&ev.fruit), 14),
                "portal" => self.fx.burst(ev.x, ev.y, rgb(0, 162, 255), 18),
                "poison" => self.fx.burst(ev.x, ev.y, rgb(125, 47, 191), 18),
                "shield" => self.fx.burst(ev.x, ev.y, rgb(47, 128, 237), 20),
                "die" if versus => {
                    self.fx.burst(ev.x, ev.y, rgb(90, 90, 90), 22);
                    if Some(ev.snake) == you {
                        self.fx.shake = 0.4;
                    }
                }
                "die" | "win" if !versus => {
                    if ev.kind == "die" {
                        self.fx.shake = 0.4;
                    }
                    finished = true;
                }
                _ => {}
            }
        }
        if finished {
            self.finish_play();
        }
    }

    fn finish_play(&mut self) {
        let now = self.ui.time;
        let name = self.online.user.clone().unwrap_or_default();
        let logged_in = self.online.logged_in();
        let Some(p) = self.play.as_mut() else { return };
        if p.reported {
            return;
        }
        p.reported = true;
        p.over_at = Some(now);
        match &p.run {
            Some(r) => {
                if r.finished {
                    let t = r.time_ms();
                    let id = r.category.id.clone();
                    if self.settings.pbs.get(&id).is_none_or(|&pb| t < pb) {
                        self.settings.pbs.insert(id, t);
                        p.new_best = true;
                    }
                    if logged_in {
                        let date = iso_now();
                        let file = r.to_json(&name, VERSION, &date);
                        self.online.post("/api/runs", file, Req::SubmitRun);
                    }
                    self.settings.save();
                }
            }
            None => {
                let score = p.game.score;
                let key = format!("{}x{} {} {}", self.settings.board_w, self.settings.board_h,
                                  catalog().mode_label(&self.settings.modes), self.settings.fruit_count);
                if score > *self.settings.best.get(&key).unwrap_or(&0) {
                    self.settings.best.insert(key, score);
                    p.new_best = score > 0;
                    self.settings.save();
                }
                if logged_in {
                    self.online.post("/api/rounds", json!({"fruit": score}), Req::Round);
                }
            }
        }
    }

    /// The skins to draw the snakes with.
    pub fn my_skin(&self) -> Skin {
        let id = self.online.equipped_skin();
        catalog().skin(&id).cloned().unwrap_or_else(default_skin)
    }

    pub fn board_colors(&self) -> BoardColors {
        if self.settings.pack.is_empty() {
            return BoardColors::default();
        }
        catalog().pack_colors.get(&self.settings.pack).map(BoardColors::from_pack).unwrap_or_default()
    }

    // ---- shared layout

    /// The frame of every page: a header with a back button, the title and your points.
    /// Returns the content area.
    pub fn page(&mut self, title: &str, icon: &str) -> Rect {
        let u = self.ui.u;
        let w = self.ui.w;
        let top = 16.0 * u;
        let hh = 52.0 * u;
        if self.ui.button_icon("", "back", Rect::new(20.0 * u, top, hh, hh), Style::Secondary) || self.ui.hotkey(KeyCode::Escape) {
            self.go(Screen::Menu);
        }
        let tx = 20.0 * u + hh + 16.0 * u;
        draw_icon(icon, tx + 14.0 * u, top + hh / 2.0, 28.0 * u, ACCENT);
        self.ui.text(title, tx + 36.0 * u, top + hh / 2.0, 30.0, INK, Align::Left, true);
        self.points_chip(w - 20.0 * u, top + hh / 2.0);
        let max_w = 1180.0 * u;
        let cw = (w - 40.0 * u).min(max_w);
        Rect::new((w - cw) / 2.0, top + hh + 18.0 * u, cw, self.ui.h - (top + hh + 18.0 * u) - 20.0 * u)
    }

    /// Your snake points, top right (or a login hint).
    pub fn points_chip(&mut self, right: f32, cy: f32) {
        let u = self.ui.u;
        if self.online.logged_in() {
            let text = format_points(self.online.points());
            let w = self.ui.measure(&text, 18.0, true) + 52.0 * u;
            let r = Rect::new(right - w, cy - 20.0 * u, w, 40.0 * u);
            rounded(r, r.h / 2.0, alpha(BLACK, 0.3));
            rounded_outline(r, r.h / 2.0, 1.0, alpha(C_GOLD, 0.35));
            draw_icon("coin", r.x + 22.0 * u, cy, 22.0 * u, C_GOLD);
            self.ui.text(&text, r.x + 38.0 * u, cy, 18.0, C_GOLD, Align::Left, true);
        }
    }

    /// A message in the middle of an empty area, with an optional button.
    pub fn empty_state(&mut self, area: Rect, icon: &str, title: &str, body: &str, button: Option<&str>) -> bool {
        let u = self.ui.u;
        let cy = area.y + area.h * 0.42;
        draw_circle(area.x + area.w / 2.0, cy - 60.0 * u, 42.0 * u, alpha(ACCENT, 0.12));
        draw_icon(icon, area.x + area.w / 2.0, cy - 60.0 * u, 40.0 * u, ACCENT);
        self.ui.text(title, area.x + area.w / 2.0, cy + 6.0 * u, 24.0, INK, Align::Center, true);
        self.ui.text(body, area.x + area.w / 2.0, cy + 38.0 * u, 16.0, MUTED, Align::Center, false);
        if let Some(b) = button {
            let r = Rect::new(area.x + area.w / 2.0 - 110.0 * u, cy + 66.0 * u, 220.0 * u, 48.0 * u);
            return self.ui.button(b, r, Style::Primary);
        }
        false
    }

    /// Pages that need an account show this when you're logged out.
    pub fn need_login(&mut self, area: Rect, what: &str) -> bool {
        if self.online.logged_in() {
            return false;
        }
        if self.empty_state(area, "user", "Log in to continue", what, Some("Log in or sign up")) {
            self.tabs.insert(Screen::Online, 0);
            self.go(Screen::Online);
        }
        true
    }

    // ---- the game screen

    fn play_screen(&mut self) {
        let u = self.ui.u;
        let (w, h) = (self.ui.w, self.ui.h);
        let Some(p) = &self.play else {
            self.go(Screen::Menu);
            return;
        };
        let ms = self.tick_ms();
        let g = p.game();
        let t = if g.started && !g.over && !p.paused { (p.acc / ms) as f32 } else { 1.0 };
        let hud_h = 64.0 * u;
        let area = Rect::new(16.0 * u, hud_h + 12.0 * u, w - 32.0 * u, h - hud_h - 28.0 * u);
        let skins = vec![self.my_skin(); g.snakes.len().max(1)];
        let view = View { game: g, t, skins: &skins, colors: self.board_colors(), now_ms: self.now_ms(),
                          reveal: g.over, names: false, you: None };
        if self.settings.view3d {
            draw_board3d(&view, area, &self.fx);
        } else {
            draw_board(&view, area, &self.fx);
        }
        // HUD
        let score = g.score;
        let started = g.started;
        let over = g.over;
        rounded(Rect::new(0.0, 0.0, w, hud_h), 0.0, alpha(BG_TOP, 0.92));
        draw_line(0.0, hud_h, w, hud_h, 1.0, alpha(LINE, 0.8));
        let cy = hud_h / 2.0;
        if self.ui.button_icon("", "back", Rect::new(14.0 * u, cy - 20.0 * u, 40.0 * u, 40.0 * u), Style::Ghost) {
            self.leave_play();
            return;
        }
        let p = self.play.as_ref().unwrap();
        draw_fruit_icon(70.0 * u, cy, 26.0 * u);
        self.ui.text(&score.to_string(), 92.0 * u, cy, 28.0, INK, Align::Left, true);
        match &p.run {
            Some(r) => {
                let live = r.elapsed + if started && !over && !p.paused { p.acc.min(r.tick_ms()) } else { 0.0 };
                self.ui.text(&format_time(live), w / 2.0, cy, 30.0, INK, Align::Center, true);
                let goal = r.category.goal.map_or("Fill the board".to_string(), |g| format!("{} / {g}", r.game.score));
                self.ui.text(&format!("{}   ·   {goal}", r.category.name), 160.0 * u, cy, 16.0, MUTED, Align::Left, false);
                let pb = p.pb_before.map_or("--".into(), |t| format_time(t as f64));
                let wr = self.online.records.get(&r.category.id).and_then(|w| w["time_ms"].as_f64()).map_or("--".into(), format_time);
                self.ui.text(&format!("PB {pb}   WR {wr}"), w - 20.0 * u, cy, 16.0, MUTED, Align::Right, true);
            }
            None => {
                let key = format!("{}x{} {} {}", self.settings.board_w, self.settings.board_h,
                                  catalog().mode_label(&self.settings.modes), self.settings.fruit_count);
                let best = *self.settings.best.get(&key).unwrap_or(&0);
                draw_icon("trophy", 160.0 * u, cy, 24.0 * u, C_GOLD);
                self.ui.text(&best.max(score).to_string(), 180.0 * u, cy, 24.0, C_GOLD, Align::Left, true);
                self.ui.text(&catalog().mode_label(&self.settings.modes), w / 2.0, cy, 18.0, MUTED, Align::Center, true);
                let view = if self.settings.view3d { "2D view" } else { "3D view" };
                if self.ui.button_icon(view, "cube", Rect::new(w - 150.0 * u, cy - 18.0 * u, 136.0 * u, 36.0 * u), Style::Ghost) {
                    self.settings.view3d = !self.settings.view3d;
                    self.settings.save();
                }
            }
        }
        // keys
        if self.ui.hotkey(KeyCode::R) {
            self.restart();
            return;
        }
        // Esc leaves the game, back to where it was started from (P or Space pause)
        if self.ui.hotkey(KeyCode::Escape) {
            self.leave_play();
            return;
        }
        if (self.ui.hotkey(KeyCode::P) || self.ui.hotkey(KeyCode::Space)) && started && !over {
            if let Some(p) = self.play.as_mut() {
                p.paused = !p.paused;
            }
        }
        // cards over the board
        let p = self.play.as_ref().unwrap();
        if !started {
            let lines = match &p.run {
                Some(r) => vec![r.category.name.clone(), r.category.desc.clone()],
                None => vec!["Ready?".into(), catalog().mode_label(&self.settings.modes)],
            };
            self.overlay_card(&lines[0], &lines[1], "Press an arrow key (or swipe) to start  ·  Esc to go back", &[]);
        } else if p.paused {
            if let Some(a) = self.overlay_card("Paused", "The speedrun timer only counts moves, so pausing costs nothing.", "",
                                               &["Resume", "Restart", "Quit"]) {
                match a {
                    0 => self.play.as_mut().unwrap().paused = false,
                    1 => self.restart(),
                    _ => self.leave_play(),
                }
            }
        } else if over && p.over_at.is_some_and(|t| self.ui.time - t > 0.6) {
            self.game_over_card();
        }
    }

    fn game_over_card(&mut self) {
        let p = self.play.as_ref().unwrap();
        let (title, sub, extra) = match &p.run {
            Some(r) if r.finished => {
                let t = r.time_ms();
                let mut extra = String::new();
                if let Some(res) = &p.result {
                    if res["valid"] == true {
                        let rank = res["rank"].as_i64().map(|r| format!("#{r} on the leaderboard")).unwrap_or_default();
                        let earned = res["earned"].as_i64().unwrap_or(0);
                        extra = if res["wr"] == true { "NEW WORLD RECORD!".into() } else { rank };
                        if earned > 0 {
                            extra += &format!("   +{} snake points", format_points(earned));
                        }
                    }
                } else if self.online.logged_in() {
                    extra = "Checking your run with the server...".into();
                } else {
                    extra = "Log in to post your times to the leaderboards.".into();
                }
                let pb = if p.new_best { "New personal best!".to_string() } else {
                    p.pb_before.map_or(String::new(), |b| format!("PB {}  ({})", format_time(b as f64), crate::speedrun::format_delta((t - b) as f64)))
                };
                (format_time(t as f64), pb, extra)
            }
            Some(r) => ("Run over".to_string(), format!("{} / {} fruit", r.game.score, r.category.goal.unwrap_or(0)), "Press R to try again".into()),
            None => {
                let g = &p.game;
                let title = if g.won { "You win!".to_string() } else { format!("{} fruit", g.score) };
                (title, if p.new_best { "New best!".into() } else { String::new() }, String::new())
            }
        };
        if let Some(a) = self.overlay_card(&title, &sub, &extra, &["Play again", "Back"]) {
            if a == 0 {
                self.restart();
            } else {
                self.leave_play();
            }
        }
        if self.ui.hotkey(KeyCode::Enter) {
            self.restart();
        }
    }

    fn leave_play(&mut self) {
        let origin = self.play.as_ref().map_or(Screen::Menu, |p| p.origin);
        self.play = None;
        self.go(origin);
    }

    /// A card over the board; returns which button was pressed.
    pub fn overlay_card(&mut self, title: &str, sub: &str, extra: &str, buttons: &[&str]) -> Option<usize> {
        let u = self.ui.u;
        let (w, h) = (self.ui.w, self.ui.h);
        let cw = (520.0 * u).min(w - 32.0 * u);
        let ch = if buttons.is_empty() { 170.0 } else { 250.0 } * u;
        let r = Rect::new((w - cw) / 2.0, (h - ch) / 2.0, cw, ch);
        let appear = self.ui.anim(id_of(&("card", title)), 1.0, 10.0);
        let r = Rect::new(r.x, r.y + (1.0 - appear) * 20.0 * u, r.w, r.h);
        self.ui.card(r);
        self.ui.text(title, r.x + cw / 2.0, r.y + 46.0 * u, 34.0, INK, Align::Center, true);
        if !sub.is_empty() {
            let s = self.ui.fit(sub, 17.0, false, cw - 40.0 * u);
            self.ui.text(&s, r.x + cw / 2.0, r.y + 86.0 * u, 17.0, MUTED, Align::Center, false);
        }
        if !extra.is_empty() {
            let s = self.ui.fit(extra, 17.0, true, cw - 40.0 * u);
            let col = if extra.contains("WORLD") { C_GOLD } else { ACCENT };
            self.ui.text(&s, r.x + cw / 2.0, r.y + 118.0 * u, 17.0, col, Align::Center, true);
        }
        let n = buttons.len();
        let mut out = None;
        if n > 0 {
            let bw = (cw - 40.0 * u - (n as f32 - 1.0) * 12.0 * u) / n as f32;
            for (i, b) in buttons.iter().enumerate() {
                let br = Rect::new(r.x + 20.0 * u + i as f32 * (bw + 12.0 * u), r.y + ch - 70.0 * u, bw, 50.0 * u);
                if self.ui.button(b, br, if i == 0 { Style::Primary } else { Style::Secondary }) {
                    out = Some(i);
                }
            }
        }
        out
    }

    // ---- versus

    fn versus_screen(&mut self) {
        let u = self.ui.u;
        let (w, h) = (self.ui.w, self.ui.h);
        let Some(mt) = &self.online.game else {
            self.go(if self.online.room.is_some() { Screen::Lobby } else { Screen::Online });
            return;
        };
        let g = &mt.game;
        let t = if g.over { 1.0 } else { (mt.acc / mt.tick_ms) as f32 };
        let hud_h = 64.0 * u;
        let area = Rect::new(16.0 * u, hud_h + 12.0 * u, w - 32.0 * u, h - hud_h - 28.0 * u);
        let my_team = g.snakes.get(mt.you).map_or(0, |s| s.team);
        let mut count = [0usize; 2];
        let skins: Vec<Skin> = g
            .snakes
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == mt.you {
                    return self.my_skin();
                }
                let k = count[(s.team.clamp(0, 1)) as usize];
                count[(s.team.clamp(0, 1)) as usize] += 1;
                team_skin(if s.team == my_team { 0 } else { 1 }, k + if s.team == my_team { 1 } else { 0 })
            })
            .collect();
        let view = View { game: g, t, skins: &skins, colors: self.board_colors(), now_ms: self.now_ms(),
                          reveal: g.over, names: true, you: Some(mt.you) };
        if self.settings.view3d {
            draw_board3d(&view, area, &self.fx);
        } else {
            draw_board(&view, area, &self.fx);
        }
        rounded(Rect::new(0.0, 0.0, w, hud_h), 0.0, alpha(BG_TOP, 0.92));
        let cy = hud_h / 2.0;
        let (a, b) = (g.team_score(my_team), g.team_score(1 - my_team));
        self.ui.text(&format!("You {a}"), w / 2.0 - 20.0 * u, cy, 26.0, C_BLUE, Align::Right, true);
        self.ui.text("vs", w / 2.0, cy, 16.0, MUTED, Align::Center, false);
        self.ui.text(&format!("{b} Them"), w / 2.0 + 20.0 * u, cy, 26.0, rgb(255, 160, 90), Align::Left, true);
        if g.goal > 0 {
            self.ui.text(&format!("first to {}", g.goal), w / 2.0, cy + 20.0 * u, 12.0, FAINT, Align::Center, false);
        }
        if let Some(p) = self.online.ping_ms {
            self.ui.text(&format!("ping {p:.0} ms"), w - 20.0 * u, cy, 15.0, MUTED, Align::Right, false);
        }
        let ranked = self.online.room.as_ref().is_some_and(|r| r["ranked"] != false);
        self.ui.text(if ranked { "Ranked" } else { "Unranked" }, 70.0 * u, cy, 16.0, if ranked { C_GOLD } else { MUTED }, Align::Left, true);
        if self.ui.button_icon("", "back", Rect::new(14.0 * u, cy - 20.0 * u, 40.0 * u, 40.0 * u), Style::Ghost)
            || self.ui.hotkey(KeyCode::Escape)
        {
            self.online.leave_match();
            self.go(Screen::Online);
            return;
        }
        let mt = self.online.game.as_ref().unwrap();
        if let Some(n) = mt.countdown {
            let k = (self.ui.time.fract() as f32).min(1.0);
            let size = 120.0 * (1.2 - 0.2 * k);
            self.ui.text(&n.to_string(), w / 2.0 + 3.0 * u, h / 2.0 + 4.0 * u, size, alpha(BLACK, 0.4), Align::Center, true);
            self.ui.text(&n.to_string(), w / 2.0, h / 2.0, size, WHITE, Align::Center, true);
        }
        if let Some(res) = mt.result.clone() {
            let won = res["winner"].as_i64().map(|x| x as i32) == Some(my_team);
            let draw = res["winner"].is_null();
            let title = if draw { "Draw" } else if won { "Victory!" } else { "Defeat" };
            let reward = mt.reward.as_ref().map(|r| {
                let mut s = format!("+{} snake points", format_points(r["points"].as_i64().unwrap_or(0)));
                if r["ranked"] == true {
                    let ch = r["rp_change"].as_i64().unwrap_or(0);
                    s += &format!("   ·   {} RP", if ch >= 0 { format!("+{ch}") } else { ch.to_string() });
                }
                s
            }).unwrap_or_default();
            let host = self.online.is_host();
            let buttons: Vec<&str> = if host { vec!["Rematch", "Leave"] } else { vec!["Leave"] };
            if let Some(i) = self.overlay_card(title, &format!("{a} - {b}"), &reward, &buttons) {
                if host && i == 0 {
                    self.online.send(json!({"t": "rematch"}));
                } else {
                    self.online.leave_match();
                    self.go(Screen::Online);
                }
            }
        }
    }

    // ---- popups over everything

    fn overlays(&mut self) {
        if self.reveal.is_some() {
            crate::screens_shop::draw_reveal(self);
        }
        let invite = self.online.invite.clone();
        if let Some((m, _)) = invite {
            if !matches!(self.screen, Screen::Versus | Screen::Play) {
                let u = self.ui.u;
                let r = Rect::new(self.ui.w - 380.0 * u, 84.0 * u, 360.0 * u, 120.0 * u);
                self.ui.card(r);
                draw_icon("users", r.x + 34.0 * u, r.y + 38.0 * u, 28.0 * u, C_BLUE);
                self.ui.text(&format!("{} invited you", m["from"].as_str().unwrap_or("A friend")), r.x + 60.0 * u, r.y + 32.0 * u, 18.0, INK, Align::Left, true);
                self.ui.text(m["match"].as_str().unwrap_or(""), r.x + 60.0 * u, r.y + 56.0 * u, 14.0, MUTED, Align::Left, false);
                if self.ui.button("Join", Rect::new(r.x + 16.0 * u, r.y + 74.0 * u, 160.0 * u, 36.0 * u), Style::Primary) {
                    let code = m["code"].as_str().unwrap_or("").to_string();
                    self.online.send(json!({"t": "join", "code": code}));
                    self.online.invite = None;
                    self.go(Screen::Lobby);
                }
                if self.ui.button("Not now", Rect::new(r.x + 184.0 * u, r.y + 74.0 * u, 160.0 * u, 36.0 * u), Style::Secondary) {
                    self.online.invite = None;
                }
            }
        }
    }
}

pub fn draw_fruit_icon(x: f32, y: f32, size: f32) {
    crate::render::draw_fruit("apple", vec2(x, y), size * 0.45);
}

pub fn iso_now() -> String {
    let secs = (crate::net::now_ms() / 1000.0) as i64;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    // civil date from days since 1970 (Howard Hinnant's algorithm)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}
