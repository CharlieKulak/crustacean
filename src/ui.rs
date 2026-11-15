//! The look of the game: one theme, one font family, and immediate-mode widgets
//! (buttons, tabs, toggles, sliders, text boxes, scrolling lists, toasts, icons).
//!
//! Everything is laid out in "units" (`ui.u`): one unit is a pixel on a 1280 x 720
//! window, and it scales with the window so the game looks the same from a phone
//! to a 4K screen.

use macroquad::prelude::*;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// ---- theme

pub const BG_TOP: Color = rgb(16, 24, 34);
pub const BG_BOTTOM: Color = rgb(9, 14, 21);
pub const CARD: Color = rgb(27, 38, 52);
pub const CARD_HI: Color = rgb(36, 50, 68);
pub const LINE: Color = rgb(52, 68, 88);
pub const INK: Color = rgb(236, 242, 248);
pub const MUTED: Color = rgb(146, 162, 182);
pub const FAINT: Color = rgb(96, 112, 132);
pub const ACCENT: Color = rgb(74, 222, 128); // snake green
pub const ACCENT_DARK: Color = rgb(22, 163, 74);
pub const C_BLUE: Color = rgb(96, 165, 250);
pub const C_GOLD: Color = rgb(251, 191, 36);
pub const C_RED: Color = rgb(248, 113, 113);
pub const C_PURPLE: Color = rgb(192, 132, 252);

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}

pub fn rgbt(c: (u8, u8, u8)) -> Color {
    rgb(c.0, c.1, c.2)
}

pub fn alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, c.a * a)
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
}

pub fn lighten(c: Color, t: f32) -> Color {
    mix(c, WHITE, t)
}

pub fn darken(c: Color, t: f32) -> Color {
    mix(c, BLACK, t)
}

/// How bright a colour looks (to pick dark or light text on it).
pub fn luma(c: Color) -> f32 {
    0.299 * c.r + 0.587 * c.g + 0.114 * c.b
}

pub fn text_on(c: Color) -> Color {
    if luma(c) > 0.62 { rgb(18, 26, 36) } else { WHITE }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Style {
    Primary,
    Secondary,
    Ghost,
    Danger,
    Gold,
    Tinted(Color),
}

pub fn id_of<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

pub struct Toast {
    pub text: String,
    pub color: Color,
    pub until: f64,
    pub born: f64,
}

pub struct Ui {
    pub regular: Font,
    pub bold: Font,
    pub u: f32,
    pub w: f32,
    pub h: f32,
    pub mouse: Vec2,
    pub clicked: bool,
    pub down: bool,
    pub wheel: f32,
    pub chars: Vec<char>,
    pub keys: Vec<KeyCode>,
    pub time: f64,
    pub dt: f32,
    /// Off while a popup is open, for everything drawn under it.
    pub input: bool,
    pub focus: Option<u64>,
    anims: HashMap<u64, f32>,
    scrolls: HashMap<u64, f32>,
    drag: Option<(u64, f32, f32)>,
    clip_stack: Vec<Rect>,
    pub toasts: Vec<Toast>,
    pub hand: bool,
    pub caret_at: f64,
    warmed_u: f32,
    /// number boxes being typed in, and when their +/- buttons were pressed
    edits: HashMap<u64, String>,
    held: HashMap<u64, f64>,
}

impl Ui {
    pub fn new(regular: Font, bold: Font) -> Ui {
        warm_fonts(&[&regular, &bold], 1.0);
        Ui {
            regular,
            bold,
            u: 1.0,
            w: 1280.0,
            h: 720.0,
            mouse: vec2(0.0, 0.0),
            clicked: false,
            down: false,
            wheel: 0.0,
            chars: Vec::new(),
            keys: Vec::new(),
            time: 0.0,
            dt: 0.0,
            input: true,
            focus: None,
            anims: HashMap::new(),
            scrolls: HashMap::new(),
            drag: None,
            clip_stack: Vec::new(),
            toasts: Vec::new(),
            hand: false,
            caret_at: 0.0,
            warmed_u: 1.0,
            edits: HashMap::new(),
            held: HashMap::new(),
        }
    }

    /// Read this frame's input. Call once at the start of every frame.
    pub fn begin(&mut self) {
        self.w = screen_width();
        self.h = screen_height();
        self.u = (self.w / 1280.0).min(self.h / 720.0).max(0.45);
        if (self.u - self.warmed_u).abs() > 0.001 {
            // (new text sizes after a resize: cache them before anything is drawn)
            warm_fonts(&[&self.regular, &self.bold], self.u);
            self.warmed_u = self.u;
        }
        let (mx, my) = mouse_position();
        self.mouse = vec2(mx, my);
        self.clicked = is_mouse_button_pressed(MouseButton::Left);
        self.down = is_mouse_button_down(MouseButton::Left);
        let (_wx, wy) = mouse_wheel();
        self.wheel = if wy.abs() > 0.0 { wy.signum() * (wy.abs().min(3.0)).max(1.0) } else { 0.0 };
        self.chars.clear();
        while let Some(c) = get_char_pressed() {
            if !c.is_control() {
                self.chars.push(c);
            }
        }
        self.keys = get_keys_pressed().into_iter().collect();
        self.dt = get_frame_time().min(0.1);
        self.time = get_time();
        self.input = true;
        self.hand = false;
        if !self.down {
            self.drag = None;
        }
    }

    pub fn end(&mut self) {
        let now = self.time;
        self.toasts.retain(|t| t.until > now);
        self.draw_toasts();
    }

    pub fn key(&self, k: KeyCode) -> bool {
        self.keys.contains(&k)
    }

    /// A key press that isn't going into a text box.
    pub fn hotkey(&self, k: KeyCode) -> bool {
        self.focus.is_none() && self.key(k)
    }

    pub fn toast(&mut self, text: impl Into<String>, color: Color) {
        let now = self.time;
        self.toasts.push(Toast { text: text.into(), color, until: now + 4.0, born: now });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    /// An animated value that eases towards `target` (0..1) for widget `id`.
    pub fn anim(&mut self, id: u64, target: f32, speed: f32) -> f32 {
        let dt = self.dt;
        let v = self.anims.entry(id).or_insert(target);
        *v += (target - *v) * (1.0 - (-speed * dt).exp());
        *v
    }

    pub fn hover(&self, r: Rect) -> bool {
        self.input && r.contains(self.mouse) && self.clip_contains(self.mouse)
    }

    fn clip_contains(&self, p: Vec2) -> bool {
        self.clip_stack.last().is_none_or(|c| c.contains(p))
    }

    pub fn pressed(&mut self, r: Rect) -> bool {
        let hit = self.hover(r) && self.clicked;
        if hit {
            self.clicked = false; // (one click, one widget)
        }
        hit
    }

    // ---- text

    pub fn font(&self, bold: bool) -> &Font {
        if bold { &self.bold } else { &self.regular }
    }

    pub fn measure(&self, s: &str, size: f32, bold: bool) -> f32 {
        let px = (size * self.u).round().max(6.0) as u16;
        measure_text(s, Some(self.font(bold)), px, 1.0).width
    }

    /// Draws text with its vertical middle at `y`. Sizes are in units; returns the width in pixels.
    pub fn text(&self, s: &str, x: f32, y: f32, size: f32, color: Color, align: Align, bold: bool) -> f32 {
        let px = (size * self.u).round().max(6.0) as u16;
        let font = self.font(bold);
        let dims = measure_text(s, Some(font), px, 1.0);
        let x = match align {
            Align::Left => x,
            Align::Center => x - dims.width / 2.0,
            Align::Right => x - dims.width,
        };
        let base = y + px as f32 * 0.36;
        draw_text_ex(s, x.round(), base.round(), TextParams { font: Some(font), font_size: px, color, ..Default::default() });
        dims.width
    }

    /// Text cut short with "..." to fit `max` pixels.
    pub fn fit(&self, s: &str, size: f32, bold: bool, max: f32) -> String {
        if self.measure(s, size, bold) <= max {
            return s.to_string();
        }
        let mut chars: Vec<char> = s.chars().collect();
        while !chars.is_empty() {
            chars.pop();
            let t: String = chars.iter().collect::<String>() + "...";
            if self.measure(&t, size, bold) <= max {
                return t;
            }
        }
        String::new()
    }

    /// Word-wrapped text; returns the height used in pixels.
    pub fn wrapped(&self, s: &str, x: f32, y: f32, width: f32, size: f32, color: Color, bold: bool) -> f32 {
        let line_h = size * self.u * 1.35;
        let mut line = String::new();
        let mut yy = y + line_h / 2.0;
        for para in s.split('\n') {
            for word in para.split_whitespace() {
                let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
                if self.measure(&candidate, size, bold) > width && !line.is_empty() {
                    self.text(&line, x, yy, size, color, Align::Left, bold);
                    yy += line_h;
                    line = word.to_string();
                } else {
                    line = candidate;
                }
            }
            self.text(&line, x, yy, size, color, Align::Left, bold);
            yy += line_h;
            line.clear();
        }
        yy - y - line_h / 2.0
    }

    // ---- shapes

    pub fn background(&self) {
        // a soft vertical gradient with a faint glow behind the middle
        let steps = 24;
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let c = mix(BG_TOP, BG_BOTTOM, t);
            draw_rectangle(0.0, self.h * t, self.w, self.h / steps as f32 + 1.0, c);
        }
        // a soft green glow behind the top (many faint circles, so there are no bands)
        let big = self.w.max(self.h) * 0.75;
        for k in 0..40 {
            let r = big * (1.0 - k as f32 / 40.0);
            draw_circle(self.w * 0.5, self.h * 0.12, r, Color::new(0.13, 0.77, 0.37, 0.0035));
        }
    }

    pub fn shadow(&self, r: Rect, radius: f32, strength: f32) {
        for k in 1..=4 {
            let g = k as f32 * 2.5 * self.u;
            rounded(Rect::new(r.x - g * 0.5, r.y - g * 0.2 + 3.0 * self.u, r.w + g, r.h + g), radius + g,
                    Color::new(0.0, 0.0, 0.0, 0.07 * strength));
        }
    }

    pub fn card(&self, r: Rect) {
        self.shadow(r, 16.0 * self.u, 1.0);
        rounded(r, 16.0 * self.u, CARD);
        rounded_outline(r, 16.0 * self.u, 1.0, alpha(LINE, 0.7));
    }

    pub fn card_colored(&self, r: Rect, c: Color) {
        self.shadow(r, 14.0 * self.u, 0.8);
        rounded(r, 14.0 * self.u, c);
    }

    // ---- widgets

    pub fn button(&mut self, label: &str, r: Rect, style: Style) -> bool {
        self.button_icon(label, "", r, style)
    }

    pub fn button_icon(&mut self, label: &str, icon: &str, r: Rect, style: Style) -> bool {
        let id = id_of(&(label, icon, (r.x as i32, r.y as i32)));
        let hover = self.hover(r);
        if hover {
            self.hand = true;
        }
        let h = self.anim(id, if hover { 1.0 } else { 0.0 }, 18.0);
        let press = if hover && self.down { 1.0 } else { 0.0 };
        let p = self.anim(id ^ 1, press, 30.0);
        let grow = (h * 2.0 - p * 3.0) * self.u;
        let rr = Rect::new(r.x - grow, r.y - grow - h * 1.5 * self.u, r.w + grow * 2.0, r.h + grow * 2.0);
        let (bg, fg) = match style {
            Style::Primary => (mix(ACCENT_DARK, ACCENT, 0.35 + h * 0.3), rgb(8, 30, 16)),
            Style::Secondary => (mix(CARD_HI, lighten(CARD_HI, 0.12), h), INK),
            Style::Ghost => (alpha(CARD_HI, h * 0.9), if h > 0.5 { INK } else { MUTED }),
            Style::Danger => (mix(rgb(185, 50, 50), C_RED, h * 0.5), WHITE),
            Style::Gold => (mix(rgb(217, 145, 10), C_GOLD, 0.3 + h * 0.4), rgb(40, 25, 0)),
            Style::Tinted(c) => (mix(c, lighten(c, 0.2), h), text_on(c)),
        };
        let radius = (rr.h * 0.5).min(14.0 * self.u);
        if style != Style::Ghost {
            self.shadow(rr, radius, 0.6 + h * 0.6);
        }
        rounded(rr, radius, bg);
        if style == Style::Primary || style == Style::Gold {
            // a little shine on top
            rounded(Rect::new(rr.x + 2.0, rr.y + 2.0, rr.w - 4.0, rr.h * 0.45), radius * 0.8, alpha(WHITE, 0.12));
        }
        let size = (r.h / self.u * 0.38).clamp(11.0, 26.0);
        let cy = rr.y + rr.h / 2.0;
        if icon.is_empty() {
            let t = self.fit(label, size, true, rr.w - 16.0 * self.u);
            self.text(&t, rr.x + rr.w / 2.0, cy, size, fg, Align::Center, true);
        } else if label.is_empty() {
            draw_icon(icon, rr.x + rr.w / 2.0, cy, rr.h * 0.5, fg);
        } else {
            let tw = self.measure(label, size, true);
            let isz = size * self.u * 1.25;
            let total = tw + isz + 8.0 * self.u;
            let x0 = rr.x + (rr.w - total) / 2.0;
            draw_icon(icon, x0 + isz / 2.0, cy, isz, fg);
            self.text(label, x0 + isz + 8.0 * self.u, cy, size, fg, Align::Left, true);
        }
        self.pressed(r)
    }

    /// A round icon button (like the menu's corners).
    pub fn round_button(&mut self, icon: &str, c: Vec2, radius: f32, color: Color) -> bool {
        let r = Rect::new(c.x - radius, c.y - radius, radius * 2.0, radius * 2.0);
        let id = id_of(&("round", icon, c.x as i32, c.y as i32));
        let hover = self.hover(r) && (self.mouse - c).length() <= radius;
        if hover {
            self.hand = true;
        }
        let h = self.anim(id, if hover { 1.0 } else { 0.0 }, 16.0);
        let rad = radius * (1.0 + 0.06 * h);
        for k in 1..=3 {
            draw_circle(c.x, c.y + 3.0 * self.u, rad + k as f32 * 2.0 * self.u, Color::new(0.0, 0.0, 0.0, 0.08));
        }
        draw_circle(c.x, c.y, rad, mix(color, lighten(color, 0.25), h));
        draw_circle_lines(c.x, c.y, rad, 1.5, alpha(WHITE, 0.15 + 0.2 * h));
        draw_icon(icon, c.x, c.y, rad * 1.0, text_on(color));
        hover && self.pressed(r)
    }

    /// Tabs; returns the one clicked.
    pub fn tabs(&mut self, labels: &[&str], selected: usize, r: Rect) -> Option<usize> {
        rounded(r, r.h / 2.0, alpha(BLACK, 0.25));
        let n = labels.len().max(1);
        let w = r.w / n as f32;
        let id = id_of(&("tabs", labels.join("|")));
        let x = self.anim(id, selected as f32, 16.0);
        let pill = Rect::new(r.x + x * w + 3.0, r.y + 3.0, w - 6.0, r.h - 6.0);
        rounded(pill, pill.h / 2.0, CARD_HI);
        rounded_outline(pill, pill.h / 2.0, 1.0, alpha(ACCENT, 0.5));
        let mut out = None;
        let size = (r.h / self.u * 0.4).clamp(11.0, 20.0);
        for (i, l) in labels.iter().enumerate() {
            let cell = Rect::new(r.x + i as f32 * w, r.y, w, r.h);
            let hover = self.hover(cell);
            if hover {
                self.hand = true;
            }
            let col = if i == selected { INK } else if hover { lighten(MUTED, 0.3) } else { MUTED };
            let t = self.fit(l, size, true, w - 10.0 * self.u);
            self.text(&t, cell.x + w / 2.0, cell.y + r.h / 2.0, size, col, Align::Center, true);
            if self.pressed(cell) {
                out = Some(i);
            }
        }
        out
    }

    pub fn toggle(&mut self, label: &str, on: bool, r: Rect) -> bool {
        let id = id_of(&("toggle", label, r.x as i32, r.y as i32));
        let hover = self.hover(r);
        if hover {
            self.hand = true;
        }
        let t = self.anim(id, if on { 1.0 } else { 0.0 }, 18.0);
        let sw = Rect::new(r.x + r.w - r.h * 1.7, r.y + r.h * 0.18, r.h * 1.6, r.h * 0.64);
        rounded(sw, sw.h / 2.0, mix(rgb(60, 74, 92), ACCENT_DARK, t));
        let kx = sw.x + sw.h / 2.0 + (sw.w - sw.h) * t;
        draw_circle(kx, sw.y + sw.h / 2.0, sw.h * 0.4, WHITE);
        let size = (r.h / self.u * 0.42).clamp(11.0, 20.0);
        self.text(label, r.x, r.y + r.h / 2.0, size, if hover { WHITE } else { INK }, Align::Left, false);
        self.pressed(r)
    }

    /// A chip that can be on or off (for modes and filters).
    pub fn chip(&mut self, label: &str, on: bool, r: Rect, color: Color) -> bool {
        let id = id_of(&("chip", label, r.x as i32, r.y as i32));
        let hover = self.hover(r);
        if hover {
            self.hand = true;
        }
        let t = self.anim(id, if on { 1.0 } else { 0.0 }, 18.0);
        let h = self.anim(id ^ 7, if hover { 1.0 } else { 0.0 }, 18.0);
        rounded(r, r.h / 2.0, mix(mix(CARD_HI, lighten(CARD_HI, 0.1), h), color, t * 0.85));
        if t < 0.5 {
            rounded_outline(r, r.h / 2.0, 1.0, alpha(LINE, 1.0));
        }
        let size = (r.h / self.u * 0.42).clamp(10.0, 18.0);
        let fg = mix(INK, text_on(color), t);
        let tt = self.fit(label, size, true, r.w - 12.0 * self.u);
        self.text(&tt, r.x + r.w / 2.0, r.y + r.h / 2.0, size, fg, Align::Center, true);
        self.pressed(r)
    }

    /// A slider from 0 to 1.
    pub fn slider(&mut self, id_label: &str, value: f32, r: Rect) -> f32 {
        let id = id_of(&("slider", id_label));
        let track = Rect::new(r.x, r.y + r.h / 2.0 - 3.0 * self.u, r.w, 6.0 * self.u);
        rounded(track, track.h / 2.0, rgb(52, 66, 84));
        rounded(Rect::new(track.x, track.y, track.w * value, track.h), track.h / 2.0, ACCENT);
        let kx = r.x + r.w * value;
        let hover = self.hover(Rect::new(r.x - 10.0, r.y, r.w + 20.0, r.h));
        if hover && self.clicked {
            self.drag = Some((id, 0.0, 0.0));
            self.clicked = false;
        }
        let dragging = matches!(self.drag, Some((d, _, _)) if d == id);
        if hover || dragging {
            self.hand = true;
        }
        let grow = self.anim(id, if hover || dragging { 1.0 } else { 0.0 }, 18.0);
        draw_circle(kx, r.y + r.h / 2.0, (9.0 + 3.0 * grow) * self.u, WHITE);
        draw_circle(kx, r.y + r.h / 2.0, (5.0 + 1.0 * grow) * self.u, ACCENT_DARK);
        if dragging {
            ((self.mouse.x - r.x) / r.w).clamp(0.0, 1.0)
        } else {
            value
        }
    }

    /// A one-line text box. Returns true when Enter is pressed in it.
    pub fn text_input(&mut self, id_label: &str, value: &mut String, r: Rect, placeholder: &str, secret: bool,
                      max_len: usize) -> bool {
        let id = id_of(&("input", id_label));
        let focused = self.focus == Some(id);
        let hover = self.hover(r);
        if hover {
            self.hand = true;
        }
        if self.input && self.clicked {
            if hover {
                self.focus = Some(id);
                self.caret_at = self.time;
                self.clicked = false;
            } else if focused {
                self.focus = None;
            }
        }
        let focused = self.focus == Some(id);
        let f = self.anim(id, if focused { 1.0 } else { 0.0 }, 18.0);
        rounded(r, 10.0 * self.u, mix(rgb(14, 21, 30), rgb(18, 27, 38), f));
        rounded_outline(r, 10.0 * self.u, 1.0 + f, mix(LINE, ACCENT, f));
        let mut entered = false;
        if focused && self.input {
            for c in self.chars.clone() {
                if value.chars().count() < max_len {
                    value.push(c);
                    self.caret_at = self.time;
                }
            }
            if self.key(KeyCode::Backspace) {
                value.pop();
                self.caret_at = self.time;
            }
            if is_key_down(KeyCode::Backspace) && self.time - self.caret_at > 0.45 {
                value.pop(); // (held down: keeps deleting)
            }
            if self.key(KeyCode::Enter) || self.key(KeyCode::KpEnter) {
                entered = true;
            }
            if self.key(KeyCode::Escape) || self.key(KeyCode::Tab) {
                self.focus = None;
            }
        }
        let size = (r.h / self.u * 0.42).clamp(11.0, 20.0);
        let shown: String = if secret { "•".repeat(value.chars().count()) } else { value.clone() };
        let pad = 12.0 * self.u;
        let cy = r.y + r.h / 2.0;
        if shown.is_empty() && !focused {
            self.text(placeholder, r.x + pad, cy, size, FAINT, Align::Left, false);
        } else {
            // show the end of long text
            let mut start = 0;
            let chars: Vec<char> = shown.chars().collect();
            while start < chars.len()
                && self.measure(&chars[start..].iter().collect::<String>(), size, false) > r.w - pad * 2.5
            {
                start += 1;
            }
            let vis: String = chars[start..].iter().collect();
            let w = self.text(&vis, r.x + pad, cy, size, INK, Align::Left, false);
            if focused && ((self.time - self.caret_at) * 2.0) as i64 % 2 == 0 {
                draw_rectangle(r.x + pad + w + 2.0, cy - size * self.u * 0.55, 2.0, size * self.u * 1.1, ACCENT);
            }
        }
        entered
    }

    /// A number box: - and + buttons (hold to keep going) around the number, which can
    /// also be clicked and typed. Returns the new value, kept between min and max.
    pub fn stepper(&mut self, key: &str, value: i32, min: i32, max: i32, r: Rect) -> i32 {
        let id = id_of(&("stepper", key));
        let u = self.u;
        let bw = r.h;
        rounded(r, 10.0 * u, rgb(14, 21, 30));
        rounded_outline(r, 10.0 * u, 1.0, LINE);
        let mut v = value;
        // the buttons
        for (k, (label, delta)) in [("-", -1), ("+", 1)].into_iter().enumerate() {
            let br = Rect::new(if k == 0 { r.x } else { r.x + r.w - bw }, r.y, bw, r.h);
            let bid = id ^ (k as u64 + 1) * 7919;
            let hover = self.hover(br);
            let at_end = (delta < 0 && v <= min) || (delta > 0 && v >= max);
            if hover && !at_end {
                self.hand = true;
            }
            let h = self.anim(bid, if hover && !at_end { 1.0 } else { 0.0 }, 18.0);
            let inner = Rect::new(br.x + 4.0 * u, br.y + 4.0 * u, br.w - 8.0 * u, br.h - 8.0 * u);
            rounded(inner, 8.0 * u, mix(CARD_HI, lighten(CARD_HI, 0.15), h));
            let col = if at_end { FAINT } else { INK };
            let cx = inner.x + inner.w / 2.0;
            let cy = inner.y + inner.h / 2.0;
            let arm = inner.h * 0.22;
            draw_line(cx - arm, cy, cx + arm, cy, 2.5 * u, col);
            if label == "+" {
                draw_line(cx, cy - arm, cx, cy + arm, 2.5 * u, col);
            }
            // a click steps once; holding it down keeps stepping, faster after a while
            if self.input && hover && self.clicked && !at_end {
                self.clicked = false;
                v += delta;
                self.held.insert(bid, self.time);
            } else if let Some(&since) = self.held.get(&bid) {
                if self.down && hover && !at_end {
                    let t = self.time - since;
                    if t > 0.4 {
                        let rate = if t > 2.0 { 0.02 } else { 0.06 };
                        let steps = ((t - 0.4) / rate) as i64 - ((t - 0.4 - self.dt as f64) / rate).max(0.0) as i64;
                        // (the longer it's held, the bigger the jumps, for big numbers)
                        let jump = if t > 7.0 { 500 } else if t > 5.0 { 50 } else if t > 3.5 { 5 } else { 1 };
                        v = v.saturating_add(delta * steps.max(0) as i32 * jump);
                    }
                } else {
                    self.held.remove(&bid);
                }
            }
        }
        // the number: click it to type one
        let mid = Rect::new(r.x + bw, r.y, r.w - 2.0 * bw, r.h);
        let size = (r.h / u * 0.44).clamp(12.0, 22.0);
        let typing = self.focus == Some(id);
        if self.input && self.clicked && self.hover(mid) && !typing {
            self.clicked = false;
            self.focus = Some(id);
            self.edits.insert(id, String::new());
            self.caret_at = self.time;
        }
        if self.focus == Some(id) {
            let clicked_away = self.input && self.clicked && !self.hover(mid);
            let buf = self.edits.entry(id).or_default();
            for c in self.chars.clone() {
                if c.is_ascii_digit() && buf.len() < 7 {
                    buf.push(c);
                }
            }
            if self.keys.contains(&KeyCode::Backspace) {
                buf.pop();
            }
            let done = self.keys.contains(&KeyCode::Enter) || self.keys.contains(&KeyCode::KpEnter)
                || self.keys.contains(&KeyCode::Tab) || self.keys.contains(&KeyCode::Escape)
                || clicked_away;
            let shown = buf.clone();
            rounded_outline(r, 10.0 * u, 2.0, ACCENT);
            let w = self.text(if shown.is_empty() { "" } else { &shown }, mid.x + mid.w / 2.0, mid.y + mid.h / 2.0, size, INK, Align::Center, true);
            if ((self.time - self.caret_at) * 2.0) as i64 % 2 == 0 {
                let cx = mid.x + mid.w / 2.0 + w / 2.0 + 2.0;
                draw_rectangle(cx, mid.y + mid.h * 0.25, 2.0, mid.h * 0.5, ACCENT);
            }
            if done {
                if let Ok(n) = shown.parse::<i32>() {
                    if !self.keys.contains(&KeyCode::Escape) {
                        v = n;
                    }
                }
                self.focus = None;
                self.edits.remove(&id);
            }
        } else {
            let hover = self.hover(mid);
            if hover {
                self.hand = true;
            }
            self.text(&value.to_string(), mid.x + mid.w / 2.0, mid.y + mid.h / 2.0, size, if hover { WHITE } else { INK }, Align::Center, true);
        }
        v.clamp(min, max)
    }

    // ---- scrolling

    /// Starts a clipped, scrollable area. Returns the scroll offset (pixels) to subtract
    /// from content positions. Finish with `end_scroll`.
    pub fn begin_scroll(&mut self, key: &str, r: Rect, content_h: f32) -> f32 {
        let id = id_of(&("scroll", key));
        let max = (content_h - r.h).max(0.0);
        let hover = self.hover(r);
        let mut target = *self.scrolls.get(&id).unwrap_or(&0.0);
        if hover && self.wheel != 0.0 {
            target -= self.wheel * 60.0 * self.u;
        }
        // drag to scroll (touch screens)
        if hover && self.clicked && self.drag.is_none() {
            self.drag = Some((id, self.mouse.y, target));
        }
        if let Some((d, y0, t0)) = self.drag {
            if d == id && self.down {
                let moved = self.mouse.y - y0;
                if moved.abs() > 6.0 * self.u {
                    target = t0 - moved;
                }
            }
        }
        target = target.clamp(0.0, max);
        self.scrolls.insert(id, target);
        let shown = self.anim(id ^ 99, target, 20.0);
        self.push_clip(r);
        if max > 0.0 {
            // scroll bar
            let bar_h = (r.h * r.h / content_h).max(30.0 * self.u);
            let y = r.y + (r.h - bar_h) * (shown / max);
            rounded(Rect::new(r.x + r.w - 5.0 * self.u, y, 4.0 * self.u, bar_h), 2.0 * self.u, alpha(WHITE, 0.18));
        }
        shown
    }

    pub fn end_scroll(&mut self) {
        self.pop_clip();
    }

    /// Scroll an area all the way down (it's kept in range next time it's drawn).
    pub fn scroll_to_end(&mut self, key: &str) {
        let id = id_of(&("scroll", key));
        self.scrolls.insert(id, f32::MAX / 4.0);
    }

    pub fn reset_scroll(&mut self, key: &str) {
        let id = id_of(&("scroll", key));
        self.scrolls.remove(&id);
        self.anims.remove(&(id ^ 99));
    }

    pub fn push_clip(&mut self, r: Rect) {
        let r = match self.clip_stack.last() {
            Some(c) => intersect(*c, r),
            None => r,
        };
        self.clip_stack.push(r);
        set_clip(Some(r));
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
        set_clip(self.clip_stack.last().copied());
    }

    /// Is a row on screen (inside the current clip)? Saves drawing rows nobody can see.
    pub fn visible(&self, r: Rect) -> bool {
        self.clip_stack.last().is_none_or(|c| c.overlaps(&r))
    }

    fn draw_toasts(&mut self) {
        let mut y = self.h - 24.0 * self.u;
        let now = self.time;
        for t in self.toasts.iter().rev() {
            let age = (now - t.born) as f32;
            let left = (t.until - now) as f32;
            let a = (age * 6.0).min(1.0).min(left * 3.0).clamp(0.0, 1.0);
            let size = 16.0;
            let w = self.measure(&t.text, size, true) + 48.0 * self.u;
            let h = 42.0 * self.u;
            let slide = (1.0 - (age * 6.0).min(1.0)) * 20.0 * self.u;
            let r = Rect::new(self.w / 2.0 - w / 2.0, y - h + slide, w, h);
            self.shadow(r, h / 2.0, a);
            rounded(r, h / 2.0, alpha(rgb(22, 31, 43), 0.97 * a));
            rounded_outline(r, h / 2.0, 1.5, alpha(t.color, a));
            draw_circle(r.x + 20.0 * self.u, r.y + h / 2.0, 5.0 * self.u, alpha(t.color, a));
            self.text(&t.text, r.x + 32.0 * self.u, r.y + h / 2.0, size, alpha(INK, a), Align::Left, true);
            y -= h + 8.0 * self.u;
        }
    }
}

/// Put every glyph the UI uses into the fonts' texture atlases up front.
///
/// macroquad moves glyphs around when an atlas fills up and has to grow, which
/// scrambles text drawn earlier in that frame (and, it turns out, later frames
/// too). Caching everything at start-up means it grows before anything is drawn.
pub fn warm_fonts(fonts: &[&Font], u: f32) {
    let chars: Vec<char> = (32u8..127).map(|c| c as char).chain("•·–—…×✓".chars()).collect();
    let mut sizes: Vec<u16> = Vec::new();
    for s in [9.5f32, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0, 20.0, 22.0, 24.0, 26.0, 28.0, 30.0, 34.0, 36.0, 40.0] {
        let px = ((s * u).round().max(6.0) * screen_dpi_scale()).ceil() as u16;
        if !sizes.contains(&px) {
            sizes.push(px);
        }
    }
    let t0 = get_time();
    for f in fonts {
        for &px in &sizes {
            f.populate_font_cache(&chars, px);
        }
    }
    if std::env::var("OPENSNAKE_DEBUG").is_ok() {
        eprintln!("warmed fonts in {:.0} ms", (get_time() - t0) * 1000.0);
    }
}

fn intersect(a: Rect, b: Rect) -> Rect {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let r = (a.x + a.w).min(b.x + b.w);
    let bt = (a.y + a.h).min(b.y + b.h);
    Rect::new(x, y, (r - x).max(0.0), (bt - y).max(0.0))
}

/// Only draw inside `r` (None: everywhere). Flushes what's been drawn so far.
pub fn set_clip(r: Option<Rect>) {
    let mut gl = unsafe { get_internal_gl() };
    gl.flush();
    match r {
        // (in physical pixels, from the top left)
        Some(r) => {
            let dpi = screen_dpi_scale();
            gl.quad_gl.scissor(Some((
                (r.x * dpi) as i32,
                (r.y * dpi) as i32,
                (r.w * dpi).max(0.0) as i32,
                (r.h * dpi).max(0.0) as i32,
            )));
        }
        None => gl.quad_gl.scissor(None),
    }
}

// ---- drawing helpers

pub fn rounded(r: Rect, radius: f32, c: Color) {
    let rad = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    if rad < 0.5 {
        draw_rectangle(r.x, r.y, r.w, r.h, c);
        return;
    }
    if c.a >= 0.999 {
        draw_rectangle(r.x + rad, r.y, r.w - 2.0 * rad, r.h, c);
        draw_rectangle(r.x, r.y + rad, rad, r.h - 2.0 * rad, c);
        draw_rectangle(r.x + r.w - rad, r.y + rad, rad, r.h - 2.0 * rad, c);
        for (cx, cy) in [(r.x + rad, r.y + rad), (r.x + r.w - rad, r.y + rad), (r.x + rad, r.y + r.h - rad),
                         (r.x + r.w - rad, r.y + r.h - rad)] {
            draw_circle(cx, cy, rad, c);
        }
        return;
    }
    // Translucent: one triangle fan, so nothing is drawn twice.
    let pts = rounded_points(r, rad);
    let center = vec2(r.x + r.w / 2.0, r.y + r.h / 2.0);
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        draw_triangle(center, a, b, c);
    }
}

fn rounded_points(r: Rect, rad: f32) -> Vec<Vec2> {
    let seg = ((rad / 2.0) as usize).clamp(4, 12);
    let mut pts = Vec::with_capacity(seg * 4 + 4);
    let corners = [
        (r.x + r.w - rad, r.y + r.h - rad, 0.0f32),
        (r.x + rad, r.y + r.h - rad, 90.0),
        (r.x + rad, r.y + rad, 180.0),
        (r.x + r.w - rad, r.y + rad, 270.0),
    ];
    for (cx, cy, start) in corners {
        for k in 0..=seg {
            let a = (start + 90.0 * k as f32 / seg as f32).to_radians();
            pts.push(vec2(cx + rad * a.cos(), cy + rad * a.sin()));
        }
    }
    pts
}

pub fn rounded_outline(r: Rect, radius: f32, thickness: f32, c: Color) {
    let rad = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    let pts = rounded_points(r, rad);
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        draw_line(a.x, a.y, b.x, b.y, thickness, c);
    }
}

pub fn progress(r: Rect, frac: f32, c: Color) {
    rounded(r, r.h / 2.0, rgb(44, 58, 76));
    let w = (r.w * frac.clamp(0.0, 1.0)).max(if frac > 0.0 { r.h } else { 0.0 });
    if w > 0.0 {
        rounded(Rect::new(r.x, r.y, w, r.h), r.h / 2.0, c);
        rounded(Rect::new(r.x + 2.0, r.y + 1.0, (w - 4.0).max(0.0), r.h * 0.4), r.h * 0.2, alpha(WHITE, 0.2));
    }
}

/// Simple vector icons, drawn with shapes so they're sharp at any size.
pub fn draw_icon(name: &str, x: f32, y: f32, size: f32, c: Color) {
    let s = size / 2.0;
    let t = (size * 0.1).max(1.5);
    match name {
        "play" => draw_triangle(vec2(x - s * 0.45, y - s * 0.6), vec2(x - s * 0.45, y + s * 0.6), vec2(x + s * 0.6, y), c),
        "trophy" => {
            rounded(Rect::new(x - s * 0.55, y - s * 0.75, s * 1.1, s * 0.8), s * 0.35, c);
            draw_rectangle(x - s * 0.12, y, s * 0.24, s * 0.45, c);
            rounded(Rect::new(x - s * 0.45, y + s * 0.42, s * 0.9, s * 0.28), s * 0.1, c);
            draw_circle_lines(x - s * 0.62, y - s * 0.38, s * 0.25, t * 0.8, c);
            draw_circle_lines(x + s * 0.62, y - s * 0.38, s * 0.25, t * 0.8, c);
        }
        "user" => {
            draw_circle(x, y - s * 0.32, s * 0.36, c);
            rounded(Rect::new(x - s * 0.7, y + s * 0.15, s * 1.4, s * 0.65), s * 0.32, c);
        }
        "users" => {
            draw_circle(x - s * 0.32, y - s * 0.3, s * 0.3, c);
            draw_circle(x + s * 0.38, y - s * 0.22, s * 0.26, alpha(c, 0.8));
            rounded(Rect::new(x - s * 0.85, y + s * 0.1, s * 1.1, s * 0.6), s * 0.28, c);
            rounded(Rect::new(x + s * 0.05, y + s * 0.15, s * 0.8, s * 0.5), s * 0.24, alpha(c, 0.8));
        }
        "cart" | "store" => {
            draw_line(x - s * 0.8, y - s * 0.6, x - s * 0.55, y - s * 0.6, t, c);
            draw_triangle(vec2(x - s * 0.55, y - s * 0.45), vec2(x + s * 0.75, y - s * 0.45), vec2(x + s * 0.5, y + s * 0.2), c);
            draw_triangle(vec2(x - s * 0.55, y - s * 0.45), vec2(x - s * 0.35, y + s * 0.2), vec2(x + s * 0.5, y + s * 0.2), c);
            draw_circle(x - s * 0.3, y + s * 0.55, s * 0.15, c);
            draw_circle(x + s * 0.4, y + s * 0.55, s * 0.15, c);
        }
        "gear" => {
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::PI / 4.0;
                draw_line(x, y, x + a.cos() * s * 0.85, y + a.sin() * s * 0.85, s * 0.28, c);
            }
            draw_circle(x, y, s * 0.6, c);
            draw_circle(x, y, s * 0.25, BLACK.with_alpha(0.0));
            draw_circle_lines(x, y, s * 0.22, t, darken(c, 0.6));
        }
        "bolt" => {
            draw_triangle(vec2(x + s * 0.2, y - s * 0.85), vec2(x - s * 0.5, y + s * 0.1), vec2(x + s * 0.05, y + s * 0.05), c);
            draw_triangle(vec2(x - s * 0.15, y - s * 0.05), vec2(x + s * 0.5, y - s * 0.1), vec2(x - s * 0.2, y + s * 0.85), c);
        }
        "globe" => {
            draw_circle_lines(x, y, s * 0.78, t, c);
            draw_line(x - s * 0.78, y, x + s * 0.78, y, t * 0.8, c);
            draw_ellipse_lines(x, y, s * 0.35, s * 0.78, 0.0, t * 0.8, c);
        }
        "star" => star(x, y, s * 0.9, s * 0.4, c),
        "gift" | "box" => {
            rounded(Rect::new(x - s * 0.75, y - s * 0.25, s * 1.5, s * 1.0), s * 0.12, c);
            rounded(Rect::new(x - s * 0.85, y - s * 0.55, s * 1.7, s * 0.38), s * 0.1, c);
            draw_rectangle(x - s * 0.1, y - s * 0.55, s * 0.2, s * 1.3, darken(c, 0.35));
            draw_circle(x - s * 0.25, y - s * 0.7, s * 0.2, c);
            draw_circle(x + s * 0.25, y - s * 0.7, s * 0.2, c);
        }
        "calendar" => {
            rounded(Rect::new(x - s * 0.75, y - s * 0.6, s * 1.5, s * 1.4), s * 0.18, c);
            draw_rectangle(x - s * 0.75, y - s * 0.3, s * 1.5, s * 0.12, darken(c, 0.5));
            for (dx, dy) in [(-0.35, 0.1), (0.0, 0.1), (0.35, 0.1), (-0.35, 0.45), (0.0, 0.45)] {
                draw_rectangle(x + s * dx - s * 0.1, y + s * dy - s * 0.08, s * 0.2, s * 0.16, darken(c, 0.5));
            }
        }
        "back" => {
            draw_line(x + s * 0.2, y - s * 0.6, x - s * 0.4, y, t * 1.3, c);
            draw_line(x - s * 0.4, y, x + s * 0.2, y + s * 0.6, t * 1.3, c);
        }
        "x" | "close" => {
            draw_line(x - s * 0.5, y - s * 0.5, x + s * 0.5, y + s * 0.5, t * 1.3, c);
            draw_line(x + s * 0.5, y - s * 0.5, x - s * 0.5, y + s * 0.5, t * 1.3, c);
        }
        "check" => {
            draw_line(x - s * 0.55, y, x - s * 0.15, y + s * 0.45, t * 1.4, c);
            draw_line(x - s * 0.15, y + s * 0.45, x + s * 0.6, y - s * 0.45, t * 1.4, c);
        }
        "chat" => {
            rounded(Rect::new(x - s * 0.8, y - s * 0.65, s * 1.6, s * 1.05), s * 0.3, c);
            draw_triangle(vec2(x - s * 0.4, y + s * 0.35), vec2(x - s * 0.05, y + s * 0.35), vec2(x - s * 0.5, y + s * 0.8), c);
        }
        "lock" => {
            draw_circle_lines(x, y - s * 0.2, s * 0.4, t * 1.2, c);
            rounded(Rect::new(x - s * 0.6, y - s * 0.15, s * 1.2, s * 0.9), s * 0.15, c);
        }
        "search" => {
            draw_circle_lines(x - s * 0.15, y - s * 0.15, s * 0.5, t * 1.2, c);
            draw_line(x + s * 0.2, y + s * 0.2, x + s * 0.7, y + s * 0.7, t * 1.6, c);
        }
        "coin" => {
            // always gold, with a dark rim so it shows on gold buttons too
            let _ = c;
            draw_circle(x, y, s * 0.82, rgb(150, 98, 8));
            draw_circle(x, y, s * 0.68, rgb(255, 205, 60));
            draw_circle_lines(x, y, s * 0.48, t * 0.8, rgb(214, 150, 20));
            draw_rectangle(x - s * 0.09, y - s * 0.3, s * 0.18, s * 0.6, rgb(214, 150, 20));
        }
        "crown" => {
            draw_triangle(vec2(x - s * 0.8, y + s * 0.4), vec2(x - s * 0.8, y - s * 0.5), vec2(x - s * 0.2, y + s * 0.1), c);
            draw_triangle(vec2(x - s * 0.5, y + s * 0.4), vec2(x, y - s * 0.7), vec2(x + s * 0.5, y + s * 0.4), c);
            draw_triangle(vec2(x + s * 0.8, y + s * 0.4), vec2(x + s * 0.8, y - s * 0.5), vec2(x + s * 0.2, y + s * 0.1), c);
            draw_rectangle(x - s * 0.8, y + s * 0.3, s * 1.6, s * 0.3, c);
        }
        "palette" => {
            draw_circle(x, y, s * 0.8, c);
            for (dx, dy, col) in [(-0.35, -0.3, C_RED), (0.1, -0.45, C_GOLD), (0.45, -0.1, C_BLUE), (-0.4, 0.15, ACCENT)] {
                draw_circle(x + s * dx, y + s * dy, s * 0.15, col);
            }
            draw_circle(x + s * 0.25, y + s * 0.35, s * 0.18, darken(c, 0.5));
        }
        "market" | "tag" => {
            draw_triangle(vec2(x - s * 0.75, y - s * 0.75), vec2(x + s * 0.1, y - s * 0.75), vec2(x + s * 0.75, y - s * 0.1), c);
            draw_triangle(vec2(x - s * 0.75, y - s * 0.75), vec2(x + s * 0.75, y - s * 0.1), vec2(x - s * 0.1, y + s * 0.75), c);
            draw_triangle(vec2(x - s * 0.75, y - s * 0.75), vec2(x - s * 0.1, y + s * 0.75), vec2(x - s * 0.75, y + s * 0.1), c);
            draw_circle(x - s * 0.35, y - s * 0.35, s * 0.14, darken(c, 0.6));
        }
        "speed" | "timer" => {
            draw_circle_lines(x, y + s * 0.1, s * 0.7, t * 1.2, c);
            draw_rectangle(x - s * 0.15, y - s * 0.9, s * 0.3, s * 0.18, c);
            draw_line(x, y + s * 0.1, x + s * 0.35, y - s * 0.25, t * 1.2, c);
        }
        "plus" => {
            draw_line(x - s * 0.6, y, x + s * 0.6, y, t * 1.4, c);
            draw_line(x, y - s * 0.6, x, y + s * 0.6, t * 1.4, c);
        }
        "cube" => {
            let p = |a: f32, b: f32| vec2(x + s * a, y + s * b);
            draw_triangle(p(0.0, -0.8), p(0.75, -0.4), p(0.0, 0.0), lighten(c, 0.2));
            draw_triangle(p(0.0, -0.8), p(-0.75, -0.4), p(0.0, 0.0), lighten(c, 0.2));
            draw_triangle(p(-0.75, -0.4), p(0.0, 0.0), p(-0.75, 0.4), c);
            draw_triangle(p(0.0, 0.0), p(-0.75, 0.4), p(0.0, 0.8), c);
            draw_triangle(p(0.75, -0.4), p(0.0, 0.0), p(0.75, 0.4), darken(c, 0.25));
            draw_triangle(p(0.0, 0.0), p(0.75, 0.4), p(0.0, 0.8), darken(c, 0.25));
        }
        "snake" => {
            for k in 0..5 {
                let a = k as f32 * 0.9;
                draw_circle(x - s * 0.6 + k as f32 * s * 0.3, y + a.sin() * s * 0.25, s * 0.22, c);
            }
        }
        "door" | "logout" => {
            rounded(Rect::new(x - s * 0.7, y - s * 0.8, s * 0.9, s * 1.6), s * 0.1, c);
            draw_line(x + s * 0.1, y, x + s * 0.8, y, t * 1.3, c);
            draw_line(x + s * 0.5, y - s * 0.3, x + s * 0.8, y, t * 1.3, c);
            draw_line(x + s * 0.5, y + s * 0.3, x + s * 0.8, y, t * 1.3, c);
        }
        _ => draw_circle(x, y, s * 0.5, c),
    }
}

/// A soft round glow (many faint circles, brightest in the middle).
pub fn glow(x: f32, y: f32, r: f32, c: Color, strength: f32) {
    for k in 0..14 {
        let t = 1.0 - k as f32 / 14.0;
        draw_circle(x, y, r * t, alpha(c, 0.022 * strength));
    }
}

pub fn star(x: f32, y: f32, outer: f32, inner: f32, c: Color) {
    let center = vec2(x, y);
    let mut pts = Vec::new();
    for k in 0..10 {
        let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
        let r = if k % 2 == 0 { outer } else { inner };
        pts.push(vec2(x + a.cos() * r, y + a.sin() * r));
    }
    for k in 0..10 {
        draw_triangle(center, pts[k], pts[(k + 1) % 10], c);
    }
}

/// A rarity tag like "LEGENDARY".
pub fn rarity_tag(ui: &Ui, rarity_name: &str, color: Color, x: f32, y: f32, size: f32) -> f32 {
    let label = rarity_name.to_uppercase();
    let w = ui.measure(&label, size, true) + 14.0 * ui.u;
    let h = size * ui.u * 1.5;
    rounded(Rect::new(x, y - h / 2.0, w, h), h / 2.0, alpha(color, 0.22));
    ui.text(&label, x + w / 2.0, y, size, lighten(color, 0.15), Align::Center, true);
    w
}
