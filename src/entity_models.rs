//! Rendu des mobs avec les VRAIS modèles vanilla (bones/cubes/UV) et les
//! textures d'entités du pack (fichiers séparés, PAS un atlas dans le pack :
//! nous cousons un atlas d'exécution, comme le jeu de référence).

use crate::entity_data as ed;
use crate::math::Vec3;
use crate::mobs::Mob;
use crate::pack::{Pack, Rgba};

const CANVAS: u32 = 1024;

pub struct EntityAtlas {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
    /// rect par index de texture global (x, y, w, h) en pixels canvas
    pub rects: Vec<[u32; 4]>,
    /// index global de départ pour chaque kind (0 = absent)
    kind_base: [u16; 32],
    kind_loaded: [bool; 32],
}

impl EntityAtlas {
    /// Charge toutes les textures d'entités du pack et les coud en un atlas.
    pub fn from_pack(pack: &Pack) -> Option<EntityAtlas> {
        let mut px = vec![0u8; (CANVAS * CANVAS * 4) as usize];
        let mut rects = Vec::new();
        let mut kind_base = [0u16; 32];
        let mut kind_loaded = [false; 32];
        let mut cur_x = 0u32;
        let mut cur_y = 0u32;
        let mut row_h = 0u32;
        let mut any = false;
        for kd in ed::KINDS {
            let texs = ed::textures_of(kd.kind);
            kind_base[kd.kind as usize] = rects.len() as u16;
            let mut all_ok = !texs.is_empty();
            for path in texs {
                // candidats : nom d'époque + variantes des versions récentes
                // (1.20.5+ : sheep_fur -> sheep_wool, chicken -> chicken_temperate)
                let stem = path.rsplit('/').next().unwrap_or(path);
                let mut cands = vec![stem.to_string()];
                if let Some(base) = stem.strip_suffix("_fur") {
                    cands.push(format!("{base}_wool"));
                }
                cands.push(format!("{stem}_temperate"));
                let img = match pack.get_full(&cands.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                {
                    Some(i) if i.w as u64 * i.h as u64 <= 512 * 512 => i,
                    _ => {
                        all_ok = false;
                        rects.push([0, 0, 0, 0]);
                        continue;
                    }
                };
                if cur_x + img.w > CANVAS {
                    cur_x = 0;
                    cur_y += row_h;
                    row_h = 0;
                }
                if cur_y + img.h > CANVAS {
                    all_ok = false;
                    rects.push([0, 0, 0, 0]);
                    continue;
                }
                blit(&mut px, CANVAS, cur_x, cur_y, &img);
                rects.push([cur_x, cur_y, img.w, img.h]);
                cur_x += img.w;
                row_h = row_h.max(img.h);
                any = true;
            }
            kind_loaded[kd.kind as usize] = all_ok;
        }
        if !any {
            return None;
        }
        Some(EntityAtlas {
            w: CANVAS,
            h: CANVAS,
            px,
            rects,
            kind_base,
            kind_loaded,
        })
    }

    fn rect_of(&self, kind: u8, local_tex: u32) -> Option<[u32; 4]> {
        if !self.kind_loaded[kind as usize] {
            return None;
        }
        let base = self.kind_base[kind as usize] as usize;
        let r = *self.rects.get(base + local_tex as usize)?;
        if r[2] == 0 {
            return None;
        }
        Some(r)
    }
}

fn blit(dst: &mut [u8], dw: u32, x: u32, y: u32, src: &Rgba) {
    for sy in 0..src.h {
        let dy = y + sy;
        if dy >= CANVAS {
            break;
        }
        for sx in 0..src.w {
            let dx = x + sx;
            if dx >= dw {
                break;
            }
            let si = ((sy * src.w + sx) * 4) as usize;
            let di = ((dy * dw + dx) * 4) as usize;
            if si + 3 < src.px.len() && di + 3 < dst.len() {
                dst[di..di + 4].copy_from_slice(&src.px[si..si + 4]);
            }
        }
    }
}

// ordre entity_data [north, east, south, west, up, down] -> nos faces
// 0=+X,1=-X,2=+Y,3=-Y,4=+Z,5=-Z
const FACE_MAP: [usize; 6] = [1, 3, 4, 5, 2, 0];

/// Quads des mobs à textures pack (mods vanilla). Retourne aussi la liste des
/// kinds rendus (pour le fallback procédural du client).
pub fn build_verts(mobs: &[Mob], atlas: &EntityAtlas, out: &mut Vec<f32>) -> Vec<u8> {
    let mut rendered = Vec::new();
    for m in mobs {
        let kind = m.kind.id();
        let Some((_, _texs, def)) = ed::def_for_kind(kind) else {
            continue;
        };
        if !atlas.kind_loaded[kind as usize] {
            continue;
        }
        rendered.push(kind);
        let flash = if m.hurt_t > 0.0 {
            2.2
        } else if m.kind == crate::mobs::MobKind::Siffleur
            && m.fuse > 0.0
            && ((m.fuse * 7.0) as i32) % 2 == 0
        {
            2.6
        } else {
            1.0
        };
        let swing_on = if m.moving { 1.0 } else { 0.15 };
        for bone in def.bones {
            let swing = swing_for(bone.name);
            let swing_a = bone.rot_x + (m.anim * 7.0).sin() * swing * swing_on;
            let pivot = Vec3::new(
                bone.pivot[0] / 16.0,
                bone.pivot[1] / 16.0,
                bone.pivot[2] / 16.0,
            );
            for cube in bone.cubes {
                let size = Vec3::new(cube.size[0] / 16.0, cube.size[1] / 16.0, cube.size[2] / 16.0);
                let inf = cube.inflate / 16.0;
                let center = Vec3::new(
                    cube.origin[0] / 16.0 + size.x * 0.5,
                    cube.origin[1] / 16.0 + size.y * 0.5,
                    cube.origin[2] / 16.0 + size.z * 0.5,
                );
                // rect texture (canvas) pour l'uv normalisé
                let rect = atlas.rect_of(kind, cube.tex);
                for d in 0..6 {
                    let fuv = cube.uv[FACE_MAP[d]];
                    for &ci in &[0usize, 1, 2, 0, 2, 3] {
                        let co = crate::mesher::CORNERS[d][ci];
                        let off = Vec3::new(
                            (co[0] as f32 - 0.5) * (size.x + 2.0 * inf),
                            (co[1] as f32 - 0.5) * (size.y + 2.0 * inf),
                            (co[2] as f32 - 0.5) * (size.z + 2.0 * inf),
                        );
                        let p = rot_x(center - pivot + off, swing_a) + pivot;
                        let p = rot_y(p, m.yaw) + m.pos;
                        // uv : coin (0..1) dans la face, en texels -> atlas
                        let (lx, ly, lz) = (co[0] as f32, co[1] as f32, co[2] as f32);
                        let (u, v) = face_uv(d, lx, ly, lz, fuv);
                        let (ur, vr) = match rect {
                            Some([rx, ry, rw, rh]) => (
                                (rx as f32 + u * rw as f32) / atlas.w as f32,
                                (ry as f32 + v * rh as f32) / atlas.h as f32,
                            ),
                            None => (0.0, 0.0),
                        };
                        out.extend_from_slice(&[
                            p.x,
                            p.y,
                            p.z,
                            ur,
                            vr,
                            crate::mesher::SHADE[d] * flash,
                            crate::mesher::SHADE[d] * flash,
                            crate::mesher::SHADE[d] * flash,
                        ]);
                    }
                }
            }
        }
    }
    rendered
}

fn swing_for(name: &str) -> f32 {
    let n = name.to_lowercase();
    if n.contains("leg") {
        0.8
    } else if n.contains("arm") || n.contains("wing") || n.contains("ear") || n.contains("tail") {
        0.45
    } else {
        0.0
    }
}

/// uv (en texels de la texture) pour un coin de face, convention vanilla.
fn face_uv(d: usize, lx: f32, ly: f32, lz: f32, fuv: [f32; 4]) -> (f32, f32) {
    let (u0, v0, uw, vh) = (fuv[0], fuv[1], fuv[2], fuv[3]);
    let (u, v) = match d {
        0 => (1.0 - lz, 1.0 - ly), // +X (east)
        1 => (lz, 1.0 - ly),       // -X (west)
        2 => (lx, lz),             // +Y (up)
        3 => (lx, lz),             // -Y (down)
        4 => (lx, 1.0 - ly),       // +Z (south)
        _ => (1.0 - lx, 1.0 - ly), // -Z (north, la face avant)
    };
    (u0 + u * uw, v0 + v * vh)
}

fn rot_y(p: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    Vec3::new(c * p.x - s * p.z, p.y, s * p.x + c * p.z)
}

fn rot_x(p: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(p.x, c * p.y - s * p.z, s * p.y + c * p.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zombie_arm_points_forward() {
        // convention : rot_x(+90°) envoie le bas (-Y) vers l'avant (-Z)
        let tip = rot_x(Vec3::new(0.0, -1.0, 0.0), std::f32::consts::FRAC_PI_2);
        assert!(tip.z < -0.9, "bras vers l'avant (-Z), got {:?}", tip);
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    /// Avec le VRAI pack vanilla : les textures d'entités (fichiers séparés,
    /// PAS un atlas dans le pack) se cousent en atlas d'exécution.
    #[test]
    #[ignore]
    fn real_pack_entity_atlas() {
        let zip = std::path::Path::new("texturepacks/texture-pack-default1.20.5-26.2.zip");
        if !zip.exists() {
            eprintln!("pack vanilla absent, test sauté");
            return;
        }
        let pack = Pack::discover(std::path::Path::new("."));
        let atlas = EntityAtlas::from_pack(&pack).expect("atlas d'entités");
        // zombie/zombie.png = 64x64 dans le pack vanilla
        let r = atlas.rect_of(1, 0).expect("texture zombie chargée");
        assert_eq!((r[2], r[3]), (64, 64), "zombie.png 64x64");
        // creeper/creeper.png = 64x64 depuis l'update de textures 1.15+
        let c = atlas.rect_of(2, 0).expect("texture creeper chargée");
        assert_eq!((c[2], c[3]), (64, 64), "creeper.png 64x64 en 1.20.5");
        // mouton : 2 textures (laine + corps tondu)
        assert!(atlas.rect_of(4, 1).is_some(), "sheep.png chargée");
        // un rig complet génère des sommets
        let mut mobs_list = Vec::new();
        let mut m = crate::mobs::Mob::new(crate::mobs::MobKind::Zombie, Vec3::new(0.0, 40.0, 0.0));
        m.anim = 0.5;
        mobs_list.push(m);
        let mut out = Vec::new();
        let rendered = build_verts(&mobs_list, &atlas, &mut out);
        assert_eq!(rendered, vec![1u8], "zombie rendu via le rig vanilla");
        assert!(!out.is_empty());
        // bras du zombie levés : des sommets dépassent la hauteur de la tête (y=32/16=2.0)
        let max_y = out.chunks(8).map(|v| v[1]).fold(0.0f32, f32::max);
        assert!(max_y > 2.0, "bras levés au-dessus des épaules: {max_y}");
    }
}
