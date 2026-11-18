//! Story mode: eight chapters of ten levels, each ending in a boss duel. The map, the
//! level and boss cards, the bosses' portraits, the end of a level and the duel itself.

use crate::app::{App, Screen};
use crate::cosmetics::{catalog, format_points, hex, Skin};
use crate::game::Game;
use crate::online::Req;
use crate::render::{draw_board, View};
use crate::render3d::draw_board3d;
use crate::speedrun::{format_time, Category};
use crate::ui::*;
use macroquad::prelude::*;
use serde_json::Value;
use std::f32::consts::{PI, TAU};

/// Story mode's data: the server's view (with your stars) once it's in, else the catalogue's.
fn story_data(app: &App) -> Value {
    app.online.story.clone().unwrap_or_else(|| catalog().story.clone())
}

fn chapter_color(ch: &Value) -> Color {
    rgbt(hex(ch["color"].as_str().unwrap_or("#4caf50")))
}

fn boss_colors(b: &Value) -> [Color; 3] {
    let c = |i: usize| rgbt(hex(b["colors"][i].as_str().unwrap_or("#888888")));
    [c(0), c(1), c(2)]
}

/// The category a story level is played as (the server builds the same one).
pub fn level_category(ch: &Value, lv: &Value) -> Option<Category> {
    let id = lv["id"].as_str()?;
    let name = format!("{}-{} {}", ch["num"], lv["num"], lv["name"].as_str().unwrap_or(""));
    let desc = format!("Chapter {}, level {}", ch["num"], lv["num"]);
    Category::from_server(&format!("story-{id}"), &name, &desc, lv)
}

/// Find a level ("s3-07") or boss ("boss-3"): (chapter, level or boss).
fn find_stage(data: &Value, stage: &str) -> Option<(Value, Value)> {
    for ch in data["chapters"].as_array()? {
        if stage == format!("boss-{}", ch["num"]) {
            return Some((ch.clone(), ch["boss"].clone()));
        }
        if let Some(lv) = ch["levels"].as_array()?.iter().find(|l| l["id"].as_str() == Some(stage)) {
            return Some((ch.clone(), lv.clone()));
        }
    }
    None
}

/// Animations on the duel screen.
#[derive(Default)]
pub struct DuelFx {
    /// Attacks flying across: (from, to, started, colour, junk).
    pub orbs: Vec<(Vec2, Vec2, f64, Color, i64)>,
    /// Banners across the middle: (text, started, colour).
    pub banners: Vec<(String, f64, Color)>,
    /// Combo pops over a board: (side, chain, started).
    pub pops: Vec<(usize, i64, f64)>,
    pub shake: [f32; 2],
}

// ---- the bosses' portraits -------------------------------------------------------------

/// A boss's portrait: a big snake head in its colours, with what makes it itself.
pub fn boss_portrait(num: usize, r: Rect, now: f64, dim: bool) {
    let cat = catalog();
    let b = cat.story["chapters"][num.saturating_sub(1)]["boss"].clone();
    let [body, belly, accent] = boss_colors(&b);
    let t = (now / 1000.0) as f32;
    let s = r.w.min(r.h);
    let c = vec2(r.x + r.w / 2.0, r.y + r.h / 2.0);
    let k = |v: f32| v * s;
    // backdrop
    draw_circle(c.x, c.y, k(0.5), mix(rgb(14, 20, 30), accent, 0.18));
    for i in 0..6 {
        let a = t * 0.3 + i as f32 * TAU / 6.0;
        draw_triangle(c, c + vec2(a.cos(), a.sin()) * k(0.5), c + vec2((a + 0.3).cos(), (a + 0.3).sin()) * k(0.5),
                      alpha(accent, 0.07));
    }
    // the coiled body under the head
    let sway = (t * 1.6).sin() * k(0.02);
    for i in 0..9 {
        let a = PI * 0.15 + i as f32 * 0.32;
        let p = c + vec2(a.cos() * k(0.3) + sway, k(0.2) + a.sin() * k(0.12));
        draw_circle(p.x, p.y, k(0.11), if i % 2 == 0 { body } else { mix(body, belly, 0.35) });
    }
    if num == 8 {
        // the Ouroboros: a ring of snake eating its own tail, round the head
        let ring = k(0.4);
        for i in 0..36 {
            let a = i as f32 / 36.0 * TAU + t * 0.4;
            let p = c + vec2(a.cos(), a.sin()) * ring;
            let gold = ((i as f32 * 0.9 - t * 4.0).sin() * 0.5 + 0.5).powi(6);
            draw_circle(p.x, p.y, k(0.055), mix(mix(body, belly, (i % 2) as f32 * 0.3), accent, gold));
        }
        let a = t * 0.4;
        let hp = c + vec2(a.cos(), a.sin()) * ring;
        draw_circle(hp.x, hp.y, k(0.075), body);
        draw_circle(hp.x + a.sin() * k(0.03), hp.y - a.cos() * k(0.03), k(0.02), accent);
    }
    // the head
    let head = c + vec2(sway, -k(0.04));
    let hw = k(0.25);
    draw_circle(head.x, head.y, hw, body);
    draw_circle(head.x, head.y + k(0.06), hw * 0.82, mix(body, belly, 0.55));
    draw_circle(head.x - k(0.07), head.y - k(0.1), k(0.06), alpha(WHITE, 0.12));
    // the tongue, flicking
    if (t * 1.3).fract() < 0.35 {
        let ty = head.y + hw * 0.9;
        draw_line(head.x, ty, head.x, ty + k(0.08), k(0.018), rgb(220, 40, 60));
        draw_line(head.x, ty + k(0.08), head.x - k(0.025), ty + k(0.11), k(0.014), rgb(220, 40, 60));
        draw_line(head.x, ty + k(0.08), head.x + k(0.025), ty + k(0.11), k(0.014), rgb(220, 40, 60));
    }
    // eyes
    let blink = ((t * 0.7 + num as f32).fract() < 0.04) as i32 as f32;
    let eye = |x: f32, white: Color, pupil: Color, slit: bool| {
        let p = head + vec2(x, -k(0.03));
        draw_circle(p.x, p.y, k(0.065), white);
        if blink > 0.0 {
            draw_rectangle(p.x - k(0.065), p.y - k(0.012), k(0.13), k(0.024), body);
        } else if slit {
            draw_rectangle(p.x - k(0.012), p.y - k(0.045), k(0.024), k(0.09), pupil);
        } else {
            draw_circle(p.x + k(0.01), p.y + k(0.005), k(0.032), pupil);
            draw_circle(p.x - k(0.005), p.y - k(0.015), k(0.012), WHITE);
        }
    };
    let dark = rgb(20, 24, 32);
    match num {
        1 => {
            // Sir Wiggleton: a knight's helmet with a red plume
            eye(-k(0.1), WHITE, dark, false);
            eye(k(0.1), WHITE, dark, false);
            let top = head.y - hw * 0.35;
            draw_rectangle(head.x - hw * 0.95, top - k(0.13), hw * 1.9, k(0.14), rgb(150, 160, 175));
            draw_circle(head.x, top - k(0.13), hw * 0.95, rgb(150, 160, 175));
            draw_rectangle(head.x - hw * 0.95, top - k(0.02), hw * 1.9, k(0.03), rgb(110, 118, 130));
            draw_rectangle(head.x - k(0.015), top - k(0.3), k(0.03), k(0.2), rgb(110, 118, 130));
            for i in 0..7 {
                let a = -0.9 + i as f32 * 0.18 + (t * 2.0 + i as f32).sin() * 0.04;
                let base = vec2(head.x, top - k(0.32));
                draw_line(base.x, base.y, base.x + a.sin() * k(0.17), base.y - a.cos() * k(0.12) + k(0.02), k(0.04), accent);
            }
            draw_circle(head.x - hw * 0.5, top - k(0.18), k(0.025), alpha(WHITE, 0.4));
        }
        2 => {
            // Mamba Mae: a golden crown and long lashes
            eye(-k(0.1), rgb(250, 240, 160), dark, true);
            eye(k(0.1), rgb(250, 240, 160), dark, true);
            for x in [-k(0.1), k(0.1)] {
                for j in 0..3 {
                    let a = -0.6 + j as f32 * 0.5;
                    let p = head + vec2(x + a * k(0.04), -k(0.09));
                    draw_line(p.x, p.y, p.x + a * k(0.04), p.y - k(0.04), k(0.012), dark);
                }
            }
            let y = head.y - hw * 0.8;
            draw_rectangle(head.x - k(0.15), y - k(0.03), k(0.3), k(0.07), accent);
            for j in 0..3 {
                let x = head.x - k(0.12) + j as f32 * k(0.12);
                draw_triangle(vec2(x - k(0.05), y - k(0.02)), vec2(x + k(0.05), y - k(0.02)), vec2(x, y - k(0.14)), accent);
                draw_circle(x, y - k(0.13), k(0.018), if j == 1 { rgb(200, 30, 60) } else { rgb(60, 200, 120) });
            }
            draw_circle(head.x, y + k(0.005), k(0.022), rgb(200, 30, 60));
        }
        3 => {
            // Professor Coil: brass goggles and a curly moustache
            for x in [-k(0.1), k(0.1)] {
                let p = head + vec2(x, -k(0.03));
                draw_circle(p.x, p.y, k(0.085), rgb(181, 136, 60));
                draw_circle(p.x, p.y, k(0.065), mix(accent, WHITE, 0.15));
                draw_circle(p.x + k(0.015), p.y + k(0.01), k(0.03), dark);
                draw_circle(p.x - k(0.02), p.y - k(0.025), k(0.015), alpha(WHITE, 0.8));
            }
            draw_line(head.x - k(0.02), head.y - k(0.03), head.x + k(0.02), head.y - k(0.03), k(0.02), rgb(181, 136, 60));
            draw_line(head.x - k(0.3), head.y - k(0.06), head.x - k(0.18), head.y - k(0.04), k(0.02), rgb(181, 136, 60));
            draw_line(head.x + k(0.3), head.y - k(0.06), head.x + k(0.18), head.y - k(0.04), k(0.02), rgb(181, 136, 60));
            let my = head.y + k(0.08);
            for side in [-1.0f32, 1.0] {
                for j in 0..8 {
                    let a = j as f32 / 7.0;
                    let p = vec2(head.x + side * (k(0.02) + a * k(0.13)), my + (a * PI).sin() * k(0.03) - a * a * k(0.06));
                    draw_circle(p.x, p.y, k(0.022) * (1.0 - a * 0.5), rgb(70, 45, 25));
                }
            }
        }
        4 => {
            // Frostfang: ice horns, fangs and frost
            eye(-k(0.1), rgb(220, 245, 255), rgb(30, 90, 160), true);
            eye(k(0.1), rgb(220, 245, 255), rgb(30, 90, 160), true);
            for side in [-1.0f32, 1.0] {
                let b0 = head + vec2(side * k(0.13), -hw * 0.75);
                draw_triangle(b0 - vec2(k(0.05), 0.0), b0 + vec2(k(0.05), 0.0), b0 + vec2(side * k(0.12), -k(0.22)),
                              rgb(200, 235, 255));
                draw_triangle(b0, b0 + vec2(k(0.05), 0.0), b0 + vec2(side * k(0.12), -k(0.22)), alpha(WHITE, 0.5));
                let f = head + vec2(side * k(0.06), hw * 0.75);
                draw_triangle(f - vec2(k(0.025), 0.0), f + vec2(k(0.025), 0.0), f + vec2(0.0, k(0.08)), WHITE);
            }
            for i in 0..8 {
                let a = i as f32 * 0.8 + t * 0.6;
                let p = c + vec2(a.cos() * k(0.42), (a * 1.3).sin() * k(0.42));
                star(p.x, p.y, k(0.02), k(0.008), alpha(WHITE, 0.5 + 0.5 * (t * 3.0 + i as f32).sin().abs()));
            }
        }
        5 => {
            // Pyra: a crest of flames
            for i in 0..9 {
                let x = head.x - k(0.2) + i as f32 * k(0.05);
                let tall = k(0.16) + ((t * 9.0 + i as f32 * 1.7).sin() * 0.5 + 0.5) * k(0.1)
                    - (i as f32 - 4.0).abs() * k(0.015);
                let y = head.y - hw * 0.8;
                draw_triangle(vec2(x - k(0.04), y), vec2(x + k(0.04), y), vec2(x, y - tall), rgb(255, 120, 30));
                draw_triangle(vec2(x - k(0.022), y), vec2(x + k(0.022), y), vec2(x, y - tall * 0.6), accent);
            }
            eye(-k(0.1), rgb(255, 230, 120), rgb(160, 20, 10), true);
            eye(k(0.1), rgb(255, 230, 120), rgb(160, 20, 10), true);
            for i in 0..6 {
                let ph = (t * 0.7 + i as f32 * 0.17).fract();
                let p = vec2(c.x + ((i * 37 % 11) as f32 / 11.0 - 0.5) * k(0.7), c.y + k(0.4) - ph * k(0.8));
                draw_circle(p.x, p.y, k(0.012) * (1.0 - ph), alpha(accent, 1.0 - ph));
            }
        }
        6 => {
            // Umbra: a shadowy hood, only the eyes glow
            let top = head + vec2(0.0, -hw * 1.35);
            draw_triangle(top, head + vec2(-hw * 1.25, hw * 0.6), head + vec2(hw * 1.25, hw * 0.6), rgb(16, 10, 26));
            draw_circle(head.x, head.y + k(0.02), hw * 0.85, rgb(10, 6, 18));
            for x in [-k(0.09), k(0.09)] {
                let p = head + vec2(x, -k(0.01));
                glow(p.x, p.y, k(0.09), accent, 1.2);
                draw_triangle(p + vec2(-k(0.05), -k(0.01)), p + vec2(k(0.05), -k(0.01)), p + vec2(0.0, k(0.03)), accent);
            }
            for i in 0..10 {
                let ph = (t * 0.25 + i as f32 * 0.1).fract();
                let a = i as f32 * 2.4;
                let p = c + vec2(a.cos(), a.sin()) * k(0.2 + ph * 0.3);
                draw_circle(p.x, p.y, k(0.014), alpha(accent, (1.0 - ph) * 0.6));
            }
        }
        7 => {
            // Gearjaw: a turning gear behind, iron jaw and a scanning red eye
            let g = head + vec2(0.0, -hw * 0.9);
            let rot = t * 40.0;
            draw_poly(g.x, g.y, 10, k(0.16), rot, rgb(90, 98, 110));
            for i in 0..10 {
                let a = (rot + i as f32 * 36.0).to_radians();
                draw_rectangle(g.x + a.cos() * k(0.16) - k(0.025), g.y + a.sin() * k(0.16) - k(0.025), k(0.05), k(0.05),
                               rgb(90, 98, 110));
            }
            draw_circle(g.x, g.y, k(0.05), rgb(40, 44, 52));
            draw_circle(head.x, head.y, hw, body);
            draw_rectangle(head.x - hw * 0.85, head.y + k(0.02), hw * 1.7, hw * 0.75, rgb(70, 76, 86));
            for j in 0..5 {
                draw_circle(head.x - hw * 0.65 + j as f32 * hw * 0.32, head.y + k(0.06), k(0.012), rgb(180, 186, 196));
            }
            draw_rectangle(head.x - hw * 0.8, head.y - k(0.08), hw * 1.6, k(0.07), rgb(20, 22, 28));
            let scan = (t * 2.0).sin() * hw * 0.65;
            glow(head.x + scan, head.y - k(0.045), k(0.06), accent, 1.4);
            draw_rectangle(head.x + scan - k(0.04), head.y - k(0.07), k(0.08), k(0.05), accent);
        }
        _ => {
            // The Ouroboros: golden eyes and a crown of light
            eye(-k(0.1), accent, dark, true);
            eye(k(0.1), accent, dark, true);
            for i in 0..7 {
                let a = -PI / 2.0 + (i as f32 - 3.0) * 0.28;
                let p = head + vec2(a.cos(), a.sin()) * (hw * 1.1);
                let q = head + vec2(a.cos(), a.sin()) * (hw * 1.1 + k(0.05 + 0.03 * ((t * 3.0 + i as f32).sin() * 0.5 + 0.5)));
                draw_line(p.x, p.y, q.x, q.y, k(0.02), accent);
            }
        }
    }
    if dim {
        draw_circle(c.x, c.y, k(0.5), alpha(BLACK, 0.6));
        draw_icon("lock", c.x, c.y, k(0.3), alpha(WHITE, 0.85));
    }
}

// ---- the map --------------------------------------------------------------------------

impl App {
    pub fn story_screen(&mut self) {
        if self.story_pick.is_some() {
            self.ui.input = false; // (a card is open)
        }
        let area = self.page("Story", "crown");
        let u = self.ui.u;
        let data = story_data(self);
        let chapters = data["chapters"].as_array().cloned().unwrap_or_default();
        if chapters.is_empty() {
            self.loading(area);
            return;
        }
        let tab = self.tab(Screen::Story).min(chapters.len() - 1);
        // chapter tabs
        let n = chapters.len();
        let gap = 8.0 * u;
        let tw = ((area.w - gap * (n as f32 - 1.0)) / n as f32).min(150.0 * u);
        let total_w = tw * n as f32 + gap * (n as f32 - 1.0);
        for (i, ch) in chapters.iter().enumerate() {
            let r = Rect::new(area.x + (area.w - total_w) / 2.0 + i as f32 * (tw + gap), area.y, tw, 40.0 * u);
            let open = ch["levels"][0]["unlocked"] != false;
            let col = chapter_color(ch);
            let on = i == tab;
            let hover = self.ui.hover(r) && self.ui.input;
            rounded(r, 10.0 * u, if on { mix(CARD_HI, col, 0.35) } else if hover { CARD_HI } else { CARD });
            if on {
                rounded_outline(r, 10.0 * u, 2.0, col);
            }
            let label = if tw > 110.0 * u { format!("{}. {}", ch["num"], ch["name"].as_str().unwrap_or("")) } else { ch["num"].to_string() };
            let label = self.ui.fit(&label, 13.0, true, tw - 26.0 * u);
            self.ui.text(&label, r.x + r.w / 2.0, r.y + r.h / 2.0, 13.0, if open { INK } else { FAINT }, Align::Center, true);
            if !open {
                draw_icon("lock", r.x + r.w - 12.0 * u, r.y + 12.0 * u, 11.0 * u, FAINT);
            }
            if self.ui.pressed(r) && self.ui.input {
                self.set_tab(Screen::Story, i);
            }
        }
        for (k, i) in [(KeyCode::Left, -1i32), (KeyCode::Right, 1)] {
            if self.ui.hotkey(k) {
                self.set_tab(Screen::Story, (tab as i32 + i).clamp(0, n as i32 - 1) as usize);
            }
        }
        let ch = chapters[tab].clone();
        let col = chapter_color(&ch);
        // the chapter's banner
        let head = Rect::new(area.x, area.y + 54.0 * u, area.w, 86.0 * u);
        rounded(head, 14.0 * u, mix(CARD, col, 0.22));
        glow(head.x + 60.0 * u, head.y + head.h / 2.0, 70.0 * u, col, 0.8);
        self.ui.text(&format!("Chapter {}", ch["num"]), head.x + 24.0 * u, head.y + 24.0 * u, 13.0, alpha(INK, 0.7), Align::Left, true);
        self.ui.text(ch["name"].as_str().unwrap_or(""), head.x + 24.0 * u, head.y + 50.0 * u, 26.0, INK, Align::Left, true);
        let blurb = self.ui.fit(ch["blurb"].as_str().unwrap_or(""), 13.0, false, head.w * 0.55);
        self.ui.text(&blurb, head.x + 24.0 * u, head.y + 72.0 * u, 13.0, MUTED, Align::Left, false);
        let levels = ch["levels"].as_array().cloned().unwrap_or_default();
        let got: i64 = levels.iter().map(|l| l["stars"].as_i64().unwrap_or(0)).sum();
        star(head.x + head.w - 130.0 * u, head.y + 34.0 * u, 14.0 * u, 6.0 * u, C_GOLD);
        self.ui.text(&format!("{got} / {}", levels.len() * 3), head.x + head.w - 110.0 * u, head.y + 34.0 * u, 20.0, INK, Align::Left, true);
        let per = data["star_points"][ch["num"].to_string()].as_i64().unwrap_or(0);
        self.ui.text(&format!("{} points a star", format_points(per)), head.x + head.w - 24.0 * u, head.y + 62.0 * u, 12.0, MUTED,
                     Align::Right, false);
        // the path: ten levels winding to the boss
        let map = Rect::new(area.x, head.y + head.h + 14.0 * u, area.w, area.h - head.h - 68.0 * u - 40.0 * u);
        let boss_r = (map.h * 0.3).min(88.0 * u);
        let pts: Vec<Vec2> = (0..11).map(|i| {
            let f = i as f32 / 10.0;
            let x = map.x + 50.0 * u + f * (map.w - 100.0 * u - boss_r * 0.6);
            let y = map.y + map.h * 0.5 + (f * PI * 2.6 + 0.4).sin() * (map.h * 0.27);
            vec2(x, if i == 10 { map.y + map.h * 0.5 } else { y })
        }).collect();
        let now = self.now_ms();
        let first_open = levels.iter().position(|l| l["unlocked"] != false && l["stars"].as_i64().unwrap_or(0) == 0);
        for i in 0..10 {
            let (a, b) = (pts[i], pts[i + 1]);
            let open = levels.get(i + 1).map_or(ch["boss"]["unlocked"] == true, |l| l["unlocked"] != false);
            let segs = 12;
            for k in 0..segs {
                let p = a.lerp(b, k as f32 / segs as f32);
                let wob = ((k as f32 + i as f32 * 3.0) * 0.9 + now as f32 / 300.0).sin() * 1.5 * u;
                draw_circle(p.x, p.y + wob, if open { 9.0 } else { 5.0 } * u,
                            if open { alpha(col, 0.55) } else { alpha(FAINT, 0.35) });
            }
        }
        let pick_lock = self.story_pick.is_some();
        for (i, lv) in levels.iter().enumerate() {
            let p = pts[i];
            let open = lv["unlocked"] != false;
            let stars = lv["stars"].as_i64().unwrap_or(0);
            let r = 30.0 * u;
            let hit = Rect::new(p.x - r, p.y - r, r * 2.0, r * 2.0);
            let hover = self.ui.hover(hit) && !pick_lock;
            let grow = self.ui.anim(crate::ui::id_of(&("story_node", i, tab)), if hover { 1.0 } else { 0.0 }, 12.0);
            let rr = r * (1.0 + 0.12 * grow);
            if first_open == Some(i) {
                glow(p.x, p.y, rr * 2.2, col, 0.9 + 0.3 * (now as f32 / 300.0).sin());
            }
            draw_circle(p.x, p.y + 4.0 * u, rr, alpha(BLACK, 0.3));
            draw_circle(p.x, p.y, rr, if open { mix(CARD_HI, col, if stars > 0 { 0.65 } else { 0.3 }) } else { CARD });
            draw_circle_lines(p.x, p.y, rr, 2.5 * u, if open { lighten(col, 0.2) } else { LINE });
            if open {
                self.ui.text(&lv["num"].to_string(), p.x, p.y - 2.0 * u, 20.0, INK, Align::Center, true);
            } else {
                draw_icon("lock", p.x, p.y, 18.0 * u, FAINT);
            }
            for k in 0..3 {
                let sx = p.x + (k as f32 - 1.0) * 15.0 * u;
                let sy = p.y + rr + 8.0 * u + if k == 1 { 3.0 * u } else { 0.0 };
                star(sx, sy, 8.0 * u, 3.5 * u, if (k as i64) < stars { C_GOLD } else { alpha(FAINT, 0.6) });
            }
            if hover {
                let name = lv["name"].as_str().unwrap_or("");
                let w_ = self.ui.measure(name, 13.0, true) + 20.0 * u;
                let tr = Rect::new(p.x - w_ / 2.0, p.y - rr - 38.0 * u, w_, 26.0 * u);
                rounded(tr, 8.0 * u, CARD);
                self.ui.text(name, p.x, tr.y + 13.0 * u, 13.0, INK, Align::Center, true);
            }
            if self.ui.pressed(hit) && self.ui.input && open {
                self.story_pick = lv["id"].as_str().map(String::from);
            } else if self.ui.pressed(hit) && self.ui.input {
                self.ui.toast("Finish the level before it first.", C_RED);
            }
        }
        // the boss at the end
        let boss = &ch["boss"];
        let bp = pts[10];
        let open = boss["unlocked"] == true;
        let beaten = boss["beaten"] == true;
        let hit = Rect::new(bp.x - boss_r, bp.y - boss_r, boss_r * 2.0, boss_r * 2.0);
        let hover = self.ui.hover(hit) && !pick_lock;
        if open && !beaten {
            glow(bp.x, bp.y, boss_r * 1.8, boss_colors(boss)[2], 1.0 + 0.3 * (now as f32 / 250.0).sin());
        }
        let bs = boss_r * if hover { 2.1 } else { 2.0 };
        boss_portrait(boss["num"].as_u64().unwrap_or(1) as usize, Rect::new(bp.x - bs / 2.0, bp.y - bs / 2.0, bs, bs), now, !open);
        draw_circle_lines(bp.x, bp.y, bs / 2.0, 3.0 * u, if beaten { C_GOLD } else if open { boss_colors(boss)[2] } else { LINE });
        self.ui.text(boss["name"].as_str().unwrap_or(""), bp.x, bp.y + boss_r + 18.0 * u, 16.0, INK, Align::Center, true);
        let tag = if beaten { "Beaten" } else if open { "Boss: ready to fight" } else { "Boss" };
        self.ui.text(tag, bp.x, bp.y + boss_r + 38.0 * u, 12.0, if beaten { C_GOLD } else if open { ACCENT } else { FAINT },
                     Align::Center, true);
        if beaten {
            draw_icon("crown", bp.x + boss_r * 0.7, bp.y - boss_r * 0.75, 26.0 * u, C_GOLD);
        }
        if self.ui.pressed(hit) && self.ui.input {
            self.story_pick = Some(format!("boss-{}", boss["num"]));
        }
        // the whole story
        let fy = area.y + area.h - 18.0 * u;
        let total = data["stars"].as_i64().unwrap_or(0);
        let note = if self.online.logged_in() {
            format!("{total} of {} stars   ·   {} of 8 bosses beaten", data["max_stars"].as_i64().unwrap_or(240),
                    data["bosses_beaten"].as_i64().unwrap_or(0))
        } else {
            "Log in to save your progress and earn stars and rewards.".to_string()
        };
        self.ui.text(&note, area.x + area.w / 2.0, fy, 14.0, MUTED, Align::Center, true);
        // a level's or boss's card
        if let Some(pick) = self.story_pick.clone() {
            match find_stage(&data, &pick) {
                Some((ch, st)) if pick.starts_with("boss-") => self.boss_card(&ch, &st),
                Some((ch, st)) => self.level_card(&ch, &st),
                None => self.story_pick = None,
            }
        }
    }

    fn card_frame(&mut self, w: f32, h: f32) -> Rect {
        let u = self.ui.u;
        let (sw, sh) = (self.ui.w, self.ui.h);
        let w = w.min(sw - 24.0 * u);
        let h = h.min(sh - 24.0 * u);
        self.ui.input = true;
        draw_rectangle(0.0, 0.0, sw, sh, alpha(BLACK, 0.6));
        let appear = self.ui.anim(crate::ui::id_of(&"story_card"), 1.0, 12.0);
        let r = Rect::new((sw - w) / 2.0, (sh - h) / 2.0 + (1.0 - appear) * 20.0 * u, w, h);
        self.ui.card(r);
        r
    }

    fn level_card(&mut self, ch: &Value, lv: &Value) {
        let u = self.ui.u;
        let r = self.card_frame(560.0 * u, 470.0 * u);
        let col = chapter_color(ch);
        rounded(Rect::new(r.x, r.y, r.w, 8.0 * u), 4.0 * u, col);
        self.ui.text(&format!("Chapter {}, level {}", ch["num"], lv["num"]), r.x + 28.0 * u, r.y + 34.0 * u, 13.0, MUTED, Align::Left, true);
        self.ui.text(lv["name"].as_str().unwrap_or(""), r.x + 28.0 * u, r.y + 64.0 * u, 28.0, INK, Align::Left, true);
        let cat = catalog();
        let modes: Vec<String> = lv["modes"].as_array().map(|a| a.iter().filter_map(|m| m.as_str().map(String::from)).collect()).unwrap_or_default();
        let mode_text = if modes.is_empty() { "Classic".to_string() } else { modes.iter().map(|m| cat.mode_name(m)).collect::<Vec<_>>().join(" + ") };
        let speed = 1000.0 / lv["tick_ms"].as_f64().unwrap_or(130.0);
        let mut rules = format!("{} x {} board   ·   {mode_text}   ·   {speed:.1} moves a second", lv["w"], lv["h"]);
        if lv["accel"].as_f64().unwrap_or(0.0) > 0.0 {
            rules += "   ·   speeds up as you eat";
        }
        let rules = self.ui.fit(&rules, 13.0, false, r.w - 56.0 * u);
        self.ui.text(&rules, r.x + 28.0 * u, r.y + 94.0 * u, 13.0, MUTED, Align::Left, false);
        let mut y = r.y + 136.0 * u;
        let have = lv["stars"].as_i64().unwrap_or(0);
        let goal = format!("Eat {} fruit", lv["goal"]);
        let texts: Vec<String> = std::iter::once(format!("Finish: {goal}"))
            .chain(lv["challenge_text"].as_array().cloned().unwrap_or_default().iter().map(|t| t.as_str().unwrap_or("").to_string()))
            .collect();
        for (k, t) in texts.iter().enumerate() {
            let row = Rect::new(r.x + 24.0 * u, y, r.w - 48.0 * u, 52.0 * u);
            rounded(row, 10.0 * u, CARD_HI);
            star(row.x + 30.0 * u, row.y + row.h / 2.0, 14.0 * u, 6.0 * u, if (k as i64) < have { C_GOLD } else { alpha(FAINT, 0.7) });
            self.ui.text(t, row.x + 56.0 * u, row.y + row.h / 2.0, 16.0, INK, Align::Left, true);
            y += 60.0 * u;
        }
        let per = story_data(self)["star_points"][ch["num"].to_string()].as_i64().unwrap_or(0);
        let reward = format!("+{} snake points for each new star", format_points(per));
        self.ui.text(&reward, r.x + 28.0 * u, y + 14.0 * u, 14.0, C_GOLD, Align::Left, true);
        if let Some(best) = lv["best_ms"].as_f64() {
            self.ui.text(&format!("Your best: {}", format_time(best)), r.x + r.w - 28.0 * u, y + 14.0 * u, 14.0, MUTED, Align::Right, true);
        }
        let by = r.y + r.h - 72.0 * u;
        let bw = (r.w - 60.0 * u) / 2.0;
        if self.ui.button("Close", Rect::new(r.x + 24.0 * u, by, bw, 50.0 * u), Style::Secondary) || self.ui.hotkey(KeyCode::Escape) {
            self.story_pick = None;
        }
        if self.ui.button_icon("Play", "play", Rect::new(r.x + 36.0 * u + bw, by, bw, 50.0 * u), Style::Primary)
            || self.ui.hotkey(KeyCode::Enter)
        {
            if let Some(cat) = level_category(ch, lv) {
                self.story_pick = None;
                self.start_run(cat, Screen::Story);
            }
        }
        self.ui.input = false;
    }

    fn boss_card(&mut self, ch: &Value, b: &Value) {
        let u = self.ui.u;
        let r = self.card_frame(760.0 * u, 540.0 * u);
        let [_body, _belly, accent] = boss_colors(b);
        let num = b["num"].as_u64().unwrap_or(1) as usize;
        let open = b["unlocked"] == true;
        let beaten = b["beaten"] == true;
        rounded(Rect::new(r.x, r.y, r.w, 8.0 * u), 4.0 * u, accent);
        let ps = (r.w * 0.36).min(r.h - 140.0 * u);
        boss_portrait(num, Rect::new(r.x + 24.0 * u, r.y + 36.0 * u, ps, ps), self.now_ms(), false);
        let tx = r.x + ps + 52.0 * u;
        let tw = r.x + r.w - 28.0 * u - tx;
        self.ui.text(&format!("Chapter {} boss", ch["num"]), tx, r.y + 40.0 * u, 13.0, MUTED, Align::Left, true);
        self.ui.text(b["name"].as_str().unwrap_or(""), tx, r.y + 72.0 * u, 30.0, INK, Align::Left, true);
        self.ui.text(b["title"].as_str().unwrap_or(""), tx, r.y + 100.0 * u, 15.0, accent, Align::Left, true);
        let taunt = format!("\"{}\"", b["taunt"].as_str().unwrap_or(""));
        let mut y = r.y + 120.0 * u + self.ui.wrapped(&taunt, tx, r.y + 120.0 * u, tw, 14.0, MUTED, false) + 14.0 * u;
        let desc = b["attack_desc"].as_str().unwrap_or("").to_string();
        let lines = if self.ui.measure(&desc, 13.0, false) > tw - 32.0 * u { 2.0 } else { 1.0 };
        let sig = Rect::new(tx, y, tw, (52.0 + 18.0 * lines) * u);
        rounded(sig, 10.0 * u, mix(CARD_HI, accent, 0.12));
        draw_icon("bolt", sig.x + 22.0 * u, sig.y + 22.0 * u, 18.0 * u, accent);
        self.ui.text(&format!("Signature: {}", b["attack_name"].as_str().unwrap_or("")), sig.x + 40.0 * u, sig.y + 22.0 * u, 15.0, INK,
                     Align::Left, true);
        self.ui.wrapped(&desc, sig.x + 16.0 * u, sig.y + 38.0 * u, sig.w - 32.0 * u, 13.0, MUTED, false);
        y += sig.h + 14.0 * u;
        for line in ["You each get your own board. Eat fruit quickly, one after another, for a combo.",
                     "When a combo ends it drops junk walls on the boss's board (and cancels junk coming at you).",
                     "First to crash loses."] {
            draw_circle(tx + 5.0 * u, y + 8.0 * u, 3.0 * u, accent);
            y += self.ui.wrapped(line, tx + 16.0 * u, y, tw - 16.0 * u, 13.0, INK, false) + 6.0 * u;
        }
        // rewards
        let ry = r.y + ps + 50.0 * u;
        let rx = r.x + 24.0 * u;
        let title = if beaten { "Beaten! Rematches pay a little" } else { "Beat it for" };
        self.ui.text(title, rx, ry, 14.0, if beaten { C_GOLD } else { MUTED }, Align::Left, true);
        let names: Vec<String> = b["reward_names"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        let mut x = rx;
        let mut yy = ry + 18.0 * u;
        for n in &names {
            let w_ = self.ui.measure(n, 13.0, true) + 22.0 * u;
            if x + w_ > r.x + ps + 30.0 * u && x > rx {
                x = rx;
                yy += 32.0 * u;
            }
            let pr = Rect::new(x, yy, w_, 26.0 * u);
            rounded(pr, 13.0 * u, mix(CARD_HI, C_GOLD, if beaten { 0.05 } else { 0.18 }));
            self.ui.text(n, pr.x + pr.w / 2.0, pr.y + 13.0 * u, 13.0, if beaten { MUTED } else { C_GOLD }, Align::Center, true);
            x += w_ + 6.0 * u;
        }
        // buttons
        let by = r.y + r.h - 72.0 * u;
        let bw = 200.0 * u;
        if self.ui.button("Close", Rect::new(r.x + r.w - 2.0 * bw - 40.0 * u, by, bw, 50.0 * u), Style::Secondary)
            || self.ui.hotkey(KeyCode::Escape)
        {
            self.story_pick = None;
        }
        let fight = Rect::new(r.x + r.w - bw - 24.0 * u, by, bw, 50.0 * u);
        if !self.online.logged_in() {
            self.ui.text("Log in to fight", fight.x + fight.w / 2.0, fight.y + fight.h / 2.0, 15.0, MUTED, Align::Center, true);
        } else if !open {
            self.ui.text("Finish the 10 levels first", fight.x + fight.w / 2.0, fight.y + fight.h / 2.0, 14.0, MUTED, Align::Center, true);
        } else if self.ui.button_icon(if beaten { "Rematch" } else { "Fight!" }, "bolt", fight, Style::Danger) || self.ui.hotkey(KeyCode::Enter) {
            self.story_pick = None;
            self.online.start_duel(num);
        }
        self.ui.input = false;
    }

    // ---- the end of a level --------------------------------------------------------------

    pub fn story_over_card(&mut self) {
        let u = self.ui.u;
        let (w, h) = (self.ui.w, self.ui.h);
        let p = self.play.as_ref().unwrap();
        let r = p.run.as_ref().unwrap();
        let finished = r.finished;
        let stage = r.category.id.trim_start_matches("story-").to_string();
        let res = p.result.clone();
        let time = r.elapsed;
        let title = if finished { "Level complete!" } else { "Out!" };
        let sub = if finished { format_time(time) } else { format!("{} / {} fruit", r.game.score, r.category.goal.unwrap_or(0)) };
        let extra = match &res {
            Some(d) if d["valid"] == true => {
                let pts = d["points_earned"].as_i64().unwrap_or(0);
                if pts > 0 { format!("+{} snake points!", format_points(pts)) } else { "No new stars this time".into() }
            }
            Some(d) => d["message"].as_str().or(d["error"].as_str()).unwrap_or("The server didn't accept the run.").to_string(),
            None if finished && self.online.logged_in() => "Checking your run with the server...".into(),
            None if finished => "Log in to earn stars and rewards.".into(),
            None => "Press R to try again".into(),
        };
        let next_ok = res.as_ref().is_some_and(|d| d["next_unlocked"] == true);
        let buttons: Vec<&str> = if finished && next_ok { vec!["Next", "Retry", "Map"] } else { vec!["Retry", "Map"] };
        let picked = self.overlay_card(title, &sub, &extra, &buttons);
        // the stars, big, above the card
        if finished {
            let earned: Vec<bool> = res.as_ref().and_then(|d| d["earned"].as_array().map(|a| a.iter().map(|x| x == true).collect()))
                .unwrap_or_else(|| vec![true, false, false]);
            let cy = (h - 250.0 * u) / 2.0 - 34.0 * u;
            for k in 0..3 {
                let pop = self.ui.anim(crate::ui::id_of(&("story_star", k, stage.as_str())), 1.0, 6.0 - k as f32 * 1.5);
                let x = w / 2.0 + (k as f32 - 1.0) * 70.0 * u;
                let y = cy - if k == 1 { 12.0 * u } else { 0.0 };
                let size = 30.0 * u * (0.6 + 0.4 * pop);
                let on = earned.get(k).copied().unwrap_or(false);
                if on {
                    glow(x, y, size * 1.6, C_GOLD, 0.6 * pop);
                }
                star(x, y, size, size * 0.45, if on { C_GOLD } else { alpha(rgb(60, 70, 85), 0.9) });
            }
        }
        match (picked, buttons.len()) {
            (Some(0), 3) => {
                let nxt = res.as_ref().and_then(|d| d["next"].as_str().map(String::from)).unwrap_or_default();
                self.play = None;
                self.go(Screen::Story);
                self.story_pick = Some(nxt);
            }
            (Some(i), n) if i == n - 2 => self.restart(),
            (Some(_), _) => {
                self.play = None;
                self.go(Screen::Story);
            }
            _ => {}
        }
        if self.ui.hotkey(KeyCode::Enter) {
            self.restart();
        }
    }

    /// The challenges, live, under the HUD while a story level is played.
    pub fn story_hud(&mut self) {
        let u = self.ui.u;
        let Some(p) = &self.play else { return };
        let Some(r) = &p.run else { return };
        let Some(stage) = r.category.id.strip_prefix("story-") else { return };
        let data = story_data(self);
        let Some((_ch, lv)) = find_stage(&data, stage) else { return };
        let mut chips = Vec::new();
        for (c, text) in lv["challenges"].as_array().cloned().unwrap_or_default().iter()
            .zip(lv["challenge_text"].as_array().cloned().unwrap_or_default())
        {
            let kind = c[0].as_str().unwrap_or("");
            let a = c[1].as_f64().unwrap_or(0.0);
            let b = c[2].as_f64().unwrap_or(0.0);
            let ok = match kind {
                "time" => r.elapsed <= a,
                "turns" => (r.inputs.len() as f64) <= a + 1.0,
                "split" => r.splits.get(a as usize - 1).map_or(r.elapsed <= b, |&s| s <= b),
                _ => true,
            };
            chips.push((text.as_str().unwrap_or("").to_string(), ok));
        }
        let mut x = self.ui.w - 16.0 * u;
        let y = 64.0 * u + 10.0 * u;
        for (text, ok) in chips.iter().rev() {
            let w_ = self.ui.measure(text, 12.0, true) + 34.0 * u;
            let cr = Rect::new(x - w_, y, w_, 26.0 * u);
            rounded(cr, 13.0 * u, alpha(if *ok { mix(CARD, C_GOLD, 0.2) } else { CARD }, 0.92));
            star(cr.x + 14.0 * u, cr.y + 13.0 * u, 7.0 * u, 3.0 * u, if *ok { C_GOLD } else { FAINT });
            self.ui.text(text, cr.x + 26.0 * u, cr.y + 13.0 * u, 12.0, if *ok { INK } else { FAINT }, Align::Left, true);
            x -= w_ + 8.0 * u;
        }
    }

    // ---- the duel ------------------------------------------------------------------------

    pub fn duel_screen(&mut self) {
        let u = self.ui.u;
        let (w, h) = (self.ui.w, self.ui.h);
        let (Some(mt), Some(du)) = (&self.online.game, &self.online.duel) else {
            self.go(if self.online.room.is_some() { Screen::Lobby } else { Screen::Story });
            return;
        };
        let now = self.now_ms();
        let tsec = self.ui.time;
        let cat = catalog();
        let battle = du.boss == 0;
        let boss_num = du.boss;
        let b = if battle { Value::Null } else { cat.story["chapters"][boss_num - 1]["boss"].clone() };
        let accent = if battle { C_PURPLE } else { boss_colors(&b)[2] };
        let sides = du.sides.clone();
        let n = sides.as_array().map_or(2, |a| a.len()).max(1);
        let you = du.you;
        let window = b["combo_window"].as_f64().unwrap_or(8.0);
        let players = du.players.clone();
        let t = if mt.game.over { 1.0 } else { (mt.acc / mt.tick_ms).min(1.0) as f32 };
        let events: Vec<Value> = self.online.duel.as_mut().map(|d| std::mem::take(&mut d.events)).unwrap_or_default();
        // the layout: up to three boards in a row, four in a square
        let hud_h = 64.0 * u;
        let strip_h = if n > 3 { 56.0 } else { 76.0 } * u;
        let gap = if n > 2 { 14.0 } else { 24.0 } * u;
        let (cols, rows) = if n == 4 { (2, 2) } else { (n, 1) };
        let cell_w = (w - 32.0 * u - gap * (cols as f32 - 1.0)) / cols as f32;
        let cell_h = (h - hud_h - 16.0 * u - gap * (rows as f32 - 1.0)) / rows as f32;
        let slot = |i: usize| -> (Rect, Rect) {
            let (c, r) = (i % cols, i / cols);
            let x = 16.0 * u + c as f32 * (cell_w + gap);
            let y = hud_h + 8.0 * u + r as f32 * (cell_h + gap);
            (Rect::new(x, y, cell_w, strip_h), Rect::new(x, y + strip_h + 8.0 * u, cell_w, cell_h - strip_h - 8.0 * u))
        };
        let center = |i: usize| { let (_s, a) = slot(i); vec2(a.x + a.w / 2.0, a.y + a.h / 2.0) };
        let tray = |i: usize| { let (s, _a) = slot(i); vec2(s.x + s.w - 60.0 * u, s.y + s.h / 2.0) };
        let name_of = |i: usize| -> String {
            if battle {
                players.get(i).and_then(|p| p["name"].as_str()).unwrap_or("?").to_string()
            } else if i == 0 {
                "You".into()
            } else {
                b["name"].as_str().unwrap_or("The boss").to_string()
            }
        };
        for ev in &events {
            let kind = ev[0].as_str().unwrap_or("");
            let side = (ev[1].as_i64().unwrap_or(0).max(0) as usize).min(n - 1);
            let amount = ev[2].as_i64().unwrap_or(0);
            match kind {
                "attack" if amount > 0 => {
                    let to = ev[4].as_i64().filter(|&x| x >= 0).map_or(1 - side.min(1), |x| (x as usize).min(n - 1));
                    let col = if side == you { C_BLUE } else if battle { rgb(255, 140, 70) } else { accent };
                    self.duel_fx.orbs.push((center(side), tray(to), tsec, col, amount));
                    self.sfx.play("portal");
                }
                "junk" => {
                    if side == you {
                        self.duel_fx.shake[0] = 0.35; // (your board shakes when junk lands on it)
                    }
                    self.sfx.play(if side == you { "die" } else { "unlock" });
                }
                "chain" if amount >= 2 => self.duel_fx.pops.push((side, amount, tsec)),
                "signature" => {
                    let which = ev[3].as_str().unwrap_or("");
                    self.duel_fx.banners.push((format!("{} used {}!", name_of(1), signature_name(which)), tsec, accent));
                    self.sfx.play("reveal");
                }
                "out" if battle => {
                    let who = if side == you { "You're".to_string() } else { format!("{} is", name_of(side)) };
                    self.duel_fx.banners.push((format!("{who} out!"), tsec, C_RED));
                }
                "sudden" => self.duel_fx.banners.push(("Sudden death!".into(), tsec, C_RED)),
                _ => {}
            }
        }
        // the boards
        let my_skin = self.my_skin();
        let boss_skin: Skin = b["skin"].as_str().and_then(|s| s.strip_prefix("skin:")).and_then(|k| cat.skin(k)).cloned()
            .unwrap_or_else(|| crate::cosmetics::team_skin(1, 0));
        let colors = self.board_colors();
        let shake_you = self.duel_fx.shake[0];
        let mt = self.online.game.as_ref().unwrap();
        let du = self.online.duel.as_ref().unwrap();
        for side in 0..n {
            let g: &Game = if side == you { &mt.game } else { &du.boards[side] };
            let st = &sides[side];
            let shown;
            let game = if st["fog"].as_i64().unwrap_or(0) > 0 {
                let mut c = g.clone();
                c.modes.insert("light".into());
                shown = c;
                &shown
            } else {
                g
            };
            let skin = if side == you { my_skin.clone() } else if battle { crate::cosmetics::team_skin(1, side) } else { boss_skin.clone() };
            let skins = [skin];
            let view = View { game, t: if side == you { t } else { 1.0f32.min(t) }, skins: &skins, colors: colors.clone(), now_ms: now,
                              reveal: g.over, names: false, you: Some(0) };
            let (_strip, mut a) = slot(side);
            if side == you && shake_you > 0.0 {
                a.x += rand::gen_range(-1.0, 1.0) * shake_you * 14.0 * u;
                a.y += rand::gen_range(-1.0, 1.0) * shake_you * 14.0 * u;
            }
            if self.settings.view3d && n <= 2 {
                draw_board3d(&view, a, &self.fx);
            } else {
                draw_board(&view, a, &crate::render::Effects::default());
            }
            if st["out"] == true || (g.over && !battle && side != you) {
                rounded(a, 12.0 * u, alpha(BLACK, 0.55));
                self.ui.text("OUT", a.x + a.w / 2.0, a.y + a.h / 2.0, 34.0, alpha(WHITE, 0.85), Align::Center, true);
            }
            if battle && side == you {
                rounded_outline(a, 12.0 * u, 3.0, alpha(C_BLUE, 0.8));
            }
        }
        for s in self.duel_fx.shake.iter_mut() {
            *s = (*s - get_frame_time() * 1.5).max(0.0);
        }
        // the HUD
        rounded(Rect::new(0.0, 0.0, w, hud_h), 0.0, alpha(BG_TOP, 0.92));
        let cy = hud_h / 2.0;
        let title = if battle { format!("Puyo battle: {n} players") } else { format!("Boss battle: {}", name_of(1)) };
        self.ui.text(&title, w / 2.0, cy, 20.0, INK, Align::Center, true);
        if let Some(p) = self.online.ping_ms {
            self.ui.text(&format!("ping {p:.0} ms"), w - 20.0 * u, cy, 14.0, MUTED, Align::Right, false);
        }
        if !battle && !mt.game.over && mt.countdown.is_none() && du.result.is_none() {
            let next = du.next_signature as f64 * mt.tick_ms / 1000.0;
            self.ui.text(&format!("{} in {:.0}s", b["attack_name"].as_str().unwrap_or(""), next.ceil()), 70.0 * u, cy, 14.0, accent,
                         Align::Left, true);
        }
        if du.sudden {
            self.ui.text("SUDDEN DEATH", w / 2.0, cy + 22.0 * u, 12.0, C_RED, Align::Center, true);
        }
        let leave = self.ui.button_icon("", "back", Rect::new(14.0 * u, cy - 20.0 * u, 40.0 * u, 40.0 * u), Style::Ghost)
            || self.ui.hotkey(KeyCode::Escape);
        // over each board: who, the combo going, and junk on its way
        for side in 0..n {
            let st = &sides[side];
            let (strip, _a) = slot(side);
            rounded(strip, 12.0 * u, if side == you && battle { mix(CARD, C_BLUE, 0.15) } else { CARD });
            let ps = strip.h - 12.0 * u;
            let pr = Rect::new(strip.x + 6.0 * u, strip.y + 6.0 * u, ps, ps);
            let name = name_of(side);
            let tx = pr.x + ps + 10.0 * u;
            if !battle && side == 1 {
                boss_portrait(boss_num, pr, now, false);
                self.ui.text(&name, tx, strip.y + strip.h * 0.32, 16.0, accent, Align::Left, true);
            } else {
                let (avatar, bot) = if battle {
                    let p = players.get(side).cloned().unwrap_or_default();
                    (p["avatar"].as_str().unwrap_or("").to_string(), !p["bot"].is_null() && p["bot"] != false)
                } else {
                    (self.online.me.as_ref().and_then(|m| m["avatar"].as_str()).unwrap_or("").to_string(), false)
                };
                let c = vec2(pr.x + ps / 2.0, pr.y + ps / 2.0);
                if bot {
                    draw_circle(c.x, c.y, ps * 0.44, rgb(80, 90, 110));
                    draw_icon("gear", c.x, c.y, ps * 0.45, WHITE);
                } else {
                    let shown = if side == you { self.online.user.clone().unwrap_or(name.clone()) } else { name.clone() };
                    crate::screens_social::avatar(self, c, ps * 0.44, &avatar, &shown);
                }
                let label = if side == you && battle { format!("{name} (you)") } else if side == you { self.online.user.clone().unwrap_or(name) } else { name };
                let label = self.ui.fit(&label, 15.0, true, strip.w * 0.5);
                self.ui.text(&label, tx, strip.y + strip.h * 0.32, 15.0, if side == you { C_BLUE } else { INK }, Align::Left, true);
            }
            let chain = st["chain"].as_i64().unwrap_or(0);
            let ly = strip.y + strip.h * 0.68;
            if chain > 0 {
                let cr = Rect::new(tx, ly - 4.0 * u, (strip.w * 0.22).min(120.0 * u), 8.0 * u);
                progress(cr, (st["chain_left"].as_f64().unwrap_or(0.0) / window) as f32, C_GOLD);
                self.ui.text(&format!("{chain} combo"), cr.x + cr.w + 8.0 * u, ly, 13.0, C_GOLD, Align::Left, true);
            } else {
                self.ui.text(&format!("sent {} junk", st["sent"].as_i64().unwrap_or(0)), tx, ly, 12.0, MUTED, Align::Left, false);
            }
            // incoming junk, Puyo style: rocks of 1, boulders of 5, skulls of 20
            let incoming: Vec<(i64, i64)> = st["incoming"].as_array().cloned().unwrap_or_default().iter()
                .map(|x| (x[0].as_i64().unwrap_or(0), x[1].as_i64().unwrap_or(0))).collect();
            let total: i64 = incoming.iter().map(|x| x.0).sum();
            if total > 0 {
                let soon = incoming.first().map_or(99, |x| x.1);
                let mut left = total;
                let mut icons = Vec::new();
                for (size, kind) in [(20, 2), (5, 1), (1, 0)] {
                    while left >= size && icons.len() < 8 {
                        icons.push(kind);
                        left -= size;
                    }
                }
                let mut x = strip.x + strip.w - 12.0 * u;
                let y = strip.y + strip.h * 0.4;
                let flash = soon < 8 && (tsec * 8.0).fract() < 0.5;
                let k = strip.h / (76.0 * u);
                for &kind in icons.iter().rev() {
                    let s = [9.0, 13.0, 15.0][kind] * u * k;
                    let col = if flash { C_RED } else { [rgb(150, 150, 160), rgb(120, 110, 100), rgb(230, 230, 240)][kind] };
                    if kind == 2 {
                        draw_circle(x - s, y, s, col);
                        draw_circle(x - s - s * 0.35, y - s * 0.15, s * 0.25, BLACK);
                        draw_circle(x - s + s * 0.35, y - s * 0.15, s * 0.25, BLACK);
                    } else {
                        rounded(Rect::new(x - 2.0 * s, y - s, 2.0 * s, 2.0 * s), s * 0.4, col);
                        draw_line(x - 1.6 * s, y - 0.3 * s, x - 0.6 * s, y + 0.4 * s, 1.5 * u, alpha(BLACK, 0.4));
                    }
                    x -= 2.0 * s + 5.0 * u;
                }
                self.ui.text(&format!("{total} junk incoming"), strip.x + strip.w - 12.0 * u, strip.y + strip.h * 0.8, 11.0,
                             if flash { C_RED } else { MUTED }, Align::Right, true);
            }
            let (_s, a) = slot(side);
            let fog = st["fog"].as_i64().unwrap_or(0);
            let oc = st["overclock"].as_i64().unwrap_or(0);
            if fog > 0 || oc > 0 {
                self.ui.text(if fog > 0 { "ECLIPSE" } else { "OVERCLOCK" }, a.x + a.w / 2.0, a.y + 18.0 * u, 14.0, accent, Align::Center, true);
            }
        }
        // attacks flying across
        self.duel_fx.orbs.retain(|o| tsec - o.2 < 0.7);
        for &(from, to, t0, col, amount) in &self.duel_fx.orbs {
            let k = ((tsec - t0) / 0.7) as f32;
            let e = 1.0 - (1.0 - k).powi(3);
            let p = from.lerp(to, e) + vec2(0.0, -(e * PI).sin() * 100.0 * u);
            let size = (8.0 + (amount as f32).sqrt() * 5.0) * u;
            glow(p.x, p.y, size * 2.5, col, 1.0);
            draw_circle(p.x, p.y, size, lighten(col, 0.3));
            self.ui.text(&amount.to_string(), p.x, p.y, 13.0, BLACK, Align::Center, true);
        }
        // combo pops
        self.duel_fx.pops.retain(|p| tsec - p.2 < 0.9);
        for &(side, chain, t0) in &self.duel_fx.pops {
            let k = ((tsec - t0) / 0.9) as f32;
            let (_s, a) = slot(side.min(n - 1));
            let c = vec2(a.x + a.w / 2.0, a.y + a.h * 0.25 - k * 40.0 * u);
            let size = (30.0 + (chain as f32 * 3.0).min(30.0)) * if n > 2 { 0.7 } else { 1.0 };
            self.ui.text(&format!("{chain} COMBO!"), c.x + 3.0 * u, c.y + 3.0 * u, size, alpha(BLACK, 0.5 * (1.0 - k)), Align::Center, true);
            self.ui.text(&format!("{chain} COMBO!"), c.x, c.y, size, alpha(C_GOLD, 1.0 - k), Align::Center, true);
        }
        // banners
        self.duel_fx.banners.retain(|bn| tsec - bn.1 < 1.8);
        if let Some((text, t0, col)) = self.duel_fx.banners.last().cloned() {
            let k = ((tsec - t0) / 1.8) as f32;
            let al = if k < 0.15 { k / 0.15 } else if k > 0.8 { (1.0 - k) / 0.2 } else { 1.0 };
            let br = Rect::new(0.0, h * 0.42, w, 64.0 * u);
            draw_rectangle(br.x, br.y, br.w, br.h, alpha(BLACK, 0.6 * al));
            draw_rectangle(br.x, br.y, br.w, 3.0 * u, alpha(col, al));
            draw_rectangle(br.x, br.y + br.h - 3.0 * u, br.w, 3.0 * u, alpha(col, al));
            self.ui.text(&text, w / 2.0 + (1.0 - al) * 60.0 * u, br.y + br.h / 2.0, 28.0, alpha(WHITE, al), Align::Center, true);
        }
        // the countdown
        let mt = self.online.game.as_ref().unwrap();
        if let Some(c) = mt.countdown {
            let k = (self.ui.time.fract() as f32).min(1.0);
            let size = 120.0 * (1.2 - 0.2 * k);
            self.ui.text(&c.to_string(), w / 2.0 + 3.0 * u, h / 2.0 + 4.0 * u, size, alpha(BLACK, 0.4), Align::Center, true);
            self.ui.text(&c.to_string(), w / 2.0, h / 2.0, size, WHITE, Align::Center, true);
            let tip = if battle { "Your board has the blue outline. Combos send junk!" } else { "Arrow keys steer your board (left). Combos send junk!" };
            self.ui.text(tip, w / 2.0, h / 2.0 + 90.0 * u, 16.0, INK, Align::Center, true);
        }
        if leave {
            self.online.leave_match();
            self.go(if battle { Screen::Online } else { Screen::Story });
            return;
        }
        // the end
        let result = self.online.duel.as_ref().and_then(|d| d.result.clone());
        let Some(res) = result else { return };
        if self.reveal.is_some() {
            return; // (the loot first)
        }
        let won = res["won"] == true;
        if battle {
            let place = res["place"].as_i64().unwrap_or(0);
            let title = if won { "You win!".to_string() } else { format!("{} place", ordinal(place)) };
            let sub = match res["winner"].as_str() {
                Some(name) if !won => format!("{name} won the battle"),
                Some(_) => format!("Last snake standing of {}", res["of"]),
                None => "Everyone crashed at once: a draw".into(),
            };
            let extra = format!("+{} snake points   ·   best combo {}", format_points(res["points_earned"].as_i64().unwrap_or(0)),
                                res["best_chain"]);
            let host = self.online.is_host();
            let buttons: Vec<&str> = if host { vec!["Rematch", "Leave"] } else { vec!["Leave"] };
            if let Some(i) = self.overlay_card(&title, &sub, &extra, &buttons) {
                if host && i == 0 {
                    self.online.send(serde_json::json!({"t": "rematch"}));
                } else {
                    self.online.leave_match();
                    self.go(Screen::Online);
                }
            }
            return;
        }
        let title = if won { "Victory!" } else { "Defeat" };
        let boss_name = name_of(1);
        let sub = if won { format!("You beat {boss_name} in {:.0} s", res["seconds"].as_f64().unwrap_or(0.0)) }
                  else { format!("{boss_name} wins this time") };
        let extra = if won {
            let names: Vec<String> = res["rewards"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
            if res["first"] == true { format!("Rewards: {}", names.join(", ")) } else { format!("Rematch reward: {}", names.join(", ")) }
        } else {
            format!("Best combo {}   ·   junk sent {}", res["best_chain"], res["sent"])
        };
        if let Some(i) = self.overlay_card(title, &sub, &extra, &[if won { "Continue" } else { "Try again" }, if won { "Rematch" } else { "Map" }]) {
            let again = (won && i == 1) || (!won && i == 0);
            self.online.game = None;
            self.online.duel = None;
            if again {
                self.online.start_duel(boss_num);
            } else {
                self.go(Screen::Story);
            }
        }
    }
}

fn ordinal(n: i64) -> String {
    match n {
        1 => "1st".into(),
        2 => "2nd".into(),
        3 => "3rd".into(),
        _ => format!("{n}th"),
    }
}

fn signature_name(kind: &str) -> &'static str {
    match kind {
        "pebbles" => "Pebble Toss",
        "venom" => "Venom",
        "scramble" => "Scramble",
        "icewall" => "Ice Wall",
        "firestorm" => "Firestorm",
        "eclipse" => "Eclipse",
        "overclock" => "Overclock",
        _ => "a secret move",
    }
}

#[allow(dead_code)]
fn _unused(_: Req) {}
