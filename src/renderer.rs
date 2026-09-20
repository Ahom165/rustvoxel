// OpenGL renderer: procedural texture atlas, shaders, chunk mesh upload/draw.
// v0.3 "Wilderness Bound": 8x16 atlas + texture-pack overrides (vanilla packs).
use crate::gl::{self, Gl};
use crate::math::{mat_mul, ortho_pixels, perspective, view_matrix, Mat4, Vec3};
use crate::mesher::CORNERS;
use crate::pack::{tile_candidates, tile_tint, Pack};
use crate::player::Player;
use crate::raycast::RayHit;
use std::collections::HashMap;

pub use crate::world::{ATLAS_COLS, ATLAS_ROWS};
const TILE: u32 = 16;

// Tile indices live in world.rs (single source of truth, always compiled).
pub use crate::world::{T_MOON, T_SUN};

pub struct ChunkMesh {
    pub vbo: u32,
    pub ibo: u32,
    pub n: usize,
    pub cvbo: u32,
    pub cibo: u32,
    pub cn: usize,
    pub wvbo: u32,
    pub wibo: u32,
    pub wn: usize,
}

impl ChunkMesh {
    pub fn empty() -> ChunkMesh {
        ChunkMesh {
            vbo: 0,
            ibo: 0,
            n: 0,
            cvbo: 0,
            cibo: 0,
            cn: 0,
            wvbo: 0,
            wibo: 0,
            wn: 0,
        }
    }
}

// ------------------------------------------------------------------ shaders
const VS_MAIN: &[u8] = b"attribute vec3 a_pos;
attribute vec2 a_uv;
attribute vec3 a_rgb;
uniform mat4 u_mvp;
uniform vec3 u_cam;
uniform float u_fog_near;
uniform float u_fog_far;
varying vec2 v_uv;
varying vec3 v_rgb;
varying float v_fog;
void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
    v_uv = a_uv;
    v_rgb = a_rgb;
    float d = distance(a_pos, u_cam);
    v_fog = clamp((u_fog_far - d) / max(u_fog_far - u_fog_near, 0.001), 0.0, 1.0);
}\0";

const FS_MAIN: &[u8] = b"uniform sampler2D u_tex;
uniform vec3 u_fog_col;
uniform float u_alpha;
uniform float u_light;
varying vec2 v_uv;
varying vec3 v_rgb;
varying float v_fog;
void main() {
    vec4 t = texture2D(u_tex, v_uv);
    if (t.a < 0.02) discard;
    vec3 col = t.rgb * v_rgb * u_light;
    gl_FragColor = vec4(mix(u_fog_col, col, v_fog), t.a * u_alpha);
}\0";

const VS_COLOR: &[u8] = b"attribute vec3 a_pos;
uniform mat4 u_mvp;
uniform vec3 u_off;
void main() {
    gl_Position = u_mvp * vec4(a_pos + u_off, 1.0);
}\0";

const FS_COLOR: &[u8] = b"uniform vec4 u_color;
void main() {
    gl_FragColor = u_color;
}\0";

const VS_POINT: &[u8] = b"attribute vec3 a_pos;
attribute vec4 a_col;
uniform mat4 u_mvp;
uniform float u_psize;
varying vec4 v_col;
void main() {
    gl_Position = u_mvp * vec4(a_pos, 1.0);
    gl_PointSize = u_psize;
    v_col = a_col;
}\0";

const FS_POINT: &[u8] = b"varying vec4 v_col;
void main() {
    gl_FragColor = v_col;
}\0";

// ------------------------------------------------------- procedural textures
fn hx(x: i64, y: i64, t: i64) -> f32 {
    crate::noise::hash01(crate::noise::hash3i(x, y, t, 0x5EED_C0DE))
}

fn spk(base: (f32, f32, f32), amt: f32, n: f32) -> (f32, f32, f32) {
    let d = (n - 0.5) * 2.0 * amt;
    (base.0 + d, base.1 + d, base.2 + d)
}

// ---- v0.4 texture families -------------------------------------------------
/// Generic mottled stone: soft cells + a few mineral patches.
fn stone_fam(base: (f32, f32, f32), amt: f32, t: i64, x: u32, y: u32, n: f32) -> (f32, f32, f32) {
    let cell = hx((x / 3) as i64, (y / 3) as i64, t);
    if cell > 0.88 {
        spk((base.0 * 0.84, base.1 * 0.84, base.2 * 0.84), amt, n)
    } else if cell < 0.09 {
        spk((base.0 * 1.10, base.1 * 1.10, base.2 * 1.10), amt, n)
    } else {
        spk(base, amt, n)
    }
}

/// Log side: bark ridges, vertical grain and the occasional knot.
fn log_side(base: (f32, f32, f32), dark: (f32, f32, f32), t: i64, x: u32, y: u32, n: f32) -> (f32, f32, f32) {
    let ridge = (x % 6) < 2;
    let knot = hx((x / 4) as i64, (y / 4) as i64, t) > 0.94 && (x % 4) < 2 && (y % 5) < 3;
    if knot {
        spk((dark.0 * 0.7, dark.1 * 0.7, dark.2 * 0.7), 4.0, n)
    } else if ridge {
        spk(dark, 7.0, n)
    } else {
        let g = hx(x as i64, (y / 6) as i64, t + 1) * 9.0 - 4.5;
        (base.0 + g, base.1 + g, base.2 + g)
    }
}

/// Log top: growth rings with a bark border.
fn log_top(ring: (f32, f32, f32), bark: (f32, f32, f32), x: u32, y: u32, n: f32) -> (f32, f32, f32) {
    let dx = x as f32 - 7.5;
    let dy = y as f32 - 7.5;
    let d = (dx * dx + dy * dy).sqrt();
    if d > 7.1 {
        spk(bark, 6.0, n)
    } else if (d as i32) % 3 == 0 {
        spk(ring, 5.0, n)
    } else {
        spk(ring, 9.0, n)
    }
}

/// Planks: rows with seams and subtle grain.
fn planks(base: (f32, f32, f32), seam: (f32, f32, f32), t: i64, x: u32, y: u32, n: f32) -> (f32, f32, f32) {
    let row = y / 4;
    let vseam = x as i64 == ((row as i64 * 5 + 3) % 16);
    if y % 4 == 0 || vseam {
        spk(seam, 4.0, n)
    } else {
        let g = hx((x / 7) as i64, row as i64, t) * 10.0 - 5.0;
        (base.0 + g, base.1 + g, base.2 + g)
    }
}

/// Ore over a stone host.
fn ore(stone: (f32, f32, f32), gem: (f32, f32, f32), t: i64, x: u32, y: u32, n: f32) -> (f32, f32, f32) {
    let c = hx((x / 2) as i64, (y / 2) as i64, t);
    if c > 0.80 {
        spk(gem, 9.0, n)
    } else if c > 0.74 {
        spk((gem.0 * 0.78, gem.1 * 0.78, gem.2 * 0.78), 8.0, n)
    } else {
        spk(stone, 9.0, n)
    }
}

/// Dense leaf block: two-tone clumps + see-through holes.
fn leaves(base: (f32, f32, f32), dark: (f32, f32, f32), t: i64, x: u32, y: u32, n: f32) -> (f32, f32, f32, f32) {
    let c = hx((x / 2) as i64, (y / 2) as i64, t);
    if c > 0.94 {
        (0.0, 0.0, 0.0, 0.0)
    } else if c > 0.60 {
        let (r, g, b) = spk(base, 9.0, n);
        (r, g, b, 255.0)
    } else {
        let (r, g, b) = spk(dark, 8.0, n);
        (r, g, b, 255.0)
    }
}

/// Dense leaf block, GRAYSCALE version: same clump pattern as `leaves` but
/// the two tones are luminance levels - the biome foliage color comes from
/// the mesh tint (matches how vanilla ships its leaves textures).
fn leaves_gray(base: f32, dark: f32, t: i64, x: u32, y: u32, n: f32) -> (f32, f32, f32, f32) {
    let c = hx((x / 2) as i64, (y / 2) as i64, t);
    if c > 0.94 {
        (0.0, 0.0, 0.0, 0.0)
    } else if c > 0.60 {
        let l = spk((base, base, base), 9.0, n).0;
        (l, l, l, 255.0)
    } else {
        let l = spk((dark, dark, dark), 8.0, n).0;
        (l, l, l, 255.0)
    }
}

/// Dye colors (vanilla order white..black) for wool/concrete/cushions.
const DYE_RGB: [(f32, f32, f32); 16] = [
    (233.0, 236.0, 236.0),
    (241.0, 118.0, 35.0),
    (193.0, 67.0, 178.0),
    (61.0, 118.0, 213.0),
    (251.0, 199.0, 33.0),
    (93.0, 191.0, 28.0),
    (241.0, 148.0, 166.0),
    (63.0, 68.0, 72.0),
    (145.0, 143.0, 143.0),
    (22.0, 135.0, 153.0),
    (122.0, 60.0, 160.0),
    (54.0, 60.0, 153.0),
    (114.0, 71.0, 40.0),
    (85.0, 110.0, 27.0),
    (162.0, 44.0, 41.0),
    (21.0, 22.0, 26.0),
];

/// Build the raw atlas pixels: texture-pack override if available (with a
/// grayscale tint for vanilla grass/leaves), else the procedural generator.
/// The buffer is ROW-MAJOR (one big ATLAS_COLS*16 x ATLAS_ROWS*16 image),
/// matching the tex_image_2d upload and the mesher UV math.
pub fn build_atlas_pixels(pack: Option<&Pack>) -> Vec<u8> {
    let n_tiles = ATLAS_COLS * ATLAS_ROWS;
    let w = (ATLAS_COLS * TILE) as usize;
    let mut px = vec![0u8; (n_tiles * TILE * TILE * 4) as usize];
    for t in 0..n_tiles {
        let tx = ((t % ATLAS_COLS) * TILE) as usize;
        let ty = ((t / ATLAS_COLS) * TILE) as usize;
        let mut drawn = false;
        if let Some(p) = pack {
            let cands = tile_candidates(t);
            if !cands.is_empty() {
                if let Some(img) = p.get(&cands) {
                    let tint = tile_tint(t);
                    for y in 0..TILE as usize {
                        for x in 0..TILE as usize {
                            let s = (y * TILE as usize + x) * 4;
                            let d = ((ty + y) * w + tx + x) * 4;
                            let (mut r, mut g, mut b) = (
                                img.px[s] as f32,
                                img.px[s + 1] as f32,
                                img.px[s + 2] as f32,
                            );
                            if img.is_grayscale(6) {
                                r *= tint[0];
                                g *= tint[1];
                                b *= tint[2];
                            }
                            px[d] = r.clamp(0.0, 255.0) as u8;
                            px[d + 1] = g.clamp(0.0, 255.0) as u8;
                            px[d + 2] = b.clamp(0.0, 255.0) as u8;
                            px[d + 3] = img.px[s + 3];
                        }
                    }
                    drawn = true;
                }
            }
        }
        if !drawn {
            let mut tmp = Vec::with_capacity((TILE * TILE * 4) as usize);
            tile_pixels(t, &mut tmp);
            for y in 0..TILE as usize {
                let srow = y * TILE as usize * 4;
                let drow = ((ty + y) * w + tx) * 4;
                px[drow..drow + TILE as usize * 4]
                    .copy_from_slice(&tmp[srow..srow + TILE as usize * 4]);
            }
        }
    }
    px
}

/// Test hook for pack.rs end-to-end tests (atlas built with a pack applied).
#[cfg(test)]
pub(crate) fn build_atlas_pixels_for_test(pack: Option<&Pack>) -> Vec<u8> {
    build_atlas_pixels(pack)
}

fn compute_tile_avg(px: &[u8]) -> Vec<[f32; 3]> {
    let n = (ATLAS_COLS * ATLAS_ROWS) as usize;
    let w = (ATLAS_COLS * TILE) as usize;
    let mut avg = Vec::with_capacity(n);
    for t in 0..n {
        let tx = ((t as u32 % ATLAS_COLS) * TILE) as usize;
        let ty = ((t as u32 / ATLAS_COLS) * TILE) as usize;
        let (mut sr, mut sg, mut sb, mut cnt) = (0f32, 0f32, 0f32, 0f32);
        for y in 0..TILE as usize {
            for x in 0..TILE as usize {
                let o = ((ty + y) * w + tx + x) * 4;
                if px[o + 3] > 40 {
                    sr += px[o] as f32;
                    sg += px[o + 1] as f32;
                    sb += px[o + 2] as f32;
                    cnt += 1.0;
                }
            }
        }
        // biome-tinted tiles are stored grayscale: re-apply their fixed
        // color so break particles keep a plausible hue
        let it = crate::world::icon_tint(t as u32);
        if cnt < 1.0 {
            avg.push([1.0, 0.0, 1.0]);
        } else {
            avg.push([
                sr / cnt / 255.0 * it[0],
                sg / cnt / 255.0 * it[1],
                sb / cnt / 255.0 * it[2],
            ]);
        }
    }
    avg
}

#[allow(unused_assignments)]
fn tile_pixels(t: u32, out: &mut Vec<u8>) {
    for y in 0..TILE {
        for x in 0..TILE {
            let n = hx(x as i64, y as i64, t as i64);
            let (mut r, mut g, mut b, mut a) = (255f32, 0f32, 255f32, 255f32);
            match t {
                0 => {
                    // grass top: GRAYSCALE (biome color comes from the mesh)
                    let clump = crate::noise::value_noise_2d(x as f32 * 0.55, y as f32 * 0.55, 404);
                    let l = 148.0 + clump * 62.0 + (n - 0.5) * 24.0;
                    (r, g, b) = (l, l, l * 0.97);
                }
                2 => {
                    // dirt
                    (r, g, b) = if n < 0.08 {
                        (110.0, 76.0, 50.0)
                    } else if n > 0.92 {
                        (150.0, 110.0, 78.0)
                    } else {
                        spk((134.0, 96.0, 66.0), 14.0, n)
                    };
                }
                1 => {
                    // grass side: dirt with a ragged green fringe
                    let green = y < 3 || (y == 3 && n < 0.55) || (y == 4 && n < 0.15);
                    if green {
                        (r, g, b) = spk((98.0, 160.0, 66.0), 12.0, n);
                    } else {
                        (r, g, b) = spk((134.0, 96.0, 66.0), 14.0, n);
                    }
                }
                3 => {
                    // stone: speckle + fine cracks
                    let crack = ((x + y * 2) % 13 == 0 && n > 0.35) || ((x * 2 + y) % 17 == 3 && n < 0.3);
                    (r, g, b) = if crack {
                        (98.0, 98.0, 100.0)
                    } else if n < 0.06 {
                        (108.0, 108.0, 110.0)
                    } else {
                        spk((128.0, 128.0, 130.0), 9.0, n)
                    };
                }
                4 => {
                    // cobble: value-noise cells with dark mortar
                    let vn = crate::noise::value_noise_2d(x as f32 * 0.55, y as f32 * 0.55, 999);
                    let v = 100.0 + vn * 55.0;
                    if vn > 0.48 && vn < 0.52 {
                        (r, g, b) = (78.0, 78.0, 78.0);
                    } else {
                        (r, g, b) = spk((v, v, v), 6.0, n);
                    }
                }
                5 => {
                    // sand: wind ripples
                    let rip = (y as f32 * 1.15 + (x as f32 * 0.35).sin() * 1.6).sin() > 0.55;
                    (r, g, b) = if rip {
                        spk((208.0, 194.0, 146.0), 6.0, n)
                    } else if n > 0.93 {
                        (200.0, 186.0, 138.0)
                    } else {
                        spk((221.0, 209.0, 162.0), 7.0, n)
                    };
                }
                6 => {
                    // log side: vertical bark stripes + knots
                    let m = x % 4;
                    let knot = (x == 4 || x == 5) && (y == 6 || y == 7) && n > 0.5;
                    (r, g, b) = if knot {
                        (72.0, 56.0, 34.0)
                    } else if m == 0 || (m == 3 && n < 0.5) {
                        spk((88.0, 68.0, 42.0), 6.0, n)
                    } else if m == 2 {
                        spk((118.0, 93.0, 60.0), 6.0, n)
                    } else {
                        spk((107.0, 84.0, 53.0), 6.0, n)
                    };
                }
                7 => {
                    // log top: rings
                    let d = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs()) as i32;
                    (r, g, b) = if d >= 7 {
                        spk((107.0, 84.0, 53.0), 6.0, n)
                    } else if d % 2 == 0 {
                        spk((176.0, 143.0, 91.0), 6.0, n)
                    } else {
                        spk((146.0, 115.0, 70.0), 6.0, n)
                    };
                }
                8 => {
                    // leaves: GRAYSCALE foliage pattern with dark holes
                    let hole = hx((x / 2) as i64, (y / 2) as i64, 8) ;
                    let l = if hole > 0.86 {
                        46.0
                    } else if n < 0.18 {
                        66.0
                    } else if n > 0.90 {
                        132.0
                    } else {
                        92.0 + (n - 0.5) * 30.0
                    };
                    (r, g, b) = (l, l, l);
                }
                9 => {
                    // planks
                    let row = y / 4;
                    (r, g, b) = if y % 4 == 3 || x == (row * 5 + 3) % 16 {
                        (130.0, 102.0, 62.0)
                    } else if n > 0.85 {
                        (158.0, 126.0, 78.0)
                    } else {
                        spk((176.0, 140.0, 88.0), 6.0, n)
                    };
                }
                10 => {
                    // water: GRAYSCALE wave bands (biome color from the mesh)
                    let wave = (x as f32 * 0.7 + y as f32 * 2.1).sin() > 0.6;
                    let l = if wave { 186.0 } else { 158.0 } + (n - 0.5) * 14.0;
                    (r, g, b) = (l, l, l);
                    a = 178.0;
                }
                11 => {
                    // glass: cutout interior + frame
                    let border = x == 0 || x == 15 || y == 0 || y == 15;
                    if border {
                        (r, g, b, a) = (215.0, 235.0, 245.0, 255.0);
                    } else if n > 0.93 {
                        (r, g, b, a) = (255.0, 255.0, 255.0, 255.0);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                12 => {
                    // bricks
                    let row = y / 4;
                    let mortar = y % 4 == 0 || (x + (row % 2) * 4) % 8 == 0;
                    (r, g, b) = if mortar { (182.0, 182.0, 182.0) } else { spk((156.0, 72.0, 58.0), 8.0, n) };
                }
                13 => {
                    // snow
                    (r, g, b) = if n < 0.06 { (222.0, 228.0, 238.0) } else { spk((240.0, 244.0, 250.0), 5.0, n) };
                }
                14 => {
                    // bedrock (chunky)
                    let c = hx((x / 2) as i64, (y / 2) as i64, 777);
                    let v = 45.0 + c * 50.0;
                    (r, g, b) = spk((v, v, v), 6.0, n);
                }
                15 => {
                    // coal ore
                    let blob = hx((x / 2) as i64, (y / 2) as i64, 55);
                    (r, g, b) = if blob > 0.78 { (38.0, 38.0, 40.0) } else { spk((127.0, 127.0, 127.0), 9.0, n) };
                }
                16 => {
                    // iron ore
                    let blob = hx((x / 2) as i64, (y / 2) as i64, 56);
                    (r, g, b) = if blob > 0.80 { (212.0, 164.0, 120.0) } else { spk((127.0, 127.0, 127.0), 9.0, n) };
                }
                17 => {
                    // gravel (chunky)
                    let c = hx((x / 2) as i64, (y / 2) as i64, 888);
                    let v = 108.0 + c * 45.0;
                    (r, g, b) = (v * 1.06, v, v * 0.94);
                }
                18 => {
                    // sandstone: banded sand
                    let band = y % 6;
                    (r, g, b) = if band == 0 {
                        spk((196.0, 182.0, 132.0), 6.0, n)
                    } else {
                        spk((214.0, 200.0, 150.0), 8.0, n)
                    };
                }
                19 => {
                    // birch side: pale bark with dark dashes
                    let dash = hx((x / 2) as i64, (y / 3) as i64, 42) > 0.82;
                    (r, g, b) = if dash {
                        (52.0, 48.0, 42.0)
                    } else {
                        spk((216.0, 214.0, 202.0), 7.0, n)
                    };
                }
                20 => {
                    // birch top: pale rings
                    let d = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs()) as i32;
                    (r, g, b) = if d % 2 == 0 {
                        spk((214.0, 206.0, 186.0), 5.0, n)
                    } else {
                        spk((188.0, 178.0, 156.0), 5.0, n)
                    };
                }
                21 => {
                    // birch leaves: GRAYSCALE (fixed tint from the mesh)
                    (r, g, b) = if n < 0.16 {
                        let l = 74.0;
                        (l, l, l)
                    } else if n > 0.90 {
                        let l = 124.0;
                        (l, l, l)
                    } else {
                        let l = 96.0 + (n - 0.5) * 26.0;
                        (l, l, l)
                    };
                }
                22 => {
                    // spruce side: dark bark
                    let m = x % 4;
                    (r, g, b) = if m == 0 || (m == 2 && n < 0.4) {
                        spk((56.0, 42.0, 28.0), 5.0, n)
                    } else {
                        spk((74.0, 56.0, 36.0), 6.0, n)
                    };
                }
                23 => {
                    // spruce top: dark rings
                    let d = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs()) as i32;
                    (r, g, b) = if d >= 7 {
                        spk((74.0, 56.0, 36.0), 5.0, n)
                    } else if d % 2 == 0 {
                        spk((142.0, 108.0, 66.0), 6.0, n)
                    } else {
                        spk((112.0, 84.0, 52.0), 6.0, n)
                    };
                }
                24 => {
                    // spruce leaves: GRAYSCALE (fixed tint from the mesh)
                    (r, g, b) = if n < 0.18 {
                        let l = 44.0;
                        (l, l, l)
                    } else if n > 0.90 {
                        let l = 84.0;
                        (l, l, l)
                    } else {
                        let l = 62.0 + (n - 0.5) * 22.0;
                        (l, l, l)
                    };
                }
                25 => {
                    // cactus side: green with ribs + spines
                    let rib = x % 4 == 0;
                    let spine = (x % 4 == 2) && (y % 5 == 2) && n > 0.4;
                    (r, g, b) = if spine {
                        (228.0, 236.0, 190.0)
                    } else if rib {
                        spk((44.0, 108.0, 52.0), 6.0, n)
                    } else {
                        spk((62.0, 136.0, 64.0), 8.0, n)
                    };
                }
                26 => {
                    // cactus top
                    (r, g, b) = if x == 0 || x == 15 || y == 0 || y == 15 {
                        spk((44.0, 108.0, 52.0), 5.0, n)
                    } else {
                        spk((76.0, 150.0, 72.0), 8.0, n)
                    };
                }
                27 => {
                    // tall grass: GRAYSCALE blades on transparent bg
                    let blade = (hx((x * 7) as i64, 1, 91) > 0.42)
                        && y > 15 - (3.0 + hx((x * 13) as i64, 2, 92) * 10.0) as u32;
                    if blade {
                        let l = 120.0 + (n - 0.5) * 34.0;
                        (r, g, b) = (l, l, l);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                28 => {
                    // red flower
                    flower(&mut r, &mut g, &mut b, &mut a, x, y, (210.0, 52.0, 48.0), n);
                }
                29 => {
                    // yellow flower
                    flower(&mut r, &mut g, &mut b, &mut a, x, y, (232.0, 200.0, 60.0), n);
                }
                30 => {
                    // mushroom: white stem, red cap with dots
                    let cap = y >= 4 && y <= 8 && x >= 3 && x <= 12 && (y != 8 || (4..12).contains(&x));
                    let stem = y > 8 && y <= 14 && x >= 7 && x <= 8;
                    if cap {
                        let dot = hx((x / 2) as i64, (y / 2) as i64, 61) > 0.85;
                        (r, g, b) = if dot { (240.0, 238.0, 230.0) } else { spk((196.0, 60.0, 52.0), 8.0, n) };
                    } else if stem {
                        (r, g, b) = spk((228.0, 222.0, 208.0), 6.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                31 => {
                    // dead bush: brown twigs
                    let twig = (x + y) % 7 == 0 || (x == 8 && y > 6) || (x + 2 * y) % 9 == 0;
                    if twig && y > 3 {
                        (r, g, b) = spk((124.0, 92.0, 48.0), 10.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                32 => {
                    // pumpkin side: orange with ribs
                    let rib = x % 5 == 0;
                    (r, g, b) = if rib {
                        spk((172.0, 96.0, 30.0), 6.0, n)
                    } else {
                        spk((214.0, 128.0, 38.0), 8.0, n)
                    };
                }
                33 => {
                    // pumpkin top: ribs + stem
                    let stem = x >= 7 && x <= 8 && y >= 7 && y <= 8;
                    let ring = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs()) as i32;
                    (r, g, b) = if stem {
                        spk((96.0, 118.0, 44.0), 8.0, n)
                    } else if ring % 3 == 0 {
                        spk((176.0, 100.0, 32.0), 6.0, n)
                    } else {
                        spk((206.0, 122.0, 40.0), 8.0, n)
                    };
                }
                34 => {
                    // glowstone: warm bright blobs
                    let c = hx((x / 2) as i64, (y / 2) as i64, 71);
                    (r, g, b) = if c > 0.62 {
                        spk((255.0, 222.0, 150.0), 10.0, n)
                    } else {
                        spk((178.0, 132.0, 74.0), 10.0, n)
                    };
                }
                35 => {
                    // wool: fluffy white
                    (r, g, b) = if n < 0.12 {
                        (204.0, 202.0, 198.0)
                    } else {
                        spk((236.0, 234.0, 230.0), 7.0, n)
                    };
                }
                36 => {
                    // sun: warm bright square
                    let d = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs());
                    (r, g, b) = if d > 6.5 {
                        (255.0, 244.0, 190.0)
                    } else {
                        (255.0, 252.0, 220.0)
                    };
                }
                37 => {
                    // moon: pale square with craters
                    let c = hx((x / 3) as i64, (y / 3) as i64, 88);
                    (r, g, b) = if c > 0.78 {
                        (188.0, 192.0, 205.0)
                    } else {
                        (222.0, 226.0, 238.0)
                    };
                }
                38 => {
                    // zombie face: sickly green, dark eyes, grim mouth
                    zombie_base(&mut r, &mut g, &mut b, x, y, n);
                    let eye = (x >= 3 && x <= 5 && y >= 5 && y <= 6)
                        || (x >= 10 && x <= 12 && y >= 5 && y <= 6);
                    let mouth = y == 10 && x >= 5 && x <= 10 && n > 0.3;
                    if eye {
                        (r, g, b) = (16.0, 20.0, 14.0);
                    } else if mouth {
                        (r, g, b) = (40.0, 46.0, 34.0);
                    }
                }
                39 => {
                    // zombie skin
                    zombie_base(&mut r, &mut g, &mut b, x, y, n);
                }
                40 => {
                    // zombie body: ragged teal shirt
                    let tear = hx((x / 3) as i64, (y / 3) as i64, 99) > 0.88;
                    (r, g, b) = if tear {
                        (56.0, 70.0, 58.0)
                    } else {
                        spk((62.0, 96.0, 92.0), 8.0, n)
                    };
                }
                41 => {
                    // zombie limbs: dark trousers / arms
                    zombie_base_dark(&mut r, &mut g, &mut b, x, y, n);
                }
                42 => {
                    // siffleur skin: mottled green camo
                    let c = hx((x / 2) as i64, (y / 2) as i64, 43);
                    (r, g, b) = if c > 0.72 {
                        spk((44.0, 122.0, 40.0), 8.0, n)
                    } else if c < 0.25 {
                        spk((96.0, 178.0, 70.0), 8.0, n)
                    } else {
                        spk((66.0, 148.0, 54.0), 8.0, n)
                    };
                }
                43 => {
                    // siffleur face: camo + black oval eyes + open mouth
                    let c = hx((x / 2) as i64, (y / 2) as i64, 43);
                    (r, g, b) = if c > 0.72 {
                        spk((44.0, 122.0, 40.0), 8.0, n)
                    } else {
                        spk((88.0, 168.0, 66.0), 8.0, n)
                    };
                    let eye = (x >= 3 && x <= 4 && y >= 4 && y <= 7)
                        || (x >= 11 && x <= 12 && y >= 4 && y <= 7);
                    let mouth = x >= 6 && x <= 9 && y >= 10 && y <= 12;
                    if eye || mouth {
                        (r, g, b) = (12.0, 14.0, 12.0);
                    }
                }
                44 => {
                    // pig skin: pink
                    (r, g, b) = if n < 0.1 {
                        (222.0, 148.0, 140.0)
                    } else {
                        spk((240.0, 168.0, 158.0), 7.0, n)
                    };
                }
                45 => {
                    // pig face: pink + snout + eyes
                    (r, g, b) = if n < 0.1 {
                        (222.0, 148.0, 140.0)
                    } else {
                        spk((240.0, 168.0, 158.0), 7.0, n)
                    };
                    let snout = x >= 5 && x <= 10 && y >= 8 && y <= 12;
                    let nostril = (x == 6 || x == 9) && (y == 9 || y == 11);
                    let eye = (x == 3 || x == 12) && (y == 5 || y == 6);
                    if nostril {
                        (r, g, b) = (90.0, 40.0, 40.0);
                    } else if snout {
                        (r, g, b) = spk((252.0, 186.0, 176.0), 4.0, n);
                    } else if eye {
                        (r, g, b) = (18.0, 16.0, 16.0);
                    }
                }
                46 => {
                    // sheep wool (body): fluffy cream
                    (r, g, b) = if n < 0.14 {
                        (206.0, 202.0, 192.0)
                    } else {
                        spk((238.0, 234.0, 224.0), 6.0, n)
                    };
                }
                47 => {
                    // sheep face
                    (r, g, b) = spk((226.0, 214.0, 196.0), 6.0, n);
                    let wool_top = y <= 3;
                    let eye = (x == 4 || x == 11) && (y == 7 || y == 8);
                    let nose = x >= 7 && x <= 8 && y >= 11 && y <= 12;
                    if wool_top {
                        (r, g, b) = spk((238.0, 234.0, 224.0), 5.0, n);
                    } else if eye {
                        (r, g, b) = (16.0, 16.0, 18.0);
                    } else if nose {
                        (r, g, b) = (214.0, 150.0, 150.0);
                    }
                }
                48 => {
                    // rabbit skin: soft brown
                    (r, g, b) = if n < 0.12 {
                        (150.0, 118.0, 88.0)
                    } else {
                        spk((186.0, 148.0, 110.0), 8.0, n)
                    };
                }
                49 => {
                    // rabbit face
                    (r, g, b) = spk((186.0, 148.0, 110.0), 8.0, n);
                    let eye = (x == 4 || x == 11) && (y == 6 || y == 7);
                    let nose = x >= 7 && x <= 8 && y == 10;
                    if eye {
                        (r, g, b) = (14.0, 14.0, 16.0);
                    } else if nose {
                        (r, g, b) = (226.0, 148.0, 148.0);
                    }
                }
                50 => {
                    // meat: cooked steak
                    let inner = x >= 3 && x <= 12 && y >= 4 && y <= 12;
                    (r, g, b) = if !inner {
                        (96.0, 58.0, 34.0)
                    } else if (x + y) % 5 == 0 {
                        spk((196.0, 132.0, 88.0), 8.0, n)
                    } else {
                        spk((158.0, 96.0, 58.0), 8.0, n)
                    };
                }
                // ---------------- v0.3 Wilderness Bound ----------------
                51 => {
                    // poplar bark: desaturated pale brown, fine streaks
                    let m = x % 3;
                    (r, g, b) = if m == 0 || (m == 2 && n < 0.35) {
                        spk((122.0, 108.0, 88.0), 6.0, n)
                    } else {
                        spk((158.0, 143.0, 118.0), 6.0, n)
                    };
                }
                52 => {
                    // poplar top: pale rings
                    let d = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs()) as i32;
                    (r, g, b) = if d >= 7 {
                        spk((132.0, 118.0, 96.0), 5.0, n)
                    } else if d % 2 == 0 {
                        spk((190.0, 172.0, 142.0), 5.0, n)
                    } else {
                        spk((162.0, 146.0, 118.0), 5.0, n)
                    };
                }
                53 | 54 | 55 => {
                    // poplar leaves: red / orange / yellow autumn sets
                    let hole = hx((x / 2) as i64, (y / 2) as i64, t as i64);
                    let (dk, base, lt) = if t == 53 {
                        ((116.0, 34.0, 28.0), (166.0, 54.0, 40.0), (198.0, 88.0, 50.0))
                    } else if t == 54 {
                        ((148.0, 74.0, 26.0), (206.0, 118.0, 38.0), (234.0, 154.0, 60.0))
                    } else {
                        ((172.0, 128.0, 30.0), (222.0, 178.0, 44.0), (242.0, 210.0, 84.0))
                    };
                    (r, g, b) = if hole > 0.88 {
                        (dk.0 * 0.8, dk.1 * 0.8, dk.2 * 0.8)
                    } else if n < 0.18 {
                        dk
                    } else if n > 0.90 {
                        lt
                    } else {
                        spk(base, 14.0, n)
                    };
                }
                56 => {
                    // poplar planks: pale
                    let row = y / 4;
                    (r, g, b) = if y % 4 == 3 || x == (row * 7 + 5) % 16 {
                        (138.0, 122.0, 96.0)
                    } else if n > 0.85 {
                        (198.0, 178.0, 144.0)
                    } else {
                        spk((186.0, 166.0, 132.0), 6.0, n)
                    };
                }
                57 => {
                    // red shrub: dark red twigs + berries
                    let twig = (x + y) % 6 == 0 || (x == 8 && y > 5) || (x + 2 * y) % 8 == 1;
                    let berry = hx((x / 2) as i64, (y / 2) as i64, 57) > 0.88 && y > 2 && y < 12;
                    if berry {
                        (r, g, b) = (204.0, 52.0, 44.0);
                    } else if twig && y > 2 {
                        (r, g, b) = spk((122.0, 52.0, 40.0), 10.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                58 | 59 => {
                    // shelf mushroom (bracket fungus), small & large
                    let big = t == 59;
                    let (x0, x1, y0, y1) = if big {
                        (1u32, 15u32, 7u32, 13u32)
                    } else {
                        (4u32, 12u32, 9u32, 13u32)
                    };
                    let shelf = x >= x0 && x <= x1 && y >= y0 && y <= y1;
                    let under = y >= y1 - 1;
                    if shelf {
                        let ring = ((x - x0 + y1 - y) / 2) % 2 == 0;
                        (r, g, b) = if under {
                            spk((224.0, 208.0, 178.0), 6.0, n)
                        } else if ring {
                            spk((198.0, 148.0, 86.0), 8.0, n)
                        } else {
                            spk((172.0, 120.0, 62.0), 8.0, n)
                        };
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                60 => {
                    // hay side: golden bands + stitching
                    let band = (x + y / 4) % 5 == 0;
                    let stitch = y % 8 == 0;
                    (r, g, b) = if stitch {
                        (152.0, 116.0, 44.0)
                    } else if band {
                        spk((190.0, 150.0, 62.0), 8.0, n)
                    } else {
                        spk((212.0, 172.0, 76.0), 9.0, n)
                    };
                }
                61 => {
                    // hay top: cross-hatched straw
                    let cross = (x + y) % 4 == 0 || (x + 16 - y) % 4 == 1;
                    (r, g, b) = if cross {
                        spk((186.0, 146.0, 58.0), 8.0, n)
                    } else {
                        spk((214.0, 178.0, 84.0), 9.0, n)
                    };
                }
                62 => {
                    // straw bed top: quilted straw + wool trim
                    let trim = x < 2 || x > 13;
                    let quilt = (x + y) % 5 == 0;
                    (r, g, b) = if trim {
                        spk((226.0, 220.0, 208.0), 5.0, n)
                    } else if quilt {
                        spk((196.0, 158.0, 70.0), 7.0, n)
                    } else {
                        spk((216.0, 180.0, 88.0), 7.0, n)
                    };
                }
                63 => {
                    // straw bed side: straw + cloth band
                    let band = y < 4;
                    (r, g, b) = if band {
                        spk((226.0, 220.0, 208.0), 5.0, n)
                    } else {
                        spk((204.0, 166.0, 76.0), 9.0, n)
                    };
                }
                64 => {
                    // campfire: crossed logs + flames
                    let log = (x + y) % 8 < 3 && y > 8;
                    let flame = (5..=10).contains(&x) && y > 3 && y <= 9
                        && hx((x / 2) as i64, (y / 2) as i64, 64) > 0.3;
                    if flame && y < 7 {
                        (r, g, b) = spk((252.0, 204.0, 88.0), 12.0, n);
                    } else if flame {
                        (r, g, b) = spk((240.0, 136.0, 40.0), 12.0, n);
                    } else if log {
                        (r, g, b) = spk((96.0, 68.0, 42.0), 6.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                65 => {
                    // barrel side: staves + iron hoops
                    let hoop = y < 2 || y > 13;
                    let stave = x % 4 == 0;
                    (r, g, b) = if hoop {
                        spk((70.0, 64.0, 58.0), 5.0, n)
                    } else if stave {
                        spk((120.0, 88.0, 50.0), 6.0, n)
                    } else {
                        spk((150.0, 112.0, 64.0), 6.0, n)
                    };
                }
                66 => {
                    // barrel top: radial planks
                    let ring = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs()) as i32;
                    (r, g, b) = if ring >= 7 {
                        spk((70.0, 64.0, 58.0), 5.0, n)
                    } else if ring % 3 == 0 {
                        spk((126.0, 94.0, 54.0), 6.0, n)
                    } else {
                        spk((148.0, 110.0, 62.0), 6.0, n)
                    };
                }
                67 => {
                    // pale grass top: desaturated sage
                    let clump = crate::noise::value_noise_2d(x as f32 * 0.55, y as f32 * 0.55, 67);
                    let base = 118.0 + clump * 30.0;
                    (r, g, b) = if n < 0.08 {
                        (108.0, 118.0, 88.0)
                    } else {
                        (base, base + 22.0, base * 0.78)
                    };
                }
                68 => {
                    // pale grass side: dirt + pale fringe
                    let green = y < 3 || (y == 3 && n < 0.55) || (y == 4 && n < 0.15);
                    if green {
                        (r, g, b) = spk((134.0, 144.0, 108.0), 8.0, n);
                    } else {
                        (r, g, b) = spk((134.0, 96.0, 66.0), 14.0, n);
                    }
                }
                t if (69..=83).contains(&t) => {
                    // colored wool (orange..black): fuzzy weave
                    let c = DYE_RGB[(t - 68) as usize];
                    let weave = (x + y) % 4 == 0;
                    (r, g, b) = if n < 0.12 {
                        spk((c.0 * 0.84, c.1 * 0.84, c.2 * 0.84), 5.0, n)
                    } else if weave {
                        spk((c.0 * 0.92, c.1 * 0.92, c.2 * 0.92), 4.0, n)
                    } else {
                        spk(c, 5.0, n)
                    };
                }
                t if (84..=99).contains(&t) => {
                    // concrete: smooth with fine speckle
                    let c = DYE_RGB[(t - 84) as usize];
                    (r, g, b) = if n > 0.94 {
                        spk((c.0 * 1.05, c.1 * 1.05, c.2 * 1.05), 3.0, n)
                    } else if n < 0.06 {
                        spk((c.0 * 0.93, c.1 * 0.93, c.2 * 0.93), 3.0, n)
                    } else {
                        c
                    };
                }
                100 => {
                    // chicken skin: white feathers
                    let feather = (x * 2 + y) % 5 == 0;
                    (r, g, b) = if feather {
                        spk((216.0, 214.0, 206.0), 4.0, n)
                    } else {
                        spk((238.0, 236.0, 228.0), 5.0, n)
                    };
                }
                101 => {
                    // chicken face: eyes + orange beak + red wattle
                    (r, g, b) = spk((238.0, 236.0, 228.0), 5.0, n);
                    let eye = (x == 4 || x == 11) && (y == 5 || y == 6);
                    let beak = x >= 6 && x <= 9 && y >= 8 && y <= 10;
                    let wattle = x >= 7 && x <= 8 && y == 11;
                    if eye {
                        (r, g, b) = (16.0, 16.0, 18.0);
                    } else if beak {
                        (r, g, b) = spk((242.0, 178.0, 56.0), 6.0, n);
                    } else if wattle {
                        (r, g, b) = (206.0, 62.0, 52.0);
                    }
                }
                102 => {
                    // cow skin: brown with white patches
                    let patch = hx((x / 3) as i64, (y / 3) as i64, 102) > 0.78;
                    (r, g, b) = if patch {
                        spk((228.0, 224.0, 216.0), 5.0, n)
                    } else {
                        spk((98.0, 68.0, 46.0), 7.0, n)
                    };
                }
                103 => {
                    // cow face: blaze + muzzle
                    let blaze = x >= 7 && x <= 8;
                    (r, g, b) = if blaze {
                        spk((228.0, 224.0, 216.0), 5.0, n)
                    } else {
                        spk((98.0, 68.0, 46.0), 7.0, n)
                    };
                    let eye = (x == 3 || x == 12) && (y == 5 || y == 6);
                    let muzzle = x >= 4 && x <= 11 && y >= 10;
                    let nostril = (x == 5 || x == 10) && y >= 11;
                    if eye {
                        (r, g, b) = (16.0, 16.0, 18.0);
                    } else if nostril {
                        (r, g, b) = (150.0, 96.0, 96.0);
                    } else if muzzle {
                        (r, g, b) = spk((214.0, 176.0, 168.0), 5.0, n);
                    }
                }
                104 => {
                    // fox skin: rusty orange, pale belly
                    let belly = y > 10;
                    (r, g, b) = if belly {
                        spk((228.0, 216.0, 198.0), 6.0, n)
                    } else {
                        spk((204.0, 118.0, 42.0), 8.0, n)
                    };
                }
                105 => {
                    // fox face: ears tip dark, white cheeks, black nose
                    (r, g, b) = spk((204.0, 118.0, 42.0), 8.0, n);
                    let cheek = (x <= 4 || x >= 11) && y >= 8;
                    let eye = (x == 4 || x == 11) && (y == 6 || y == 7);
                    let nose = x >= 7 && x <= 8 && y == 11;
                    if eye || nose {
                        (r, g, b) = (22.0, 18.0, 16.0);
                    } else if cheek {
                        (r, g, b) = spk((234.0, 226.0, 210.0), 6.0, n);
                    }
                }
                // ------- v0.4 "Legacy Update" tiles -------
                106 => {
                    // red sand: warm rippled dunes
                    let wave = crate::noise::value_noise_2d(x as f32 * 0.7, y as f32 * 0.35, 777);
                    (r, g, b) = if wave > 0.62 {
                        spk((208.0, 124.0, 78.0), 6.0, n)
                    } else {
                        spk((192.0, 112.0, 70.0), 8.0, n)
                    };
                }
                107 => {
                    // red sandstone: strata bands
                    (r, g, b) = if y % 6 == 0 {
                        spk((170.0, 96.0, 60.0), 5.0, n)
                    } else {
                        spk((196.0, 118.0, 74.0), 7.0, n)
                    };
                }
                108 => {
                    // podzol top: leaf-mold brown/orange
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9001);
                    (r, g, b) = if c > 0.75 {
                        spk((142.0, 96.0, 44.0), 8.0, n)
                    } else {
                        spk((102.0, 72.0, 40.0), 9.0, n)
                    };
                }
                109 => {
                    // podzol side: dirt with a moldy fringe
                    let green = y < 4 || (y == 4 && n < 0.5);
                    if green {
                        (r, g, b) = spk((118.0, 82.0, 46.0), 9.0, n);
                    } else {
                        (r, g, b) = spk((134.0, 96.0, 66.0), 14.0, n);
                    }
                }
                110 => {
                    // coarse dirt: dirt with gray pebbles
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9002);
                    (r, g, b) = if c > 0.82 {
                        spk((128.0, 124.0, 118.0), 6.0, n)
                    } else {
                        spk((124.0, 90.0, 62.0), 12.0, n)
                    };
                }
                111 => {
                    // packed ice: pale blue with white cracks
                    let crack = ((x + 3 * y) % 11 == 0) || ((2 * x + y) % 13 == 4);
                    (r, g, b) = if crack {
                        spk((214.0, 232.0, 246.0), 5.0, n)
                    } else {
                        spk((162.0, 198.0, 232.0), 8.0, n)
                    };
                }
                112 => (r, g, b) = stone_fam((158.0, 118.0, 108.0), 10.0, 9101, x, y, n), // granite
                113 => {
                    // diorite: salt & pepper
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9102);
                    (r, g, b) = if c > 0.62 {
                        spk((92.0, 90.0, 94.0), 7.0, n)
                    } else {
                        spk((192.0, 190.0, 188.0), 8.0, n)
                    };
                }
                114 => (r, g, b) = stone_fam((136.0, 138.0, 136.0), 8.0, 9103, x, y, n), // andesite
                115 => {
                    // sponge: yellow with dark pores
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9104);
                    (r, g, b) = if c > 0.84 {
                        spk((96.0, 84.0, 36.0), 6.0, n)
                    } else if c > 0.72 {
                        spk((168.0, 154.0, 66.0), 7.0, n)
                    } else {
                        spk((204.0, 190.0, 86.0), 7.0, n)
                    };
                }
                116 => {
                    // magma: black rock with glowing cracks
                    let c = hx((x / 3) as i64, (y / 3) as i64, 9105);
                    let vein = (x + y) % 7 == 0 || (2 * x + y) % 9 == 2;
                    (r, g, b) = if vein && c > 0.3 {
                        spk((244.0, 130.0, 34.0), 14.0, n)
                    } else if vein {
                        spk((188.0, 74.0, 26.0), 10.0, n)
                    } else {
                        spk((56.0, 32.0, 28.0), 8.0, n)
                    };
                }
                117 => {
                    // seagrass: PRE-COLORED water blades (vanilla does not
                    // tint seagrass - no tintindex in its model)
                    let blade = x % 4 == 1 || x % 4 == 2;
                    if blade && y > 2 + (x % 7) {
                        let l = (n - 0.5) * 30.0;
                        (r, g, b) = (62.0 + l, 138.0 + l * 0.7, 14.0 + l * 0.4);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                118 => {
                    // kelp: PRE-COLORED olive stem with side fronds
                    let stem = x == 7 || x == 8;
                    let frond = y % 4 < 2 && ((x == 5 && y % 8 < 4) || (x == 10 && y % 8 >= 4));
                    if stem || frond {
                        let l = (n - 0.5) * 26.0;
                        (r, g, b) = (84.0 + l, 118.0 + l, 34.0 + l * 0.5);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                119 => {
                    // coral pink: bumpy
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9119);
                    (r, g, b) = if c > 0.7 {
                        spk((238.0, 138.0, 168.0), 8.0, n)
                    } else {
                        spk((206.0, 96.0, 132.0), 8.0, n)
                    };
                }
                120 => {
                    // coral blue: bumpy
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9120);
                    (r, g, b) = if c > 0.7 {
                        spk((120.0, 176.0, 226.0), 8.0, n)
                    } else {
                        spk((84.0, 132.0, 196.0), 8.0, n)
                    };
                }
                121 => (r, g, b) = stone_fam((116.0, 106.0, 116.0), 10.0, 9121, x, y, n), // dead coral
                122 => {
                    // lily pad: GRAYSCALE disc with a notch
                    let dx = x as f32 - 7.5;
                    let dy = y as f32 - 7.5;
                    let d = (dx * dx + dy * dy).sqrt();
                    let notch = dx.abs() < 1.5 && dy < 0.0;
                    if d < 7.4 && !notch {
                        let vein = (x + y) % 5 == 0;
                        let l = if vein { 96.0 } else { 118.0 } + (n - 0.5) * 14.0;
                        (r, g, b) = (l, l, l);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                123 => {
                    // sugarcane: PRE-COLORED yellowish-green jointed stalks
                    // (vanilla ships it colored AND still applies the grass
                    // tint on top, per the tinted_cross model)
                    let stalk = x % 5 == 2;
                    let joint = y % 5 == 2;
                    if stalk {
                        let l = (n - 0.5) * 20.0;
                        (r, g, b) = if joint {
                            (104.0 + l, 132.0 + l, 68.0 + l * 0.5)
                        } else {
                            (146.0 + l, 186.0 + l, 96.0 + l * 0.5)
                        };
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                124 => {
                    // berry bush: dark bush with red berries
                    let bush = hx((x / 2) as i64, (y / 2) as i64, 9124) > 0.18 && y > 2;
                    let berry = hx((x / 3) as i64, (y / 3) as i64, 9125) > 0.88;
                    if bush && berry {
                        (r, g, b) = spk((208.0, 44.0, 52.0), 10.0, n);
                    } else if bush {
                        (r, g, b) = spk((58.0, 96.0, 56.0), 10.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                125 => {
                    // bamboo plant: stalk with nodes
                    let stalk = x == 6 || x == 7 || x == 9;
                    let joint = y % 6 == 3;
                    if stalk {
                        (r, g, b) = if joint {
                            spk((94.0, 148.0, 58.0), 6.0, n)
                        } else {
                            spk((124.0, 178.0, 80.0), 8.0, n)
                        };
                    } else if y < 3 && (x == 5 || x == 10) {
                        (r, g, b) = spk((110.0, 164.0, 70.0), 7.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                126 => (r, g, b) = log_side((206.0, 196.0, 132.0), (176.0, 164.0, 104.0), 9126, x, y, n),
                127 => (r, g, b) = log_top((206.0, 196.0, 132.0), (176.0, 164.0, 104.0), x, y, n),
                128 => {
                    // honey: amber with a drip highlight
                    let c = hx((x / 4) as i64, (y / 4) as i64, 9128);
                    (r, g, b) = if c > 0.72 {
                        spk((248.0, 190.0, 60.0), 7.0, n)
                    } else {
                        spk((226.0, 156.0, 44.0), 7.0, n)
                    };
                }
                129 => {
                    // beehive side: woody with honeycomb dots
                    let comb = hx((x / 3) as i64, (y / 3) as i64, 9129) > 0.78;
                    (r, g, b) = if comb {
                        spk((226.0, 178.0, 92.0), 6.0, n)
                    } else {
                        spk((188.0, 142.0, 74.0), 8.0, n)
                    };
                }
                130 => (r, g, b) = log_top((214.0, 172.0, 96.0), (170.0, 128.0, 66.0), x, y, n),
                131 => {
                    // basalt side: vertical columns
                    (r, g, b) = if x % 5 == 0 {
                        spk((40.0, 40.0, 46.0), 5.0, n)
                    } else {
                        spk((64.0, 63.0, 70.0), 7.0, n)
                    };
                }
                132 => {
                    // basalt top: polygon cells
                    let c = hx((x / 4) as i64, (y / 4) as i64, 9132);
                    (r, g, b) = if c > 0.6 {
                        spk((72.0, 71.0, 78.0), 6.0, n)
                    } else {
                        spk((52.0, 51.0, 58.0), 6.0, n)
                    };
                }
                133 => {
                    // blackstone: near-black with gray specks
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9133);
                    (r, g, b) = if c > 0.86 {
                        spk((88.0, 84.0, 90.0), 8.0, n)
                    } else {
                        spk((44.0, 41.0, 46.0), 7.0, n)
                    };
                }
                134 => (r, g, b) = ore((128.0, 128.0, 130.0), (206.0, 120.0, 70.0), 9134, x, y, n),
                135 => {
                    // copper block: panel with darker border
                    let border = x == 0 || y == 0 || x == 15 || y == 15;
                    (r, g, b) = if border {
                        spk((168.0, 96.0, 62.0), 6.0, n)
                    } else {
                        spk((202.0, 122.0, 78.0), 7.0, n)
                    };
                }
                136 => {
                    // amethyst: faceted purple crystals
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9136);
                    (r, g, b) = if c > 0.82 {
                        spk((208.0, 172.0, 248.0), 9.0, n)
                    } else if c > 0.4 {
                        spk((168.0, 116.0, 224.0), 9.0, n)
                    } else {
                        spk((128.0, 84.0, 190.0), 9.0, n)
                    };
                }
                137 => {
                    // calcite: white marble veins
                    (r, g, b) = if (2 * x + y) % 9 == 3 {
                        spk((196.0, 194.0, 190.0), 5.0, n)
                    } else {
                        spk((228.0, 226.0, 220.0), 6.0, n)
                    };
                }
                138 => (r, g, b) = stone_fam((112.0, 114.0, 104.0), 9.0, 9138, x, y, n), // tuff
                139 => {
                    // deepslate: dark horizontal striations
                    (r, g, b) = if (y / 2) % 3 == 0 {
                        spk((62.0, 64.0, 70.0), 5.0, n)
                    } else {
                        spk((78.0, 80.0, 86.0), 6.0, n)
                    };
                }
                140 => {
                    // dripstone: porous tan
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9140);
                    (r, g, b) = if c > 0.84 {
                        spk((110.0, 78.0, 54.0), 6.0, n)
                    } else {
                        spk((158.0, 118.0, 82.0), 8.0, n)
                    };
                }
                141 => {
                    // moss: fuzzy rich green
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9141);
                    (r, g, b) = if c > 0.68 {
                        spk((104.0, 148.0, 64.0), 9.0, n)
                    } else {
                        spk((82.0, 122.0, 50.0), 9.0, n)
                    };
                }
                142 => {
                    // azalea: leafy bush
                    let bush = hx((x / 2) as i64, (y / 2) as i64, 9142) > 0.22 && y > 2;
                    if bush {
                        (r, g, b) = spk((74.0, 128.0, 60.0), 10.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                143 => {
                    // flowering azalea: bush with magenta blossoms
                    let bush = hx((x / 2) as i64, (y / 2) as i64, 9142) > 0.22 && y > 2;
                    let bloom = hx((x / 3) as i64, (y / 3) as i64, 9143) > 0.78;
                    if bush && bloom {
                        (r, g, b) = spk((216.0, 96.0, 168.0), 9.0, n);
                    } else if bush {
                        (r, g, b) = spk((74.0, 128.0, 60.0), 10.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                144 => {
                    // glow berries: vine strands with glowing berries
                    let vine = (x == 4 || x == 11) && y > 1;
                    let berry = hx((x / 2) as i64, (y / 2) as i64, 9144) > 0.86 && y % 4 == 1;
                    if berry {
                        (r, g, b) = spk((244.0, 168.0, 56.0), 10.0, n);
                    } else if vine {
                        (r, g, b) = spk((70.0, 112.0, 56.0), 8.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                145 => {
                    // mud: wet dark clay with sheen
                    let c = hx((x / 3) as i64, (y / 3) as i64, 9145);
                    (r, g, b) = if c > 0.84 {
                        spk((106.0, 96.0, 84.0), 6.0, n)
                    } else {
                        spk((80.0, 70.0, 60.0), 7.0, n)
                    };
                }
                146 => (r, g, b) = stone_fam((172.0, 142.0, 104.0), 7.0, 9146, x, y, n), // packed mud
                147 => {
                    // mud bricks
                    let row = y / 5;
                    let seam = y % 5 == 0 || x as i64 == (row as i64 * 7 + 5) % 16;
                    (r, g, b) = if seam {
                        spk((120.0, 94.0, 66.0), 5.0, n)
                    } else {
                        spk((162.0, 128.0, 92.0), 7.0, n)
                    };
                }
                148 => {
                    // sculk: dark teal with cyan swirls
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9148);
                    (r, g, b) = if c > 0.9 {
                        spk((90.0, 190.0, 170.0), 10.0, n)
                    } else if c > 0.6 {
                        spk((44.0, 76.0, 72.0), 8.0, n)
                    } else {
                        spk((26.0, 42.0, 44.0), 7.0, n)
                    };
                }
                149 => (r, g, b) = log_side((116.0, 92.0, 76.0), (88.0, 68.0, 56.0), 9149, x, y, n),
                150 => (r, g, b) = log_top((150.0, 118.0, 92.0), (100.0, 78.0, 62.0), x, y, n),
                151 => (r, g, b, a) = leaves_gray(98.0, 75.0, 9151, x, y, n),
                152 => (r, g, b) = planks((154.0, 114.0, 82.0), (122.0, 88.0, 62.0), 9152, x, y, n),
                153 => {
                    // cherry log: dark maroon bark with pink lenticels
                    let ridge = (x % 6) < 2;
                    let lent = hx((x / 3) as i64, (y / 3) as i64, 9153) > 0.9;
                    (r, g, b) = if lent {
                        spk((206.0, 132.0, 150.0), 6.0, n)
                    } else if ridge {
                        spk((52.0, 32.0, 40.0), 5.0, n)
                    } else {
                        spk((76.0, 48.0, 58.0), 7.0, n)
                    };
                }
                154 => (r, g, b) = log_top((206.0, 148.0, 160.0), (72.0, 44.0, 54.0), x, y, n),
                155 => (r, g, b, a) = leaves((234.0, 148.0, 186.0), (206.0, 116.0, 160.0), 9155, x, y, n),
                156 => (r, g, b) = planks((226.0, 184.0, 184.0), (196.0, 152.0, 154.0), 9156, x, y, n),
                157 => {
                    // pink petals: scattered petals on transparent
                    let petal = hx((x / 2) as i64, (y / 2) as i64, 9157);
                    if petal > 0.55 && petal < 0.9 {
                        (r, g, b) = spk((238.0, 152.0, 192.0), 8.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                158 => (r, g, b) = log_side((104.0, 94.0, 88.0), (78.0, 70.0, 66.0), 9158, x, y, n),
                159 => (r, g, b) = log_top((172.0, 110.0, 66.0), (92.0, 80.0, 74.0), x, y, n),
                160 => (r, g, b, a) = leaves_gray(132.0, 106.0, 9160, x, y, n),
                161 => (r, g, b) = planks((176.0, 118.0, 76.0), (140.0, 92.0, 58.0), 9161, x, y, n),
                162 => {
                    // melon side: striped rind
                    (r, g, b) = if (x % 8) < 4 {
                        spk((92.0, 148.0, 58.0), 8.0, n)
                    } else {
                        spk((158.0, 196.0, 88.0), 8.0, n)
                    };
                }
                163 => (r, g, b) = log_top((168.0, 196.0, 96.0), (104.0, 140.0, 62.0), x, y, n),
                164 => (r, g, b) = stone_fam((152.0, 96.0, 68.0), 8.0, 9164, x, y, n),
                165 => (r, g, b) = stone_fam((142.0, 64.0, 50.0), 8.0, 9165, x, y, n),
                166 => (r, g, b) = stone_fam((164.0, 86.0, 44.0), 8.0, 9166, x, y, n),
                167 => (r, g, b) = stone_fam((188.0, 134.0, 62.0), 8.0, 9167, x, y, n),
                168 => (r, g, b) = ore((128.0, 128.0, 130.0), (244.0, 206.0, 76.0), 9168, x, y, n),
                169 => (r, g, b) = ore((74.0, 76.0, 82.0), (110.0, 232.0, 226.0), 9169, x, y, n),
                170 => {
                    // copper bulb: copper frame + glowing center
                    let border = x < 2 || y < 2 || x > 13 || y > 13;
                    (r, g, b) = if border {
                        spk((198.0, 118.0, 74.0), 7.0, n)
                    } else {
                        spk((248.0, 196.0, 120.0), 8.0, n)
                    };
                }
                171 => {
                    // obsidian: black glass with purple glints
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9171);
                    (r, g, b) = if c > 0.92 {
                        spk((86.0, 58.0, 128.0), 10.0, n)
                    } else {
                        spk((30.0, 24.0, 44.0), 6.0, n)
                    };
                }
                172 => {
                    // crying obsidian: obsidian with magenta drips
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9171);
                    let drip = x % 5 == 2 && (y / 2 + x) % 3 == 0;
                    (r, g, b) = if drip {
                        spk((216.0, 64.0, 190.0), 10.0, n)
                    } else if c > 0.92 {
                        spk((86.0, 58.0, 128.0), 10.0, n)
                    } else {
                        spk((30.0, 24.0, 44.0), 6.0, n)
                    };
                }
                173 => {
                    // torch: wooden stick with a glowing tip
                    let stick = (x == 7 || x == 8) && y >= 6;
                    let tip = (6..=9).contains(&x) && (3..=5).contains(&y);
                    if tip {
                        (r, g, b) = if y == 3 {
                            spk((255.0, 232.0, 150.0), 6.0, n)
                        } else {
                            spk((250.0, 180.0, 60.0), 10.0, n)
                        };
                    } else if stick {
                        (r, g, b) = spk((120.0, 88.0, 52.0), 8.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                174 => {
                    // slime: green gel with a darker core
                    let dx = x as f32 - 7.5;
                    let dy = y as f32 - 7.5;
                    (r, g, b, a) = if dx * dx + dy * dy < 16.0 {
                        (84.0, 168.0, 70.0, 235.0)
                    } else {
                        (118.0, 208.0, 96.0, 190.0)
                    };
                }
                175 => {
                    // shadow skin: void-black with purple patches
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9175);
                    (r, g, b) = if c > 0.9 {
                        spk((52.0, 38.0, 74.0), 6.0, n)
                    } else {
                        spk((20.0, 16.0, 30.0), 5.0, n)
                    };
                }
                176 => {
                    // shadow face: void-black with magenta eyes
                    (r, g, b) = spk((18.0, 14.0, 26.0), 4.0, n);
                    let eye = (5..=6).contains(&y) && ((3..=5).contains(&x) || (10..=12).contains(&x));
                    if eye {
                        (r, g, b) = spk((214.0, 62.0, 208.0), 10.0, n);
                    }
                }
                177 => {
                    // bee skin: fuzzy yellow-black stripes
                    (r, g, b) = if (x / 2) % 2 == 0 {
                        spk((232.0, 184.0, 66.0), 8.0, n)
                    } else {
                        spk((58.0, 46.0, 32.0), 7.0, n)
                    };
                }
                178 => {
                    // bee face: yellow with big eyes
                    (r, g, b) = spk((232.0, 184.0, 66.0), 8.0, n);
                    let eye = (5..=8).contains(&y) && ((2..=4).contains(&x) || (11..=13).contains(&x));
                    if eye {
                        (r, g, b) = spk((34.0, 28.0, 20.0), 6.0, n);
                    }
                }
                179 => {
                    // parrot: red with blue wings
                    (r, g, b) = if x < 4 || x > 11 {
                        spk((56.0, 84.0, 196.0), 9.0, n)
                    } else {
                        spk((204.0, 52.0, 46.0), 9.0, n)
                    };
                }
                180 => {
                    // parrot face: red with white eye patch
                    (r, g, b) = spk((204.0, 52.0, 46.0), 9.0, n);
                    let patch = (4..=7).contains(&y) && ((3..=5).contains(&x) || (10..=12).contains(&x));
                    let eye = (5..=6).contains(&y) && (x == 4 || x == 11);
                    if patch {
                        (r, g, b) = spk((244.0, 240.0, 236.0), 5.0, n);
                    }
                    if eye {
                        (r, g, b) = spk((24.0, 22.0, 20.0), 4.0, n);
                    }
                }
                181 => {
                    // turtle skin: green scutes
                    let c = hx((x / 3) as i64, (y / 3) as i64, 9181);
                    (r, g, b) = if c > 0.6 {
                        spk((88.0, 158.0, 88.0), 8.0, n)
                    } else {
                        spk((58.0, 112.0, 60.0), 8.0, n)
                    };
                }
                182 => {
                    // turtle face: green with eyes
                    (r, g, b) = spk((96.0, 164.0, 96.0), 8.0, n);
                    let eye = (y == 6 || y == 7) && ((3..=4).contains(&x) || (11..=12).contains(&x));
                    if eye {
                        (r, g, b) = spk((26.0, 30.0, 24.0), 5.0, n);
                    }
                }
                183 => {
                    // dolphin: blue-gray sheen
                    let c = hx((x / 3) as i64, (y / 3) as i64, 9183);
                    (r, g, b) = if c > 0.7 {
                        spk((136.0, 168.0, 202.0), 7.0, n)
                    } else {
                        spk((104.0, 136.0, 172.0), 7.0, n)
                    };
                }
                184 => {
                    // dolphin face: eyes + pale beak
                    (r, g, b) = spk((120.0, 152.0, 188.0), 7.0, n);
                    let eye = (y == 6 || y == 7) && ((3..=4).contains(&x) || (11..=12).contains(&x));
                    let beak = y > 11 && (5..=10).contains(&x);
                    if eye {
                        (r, g, b) = spk((22.0, 26.0, 32.0), 5.0, n);
                    }
                    if beak {
                        (r, g, b) = spk((198.0, 206.0, 210.0), 6.0, n);
                    }
                }
                185 => {
                    // goat: shaggy off-white
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9185);
                    (r, g, b) = if c > 0.75 {
                        spk((238.0, 234.0, 224.0), 6.0, n)
                    } else {
                        spk((216.0, 210.0, 198.0), 7.0, n)
                    };
                }
                186 => {
                    // goat face: horns + eyes
                    (r, g, b) = spk((222.0, 216.0, 204.0), 7.0, n);
                    let eye = y == 7 && ((x == 4 || x == 5) || (x == 10 || x == 11));
                    let horn = ((1..=3).contains(&x) || (12..=14).contains(&x)) && y <= 3;
                    if eye {
                        (r, g, b) = spk((40.0, 36.0, 34.0), 4.0, n);
                    }
                    if horn {
                        (r, g, b) = spk((160.0, 148.0, 132.0), 6.0, n);
                    }
                }
                187 => {
                    // frog: green with darker spots
                    let c = hx((x / 3) as i64, (y / 3) as i64, 9187);
                    (r, g, b) = if c > 0.82 {
                        spk((64.0, 122.0, 48.0), 7.0, n)
                    } else {
                        spk((104.0, 172.0, 72.0), 8.0, n)
                    };
                }
                188 => {
                    // frog face: big high-set eyes
                    (r, g, b) = spk((104.0, 172.0, 72.0), 8.0, n);
                    let bulge = ((2..=5).contains(&x) || (10..=13).contains(&x)) && (3..=6).contains(&y);
                    let eye = ((x == 3 || x == 4) || (x == 11 || x == 12)) && (4..=5).contains(&y);
                    if bulge {
                        (r, g, b) = spk((126.0, 190.0, 88.0), 6.0, n);
                    }
                    if eye {
                        (r, g, b) = spk((30.0, 28.0, 22.0), 5.0, n);
                    }
                }
                189 => {
                    // axolotl: pink with gill hints
                    let c = hx((x / 2) as i64, (y / 2) as i64, 9189);
                    (r, g, b) = if c > 0.88 {
                        spk((246.0, 170.0, 196.0), 6.0, n)
                    } else {
                        spk((232.0, 158.0, 182.0), 7.0, n)
                    };
                }
                190 => {
                    // axolotl face: happy eyes
                    (r, g, b) = spk((236.0, 164.0, 186.0), 7.0, n);
                    let eye = (y == 6 || y == 7) && ((x == 4 || x == 5) || (x == 10 || x == 11));
                    if eye {
                        (r, g, b) = spk((36.0, 30.0, 32.0), 5.0, n);
                    }
                }
                191 => {
                    // berries item: a little cluster
                    let berry = hx((x / 2) as i64, (y / 2) as i64, 9191);
                    let near = ((x % 5) == 2 || (x % 5) == 3) && ((y % 5) == 2 || (y % 5) == 3);
                    if berry > 0.35 && near {
                        (r, g, b) = spk((214.0, 48.0, 56.0), 10.0, n);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                192 => {
                    // grass side overlay: GRAYSCALE fringe, alpha below.
                    // Drawn over grass_block_side and tinted per biome.
                    let green = y < 3 || (y == 3 && n < 0.55) || (y == 4 && n < 0.15);
                    if green {
                        let l = 118.0 + (n - 0.5) * 30.0;
                        (r, g, b) = (l, l, l);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                193 => {
                    // pointed dripstone: narrow spike on transparent bg
                    let cx = (x as f32 - 7.5).abs();
                    let taper = if y < 8 { y as f32 * 0.9 } else { (15 - y) as f32 * 0.9 };
                    if cx < taper.max(0.9) {
                        let l = 120.0 + (n - 0.5) * 36.0;
                        (r, g, b) = (l, l * 0.92, l * 0.84);
                    } else {
                        (r, g, b, a) = (0.0, 0.0, 0.0, 0.0);
                    }
                }
                194 => {
                    // glow squid skin: luminous teal speckles
                    let glow = hx((x / 2) as i64, (y / 2) as i64, 9194);
                    let base = if glow > 0.75 {
                        (168.0, 238.0, 236.0)
                    } else if glow > 0.5 {
                        (110.0, 196.0, 206.0)
                    } else {
                        (64.0, 134.0, 158.0)
                    };
                    (r, g, b) = spk(base, 8.0, n);
                }
                195 => {
                    // glow squid face: teal + big eyes
                    let eye_l = x >= 2 && x <= 5 && y >= 4 && y <= 7;
                    let eye_r = x >= 10 && x <= 13 && y >= 4 && y <= 7;
                    if eye_l || eye_r {
                        let pupil = x >= 3 && x <= 4 && y >= 5 && y <= 6
                            || x >= 11 && x <= 12 && y >= 5 && y <= 6;
                        (r, g, b) = if pupil { (18.0, 20.0, 40.0) } else { (228.0, 240.0, 244.0) };
                    } else {
                        let glow = hx((x / 2) as i64, (y / 2) as i64, 9194);
                        let base = if glow > 0.6 {
                            (140.0, 214.0, 220.0)
                        } else {
                            (86.0, 164.0, 184.0)
                        };
                        (r, g, b) = spk(base, 6.0, n);
                    }
                }
                196 => {
                    // player skin: warm tan speckle
                    (r, g, b) = spk((226.0, 184.0, 152.0), 10.0, n);
                }
                197 => {
                    // player face: hair band, eyes with pupils, mouth
                    let hair = y <= 2 || (y == 3 && (x <= 1 || x >= 14));
                    let eye_l = x >= 3 && x <= 5 && y >= 6 && y <= 8;
                    let eye_r = x >= 10 && x <= 12 && y >= 6 && y <= 8;
                    if hair {
                        (r, g, b) = spk((72.0, 48.0, 28.0), 8.0, n);
                    } else if eye_l || eye_r {
                        let pupil = (x == 4 && y >= 6 && y <= 8)
                            || (x == 11 && y >= 6 && y <= 8)
                            || (x == 3 && y == 7)
                            || (x == 12 && y == 7);
                        (r, g, b) = if pupil { (44.0, 60.0, 130.0) } else { (240.0, 240.0, 240.0) };
                    } else if y == 11 && x >= 5 && x <= 10 {
                        (r, g, b) = spk((150.0, 96.0, 84.0), 6.0, n);
                    } else {
                        (r, g, b) = spk((226.0, 184.0, 152.0), 8.0, n);
                    }
                }
                198..=205 => {
                    // player shirts: 8 saturated colors with fabric shading
                    const SHIRTS: [(f32, f32, f32); 8] = [
                        (0.0, 148.0, 148.0),  // teal (default)
                        (196.0, 56.0, 48.0),  // red
                        (224.0, 132.0, 32.0), // orange
                        (216.0, 188.0, 40.0), // yellow
                        (72.0, 152.0, 56.0),  // green
                        (56.0, 84.0, 196.0),  // blue
                        (136.0, 64.0, 176.0), // purple
                        (224.0, 128.0, 176.0),// pink
                    ];
                    let c = SHIRTS[(t - 198) as usize];
                    let seam = y % 4 == 0 && n > 0.55;
                    (r, g, b) = if seam {
                        spk((c.0 * 0.8, c.1 * 0.8, c.2 * 0.8), 6.0, n)
                    } else {
                        spk(c, 9.0, n)
                    };
                }
                206 => {
                    // player pants: denim
                    let seam = y % 5 == 0 && n > 0.5;
                    (r, g, b) = if seam {
                        spk((44.0, 56.0, 104.0), 5.0, n)
                    } else {
                        spk((58.0, 72.0, 128.0), 7.0, n)
                    };
                }
                _ => {
                    (r, g, b) = (255.0, 0.0, 255.0);
                }
            }
            out.push(r.clamp(0.0, 255.0) as u8);
            out.push(g.clamp(0.0, 255.0) as u8);
            out.push(b.clamp(0.0, 255.0) as u8);
            out.push(a.clamp(0.0, 255.0) as u8);
        }
    }
}

fn zombie_base(r: &mut f32, g: &mut f32, b: &mut f32, x: u32, y: u32, n: f32) {
    let c = hx((x / 2) as i64, (y / 2) as i64, 13);
    (*r, *g, *b) = if c > 0.8 {
        spk((76.0, 118.0, 58.0), 6.0, n)
    } else if c < 0.2 {
        spk((48.0, 78.0, 40.0), 6.0, n)
    } else {
        spk((62.0, 98.0, 50.0), 8.0, n)
    };
}

fn zombie_base_dark(r: &mut f32, g: &mut f32, b: &mut f32, x: u32, y: u32, n: f32) {
    let c = hx((x / 2) as i64, (y / 2) as i64, 17);
    (*r, *g, *b) = if c > 0.7 {
        spk((66.0, 82.0, 96.0), 6.0, n)
    } else {
        spk((48.0, 60.0, 74.0), 6.0, n)
    };
}

fn flower(
    r: &mut f32,
    g: &mut f32,
    b: &mut f32,
    a: &mut f32,
    x: u32,
    y: u32,
    petal: (f32, f32, f32),
    n: f32,
) {
    let cx = 7.5f32;
    let dx = (x as f32 - cx).abs();
    let dy = (y as f32 - 4.5).abs();
    let stem = x >= 7 && x <= 8 && y >= 7 && y <= 15;
    let leaf = (x == 5 && y == 10) || (x == 10 && y == 12) || (x == 6 && y == 11);
    let petal_area = dy < 3.0 && dx < 3.0 && (dx + dy) < 4.2;
    let core = dx < 1.2 && dy < 1.2;
    if petal_area && !core {
        (*r, *g, *b) = spk(petal, 10.0, n);
    } else if core {
        (*r, *g, *b) = (250.0, 236.0, 160.0);
    } else if stem || leaf {
        (*r, *g, *b) = spk((76.0, 132.0, 52.0), 10.0, n);
    } else {
        (*r, *g, *b, *a) = (0.0, 0.0, 0.0, 0.0);
    }
}

// ------------------------------------------------------------------- program
fn compile(gl: &Gl, ty: u32, src: &[u8]) -> Result<u32, String> {
    unsafe {
        let s = (gl.create_shader)(ty);
        (gl.shader_source)(s, 1, [src.as_ptr()].as_ptr(), std::ptr::null());
        (gl.compile_shader)(s);
        let mut ok = 0i32;
        (gl.get_shaderiv)(s, gl::GL_COMPILE_STATUS, &mut ok);
        if ok == 0 {
            let mut len = 0i32;
            (gl.get_shader_info_log)(s, 0, &mut len, std::ptr::null_mut());
            let mut buf = vec![0u8; len.max(1) as usize];
            (gl.get_shader_info_log)(s, len, std::ptr::null_mut(), buf.as_mut_ptr());
            return Err(format!(
                "shader compile failed: {}",
                String::from_utf8_lossy(&buf)
            ));
        }
        Ok(s)
    }
}

fn link(gl: &Gl, vs: u32, fs: u32, bindings: &[(&[u8], u32)]) -> Result<u32, String> {
    unsafe {
        let p = (gl.create_program)();
        (gl.attach_shader)(p, vs);
        (gl.attach_shader)(p, fs);
        for (name, loc) in bindings {
            (gl.bind_attrib_location)(p, *loc, name.as_ptr());
        }
        (gl.link_program)(p);
        let mut ok = 0i32;
        (gl.get_programiv)(p, gl::GL_LINK_STATUS, &mut ok);
        if ok == 0 {
            let mut len = 0i32;
            (gl.get_program_info_log)(p, 0, &mut len, std::ptr::null_mut());
            let mut buf = vec![0u8; len.max(1) as usize];
            (gl.get_program_info_log)(p, len, std::ptr::null_mut(), buf.as_mut_ptr());
            return Err(format!(
                "program link failed: {}",
                String::from_utf8_lossy(&buf)
            ));
        }
        Ok(p)
    }
}

// ------------------------------------------------------------------ renderer
pub struct Renderer {
    pub(crate) prog: u32,
    pub(crate) prog_c: u32,
    pub(crate) prog_p: u32,
    pub(crate) atlas: u32,
    pub(crate) tex_entity: u32,
    pub(crate) dyn_vbo: u32,
    pub(crate) part_vbo: u32,
    pub(crate) cube_vbo: u32,
    pub(crate) u_mvp: i32,
    pub(crate) u_cam: i32,
    pub(crate) u_fog_near: i32,
    pub(crate) u_fog_far: i32,
    pub(crate) u_fog_col: i32,
    pub(crate) u_alpha: i32,
    pub(crate) u_light: i32,
    pub(crate) cu_mvp: i32,
    pub(crate) cu_off: i32,
    pub(crate) cu_color: i32,
    pub(crate) pp_mvp: i32,
    pub(crate) pp_psize: i32,
    pub tile_avg: Vec<[f32; 3]>,
    pub atlas_px: Vec<u8>,
    pub star_dirs: Vec<[f32; 3]>,
    pub gl: Gl,
}

impl Renderer {
    pub fn new(pack: Option<&Pack>) -> Result<Renderer, String> {
        let gl = gl::g();
        unsafe {
            // --- atlas
            let mut atlas = 0u32;
            let w = (ATLAS_COLS * TILE) as i32;
            let h = (ATLAS_ROWS * TILE) as i32;
            let px = build_atlas_pixels(pack);
            let tile_avg = compute_tile_avg(&px);

            // --- stars (fixed random directions)
            let mut star_dirs = Vec::with_capacity(170);
            for i in 0..170 {
                let u = hx(i, 1, 0xABCD) * 2.0 - 1.0;
                let phi = hx(i, 2, 0xEF01) * std::f32::consts::TAU;
                let s = (1.0 - u * u).sqrt().max(0.001);
                star_dirs.push([s * phi.cos(), u.abs() * 0.9 + 0.08, s * phi.sin()]);
            }

            (gl.gen_textures)(1, &mut atlas);
            (gl.bind_texture)(gl::GL_TEXTURE_2D, atlas);
            (gl.pixel_storei)(gl::GL_UNPACK_ALIGNMENT, 1);
            (gl.tex_image_2d)(
                gl::GL_TEXTURE_2D,
                0,
                gl::GL_RGBA8,
                w,
                h,
                0,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE as u32,
                px.as_ptr() as *const _,
            );
            (gl.tex_parameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MIN_FILTER, gl::GL_NEAREST);
            (gl.tex_parameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MAG_FILTER, gl::GL_NEAREST);
            (gl.tex_parameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_WRAP_S, gl::GL_CLAMP);
            (gl.tex_parameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_WRAP_T, gl::GL_CLAMP);

            // --- texture entités (atlas d'exécution, absent par défaut)
            let mut tex_entity = 0u32;
            (gl.gen_textures)(1, &mut tex_entity);

            // --- programs (attribute locations bound BEFORE linking)
            let prog = {
                let vs = compile(&gl, gl::GL_VERTEX_SHADER, VS_MAIN)?;
                let fs = compile(&gl, gl::GL_FRAGMENT_SHADER, FS_MAIN)?;
                let p = link(
                    &gl,
                    vs,
                    fs,
                    &[(b"a_pos\0", 0), (b"a_uv\0", 1), (b"a_rgb\0", 2)],
                );
                (gl.delete_shader)(vs);
                (gl.delete_shader)(fs);
                p?
            };

            let prog_c = {
                let vs = compile(&gl, gl::GL_VERTEX_SHADER, VS_COLOR)?;
                let fs = compile(&gl, gl::GL_FRAGMENT_SHADER, FS_COLOR)?;
                let p = link(&gl, vs, fs, &[(b"a_pos\0", 0)]);
                (gl.delete_shader)(vs);
                (gl.delete_shader)(fs);
                p?
            };

            let prog_p = {
                let vs = compile(&gl, gl::GL_VERTEX_SHADER, VS_POINT)?;
                let fs = compile(&gl, gl::GL_FRAGMENT_SHADER, FS_POINT)?;
                let p = link(&gl, vs, fs, &[(b"a_pos\0", 0), (b"a_col\0", 1)]);
                (gl.delete_shader)(vs);
                (gl.delete_shader)(fs);
                p?
            };

            // --- dynamic VBOs + selection cube VBO
            let mut dyn_vbo = 0u32;
            let mut part_vbo = 0u32;
            let mut cube_vbo = 0u32;
            (gl.gen_buffers)(1, &mut dyn_vbo);
            (gl.gen_buffers)(1, &mut part_vbo);
            (gl.gen_buffers)(1, &mut cube_vbo);
            let mut cube = Vec::new();
            push_cube_lines(&mut cube);
            (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, cube_vbo);
            (gl.buffer_data)(
                gl::GL_ARRAY_BUFFER,
                (cube.len() * 4) as isize,
                cube.as_ptr() as *const _,
                gl::GL_STATIC_DRAW,
            );
            (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, 0);

            let u = |p: u32, n: &[u8]| (gl.get_uniform_location)(p, n.as_ptr());
            let r = Renderer {
                prog,
                prog_c,
                prog_p,
                atlas,
                tex_entity,
                dyn_vbo,
                part_vbo,
                cube_vbo,
                u_mvp: u(prog, b"u_mvp\0"),
                u_cam: u(prog, b"u_cam\0"),
                u_fog_near: u(prog, b"u_fog_near\0"),
                u_fog_far: u(prog, b"u_fog_far\0"),
                u_fog_col: u(prog, b"u_fog_col\0"),
                u_alpha: u(prog, b"u_alpha\0"),
                u_light: u(prog, b"u_light\0"),
                cu_mvp: u(prog_c, b"u_mvp\0"),
                cu_off: u(prog_c, b"u_off\0"),
                cu_color: u(prog_c, b"u_color\0"),
                pp_mvp: u(prog_p, b"u_mvp\0"),
                pp_psize: u(prog_p, b"u_psize\0"),
                tile_avg,
                atlas_px: px,
                star_dirs,
                gl,
            };
            (gl.use_program)(prog);
            (gl.uniform1i)(r.u_tex(), 0);
            Ok(r)
        }
    }

    /// Rebuild the atlas from disk (F4): picks up pack changes instantly.
    pub fn reload_atlas(&mut self, pack: Option<&Pack>) {
        let px = build_atlas_pixels(pack);
        self.tile_avg = compute_tile_avg(&px);
        self.atlas_px = px;
        unsafe {
            let gl = &self.gl;
            let w = (ATLAS_COLS * TILE) as i32;
            let h = (ATLAS_ROWS * TILE) as i32;
            (gl.bind_texture)(gl::GL_TEXTURE_2D, self.atlas);
            (gl.pixel_storei)(gl::GL_UNPACK_ALIGNMENT, 1);
            (gl.tex_image_2d)(
                gl::GL_TEXTURE_2D,
                0,
                gl::GL_RGBA8,
                w,
                h,
                0,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE as u32,
                self.atlas_px.as_ptr() as *const _,
            );
        }
    }

    /// Export every atlas tile that maps to a pack name as individual PNGs
    /// (a ready-to-edit texture pack template).
    pub fn export_tiles(&self, dir: &std::path::Path) -> std::io::Result<usize> {
        std::fs::create_dir_all(dir)?;
        let mut count = 0;
        for t in 0..(ATLAS_COLS * ATLAS_ROWS) {
            let cands = tile_candidates(t);
            if cands.is_empty() {
                continue;
            }
            let tx = (t % ATLAS_COLS) * TILE;
            let ty = (t / ATLAS_COLS) * TILE;
            let mut rgba = Vec::with_capacity((TILE * TILE * 4) as usize);
            for y in 0..TILE {
                for x in 0..TILE {
                    let o = (((ty + y) * ATLAS_COLS * TILE + tx + x) * 4) as usize;
                    rgba.extend_from_slice(&self.atlas_px[o..o + 4]);
                }
            }
            let png = crate::pack::png_encode(TILE, TILE, &rgba);
            let name = format!("{}.png", cands[0]);
            std::fs::write(dir.join(name), png)?;
            count += 1;
        }
        Ok(count)
    }

    pub(crate) fn u_tex(&self) -> i32 {
        unsafe { (self.gl.get_uniform_location)(self.prog, b"u_tex\0".as_ptr()) }
    }

    pub fn upload_mesh(&self, m: &mut ChunkMesh, data: &crate::mesher::MeshData) {
        unsafe {
            self.free_mesh(m);
            if !data.solid_v.is_empty() {
                (self.gl.gen_buffers)(1, &mut m.vbo);
                (self.gl.bind_buffer)(gl::GL_ARRAY_BUFFER, m.vbo);
                (self.gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (data.solid_v.len() * 4) as isize,
                    data.solid_v.as_ptr() as *const _,
                    gl::GL_STATIC_DRAW,
                );
                (self.gl.bind_buffer)(gl::GL_ELEMENT_ARRAY_BUFFER, 0);
                (self.gl.gen_buffers)(1, &mut m.ibo);
                (self.gl.bind_buffer)(gl::GL_ELEMENT_ARRAY_BUFFER, m.ibo);
                (self.gl.buffer_data)(
                    gl::GL_ELEMENT_ARRAY_BUFFER,
                    (data.solid_i.len() * 4) as isize,
                    data.solid_i.as_ptr() as *const _,
                    gl::GL_STATIC_DRAW,
                );
                m.n = data.solid_i.len();
            }
            if !data.cutout_v.is_empty() {
                (self.gl.gen_buffers)(1, &mut m.cvbo);
                (self.gl.bind_buffer)(gl::GL_ARRAY_BUFFER, m.cvbo);
                (self.gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (data.cutout_v.len() * 4) as isize,
                    data.cutout_v.as_ptr() as *const _,
                    gl::GL_STATIC_DRAW,
                );
                (self.gl.gen_buffers)(1, &mut m.cibo);
                (self.gl.bind_buffer)(gl::GL_ELEMENT_ARRAY_BUFFER, m.cibo);
                (self.gl.buffer_data)(
                    gl::GL_ELEMENT_ARRAY_BUFFER,
                    (data.cutout_i.len() * 4) as isize,
                    data.cutout_i.as_ptr() as *const _,
                    gl::GL_STATIC_DRAW,
                );
                m.cn = data.cutout_i.len();
            }
            if !data.water_v.is_empty() {
                (self.gl.gen_buffers)(1, &mut m.wvbo);
                (self.gl.bind_buffer)(gl::GL_ARRAY_BUFFER, m.wvbo);
                (self.gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (data.water_v.len() * 4) as isize,
                    data.water_v.as_ptr() as *const _,
                    gl::GL_STATIC_DRAW,
                );
                (self.gl.gen_buffers)(1, &mut m.wibo);
                (self.gl.bind_buffer)(gl::GL_ELEMENT_ARRAY_BUFFER, m.wibo);
                (self.gl.buffer_data)(
                    gl::GL_ELEMENT_ARRAY_BUFFER,
                    (data.water_i.len() * 4) as isize,
                    data.water_i.as_ptr() as *const _,
                    gl::GL_STATIC_DRAW,
                );
                m.wn = data.water_i.len();
            }
            (self.gl.bind_buffer)(gl::GL_ELEMENT_ARRAY_BUFFER, 0);
        }
    }

    pub fn free_mesh(&self, m: &mut ChunkMesh) {
        unsafe {
            if m.n > 0 {
                (self.gl.delete_buffers)(1, [m.vbo].as_ptr());
                (self.gl.delete_buffers)(1, [m.ibo].as_ptr());
                m.n = 0;
            }
            if m.cn > 0 {
                (self.gl.delete_buffers)(1, [m.cvbo].as_ptr());
                (self.gl.delete_buffers)(1, [m.cibo].as_ptr());
                m.cn = 0;
            }
            if m.wn > 0 {
                (self.gl.delete_buffers)(1, [m.wvbo].as_ptr());
                (self.gl.delete_buffers)(1, [m.wibo].as_ptr());
                m.wn = 0;
            }
        }
    }

    unsafe fn set_full_attribs(&self) {
        let gl = &self.gl;
        (gl.vertex_attrib_pointer)(0, 3, gl::GL_FLOAT, gl::GL_FALSE, 32, 0);
        (gl.vertex_attrib_pointer)(1, 2, gl::GL_FLOAT, gl::GL_FALSE, 32, 12);
        (gl.vertex_attrib_pointer)(2, 3, gl::GL_FLOAT, gl::GL_FALSE, 32, 20);
        (gl.enable_vertex_attrib_array)(0);
        (gl.enable_vertex_attrib_array)(1);
        (gl.enable_vertex_attrib_array)(2);
    }

    unsafe fn set_pos_attrib(&self) {
        let gl = &self.gl;
        (gl.vertex_attrib_pointer)(0, 3, gl::GL_FLOAT, gl::GL_FALSE, 12, 0);
        (gl.enable_vertex_attrib_array)(0);
        (gl.disable_vertex_attrib_array)(1);
        (gl.disable_vertex_attrib_array)(2);
    }

    unsafe fn set_point_attribs(&self) {
        let gl = &self.gl;
        (gl.vertex_attrib_pointer)(0, 3, gl::GL_FLOAT, gl::GL_FALSE, 28, 0);
        (gl.vertex_attrib_pointer)(1, 4, gl::GL_FLOAT, gl::GL_FALSE, 28, 12);
        (gl.enable_vertex_attrib_array)(0);
        (gl.enable_vertex_attrib_array)(1);
    }

    pub(crate) unsafe fn set_full_attribs_pub(&self) {
        self.set_full_attribs();
    }

    pub(crate) unsafe fn set_pos_attrib_pub(&self) {
        self.set_pos_attrib();
    }

    /// Charge l'atlas d'entités (textures de mobs du pack, pleine résolution).
    pub fn upload_entity_atlas(&mut self, a: &crate::entity_models::EntityAtlas) {
        unsafe {
            let gl = &self.gl;
            (gl.bind_texture)(gl::GL_TEXTURE_2D, self.tex_entity);
            (gl.pixel_storei)(gl::GL_UNPACK_ALIGNMENT, 1);
            (gl.tex_image_2d)(
                gl::GL_TEXTURE_2D,
                0,
                gl::GL_RGBA8,
                a.w as i32,
                a.h as i32,
                0,
                gl::GL_RGBA,
                gl::GL_UNSIGNED_BYTE as u32,
                a.px.as_ptr() as *const _,
            );
            (gl.tex_parameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MIN_FILTER, gl::GL_NEAREST);
            (gl.tex_parameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MAG_FILTER, gl::GL_NEAREST);
            (gl.tex_parameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_WRAP_S, gl::GL_CLAMP);
            (gl.tex_parameteri)(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_WRAP_T, gl::GL_CLAMP);
            (gl.bind_texture)(gl::GL_TEXTURE_2D, self.atlas);
        }
    }

    pub fn has_entity_atlas(&self) -> bool {
        self.tex_entity != 0
    }

    /// Draw the world + HUD.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_frame(
        &self,
        meshes: &HashMap<(i32, i32), ChunkMesh>,
        player: &Player,
        sel: Option<&RayHit>,
        w: i32,
        h: i32,
        fov_deg: f32,
        fog_near: f32,
        fog_far: f32,
        fog_col: [f32; 3],
        light: f32,
        sun_dir: Vec3,
        night: f32,
        time: f32,
        mob_verts: &[f32],
        mob_verts_pack: &[f32],
        particles: &[f32],
        mine_overlay: Option<(i32, i32, i32, f32)>,
        painter: &mut crate::ui::Painter,
        gui_scale: f32,
        tags: &[(Vec3, String)],
    ) {
        if w <= 0 || h <= 0 {
            return;
        }
        let (w, h) = (w as f32, h as f32);
        let eye = player.eye();
        let proj = perspective(fov_deg.to_radians(), w / h, 0.1, 420.0);
        let view = view_matrix(eye, player.yaw, player.pitch);
        let mvp: Mat4 = mat_mul(&proj, &view);
        let fwd = crate::math::forward(player.yaw, player.pitch);

        unsafe {
            let gl = &self.gl;
            (gl.viewport)(0, 0, w as i32, h as i32);
            (gl.clear_color)(fog_col[0], fog_col[1], fog_col[2], 1.0);
            (gl.clear)(gl::GL_COLOR_BUFFER_BIT | gl::GL_DEPTH_BUFFER_BIT);
            (gl.enable)(gl::GL_DEPTH_TEST);
            (gl.enable)(gl::GL_CULL_FACE);
            (gl.disable)(gl::GL_BLEND);

            (gl.use_program)(self.prog);
            (gl.uniform_matrix4fv)(self.u_mvp, 1, gl::GL_FALSE, mvp.as_ptr());
            (gl.uniform3f)(self.u_cam, eye.x, eye.y, eye.z);
            (gl.uniform1f)(self.u_fog_near, fog_near);
            (gl.uniform1f)(self.u_fog_far, fog_far);
            (gl.uniform3f)(self.u_fog_col, fog_col[0], fog_col[1], fog_col[2]);
            (gl.uniform1f)(self.u_alpha, 1.0);
            (gl.uniform1f)(self.u_light, light);
            (gl.active_texture)(gl::GL_TEXTURE0);
            (gl.bind_texture)(gl::GL_TEXTURE_2D, self.atlas);

            // ---- solid pass
            for (c, m) in meshes {
                if m.n == 0 {
                    continue;
                }
                // cheap backface chunk cull
                let cx = (c.0 * 16 + 8) as f32;
                let cz = (c.1 * 16 + 8) as f32;
                let to = Vec3::new(cx - eye.x, 24.0 - eye.y, cz - eye.z);
                if to.dot(fwd) < -20.0 {
                    continue;
                }
                (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, m.vbo);
                self.set_full_attribs();
                (gl.bind_buffer)(gl::GL_ELEMENT_ARRAY_BUFFER, m.ibo);
                (gl.draw_elements)(
                    gl::GL_TRIANGLES,
                    m.n as i32,
                    gl::GL_UNSIGNED_INT,
                    std::ptr::null(),
                );
            }

            // ---- cutout pass (cross plants, both sides)
            (gl.disable)(gl::GL_CULL_FACE);
            for (c, m) in meshes {
                if m.cn == 0 {
                    continue;
                }
                let cx = (c.0 * 16 + 8) as f32;
                let cz = (c.1 * 16 + 8) as f32;
                let to = Vec3::new(cx - eye.x, 24.0 - eye.y, cz - eye.z);
                if to.dot(fwd) < -20.0 {
                    continue;
                }
                (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, m.cvbo);
                self.set_full_attribs();
                (gl.bind_buffer)(gl::GL_ELEMENT_ARRAY_BUFFER, m.cibo);
                (gl.draw_elements)(
                    gl::GL_TRIANGLES,
                    m.cn as i32,
                    gl::GL_UNSIGNED_INT,
                    std::ptr::null(),
                );
            }
            (gl.enable)(gl::GL_CULL_FACE);

            // ---- mob pass (textured, same vertex layout)
            // 1) rigs avec les textures d'entités du pack (atlas séparé)
            if !mob_verts_pack.is_empty() && self.tex_entity != 0 {
                (gl.active_texture)(gl::GL_TEXTURE0);
                (gl.bind_texture)(gl::GL_TEXTURE_2D, self.tex_entity);
                (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, self.dyn_vbo);
                (gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (mob_verts_pack.len() * 4) as isize,
                    mob_verts_pack.as_ptr() as *const _,
                    gl::GL_DYNAMIC_DRAW,
                );
                self.set_full_attribs();
                (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (mob_verts_pack.len() / 8) as i32);
                (gl.bind_texture)(gl::GL_TEXTURE_2D, self.atlas);
            }
            // 2) rigs procéduraux (atlas principal)
            if !mob_verts.is_empty() {
                (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, self.dyn_vbo);
                (gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (mob_verts.len() * 4) as isize,
                    mob_verts.as_ptr() as *const _,
                    gl::GL_DYNAMIC_DRAW,
                );
                self.set_full_attribs();
                (gl.draw_arrays)(
                    gl::GL_TRIANGLES,
                    0,
                    (mob_verts.len() / 8) as i32,
                );
            }

            // ---- mining progress overlay (black growing on the targeted block)
            if let Some((bx, by, bz, p)) = mine_overlay {
                if p > 0.01 {
                    (gl.use_program)(self.prog_c);
                    (gl.enable)(gl::GL_BLEND);
                    (gl.blend_func)(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);
                    (gl.disable)(gl::GL_CULL_FACE);
                    (gl.uniform_matrix4fv)(self.cu_mvp, 1, gl::GL_FALSE, mvp.as_ptr());
                    (gl.uniform3f)(self.cu_off, bx as f32, by as f32, bz as f32);
                    (gl.uniform4f)(self.cu_color, 0.05, 0.04, 0.04, (p * 0.6).min(0.65));
                    let mut ov = Vec::with_capacity(36 * 3);
                    for d in 0..6 {
                        for c in 0..4 {
                            let co = CORNERS[d][c];
                            let e = 0.003;
                            let vx = if co[0] == 1 { 1.0 + e } else { -e };
                            let vy = if co[1] == 1 { 1.0 + e } else { -e };
                            let vz = if co[2] == 1 { 1.0 + e } else { -e };
                            ov.extend_from_slice(&[vx, vy, vz]);
                        }
                        // two triangles: 0,1,2  0,2,3
                        let base = ov.len() / 3 - 4;
                        let idx = [base, base + 1, base + 2, base, base + 2, base + 3];
                        let mut quad = Vec::with_capacity(18);
                        for i in idx {
                            quad.extend_from_slice(&ov[i * 3..i * 3 + 3]);
                        }
                        ov.truncate(base * 3);
                        ov.extend_from_slice(&quad);
                    }
                    (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, self.dyn_vbo);
                    (gl.buffer_data)(
                        gl::GL_ARRAY_BUFFER,
                        (ov.len() * 4) as isize,
                        ov.as_ptr() as *const _,
                        gl::GL_DYNAMIC_DRAW,
                    );
                    self.set_pos_attrib_pub();
                    (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (ov.len() / 3) as i32);
                    (gl.enable)(gl::GL_CULL_FACE);
                    (gl.disable)(gl::GL_BLEND);
                    (gl.use_program)(self.prog);
                }
            }

            // ---- points: stars + particles
            (gl.use_program)(self.prog_p);
            (gl.enable)(gl::GL_BLEND);
            (gl.blend_func)(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);
            (gl.depth_mask)(0);
            (gl.uniform_matrix4fv)(self.pp_mvp, 1, gl::GL_FALSE, mvp.as_ptr());
            (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, self.part_vbo);

            if night > 0.01 {
                let mut stars = Vec::with_capacity(self.star_dirs.len() * 7);
                for (i, d) in self.star_dirs.iter().enumerate() {
                    let tw = 0.55
                        + 0.45 * (time * 2.2 + hx(i as i64, 9, 5) * 6.28).sin();
                    let a = night * tw;
                    stars.extend_from_slice(&[
                        eye.x + d[0] * 390.0,
                        eye.y + d[1] * 390.0,
                        eye.z + d[2] * 390.0,
                        0.95,
                        0.95,
                        1.0,
                        a,
                    ]);
                }
                (gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (stars.len() * 4) as isize,
                    stars.as_ptr() as *const _,
                    gl::GL_DYNAMIC_DRAW,
                );
                self.set_point_attribs();
                (gl.uniform1f)(self.pp_psize, 2.0);
                (gl.draw_arrays)(
                    gl::GL_POINTS,
                    0,
                    (stars.len() / 7) as i32,
                );
            }

            if !particles.is_empty() {
                (gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (particles.len() * 4) as isize,
                    particles.as_ptr() as *const _,
                    gl::GL_DYNAMIC_DRAW,
                );
                self.set_point_attribs();
                (gl.uniform1f)(self.pp_psize, 3.0);
                (gl.draw_arrays)(
                    gl::GL_POINTS,
                    0,
                    (particles.len() / 7) as i32,
                );
            }
            (gl.depth_mask)(1);
            (gl.disable)(gl::GL_BLEND);

            // ---- sun & moon billboards (textured, fog-free)
            (gl.use_program)(self.prog);
            (gl.uniform1f)(self.u_fog_near, 1.0e9);
            (gl.uniform1f)(self.u_fog_far, 2.0e9);
            (gl.uniform1f)(self.u_alpha, 1.0);
            (gl.enable)(gl::GL_BLEND);
            (gl.blend_func)(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);
            (gl.depth_mask)(0);
            (gl.disable)(gl::GL_CULL_FACE);
            for (dir, tile) in [(sun_dir, T_SUN), (sun_dir * -1.0, T_MOON)] {
                let d = dir.normalize();
                if d.y < -0.3 {
                    continue; // below horizon enough
                }
                let center = eye + d * 350.0;
                let up_ref = if d.y.abs() > 0.95 {
                    Vec3::new(1.0, 0.0, 0.0)
                } else {
                    Vec3::new(0.0, 1.0, 0.0)
                };
                let r = d.cross(up_ref).normalize();
                let u2 = r.cross(d).normalize();
                let s = 34.0;
                let p = |a: f32, b: f32| center + r * (a * s) + u2 * (b * s);
                let tsu = 1.0 / ATLAS_COLS as f32;
                let tsv = 1.0 / ATLAS_ROWS as f32;
                let (tu, tv) = ((tile % ATLAS_COLS) as f32 * tsu, (tile / ATLAS_COLS) as f32 * tsv);
                let e = 0.002;
                let mut verts: Vec<f32> = Vec::with_capacity(36);
                let push = |verts: &mut Vec<f32>, pos: Vec3, uu: f32, vv: f32| {
                    verts.extend_from_slice(&[pos.x, pos.y, pos.z, uu, vv, 1.0, 1.0, 1.0]);
                };
                let (u0, v0, u1, v1) = (
                    tu + e,
                    tv + e,
                    tu + tsu - e,
                    tv + tsv - e,
                );
                push(&mut verts, p(-1.0, 1.0), u0, v0);
                push(&mut verts, p(-1.0, -1.0), u0, v1);
                push(&mut verts, p(1.0, -1.0), u1, v1);
                push(&mut verts, p(-1.0, 1.0), u0, v0);
                push(&mut verts, p(1.0, -1.0), u1, v1);
                push(&mut verts, p(1.0, 1.0), u1, v0);
                (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, self.dyn_vbo);
                (gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (verts.len() * 4) as isize,
                    verts.as_ptr() as *const _,
                    gl::GL_DYNAMIC_DRAW,
                );
                self.set_full_attribs();
                (gl.draw_arrays)(gl::GL_TRIANGLES, 0, 6);
            }
            (gl.enable)(gl::GL_CULL_FACE);
            (gl.depth_mask)(1);
            (gl.disable)(gl::GL_BLEND);
            (gl.uniform1f)(self.u_fog_near, fog_near);
            (gl.uniform1f)(self.u_fog_far, fog_far);

            // ---- water pass
            (gl.disable)(gl::GL_CULL_FACE);
            (gl.enable)(gl::GL_BLEND);
            (gl.blend_func)(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);
            (gl.depth_mask)(0);
            (gl.uniform1f)(self.u_alpha, 0.85);
            for (c, m) in meshes {
                if m.wn == 0 {
                    continue;
                }
                let cx = (c.0 * 16 + 8) as f32;
                let cz = (c.1 * 16 + 8) as f32;
                let to = Vec3::new(cx - eye.x, 24.0 - eye.y, cz - eye.z);
                if to.dot(fwd) < -20.0 {
                    continue;
                }
                (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, m.wvbo);
                self.set_full_attribs();
                (gl.bind_buffer)(gl::GL_ELEMENT_ARRAY_BUFFER, m.wibo);
                (gl.draw_elements)(
                    gl::GL_TRIANGLES,
                    m.wn as i32,
                    gl::GL_UNSIGNED_INT,
                    std::ptr::null(),
                );
            }
            (gl.depth_mask)(1);
            (gl.disable)(gl::GL_BLEND);
            (gl.enable)(gl::GL_CULL_FACE);

            // ---- HUD (no depth): nametags go into the painter (GUI coords)
            (gl.disable)(gl::GL_DEPTH_TEST);
            if let Some(hit) = sel {
                (gl.enable)(gl::GL_DEPTH_TEST);
                crate::hud::draw_selection(self, &mvp, hit);
                (gl.disable)(gl::GL_DEPTH_TEST);
            }

            // ---- screen tints: underwater + hurt (death is drawn by the UI)
            let ortho = ortho_pixels(w, h);
            (gl.use_program)(self.prog_c);
            (gl.uniform_matrix4fv)(self.cu_mvp, 1, gl::GL_FALSE, ortho.as_ptr());
            (gl.uniform3f)(self.cu_off, 0.0, 0.0, 0.0);
            (gl.enable)(gl::GL_BLEND);
            (gl.blend_func)(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);
            (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, self.dyn_vbo);
            let mut tint: Vec<f32> = Vec::with_capacity(18);
            if player.in_water {
                push_rect(&mut tint, 0.0, 0.0, w, h);
                (gl.uniform4f)(self.cu_color, 0.12, 0.3, 0.6, 0.22);
                (gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (tint.len() * 4) as isize,
                    tint.as_ptr() as *const _,
                    gl::GL_DYNAMIC_DRAW,
                );
                self.set_pos_attrib_pub();
                (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (tint.len() / 3) as i32);
                tint.clear();
            }
            if player.hurt_flash > 0.0 {
                push_rect(&mut tint, 0.0, 0.0, w, h);
                (gl.uniform4f)(
                    self.cu_color,
                    0.65,
                    0.05,
                    0.05,
                    (player.hurt_flash * 0.9).min(0.5),
                );
                (gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (tint.len() * 4) as isize,
                    tint.as_ptr() as *const _,
                    gl::GL_DYNAMIC_DRAW,
                );
                self.set_pos_attrib_pub();
                (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (tint.len() / 3) as i32);
                tint.clear();
            }
            (gl.disable)(gl::GL_BLEND);

            // ---- player nametags (GUI coords, painter executed by the app)
            for (pos, name) in tags {
                let wp = *pos + Vec3::new(0.0, 2.35, 0.0);
                let rel = wp - eye;
                let view_rel = Vec3::new(
                    mvp[0] * rel.x + mvp[4] * rel.y + mvp[8] * rel.z,
                    mvp[1] * rel.x + mvp[5] * rel.y + mvp[9] * rel.z,
                    mvp[2] * rel.x + mvp[6] * rel.y + mvp[10] * rel.z,
                );
                let cw = mvp[3] * rel.x + mvp[7] * rel.y + mvp[11] * rel.z + mvp[15];
                if cw < 0.1 {
                    continue;
                }
                let ndc = (view_rel.x / cw, view_rel.y / cw);
                if ndc.0 < -1.2 || ndc.0 > 1.2 || ndc.1 < -1.2 || ndc.1 > 1.2 {
                    continue;
                }
                let d = rel.length();
                if d > 40.0 {
                    continue;
                }
                let px = (ndc.0 * 0.5 + 0.5) * w / gui_scale;
                let py = (1.0 - (ndc.1 * 0.5 + 0.5)) * h / gui_scale;
                let tw = crate::ui::text_w(name);
                let a = if d > 30.0 { (40.0 - d) / 10.0 } else { 1.0 };
                painter.rect(px - tw * 0.5 - 2.0, py - 1.5, px + tw * 0.5 + 2.0, py + 8.5, [0.0, 0.0, 0.0, 0.28 * a]);
                crate::ui::text(painter, name, px - tw * 0.5, py, [1.0, 1.0, 1.0, 0.95 * a], true);
            }
        }
    }

    /// Execute a UI painter: colored rects on the color program, textured
    /// quads (icons, dirt tiles) on the atlas program. GUI ortho expected.
    pub fn exec_painter(&self, p: &crate::ui::Painter, ortho: &Mat4) {
        let gl = &self.gl;
        unsafe {
            (gl.disable)(gl::GL_DEPTH_TEST);
            (gl.enable)(gl::GL_BLEND);
            (gl.blend_func)(gl::GL_SRC_ALPHA, gl::GL_ONE_MINUS_SRC_ALPHA);
            (gl.bind_buffer)(gl::GL_ARRAY_BUFFER, self.dyn_vbo);
            // consecutive same-color rects share one draw call
            let mut batch: Vec<f32> = Vec::with_capacity(4096);
            let mut cur_col: Option<[f32; 4]> = None;
            let flush = |batch: &mut Vec<f32>, gl: &Gl, r: &Renderer| {
                if batch.is_empty() {
                    return;
                }
                (gl.buffer_data)(
                    gl::GL_ARRAY_BUFFER,
                    (batch.len() * 4) as isize,
                    batch.as_ptr() as *const _,
                    gl::GL_DYNAMIC_DRAW,
                );
                r.set_pos_attrib_pub();
                (gl.draw_arrays)(gl::GL_TRIANGLES, 0, (batch.len() / 3) as i32);
                batch.clear();
            };
            for op in &p.ops {
                match op {
                    crate::ui::Op::Rect(r) => {
                        if cur_col != Some(r.col) {
                            flush(&mut batch, gl, self);
                            (gl.use_program)(self.prog_c);
                            (gl.uniform_matrix4fv)(self.cu_mvp, 1, gl::GL_FALSE, ortho.as_ptr());
                            (gl.uniform3f)(self.cu_off, 0.0, 0.0, 0.0);
                            (gl.uniform4f)(self.cu_color, r.col[0], r.col[1], r.col[2], r.col[3]);
                            cur_col = Some(r.col);
                        }
                        push_rect(&mut batch, r.x0, r.y0, r.x1, r.y1);
                    }
                    crate::ui::Op::Tex(t) => {
                        flush(&mut batch, gl, self);
                        cur_col = None;
                        (gl.use_program)(self.prog);
                        (gl.uniform_matrix4fv)(self.u_mvp, 1, gl::GL_FALSE, ortho.as_ptr());
                        (gl.uniform1f)(self.u_fog_near, 1.0e9);
                        (gl.uniform1f)(self.u_fog_far, 2.0e9);
                        (gl.uniform1f)(self.u_alpha, 1.0);
                        (gl.uniform1f)(self.u_light, 1.0);
                        (gl.active_texture)(gl::GL_TEXTURE0);
                        (gl.bind_texture)(gl::GL_TEXTURE_2D, self.atlas);
                        let mut v = Vec::with_capacity(48);
                        for &(i0, i1, i2) in &[(0usize, 1usize, 2usize), (0, 2, 3)] {
                            for &i in &[i0, i1, i2] {
                                let sh = t.shade[i];
                                v.extend_from_slice(&[
                                    t.xy[i][0], t.xy[i][1], 0.0,
                                    t.uv[i][0], t.uv[i][1],
                                    t.tint[0] * sh, t.tint[1] * sh, t.tint[2] * sh,
                                ]);
                            }
                        }
                        (gl.buffer_data)(
                            gl::GL_ARRAY_BUFFER,
                            (v.len() * 4) as isize,
                            v.as_ptr() as *const _,
                            gl::GL_DYNAMIC_DRAW,
                        );
                        self.set_full_attribs();
                        (gl.draw_arrays)(gl::GL_TRIANGLES, 0, 6);
                    }
                }
            }
            flush(&mut batch, gl, self);
            (gl.disable)(gl::GL_BLEND);
        }
    }
}

pub(crate) fn push_rect(v: &mut Vec<f32>, x0: f32, y0: f32, x1: f32, y1: f32) {
    let quad = |ax: f32, ay: f32, out: &mut Vec<f32>| out.extend_from_slice(&[ax, ay, 0.0]);
    quad(x0, y0, v);
    quad(x0, y1, v);
    quad(x1, y1, v);
    quad(x0, y0, v);
    quad(x1, y1, v);
    quad(x1, y0, v);
}

fn push_cube_lines(out: &mut Vec<f32>) {
    let s = 0.004; // slight inflation around the block
    let (lo, hi) = (-s, 1.0 + s);
    let p = |x: f32, y: f32, z: f32| [x, y, z];
    let corners: [[f32; 3]; 8] = [
        p(lo, lo, lo),
        p(hi, lo, lo),
        p(hi, lo, hi),
        p(lo, lo, hi),
        p(lo, hi, lo),
        p(hi, hi, lo),
        p(hi, hi, hi),
        p(lo, hi, hi),
    ];
    let pairs = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    for (a, b) in pairs {
        let (a, b) = (corners[a], corners[b]);
        out.extend_from_slice(&[a[0], a[1], a[2], b[0], b[1], b[2]]);
    }
}

#[cfg(test)]
mod atlas_tests {
    use super::{ATLAS_COLS, ATLAS_ROWS};
    #[test]
    #[ignore] // run with: cargo test --features fficheck dump_atlas -- --ignored
    fn dump_atlas_for_review() {
        let mut px = Vec::new();
        for t in 0..ATLAS_COLS * ATLAS_ROWS {
            super::tile_pixels(t, &mut px);
        }
        let path = std::env::temp_dir().join("rustvoxel_atlas.rgba");
        std::fs::write(&path, &px).unwrap();
        println!("atlas dumped to {:?}", path);
    }

    /// Export the full procedural atlas (no pack) as a reviewable PNG.
    #[test]
    #[ignore] // run with: cargo test --features fficheck atlas_png -- --ignored
    fn atlas_review_png() {
        let px = super::build_atlas_pixels(None);
        let w = (ATLAS_COLS * 16) as usize;
        let h = (ATLAS_ROWS * 16) as usize;
        // nearest 2x upscale so tiles are easy to eyeball
        let mut big = Vec::with_capacity(w * h * 4 * 4);
        for y in 0..h {
            let row = &px[y * w * 4..(y + 1) * w * 4];
            for _ in 0..2 {
                for x in 0..w {
                    let c = &row[x * 4..x * 4 + 4];
                    big.extend_from_slice(c);
                    big.extend_from_slice(c);
                }
            }
        }
        let png = crate::pack::png_encode((w * 2) as u32, (h * 2) as u32, &big);
        let path = std::path::Path::new("docs/atlas_procedural_x2.png");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &png).unwrap();
        println!("atlas review written to {:?}", path);
    }

    /// REGRESSION: every tile referenced by any block id must be actually
    /// drawn by the procedural generator. The generator paints undefined
    /// slots magenta; a magenta pixel in a referenced tile means a block
    /// would show up magenta in the world.
    #[test]
    fn referenced_tiles_are_defined() {
        let mut bad: Vec<String> = Vec::new();
        for b in 1u16..crate::world::MAX_BLOCK_ID {
            for t in crate::world::tiles_of(b) {
                let mut px = Vec::new();
                super::tile_pixels(t, &mut px);
                let mut magenta = false;
                for p in px.chunks(4) {
                    if p[0] > 200 && p[1] < 60 && p[2] > 200 {
                        magenta = true;
                        break;
                    }
                }
                if magenta {
                    bad.push(format!("block {} -> tile {}", b, t));
                }
            }
        }
        assert!(bad.is_empty(), "undefined tiles referenced: {:?}", bad);
    }
}
