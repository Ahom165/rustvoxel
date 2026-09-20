//! Protocole vanilla Minecraft Java 1.20.5 (protocole 766), zéro dépendance.
//! Implémentation interopérable fondée sur la spécification publique (wiki.vg /
//! minecraft.wiki, données machine PrismarineJS). Aucun code ni asset Mojang.
//!
//! Mode offline (sans Mojang Yggdrasil) : pas de chiffrement, pas de compression
//! (threshold -1, supporté par le client vanilla). UUID offline = UUID v3
//! `OfflinePlayer:<nom>` (MD5 maison).

use std::io::{self, Read, Write};

pub const MAX_PACKET: usize = 4 * 1024 * 1024; // 4 Mio comme le serveur vanilla

// ---------------------------------------------------------------- varint / varlong

pub fn varint_len(v: u32) -> usize {
    let mut n = 1;
    let mut v = v >> 7;
    while v != 0 {
        n += 1;
        v >>= 7;
    }
    n
}

pub fn write_varint(out: &mut Vec<u8>, v: i32) {
    let mut v = v as u32;
    loop {
        if v & 0xFFFFFF80 == 0 {
            out.push(v as u8);
            return;
        }
        out.push(((v & 0x7F) | 0x80) as u8);
        v >>= 7;
    }
}

pub fn write_varlong(out: &mut Vec<u8>, v: i64) {
    let mut v = v as u64;
    loop {
        if v & 0xFFFFFFFFFFFFFF80 == 0 {
            out.push(v as u8);
            return;
        }
        out.push(((v & 0x7F) | 0x80) as u8);
        v >>= 7;
    }
}

pub fn read_varint(b: &[u8], p: &mut usize) -> Option<i32> {
    let mut v: u32 = 0;
    for i in 0..5 {
        let byte = *b.get(*p)?;
        *p += 1;
        v |= ((byte & 0x7F) as u32) << (7 * i);
        if byte & 0x80 == 0 {
            return Some(v as i32);
        }
    }
    None // varint trop long
}

pub fn read_varlong(b: &[u8], p: &mut usize) -> Option<i64> {
    let mut v: u64 = 0;
    for i in 0..10 {
        let byte = *b.get(*p)?;
        *p += 1;
        v |= ((byte & 0x7F) as u64) << (7 * i);
        if byte & 0x80 == 0 {
            return Some(v as i64);
        }
    }
    None
}

// ---------------------------------------------------------------- primitives

pub fn write_string(out: &mut Vec<u8>, s: &str) {
    write_varint(out, s.len() as i32);
    out.extend_from_slice(s.as_bytes());
}

pub fn read_string(b: &[u8], p: &mut usize, max: usize) -> Option<String> {
    let n = read_varint(b, p)? as usize;
    if n > max || *p + n > b.len() {
        return None;
    }
    let s = String::from_utf8_lossy(&b[*p..*p + n]).to_string();
    *p += n;
    Some(s)
}

pub fn write_uuid(out: &mut Vec<u8>, u: [u8; 16]) {
    out.extend_from_slice(&u);
}

pub fn read_uuid(b: &[u8], p: &mut usize) -> Option<[u8; 16]> {
    let u = b.get(*p..*p + 16)?;
    *p += 16;
    Some(u.try_into().ok()?)
}

/// Position bloc compactée : x:26 | z:26 | y:12 (signés).
pub fn pack_position(x: i32, y: i32, z: i32) -> i64 {
    ((x as i64 & 0x3FF_FFFF) << 38) | ((z as i64 & 0x3FF_FFFF) << 12) | (y as i64 & 0xFFF)
}

pub struct BlockPos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

pub fn unpack_position(v: i64) -> BlockPos {
    let x = (v >> 38) as i32;
    let z = ((v << 26) >> 38) as i32;
    let y = ((v << 52) >> 52) as i32;
    BlockPos { x, y, z }
}

// ---------------------------------------------------------------- angles

pub const PI: f32 = std::f32::consts::PI;

/// Notre yaw (rad, 0 = -Z) -> degrés vanilla (0 = +Z, sens horaire vu du dessus).
pub fn van_yaw_deg(our_yaw: f32) -> f32 {
    let deg = (our_yaw - PI) * 180.0 / PI;
    let deg = deg.rem_euclid(360.0);
    deg
}

/// Degrés vanilla -> notre yaw (rad).
pub fn our_yaw(van_deg: f32) -> f32 {
    (van_deg + 180.0) * PI / 180.0
}

pub fn van_pitch(our_pitch: f32) -> f32 {
    -our_pitch * 180.0 / PI
}

pub fn our_pitch(van: f32) -> f32 {
    -van * PI / 180.0
}

pub fn angle_i8(van_deg: f32) -> u8 {
    let v = ((van_deg * 256.0 / 360.0).round() as i32).rem_euclid(256);
    v as u8
}

pub fn i8_angle(v: u8) -> f32 {
    (v as i32 as f32) * 360.0 / 256.0
}

// ---------------------------------------------------------------- MD5 (UUID offline)

/// MD5 (RFC 1321) — uniquement pour les UUID offline v3 (interopérabilité).
pub fn md5(msg: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6,
        10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = K_TABLE;

    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xEFCDAB89;
    let mut c0: u32 = 0x98BADCFE;
    let mut d0: u32 = 0x10325476;

    let mut data = msg.to_vec();
    let bitlen = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bitlen.to_le_bytes());

    for chunk in data.chunks(64) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            *w = u32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f2 = f
                .wrapping_add(a)
                .wrapping_add(K[i])
                .wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f2.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    out
}

const K_TABLE: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];

/// UUID offline vanilla : v3 sur "OfflinePlayer:<nom>".
pub fn offline_uuid(name: &str) -> [u8; 16] {
    let mut u = md5(format!("OfflinePlayer:{}", name).as_bytes());
    // version 3 + variant RFC 4122
    u[6] = (u[6] & 0x0F) | 0x30;
    u[8] = (u[8] & 0x3F) | 0x80;
    u
}

/// UUID aléatoire (v4) depuis un PRNG xorshift — pour les entités.
pub fn random_uuid(seed: &mut u64) -> [u8; 16] {
    let mut next = || {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    };
    let hi = next().to_be_bytes();
    let lo = next().to_be_bytes();
    let mut u = [0u8; 16];
    u[0..8].copy_from_slice(&hi);
    u[8..16].copy_from_slice(&lo);
    u[6] = (u[6] & 0x0F) | 0x40;
    u[8] = (u[8] & 0x3F) | 0x80;
    u
}

// ---------------------------------------------------------------- lecture bloquante

/// Lit exactement `n` octets (bloquant).
pub fn read_exact(stream: &mut impl Read, buf: &mut [u8]) -> io::Result<()> {
    stream.read_exact(buf)
}

/// Lit un paquet complet (longueur varint + payload) en bloquant.
pub fn read_frame(stream: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut vb = [0u8; 1];
    let mut len: u32 = 0;
    let mut shift = 0;
    loop {
        stream.read_exact(&mut vb)?;
        len |= ((vb[0] & 0x7F) as u32) << shift;
        if vb[0] & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift > 28 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "varint longueur"));
        }
    }
    if len as usize > MAX_PACKET {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "paquet trop grand"));
    }
    let mut out = vec![0u8; len as usize];
    stream.read_exact(&mut out)?;
    Ok(out)
}

/// Écrit un paquet (préfixe longueur varint).
pub fn write_frame(stream: &mut impl Write, payload: &[u8]) -> io::Result<()> {
    let mut head = Vec::with_capacity(5);
    write_varint(&mut head, payload.len() as i32);
    stream.write_all(&head)?;
    stream.write_all(payload)
}

// ---------------------------------------------------------------- IDs de paquets (766)

pub mod cb {
    // configuration
    pub const CFG_REGISTRY_DATA: i32 = 0x07;
    pub const CFG_FINISH: i32 = 0x03;
    pub const CFG_KEEP_ALIVE: i32 = 0x04;
    pub const CFG_DISCONNECT: i32 = 0x02;
    // login
    pub const LOGIN_DISCONNECT: i32 = 0x00;
    pub const LOGIN_SUCCESS: i32 = 0x02;
    // play
    pub const BUNDLE_DELIMITER: i32 = 0x00;
    pub const SPAWN_ENTITY: i32 = 0x01;
    pub const ANIMATION: i32 = 0x03;
    pub const CHUNK_BATCH_FINISHED: i32 = 0x0C;
    pub const CHUNK_BATCH_START: i32 = 0x0D;
    pub const BLOCK_CHANGE: i32 = 0x09;
    pub const UNLOAD_CHUNK: i32 = 0x21;
    pub const KEEP_ALIVE: i32 = 0x26;
    pub const MAP_CHUNK: i32 = 0x27;
    pub const LOGIN: i32 = 0x2B;
    pub const REL_ENTITY_MOVE: i32 = 0x2E;
    pub const ENTITY_MOVE_LOOK: i32 = 0x2F;
    pub const ENTITY_LOOK: i32 = 0x30;
    pub const ABILITIES: i32 = 0x38;
    pub const PLAYER_REMOVE: i32 = 0x3D;
    pub const PLAYER_INFO: i32 = 0x3E;
    pub const POSITION: i32 = 0x40;
    pub const ENTITY_DESTROY: i32 = 0x42;
    pub const RESPAWN: i32 = 0x47;
    pub const ENTITY_HEAD_ROTATION: i32 = 0x48;
    pub const HELD_ITEM_SLOT: i32 = 0x53;
    pub const UPDATE_VIEW_POSITION: i32 = 0x54;
    pub const SPAWN_POSITION: i32 = 0x56;
    pub const EXPERIENCE: i32 = 0x5C;
    pub const UPDATE_HEALTH: i32 = 0x5D;
    pub const SIMULATION_DISTANCE: i32 = 0x62;
    pub const UPDATE_TIME: i32 = 0x64;
    pub const SYSTEM_CHAT: i32 = 0x6C;
    pub const ENTITY_TELEPORT: i32 = 0x70;
    pub const SET_SLOT: i32 = 0x15;
    pub const GAME_STATE_CHANGE: i32 = 0x22;
    pub const KICK_DISCONNECT: i32 = 0x1D;
    pub const STATUS_SERVER_INFO: i32 = 0x00;
}

pub mod sb {
    // status
    pub const PING_START: i32 = 0x00;
    pub const PING: i32 = 0x01;
    // login
    pub const LOGIN_START: i32 = 0x00;
    pub const LOGIN_ACK: i32 = 0x03;
    // configuration
    pub const CFG_SETTINGS: i32 = 0x00;
    pub const CFG_PLUGIN_MESSAGE: i32 = 0x02;
    pub const CFG_FINISH: i32 = 0x03;
    pub const CFG_KEEP_ALIVE: i32 = 0x04;
    pub const CFG_PONG: i32 = 0x05;
    pub const CFG_SELECT_KNOWN_PACKS: i32 = 0x07;
    // play
    pub const TELEPORT_CONFIRM: i32 = 0x00;
    pub const CHAT_COMMAND: i32 = 0x04;
    pub const CHAT_COMMAND_SIGNED: i32 = 0x05;
    pub const CHAT_MESSAGE: i32 = 0x06;
    pub const CHUNK_BATCH_RECEIVED: i32 = 0x08;
    pub const CLIENT_COMMAND: i32 = 0x09;
    pub const SETTINGS: i32 = 0x0A;
    pub const CONFIGURATION_ACK: i32 = 0x0C;
    pub const PLUGIN_MESSAGE: i32 = 0x12;
    pub const USE_ENTITY: i32 = 0x16;
    pub const KEEP_ALIVE: i32 = 0x18;
    pub const POSITION: i32 = 0x1A;
    pub const POSITION_LOOK: i32 = 0x1B;
    pub const LOOK: i32 = 0x1C;
    pub const FLYING: i32 = 0x1D;
    pub const ABILITIES: i32 = 0x23;
    pub const BLOCK_DIG: i32 = 0x24;
    pub const ENTITY_ACTION: i32 = 0x25;
    pub const HELD_ITEM_SLOT: i32 = 0x2F;
    pub const SET_CREATIVE_SLOT: i32 = 0x32;
    pub const ARM_ANIMATION: i32 = 0x36;
    pub const BLOCK_PLACE: i32 = 0x38;
    pub const USE_ITEM: i32 = 0x39;
}

// ---------------------------------------------------------------- builders (clientbound)

/// Constructeur de payload (big-endian, façon vanilla).
pub struct W2 {
    pub v: Vec<u8>,
}

impl W2 {
    pub fn new() -> W2 {
        W2 { v: Vec::new() }
    }
    pub fn vi(mut self, v: i32) -> W2 {
        write_varint(&mut self.v, v);
        self
    }
    pub fn vl(mut self, v: i64) -> W2 {
        write_varlong(&mut self.v, v);
        self
    }
    pub fn u8v(mut self, v: u8) -> W2 {
        self.v.push(v);
        self
    }
    pub fn i8v(mut self, v: i8) -> W2 {
        self.v.push(v as u8);
        self
    }
    pub fn b(mut self, v: bool) -> W2 {
        self.v.push(if v { 1 } else { 0 });
        self
    }
    pub fn i16v(mut self, v: i16) -> W2 {
        self.v.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn u16v(mut self, v: u16) -> W2 {
        self.v.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn i32v(mut self, v: i32) -> W2 {
        self.v.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn i64v(mut self, v: i64) -> W2 {
        self.v.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn f32v(mut self, v: f32) -> W2 {
        self.v.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn f64v(mut self, v: f64) -> W2 {
        self.v.extend_from_slice(&v.to_be_bytes());
        self
    }
    pub fn s(mut self, s: &str) -> W2 {
        write_string(&mut self.v, s);
        self
    }
    pub fn uuid(mut self, u: &[u8; 16]) -> W2 {
        write_uuid(&mut self.v, *u);
        self
    }
    pub fn pos(self, x: i32, y: i32, z: i32) -> W2 {
        self.i64v(pack_position(x, y, z))
    }
    pub fn bytes(mut self, b: &[u8]) -> W2 {
        self.v.extend_from_slice(b);
        self
    }
    pub fn nbt(mut self, n: &crate::nbt::Nbt) -> W2 {
        n.write_network(&mut self.v);
        self
    }
    pub fn done(self) -> Vec<u8> {
        self.v
    }
}

// ---------------------------------------------------------------- reader (serverbound)

pub struct R2<'a> {
    pub b: &'a [u8],
    pub p: usize,
}

impl<'a> R2<'a> {
    pub fn new(b: &'a [u8]) -> R2<'a> {
        R2 { b, p: 0 }
    }
    pub fn vi(&mut self) -> Option<i32> {
        read_varint(self.b, &mut self.p)
    }
    pub fn u8v(&mut self) -> Option<u8> {
        let v = *self.b.get(self.p)?;
        self.p += 1;
        Some(v)
    }
    pub fn i8v(&mut self) -> Option<i8> {
        self.u8v().map(|v| v as i8)
    }
    pub fn b(&mut self) -> Option<bool> {
        self.u8v().map(|v| v != 0)
    }
    pub fn i16v(&mut self) -> Option<i16> {
        let v = self.b.get(self.p..self.p + 2)?;
        self.p += 2;
        Some(i16::from_be_bytes(v.try_into().ok()?))
    }
    pub fn u16v(&mut self) -> Option<u16> {
        let v = self.b.get(self.p..self.p + 2)?;
        self.p += 2;
        Some(u16::from_be_bytes(v.try_into().ok()?))
    }
    pub fn i32v(&mut self) -> Option<i32> {
        let v = self.b.get(self.p..self.p + 4)?;
        self.p += 4;
        Some(i32::from_be_bytes(v.try_into().ok()?))
    }
    pub fn i64v(&mut self) -> Option<i64> {
        let v = self.b.get(self.p..self.p + 8)?;
        self.p += 8;
        Some(i64::from_be_bytes(v.try_into().ok()?))
    }
    pub fn f32v(&mut self) -> Option<f32> {
        self.i32v().map(|x| f32::from_bits(x as u32))
    }
    pub fn f64v(&mut self) -> Option<f64> {
        self.i64v().map(|x| f64::from_bits(x as u64))
    }
    pub fn s(&mut self, max: usize) -> Option<String> {
        read_string(self.b, &mut self.p, max)
    }
    pub fn uuid(&mut self) -> Option<[u8; 16]> {
        read_uuid(self.b, &mut self.p)
    }
    pub fn pos(&mut self) -> Option<(i32, i32, i32)> {
        let v = self.i64v()?;
        let bp = unpack_position(v);
        Some((bp.x, bp.y, bp.z))
    }
    pub fn skip(&mut self, n: usize) -> Option<()> {
        if self.p + n > self.b.len() {
            return None;
        }
        self.p += n;
        Some(())
    }
    pub fn rest(&self) -> &'a [u8] {
        &self.b[self.p.min(self.b.len())..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip() {
        for v in [0i32, 1, 127, 128, 255, 256, 2097151, 2097152, -1, i32::MIN, i32::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, v);
            let mut p = 0;
            assert_eq!(read_varint(&buf, &mut p), Some(v));
            assert_eq!(p, buf.len());
        }
    }

    #[test]
    fn varlong_roundtrip() {
        for v in [0i64, -1, i64::MAX, i64::MIN, 0x1234_5678_9ABC_DEF] {
            let mut buf = Vec::new();
            write_varlong(&mut buf, v);
            let mut p = 0;
            assert_eq!(read_varlong(&buf, &mut p), Some(v));
        }
    }

    #[test]
    fn position_packing() {
        let (x, y, z) = (12_345, 319, -65_000);
        let p = pack_position(x, y, z);
        let bp = unpack_position(p);
        assert_eq!((bp.x, bp.y, bp.z), (x, y, z));
    }

    #[test]
    fn md5_known_vectors() {
        assert_eq!(
            hex(&md5(b"")),
            "d41d8cd98f00b204e9800998ecf8427e"
        );
        assert_eq!(
            hex(&md5(b"The quick brown fox jumps over the lazy dog")),
            "9e107d9d372bb6826bd81d3542a419d6"
        );
    }

    #[test]
    fn offline_uuid_shape() {
        let u = offline_uuid("Notch");
        assert_eq!(u[6] & 0xF0, 0x30); // v3
        assert_eq!(u[8] & 0xC0, 0x80); // variant
    }

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{:02x}", x)).collect()
    }

    #[test]
    fn angles_roundtrip() {
        // notre yaw 0 (face -Z/nord) = vanilla 180°
        assert!((van_yaw_deg(0.0) - 180.0).abs() < 0.01);
        // notre yaw +90°(rad) : fwd=(sin90,-cos90)=(1,0) -> +X ouest -> vanilla 270°
        assert!((van_yaw_deg(std::f32::consts::FRAC_PI_2) - 270.0).abs() < 0.01);
        assert!((our_yaw(180.0) - 2.0 * PI).abs() < 1e-3); // vanilla 180° = notre 0
        // pitch : notre + = haut, vanilla + = bas
        assert!((van_pitch(0.5) + 0.5 * 180.0 / PI).abs() < 1e-4);
        assert!((our_pitch(-90.0) - std::f32::consts::FRAC_PI_2).abs() < 1e-3);
    }

    #[test]
    fn string_roundtrip() {
        let mut buf = Vec::new();
        write_string(&mut buf, "bonjour é §");
        let mut p = 0;
        assert_eq!(read_string(&buf, &mut p, 1000).unwrap(), "bonjour é §");
    }
}
