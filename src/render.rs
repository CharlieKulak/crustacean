//! Drawing the board in 2D: everything is procedural (no image files), drawn
//! smoothly in between the game's moves.

use crate::cosmetics::{skin_color, skin_outline, Rgb, Skin};
use crate::game::{Dir, Game};
use crate::ui::{alpha, darken, lighten, mix, rgb, rgbt, rounded, star};
use macroquad::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct BoardColors {
    pub light: Color,
    pub dark: Color,
    /// Minesweeper: the cells a snake has been over (dug up, like the original's brown tiles).
    pub sand: Color,
    pub sand_dark: Color,
    pub border: Color,
    pub header: Color,
}

impl Default for BoardColors {
    fn default() -> Self {
        BoardColors { light: rgb(170, 215, 81), dark: rgb(162, 209, 73), sand: rgb(229, 194, 159),
                      sand_dark: rgb(215, 184, 153), border: rgb(87, 138, 52), header: rgb(74, 117, 44) }
    }
}

impl BoardColors {
    /// The colour of one board cell.
    pub fn tile(&self, x: i32, y: i32, dug: bool) -> Color {
        match ((x + y) % 2 == 0, dug) {
            (true, false) => self.light,
            (false, false) => self.dark,
            (true, true) => self.sand,
            (false, true) => self.sand_dark,
        }
    }

    pub fn from_pack(colors: &HashMap<String, Rgb>) -> BoardColors {
        let d = BoardColors::default();
        let get = |k: &str, def: Color| colors.get(k).map(|c| rgbt(*c)).unwrap_or(def);
        BoardColors {
            light: get("board_light", d.light),
            dark: get("board_dark", d.dark),
            sand: get("board_sand", d.sand),
            sand_dark: get("board_sand_dark", d.sand_dark),
            border: get("border", d.border),
            header: get("header", d.header),
        }
    }
}

pub fn fruit_color(f: &str) -> Color {
    match f {
        "orange" => rgb(245, 154, 27),
        "cherry" => rgb(200, 16, 46),
        "grape" => rgb(123, 63, 181),
        "strawberry" => rgb(232, 36, 60),
        "lemon" => rgb(255, 213, 46),
        "banana" => rgb(255, 217, 59),
        _ => rgb(231, 71, 29),
    }
}

pub const GATE_OPEN: Color = rgb(46, 196, 92);
pub const GATE_SHUT: Color = rgb(214, 56, 48);
pub const GATE_SOON: Color = rgb(255, 184, 28);

/// How much of a gate's current state is left (1 = just switched, 0 = switching now), and
/// whether it switches in the next couple of moves.
pub fn gate_timer(g: &Game, gt: &crate::game::Gate) -> (f32, bool) {
    let period = gt.period.max(2) as i64;
    let half = period / 2;
    let pos = (g.tick + gt.phase as i64).rem_euclid(period);
    let (left, span) = if pos < half { (half - pos, half) } else { (period - pos, period - half) };
    (left as f32 / span as f32, left <= 2)
}

pub const PORTAL_COLORS: [Color; 6] =
    [rgb(255, 122, 0), rgb(0, 162, 255), rgb(225, 59, 214), rgb(255, 212, 0), rgb(32, 201, 151), rgb(255, 71, 87)];

const MINE_NUMBER: [Color; 9] = [WHITE, rgb(25, 118, 210), rgb(56, 142, 60), rgb(211, 47, 47), rgb(123, 31, 162),
                                 rgb(255, 143, 0), rgb(0, 151, 167), rgb(66, 66, 66), rgb(158, 158, 158)];

pub struct Particle {
    pub pos: Vec2,
    pub vel: Vec2,
    pub life: f32,
    pub max: f32,
    pub color: Color,
    pub size: f32,
}

#[derive(Default)]
pub struct Effects {
    pub particles: Vec<Particle>,
    pub shake: f32,
}

impl Effects {
    /// A burst at a board cell (in cell units; the renderer turns them into pixels).
    pub fn burst(&mut self, x: i32, y: i32, color: Color, n: usize) {
        for k in 0..n {
            let a = k as f32 / n as f32 * std::f32::consts::TAU + rand::gen_range(-0.3, 0.3);
            let speed = rand::gen_range(1.5, 4.5);
            self.particles.push(Particle {
                pos: vec2(x as f32 + 0.5, y as f32 + 0.5),
                vel: vec2(a.cos(), a.sin()) * speed,
                life: 0.0,
                max: rand::gen_range(0.35, 0.7),
                color,
                size: rand::gen_range(0.07, 0.16),
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in self.particles.iter_mut() {
            p.life += dt;
            p.pos += p.vel * dt;
            p.vel *= 1.0 - 3.0 * dt;
        }
        self.particles.retain(|p| p.life < p.max);
        self.shake = (self.shake - dt * 2.5).max(0.0);
    }
}

pub struct View<'a> {
    pub game: &'a Game,
    /// How far the snakes are between their last move and the next (0..1).
    pub t: f32,
    /// One skin per snake.
    pub skins: &'a [Skin],
    pub colors: BoardColors,
    pub now_ms: f64,
    /// Show the mines (after the game).
    pub reveal: bool,
    /// Names over the snakes (versus) and which is yours.
    pub names: bool,
    pub you: Option<usize>,
}

/// The board's place on screen: a square-celled rectangle centred in `area`.
pub fn board_rect(g: &Game, area: Rect) -> (Rect, f32) {
    let cell = (area.w / g.w as f32).min(area.h / g.h as f32).floor().max(2.0);
    let (w, h) = (cell * g.w as f32, cell * g.h as f32);
    (Rect::new((area.x + (area.w - w) / 2.0).round(), (area.y + (area.h - h) / 2.0).round(), w, h), cell)
}

/// Where each snake's cells are drawn right now, in cell units (interpolated).
pub fn snake_points(g: &Game, idx: usize, t: f32) -> Vec<Vec2> {
    let s = &g.snakes[idx];
    let prev = g.prev.get(idx);
    let ease = t.clamp(0.0, 1.0);
    s.body
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let now = vec2(p.x as f32, p.y as f32);
            let from = prev.and_then(|pv| pv.get(i)).map(|&(x, y)| vec2(x as f32, y as f32));
            match from {
                // (no sliding across a wrap-around or a teleport)
                Some(f) if !s.no_interp && (f - now).length() <= 1.01 && !g.over => f.lerp(now, ease),
                _ => now,
            }
        })
        .collect()
}

/// The smallest a cell is drawn: bigger boards scroll with the snake instead of shrinking.
pub const MIN_CELL: f32 = 32.0;

/// Which snake the view follows on boards too big to show whole.
pub fn focus_snake(v: &View) -> usize {
    v.you.unwrap_or(0).min(v.game.snakes.len().saturating_sub(1))
}

pub fn draw_board(v: &View, area: Rect, fx: &Effects) -> (Rect, f32) {
    let g = v.game;
    let (mut r, mut c) = board_rect(g, area);
    // Too big to show whole at a readable size: keep cells big and follow the snake.
    let min_cell = MIN_CELL * (area.h / 700.0).clamp(0.7, 1.6);
    let scrolling = c < min_cell && !g.snakes.is_empty();
    if scrolling {
        c = min_cell.floor();
        let head = snake_points(g, focus_snake(v), v.t).first().copied().unwrap_or(vec2(0.0, 0.0));
        let (bw, bh) = (g.w as f32 * c, g.h as f32 * c);
        let follow = |len: f32, avail: f32, start: f32, at: f32| {
            if len <= avail {
                start + (avail - len) / 2.0 // (fits this way: centre it)
            } else {
                (start + avail / 2.0 - (at + 0.5) * c).clamp(start + avail - len, start)
            }
        };
        r = Rect::new(follow(bw, area.w, area.x, head.x), follow(bh, area.h, area.y, head.y), bw, bh);
        crate::ui::set_clip(Some(area));
    }
    // the cells that are on screen
    let x0 = (((area.x - r.x) / c).floor().max(0.0) as i32).min(g.w);
    let y0 = (((area.y - r.y) / c).floor().max(0.0) as i32).min(g.h);
    let x1 = (((area.x + area.w - r.x) / c).ceil() as i32 + 1).clamp(0, g.w);
    let y1 = (((area.y + area.h - r.y) / c).ceil() as i32 + 1).clamp(0, g.h);
    if fx.shake > 0.0 {
        r.x += rand::gen_range(-1.0, 1.0) * fx.shake * c;
        r.y += rand::gen_range(-1.0, 1.0) * fx.shake * c;
    }
    // frame
    let pad = (c * 0.22).max(4.0);
    rounded(Rect::new(r.x - pad, r.y - pad + 4.0, r.w + pad * 2.0, r.h + pad * 2.0), pad * 1.5, alpha(BLACK, 0.3));
    rounded(Rect::new(r.x - pad, r.y - pad, r.w + pad * 2.0, r.h + pad * 2.0), pad * 1.5, v.colors.border);
    // checkerboard (minesweeper: cells a snake has been over are dug up, sandy brown)
    let mines = g.has("minesweeper");
    for y in y0..y1 {
        for x in x0..x1 {
            let col = v.colors.tile(x, y, mines && g.revealed.contains(&(x, y)));
            draw_rectangle(r.x + x as f32 * c, r.y + y as f32 * c, c + 0.5, c + 0.5, col);
        }
    }
    let at = |x: f32, y: f32| vec2(r.x + x * c, r.y + y * c);
    let time = v.now_ms / 1000.0;

    // minesweeper numbers on visited cells
    if g.has("minesweeper") {
        for &(x, y) in &g.revealed {
            let n = g.mine_count(x, y);
            if n > 0 && !g.mines.contains(&(x, y)) {
                let p = at(x as f32 + 0.5, y as f32 + 0.5);
                draw_circle(p.x, p.y, c * 0.28, alpha(WHITE, 0.35));
                let txt = n.to_string();
                let size = (c * 0.5) as u16;
                let d = measure_text(&txt, None, size, 1.0);
                draw_text(&txt, p.x - d.width / 2.0, p.y + d.height / 2.0, size as f32, MINE_NUMBER[n as usize % 9]);
            }
        }
    }
    // sokoban targets
    for &(x, y) in &g.targets {
        let p = at(x as f32 + 0.5, y as f32 + 0.5);
        draw_circle_lines(p.x, p.y, c * 0.36, c * 0.08, alpha(rgb(60, 60, 60), 0.6));
        draw_circle(p.x, p.y, c * 0.1, alpha(rgb(60, 60, 60), 0.6));
    }
    // arrows
    for (&(x, y), &d) in &g.arrows {
        let p = at(x as f32 + 0.5, y as f32 + 0.5);
        rounded(Rect::new(p.x - c * 0.42, p.y - c * 0.42, c * 0.84, c * 0.84), c * 0.15, alpha(rgb(255, 255, 255), 0.45));
        arrow(p, c * 0.32, d, rgb(230, 120, 20));
    }
    // walls and statues
    for w in g.walls.values() {
        let (x, y) = (w.x, w.y);
        if x < x0 - 1 || x > x1 || y < y0 - 1 || y > y1 {
            continue; // (off screen on a big board)
        }
        let p = at(w.x as f32, w.y as f32);
        let active = g.wall_active(w);
        if w.kind == "statue" {
            rounded(Rect::new(p.x + c * 0.06, p.y + c * 0.06, c * 0.88, c * 0.88), c * 0.3, rgb(150, 150, 160));
            draw_circle(p.x + c * 0.5, p.y + c * 0.5, c * 0.22, rgb(120, 120, 130));
            draw_circle(p.x + c * 0.4, p.y + c * 0.38, c * 0.08, rgb(190, 190, 200));
        } else if active {
            let base = if w.dim.is_some() { if w.dim == Some(0) { rgb(80, 90, 200) } else { rgb(190, 70, 160) } } else { rgb(98, 78, 64) };
            rounded(Rect::new(p.x + c * 0.04, p.y + c * 0.1, c * 0.92, c * 0.9), c * 0.16, darken(base, 0.35));
            rounded(Rect::new(p.x + c * 0.04, p.y + c * 0.02, c * 0.92, c * 0.86), c * 0.16, base);
            rounded(Rect::new(p.x + c * 0.14, p.y + c * 0.1, c * 0.72, c * 0.2), c * 0.1, alpha(WHITE, 0.18));
        } else {
            let base = if w.dim == Some(0) { rgb(80, 90, 200) } else { rgb(190, 70, 160) };
            crate::ui::rounded_outline(Rect::new(p.x + c * 0.12, p.y + c * 0.12, c * 0.76, c * 0.76), c * 0.14, (c * 0.06).max(1.0), alpha(base, 0.5));
        }
    }
    // gates: green and open, red and shut, amber when about to switch, with a timer bar
    for gt in &g.gates {
        let (x, y) = (gt.x, gt.y);
        if x < x0 - 1 || x > x1 || y < y0 - 1 || y > y1 {
            continue; // (off screen on a big board)
        }
        let p = at(gt.x as f32, gt.y as f32);
        let (left, soon) = gate_timer(g, gt);
        let pulse = 0.5 + 0.5 * ((time * 10.0) as f32).sin();
        if gt.open {
            let col = if soon { mix(GATE_OPEN, GATE_SOON, pulse) } else { GATE_OPEN };
            rounded(Rect::new(p.x + c * 0.04, p.y + c * 0.04, c * 0.92, c * 0.92), c * 0.12, alpha(col, 0.38));
            crate::ui::rounded_outline(Rect::new(p.x + c * 0.06, p.y + c * 0.06, c * 0.88, c * 0.88), c * 0.12,
                                       (c * 0.06).max(1.5), col);
            for k in [0.13, 0.87] {
                rounded(Rect::new(p.x + c * k - c * 0.06, p.y + c * 0.06, c * 0.12, c * 0.88), c * 0.05, darken(col, 0.2));
            }
        } else {
            let col = if soon { mix(GATE_SHUT, GATE_SOON, pulse) } else { GATE_SHUT };
            rounded(Rect::new(p.x + c * 0.03, p.y + c * 0.08, c * 0.94, c * 0.9), c * 0.12, darken(col, 0.45));
            rounded(Rect::new(p.x + c * 0.03, p.y + c * 0.03, c * 0.94, c * 0.88), c * 0.12, col);
            for k in 0..4 {
                let xx = p.x + c * (0.2 + k as f32 * 0.2);
                draw_rectangle(xx - c * 0.045, p.y + c * 0.1, c * 0.09, c * 0.7, darken(col, 0.4));
            }
            draw_rectangle(p.x + c * 0.1, p.y + c * 0.42, c * 0.8, c * 0.08, darken(col, 0.4));
        }
        // how long until it switches: a bar along the bottom
        let bar = Rect::new(p.x + c * 0.14, p.y + c * 0.84, c * 0.72, (c * 0.09).max(2.0));
        rounded(bar, bar.h / 2.0, alpha(BLACK, 0.45));
        rounded(Rect::new(bar.x, bar.y, (bar.w * left).max(bar.h), bar.h), bar.h / 2.0, WHITE);
    }
    // mines (shown when the game's over)
    if v.reveal {
        for &(x, y) in &g.mines {
            let p = at(x as f32 + 0.5, y as f32 + 0.5);
            draw_circle(p.x, p.y, c * 0.28, rgb(40, 40, 46));
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                draw_line(p.x, p.y, p.x + a.cos() * c * 0.4, p.y + a.sin() * c * 0.4, c * 0.07, rgb(40, 40, 46));
            }
            draw_circle(p.x - c * 0.08, p.y - c * 0.08, c * 0.07, WHITE);
        }
    }
    // items
    for it in &g.items {
        let (x, y) = (it.x, it.y);
        if x < x0 - 1 || x > x1 || y < y0 - 1 || y > y1 {
            continue; // (off screen on a big board)
        }
        let bob = ((time * 3.0 + it.x as f64) as f32).sin() * c * 0.04;
        let p = at(it.x as f32 + 0.5, it.y as f32 + 0.5) + vec2(0.0, bob);
        if it.kind == "key" {
            let col = if g.has("portal") { PORTAL_COLORS[it.group as usize % 6] } else { rgb(255, 196, 0) };
            key_icon(p, c * 0.4, col);
        } else {
            shield_icon(p, c * 0.38, rgb(47, 128, 237));
        }
    }
    // fruit
    for a in &g.apples {
        let (x, y) = (a.x, a.y);
        if x < x0 - 1 || x > x1 || y < y0 - 1 || y > y1 {
            continue; // (off screen on a big board)
        }
        let p = at(a.x as f32 + 0.5, a.y as f32 + 0.5);
        let pulse = 1.0 + 0.04 * ((time * 4.0 + a.x as f64 * 0.7) as f32).sin();
        if g.has("portal") {
            let pc = PORTAL_COLORS[a.group as usize % 6];
            let spin = time as f32 * 2.0;
            for k in 0..3 {
                let rr = c * (0.46 - k as f32 * 0.04);
                draw_poly_lines(p.x, p.y, 12, rr, spin.to_degrees() * (1.0 + k as f32 * 0.3), c * 0.04, alpha(pc, 0.7));
            }
        }
        if a.poison {
            poison_fruit(p, c * 0.36 * pulse, time);
        } else {
            draw_fruit(&a.fruit, p, c * 0.4 * pulse);
        }
        if a.locked {
            let lp = p + vec2(c * 0.22, c * 0.22);
            draw_circle(lp.x, lp.y, c * 0.2, rgb(60, 60, 70));
            crate::ui::draw_icon("lock", lp.x, lp.y, c * 0.26, rgb(255, 196, 0));
        }
    }
    // snakes
    let order: Vec<usize> = (0..g.snakes.len()).collect();
    for &i in &order {
        let s = &g.snakes[i];
        let skin = &v.skins[i.min(v.skins.len() - 1)];
        let pts = snake_points(g, i, v.t);
        let faded = s.dead && g.versus();
        draw_snake(g, i, v.t, skin, &r, c, v.now_ms, faded);
        if v.names && !s.name.is_empty() {
            let h = at(pts[0].x + 0.5, pts[0].y - 0.35);
            let you = v.you == Some(i);
            let label = if you { format!("{} (you)", s.name) } else { s.name.clone() };
            let size = (c * 0.38).clamp(10.0, 18.0) as u16;
            let d = measure_text(&label, None, size, 1.0);
            rounded(Rect::new(h.x - d.width / 2.0 - 5.0, h.y - d.height - 4.0, d.width + 10.0, d.height + 8.0), 6.0,
                    alpha(BLACK, if faded { 0.25 } else { 0.5 }));
            draw_text(&label, h.x - d.width / 2.0, h.y, size as f32, if faded { alpha(WHITE, 0.5) } else { WHITE });
        }
    }
    // light mode: dark except near the heads
    if g.has("light") && !g.over {
        let heads: Vec<Vec2> = (0..g.snakes.len()).filter(|&i| !g.snakes[i].dead).map(|i| snake_points(g, i, v.t)[0]).collect();
        for y in y0..y1 {
            for x in x0..x1 {
                let d = heads.iter().map(|h| (vec2(x as f32, y as f32) - *h).length()).fold(f32::MAX, f32::min);
                let dark = ((d - 1.6) / 1.6).clamp(0.0, 0.94);
                if dark > 0.0 {
                    draw_rectangle(r.x + x as f32 * c, r.y + y as f32 * c, c + 0.5, c + 0.5, Color::new(0.03, 0.04, 0.08, dark));
                }
            }
        }
        // the fruit glows in the dark: a soft light round it, and the fruit on top
        for a in &g.apples {
            let (x, y) = (a.x, a.y);
            if x < x0 - 1 || x > x1 || y < y0 - 1 || y > y1 {
                continue;
            }
            let pnt = at(a.x as f32 + 0.5, a.y as f32 + 0.5);
            let col = if a.poison { rgb(160, 80, 230) } else { lighten(fruit_color(&a.fruit), 0.25) };
            let flicker = 1.0 + 0.08 * ((time * 3.0 + a.x as f64 * 1.3 + a.y as f64) as f32).sin();
            crate::ui::glow(pnt.x, pnt.y, c * 1.25 * flicker, col, 2.4);
            crate::ui::glow(pnt.x, pnt.y, c * 0.6, lighten(col, 0.5), 3.0);
            if a.poison {
                poison_fruit(pnt, c * 0.36, time);
            } else {
                draw_fruit(&a.fruit, pnt, c * 0.4);
            }
            if a.locked {
                let lp = pnt + vec2(c * 0.22, c * 0.22);
                draw_circle(lp.x, lp.y, c * 0.2, rgb(60, 60, 70));
                crate::ui::draw_icon("lock", lp.x, lp.y, c * 0.26, rgb(255, 196, 0));
            }
        }
    }
    // particles
    for p in &fx.particles {
        let k = 1.0 - p.life / p.max;
        let pos = at(p.pos.x, p.pos.y);
        draw_circle(pos.x, pos.y, p.size * c * k + 0.5, alpha(p.color, k));
    }
    if scrolling {
        crate::ui::set_clip(None);
    }
    (r, c)
}

/// One point along a snake's smooth body: where it is (in cells) and how far down
/// the body it is (0 = the head, 1 = the next cell...), for its colour.
#[derive(Clone, Copy)]
pub struct BodyPoint {
    pub pos: Vec2,
    pub k: f32,
    /// The body is broken here (it wrapped round the board edge or teleported).
    pub gap: bool,
}

/// The direction from cell `a` to the next cell `b` and whether the move wrapped
/// round the board edge; None if they aren't next to each other (a teleport).
fn step_dir(a: (i32, i32), b: (i32, i32), w: i32, h: i32, wrap: bool) -> (Option<(i32, i32)>, bool) {
    let (mut dx, mut dy) = (b.0 - a.0, b.1 - a.1);
    let mut wrapped = false;
    if wrap {
        if w > 2 && dx.abs() == w - 1 && dy == 0 {
            dx = if dx > 0 { -1 } else { 1 };
            wrapped = true;
        } else if h > 2 && dy.abs() == h - 1 && dx == 0 {
            dy = if dy > 0 { -1 } else { 1 };
            wrapped = true;
        }
    }
    if dx.abs() + dy.abs() != 1 {
        return (None, false);
    }
    (Some((dx, dy)), wrapped)
}

/// A snake's smooth centre line, like the Python game's SnakePath: cell i (0 = head)
/// covers s from i - 0.5 to i + 0.5. Straight cells are straight; turning cells are
/// quarter circles round the corner the two edges share, so the body bends smoothly.
struct SnakeLine {
    cells: Vec<(i32, i32)>,
    entry: Vec<(i32, i32)>,
    exit: Vec<(i32, i32)>,
    /// no line between cell i and i + 1 (a wrap or a teleport)
    cut: Vec<bool>,
}

impl SnakeLine {
    fn new(cells: Vec<(i32, i32)>, exit_dir: (i32, i32), w: i32, h: i32, wrap: bool) -> SnakeLine {
        let n = cells.len();
        let mut entry: Vec<Option<(i32, i32)>> = vec![None; n];
        let mut exit: Vec<Option<(i32, i32)>> = vec![None; n];
        let mut cut = vec![false; n];
        for i in 0..n.saturating_sub(1) {
            let (d, wrapped) = step_dir(cells[i + 1], cells[i], w, h, wrap);
            entry[i] = d;
            exit[i + 1] = d;
            if d.is_none() || wrapped {
                cut[i] = true;
            }
        }
        if n > 0 {
            exit[0] = Some(exit_dir);
        }
        let mut en = Vec::with_capacity(n);
        let mut ex = Vec::with_capacity(n);
        for i in 0..n {
            let e = entry[i].or(exit[i]).unwrap_or((1, 0));
            en.push(e);
            ex.push(exit[i].unwrap_or(e));
        }
        SnakeLine { cells, entry: en, exit: ex, cut }
    }

    fn cell_of(&self, s: f32) -> usize {
        ((s + 0.5).floor().max(0.0) as usize).min(self.cells.len() - 1)
    }

    /// Where the line is at `s`, in cell units (the cell's corner is 0, 0).
    fn point(&self, s: f32) -> Vec2 {
        let i = self.cell_of(s);
        let u = (i as f32 + 0.5 - s).clamp(0.0, 1.0);
        let c = vec2(self.cells[i].0 as f32 + 0.5, self.cells[i].1 as f32 + 0.5);
        let (e, x) = (self.entry[i], self.exit[i]);
        let ev = vec2(e.0 as f32, e.1 as f32);
        if e == x || (e.0 == -x.0 && e.1 == -x.1) {
            return c + ev * (u - 0.5);
        }
        let xv = vec2(x.0 as f32, x.1 as f32);
        let k = c - ev / 2.0 + xv / 2.0;
        let a0 = (-xv.y).atan2(-xv.x);
        let mut d = ev.y.atan2(ev.x) - a0;
        d = (d + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let a = a0 + d * u;
        k + vec2(a.cos(), a.sin()) * 0.5
    }
}

/// Points every `step` cells along a snake's body, following the same smooth line as
/// the Python game (quarter-circle corners), with the head and tail part way between cells.
pub fn body_path(g: &Game, idx: usize, t: f32, step: f32) -> Vec<BodyPoint> {
    let s = &g.snakes[idx];
    let mut cells: Vec<(i32, i32)> = s.body.iter().map(|p| (p.x, p.y)).collect();
    let n = cells.len();
    if n == 0 {
        return Vec::new();
    }
    let t = t.clamp(0.0, 1.0);
    let wrap = g.has("borderless");
    let (mut s_head, mut s_tail) = (0.0f32, (n - 1) as f32);
    if let Some(prev) = g.prev.get(idx) {
        if !s.no_interp && !g.over && !prev.is_empty() && prev[0] != cells[0] {
            if step_dir(prev[0], cells[0], g.w, g.h, wrap).0.is_some() && n > 1 && prev[0] == cells[1] {
                s_head = 1.0 - t;
            }
            let last = *prev.last().unwrap();
            if last != cells[n - 1] && step_dir(last, cells[n - 1], g.w, g.h, wrap).0.is_some() {
                cells.push(last); // the cell the tail is sliding out of
                s_tail = n as f32 - t;
            }
        }
    }
    let exit = s.planned.unwrap_or(s.dir).delta();
    let line = SnakeLine::new(cells, exit, g.w, g.h, wrap);
    let mut out = Vec::new();
    let mut sv = s_head;
    let mut last_cell = line.cell_of(s_head);
    loop {
        let sc = sv.min(s_tail);
        let cell = line.cell_of(sc);
        // crossing a wrap or teleport: don't join across it
        let gap = cell != last_cell && (last_cell.min(cell)..last_cell.max(cell)).any(|i| line.cut[i]);
        last_cell = cell;
        out.push(BodyPoint { pos: line.point(sc) - vec2(0.5, 0.5), k: sc - s_head, gap });
        if sv >= s_tail {
            break;
        }
        sv += step;
    }
    if out.len() == 1 {
        out.push(out[0]);
    }
    out
}

/// Skins whose colours change cell by cell (stripes and the like) keep crisp edges;
/// the rest blend smoothly along the body.
fn crisp(skin: &Skin) -> bool {
    matches!(skin.pattern.as_str(), "stripes" | "chevron" | "scales" | "spark" | "galaxy" | "lights")
}

/// A skin's colour at any point down the body (between cells too).
pub fn body_color(skin: &Skin, k: f32, n: usize, now: f64) -> Rgb {
    let k = k.max(0.0);
    if crisp(skin) {
        return skin_color(skin, (k.round() as usize).min(n.saturating_sub(1).max(0) + 1), n, now);
    }
    let i = k.floor() as usize;
    let a = skin_color(skin, i, n, now);
    let b = skin_color(skin, i + 1, n, now);
    let f = k - i as f32;
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * f).round() as u8;
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

#[allow(clippy::too_many_arguments)]
pub fn draw_snake(g: &Game, idx: usize, t: f32, skin: &Skin, board: &Rect, c: f32, now: f64, faded: bool) {
    let s = &g.snakes[idx];
    let pts = body_path(g, idx, t, 0.12);
    if pts.is_empty() {
        return;
    }
    let n = s.body.len();
    let at = |p: Vec2| vec2(board.x + (p.x + 0.5) * c, board.y + (p.y + 0.5) * c);
    let width = c * 0.8;
    // (knocked out in versus: greyed out, not see-through, so the overlapping pieces don't show)
    let fade = 1.0;
    let tint = |c: Color| if faded { mix(c, rgb(128, 132, 140), 0.65) } else { c };
    let len = pts.last().map_or(1.0, |p| p.k).max(0.01);
    // a little thinner towards the tail
    let radius = |k: f32| width / 2.0 * (1.0 - 0.28 * ((k - (len - 1.6)) / 1.6).clamp(0.0, 1.0));
    // A circle at every point, joined to the last by a bar, so the body is one smooth tube.
    let tube = |grow: f32, offset: Vec2, color: &dyn Fn(f32) -> Color| {
        for w in (1..pts.len()).rev() {
            let (a, b) = (pts[w - 1], pts[w]);
            let (pa, pb) = (at(a.pos) + offset, at(b.pos) + offset);
            let col = color(b.k);
            if !b.gap {
                draw_line(pa.x, pa.y, pb.x, pb.y, (radius(b.k) + grow) * 2.0, col);
            }
            draw_circle(pb.x, pb.y, radius(b.k) + grow, col);
        }
        let p0 = at(pts[0].pos) + offset;
        draw_circle(p0.x, p0.y, radius(0.0) + grow, color(0.0));
    };
    // outline, shadow, body
    // a soft shadow: evenly spaced faint circles, so it's the same darkness all along
    for w in (0..pts.len()).step_by(2) {
        let p = at(pts[w].pos) + vec2(0.0, c * 0.1);
        draw_circle(p.x, p.y, radius(pts[w].k), alpha(BLACK, 0.045));
    }
    if let Some(o) = skin_outline(skin, now) {
        let oc = tint(rgbt(o));
        tube(c * 0.07, vec2(0.0, 0.0), &|_| oc);
    }
    tube(0.0, vec2(0.0, 0.0), &|k| tint(rgbt(body_color(skin, k, n, now))));
    // a glossy stripe along the top of the body (a lighter shade, not see-through,
    // so it's even all the way along)
    let shine = vec2(-c * 0.07, -c * 0.11);
    for w in (1..pts.len()).rev() {
        let (a, b) = (pts[w - 1], pts[w]);
        let col = tint(mix(rgbt(body_color(skin, b.k, n, now)), WHITE, 0.12));
        let r = radius(b.k) * 0.24;
        let (pa, pb) = (at(a.pos) + shine, at(b.pos) + shine);
        if !b.gap {
            draw_line(pa.x, pa.y, pb.x, pb.y, r * 2.0, col);
        }
        draw_circle(pb.x, pb.y, r, col);
    }
    let _ = fade;
    // cheese holes
    if g.has("cheese") {
        for (i, seg) in s.body.iter().enumerate().skip(1) {
            if seg.hole {
                let p = at(vec2(seg.x as f32, seg.y as f32));
                draw_circle(p.x, p.y, width * 0.26, alpha(rgb(250, 230, 150), 0.95 * fade));
                draw_circle(p.x + width * 0.05, p.y + width * 0.05, width * 0.16, alpha(rgb(220, 190, 100), fade));
                let _ = i;
            }
        }
    }
    // head and eyes, facing the way it's going
    let head = at(pts[0].pos);
    let d = match s.planned.unwrap_or(s.dir) {
        Dir::Up => vec2(0.0, -1.0),
        Dir::Down => vec2(0.0, 1.0),
        Dir::Left => vec2(-1.0, 0.0),
        Dir::Right => vec2(1.0, 0.0),
    };
    let side = vec2(-d.y, d.x);
    draw_circle(head.x, head.y, width * 0.55, tint(rgbt(body_color(skin, 0.0, n, now))));
    for k in [-1.0, 1.0] {
        let e = head + d * c * 0.14 + side * c * 0.2 * k;
        draw_circle(e.x, e.y, c * 0.15, alpha(WHITE, fade));
        if s.dead {
            draw_line(e.x - c * 0.08, e.y - c * 0.08, e.x + c * 0.08, e.y + c * 0.08, c * 0.05, alpha(BLACK, fade));
            draw_line(e.x + c * 0.08, e.y - c * 0.08, e.x - c * 0.08, e.y + c * 0.08, c * 0.05, alpha(BLACK, fade));
        } else {
            let pupil = e + d * c * 0.05;
            draw_circle(pupil.x, pupil.y, c * 0.08, alpha(rgb(20, 24, 32), fade));
            draw_circle(pupil.x - c * 0.025, pupil.y - c * 0.03, c * 0.025, alpha(WHITE, fade));
        }
    }
    if s.shield > 0 {
        draw_circle_lines(head.x, head.y, c * 0.62, c * 0.06, alpha(rgb(80, 160, 255), 0.8 * fade));
    }
}

fn arrow(p: Vec2, s: f32, d: Dir, c: Color) {
    let (dx, dy) = d.delta();
    let f = vec2(dx as f32, dy as f32);
    let side = vec2(-f.y, f.x);
    draw_line(p.x - f.x * s, p.y - f.y * s, p.x + f.x * s * 0.3, p.y + f.y * s * 0.3, s * 0.4, c);
    draw_triangle(p + f * s, p + side * s * 0.75 - f * s * 0.05, p - side * s * 0.75 - f * s * 0.05, c);
}

pub fn key_icon(p: Vec2, s: f32, c: Color) {
    draw_circle(p.x - s * 0.4, p.y, s * 0.42, darken(c, 0.15));
    draw_circle(p.x - s * 0.4, p.y, s * 0.18, rgb(60, 40, 0));
    draw_rectangle(p.x - s * 0.05, p.y - s * 0.11, s * 1.05, s * 0.22, darken(c, 0.15));
    draw_rectangle(p.x + s * 0.55, p.y, s * 0.15, s * 0.35, darken(c, 0.15));
    draw_rectangle(p.x + s * 0.8, p.y, s * 0.15, s * 0.28, darken(c, 0.15));
    draw_circle(p.x - s * 0.52, p.y - s * 0.15, s * 0.08, alpha(WHITE, 0.6));
}

pub fn shield_icon(p: Vec2, s: f32, c: Color) {
    draw_triangle(vec2(p.x - s * 0.8, p.y - s * 0.6), vec2(p.x + s * 0.8, p.y - s * 0.6), vec2(p.x, p.y + s * 0.95), c);
    rounded(Rect::new(p.x - s * 0.8, p.y - s * 0.85, s * 1.6, s * 0.6), s * 0.2, c);
    draw_triangle(vec2(p.x - s * 0.45, p.y - s * 0.45), vec2(p.x, p.y - s * 0.45), vec2(p.x, p.y + s * 0.55), alpha(WHITE, 0.3));
}

fn poison_fruit(p: Vec2, s: f32, time: f64) {
    let wob = ((time * 5.0) as f32).sin() * 0.05;
    draw_circle(p.x, p.y + s * 0.1, s * (0.95 + wob), rgb(125, 47, 191));
    draw_circle(p.x - s * 0.3, p.y - s * 0.2, s * 0.25, alpha(WHITE, 0.25));
    for k in [-1.0, 1.0] {
        let e = p + vec2(s * 0.32 * k, 0.0);
        draw_line(e.x - s * 0.15, e.y - s * 0.15, e.x + s * 0.15, e.y + s * 0.15, s * 0.12, rgb(230, 255, 200));
        draw_line(e.x + s * 0.15, e.y - s * 0.15, e.x - s * 0.15, e.y + s * 0.15, s * 0.12, rgb(230, 255, 200));
    }
    draw_rectangle(p.x - s * 0.08, p.y - s * 1.05, s * 0.16, s * 0.3, rgb(90, 60, 30));
}

/// Fruit, each drawn its own way.
pub fn draw_fruit(kind: &str, p: Vec2, s: f32) {
    let col = fruit_color(kind);
    let leaf = rgb(76, 175, 55);
    let stem = rgb(107, 68, 35);
    draw_circle(p.x, p.y + s * 0.85, s * 0.6, alpha(BLACK, 0.12));
    match kind {
        "cherry" => {
            draw_line(p.x - s * 0.4, p.y + s * 0.2, p.x + s * 0.1, p.y - s * 0.8, s * 0.12, stem);
            draw_line(p.x + s * 0.45, p.y + s * 0.25, p.x + s * 0.1, p.y - s * 0.8, s * 0.12, stem);
            for (dx, dy) in [(-0.45, 0.4), (0.45, 0.45)] {
                draw_circle(p.x + s * dx, p.y + s * dy, s * 0.48, col);
                draw_circle(p.x + s * (dx - 0.15), p.y + s * (dy - 0.15), s * 0.13, alpha(WHITE, 0.5));
            }
            draw_ellipse(p.x + s * 0.35, p.y - s * 0.75, s * 0.3, s * 0.14, -20.0, leaf);
        }
        "grape" => {
            for (dx, dy) in [(-0.4, -0.35), (0.0, -0.4), (0.4, -0.35), (-0.2, 0.05), (0.2, 0.05), (0.0, 0.45)] {
                draw_circle(p.x + s * dx, p.y + s * dy, s * 0.34, col);
                draw_circle(p.x + s * (dx - 0.1), p.y + s * (dy - 0.1), s * 0.08, alpha(WHITE, 0.45));
            }
            draw_rectangle(p.x - s * 0.05, p.y - s * 0.95, s * 0.1, s * 0.3, stem);
            draw_ellipse(p.x + s * 0.25, p.y - s * 0.85, s * 0.28, s * 0.13, 0.0, leaf);
        }
        "strawberry" => {
            draw_triangle(vec2(p.x - s * 0.85, p.y - s * 0.35), vec2(p.x + s * 0.85, p.y - s * 0.35), vec2(p.x, p.y + s * 0.95), col);
            draw_circle(p.x - s * 0.42, p.y - s * 0.25, s * 0.45, col);
            draw_circle(p.x + s * 0.42, p.y - s * 0.25, s * 0.45, col);
            for (dx, dy) in [(-0.4, -0.15), (0.0, -0.1), (0.4, -0.15), (-0.2, 0.25), (0.2, 0.25), (0.0, 0.55)] {
                draw_ellipse(p.x + s * dx, p.y + s * dy, s * 0.05, s * 0.08, 0.0, rgb(255, 230, 120));
            }
            for k in -2..=2 {
                draw_ellipse(p.x + k as f32 * s * 0.22, p.y - s * 0.62, s * 0.22, s * 0.1, k as f32 * 25.0, leaf);
            }
        }
        "lemon" => {
            draw_ellipse(p.x, p.y, s * 0.95, s * 0.72, -15.0, col);
            draw_circle(p.x + s * 0.92, p.y - s * 0.25, s * 0.12, col);
            draw_circle(p.x - s * 0.92, p.y + s * 0.25, s * 0.12, col);
            draw_ellipse(p.x - s * 0.3, p.y - s * 0.25, s * 0.3, s * 0.15, -15.0, alpha(WHITE, 0.4));
        }
        "banana" => {
            for k in 0..9 {
                let a = (200.0 + k as f32 * 17.0).to_radians();
                let q = p + vec2(a.cos() * s * 0.7, a.sin() * s * 0.7 + s * 0.75);
                draw_circle(q.x, q.y, s * (0.24 + 0.08 * (1.0 - ((k as f32 - 4.0) / 4.0).abs())), col);
            }
            draw_circle(p.x - s * 0.7, p.y + s * 0.5, s * 0.1, stem);
            draw_circle(p.x + s * 0.68, p.y + s * 0.42, s * 0.08, stem);
        }
        "orange" => {
            draw_circle(p.x, p.y + s * 0.05, s * 0.92, col);
            draw_circle(p.x - s * 0.3, p.y - s * 0.25, s * 0.22, alpha(WHITE, 0.35));
            draw_circle(p.x, p.y - s * 0.8, s * 0.08, stem);
            draw_ellipse(p.x + s * 0.25, p.y - s * 0.85, s * 0.3, s * 0.12, -25.0, leaf);
            for (dx, dy) in [(0.3, 0.2), (-0.1, 0.45), (0.45, -0.2)] {
                draw_circle(p.x + s * dx, p.y + s * dy, s * 0.04, darken(col, 0.15));
            }
        }
        _ => {
            // apple
            draw_circle(p.x - s * 0.32, p.y + s * 0.05, s * 0.7, col);
            draw_circle(p.x + s * 0.32, p.y + s * 0.05, s * 0.7, col);
            draw_circle(p.x, p.y + s * 0.3, s * 0.62, col);
            draw_circle(p.x - s * 0.4, p.y - s * 0.2, s * 0.2, alpha(WHITE, 0.4));
            draw_line(p.x, p.y - s * 0.5, p.x + s * 0.12, p.y - s * 0.95, s * 0.14, stem);
            draw_ellipse(p.x + s * 0.4, p.y - s * 0.8, s * 0.32, s * 0.15, -25.0, leaf);
        }
    }
}

/// A skin's look on a little sample snake (for the store and collection).
pub fn skin_preview(skin: &Skin, r: Rect, now: f64) {
    let n = 9usize;
    let c = (r.w / 7.0).min(r.h / 3.0);
    let base = vec2(r.x + r.w / 2.0 - c * 3.0, r.y + r.h / 2.0);
    let mut pts = Vec::new();
    for i in 0..n {
        let t = i as f32;
        pts.push(vec2(6.0 - t * 0.75, (t * 0.75 + now as f32 / 500.0).sin() * 0.6));
    }
    let board = Rect::new(base.x - c * 0.5, base.y - c * 0.5, 0.0, 0.0);
    // (the sample curves smoothly, so draw it with cells joined however far apart)
    let pts2: Vec<Vec2> = pts.clone();
    let at = |p: Vec2| vec2(board.x + (p.x + 0.5) * c, board.y + (p.y + 0.5) * c);
    let width = c * 0.85;
    if let Some(o) = skin_outline(skin, now) {
        for i in 0..n {
            let p = at(pts2[i]);
            draw_circle(p.x, p.y, width / 2.0 + c * 0.08, rgbt(o));
            if i + 1 < n {
                let q = at(pts2[i + 1]);
                draw_line(p.x, p.y, q.x, q.y, width + c * 0.16, rgbt(o));
            }
        }
    }
    for i in (0..n).rev() {
        let p = at(pts2[i]);
        let col = rgbt(skin_color(skin, i, n, now));
        if i + 1 < n {
            let q = at(pts2[i + 1]);
            draw_line(p.x, p.y, q.x, q.y, width, col);
        }
        draw_circle(p.x, p.y, width / 2.0, col);
    }
    let head = at(pts2[0]);
    draw_circle(head.x, head.y, width * 0.56, rgbt(skin_color(skin, 0, n, now)));
    for k in [-1.0, 1.0] {
        let e = head + vec2(c * 0.14, c * 0.2 * k);
        draw_circle(e.x, e.y, c * 0.15, WHITE);
        draw_circle(e.x + c * 0.05, e.y, c * 0.08, rgb(20, 24, 32));
    }
}

/// A pack's colours as a little checkerboard.
pub fn pack_preview(swatch: &[Rgb], r: Rect) {
    let cols = 6;
    let rows = 3;
    let c = (r.w / cols as f32).min(r.h / rows as f32);
    let x0 = r.x + (r.w - c * cols as f32) / 2.0;
    let y0 = r.y + (r.h - c * rows as f32) / 2.0;
    let a = swatch.first().map(|c| rgbt(*c)).unwrap_or(GRAY);
    let b = swatch.get(1).map(|c| rgbt(*c)).unwrap_or(DARKGRAY);
    let accent = swatch.get(2).map(|c| rgbt(*c)).unwrap_or(WHITE);
    for y in 0..rows {
        for x in 0..cols {
            draw_rectangle(x0 + x as f32 * c, y0 + y as f32 * c, c, c, if (x + y) % 2 == 0 { a } else { b });
        }
    }
    draw_circle(x0 + c * 4.5, y0 + c * 1.5, c * 0.35, accent);
    star(x0 + c * 1.5, y0 + c * 1.5, c * 0.35, c * 0.15, mix(accent, WHITE, 0.3));
    let _ = lighten;
}
