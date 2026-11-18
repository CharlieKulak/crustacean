//! The store (shop, loot boxes, the player market, your collection), the battle
//! pass and daily challenges.

use crate::app::{App, Screen};
use crate::cosmetics::{catalog, format_points, hex};
use crate::online::Req;
use crate::render::{pack_preview, skin_preview};
use crate::ui::*;
use macroquad::prelude::*;
use serde_json::{json, Value};

/// A picture of any item: a skin on a little snake, a pack's colours, a loot box, points.
pub fn item_art(item: &str, r: Rect, now: f64) {
    let cat = catalog();
    match item.split_once(':') {
        Some(("skin", k)) => {
            if let Some(s) = cat.skin(k) {
                skin_preview(s, r, now);
            }
        }
        Some(("pack", k)) => {
            if let Some(p) = cat.pack(k) {
                let sw: Vec<_> = p.swatch.iter().map(|s| hex(s)).collect();
                pack_preview(&sw, r);
            }
        }
        Some(("crate", k)) => {
            let c = cat.crates.iter().find(|c| c.id == k).map(|c| rgbt(hex(&c.color))).unwrap_or(C_GOLD);
            crate_art(r, c, now);
        }
        Some(("points", _)) => {
            let s = r.h.min(r.w) * 0.32;
            for k in 0..3 {
                let p = vec2(r.x + r.w / 2.0 + (k as f32 - 1.0) * s * 0.7, r.y + r.h / 2.0 - k as f32 * s * 0.15);
                draw_icon("coin", p.x, p.y, s * 1.6, C_GOLD);
            }
        }
        _ => {}
    }
}

pub fn crate_art(r: Rect, c: Color, now: f64) {
    let s = r.h.min(r.w) * 0.42;
    let bob = ((now / 400.0).sin() as f32) * s * 0.04;
    let p = vec2(r.x + r.w / 2.0, r.y + r.h / 2.0 + bob);
    draw_ellipse(p.x, p.y + s * 0.95, s * 0.9, s * 0.18, 0.0, alpha(BLACK, 0.25));
    rounded(Rect::new(p.x - s, p.y - s * 0.35, s * 2.0, s * 1.2), s * 0.12, darken(c, 0.15));
    rounded(Rect::new(p.x - s * 1.08, p.y - s * 0.7, s * 2.16, s * 0.45), s * 0.12, c);
    draw_rectangle(p.x - s * 0.14, p.y - s * 0.7, s * 0.28, s * 1.55, lighten(c, 0.35));
    rounded(Rect::new(p.x - s * 0.22, p.y - s * 0.32, s * 0.44, s * 0.3), s * 0.06, rgb(60, 50, 40));
    for k in [-0.7, 0.7] {
        draw_line(p.x + s * k, p.y - s * 0.3, p.x + s * k, p.y + s * 0.8, s * 0.05, alpha(BLACK, 0.18));
    }
    rounded(Rect::new(p.x - s * 0.95, p.y - s * 0.65, s * 1.9, s * 0.12), s * 0.06, alpha(WHITE, 0.25));
}

/// The strip of items a loot box spins through, landing on what you won.
pub fn make_reel(item: &str, crate_id: &str) -> Vec<String> {
    let cat = catalog();
    let crate_ = cat.crates.iter().find(|c| c.id == crate_id);
    let odds = crate_.map(|c| c.odds.clone()).unwrap_or_default();
    let series = crate_.map(|c| c.series.clone()).unwrap_or_default();
    // what this crate can give: a themed crate's own series, or else the usual pool
    let themed: std::collections::HashSet<String> = cat.crates.iter().filter(|c| !c.series.is_empty()).map(|c| c.series.clone()).collect();
    let pool: Vec<String> = if series.is_empty() {
        cat.skins.iter().filter(|s| s.series != "Pass" && s.series != "Ranked" && s.series != "Gift" && !themed.contains(&s.series))
            .map(|s| format!("skin:{}", s.id))
            .chain(cat.packs.iter().map(|p| format!("pack:{}", p.id)))
            .collect()
    } else {
        cat.skins.iter().filter(|s| s.series == series).map(|s| format!("skin:{}", s.id)).collect()
    };
    let mut reel = Vec::new();
    for _ in 0..48 {
        let roll = rand::gen_range(0.0, 100.0f64);
        let mut acc = 0.0;
        let mut rarity = "common".to_string();
        for r in &cat.rarities {
            acc += odds.get(&r.id).copied().unwrap_or(0.0);
            if roll < acc {
                rarity = r.id.clone();
                break;
            }
        }
        let choices: Vec<&String> = pool.iter().filter(|i| cat.item_rarity(i) == rarity).collect();
        let pick = if choices.is_empty() { &pool[rand::gen_range(0, pool.len())] } else { choices[rand::gen_range(0, choices.len())] };
        reel.push(pick.clone());
    }
    reel[42] = item.to_string();
    reel
}

const REEL_TIME: f64 = 5.2;

pub fn draw_reveal(app: &mut App) {
    let Some(rv) = &app.reveal else { return };
    let u = app.ui.u;
    let (w, h) = (app.ui.w, app.ui.h);
    let elapsed = app.ui.time - rv.started;
    let data = rv.data.clone();
    let item = data["item"].as_str().unwrap_or("").to_string();
    let reel = rv.reel.clone();
    app.ui.input = true;
    draw_rectangle(0.0, 0.0, w, h, alpha(rgb(4, 8, 12), 0.85));
    let cat = catalog();
    let rarity = cat.item_rarity(&item);
    let rc = rgbt(cat.rarity_color(&rarity));
    let spinning = !item.is_empty() && elapsed < REEL_TIME && !rv.crate_id.is_empty();
    let cw = 150.0 * u;
    let gap = 10.0 * u;
    let cy = h * 0.42;
    if spinning {
        // ease out: fast at first, slowing to a stop on the prize
        let t = (elapsed / REEL_TIME).min(1.0);
        let ease = 1.0 - (1.0 - t).powi(4);
        let target = 42.0 * (cw + gap) as f64 + rand_offset(&item) * (cw as f64 * 0.35);
        let pos = (target * ease) as f32;
        let prev = ((target * (1.0 - (1.0 - (elapsed - 1.0 / 60.0).max(0.0) / REEL_TIME).powi(4))) / (cw + gap) as f64) as i64;
        if prev != (pos / (cw + gap)) as i64 {
            app.sfx.play("tick");
        }
        app.ui.push_clip(Rect::new(0.0, cy - 110.0 * u, w, 220.0 * u));
        for (i, it) in reel.iter().enumerate() {
            let x = w / 2.0 - cw / 2.0 + i as f32 * (cw + gap) - pos;
            if x > w || x + cw < 0.0 {
                continue;
            }
            let r = Rect::new(x, cy - 90.0 * u, cw, 180.0 * u);
            let c = rgbt(cat.rarity_color(&cat.item_rarity(it)));
            rounded(r, 12.0 * u, CARD);
            rounded(Rect::new(r.x, r.y + r.h - 8.0 * u, r.w, 8.0 * u), 4.0 * u, c);
            item_art(it, Rect::new(r.x + 8.0 * u, r.y + 12.0 * u, r.w - 16.0 * u, 110.0 * u), app.now_ms());
            let name = app.ui.fit(&cat.item_name(it), 13.0, true, cw - 12.0 * u);
            app.ui.text(&name, r.x + cw / 2.0, r.y + 146.0 * u, 13.0, INK, Align::Center, true);
        }
        app.ui.pop_clip();
        // the marker
        draw_triangle(vec2(w / 2.0 - 12.0 * u, cy - 112.0 * u), vec2(w / 2.0 + 12.0 * u, cy - 112.0 * u), vec2(w / 2.0, cy - 92.0 * u), C_GOLD);
        draw_triangle(vec2(w / 2.0 - 12.0 * u, cy + 112.0 * u), vec2(w / 2.0 + 12.0 * u, cy + 112.0 * u), vec2(w / 2.0, cy + 92.0 * u), C_GOLD);
        draw_line(w / 2.0, cy - 92.0 * u, w / 2.0, cy + 92.0 * u, 2.0, alpha(C_GOLD, 0.7));
        if app.ui.hotkey(KeyCode::Space) || app.ui.clicked {
            if let Some(r) = app.reveal.as_mut() {
                r.started -= REEL_TIME; // skip
            }
        }
        return;
    }
    if !item.is_empty() && (elapsed - REEL_TIME).abs() < 0.03 || (rv.crate_id.is_empty() && elapsed < 0.03) {
        app.sfx.play(if cat.rarity_index(&rarity) >= 4 { "reveal" } else { "win" });
    }
    // the prize
    let shown = if rv.crate_id.is_empty() { elapsed } else { elapsed - REEL_TIME };
    let pop = (shown as f32 * 4.0).min(1.0);
    let s = 0.85 + 0.15 * (1.0 - (1.0 - pop).powi(3));
    let card = Rect::new(w / 2.0 - 190.0 * u * s, cy - 150.0 * u * s, 380.0 * u * s, 300.0 * u * s);
    for k in 0..10 {
        let a = app.ui.time as f32 * 0.5 + k as f32 * std::f32::consts::TAU / 10.0;
        let len = 300.0 * u;
        draw_triangle(vec2(w / 2.0, cy), vec2(w / 2.0 + a.cos() * len, cy + a.sin() * len),
                      vec2(w / 2.0 + (a + 0.18).cos() * len, cy + (a + 0.18).sin() * len), alpha(rc, 0.08));
    }
    app.ui.card_colored(card, CARD);
    rounded_outline(card, 14.0 * u, 2.5, rc);
    if item.is_empty() {
        app.reveal = None;
        return;
    }
    item_art(&item, Rect::new(card.x + 30.0 * u, card.y + 20.0 * u, card.w - 60.0 * u, 150.0 * u * s), app.now_ms());
    rarity_tag(&app.ui, &cat.rarity_name(&rarity), rc, card.x + card.w / 2.0 - 50.0 * u, card.y + 190.0 * u * s, 12.0);
    app.ui.text(&cat.item_name(&item), card.x + card.w / 2.0, card.y + 224.0 * u * s, 26.0, INK, Align::Center, true);
    let note = if data["new"] == true { "New! It's in your collection.".to_string() } else {
        format!("Another copy: you have {} now. Sell spares on the market.", data["copies"].as_i64().unwrap_or(2))
    };
    app.ui.text(&note, card.x + card.w / 2.0, card.y + 256.0 * u * s, 14.0, MUTED, Align::Center, false);
    let by = card.y + card.h + 20.0 * u;
    let skin = item.strip_prefix("skin:").map(String::from);
    let bw = 170.0 * u;
    if let Some(sk) = skin {
        if app.ui.button("Equip", Rect::new(w / 2.0 - bw - 6.0 * u, by, bw, 48.0 * u), Style::Secondary) {
            app.online.post("/api/profile", json!({"skin": sk}), Req::SetProfile);
            app.reveal = None;
            return;
        }
        if app.ui.button("Continue", Rect::new(w / 2.0 + 6.0 * u, by, bw, 48.0 * u), Style::Primary) || app.ui.hotkey(KeyCode::Enter) {
            app.reveal = None;
        }
    } else if app.ui.button("Continue", Rect::new(w / 2.0 - bw / 2.0, by, bw, 48.0 * u), Style::Primary) || app.ui.hotkey(KeyCode::Enter) {
        app.reveal = None;
    }
    if app.ui.hotkey(KeyCode::Escape) {
        app.reveal = None;
    }
    app.ui.input = false; // (nothing under the popup reacts)
}

fn rand_offset(item: &str) -> f64 {
    // a fixed bit of wobble so the marker doesn't always land dead centre
    (crate::ui::id_of(item) % 1000) as f64 / 1000.0 - 0.5
}

impl App {
    pub fn store_screen(&mut self) {
        if self.reveal.is_some() || self.selling.is_some() || self.trade_draft.is_some() || self.jackpot_anim.is_some() {
            self.ui.input = false; // (a popup is open: nothing under it reacts)
        }
        let area = self.page("Store", "cart");
        let u = self.ui.u;
        let tab = self.tab(Screen::Store);
        let tw = (900.0 * u).min(area.w);
        if let Some(t) = self.ui.tabs(&["Shop", "Loot boxes", "Market", "Trades", "Jackpot", "Collection", "Index"], tab, Rect::new(area.x, area.y, tw, 44.0 * u)) {
            self.set_tab(Screen::Store, t);
        }
        for (k, i) in [(KeyCode::Key1, 0), (KeyCode::Key2, 1), (KeyCode::Key3, 2), (KeyCode::Key4, 3), (KeyCode::Key5, 4), (KeyCode::Key6, 5), (KeyCode::Key7, 6)] {
            if self.ui.hotkey(k) {
                self.set_tab(Screen::Store, i);
            }
        }
        let body = Rect::new(area.x, area.y + 60.0 * u, area.w, area.h - 60.0 * u);
        if self.need_login(body, "Earn snake points by playing, then spend them here.") {
            return;
        }
        match tab {
            0 => self.shop(body),
            1 => self.crates(body),
            2 => self.market(body),
            3 => self.trades(body),
            4 => self.jackpot(body),
            5 => self.collection(body),
            _ => self.index(body),
        }
        if self.selling.is_some() {
            self.sell_popup();
        }
        if self.trade_draft.is_some() {
            self.trade_popup();
        }
        if self.jackpot_anim.is_some() {
            self.jackpot_draw();
        }
    }

    fn item_card(&mut self, r: Rect, item: &str) {
        let u = self.ui.u;
        let cat = catalog();
        let rarity = cat.item_rarity(item);
        let rc = rgbt(cat.rarity_color(&rarity));
        let hover = self.ui.hover(r);
        let h = self.ui.anim(id_of(&("card", item, r.x as i32)), if hover { 1.0 } else { 0.0 }, 16.0);
        self.ui.shadow(r, 14.0 * u, 0.8 + h);
        rounded(r, 14.0 * u, mix(CARD, CARD_HI, h * 0.6));
        rounded(Rect::new(r.x + 14.0 * u, r.y, r.w - 28.0 * u, 4.0 * u), 2.0 * u, rc);
        // a glow behind rare things
        if cat.rarity_index(&rarity) >= 3 {
            glow(r.x + r.w / 2.0, r.y + 62.0 * u, 70.0 * u, rc, 1.0 + h);
        }
        item_art(item, Rect::new(r.x + 12.0 * u, r.y + 14.0 * u, r.w - 24.0 * u, 96.0 * u), self.now_ms());
        let name = self.ui.fit(&cat.item_name(item), 16.0, true, r.w - 24.0 * u);
        self.ui.text(&name, r.x + 14.0 * u, r.y + 128.0 * u, 16.0, INK, Align::Left, true);
        let series = item.strip_prefix("skin:").and_then(|k| cat.skin(k)).map(|s| s.series.clone()).unwrap_or_default();
        let tx = rarity_tag(&self.ui, &cat.rarity_name(&rarity), rc, r.x + 14.0 * u, r.y + 152.0 * u, 9.5);
        if !series.is_empty() {
            self.ui.text(&series, r.x + 22.0 * u + tx, r.y + 152.0 * u, 11.0, MUTED, Align::Left, true);
        }
    }

    fn grid(&self, area: Rect, n: usize, min_w: f32, h: f32) -> (usize, f32, f32, f32) {
        let u = self.ui.u;
        let gap = 12.0 * u;
        let cols = ((area.w + gap) / (min_w * u + gap)).floor().max(1.0) as usize;
        let cw = (area.w - 12.0 * u - gap * (cols as f32 - 1.0)) / cols as f32;
        let rows = n.div_ceil(cols);
        (cols, cw, gap, rows as f32 * (h * u + gap))
    }

    fn shop(&mut self, body: Rect) {
        let u = self.ui.u;
        let cat = catalog();
        let items: Vec<String> = cat.skins.iter().filter(|s| s.price.is_some()).map(|s| format!("skin:{}", s.id))
            .chain(cat.packs.iter().filter(|p| p.price.is_some()).map(|p| format!("pack:{}", p.id))).collect();
        let (cols, cw, gap, total) = self.grid(body, items.len(), 200.0, 226.0);
        let off = self.ui.begin_scroll("shop", body, total);
        for (i, item) in items.iter().enumerate() {
            let r = Rect::new(body.x + (i % cols) as f32 * (cw + gap), body.y + (i / cols) as f32 * (226.0 * u + gap) - off, cw, 226.0 * u);
            if !self.ui.visible(r) {
                continue;
            }
            self.item_card(r, item);
            let price = cat.item_price(item).unwrap_or(0);
            let br = Rect::new(r.x + 12.0 * u, r.y + r.h - 52.0 * u, r.w - 24.0 * u, 40.0 * u);
            if self.online.owns(item) {
                self.ui.text("Owned", br.x + br.w / 2.0, br.y + br.h / 2.0, 15.0, ACCENT, Align::Center, true);
            } else {
                let afford = self.online.points() >= price;
                if self.ui.button_icon(&format_points(price), "coin", br, if afford { Style::Gold } else { Style::Secondary }) {
                    if afford {
                        self.online.post("/api/store/buy", json!({"item": item}), Req::Buy(item.clone()));
                    } else {
                        self.ui.toast("Not enough snake points.", C_RED);
                    }
                }
            }
        }
        self.ui.end_scroll();
    }

    fn crates(&mut self, body: Rect) {
        let u = self.ui.u;
        let cat = catalog();
        let n = cat.crates.len();
        let (cols, cw, gap, total) = self.grid(body, n, 210.0, 380.0);
        let off = self.ui.begin_scroll("crates", body, total);
        for (i, c) in cat.crates.iter().enumerate() {
            let r = Rect::new(body.x + (i % cols) as f32 * (cw + gap), body.y + (i / cols) as f32 * (380.0 * u + gap) - off, cw, 380.0 * u);
            if !self.ui.visible(r) {
                continue;
            }
            let col = rgbt(hex(&c.color));
            self.ui.card(r);
            glow(r.x + r.w / 2.0, r.y + 80.0 * u, 90.0 * u, col, 1.4);
            crate_art(Rect::new(r.x, r.y + 20.0 * u, r.w, 120.0 * u), col, self.now_ms());
            self.ui.text(&c.name, r.x + r.w / 2.0, r.y + 164.0 * u, 19.0, INK, Align::Center, true);
            let mut y = r.y + 192.0 * u;
            for rar in &cat.rarities {
                let Some(p) = c.odds.get(&rar.id) else { continue };
                let rc = rgbt(hex(&rar.color));
                draw_circle(r.x + 22.0 * u, y, 4.0 * u, rc);
                self.ui.text(&rar.name, r.x + 32.0 * u, y, 13.0, MUTED, Align::Left, false);
                self.ui.text(&format!("{p}%"), r.x + r.w - 18.0 * u, y, 13.0, lighten(rc, 0.2), Align::Right, true);
                y += 20.0 * u;
            }
            let br = Rect::new(r.x + 14.0 * u, r.y + r.h - 54.0 * u, r.w - 28.0 * u, 42.0 * u);
            let afford = self.online.points() >= c.price;
            let busy = self.online.is_busy(&Req::OpenCrate(c.id.clone()));
            if self.ui.button_icon(&format_points(c.price), "coin", br, if afford { Style::Gold } else { Style::Secondary }) && !busy && self.reveal.is_none() {
                if afford {
                    self.online.post("/api/store/open", json!({"crate": c.id}), Req::OpenCrate(c.id.clone()));
                    // the reel needs to know which box it was
                    self.selected = Some(c.id.clone());
                } else {
                    self.ui.toast("Not enough snake points.", C_RED);
                }
            }
        }
        self.ui.end_scroll();
        // loot box replies don't say which box: fill it in
        if let (Some(rv), Some(cid)) = (self.reveal.as_mut(), self.selected.clone()) {
            if rv.crate_id.is_empty() && rv.data.get("tier").is_none() {
                rv.crate_id = cid.clone();
                let item = rv.data["item"].as_str().unwrap_or("").to_string();
                rv.reel = make_reel(&item, &cid);
                self.selected = None;
            }
        }
    }

    fn market(&mut self, body: Rect) {
        let u = self.ui.u;
        let cat = catalog();
        // search and filters
        let mut q = self.fields.remove("market_q").unwrap_or_default();
        let entered = self.ui.text_input("market_q", &mut q, Rect::new(body.x, body.y, 260.0 * u, 40.0 * u), "Search skins, packs or sellers", false, 40);
        let rar = self.fields.get("market_r").cloned().unwrap_or_default();
        let sort = self.fields.get("market_s").cloned().unwrap_or_else(|| "cheap".into());
        let mut x = body.x + 272.0 * u;
        let mut refresh = entered;
        for r in [""].iter().map(|s| s.to_string()).chain(cat.rarities.iter().map(|r| r.id.clone())) {
            let label = if r.is_empty() { "All".to_string() } else { cat.rarity_name(&r) };
            let w = self.ui.measure(&label, 13.0, true) + 24.0 * u;
            let col = if r.is_empty() { ACCENT } else { rgbt(cat.rarity_color(&r)) };
            if x + w > body.x + body.w - 200.0 * u {
                break;
            }
            if self.ui.chip(&label, rar == r, Rect::new(x, body.y + 4.0 * u, w, 32.0 * u), col) {
                self.fields.insert("market_r", r.clone());
                refresh = true;
            }
            x += w + 6.0 * u;
        }
        let sorts = [("cheap", "Cheapest"), ("pricey", "Priciest"), ("new", "Newest")];
        let si = sorts.iter().position(|s| s.0 == sort).unwrap_or(0);
        if self.ui.button(sorts[si].1, Rect::new(body.x + body.w - 190.0 * u, body.y, 120.0 * u, 40.0 * u), Style::Secondary) {
            self.fields.insert("market_s", sorts[(si + 1) % 3].0.to_string());
            refresh = true;
        }
        if self.ui.button_icon("", "search", Rect::new(body.x + body.w - 58.0 * u, body.y, 48.0 * u, 40.0 * u), Style::Secondary) {
            refresh = true;
        }
        self.fields.insert("market_q", q.clone());
        if refresh {
            let r = self.fields.get("market_r").cloned().unwrap_or_default();
            let s = self.fields.get("market_s").cloned().unwrap_or_else(|| "cheap".into());
            self.online.get(&format!("/api/market?q={}&rarity={}&sort={}", crate::online::urlencode(&q), r, s), Req::Market);
        }
        let list = Rect::new(body.x, body.y + 54.0 * u, body.w, body.h - 54.0 * u);
        let Some(m) = self.online.market.clone() else {
            self.loading(list);
            return;
        };
        let listings = m["listings"].as_array().cloned().unwrap_or_default();
        let mine = m["mine"].as_array().cloned().unwrap_or_default();
        let fee = m["fee"].as_f64().unwrap_or(0.05);
        let card_h = 250.0;
        let (cols, cw, gap, total) = self.grid(list, listings.len(), 200.0, card_h);
        let mine_h = if mine.is_empty() { 0.0 } else { 60.0 * u + mine.len() as f32 * 54.0 * u };
        let off = self.ui.begin_scroll("market", list, total + mine_h + 60.0 * u);
        let mut y0 = list.y - off;
        if !mine.is_empty() {
            self.ui.text("Your listings", list.x, y0 + 14.0 * u, 18.0, INK, Align::Left, true);
            self.ui.text(&format!("When one sells you get its price, less a {:.0}% market fee.", fee * 100.0), list.x + 150.0 * u, y0 + 15.0 * u, 13.0, MUTED, Align::Left, false);
            y0 += 36.0 * u;
            for l in &mine {
                let r = Rect::new(list.x, y0, list.w - 12.0 * u, 46.0 * u);
                rounded(r, 10.0 * u, CARD);
                let item = l["item"].as_str().unwrap_or("");
                let rc = rgbt(cat.rarity_color(&cat.item_rarity(item)));
                draw_circle(r.x + 20.0 * u, r.y + r.h / 2.0, 6.0 * u, rc);
                self.ui.text(&cat.item_name(item), r.x + 36.0 * u, r.y + r.h / 2.0, 15.0, INK, Align::Left, true);
                draw_icon("coin", r.x + r.w - 250.0 * u, r.y + r.h / 2.0, 18.0 * u, C_GOLD);
                self.ui.text(&format_points(l["price"].as_i64().unwrap_or(0)), r.x + r.w - 236.0 * u, r.y + r.h / 2.0, 15.0, C_GOLD, Align::Left, true);
                if self.ui.button("Take down", Rect::new(r.x + r.w - 126.0 * u, r.y + 6.0 * u, 116.0 * u, 34.0 * u), Style::Secondary) {
                    self.online.post("/api/market/cancel", json!({"id": l["id"]}), Req::MarketCancel);
                }
                y0 += 54.0 * u;
            }
            y0 += 18.0 * u;
        }
        if listings.is_empty() {
            self.ui.text("Nothing for sale right now.", list.x + list.w / 2.0, y0 + 60.0 * u, 18.0, MUTED, Align::Center, true);
            self.ui.text("Sell your spare copies from your Collection.", list.x + list.w / 2.0, y0 + 88.0 * u, 15.0, FAINT, Align::Center, false);
        }
        let me = self.online.user.clone().unwrap_or_default();
        for (i, l) in listings.iter().enumerate() {
            let r = Rect::new(list.x + (i % cols) as f32 * (cw + gap), y0 + (i / cols) as f32 * (card_h * u + gap), cw, card_h * u);
            if !self.ui.visible(r) {
                continue;
            }
            let item = l["item"].as_str().unwrap_or("").to_string();
            self.item_card(r, &item);
            let seller = l["seller"].as_str().unwrap_or("");
            let s = self.ui.fit(&format!("from {seller}"), 12.0, false, r.w - 28.0 * u);
            self.ui.text(&s, r.x + 14.0 * u, r.y + 174.0 * u, 12.0, MUTED, Align::Left, false);
            let price = l["price"].as_i64().unwrap_or(0);
            let br = Rect::new(r.x + 12.0 * u, r.y + r.h - 50.0 * u, r.w - 24.0 * u, 40.0 * u);
            if seller == me {
                self.ui.text("Your listing", br.x + br.w / 2.0, br.y + br.h / 2.0, 14.0, MUTED, Align::Center, true);
            } else {
                let afford = self.online.points() >= price;
                if self.ui.button_icon(&format_points(price), "coin", br, if afford { Style::Gold } else { Style::Secondary }) {
                    if afford {
                        self.online.post("/api/market/buy", json!({"id": l["id"]}), Req::MarketBuy);
                    } else {
                        self.ui.toast("Not enough snake points.", C_RED);
                    }
                }
            }
        }
        self.ui.end_scroll();
    }

    fn collection(&mut self, body: Rect) {
        let u = self.ui.u;
        let cat = catalog();
        let Some(me) = self.online.me.clone() else {
            self.loading(body);
            return;
        };
        let mut items: Vec<String> = me["inventory"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        // (the dunce cap skin always comes first; then the rarest down)
        let dunce = |i: &str| i.strip_prefix("skin:").and_then(|k| cat.skin(k)).is_some_and(|s| s.pattern == "dunce");
        items.sort_by_key(|i| (!dunce(i), std::cmp::Reverse(cat.rarity_index(&cat.item_rarity(i))), cat.item_name(i)));
        if items.is_empty() {
            if self.empty_state(body, "box", "Nothing yet", "Buy skins in the Shop or open loot boxes.", Some("Go to the Shop")) {
                self.set_tab(Screen::Store, 0);
            }
            return;
        }
        let equipped = self.online.equipped_skin();
        let card_h = 262.0;
        let (cols, cw, gap, total) = self.grid(body, items.len() + 1, 200.0, card_h);
        let off = self.ui.begin_scroll("collection", body, total);
        // the plain snake comes first, so you can take a skin off
        let firsts = items.iter().take_while(|i| dunce(i)).count();
        let all = items[..firsts].iter().cloned().chain(std::iter::once(String::new())).chain(items[firsts..].iter().cloned())
            .collect::<Vec<_>>();
        for (i, item) in all.iter().enumerate() {
            let r = Rect::new(body.x + (i % cols) as f32 * (cw + gap), body.y + (i / cols) as f32 * (card_h * u + gap) - off, cw, card_h * u);
            if !self.ui.visible(r) {
                continue;
            }
            if item.is_empty() {
                self.ui.card(r);
                skin_preview(&crate::cosmetics::default_skin(), Rect::new(r.x + 12.0 * u, r.y + 14.0 * u, r.w - 24.0 * u, 96.0 * u), self.now_ms());
                self.ui.text("Classic Blue", r.x + 14.0 * u, r.y + 128.0 * u, 16.0, INK, Align::Left, true);
                let br = Rect::new(r.x + 12.0 * u, r.y + r.h - 50.0 * u, r.w - 24.0 * u, 40.0 * u);
                if equipped.is_empty() {
                    self.ui.text("Equipped", br.x + br.w / 2.0, br.y + br.h / 2.0, 15.0, ACCENT, Align::Center, true);
                } else if self.ui.button("Equip", br, Style::Secondary) {
                    self.online.post("/api/profile", json!({"skin": ""}), Req::SetProfile);
                }
                continue;
            }
            self.item_card(r, item);
            let copies = self.online.copies(item);
            if copies > 1 {
                let t = format!("x{copies}");
                let bw = self.ui.measure(&t, 13.0, true) + 16.0 * u;
                let b = Rect::new(r.x + r.w - bw - 10.0 * u, r.y + 12.0 * u, bw, 24.0 * u);
                rounded(b, 12.0 * u, alpha(BLACK, 0.5));
                self.ui.text(&t, b.x + bw / 2.0, b.y + 12.0 * u, 13.0, INK, Align::Center, true);
            }
            let tradeable = !cat.untradeable.contains(item);
            let half = (r.w - 32.0 * u) / 2.0;
            let b1 = Rect::new(r.x + 12.0 * u, r.y + r.h - 50.0 * u, if tradeable { half } else { r.w - 24.0 * u }, 40.0 * u);
            let b2 = Rect::new(r.x + 20.0 * u + half, r.y + r.h - 50.0 * u, half, 40.0 * u);
            if let Some(k) = item.strip_prefix("skin:") {
                if equipped == k {
                    self.ui.text("Equipped", b1.x + b1.w / 2.0, b1.y + b1.h / 2.0, 14.0, ACCENT, Align::Center, true);
                } else if self.ui.button("Equip", b1, Style::Secondary) {
                    self.online.post("/api/profile", json!({"skin": k}), Req::SetProfile);
                }
            } else if let Some(k) = item.strip_prefix("pack:") {
                if self.settings.pack == k {
                    self.ui.text("In use", b1.x + b1.w / 2.0, b1.y + b1.h / 2.0, 14.0, ACCENT, Align::Center, true);
                } else if self.ui.button("Use", b1, Style::Secondary) {
                    self.settings.pack = k.to_string();
                    self.settings.save();
                }
            }
            if tradeable {
                if self.ui.button("Sell", b2, Style::Ghost) {
                    self.selling = Some(item.clone());
                    let suggest = cat.item_price(item).unwrap_or(match cat.rarity_index(&cat.item_rarity(item)) {
                        0 => 500,
                        1 => 2_000,
                        2 => 8_000,
                        3 => 30_000,
                        4 => 120_000,
                        _ => 400_000,
                    });
                    self.fields.insert("sell_price", suggest.to_string());
                }
            } else {
                let tag = if item.starts_with("skin:") && cat.skin(&item[5..]).is_some_and(|s| s.series == "Gift") { "Gift" } else { "Ranked reward" };
                self.ui.text(tag, r.x + r.w - 14.0 * u, r.y + 174.0 * u, 11.0, C_GOLD, Align::Right, true);
            }
        }
        self.ui.end_scroll();
    }

    fn sell_popup(&mut self) {
        let Some(item) = self.selling.clone() else { return };
        let u = self.ui.u;
        let (w, h) = (self.ui.w, self.ui.h);
        self.ui.input = true;
        draw_rectangle(0.0, 0.0, w, h, alpha(rgb(4, 8, 12), 0.7));
        let r = Rect::new(w / 2.0 - 220.0 * u, h / 2.0 - 190.0 * u, 440.0 * u, 380.0 * u);
        self.ui.card(r);
        let cat = catalog();
        item_art(&item, Rect::new(r.x + 40.0 * u, r.y + 20.0 * u, r.w - 80.0 * u, 100.0 * u), self.now_ms());
        self.ui.text(&format!("Sell {}", cat.item_name(&item)), r.x + r.w / 2.0, r.y + 146.0 * u, 22.0, INK, Align::Center, true);
        let copies = self.online.copies(&item);
        let equipped = item.strip_prefix("skin:") == Some(self.online.equipped_skin().as_str());
        let note = if copies <= 1 && equipped { "This is your last copy, and you're wearing it: it comes off when listed." } else if copies <= 1 { "This is your only copy." } else { "" };
        if !note.is_empty() {
            self.ui.text(note, r.x + r.w / 2.0, r.y + 174.0 * u, 12.0, C_GOLD, Align::Center, false);
        }
        let mut price = self.fields.remove("sell_price").unwrap_or_default();
        price.retain(|c| c.is_ascii_digit());
        let entered = self.ui.text_input("sell_price", &mut price, Rect::new(r.x + 40.0 * u, r.y + 196.0 * u, r.w - 80.0 * u, 46.0 * u), "Price in snake points", false, 9);
        let p: i64 = price.parse().unwrap_or(0);
        let fee = (p as f64 * 0.05).round() as i64;
        self.ui.text(&format!("You get {} after the 5% fee", format_points(p - fee)), r.x + r.w / 2.0, r.y + 262.0 * u, 14.0, MUTED, Align::Center, false);
        self.fields.insert("sell_price", price);
        let bw = (r.w - 92.0 * u) / 2.0;
        if self.ui.button("Cancel", Rect::new(r.x + 40.0 * u, r.y + r.h - 76.0 * u, bw, 48.0 * u), Style::Secondary) || self.ui.hotkey(KeyCode::Escape) {
            self.selling = None;
        } else if self.ui.button("List it", Rect::new(r.x + 52.0 * u + bw, r.y + r.h - 76.0 * u, bw, 48.0 * u), Style::Primary) || entered {
            self.online.post("/api/market/list", json!({"item": item, "price": p}), Req::MarketList);
            self.selling = None;
        }
        self.ui.input = false;
    }

    // ---- the battle pass

    pub fn pass_screen(&mut self) {
        if self.reveal.is_some() {
            self.ui.input = false;
        }
        let area = self.page("Battle Pass", "star");
        let u = self.ui.u;
        if self.need_login(area, "Level up by playing and claim this week's rewards.") {
            return;
        }
        let Some(p) = self.online.pass.clone() else {
            self.loading(area);
            return;
        };
        let level = p["level"].as_i64().unwrap_or(0);
        let owned = p["owned"].as_bool().unwrap_or(false);
        // header card
        let head = Rect::new(area.x, area.y, area.w, 150.0 * u);
        self.ui.card(head);
        draw_circle(head.x + 70.0 * u, head.y + 75.0 * u, 48.0 * u, alpha(C_PURPLE, 0.2));
        self.ui.text(&level.to_string(), head.x + 70.0 * u, head.y + 70.0 * u, 40.0, INK, Align::Center, true);
        self.ui.text("LEVEL", head.x + 70.0 * u, head.y + 102.0 * u, 11.0, MUTED, Align::Center, true);
        let bx = head.x + 140.0 * u;
        let bw = head.w - 140.0 * u - 260.0 * u;
        let into = p["into_level"].as_i64().unwrap_or(0);
        let lxp = p["level_xp"].as_i64().unwrap_or(1).max(1);
        self.ui.text(&format!("Week {}", p["week"].as_str().unwrap_or("")), bx, head.y + 34.0 * u, 20.0, INK, Align::Left, true);
        self.ui.text(&format!("ends in {}", p["left"].as_str().unwrap_or("")), bx + bw, head.y + 34.0 * u, 14.0, MUTED, Align::Right, false);
        progress(Rect::new(bx, head.y + 62.0 * u, bw, 16.0 * u), into as f32 / lxp as f32, C_PURPLE);
        self.ui.text(&format!("{} / {} XP to level {}", format_points(into), format_points(lxp), level + 1), bx, head.y + 96.0 * u, 14.0, MUTED, Align::Left, false);
        self.ui.text("Play anything to earn XP. Levels past 100 carry over to next week.", bx, head.y + 120.0 * u, 13.0, FAINT, Align::Left, false);
        let br = Rect::new(head.x + head.w - 240.0 * u, head.y + 50.0 * u, 220.0 * u, 52.0 * u);
        if owned {
            let ready: Vec<Value> = p["tiers"].as_array().cloned().unwrap_or_default().into_iter().filter(|t| t["reached"] == true && t["claimed"] == false).collect();
            if let Some(t) = ready.first() {
                let lvl = t["level"].as_i64().unwrap_or(0);
                if self.ui.button(&format!("Claim level {lvl}"), br, Style::Primary) && self.reveal.is_none() {
                    self.online.post("/api/battlepass/claim", json!({"level": lvl}), Req::ClaimPass(lvl));
                }
            } else {
                self.ui.text("Pass owned", br.x + br.w / 2.0, br.y + br.h / 2.0, 18.0, ACCENT, Align::Center, true);
            }
        } else {
            let price = p["price"].as_i64().unwrap_or(10_000);
            if self.ui.button_icon(&format!("Unlock  {}", format_points(price)), "coin", br, Style::Gold) {
                self.online.post("/api/battlepass/buy", json!({}), Req::BuyPass);
            }
        }
        // the reward track
        let tiers = p["tiers"].as_array().cloned().unwrap_or_default();
        let track = Rect::new(area.x, head.y + head.h + 20.0 * u, area.w, area.h - head.h - 20.0 * u);
        let tw = 170.0 * u;
        let gap = 12.0 * u;
        let total = tiers.len() as f32 * (tw + gap);
        // horizontal scrolling with the wheel (or drag)
        let off_id = "pass_scroll";
        let mut off: f32 = self.fields.get(off_id).and_then(|s| s.parse().ok()).unwrap_or(-1.0);
        if off < 0.0 {
            // start at the next reward to claim or reach
            let idx = tiers.iter().position(|t| t["claimed"] == false).unwrap_or(0);
            off = (idx as f32 * (tw + gap) - tw).max(0.0);
        }
        if self.ui.hover(track) && self.ui.wheel != 0.0 {
            off -= self.ui.wheel * 120.0 * u;
        }
        if self.ui.hover(track) && self.ui.down {
            off -= mouse_delta_position().x * self.ui.w / 2.0;
        }
        off = off.clamp(0.0, (total - track.w).max(0.0));
        self.fields.insert(off_id, off.to_string());
        let shown = self.ui.anim(id_of("pass_off"), off, 14.0);
        self.ui.push_clip(track);
        let th = track.h.min(320.0 * u);
        let ty = track.y + 20.0 * u;
        // the level line
        draw_line(track.x, ty + th - 30.0 * u, track.x + track.w, ty + th - 30.0 * u, 4.0 * u, alpha(LINE, 0.8));
        for (i, t) in tiers.iter().enumerate() {
            let r = Rect::new(track.x + i as f32 * (tw + gap) - shown, ty, tw, th - 60.0 * u);
            if r.x > track.x + track.w || r.x + r.w < track.x {
                continue;
            }
            let lvl = t["level"].as_i64().unwrap_or(0);
            let reached = t["reached"] == true;
            let claimed = t["claimed"] == true;
            let art = match t["kind"].as_str() {
                Some("points") => "points:".to_string(),
                Some("crate") => format!("crate:{}", t["crate"].as_str().unwrap_or("")),
                _ => t["item"].as_str().unwrap_or("").to_string(),
            };
            let rarity = if art.starts_with("skin:") || art.starts_with("pack:") { catalog().item_rarity(&art) } else if art.contains("royal") { "mythic".into() } else { "rare".into() };
            let rc = rgbt(catalog().rarity_color(&rarity));
            self.ui.shadow(r, 14.0 * u, 0.8);
            rounded(r, 14.0 * u, if reached { CARD_HI } else { CARD });
            rounded(Rect::new(r.x + 14.0 * u, r.y, r.w - 28.0 * u, 4.0 * u), 2.0 * u, rc);
            item_art(&art, Rect::new(r.x + 10.0 * u, r.y + 18.0 * u, r.w - 20.0 * u, 100.0 * u), self.now_ms());
            let name = self.ui.fit(t["name"].as_str().unwrap_or(""), 14.0, true, r.w - 20.0 * u);
            self.ui.text(&name, r.x + r.w / 2.0, r.y + 140.0 * u, 14.0, INK, Align::Center, true);
            if lvl == 75 || lvl == 99 {
                self.ui.text("PASS ONLY", r.x + r.w / 2.0, r.y + 162.0 * u, 10.0, C_PURPLE, Align::Center, true);
            }
            if claimed {
                draw_icon("check", r.x + r.w / 2.0, r.y + r.h - 28.0 * u, 26.0 * u, ACCENT);
            } else if !reached || !owned {
                draw_icon("lock", r.x + r.w / 2.0, r.y + r.h - 28.0 * u, 22.0 * u, FAINT);
            } else if self.ui.button("Claim", Rect::new(r.x + 14.0 * u, r.y + r.h - 50.0 * u, r.w - 28.0 * u, 40.0 * u), Style::Primary) && self.reveal.is_none() {
                self.online.post("/api/battlepass/claim", json!({"level": lvl}), Req::ClaimPass(lvl));
            }
            let dot = vec2(r.x + r.w / 2.0, ty + th - 30.0 * u);
            draw_circle(dot.x, dot.y, 16.0 * u, if reached { C_PURPLE } else { CARD_HI });
            self.ui.text(&lvl.to_string(), dot.x, dot.y, 12.0, INK, Align::Center, true);
        }
        self.ui.pop_clip();
    }

    // ---- daily challenges

    pub fn daily_screen(&mut self) {
        let area = self.page("Daily challenges", "calendar");
        let u = self.ui.u;
        if self.need_login(area, "Three new challenges every day, worth snake points.") {
            return;
        }
        let Some(d) = self.online.daily.clone() else {
            self.online.get("/api/daily", Req::Daily);
            self.loading(area);
            return;
        };
        let w = area.w.min(820.0 * u);
        let x = area.x + (area.w - w) / 2.0;
        let off = self.ui.begin_scroll("daily", area, 690.0 * u);
        let mut y = area.y - off;
        y = self.login_reward_card(x, y, w) + 16.0 * u;
        self.ui.text(&format!("Challenges: new ones in {}", d["left"].as_str().unwrap_or("")), x, y + 14.0 * u, 16.0, MUTED, Align::Left, false);
        y += 40.0 * u;
        let colors = [ACCENT, C_BLUE, C_PURPLE];
        for (i, c) in d["challenges"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            let r = Rect::new(x, y, w, 110.0 * u);
            self.ui.card(r);
            let col = colors[i % 3];
            rounded(Rect::new(r.x, r.y, 6.0 * u, r.h), 3.0 * u, col);
            let diff = c["difficulty"].as_str().unwrap_or("").to_uppercase();
            self.ui.text(&diff, r.x + 26.0 * u, r.y + 26.0 * u, 11.0, col, Align::Left, true);
            self.ui.text(c["text"].as_str().unwrap_or(""), r.x + 26.0 * u, r.y + 52.0 * u, 19.0, INK, Align::Left, true);
            let (prog, goal) = (c["progress"].as_i64().unwrap_or(0), c["goal"].as_i64().unwrap_or(1).max(1));
            progress(Rect::new(r.x + 26.0 * u, r.y + 78.0 * u, w * 0.5, 12.0 * u), prog as f32 / goal as f32, col);
            self.ui.text(&format!("{prog} / {goal}"), r.x + 36.0 * u + w * 0.5, r.y + 84.0 * u, 13.0, MUTED, Align::Left, true);
            let br = Rect::new(r.x + w - 210.0 * u, r.y + 32.0 * u, 186.0 * u, 46.0 * u);
            let reward = format_points(c["reward"].as_i64().unwrap_or(0));
            if c["claimed"] == true {
                draw_icon("check", br.x + 30.0 * u, br.y + br.h / 2.0, 24.0 * u, ACCENT);
                self.ui.text("Claimed", br.x + 52.0 * u, br.y + br.h / 2.0, 16.0, ACCENT, Align::Left, true);
            } else if c["done"] == true {
                if self.ui.button_icon(&format!("Claim {reward}"), "coin", br, Style::Gold) {
                    self.online.post("/api/daily/claim", json!({"id": c["id"]}), Req::ClaimDaily);
                }
            } else {
                draw_icon("coin", br.x + 30.0 * u, br.y + br.h / 2.0, 20.0 * u, alpha(C_GOLD, 0.6));
                self.ui.text(&reward, br.x + 48.0 * u, br.y + br.h / 2.0, 17.0, alpha(C_GOLD, 0.7), Align::Left, true);
            }
            y += 124.0 * u;
        }
        let r = Rect::new(x, y + 6.0 * u, w, 84.0 * u);
        self.ui.card_colored(r, mix(CARD, rgb(120, 80, 10), 0.25));
        draw_icon("gift", r.x + 44.0 * u, r.y + r.h / 2.0, 34.0 * u, C_GOLD);
        self.ui.text("Finish all three", r.x + 80.0 * u, r.y + 30.0 * u, 19.0, INK, Align::Left, true);
        self.ui.text(&format!("Bonus: {} snake points", format_points(d["bonus"].as_i64().unwrap_or(0))), r.x + 80.0 * u, r.y + 56.0 * u, 14.0, MUTED, Align::Left, false);
        let br = Rect::new(r.x + w - 210.0 * u, r.y + 19.0 * u, 186.0 * u, 46.0 * u);
        if d["bonus_claimed"] == true {
            self.ui.text("Claimed", br.x + br.w / 2.0, br.y + br.h / 2.0, 16.0, ACCENT, Align::Center, true);
        } else if d["bonus_ready"] == true {
            if self.ui.button("Claim bonus", br, Style::Gold) {
                self.online.post("/api/daily/claim", json!({"id": "bonus"}), Req::ClaimDaily);
            }
        } else {
            draw_icon("lock", br.x + br.w / 2.0, br.y + br.h / 2.0, 22.0 * u, FAINT);
        }
        self.ui.end_scroll();
    }

    /// The daily login reward: seven days, better each day in a row. Returns where it ends.
    pub fn login_reward_card(&mut self, x: f32, y: f32, w: f32) -> f32 {
        let u = self.ui.u;
        let Some(lr) = self.online.login_reward.clone() else {
            self.online.get("/api/login-reward", Req::LoginReward);
            return y;
        };
        let r = Rect::new(x, y, w, 236.0 * u);
        self.ui.card(r);
        let ready = lr["ready"] == true;
        let streak = lr["streak"].as_i64().unwrap_or(1);
        let step = lr["step"].as_i64().unwrap_or(0) as usize;
        let rewards: Vec<String> = lr["rewards"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
        let milestones: Vec<(usize, String)> = lr["milestones"].as_array().map(|a| a.iter()
            .map(|m| (m["day"].as_u64().unwrap_or(0) as usize, m["reward"].as_str().unwrap_or("").to_string())).collect()).unwrap_or_default();
        let n = rewards.len().max(1);
        self.ui.text("Daily login reward", r.x + 22.0 * u, r.y + 26.0 * u, 18.0, INK, Align::Left, true);
        let note = if ready { format!("Day {} of {n}: claim it!", step + 1) } else { format!("Day {} of {n} claimed. Come back tomorrow for the next.", step + 1) };
        self.ui.text(&note, r.x + 210.0 * u, r.y + 27.0 * u, 13.0, if ready { ACCENT } else { MUTED }, Align::Left, false);
        if streak > n as i64 {
            self.ui.text(&format!("{streak} days in a row"), r.x + r.w - 22.0 * u, r.y + 27.0 * u, 13.0, C_GOLD, Align::Right, true);
        }
        // a week of days around today
        let shown = 7.min(n);
        let first = step.saturating_sub(1).min(n - shown);
        let gap = 8.0 * u;
        let bw_ = (r.w - 44.0 * u - 200.0 * u - gap * (shown as f32 - 1.0)) / shown as f32;
        for k in 0..shown {
            let i = first + k;
            let name = &rewards[i];
            let b = Rect::new(r.x + 22.0 * u + k as f32 * (bw_ + gap), r.y + 50.0 * u, bw_, 92.0 * u);
            let done = i < step || (i == step && !ready);
            let today = i == step;
            let big = milestones.iter().any(|(d, _)| *d == i + 1);
            rounded(b, 10.0 * u, if today { mix(CARD_HI, C_GOLD, 0.18) } else if big { mix(CARD_HI, C_PURPLE, 0.14) } else { CARD_HI });
            if today {
                rounded_outline(b, 10.0 * u, 2.0, if ready { C_GOLD } else { ACCENT });
            } else if big {
                rounded_outline(b, 10.0 * u, 1.5, alpha(C_PURPLE, 0.7));
            }
            self.ui.text(&format!("Day {}", i + 1), b.x + b.w / 2.0, b.y + 16.0 * u, 11.0, if big { C_PURPLE } else { MUTED }, Align::Center, true);
            reward_art(name, Rect::new(b.x, b.y + 24.0 * u, b.w, 36.0 * u), self.now_ms());
            let label = short_reward(name);
            let label = self.ui.fit(&label, 12.0, true, b.w - 6.0 * u);
            self.ui.text(&label, b.x + b.w / 2.0, b.y + 76.0 * u, 12.0, INK, Align::Center, true);
            if done {
                draw_circle(b.x + b.w - 12.0 * u, b.y + 12.0 * u, 9.0 * u, ACCENT);
                draw_icon("check", b.x + b.w - 12.0 * u, b.y + 12.0 * u, 12.0 * u, rgb(8, 30, 16));
            }
        }
        let br = Rect::new(r.x + r.w - 196.0 * u, r.y + 70.0 * u, 176.0 * u, 50.0 * u);
        let busy = self.online.is_busy(&Req::ClaimLogin);
        if ready {
            if self.ui.button_icon("Claim", "gift", br, Style::Gold) && !busy && self.reveal.is_none() {
                self.online.post("/api/login-reward/claim", json!({}), Req::ClaimLogin);
            }
        } else {
            self.ui.text("See you tomorrow", br.x + br.w / 2.0, br.y + br.h / 2.0, 14.0, MUTED, Align::Center, true);
        }
        // the whole ladder: how far along it you are, and its big days
        let ty = r.y + 160.0 * u;
        let (tx, tw) = (r.x + 22.0 * u, r.w - 44.0 * u);
        let at = |day: usize| tx + tw * (day as f32 - 1.0) / (n as f32 - 1.0).max(1.0);
        let reached = if ready { step } else { step + 1 };
        rounded(Rect::new(tx, ty - 3.0 * u, tw, 6.0 * u), 3.0 * u, CARD_HI);
        if reached > 0 {
            rounded(Rect::new(tx, ty - 3.0 * u, (at(reached) - tx).max(6.0 * u), 6.0 * u), 3.0 * u, ACCENT);
        }
        for (day, _) in &milestones {
            draw_circle(at(*day), ty, 5.0 * u, if *day <= reached { ACCENT } else { C_PURPLE });
        }
        draw_circle(at(step + 1), ty, 7.0 * u, if ready { C_GOLD } else { ACCENT });
        let k = milestones.len().max(1) as f32;
        let pg = 8.0 * u;
        let pw = (tw - pg * (k - 1.0)) / k;
        for (j, (day, reward)) in milestones.iter().enumerate() {
            let pr = Rect::new(tx + j as f32 * (pw + pg), ty + 14.0 * u, pw, 30.0 * u);
            let got = *day <= reached;
            let col = if got { ACCENT } else { C_PURPLE };
            rounded(pr, 15.0 * u, mix(CARD_HI, col, 0.12));
            rounded_outline(pr, 15.0 * u, 1.0, alpha(col, 0.6));
            let text = self.ui.fit(&format!("Day {day}: {}", short_reward(reward)), 12.0, true, pw - 16.0 * u);
            self.ui.text(&text, pr.x + pr.w / 2.0, pr.y + pr.h / 2.0, 12.0, if got { ACCENT } else { INK }, Align::Center, true);
        }
        if let Some((day, reward)) = milestones.iter().find(|(d, _)| *d > step + 1) {
            let left = day - (step + 1);
            let text = format!("Next big one: {reward} on day {day}, {left} day{} away. Missing a day starts again from day 1.",
                               if left == 1 { "" } else { "s" });
            let text = self.ui.fit(&text, 12.0, false, r.w - 44.0 * u);
            self.ui.text(&text, r.x + 22.0 * u, r.y + r.h - 14.0 * u, 12.0, MUTED, Align::Left, false);
        }
        r.y + r.h
    }
}

/// A login reward, short enough for a day's box: "2 Royal", "250,000".
fn short_reward(name: &str) -> String {
    name.split(" + ").map(|part| {
        let part = part.replace(" snake points", "").replace(" Crates", "").replace(" Crate", "");
        match part.replace(',', "").parse::<i64>() {
            Ok(n) if n >= 1_000_000 && n % 100_000 == 0 => format!("{}M", n as f64 / 1e6),
            Ok(n) if n >= 10_000 && n % 1_000 == 0 => format!("{}k", n / 1_000),
            _ => part,
        }
    }).collect::<Vec<_>>().join(" + ")
}

/// A login reward's picture: its (best) loot box, or a coin.
fn reward_art(name: &str, r: Rect, now: f64) {
    let cat = catalog();
    let best = cat.crates.iter().filter(|c| name.contains(c.name.trim_end_matches(" Crate"))).max_by_key(|c| c.price);
    match best {
        Some(c) => crate_art(r, rgbt(hex(&c.color)), now),
        None => draw_icon("coin", r.x + r.w / 2.0, r.y + r.h / 2.0, 24.0 * r.h / 36.0, C_GOLD),
    }
}

/// What's in one side of a trade, as words: "Galaxy, Snowflake + 5,000 points".
fn trade_side(items: &Value, points: &Value) -> String {
    let cat = catalog();
    let mut parts: Vec<String> = items.as_array().map(|a| a.iter().filter_map(|i| i.as_str()).map(|i| cat.item_name(i)).collect())
        .unwrap_or_default();
    let p = points.as_i64().unwrap_or(0);
    if p > 0 {
        parts.push(format!("{} points", format_points(p)));
    }
    if parts.is_empty() { "nothing".into() } else { parts.join(", ") }
}

/// How a skin or pack is got (for the Index).
pub fn how_to_get(item: &str) -> String {
    let cat = catalog();
    if let Some(price) = cat.item_price(item) {
        return format!("Shop: {}", format_points(price));
    }
    if let Some(sk) = item.strip_prefix("skin:").and_then(|k| cat.skin(k)) {
        if sk.series == "Pass" {
            return "Battle pass only".into();
        }
        if sk.series == "Gift" {
            return "A one-off gift".into();
        }
        if sk.series == "Ranked" {
            let rank = cat.ranks.iter().find(|r| r.rewards.iter().any(|w| *w == sk.name)).map(|r| r.name.clone());
            return rank.map_or("Ranked reward".into(), |r| format!("Reach {r}"));
        }
        if let Some(c) = cat.crates.iter().find(|c| !c.series.is_empty() && c.series == sk.series) {
            return format!("{} only", c.name);
        }
    }
    let rarity = cat.item_rarity(item);
    if rarity == "legendary" || rarity == "mythic" { "Loot boxes (Royal Crate too)".into() } else { "Loot boxes".into() }
}

impl App {
    // ---- trading with other players

    fn trades(&mut self, body: Rect) {
        let u = self.ui.u;
        self.ui.text("Swap skins and packs (and snake points) with any player. Ranked and gift skins can't be traded.",
                     body.x, body.y + 18.0 * u, 14.0, MUTED, Align::Left, false);
        if self.ui.button_icon("New trade", "plus", Rect::new(body.x + body.w - 170.0 * u, body.y, 160.0 * u, 40.0 * u), Style::Primary) {
            self.trade_draft = Some(crate::app::TradeDraft::default());
            self.online.their_items = None;
        }
        let list = Rect::new(body.x, body.y + 54.0 * u, body.w, body.h - 54.0 * u);
        let Some(t) = self.online.trades.clone() else {
            self.loading(list);
            return;
        };
        let get = |k: &str| t[k].as_array().cloned().unwrap_or_default();
        let (incoming, outgoing, history) = (get("incoming"), get("outgoing"), get("history"));
        let row = 84.0 * u;
        let total = (incoming.len() + outgoing.len() + history.len()) as f32 * (row + 8.0 * u) + 3.0 * 40.0 * u;
        let off = self.ui.begin_scroll("trades", list, total);
        let mut y = list.y - off;
        if incoming.is_empty() && outgoing.is_empty() && history.is_empty() {
            self.ui.text("No trades yet. Start one with New trade.", list.x + list.w / 2.0, y + 60.0 * u, 17.0, MUTED, Align::Center, false);
        }
        for (title, rows, kind) in [("Offered to you", &incoming, 0), ("Your offers", &outgoing, 1), ("Recent", &history, 2)] {
            if rows.is_empty() {
                continue;
            }
            self.ui.text(title, list.x, y + 16.0 * u, 17.0, INK, Align::Left, true);
            y += 36.0 * u;
            for tr in rows.iter() {
                let r = Rect::new(list.x, y, list.w - 12.0 * u, row);
                y += row + 8.0 * u;
                if !self.ui.visible(r) {
                    continue;
                }
                rounded(r, 12.0 * u, if kind == 2 { alpha(CARD, 0.6) } else { CARD });
                let (from, to) = (tr["from"].as_str().unwrap_or("?"), tr["to"].as_str().unwrap_or("?"));
                let give = trade_side(&tr["give"], &tr["give_points"]);
                let want = trade_side(&tr["want"], &tr["want_points"]);
                let tw = r.w - 300.0 * u;
                let (l1, l2) = match kind {
                    0 => (format!("{from} gives you: {give}"), format!("You give {from}: {want}")),
                    1 => (format!("You give {to}: {give}"), format!("{to} gives you: {want}")),
                    _ => (format!("{from} > {to}: {give}"), format!("{to} > {from}: {want}")),
                };
                let l1 = self.ui.fit(&l1, 15.0, true, tw);
                let l2 = self.ui.fit(&l2, 15.0, false, tw);
                self.ui.text(&l1, r.x + 18.0 * u, r.y + 28.0 * u, 15.0, if kind == 0 { ACCENT } else { INK }, Align::Left, true);
                self.ui.text(&l2, r.x + 18.0 * u, r.y + 56.0 * u, 15.0, MUTED, Align::Left, false);
                let id = tr["id"].clone();
                let bx = r.x + r.w - 270.0 * u;
                match kind {
                    0 => {
                        if self.ui.button("Accept", Rect::new(bx, r.y + 20.0 * u, 124.0 * u, 44.0 * u), Style::Primary) {
                            self.online.post("/api/trades/answer", json!({"id": id, "action": "accept"}), Req::AnswerTrade);
                        }
                        if self.ui.button("Decline", Rect::new(bx + 134.0 * u, r.y + 20.0 * u, 120.0 * u, 44.0 * u), Style::Secondary) {
                            self.online.post("/api/trades/answer", json!({"id": id, "action": "decline"}), Req::AnswerTrade);
                        }
                    }
                    1 => {
                        if self.ui.button("Cancel offer", Rect::new(bx + 74.0 * u, r.y + 20.0 * u, 180.0 * u, 44.0 * u), Style::Secondary) {
                            self.online.post("/api/trades/answer", json!({"id": id, "action": "cancel"}), Req::AnswerTrade);
                        }
                    }
                    _ => {
                        let st = tr["status"].as_str().unwrap_or("");
                        let col = if st == "accepted" { ACCENT } else { FAINT };
                        let label = format!("{}{}", st[..1.min(st.len())].to_uppercase(), &st[1.min(st.len())..]);
                        self.ui.text(&label, r.x + r.w - 24.0 * u, r.y + r.h / 2.0, 14.0, col, Align::Right, true);
                    }
                }
            }
            y += 12.0 * u;
        }
        self.ui.end_scroll();
    }

    /// Put a trade together: pick a player, what you give and what you want.
    fn trade_popup(&mut self) {
        let Some(mut d) = self.trade_draft.take() else { return };
        let u = self.ui.u;
        let (sw, sh) = (self.ui.w, self.ui.h);
        self.ui.input = true;
        draw_rectangle(0.0, 0.0, sw, sh, alpha(rgb(4, 8, 12), 0.72));
        let w = (900.0 * u).min(sw - 24.0 * u);
        let h = (620.0 * u).min(sh - 24.0 * u);
        let r = Rect::new((sw - w) / 2.0, (sh - h) / 2.0, w, h);
        self.ui.card(r);
        let x = r.x + 26.0 * u;
        let iw = w - 52.0 * u;
        self.ui.text("New trade", x, r.y + 32.0 * u, 22.0, INK, Align::Left, true);
        // who with
        let mut name = d.to.clone();
        let entered = self.ui.text_input("trade_to", &mut name, Rect::new(x, r.y + 56.0 * u, iw - 150.0 * u, 42.0 * u), "Player's name", false, 20);
        let load = self.ui.button("Find", Rect::new(x + iw - 138.0 * u, r.y + 56.0 * u, 138.0 * u, 42.0 * u), Style::Secondary);
        if (load || entered) && !name.trim().is_empty() {
            self.online.get(&format!("/api/users/{}/items", crate::online::urlencode(name.trim())), Req::UserItems(name.trim().to_string()));
        }
        if name != d.to {
            d.want.clear(); // (another player: their items aren't these)
        }
        d.to = name;
        let theirs_loaded = self.online.their_items.as_ref().filter(|(n, _)| n.eq_ignore_ascii_case(d.to.trim())).cloned();
        // the two sides
        let top = r.y + 114.0 * u;
        let col_w = (iw - 20.0 * u) / 2.0;
        let bottom = r.y + h - 80.0 * u;
        let cat = catalog();
        let mine: Vec<(String, i64)> = {
            let me = self.online.me.clone().unwrap_or_default();
            let mut v: Vec<(String, i64)> = me["counts"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(1))).collect())
                .unwrap_or_default();
            v.retain(|(it, n)| *n > 0 && !cat.untradeable.contains(it));
            v.sort_by_key(|(it, _)| (std::cmp::Reverse(cat.rarity_index(&cat.item_rarity(it))), cat.item_name(it)));
            v
        };
        let theirs: Vec<(String, i64)> = theirs_loaded.as_ref().map(|(_, m)| {
            let mut v: Vec<(String, i64)> = m.as_object().map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(1))).collect()).unwrap_or_default();
            v.sort_by_key(|(it, _)| (std::cmp::Reverse(cat.rarity_index(&cat.item_rarity(it))), cat.item_name(it)));
            v
        }).unwrap_or_default();
        let my_points = self.online.points() as i32;
        for side in 0..2 {
            let cx = x + side as f32 * (col_w + 20.0 * u);
            let title = if side == 0 { "You give".to_string() } else { format!("You get{}", if d.to.trim().is_empty() { String::new() } else { format!(" from {}", d.to.trim()) }) };
            self.ui.text(&title, cx, top + 12.0 * u, 16.0, INK, Align::Left, true);
            let area = Rect::new(cx, top + 30.0 * u, col_w, bottom - top - 90.0 * u);
            rounded(area, 10.0 * u, alpha(BLACK, 0.18));
            let items = if side == 0 { &mine } else { &theirs };
            let picked = if side == 0 { &mut d.give } else { &mut d.want };
            if side == 1 && theirs_loaded.is_none() {
                self.ui.text("Type a name and press Find", area.x + area.w / 2.0, area.y + area.h / 2.0, 14.0, MUTED, Align::Center, false);
            } else if items.is_empty() {
                self.ui.text("Nothing to trade", area.x + area.w / 2.0, area.y + area.h / 2.0, 14.0, MUTED, Align::Center, false);
            }
            let chip_h = 34.0 * u;
            let cols = 2;
            let cw = (area.w - 24.0 * u - 8.0 * u) / cols as f32;
            let off = self.ui.begin_scroll(&format!("trade_side{side}"), area, items.len().div_ceil(cols) as f32 * (chip_h + 6.0 * u) + 20.0 * u);
            for (i, (it, n)) in items.iter().enumerate() {
                let cr = Rect::new(area.x + 8.0 * u + (i % cols) as f32 * (cw + 8.0 * u), area.y + 8.0 * u + (i / cols) as f32 * (chip_h + 6.0 * u) - off, cw, chip_h);
                if !self.ui.visible(cr) {
                    continue;
                }
                let label = if *n > 1 { format!("{} x{n}", cat.item_name(it)) } else { cat.item_name(it) };
                let col = rgbt(cat.rarity_color(&cat.item_rarity(it)));
                let on = picked.contains(it);
                if self.ui.chip(&label, on, cr, col) {
                    if on {
                        picked.retain(|p| p != it);
                    } else if picked.len() < 12 {
                        picked.push(it.clone());
                    }
                }
            }
            self.ui.end_scroll();
            // points on this side
            let py = area.y + area.h + 12.0 * u;
            self.ui.text("+ points", cx, py + 20.0 * u, 14.0, MUTED, Align::Left, true);
            let pts = if side == 0 { &mut d.give_points } else { &mut d.want_points };
            let max = if side == 0 { my_points.max(0) } else { 10_000_000 };
            *pts = self.ui.stepper(&format!("trade_points{side}"), (*pts).min(max), 0, max, Rect::new(cx + 80.0 * u, py, col_w - 80.0 * u, 40.0 * u));
        }
        let bw = (iw - 12.0 * u) / 2.0;
        let by = r.y + h - 64.0 * u;
        if self.ui.button("Cancel", Rect::new(x, by, bw, 46.0 * u), Style::Secondary) || self.ui.hotkey(KeyCode::Escape) {
            self.ui.input = false;
            return;
        }
        let busy = self.online.is_busy(&Req::OfferTrade);
        if self.ui.button_icon(if busy { "Sending..." } else { "Offer trade" }, "market", Rect::new(x + bw + 12.0 * u, by, bw, 46.0 * u), Style::Primary) && !busy {
            if d.to.trim().is_empty() {
                self.ui.toast("Who's it with? Type a player's name.", C_RED);
            } else if (d.give.is_empty() && d.give_points == 0) || (d.want.is_empty() && d.want_points == 0) {
                self.ui.toast("Put something on both sides.", C_RED);
            } else {
                self.online.post("/api/trades", json!({"to": d.to.trim(), "give": d.give, "give_points": d.give_points,
                                                       "want": d.want, "want_points": d.want_points}), Req::OfferTrade);
            }
        }
        self.ui.input = false;
        self.trade_draft = Some(d); // (closed when the offer's sent)
    }

    // ---- the index of every skin and pack

    fn index(&mut self, body: Rect) {
        let u = self.ui.u;
        let cat = catalog();
        // (one-off gift skins aren't listed: nobody can get them)
        let mut items: Vec<String> = cat.skins.iter().filter(|s| s.series != "Gift").map(|s| format!("skin:{}", s.id)).chain(cat.packs.iter().map(|p| format!("pack:{}", p.id))).collect();
        items.sort_by_key(|i| (cat.rarity_index(&cat.item_rarity(i)), i.starts_with("pack:"), cat.item_name(i)));
        let owned_n = items.iter().filter(|i| self.online.owns(i)).count();
        // search and filter
        let mut q = self.fields.remove("index_q").unwrap_or_default();
        self.ui.text_input("index_q", &mut q, Rect::new(body.x, body.y, 260.0 * u, 40.0 * u), "Search by name, series or rarity", false, 30);
        let filter = self.fields.get("index_f").cloned().unwrap_or_else(|| "all".into());
        let mut fx = body.x + 272.0 * u;
        for (key, label) in [("all", "All"), ("owned", "Owned"), ("missing", "Missing")] {
            let w = self.ui.measure(label, 13.0, true) + 28.0 * u;
            if self.ui.chip(label, filter == key, Rect::new(fx, body.y + 4.0 * u, w, 32.0 * u), ACCENT) {
                self.fields.insert("index_f", key.into());
            }
            fx += w + 6.0 * u;
        }
        if self.online.logged_in() {
            self.ui.text(&format!("You have {owned_n} of {}", items.len()), body.x + body.w - 10.0 * u, body.y + 20.0 * u, 15.0, MUTED, Align::Right, true);
        }
        let filter = self.fields.get("index_f").cloned().unwrap_or_else(|| "all".into());
        let words: Vec<String> = q.to_lowercase().split_whitespace().map(String::from).collect();
        items.retain(|it| {
            let owned = self.online.owns(it);
            let series = it.strip_prefix("skin:").and_then(|k| cat.skin(k)).map(|s| s.series.clone()).unwrap_or_default();
            let text = format!("{} {} {} {}", cat.item_name(it), series, cat.rarity_name(&cat.item_rarity(it)), how_to_get(it)).to_lowercase();
            (filter == "all" || (filter == "owned") == owned) && words.iter().all(|w| text.contains(w))
        });
        self.fields.insert("index_q", q);
        let grid = Rect::new(body.x, body.y + 54.0 * u, body.w, body.h - 54.0 * u);
        let card_h = 214.0;
        let (cols, cw, gap, total) = self.grid(grid, items.len(), 190.0, card_h);
        let off = self.ui.begin_scroll("index", grid, total);
        for (i, item) in items.iter().enumerate() {
            let r = Rect::new(grid.x + (i % cols) as f32 * (cw + gap), grid.y + (i / cols) as f32 * (card_h * u + gap) - off, cw, card_h * u);
            if !self.ui.visible(r) {
                continue;
            }
            self.item_card(r, item);
            let how = self.ui.fit(&how_to_get(item), 12.0, false, r.w - 28.0 * u);
            self.ui.text(&how, r.x + 14.0 * u, r.y + 182.0 * u, 12.0, MUTED, Align::Left, false);
            if self.online.logged_in() {
                if self.online.owns(item) {
                    let c = self.online.copies(item);
                    draw_circle(r.x + r.w - 20.0 * u, r.y + 22.0 * u, 11.0 * u, ACCENT);
                    draw_icon("check", r.x + r.w - 20.0 * u, r.y + 22.0 * u, 13.0 * u, rgb(8, 30, 16));
                    if c > 1 {
                        self.ui.text(&format!("x{c}"), r.x + r.w - 36.0 * u, r.y + 22.0 * u, 12.0, ACCENT, Align::Right, true);
                    }
                } else {
                    rounded(r, 14.0 * u, alpha(BG_BOTTOM, 0.35)); // (not yours yet: a little faded)
                }
            }
        }
        if items.is_empty() {
            self.ui.text("Nothing matches.", grid.x + grid.w / 2.0, grid.y + 60.0 * u, 17.0, MUTED, Align::Center, false);
        }
        self.ui.end_scroll();
    }
}
