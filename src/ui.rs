// Minecraft-style UI kit: a pure "Painter" that accumulates colored rects
// and textured quads, executed by the renderer on the GPU.
//
// Pure = testable everywhere: a software rasterizer turns a Painter + a
// fake atlas into PNG previews of every screen (see tests at the bottom).
//
// All coordinates are GUI pixels (the app supplies a scaled ortho).

use crate::font;
use crate::world::{icon_tint, is_cross_id, tiles_of, ATLAS_COLS, ATLAS_ROWS};

pub const GUI_W_DEF: f32 = 320.0;

// ------------------------------------------------------------------ painter
#[derive(Clone, Copy)]
pub struct RectOp {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub col: [f32; 4],
}

#[derive(Clone, Copy)]
pub struct TexOp {
    /// four corners in GUI px (any convex parallelogram/quad)
    pub xy: [[f32; 2]; 4],
    /// four corners in atlas UV
    pub uv: [[f32; 2]; 4],
    /// per-corner shade (r=g=b) times tint
    pub shade: [f32; 4],
    pub tint: [f32; 3],
}

#[derive(Clone, Copy)]
pub enum Op {
    Rect(RectOp),
    Tex(TexOp),
}

#[derive(Default)]
pub struct Painter {
    pub ops: Vec<Op>,
}

impl Painter {
    pub fn new() -> Painter {
        Painter { ops: Vec::new() }
    }
    pub fn clear(&mut self) {
        self.ops.clear();
    }
    pub fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, col: [f32; 4]) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        self.ops.push(Op::Rect(RectOp { x0, y0, x1, y1, col }));
    }
    pub fn tex(&mut self, op: TexOp) {
        self.ops.push(Op::Tex(op));
    }
    pub fn rect_outline(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, t: f32, col: [f32; 4]) {
        self.rect(x0, y0, x1, y0 + t, col);
        self.rect(x0, y1 - t, x1, y1, col);
        self.rect(x0, y0 + t, x0 + t, y1 - t, col);
        self.rect(x1 - t, y0 + t, x1, y1 - t, col);
    }
}

// ------------------------------------------------------------------ colors
pub const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const YELLOW: [f32; 4] = [1.0, 1.0, 0.33, 1.0];
pub const GRAY: [f32; 4] = [0.67, 0.67, 0.67, 1.0];
pub const DARKGRAY: [f32; 4] = [0.33, 0.33, 0.33, 1.0];
pub const RED: [f32; 4] = [1.0, 0.33, 0.33, 1.0];
pub const GREEN: [f32; 4] = [0.33, 1.0, 0.33, 1.0];

/// Minecraft chat colors (§0..§f).
fn chat_color(code: char) -> Option<[f32; 4]> {
    Some(match code {
        '0' => [0.0, 0.0, 0.0, 1.0],
        '1' => [0.0, 0.0, 0.66, 1.0],
        '2' => [0.0, 0.66, 0.0, 1.0],
        '3' => [0.0, 0.66, 0.66, 1.0],
        '4' => [0.66, 0.0, 0.0, 1.0],
        '5' => [0.66, 0.0, 0.66, 1.0],
        '6' => [1.0, 0.66, 0.0, 1.0],
        '7' => [0.66, 0.66, 0.66, 1.0],
        '8' => [0.33, 0.33, 0.33, 1.0],
        '9' => [0.33, 0.33, 1.0, 1.0],
        'a' => [0.33, 1.0, 0.33, 1.0],
        'b' => [0.33, 1.0, 1.0, 1.0],
        'c' => [1.0, 0.33, 0.33, 1.0],
        'd' => [1.0, 0.33, 1.0, 1.0],
        'e' => [1.0, 1.0, 0.33, 1.0],
        'f' => [1.0, 1.0, 1.0, 1.0],
        _ => return None,
    })
}

/// Split on § color codes -> (text, color) runs.
pub fn parse_colors(s: &str) -> Vec<(String, [f32; 4])> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut col = WHITE;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '§' {
            if let Some(code) = chars.next() {
                if !cur.is_empty() {
                    out.push((std::mem::take(&mut cur), col));
                }
                if let Some(nc) = chat_color(code) {
                    col = nc;
                }
                continue;
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push((cur, col));
    }
    if out.is_empty() {
        out.push((String::new(), WHITE));
    }
    out
}

// --------------------------------------------------------------------- text
pub fn text_w(s: &str) -> f32 {
    font::width(s) as f32
}

/// Draw text with a hard drop shadow (Minecraft style). Returns the width.
pub fn text(p: &mut Painter, s: &str, x: f32, y: f32, col: [f32; 4], shadow: bool) -> f32 {
    let mut runs = parse_colors(s);
    if runs.len() == 1 {
        runs[0].1 = col;
    }
    let mut cx = x;
    for (part, pcol) in &runs {
        let c = *pcol;
        for ch in part.chars() {
            let (rows, w) = match font::glyph(ch) {
                Some(g) => g,
                None => {
                    cx += 5.0;
                    continue;
                }
            };
            if shadow {
                push_glyph(p, &rows, w, cx + 1.0, y + 1.0, [c[0] * 0.22, c[1] * 0.22, c[2] * 0.22, c[3]]);
            }
            push_glyph(p, &rows, w, cx, y, c);
            cx += w as f32 + 1.0;
        }
    }
    cx - x
}

fn push_glyph(p: &mut Painter, rows: &[u8; font::ROWS], w: usize, x: f32, y: f32, col: [f32; 4]) {
    for (r, &bits) in rows.iter().enumerate() {
        let mut b = 0usize;
        while b < w {
            if bits & (1 << (4 - b)) != 0 {
                let mut run = 1usize;
                while b + run < w && bits & (1 << (4 - b - run)) != 0 {
                    run += 1;
                }
                p.rect(x + b as f32, y + r as f32, x + (b + run) as f32, y + r as f32 + 1.0, col);
                b += run;
            } else {
                b += 1;
            }
        }
    }
}

pub fn center_text(p: &mut Painter, s: &str, cx: f32, y: f32, col: [f32; 4], shadow: bool) -> f32 {
    let w = text_w(s);
    text(p, s, cx - w * 0.5, y, col, shadow);
    w
}

// ------------------------------------------------------------------ widgets
/// Minecraft gray button (bevel + hover tint). Returns true when hovered.
pub fn button(p: &mut Painter, label: &str, x: f32, y: f32, w: f32, h: f32, hovered: bool) {
    let base = if hovered {
        [0.45, 0.55, 0.75, 0.95]
    } else {
        [0.42, 0.42, 0.44, 0.95]
    };
    p.rect(x, y, x + w, y + h, base);
    // bevel: light top/left, dark bottom/right
    let light = [0.62, 0.62, 0.64, 0.95];
    let dark = [0.2, 0.2, 0.22, 0.95];
    p.rect(x, y, x + w, y + 1.0, light);
    p.rect(x, y, x + 1.0, y + h, light);
    p.rect(x, y + h - 1.0, x + w, y + h, dark);
    p.rect(x + w - 1.0, y, x + w, y + h, dark);
    // black outer outline (MC buttons are outlined)
    p.rect_outline(x - 1.0, y - 1.0, x + w + 1.0, y + h + 1.0, 1.0, [0.05, 0.05, 0.05, 0.9]);
    center_text(p, label, x + w * 0.5, y + (h - 7.0) * 0.5 + 0.5, WHITE, true);
}

pub fn hover(mouse: (f32, f32), x: f32, y: f32, w: f32, h: f32) -> bool {
    mouse.0 >= x && mouse.0 <= x + w && mouse.1 >= y && mouse.1 <= y + h
}

/// Tiled dirt background (menus), darkened like the vanilla screens.
pub fn dirt_bg(p: &mut Painter, tile: u32, w: f32, h: f32) {
    let (tu, tv) = tile_uv(tile);
    let s = 16.0;
    let mut y = 0.0;
    while y < h {
        let mut x = 0.0;
        while x < w {
            let uv = tile_uv(tile);
            let _ = (tu, tv);
            p.tex(TexOp {
                xy: [[x, y], [x + s, y], [x + s, y + s], [x, y + s]],
                uv: [[uv.0, uv.1], [uv.0 + TSU, uv.1], [uv.0 + TSU, uv.1 + TSV], [uv.0, uv.1 + TSV]],
                shade: [0.32, 0.32, 0.32, 1.0],
                tint: [1.0, 1.0, 1.0],
            });
            x += s;
        }
        y += s;
    }
}

// ------------------------------------------------------------------ icons
pub const TSU: f32 = 1.0 / ATLAS_COLS as f32;
pub const TSV: f32 = 1.0 / ATLAS_ROWS as f32;

pub fn tile_uv(t: u32) -> (f32, f32) {
    (
        (t % ATLAS_COLS) as f32 * TSU,
        (t / ATLAS_COLS) as f32 * TSV,
    )
}

fn tex_tile(p: &mut Painter, tile: u32, x0: f32, y0: f32, x1: f32, y1: f32, shade: f32) {
    let (u, v) = tile_uv(tile);
    let tint = icon_tint(tile);
    p.tex(TexOp {
        xy: [[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
        uv: [[u, v], [u + TSU, v], [u + TSU, v + TSV], [u, v + TSV]],
        shade: [shade, shade, shade, 1.0],
        tint,
    });
}

/// Isometric block icon (3 affine parallelograms) like the vanilla hotbar.
/// Flat sprite for cross plants / torches / items.
pub fn block_icon(p: &mut Painter, b: u16, cx: f32, cy: f32, size: f32) {
    let tiles = tiles_of(b);
    if is_cross_id(b) || b == crate::world::TORCH || b == crate::world::MEAT {
        let half = size * 0.5;
        tex_tile(p, tiles[1], cx - half, cy - half, cx + half, cy + half, 1.0);
        return;
    }
    let (top, side) = (tiles[0], tiles[1]);
    let h = size * 0.5; // half width
    let v = size * 0.5; // side height
    let n = [cx, cy - h * 0.5 - v * 0.5];
    let e = [cx + h, cy - v * 0.5];
    let s_ = [cx, cy + h * 0.5 - v * 0.5];
    let w = [cx - h, cy - v * 0.5];
    let (u0, v0) = tile_uv(top);
    // top face: parallelogram N,E,S,W with affine uv
    p.tex(TexOp {
        xy: [n, e, s_, w],
        uv: [[u0, v0], [u0 + TSU, v0], [u0 + TSU, v0 + TSV], [u0, v0 + TSV]],
        shade: [1.0; 4],
        tint: icon_tint(top),
    });
    // left + right faces
    let (su, sv) = tile_uv(side);
    let tint = icon_tint(side);
    let left = {
        let xy = [w, s_, [s_[0], s_[1] + v], [w[0], w[1] + v]];
        let uv = [[su, sv], [su + TSU, sv], [su + TSU, sv + TSV], [su, sv + TSV]];
        TexOp { xy, uv, shade: [0.62, 0.62, 0.62, 1.0], tint }
    };
    let right = {
        let xy = [s_, e, [e[0], e[1] + v], [s_[0], s_[1] + v]];
        let uv = [[su, sv], [su + TSU, sv], [su + TSU, sv + TSV], [su, sv + TSV]];
        TexOp { xy, uv, shade: [0.8, 0.8, 0.8, 1.0], tint }
    };
    p.tex(left);
    p.tex(right);
}

// ------------------------------------------------------------------- HUD
pub struct HudData {
    pub hotbar: [u16; 9],
    pub slot: usize,
    pub hp: i32,
    pub hunger: i32,
    pub air: i32,
    pub xp: f32,
    pub level: i32,
    pub dead: bool,
    pub creative: bool,
    pub item_name_t: f32,
    pub chat: Vec<(f64, String)>,
    pub chat_input: Option<String>,
    pub f3: Option<Vec<String>>,
    pub tab: Option<Vec<(String, u32)>>,
    pub swing_t: f32,
    pub hurt_shake: f32,
}

const HEART: [u8; 7] = [0b0110110, 0b1111111, 0b1111111, 0b1111111, 0b0111110, 0b0011100, 0b0001000];
const HUNGER: [u8; 7] = [0b0011110, 0b0111111, 0b1111110, 0b1111100, 0b0111000, 0b1101100, 0b0110110];
const BUBBLE: [u8; 7] = [0b0011100, 0b0111110, 0b1101111, 0b1011111, 0b1111111, 0b0111110, 0b0011100];

fn sprite(p: &mut Painter, bmp: &[u8; 7], x0: f32, y0: f32, s: f32, col: [f32; 4], max_col: i32) {
    for (row, &bits) in bmp.iter().enumerate() {
        let mut x = 0i32;
        while x < 7 {
            if (bits >> (6 - x)) & 1 == 1 && x < max_col {
                let mut run = 1;
                while x + run < 7 && run + x < max_col && (bits >> (6 - x - run)) & 1 == 1 {
                    run += 1;
                }
                p.rect(
                    x0 + x as f32 * s,
                    y0 + row as f32 * s,
                    x0 + (x + run) as f32 * s,
                    y0 + (row + 1) as f32 * s,
                    col,
                );
                x += run;
            } else {
                x += 1;
            }
        }
    }
}

/// The full in-game HUD (hotbar, bars, crosshair, chat, F3, tab list...).
pub fn draw_hud(p: &mut Painter, hud: &HudData, gw: f32, gh: f32, time: f64) {
    let slot_pitch = 20.0;
    let bar_w = 9.0 * slot_pitch + 2.0;
    let bx0 = (gw - bar_w) * 0.5;
    let by0 = gh - 24.0;

    // ---- hotbar (182x22 style): dark translucent with slot separators
    p.rect(bx0, by0, bx0 + bar_w, by0 + 22.0, [0.08, 0.08, 0.08, 0.65]);
    p.rect_outline(bx0, by0, bx0 + bar_w, by0 + 22.0, 1.0, [0.25, 0.25, 0.25, 0.8]);
    for i in 1..9 {
        let x = bx0 + i as f32 * slot_pitch + 1.0;
        p.rect(x, by0 + 1.0, x + 1.0, by0 + 21.0, [0.22, 0.22, 0.22, 0.7]);
    }
    // selection frame: 24x24 white outline around the active slot
    let sx = bx0 + 1.0 + hud.slot as f32 * slot_pitch - 2.0;
    p.rect_outline(sx, by0 - 2.0, sx + 24.0, by0 + 24.0, 2.0, [0.95, 0.95, 0.95, 0.95]);

    // icons
    for (i, &b) in hud.hotbar.iter().enumerate() {
        if b == 0 {
            continue;
        }
        block_icon(p, b, bx0 + 1.0 + i as f32 * slot_pitch + 10.0, by0 + 11.0, 15.0);
    }

    // ---- experience bar + level
    let xw = bar_w - 2.0;
    p.rect(bx0 + 1.0, by0 - 5.0, bx0 + 1.0 + xw, by0 - 2.0, [0.05, 0.05, 0.05, 0.8]);
    if hud.xp > 0.0 {
        p.rect(
            bx0 + 1.0,
            by0 - 5.0,
            bx0 + 1.0 + xw * hud.xp.clamp(0.0, 1.0),
            by0 - 2.0,
            [0.35, 0.85, 0.3, 0.95],
        );
    }
    if hud.level > 0 {
        let s = hud.level.to_string();
        let w = text_w(&s);
        text(p, &s, bx0 + bar_w * 0.5 - w * 0.5 + 1.0, by0 - 13.0, [0.1, 0.35, 0.1, 1.0], true);
        text(p, &s, bx0 + bar_w * 0.5 - w * 0.5, by0 - 14.0, [0.5, 1.0, 0.4, 1.0], true);
    }

    // ---- hearts (left) / hunger (right)
    if !hud.creative {
        for i in 0..10 {
            let hx = bx0 + 1.0 + i as f32 * 8.0;
            let hy = by0 - 15.0;
            sprite(p, &HEART, hx, hy, 1.0, [0.1, 0.02, 0.03, 0.85], 7);
            let fill = (hud.hp - i * 2).clamp(0, 2);
            if fill > 0 && !hud.dead {
                sprite(p, &HEART, hx, hy, 1.0, [0.85, 0.1, 0.12, 0.95], if fill == 2 { 7 } else { 4 });
            }
        }
        for i in 0..10 {
            let hx = bx0 + bar_w - 8.0 - i as f32 * 8.0;
            let hy = by0 - 15.0;
            let fill = (hud.hunger - i * 2).clamp(0, 2);
            if fill > 0 && !hud.dead {
                sprite(p, &HUNGER, hx, hy, 1.0, [0.72, 0.45, 0.15, 0.95], if fill == 2 { 7 } else { 4 });
            }
        }
    }

    // ---- air bubbles above hunger when submerged
    if hud.air < 10 {
        for i in 0..10 {
            if i >= hud.air {
                break;
            }
            let hx = bx0 + bar_w - 8.0 - i as f32 * 8.0;
            sprite(p, &BUBBLE, hx, by0 - 25.0, 1.0, [0.55, 0.8, 1.0, 0.95], 7);
        }
    }

    // ---- crosshair
    if hud.chat_input.is_none() {
        let (cx, cy) = (gw * 0.5, gh * 0.5);
        p.rect(cx - 4.5, cy - 0.5, cx + 4.5, cy + 0.5, [0.9, 0.9, 0.9, 0.8]);
        p.rect(cx - 0.5, cy - 4.5, cx + 0.5, cy + 4.5, [0.9, 0.9, 0.9, 0.8]);
    }

    // ---- held item (bottom right) with swing animation
    let held = hud.hotbar.get(hud.slot).copied().unwrap_or(0);
    if held != 0 {
        let sw = if hud.swing_t >= 0.0 {
            let t = hud.swing_t;
            ((t * std::f32::consts::PI).sin().abs()) * 14.0
        } else {
            0.0
        };
        let bob = (time * 2.4).sin() as f32 * 0.8;
        block_icon(p, held, gw - 24.0 + sw, gh - 24.0 - sw * 0.6 + bob, 34.0);
    }

    // ---- item name popup above the hotbar
    if hud.item_name_t > 0.0 {
        let name = crate::world::block_name(held);
        let a = (hud.item_name_t / 1.0).clamp(0.0, 1.0);
        let mut col = YELLOW;
        col[3] = a;
        center_text(p, &name, gw * 0.5, by0 - 30.0, col, true);
    }

    // ---- chat
    draw_chat(p, hud, gw, gh, time);

    // ---- tab list
    if let Some(players) = &hud.tab {
        let rows = players.len() + 1;
        let w = 160.0;
        let h = rows as f32 * 12.0 + 8.0;
        let x0 = (gw - w) * 0.5;
        let y0 = 8.0;
        p.rect(x0, y0, x0 + w, y0 + h, [0.05, 0.05, 0.08, 0.6]);
        p.rect_outline(x0, y0, x0 + w, y0 + h, 1.0, [0.5, 0.5, 0.55, 0.8]);
        center_text(p, "en ligne", gw * 0.5, y0 + 4.0, GRAY, true);
        for (i, (name, ping)) in players.iter().enumerate() {
            let y = y0 + 16.0 + i as f32 * 12.0;
            text(p, name, x0 + 8.0, y, WHITE, true);
            let bars = if *ping < 50 {
                5
            } else if *ping < 120 {
                4
            } else if *ping < 250 {
                3
            } else if *ping < 500 {
                2
            } else {
                1
            };
            for b in 0..5 {
                let col = if b < bars {
                    [0.3 + 0.14 * b as f32, 0.9, 0.3, 0.95]
                } else {
                    [0.3, 0.3, 0.3, 0.6]
                };
                p.rect(x0 + w - 26.0 + b as f32 * 4.0, y + 5.0 - b as f32, x0 + w - 24.0 + b as f32 * 4.0, y + 7.0, col);
            }
        }
    }

    // ---- F3 debug overlay
    if let Some(lines) = &hud.f3 {
        for (i, l) in lines.iter().enumerate() {
            let y = 2.0 + i as f32 * 10.0;
            let w = text_w(l) + 3.0;
            p.rect(1.0, y, 1.0 + w, y + 9.0, [0.08, 0.08, 0.08, 0.5]);
            text(p, l, 2.5, y + 1.0, WHITE, true);
        }
    }
}

fn draw_chat(p: &mut Painter, hud: &HudData, _gw: f32, gh: f32, time: f64) {
    let mut lines: Vec<(f64, &String)> = hud.chat.iter().map(|(a, s)| (*a, s)).collect();
    let open = hud.chat_input.is_some();
    // vanilla stacking above the hotbar: hotbar -> xp -> hearts/air -> chat.
    // by0 = hotbar top; hearts occupy by0-15..by0-8, air by0-25..by0-18.
    let by0 = gh - 24.0;
    let anchor = by0 - 26.0; // bottom chat line, clears the health/air rows
    let mut y = if open { anchor - 11.0 } else { anchor };
    lines.retain(|(age, _)| open || *age < 10.0);
    let start = lines.len().saturating_sub(if open { 18 } else { 10 });
    for (age, s) in lines.iter().skip(start).rev() {
        let a = if open { 1.0 } else { ((10.0 - *age) / 2.0).clamp(0.0, 1.0) as f32 };
        let w = text_w(s) + 4.0;
        p.rect(2.0, y - 1.0, 2.0 + w, y + 9.0, [0.0, 0.0, 0.0, 0.35 * a]);
        let mut x = 4.0;
        for (part, col) in parse_colors(s) {
            let mut c = col;
            c[3] = a;
            x += text(p, &part, x, y, c, true);
        }
        y -= 10.0;
        let _ = time;
    }
    if let Some(input) = &hud.chat_input {
        let y = anchor;
        let cursor = if (time * 3.0) as i64 % 2 == 0 { "_" } else { " " };
        let s = format!("{}{}", input, cursor);
        let w = text_w(&s) + 6.0;
        p.rect(2.0, y - 1.0, 2.0 + w.max(120.0), y + 9.0, [0.0, 0.0, 0.0, 0.7]);
        text(p, &s, 4.0, y, WHITE, true);
    }
}

// ------------------------------------------------------------- menu pieces
pub fn title(p: &mut Painter, gw: f32, splash: &str, time: f64) {
    let t = "RUSTVOXEL";
    let scale = 3.0;
    // chunky shadowed logo
    let w = text_w(t) * scale;
    let x = gw * 0.5 - w * 0.5;
    let mut cx = x;
    for ch in t.chars() {
        let (rows, gw2) = match font::glyph(ch) {
            Some(g) => g,
            None => {
                cx += 6.0 * scale;
                continue;
            }
        };
        for r in 0..font::ROWS {
            for b in 0..gw2 {
                if rows[r] & (1 << (4 - b)) != 0 {
                    let x0 = cx + b as f32 * scale;
                    let y0 = 34.0 + r as f32 * scale;
                    p.rect(x0 + 2.0, y0 + 2.0, x0 + scale + 2.0, y0 + scale + 2.0, [0.12, 0.12, 0.13, 1.0]);
                    p.rect(x0, y0, x0 + scale, y0 + scale, [0.78, 0.80, 0.82, 1.0]);
                }
            }
        }
        cx += (gw2 + 1) as f32 * scale;
    }
    // splash, gently pulsing
    let pulse = 1.0 + ((time * 2.2).sin() as f32) * 0.06;
    let a = 0.45 + 0.55 * pulse;
    let mut col = YELLOW;
    col[3] = a;
    let sx = gw * 0.5 + w * 0.42;
    let sy = 74.0;
    let sw = text_w(splash);
    p.ops.push(Op::Rect(RectOp { x0: 0.0, y0: 0.0, x1: 0.0, y1: 0.0, col: [0.0; 4] }));
    // rotated-ish splash: draw with a slight slant via per-row offset
    let rows = 9.0;
    for r in 0..rows as i32 {
        let off = (rows as f32 - r as f32) * 0.35;
        let _ = off;
    }
    let _ = (sx, sy, sw);
    text(p, splash, sx, sy, col, true);
}

pub fn version_lines(p: &mut Painter, gw: f32, gh: f32) {
    text(p, &format!("RustVoxel {} (multijoueur)", env!("CARGO_PKG_VERSION")), 2.0, gh - 10.0, GRAY, true);
    let s = "Non affilié à Mojang";
    let w = text_w(s);
    text(p, s, gw - w - 2.0, gh - 10.0, GRAY, true);
}

pub fn panel(p: &mut Painter, x: f32, y: f32, w: f32, h: f32) {
    p.rect(x, y, x + w, y + h, [0.76, 0.76, 0.78, 0.97]);
    p.rect(x, y, x + w, y + 1.5, [0.98, 0.98, 0.98, 1.0]);
    p.rect(x, y, x + 1.5, y + h, [0.98, 0.98, 0.98, 1.0]);
    p.rect(x, y + h - 1.5, x + w, y + h, [0.38, 0.38, 0.42, 1.0]);
    p.rect(x + w - 1.5, y, x + w, y + h, [0.38, 0.38, 0.42, 1.0]);
}

pub fn slot_inset(p: &mut Painter, x: f32, y: f32, s: f32) {
    p.rect(x, y, x + s, y + s, [0.55, 0.55, 0.57, 1.0]);
    p.rect(x + 1.0, y + 1.0, x + s - 1.0, y + s - 1.0, [0.44, 0.44, 0.47, 1.0]);
    p.rect(x + 1.0, y + 1.0, x + s - 1.0, y + 1.5, [0.3, 0.3, 0.33, 1.0]);
    p.rect(x + 1.0, y + 1.0, x + 1.5, y + s - 1.0, [0.3, 0.3, 0.33, 1.0]);
}

// ------------------------------------------------------------ software view
/// Rasterize a painter into RGBA (tests / previews). `scale` mimics the GUI
/// scale (coords are GUI px, pixels are scale*GUI). Pass the real atlas
/// (renderer::build_atlas_pixels_for_test) to see actual textures; `None`
/// falls back to a gray hash per uv.
pub fn rasterize(
    p: &Painter,
    w: usize,
    h: usize,
    atlas: Option<&[u8]>,
    scale: f32,
) -> Vec<u8> {
    let mut buf = vec![0u8; w * h * 4];
    let px = |x: f32, y: f32| -> (usize, usize) { (x.max(0.0) as usize, y.max(0.0) as usize) };
    let sxy = |q: [[f32; 2]; 4]| -> [[f32; 2]; 4] {
        [
            [q[0][0] * scale, q[0][1] * scale],
            [q[1][0] * scale, q[1][1] * scale],
            [q[2][0] * scale, q[2][1] * scale],
            [q[3][0] * scale, q[3][1] * scale],
        ]
    };
    let blend = |dst: &mut [u8], c: [f32; 4]| {
        let a = c[3].clamp(0.0, 1.0);
        for k in 0..3 {
            let v = c[k].clamp(0.0, 1.0) * 255.0;
            dst[k] = (v * a + dst[k] as f32 * (1.0 - a)) as u8;
        }
        dst[3] = ((c[3] * 255.0) as u8).max(dst[3]);
    };
    for op in &p.ops {
        match op {
            Op::Rect(r) => {
                let (x0, y0) = px(r.x0 * scale, r.y0 * scale);
                let (x1, y1) = px((r.x1 * scale).min(w as f32), (r.y1 * scale).min(h as f32));
                for y in y0..y1 {
                    for x in x0..x1 {
                        let i = (y * w + x) * 4;
                        blend(&mut buf[i..i + 4], r.col);
                    }
                }
            }
            Op::Tex(t) => {
                // barycentric-free: scan the bbox, point-in-quad via 4 half
                // plane tests using the two triangle fans (quad assumed convex)
                let q = sxy(t.xy);
                let xs: Vec<f32> = q.iter().map(|c| c[0]).collect();
                let ys: Vec<f32> = q.iter().map(|c| c[1]).collect();
                let (minx, miny) = (xs.iter().cloned().fold(0.0, f32::min).max(0.0) as usize, ys.iter().cloned().fold(0.0, f32::min).max(0.0) as usize);
                let (maxx, maxy) = (
                    (xs.iter().cloned().fold(f32::MAX, f32::max) as usize).min(w),
                    (ys.iter().cloned().fold(f32::MAX, f32::max) as usize).min(h),
                );
                for y in miny..maxy {
                    for x in minx..maxx {
                        if !point_in_quad(q, x as f32 + 0.5, y as f32 + 0.5) {
                            continue;
                        }
                        // bilinear uv over the quad
                        let uv = bilinear_uv(q, t.uv, x as f32 + 0.5, y as f32 + 0.5);
                        let col = sample_tile(atlas, uv, t.shade[0], t.tint);
                        let i = (y * w + x) * 4;
                        blend(&mut buf[i..i + 4], col);
                    }
                }
            }
        }
    }
    buf
}

fn point_in_quad(q: [[f32; 2]; 4], x: f32, y: f32) -> bool {
    // convex quad 0,1,2,3: inside = same orientation for all edges
    let mut signs = [0.0f32; 4];
    for i in 0..4 {
        let a = q[i];
        let b = q[(i + 1) % 4];
        signs[i] = (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
    }
    let pos = signs.iter().all(|s| *s >= -0.001);
    let neg = signs.iter().all(|s| *s <= 0.001);
    pos || neg
}

fn bilinear_uv(q: [[f32; 2]; 4], uv: [[f32; 2]; 4], x: f32, y: f32) -> (f32, f32) {
    // split into triangles 0-1-2 / 0-2-3 and solve affine in the right one
    for (a, b, c, ua, ub, uc) in [
        (0usize, 1usize, 2usize, 0usize, 1usize, 2usize),
        (0usize, 2usize, 3usize, 0usize, 2usize, 3usize),
    ] {
        let (ax, ay) = (q[a][0], q[a][1]);
        let (bx, by) = (q[b][0], q[b][1]);
        let (cx, cy) = (q[c][0], q[c][1]);
        let det = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
        if det.abs() < 1e-9 {
            continue;
        }
        let w1 = ((x - ax) * (cy - ay) - (y - ay) * (cx - ax)) / det;
        let w2 = ((bx - ax) * (y - ay) - (by - ay) * (x - ax)) / det;
        let w0 = 1.0 - w1 - w2;
        if w0 < -0.01 || w1 < -0.01 || w2 < -0.01 {
            continue;
        }
        return (
            ua as f32 * 0.0 + uv[ua][0] * w0 + uv[ub][0] * w1 + uv[uc][0] * w2,
            uv[ua][1] * w0 + uv[ub][1] * w1 + uv[uc][1] * w2,
        );
    }
    (uv[0][0], uv[0][1])
}

fn sample_tile(atlas: Option<&[u8]>, uv: (f32, f32), shade: f32, tint: [f32; 3]) -> [f32; 4] {
    if let Some(a) = atlas {
        const TILE: usize = 16;
        let cols = ATLAS_COLS as usize;
        let rows = ATLAS_ROWS as usize;
        let w = cols * TILE; // atlas width in pixels
        let bpp = a.len() / (rows * TILE * w).max(1); // bytes per pixel (4)
        // pixel inside the tile + which tile -> ROW-MAJOR full-width atlas
        let tu = ((uv.0.fract() * cols as f32) * TILE as f32) as usize % TILE;
        let tv = ((uv.1.fract() * rows as f32) * TILE as f32) as usize % TILE;
        let tx = (uv.0 * cols as f32) as usize % cols;
        let ty = (uv.1 * rows as f32) as usize % rows;
        let x = tx * TILE + tu;
        let y = ty * TILE + tv;
        let px_i = (y * w + x) * bpp;
        if px_i + bpp <= a.len() {
            let r = a[px_i] as f32 / 255.0;
            let g = a[px_i + 1] as f32 / 255.0;
            let b = a[px_i + 2] as f32 / 255.0;
            let alpha = a[px_i + 3] as f32 / 255.0;
            if alpha < 0.1 {
                return [0.0; 4];
            }
            return [r * shade * tint[0], g * shade * tint[1], b * shade * tint[2], alpha];
        }
    }
    // fallback: gray from uv hash
    let v = 0.35 + 0.3 * ((uv.0 * 91.7 + uv.1 * 57.3).sin().abs());
    [v * shade, v * shade, v * shade, 1.0]
}

// -------------------------------------------------------------------- tests
// ces aperçus exigent l'atlas réel de renderer.rs (comme le GPU en jeu)
#[cfg(all(test, any(windows, feature = "fficheck")))]
mod tests {
    use super::*;

    fn save_png(name: &str, w: usize, h: usize, px: &[u8]) {
        let png = crate::pack::png_encode(w as u32, h as u32, px);
        let dir = std::path::Path::new("docs");
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), png).unwrap();
    }

    /// The REAL procedural atlas, same pixels the GPU uploads in-game.
    fn real_atlas() -> Vec<u8> {
        crate::renderer::build_atlas_pixels_for_test(None)
    }

    #[test]
    fn hud_preview_png() {
        let atlas = real_atlas();
        let mut p = Painter::new();
        p.rect(0.0, 0.0, 320.0, 180.0, [0.35, 0.55, 0.75, 1.0]);
        let hud = HudData {
            hotbar: [
                crate::world::GRASS,
                crate::world::TORCH,
                crate::world::CHERRY_PLANKS,
                crate::world::DEEPSLATE,
                crate::world::COPPER_BLOCK,
                crate::world::OAK_FENCE,
                crate::world::SNOW_LAYER,
                crate::world::WOOL_STAIRS_BASE,
                crate::world::MEAT,
            ],
            slot: 1,
            hp: 14,
            hunger: 17,
            air: 7,
            xp: 0.55,
            level: 3,
            dead: false,
            creative: false,
            item_name_t: 1.0,
            chat: vec![
                (1.0, "§eSteve a rejoint la partie".into()),
                (2.0, "Steve: salut, bienvenue sur mon serveur !".into()),
                (3.0, "§cAlex est mort".into()),
            ],
            chat_input: Some("bonjour /".into()),
            f3: Some(vec![
                "RustVoxel 0.6.1 (62 fps)".into(),
                "XYZ: 12.345 / 71.000 / -8.210".into(),
                "Chunk: 0 8 in 0 -1".into(),
                "Facing: nord (nord-ouest) biome: forêt de cerisiers".into(),
            ]),
            tab: Some(vec![("Steve".into(), 23), ("Alex".into(), 87)]),
            swing_t: 0.3,
            hurt_shake: 0.0,
        };
        draw_hud(&mut p, &hud, 320.0, 180.0, 1.23);
        // scale 2 = what a 640x360 window shows with the auto GUI scale
        let px = rasterize(&p, 640, 360, Some(&atlas), 2.0);
        save_png("ui_hud_preview.png", 640, 360, &px);
        // sanity: hotbar area must have dark pixels
        let i = (360 - 10) * 640 * 4 + 320 * 4;
        assert!(px[i] < 120, "hotbar bg should darken the bottom");

        // REGRESSION: two different slots must show DIFFERENT textures
        // (icon centers in GUI px: x = 80 + i*20, y = 167)
        let at = |gx: f32, gy: f32| -> [u8; 3] {
            let (x, y) = ((gx * 2.0) as usize, (gy * 2.0) as usize);
            let i = (y * 640 + x) * 4;
            [px[i], px[i + 1], px[i + 2]]
        };
        let grass_icon = at(80.0, 167.0);
        let deep_icon = at(140.0, 167.0);
        let d = (0..3)
            .map(|k| (grass_icon[k] as i32 - deep_icon[k] as i32).abs())
            .sum::<i32>();
        assert!(
            d > 40,
            "hotbar icons must differ, got {:?} vs {:?}",
            grass_icon,
            deep_icon
        );

        // REGRESSION: chat must NOT overlap the hearts row
        // hearts top at GUI y=141 (canvas 282); chat bg ends at 139*2=278
        let gap = at(50.0, 145.0); // left of hearts, right below chat
        assert!(gap[1] > 110, "chat overlaps the health row (px {:?})", gap);
    }

    #[test]
    fn menu_preview_png() {
        let atlas = real_atlas();
        let mut p = Painter::new();
        dirt_bg(&mut p, crate::world::T_DIRT, 320.0, 180.0);
        title(&mut p, 320.0, "100% logiciel !", 0.8);
        button(&mut p, "Jouer en solo", 100.0, 90.0, 120.0, 16.0, false);
        button(&mut p, "Multijoueur", 100.0, 110.0, 120.0, 16.0, true);
        button(&mut p, "Options...", 100.0, 130.0, 120.0, 16.0, false);
        button(&mut p, "Quitter", 100.0, 150.0, 120.0, 16.0, false);
        version_lines(&mut p, 320.0, 180.0);
        let px = rasterize(&p, 640, 360, Some(&atlas), 2.0);
        save_png("ui_menu_preview.png", 640, 360, &px);
        assert!(!px.is_empty());
    }

    #[test]
    fn inventory_preview_png() {
        let atlas = real_atlas();
        let mut p = Painter::new();
        dirt_bg(&mut p, crate::world::T_DIRT, 320.0, 180.0);
        let w = 200.0;
        let h = 150.0;
        let x0 = (320.0 - w) * 0.5;
        let y0 = (180.0 - h) * 0.5;
        panel(&mut p, x0, y0, w, h);
        center_text(&mut p, "Inventaire", 160.0, y0 + 6.0, [0.25, 0.25, 0.3, 1.0], false);
        for i in 0..18u16 {
            let col = i % 9;
            let row = i / 9;
            let sx = x0 + 8.0 + col as f32 * 20.0;
            let sy = y0 + 18.0 + row as f32 * 20.0;
            slot_inset(&mut p, sx, sy, 18.0);
            block_icon(
                &mut p,
                crate::world::PLACEABLE[i as usize],
                sx + 9.0,
                sy + 9.0,
                13.0,
            );
        }
        // hotbar row with distinct items (like in-game)
        let hb = [
            crate::world::GRASS,
            crate::world::STONE,
            crate::world::LOG,
            crate::world::TORCH,
            crate::world::CHERRY_PLANKS,
            crate::world::GLASS,
            crate::world::WOOL_COLOR_BASE + 14, // laine rouge
            crate::world::SNOW_LAYER,
            crate::world::MEAT,
        ];
        for (i, &b) in hb.iter().enumerate() {
            let sx = x0 + 8.0 + i as f32 * 20.0;
            let sy = y0 + h - 26.0;
            slot_inset(&mut p, sx, sy, 18.0);
            block_icon(&mut p, b, sx + 9.0, sy + 9.0, 13.0);
        }
        let px = rasterize(&p, 640, 360, Some(&atlas), 2.0);
        save_png("ui_inventory_preview.png", 640, 360, &px);

        // REGRESSION: icons must not all be identical. Average each main-grid
        // icon cell and count distinct quantized colors.
        let mut colors: Vec<[i32; 3]> = Vec::new();
        for i in 0..18u16 {
            let col = i % 9;
            let row = i / 9;
            let cx = ((x0 + 8.0 + col as f32 * 20.0 + 9.0) * 2.0) as usize;
            let cy = ((y0 + 18.0 + row as f32 * 20.0 + 9.0) * 2.0) as usize;
            let (mut r, mut g, mut b, mut n) = (0i32, 0i32, 0i32, 0i32);
            for dy in -4..4isize {
                for dx in -4..4isize {
                    let o = ((cy as isize + dy) * 640 + cx as isize + dx) as usize * 4;
                    r += px[o] as i32;
                    g += px[o + 1] as i32;
                    b += px[o + 2] as i32;
                    n += 1;
                }
            }
            colors.push([r / n / 48, g / n / 48, b / n / 48]);
        }
        colors.sort();
        colors.dedup();
        assert!(
            colors.len() >= 10,
            "only {} distinct icons in the inventory grid (all identical?)",
            colors.len()
        );
    }

    #[test]
    fn parse_colors_splits_sections() {
        let runs = parse_colors("§ea§f b");
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].0, "a");
        assert_eq!(runs[1].1, WHITE);
    }

}
