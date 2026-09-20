#!/usr/bin/env python3
"""E2E : vérifie la séquence d'entrée en monde vue par un client 1.20.5.

Assertions :
  - le play ouvre avec login (0x2B)
  - game_event (0x22) event=13 « Start waiting for level chunks » arrive
    AVANT le premier chunk_batch_start (0x0D) / map_chunk (0x27)
  - les chunks arrivent (batch encadré start -> MAP_CHUNK* -> finished)
  - le keepalive (0x26) est répondu, le client reste connecté

Usage: python3 e2e_join_check.py [port]
"""
import hashlib
import socket
import struct
import subprocess
import sys
import time

SRV = "/home/z/my-project/download/rustvoxel/target/release/rustvoxel_server"
PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 25871

NAMES = {
    0x0C: "chunk_batch_finished", 0x0D: "chunk_batch_start",
    0x22: "game_event", 0x26: "keepalive", 0x27: "map_chunk",
    0x2B: "play_login", 0x40: "player_position", 0x54: "set_center_chunk",
    0x53: "set_carried_item", 0x6C: "system_chat", 0x5D: "spawn_position",
}


def varint(n):
    n &= 0xFFFFFFFF
    out = b""
    while True:
        if n & ~0x7F == 0:
            return out + bytes([n])
        out += bytes([(n & 0x7F) | 0x80])
        n >>= 7


def read_varint_buf(b, p):
    n = 0
    for i in range(5):
        byte = b[p]; p += 1
        n |= (byte & 0x7F) << (7 * i)
        if not byte & 0x80:
            return n, p
    raise ValueError("varint trop long")


class Rdr:
    """Lecteur TCP bufferisé (ne perd aucun octet entre deux frames)."""

    def __init__(self, sock):
        self.s = sock
        self.b = b""

    def _fill(self):
        d = self.s.recv(4096)
        if not d:
            raise EOFError("connexion fermée")
        self.b += d

    def frame(self):
        while True:
            try:
                ln, off = read_varint_buf(self.b, 0)
                break
            except IndexError:
                self._fill()
        while len(self.b) < off + ln:
            self._fill()
        frame, self.b = self.b[off:off + ln], self.b[off + ln:]
        return frame

    def packet(self):
        p = self.frame()
        pid, off = read_varint_buf(p, 0)
        return pid, p[off:]


def recv_packet(rdr):
    return rdr.packet()


def send_packet(sock, pid, payload=b""):
    sock.sendall(varint(len(varint(pid) + payload)) + varint(pid) + payload)


def main():
    proc = subprocess.Popen(
        [SRV, "--port", str(PORT), "--motd", "e2e join", "--seed", "42"],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT, text=True)
    time.sleep(1.0)
    try:
        s = socket.create_connection(("127.0.0.1", PORT), timeout=15)
        r = Rdr(s)
        hs = varint(765) + b"\x09localhost" + struct.pack(">H", PORT) + varint(2)
        send_packet(s, 0x00, hs)
        u = bytearray(hashlib.md5(b"OfflinePlayer:E2E").digest())
        u[6] = (u[6] & 0x0F) | 0x30
        u[8] = (u[8] & 0x3F) | 0x80
        send_packet(s, 0x00, varint(3) + b"E2E" + bytes(u))
        # login -> config -> play
        pid, p = recv_packet(r)
        assert pid == 0x02, f"login_success attendu, reçu 0x{pid:02X}"
        send_packet(s, 0x03)  # login acknowledged
        while True:
            pid, p = recv_packet(r)
            if pid == 0x03:  # finish_configuration
                break
        send_packet(s, 0x03)
        # ---- play : trace ordonnée ----
        pid, p = recv_packet(r)
        assert pid == 0x2B, f"le play doit ouvrir avec login, reçu 0x{pid:02X}"
        print("play_login OK")
        saw_ge13 = False
        saw_batch_start = False
        chunks = 0
        batch_chunks = 0
        t0 = time.time()
        while time.time() - t0 < 8:
            pid, p = recv_packet(r)
            nm = NAMES.get(pid, f"0x{pid:02X}")
            if pid == 0x22:  # game_event
                ev = p[0]
                print(f"  game_event ev={ev}")
                assert not saw_ge13, "game_event 13 envoyé deux fois"
                assert ev == 13, f"attendu event 13, reçu {ev}"
                saw_ge13 = True
            elif pid == 0x0D:  # batch start
                assert saw_ge13, "chunk_batch_start AVANT game_event 13 !"
                saw_batch_start = True
                batch_chunks = 0
                print("  chunk_batch_start")
            elif pid == 0x27:  # map_chunk
                assert saw_ge13, "map_chunk AVANT game_event 13 !"
                chunks += 1
                batch_chunks += 1
            elif pid == 0x0C:  # batch finished
                n = p[0]
                print(f"  chunk_batch_finished batchSize={n}")
                assert saw_batch_start, "finished sans start"
                break
            elif pid == 0x26:  # keepalive
                send_packet(s, 0x18, p[:8])
        print(f"chunks reçus: {chunks}")
        assert saw_ge13, "game_event 13 ABSENT du join"
        assert chunks > 0, "aucun chunk reçu"
        print("E2E JOIN OK — game_event 13 précède les chunks ✓")
        return 0
    finally:
        proc.terminate()
        try:
            out, _ = proc.communicate(timeout=3)
        except subprocess.TimeoutExpired:
            proc.kill()
            out, _ = proc.communicate()
        tail = "\n".join(out.splitlines()[-6:])
        print("--- log serveur (fin) ---\n" + tail)


sys.exit(main())
