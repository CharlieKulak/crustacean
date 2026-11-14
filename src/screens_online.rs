//! Online versus: hosting, joining, public matches and the lobby.

use crate::app::{App, Screen};
use crate::cosmetics::catalog;
use crate::online::{urlencode, Req};
use crate::screens_main::toggle_mode;
use crate::ui::*;
use macroquad::prelude::*;
use serde_json::json;

impl App {
    pub fn online_screen(&mut self) {
        let area = self.page("Online", "globe");
        let u = self.ui.u;
        if self.online.room.is_some() {
            // already in a match (from an invite, say)
            self.go(Screen::Lobby);
            return;
        }
        if !self.online.logged_in() {
            let wide = area.w > 980.0 * u;
            let login = if wide { Rect::new(area.x, area.y, area.w * 0.5, area.h) } else { area };
            self.login_form(login);
            if wide {
                let r = Rect::new(area.x + area.w * 0.5 + 20.0 * u, area.y + 10.0 * u, area.w * 0.5 - 20.0 * u, 230.0 * u);
                self.ui.card(r);
                self.ui.text("Or play as a guest", r.x + 24.0 * u, r.y + 34.0 * u, 20.0, INK, Align::Left, true);
                self.ui.text("Guests can play versus, but don't earn points or rank.", r.x + 24.0 * u, r.y + 62.0 * u, 13.0, MUTED, Align::Left, false);
                let mut name = self.fields.remove("guest").unwrap_or_else(|| self.settings.guest_name.clone());
                self.ui.text_input("guest", &mut name, Rect::new(r.x + 24.0 * u, r.y + 90.0 * u, r.w - 48.0 * u, 44.0 * u), "Your name in matches", false, 16);
                let mut code = self.fields.remove("code").unwrap_or_default();
                let entered = self.ui.text_input("code", &mut code, Rect::new(r.x + 24.0 * u, r.y + 150.0 * u, 160.0 * u, 46.0 * u), "Match code", false, 4);
                code = code.to_uppercase();
                if (self.ui.button("Join", Rect::new(r.x + 196.0 * u, r.y + 150.0 * u, 120.0 * u, 46.0 * u), Style::Primary) || entered) && code.len() == 4 {
                    self.settings.guest_name = name.clone();
                    self.online.send_as(&name, json!({"t": "join", "code": code}));
                    self.go(Screen::Lobby);
                }
                self.fields.insert("guest", name);
                self.fields.insert("code", code);
            }
            return;
        }
        let wide = area.w > 980.0 * u;
        let host = if wide { Rect::new(area.x, area.y, area.w * 0.46, area.h) } else { Rect::new(area.x, area.y, area.w, 560.0 * u) };
        self.host_card(host);
        let right = if wide { Rect::new(area.x + area.w * 0.46 + 16.0 * u, area.y, area.w * 0.54 - 16.0 * u, area.h) } else {
            Rect::new(area.x, area.y + 576.0 * u, area.w, (area.h - 576.0 * u).max(300.0 * u))
        };
        // join with a code
        let jr = Rect::new(right.x, right.y, right.w, 96.0 * u);
        self.ui.card(jr);
        self.ui.text("Join with a code", jr.x + 20.0 * u, jr.y + 26.0 * u, 18.0, INK, Align::Left, true);
        let mut code = self.fields.remove("code").unwrap_or_default();
        let entered = self.ui.text_input("code", &mut code, Rect::new(jr.x + 20.0 * u, jr.y + 44.0 * u, 160.0 * u, 40.0 * u), "ABCD", false, 4);
        code = code.to_uppercase();
        if (self.ui.button("Join", Rect::new(jr.x + 192.0 * u, jr.y + 44.0 * u, 110.0 * u, 40.0 * u), Style::Primary) || entered) && code.len() == 4 {
            self.online.send(json!({"t": "join", "code": code}));
            self.go(Screen::Lobby);
        }
        self.fields.insert("code", code);
        // public matches
        let pr = Rect::new(right.x, right.y + 110.0 * u, right.w, right.h - 110.0 * u);
        self.ui.card(pr);
        self.ui.text("Public matches", pr.x + 20.0 * u, pr.y + 28.0 * u, 18.0, INK, Align::Left, true);
        let mut q = self.fields.remove("match_q").unwrap_or_default();
        let entered = self.ui.text_input("match_q", &mut q, Rect::new(pr.x + pr.w - 290.0 * u, pr.y + 10.0 * u, 220.0 * u, 38.0 * u), "Search", false, 40);
        if self.ui.button_icon("", "search", Rect::new(pr.x + pr.w - 60.0 * u, pr.y + 10.0 * u, 44.0 * u, 38.0 * u), Style::Secondary) || entered {
            self.online.get(&format!("/api/matches?q={}", urlencode(&q)), Req::Matches);
        }
        self.fields.insert("match_q", q);
        let list = Rect::new(pr.x + 12.0 * u, pr.y + 60.0 * u, pr.w - 24.0 * u, pr.h - 70.0 * u);
        let matches = self.online.matches.as_ref().and_then(|m| m["matches"].as_array().cloned()).unwrap_or_default();
        if matches.is_empty() {
            self.ui.text("No public matches right now: host one!", list.x + list.w / 2.0, list.y + 40.0 * u, 15.0, MUTED, Align::Center, false);
        }
        let off = self.ui.begin_scroll("matches", list, matches.len() as f32 * 70.0 * u);
        for (i, m) in matches.iter().enumerate() {
            let r = Rect::new(list.x, list.y + i as f32 * 70.0 * u - off, list.w - 8.0 * u, 62.0 * u);
            if !self.ui.visible(r) {
                continue;
            }
            rounded(r, 12.0 * u, CARD_HI);
            let name = self.ui.fit(m["name"].as_str().unwrap_or(""), 16.0, true, r.w * 0.5);
            self.ui.text(&name, r.x + 16.0 * u, r.y + 21.0 * u, 16.0, INK, Align::Left, true);
            let modes: Vec<String> = m["modes"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            let ranked = m["ranked"] != false;
            let info = format!("{}  ·  {} / {} players  ·  {}", m["size"].as_str().unwrap_or(""), m["players"], m["max"], catalog().mode_label(&modes));
            let info = self.ui.fit(&info, 12.0, false, r.w * 0.62);
            self.ui.text(&info, r.x + 16.0 * u, r.y + 44.0 * u, 12.0, MUTED, Align::Left, false);
            self.ui.text(if ranked { "RANKED" } else { "UNRANKED" }, r.x + r.w - 120.0 * u, r.y + 31.0 * u, 11.0, if ranked { C_GOLD } else { MUTED }, Align::Right, true);
            if self.ui.button("Join", Rect::new(r.x + r.w - 104.0 * u, r.y + 12.0 * u, 92.0 * u, 38.0 * u), Style::Primary) {
                let code = m["code"].as_str().unwrap_or("").to_string();
                self.online.send(json!({"t": "join", "code": code}));
                self.go(Screen::Lobby);
            }
        }
        self.ui.end_scroll();
    }

    fn host_card(&mut self, r: Rect) {
        let u = self.ui.u;
        self.ui.card(r);
        let x = r.x + 20.0 * u;
        let w = r.w - 40.0 * u;
        let mut y = r.y + 28.0 * u;
        self.ui.text("Host a match", x, y, 20.0, INK, Align::Left, true);
        y += 26.0 * u;
        // ranked or not
        let half = (w - 10.0 * u) / 2.0;
        if self.ui.chip("Ranked", self.settings.vs_ranked, Rect::new(x, y, half, 40.0 * u), C_GOLD) {
            self.settings.vs_ranked = true;
        }
        if self.ui.chip("Unranked", !self.settings.vs_ranked, Rect::new(x + half + 10.0 * u, y, half, 40.0 * u), C_BLUE) {
            self.settings.vs_ranked = false;
        }
        y += 50.0 * u;
        let note = if self.settings.vs_ranked { "Wins and losses against people change your rank points." } else { "Just for fun: no rank points won or lost. Still earns snake points." };
        self.ui.text(note, x, y + 4.0 * u, 12.0, MUTED, Align::Left, false);
        y += 26.0 * u;
        self.ui.text("Board", x, y + 10.0 * u, 15.0, MUTED, Align::Left, true);
        let sizes = ["small", "medium", "large"];
        let sw = (w - 90.0 * u - 16.0 * u) / 3.0;
        for (i, s) in sizes.iter().enumerate() {
            let label = format!("{}{}", &s[..1].to_uppercase(), &s[1..]);
            if self.ui.chip(&label, self.settings.vs_size == *s, Rect::new(x + 90.0 * u + i as f32 * (sw + 8.0 * u), y, sw, 34.0 * u), ACCENT) {
                self.settings.vs_size = s.to_string();
            }
        }
        y += 44.0 * u;
        self.ui.text("Speed", x, y + 12.0 * u, 15.0, MUTED, Align::Left, true);
        let v = self.ui.slider("vs_speed", self.settings.vs_speed, Rect::new(x + 90.0 * u, y, w - 180.0 * u, 26.0 * u));
        self.settings.vs_speed = (v * 4.0).round() / 4.0;
        self.ui.text(crate::settings::speed_label(self.settings.vs_speed), x + w, y + 12.0 * u, 13.0, ACCENT, Align::Right, true);
        y += 40.0 * u;
        self.ui.text("Win by", x, y + 12.0 * u, 15.0, MUTED, Align::Left, true);
        let goals = [(0, "Last alive"), (10, "10 fruit"), (20, "20 fruit"), (40, "40 fruit")];
        let gw = (w - 90.0 * u - 24.0 * u) / 4.0;
        for (i, (g, l)) in goals.iter().enumerate() {
            if self.ui.chip(l, self.settings.vs_goal == *g, Rect::new(x + 90.0 * u + i as f32 * (gw + 8.0 * u), y, gw, 32.0 * u), ACCENT) {
                self.settings.vs_goal = *g;
            }
        }
        y += 44.0 * u;
        self.ui.text("Modes", x, y + 10.0 * u, 15.0, MUTED, Align::Left, true);
        y += 26.0 * u;
        let modes = catalog().versus_modes.clone();
        let cols = 4;
        let mw = (w - (cols as f32 - 1.0) * 8.0 * u) / cols as f32;
        for (i, m) in modes.iter().enumerate() {
            let rr = Rect::new(x + (i % cols) as f32 * (mw + 8.0 * u), y + (i / cols) as f32 * 40.0 * u, mw, 32.0 * u);
            if self.ui.chip(&catalog().mode_name(m), self.settings.vs_modes.contains(m), rr, C_BLUE) {
                toggle_mode(&mut self.settings.vs_modes, m);
            }
        }
        y += modes.len().div_ceil(cols) as f32 * 40.0 * u + 4.0 * u;
        if self.ui.toggle("Public (shows in match search)", self.settings.vs_public, Rect::new(x, y, w, 36.0 * u)) {
            self.settings.vs_public = !self.settings.vs_public;
        }
        let br = Rect::new(x, r.y + r.h - 70.0 * u, w, 54.0 * u);
        if self.ui.button_icon("Host", "play", br, Style::Primary) {
            self.settings.save();
            let s = &self.settings;
            let msg = json!({"t": "host", "ranked": s.vs_ranked, "public": s.vs_public,
                             "settings": {"size": s.vs_size, "speed": s.vs_speed, "apples": s.vs_apples, "goal": s.vs_goal, "modes": s.vs_modes}});
            self.online.send(msg);
            self.go(Screen::Lobby);
        }
    }

    pub fn lobby_screen(&mut self) {
        let u = self.ui.u;
        let Some(room) = self.online.room.clone() else {
            let area = self.page("Match", "globe");
            self.loading(area);
            if self.ui.time - self.entered > 8.0 {
                self.go(Screen::Online);
            }
            return;
        };
        let area = self.page(room["name"].as_str().unwrap_or("Match"), "globe");
        let host = self.online.is_host();
        let my_id = self.online.my_id;
        // the code, big, to share
        let code = room["code"].as_str().unwrap_or("????");
        let head = Rect::new(area.x, area.y, area.w, 90.0 * u);
        self.ui.card(head);
        self.ui.text("CODE", head.x + 24.0 * u, head.y + 28.0 * u, 12.0, MUTED, Align::Left, true);
        self.ui.text(code, head.x + 24.0 * u, head.y + 60.0 * u, 36.0, ACCENT, Align::Left, true);
        let ranked = room["ranked"] != false;
        let public = room["public"] == true;
        let s = &room["settings"];
        let modes: Vec<String> = s["modes"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        let goal = s["goal"].as_i64().unwrap_or(0);
        let info = format!("{} board  ·  {}  ·  {}  ·  {}", s["size"].as_str().unwrap_or(""), crate::settings::speed_label(s["speed"].as_f64().unwrap_or(0.5) as f32),
                           if goal > 0 { format!("first to {goal}") } else { "last team alive".into() }, catalog().mode_label(&modes));
        self.ui.text(&info, head.x + 160.0 * u, head.y + 34.0 * u, 14.0, MUTED, Align::Left, false);
        if host {
            let cx = head.x + 160.0 * u;
            if self.ui.chip(if ranked { "Ranked" } else { "Unranked" }, true, Rect::new(cx, head.y + 50.0 * u, 120.0 * u, 30.0 * u), if ranked { C_GOLD } else { C_BLUE }) {
                self.online.send(json!({"t": "details", "ranked": !ranked}));
            }
            if self.ui.chip(if public { "Public" } else { "Private" }, public, Rect::new(cx + 130.0 * u, head.y + 50.0 * u, 110.0 * u, 30.0 * u), ACCENT) {
                self.online.send(json!({"t": "details", "public": !public}));
            }
        } else {
            self.ui.text(if ranked { "RANKED" } else { "UNRANKED" }, head.x + 160.0 * u, head.y + 64.0 * u, 13.0, if ranked { C_GOLD } else { MUTED }, Align::Left, true);
        }
        if self.ui.button("Leave", Rect::new(head.x + head.w - 130.0 * u, head.y + 22.0 * u, 110.0 * u, 46.0 * u), Style::Secondary) {
            self.online.leave_match();
            self.go(Screen::Online);
            return;
        }
        // the two teams
        let players = room["players"].as_array().cloned().unwrap_or_default();
        let teams_y = head.y + head.h + 16.0 * u;
        let side_w = if area.w > 900.0 * u { 300.0 * u } else { 0.0 };
        let tw = (area.w - side_w - 16.0 * u - if side_w > 0.0 { 16.0 * u } else { 0.0 }) / 2.0;
        let th = area.h - head.h - 16.0 * u - 86.0 * u;
        for team in 0..2 {
            let r = Rect::new(area.x + team as f32 * (tw + 16.0 * u), teams_y, tw, th);
            let col = if team == 0 { C_BLUE } else { rgb(255, 140, 70) };
            self.ui.card(r);
            rounded(Rect::new(r.x + 16.0 * u, r.y, r.w - 32.0 * u, 5.0 * u), 2.5 * u, col);
            self.ui.text(&format!("Team {}", team + 1), r.x + 20.0 * u, r.y + 30.0 * u, 18.0, col, Align::Left, true);
            let mut y = r.y + 54.0 * u;
            for p in players.iter().filter(|p| p["team"].as_i64() == Some(team)) {
                let pr = Rect::new(r.x + 12.0 * u, y, r.w - 24.0 * u, 54.0 * u);
                let me = p["id"].as_i64() == my_id;
                rounded(pr, 10.0 * u, if me { CARD_HI } else { alpha(CARD_HI, 0.5) });
                let name = p["name"].as_str().unwrap_or("").to_string();
                let bot = !p["bot"].is_null() && p["bot"] != false;
                if bot {
                    draw_circle(pr.x + 28.0 * u, pr.y + 27.0 * u, 18.0 * u, rgb(80, 90, 110));
                    draw_icon("gear", pr.x + 28.0 * u, pr.y + 27.0 * u, 20.0 * u, WHITE);
                } else {
                    crate::screens_social::avatar(self, vec2(pr.x + 28.0 * u, pr.y + 27.0 * u), 18.0 * u, p["avatar"].as_str().unwrap_or(""), &name);
                }
                let label = if me { format!("{name} (you)") } else { name.clone() };
                self.ui.text(&label, pr.x + 56.0 * u, pr.y + 19.0 * u, 15.0, INK, Align::Left, true);
                self.ui.text(p["rank"].as_str().unwrap_or(""), pr.x + 56.0 * u, pr.y + 38.0 * u, 12.0, MUTED, Align::Left, false);
                if room["host"] == p["id"] {
                    draw_icon("crown", pr.x + pr.w - 24.0 * u, pr.y + 27.0 * u, 18.0 * u, C_GOLD);
                } else if host && bot && self.ui.button_icon("", "x", Rect::new(pr.x + pr.w - 44.0 * u, pr.y + 9.0 * u, 36.0 * u, 36.0 * u), Style::Ghost) {
                    self.online.send(json!({"t": "remove_bot", "id": p["id"]}));
                }
                y += 62.0 * u;
            }
            let mine = players.iter().find(|p| p["id"].as_i64() == my_id).and_then(|p| p["team"].as_i64());
            let br = Rect::new(r.x + 12.0 * u, r.y + r.h - 52.0 * u, (r.w - 32.0 * u) / 2.0, 40.0 * u);
            if mine != Some(team) && self.ui.button("Join team", br, Style::Secondary) {
                self.online.send(json!({"t": "team", "team": team}));
            }
            if host {
                let br2 = Rect::new(r.x + 20.0 * u + (r.w - 32.0 * u) / 2.0, r.y + r.h - 52.0 * u, (r.w - 32.0 * u) / 2.0, 40.0 * u);
                let key = if team == 0 { "bot0" } else { "bot1" };
                let level = self.fields.get(key).cloned().unwrap_or_else(|| "normal".into());
                if self.ui.button_icon(&format!("Bot ({level})"), "plus", br2, Style::Ghost) {
                    self.online.send(json!({"t": "add_bot", "team": team, "level": level}));
                }
                // right click (or long press) cycles the level
                if self.ui.hover(br2) && is_mouse_button_pressed(MouseButton::Right) {
                    let next = match level.as_str() { "easy" => "normal", "normal" => "hard", _ => "easy" };
                    self.fields.insert(key, next.into());
                }
            }
        }
        // invite friends
        if side_w > 0.0 {
            let r = Rect::new(area.x + area.w - side_w, teams_y, side_w, th);
            self.ui.card(r);
            self.ui.text("Invite friends", r.x + 18.0 * u, r.y + 28.0 * u, 17.0, INK, Align::Left, true);
            if self.online.friends.is_none() && self.online.logged_in() {
                self.online.get("/api/friends", Req::Friends);
            }
            let friends = self.online.friends.as_ref().and_then(|f| f["friends"].as_array().cloned()).unwrap_or_default();
            let online: Vec<_> = friends.into_iter().filter(|f| f["online"] == true).collect();
            if online.is_empty() {
                self.ui.text("No friends online.", r.x + 18.0 * u, r.y + 60.0 * u, 13.0, MUTED, Align::Left, false);
            }
            for (i, f) in online.iter().enumerate().take(8) {
                let y = r.y + 50.0 * u + i as f32 * 50.0 * u;
                let name = f["name"].as_str().unwrap_or("").to_string();
                crate::screens_social::avatar(self, vec2(r.x + 34.0 * u, y + 20.0 * u), 15.0 * u, f["avatar"].as_str().unwrap_or(""), &name);
                let n = self.ui.fit(&name, 14.0, true, side_w - 150.0 * u);
                self.ui.text(&n, r.x + 58.0 * u, y + 20.0 * u, 14.0, INK, Align::Left, true);
                if self.ui.button("Invite", Rect::new(r.x + r.w - 90.0 * u, y + 4.0 * u, 76.0 * u, 32.0 * u), Style::Secondary) {
                    self.online.send(json!({"t": "invite", "name": name}));
                }
            }
            self.ui.text("Right-click a Bot button to change its level.", r.x + 18.0 * u, r.y + r.h - 20.0 * u, 11.0, FAINT, Align::Left, false);
        }
        let br = Rect::new(area.x, area.y + area.h - 64.0 * u, area.w - side_w - if side_w > 0.0 { 16.0 * u } else { 0.0 }, 60.0 * u);
        if host {
            if self.ui.button_icon("Start match", "play", br, Style::Primary) || self.ui.hotkey(KeyCode::Enter) {
                self.online.send(json!({"t": "start"}));
            }
        } else {
            self.ui.text("Waiting for the host to start...", br.x + br.w / 2.0, br.y + br.h / 2.0, 18.0, MUTED, Align::Center, true);
        }
    }
}
