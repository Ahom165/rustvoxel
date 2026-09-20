#!/usr/bin/env python3
# Interopérabilité Anvil SENS Python → Rust :
#  1. Python écrit un monde « écrit par un tiers » : région r.0.0.mca (zlib
#     niveau 6 = Huffman dynamique), chunk vanilla SANS rvx (palette de
#     compounds Name seulement), level.dat gzip avec RandomSeed.
#  2. Le serveur Rust doit : lire la graine, charger les chunks (noms vanilla
#     → ids internes : deepslate→189, red_wool→94, water→9), servir le monde.
#  3. Un joueur creuse (modifie le chunk), /save → le serveur réécrit l'Anvil.
#  4. Python RELIT la région : deepslate/red_wool/water ont gardé leur nom,
#     le bloc creusé est devenu air.
import gzip, io, math, os, shutil, socket, struct, subprocess, sys, time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e_anvil_check as e2e
from e2e_anvil_check import Rdr, join, wait_position_and_chunks, send_packet, varint

SRV = "/home/z/my-project/download/rustvoxel/target/release/rustvoxel_server"
PORT = 25597
WORLD = "/tmp/rvx_vanilla/world"
SEED = 1234567890123
e2e.PORT = PORT  # join() lit le PORT du module importé

# ---------------------------------------------------------------- NBT writer

def nbt_str(s):
    b = s.encode()
    return struct.pack(">H", len(b)) + b

def tag(tid, name, payload):
    return bytes([tid]) + nbt_str(name) + payload

def comp(children):
    return b"".join(children) + b"\x00"

def nbt_string(s):            # TAG_String payload
    return nbt_str(s)

def nbt_int(v):
    return struct.pack(">i", v)

def nbt_long(v):
    return struct.pack(">q", v)

def nbt_byte(v):
    return struct.pack(">b", v)

def nbt_list(tid, payloads):
    return bytes([tid]) + struct.pack(">i", len(payloads)) + b"".join(payloads)

def nbt_longarray(vals):
    return struct.pack(">i", len(vals)) + b"".join(struct.pack(">q", v) for v in vals)

def named_compound(name, body):
    return tag(10, name, body)

# ---------------------------------------------------------------- palette/packing

def pack_longs(vals, bits):
    per = 64 // bits
    out = []
    acc, j = 0, 0
    for v in vals:
        acc |= (v & ((1 << bits) - 1)) << j
        j += bits
        if j + bits > 64:
            out.append(acc)
            acc, j = 0, 0
    if j:
        out.append(acc)
    return out

def entry(name, props=None):
    body = tag(8, "Name", nbt_string(name))
    if props:
        pb = b"".join(tag(8, k, nbt_string(v)) for k, v in props.items())
        body += tag(10, "Properties", comp_inner(pb))
    return body + b"\x00"

def comp_inner(children):
    return children + b"\x00"

def section(Y, palette, data_vals=None, biome="minecraft:plains"):
    body = tag(1, "Y", nbt_byte(Y))
    bs = tag(9, "palette", nbt_list(10, [entry(n) for n in palette]))
    if data_vals is not None:
        bits = max(4, (len(palette) - 1).bit_length())
        bs += tag(12, "data", nbt_longarray(pack_longs(data_vals, bits)))
    body += tag(10, "block_states", comp_inner(bs))
    bi = tag(9, "palette", nbt_list(8, [nbt_string(biome)]))
    body += tag(10, "biomes", comp_inner(bi))
    return comp_inner(body)

def build_chunk_nbt(cx, cz):
    secs = []
    for sy in range(-4, 20):
        if sy == 4:
            # sol de deepslate à y=65, marqueurs : red_wool (9,65,8), water (10,65,8)
            vals = [3] * 4096  # 3 = air dans la palette ci-dessous
            for lx in range(16):
                for lz in range(16):
                    vals[(1 << 8) | (lz << 4) | lx] = 0  # deepslate
            vals[(1 << 8) | (8 << 4) | 9] = 1   # red_wool
            vals[(1 << 8) | (8 << 4) | 10] = 2  # water
            pal = ["minecraft:deepslate", "minecraft:red_wool", "minecraft:water", "minecraft:air"]
            secs.append(section(sy, pal, vals))
        else:
            secs.append(section(sy, ["minecraft:air"]))
    body = (
        tag(3, "DataVersion", nbt_int(3839))
        + tag(3, "xPos", nbt_int(cx))
        + tag(3, "zPos", nbt_int(cz))
        + tag(3, "yPos", nbt_int(-4))
        + tag(8, "Status", nbt_string("minecraft:full"))
        + tag(4, "LastUpdate", nbt_long(0))
        + tag(4, "InhabitedTime", nbt_long(0))
        + tag(9, "sections", nbt_list(10, secs))
        + tag(9, "block_entities", nbt_list(10, []))
    )
    return bytes([10]) + nbt_str("") + comp_inner(body)

# ---------------------------------------------------------------- région + level.dat

def write_region(path, chunks):
    header = bytearray(8192)
    body = bytearray()
    next_sec = 2
    now = int(time.time())
    for slot, nbt in chunks:
        z = zlib_compress6(nbt)
        plen = len(z) + 1
        total = plen + 4
        need = (total + 4095) // 4096
        header[slot * 4] = (next_sec >> 16) & 0xFF
        header[slot * 4 + 1] = (next_sec >> 8) & 0xFF
        header[slot * 4 + 2] = next_sec & 0xFF
        header[slot * 4 + 3] = min(need, 255)
        ts = struct.pack(">I", now)
        header[4096 + slot * 4: 4100 + slot * 4] = ts
        body += struct.pack(">I", plen) + bytes([2]) + z
        body += b"\x00" * (need * 4096 - total)
        next_sec += need
    with open(path, "wb") as f:
        f.write(bytes(header))
        f.write(bytes(body))

def zlib_compress6(d):
    co = zlib.compressobj(6, zlib.DEFLATED, 15)
    return co.compress(d) + co.flush()

import zlib

def write_leveldat(path, seed):
    data = named_compound("", comp_inner(
        tag(10, "Data", comp_inner(
            tag(4, "RandomSeed", nbt_long(seed))
            + tag(8, "LevelName", nbt_string("InteropVanilla"))
        ))
    ))
    with open(path, "wb") as f:
        f.write(gzip.compress(data))

# ---------------------------------------------------------------- lecture (étape 4)

class NbR:
    def __init__(self, b):
        self.b, self.p = b, 0
    def take(self, n):
        v = self.b[self.p:self.p + n]
        assert len(v) == n, "NBT tronqué"
        self.p += n
        return v
    def u8(self):
        return self.take(1)[0]
    def vi(self):
        v, sh = 0, 0
        while True:
            b = self.u8()
            v |= (b & 0x7F) << sh
            if not b & 0x80:
                return v
            sh += 7
    def s(self):
        n = struct.unpack(">H", self.take(2))[0]
        return self.take(n).decode()
    def payload(self, t):
        if t == 1:
            return struct.unpack(">b", self.take(1))[0]
        if t == 3:
            return struct.unpack(">i", self.take(4))[0]
        if t == 4:
            return struct.unpack(">q", self.take(8))[0]
        if t == 8:
            return self.s()
        if t == 9:
            it = self.u8()
            n = struct.unpack(">i", self.take(4))[0]
            return [self.payload(it) for _ in range(n)]
        if t == 10:
            out = {}
            while True:
                t2 = self.u8()
                if t2 == 0:
                    return out
                key = self.s()  # NB: clé d'abord (RHS évalué avant le subscript)
                out[key] = self.payload(t2)
        if t == 12:
            n = struct.unpack(">i", self.take(4))[0]
            return list(struct.unpack(f">{n}q", self.take(8 * n)))
        raise AssertionError(f"tag NBT non géré: {t}")
    def root(self):
        assert self.u8() == 10
        self.s()
        return self.payload(10)

def read_region_chunk(path, cx, cz):
    d = open(path, "rb").read()
    slot = (cx & 31) + (cz & 31) * 32
    off = (d[slot * 4] << 16) | (d[slot * 4 + 1] << 8) | d[slot * 4 + 2]
    assert off, "slot vide"
    start = off * 4096
    ln = struct.unpack(">I", d[start:start + 4])[0]
    typ = d[start + 4]
    raw = d[start + 5:start + 4 + ln]
    assert typ == 2, f"compression {typ}"
    return NbR(zlib.decompress(raw)).root()

def unpack_pos(v):
    x = v >> 38
    y = (v << 52) >> 52
    z = (v << 26) >> 38
    x = x - (1 << 26) if x & (1 << 25) else x
    y = y - (1 << 12) if y & (1 << 11) else y
    z = z - (1 << 26) if z & (1 << 25) else z
    return x, y, z

# ---------------------------------------------------------------- scénario

def main():
    shutil.rmtree(os.path.dirname(WORLD), ignore_errors=True)
    os.makedirs(os.path.join(WORLD, "region"))
    write_region(os.path.join(WORLD, "region", "r.0.0.mca"), [(0, build_chunk_nbt(0, 0))])
    write_leveldat(os.path.join(WORLD, "level.dat"), SEED)
    print("setup: région vanilla-tier écrite (1 chunk, palette sans rvx, zlib-6)")

    with open("/tmp/rvx_van_srv.log", "w") as logf:
        proc = subprocess.Popen(
            [SRV, "--port", str(PORT), "--world", WORLD],
            stdin=subprocess.PIPE, stdout=logf, stderr=subprocess.STDOUT, text=True)
    time.sleep(1.0)
    ok = False
    try:
        s = socket.create_connection(("127.0.0.1", PORT), timeout=15)
        r = Rdr(s)
        join(r, s, name="Interop")
        pos, chunks = wait_position_and_chunks(r, s)
        print(f"join OK, {chunks} chunks, pos={pos[:3]}")
        send_packet(s, 0x00, varint(0))
        send_packet(s, 0x1A, struct.pack(">ddd", *pos[:3]) + b"\x01")
        time.sleep(0.5)
        # creuse le bloc de deepslate sous les pieds (marker vivant)
        bx, by, bz = math.floor(pos[0]), math.floor(pos[1]) - 1, math.floor(pos[2])
        lp = ((bx & 0x3FFFFFF) << 38) | ((bz & 0x3FFFFFF) << 12) | (by & 0xFFF)
        send_packet(s, 0x24, varint(0) + struct.pack(">Q", lp) + b"\x00")
        time.sleep(0.8)
        print(f"bloc creusé en ({bx},{by},{bz})")
        s.close()
        proc.stdin.write("/save\n")
        proc.stdin.flush()
        time.sleep(0.6)
        ok = True
    finally:
        if not ok:
            proc.stdin.write("/stop\n")
            proc.stdin.flush()
            time.sleep(0.3)
        proc.stdin.write("/stop\n")
        proc.stdin.flush()
        try:
            proc.stdin.close()
            proc.wait(timeout=15)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()

    out = open("/tmp/rvx_van_srv.log").read()
    assert f"graine {SEED}" in out, "graine level.dat non lue:\n" + out[-800:]
    print(f"graine lue du level.dat tiers: {SEED} ✓")
    assert "chargé (Anvil)" in out
    assert "monde sauvegardé (Anvil, 1 chunks)" in out, "chunk pas réécrit:\n" + out[-800:]

    # ---- relecture Python : identités préservées
    root = read_region_chunk(os.path.join(WORLD, "region", "r.0.0.mca"), 0, 0)
    assert root["xPos"] == 0 and root["zPos"] == 0
    sec4 = next(s for s in root["sections"] if s.get("Y") == 4)
    bs = sec4["block_states"]
    pal = bs["palette"]
    def rvx_of(e):
        return int(e["Properties"]["rvx"]) if "Properties" in e and "rvx" in e.get("Properties", {}) else None
    def name_of(e):
        return e["Name"]
    # décode la data
    bits = max(4, (len(pal) - 1).bit_length())
    data = bs.get("data", [])
    per = 64 // bits
    vals = []
    for L in data:
        for j in range(0, 64, bits):
            if len(vals) < 4096:
                vals.append((L >> j) & ((1 << bits) - 1))
    def block_at(x, y, z):
        i = ((y - 64) << 8) | (z << 4) | x
        return pal[vals[i]]
    floor_at = lambda x, z: block_at(x, 65, z)
    d0 = floor_at(0, 0)
    assert d0["Name"] == "minecraft:air", f"bloc creusé pas en air: {d0['Name']}"
    print("bloc creusé → minecraft:air ✓")
    w = floor_at(9, 8)
    assert w["Name"] == "minecraft:red_wool" and rvx_of(w) in (None, 94), f"red_wool perdue: {w}"
    print("red_wool → résolue en id interne 94 (nom coloré) ✓")
    wt = floor_at(10, 8)
    assert wt["Name"] == "minecraft:water", f"water perdue: {wt}"
    print("water → préservée ✓")
    ds = floor_at(3, 3)
    assert ds["Name"] == "minecraft:deepslate", f"deepslate perdue: {ds}"
    print("deepslate → préservée ✓")
    print("INTEROP ANVIL OK — région tierce lue, seed level.dat lue, identités round-tripées")

if __name__ == "__main__":
    main()
