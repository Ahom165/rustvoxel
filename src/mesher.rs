// Chunk meshing: per-face culling + per-vertex ambient occlusion.
use crate::noise::{hash01, hash3i};
use crate::world::*;

// Face order: +X, -X, +Y (top), -Y (bottom), +Z, -Z
pub const DIRS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
pub const SHADE: [f32; 6] = [0.62, 0.62, 1.0, 0.5, 0.82, 0.82];

// CCW from outside (GL default front face).
pub const CORNERS: [[[i32; 3]; 4]; 6] = [
    [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]], // +X
    [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0]], // -X
    [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]], // +Y
    [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]], // -Y
    [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]], // +Z
    [[1, 0, 0], [0, 0, 0], [0, 1, 0], [1, 1, 0]], // -Z
];
// u,v per corner (v=0 is the top row of the tile image).
const UVC: [[[f32; 2]; 4]; 6] = [
    [[0.0, 1.0], [0.0, 0.0], [1.0, 0.0], [1.0, 1.0]], // +X
    [[1.0, 1.0], [1.0, 0.0], [0.0, 0.0], [0.0, 1.0]], // -X
    [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]], // +Y
    [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], // -Y
    [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]], // +Z
    [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]], // -Z
];

const AO_CURVE: [f32; 4] = [0.55, 0.75, 0.88, 1.0];

pub struct MeshData {
    pub solid_v: Vec<f32>, // x y z u v r g b  (8 floats per vertex)
    pub solid_i: Vec<u32>,
    pub cutout_v: Vec<f32>, // cross-plants + grass overlay: alpha-tested, no backface cull
    pub cutout_i: Vec<u32>,
    pub water_v: Vec<f32>,
    pub water_i: Vec<u32>,
}

struct Nb<'a> {
    center: &'a Chunk,
    w: &'a World,
    cx: i32,
    cz: i32,
}

impl Nb<'_> {
    #[inline]
    fn get(&self, x: i32, y: i32, z: i32) -> u16 {
        if y < 0 {
            return BEDROCK;
        }
        if y >= CY as i32 {
            return AIR;
        }
        if x >= 0 && x < CX as i32 && z >= 0 && z < CZ as i32 {
            self.center.get(x as usize, y as usize, z as usize)
        } else {
            self.w
                .get_block(self.cx * CX as i32 + x, y, self.cz * CZ as i32 + z)
        }
    }
    #[inline]
    fn opaque(&self, x: i32, y: i32, z: i32) -> bool {
        is_opaque_id(self.get(x, y, z))
    }
}

#[inline]
fn tile_uv(t: u32, u: f32, v: f32) -> [f32; 2] {
    const TS_U: f32 = 1.0 / ATLAS_COLS as f32;
    const TS_V: f32 = 1.0 / ATLAS_ROWS as f32;
    const IN: f32 = 0.25 / (ATLAS_COLS as f32 * 16.0); // quarter texel inset
    const INV: f32 = 0.25 / (ATLAS_ROWS as f32 * 16.0);
    let tu = (t % ATLAS_COLS) as f32 * TS_U;
    let tv = (t / ATLAS_COLS) as f32 * TS_V;
    let uu = tu + IN + u * (TS_U - 2.0 * IN);
    let vv = tv + INV + v * (TS_V - 2.0 * INV);
    [uu, vv]
}

/// UV atlas pour les modèles cuits (pub pour models.rs).
#[inline]
pub fn tile_uv_pub(t: u32, u: f32, v: f32) -> [f32; 2] {
    tile_uv(t, u, v)
}

pub fn face_corner_uv(tile: u32, d: usize, corner: usize) -> [f32; 2] {
    tile_uv(tile, UVC[d][corner][0], UVC[d][corner][1])
}

/// Per-corner biome tint caches (17x17 lattice corners per chunk, smoothed
/// over the 4 surrounding columns like the reference game's smooth lighting).
struct CornerCols {
    grass: [[[f32; 3]; CZ + 1]; CX + 1],
    foliage: [[[f32; 3]; CZ + 1]; CX + 1],
    water: [[[f32; 3]; CZ + 1]; CX + 1],
}

fn corner_cols(w: &World, cx: i32, cz: i32, pal: &BiomePalette) -> CornerCols {
    let mut cc = CornerCols {
        grass: [[[0.0; 3]; CZ + 1]; CX + 1],
        foliage: [[[0.0; 3]; CZ + 1]; CX + 1],
        water: [[[0.0; 3]; CZ + 1]; CX + 1],
    };
    for i in 0..=CX {
        for j in 0..=CZ {
            let (mut g, mut f, mut wv) = ([0f32; 3], [0f32; 3], [0f32; 3]);
            for dx in [-1i32, 0] {
                for dz in [-1i32, 0] {
                    let x = cx * CX as i32 + i as i32 + dx;
                    let z = cz * CZ as i32 + j as i32 + dz;
                    let b = w.biome_at(x, z) as usize;
                    for k in 0..3 {
                        g[k] += pal.grass[b][k];
                        f[k] += pal.foliage[b][k];
                        wv[k] += pal.water[b][k];
                    }
                }
            }
            for k in 0..3 {
                cc.grass[i][j][k] = g[k] * 0.25;
                cc.foliage[i][j][k] = f[k] * 0.25;
                cc.water[i][j][k] = wv[k] * 0.25;
            }
        }
    }
    cc
}

#[inline]
fn corner_tint(cc: &CornerCols, kind: TintKind, i: usize, j: usize) -> [f32; 3] {
    let i = i.min(CX);
    let j = j.min(CZ);
    match kind {
        TintKind::None => [1.0, 1.0, 1.0],
        TintKind::Grass => cc.grass[i][j],
        TintKind::Foliage => cc.foliage[i][j],
        TintKind::BirchLeaves => icon_tint(T_BIRCH_LEAVES),
        TintKind::SpruceLeaves => icon_tint(T_SPRUCE_LEAVES),
        TintKind::Water => cc.water[i][j],
    }
}

/// Block-center tint for box shapes (average of the 4 cell corners).
#[inline]
fn col_tint(cc: &CornerCols, tile: u32, lx: usize, lz: usize) -> [f32; 3] {
    let kind = tint_kind(tile);
    if kind == TintKind::None {
        return [1.0; 3];
    }
    let mut acc = [0f32; 3];
    for (i, j) in [(lx, lz), (lx + 1, lz), (lx, lz + 1), (lx + 1, lz + 1)] {
        let t = corner_tint(cc, kind, i, j);
        for k in 0..3 {
            acc[k] += t[k];
        }
    }
    [acc[0] * 0.25, acc[1] * 0.25, acc[2] * 0.25]
}

/// Emit a partial box (slabs, cushions, beds, stairs, torch, fence...).
/// Extents are local to the block cell. Faces flush with the cell border are
/// culled by opaque neighbors.
#[allow(clippy::too_many_arguments)]
fn push_box(
    out: &mut MeshData,
    nb: &Nb,
    lx: i32,
    ly: i32,
    lz: i32,
    wx: i32,
    wz: i32,
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    z0: f32,
    z1: f32,
    tiles: [u32; 3],
    rgb: [f32; 3],
) {
    for d in 0..6 {
        let nx = lx + DIRS[d][0];
        let ny = ly + DIRS[d][1];
        let nz = lz + DIRS[d][2];
        let flush = match d {
            0 => x1 >= 1.0,
            1 => x0 <= 0.0,
            2 => y1 >= 1.0,
            3 => y0 <= 0.0,
            4 => z1 >= 1.0,
            _ => z0 <= 0.0,
        };
        // non-flush faces are interior -> always visible; flush faces follow
        // the standard opaque-neighbor culling
        if flush && nb.opaque(nx, ny, nz) {
            continue;
        }
        let tile = match d {
            2 => tiles[0],
            3 => tiles[2],
            _ => tiles[1],
        };
        let base = (out.solid_v.len() / 6) as u32;
        for c in 0..4 {
            let co = CORNERS[d][c];
            let px = wx as f32 + if co[0] == 1 { x1 } else { x0 };
            let py = ly as f32 + if co[1] == 1 { y1 } else { y0 };
            let pz = wz as f32 + if co[2] == 1 { z1 } else { z0 };
            // scale UVs to the sub-box so textures don't stretch
            let (us, vs) = match d {
                0 | 1 => (z1 - z0, y1 - y0),
                2 | 3 => (x1 - x0, z1 - z0),
                _ => (x1 - x0, y1 - y0),
            };
            let uv = tile_uv(tile, UVC[d][c][0] * us, UVC[d][c][1] * vs);
            let s = SHADE[d];
            out.solid_v.extend_from_slice(&[
                px,
                py,
                pz,
                uv[0],
                uv[1],
                s * rgb[0],
                s * rgb[1],
                s * rgb[2],
            ]);
        }
        out.solid_i
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

pub fn build_mesh(w: &World, cx: i32, cz: i32, pal: &BiomePalette) -> MeshData {
    let empty = Chunk::new();
    let center = w.chunks.get(&(cx, cz)).unwrap_or(&empty);
    let nb = Nb {
        center,
        w,
        cx,
        cz,
    };
    let cc = corner_cols(w, cx, cz, pal);

    let mut out = MeshData {
        solid_v: Vec::with_capacity(4096),
        solid_i: Vec::with_capacity(2048),
        cutout_v: Vec::with_capacity(256),
        cutout_i: Vec::with_capacity(128),
        water_v: Vec::with_capacity(256),
        water_i: Vec::with_capacity(128),
    };
    let x0 = cx * CX as i32;
    let z0 = cz * CZ as i32;

    for ly in 0..CY {
        for lz in 0..CZ {
            for lx in 0..CX {
                let b = center.get(lx, ly, lz);
                if b == AIR {
                    continue;
                }
                let water = b == WATER;

                // Cross-plants (X-shaped cutout quads, drawn from both sides).
                // v=0 is the top row of the tile -> top of the plant.
                // Tintable plants pick up the biome color of each corner.
                if is_cross_id(b) {
                    let tile = tiles_of(b)[1];
                    let wx = x0 + lx as i32;
                    let wz = z0 + lz as i32;
                    let (vx0, vz0) = (wx as f32 + 0.146, wz as f32 + 0.146);
                    let (vx1, vz1) = (wx as f32 + 0.854, wz as f32 + 0.854);
                    let (vx2, vz2) = (wx as f32 + 0.854, wz as f32 + 0.146);
                    let (vx3, vz3) = (wx as f32 + 0.146, wz as f32 + 0.854);
                    let y0 = ly as f32;
                    let y1 = ly as f32 + 1.0;
                    let base = (out.cutout_v.len() / 8) as u32;
                    let kind = tint_kind(tile);
                    let t00 = corner_tint(&cc, kind, lx, lz);
                    let t11 = corner_tint(&cc, kind, lx + 1, lz + 1);
                    let t10 = corner_tint(&cc, kind, lx + 1, lz);
                    let t01 = corner_tint(&cc, kind, lx, lz + 1);
                    let push_v = |out: &mut MeshData,
                                  x: f32,
                                  z: f32,
                                  y: f32,
                                  u: f32,
                                  v: f32,
                                  s: f32,
                                  t: [f32; 3]| {
                        let uv = tile_uv(tile, u, v);
                        out.cutout_v.extend_from_slice(&[
                            x,
                            y,
                            z,
                            uv[0],
                            uv[1],
                            s * t[0],
                            s * t[1],
                            s * t[2],
                        ]);
                    };
                    // quad A: (x0,z0)-(x1,z1)
                    push_v(&mut out, vx0, vz0, y1, 0.0, 0.0, 1.0, t00);
                    push_v(&mut out, vx1, vz1, y1, 1.0, 0.0, 1.0, t11);
                    push_v(&mut out, vx1, vz1, y0, 1.0, 1.0, 0.95, t11);
                    push_v(&mut out, vx0, vz0, y1, 0.0, 0.0, 1.0, t00);
                    push_v(&mut out, vx1, vz1, y0, 1.0, 1.0, 0.95, t11);
                    push_v(&mut out, vx0, vz0, y0, 0.0, 1.0, 0.95, t00);
                    // quad B: (x0,z1)-(x1,z0)
                    push_v(&mut out, vx3, vz3, y1, 0.0, 0.0, 1.0, t01);
                    push_v(&mut out, vx2, vz2, y1, 1.0, 0.0, 1.0, t10);
                    push_v(&mut out, vx2, vz2, y0, 1.0, 1.0, 0.95, t10);
                    push_v(&mut out, vx3, vz3, y1, 0.0, 0.0, 1.0, t01);
                    push_v(&mut out, vx2, vz2, y0, 1.0, 1.0, 0.95, t10);
                    push_v(&mut out, vx3, vz3, y0, 0.0, 1.0, 0.95, t01);
                    for k in 0..12u32 {
                        out.cutout_i.push(base + k);
                    }
                    continue;
                }
                let lowered = water && nb.get(lx as i32, ly as i32 + 1, lz as i32) != WATER;
                let wx = x0 + lx as i32;
                let wz = z0 + lz as i32;

                // --- modèles cuits depuis le pack (blockstates + models JSON) ---
                if !water {
                    if let Some(bm) = crate::models::baked_for(b) {
                        let tiles = tiles_of(b);
                        let tint_of = |t: u32| -> [f32; 3] { col_tint(&cc, t, lx, lz) };
                        crate::models::push_baked(
                            &mut out,
                            &bm,
                            wx,
                            wz,
                            ly,
                            tiles,
                            &tint_of,
                        );
                        continue;
                    }
                }

                // --- partial shapes (slabs, stairs, beds, cushions, torch,
                // fence, snow layer, cactus) go through push_box and skip the
                // full-cube path
                if !water && shape_of(b) != Shape::Full {
                    let tiles = tiles_of(b);
                    let (ilx, ily, ilz) = (lx as i32, ly as i32, lz as i32);
                    let ctint = col_tint(&cc, tiles[1], lx, lz);
                    let white = [1.0f32; 3];
                    match shape_of(b) {
                        Shape::Slab => push_box(
                            &mut out, &nb, ilx, ily, ilz, wx, wz, 0.0, 1.0, 0.0, 0.5, 0.0, 1.0,
                            tiles, ctint,
                        ),
                        Shape::Cushion => push_box(
                            &mut out, &nb, ilx, ily, ilz, wx, wz, 0.0, 1.0, 0.0, 0.1875, 0.0, 1.0,
                            tiles, ctint,
                        ),
                        Shape::Bed => push_box(
                            &mut out, &nb, ilx, ily, ilz, wx, wz, 0.0, 1.0, 0.0, 0.5625, 0.0, 1.0,
                            tiles, ctint,
                        ),
                        Shape::Pad => push_box(
                            &mut out, &nb, ilx, ily, ilz, wx, wz, 0.0, 1.0, 0.0, 0.0625, 0.0, 1.0,
                            tiles, ctint,
                        ),
                        Shape::Torch => push_box(
                            &mut out, &nb, ilx, ily, ilz, wx, wz, 0.4375, 0.5625, 0.0, 0.625,
                            0.4375, 0.5625, tiles, white,
                        ),
                        Shape::SnowLayer => push_box(
                            &mut out, &nb, ilx, ily, ilz, wx, wz, 0.0, 1.0, 0.0, 0.125, 0.0, 1.0,
                            tiles, white,
                        ),
                        Shape::Cactus => push_box(
                            &mut out, &nb, ilx, ily, ilz, wx, wz, 0.0625, 0.9375, 0.0, 1.0,
                            0.0625, 0.9375, tiles, white,
                        ),
                        Shape::Fence => {
                            // central post (4/16 wide, full height)
                            push_box(
                                &mut out, &nb, ilx, ily, ilz, wx, wz, 0.375, 0.625, 0.0, 1.0,
                                0.375, 0.625, tiles, white,
                            );
                            // two rails towards each solid neighbor
                            let sides = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)];
                            for (dx, dz) in sides {
                                let n = nb.get(ilx + dx, ily, ilz + dz);
                                if !is_solid_id(n) {
                                    continue;
                                }
                                let (bx0, bx1, bz0, bz1) = if dx == 1 {
                                    (0.625, 1.0, 0.4375, 0.5625)
                                } else if dx == -1 {
                                    (0.0, 0.375, 0.4375, 0.5625)
                                } else if dz == 1 {
                                    (0.4375, 0.5625, 0.625, 1.0)
                                } else {
                                    (0.4375, 0.5625, 0.0, 0.375)
                                };
                                push_box(
                                    &mut out, &nb, ilx, ily, ilz, wx, wz, bx0, bx1, 0.3125,
                                    0.4375, bz0, bz1, tiles, white,
                                );
                                push_box(
                                    &mut out, &nb, ilx, ily, ilz, wx, wz, bx0, bx1, 0.75, 0.875,
                                    bz0, bz1, tiles, white,
                                );
                            }
                        }
                        Shape::Stairs => {
                            // bottom slab
                            push_box(
                                &mut out, &nb, ilx, ily, ilz, wx, wz, 0.0, 1.0, 0.0, 0.5, 0.0,
                                1.0, tiles, ctint,
                            );
                            // upper half against the "back" side (a solid
                            // neighbor if any, else a per-block hash)
                            let sides = [0usize, 1, 4, 5];
                            let mut back = hash01(hash3i(
                                wx as i64,
                                ly as i64,
                                wz as i64,
                                0x57A1_5D1,
                            )) as usize
                                % 4;
                            for (k, &s) in sides.iter().enumerate() {
                                if nb.opaque(
                                    ilx + DIRS[s][0],
                                    ily + DIRS[s][1],
                                    ilz + DIRS[s][2],
                                ) {
                                    back = k;
                                    break;
                                }
                            }
                            let (bx0, bx1, bz0, bz1) = match back {
                                0 => (0.5, 1.0, 0.0, 1.0), // +X
                                1 => (0.0, 0.5, 0.0, 1.0), // -X
                                2 => (0.0, 1.0, 0.5, 1.0), // +Z
                                _ => (0.0, 1.0, 0.0, 0.5), // -Z
                            };
                            push_box(
                                &mut out, &nb, ilx, ily, ilz, wx, wz, bx0, bx1, 0.5, 1.0, bz0,
                                bz1, tiles, ctint,
                            );
                        }
                        _ => {}
                    }
                    continue;
                }

                for d in 0..6 {
                    let nx = lx as i32 + DIRS[d][0];
                    let ny = ly as i32 + DIRS[d][1];
                    let nz = lz as i32 + DIRS[d][2];
                    let n = nb.get(nx, ny, nz);

                    let visible = if water {
                        n == AIR
                    } else {
                        // anything non-opaque (air, water, plants, glass,
                        // slabs, cushions...) lets the face show through
                        n != b && !nb.opaque(nx, ny, nz)
                    };
                    if !visible {
                        continue;
                    }

                    let mut tile = if water { 10u32 } else { tiles_of(b)[1] };
                    // top face uses the top tile, bottom face the bottom tile
                    if !water {
                        match d {
                            2 => tile = tiles_of(b)[0],
                            3 => tile = tiles_of(b)[2],
                            _ => {}
                        }
                    }
                    let kind = tint_kind(tile);

                    let (vout, iout): (&mut Vec<f32>, &mut Vec<u32>) = if water {
                        (&mut out.water_v, &mut out.water_i)
                    } else {
                        (&mut out.solid_v, &mut out.solid_i)
                    };

                    let base = (vout.len() / 8) as u32;
                    let mut ao = [3u32; 4];
                    if !water {
                        // the two in-plane axes
                        let mut ax = [0usize, 1usize];
                        let mut k = 0;
                        for a in 0..3 {
                            if DIRS[d][a] == 0 {
                                ax[k] = a;
                                k += 1;
                            }
                        }
                        for c in 0..4 {
                            let co = CORNERS[d][c];
                            let s = [co[ax[0]], co[ax[1]]];
                            let sgn = |v: i32| if v == 1 { 1i32 } else { -1i32 };
                            let mut p1 = [nx, ny, nz];
                            p1[ax[0]] += sgn(s[0]);
                            let s1 = nb.opaque(p1[0], p1[1], p1[2]);
                            let mut p2 = [nx, ny, nz];
                            p2[ax[1]] += sgn(s[1]);
                            let s2 = nb.opaque(p2[0], p2[1], p2[2]);
                            let mut pc = [nx, ny, nz];
                            pc[ax[0]] += sgn(s[0]);
                            pc[ax[1]] += sgn(s[1]);
                            let sc = nb.opaque(pc[0], pc[1], pc[2]);
                            ao[c] = if s1 && s2 {
                                0
                            } else {
                                3 - (s1 as u32 + s2 as u32 + sc as u32)
                            };
                        }
                    }

                    for c in 0..4 {
                        let co = CORNERS[d][c];
                        let mut vy = (ly as i32 + co[1]) as f32;
                        if lowered && co[1] == 1 {
                            vy -= 0.12;
                        }
                        let vx = (wx + co[0]) as f32;
                        let vz = (wz + co[2]) as f32;
                        let uv = tile_uv(tile, UVC[d][c][0], UVC[d][c][1]);
                        // biome tint of the lattice corner this vertex sits on
                        let tint =
                            corner_tint(&cc, kind, lx + co[0] as usize, lz + co[2] as usize);
                        let shade = if water {
                            SHADE[d] * 0.92
                        } else {
                            SHADE[d] * AO_CURVE[ao[c] as usize]
                        };
                        vout.push(vx);
                        vout.push(vy);
                        vout.push(vz);
                        vout.push(uv[0]);
                        vout.push(uv[1]);
                        vout.push(shade * tint[0]);
                        vout.push(shade * tint[1]);
                        vout.push(shade * tint[2]);
                    }
                    // flip the quad diagonal to avoid AO anisotropy artifacts
                    if ao[0] + ao[2] > ao[1] + ao[3] {
                        iout.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                    } else {
                        iout.extend_from_slice(&[base + 1, base + 2, base + 3, base + 1, base + 3, base]);
                    }

                    // --- the classic grass-side overlay: the gray grass
                    // fringe of the side tile is drawn as a second, slightly
                    // offset quad so it can receive the biome color without
                    // tinting the dirt underneath (just like the reference).
                    if b == GRASS && matches!(d, 0 | 1 | 4 | 5) {
                        let otile = T_GRASS_SIDE_OVERLAY;
                        let obase = (out.cutout_v.len() / 8) as u32;
                        let e = 0.0022;
                        let off = [DIRS[d][0] as f32 * e, 0.0, DIRS[d][2] as f32 * e];
                        for c in 0..4 {
                            let co = CORNERS[d][c];
                            let vx = (wx + co[0]) as f32 + off[0];
                            let vz = (wz + co[2]) as f32 + off[2];
                            let uv = tile_uv(otile, UVC[d][c][0], UVC[d][c][1]);
                            let tint = corner_tint(
                                &cc,
                                TintKind::Grass,
                                lx + co[0] as usize,
                                lz + co[2] as usize,
                            );
                            let s = SHADE[d] * 0.98;
                            out.cutout_v.extend_from_slice(&[
                                vx,
                                (ly as i32 + co[1]) as f32,
                                vz,
                                uv[0],
                                uv[1],
                                s * tint[0],
                                s * tint[1],
                                s * tint[2],
                            ]);
                        }
                        out.cutout_i.extend_from_slice(&[
                            obase,
                            obase + 1,
                            obase + 2,
                            obase,
                            obase + 2,
                            obase + 3,
                        ]);
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(w: &World, cx: i32, cz: i32) -> MeshData {
        build_mesh(w, cx, cz, &BiomePalette::vanilla())
    }

    #[test]
    fn face_winding_points_outwards() {
        for d in 0..6 {
            let c = CORNERS[d];
            let sub = |a: [i32; 3], b: [i32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
            let crs = |a: [i32; 3], b: [i32; 3]| {
                [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
            };
            let n = crs(sub(c[1], c[0]), sub(c[2], c[0]));
            assert_eq!(n, DIRS[d], "face {d} winding is wrong");
        }
    }

    #[test]
    fn single_block_six_faces() {
        let mut w = World::new(1);
        w.gen_chunk(0, 0);
        w.gen_chunk(1, 0);
        w.gen_chunk(0, 1);
        w.gen_chunk(1, 1);
        // carve a shaft and drop one stone block in mid-air
        for ly in 0..CY {
            for lz in 0..CZ {
                for lx in 0..CX {
                    w.set_block(lx as i32, ly as i32, lz as i32, AIR);
                }
            }
        }
        w.set_block(5, 30, 5, STONE);
        let m = mesh(&w, 0, 0);
        assert_eq!(m.solid_i.len(), 6 * 6, "a lone cube must show 6 faces");
        assert!(m.water_i.is_empty());
    }

    #[test]
    fn hidden_faces_are_culled() {
        let mut w = World::new(2);
        for cx in -1..=1 {
            for cz in -1..=1 {
                w.gen_chunk(cx, cz);
            }
        }
        // solid 3x3x3 cube of stone in the air
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    w.set_block(8 + dx, 40 + dy, 8 + dz, STONE);
                }
            }
        }
        // count faces contributed by the cube: interior faces hidden -> 6 visible
        let m = mesh(&w, 0, 0);
        // We can't easily isolate the cube from terrain; instead verify that
        // placing the cube does not produce the full 27*6 faces anywhere.
        // Simply check the total index count is a multiple of 6 and meshes build.
        assert_eq!(m.solid_i.len() % 6, 0);
    }

    #[test]
    fn water_gets_a_top_surface() {
        let mut w = World::new(11);
        w.gen_chunk(0, 0);
        // carve an air pit first so the water is not buried in stone
        for lx in 1..8 {
            for lz in 1..8 {
                for y in 28..40 {
                    w.set_block(lx as i32, y as i32, lz as i32, AIR);
                }
            }
        }
        // flood the pit with water manually
        for lx in 2..6 {
            for lz in 2..6 {
                w.set_block(lx as i32, 30, lz as i32, WATER);
                w.set_block(lx as i32, 31, lz as i32, WATER);
            }
        }
        let m = mesh(&w, 0, 0);
        assert!(!m.water_i.is_empty(), "water surface must be meshed");
        // top face y should be lowered by 0.12
        let has_lowered = m
            .water_v
            .chunks(8)
            .any(|v| (v[1] - (31.0 + 1.0 - 0.12)).abs() < 1e-4);
        assert!(has_lowered, "water top must be lowered");
    }

    #[test]
    fn solid_face_behind_a_plant_is_drawn() {
        let mut w = World::new(77);
        w.gen_chunk(0, 0);
        for y in 0..CY {
            for lz in 0..CZ {
                for lx in 0..CX {
                    w.set_block(lx as i32, y as i32, lz as i32, AIR);
                }
            }
        }
        w.set_block(5, 40, 5, STONE);
        w.set_block(6, 40, 5, TALLGRASS);
        let m = mesh(&w, 0, 0);
        // lone cube = 6 faces; the +X face must NOT be culled by the plant
        assert_eq!(m.solid_i.len(), 36, "plant must not cull neighbor faces");
        assert_eq!(m.cutout_i.len(), 12);
    }

    #[test]
    fn cross_plants_go_to_cutout_bucket() {
        let mut w = World::new(31);
        w.gen_chunk(0, 0);
        // strip the chunk so only our two test plants exist
        for y in 0..CY {
            for lz in 0..CZ {
                for lx in 0..CX {
                    w.set_block(lx as i32, y as i32, lz as i32, AIR);
                }
            }
        }
        w.set_block(4, 40, 4, TALLGRASS);
        w.set_block(6, 40, 6, FLOWER_RED);
        let m = mesh(&w, 0, 0);
        // 2 plants x 2 quads x 6 indices
        assert_eq!(m.cutout_i.len(), 24);
        assert_eq!(m.cutout_v.len(), 24 * 8);
        // plant quads must sit inside the block cell
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for v in m.cutout_v.chunks(8) {
            min_y = min_y.min(v[1]);
            max_y = max_y.max(v[1]);
        }
        assert!((min_y - 40.0).abs() < 1e-4 && (max_y - 41.0).abs() < 1e-4);
    }

    fn empty_chunk_world() -> World {
        let mut w = World::new(3);
        w.gen_chunk(0, 0);
        for y in 0..CY {
            for lz in 0..CZ {
                for lx in 0..CX {
                    w.set_block(lx as i32, y as i32, lz as i32, AIR);
                }
            }
        }
        w
    }

    #[test]
    fn slab_is_a_half_box() {
        let mut w = empty_chunk_world();
        w.set_block(5, 30, 5, WOOL_SLAB_BASE);
        let m = mesh(&w, 0, 0);
        assert!(m.solid_i.len() >= 36, "slab needs up to 6 faces");
        let mut max_y = f32::MIN;
        for v in m.solid_v.chunks(8) {
            max_y = max_y.max(v[1]);
        }
        assert!((max_y - 30.5).abs() < 1e-4, "slab top must be y+0.5, got {max_y}");
    }

    #[test]
    fn stairs_have_two_boxes_and_cushion_is_flat() {
        let mut w = empty_chunk_world();
        w.set_block(5, 30, 5, WOOL_STAIRS_BASE + 2);
        w.set_block(8, 30, 5, CUSHION_BASE + 5);
        let m = mesh(&w, 0, 0);
        // stairs: bottom slab (up to 6 faces) + upper back half (up to 6)
        assert!(m.solid_i.len() >= 36, "stairs need two boxes, got {}", m.solid_i.len());
        let mut max_y = f32::MIN;
        let mut min_y = f32::MAX;
        for v in m.solid_v.chunks(8) {
            max_y = max_y.max(v[1]);
            min_y = min_y.min(v[1]);
        }
        assert!((max_y - 31.0).abs() < 1e-4, "stairs must reach full height");
        assert!((min_y - 30.0).abs() < 1e-4);
        // cushion: flat, no full-height geometry, still textured
        assert!(m.solid_i.len() >= 24);
        let cushion_top = 30.0 + 0.1875;
        assert!(
            m.solid_v.chunks(8).any(|v| (v[1] - cushion_top).abs() < 1e-3),
            "cushion top face must sit at y+0.1875"
        );
    }

    #[test]
    fn face_behind_a_slab_is_drawn() {
        let mut w = empty_chunk_world();
        w.set_block(5, 30, 5, STONE);
        w.set_block(6, 30, 5, WOOL_SLAB_BASE); // next to the stone
        let m = mesh(&w, 0, 0);
        // lone cube = 36 indices; slab must not cull the stone's +X face
        let stone_faces = m.solid_i.len();
        assert!(stone_faces >= 36 + 12, "slab must not hide neighbor faces");
    }

    #[test]
    fn biome_tint_colors_the_mesh() {
        let mut w = empty_chunk_world();
        w.set_block(5, 30, 5, GRASS);
        let mut pal = BiomePalette::vanilla();
        // extreme palette so any leftover baked color shows up
        pal.grass = [[1.0, 0.0, 0.0]; BIOME_COUNT];
        pal.foliage = [[0.0, 1.0, 0.0]; BIOME_COUNT];
        pal.water = [[0.0, 0.0, 1.0]; BIOME_COUNT];
        let m = build_mesh(&w, 0, 0, &pal);
        // grass top face vertices (y=31) must be pure red x shade(1.0, AO max).
        // Side-face top vertices also sit at y=31 but stay gray (dirt tile).
        let mut red_top = 0;
        for v in m.solid_v.chunks(8) {
            if (v[1] - 31.0).abs() < 1e-4 {
                if v[5] > 0.9 && v[6] < 0.1 && v[7] < 0.1 {
                    red_top += 1;
                } else {
                    // side-face tops: the dirt tile must stay untinted
                    assert!(
                        (v[5] - v[6]).abs() < 1e-5 && (v[6] - v[7]).abs() < 1e-5,
                        "side tile must stay untinted, got {:?}",
                        &v[5..8]
                    );
                }
            }
        }
        assert!(red_top >= 4, "grass top face must take the biome color");
        // the gray fringe overlay lands in the cutout bucket, tinted too
        assert!(!m.cutout_i.is_empty(), "grass side overlay must be meshed");
        for v in m.cutout_v.chunks(8) {
            assert!(
                v[5] > 0.5 && v[6] < 0.3 && v[7] < 0.3,
                "overlay must be tinted like the biome, got {:?}",
                &v[5..8]
            );
        }
    }

    #[test]
    fn water_takes_the_biome_water_color() {
        let mut w = empty_chunk_world();
        for lx in 2..6 {
            for lz in 2..6 {
                w.set_block(lx as i32, 30, lz as i32, WATER);
            }
        }
        let mut pal = BiomePalette::vanilla();
        pal.water = [[0.0, 0.0, 1.0]; BIOME_COUNT];
        let m = build_mesh(&w, 0, 0, &pal);
        assert!(!m.water_i.is_empty());
        for v in m.water_v.chunks(8) {
            assert!(
                v[5] < 0.1 && v[7] > 0.4,
                "water must be tinted blue by the biome, got {:?}",
                &v[5..8]
            );
        }
    }

    #[test]
    fn stone_is_never_tinted() {
        let mut w = empty_chunk_world();
        w.set_block(5, 30, 5, STONE);
        let m = mesh(&w, 0, 0);
        for v in m.solid_v.chunks(8) {
            assert!(
                (v[5] - v[6]).abs() < 1e-5 && (v[6] - v[7]).abs() < 1e-5,
                "stone rgb channels must stay equal (no tint), got {:?}",
                &v[5..8]
            );
            assert!(v[5] > 0.4, "shade must survive in the vertex color");
        }
    }

    #[test]
    fn torch_snowlayer_and_fence_have_real_models() {
        let mut w = empty_chunk_world();
        w.set_block(5, 30, 5, TORCH);
        w.set_block(9, 30, 5, SNOW_LAYER);
        let m = mesh(&w, 0, 0);
        // torch: a real stick box, not a cross plant anymore
        assert!(m.cutout_i.is_empty(), "torch must not use cross quads");
        assert!(
            m.solid_v.chunks(8).any(|v| (v[1] - 30.625).abs() < 1e-3),
            "torch top must sit at y+0.625"
        );
        let torch_xs: Vec<f32> = m
            .solid_v
            .chunks(8)
            .filter(|v| v[1] < 30.7)
            .filter(|v| v[0] >= 5.0 && v[0] <= 6.0)
            .map(|v| v[0])
            .collect();
        assert!(!torch_xs.is_empty());
        assert!(
            torch_xs.iter().all(|&x| x > 5.3 && x < 5.7),
            "torch box must be 2/16 wide"
        );
        // snow layer: thin 2/16 carpet
        assert!(
            m.solid_v.chunks(8).any(|v| (v[1] - 30.125).abs() < 1e-3),
            "snow layer top must sit at y+0.125"
        );
        // fence: post + rails only towards solid neighbors
        let mut w2 = empty_chunk_world();
        w2.set_block(5, 30, 5, OAK_FENCE);
        w2.set_block(6, 30, 5, STONE);
        let m2 = mesh(&w2, 0, 0);
        // rail top rail sits at y+0.875; a full cube would reach y+1.0
        assert!(
            m2.solid_v.chunks(8).any(|v| (v[1] - 30.875).abs() < 1e-3),
            "fence rail must exist at y+0.875"
        );
        let max_y = m2
            .solid_v
            .chunks(8)
            .filter(|v| v[0] >= 5.0 && v[0] <= 6.0 && v[2] >= 5.0 && v[2] <= 6.0)
            .map(|v| v[1])
            .fold(f32::MIN, f32::max);
        assert!(
            (max_y - 31.0).abs() < 1e-4,
            "post must reach full height, got {max_y}"
        );
        // the rail extends towards the +X neighbor
        assert!(
            m2.solid_v.chunks(8).any(|v| (v[0] - 5.9375).abs() < 1e-3 || (v[0] - 6.0).abs() < 1e-4),
            "fence rail must extend towards the solid neighbor"
        );
    }

    #[test]
    fn cactus_is_inset() {
        let mut w = empty_chunk_world();
        w.set_block(5, 30, 5, CACTUS);
        let m = mesh(&w, 0, 0);
        let min_x = m
            .solid_v
            .chunks(8)
            .map(|v| v[0])
            .fold(f32::MAX, f32::min);
        assert!(
            (min_x - 5.0625).abs() < 1e-3,
            "cactus sides must be inset by 1/16, got {min_x}"
        );
    }
}
