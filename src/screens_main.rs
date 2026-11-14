//! The menu, game setup, speedruns, packs and settings.

use crate::app::{App, Screen};
use crate::cosmetics::catalog;
use crate::online::Req;
use crate::render::pack_preview;
use crate::settings::speed_label;
use crate::speedrun::{format_time, Category};
use crate::ui::*;
use macroquad::prelude::*;
use serde_json::Value;

struct Tile {
    label: &'static str,
    sub: &'static str,
    icon: &'static str,
    color: Color,
    to: Screen,
}

impl App {
    pub fn menu(&mut self) {
        let u = self.ui.u;
        let (w, h) = (self.ui.w, self.ui.h);
        let t = self.ui.time as f32;
        // a snake gliding behind the title
        let skin = self.my_skin();
        let n = 22;
        let mut pts = Vec::new();
        for i in 0..n {
            let x = ((t * 0.12 - i as f32 * 0.012).rem_euclid(1.3) - 0.15) * w;
            let y = h * 0.2 + (t * 1.5 - i as f32 * 0.35).sin() * 26.0 * u;
            pts.push(vec2(x, y));
        }
        for i in (0..n).rev() {
            // (solid, but faded into the background, so overlapping parts don't show)
            let c = mix(BG_TOP, rgbt(crate::cosmetics::skin_color(&skin, i, n, self.now_ms())), 0.22);
            if i + 1 < n && (pts[i] - pts[i + 1]).length() < 60.0 * u {
                draw_line(pts[i].x, pts[i].y, pts[i + 1].x, pts[i + 1].y, 22.0 * u, c);
            }
            draw_circle(pts[i].x, pts[i].y, 11.0 * u, c);
        }

        // title
        let ty = h * 0.2;
        let pulse = 1.0 + 0.015 * (t * 2.0).sin();
        self.ui.text("OpenSnake", w / 2.0 + 3.0 * u, ty + 5.0 * u, 76.0 * pulse, alpha(BLACK, 0.35), Align::Center, true);
        let tw = self.ui.text("OpenSnake", w / 2.0, ty, 76.0 * pulse, INK, Align::Center, true);
        let apple = vec2(w / 2.0 + tw / 2.0 + 30.0 * u, ty - 6.0 * u + (t * 3.0).sin() * 4.0 * u);
        crate::render::draw_fruit("apple", apple, 22.0 * u);
        self.ui.text("Eat. Grow. Don't crash.", w / 2.0, ty + 50.0 * u, 18.0, MUTED, Align::Center, false);

        // corners: the leaderboard (left) and your profile (right)
        let cr = 30.0 * u;
        if self.ui.round_button("trophy", vec2(26.0 * u + cr, 26.0 * u + cr), cr, rgb(217, 145, 10)) {
            self.go(Screen::Leaderboard);
        }
        self.ui.text("Leaderboard", 26.0 * u + cr, 26.0 * u + cr * 2.0 + 14.0 * u, 13.0, MUTED, Align::Center, true);
        let pc = vec2(w - 26.0 * u - cr, 26.0 * u + cr);
        if self.online.logged_in() {
            let me = self.online.me.clone();
            let name = self.online.user.clone().unwrap_or_default();
            crate::screens_social::avatar(self, pc, cr, me.as_ref().and_then(|m| m["avatar"].as_str()).unwrap_or(""), &name);
            let hover = self.ui.hover(Rect::new(pc.x - cr, pc.y - cr, cr * 2.0, cr * 2.0));
            if hover {
                self.ui.hand = true;
                draw_circle_lines(pc.x, pc.y, cr + 3.0 * u, 2.0 * u, ACCENT);
            }
            if self.ui.pressed(Rect::new(pc.x - cr, pc.y - cr, cr * 2.0, cr * 2.0)) {
                self.go(Screen::Profile);
            }
            let label = self.ui.fit(&name, 13.0, true, 120.0 * u);
            self.ui.text(&label, pc.x, pc.y + cr + 14.0 * u, 13.0, MUTED, Align::Center, true);
            self.points_chip(pc.x - cr - 16.0 * u, pc.y);
        } else {
            if self.ui.round_button("user", pc, cr, rgb(60, 80, 110)) {
                self.go(Screen::Profile);
            }
            self.ui.text("Log in", pc.x, pc.y + cr + 14.0 * u, 13.0, MUTED, Align::Center, true);
        }

        // play
        let pw = (360.0 * u).min(w - 40.0 * u);
        let play = Rect::new(w / 2.0 - pw / 2.0, h * 0.36, pw, 76.0 * u);
        if self.ui.button_icon("Play", "play", play, Style::Primary) || self.ui.hotkey(KeyCode::Enter) {
            self.go(Screen::Setup);
        }

        let tiles = [
            Tile { label: "Speedrun", sub: "Records & weekly runs", icon: "timer", color: rgb(59, 130, 246), to: Screen::Speedrun },
            Tile { label: "Online", sub: "Versus, ranked or not", icon: "globe", color: rgb(20, 184, 166), to: Screen::Online },
            Tile { label: "Store", sub: "Skins, crates, market", icon: "cart", color: rgb(236, 72, 153), to: Screen::Store },
            Tile { label: "Battle Pass", sub: "This week's rewards", icon: "star", color: rgb(168, 85, 247), to: Screen::Pass },
            Tile { label: "Daily", sub: "Three challenges a day", icon: "calendar", color: rgb(245, 158, 11), to: Screen::Daily },
            Tile { label: "Packs", sub: "Board colours", icon: "palette", color: rgb(34, 197, 94), to: Screen::Packs },
            Tile { label: "Settings", sub: "3D view, sound, server", icon: "gear", color: rgb(100, 116, 139), to: Screen::Settings },
        ];
        let cols = if w > 980.0 * u { 4 } else if w > 520.0 * u { 3 } else { 2 };
        let gap = 14.0 * u;
        let tw_ = ((w - 40.0 * u).min(980.0 * u) - gap * (cols as f32 - 1.0)) / cols as f32;
        let th = 96.0 * u;
        let rows = tiles.len().div_ceil(cols);
        let total_w = tw_ * cols as f32 + gap * (cols as f32 - 1.0);
        let x0 = w / 2.0 - total_w / 2.0;
        let y0 = (play.y + play.h + 28.0 * u).min(h - rows as f32 * (th + gap) - 30.0 * u);
        for (i, tile) in tiles.iter().enumerate() {
            let (cx, cy) = (i % cols, i / cols);
            // centre the last row
            let in_row = if cy == rows - 1 { tiles.len() - cy * cols } else { cols };
            let row_w = tw_ * in_row as f32 + gap * (in_row as f32 - 1.0);
            let rx = w / 2.0 - row_w / 2.0 + cx as f32 * (tw_ + gap);
            let r = Rect::new(if in_row == cols { x0 + cx as f32 * (tw_ + gap) } else { rx }, y0 + cy as f32 * (th + gap), tw_, th);
            if self.tile(r, tile) {
                self.go(tile.to);
            }
        }
        // badges: daily challenges ready to claim
        self.ui.text(&format!("v{}  ·  {}", crate::app::VERSION, self.online.server.trim_start_matches("https://")),
                     w / 2.0, h - 16.0 * u, 12.0, FAINT, Align::Center, false);
        if self.ui.hotkey(KeyCode::Escape) {
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.quit = true;
            }
        }
    }

    fn tile(&mut self, r: Rect, t: &Tile) -> bool {
        let u = self.ui.u;
        let id = id_of(&("tile", t.label));
        let hover = self.ui.hover(r);
        if hover {
            self.ui.hand = true;
        }
        let h = self.ui.anim(id, if hover { 1.0 } else { 0.0 }, 16.0);
        let rr = Rect::new(r.x, r.y - h * 4.0 * u, r.w, r.h);
        self.ui.shadow(rr, 16.0 * u, 1.0 + h);
        rounded(rr, 16.0 * u, mix(CARD, CARD_HI, h));
        rounded_outline(rr, 16.0 * u, 1.0 + h, mix(alpha(LINE, 0.6), t.color, h));
        let ic = vec2(rr.x + 40.0 * u, rr.y + rr.h / 2.0);
        draw_circle(ic.x, ic.y, 24.0 * u, alpha(t.color, 0.18 + 0.1 * h));
        draw_icon(t.icon, ic.x, ic.y, 26.0 * u, lighten(t.color, 0.2));
        let tx = rr.x + 76.0 * u;
        let maxw = rr.w - 86.0 * u;
        let label = self.ui.fit(t.label, 19.0, true, maxw);
        self.ui.text(&label, tx, rr.y + rr.h / 2.0 - 10.0 * u, 19.0, INK, Align::Left, true);
        let sub = self.ui.fit(t.sub, 13.0, false, maxw);
        self.ui.text(&sub, tx, rr.y + rr.h / 2.0 + 13.0 * u, 13.0, MUTED, Align::Left, false);
        self.ui.pressed(r)
    }

    // ---- setting up a round

    pub fn setup_screen(&mut self) {
        let area = self.page("Play", "play");
        let u = self.ui.u;
        let wide = area.w > 860.0 * u;
        let left = if wide { Rect::new(area.x, area.y, area.w * 0.58 - 8.0 * u, area.h) } else { area };
        let off = self.ui.begin_scroll("setup", left, 760.0 * u);
        let mut y = left.y - off;
        let x = left.x;
        let w = left.w;
        // board size
        self.ui.text("Board", x, y + 14.0 * u, 20.0, INK, Align::Left, true);
        y += 34.0 * u;
        let sizes = [("Small", 10, 9), ("Medium", 17, 15), ("Large", 24, 21), ("Huge", 34, 28)];
        let bw = (w - 3.0 * 10.0 * u) / 4.0;
        for (i, (name, bw_, bh)) in sizes.iter().enumerate() {
            let on = self.settings.board_w == *bw_ && self.settings.board_h == *bh;
            if self.ui.chip(&format!("{name}  {bw_}x{bh}"), on, Rect::new(x + i as f32 * (bw + 10.0 * u), y, bw, 40.0 * u), ACCENT) {
                self.settings.board_w = *bw_;
                self.settings.board_h = *bh;
            }
        }
        y += 62.0 * u;
        // speed
        self.ui.text("Speed", x, y + 14.0 * u, 20.0, INK, Align::Left, true);
        self.ui.text(speed_label(self.settings.speed), x + w, y + 14.0 * u, 16.0, ACCENT, Align::Right, true);
        y += 34.0 * u;
        let s = self.ui.slider("speed", self.settings.speed, Rect::new(x + 8.0 * u, y, w - 16.0 * u, 30.0 * u));
        self.settings.speed = (s * 4.0).round() / 4.0;
        y += 50.0 * u;
        // fruit
        self.ui.text("Fruit at once", x, y + 14.0 * u, 20.0, INK, Align::Left, true);
        y += 34.0 * u;
        for (i, n) in [1, 3, 5, 10].iter().enumerate() {
            if self.ui.chip(&n.to_string(), self.settings.fruit_count == *n, Rect::new(x + i as f32 * 70.0 * u, y, 60.0 * u, 38.0 * u), ACCENT) {
                self.settings.fruit_count = *n;
            }
        }
        let mixed = self.settings.fruit == "mixed";
        if self.ui.chip("Mixed fruit", mixed, Rect::new(x + 290.0 * u, y, 140.0 * u, 38.0 * u), rgb(245, 158, 11)) {
            self.settings.fruit = if mixed { "apple".into() } else { "mixed".into() };
        }
        y += 60.0 * u;
        // modes
        self.ui.text("Modes", x, y + 14.0 * u, 20.0, INK, Align::Left, true);
        self.ui.text("Mix as many as you like", x + 80.0 * u, y + 15.0 * u, 14.0, MUTED, Align::Left, false);
        y += 34.0 * u;
        let modes: Vec<_> = catalog().modes.iter().filter(|m| m.id != "classic").cloned().collect();
        let cols = if w > 600.0 * u { 4 } else { 3 };
        let mw = (w - (cols as f32 - 1.0) * 8.0 * u) / cols as f32;
        for (i, m) in modes.iter().enumerate() {
            let r = Rect::new(x + (i % cols) as f32 * (mw + 8.0 * u), y + (i / cols) as f32 * 46.0 * u, mw, 38.0 * u);
            let on = self.settings.modes.contains(&m.id);
            if self.ui.chip(&m.name, on, r, C_BLUE) {
                toggle_mode(&mut self.settings.modes, &m.id);
            }
            if self.ui.hover(r) {
                self.selected = Some(m.id.clone());
            }
        }
        y += (modes.len().div_ceil(cols)) as f32 * 46.0 * u + 10.0 * u;
        let desc = self.selected.as_ref().and_then(|id| catalog().modes.iter().find(|m| m.id == *id)).map(|m| m.desc.clone())
            .unwrap_or_else(|| "Point at a mode to see what it does.".into());
        self.ui.text(&desc, x, y + 10.0 * u, 15.0, MUTED, Align::Left, false);
        self.ui.end_scroll();

        // the right side: a summary and the start button
        let side = if wide { Rect::new(area.x + area.w * 0.58 + 8.0 * u, area.y, area.w * 0.42 - 8.0 * u, 300.0 * u) } else {
            Rect::new(area.x, area.y + area.h - 76.0 * u, area.w, 76.0 * u)
        };
        if wide {
            self.ui.card(side);
            let cx = side.x + side.w / 2.0;
            self.ui.text(&catalog().mode_label(&self.settings.modes), cx, side.y + 40.0 * u, 22.0, INK, Align::Center, true);
            self.ui.text(&format!("{} x {} board  ·  {}  ·  {} fruit", self.settings.board_w, self.settings.board_h,
                                  speed_label(self.settings.speed), self.settings.fruit_count),
                         cx, side.y + 72.0 * u, 15.0, MUTED, Align::Center, false);
            let key = format!("{}x{} {} {}", self.settings.board_w, self.settings.board_h,
                              catalog().mode_label(&self.settings.modes), self.settings.fruit_count);
            let best = *self.settings.best.get(&key).unwrap_or(&0);
            draw_icon("trophy", cx - 50.0 * u, side.y + 118.0 * u, 26.0 * u, C_GOLD);
            self.ui.text(&format!("Best {best}"), cx - 30.0 * u, side.y + 118.0 * u, 20.0, C_GOLD, Align::Left, true);
            let v3 = self.settings.view3d;
            if self.ui.toggle("3D view (fixed camera)", v3, Rect::new(side.x + 24.0 * u, side.y + 150.0 * u, side.w - 48.0 * u, 40.0 * u)) {
                self.settings.view3d = !v3;
            }
            // these settings as a speedrun: a community category once someone finishes it
            let sr = Rect::new(side.x, side.y + side.h + 16.0 * u, side.w, 150.0 * u);
            self.ui.card(sr);
            self.ui.text("Speedrun these settings", sr.x + 24.0 * u, sr.y + 30.0 * u, 18.0, INK, Align::Left, true);
            self.ui.text("Finish it and it's shared in Speedrun > Community.", sr.x + 24.0 * u, sr.y + 54.0 * u, 13.0, MUTED, Align::Left, false);
            let goals = [(10, "Eat 10"), (20, "Eat 20"), (30, "Eat 30"), (0, "Fill it")];
            let gw = (sr.w - 48.0 * u - 18.0 * u) / 4.0;
            let goal: i32 = self.fields.get("sr_goal").and_then(|g| g.parse().ok()).unwrap_or(20);
            for (i, (g, l)) in goals.iter().enumerate() {
                if self.ui.chip(l, goal == *g, Rect::new(sr.x + 24.0 * u + i as f32 * (gw + 6.0 * u), sr.y + 72.0 * u, gw, 30.0 * u), C_BLUE) {
                    self.fields.insert("sr_goal", g.to_string());
                }
            }
            if self.ui.button_icon("Start the run", "timer", Rect::new(sr.x + 24.0 * u, sr.y + 108.0 * u, sr.w - 48.0 * u, 34.0 * u), Style::Secondary) {
                let s = &self.settings;
                let cat = Category::custom(s.board_w, s.board_h, &s.modes, Some(goal), s.fruit_count,
                                           crate::settings::speed_ms(s.speed), &s.fruit, "");
                self.settings.save();
                self.start_run(cat, Screen::Setup);
            }
            if self.ui.button_icon("Start", "play", Rect::new(side.x + 24.0 * u, side.y + 214.0 * u, side.w - 48.0 * u, 62.0 * u), Style::Primary)
                || self.ui.hotkey(KeyCode::Enter)
            {
                self.settings.save();
                self.start_round();
            }
        } else if self.ui.button_icon("Start", "play", side, Style::Primary) || self.ui.hotkey(KeyCode::Enter) {
            self.settings.save();
            self.start_round();
        }
    }

    // ---- speedruns

    pub fn speedrun_screen(&mut self) {
        let area = self.page("Speedrun", "timer");
        let u = self.ui.u;
        let tab = self.tab(Screen::Speedrun);
        let tw = (480.0 * u).min(area.w);
        if let Some(t) = self.ui.tabs(&["Speedruns", "Weekly", "Community"], tab, Rect::new(area.x, area.y, tw, 44.0 * u)) {
            self.set_tab(Screen::Speedrun, t);
            self.viewing = None;
        }
        for (k, i) in [(KeyCode::Key1, 0), (KeyCode::Key2, 1), (KeyCode::Key3, 2)] {
            if self.ui.hotkey(k) {
                self.set_tab(Screen::Speedrun, i);
            }
        }
        let body = Rect::new(area.x, area.y + 60.0 * u, area.w, area.h - 60.0 * u);
        let mut extra = String::new();
        let cats: Vec<(Category, Option<Value>)> = match tab {
            0 => catalog().categories.iter().map(|c| (c.clone(), self.online.records.get(&c.id).cloned())).collect(),
            1 => {
                let Some(w) = self.online.weekly.clone() else {
                    self.loading(body);
                    return;
                };
                extra = format!("Week {}  ·  new ones in {}  ·  pays like the built-in categories", w["week"].as_str().unwrap_or(""), w["left"].as_str().unwrap_or(""));
                w["categories"].as_array().cloned().unwrap_or_default().iter().filter_map(|c| {
                    let cat = Category::from_server(c["id"].as_str()?, c["name"].as_str()?, c["desc"].as_str().unwrap_or(""), &c["def"])?;
                    Some((cat, if c["wr"].is_null() { None } else { Some(c["wr"].clone()) }))
                }).collect()
            }
            _ => {
                let Some(cm) = self.online.community.clone() else {
                    self.loading(body);
                    return;
                };
                extra = "Made by players: speedrun your own settings from Play, finish, and it shows up here. No rewards.".into();
                cm["categories"].as_array().cloned().unwrap_or_default().iter().filter_map(|c| {
                    let mut cat = Category::from_server(c["id"].as_str()?, c["name"].as_str()?, "", &c["def"])?;
                    cat.desc = format!("by {}  ·  {} players", c["creator"].as_str().unwrap_or("?"), c["players"].as_i64().unwrap_or(0));
                    Some((cat, if c["wr"].is_null() { None } else { Some(c["wr"].clone()) }))
                }).collect()
            }
        };
        if !extra.is_empty() {
            self.ui.text(&self.ui.fit(&extra, 14.0, false, area.w - tw - 20.0 * u), area.x + tw + 16.0 * u, area.y + 22.0 * u, 14.0, MUTED, Align::Left, false);
        }
        // a list of categories, and the leaderboard of the one you're looking at
        let wide = body.w > 900.0 * u;
        let list = if wide { Rect::new(body.x, body.y, body.w * 0.6 - 8.0 * u, body.h) } else { body };
        let row_h = 74.0 * u;
        let off = self.ui.begin_scroll(&format!("sr{tab}"), list, cats.len() as f32 * (row_h + 8.0 * u));
        for (i, (cat, wr)) in cats.iter().enumerate() {
            let r = Rect::new(list.x, list.y + i as f32 * (row_h + 8.0 * u) - off, list.w - 10.0 * u, row_h);
            if !self.ui.visible(r) {
                continue;
            }
            let hover = self.ui.hover(r);
            let sel = self.viewing.as_deref() == Some(cat.id.as_str());
            rounded(r, 12.0 * u, if sel { CARD_HI } else if hover { mix(CARD, CARD_HI, 0.5) } else { CARD });
            if sel {
                rounded_outline(r, 12.0 * u, 1.5, ACCENT);
            }
            let name = self.ui.fit(&cat.name, 19.0, true, r.w * 0.5);
            self.ui.text(&name, r.x + 18.0 * u, r.y + 24.0 * u, 19.0, INK, Align::Left, true);
            let desc = if cat.desc.is_empty() { cat.goal_text() } else { cat.desc.clone() };
            let desc = self.ui.fit(&desc, 13.0, false, r.w * 0.55);
            self.ui.text(&desc, r.x + 18.0 * u, r.y + 50.0 * u, 13.0, MUTED, Align::Left, false);
            let pb = self.settings.pbs.get(&cat.id).map_or("--".into(), |t| format_time(*t as f64));
            let wrt = wr.as_ref().and_then(|w| w["time_ms"].as_f64()).map_or("--".into(), format_time);
            let who = wr.as_ref().and_then(|w| w["player"].as_str()).unwrap_or("");
            self.ui.text(&format!("PB {pb}"), r.x + r.w - 150.0 * u, r.y + 24.0 * u, 15.0, C_BLUE, Align::Right, true);
            self.ui.text(&format!("WR {wrt}"), r.x + r.w - 150.0 * u, r.y + 50.0 * u, 15.0, C_GOLD, Align::Right, true);
            if !who.is_empty() {
                let who = self.ui.fit(who, 12.0, false, 120.0 * u);
                self.ui.text(&who, r.x + r.w - 150.0 * u - 100.0 * u, r.y + 50.0 * u, 12.0, FAINT, Align::Right, false);
            }
            if self.ui.button_icon("", "play", Rect::new(r.x + r.w - 126.0 * u, r.y + 15.0 * u, 110.0 * u, 44.0 * u), Style::Primary) {
                let c = cat.clone();
                self.start_run(c, Screen::Speedrun);
                self.ui.end_scroll();
                return;
            }
            if self.ui.pressed(r) {
                self.viewing = Some(cat.id.clone());
                self.online.leaderboards.remove(&cat.id);
                self.online.get(&format!("/api/leaderboard/{}", cat.id), Req::Leaderboard(cat.id.clone()));
            }
        }
        if cats.is_empty() {
            self.ui.text("Nothing here yet.", list.x + list.w / 2.0, list.y + 60.0 * u, 18.0, MUTED, Align::Center, false);
        }
        self.ui.end_scroll();
        if wide {
            let side = Rect::new(body.x + body.w * 0.6 + 8.0 * u, body.y, body.w * 0.4 - 8.0 * u, body.h);
            self.ui.card(side);
            let Some(id) = self.viewing.clone() else {
                self.ui.text("Pick a category", side.x + side.w / 2.0, side.y + side.h / 2.0 - 12.0 * u, 20.0, INK, Align::Center, true);
                self.ui.text("to see its leaderboard.", side.x + side.w / 2.0, side.y + side.h / 2.0 + 14.0 * u, 15.0, MUTED, Align::Center, false);
                return;
            };
            let name = cats.iter().find(|(c, _)| c.id == id).map(|(c, _)| c.name.clone()).unwrap_or_default();
            self.ui.text(&name, side.x + 20.0 * u, side.y + 30.0 * u, 20.0, INK, Align::Left, true);
            let Some(lb) = self.online.leaderboards.get(&id).cloned() else {
                self.ui.text("Loading...", side.x + 20.0 * u, side.y + 70.0 * u, 15.0, MUTED, Align::Left, false);
                return;
            };
            let entries = lb["entries"].as_array().cloned().unwrap_or_default();
            if entries.is_empty() {
                self.ui.text("No runs yet: be the first!", side.x + 20.0 * u, side.y + 70.0 * u, 15.0, MUTED, Align::Left, false);
            }
            let inner = Rect::new(side.x, side.y + 56.0 * u, side.w, side.h - 64.0 * u);
            let off = self.ui.begin_scroll(&format!("lb{id}"), inner, entries.len() as f32 * 40.0 * u);
            let me = self.online.user.clone().unwrap_or_default().to_lowercase();
            for (i, e) in entries.iter().enumerate() {
                let y = inner.y + i as f32 * 40.0 * u - off;
                let player = e["player"].as_str().unwrap_or("");
                if player.to_lowercase() == me {
                    rounded(Rect::new(side.x + 10.0 * u, y + 2.0 * u, side.w - 20.0 * u, 36.0 * u), 8.0 * u, alpha(ACCENT, 0.12));
                }
                let medal = [C_GOLD, rgb(190, 198, 208), rgb(205, 127, 50)];
                if i < 3 {
                    draw_circle(side.x + 34.0 * u, y + 20.0 * u, 13.0 * u, medal[i]);
                }
                self.ui.text(&(i + 1).to_string(), side.x + 34.0 * u, y + 20.0 * u, 14.0, if i < 3 { rgb(30, 30, 30) } else { MUTED }, Align::Center, true);
                let p = self.ui.fit(player, 16.0, true, side.w * 0.5);
                self.ui.text(&p, side.x + 60.0 * u, y + 20.0 * u, 16.0, INK, Align::Left, true);
                self.ui.text(&format_time(e["time_ms"].as_f64().unwrap_or(0.0)), side.x + side.w - 20.0 * u, y + 20.0 * u, 16.0, INK, Align::Right, true);
            }
            self.ui.end_scroll();
        }
    }

    pub fn loading(&mut self, area: Rect) {
        let u = self.ui.u;
        let t = self.ui.time as f32;
        for k in 0..3 {
            let a = 0.3 + 0.7 * ((t * 4.0 - k as f32 * 0.6).sin() * 0.5 + 0.5);
            draw_circle(area.x + area.w / 2.0 + (k as f32 - 1.0) * 20.0 * u, area.y + area.h * 0.35, 6.0 * u, alpha(ACCENT, a));
        }
        if self.online.server_ok == Some(false) {
            self.ui.text("Can't reach the server.", area.x + area.w / 2.0, area.y + area.h * 0.35 + 36.0 * u, 16.0, MUTED, Align::Center, false);
        }
    }

    // ---- packs

    pub fn packs_screen(&mut self) {
        let area = self.page("Packs", "palette");
        let u = self.ui.u;
        self.ui.text("Board colours. Shop packs are bought in the Store; the rarest come from loot boxes.", area.x, area.y + 12.0 * u, 15.0, MUTED, Align::Left, false);
        let grid = Rect::new(area.x, area.y + 36.0 * u, area.w, area.h - 36.0 * u);
        let mut items: Vec<(String, String, String, Vec<(u8, u8, u8)>)> = vec![("".into(), "Classic".into(), "common".into(),
            vec![(170, 215, 81), (162, 209, 73), (231, 71, 29)])];
        // the free packs that come with the game
        for (id, name) in FREE_PACKS {
            let c = &catalog().pack_colors[*id];
            let get = |k: &str| c.get(k).copied().unwrap_or((128, 128, 128));
            items.push((id.to_string(), name.to_string(), "common".into(), vec![get("board_light"), get("board_dark"), get("border")]));
        }
        for p in &catalog().packs {
            items.push((p.id.clone(), p.name.clone(), p.rarity.clone(), p.swatch.iter().map(|s| crate::cosmetics::hex(s)).collect()));
        }
        let cols = ((grid.w / (250.0 * u)) as usize).max(2);
        let gap = 12.0 * u;
        let cw = (grid.w - gap * (cols as f32 - 1.0)) / cols as f32;
        let ch = 170.0 * u;
        let rows = items.len().div_ceil(cols);
        let off = self.ui.begin_scroll("packs", grid, rows as f32 * (ch + gap));
        for (i, (id, name, rarity, sw)) in items.iter().enumerate() {
            let r = Rect::new(grid.x + (i % cols) as f32 * (cw + gap), grid.y + (i / cols) as f32 * (ch + gap) - off, cw, ch);
            if !self.ui.visible(r) {
                continue;
            }
            let owned = id.is_empty() || FREE_PACKS.iter().any(|(f, _)| f == id) || self.online.owns(&format!("pack:{id}"));
            let on = self.settings.pack == *id;
            self.ui.card(r);
            if on {
                rounded_outline(r, 16.0 * u, 2.0, ACCENT);
            }
            pack_preview(sw, Rect::new(r.x + 14.0 * u, r.y + 14.0 * u, r.w - 28.0 * u, 80.0 * u));
            self.ui.text(name, r.x + 16.0 * u, r.y + 114.0 * u, 17.0, INK, Align::Left, true);
            let rc = rgbt(catalog().rarity_color(rarity));
            rarity_tag(&self.ui, &catalog().rarity_name(rarity), rc, r.x + 16.0 * u, r.y + 142.0 * u, 10.0);
            let br = Rect::new(r.x + r.w - 104.0 * u, r.y + 124.0 * u, 90.0 * u, 34.0 * u);
            if on {
                self.ui.text("In use", br.x + br.w / 2.0, br.y + br.h / 2.0, 14.0, ACCENT, Align::Center, true);
            } else if owned {
                if self.ui.button("Use", br, Style::Secondary) {
                    self.settings.pack = id.clone();
                    self.settings.save();
                }
            } else {
                draw_icon("lock", br.x + br.w / 2.0, br.y + br.h / 2.0, 18.0 * u, FAINT);
            }
        }
        self.ui.end_scroll();
    }

    // ---- settings

    pub fn settings_screen(&mut self) {
        let area = self.page("Settings", "gear");
        let u = self.ui.u;
        let w = area.w.min(640.0 * u);
        let x = area.x + (area.w - w) / 2.0;
        let mut y = area.y + 10.0 * u;
        let card = Rect::new(x, y, w, 430.0 * u);
        self.ui.card(card);
        y += 20.0 * u;
        let row = |y: f32| Rect::new(x + 24.0 * u, y, w - 48.0 * u, 44.0 * u);
        if self.ui.toggle("3D view (a fixed camera over the board)", self.settings.view3d, row(y)) {
            self.settings.view3d = !self.settings.view3d;
            self.settings.save();
        }
        y += 60.0 * u;
        self.ui.text("Sound", x + 24.0 * u, y + 14.0 * u, 18.0, INK, Align::Left, false);
        self.ui.text(&format!("{:.0}%", self.settings.volume * 100.0), x + w - 24.0 * u, y + 14.0 * u, 16.0, MUTED, Align::Right, true);
        let v = self.ui.slider("volume", self.settings.volume, Rect::new(x + 140.0 * u, y, w - 240.0 * u, 30.0 * u));
        if (v - self.settings.volume).abs() > 0.001 {
            self.settings.volume = v;
            self.sfx.volume = v;
        }
        if !self.ui.down && (self.settings.volume - v).abs() < 0.001 && is_mouse_button_released(MouseButton::Left) {
            self.settings.save();
            self.sfx.play("eat");
        }
        y += 56.0 * u;
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.ui.toggle("Full screen", self.settings.fullscreen, row(y)) {
                self.settings.fullscreen = !self.settings.fullscreen;
                set_fullscreen(self.settings.fullscreen);
                self.settings.save();
            }
            y += 60.0 * u;
            self.ui.text("Server", x + 24.0 * u, y + 14.0 * u, 18.0, INK, Align::Left, false);
            y += 34.0 * u;
            if !self.fields.contains_key("server") {
                self.fields.insert("server", self.online.server.clone());
            }
            let mut s = self.fields.remove("server").unwrap_or_default();
            let entered = self.ui.text_input("server", &mut s, Rect::new(x + 24.0 * u, y, w - 180.0 * u, 44.0 * u), "https://...", false, 200);
            let changed = s.trim() != self.online.server;
            if (self.ui.button("Connect", Rect::new(x + w - 146.0 * u, y, 122.0 * u, 44.0 * u), if changed { Style::Primary } else { Style::Secondary }) || entered) && !s.trim().is_empty() {
                let mut url = s.trim().trim_end_matches('/').to_string();
                if !url.contains("://") {
                    url = format!("https://{url}");
                }
                if url != self.online.server {
                    self.online.logout();
                    self.online.server = url.clone();
                    self.online.records.clear();
                    self.online.check_server();
                    self.save();
                }
                s = url;
            }
            self.fields.insert("server", s);
            y += 56.0 * u;
            let status = match self.online.server_ok {
                Some(true) => ("Connected", ACCENT),
                Some(false) => ("Can't reach this server", C_RED),
                None => ("Checking...", MUTED),
            };
            self.ui.text(status.0, x + 24.0 * u, y + 8.0 * u, 14.0, status.1, Align::Left, true);
        }
        let _ = y;
        let help = Rect::new(x, card.y + card.h + 16.0 * u, w, 120.0 * u);
        self.ui.card(help);
        self.ui.text("Controls", help.x + 24.0 * u, help.y + 28.0 * u, 18.0, INK, Align::Left, true);
        self.ui.text("Arrow keys or WASD to steer (swipe on touch screens)  ·  P or Space to pause  ·  R to restart  ·  Esc to quit",
                     help.x + 24.0 * u, help.y + 60.0 * u, 14.0, MUTED, Align::Left, false);
        self.ui.text("Esc goes back  ·  1 2 3 switch tabs", help.x + 24.0 * u, help.y + 86.0 * u, 14.0, MUTED, Align::Left, false);
    }
}

/// Packs anyone can use (the rest are won or bought).
pub const FREE_PACKS: &[(&str, &str)] = &[("everforest", "Everforest"), ("grave_viper", "Grave Viper"), ("shadow_venom", "Shadow Venom")];

pub fn toggle_mode(active: &mut Vec<String>, id: &str) {
    if let Some(i) = active.iter().position(|m| m == id) {
        active.remove(i);
        return;
    }
    for (a, b) in &catalog().conflicts {
        if a == id {
            active.retain(|m| m != b);
        }
        if b == id {
            active.retain(|m| m != a);
        }
    }
    active.push(id.to_string());
}
