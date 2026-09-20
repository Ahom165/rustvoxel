//! Persistance du monde au format **Anvil** vanilla (fichiers région `.mca`).
//!
//! Format disque de Minecraft (1.18+) : `<monde>/region/r.<rx>.<rz>.mca`,
//! 32×32 chunks par région, chaque chunk = NBT compressé (zlib/gzip).
//! Intérêts :
//! - le monde du serveur survit aux redémarrages (comme avant, mais dans le
//!   format standard) ;
//! - un monde vanilla réel peut être servi tel quel :
//!   `rustvoxel_server --world "<.minecraft>/saves/Mon monde"` — les chunks
//!   sont lus dans `region/*.mca`, la graine dans `level.dat` ;
//! - round-trip exact de MONDE rustvoxel : les palettes écrites portent une
//!   propriété `rvx` (id interne) que vanilla ignore silencieusement.
//!
//! Fenêtre verticale : le monde interne est y∈[0,127] (8 sections). Les
//! sections vanilla hors fenêtre (y<0 : deepslate/bedrock, y>127 : sommets)
//! ne sont PAS conservées en mémoire, mais les chunks jamais édités gardent
//! leurs octets d'origine dans la région (réécriture qui préserve).
//!
//! Zéro dépendance : inflate réutilise `pack::inflate` (RFC 1951 complet),
//! la compression est un DEFLATE fixed-Huffman + LZ77 écrit ici (RFC 1951),
//! encapsulé zlib (RFC 1950) ou lu en gzip (RFC 1952).

use crate::nbt::{Nbt, NbtRdr};
use crate::world::{Chunk, World, CX, CZ};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// DataVersion 1.20.5 (les chunks écrits sont étiquetés avec).
pub const DATA_VERSION: i32 = 3839;
/// Fenêtre verticale conservée : sections 0..=7 → y 0..=127 (= CY).
pub const KEEP_SECTIONS: std::ops::RangeInclusive<i32> = 0..=7;
const SECT: usize = 4096;
const MAX_CACHED_REGIONS: usize = 24;

// ============================================================ somme de contrôle

fn adler32(d: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &x in d {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn crc32_table() -> &'static [u32; 256] {
    use std::sync::OnceLock;
    static T: OnceLock<[u32; 256]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u32; 256];
        for (i, e) in t.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *e = c;
        }
        t
    })
}

pub fn crc32(d: &[u8]) -> u32 {
    let t = crc32_table();
    let mut c = 0xFFFF_FFFFu32;
    for &x in d {
        c = t[((c ^ x as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

// ============================================================ DEFLATE (compresseur)

/// Écrivain de bits LSB-first (RFC 1951 : les bits arrivent du poids faible).
struct BitW {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitW {
    fn push(&mut self, v: u32, n: u32) {
        if n == 0 {
            return;
        }
        self.acc |= ((v as u64) & ((1u64 << n) - 1)) << self.n;
        self.n += n;
        while self.n >= 8 {
            self.out.push((self.acc & 0xFF) as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    /// Code Huffman : écrit MSB d'abord (dans le flux LSB-first → bits inversés).
    fn push_rev(&mut self, code: u32, n: u32) {
        let mut r = 0u32;
        for i in 0..n {
            if code & (1 << i) != 0 {
                r |= 1 << (n - 1 - i);
            }
        }
        self.push(r, n);
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push((self.acc & 0xFF) as u8);
        }
        self.out
    }
}

// Codes fixed-Huffman (RFC 1951 §3.2.6).
fn fixed_lit(sym: u16) -> (u32, u32) {
    match sym {
        0..=143 => (0x30 + sym as u32, 8),
        144..=255 => (0x190 + (sym as u32 - 144), 9),
        256..=279 => (sym as u32 - 256, 7),
        _ => (0xC0 + (sym as u32 - 280), 8),
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn len_sym(len: u16) -> usize {
    debug_assert!((3..=258).contains(&len));
    let mut i = 0;
    while i + 1 < 29 && LEN_BASE[i + 1] <= len {
        i += 1;
    }
    i
}

fn dist_sym(d: u16) -> usize {
    let mut i = 0;
    while i + 1 < 30 && DIST_BASE[i + 1] <= d {
        i += 1;
    }
    i
}

const WSIZE: usize = 32768;
const HSIZE: usize = 1 << 15;
const MAX_CHAIN: usize = 96;
const NICE_LEN: usize = 130;

#[inline]
fn hash3(d: &[u8], i: usize) -> usize {
    let v = (d[i] as u32) | ((d[i + 1] as u32) << 8) | ((d[i + 2] as u32) << 16);
    (v.wrapping_mul(0x9E37_79B1) >> 17) as usize & (HSIZE - 1)
}

/// Compresseur DEFLATE fixed-Huffman, appariement LZ77 glouton (chaînes de hachage).
fn deflate_fixed(d: &[u8]) -> Vec<u8> {
    let mut w = BitW { out: Vec::with_capacity(d.len() / 2 + 64), acc: 0, n: 0 };
    w.push(1, 1); // BFINAL
    w.push(1, 2); // BTYPE = fixed Huffman
    let mut head = vec![-1i32; HSIZE];
    let mut prev = vec![-1i32; WSIZE];
    let n = d.len();
    let mut i = 0usize;
    let emit_lit = |w: &mut BitW, b: u8| {
        let (c, k) = fixed_lit(b as u16);
        w.push_rev(c, k);
    };
    while i < n {
        let mut best_len = 0usize;
        let mut best_dist = 0usize;
        if i + 3 <= n {
            let h = hash3(d, i);
            let mut cand = head[h];
            let mut chain = 0usize;
            let max_len = (n - i).min(258);
            while cand >= 0 && chain < MAX_CHAIN {
                let c = cand as usize;
                let dist = i - c;
                if dist > WSIZE {
                    break;
                }
                // longueur commune rapide puis extension
                if d[c + best_len.min(max_len - 1).min((n - c).saturating_sub(1))] == d[i + best_len.min(max_len - 1).min((n - i).saturating_sub(1))] || best_len == 0 {
                    let mut l = 0usize;
                    while l < max_len && d[c + l] == d[i + l] {
                        l += 1;
                    }
                    if l > best_len {
                        best_len = l;
                        best_dist = dist;
                        if l >= NICE_LEN {
                            break;
                        }
                    }
                }
                chain += 1;
                let nx = prev[c & (WSIZE - 1)];
                if nx == cand || nx < 0 || (c as i64 - nx as i64) > WSIZE as i64 {
                    break;
                }
                cand = nx;
            }
        }
        if best_len >= 3 {
            let s = len_sym(best_len as u16) as u16;
            let (code, k) = fixed_lit(257 + s);
            w.push_rev(code, k);
            let li = len_sym(best_len as u16);
            let extra = (best_len as u32) - LEN_BASE[li] as u32;
            w.push(extra, LEN_EXTRA[li]);
            let di = dist_sym(best_dist as u16);
            w.push_rev(di as u32, 5);
            w.push((best_dist as u32) - DIST_BASE[di] as u32, DIST_EXTRA[di]);
            // insère les hachages de la séquence couverte (hors 2 premiers déjà faits)
            let end = (i + best_len).min(n.saturating_sub(2));
            let mut j = i + 1;
            while j < end {
                if j + 3 <= n {
                    let h = hash3(d, j);
                    prev[j & (WSIZE - 1)] = head[h];
                    head[h] = j as i32;
                }
                j += 1;
            }
            i += best_len;
        } else {
            if i + 3 <= n {
                let h = hash3(d, i);
                prev[i & (WSIZE - 1)] = head[h];
                head[h] = i as i32;
            }
            emit_lit(&mut w, d[i]);
            i += 1;
        }
    }
    let (c, k) = fixed_lit(256);
    w.push_rev(c, k);
    w.finish()
}

/// Compresse en zlib (RFC 1950) : en-tête 0x78 0x01 + deflate + adler32 BE.
pub fn zlib_compress(d: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(d.len() / 2 + 64);
    out.push(0x78);
    out.push(0x01);
    out.extend_from_slice(&deflate_fixed(d));
    out.extend_from_slice(&adler32(d).to_be_bytes());
    out
}

// ============================================================ décompresseurs

/// Décompresse un flux zlib complet (en-tête + deflate + adler32 vérifié).
pub fn zlib_decompress(d: &[u8]) -> Option<Vec<u8>> {
    if d.len() < 6 {
        return None;
    }
    let (cmf, flg) = (d[0], d[1]);
    if cmf & 0x0F != 8 || (u16::from(cmf) << 8 | u16::from(flg)) % 31 != 0 || flg & 0x20 != 0 {
        return None;
    }
    let raw = crate::pack::inflate(&d[2..d.len() - 4])?;
    let want = u32::from_be_bytes(d[d.len() - 4..].try_into().ok()?);
    if adler32(&raw) != want {
        return None;
    }
    Some(raw)
}

/// Décompresse un flux gzip (RFC 1952) — level.dat, chunks type 1.
pub fn gzip_decompress(d: &[u8]) -> Option<Vec<u8>> {
    if d.len() < 20 || d[0] != 0x1F || d[1] != 0x8B || d[2] != 8 {
        return None;
    }
    let flg = d[3];
    let mut p = 10usize;
    let field_end = |d: &[u8], mut p: usize| -> Option<usize> {
        while p < d.len() && d[p] != 0 {
            p += 1;
        }
        if p < d.len() {
            Some(p + 1)
        } else {
            None
        }
    };
    if flg & 4 != 0 {
        if p + 2 > d.len() {
            return None;
        }
        let n = u16::from_le_bytes(d[p..p + 2].try_into().ok()?) as usize;
        p += 2 + n;
    }
    if flg & 8 != 0 {
        p = field_end(d, p)?;
    }
    if flg & 16 != 0 {
        p = field_end(d, p)?;
    }
    if flg & 2 != 0 {
        p += 2;
    }
    if p + 8 > d.len() {
        return None;
    }
    let raw = crate::pack::inflate(&d[p..d.len() - 8])?;
    let crc = u32::from_le_bytes(d[d.len() - 8..d.len() - 4].try_into().ok()?);
    if crc32(&raw) != crc {
        return None;
    }
    Some(raw)
}

// ============================================================ fichiers région

pub struct RegionData {
    /// 1024 slots : (type compression, payload compressé) bruts.
    pub slots: Vec<Option<(u8, Vec<u8>)>>,
}

impl RegionData {
    fn empty() -> RegionData {
        RegionData { slots: vec![None; 1024] }
    }
}

#[inline]
fn region_of(c: i32) -> i32 {
    c >> 5
}

#[inline]
fn slot_of(cx: i32, cz: i32) -> usize {
    ((cx & 31) + (cz & 31) * 32) as usize
}

fn region_path(dir: &Path, rx: i32, rz: i32) -> PathBuf {
    dir.join(format!("r.{rx}.{rz}.mca"))
}

fn region_key(cx: i32, cz: i32) -> (i32, i32) {
    (region_of(cx), region_of(cz))
}

/// Lit un `.mca` : conserve les payloads BRUTS (compression + octets) des 1024
/// slots. Les chunks non touchés sont réécrits tels quels (zéro perte).
fn read_region(path: &Path) -> Option<RegionData> {
    let d = std::fs::read(path).ok()?;
    if d.len() < 8192 {
        return None;
    }
    let mut rd = RegionData::empty();
    for i in 0..1024usize {
        let e = i * 4;
        let off = ((d[e] as usize) << 16) | ((d[e + 1] as usize) << 8) | d[e + 2] as usize;
        if off == 0 || d[e + 3] == 0 {
            continue;
        }
        let start = off * SECT;
        if start + 5 > d.len() {
            continue;
        }
        let len = u32::from_be_bytes(d[start..start + 4].try_into().ok()?) as usize;
        if len < 1 || start + 4 + len > d.len() {
            continue;
        }
        let typ = d[start + 4];
        rd.slots[i] = Some((typ, d[start + 5..start + 4 + len].to_vec()));
    }
    Some(rd)
}

/// Réécrit un `.mca` complet (layout compacté, écriture atomique tmp+rename).
fn write_region(path: &Path, slots: &[Option<(u8, Vec<u8>)>]) -> std::io::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|t| t.as_secs() as u32)
        .unwrap_or(0);
    let mut header = vec![0u8; 8192];
    let mut body: Vec<u8> = Vec::new();
    let mut next: usize = 2; // secteurs 0 (offsets) et 1 (timestamps) réservés
    for (i, slot) in slots.iter().enumerate() {
        let (typ, data) = match slot {
            Some(x) => x,
            None => continue,
        };
        let plen = data.len() + 1; // le champ longueur compte l'octet de type
        // + 4 : le champ longueur LUI-MÊME occupe le début du premier secteur
        let total = plen + 4;
        let need = total.div_ceil(SECT);
        header[i * 4] = (next >> 16) as u8;
        header[i * 4 + 1] = (next >> 8) as u8;
        header[i * 4 + 2] = next as u8;
        header[i * 4 + 3] = need.min(255) as u8;
        let ts = now.to_be_bytes();
        header[4096 + i * 4..4096 + i * 4 + 4].copy_from_slice(&ts);
        body.extend_from_slice(&(plen as u32).to_be_bytes());
        body.push(*typ);
        body.extend_from_slice(data);
        let pad = need * SECT - total;
        body.extend(std::iter::repeat(0u8).take(pad));
        next += need;
    }
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        use std::io::Write;
        let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        f.write_all(&header)?;
        f.write_all(&body)?;
        f.flush()?;
    }
    std::fs::rename(&tmp, path)
}

/// Un `.mca` est-il présent dans le dossier région ?
pub fn dir_has_regions(world_dir: &Path) -> bool {
    let rd = world_dir.join("region");
    match std::fs::read_dir(&rd) {
        Ok(it) => it.filter_map(|e| e.ok()).any(|e| {
            e.path().extension().map(|x| x == "mca").unwrap_or(false)
        }),
        Err(_) => false,
    }
}

// ============================================================ palettes de blocs

const COLORS: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray",
    "cyan", "purple", "blue", "brown", "green", "red", "black",
];

#[inline]
fn color_of(b: u16, base: u16) -> Option<usize> {
    if b >= base && b < base + 16 {
        Some((b - base) as usize)
    } else {
        None
    }
}

/// Nom vanilla (SANS préfixe) utilisé SUR LE DISQUE pour le bloc interne `b`.
/// Les familles colorées reçoivent leur vraie couleur (plus joli dans vanilla
/// que le blanc systématique de block_vanilla_name) ; l'ambiguïté restante
/// (dalles/escaliers de laine = nom de laine pleine) est levée par la
/// propriété `rvx` écrite dans la palette.
fn disk_name(b: u16) -> String {
    let fam = |base: u16, suffix: &str| {
        color_of(b, base).map(|i| format!("{}_{}", COLORS[i], suffix))
    };
    fam(crate::world::WOOL_COLOR_BASE, "wool")
        .or_else(|| fam(crate::world::WOOL_SLAB_BASE, "wool"))
        .or_else(|| fam(crate::world::WOOL_STAIRS_BASE, "wool"))
        .or_else(|| fam(crate::world::CONCRETE_BASE, "concrete"))
        .or_else(|| fam(crate::world::CONC_SLAB_BASE, "concrete"))
        .or_else(|| fam(crate::world::CONC_STAIRS_BASE, "concrete"))
        .or_else(|| fam(crate::world::CUSHION_BASE, "carpet"))
        .unwrap_or_else(|| crate::vanilla_data::block_vanilla_name(b).to_string())
}

/// Table inverse nom (sans préfixe) → id interne. Premier id gagnant :
/// `white_wool` → WOOL (30) et non la nuance blanche (80) — visuellement
/// identique, et `rvx` lève le doute au chargement.
fn reverse_map() -> &'static HashMap<&'static str, u16> {
    use std::sync::OnceLock;
    static M: OnceLock<HashMap<&'static str, u16>> = OnceLock::new();
    M.get_or_init(|| {
        let mut m = HashMap::new();
        for id in 0u16..=223 {
            let n = disk_name(id);
            let leased = Box::<str>::try_from(n).unwrap_or_default();
            if leased.is_empty() {
                continue;
            }
            let s: &'static str = Box::leak(leased);
            m.entry(s).or_insert(id);
        }
        m
    })
}

/// Alias de chargement : noms vanilla courants absents de la table interne →
/// nom interne le plus proche (les plantes disparaissent plutôt que de
/// devenir des colonnes de pierre).
fn alias_of(short: &str) -> Option<&'static str> {
    const A: &[(&str, &str)] = &[
        ("tall_grass", "short_grass"),
        ("fern", "short_grass"),
        ("large_fern", "short_grass"),
        ("bush", "short_grass"),
        ("wheat", "short_grass"),
        ("carrots", "short_grass"),
        ("potatoes", "short_grass"),
        ("beetroots", "short_grass"),
        ("vine", "short_grass"),
        ("glow_lichen", "short_grass"),
        ("nether_sprouts", "short_grass"),
        ("redstone_wire", "air"),
        ("rail", "air"),
        ("powered_rail", "air"),
        ("detector_rail", "air"),
        ("activator_rail", "air"),
        ("cobweb", "air"),
        ("scaffolding", "air"),
        ("ice", "packed_ice"),
        ("frosted_ice", "packed_ice"),
        ("clay", "mud"),
        ("stone_bricks", "stone"),
        ("mossy_stone_bricks", "stone"),
        ("chiseled_stone_bricks", "stone"),
        ("cracked_stone_bricks", "stone"),
        ("smooth_stone", "stone"),
        ("infested_stone", "stone"),
        ("andesite_bricks", "andesite"),
        ("cobbled_deepslate", "deepslate"),
        ("deepslate_bricks", "deepslate"),
        ("deepslate_tiles", "deepslate"),
        ("chiseled_deepslate", "deepslate"),
        ("polished_deepslate", "deepslate"),
        ("cracked_deepslate_bricks", "deepslate"),
        ("cracked_deepslate_tiles", "deepslate"),
        ("reinforced_deepslate", "deepslate"),
        ("dirt_path", "coarse_dirt"),
        ("rooted_dirt", "dirt"),
        ("mycelium", "podzol"),
        ("netherrack", "stone"),
        ("soul_sand", "mud"),
        ("soul_soil", "mud"),
        ("mossy_cobblestone", "moss_block"),
        ("polished_andesite", "andesite"),
        ("polished_diorite", "diorite"),
        ("polished_granite", "granite"),
        ("polished_basalt", "basalt"),
        ("smooth_sandstone", "sandstone"),
        ("cut_sandstone", "sandstone"),
        ("smooth_red_sandstone", "red_sandstone"),
        ("cut_red_sandstone", "red_sandstone"),
        ("chest", "barrel"),
        ("trapped_chest", "barrel"),
        ("ender_chest", "barrel"),
        ("furnace", "barrel"),
        ("blast_furnace", "barrel"),
        ("smoker", "barrel"),
        ("hopper", "barrel"),
        ("dispenser", "barrel"),
        ("dropper", "barrel"),
        ("observer", "barrel"),
        ("crafting_table", "oak_planks"),
        ("fletching_table", "oak_planks"),
        ("cartography_table", "oak_planks"),
        ("smithing_table", "oak_planks"),
        ("loom", "oak_planks"),
        ("bookshelf", "oak_planks"),
        ("chiseled_bookshelf", "oak_planks"),
        ("lectern", "oak_planks"),
        ("composter", "barrel"),
        ("note_block", "oak_planks"),
        ("jukebox", "oak_planks"),
        ("oak_slab", "oak_planks"),
        ("oak_stairs", "oak_planks"),
        ("oak_door", "oak_planks"),
        ("oak_trapdoor", "oak_planks"),
        ("oak_fence", "oak_planks"),
        ("oak_fence_gate", "oak_planks"),
        ("spruce_slab", "spruce_planks"),
        ("birch_slab", "birch_planks"),
        ("stone_slab", "stone"),
        ("cobblestone_slab", "cobblestone"),
        ("brick_slab", "bricks"),
        ("glass_pane", "glass"),
        ("iron_bars", "glass"),
        ("lantern", "torch"),
        ("soul_lantern", "torch"),
        ("soul_torch", "torch"),
        ("redstone_torch", "torch"),
        ("candle", "torch"),
        ("sea_lantern", "glowstone"),
        ("shroomlight", "glowstone"),
        ("ochre_froglight", "glowstone"),
        ("verdant_froglight", "glowstone"),
        ("pearlescent_froglight", "glowstone"),
        ("ladder", "oak_planks"),
        ("bell", "glowstone"),
        ("spawner", "stone"),
        ("bell_block", "glowstone"),
    ];
    A.iter().find(|(k, _)| *k == short).map(|(_, v)| *v)
}

/// id interne pour un nom vanilla court — résolution en cascade.
fn block_from_name(short: &str) -> Option<u16> {
    let m = reverse_map();
    if let Some(&id) = m.get(short) {
        return Some(id);
    }
    if let Some(a) = alias_of(short) {
        if let Some(&id) = m.get(a) {
            return Some(id);
        }
    }
    // air sous toutes ses formes
    if short == "air" || short == "cave_air" || short == "void_air" {
        return Some(crate::world::AIR);
    }
    // eau (n'importe quel niveau)
    if short.contains("water") {
        return Some(crate::world::WATER);
    }
    if short.contains("coral") {
        if let Some(&id) = m.get("tube_coral_block") {
            return Some(id);
        }
    }
    // suffixes de formes : enlever UN suffixe puis retenter (nom + alias)
    const SUF: [&str; 11] = [
        "_slab", "_stairs", "_wall", "_fence_gate", "_fence", "_trapdoor", "_door", "_sign",
        "_button", "_pressure_plate", "_pane",
    ];
    for s in SUF {
        if let Some(base) = short.strip_suffix(s) {
            if let Some(&id) = m.get(base) {
                return Some(id);
            }
            if let Some(a) = alias_of(base) {
                if let Some(&id) = m.get(a) {
                    return Some(id);
                }
            }
        }
    }
    None
}

/// Entrée de palette DISQUE pour le bloc interne `b` :
/// {Name, Properties?, rvx?} — `rvx` seulement si le nom ne suffit pas à
/// retrouver `b` (vanilla ignore les propriétés inconnues avec un simple log).
fn palette_entry(b: u16) -> Nbt {
    let mut e = Nbt::compound();
    e.set("Name", Nbt::Str(format!("minecraft:{}", disk_name(b))));
    let props = crate::vanilla_data::block_props(b);
    let needs_rvx = reverse_map().get(disk_name(b).as_str()) != Some(&b)
        || !props.is_empty();
    if !props.is_empty() || needs_rvx {
        let mut p = Nbt::compound();
        for (k, v) in props {
            p.set(k, Nbt::Str((*v).to_string()));
        }
        if needs_rvx {
            p.set("rvx", Nbt::Str(b.to_string()));
        }
        e.set("Properties", p);
    }
    e
}

/// id interne depuis une entrée de palette disque (vanilla OU rustvoxel).
fn block_from_palette(e: &Nbt) -> u16 {
    let fallback = |short: Option<&str>| -> u16 {
        match short.and_then(block_from_name) {
            Some(id) => id,
            None => crate::world::STONE, // inconnu : conserve la masse
        }
    };
    let name = match e.get("Name").and_then(|n| n.as_str()) {
        Some(n) => n,
        None => return fallback(None),
    };
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    if let Some(Nbt::Compound(props)) = e.get("Properties") {
        if let Some((_, Nbt::Str(rv))) = props.iter().find(|(k, _)| k == "rvx") {
            if let Ok(v) = rv.parse::<u16>() {
                if v <= 223 {
                    return v;
                }
            }
        }
    }
    fallback(Some(short))
}

// ============================================================ tableaux de longs

fn ceil_log2s(v: usize) -> u32 {
    let mut n = 0u32;
    let mut v = v.saturating_sub(1);
    while v > 0 {
        n += 1;
        v >>= 1;
    }
    n
}

/// Paquet disque (identique réseau : les valeurs ne traversent pas les longs).
fn pack_longs_disk(vals: &[u32], bits: u32) -> Vec<i64> {
    let per = (64 / bits) as usize;
    let mut out = Vec::with_capacity(vals.len().div_ceil(per));
    let mut acc: u64 = 0;
    let mut j = 0u32;
    for &v in vals {
        acc |= (v as u64 & ((1u64 << bits) - 1)) << j;
        j += bits;
        if j + bits > 64 {
            out.push(acc as i64);
            acc = 0;
            j = 0;
        }
    }
    if j > 0 {
        out.push(acc as i64);
    }
    out
}

/// Dén paquet disque : devine les bits (les fichiers vanilla varient) en
/// cherchant la largeur dont le nombre de longs attendu correspond exactement.
fn unpack_longs_disk(longs: &[i64], min_bits: u32, count: usize) -> Option<Vec<u32>> {
    for bits in min_bits..=15u32 {
        let per = (64 / bits) as usize;
        let need = count.div_ceil(per);
        if longs.len() < need {
            return None; // trop court : aucun bits supérieur n'aidera (need croît)
        }
        if longs.len() > need {
            continue; // trop long : les bits doivent être plus grands
        }
        let mut out = Vec::with_capacity(count);
        'outer: for l in longs {
            let v = *l as u64;
            let mut j = 0u32;
            while j + bits <= 64 {
                out.push(((v >> j) & ((1u64 << bits) - 1)) as u32);
                if out.len() == count {
                    break 'outer;
                }
                j += bits;
            }
        }
        while out.len() < count {
            out.push(0);
        }
        return Some(out);
    }
    None
}

// ============================================================ NBT de chunk

/// Palette de biomes (index interne → nom vanilla sans préfixe).
fn biome_name(i: u8) -> &'static str {
    crate::vanilla_data::BIOME_ENTRIES
        .get(i as usize)
        .map(|(n, ..)| *n)
        .unwrap_or("plains")
}

fn biome_from_name(short: &str) -> u8 {
    let e = &crate::vanilla_data::BIOME_ENTRIES;
    if let Some(i) = e.iter().position(|(n, ..)| *n == short) {
        return i as u8;
    }
    const A: &[(&str, u8)] = &[
        ("taiga", 1),               // forest
        ("snowy_taiga", 4),         // snowy_plains
        ("dark_forest", 1),
        ("old_growth_pine_taiga", 1),
        ("old_growth_spruce_taiga", 1),
        ("old_growth_birch_forest", 2),
        ("sunflower_plains", 0),
        ("river", 0),
        ("frozen_river", 4),
        ("beach", 0),
        ("snowy_beach", 4),
        ("ocean", 0),
        ("deep_ocean", 0),
        ("frozen_ocean", 4),
        ("warm_ocean", 0),
        ("lukewarm_ocean", 0),
        ("cold_ocean", 0),
        ("deep_frozen_ocean", 4),
        ("deep_cold_ocean", 0),
        ("deep_lukewarm_ocean", 0),
        ("ice_spikes", 4),
        ("windswept_hills", 12),    // meadow
        ("windswept_gravelly_hills", 12),
        ("windswept_forest", 1),
        ("savanna_plateau", 7),
        ("windswept_savanna", 7),
        ("bamboo_jungle", 9),
        ("sparse_jungle", 9),
        ("wooded_badlands", 8),
        ("eroded_badlands", 8),
        ("swamp_hills", 5),
        ("mangrove_swamp", 5),
        ("stony_shore", 15),
        ("mushroom_fields", 0),
        ("dripstone_caves", 0),
        ("lush_caves", 0),
        ("deep_dark", 0),
        ("the_void", 0),
        ("nether_wastes", 0),
        ("the_end", 0),
        ("end_barrens", 0),
        ("end_highlands", 0),
        ("end_midlands", 0),
        ("small_end_islands", 0),
    ];
    A.iter().find(|(k, _)| *k == short).map(|(_, i)| *i).unwrap_or(0)
}

/// NBT de chunk Anvil complet (24 sections, format 1.18+).
pub fn chunk_nbt(chunk: &Chunk, cx: i32, cz: i32, biome_col: &[u8; 256]) -> Nbt {
    let mut root = Nbt::compound();
    root.set("DataVersion", Nbt::Int(DATA_VERSION));
    root.set("xPos", Nbt::Int(cx));
    root.set("zPos", Nbt::Int(cz));
    root.set("yPos", Nbt::Int(-4));
    root.set("Status", Nbt::Str("minecraft:full".into()));
    root.set("LastUpdate", Nbt::Long(0));
    root.set("InhabitedTime", Nbt::Long(0));
    let mut sections = Vec::with_capacity(24);
    for k in -4i32..=19 {
        let mut s = Nbt::compound();
        s.set("Y", Nbt::Byte(k as i8));
        if KEEP_SECTIONS.contains(&k) {
            let y0 = (k * 16) as usize;
            // blocs : palette + data (ordre YZX)
            let mut pal: Vec<u16> = Vec::new();
            let mut idx = vec![0u32; 4096];
            let mut raw = [0u16; 4096];
            for ly in 0..16usize {
                for lz in 0..16usize {
                    for lx in 0..16usize {
                        let b = chunk.blocks[lx + CX * (lz + CZ * (y0 + ly))];
                        raw[(ly << 8) | (lz << 4) | lx] = b;
                    }
                }
            }
            for (i, &b) in raw.iter().enumerate() {
                match pal.iter().position(|&p| p == b) {
                    Some(p) => idx[i] = p as u32,
                    None => {
                        idx[i] = pal.len() as u32;
                        pal.push(b);
                    }
                }
            }
            let mut bs = Nbt::compound();
            bs.set("palette", Nbt::List(pal.iter().map(|&b| palette_entry(b)).collect()));
            if pal.len() > 1 {
                let bits = ceil_log2s(pal.len()).max(4);
                bs.set("data", Nbt::LongArray(pack_longs_disk(&idx, bits)));
            }
            s.set("block_states", bs);
            // biomes : uniformes verticalement dans notre monde
            let mut bpal: Vec<u8> = Vec::new();
            let mut bidx = vec![0u32; 64];
            for by in 0..4usize {
                for bz in 0..4usize {
                    for bx in 0..4usize {
                        let b = biome_col[(bx * 4) * 16 + (bz * 4)];
                        let cell = (by << 4) | (bz << 2) | bx;
                        match bpal.iter().position(|&p| p == b) {
                            Some(p) => bidx[cell] = p as u32,
                            None => {
                                bidx[cell] = bpal.len() as u32;
                                bpal.push(b);
                            }
                        }
                    }
                }
            }
            let mut bi = Nbt::compound();
            bi.set(
                "palette",
                Nbt::List(bpal.iter().map(|&b| Nbt::Str(format!("minecraft:{}", biome_name(b)))).collect()),
            );
            if bpal.len() > 1 {
                let bits = ceil_log2s(bpal.len()).max(1);
                bi.set("data", Nbt::LongArray(pack_longs_disk(&bidx, bits)));
            }
            s.set("biomes", bi);
        } else {
            // section hors fenêtre : vide mais vanilla-conforme
            let mut bs = Nbt::compound();
            bs.set(
                "palette",
                Nbt::List(vec![palette_entry(crate::world::AIR)]),
            );
            s.set("block_states", bs);
            let mut bi = Nbt::compound();
            bi.set("palette", Nbt::List(vec![Nbt::Str("minecraft:plains".into())]));
            s.set("biomes", bi);
        }
        sections.push(s);
    }
    root.set("sections", Nbt::List(sections));
    root.set("block_entities", Nbt::List(vec![]));
    root
}

/// Décode un NBT de chunk Anvil → (cx, cz, chunk, colonne de biomes ?).
/// Seules les sections y∈[0,127] alimentent le chunk interne.
pub fn chunk_from_nbt(root: &Nbt) -> Option<(i32, i32, Chunk, Option<[u8; 256]>)> {
    let cx = root.get("xPos").and_then(|v| v.as_i64())? as i32;
    let cz = root.get("zPos").and_then(|v| v.as_i64())? as i32;
    let mut ch = Chunk::new();
    let mut biome_col: Option<[u8; 256]> = None;
    if let Some(Nbt::List(sections)) = root.get("sections") {
        for s in sections {
            if !matches!(s, Nbt::Compound(_)) {
                continue;
            }
            let sy = s.get("Y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            if KEEP_SECTIONS.contains(&sy) {
                let y0 = (sy * 16) as usize;
                if let Some(Nbt::Compound(_)) = s.get("block_states") {
                    let bs = s.get("block_states").unwrap();
                    let empty: Vec<Nbt> = Vec::new();
                    let pal = match bs.get("palette") {
                        Some(Nbt::List(p)) => p,
                        _ => &empty,
                    };
                    if pal.is_empty() {
                        continue;
                    }
                    let ids: Vec<u16> = pal.iter().map(block_from_palette).collect();
                    // base = palette[0] partout, puis data par-dessus
                    for ly in 0..16usize {
                        for lz in 0..16usize {
                            for lx in 0..16usize {
                                ch.blocks[lx + CX * (lz + CZ * (y0 + ly))] = ids[0];
                            }
                        }
                    }
                    if let Some(Nbt::LongArray(longs)) = bs.get("data") {
                        let bits_min = ceil_log2s(ids.len()).max(4);
                        if let Some(vals) = unpack_longs_disk(longs, bits_min, 4096) {
                            for (i, &v) in vals.iter().enumerate() {
                                let b = ids.get(v as usize).copied().unwrap_or(ids[0]);
                                let ly = i >> 8;
                                let lz = (i >> 4) & 15;
                                let lx = i & 15;
                                ch.blocks[lx + CX * (lz + CZ * (y0 + ly))] = b;
                            }
                        }
                    }
                }
            }
            // biomes : la section y∈[64,80) donne la colonne (biome de surface)
            if sy == 4 && biome_col.is_none() {
                if let Some(Nbt::Compound(_)) = s.get("biomes") {
                    let bi = s.get("biomes").unwrap();
                    let empty: Vec<Nbt> = Vec::new();
                    let pal = match bi.get("palette") {
                        Some(Nbt::List(p)) => p,
                        _ => &empty,
                    };
                    if !pal.is_empty() {
                        let names: Vec<&str> = pal
                            .iter()
                            .map(|p| {
                                p.as_str()
                                    .map(|s| s.strip_prefix("minecraft:").unwrap_or(s))
                                    .unwrap_or("plains")
                            })
                            .collect();
                        let ids: Vec<u8> = names.iter().map(|n| biome_from_name(n)).collect();
                        let mut col = [0u8; 256];
                        let mut cells = vec![ids[0]; 64];
                        if let Some(Nbt::LongArray(longs)) = bi.get("data") {
                            let bits_min = ceil_log2s(ids.len()).max(1);
                            if let Some(vals) = unpack_longs_disk(longs, bits_min, 64) {
                                for (i, &v) in vals.iter().enumerate() {
                                    cells[i] = ids.get(v as usize).copied().unwrap_or(ids[0]);
                                }
                            }
                        }
                        for bz in 0..4usize {
                            for bx in 0..4usize {
                                let b = cells[(1 << 4) | (bz << 2) | bx];
                                for dy in 0..4usize {
                                    for dx in 0..4usize {
                                        let x = bx * 4 + dx;
                                        let z = bz * 4 + dy;
                                        col[x * 16 + z] = b;
                                    }
                                }
                            }
                        }
                        biome_col = Some(col);
                    }
                }
            }
        }
    }
    ch.modified = false;
    Some((cx, cz, ch, biome_col))
}

// ============================================================ store paresseux

/// Accès au monde Anvil : chargement paresseux par région (LRU), écriture par
/// région avec préservation des octets des chunks intacts.
pub struct AnvilStore {
    dir: PathBuf, // <monde>/region
    cache: HashMap<(i32, i32), Option<Arc<RegionData>>>,
    order: VecDeque<(i32, i32)>,
    /// chunks chargés depuis le disque (statistique de log)
    pub loaded_chunks: u64,
}

impl AnvilStore {
    pub fn open(world_dir: &Path) -> AnvilStore {
        AnvilStore {
            dir: world_dir.join("region"),
            cache: HashMap::new(),
            order: VecDeque::new(),
            loaded_chunks: 0,
        }
    }

    /// Le dossier contient-il déjà des données de monde ?
    pub fn has_world_data(&self) -> bool {
        dir_has_regions(self.dir.parent().unwrap_or(Path::new(".")))
    }

    /// Graine depuis `level.dat` (gzip + NBT) — worlds vanilla inclus.
    pub fn read_seed(&self) -> Option<u64> {
        let d = std::fs::read(self.dir.parent()?.join("level.dat")).ok()?;
        let raw = gzip_decompress(&d)?;
        let root = NbtRdr::new(&raw).read_disk_root()?;
        let data = root.get("Data")?;
        if let Some(v) = data.get("RandomSeed").and_then(|v| v.as_i64()) {
            return Some(v as u64);
        }
        data.get("WorldGenSettings")?
            .get("seed")
            .and_then(|v| v.as_i64())
            .map(|v| v as u64)
    }

    fn region(&mut self, r: (i32, i32)) -> Option<Arc<RegionData>> {
        if let Some(e) = self.cache.get(&r) {
            return e.clone();
        }
        let data = read_region(&region_path(&self.dir, r.0, r.1)).map(Arc::new);
        self.cache.insert(r, data.clone());
        self.order.push_back(r);
        while self.order.len() > MAX_CACHED_REGIONS {
            let old = self.order.pop_front().unwrap();
            if old != r {
                self.cache.remove(&old);
            }
        }
        data
    }

    /// Charge un chunk depuis le disque (None si généré : absent ou illisible).
    pub fn load_chunk(&mut self, cx: i32, cz: i32) -> Option<Chunk> {
        let rd = self.region(region_key(cx, cz))?;
        let (typ, comp) = rd.slots[slot_of(cx, cz)].as_ref()?;
        let raw = match *typ {
            1 => gzip_decompress(comp)?,
            2 => zlib_decompress(comp)?,
            3 => comp.clone(),
            _ => return None, // lz4/custom : non supporté
        };
        let root = NbtRdr::new(&raw).read_disk_root()?;
        let (_, _, mut ch, _) = chunk_from_nbt(&root)?;
        ch.modified = false; // déjà sur disque : pas besoin de réécriture
        self.loaded_chunks += 1;
        Some(ch)
    }

    /// Sauvegarde tous les chunks modifiés, regroupés par région. Les slots
    /// non modifiés d'une région sont conservés octet pour octet.
    /// Renvoie le nombre de chunks écrits.
    pub fn save_world(
        &mut self,
        w: &World,
        biomes: &HashMap<(i32, i32), [u8; 256]>,
    ) -> std::io::Result<usize> {
        let mut by_region: HashMap<(i32, i32), Vec<(i32, i32)>> = HashMap::new();
        for (&k, c) in &w.chunks {
            if c.modified {
                by_region.entry(region_key(k.0, k.1)).or_default().push(k);
            }
        }
        let mut saved = 0usize;
        for (r, mut list) in by_region {
            list.sort();
            let mut slots = match self.region(r) {
                Some(rd) => rd.slots.clone(),
                None => vec![None; 1024],
            };
            for &(cx, cz) in &list {
                let chunk = &w.chunks[&(cx, cz)];
                let biome = biomes
                    .get(&(cx, cz))
                    .map(|b| *b)
                    .unwrap_or([0u8; 256]);
                let nbt = chunk_nbt(chunk, cx, cz, &biome);
                let mut buf = Vec::with_capacity(8192);
                nbt.write_disk(&mut buf);
                let comp = zlib_compress(&buf);
                slots[slot_of(cx, cz)] = Some((2, comp));
                saved += 1;
            }
            std::fs::create_dir_all(&self.dir)?;
            write_region(&region_path(&self.dir, r.0, r.1), &slots)?;
            // rafraîchit le cache avec la version écrite
            self.cache.insert(r, Some(Arc::new(RegionData { slots })));
            self.order.retain(|&x| x != r);
            self.order.push_back(r);
            while self.order.len() > MAX_CACHED_REGIONS {
                let old = self.order.pop_front().unwrap();
                if old != r {
                    self.cache.remove(&old);
                }
            }
        }
        Ok(saved)
    }
}

// ============================================================ tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world;

    /// Données pseudo-aléatoires déterministes mêlant plages répétées et
    /// bruit (exercice littéraux + correspondances LZ77).
    fn lcg_data(n: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(n);
        let mut s: u64 = 0x1234_5678_9ABC_DEF0;
        while v.len() < n {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let k = (s >> 33) as usize;
            if k % 7 == 0 {
                let run = 3 + k % 250;
                let b = (k >> 8) as u8;
                for _ in 0..run.min(n - v.len()) {
                    v.push(b);
                }
            } else {
                v.push(k as u8);
            }
        }
        v
    }

    #[test]
    fn checksum_known_vectors() {
        // vecteurs de référence RFC / zoo de tests
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn zlib_roundtrip_and_pack_inflate_agree() {
        for size in [0usize, 1, 17, 4096, 70000] {
            let d = lcg_data(size);
            let z = zlib_compress(&d);
            assert_eq!(zlib_decompress(&z).unwrap(), d, "taille {}", size);
            // le corps deflate est aussi lu par l'inflater « vanilla files »
            assert_eq!(
                crate::pack::inflate(&z[2..z.len() - 4]).unwrap(),
                d,
                "corps deflate taille {}",
                size
            );
            // adler invalide → rejet
            let mut bad = z.clone();
            let l = bad.len();
            bad[l - 1] = bad[l - 1].wrapping_add(1);
            assert!(zlib_decompress(&bad).is_none());
        }
    }

    #[test]
    fn gzip_roundtrip_handbuilt() {
        let d = lcg_data(33000);
        // construit un gzip RFC 1952 : en-tête + deflate + crc32 + isize (LE)
        let mut g = vec![0x1Fu8, 0x8B, 8, 0, 0, 0, 0, 0, 0, 255];
        g.extend_from_slice(&deflate_fixed(&d));
        g.extend_from_slice(&crc32(&d).to_le_bytes());
        g.extend_from_slice(&(d.len() as u32).to_le_bytes());
        assert_eq!(gzip_decompress(&g).unwrap(), d);
        // crc corrompu → rejet
        let l = g.len();
        g[l - 5] ^= 0xFF;
        assert!(gzip_decompress(&g).is_none());
    }

    #[test]
    fn pack_unpack_longs_various_bits() {
        for bits in 4u32..=8 {
            let per = (64 / bits) as usize;
            let count = per * 3 + 5; // dernier long partiel
            let vals: Vec<u32> = (0..count).map(|i| (i * 2654435761) as u32 & ((1 << bits) - 1)).collect();
            let longs = pack_longs_disk(&vals, bits);
            assert_eq!(unpack_longs_disk(&longs, bits, count).unwrap(), vals);
        }
        // paquet vanilla 4 bits, 4096 entrées → 256 longs exactement
        let vals = vec![3u32; 4096];
        assert_eq!(pack_longs_disk(&vals, 4).len(), 256);
    }

    #[test]
    fn chunk_nbt_roundtrip_is_bit_exact() {
        let mut w = World::new(1);
        w.gen_chunk(0, 0);
        // familles ambiguës + eau + air + blocs à propriétés
        w.set_block(0, 70, 0, world::WOOL_STAIRS_BASE + 9);
        w.set_block(1, 70, 0, world::WOOL_COLOR_BASE + 14);
        w.set_block(2, 70, 0, world::CONC_SLAB_BASE + 2);
        w.set_block(3, 70, 0, world::CUSHION_BASE + 5);
        w.set_block(4, 70, 0, world::WATER);
        w.set_block(5, 70, 0, world::AIR);
        w.set_block(6, 70, 0, world::CHERRY_LOG);
        w.set_block(7, 70, 0, world::DEEPSLATE);
        let chunk = &w.chunks[&(0, 0)];
        let biome = [3u8; 256]; // desert
        let nbt = chunk_nbt(chunk, -7, 12, &biome);
        let (cx, cz, back, bio) = chunk_from_nbt(&nbt).unwrap();
        assert_eq!((cx, cz), (-7, 12));
        assert_eq!(back.blocks, chunk.blocks, "24 sections restituées bit à bit");
        assert_eq!(bio.unwrap(), biome);
    }

    #[test]
    fn vanilla_style_palette_without_rvx_is_understood() {
        // Palette écrite « comme vanilla » : pas de rvx, noms seuls
        // (couvre les alias et les replis).
        let entry = |n: &str| {
            let mut e = Nbt::compound();
            e.set("Name", Nbt::Str(format!("minecraft:{n}")));
            e
        };
        let pal = vec![
            entry("deepslate"),      // 0 : dans la table
            entry("red_wool"),       // 1 : couleur → WOOL_COLOR_BASE+14
            entry("tall_grass"),     // 2 : alias → TALLGRASS (short_grass)
            entry("shulker_box"),    // 3 : inconnu → STONE
            entry("cave_air"),       // 4 : → AIR
            entry("ice"),            // 5 : alias → PACKED_ICE
            entry("clay"),           // 6 : alias → MUD
        ];
        let mut vals = [0u32; 4096];
        for i in 0..4096 {
            vals[i] = (i * 7 % 11 == 0) as u32 * (1 + i % 6) as u32;
        }
        let mut bs = Nbt::compound();
        bs.set("palette", Nbt::List(pal));
        bs.set("data", Nbt::LongArray(pack_longs_disk(&vals, 4)));
        let mut sec = Nbt::compound();
        sec.set("Y", Nbt::Byte(2));
        sec.set("block_states", bs);
        let mut root = Nbt::compound();
        root.set("xPos", Nbt::Int(4));
        root.set("zPos", Nbt::Int(-4));
        root.set("sections", Nbt::List(vec![sec]));
        let (_, _, ch, _) = chunk_from_nbt(&root).unwrap();
        let expect = [
            world::DEEPSLATE,
            world::WOOL_COLOR_BASE + 14,
            world::TALLGRASS,
            world::STONE,
            world::AIR,
            world::PACKED_ICE,
            world::MUD,
        ];
        for i in 0..4096usize {
            let ly = i >> 8;
            let lz = (i >> 4) & 15;
            let lx = i & 15;
            let got = ch.blocks[lx + 16 * (lz + 16 * (32 + ly))];
            assert_eq!(got, expect[vals[i] as usize], "entrée {}", vals[i]);
        }
    }

    #[test]
    fn region_roundtrip_and_untouched_slot_preserved() {
        let base = std::env::temp_dir().join(format!("rvx_anvilt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        // 1) un premier serveur écrit un chunk marqué
        let mut w1 = World::new(3);
        w1.gen_chunk(0, 0);
        w1.set_block(10, 60, 10, world::WOOL_STAIRS_BASE + 9);
        let mut st1 = AnvilStore::open(&base);
        assert_eq!(st1.save_world(&w1, &HashMap::new()).unwrap(), 1);
        // 2) un DEUXIÈME monde ne réécrit QUE son chunk : le slot (0,0)
        //    doit survivre octet pour octet (chunks non modifiés préservés)
        let mut w2 = World::new(4);
        w2.gen_chunk(2, 0);
        w2.set_block(35, 60, 10, world::BRICK);
        let mut st2 = AnvilStore::open(&base);
        assert_eq!(st2.save_world(&w2, &HashMap::new()).unwrap(), 1);
        // 3) relecture : les deux chunks sont intacts
        let mut st3 = AnvilStore::open(&base);
        let a = st3.load_chunk(0, 0).expect("chunk préservé");
        assert_eq!(a.get(10, 60, 10), world::WOOL_STAIRS_BASE + 9);
        assert!(!a.modified, "chunk déjà sur disque : pas de réécriture");
        let b = st3.load_chunk(2, 0).expect("chunk réécrit");
        assert_eq!(b.get(3, 60, 10), world::BRICK);
        // 4) chunk absent → None (génération procédurale côté serveur)
        assert!(st3.load_chunk(9, 9).is_none());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn seed_read_from_leveldat_gzip() {
        let base = std::env::temp_dir().join(format!("rvx_lvl_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("region")).unwrap();
        let mut data = Nbt::compound();
        let mut d = Nbt::compound();
        d.set("RandomSeed", Nbt::Long(1234567890123));
        data.set("Data", d);
        let mut raw = Vec::new();
        data.write_disk(&mut raw);
        let mut g = vec![0x1Fu8, 0x8B, 8, 0, 0, 0, 0, 0, 0, 255];
        g.extend_from_slice(&deflate_fixed(&raw));
        g.extend_from_slice(&crc32(&raw).to_le_bytes());
        g.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        std::fs::write(base.join("level.dat"), &g).unwrap();
        let st = AnvilStore::open(&base);
        assert_eq!(st.read_seed(), Some(1234567890123));
        assert!(!st.has_world_data(), "pas de .mca → monde vierge");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    /// Vecteur RÉEL généré par zlib Python (niveau 6, Huffman dynamique) :
    /// un level.dat gzip valide écrit par un OUTIL TIERS doit être lu.
    fn python_zlib6_gzip_level_dat_is_readable() {
    let raw: Vec<u8> = hex_to_bytes("1f8b08001603b06a02ffe36260e0626071492c496461e00a4acc4bc9cf0d4e4d4d616060942ffccd729a8381d327b52c35c72f31379581cf33af24b528bf202c312f332727918101001224bb133d000000");
    let gz = crate::anvil::gzip_decompress(&raw);
    assert!(gz.is_some(), "gzip_decompress a échoué sur le flux Python");
    let raw_nbt = gz.unwrap();
    let root = crate::nbt::NbtRdr::new(&raw_nbt).read_disk_root();
    assert!(root.is_some(), "NBT disque non parsé");
    let root = root.unwrap();
    let data = root.get("Data");
    assert!(data.is_some(), "Data absent: {:?}", root);
    let seed = data.unwrap().get("RandomSeed").and_then(|v| v.as_i64());
    assert_eq!(seed, Some(1234567890123), "graine: {:?}", seed);
}

fn hex_to_bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

    #[test]
    fn disk_names_cover_color_families() {
        assert_eq!(disk_name(world::WOOL_COLOR_BASE + 14), "red_wool");
        assert_eq!(disk_name(world::WOOL_SLAB_BASE + 14), "red_wool");
        assert_eq!(disk_name(world::CONCRETE_BASE + 3), "light_blue_concrete");
        assert_eq!(disk_name(world::CUSHION_BASE + 1), "orange_carpet");
        assert_eq!(disk_name(world::STONE), "stone");
        assert_eq!(disk_name(world::GRASS), "grass_block");
        // round-trip nom → id pour toute la table (rvx pour les ambiguïtés)
        let m = reverse_map();
        for id in 0u16..=223 {
            let n = disk_name(id);
            match m.get(n.as_str()) {
                Some(&first) if first == id => {}                       // résolu par le nom
                Some(_) => {
                    // ambigu : palette_entry doit porter rvx = id
                    let e = palette_entry(id);
                    let props = e.get("Properties").expect("rvx attendu");
                    assert!(matches!(
                        props.get("rvx"),
                        Some(Nbt::Str(s)) if *s == id.to_string()
                    ));
                }
                None => panic!("nom non indexé: {}", n),
            }
        }
    }
}
