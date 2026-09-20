// Zero-dependency networking for RustVoxel multiplayer.
//
// Transport: plain TCP (std::net). Framing: [u32 LE length][u8 opcode][payload].
// All integers are little-endian. A hand-rolled RLE keeps chunk payloads small.
//
// Design notes:
// - The server is authoritative for the WORLD (blocks, mobs, time, health).
//   Clients simulate their own movement (LAN trust) and stream positions.
// - The same protocol is used by the dedicated server binary and by the
//   client's integrated server (solo mode loops back over 127.0.0.1).

use std::io::{Read, Write};
use std::net::TcpStream;

pub const PROTOCOL: u16 = 6;

/// Hard cap for one framed packet (a chunk with biomes is ~70 KB worst case).
pub const MAX_PACKET: u32 = 4 * 1024 * 1024;

// ------------------------------------------------------------- opcodes C->S
pub const C_HANDSHAKE: u8 = 1;
pub const C_STATUS: u8 = 2;
pub const C_POS: u8 = 3;
pub const C_DIG: u8 = 4;
pub const C_PLACE: u8 = 5;
pub const C_ATTACK: u8 = 6;
pub const C_CHAT: u8 = 7;
pub const C_HOTBAR: u8 = 8;
pub const C_DAMAGE: u8 = 9;
pub const C_EAT: u8 = 10;
pub const C_RESPAWN: u8 = 11;
pub const C_PING: u8 = 12;

// ------------------------------------------------------------- opcodes S->C
pub const S_WELCOME: u8 = 16;
pub const S_STATUS_REPLY: u8 = 17;
pub const S_CHUNK: u8 = 18;
pub const S_BLOCK: u8 = 19;
pub const S_ENTITIES: u8 = 20;
pub const S_TIME: u8 = 21;
pub const S_CHAT: u8 = 22;
pub const S_PLAYERS: u8 = 23;
pub const S_PLAYER_LEAVE: u8 = 24;
pub const S_HEALTH: u8 = 25;
pub const S_PONG: u8 = 26;
pub const S_RESPAWN_AT: u8 = 27;
pub const S_KICK: u8 = 28;
pub const S_EVENT: u8 = 29;

// entity flags (S_ENTITIES)
pub const EF_MOVING: u8 = 1;
pub const EF_ON_GROUND: u8 = 2;
pub const EF_HURT: u8 = 4;
pub const EF_PRIMED: u8 = 8;
pub const EF_BURNING: u8 = 16;

// S_EVENT kinds
pub const EV_EXPLOSION: u8 = 0;
pub const EV_BREAK: u8 = 1;
pub const EV_HURT_MOB: u8 = 2;
pub const EV_EAT: u8 = 3;

// ------------------------------------------------------------------ reader
pub struct Rdr<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Rdr<'a> {
    pub fn new(b: &'a [u8]) -> Rdr<'a> {
        Rdr { b, p: 0 }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.p + n > self.b.len() {
            return None;
        }
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        Some(s)
    }
    fn arr<const N: usize>(&mut self) -> Option<[u8; N]> {
        let s = self.take(N)?;
        let mut a = [0u8; N];
        a.copy_from_slice(s);
        Some(a)
    }
    pub fn u8(&mut self) -> Option<u8> {
        self.arr::<1>().map(|a| a[0])
    }
    pub fn u16(&mut self) -> Option<u16> {
        self.arr::<2>().map(u16::from_le_bytes)
    }
    pub fn u32(&mut self) -> Option<u32> {
        self.arr::<4>().map(u32::from_le_bytes)
    }
    pub fn u64(&mut self) -> Option<u64> {
        self.arr::<8>().map(u64::from_le_bytes)
    }
    pub fn i32(&mut self) -> Option<i32> {
        self.arr::<4>().map(i32::from_le_bytes)
    }
    pub fn f32(&mut self) -> Option<f32> {
        self.arr::<4>().map(f32::from_le_bytes)
    }
    pub fn str(&mut self) -> Option<String> {
        let n = self.u16()? as usize;
        if n > 32 * 1024 {
            return None; // sanity cap
        }
        let s = self.take(n)?;
        Some(String::from_utf8_lossy(s).into_owned())
    }
    pub fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        self.take(n)
    }
}

// ------------------------------------------------------------------ writer
#[derive(Default)]
pub struct W {
    pub v: Vec<u8>,
}

impl W {
    pub fn new() -> W {
        W { v: Vec::with_capacity(64) }
    }
    pub fn u8(&mut self, x: u8) -> &mut Self {
        self.v.push(x);
        self
    }
    pub fn u16(&mut self, x: u16) -> &mut Self {
        self.v.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn u32(&mut self, x: u32) -> &mut Self {
        self.v.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn u64(&mut self, x: u64) -> &mut Self {
        self.v.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn i32(&mut self, x: i32) -> &mut Self {
        self.v.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn f32(&mut self, x: f32) -> &mut Self {
        self.v.extend_from_slice(&x.to_le_bytes());
        self
    }
    pub fn str(&mut self, s: &str) -> &mut Self {
        let b = s.as_bytes();
        self.u16(b.len().min(u16::MAX as usize) as u16);
        self.v.extend_from_slice(&b[..b.len().min(u16::MAX as usize)]);
        self
    }
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.v.extend_from_slice(b);
        self
    }
    pub fn done(&self) -> Vec<u8> {
        self.v.clone()
    }
}

/// Frame one packet: [u32 len][u8 id][payload].
pub fn frame(id: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 5);
    out.extend_from_slice(&((payload.len() + 1) as u32).to_le_bytes());
    out.push(id);
    out.extend_from_slice(payload);
    out
}

pub fn write_packet(s: &mut TcpStream, id: u8, payload: &[u8]) -> std::io::Result<()> {
    s.write_all(&frame(id, payload))
}

/// Blocking read of one framed packet. Returns (opcode, payload).
pub fn read_packet(s: &mut TcpStream) -> std::io::Result<(u8, Vec<u8>)> {
    let mut lb = [0u8; 4];
    s.read_exact(&mut lb)?;
    let len = u32::from_le_bytes(lb);
    if len < 1 || len > MAX_PACKET {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bad packet length",
        ));
    }
    let mut buf = vec![0u8; len as usize];
    s.read_exact(&mut buf)?;
    Ok((buf[0], buf[1..].to_vec()))
}

// --------------------------------------------------------------------- RLE
/// RLE-encode a u16 block array: [u16 value][u32 run] repeated.
pub fn rle_encode(data: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 2 + 8);
    let mut i = 0usize;
    while i < data.len() {
        let v = data[i];
        let mut run = 1usize;
        while i + run < data.len() && data[i + run] == v && run < u32::MAX as usize {
            run += 1;
        }
        out.extend_from_slice(&v.to_le_bytes());
        out.extend_from_slice(&(run as u32).to_le_bytes());
        i += run;
    }
    out
}

/// Decode an RLE stream into `out` (which must already be sized).
pub fn rle_decode(encoded: &[u8], out: &mut [u16]) -> Option<()> {
    let mut r = Rdr::new(encoded);
    let mut filled = 0usize;
    while filled < out.len() {
        let v = r.u16()?;
        let run = r.u32()? as usize;
        if run == 0 || filled + run > out.len() {
            return None;
        }
        for k in 0..run {
            out[filled + k] = v;
        }
        filled += run;
    }
    Some(())
}

// ------------------------------------------------------------ float helpers
/// Compress a yaw angle to a byte (full circle = 256).
pub fn yaw_to_u8(yaw: f32) -> u8 {
    let t = yaw / std::f32::consts::TAU * 256.0;
    (t.rem_euclid(256.0)) as u8
}

pub fn u8_to_yaw(v: u8) -> f32 {
    v as f32 / 256.0 * std::f32::consts::TAU
}

// -------------------------------------------------------------------- tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};

    #[test]
    fn rle_roundtrip_and_compression() {
        // air-heavy chunk must compress hard
        let mut blocks = vec![0u16; 65536];
        for i in (0..65536).step_by(97) {
            blocks[i] = 3;
        }
        let enc = rle_encode(&blocks);
        assert!(enc.len() < 9000, "must compress: {}", enc.len());
        let mut out = vec![9u16; 65536];
        rle_decode(&enc, &mut out).unwrap();
        assert_eq!(out, blocks);
    }

    #[test]
    fn rle_rejects_overflow() {
        let mut enc = Vec::new();
        enc.extend_from_slice(&7u16.to_le_bytes());
        enc.extend_from_slice(&99u32.to_le_bytes()); // run > out.len()
        let mut out = [0u16; 10];
        assert!(rle_decode(&enc, &mut out).is_none());
    }

    #[test]
    fn writer_reader_roundtrip() {
        let mut w = W::new();
        w.u8(42).u16(300).u32(70000).i32(-5).f32(1.5).str("créperie").u64(99);
        let buf = w.done();
        let mut r = Rdr::new(&buf);
        assert_eq!(r.u8(), Some(42));
        assert_eq!(r.u16(), Some(300));
        assert_eq!(r.u32(), Some(70000));
        assert_eq!(r.i32(), Some(-5));
        assert_eq!(r.f32(), Some(1.5));
        assert_eq!(r.str().as_deref(), Some("créperie"));
        assert_eq!(r.u64(), Some(99));
        assert_eq!(r.u8(), None, "must be exhausted");
    }

    /// Echo server over real TCP: every packet we send must survive framing,
    /// fragmentation and come back identical.
    #[test]
    fn framed_tcp_echo_all_packets() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let jh = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            // echo up to 16 packets
            for _ in 0..16 {
                let (id, payload) = match read_packet(&mut s) {
                    Ok(p) => p,
                    Err(_) => return,
                };
                if write_packet(&mut s, id, &payload).is_err() {
                    return;
                }
            }
        });

        let mut s = TcpStream::connect(addr).unwrap();
        // a big chunk-like payload (> socket buffers, forces fragmentation)
        let big = vec![0xABu8; 900_000];

        let packets: Vec<(u8, Vec<u8>)> = vec![
            (C_HANDSHAKE, W::new().u16(PROTOCOL).str("Steve").done()),
            (C_STATUS, vec![]),
            (C_POS, W::new().f32(1.0).f32(2.0).f32(3.0).f32(0.25).f32(-1.0).u8(5).done()),
            (C_DIG, W::new().i32(-12).i32(64).i32(300).done()),
            (C_PLACE, W::new().i32(1).i32(2).i32(3).u16(189).done()),
            (C_ATTACK, W::new().u32(12345).done()),
            (C_CHAT, W::new().str("salut / ça marche ?").done()),
            (C_HOTBAR, W::new().u8(3).u16(219).done()),
            (C_DAMAGE, W::new().u8(3).done()),
            (C_EAT, vec![]),
            (C_RESPAWN, vec![]),
            (C_PING, W::new().u64(123456789).done()),
            (S_CHUNK, W::new().i32(7).i32(-7).bytes(&big).done()),
            (S_WELCOME, W::new().u32(1).f32(8.5).f32(70.0).f32(8.5).f32(0.0).u64(42).u16(6000).u8(0).done()),
            (S_ENTITIES, W::new().u16(1).u32(9).u8(1).f32(1.).f32(2.).f32(3.).u8(128).u8(7).u8(60).done()),
            (S_KICK, W::new().str("au revoir").done()),
        ];

        for (id, payload) in &packets {
            write_packet(&mut s, *id, payload).unwrap();
        }
        for (id, payload) in &packets {
            let (rid, rp) = read_packet(&mut s).unwrap();
            assert_eq!(rid, *id);
            assert_eq!(&rp, payload);
        }
        jh.join().unwrap();
    }

    #[test]
    fn yaw_byte_roundtrip() {
        for yaw in [-3.1f32, -1.0, 0.0, 0.5, 3.1, 6.0] {
            let back = u8_to_yaw(yaw_to_u8(yaw));
            let d = (back - yaw.rem_euclid(std::f32::consts::TAU)).abs();
            let d = d.min((d - std::f32::consts::TAU).abs());
            assert!(d < 0.03, "yaw {yaw} -> {back}");
        }
    }
}
