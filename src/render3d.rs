//! The board in 3D, seen from a fixed camera above and behind it.
//!
//! macroquad has no lights, so shapes are built as meshes with the shading baked
//! into their vertex colours (light from the upper left), batched into big meshes.

use crate::cosmetics::{skin_outline, Skin};
use crate::game::Dir;
use crate::render::{body_color, body_path, fruit_color, snake_points, BoardColors, Effects, View, PORTAL_COLORS};
use crate::ui::{alpha, darken, lighten, mix, rgb, rgbt};
use macroquad::models::{Mesh, Vertex};
use macroquad::prelude::*;

const LIGHT: Vec3 = Vec3::new(-0.45, 0.85, 0.35);

#[derive(Clone, Copy)]
struct Ring {
    center: Vec3,
    radius: f32,
    dir: Vec3,
    color: Color,
    gap: bool,
}

struct Batch {
    verts: Vec<Vertex>,
    idx: Vec<u16>,
}

impl Batch {
    fn new() -> Batch {
        Batch { verts: Vec::with_capacity(60000), idx: Vec::with_capacity(90000) }
    }

    fn flush(&mut self) {
        if self.idx.is_empty() {
            return;
        }
        let mesh = Mesh { vertices: std::mem::take(&mut self.verts), indices: std::mem::take(&mut self.idx), texture: None };
        draw_mesh(&mesh);
        self.verts = Vec::with_capacity(60000);
        self.idx = Vec::with_capacity(90000);
    }

    fn room(&mut self, n: usize) {
        if self.verts.len() + n > 64000 {
            self.flush();
        }
    }

    fn shade(c: Color, normal: Vec3) -> Color {
        let k = 0.55 + 0.45 * normal.normalize().dot(LIGHT.normalize()).max(0.0);
        Color::new(c.r * k, c.g * k, c.b * k, c.a)
    }

    fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, color: Color) {
        self.room(4);
        let n = (b - a).cross(d - a);
        let col = Self::shade(color, -n);
        let base = self.verts.len() as u16;
        for p in [a, b, c, d] {
            self.verts.push(Vertex::new2(p, vec2(0.0, 0.0), col));
        }
        self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A box from its centre and size (no bottom face: nothing looks up from under the board).
    fn cuboid(&mut self, center: Vec3, size: Vec3, color: Color) {
        let h = size / 2.0;
        let p = |x: f32, y: f32, z: f32| center + vec3(x * h.x, y * h.y, z * h.z);
        self.quad(p(-1., 1., -1.), p(1., 1., -1.), p(1., 1., 1.), p(-1., 1., 1.), color); // top
        self.quad(p(-1., -1., 1.), p(-1., 1., 1.), p(1., 1., 1.), p(1., -1., 1.), color); // front (+z)
        self.quad(p(1., -1., -1.), p(1., 1., -1.), p(-1., 1., -1.), p(-1., -1., -1.), color); // back
        self.quad(p(-1., -1., -1.), p(-1., 1., -1.), p(-1., 1., 1.), p(-1., -1., 1.), color); // left
        self.quad(p(1., -1., 1.), p(1., 1., 1.), p(1., 1., -1.), p(1., -1., -1.), color); // right
    }

    fn sphere(&mut self, center: Vec3, radius: f32, color: Color, detail: usize) {
        let (rings, segs) = (detail, detail * 2);
        self.room((rings + 1) * (segs + 1));
        let base = self.verts.len() as u16;
        for i in 0..=rings {
            let v = i as f32 / rings as f32 * std::f32::consts::PI;
            for j in 0..=segs {
                let u = j as f32 / segs as f32 * std::f32::consts::TAU;
                let n = vec3(v.sin() * u.cos(), v.cos(), v.sin() * u.sin());
                // a soft specular dot towards the light
                let spec = n.dot(LIGHT.normalize()).max(0.0).powi(16) * 0.5;
                let c = Self::shade(color, n);
                let c = Color::new((c.r + spec).min(1.0), (c.g + spec).min(1.0), (c.b + spec).min(1.0), c.a);
                self.verts.push(Vertex::new2(center + n * radius, vec2(0.0, 0.0), c));
            }
        }
        for i in 0..rings {
            for j in 0..segs {
                let a = base + (i * (segs + 1) + j) as u16;
                let b = a + (segs + 1) as u16;
                self.idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
    }

    /// A flat soft shadow on the floor.
    fn shadow(&mut self, x: f32, z: f32, r: f32, a: f32) {
        let segs = 12;
        self.room(segs + 2);
        let base = self.verts.len() as u16;
        self.verts.push(Vertex::new2(vec3(x, 0.01, z), vec2(0.0, 0.0), Color::new(0.0, 0.0, 0.0, a)));
        for k in 0..=segs {
            let t = k as f32 / segs as f32 * std::f32::consts::TAU;
            self.verts.push(Vertex::new2(vec3(x + t.cos() * r, 0.01, z + t.sin() * r), vec2(0.0, 0.0),
                                         Color::new(0.0, 0.0, 0.0, 0.0)));
        }
        for k in 0..segs as u16 {
            self.idx.extend_from_slice(&[base, base + 1 + k, base + 2 + k]);
        }
    }

    /// A smooth tube through rings (a snake's body), shaded by each vertex's normal.
    /// `lower`: only the bottom half (an outline band showing round the belly).
    fn tube(&mut self, rings: &[Ring], lower: bool) {
        const AROUND: usize = 14;
        if rings.len() < 2 {
            return;
        }
        let mut start = 0;
        while start < rings.len() {
            // one unbroken stretch at a time
            let mut end = start + 1;
            while end < rings.len() && !rings[end].gap {
                end += 1;
            }
            let part = &rings[start..end];
            start = end;
            if part.len() < 2 {
                continue;
            }
            // (big bodies go in several batches)
            for chunk in part.windows(2).collect::<Vec<_>>().chunks(300) {
                let count = chunk.len() + 1;
                self.room(count * (AROUND + 1));
                let base = self.verts.len() as u16;
                let ring_list: Vec<&Ring> = chunk.iter().map(|w| &w[0]).chain(std::iter::once(&chunk[chunk.len() - 1][1])).collect();
                for r in &ring_list {
                    let side = r.dir.cross(Vec3::Y).normalize_or_zero();
                    for k in 0..=AROUND {
                        let a = k as f32 / AROUND as f32 * std::f32::consts::TAU;
                        let normal = Vec3::Y * a.cos() + side * a.sin();
                        let c = Self::shade(r.color, normal);
                        let spec = normal.dot(LIGHT.normalize()).max(0.0).powi(24) * 0.35;
                        let c = Color::new((c.r + spec).min(1.0), (c.g + spec).min(1.0), (c.b + spec).min(1.0), c.a);
                        self.verts.push(Vertex::new2(r.center + normal * r.radius, vec2(0.0, 0.0), c));
                    }
                }
                for j in 0..count - 1 {
                    for k in 0..AROUND {
                        if lower && (k < AROUND / 4 || k >= AROUND * 3 / 4) {
                            continue; // (the top half: the body covers it anyway)
                        }
                        let a = base + (j * (AROUND + 1) + k) as u16;
                        let b2 = a + (AROUND + 1) as u16;
                        self.idx.extend_from_slice(&[a, b2, a + 1, a + 1, b2, b2 + 1]);
                    }
                }
            }
        }
    }

    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color) {
        self.room(3);
        let base = self.verts.len() as u16;
        for p in [a, b, c] {
            self.verts.push(Vertex::new2(p, vec2(0.0, 0.0), color));
        }
        self.idx.extend_from_slice(&[base, base + 1, base + 2]);
    }
}

pub fn camera_for(w: i32, h: i32, area: Rect) -> Camera3D {
    let (fw, fh) = (w as f32, h as f32);
    let size = fw.max(fh * 1.25);
    let target = vec3(fw / 2.0, 0.0, fh / 2.0 + fh * 0.04);
    let aspect = area.w / area.h;
    // back far enough that the whole board fits, whatever the window's shape
    let dist = size * 1.08 / aspect.min(1.6).max(0.55) + 2.5;
    Camera3D {
        position: target + vec3(0.0, dist * 0.92, dist * 0.62),
        target,
        up: vec3(0.0, 1.0, 0.0),
        fovy: 45f32.to_radians(),
        aspect: Some(aspect),
        projection: Projection::Perspective,
        render_target: None,
        viewport: Some((area.x as i32, (screen_height() - area.y - area.h) as i32, area.w as i32, area.h as i32)),
        ..Default::default()
    }
}

/// Where a point on the board shows up on screen.
pub fn to_screen(cam: &Camera3D, area: Rect, p: Vec3) -> Option<Vec2> {
    let m = cam.matrix();
    let clip = m * p.extend(1.0);
    if clip.w <= 0.0 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(vec2(area.x + (ndc.x + 1.0) / 2.0 * area.w, area.y + (1.0 - ndc.y) / 2.0 * area.h))
}

pub fn draw_board3d(v: &View, area: Rect, fx: &Effects) {
    let g = v.game;
    let cam = camera_for(g.w, g.h, area);
    set_camera(&cam);
    let mut b = Batch::new();
    let time = v.now_ms / 1000.0;
    let colors: BoardColors = v.colors;

    // light mode: how lit each cell is
    let heads: Vec<Vec2> = if g.has("light") && !g.over {
        (0..g.snakes.len()).filter(|&i| !g.snakes[i].dead).map(|i| snake_points(g, i, v.t)[0]).collect()
    } else {
        Vec::new()
    };
    let lit = |x: f32, y: f32| -> f32 {
        if heads.is_empty() {
            return 1.0;
        }
        let d = heads.iter().map(|h| (vec2(x, y) - *h).length()).fold(f32::MAX, f32::min);
        1.0 - ((d - 1.6) / 1.6).clamp(0.0, 0.9)
    };
    let dim = |c: Color, k: f32| Color::new(c.r * k, c.g * k, c.b * k, c.a);

    // the board: a raised slab with a border
    let (fw, fh) = (g.w as f32, g.h as f32);
    // (its top sits just under the tiles, so the two never fight over the same depth)
    b.cuboid(vec3(fw / 2.0, -0.37, fh / 2.0), vec3(fw + 0.6, 0.7, fh + 0.6), colors.border);
    for y in 0..g.h {
        for x in 0..g.w {
            let c = if (x + y) % 2 == 0 { colors.light } else { colors.dark };
            let k = lit(x as f32, y as f32);
            let (x0, z0) = (x as f32, y as f32);
            b.quad(vec3(x0, 0.0, z0), vec3(x0 + 1.0, 0.0, z0), vec3(x0 + 1.0, 0.0, z0 + 1.0), vec3(x0, 0.0, z0 + 1.0), dim(c, k));
        }
    }
    // shadows go on the floor before anything stands on it
    for s in &g.snakes {
        for p in &s.body {
            b.shadow(p.x as f32 + 0.55, p.y as f32 + 0.6, 0.5, 0.18);
        }
    }
    for a in &g.apples {
        b.shadow(a.x as f32 + 0.55, a.y as f32 + 0.6, 0.38, 0.22);
    }
    let center = |x: i32, y: i32, hgt: f32| vec3(x as f32 + 0.5, hgt, y as f32 + 0.5);

    // targets, arrows, minesweeper hints on the floor
    for &(x, y) in &g.targets {
        b.cuboid(center(x, y, 0.03), vec3(0.7, 0.05, 0.7), alpha(rgb(60, 60, 60), 0.8));
        b.cuboid(center(x, y, 0.05), vec3(0.45, 0.05, 0.45), dim(colors.light, 1.0));
    }
    for (&(x, y), &d) in &g.arrows {
        let c = center(x, y, 0.04);
        let (dx, dy) = d.delta();
        let f = vec3(dx as f32, 0.0, dy as f32);
        let s = vec3(-f.z, 0.0, f.x);
        b.cuboid(c, vec3(0.86, 0.04, 0.86), alpha(WHITE, 0.6));
        let tip = c + f * 0.38 + vec3(0.0, 0.03, 0.0);
        b.tri(tip, c + s * 0.32 + vec3(0.0, 0.03, 0.0), c - s * 0.32 + vec3(0.0, 0.03, 0.0), rgb(230, 120, 20));
        b.tri(tip, c - s * 0.32 + vec3(0.0, 0.03, 0.0), c + s * 0.32 + vec3(0.0, 0.03, 0.0), rgb(230, 120, 20));
        b.cuboid(c - f * 0.12 + vec3(0.0, 0.03, 0.0), vec3(0.18, 0.02, 0.18) + f.abs() * 0.3, rgb(230, 120, 20));
    }
    if g.has("minesweeper") {
        for &(x, y) in &g.revealed {
            let n = g.mine_count(x, y);
            for k in 0..n {
                let a = k as f32 / n as f32 * std::f32::consts::TAU;
                b.cuboid(center(x, y, 0.03) + vec3(a.cos() * 0.22, 0.0, a.sin() * 0.22), vec3(0.12, 0.04, 0.12),
                         rgb(211, 47, 47));
            }
        }
    }
    // walls, statues, gates
    for w in g.walls.values() {
        if w.kind == "statue" {
            b.cuboid(center(w.x, w.y, 0.35), vec3(0.8, 0.7, 0.8), rgb(150, 150, 160));
            b.sphere(center(w.x, w.y, 0.8), 0.3, rgb(170, 170, 180), 6);
        } else if g.wall_active(w) {
            let c = match w.dim {
                Some(0) => rgb(80, 90, 200),
                Some(_) => rgb(190, 70, 160),
                None => rgb(120, 96, 78),
            };
            b.cuboid(center(w.x, w.y, 0.42), vec3(0.94, 0.84, 0.94), dim(c, lit(w.x as f32, w.y as f32)));
            b.cuboid(center(w.x, w.y, 0.86), vec3(0.8, 0.06, 0.8), lighten(c, 0.15));
        } else {
            let c = if w.dim == Some(0) { rgb(80, 90, 200) } else { rgb(190, 70, 160) };
            b.cuboid(center(w.x, w.y, 0.03), vec3(0.8, 0.05, 0.8), alpha(c, 0.45));
        }
    }
    for gt in &g.gates {
        if gt.open {
            for k in [-0.38, 0.38] {
                b.cuboid(center(gt.x, gt.y, 0.45) + vec3(k, 0.0, 0.0), vec3(0.1, 0.9, 0.1), rgb(110, 110, 120));
            }
        } else {
            b.cuboid(center(gt.x, gt.y, 0.45), vec3(0.9, 0.9, 0.9), rgb(90, 92, 104));
            for k in 0..4 {
                b.cuboid(center(gt.x, gt.y, 0.45) + vec3(-0.3 + k as f32 * 0.2, 0.0, 0.46), vec3(0.06, 0.8, 0.02),
                         rgb(170, 172, 185));
            }
        }
    }
    if v.reveal {
        for &(x, y) in &g.mines {
            b.sphere(center(x, y, 0.3), 0.28, rgb(40, 40, 46), 6);
            b.sphere(center(x, y, 0.62), 0.07, rgb(230, 60, 40), 4);
        }
    }
    // items and fruit, bobbing
    for it in &g.items {
        let bob = 0.45 + 0.08 * ((time * 3.0 + it.x as f64) as f32).sin();
        let c = center(it.x, it.y, bob);
        if it.kind == "key" {
            let col = rgb(255, 196, 0);
            b.sphere(c + vec3(-0.18, 0.0, 0.0), 0.16, col, 6);
            b.cuboid(c + vec3(0.12, 0.0, 0.0), vec3(0.45, 0.08, 0.08), col);
            b.cuboid(c + vec3(0.3, -0.08, 0.0), vec3(0.06, 0.16, 0.06), col);
        } else {
            b.cuboid(c, vec3(0.5, 0.55, 0.12), rgb(47, 128, 237));
            b.cuboid(c + vec3(0.0, 0.0, 0.07), vec3(0.16, 0.38, 0.03), lighten(rgb(47, 128, 237), 0.4));
        }
    }
    for a in &g.apples {
        let bob = 0.38 + 0.06 * ((time * 3.5 + a.x as f64 * 0.7) as f32).sin();
        let c = center(a.x, a.y, bob);
        if g.has("portal") {
            let pc = PORTAL_COLORS[a.group as usize % 6];
            for k in 0..10 {
                let t = k as f32 / 10.0 * std::f32::consts::TAU + time as f32 * 2.0;
                b.cuboid(center(a.x, a.y, 0.05) + vec3(t.cos() * 0.42, 0.0, t.sin() * 0.42), vec3(0.1, 0.06, 0.1), pc);
            }
        }
        if a.poison {
            b.sphere(c, 0.3, rgb(125, 47, 191), 7);
            b.sphere(c + vec3(0.0, 0.0, 0.26), 0.07, rgb(230, 255, 200), 4);
        } else {
            let col = fruit_color(&a.fruit);
            b.sphere(c, 0.3, col, 8);
            b.cuboid(c + vec3(0.0, 0.32, 0.0), vec3(0.05, 0.14, 0.05), rgb(107, 68, 35));
            b.cuboid(c + vec3(0.1, 0.34, 0.0), vec3(0.16, 0.03, 0.08), rgb(76, 175, 55));
        }
        if a.locked {
            b.cuboid(c + vec3(0.0, 0.0, 0.32), vec3(0.24, 0.2, 0.06), rgb(255, 196, 0));
        }
    }
    // snakes: one smooth tube along each body's path, with rounded ends
    for i in 0..g.snakes.len() {
        let s = &g.snakes[i];
        let skin: &Skin = &v.skins[i.min(v.skins.len() - 1)];
        let path = body_path(g, i, v.t, 0.2);
        let pts = snake_points(g, i, v.t);
        let n = s.body.len();
        let faded = s.dead && g.versus();
        let fade = |c: Color| if faded { mix(c, rgb(120, 120, 120), 0.6) } else { c };
        let len = path.last().map_or(1.0, |p| p.k).max(0.01);
        let radius = |k: f32| 0.38 * (1.0 - 0.3 * ((k - (len - 1.6)) / 1.6).clamp(0.0, 1.0));
        let rings: Vec<Ring> = path
            .iter()
            .enumerate()
            .map(|(j, p)| {
                // the direction of the body here (from the points either side)
                let a = path[j.saturating_sub(1)].pos;
                let bpt = path[(j + 1).min(path.len() - 1)].pos;
                let mut dir = bpt - a;
                if dir.length() < 1e-4 || dir.length() > 1.5 {
                    dir = vec2(1.0, 0.0);
                }
                let r = radius(p.k);
                Ring {
                    center: vec3(p.pos.x + 0.5, r + 0.02, p.pos.y + 0.5),
                    radius: r,
                    dir: vec3(dir.x, 0.0, dir.y).normalize(),
                    color: fade(rgbt(body_color(skin, p.k, n, v.now_ms))),
                    gap: p.gap,
                }
            })
            .collect();
        if let Some(o) = skin_outline(skin, v.now_ms).map(rgbt) {
            // a glowing band around the belly for skins with an outline
            let belly: Vec<Ring> = rings.iter().map(|r| Ring { radius: r.radius + 0.045, color: fade(o), ..*r }).collect();
            b.tube(&belly, true);
        }
        b.tube(&rings, false);
        if let (Some(first), Some(last)) = (rings.first(), rings.last()) {
            b.sphere(first.center, first.radius * 1.12, first.color, 10);
            b.sphere(last.center, last.radius, last.color, 8);
        }
        // eyes
        if let Some(&h) = pts.first() {
            let d = match s.planned.unwrap_or(s.dir) {
                Dir::Up => vec3(0.0, 0.0, -1.0),
                Dir::Down => vec3(0.0, 0.0, 1.0),
                Dir::Left => vec3(-1.0, 0.0, 0.0),
                Dir::Right => vec3(1.0, 0.0, 0.0),
            };
            let side = vec3(-d.z, 0.0, d.x);
            for k in [-1.0, 1.0] {
                let e = vec3(h.x + 0.5, 0.66, h.y + 0.5) + d * 0.24 + side * 0.2 * k;
                b.sphere(e, 0.14, WHITE, 6);
                if s.dead {
                    b.cuboid(e + d * 0.1, vec3(0.16, 0.04, 0.16), rgb(20, 24, 32));
                } else {
                    b.sphere(e + d * 0.08 + vec3(0.0, 0.02, 0.0), 0.07, rgb(20, 24, 32), 5);
                }
            }
            if s.shield > 0 {
                for k in 0..12 {
                    let t = k as f32 / 12.0 * std::f32::consts::TAU + time as f32 * 2.0;
                    b.sphere(vec3(h.x + 0.5 + t.cos() * 0.6, 0.45, h.y + 0.5 + t.sin() * 0.6), 0.05,
                             rgb(80, 160, 255), 4);
                }
            }
        }
    }
    // particles
    for p in &fx.particles {
        let k = 1.0 - p.life / p.max;
        let y = 0.4 + p.life * 1.2;
        b.cuboid(vec3(p.pos.x, y, p.pos.y), Vec3::splat(p.size * 1.5 * k + 0.01), alpha(p.color, k));
    }
    b.flush();
    set_default_camera();

    // names over the snakes
    if v.names {
        for (i, s) in g.snakes.iter().enumerate() {
            if s.name.is_empty() {
                continue;
            }
            let h = snake_points(g, i, v.t)[0];
            if let Some(p) = to_screen(&cam, area, vec3(h.x + 0.5, 1.3, h.y + 0.5)) {
                let you = v.you == Some(i);
                let label = if you { format!("{} (you)", s.name) } else { s.name.clone() };
                let d = measure_text(&label, None, 16, 1.0);
                crate::ui::rounded(Rect::new(p.x - d.width / 2.0 - 5.0, p.y - d.height - 4.0, d.width + 10.0, d.height + 8.0),
                                   6.0, alpha(BLACK, 0.5));
                draw_text(&label, p.x - d.width / 2.0, p.y, 16.0, WHITE);
            }
        }
    }
    let _ = darken;
}
