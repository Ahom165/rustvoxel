// Texture pack support: minimal PNG decoder + DEFLATE inflate (RFC 1950/1951),
// a tiny PNG *encoder* (for exporting tiles) and the pack loader.
//
// Packs are plain folders of PNG files. Vanilla resource packs work as-is,
// either unpacked or as .zip archives (the official format, see the wiki:
//   name/pack.mcmeta + name/assets/<ns>/textures/block/<file>.png):
//   texturepacks/<pack>.zip  (or <pack>/ folder, or loose files)
//   texturepacks/<pack>/assets/minecraft/textures/block/<name>.png
// Loose overrides live in:  textures/<name>.png   (highest priority)
//
// 100% from scratch, zero crates.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ------------------------------------------------------------------ inflate
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,  // byte position
    bit: u32,    // bit position within current byte (0..8)
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> BitReader<'a> {
        BitReader { data, pos: 0, bit: 0 }
    }
    #[inline]
    fn read_bit(&mut self) -> Option<u32> {
        if self.pos >= self.data.len() {
            return None;
        }
        let b = (self.data[self.pos] >> self.bit) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.pos += 1;
        }
        Some(b as u32)
    }
    #[inline]
    fn read_bits(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for i in 0..n {
            v |= self.read_bit()? << i;
        }
        Some(v)
    }
    fn align_byte(&mut self) {
        if self.bit != 0 {
            self.bit = 0;
            self.pos += 1;
        }
    }
}

struct Huff {
    counts: [u16; 16],
    symbols: Vec<u16>,
}

impl Huff {
    fn new(lengths: &[u8]) -> Huff {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut offs = [0u16; 16];
        for i in 1..15 {
            offs[i + 1] = offs[i] + counts[i];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (sym, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offs[l as usize] as usize] = sym as u16;
                offs[l as usize] += 1;
            }
        }
        Huff { counts, symbols }
    }
    fn decode(&self, br: &mut BitReader) -> Option<u16> {
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0i32;
        for len in 1..=15 {
            code |= br.read_bit()? as i32;
            let count = self.counts[len] as i32;
            if code - first < count {
                return Some(self.symbols[(index + (code - first)) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        None
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13,
];

/// Raw DEFLATE stream (RFC 1951).
pub fn inflate(data: &[u8]) -> Option<Vec<u8>> {
    let mut br = BitReader::new(data);
    let mut out: Vec<u8> = Vec::with_capacity(data.len() * 4 + 64);
    loop {
        let bfinal = br.read_bit()?;
        let btype = br.read_bits(2)?;
        match btype {
            0 => {
                br.align_byte();
                if br.pos + 4 > br.data.len() {
                    return None;
                }
                let len = u16::from_le_bytes([br.data[br.pos], br.data[br.pos + 1]]) as usize;
                br.pos += 4; // LEN + NLEN
                if br.pos + len > br.data.len() {
                    return None;
                }
                out.extend_from_slice(&br.data[br.pos..br.pos + len]);
                br.pos += len;
            }
            1 | 2 => {
                let (lit, dist) = if btype == 1 {
                    let mut ll = [0u8; 288];
                    for (i, l) in ll.iter_mut().enumerate() {
                        *l = if i < 144 {
                            8
                        } else if i < 256 {
                            9
                        } else if i < 280 {
                            7
                        } else {
                            8
                        };
                    }
                    (Huff::new(&ll), Huff::new(&[5u8; 30]))
                } else {
                    let hlit = br.read_bits(5)? as usize + 257;
                    let hdist = br.read_bits(5)? as usize + 1;
                    let hclen = br.read_bits(4)? as usize + 4;
                    const ORDER: [usize; 19] = [
                        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                    ];
                    let mut cl = [0u8; 19];
                    for &o in ORDER.iter().take(hclen) {
                        cl[o] = br.read_bits(3)? as u8;
                    }
                    let clh = Huff::new(&cl);
                    let mut lengths = vec![0u8; hlit + hdist];
                    let mut i = 0usize;
                    while i < hlit + hdist {
                        let sym = clh.decode(&mut br)?;
                        match sym {
                            0..=15 => {
                                lengths[i] = sym as u8;
                                i += 1;
                            }
                            16 => {
                                if i == 0 {
                                    return None;
                                }
                                let prev = lengths[i - 1];
                                let rep = 3 + br.read_bits(2)? as usize;
                                for _ in 0..rep {
                                    if i >= lengths.len() {
                                        return None;
                                    }
                                    lengths[i] = prev;
                                    i += 1;
                                }
                            }
                            17 => {
                                let rep = 3 + br.read_bits(3)? as usize;
                                i = (i + rep).min(lengths.len());
                            }
                            18 => {
                                let rep = 11 + br.read_bits(7)? as usize;
                                i = (i + rep).min(lengths.len());
                            }
                            _ => return None,
                        }
                    }
                    (
                        Huff::new(&lengths[..hlit]),
                        Huff::new(&lengths[hlit..]),
                    )
                };
                loop {
                    let sym = lit.decode(&mut br)?;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else {
                        let li = (sym - 257) as usize;
                        if li >= 29 {
                            return None;
                        }
                        let len = LEN_BASE[li] as usize + br.read_bits(LEN_EXTRA[li] as u32)? as usize;
                        let ds = dist.decode(&mut br)? as usize;
                        if ds >= 30 {
                            return None;
                        }
                        let d = DIST_BASE[ds] as usize + br.read_bits(DIST_EXTRA[ds] as u32)? as usize;
                        if d > out.len() {
                            return None;
                        }
                        let start = out.len() - d;
                        for k in 0..len {
                            let b = out[start + k];
                            out.push(b);
                        }
                    }
                }
            }
            _ => return None,
        }
        if bfinal == 1 {
            break;
        }
    }
    Some(out)
}

// ------------------------------------------------------------------- PNG in
#[derive(Clone)]
pub struct Rgba {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>, // rgba8
}

impl Rgba {
    /// All pixels with alpha > 0 have r == g == b (within tol) -> grayscale.
    pub fn is_grayscale(&self, tol: u8) -> bool {
        for p in self.px.chunks_exact(4) {
            if p[3] > 0 {
                let d1 = (p[0] as i32 - p[1] as i32).abs();
                let d2 = (p[1] as i32 - p[2] as i32).abs();
                if d1 > tol as i32 || d2 > tol as i32 {
                    return false;
                }
            }
        }
        true
    }

    /// Nearest-neighbour resample to w x h. Animation strips (height = n*width)
    /// are cropped to their first frame first.
    pub fn to_tile(&self, size: u32) -> Rgba {
        // crop first frame of vertical strips
        let (w, h, px) = if self.h > self.w && self.h % self.w == 0 {
            (self.w, self.w, self.px[..(self.w * self.w * 4) as usize].to_vec())
        } else {
            (self.w, self.h, self.px.clone())
        };
        let mut out = vec![0u8; (size * size * 4) as usize];
        for y in 0..size {
            let sy = (y as u64 * h as u64 / size as u64) as u32;
            for x in 0..size {
                let sx = (x as u64 * w as u64 / size as u64) as u32;
                let s = ((sy * w + sx) * 4) as usize;
                let d = ((y * size + x) * 4) as usize;
                if s + 3 < px.len() {
                    out[d..d + 4].copy_from_slice(&px[s..s + 4]);
                }
            }
        }
        Rgba { w: size, h: size, px: out }
    }
}

/// Decode a PNG (bit depths 1/2/4/8 - vanilla palette textures are usually
/// 4-bit - color types 0/2/3/4/6, non-interlaced).
pub fn png_decode(data: &[u8]) -> Option<Rgba> {
    const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if data.len() < 8 || data[..8] != SIG {
        return None;
    }
    let mut pos = 8usize;
    let mut w = 0u32;
    let mut h = 0u32;
    let mut depth = 0u8;
    let mut ctype = 0u8;
    let mut interlace = 0u8;
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat: Vec<u8> = Vec::new();
    while pos + 8 <= data.len() {
        let len = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?) as usize;
        let tag = &data[pos + 4..pos + 8];
        let body_start = pos + 8;
        if body_start + len + 4 > data.len() {
            return None;
        }
        let body = &data[body_start..body_start + len];
        match tag {
            b"IHDR" => {
                if body.len() < 13 {
                    return None;
                }
                w = u32::from_be_bytes(body[0..4].try_into().ok()?);
                h = u32::from_be_bytes(body[4..8].try_into().ok()?);
                depth = body[8];
                ctype = body[9];
                interlace = body[12];
            }
            b"PLTE" => {
                palette = body.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
            }
            b"tRNS" => trns = body.to_vec(),
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        pos = body_start + len + 4;
    }
    if w == 0 || h == 0 || w > 1024 || h > 8192 || interlace != 0 {
        return None;
    }
    // depth 16 (HD packs) is not supported; 1/2/4/8 covers every vanilla asset
    if !matches!(depth, 1 | 2 | 4 | 8) {
        return None;
    }
    let channels: u32 = match ctype {
        0 => 1,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        _ => return None,
    };
    // zlib wrapper: 2-byte header, deflate data, adler32
    if idat.len() < 6 {
        return None;
    }
    let raw = inflate(&idat[2..idat.len() - 4])?;
    // sub-byte depths (1/2/4) pack several samples per byte (MSB first)
    let stride = ((w * channels * depth as u32 + 7) / 8) as usize;
    let expected = (stride + 1) * h as usize;
    if raw.len() < expected {
        return None;
    }
    // unfilter
    let mut img = vec![0u8; stride * h as usize];
    let bpp = if depth == 8 { (channels as usize).max(1) } else { 1 };
    for y in 0..h as usize {
        let filter = raw[y * (stride + 1)];
        let row_off = y * stride;
        let prev_off = if y > 0 { (y - 1) * stride } else { usize::MAX };
        for x in 0..stride {
            let srcv = raw[y * (stride + 1) + 1 + x] as i32;
            let a = if x >= bpp { img[row_off + x - bpp] as i32 } else { 0 };
            let b = if prev_off != usize::MAX { img[prev_off + x] as i32 } else { 0 };
            let c = if x >= bpp && prev_off != usize::MAX {
                img[prev_off + x - bpp] as i32
            } else {
                0
            };
            let v = match filter {
                0 => srcv,
                1 => srcv + a,
                2 => srcv + b,
                3 => srcv + (a + b) / 2,
                4 => {
                    let p = a + b - c;
                    let pa = (p - a).abs();
                    let pb = (p - b).abs();
                    let pc = (p - c).abs();
                    srcv + if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c }
                }
                _ => return None,
            };
            img[row_off + x] = (v & 0xFF) as u8;
        }
    }
    // expand packed samples to one byte each before the rgba conversion
    let stride = if depth == 8 {
        (w * channels) as usize
    } else {
        let ns = (w * channels) as usize;
        let mut up = vec![0u8; ns * h as usize];
        let maxv = ((1u16 << depth) - 1) as u8;
        for y in 0..h as usize {
            for x in 0..ns {
                let bit = x * depth as usize;
                let byte = img[y * stride + bit / 8];
                let sh = 8 - depth as usize - (bit % 8);
                let v = (byte >> sh) & maxv;
                // grayscale stores levels (not palette indices): scale up
                up[y * ns + x] = if ctype == 0 {
                    (v as u32 * 255 / maxv as u32) as u8
                } else {
                    v
                };
            }
        }
        img = up;
        ns
    };
    // to rgba
    let mut out = vec![255u8; (w * h * 4) as usize];
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = y * stride + x * channels as usize;
            let d = ((y as u32 * w + x as u32) * 4) as usize;
            match ctype {
                0 => {
                    let g = img[i];
                    out[d] = g;
                    out[d + 1] = g;
                    out[d + 2] = g;
                    if let Some(t) = trns.get(0) {
                        if g == *t {
                            out[d + 3] = 0;
                        }
                    }
                }
                2 => {
                    out[d..d + 3].copy_from_slice(&img[i..i + 3]);
                }
                3 => {
                    let pi = img[i] as usize;
                    if pi >= palette.len() {
                        return None;
                    }
                    out[d..d + 3].copy_from_slice(&palette[pi]);
                    out[d + 3] = trns.get(pi).copied().unwrap_or(255);
                }
                4 => {
                    let g = img[i];
                    out[d] = g;
                    out[d + 1] = g;
                    out[d + 2] = g;
                    out[d + 3] = img[i + 1];
                }
                _ => {
                    out[d..d + 4].copy_from_slice(&img[i..i + 4]);
                }
            }
        }
    }
    Some(Rgba { w, h, px: out })
}

// ------------------------------------------------------------------ PNG out
fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(body);
    let crc = crc_data(tag, body);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc_data(tag: &[u8; 4], body: &[u8]) -> u32 {
    let mut v = Vec::with_capacity(4 + body.len());
    v.extend_from_slice(tag);
    v.extend_from_slice(body);
    crc32(&v)
}

/// Encode RGBA pixels as a PNG (filter 0 rows, zlib "stored" blocks).
pub fn png_encode(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * h * 4 + h) as usize);
    for y in 0..h {
        raw.push(0u8); // filter none
        raw.extend_from_slice(&rgba[(y * w * 4) as usize..((y + 1) * w * 4) as usize]);
    }
    // zlib header + stored deflate blocks
    let mut z = vec![0x78u8, 0x01u8];
    let mut off = 0usize;
    while off < raw.len() {
        let n = (raw.len() - off).min(65535);
        let last = if off + n >= raw.len() { 1u8 } else { 0u8 };
        z.push(last);
        z.extend_from_slice(&(n as u16).to_le_bytes());
        z.extend_from_slice(&(!(n as u16)).to_le_bytes());
        z.extend_from_slice(&raw[off..off + n]);
        off += n;
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

// --------------------------------------------------------------------- zip
#[inline]
fn u16le(d: &[u8], o: usize) -> u32 {
    d[o] as u32 | ((d[o + 1] as u32) << 8)
}
#[inline]
fn u32le(d: &[u8], o: usize) -> u32 {
    d[o] as u32
        | ((d[o + 1] as u32) << 8)
        | ((d[o + 2] as u32) << 16)
        | ((d[o + 3] as u32) << 24)
}

/// Minimal ZIP reader good enough for resource packs: walks the central
/// directory, returns (name, bytes) for every entry. Entries are stored
/// (method 0) or deflated (method 8, inflated with our own inflate).
fn zip_entries(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    if data.len() < 22 {
        return out;
    }
    // End Of Central Directory: scan the last 64 KiB for its signature
    let mut eocd = None;
    let start = data.len().saturating_sub(66_000);
    let mut i = data.len() - 22;
    loop {
        if data[i] == 0x50
            && data[i + 1] == 0x4b
            && data[i + 2] == 0x05
            && data[i + 3] == 0x06
        {
            eocd = Some(i);
            break;
        }
        if i == start {
            break;
        }
        i -= 1;
    }
    let eocd = match eocd {
        Some(e) => e,
        None => return out,
    };
    let n_entries = u16le(data, eocd + 10) as usize;
    let cd_off = u32le(data, eocd + 16) as usize;
    let cd_size = u32le(data, eocd + 12) as usize;
    if cd_off + cd_size > data.len() {
        return out;
    }
    let mut p = cd_off;
    for _ in 0..n_entries {
        if p + 46 > data.len() || u32le(data, p) != 0x0201_4b50 {
            break;
        }
        let method = u16le(data, p + 10) as usize;
        let csize = u32le(data, p + 20) as usize;
        let name_len = u16le(data, p + 28) as usize;
        let extra_len = u16le(data, p + 30) as usize;
        let comment_len = u16le(data, p + 32) as usize;
        let lho = u32le(data, p + 42) as usize;
        let name = String::from_utf8_lossy(&data[p + 46..p + 46 + name_len]).into_owned();
        p += 46 + name_len + extra_len + comment_len;
        // read the local header to find where the data starts
        if lho + 30 > data.len() || u32le(data, lho) != 0x0403_4b50 {
            continue;
        }
        let lname = u16le(data, lho + 26) as usize;
        let lextra = u16le(data, lho + 28) as usize;
        let dstart = lho + 30 + lname + lextra;
        if dstart + csize > data.len() {
            continue;
        }
        let raw = &data[dstart..dstart + csize];
        let bytes = match method {
            0 => raw.to_vec(),
            8 => match inflate(raw) {
                Some(b) => b,
                None => continue,
            },
            _ => continue,
        };
        out.push((name, bytes));
    }
    out
}

// ------------------------------------------------------------------ the pack
/// All PNGs from textures/ + texturepacks/*/, indexed by lowercase file stem.
pub struct Pack {
    decoded: HashMap<String, Rgba>,
    names: Vec<String>,
    /// vanilla colormap files (assets/.../colormap/{grass,foliage}.png)
    pub colormap_grass: Option<Rgba>,
    pub colormap_foliage: Option<Rgba>,
    /// fichiers bruts utiles hors PNG : blockstates/*.json, models/**.json
    /// (clé = chemin normalisé après assets/<namespace>/, minuscule)
    pub raw: HashMap<String, Vec<u8>>,
}

/// Normalise "assets/<ns>/blockstates/x.json" (zip) ou un chemin disque en clé
/// "blockstates/x.json". Renvoie None si pas un asset d'intérêt.
fn norm_asset_path(path: &str) -> Option<String> {
    let lower = path.replace('\\', "/").to_lowercase();
    let rest = if lower.starts_with("assets/") {
        &lower[..]
    } else {
        let idx = lower.find("/assets/")?;
        &lower[idx + 1..]
    };
    let mut it = rest.splitn(3, '/');
    let _assets = it.next()?;
    let _ns = it.next()?;
    let rem = it.next()?;
    if rem.starts_with("blockstates/") || rem.starts_with("models/") {
        Some(rem.to_string())
    } else {
        None
    }
}

/// Stems that are pack metadata, never block textures.
fn is_meta_stem(stem: &str) -> bool {
    matches!(stem, "pack" | "pack_icon" | "pack.mcmeta")
}

impl Pack {
    pub fn discover(base: &Path) -> Pack {
        let mut dirs: Vec<PathBuf> = Vec::new();
        dirs.push(base.join("textures"));
        let tp = base.join("texturepacks");
        if let Ok(rd) = std::fs::read_dir(&tp) {
            let mut subs: Vec<PathBuf> = rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            subs.sort();
            dirs.extend(subs);
        }
        let mut pack = Pack {
            decoded: HashMap::new(),
            names: Vec::new(),
            colormap_grass: None,
            colormap_foliage: None,
            raw: HashMap::new(),
        };
        for d in dirs {
            pack.scan_dir(&d, 0);
        }
        // vanilla packs are usually distributed as .zip archives
        if let Ok(rd) = std::fs::read_dir(&tp) {
            let mut zips: Vec<PathBuf> = rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .map(|x| x.eq_ignore_ascii_case("zip"))
                        .unwrap_or(false)
                })
                .collect();
            zips.sort();
            for z in zips {
                pack.scan_zip(&z);
            }
        }
        pack
    }

    /// Colormap PNGs keep their full resolution (they are lookup tables,
    /// never resampled to a 16x16 tile).
    fn insert_colormap(&mut self, stem: &str, bytes: &[u8]) {
        let img = match png_decode(bytes) {
            Some(i) => i,
            None => return,
        };
        match stem {
            "grass" => {
                if self.colormap_grass.is_none() {
                    self.colormap_grass = Some(img);
                }
            }
            "foliage" => {
                if self.colormap_foliage.is_none() {
                    self.colormap_foliage = Some(img);
                }
            }
            _ => {}
        }
    }

    fn insert_png(&mut self, key: String, bytes: &[u8]) {
        if is_meta_stem(&key) || self.decoded.contains_key(&key) {
            return;
        }
        if let Some(img) = png_decode(bytes) {
            self.decoded.insert(key.clone(), img);
            self.names.push(key);
        }
    }

    fn scan_zip(&mut self, path: &Path) {
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(_) => return,
        };
        for (name, bytes) in zip_entries(&data) {
            // only .png entries (case-insensitive), skip directories
            let lower = name.to_lowercase();
            if lower.ends_with(".png.mcmeta") {
                continue;
            }
            if !lower.ends_with(".png") {
                // JSON d'intérêt : blockstates + models
                if lower.ends_with(".json") {
                    if let Some(key) = norm_asset_path(&name) {
                        self.raw.insert(key, bytes);
                    }
                }
                continue;
            }
            let stem = match name.rsplit('/').next() {
                Some(s) if !s.is_empty() => s.trim_end_matches(".png").to_lowercase(),
                _ => continue,
            };
            if lower.contains("/colormap/") {
                self.insert_colormap(&stem, &bytes);
                continue;
            }
            self.insert_png(stem, &bytes);
        }
    }

    fn scan_dir(&mut self, dir: &Path, depth: u32) {
        if depth > 6 {
            return;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for e in entries.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                self.scan_dir(&p, depth + 1);
            } else if p.extension().map(|x| x == "json").unwrap_or(false) {
                if let Some(key) = norm_asset_path(&p.to_string_lossy()) {
                    if let Ok(bytes) = std::fs::read(&p) {
                        self.raw.insert(key, bytes);
                    }
                }
            } else if p.extension().map(|x| x == "png").unwrap_or(false) {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    let key = stem.to_lowercase();
                    let in_colormap = p
                        .to_string_lossy()
                        .to_lowercase()
                        .contains("colormap");
                    if in_colormap {
                        if let Ok(bytes) = std::fs::read(&p) {
                            self.insert_colormap(&key, &bytes);
                        }
                        continue;
                    }
                    if !self.decoded.contains_key(&key) {
                        if let Ok(bytes) = std::fs::read(&p) {
                            self.insert_png(key, &bytes);
                        }
                    }
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.decoded.len()
    }

    /// Fichier brut du pack (blockstates/models), clé normalisée.
    pub fn get_raw(&self, key: &str) -> Option<&[u8]> {
        self.raw.get(&key.to_lowercase()).map(|v| v.as_slice())
    }

    /// Toutes les clés raw sous un préfixe (ex: "blockstates/").
    pub fn raw_keys(&self, prefix: &str) -> Vec<String> {
        let mut v: Vec<String> = self
            .raw
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        v.sort();
        v
    }

    /// Image à PLEINE résolution (textures d'entités 64x64, colormaps...).
    pub fn get_full(&self, names: &[&str]) -> Option<Rgba> {
        for n in names {
            if let Some(img) = self.decoded.get(&n.to_lowercase()) {
                return Some(img.clone());
            }
        }
        None
    }

    /// First matching candidate (lowercase stem), resampled to a size x size tile.
    pub fn get(&self, names: &[&str]) -> Option<Rgba> {
        for n in names {
            if let Some(img) = self.decoded.get(&n.to_lowercase()) {
                return Some(img.to_tile(16));
            }
        }
        None
    }

    pub fn loaded_names(&self) -> &[String] {
        &self.names
    }
}

// ------------------------------------------------- tile name map + tints
/// Vanilla resource-pack names tried for each atlas tile (in order), plus
/// our own names so custom packs can override anything.
pub fn tile_candidates(t: u32) -> Vec<&'static str> {
    use crate::world::*;
    match t {
        T_GRASS_TOP => vec!["grass_block_top", "grass_top"],
        T_GRASS_SIDE => vec!["grass_block_side", "grass_side"],
        T_GRASS_SIDE_OVERLAY => vec!["grass_block_side_overlay", "grass_side_overlay"],
        T_DIRT => vec!["dirt"],
        T_STONE => vec!["stone"],
        T_COBBLE => vec!["cobblestone"],
        T_SAND => vec!["sand"],
        T_LOG_SIDE => vec!["oak_log"],
        T_LOG_TOP => vec!["oak_log_top"],
        T_LEAVES => vec!["oak_leaves"],
        T_PLANKS => vec!["oak_planks"],
        T_WATER => vec!["water_still", "water"],
        T_GLASS => vec!["glass"],
        T_BRICK => vec!["bricks", "brick"],
        T_SNOW => vec!["snow"],
        T_BEDROCK => vec!["bedrock"],
        T_COAL => vec!["coal_ore"],
        T_IRON => vec!["iron_ore"],
        T_GRAVEL => vec!["gravel"],
        T_SANDSTONE => vec!["sandstone"],
        T_BIRCH_SIDE => vec!["birch_log"],
        T_BIRCH_TOP => vec!["birch_log_top"],
        T_BIRCH_LEAVES => vec!["birch_leaves"],
        T_SPRUCE_SIDE => vec!["spruce_log"],
        T_SPRUCE_TOP => vec!["spruce_log_top"],
        T_SPRUCE_LEAVES => vec!["spruce_leaves"],
        T_CACTUS_SIDE => vec!["cactus_side"],
        T_CACTUS_TOP => vec!["cactus_top"],
        T_TALLGRASS => vec!["short_grass", "grass"],
        T_FLOWER_RED => vec!["poppy", "flower_red"],
        T_FLOWER_YELLOW => vec!["dandelion", "flower_yellow"],
        T_MUSHROOM => vec!["red_mushroom"],
        T_DEADBUSH => vec!["dead_bush"],
        T_PUMPKIN_SIDE => vec!["pumpkin_side", "carved_pumpkin"],
        T_PUMPKIN_TOP => vec!["pumpkin_top"],
        T_GLOWSTONE => vec!["glowstone"],
        T_WOOL => vec!["white_wool", "wool_colored_white"],
        T_MEAT => vec!["cooked_beef", "meat"],
        T_POPLAR_SIDE => vec!["poplar_log"],
        T_POPLAR_TOP => vec!["poplar_log_top"],
        T_POP_LEAVES_R => vec!["poplar_leaves_red"],
        T_POP_LEAVES_O => vec!["poplar_leaves_orange"],
        T_POP_LEAVES_Y => vec!["poplar_leaves_yellow"],
        T_POPLAR_PLANKS => vec!["poplar_planks"],
        T_RED_SHRUB => vec!["red_shrub"],
        T_SHELF_SM => vec!["shelf_mushroom"],
        T_SHELF_LG => vec!["shelf_mushroom_large"],
        T_HAY_SIDE => vec!["hay_block_side"],
        T_HAY_TOP => vec!["hay_block_top"],
        T_BED_TOP => vec!["straw_bed_top"],
        T_BED_SIDE => vec!["straw_bed_side"],
        T_CAMPFIRE => vec!["campfire_log", "campfire", "campfire_fire"],
        T_BARREL_SIDE => vec!["barrel_side"],
        T_BARREL_TOP => vec!["barrel_top"],
        T_PALE_GRASS_TOP => vec!["pale_grass_top"],
        T_PALE_GRASS_SIDE => vec!["pale_grass_side"],
        t if (T_WOOL_COLORS..T_WOOL_COLORS + 15).contains(&t) => {
            vec![WOOL_NAMES[(t - T_WOOL_COLORS + 1) as usize]]
        }
        t if (T_CONCRETE..T_CONCRETE + 16).contains(&t) => {
            vec![CONCRETE_NAMES[(t - T_CONCRETE) as usize]]
        }
        T_CHICKEN_SKIN => vec!["chicken_skin"],
        T_CHICKEN_FACE => vec!["chicken_face"],
        T_COW_SKIN => vec!["cow_skin"],
        T_COW_FACE => vec!["cow_face"],
        T_FOX_SKIN => vec!["fox_skin"],
        T_FOX_FACE => vec!["fox_face"],
        // --- v0.4 "Legacy Update" ---
        T_RED_SAND => vec!["red_sand"],
        T_RED_SANDSTONE => vec!["red_sandstone"],
        T_PODZOL_TOP => vec!["podzol_top"],
        T_PODZOL_SIDE => vec!["podzol_side"],
        T_COARSE_DIRT => vec!["coarse_dirt"],
        T_PACKED_ICE => vec!["packed_ice"],
        T_GRANITE => vec!["granite"],
        T_DIORITE => vec!["diorite"],
        T_ANDESITE => vec!["andesite"],
        T_SPONGE => vec!["sponge"],
        T_MAGMA => vec!["magma"],
        T_SEAGRASS => vec!["seagrass", "tall_seagrass_top"],
        T_KELP => vec!["kelp", "kelp_plant"],
        T_CORAL_PINK => vec!["tube_coral", "horn_coral", "coral_pink"],
        T_CORAL_BLUE => vec!["bubble_coral", "brain_coral", "coral_blue"],
        T_CORAL_DEAD => vec!["dead_tube_coral", "dead_bubble_coral", "coral_dead"],
        T_LILYPAD => vec!["lily_pad"],
        T_SUGARCANE => vec!["sugar_cane"],
        T_BERRY_BUSH => vec![
            "sweet_berry_bush_stage3",
            "sweet_berry_bush_stage2",
            "sweet_berry_bush",
        ],
        T_BAMBOO => vec!["bamboo_stalk", "bamboo"],
        T_BAMBOO_SIDE => vec!["bamboo_block"],
        T_BAMBOO_TOP => vec!["bamboo_block_top"],
        T_HONEY => vec!["honey_block_top", "honey_block_side", "honey"],
        T_BEEHIVE_SIDE => vec!["beehive_side"],
        T_BEEHIVE_TOP => vec!["beehive_end", "beehive_top"],
        T_BASALT_SIDE => vec!["basalt_side"],
        T_BASALT_TOP => vec!["basalt_top"],
        T_BLACKSTONE => vec!["blackstone"],
        T_COPPER_ORE => vec!["copper_ore"],
        T_COPPER_BLOCK => vec!["copper_block", "cut_copper"],
        T_COPPER_BULB => vec!["copper_bulb_lit", "copper_bulb"],
        T_AMETHYST => vec!["amethyst_block"],
        T_CALCITE => vec!["calcite"],
        T_TUFF => vec!["tuff"],
        T_DEEPSLATE => vec!["deepslate", "deepslate_side"],
        T_DRIPSTONE => vec!["dripstone_block"],
        T_MOSS => vec!["moss_block"],
        T_AZALEA => vec!["azalea_plant", "azalea_side"],
        T_AZALEA_FLOWER => vec!["flowering_azalea_plant", "flowering_azalea_side"],
        T_GLOW_BERRIES => vec!["cave_vines_plant", "cave_vines"],
        T_MUD => vec!["mud"],
        T_PACKED_MUD => vec!["packed_mud"],
        T_MUD_BRICKS => vec!["mud_bricks"],
        T_SCULK => vec!["sculk"],
        T_MANGROVE_SIDE => vec!["mangrove_log"],
        T_MANGROVE_TOP => vec!["mangrove_log_top"],
        T_MANGROVE_LEAVES => vec!["mangrove_leaves"],
        T_MANGROVE_PLANKS => vec!["mangrove_planks"],
        T_CHERRY_SIDE => vec!["cherry_log"],
        T_CHERRY_TOP => vec!["cherry_log_top"],
        T_CHERRY_LEAVES => vec!["cherry_leaves"],
        T_CHERRY_PLANKS => vec!["cherry_planks"],
        T_PINK_PETALS => vec!["pink_petals"],
        T_ACACIA_SIDE => vec!["acacia_log"],
        T_ACACIA_TOP => vec!["acacia_log_top"],
        T_ACACIA_LEAVES => vec!["acacia_leaves"],
        T_ACACIA_PLANKS => vec!["acacia_planks"],
        T_MELON_SIDE => vec!["melon_side"],
        T_MELON_TOP => vec!["melon_top"],
        T_TERRACOTTA => vec!["terracotta"],
        T_TERRACOTTA_RED => vec!["red_terracotta"],
        T_TERRACOTTA_ORANGE => vec!["orange_terracotta"],
        T_TERRACOTTA_YELLOW => vec!["yellow_terracotta"],
        T_GOLD_ORE => vec!["gold_ore"],
        T_DIAMOND_ORE => vec!["diamond_ore"],
        T_OBSIDIAN => vec!["obsidian"],
        T_CRYING_OBSIDIAN => vec!["crying_obsidian"],
        T_TORCH => vec!["torch"],
        T_BERRY_ITEM => vec!["sweet_berries", "berries"],
        // our own mob skins (custom names so packs can override them too)
        T_SLIME => vec!["slime"],
        T_SHADOW_SKIN => vec!["shadow_skin"],
        T_SHADOW_FACE => vec!["shadow_face"],
        T_BEE_SKIN => vec!["bee_skin"],
        T_BEE_FACE => vec!["bee_face"],
        T_PARROT_SKIN => vec!["parrot_skin"],
        T_PARROT_FACE => vec!["parrot_face"],
        T_TURTLE_SKIN => vec!["turtle_skin"],
        T_TURTLE_FACE => vec!["turtle_face"],
        T_DOLPHIN_SKIN => vec!["dolphin_skin"],
        T_DOLPHIN_FACE => vec!["dolphin_face"],
        T_GOAT_SKIN => vec!["goat_skin"],
        T_GOAT_FACE => vec!["goat_face"],
        T_FROG_SKIN => vec!["frog_skin"],
        T_FROG_FACE => vec!["frog_face"],
        T_AXOLOTL_SKIN => vec!["axolotl_skin"],
        T_AXOLOTL_FACE => vec!["axolotl_face"],
        // --- v0.5 "Caves & Cliffs" ---
        T_DRIPSTONE_SPIKE => vec!["pointed_dripstone", "dripstone_spike"],
        T_GLOW_SQUID_SKIN => vec!["glow_squid_skin"],
        T_GLOW_SQUID_FACE => vec!["glow_squid_face"],
        _ => vec![],
    }
}

/// Dye-color order used by vanilla (white..black), index 0 = white.
pub const COLOR_NAMES: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray",
    "cyan", "purple", "blue", "brown", "green", "red", "black",
];
const WOOL_NAMES: [&str; 16] = [
    "white_wool", "orange_wool", "magenta_wool", "light_blue_wool", "yellow_wool", "lime_wool",
    "pink_wool", "gray_wool", "light_gray_wool", "cyan_wool", "purple_wool", "blue_wool",
    "brown_wool", "green_wool", "red_wool", "black_wool",
];
const CONCRETE_NAMES: [&str; 16] = [
    "white_concrete", "orange_concrete", "magenta_concrete", "light_blue_concrete",
    "yellow_concrete", "lime_concrete", "pink_concrete", "gray_concrete", "light_gray_concrete",
    "cyan_concrete", "purple_concrete", "blue_concrete", "brown_concrete", "green_concrete",
    "red_concrete", "black_concrete",
];

/// Biome-dependent tints (grass, foliage, water) are NO LONGER baked into
/// the atlas: the mesher multiplies grayscale textures by per-vertex biome
/// colors (the reference game's colormap system). Only the fixed poplar
/// leaf colors remain here for grayscale pack textures.
pub fn tile_tint(t: u32) -> [f32; 3] {
    use crate::world::*;
    match t {
        T_POP_LEAVES_R => [0.82, 0.20, 0.12],
        T_POP_LEAVES_O => [0.90, 0.45, 0.10],
        T_POP_LEAVES_Y => [0.92, 0.72, 0.14],
        _ => [1.0, 1.0, 1.0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::tiles_of;

    /// Decode every PNG in scripts/pngtest/ (generated externally by Python
    /// with zlib level 9 -> dynamic Huffman, all filters, palette/gray/rgb).
    /// Run: cargo test --features fficheck external_pngs -- --ignored
    #[test]
    #[ignore]
    fn external_pngs() {
        let dir = std::path::Path::new("../../scripts/pngtest");
        let mut n = 0;
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                if p.extension().map(|x| x == "png").unwrap_or(false) {
                    let bytes = std::fs::read(&p).unwrap();
                    let img = png_decode(&bytes)
                        .unwrap_or_else(|| panic!("failed to decode {:?}", p));
                    assert!(img.w > 0 && img.h > 0, "{:?} bad size", p);
                    n += 1;
                }
            }
        }
        assert!(n >= 5, "expected at least 5 external test PNGs, got {n}");

        // value-level check: strip16x64.png has all 5 filter types and every
        // pixel of frame 0 is exactly (30, 90, 200, 180)
        let bytes = std::fs::read(dir.join("strip16x64.png")).unwrap();
        let img = png_decode(&bytes).unwrap();
        assert_eq!((img.w, img.h), (16, 64));
        let f0 = img.to_tile(16);
        for p in f0.px.chunks_exact(4) {
            assert_eq!(p, [30, 90, 200, 180], "frame 0 pixel wrong: unfilter bug");
        }
        // rgba16.png pixel checks (deterministic generator)
        let bytes = std::fs::read(dir.join("rgba16.png")).unwrap();
        let img = png_decode(&bytes).unwrap();
        let at = |x: u32, y: u32| {
            let o = ((y * 16 + x) * 4) as usize;
            [img.px[o], img.px[o + 1], img.px[o + 2], img.px[o + 3]]
        };
        assert_eq!(at(15, 15), [240, 240, 225, 0]);
        assert_eq!(at(3, 5), [48, 80, 15, 255]);
        assert_eq!(at(0, 0), [0, 0, 0, 0]);

        // sub-byte depths (the vanilla "default" pack ships ~400 palette
        // 4-bit PNGs): palette indices are NOT scaled, gray levels ARE
        if let Ok(bytes) = std::fs::read(dir.join("palette4bit.png")) {
            let img = png_decode(&bytes).expect("4-bit palette must decode");
            let at = |x: u32, y: u32| {
                let o = ((y * 16 + x) * 4) as usize;
                [img.px[o], img.px[o + 1], img.px[o + 2], img.px[o + 3]]
            };
            // idx (x+y)%16, entry i = (i*17, 255-i*17, (i*37)%256), trns [0,255,128,..]
            assert_eq!(at(0, 0), [0, 255, 0, 0]); // idx 0 -> alpha 0
            assert_eq!(at(1, 0), [17, 238, 37, 255]); // idx 1
            assert_eq!(at(0, 1), [17, 238, 37, 255]); // idx 1
            assert_eq!(at(15, 15), [238, 17, 6, 255]); // idx (30%16)=14
        }
        if let Ok(bytes) = std::fs::read(dir.join("palette2bit.png")) {
            let img = png_decode(&bytes).expect("2-bit palette must decode");
            let o = ((1 * 16 + 1) * 4) as usize; // (1,1): (x*y)%4 = 1 -> 85
            assert_eq!(&img.px[o..o + 3], &[85, 85, 85]);
        }
        if let Ok(bytes) = std::fs::read(dir.join("gray1bit.png")) {
            let img = png_decode(&bytes).expect("1-bit gray must decode");
            // levels are scaled: 0 -> 0, 1 -> 255
            assert_eq!(img.px[0], 0);
            assert_eq!(img.px[4], 255);
            assert!(img.is_grayscale(0));
        }
    }

    /// Decode a real vanilla-style .zip pack generated externally by Python
    /// (zipfile with zlib level 9 = dynamic Huffman). Verifies the zip reader
    /// against genuinely compressed data. Run:
    /// cargo test --features fficheck external_zip -- --ignored
    #[test]
    #[ignore]
    fn external_vanilla_zip_pack() {
        let z = std::fs::read("../../scripts/pngtest/vanillapack.zip")
            .expect("run scripts/gen_png_tests.py first");
        let ents = zip_entries(&z);
        assert_eq!(ents.len(), 5, "all entries must be read: {}", ents.len());
        let mut by: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
        for (n, d) in ents {
            by.insert(n, d);
        }
        let mc = by.get("pack.mcmeta").expect("pack.mcmeta");
        assert!(mc.starts_with(b"{\"pack\":"));
        let dirt = by.get("assets/minecraft/textures/block/dirt.png").expect("dirt");
        let img = png_decode(dirt).expect("dirt decodes");
        assert_eq!(img.px[0], 134);
        // HD 32x32 grass side resamples down to a 16x16 tile
        let side = by.get("assets/minecraft/textures/block/grass_block_side.png").expect("side");
        let img = png_decode(side).expect("side decodes");
        assert_eq!((img.w, img.h), (32, 32));
        let tile = img.to_tile(16);
        assert_eq!(tile.px[0], 90);
        // grayscale grass + tint via atlas builder
        let grass = by.get("assets/minecraft/textures/block/grass_block_top.png").expect("grass");
        let img = png_decode(grass).expect("grass decodes");
        assert!(img.is_grayscale(6), "external grass must be grayscale");
        let tile = img.to_tile(16);
        assert!(tile.px[0] == tile.px[1] && tile.px[1] == tile.px[2]);
        // water strip frame 0
        let water = by.get("assets/minecraft/textures/block/water_still.png").expect("water");
        let img = png_decode(water).expect("water decodes");
        assert_eq!((img.w, img.h), (16, 512));
        let f0 = img.to_tile(16);
        assert_eq!((f0.px[0], f0.px[1], f0.px[2]), (40, 90, 210));
    }

    /// THE REAL THING: the actual vanilla "Default" resource pack for
    /// 1.20.5+ (11k entries, ~2.6k PNGs incl. ~400 four-bit palette files,
    /// 16x512 animation strips, 256x256 colormaps). Exercises the whole
    /// chain on genuine Mojang-format data: zip central directory, sub-byte
    /// palette PNGs, strips, colormap pickup and atlas tile coverage.
    /// Run:
    ///   cargo test --release --features fficheck real_vanilla_default_pack -- --ignored --nocapture
    #[test]
    #[ignore]
    #[cfg(any(windows, feature = "fficheck"))]
    fn real_vanilla_default_pack() {
        let zip = std::path::Path::new("texturepacks/texture-pack-default1.20.5-26.2.zip");
        if !zip.exists() {
            eprintln!("pack vanilla absent (texturepacks/), test sauté");
            return;
        }
        let t0 = std::time::Instant::now();
        let pack = Pack::discover(std::path::Path::new("."));
        eprintln!(
            "Pack::discover: {:?} - {} textures décodées depuis le zip",
            t0.elapsed(),
            pack.len()
        );
        assert!(pack.len() > 2000, "trop peu de textures: {}", pack.len());
        assert!(
            pack.colormap_grass.is_some() && pack.colormap_foliage.is_some(),
            "les colormaps grass/foliage du pack doivent charger"
        );

        // 4-bit palette textures of the vanilla pack must decode now
        for name in [
            "water_still",
            "oak_leaves",
            "birch_leaves",
            "acacia_leaves",
            "mangrove_leaves",
            "sugar_cane",
            "seagrass",
            "kelp",
            "torch",
            "grass_block_top",
        ] {
            let img = pack
                .get(&[name])
                .unwrap_or_else(|| panic!("{name} doit se décoder depuis le pack vanilla"));
            eprintln!(
                "  {name}: gray={:?} px0={:?}",
                img.is_grayscale(6),
                &img.px[..4]
            );
        }
        let water = pack.get(&["water_still"]).unwrap();
        assert!(water.is_grayscale(6), "water_still vanilla = grayscale (teinté par colormap)");

        // atlas coverage: how many of our named tiles get a pack texture?
        let mut matched = 0usize;
        let mut missing: Vec<&'static str> = Vec::new();
        for t in 0..256u32 {
            let cands = tile_candidates(t);
            if cands.is_empty() {
                continue;
            }
            if pack.get(&cands).is_some() {
                matched += 1;
            } else {
                missing.push(cands[0]);
            }
        }
        eprintln!(
            "tuiles atlas couvertes par le pack: {matched} - manquantes (fallback procédural): {missing:?}"
        );
        assert!(matched >= 140, "couverture atlas trop faible: {matched}");

        // build the atlas WITH the pack and dump it for visual review
        let t1 = std::time::Instant::now();
        let px = crate::renderer::build_atlas_pixels_for_test(Some(&pack));
        eprintln!("build_atlas_pixels: {:?}", t1.elapsed());
        let png = png_encode(crate::world::ATLAS_COLS * 16, crate::world::ATLAS_ROWS * 16, &px);
        std::fs::create_dir_all("docs").unwrap();
        std::fs::write("docs/atlas_vanilla_pack.png", png).unwrap();
        // reference (procedural) atlas for side-by-side review
        let pxp = crate::renderer::build_atlas_pixels_for_test(None);
        let pngp =
            png_encode(crate::world::ATLAS_COLS * 16, crate::world::ATLAS_ROWS * 16, &pxp);
        std::fs::write("docs/atlas_procedural.png", pngp).unwrap();

        // grass top + water stay grayscale in the atlas (mesh tints them)
        let at_tile = |t: u32, x: usize, y: usize| {
            let w = (crate::world::ATLAS_COLS * 16) as usize;
            let tx = (t as usize % crate::world::ATLAS_COLS as usize) * 16 + x;
            let ty = (t as usize / crate::world::ATLAS_COLS as usize) * 16 + y;
            let o = (ty * w + tx) * 4;
            [px[o], px[o + 1], px[o + 2]]
        };
        let g = at_tile(crate::world::T_GRASS_TOP, 0, 0);
        assert!(
            (g[0] as i32 - g[1] as i32).abs() < 3 && (g[1] as i32 - g[2] as i32).abs() < 3,
            "grass_block_top doit rester grayscale dans l'atlas: {g:?}"
        );
        let wt = at_tile(crate::world::T_WATER, 0, 0);
        eprintln!("eau (pack): {wt:?}");
    }

    #[test]
    fn stored_block_roundtrip() {
        // zlib stream with stored blocks, built by hand
        let data = b"hello hello hello world!".to_vec();
        let mut z = vec![0x78, 0x01];
        z.push(1u8);
        z.extend_from_slice(&(data.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(data.len() as u16)).to_le_bytes());
        z.extend_from_slice(&data);
        z.extend_from_slice(&adler32(&data).to_be_bytes());
        let out = inflate(&z[2..z.len() - 4]).unwrap();
        assert_eq!(out, data);
    }

    #[test]
    fn png_roundtrip_all_filters() {
        // build a noisy 16x16 rgba image, encode, decode
        let mut px = Vec::with_capacity(16 * 16 * 4);
        for y in 0..16u32 {
            for x in 0..16u32 {
                let v = ((x * 13 + y * 7) % 256) as u8;
                px.extend_from_slice(&[v, v.wrapping_mul(3), v.wrapping_add(9), if (x + y) % 5 == 0 { 0 } else { 255 }]);
            }
        }
        let enc = png_encode(16, 16, &px);
        let dec = png_decode(&enc).expect("decode must succeed");
        assert_eq!(dec.w, 16);
        assert_eq!(dec.h, 16);
        assert_eq!(dec.px, px);
        assert!(!dec.is_grayscale(4));
    }

    #[test]
    fn gray_detection_and_tint_source() {
        let mut px = vec![128u8; 16 * 16 * 4];
        for i in 0..16 * 16 {
            px[i * 4] = (i % 256) as u8;
            px[i * 4 + 1] = (i % 256) as u8;
            px[i * 4 + 2] = (i % 256) as u8;
            px[i * 4 + 3] = 255;
        }
        let img = Rgba { w: 16, h: 16, px };
        assert!(img.is_grayscale(4));
        let t = img.to_tile(16);
        assert_eq!(t.px, img.px);
        let mut p8 = Vec::with_capacity(8 * 8 * 4);
        for _ in 0..(8 * 8) {
            p8.extend_from_slice(&[10u8, 20, 30, 255]);
        }
        let up = Rgba { w: 8, h: 8, px: p8 };
        let down = up.to_tile(16);
        assert_eq!(down.px[0..4], [10, 20, 30, 255]);
    }

    #[test]
    fn pack_discovery_finds_vanilla_layout() {
        let base = std::env::temp_dir().join(format!("rvx_pack_test_{}", std::process::id()));
        let vanilla = base.join("texturepacks/vanilla/assets/minecraft/textures/block");
        std::fs::create_dir_all(&vanilla).unwrap();
        let px = vec![200u8; 16 * 16 * 4];
        std::fs::write(vanilla.join("dirt.png"), png_encode(16, 16, &px)).unwrap();
        let pack = Pack::discover(&base);
        assert!(pack.len() >= 1);
        let img = pack.get(&["dirt"]).expect("dirt must be found");
        assert_eq!(img.px[0], 200);
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn every_tile_has_candidates_or_is_intentional() {
        // all BLOCK tiles must map to at least one pack name
        for b in 0..=crate::world::MAX_BLOCK_ID {
            if b == crate::world::AIR || b == crate::world::WATER && b < 10 {
                continue;
            }
            let t = tiles_of(b);
            for tv in t {
                if tv != crate::world::T_SUN && tv != crate::world::T_MOON {
                    assert!(
                        !tile_candidates(tv).is_empty(),
                        "tile {tv} (block {b}) has no pack name"
                    );
                }
            }
        }
    }

    /// Build a real zip archive in-memory (stored or deflate_stored entries).
    fn build_zip(entries: &[(&str, Vec<u8>, bool)]) -> Vec<u8> {
        let mut z: Vec<u8> = Vec::new();
        let mut central: Vec<u8> = Vec::new();
        let mut count = 0u16;
        for (name, data, stored) in entries {
            let off = z.len() as u32;
            let crc = crc32(data);
            let (method, payload): (u16, Vec<u8>) = if *stored {
                (0, data.clone())
            } else {
                (8, deflate_stored(data))
            };
            z.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
            z.extend_from_slice(&[10, 0, 0, 0]);
            z.extend_from_slice(&method.to_le_bytes());
            z.extend_from_slice(&[0u8; 4]);
            z.extend_from_slice(&crc.to_le_bytes());
            z.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            z.extend_from_slice(&(data.len() as u32).to_le_bytes());
            z.extend_from_slice(&(name.len() as u16).to_le_bytes());
            z.extend_from_slice(&0u16.to_le_bytes());
            z.extend_from_slice(name.as_bytes());
            z.extend_from_slice(&payload);

            central.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
            central.extend_from_slice(&[10, 0, 10, 0]);
            central.extend_from_slice(&[0u8; 2]); // general purpose flags
            central.extend_from_slice(&method.to_le_bytes());
            central.extend_from_slice(&[0u8; 4]);
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            central.extend_from_slice(&(data.len() as u32).to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            // extra, comment, disk, internal attrs, external attrs
            central.extend_from_slice(&[0u8; 12]);
            central.extend_from_slice(&off.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
            count += 1;
        }
        let cd_start = z.len();
        z.extend_from_slice(&central);
        let cd_size = z.len() - cd_start;
        z.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        z.extend_from_slice(&[0u8; 4]);
        z.extend_from_slice(&count.to_le_bytes());
        z.extend_from_slice(&count.to_le_bytes());
        z.extend_from_slice(&(cd_size as u32).to_le_bytes());
        z.extend_from_slice(&(cd_start as u32).to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        z
    }

    #[test]
    fn zip_reader_handles_stored_and_deflated() {
        // hand-built zip: pack.mcmeta (stored), stone.png (stored) and a
        // vanilla-layout dirt.png (deflated with stored-block deflate)
        let files: Vec<(&str, Vec<u8>, bool)> = vec![
            ("pack.mcmeta", b"{\"pack\":{\"pack_format\":34}}".to_vec(), true),
            ("stone.png", b"PNGDATA-STORED".to_vec(), true),
            (
                "assets/minecraft/textures/block/dirt.png",
                b"PNGDATA-DEFLATED!".to_vec(),
                false,
            ),
        ];
        let z = build_zip(&files);
        let ents = zip_entries(&z);
        assert_eq!(ents.len(), 3, "all three entries must be read");
        assert_eq!(ents[0].0, "pack.mcmeta");
        assert_eq!(ents[1].0, "stone.png");
        assert_eq!(ents[1].1, b"PNGDATA-STORED");
        assert_eq!(ents[2].0, "assets/minecraft/textures/block/dirt.png");
        assert_eq!(ents[2].1, b"PNGDATA-DEFLATED!");
    }

    /// Raw deflate stream made of stored blocks (RFC 1951 type-00 blocks):
    /// enough to exercise the inflate path inside a zip with method 8.
    fn deflate_stored(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut off = 0usize;
        while off < data.len() {
            let n = (data.len() - off).min(65535);
            let last = if off + n >= data.len() { 1u8 } else { 0u8 };
            out.push(last);
            out.extend_from_slice(&(n as u16).to_le_bytes());
            out.extend_from_slice(&(!(n as u16)).to_le_bytes());
            out.extend_from_slice(&data[off..off + n]);
            off += n;
        }
        if data.is_empty() {
            out.push(1u8);
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0xffffu16.to_le_bytes());
        }
        out
    }

    /// End-to-end: a vanilla-layout .zip resource pack (pack.mcmeta,
    /// assets/minecraft/textures/block/*.png with an animation strip and a
    /// grayscale grass) must load, override tiles and get tinted.
    // Needs the renderer module (Windows FFI / fficheck) for the atlas build.
    #[test]
    #[cfg(any(windows, feature = "fficheck"))]
    fn vanilla_zip_pack_end_to_end() {
        use crate::world::{T_GRASS_TOP, T_WATER};
        let base = std::env::temp_dir().join(format!("rvx_vanzip_{}", std::process::id()));
        let tp = base.join("texturepacks");
        std::fs::create_dir_all(&tp).unwrap();

        let solid = |r: u8, g: u8, b: u8, a: u8| -> Vec<u8> {
            let mut px = Vec::with_capacity(16 * 16 * 4);
            for _ in 0..(16 * 16) {
                px.extend_from_slice(&[r, g, b, a]);
            }
            png_encode(16, 16, &px)
        };
        // vanilla grass_block_top is grayscale -> our tint must colorize it
        let mut gray = Vec::with_capacity(16 * 16 * 4);
        for i in 0..(16 * 16) {
            let v = 150 + (i % 7) as u8;
            gray.extend_from_slice(&[v, v, v, 255]);
        }
        let grass_png = png_encode(16, 16, &gray);
        // vanilla water_still is a 16x512 animation strip; frame 0 is used
        let mut wpx = Vec::with_capacity(16 * 512 * 4);
        for f in 0..32u8 {
            for _ in 0..(16 * 16) {
                wpx.extend_from_slice(&[30 + f, 90, 200, 255]);
            }
        }
        let water_png = png_encode(16, 512, &wpx);

        let entries: Vec<(&str, Vec<u8>, bool)> = vec![
            (
                "pack.mcmeta",
                b"{\"pack\":{\"pack_format\":48,\"description\":\"test\"}}".to_vec(),
                true,
            ),
            ("pack.png", solid(1, 2, 3, 255), true),
            (
                "assets/minecraft/textures/block/dirt.png",
                solid(120, 90, 60, 255),
                false,
            ),
            (
                "assets/minecraft/textures/block/grass_block_top.png",
                grass_png,
                false,
            ),
            (
                "assets/minecraft/textures/block/water_still.png",
                water_png,
                false,
            ),
            (
                "assets/minecraft/textures/block/cherry_log.png",
                solid(80, 50, 60, 255),
                false,
            ),
        ];
        let z = build_zip(&entries);
        std::fs::write(tp.join("vanilla_test.zip"), &z).unwrap();

        let pack = Pack::discover(&base);
        // pack.png / pack.mcmeta must NOT become textures
        assert!(!pack.loaded_names().iter().any(|n| n == "pack"));
        let dirt = pack.get(&["dirt"]).expect("dirt from zip");
        assert_eq!(dirt.px[0], 120);
        // strip: only frame 0 survives the crop
        let water = pack.get(&["water_still"]).expect("water from zip");
        assert_eq!(water.px[0], 30);
        let cherry = pack.get(&["cherry_log"]).expect("cherry log from zip");
        assert_eq!(cherry.px[0], 80);

        // grayscale grass stays grayscale in the atlas: the MESHER now
        // applies the biome color per vertex (colormap system)
        let px = crate::renderer::build_atlas_pixels_for_test(Some(&pack));
        let t = T_GRASS_TOP as usize * 16 * 16 * 4;
        let (r, g, b) = (px[t] as f32, px[t + 1] as f32, px[t + 2] as f32);
        assert!(
            (r - g).abs() < 2.0 && (g - b).abs() < 2.0,
            "grass tile must stay grayscale in the atlas: {r},{g},{b}"
        );
        // water: colored frame wins, frame 0 pixel preserved
        // row-major atlas: T_WATER=10 -> tx=10*16, ty=0
        let w = ((T_WATER % crate::world::ATLAS_COLS) as usize * 16
            + (T_WATER / crate::world::ATLAS_COLS) as usize * 16 * (crate::world::ATLAS_COLS * 16) as usize)
            * 4;
        assert_eq!(px[w], 30, "pack water frame 0 wins over procedural");

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn colormap_pngs_are_picked_up_and_sampled() {
        use crate::world::{B_BADLANDS, B_PLAINS, B_SWAMP, BIOME_COUNT, BiomePalette};
        let base = std::env::temp_dir().join(format!("rvx_cmap_{}", std::process::id()));
        let dir = base.join("texturepacks/cmap/assets/minecraft/textures/colormap");
        std::fs::create_dir_all(&dir).unwrap();
        // 256x256 grass colormap: base blue-ish, magenta exactly at the
        // plains climate point (t=0.8, d=0.4 -> x=51, y=173)
        let mut px = vec![0u8; 256 * 256 * 4];
        for i in 0..(256 * 256) {
            px[i * 4] = 10;
            px[i * 4 + 1] = 20;
            px[i * 4 + 2] = 200;
            px[i * 4 + 3] = 255;
        }
        let at = |x: u32, y: u32| ((y * 256 + x) * 4) as usize;
        // paint a 2x2 patch (x 50..52, y 172..174) so tiny f32 truncation
        // differences around the climate point cannot break the sample
        for x in 50..52u32 {
            for y in 172..174u32 {
                px[at(x, y)] = 255;
                px[at(x, y) + 1] = 0;
                px[at(x, y) + 2] = 255;
            }
        }
        std::fs::write(dir.join("grass.png"), png_encode(256, 256, &px)).unwrap();

        let pack = Pack::discover(&base);
        assert!(pack.colormap_grass.is_some(), "grass colormap must load");
        assert!(pack.colormap_foliage.is_none(), "no foliage map in this pack");

        let pal = BiomePalette::from_pack(&pack);
        let g = pal.grass[B_PLAINS as usize];
        assert!(
            g[0] > 0.9 && g[2] > 0.9 && g[1] < 0.2,
            "plains grass must sample the colormap at its climate, got {:?}",
            g
        );
        // other biomes get the map base color
        let f = pal.grass[B_BADLANDS as usize];
        // badlands keeps its fixed override -> must NOT be the colormap base
        assert!((f[0] - 10.0 / 255.0).abs() > 0.01);
        // swamp override untouched too
        let sw = pal.grass[B_SWAMP as usize];
        assert!((sw[0] - 0x6A as f32 / 255.0).abs() < 1e-4);
        // full coverage of the table
        assert_eq!(pal.grass.len(), BIOME_COUNT);
        std::fs::remove_dir_all(&base).ok();
    }
}
