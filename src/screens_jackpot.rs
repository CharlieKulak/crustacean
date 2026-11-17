//! The jackpot (Store > Jackpot): players put skins and packs into one pot, and one of
//! them, picked at random (a bigger share of the pot's value being a bigger chance), wins
//! everything in it. All in snake points and items: nothing here is real money.

use crate::app::App;
use crate::cosmetics::{catalog, format_points};
use crate::online::Req;
use crate::screens_shop::item_art;
use crate::screens_social::avatar;
use crate::ui::*;
use macroquad::prelude::*;
use serde_json::{json, Value};

/// How long the winner reel spins.
const DRAW_TIME: f64 = 5.5;

/// Each player's colour in the pot (in the order they joined).
const PLAYER_COLORS: [Color; 8] = [rgb(74, 222, 128), rgb(96, 165, 250), rgb(251, 191, 36), rgb(192, 132, 252),
                                   rgb(248, 113, 113), rgb(45, 212, 191), rgb(251, 146, 60), rgb(244, 114, 182)];

fn player_color(i: usize) -> Color {
    PLAYER_COLORS[i % PLAYER_COLORS.len()]
}

/// What an item counts for in the pot: its shop price, or else a value by rarity (as the server does).
pub fn item_value(item: &str) -> i64 {
    let cat = catalog();
    if let Some(p) = cat.item_price(item).filter(|p| *p > 0) {
        return p;
    }
    cat.economy["rarity_value"][cat.item_rarity(item)].as_i64().unwrap_or(500)
}

fn players(v: &Value) -> Vec<Value> {
    v["players"].as_array().cloned().unwrap_or_default()
}

impl App {
    pub fn jackpot(&mut self, body: Rect) {
        let u = self.ui.u;
        let now = get_time();
        // keep it fresh while it's open
        if self.online.jackpot.is_none() || now - self.online.jackpot_at > 1.0 {
            self.online.get("/api/jackpot", Req::Jackpot);
        }
        let Some(pot) = self.online.jackpot.clone() else {
            self.loading(body);
            return;
        };
        // a draw happened since we last looked: show it
        let last_round = pot["last"]["round"].as_i64();
        match (self.jackpot_seen, last_round) {
            (None, r) => self.jackpot_seen = Some(r.unwrap_or(0)),
            (Some(seen), Some(r)) if r > seen => {
                self.jackpot_seen = Some(r);
                self.jackpot_anim = Some((pot["last"].clone(), self.ui.time));
            }
            _ => {}
        }
        self.ui.text("Put skins in the pot. When the timer ends, one player wins everything: the more you put in, the better your chance.",
                     body.x, body.y + 14.0 * u, 14.0, MUTED, Align::Left, false);
        let top = body.y + 38.0 * u;
        let gap = 16.0 * u;
        let lw = ((body.w - gap) * 0.58).max(body.w.min(420.0 * u));
        let wide = body.w - lw - gap > 300.0 * u;
        let lw = if wide { lw } else { body.w };
        let left = Rect::new(body.x, top, lw, if wide { body.h - 38.0 * u } else { 420.0 * u });
        self.jackpot_pot(left, &pot, now);
        let right = if wide {
            Rect::new(body.x + lw + gap, top, body.w - lw - gap, body.h - 38.0 * u)
        } else {
            Rect::new(body.x, left.y + left.h + gap, body.w, body.h - left.h - 38.0 * u - gap)
        };
        self.jackpot_entry(right, &pot);
    }

    /// The pot: its value, the countdown, who's in with what and their chances.
    fn jackpot_pot(&mut self, r: Rect, pot: &Value, now: f64) {
        let u = self.ui.u;
        self.ui.card(r);
        let total = pot["total"].as_i64().unwrap_or(0);
        let ps = players(pot);
        // the value, with a coin
        glow(r.x + 50.0 * u, r.y + 46.0 * u, 50.0 * u, C_GOLD, 0.6);
        draw_icon("coin", r.x + 46.0 * u, r.y + 46.0 * u, 34.0 * u, C_GOLD);
        self.ui.text(&format_points(total), r.x + 76.0 * u, r.y + 40.0 * u, 34.0, INK, Align::Left, true);
        self.ui.text("snake points of skins in the pot", r.x + 78.0 * u, r.y + 70.0 * u, 13.0, MUTED, Align::Left, false);
        // the countdown
        let cx = r.x + r.w - 56.0 * u;
        let cy = r.y + 50.0 * u;
        let rad = 34.0 * u;
        match pot["seconds_left"].as_f64() {
            Some(left) => {
                let left = (left - (now - self.online.jackpot_at)).max(0.0);
                let full = pot["countdown"].as_f64().unwrap_or(45.0).max(1.0);
                draw_circle(cx, cy, rad, CARD_HI);
                let frac = (left / full).clamp(0.0, 1.0) as f32;
                let col = if left < 10.0 { C_RED } else { C_GOLD };
                let steps = 48;
                for k in 0..(steps as f32 * frac).ceil() as i32 {
                    let a0 = -std::f32::consts::FRAC_PI_2 + k as f32 / steps as f32 * std::f32::consts::TAU;
                    let a1 = a0 + std::f32::consts::TAU / steps as f32;
                    draw_line(cx + a0.cos() * rad, cy + a0.sin() * rad, cx + a1.cos() * rad, cy + a1.sin() * rad, 5.0 * u, col);
                }
                let label = if left <= 0.0 { "...".to_string() } else { format!("{}", left.ceil() as i64) };
                self.ui.text(&label, cx, cy - 2.0 * u, 24.0, INK, Align::Center, true);
                self.ui.text("to the draw", cx, cy + rad + 14.0 * u, 11.0, MUTED, Align::Center, false);
            }
            None => {
                let msg = if ps.is_empty() { "Empty pot: be the first in" } else { "Waiting for a second player" };
                self.ui.text(msg, r.x + r.w - 20.0 * u, cy, 13.0, MUTED, Align::Right, true);
            }
        }
        // everyone's chance, as one bar
        let bar = Rect::new(r.x + 20.0 * u, r.y + 104.0 * u, r.w - 40.0 * u, 16.0 * u);
        rounded(bar, 8.0 * u, CARD_HI);
        if total > 0 {
            let mut x = bar.x;
            for (i, p) in ps.iter().enumerate() {
                let w = bar.w * p["value"].as_f64().unwrap_or(0.0) as f32 / total as f32;
                draw_rectangle(x, bar.y, w.max(1.0), bar.h, player_color(i));
                x += w;
            }
            rounded_outline(bar, 8.0 * u, 2.0, CARD);
        }
        // the players
        let list = Rect::new(r.x + 12.0 * u, bar.y + bar.h + 14.0 * u, r.w - 24.0 * u, r.y + r.h - bar.y - bar.h - 26.0 * u);
        if ps.is_empty() {
            self.ui.text("Nobody's in yet.", list.x + list.w / 2.0, list.y + 50.0 * u, 16.0, MUTED, Align::Center, false);
            return;
        }
        let row = 92.0 * u;
        let me = self.online.user.clone().unwrap_or_default();
        let off = self.ui.begin_scroll("jackpot_players", list, ps.len() as f32 * (row + 8.0 * u));
        let mut tip: Option<(String, Vec2)> = None;
        for (i, p) in ps.iter().enumerate() {
            let pr = Rect::new(list.x, list.y + i as f32 * (row + 8.0 * u) - off, list.w - 12.0 * u, row);
            if !self.ui.visible(pr) {
                continue;
            }
            let name = p["name"].as_str().unwrap_or("?").to_string();
            let mine = name.eq_ignore_ascii_case(&me);
            rounded(pr, 12.0 * u, if mine { mix(CARD_HI, ACCENT, 0.08) } else { CARD_HI });
            rounded(Rect::new(pr.x, pr.y, 6.0 * u, pr.h), 3.0 * u, player_color(i));
            avatar(self, vec2(pr.x + 38.0 * u, pr.y + 30.0 * u), 20.0 * u, p["avatar"].as_str().unwrap_or(""), &name);
            let label = if mine { format!("{name} (you)") } else { name.clone() };
            self.ui.text(&label, pr.x + 66.0 * u, pr.y + 22.0 * u, 16.0, INK, Align::Left, true);
            self.ui.text(&format!("{} points", format_points(p["value"].as_i64().unwrap_or(0))), pr.x + 66.0 * u, pr.y + 44.0 * u,
                         13.0, MUTED, Align::Left, false);
            self.ui.text(&format!("{:.1}%", p["chance"].as_f64().unwrap_or(0.0)), pr.x + pr.w - 16.0 * u, pr.y + 30.0 * u, 22.0,
                         player_color(i), Align::Right, true);
            // their items, as little pictures
            let items: Vec<String> = p["items"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            let s = 30.0 * u;
            let room = ((pr.w - 82.0 * u) / (s + 4.0 * u)) as usize;
            for (k, it) in items.iter().take(room).enumerate() {
                let ir = Rect::new(pr.x + 16.0 * u + k as f32 * (s + 4.0 * u), pr.y + 56.0 * u, s, s);
                let cat = catalog();
                rounded(ir, 6.0 * u, CARD);
                rounded_outline(ir, 6.0 * u, 1.5, rgbt(cat.rarity_color(&cat.item_rarity(it))));
                item_art(it, Rect::new(ir.x + 2.0 * u, ir.y + 2.0 * u, ir.w - 4.0 * u, ir.h - 4.0 * u), self.now_ms());
                if self.ui.hover(ir) {
                    tip = Some((format!("{} ({} points)", cat.item_name(it), format_points(item_value(it))), vec2(ir.x, ir.y)));
                }
            }
            if items.len() > room {
                self.ui.text(&format!("+{}", items.len() - room), pr.x + 20.0 * u + room as f32 * (s + 4.0 * u), pr.y + 71.0 * u,
                             13.0, MUTED, Align::Left, true);
            }
        }
        self.ui.end_scroll();
        if let Some((text, at)) = tip {
            let tw = self.ui.measure(&text, 13.0, true) + 20.0 * u;
            let tr = Rect::new(at.x.min(self.ui.w - tw - 8.0 * u), at.y - 36.0 * u, tw, 30.0 * u);
            rounded(tr, 8.0 * u, CARD);
            rounded_outline(tr, 8.0 * u, 1.0, LINE);
            self.ui.text(&text, tr.x + 10.0 * u, tr.y + tr.h / 2.0, 13.0, INK, Align::Left, true);
        }
    }

    /// Your entry (pick what to put in) and the recent winners.
    fn jackpot_entry(&mut self, r: Rect, pot: &Value) {
        let u = self.ui.u;
        let cat = catalog();
        let max_items = cat.economy["jackpot_max_items"].as_u64().unwrap_or(20) as usize;
        let history: Vec<Value> = pot["history"].as_array().cloned().unwrap_or_default();
        let hist_h = if history.is_empty() { 0.0 } else { 40.0 * u + history.len().min(5) as f32 * 30.0 * u };
        let er = Rect::new(r.x, r.y, r.w, (r.h - hist_h - if hist_h > 0.0 { 14.0 * u } else { 0.0 }).max(260.0 * u));
        self.ui.card(er);
        self.ui.text("Your entry", er.x + 18.0 * u, er.y + 24.0 * u, 18.0, INK, Align::Left, true);
        let mine: Vec<(String, i64)> = {
            let me = self.online.me.clone().unwrap_or_default();
            let mut v: Vec<(String, i64)> = me["counts"].as_object()
                .map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(1))).collect()).unwrap_or_default();
            v.retain(|(it, n)| *n > 0 && !cat.untradeable.contains(it));
            v.sort_by_key(|(it, _)| (std::cmp::Reverse(item_value(it)), cat.item_name(it)));
            v
        };
        self.jackpot_pick.retain(|it| mine.iter().any(|(m, _)| m == it));
        let picked_value: i64 = self.jackpot_pick.iter().map(|it| item_value(it)).sum();
        let total = pot["total"].as_i64().unwrap_or(0);
        let me = self.online.user.clone().unwrap_or_default();
        let in_now = players(pot).iter().find(|p| p["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(&me)))
            .and_then(|p| p["value"].as_i64()).unwrap_or(0);
        let chance = if picked_value + total > 0 { 100.0 * (picked_value + in_now) as f64 / (picked_value + total) as f64 } else { 0.0 };
        // the items to pick from
        let area = Rect::new(er.x + 14.0 * u, er.y + 44.0 * u, er.w - 28.0 * u, er.h - 150.0 * u);
        rounded(area, 10.0 * u, alpha(BLACK, 0.18));
        if mine.is_empty() {
            self.ui.text("No skins or packs to put in. Open loot boxes or buy some first.", area.x + area.w / 2.0,
                         area.y + area.h / 2.0, 13.0, MUTED, Align::Center, false);
        }
        let chip_h = 34.0 * u;
        let cols = if area.w > 420.0 * u { 2 } else { 1 };
        let cw = (area.w - 24.0 * u - 8.0 * u * (cols - 1) as f32) / cols as f32;
        let off = self.ui.begin_scroll("jackpot_mine", area, mine.len().div_ceil(cols) as f32 * (chip_h + 6.0 * u) + 16.0 * u);
        for (i, (it, n)) in mine.iter().enumerate() {
            let cr = Rect::new(area.x + 8.0 * u + (i % cols) as f32 * (cw + 8.0 * u), area.y + 8.0 * u + (i / cols) as f32 * (chip_h + 6.0 * u) - off,
                               cw, chip_h);
            if !self.ui.visible(cr) {
                continue;
            }
            let picked = self.jackpot_pick.iter().filter(|p| *p == it).count() as i64;
            let label = format!("{}{}  ·  {}", cat.item_name(it), if *n > 1 { format!(" ({picked}/{n})") } else { String::new() },
                                format_points(item_value(it)));
            let label = self.ui.fit(&label, 13.0, true, cw - 16.0 * u);
            let col = rgbt(cat.rarity_color(&cat.item_rarity(it)));
            if self.ui.chip(&label, picked > 0, cr, col) {
                if picked < *n && self.jackpot_pick.len() < max_items {
                    self.jackpot_pick.push(it.clone()); // (another copy, while there are more)
                } else {
                    self.jackpot_pick.retain(|p| p != it);
                }
            }
        }
        self.ui.end_scroll();
        // what that comes to
        let iy = area.y + area.h + 14.0 * u;
        let summary = if self.jackpot_pick.is_empty() {
            "Pick skins or packs to put in.".to_string()
        } else {
            format!("{} item{}: {} points", self.jackpot_pick.len(), if self.jackpot_pick.len() == 1 { "" } else { "s" },
                    format_points(picked_value))
        };
        self.ui.text(&summary, er.x + 18.0 * u, iy + 8.0 * u, 14.0, INK, Align::Left, true);
        if !self.jackpot_pick.is_empty() || in_now > 0 {
            let line = if in_now > 0 && self.jackpot_pick.is_empty() { format!("Your chance now: {:.1}%", 100.0 * in_now as f64 / total.max(1) as f64) }
                       else { format!("Your chance would be {chance:.1}%") };
            self.ui.text(&line, er.x + er.w - 18.0 * u, iy + 8.0 * u, 14.0, ACCENT, Align::Right, true);
        }
        let busy = self.online.is_busy(&Req::JackpotEnter);
        let br = Rect::new(er.x + 14.0 * u, iy + 26.0 * u, er.w - 28.0 * u, 46.0 * u);
        let label = if busy { "Putting in...".to_string() } else if in_now > 0 { "Add to your entry".to_string() } else { "Put in the pot".to_string() };
        if self.ui.button_icon(&label, "coin", br, if self.jackpot_pick.is_empty() { Style::Secondary } else { Style::Gold }) && !busy {
            if self.jackpot_pick.is_empty() {
                self.ui.toast("Pick something to put in first.", C_RED);
            } else {
                self.online.post("/api/jackpot/enter", json!({"items": self.jackpot_pick}), Req::JackpotEnter);
            }
        }
        let note = self.ui.fit("Just for fun: values are in snake points. No real money goes in or comes out.",
                               11.0, false, er.w - 28.0 * u);
        self.ui.text(&note, er.x + er.w / 2.0, er.y + er.h - 12.0 * u, 11.0, FAINT, Align::Center, false);
        // recent winners
        if !history.is_empty() {
            let hy = er.y + er.h + 14.0 * u;
            let hr = Rect::new(r.x, hy, r.w, hist_h);
            self.ui.card(hr);
            self.ui.text("Recent winners", hr.x + 18.0 * u, hr.y + 22.0 * u, 15.0, INK, Align::Left, true);
            for (k, h) in history.iter().take(5).enumerate() {
                let y = hr.y + 50.0 * u + k as f32 * 30.0 * u;
                draw_icon("trophy", hr.x + 26.0 * u, y, 16.0 * u, C_GOLD);
                self.ui.text(h["winner"].as_str().unwrap_or("?"), hr.x + 44.0 * u, y, 14.0, INK, Align::Left, true);
                let t = format!("{} points from {} players", format_points(h["total"].as_i64().unwrap_or(0)), h["players"].as_i64().unwrap_or(0));
                self.ui.text(&t, hr.x + hr.w - 18.0 * u, y, 13.0, MUTED, Align::Right, false);
            }
        }
    }

    /// The draw: a reel of the players (each as often as their chance) slowing to the winner.
    pub fn jackpot_draw(&mut self) {
        let Some((last, started)) = self.jackpot_anim.clone() else { return };
        let u = self.ui.u;
        let (w, h) = (self.ui.w, self.ui.h);
        let elapsed = self.ui.time - started;
        self.ui.input = true;
        draw_rectangle(0.0, 0.0, w, h, alpha(rgb(4, 8, 12), 0.88));
        let ps = players(&last);
        let winner = last["winner"].as_str().unwrap_or("").to_string();
        let total = last["total"].as_i64().unwrap_or(0);
        let wi = ps.iter().position(|p| p["name"].as_str() == Some(winner.as_str())).unwrap_or(0);
        // the reel: tiles picked by chance (the same every frame), the winner's on tile 44
        let round = last["round"].as_u64().unwrap_or(1);
        let mut seed = round.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut tiles: Vec<usize> = (0..56).map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut t = (seed % 10_000) as f64 / 100.0;
            for (i, p) in ps.iter().enumerate() {
                t -= p["chance"].as_f64().unwrap_or(0.0);
                if t < 0.0 {
                    return i;
                }
            }
            ps.len().saturating_sub(1)
        }).collect();
        const LAND: usize = 44;
        tiles[LAND] = wi;
        let tw = 132.0 * u;
        let gap = 10.0 * u;
        let cy = h * 0.42;
        let t = (elapsed / DRAW_TIME).min(1.0);
        let ease = 1.0 - (1.0 - t).powi(4);
        let wobble = ((round % 7) as f64 / 7.0 - 0.5) * tw as f64 * 0.5;
        let target = LAND as f64 * (tw + gap) as f64 + wobble;
        let pos = (target * ease) as f32;
        let before = (target * (1.0 - (1.0 - ((elapsed - 1.0 / 60.0) / DRAW_TIME).clamp(0.0, 1.0)).powi(4))) as f32;
        if (before / (tw + gap)) as i64 != (pos / (tw + gap)) as i64 && t < 1.0 {
            self.sfx.play("tick");
        }
        self.ui.text("Jackpot draw", w / 2.0, cy - 140.0 * u, 28.0, INK, Align::Center, true);
        self.ui.text(&format!("{} points of skins, {} players", format_points(total), ps.len()), w / 2.0, cy - 108.0 * u, 15.0, MUTED,
                     Align::Center, false);
        self.ui.push_clip(Rect::new(0.0, cy - 90.0 * u, w, 180.0 * u));
        for (k, &pi) in tiles.iter().enumerate() {
            let x = w / 2.0 - tw / 2.0 + k as f32 * (tw + gap) - pos;
            if x > w || x + tw < 0.0 {
                continue;
            }
            let Some(p) = ps.get(pi) else { continue };
            let r = Rect::new(x, cy - 74.0 * u, tw, 148.0 * u);
            let landed = t >= 1.0 && k == LAND;
            rounded(r, 12.0 * u, if landed { mix(CARD, player_color(pi), 0.25) } else { CARD });
            rounded(Rect::new(r.x, r.y + r.h - 8.0 * u, r.w, 8.0 * u), 4.0 * u, player_color(pi));
            let name = p["name"].as_str().unwrap_or("?").to_string();
            avatar(self, vec2(r.x + tw / 2.0, r.y + 50.0 * u), 30.0 * u, p["avatar"].as_str().unwrap_or(""), &name);
            let label = self.ui.fit(&name, 14.0, true, tw - 12.0 * u);
            self.ui.text(&label, r.x + tw / 2.0, r.y + 102.0 * u, 14.0, INK, Align::Center, true);
            self.ui.text(&format!("{:.1}%", p["chance"].as_f64().unwrap_or(0.0)), r.x + tw / 2.0, r.y + 124.0 * u, 12.0, MUTED,
                         Align::Center, false);
        }
        self.ui.pop_clip();
        draw_triangle(vec2(w / 2.0 - 12.0 * u, cy - 94.0 * u), vec2(w / 2.0 + 12.0 * u, cy - 94.0 * u), vec2(w / 2.0, cy - 76.0 * u), C_GOLD);
        draw_triangle(vec2(w / 2.0 - 12.0 * u, cy + 94.0 * u), vec2(w / 2.0 + 12.0 * u, cy + 94.0 * u), vec2(w / 2.0, cy + 76.0 * u), C_GOLD);
        if t < 1.0 {
            draw_line(w / 2.0, cy - 76.0 * u, w / 2.0, cy + 76.0 * u, 2.0, alpha(C_GOLD, 0.7));
            if self.ui.hotkey(KeyCode::Space) || self.ui.clicked {
                if let Some(a) = self.jackpot_anim.as_mut() {
                    a.1 -= DRAW_TIME; // skip
                }
            }
            self.ui.input = false;
            return;
        }
        if elapsed - DRAW_TIME < 0.03 {
            self.sfx.play("reveal");
        }
        // the winner
        let me = self.online.user.clone().unwrap_or_default();
        let you = winner.eq_ignore_ascii_case(&me);
        let pop = (((elapsed - DRAW_TIME) * 4.0) as f32).min(1.0);
        let col = player_color(wi);
        for k in 0..10 {
            let a = self.ui.time as f32 * 0.5 + k as f32 * std::f32::consts::TAU / 10.0;
            let len = 260.0 * u * pop;
            draw_triangle(vec2(w / 2.0, cy), vec2(w / 2.0 + a.cos() * len, cy + a.sin() * len),
                          vec2(w / 2.0 + (a + 0.18).cos() * len, cy + (a + 0.18).sin() * len), alpha(col, 0.07));
        }
        let title = if you { "You won the pot!".to_string() } else { format!("{winner} won the pot!") };
        self.ui.text(&title, w / 2.0, cy + 122.0 * u, 30.0 * (0.8 + 0.2 * pop), if you { C_GOLD } else { INK }, Align::Center, true);
        let chance = ps.get(wi).and_then(|p| p["chance"].as_f64()).unwrap_or(0.0);
        let sub = format!("{} points of skins, with a {chance:.1}% chance", format_points(total));
        self.ui.text(&sub, w / 2.0, cy + 154.0 * u, 15.0, MUTED, Align::Center, false);
        let bw = 170.0 * u;
        if self.ui.button("Continue", Rect::new(w / 2.0 - bw / 2.0, cy + 180.0 * u, bw, 48.0 * u), Style::Primary)
            || self.ui.hotkey(KeyCode::Enter)
            || self.ui.hotkey(KeyCode::Escape)
        {
            self.jackpot_anim = None;
        }
        self.ui.input = false;
    }
}
