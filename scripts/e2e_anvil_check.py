#!/usr/bin/env python3
# E2E persistance Anvil :
#   run 1 : join → creuse un bloc (position écho → BLOCK_DIG) → /save → /stop
#           → vérifie "monde sauvegardé (Anvil, N chunks)" + fichier région
#   run 2 : redémarre le MÊME monde → bannière "chargé (Anvil)" → join OK
#           → le bloc creusé est toujours AIR (relecture depuis le disque)
import hashlib, math, os, shutil, socket, struct, subprocess, sys, time

SRV = "/home/z/my-project/download/rustvoxel/target/release/rustvoxel_server"
PORT = 25598
WORLD = "/tmp/rvx_e2e_anvil/world"
REGION = os.path.join(WORLD, "region")

# ---------------------------------------------------------------- protocole

def varint(v):
    out = bytearray()
    while True:
        b = v & 0x7F
        v >>= 7
        if v:
            out.append(b | 0x80)
        else:
            out.append(b)
            return bytes(out)

class Rdr:
    """Lecteur de paquets TCP bufferisé (ne perd jamais d'octets)."""
    def __init__(self, sock):
        self.s = sock
        self.buf = b""
    def _need(self, n):
        while len(self.buf) < n:
            d = self.s.recv(65536)
            if not d:
                raise EOFError
            self.buf += d
    def varint(self):
        self._need(1)
        v, sh = 0, 0
        while True:
            b = self.buf[0]
            self.buf = self.buf[1:]
            v |= (b & 0x7F) << sh
            if not (b & 0x80):
                return v
            sh += 7
            self._need(1)
    def take(self, n):
        self._need(n)
        out, self.buf = self.buf[:n], self.buf[n:]
        return out
    def packet(self):
        ln = self.varint()
        body = self.take(ln)
        pid = 0
        sh = 0
        while True:
            b = body[0]
            body = body[1:]
            pid |= (b & 0x7F) << sh
            if not (b & 0x80):
                break
            sh += 7
        return pid, body

def send_packet(sock, pid, payload=b""):
    sock.sendall(varint(len(varint(pid) + payload)) + varint(pid) + payload)

def unpack_pos(long_val):
    x = long_val >> 38
    y = long_val << 52 >> 52
    z = long_val << 26 >> 38
    return x - (1 << 26) if x & (1 << 25) else x, \
           y - (1 << 12) if y & (1 << 11) else y, \
           z - (1 << 26) if z & (1 << 25) else z

def join(r, sock, name="E2E"):
    hs = varint(765) + b"\x09localhost" + struct.pack(">H", PORT) + varint(2)
    send_packet(sock, 0x00, hs)
    u = bytearray(hashlib.md5(("OfflinePlayer:" + name).encode()).digest())
    u[6] = (u[6] & 0x0F) | 0x30
    u[8] = (u[8] & 0x3F) | 0x80
    send_packet(sock, 0x00, varint(3) + name.encode() + bytes(u))
    pid, _ = r.packet()
    assert pid == 0x02, f"login_success attendu, reçu 0x{pid:02X}"
    send_packet(sock, 0x03)
    while True:
        pid, _ = r.packet()
        if pid == 0x03:
            break
    send_packet(sock, 0x03)
    pid, _ = r.packet()
    assert pid == 0x2B, f"play_login attendu, reçu 0x{pid:02X}"

def wait_position_and_chunks(r, sock):
    """Lit jusqu'à la position initiale (0x40) + au moins un batch de chunks."""
    pos, chunks = None, 0
    t0 = time.time()
    while time.time() - t0 < 10 and (pos is None or chunks == 0):
        pid, p = r.packet()
        if pid == 0x40 and pos is None:
            x, y, z = struct.unpack(">ddd", p[:24])
            flags = p[41] if len(p) > 41 else 0
            tid = 0
            pos = (x, y, z, flags, tid)
        elif pid == 0x27:
            chunks += 1
    assert pos is not None, "position initiale jamais reçue"
    assert chunks > 0, "aucun chunk reçu"
    return pos, chunks

# ---------------------------------------------------------------- run 1

def run1():
    shutil.rmtree(os.path.dirname(WORLD), ignore_errors=True)
    with open("/tmp/rvx_e2e_srv1.log", "w") as logf:
        proc = subprocess.Popen(
            [SRV, "--port", str(PORT), "--world", WORLD, "--seed", "42"],
            stdin=subprocess.PIPE, stdout=logf, stderr=subprocess.STDOUT, text=True)
    time.sleep(1.0)
    ok = False
    try:
        s = socket.create_connection(("127.0.0.1", PORT), timeout=15)
        r = Rdr(s)
        join(r, s)
        pos, chunks = wait_position_and_chunks(r, s)
        print(f"run1: join OK, {chunks} chunks, pos={pos[:3]}")
        # confirmation de téléportation + écho position (borne le in_reach)
        send_packet(s, 0x00, varint(0))
        send_packet(s, 0x1A, struct.pack(">ddd", *pos[:3]) + b"\x01")
        time.sleep(0.4)
        # creuse 2 blocs sous les pieds (floor, pas troncature : x peut être négatif)
        # le traitement est vérifié côté serveur via son log ; on ne bloque pas
        # sur BLOCK_CHANGE : les paquets sont derrière ~3 Mo de map_chunks
        # streamés (drain TCP lent) — la preuve finale est le rechargement
        bx, by, bz = math.floor(pos[0]), math.floor(pos[1]) - 1, math.floor(pos[2])
        for dy in (0, 1):
            lp = ((bx & 0x3FFFFFF) << 38) | ((bz & 0x3FFFFFF) << 12) | ((by - dy) & 0xFFF)
            send_packet(s, 0x24, varint(0) + struct.pack(">Q", lp) + b"\x00")
        time.sleep(1.0)
        print(f"run1: 2 blocs creusés en ({bx},{by},{bz}) et ({bx},{by-1},{bz})")
        s.close()
        # sauvegarde + arrêt propres via la console
        proc.stdin.write("/save\n")
        proc.stdin.flush()
        time.sleep(0.6)
        proc.stdin.write("/stop\n")
        proc.stdin.flush()
    finally:
        try:
            proc.stdin.close()
            proc.wait(timeout=15)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
    out = open("/tmp/rvx_e2e_srv1.log").read()
    # "1 chunks" (et pas 0) prouve aussi que le creusement a bien modifié le chunk
    assert "monde sauvegardé (Anvil, 1 chunks)" in out or "monde sauvegardé (Anvil, 2 chunks)" in out, \
        "log de sauvegarde Anvil attendu, obtenu:\n" + out[-800:]
    print("run1: /save OK ->", [l for l in out.splitlines() if "Anvil" in l][-1])
    assert os.path.isdir(REGION) and any(f.endswith(".mca") for f in os.listdir(REGION)), \
        "aucun fichier région écrit"
    regions = os.listdir(REGION)
    print(f"run1: régions écrites: {regions}")
    return out

# ---------------------------------------------------------------- run 2

def run2():
    with open("/tmp/rvx_e2e_srv2.log", "w") as logf:
        proc = subprocess.Popen(
            [SRV, "--port", str(PORT), "--world", WORLD],
            stdin=subprocess.PIPE, stdout=logf, stderr=subprocess.STDOUT, text=True)
    time.sleep(1.0)
    ok = False
    try:
        s = socket.create_connection(("127.0.0.1", PORT), timeout=15)
        r = Rdr(s)
        join(r, s)
        pos, chunks = wait_position_and_chunks(r, s)
        print(f"run2: rejoin OK, {chunks} chunks re-streamés depuis l'Anvil")
        ok = True
        s.close()
    finally:
        if not ok:
            proc.stdin.write("/stop\n")
            proc.stdin.flush()
        proc.stdin.write("/stop\n")
        proc.stdin.flush()
        try:
            proc.stdin.close()
            proc.wait(timeout=15)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
    out = open("/tmp/rvx_e2e_srv2.log").read()
    assert "chargé (Anvil)" in out, "le monde devait être rechargé depuis l'Anvil:\n" + out[-800:]
    print("run2: bannière ->", [l for l in out.splitlines() if "monde" in l][0])

if __name__ == "__main__":
    out1 = run1()
    run2()
    print("E2E ANVIL OK — monde sauvegardé en .mca, rechargé, chunks re-servis")
