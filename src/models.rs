//! Vrais modèles de blocs : blockstates/*.json + models/**/*.json du pack,
//! cuits en quads (elements/from-to/faces/uv/rotation) comme le format vanilla.
//! Le pack fournit la géométrie ; sans pack (ou forme non modélisée) le mesher
//! utilise ses formes procédurales habituelles.

use crate::json::Json;
use crate::pack::Pack;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Quad cuit : coins en espace bloc (0..1), uv 0..1 dans la tuile, teinte.
pub struct Quad {
    pub corners: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub tile: Option<u32>,
    pub tint: bool,
}

pub struct BakedModel {
    pub quads: Vec<Quad>,
}

pub struct ModelCache {
    by_block: Vec<Option<Arc<BakedModel>>>,
}

static CACHE: Mutex<Option<Arc<ModelCache>>> = Mutex::new(None);

/// Construit le cache depuis le pack (appelé au démarrage et au F4).
pub fn install(pack: &Pack) {
    let mut by_block: Vec<Option<Arc<BakedModel>>> = (0..256).map(|_| None).collect();
    let mut lib = ModelLib::new(pack);
    for b in 1..=crate::world::MAX_BLOCK_ID {
        let name = crate::vanilla_data::block_vanilla_name(b);
        let props = crate::vanilla_data::block_props(b);
        if let Some(model) = lib.bake(name, props) {
            if !model.quads.is_empty() {
                by_block[b as usize] = Some(Arc::new(model));
            }
        }
    }
    *CACHE.lock().unwrap() = Some(Arc::new(ModelCache { by_block }));
}

/// Modèle cuit pour un bloc (None = formes procédurales du mesher).
pub fn baked_for(b: u16) -> Option<Arc<BakedModel>> {
    let c = CACHE.lock().ok()?.clone()?;
    if b as usize >= c.by_block.len() {
        return None;
    }
    c.by_block[b as usize].clone()
}

/// Remplace un bloc cuit par rien (tests).
pub fn is_installed() -> bool {
    CACHE.lock().map(|c| c.is_some()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::Pack;

    #[test]
    fn no_pack_means_no_baked_models() {
        let dir = std::env::temp_dir().join(format!("rvx_models_{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let pack = Pack::discover(&dir);
        install(&pack);
        assert!(baked_for(crate::world::STONE).is_none());
        assert!(baked_for(crate::world::TORCH).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Avec le VRAI pack vanilla : les blockstates/models JSON cuisent les
    /// géométries (fleur croisée, torche, dalle).
    #[test]
    #[ignore]
    fn real_pack_bakes_cross_torch_slab() {
        let zip = std::path::Path::new("texturepacks/texture-pack-default1.20.5-26.2.zip");
        if !zip.exists() {
            eprintln!("pack vanilla absent, test sauté");
            return;
        }
        let pack = Pack::discover(std::path::Path::new("."));
        assert!(pack.raw_keys("blockstates/").len() > 100);
        install(&pack);
        // fleur rouge = modèle croisé, teinte absente
        let poppy = baked_for(crate::world::FLOWER_RED).expect("poppy doit cuire");
        assert!(poppy.quads.len() >= 2, "croix = plusieurs quads");
        assert!(poppy.quads.iter().all(|q| !q.tint));
        // herbe = croisée TEINTÉE (tintindex)
        let sg = baked_for(crate::world::TALLGRASS).expect("short_grass doit cuire");
        assert!(sg.quads.iter().any(|q| q.tint), "herbe teintée");
        // torche = petit bâton (coins < 1 bloc)
        let torch = baked_for(crate::world::TORCH).expect("torche doit cuire");
        let max_x = torch
            .quads
            .iter()
            .flat_map(|q| q.corners.iter().map(|c| c[0]))
            .fold(0.0f32, f32::max);
        assert!(max_x < 0.9, "la torche ne remplit pas le bloc: {max_x}");
        // les quads référencent des tuiles atlas connues
        assert!(poppy.quads.iter().any(|q| q.tile.is_some()));
    }
}

// ------------------------------------------------------------------ bibliothèque

struct ModelLib<'a> {
    pack: &'a Pack,
    /// stem de texture -> tuile atlas (candidates vanilla)
    tile_of_stem: HashMap<String, u32>,
}

impl<'a> ModelLib<'a> {
    fn new(pack: &'a Pack) -> ModelLib<'a> {
        let mut tile_of_stem = HashMap::new();
        for t in 0..crate::world::ATLAS_COLS * crate::world::ATLAS_ROWS {
            for c in crate::pack::tile_candidates(t) {
                tile_of_stem.entry(c.to_string()).or_insert(t);
            }
        }
        ModelLib { pack, tile_of_stem }
    }

    fn raw_json(&self, key: &str) -> Option<Json> {
        let data = self.pack.get_raw(key)?;
        let s = std::str::from_utf8(data).ok()?;
        crate::json::parse(s)
    }

    /// blockstates/<name>.json -> nom du modèle correspondant à nos props canoniques.
    fn variant_model(&self, name: &str, props: &[(&str, &str)]) -> Option<String> {
        let bs = self.raw_json(&format!("blockstates/{name}.json"))?;
        if bs.get("multipart").is_some() {
            return None; // multipart (clôtures, murs...) : formes procédurales
        }
        let variants = bs.get("variants")?.as_obj()?;
        // clé "" = état unique
        for (k, v) in variants {
            if k.is_empty() {
                if let Some(m) = v.get("model").and_then(|m| m.as_str()) {
                    return Some(m.to_string());
                }
            }
        }
        // sinon : variante dont toutes les conditions correspondent à nos props
        for (k, v) in variants {
            let ok = k.split(',').all(|part| {
                let mut it = part.splitn(2, '=');
                let (Some(kk), Some(vv)) = (it.next(), it.next()) else {
                    return false;
                };
                props
                    .iter()
                    .find(|(pk, _)| *pk == kk)
                    .map(|(_, pv)| *pv == vv)
                    .unwrap_or(false)
            });
            if ok {
                if let Some(m) = v.get("model").and_then(|m| m.as_str()) {
                    return Some(m.to_string());
                }
            }
        }
        None
    }

    /// Charge un modèle (parent chain) et résout textures + éléments.
    fn load_model(&self, path: &str, depth: u32) -> Option<Json> {
        if depth > 8 {
            return None;
        }
        let p = path
            .trim_start_matches("minecraft:")
            .trim_start_matches("block/");
        let model = self.raw_json(&format!("models/block/{p}.json"))?;
        // parent
        if let Some(parent) = model.get("parent").and_then(|p| p.as_str()) {
            if parent != "builtin/generated" && parent != "builtin/entity" {
                if let Some(pp) = self.load_model(parent, depth + 1) {
                    // fusion : textures enfant > parent ; éléments enfant remplacent
                    let mut merged_maps: Vec<(String, Json)> =
                        pp.get("textures").and_then(|t| t.as_obj())?.to_vec();
                    if let Some(ct) = model.get("textures").and_then(|t| t.as_obj()) {
                        for (k, v) in ct {
                            if let Some(e) = merged_maps.iter_mut().find(|(ek, _)| ek == k) {
                                e.1 = v.clone();
                            } else {
                                merged_maps.push((k.clone(), v.clone()));
                            }
                        }
                    }
                    let elements = if model.get("elements").is_some() {
                        model.get("elements").cloned().unwrap_or(Json::Null)
                    } else {
                        pp.get("elements").cloned().unwrap_or(Json::Null)
                    };
                    return Some(Json::Obj(vec![
                        ("textures".to_string(), Json::Obj(merged_maps)),
                        ("elements".to_string(), elements),
                    ]));
                }
            }
        }
        Some(model)
    }

    fn resolve_texture(&self, tex: &Json, maps: &[(String, Json)]) -> Option<String> {
        let mut cur = tex.clone();
        for _ in 0..8 {
            match cur.as_str()? {
                s if s.starts_with('#') => {
                    cur = maps
                        .iter()
                        .find(|(k, _)| k == &s[1..])
                        .map(|(_, v)| v.clone())?;
                }
                s => return Some(s.to_string()),
            }
        }
        None
    }

    fn stem_tile(&self, tex: &str) -> Option<u32> {
        let stem = tex.rsplit('/').next()?;
        self.tile_of_stem.get(stem).copied()
    }

    /// Cuit le modèle d'un bloc en quads.
    pub fn bake(&mut self, name: &str, props: &[(&str, &str)]) -> Option<BakedModel> {
        let model_path = self.variant_model(name, props)?;
        let model = self.load_model(&model_path, 0)?;
        let elements = model.get("elements")?.as_arr()?;
        let maps: Vec<(String, Json)> = model
            .get("textures")
            .and_then(|t| t.as_obj())
            .map(|o| o.to_vec())
            .unwrap_or_default();
        let mut quads = Vec::new();
        for el in elements {
            let from = el.get("from")?.as_arr()?;
            let to = el.get("to")?.as_arr()?;
            if from.len() < 3 || to.len() < 3 {
                continue;
            }
            let (fx, fy, fz) = (
                from[0].as_num()? as f32 / 16.0,
                from[1].as_num()? as f32 / 16.0,
                from[2].as_num()? as f32 / 16.0,
            );
            let (tx, ty, tz) = (
                to[0].as_num()? as f32 / 16.0,
                to[1].as_num()? as f32 / 16.0,
                to[2].as_num()? as f32 / 16.0,
            );
            // rotation d'élément (origin, axis, angle)
            let mut rot = [[0.0f32; 3]; 3]; // matrice identité
            rot[0][0] = 1.0;
            rot[1][1] = 1.0;
            rot[2][2] = 1.0;
            let mut origin = [0.0f32; 3];
            if let Some(r) = el.get("rotation") {
                let axis = r.get("axis").and_then(|a| a.as_str()).unwrap_or("y");
                let angle = r.get("angle").and_then(|a| a.as_num()).unwrap_or(0.0);
                if angle != 0.0 {
                    if let Some(o) = r.get("origin").and_then(|o| o.as_arr()) {
                        if o.len() == 3 {
                            origin = [
                                o[0].as_num().unwrap_or(0.0) as f32 / 16.0,
                                o[1].as_num().unwrap_or(0.0) as f32 / 16.0,
                                o[2].as_num().unwrap_or(0.0) as f32 / 16.0,
                            ];
                        }
                    }
                    let rad = (angle as f32).to_radians();
                    let (s, c) = (rad.sin(), rad.cos());
                    rot = match axis {
                        "x" => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
                        "y" => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
                        _ => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
                    };
                }
            }
            let faces = el.get("faces")?;
            for (dir, f) in faces.as_obj()? {
                let tile = self
                    .resolve_texture(f.get("texture")?, &maps)
                    .and_then(|t| self.stem_tile(&t));
                let uvj = f.get("uv");
                let tint = f.get("tintindex").is_some();
                // 4 coins de la face (dans l'ordre uv)
                let (c0, c1, c2, c3) = match dir.as_str() {
                    "down" => ([fx, fy, fz], [fx, fy, tz], [tx, fy, tz], [tx, fy, fz]),
                    "up" => ([fx, ty, fz], [fx, ty, tz], [tx, ty, tz], [tx, ty, fz]),
                    "north" => ([tx, ty, fz], [tx, fy, fz], [fx, fy, fz], [fx, ty, fz]),
                    "south" => ([fx, ty, tz], [fx, fy, tz], [tx, fy, tz], [tx, ty, tz]),
                    "west" => ([fx, ty, fz], [fx, fy, fz], [fx, fy, tz], [fx, ty, tz]),
                    "east" => ([tx, ty, tz], [tx, fy, tz], [fx, fy, tz], [fx, ty, tz]),
                    _ => continue,
                };
                let (mut u0, mut v0, mut u1, mut v1) = (0.0f32, 0.0f32, 1.0f32, 1.0f32);
                if let Some(uv) = uvj.and_then(|u| u.as_arr()) {
                    if uv.len() == 4 {
                        u0 = uv[0].as_num().unwrap_or(0.0) as f32 / 16.0;
                        v0 = uv[1].as_num().unwrap_or(0.0) as f32 / 16.0;
                        u1 = uv[2].as_num().unwrap_or(16.0) as f32 / 16.0;
                        v1 = uv[3].as_num().unwrap_or(16.0) as f32 / 16.0;
                    }
                }
                let corners = [
                    apply_rot(rot, origin, c0),
                    apply_rot(rot, origin, c1),
                    apply_rot(rot, origin, c2),
                    apply_rot(rot, origin, c3),
                ];
                quads.push(Quad {
                    corners,
                    uv: [[u0, v0], [u0, v1], [u1, v1], [u1, v0]],
                    tile,
                    tint,
                });
            }
        }
        Some(BakedModel { quads })
    }
}

fn apply_rot(m: [[f32; 3]; 3], origin: [f32; 3], p: [f32; 3]) -> [f32; 3] {
    let d = [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]];
    let r = [
        m[0][0] * d[0] + m[0][1] * d[1] + m[0][2] * d[2],
        m[1][0] * d[0] + m[1][1] * d[1] + m[1][2] * d[2],
        m[2][0] * d[0] + m[2][1] * d[1] + m[2][2] * d[2],
    ];
    [r[0] + origin[0], r[1] + origin[1], r[2] + origin[2]]
}

/// Ombre par face depuis la normale du quad.
pub fn shade_of(corners: &[[f32; 3]; 4]) -> f32 {
    let a = [corners[1][0] - corners[0][0], corners[1][1] - corners[0][1], corners[1][2] - corners[0][2]];
    let b = [corners[3][0] - corners[0][0], corners[3][1] - corners[0][1], corners[3][2] - corners[0][2]];
    let n = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len < 1e-6 {
        return 1.0;
    }
    let n = [n[0] / len, n[1] / len, n[2] / len];
    // face la plus proche : +X,-X,+Y,-Y,+Z,-Z
    let face = if n[0].abs() >= n[1].abs() && n[0].abs() >= n[2].abs() {
        if n[0] > 0.0 {
            0
        } else {
            1
        }
    } else if n[1].abs() >= n[2].abs() {
        if n[1] > 0.0 {
            2
        } else {
            3
        }
    } else if n[2] > 0.0 {
        4
    } else {
        5
    };
    crate::mesher::SHADE[face]
}

// ------------------------------------------------------------- émission mesh

/// Pousse le modèle cuit dans le bucket cutout du mesh (uv atlas).
/// `tint_of_tile` donne la couleur de biome moyenne pour une tuile teintable.
#[allow(clippy::too_many_arguments)]
pub fn push_baked(
    out: &mut crate::mesher::MeshData,
    bm: &BakedModel,
    wx: i32,
    wz: i32,
    ly: usize,
    tiles: [u32; 3],
    tint_of_tile: &dyn Fn(u32) -> [f32; 3],
) {
    for q in &bm.quads {
        let tile = q.tile.unwrap_or(tiles[1]);
        let shade = shade_of(&q.corners);
        let base = (out.cutout_v.len() / 8) as u32;
        let (mut tr, mut tg, mut tb) = (1.0f32, 1.0f32, 1.0f32);
        if q.tint {
            let t = tint_of_tile(tile);
            tr = t[0];
            tg = t[1];
            tb = t[2];
        }
        for (c, corner) in q.corners.iter().enumerate() {
            let px = wx as f32 + corner[0];
            let py = ly as f32 + corner[1];
            let pz = wz as f32 + corner[2];
            let uv = crate::mesher::tile_uv_pub(tile, q.uv[c][0], q.uv[c][1]);
            let r = shade * tr;
            let g = shade * tg;
            let b = shade * tb;
            out.cutout_v.extend_from_slice(&[px, py, pz, uv[0], uv[1], r, g, b]);
        }
        out.cutout_i
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}
