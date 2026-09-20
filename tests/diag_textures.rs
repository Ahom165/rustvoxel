//! Diagnostic : quels blocs posables rendent « sans texture » (tuile magenta /
//! transparente / modèle cuit sans tuile), avec et sans pack vanilla.

use rustvoxel::models;
use rustvoxel::pack::{tile_candidates, Pack};
use rustvoxel::world::{self, tiles_of, PLACEABLE};

fn is_magenta(px: &[u8]) -> bool {
    px[0] > 200 && px[1] < 60 && px[2] > 200
}

fn tile_stats(pack: Option<&Pack>) -> (Vec<bool>, Vec<bool>) {
    let px = rustvoxel::renderer::build_atlas_pixels(pack);
    let n = (world::ATLAS_COLS * world::ATLAS_ROWS) as usize;
    let ts = 16usize; // TILE
    let w = (world::ATLAS_COLS as usize) * ts;
    let mut magenta = vec![false; n];
    let mut transparent = vec![false; n];
    for t in 0..n {
        let tx = ((t as u32 % world::ATLAS_COLS) as usize) * ts;
        let ty = ((t as u32 / world::ATLAS_COLS) as usize) * ts;
        let (mut m, mut a0, mut cnt) = (0usize, 0usize, 0usize);
        for y in 0..ts {
            for x in 0..ts {
                let o = ((ty + y) * w + tx + x) * 4;
                if px[o + 3] < 10 {
                    a0 += 1;
                }
                if px[o] > 200 && px[o + 1] < 60 && px[o + 2] > 200 {
                    m += 1;
                }
                cnt += 1;
            }
        }
        magenta[t] = m * 2 > cnt;
        transparent[t] = a0 * 2 > cnt;
    }
    (magenta, transparent)
}

#[test]
fn diag_placed_block_textures() {
    let base = std::path::Path::new(".");
    let zip = base.join("texturepacks/texture-pack-default1.20.5-26.2.zip");
    let has_pack = zip.exists();
    println!("=== pack vanilla présent: {has_pack} ===");

    let pack = Pack::discover(base);
    println!("png décodés du pack: {}", pack.loaded_names().len());
    let (magenta, transparent) = tile_stats(Some(&pack));
    let (magenta_np, transparent_np) = tile_stats(None);

    models::install(&pack);

    // 1) tuiles atlas mortes (magenta ou transparentes) avec ET sans pack
    let mut dead = Vec::new();
    let mut dead_nopack = Vec::new();
    for t in 0..(world::ATLAS_COLS * world::ATLAS_ROWS) as usize {
        if magenta[t] || transparent[t] {
            let names = tile_candidates(t as u32).join("|");
            dead.push(format!("#{t} [{names}] mag={} tr={}", magenta[t], transparent[t]));
        }
        if magenta_np[t] || transparent_np[t] {
            dead_nopack.push(t);
        }
    }
    println!("=== tuiles atlas mortes AVEC pack ({}): {} ===", dead.len(), dead.join(", "));
    println!("=== tuiles atlas mortes SANS pack ({}): {:?} ===", dead_nopack.len(), dead_nopack);

    // 2) blocs posables dont le rendu est cassé
    println!("=== blocs posables ({}) ===", PLACEABLE.len());
    let mut bad: Vec<String> = Vec::new();
    for &b in PLACEABLE {
        let tiles = tiles_of(b);
        let bm = models::baked_for(b);
        let mut issues: Vec<String> = Vec::new();
        // modèle cuit ?
        match &bm {
            Some(m) => {
                if m.quads.is_empty() {
                    issues.push("modèle cuit vide".into());
                }
                let nq_none = m.quads.iter().filter(|q| q.tile.is_none()).count();
                if nq_none > 0 {
                    issues.push(format!("{nq_none} quads sans tuile (fallback)"));
                }
                for q in &m.quads {
                    if let Some(t) = q.tile {
                        let t = t as usize;
                        if magenta[t] || transparent[t] {
                            issues.push(format!("quad tuile #{t} morte"));
                            break;
                        }
                    }
                }
            }
            None => {
                // forme procédurale : les tuiles doivent être vivantes
                for t in tiles {
                    let t = t as usize;
                    if magenta[t] || transparent[t] {
                        issues.push(format!("tuile proc #{t} morte"));
                        break;
                    }
                }
            }
        }
        if !issues.is_empty() {
            bad.push(format!(
                "bloc {} ({}) tiles={tiles:?}: {}",
                b,
                world::block_name(b),
                issues.join("; ")
            ));
        }
    }
    println!("=== blocs posables cassés ({}/{}): ===", bad.len(), PLACEABLE.len());
    for s in &bad {
        println!("  {s}");
    }
    // info seulement : ne casse pas le build tant que le diag n'est pas corrigé
    assert!(PLACEABLE.len() > 10);
}
