//! Skins, packs, loot boxes, ranks and the rest of the catalogue.
//!
//! The catalogue comes from the Python game (assets/catalog.json, made by its
//! tools/export_catalog.py), so both games and the server agree on it. The skin
//! colour patterns are ported from `cosmetics.skin_color`.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

pub type Rgb = (u8, u8, u8);

pub fn hex(s: &str) -> Rgb {
    let h = s.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    (p(0), p(2), p(4))
}

#[derive(Deserialize, Clone, Debug)]
pub struct RawSkin {
    pub id: String,
    pub name: String,
    pub rarity: String,
    pub pattern: String,
    pub colors: Vec<String>,
    pub outline: Option<String>,
    pub glow: bool,
    pub price: Option<i64>,
    pub series: String,
}

#[derive(Clone, Debug)]
pub struct Skin {
    pub id: String,
    pub name: String,
    pub rarity: String,
    pub pattern: String,
    pub colors: Vec<Rgb>,
    pub outline: Option<Rgb>,
    pub glow: bool,
    pub price: Option<i64>,
    pub series: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Pack {
    pub id: String,
    pub name: String,
    pub rarity: String,
    pub swatch: Vec<String>,
    pub price: Option<i64>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Crate {
    pub id: String,
    pub name: String,
    pub price: i64,
    pub color: String,
    pub odds: HashMap<String, f64>,
    /// A themed crate gives only this series' skins (the Holiday Crate); "" for the usual pool.
    #[serde(default)]
    pub series: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Rarity {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Rank {
    pub rp: i64,
    pub name: String,
    pub color: String,
    pub rewards: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Mode {
    pub id: String,
    pub name: String,
    pub desc: String,
}

pub struct Catalog {
    pub version: String,
    pub skins: Vec<Skin>,
    pub packs: Vec<Pack>,
    pub crates: Vec<Crate>,
    pub rarities: Vec<Rarity>,
    pub ranks: Vec<Rank>,
    pub modes: Vec<Mode>,
    pub conflicts: Vec<(String, String)>,
    pub versus_modes: Vec<String>,
    pub avatar_icons: Vec<String>,
    pub avatar_colors: Vec<Rgb>,
    pub untradeable: Vec<String>,
    pub categories: Vec<crate::speedrun::Category>,
    pub economy: Value,
    /// Story mode: chapters, levels and bosses.
    pub story: Value,
    pub pack_colors: HashMap<String, HashMap<String, Rgb>>,
}

static CATALOG: OnceLock<Catalog> = OnceLock::new();

pub fn catalog() -> &'static Catalog {
    CATALOG.get_or_init(|| {
        let v: Value = serde_json::from_str(include_str!("../assets/catalog.json")).expect("catalog.json");
        let skins: Vec<RawSkin> = serde_json::from_value(v["skins"].clone()).unwrap();
        let skins = skins
            .into_iter()
            .map(|s| Skin {
                colors: s.colors.iter().map(|c| hex(c)).collect(),
                outline: s.outline.as_deref().map(hex),
                id: s.id,
                name: s.name,
                rarity: s.rarity,
                pattern: s.pattern,
                glow: s.glow,
                price: s.price,
                series: s.series,
            })
            .collect();
        let mut pack_colors = HashMap::new();
        for (id, text) in PACK_JSON {
            let mut colors = HashMap::new();
            if let Ok(Value::Object(m)) = serde_json::from_str::<Value>(text) {
                for (k, val) in m {
                    if let Some(s) = val.as_str() {
                        if s.starts_with('#') && s.len() == 7 {
                            colors.insert(k, hex(s));
                        }
                    }
                }
            }
            pack_colors.insert(id.to_string(), colors);
        }
        Catalog {
            version: v["version"].as_str().unwrap_or("").into(),
            skins,
            packs: serde_json::from_value(v["packs"].clone()).unwrap(),
            crates: serde_json::from_value(v["crates"].clone()).unwrap(),
            rarities: serde_json::from_value(v["rarities"].clone()).unwrap(),
            ranks: serde_json::from_value(v["ranks"].clone()).unwrap(),
            modes: serde_json::from_value(v["modes"].clone()).unwrap(),
            conflicts: serde_json::from_value(v["conflicts"].clone()).unwrap(),
            versus_modes: serde_json::from_value(v["versus_modes"].clone()).unwrap(),
            avatar_icons: serde_json::from_value(v["avatar_icons"].clone()).unwrap(),
            avatar_colors: v["avatar_colors"].as_array().unwrap().iter().map(|c| hex(c.as_str().unwrap())).collect(),
            untradeable: serde_json::from_value(v["untradeable"].clone()).unwrap(),
            categories: serde_json::from_value(v["categories"].clone()).unwrap(),
            economy: v["economy"].clone(),
            story: v["story"].clone(),
            pack_colors,
        }
    })
}

/// Every built-in pack's pack.json (colours; image packs are drawn by their colours here).
const PACK_JSON: &[(&str, &str)] = &[
    ("arctic", include_str!("../assets/packs/arctic/pack.json")),
    ("autumn", include_str!("../assets/packs/autumn/pack.json")),
    ("candyland", include_str!("../assets/packs/candyland/pack.json")),
    ("deepsea", include_str!("../assets/packs/deepsea/pack.json")),
    ("desertnight", include_str!("../assets/packs/desertnight/pack.json")),
    ("desertsand", include_str!("../assets/packs/desertsand/pack.json")),
    ("everforest", include_str!("../assets/packs/everforest/pack.json")),
    ("goldenhour", include_str!("../assets/packs/goldenhour/pack.json")),
    ("grave_viper", include_str!("../assets/packs/grave_viper/pack.json")),
    ("lavalake", include_str!("../assets/packs/lavalake/pack.json")),
    ("meadow", include_str!("../assets/packs/meadow/pack.json")),
    ("midnight", include_str!("../assets/packs/midnight/pack.json")),
    ("neongrid", include_str!("../assets/packs/neongrid/pack.json")),
    ("obsidian", include_str!("../assets/packs/obsidian/pack.json")),
    ("oceanbreeze", include_str!("../assets/packs/oceanbreeze/pack.json")),
    ("sakura", include_str!("../assets/packs/sakura/pack.json")),
    ("shadow_venom", include_str!("../assets/packs/shadow_venom/pack.json")),
    ("synthwave", include_str!("../assets/packs/synthwave/pack.json")),
    ("toxicswamp", include_str!("../assets/packs/toxicswamp/pack.json")),
];

impl Catalog {
    pub fn skin(&self, id: &str) -> Option<&Skin> {
        self.skins.iter().find(|s| s.id == id)
    }
    pub fn pack(&self, id: &str) -> Option<&Pack> {
        self.packs.iter().find(|p| p.id == id)
    }
    pub fn rarity_color(&self, rarity: &str) -> Rgb {
        self.rarities.iter().find(|r| r.id == rarity).map_or((176, 186, 190), |r| hex(&r.color))
    }
    pub fn rarity_name(&self, rarity: &str) -> String {
        self.rarities.iter().find(|r| r.id == rarity).map_or(rarity.to_string(), |r| r.name.clone())
    }
    pub fn rarity_index(&self, rarity: &str) -> usize {
        self.rarities.iter().position(|r| r.id == rarity).unwrap_or(0)
    }
    /// "skin:mint" -> "Mint", "pack:sakura" -> "Sakura pack".
    pub fn item_name(&self, item: &str) -> String {
        match item.split_once(':') {
            Some(("skin", k)) => self.skin(k).map_or(item.into(), |s| s.name.clone()),
            Some(("pack", k)) => self.pack(k).map_or(item.into(), |p| format!("{} pack", p.name)),
            _ => item.into(),
        }
    }
    pub fn item_rarity(&self, item: &str) -> String {
        match item.split_once(':') {
            Some(("skin", k)) => self.skin(k).map_or("common".into(), |s| s.rarity.clone()),
            Some(("pack", k)) => self.pack(k).map_or("common".into(), |p| p.rarity.clone()),
            _ => "common".into(),
        }
    }
    pub fn item_price(&self, item: &str) -> Option<i64> {
        match item.split_once(':') {
            Some(("skin", k)) => self.skin(k).and_then(|s| s.price),
            Some(("pack", k)) => self.pack(k).and_then(|p| p.price),
            _ => None,
        }
    }
    pub fn mode_name(&self, id: &str) -> String {
        self.modes.iter().find(|m| m.id == id).map_or(id.to_string(), |m| m.name.clone())
    }
    pub fn rank_index(&self, name: &str) -> usize {
        self.ranks.iter().position(|r| r.name == name).unwrap_or(0)
    }
    pub fn rank_for(&self, rp: i64) -> usize {
        self.ranks.iter().rposition(|r| rp >= r.rp).unwrap_or(0)
    }
    pub fn econ(&self, key: &str) -> i64 {
        self.economy[key].as_i64().unwrap_or(0)
    }
    pub fn mode_label(&self, modes: &[String]) -> String {
        if modes.is_empty() {
            return "Classic".into();
        }
        let names: Vec<String> = self
            .modes
            .iter()
            .filter(|m| modes.contains(&m.id))
            .map(|m| m.name.clone())
            .collect();
        names.join(" + ")
    }
}

// ---- skin colours (ported from cosmetics.skin_color)

fn lerp(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round().clamp(0.0, 255.0) as u8;
    (f(a.0, b.0), f(a.1, b.1), f(a.2, b.2))
}

fn cycle(colors: &[Rgb], t: f64) -> Rgb {
    let n = colors.len();
    let t = t.rem_euclid(1.0) * n as f64;
    let i = t.floor() as usize;
    lerp(colors[i % n], colors[(i + 1) % n], t - i as f64)
}

pub fn hsv(h: f64, s: f64, v: f64) -> Rgb {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match (i as i64).rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
}

fn hash01(i: i64, j: i64) -> f64 {
    let mut h = (i.wrapping_mul(374761393).wrapping_add(j.wrapping_mul(668265263))) as u64 & 0xFFFF_FFFF;
    h = ((h ^ (h >> 13)).wrapping_mul(1274126177)) & 0xFFFF_FFFF;
    (h ^ (h >> 16)) as f64 / 4294967296.0
}

/// Colour of body cell `i` (0 = head) of an `n`-cell snake at time `now` (ms).
pub fn skin_color(skin: &Skin, i: usize, n: usize, now: f64) -> Rgb {
    let c = &skin.colors;
    let fi = i as f64;
    let nn = n.max(1) as f64;
    let k = (fi / (nn - 1.0).max(1.0)).clamp(0.0, 1.0);
    let last = *c.last().unwrap();
    match skin.pattern.as_str() {
        "grad" => lerp(c[0], last, k),
        "stripes" => c[i % c.len()],
        "rainbow" => {
            let hue = (fi * 0.07 - now / 2500.0).rem_euclid(1.0);
            hsv(hue, if c[0] != (255, 255, 255) { 0.85 } else { 0.45 }, 1.0)
        }
        "flow" => cycle(c, fi * 0.09 - now / 1800.0),
        "pulse" => lerp(c[0], c[1], 0.5 + 0.5 * (now / 260.0 - fi * 0.55).sin()),
        "shimmer" => {
            let band = (now / 90.0).rem_euclid(nn + 8.0) - 4.0;
            lerp(c[0], c[1], (1.0 - (fi - band).abs() / 2.5).max(0.0))
        }
        "aurora" => {
            let wave = 0.5 + 0.25 * (fi * 0.45 - now / 700.0).sin() + 0.25 * (fi * 0.17 + now / 1100.0).sin();
            lerp(c[0], cycle(&c[1..], fi * 0.04 + now / 4000.0), 0.25 + 0.75 * wave)
        }
        "lava" => {
            let heat = ((fi * 1.7 + now / 450.0).sin() * (fi * 0.6 - now / 800.0).sin()).max(0.0);
            let flare = hash01(i as i64, (now / 160.0).floor() as i64) > 0.93;
            lerp(lerp(c[0], c[1], heat * 1.6), c[2], if flare { 1.0 } else { (heat - 0.55).max(0.0) * 2.0 })
        }
        "galaxy" => {
            let base = lerp(c[0], c[1], 0.5 + 0.5 * (fi * 0.3 - now / 1500.0).sin());
            let star = hash01(i as i64, (now / 220.0).floor() as i64);
            lerp(base, c[2], (star - 0.82).max(0.0) / 0.18)
        }
        "circuit" => {
            let trace = if i % 3 != 0 { c[1] } else { c[0] };
            let packet = (now / 70.0).rem_euclid(nn + 6.0) - 3.0;
            lerp(trace, c[2], (1.0 - (fi - packet).abs() / 1.5).max(0.0) + if i % 6 == 0 { 0.35 } else { 0.0 })
        }
        "prism" => {
            let col = hsv((fi * 0.11 + now / 900.0).rem_euclid(1.0), 1.0, 1.0);
            let flash = (now / 160.0 - fi * 0.9).sin().max(0.0).powi(8);
            lerp(lerp(col, c[0], 0.25), (255, 255, 255), flash)
        }
        "heartbeat" => {
            let t = (now / 1000.0).rem_euclid(1.0);
            let beat = (1.0 - (fi - t * 18.0).abs() / 2.0).max(0.0)
                + (1.0 - (fi - (t - 0.18) * 18.0).abs() / 2.0).max(0.0) * 0.7;
            lerp(c[0], c[1], (0.25 + beat).min(1.0))
        }
        "chevron" => {
            let phase = (fi + now / 180.0).rem_euclid(4.0);
            if phase < 1.6 { c[1] } else { lerp(c[0], c[1], 0.15) }
        }
        "scales" => {
            let base = if i % 2 == 1 { c[1] } else { c[0] };
            let gleam = (1.0 - (fi - (now / 110.0).rem_euclid(nn + 10.0) + 5.0).abs() / 3.0).max(0.0);
            lerp(base, (255, 255, 255), 0.6 * gleam)
        }
        "spark" => {
            let base = lerp(c[0], c[1], 0.3 + 0.3 * (fi * 0.8 + now / 300.0).sin());
            let on = hash01(i as i64, (now / 90.0).floor() as i64) > 0.88;
            lerp(base, c[2], if on { 1.0 } else { 0.0 })
        }
        "ouroboros" => {
            // emerald scales, with a golden ring chasing round from tail to head
            let base = lerp(c[0], c[1], 0.5 + 0.5 * (i as f64 * 0.9 + now / 400.0).sin());
            let ring = (n as f64 + 4.0) - (now / 85.0).rem_euclid(n as f64 + 8.0);
            lerp(base, c[2], (1.0 - (i as f64 - ring).abs() / 2.2).max(0.0))
        }
        "moderator" => {
            // navy and blue bands with a gold sweep, like a badge catching the light
            let base = if (i / 2) % 2 == 1 { c[1] } else { c[0] };
            let sweep = (now / 70.0).rem_euclid(n as f64 + 10.0) - 5.0;
            lerp(base, c[2], (1.0 - (i as f64 - sweep).abs() / 1.8).max(0.0))
        }
        // paper like a dunce cap: cream, with a grey band every few cells
        "dunce" => if i % 4 == 3 { c[1] } else { c[0] },
        "lights" => {
            // a string of coloured bulbs, twinkling on and off
            let bulb = c[i % c.len()];
            if hash01(i as i64, (now / 450.0).floor() as i64) > 0.25 { bulb } else { lerp(bulb, (24, 24, 30), 0.65) }
        }
        _ => c[0],
    }
}

pub fn skin_outline(skin: &Skin, now: f64) -> Option<Rgb> {
    let o = skin.outline?;
    if !skin.glow {
        return Some(o);
    }
    let k = 0.5 + 0.5 * (now / 220.0).sin();
    Some(lerp(o, (255, 255, 255), 0.35 * k))
}

/// The plain snake when no skin is on: blue, like the Python game's default.
pub fn default_skin() -> Skin {
    Skin {
        id: String::new(),
        name: "Classic Blue".into(),
        rarity: "common".into(),
        pattern: "grad".into(),
        colors: vec![(61, 111, 232), (122, 162, 255)],
        outline: None,
        glow: false,
        price: None,
        series: String::new(),
    }
}

/// Snakes in versus: your team and theirs.
pub fn team_skin(team: i32, n: usize) -> Skin {
    let sets: [[Rgb; 2]; 4] = [[(61, 111, 232), (122, 162, 255)], [(19, 154, 143), (111, 224, 212)],
                                [(232, 112, 26), (255, 179, 107)], [(224, 69, 123), (255, 158, 192)]];
    let c = sets[((team * 2) as usize + n % 2) % 4];
    Skin { colors: c.to_vec(), ..default_skin() }
}

pub fn format_points(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}
