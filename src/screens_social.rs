//! Your profile, friends, messages and the community leaderboard.

use crate::app::{App, Screen};
use crate::cosmetics::{catalog, format_points};
use crate::online::{urlencode, Req};
use crate::render::{draw_fruit, key_icon, shield_icon};
use crate::ui::*;
use macroquad::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

/// Textures of players' own pictures, made once.
fn texture_cache() -> &'static Mutex<HashMap<String, Option<Texture2D>>> {
    static C: std::sync::OnceLock<Mutex<HashMap<String, Option<Texture2D>>>> = std::sync::OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A player's picture: their own uploaded one, or an icon on a colour.
pub fn avatar(app: &mut App, c: Vec2, r: f32, avatar: &str, name: &str) {
    let (kind, rest) = avatar.split_once(':').unwrap_or(("opensnake", "0"));
    if kind == "custom" {
        let key = format!("{name}:{rest}");
        let cached = texture_cache().lock().unwrap().get(&key).cloned();
        match cached {
            Some(Some(tex)) => {
                draw_circle(c.x, c.y, r, CARD_HI);
                draw_texture_ex(&tex, c.x - r * 0.92, c.y - r * 0.92, WHITE,
                                DrawTextureParams { dest_size: Some(vec2(r * 1.84, r * 1.84)), ..Default::default() });
                // round the corners off with a ring in the page's colour
                for k in 0..6 {
                    draw_circle_lines(c.x, c.y, r * 0.92 + k as f32 * r * 0.06, r * 0.08, BG_TOP);
                }
                draw_circle_lines(c.x, c.y, r, 2.0, alpha(WHITE, 0.25));
                return;
            }
            Some(None) => {}
            None => {
                if let Some(b64) = app.online.picture(name) {
                    let tex = decode_b64(&b64).and_then(|bytes| Texture2D::from_file_with_format(&bytes, Some(ImageFormat::Png)).into());
                    texture_cache().lock().unwrap().insert(key, Some(tex).flatten());
                }
            }
        }
        draw_circle(c.x, c.y, r, CARD_HI);
        draw_icon("user", c.x, c.y, r, MUTED);
        return;
    }
    let ci: usize = rest.parse().unwrap_or(0);
    let col = catalog().avatar_colors.get(ci).map(|c| rgbt(*c)).unwrap_or(C_BLUE);
    draw_circle(c.x, c.y, r, col);
    draw_circle(c.x - r * 0.3, c.y - r * 0.35, r * 0.45, alpha(WHITE, 0.08));
    avatar_icon(kind, c, r * 0.62);
}

fn avatar_icon(kind: &str, c: Vec2, s: f32) {
    if let Some(f) = kind.strip_prefix("fruit_") {
        draw_fruit(f, c, s * 0.8);
        return;
    }
    match kind {
        "key" => key_icon(c, s * 0.8, rgb(255, 196, 0)),
        "shield" => shield_icon(c, s * 0.8, WHITE),
        "trophy" => draw_icon("trophy", c.x, c.y, s * 1.5, C_GOLD),
        "lock" => draw_icon("lock", c.x, c.y, s * 1.3, WHITE),
        "skull" => {
            draw_circle(c.x, c.y - s * 0.1, s * 0.75, WHITE);
            rounded(Rect::new(c.x - s * 0.45, c.y + s * 0.3, s * 0.9, s * 0.45), s * 0.1, WHITE);
            draw_circle(c.x - s * 0.3, c.y - s * 0.1, s * 0.2, rgb(30, 30, 30));
            draw_circle(c.x + s * 0.3, c.y - s * 0.1, s * 0.2, rgb(30, 30, 30));
        }
        "statue" => {
            rounded(Rect::new(c.x - s * 0.6, c.y - s * 0.6, s * 1.2, s * 1.2), s * 0.35, rgb(190, 190, 200));
            draw_circle(c.x, c.y, s * 0.3, rgb(150, 150, 160));
        }
        "mine" => {
            draw_circle(c.x, c.y, s * 0.55, rgb(30, 30, 36));
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                draw_line(c.x, c.y, c.x + a.cos() * s * 0.85, c.y + a.sin() * s * 0.85, s * 0.15, rgb(30, 30, 36));
            }
            draw_circle(c.x - s * 0.18, c.y - s * 0.18, s * 0.14, WHITE);
        }
        _ => {
            // the OpenSnake head
            draw_circle(c.x, c.y, s * 0.8, rgb(122, 162, 255));
            draw_circle(c.x - s * 0.3, c.y - s * 0.15, s * 0.22, WHITE);
            draw_circle(c.x + s * 0.3, c.y - s * 0.15, s * 0.22, WHITE);
            draw_circle(c.x - s * 0.26, c.y - s * 0.12, s * 0.11, rgb(20, 24, 32));
            draw_circle(c.x + s * 0.34, c.y - s * 0.12, s * 0.11, rgb(20, 24, 32));
            draw_line(c.x - s * 0.05, c.y + s * 0.45, c.x - s * 0.05, c.y + s * 0.75, s * 0.1, rgb(232, 36, 60));
        }
    }
}

pub fn decode_b64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;
    for ch in s.bytes() {
        let v = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

pub fn encode_b64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

impl App {
    pub fn profile_screen(&mut self) {
        let area = self.page("Profile", "user");
        let u = self.ui.u;
        if !self.online.logged_in() {
            self.login_form(area);
            return;
        }
        let tab = self.tab(Screen::Profile);
        let unread = self.online.chats.as_ref().and_then(|c| c["unread"].as_i64()).unwrap_or(0);
        let requests = self.online.friends.as_ref().and_then(|f| f["incoming"].as_array().map(|a| a.len())).unwrap_or(0);
        let l1 = if requests > 0 { format!("Friends ({requests})") } else { "Friends".into() };
        let l2 = if unread > 0 { format!("Messages ({unread})") } else { "Messages".into() };
        let tw = (480.0 * u).min(area.w);
        if let Some(t) = self.ui.tabs(&["Profile", &l1, &l2], tab, Rect::new(area.x, area.y, tw, 44.0 * u)) {
            self.set_tab(Screen::Profile, t);
        }
        for (k, i) in [(KeyCode::Key1, 0), (KeyCode::Key2, 1), (KeyCode::Key3, 2)] {
            if self.ui.hotkey(k) {
                self.set_tab(Screen::Profile, i);
            }
        }
        let body = Rect::new(area.x, area.y + 60.0 * u, area.w, area.h - 60.0 * u);
        match tab {
            0 => self.my_profile(body),
            1 => self.friends(body),
            _ => self.messages(body),
        }
        if self.fields.get("view_profile").is_some_and(|s| !s.is_empty()) {
            self.profile_popup();
        }
    }

    pub fn login_form(&mut self, area: Rect) {
        let u = self.ui.u;
        let w = area.w.min(460.0 * u);
        let r = Rect::new(area.x + (area.w - w) / 2.0, area.y + 10.0 * u, w, 400.0 * u);
        self.ui.card(r);
        self.ui.text("Your OpenSnake account", r.x + w / 2.0, r.y + 40.0 * u, 22.0, INK, Align::Center, true);
        self.ui.text("Snake points, skins, ranks and records live on the server.", r.x + w / 2.0, r.y + 68.0 * u, 13.0, MUTED, Align::Center, false);
        let mut name = self.fields.remove("login_name").unwrap_or_else(|| self.settings.user.clone().unwrap_or_default());
        let mut pw = self.fields.remove("login_pw").unwrap_or_default();
        let x = r.x + 30.0 * u;
        let fw = w - 60.0 * u;
        self.ui.text("Username", x, r.y + 104.0 * u, 14.0, MUTED, Align::Left, true);
        let e1 = self.ui.text_input("login_name", &mut name, Rect::new(x, r.y + 118.0 * u, fw, 46.0 * u), "3-20 letters, numbers, _ or -", false, 20);
        self.ui.text("Password", x, r.y + 184.0 * u, 14.0, MUTED, Align::Left, true);
        let e2 = self.ui.text_input("login_pw", &mut pw, Rect::new(x, r.y + 198.0 * u, fw, 46.0 * u), "At least 6 characters", true, 100);
        if e1 {
            self.ui.focus = Some(id_of(&("input", "login_pw")));
        }
        let bw = (fw - 12.0 * u) / 2.0;
        let busy = self.online.is_busy(&Req::Login { create: false }) || self.online.is_busy(&Req::Login { create: true });
        if (self.ui.button(if busy { "..." } else { "Log in" }, Rect::new(x, r.y + 270.0 * u, bw, 50.0 * u), Style::Primary) || e2) && !busy {
            self.online.login(name.trim(), &pw, false);
            pw.clear();
        }
        if self.ui.button("Create account", Rect::new(x + bw + 12.0 * u, r.y + 270.0 * u, bw, 50.0 * u), Style::Secondary) && !busy {
            self.online.login(name.trim(), &pw, true);
            pw.clear();
        }
        let server = self.online.server.trim_start_matches("https://").to_string();
        self.ui.text(&format!("Server: {server}"), r.x + w / 2.0, r.y + 350.0 * u, 12.0, FAINT, Align::Center, false);
        self.fields.insert("login_name", name);
        self.fields.insert("login_pw", pw);
    }

    fn my_profile(&mut self, body: Rect) {
        let u = self.ui.u;
        let Some(me) = self.online.me.clone() else {
            self.loading(body);
            return;
        };
        let cat = catalog();
        let wide = body.w > 900.0 * u;
        let card = Rect::new(body.x, body.y, if wide { body.w * 0.42 } else { body.w }, 380.0 * u);
        self.ui.card(card);
        let name = self.online.user.clone().unwrap_or_default();
        let av = me["avatar"].as_str().unwrap_or("").to_string();
        avatar(self, vec2(card.x + 76.0 * u, card.y + 76.0 * u), 48.0 * u, &av, &name);
        self.ui.text(&name, card.x + 142.0 * u, card.y + 58.0 * u, 26.0, INK, Align::Left, true);
        let rank = me["rank"].as_str().unwrap_or("Hatchling").to_string();
        let ri = cat.rank_index(&rank);
        let rc = rgbt(crate::cosmetics::hex(&cat.ranks[ri].color));
        draw_icon("crown", card.x + 154.0 * u, card.y + 92.0 * u, 20.0 * u, rc);
        self.ui.text(&format!("{rank}  ·  {} RP", me["rp"].as_i64().unwrap_or(0)), card.x + 170.0 * u, card.y + 92.0 * u, 16.0, rc, Align::Left, true);
        // the next rank and its rewards
        let rp = me["rp"].as_i64().unwrap_or(0);
        let mut y = card.y + 150.0 * u;
        if let Some(next) = cat.ranks.get(ri + 1) {
            let lo = cat.ranks[ri].rp;
            let frac = (rp - lo) as f32 / (next.rp - lo).max(1) as f32;
            self.ui.text(&format!("Next: {} at {} RP", next.name, next.rp), card.x + 24.0 * u, y, 14.0, MUTED, Align::Left, true);
            progress(Rect::new(card.x + 24.0 * u, y + 16.0 * u, card.w - 48.0 * u, 10.0 * u), frac, rc);
            let best = me["best_rank"].as_str().unwrap_or("Hatchling");
            let rewards = if cat.rank_index(best) >= ri + 1 { "(already claimed once)".to_string() } else { next.rewards.join(", ") };
            let rw = self.ui.fit(&format!("Reward: {rewards}"), 12.0, false, card.w - 48.0 * u);
            self.ui.text(&rw, card.x + 24.0 * u, y + 44.0 * u, 12.0, FAINT, Align::Left, false);
        } else {
            self.ui.text("Top rank!", card.x + 24.0 * u, y, 16.0, C_GOLD, Align::Left, true);
        }
        y += 76.0 * u;
        let stats = [
            ("Wins", me["wins"].as_i64().unwrap_or(0).to_string()),
            ("Losses", me["losses"].as_i64().unwrap_or(0).to_string()),
            ("WRs", me["wrs"].as_i64().unwrap_or(0).to_string()),
            ("Points", format_points(me["points"].as_i64().unwrap_or(0))),
        ];
        let sw = (card.w - 48.0 * u) / 4.0;
        for (i, (l, v)) in stats.iter().enumerate() {
            let cx = card.x + 24.0 * u + sw * (i as f32 + 0.5);
            self.ui.text(v, cx, y + 10.0 * u, 20.0, INK, Align::Center, true);
            self.ui.text(l, cx, y + 34.0 * u, 12.0, MUTED, Align::Center, false);
        }
        y += 70.0 * u;
        if self.ui.button_icon("Log out", "logout", Rect::new(card.x + 24.0 * u, y, 150.0 * u, 40.0 * u), Style::Ghost) {
            self.online.logout();
            self.save();
            return;
        }
        self.ui.text(&format!("Joined {}", me["joined"].as_str().unwrap_or("")), card.x + card.w - 24.0 * u, y + 20.0 * u, 12.0, FAINT, Align::Right, false);

        // editing: bio and picture
        let ed = if wide { Rect::new(body.x + body.w * 0.42 + 16.0 * u, body.y, body.w * 0.58 - 16.0 * u, body.h) } else {
            Rect::new(body.x, body.y + 396.0 * u, body.w, body.h - 396.0 * u)
        };
        let off = self.ui.begin_scroll("profile_edit", ed, 560.0 * u);
        let mut y = ed.y - off;
        self.ui.text("Bio", ed.x, y + 12.0 * u, 18.0, INK, Align::Left, true);
        y += 30.0 * u;
        if !self.fields.contains_key("bio") {
            self.fields.insert("bio", me["bio"].as_str().unwrap_or("").to_string());
        }
        let mut bio = self.fields.remove("bio").unwrap_or_default();
        let entered = self.ui.text_input("bio", &mut bio, Rect::new(ed.x, y, ed.w - 130.0 * u, 46.0 * u), "Say something about yourself", false, 160);
        let changed = bio != me["bio"].as_str().unwrap_or("");
        if (self.ui.button("Save", Rect::new(ed.x + ed.w - 118.0 * u, y, 106.0 * u, 46.0 * u), if changed { Style::Primary } else { Style::Secondary }) || entered) && changed {
            self.online.post("/api/profile", json!({"bio": bio}), Req::SetProfile);
        }
        self.fields.insert("bio", bio);
        y += 70.0 * u;
        self.ui.text("Picture", ed.x, y + 12.0 * u, 18.0, INK, Align::Left, true);
        y += 34.0 * u;
        let n = cat.avatar_icons.len();
        let cols = ((ed.w / (54.0 * u)) as usize).max(4);
        let col_idx: usize = av.split_once(':').and_then(|(k, c)| if k == "custom" { None } else { c.parse().ok() }).unwrap_or(0);
        for i in 0..n {
            let c = vec2(ed.x + 24.0 * u + (i % cols) as f32 * 54.0 * u, y + 24.0 * u + (i / cols) as f32 * 54.0 * u);
            let icon = cat.avatar_icons[i].clone();
            let val = format!("{icon}:{col_idx}");
            avatar(self, c, 22.0 * u, &val, "");
            let r = Rect::new(c.x - 24.0 * u, c.y - 24.0 * u, 48.0 * u, 48.0 * u);
            if av == val {
                draw_circle_lines(c.x, c.y, 25.0 * u, 2.5 * u, ACCENT);
            } else if self.ui.hover(r) {
                self.ui.hand = true;
                draw_circle_lines(c.x, c.y, 25.0 * u, 1.5 * u, alpha(WHITE, 0.5));
            }
            if self.ui.pressed(r) {
                self.online.post("/api/profile", json!({"avatar": val}), Req::SetProfile);
            }
        }
        y += (n.div_ceil(cols)) as f32 * 54.0 * u + 12.0 * u;
        let (icon, _) = av.split_once(':').unwrap_or(("opensnake", "0"));
        let icon = if icon == "custom" { "opensnake".to_string() } else { icon.to_string() };
        for (i, c) in cat.avatar_colors.iter().enumerate() {
            let p = vec2(ed.x + 20.0 * u + i as f32 * 42.0 * u, y + 18.0 * u);
            draw_circle(p.x, p.y, 15.0 * u, rgbt(*c));
            let r = Rect::new(p.x - 17.0 * u, p.y - 17.0 * u, 34.0 * u, 34.0 * u);
            if i == col_idx && !av.starts_with("custom") {
                draw_circle_lines(p.x, p.y, 18.0 * u, 2.0 * u, WHITE);
            }
            if self.ui.pressed(r) {
                self.online.post("/api/profile", json!({"avatar": format!("{icon}:{i}")}), Req::SetProfile);
            }
        }
        y += 52.0 * u;
        self.ui.text("Or upload your own (a square PNG works best, up to 256 KB).", ed.x, y + 8.0 * u, 13.0, MUTED, Align::Left, false);
        y += 26.0 * u;
        #[cfg(target_arch = "wasm32")]
        {
            if self.ui.button_icon("Choose a picture", "plus", Rect::new(ed.x, y, 220.0 * u, 44.0 * u), Style::Secondary) {
                crate::net::pick_image();
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut path = self.fields.remove("pic_path").unwrap_or_default();
            let entered = self.ui.text_input("pic_path", &mut path, Rect::new(ed.x, y, ed.w - 130.0 * u, 44.0 * u), "Path to a PNG file", false, 400);
            if self.ui.button("Upload", Rect::new(ed.x + ed.w - 118.0 * u, y, 106.0 * u, 44.0 * u), Style::Secondary) || entered {
                match crate::net::read_image_file(&path) {
                    Some(bytes) if bytes.starts_with(b"\x89PNG") => {
                        let b64 = encode_b64(&bytes);
                        self.online.upload_picture(&b64);
                        texture_cache().lock().unwrap().clear();
                    }
                    Some(_) => self.ui.toast("That isn't a PNG file.", C_RED),
                    None => self.ui.toast("Couldn't open that file.", C_RED),
                }
            }
            self.fields.insert("pic_path", path);
        }
        self.ui.end_scroll();
    }

    fn friends(&mut self, body: Rect) {
        let u = self.ui.u;
        let wide = body.w > 860.0 * u;
        let left = if wide { Rect::new(body.x, body.y, body.w * 0.5 - 8.0 * u, body.h) } else { Rect::new(body.x, body.y, body.w, body.h * 0.55) };
        let right = if wide { Rect::new(body.x + body.w * 0.5 + 8.0 * u, body.y, body.w * 0.5 - 8.0 * u, body.h) } else {
            Rect::new(body.x, body.y + body.h * 0.55 + 12.0 * u, body.w, body.h * 0.45 - 12.0 * u)
        };
        // your friends and requests
        let Some(f) = self.online.friends.clone() else {
            self.loading(left);
            return;
        };
        let mut rows: Vec<(String, Value)> = Vec::new();
        for (k, label) in [("incoming", "Wants to be friends"), ("friends", ""), ("outgoing", "Request sent")] {
            for p in f[k].as_array().cloned().unwrap_or_default() {
                rows.push((label.to_string(), p));
            }
        }
        self.ui.text(&format!("Friends ({})", f["friends"].as_array().map_or(0, |a| a.len())), left.x, left.y + 12.0 * u, 18.0, INK, Align::Left, true);
        let list = Rect::new(left.x, left.y + 34.0 * u, left.w, left.h - 34.0 * u);
        if rows.is_empty() {
            self.ui.text("No friends yet: search for players on the right.", list.x, list.y + 30.0 * u, 14.0, MUTED, Align::Left, false);
        }
        let off = self.ui.begin_scroll("friends", list, rows.len() as f32 * 70.0 * u);
        let in_lobby = self.online.room.as_ref().is_some_and(|r| r["state"] == "lobby");
        for (i, (label, p)) in rows.iter().enumerate() {
            let r = Rect::new(list.x, list.y + i as f32 * 70.0 * u - off, list.w - 10.0 * u, 62.0 * u);
            if !self.ui.visible(r) {
                continue;
            }
            let name = p["name"].as_str().unwrap_or("").to_string();
            rounded(r, 12.0 * u, CARD);
            avatar(self, vec2(r.x + 34.0 * u, r.y + 31.0 * u), 20.0 * u, p["avatar"].as_str().unwrap_or(""), &name);
            let online = p["online"] == true;
            if online {
                draw_circle(r.x + 48.0 * u, r.y + 46.0 * u, 6.0 * u, CARD);
                draw_circle(r.x + 48.0 * u, r.y + 46.0 * u, 4.5 * u, ACCENT);
            }
            self.ui.text(&name, r.x + 66.0 * u, r.y + 22.0 * u, 16.0, INK, Align::Left, true);
            let sub = if !label.is_empty() { label.clone() } else if p["in_match"] == true { "In a match".into() } else if online { "Online".into() } else {
                p["rank"].as_str().unwrap_or("").to_string()
            };
            self.ui.text(&sub, r.x + 66.0 * u, r.y + 44.0 * u, 12.0, if online { ACCENT } else { MUTED }, Align::Left, false);
            let mut bx = r.x + r.w - 10.0 * u;
            let mut btn = |app: &mut App, icon: &str, style: Style| -> bool {
                bx -= 44.0 * u;
                app.ui.button_icon("", icon, Rect::new(bx, r.y + 12.0 * u, 38.0 * u, 38.0 * u), style)
            };
            if label == "Wants to be friends" {
                if btn(self, "x", Style::Ghost) {
                    self.online.post("/api/friends/remove", json!({"name": name}), Req::FriendAction);
                }
                if btn(self, "check", Style::Primary) {
                    self.online.post("/api/friends/add", json!({"name": name}), Req::FriendAction);
                }
            } else if label.is_empty() {
                if btn(self, "x", Style::Ghost) {
                    self.online.post("/api/friends/remove", json!({"name": name}), Req::FriendAction);
                }
                if btn(self, "chat", Style::Secondary) {
                    self.open_chat(&name);
                }
                if online && in_lobby && btn(self, "plus", Style::Secondary) {
                    self.online.send(json!({"t": "invite", "name": name}));
                }
            } else if btn(self, "x", Style::Ghost) {
                self.online.post("/api/friends/remove", json!({"name": name}), Req::FriendAction);
            }
            if self.ui.pressed(Rect::new(r.x, r.y, r.w * 0.5, r.h)) {
                self.view_profile(&name);
            }
        }
        self.ui.end_scroll();

        // find players
        self.ui.text("Find players", right.x, right.y + 12.0 * u, 18.0, INK, Align::Left, true);
        let mut q = self.fields.remove("user_q").unwrap_or_default();
        let before = q.clone();
        self.ui.text_input("user_q", &mut q, Rect::new(right.x, right.y + 34.0 * u, right.w, 44.0 * u), "Search by name", false, 20);
        if q != before && q.trim().len() >= 2 {
            self.online.busy.remove(&format!("{:?}", Req::Users));
            self.online.get(&format!("/api/users?q={}", urlencode(q.trim())), Req::Users);
        }
        self.fields.insert("user_q", q.clone());
        let list = Rect::new(right.x, right.y + 92.0 * u, right.w, right.h - 92.0 * u);
        let found = if q.trim().len() >= 2 { self.online.users.as_ref().and_then(|v| v["users"].as_array().cloned()).unwrap_or_default() } else { vec![] };
        let me = self.online.user.clone().unwrap_or_default();
        let known: Vec<String> = rows.iter().filter_map(|(_, p)| p["name"].as_str().map(String::from)).collect();
        let off = self.ui.begin_scroll("users", list, found.len() as f32 * 64.0 * u);
        for (i, p) in found.iter().enumerate() {
            let r = Rect::new(list.x, list.y + i as f32 * 64.0 * u - off, list.w - 10.0 * u, 56.0 * u);
            if !self.ui.visible(r) {
                continue;
            }
            let name = p["name"].as_str().unwrap_or("").to_string();
            rounded(r, 12.0 * u, CARD);
            avatar(self, vec2(r.x + 30.0 * u, r.y + 28.0 * u), 18.0 * u, p["avatar"].as_str().unwrap_or(""), &name);
            self.ui.text(&name, r.x + 58.0 * u, r.y + 20.0 * u, 15.0, INK, Align::Left, true);
            self.ui.text(p["rank"].as_str().unwrap_or(""), r.x + 58.0 * u, r.y + 40.0 * u, 12.0, MUTED, Align::Left, false);
            if name != me && !known.contains(&name) {
                if self.ui.button_icon("Add", "plus", Rect::new(r.x + r.w - 100.0 * u, r.y + 10.0 * u, 90.0 * u, 36.0 * u), Style::Secondary) {
                    self.online.post("/api/friends/add", json!({"name": name}), Req::FriendAction);
                }
            }
            if self.ui.pressed(Rect::new(r.x, r.y, r.w * 0.6, r.h)) {
                self.view_profile(&name);
            }
        }
        self.ui.end_scroll();
    }

    pub fn view_profile(&mut self, name: &str) {
        self.fields.insert("view_profile", name.to_string());
        self.online.get(&format!("/api/users/{}", urlencode(name)), Req::Profile(name.to_string()));
    }

    fn profile_popup(&mut self) {
        let name = self.fields.get("view_profile").cloned().unwrap_or_default();
        let u = self.ui.u;
        let (w, h) = (self.ui.w, self.ui.h);
        self.ui.input = true;
        draw_rectangle(0.0, 0.0, w, h, alpha(rgb(4, 8, 12), 0.7));
        let r = Rect::new(w / 2.0 - 230.0 * u, h / 2.0 - 200.0 * u, 460.0 * u, 400.0 * u);
        self.ui.card(r);
        if let Some(p) = self.online.profiles.get(&name).cloned() {
            avatar(self, vec2(r.x + r.w / 2.0, r.y + 70.0 * u), 46.0 * u, p["avatar"].as_str().unwrap_or(""), &name);
            self.ui.text(&name, r.x + r.w / 2.0, r.y + 140.0 * u, 24.0, INK, Align::Center, true);
            let cat = catalog();
            let rank = p["rank"].as_str().unwrap_or("");
            let rc = cat.ranks.get(cat.rank_index(rank)).map(|r| rgbt(crate::cosmetics::hex(&r.color))).unwrap_or(MUTED);
            self.ui.text(&format!("{rank}  ·  {} RP", p["rp"].as_i64().unwrap_or(0)), r.x + r.w / 2.0, r.y + 168.0 * u, 15.0, rc, Align::Center, true);
            let bio = p["bio"].as_str().unwrap_or("");
            if !bio.is_empty() {
                let b = self.ui.fit(bio, 14.0, false, r.w - 40.0 * u);
                self.ui.text(&b, r.x + r.w / 2.0, r.y + 200.0 * u, 14.0, MUTED, Align::Center, false);
            }
            let stats = [("Wins", p["wins"].as_i64().unwrap_or(0)), ("Losses", p["losses"].as_i64().unwrap_or(0)), ("WRs", p["wrs"].as_i64().unwrap_or(0))];
            for (i, (l, v)) in stats.iter().enumerate() {
                let cx = r.x + r.w * (0.25 + 0.25 * i as f32);
                self.ui.text(&v.to_string(), cx, r.y + 244.0 * u, 20.0, INK, Align::Center, true);
                self.ui.text(l, cx, r.y + 268.0 * u, 12.0, MUTED, Align::Center, false);
            }
            if let Some(sk) = p["skin"].as_str().and_then(|s| cat.skin(s)) {
                self.ui.text(&format!("Wearing {}", sk.name), r.x + r.w / 2.0, r.y + 298.0 * u, 13.0, FAINT, Align::Center, false);
            }
            let me = self.online.user.clone().unwrap_or_default();
            let fs = p["friend"].as_str().unwrap_or("none").to_string();
            let br = Rect::new(r.x + 30.0 * u, r.y + r.h - 70.0 * u, r.w / 2.0 - 40.0 * u, 46.0 * u);
            if name != me && self.online.logged_in() {
                let (label, style) = match fs.as_str() {
                    "friends" => ("Message", Style::Primary),
                    "incoming" => ("Accept request", Style::Primary),
                    "outgoing" => ("Request sent", Style::Ghost),
                    _ => ("Add friend", Style::Primary),
                };
                if self.ui.button(label, br, style) {
                    match fs.as_str() {
                        "friends" => {
                            self.fields.remove("view_profile");
                            self.open_chat(&name);
                            self.ui.input = false;
                            return;
                        }
                        "outgoing" => {}
                        _ => {
                            self.online.post("/api/friends/add", json!({"name": name}), Req::FriendAction);
                            self.online.profiles.remove(&name);
                            self.online.get(&format!("/api/users/{}", urlencode(&name)), Req::Profile(name.clone()));
                        }
                    }
                }
            }
        } else {
            self.loading(r);
        }
        if self.ui.button("Close", Rect::new(r.x + r.w / 2.0 + 10.0 * u, r.y + r.h - 70.0 * u, r.w / 2.0 - 40.0 * u, 46.0 * u), Style::Secondary)
            || self.ui.hotkey(KeyCode::Escape)
        {
            self.fields.remove("view_profile");
        }
        self.ui.input = false;
    }

    pub fn open_chat(&mut self, name: &str) {
        self.tabs.insert(Screen::Profile, 2);
        self.screen = Screen::Profile;
        self.online.conversation = Some((name.to_string(), Vec::new()));
        self.online.get(&format!("/api/messages/{}", urlencode(name)), Req::Conversation(name.to_string()));
    }

    fn messages(&mut self, body: Rect) {
        let u = self.ui.u;
        let wide = body.w > 760.0 * u;
        let open = self.online.conversation.clone();
        let list = if wide { Rect::new(body.x, body.y, 300.0 * u, body.h) } else { body };
        if wide || open.is_none() {
            let Some(chats) = self.online.chats.clone() else {
                self.online.get("/api/messages", Req::Chats);
                self.loading(list);
                return;
            };
            let chats = chats["chats"].as_array().cloned().unwrap_or_default();
            if chats.is_empty() {
                self.ui.text("No messages yet.", list.x + 10.0 * u, list.y + 20.0 * u, 15.0, MUTED, Align::Left, false);
                self.ui.text("Message a friend from the Friends tab.", list.x + 10.0 * u, list.y + 44.0 * u, 13.0, FAINT, Align::Left, false);
            }
            let off = self.ui.begin_scroll("chats", list, chats.len() as f32 * 72.0 * u);
            for (i, c) in chats.iter().enumerate() {
                let r = Rect::new(list.x, list.y + i as f32 * 72.0 * u - off, list.w - 10.0 * u, 64.0 * u);
                if !self.ui.visible(r) {
                    continue;
                }
                let name = c["name"].as_str().unwrap_or("").to_string();
                let sel = open.as_ref().is_some_and(|(w, _)| *w == name);
                let hover = self.ui.hover(r);
                rounded(r, 12.0 * u, if sel { CARD_HI } else if hover { mix(CARD, CARD_HI, 0.5) } else { CARD });
                avatar(self, vec2(r.x + 32.0 * u, r.y + 32.0 * u), 20.0 * u, c["avatar"].as_str().unwrap_or(""), &name);
                self.ui.text(&name, r.x + 62.0 * u, r.y + 22.0 * u, 15.0, INK, Align::Left, true);
                let last = c["last"]["body"].as_str().unwrap_or("");
                let last = self.ui.fit(last, 12.0, false, r.w - 100.0 * u);
                self.ui.text(&last, r.x + 62.0 * u, r.y + 44.0 * u, 12.0, MUTED, Align::Left, false);
                let unread = c["unread"].as_i64().unwrap_or(0);
                if unread > 0 {
                    draw_circle(r.x + r.w - 22.0 * u, r.y + 32.0 * u, 11.0 * u, ACCENT);
                    self.ui.text(&unread.to_string(), r.x + r.w - 22.0 * u, r.y + 32.0 * u, 11.0, rgb(8, 30, 16), Align::Center, true);
                }
                if self.ui.pressed(r) {
                    self.open_chat(&name);
                }
            }
            self.ui.end_scroll();
            if !wide {
                return;
            }
        }
        let conv = if wide { Rect::new(body.x + 316.0 * u, body.y, body.w - 316.0 * u, body.h) } else { body };
        let Some((with, msgs)) = self.online.conversation.clone() else {
            self.ui.card(conv);
            self.ui.text("Pick a chat", conv.x + conv.w / 2.0, conv.y + conv.h / 2.0, 18.0, MUTED, Align::Center, true);
            return;
        };
        self.ui.card(conv);
        if !wide && self.ui.button_icon("", "back", Rect::new(conv.x + 12.0 * u, conv.y + 10.0 * u, 36.0 * u, 36.0 * u), Style::Ghost) {
            self.online.conversation = None;
            self.online.chats = None;
            return;
        }
        self.ui.text(&with, conv.x + if wide { 20.0 } else { 58.0 } * u, conv.y + 28.0 * u, 18.0, INK, Align::Left, true);
        if self.ui.button("Profile", Rect::new(conv.x + conv.w - 110.0 * u, conv.y + 10.0 * u, 96.0 * u, 36.0 * u), Style::Ghost) {
            self.view_profile(&with);
        }
        draw_line(conv.x, conv.y + 56.0 * u, conv.x + conv.w, conv.y + 56.0 * u, 1.0, alpha(LINE, 0.7));
        let area = Rect::new(conv.x + 12.0 * u, conv.y + 60.0 * u, conv.w - 24.0 * u, conv.h - 132.0 * u);
        let me = self.online.user.clone().unwrap_or_default();
        // bubbles, newest at the bottom
        let mut heights = Vec::new();
        let maxw = area.w * 0.7;
        for m in &msgs {
            let text = m["body"].as_str().unwrap_or("");
            let lines = (self.ui.measure(text, 15.0, false) / (maxw - 24.0 * u)).ceil().max(1.0);
            heights.push(lines * 20.0 * u + 22.0 * u);
        }
        let total: f32 = heights.iter().map(|h| h + 8.0 * u).sum();
        let key = format!("conv{with}");
        // jump to the newest message when the chat opens or a message comes in; otherwise
        // leave the scrolling to the player
        let seen = format!("{with}:{}", msgs.len());
        if self.fields.get("dm_seen") != Some(&seen) {
            self.fields.insert("dm_seen", seen);
            self.ui.scroll_to_end(&key);
        }
        let off = self.ui.begin_scroll(&key, area, total);
        let mut y = area.y - off;
        for (m, h) in msgs.iter().zip(&heights) {
            let text = m["body"].as_str().unwrap_or("");
            let mine = m["from"].as_str() == Some(me.as_str());
            let tw = (self.ui.measure(text, 15.0, false) + 24.0 * u).min(maxw);
            let r = Rect::new(if mine { area.x + area.w - tw - 8.0 * u } else { area.x }, y, tw, *h);
            if self.ui.visible(r) {
                rounded(r, 14.0 * u, if mine { ACCENT_DARK } else { CARD_HI });
                self.ui.wrapped(text, r.x + 12.0 * u, r.y + 10.0 * u, tw - 24.0 * u, 15.0, if mine { WHITE } else { INK }, false);
            }
            y += h + 8.0 * u;
        }
        self.ui.end_scroll();
        let mut text = self.fields.remove("dm").unwrap_or_default();
        let ir = Rect::new(conv.x + 12.0 * u, conv.y + conv.h - 62.0 * u, conv.w - 130.0 * u, 48.0 * u);
        let entered = self.ui.text_input("dm", &mut text, ir, &format!("Message {with}"), false, 500);
        if (self.ui.button("Send", Rect::new(conv.x + conv.w - 108.0 * u, ir.y, 96.0 * u, 48.0 * u), Style::Primary) || entered) && !text.trim().is_empty() {
            self.online.post("/api/messages", json!({"to": with, "body": text.trim()}), Req::SendDm);
            text.clear();
            self.ui.focus = Some(id_of(&("input", "dm")));
        }
        self.fields.insert("dm", text);
    }

    // ---- the community leaderboard

    pub fn leaderboard_screen(&mut self) {
        let area = self.page("Leaderboard", "trophy");
        let u = self.ui.u;
        let Some(d) = self.online.rankings.clone() else {
            self.loading(area);
            return;
        };
        let parts = d["parts"].as_array().cloned().unwrap_or_default();
        let colors = [C_GOLD, C_BLUE, ACCENT, C_PURPLE];
        let mut lx = area.x;
        for (i, p) in parts.iter().enumerate() {
            let label = format!("{} (up to {})", p["label"].as_str().unwrap_or(""), p["most"].as_i64().unwrap_or(0));
            draw_circle(lx + 6.0 * u, area.y + 12.0 * u, 5.0 * u, colors[i % 4]);
            lx += 16.0 * u + self.ui.text(&label, lx + 16.0 * u, area.y + 12.0 * u, 13.0, MUTED, Align::Left, false) + 18.0 * u;
        }
        self.ui.text("Scores out of 100: world records and PRs in built-in speedruns, and snake points earned.",
                     area.x, area.y + 36.0 * u, 13.0, FAINT, Align::Left, false);
        let players = d["players"].as_array().cloned().unwrap_or_default();
        let list = Rect::new(area.x, area.y + 60.0 * u, area.w, area.h - 60.0 * u);
        if players.is_empty() {
            self.empty_state(list, "trophy", "Nobody has a score yet", "Set records and earn snake points to get on the board.", None);
            return;
        }
        let me = self.online.user.clone().unwrap_or_default().to_lowercase();
        let row = 62.0 * u;
        let wide = list.w > 820.0 * u;
        let off = self.ui.begin_scroll("rankings", list, players.len() as f32 * (row + 6.0 * u) + 80.0 * u);
        let medals = [C_GOLD, rgb(190, 198, 208), rgb(205, 127, 50)];
        let mut rows: Vec<Value> = players.clone();
        let mine = d["me"].clone();
        if !mine.is_null() && !players.iter().any(|p| p["name"].as_str().map(|s| s.to_lowercase()) == Some(me.clone())) {
            rows.push(Value::Null);
            rows.push(mine);
        }
        let mut y = list.y - off;
        for p in rows.iter() {
            if p.is_null() {
                self.ui.text("...", list.x + 20.0 * u, y + 12.0 * u, 18.0, MUTED, Align::Left, true);
                y += 30.0 * u;
                continue;
            }
            let r = Rect::new(list.x, y, list.w - 10.0 * u, row);
            y += row + 6.0 * u;
            if !self.ui.visible(r) {
                continue;
            }
            let name = p["name"].as_str().unwrap_or("").to_string();
            let you = name.to_lowercase() == me;
            rounded(r, 12.0 * u, if you { mix(CARD, ACCENT_DARK, 0.25) } else { CARD });
            let place = p["place"].as_i64().unwrap_or(0) as usize;
            if (1..=3).contains(&place) {
                draw_circle(r.x + 30.0 * u, r.y + row / 2.0, 16.0 * u, medals[place - 1]);
            }
            self.ui.text(&place.to_string(), r.x + 30.0 * u, r.y + row / 2.0, 16.0, if place <= 3 { rgb(30, 30, 30) } else { MUTED }, Align::Center, true);
            avatar(self, vec2(r.x + 78.0 * u, r.y + row / 2.0), 20.0 * u, p["avatar"].as_str().unwrap_or(""), &name);
            let n = self.ui.fit(&name, 17.0, true, r.w * 0.25);
            self.ui.text(&n, r.x + 108.0 * u, r.y + row / 2.0 - 9.0 * u, 17.0, INK, Align::Left, true);
            self.ui.text(p["rank"].as_str().unwrap_or(""), r.x + 108.0 * u, r.y + row / 2.0 + 12.0 * u, 12.0, MUTED, Align::Left, false);
            // the score bar, split into its parts
            let bx = r.x + r.w * if wide { 0.36 } else { 0.5 };
            let bw = r.w * if wide { 0.26 } else { 0.3 };
            let bar = Rect::new(bx, r.y + row / 2.0 - 7.0 * u, bw, 14.0 * u);
            rounded(bar, 7.0 * u, rgb(44, 58, 76));
            let mut x = bar.x;
            for (i, pt) in parts.iter().enumerate() {
                let key = pt["key"].as_str().unwrap_or("");
                let v = p["parts"][key].as_f64().unwrap_or(0.0) as f32;
                let seg = bw * v / 100.0;
                if seg >= 1.0 {
                    draw_rectangle(x, bar.y, seg, bar.h, colors[i % 4]);
                    x += seg;
                }
            }
            self.ui.text(&format!("{:.1}", p["score"].as_f64().unwrap_or(0.0)), bar.x + bw + 12.0 * u, r.y + row / 2.0, 18.0, INK, Align::Left, true);
            if wide {
                let cols = [(0.74, format!("{} WR", p["wrs"].as_i64().unwrap_or(0))), (0.82, format!("{} PR", p["prs"].as_i64().unwrap_or(0))),
                            (0.995, format!("{} earned", format_points(p["earned"].as_i64().unwrap_or(0))))];
                for (fx, t) in cols {
                    let al = if fx > 0.9 { Align::Right } else { Align::Left };
                    self.ui.text(&t, r.x + r.w * fx - if fx > 0.9 { 10.0 * u } else { 0.0 }, r.y + row / 2.0, 14.0, MUTED, al, true);
                }
            }
            if self.ui.pressed(Rect::new(r.x, r.y, r.w * 0.35, r.h)) {
                self.view_profile(&name);
            }
        }
        self.ui.end_scroll();
        if self.fields.get("view_profile").is_some_and(|s| !s.is_empty()) {
            self.profile_popup();
        }
    }
}
