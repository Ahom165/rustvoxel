// Client multiplayers RustVoxel — parle le PROTOCOLE VANILLA Minecraft Java
// 1.20.5 (protocole 766), le même que le serveur dédié. Le solo passe par le
// serveur intégré en loopback avec exactement le même chemin réseau.
//
// Connexion (bloquante) : handshake -> login -> configuration -> play, puis
// la socket passe en non-bloquant et pump()/tick() prennent le relais.

use crate::math::Vec3;
use crate::mcproto::{self, cb, sb, R2, W2};
use crate::mobs::{Mob, MobKind};
use crate::player::Player;
use crate::vanilla_data as vd;
use crate::world::World;
use std::collections::{HashMap, VecDeque, HashSet};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

pub struct Ghost {
    pub kind: MobKind,
    pub prev: Vec3,
    pub cur: Vec3,
    pub yaw_prev: f32,
    pub yaw: f32,
    pub flags: u8,
    pub extra: u8,
    pub anim: f32,
    pub moving: bool,
    pub last_update: Instant,
}

impl Ghost {
    /// Position interpolée pour le rendu (snap sur les téléportations).
    pub fn lerped(&self) -> (Vec3, f32) {
        let t = ((self.last_update.elapsed().as_secs_f32()) / 0.1).clamp(0.0, 1.0);
        let mut d = self.yaw - self.yaw_prev;
        if d > std::f32::consts::PI {
            d -= std::f32::consts::TAU;
        }
        if d < -std::f32::consts::PI {
            d += std::f32::consts::TAU;
        }
        (
            self.prev + (self.cur - self.prev) * t,
            self.yaw_prev + d * t,
        )
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Phase {
    Connecting,
    Downloading,
    Playing,
}

#[derive(Clone)]
pub enum NetEvent {
    Break(Vec3, u16),
    Explosion(Vec3),
    HurtMob(Vec3),
    Eat(Vec3),
}

pub struct Client {
    pub stream: TcpStream,
    buf: Vec<u8>,
    /// paquets configurés en attente pendant la phase de login bloquante
    backlog: Vec<(i32, Vec<u8>)>,
    pub id: u32,
    pub world: World,
    pub me: Player,
    pub name: String,
    pub creative: bool,
    pub players: Vec<(u32, String)>,
    pub ghosts: HashMap<u32, Ghost>,
    pub chat: VecDeque<(Instant, String)>,
    pub hp: i32,
    pub hunger: i32,
    pub air: i32,
    pub xp: u32,
    pub level: u32,
    pub dead: bool,
    pub ping_ms: u32,
    pub kick: Option<String>,
    pub phase: Phase,
    pub spawn: Vec3,
    pub day_t: f32,
    pub events: Vec<NetEvent>,
    pub chunks_needed: usize,
    pub chunks_have: usize,
    last_pos: Instant,
    slot: usize,
    uuid_names: HashMap<[u8; 16], String>,
    state_to_block: HashMap<u32, u16>,
    spawned_players: HashSet<u32>,
}

fn write_retry(s: &mut TcpStream, mut buf: &[u8]) -> std::io::Result<()> {
    while !buf.is_empty() {
        match s.write(buf) {
            Ok(0) => return Err(std::io::Error::new(std::io::ErrorKind::WriteZero, "write 0")),
            Ok(n) => buf = &buf[n..],
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Carte inverse état vanilla -> bloc rustvoxel (première occurrence gagne).
fn build_state_map() -> HashMap<u32, u16> {
    let mut m = HashMap::new();
    for (b, s) in vd::BLOCK_STATES {
        m.entry(s).or_insert(b);
    }
    m
}

impl Client {
    pub fn connect(addr: &str, name: &str) -> std::io::Result<Client> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true).ok();
        stream.set_read_timeout(Some(Duration::from_secs(15))).ok();
        let mut c = Client {
            stream,
            buf: Vec::new(),
            backlog: Vec::new(),
            id: 0,
            world: World::new(0),
            me: Player::new(),
            name: name.to_string(),
            creative: true,
            players: Vec::new(),
            ghosts: HashMap::new(),
            chat: VecDeque::new(),
            hp: 20,
            hunger: 20,
            air: 10,
            xp: 0,
            level: 0,
            dead: false,
            ping_ms: 0,
            kick: None,
            phase: Phase::Connecting,
            spawn: Vec3::new(8.5, 80.0, 8.5),
            day_t: 0.06,
            events: Vec::new(),
            chunks_needed: 25,
            chunks_have: 0,
            last_pos: Instant::now(),
            slot: 0,
            uuid_names: HashMap::new(),
            state_to_block: build_state_map(),
            spawned_players: HashSet::new(),
        };
        c.login(name)?;
        // socket non-bloquante pour la boucle de jeu
        c.stream.set_nonblocking(true).ok();
        Ok(c)
    }

    /// Handshake + login + configuration (bloquant), s'arrête avant le play.
    fn login(&mut self, name: &str) -> std::io::Result<()> {
        // handshake : nextState 2 = login
        let mut h = Vec::new();
        mcproto::write_varint(&mut h, 0x00);
        mcproto::write_varint(&mut h, vd::PROTOCOL_VERSION);
        mcproto::write_string(&mut h, "rustvoxel");
        h.extend_from_slice(&25565u16.to_be_bytes());
        mcproto::write_varint(&mut h, 2);
        mcproto::write_frame(&mut self.stream, &h)?;
        // login start : nom + uuid offline
        let uuid = mcproto::offline_uuid(name);
        let mut p = Vec::new();
        mcproto::write_varint(&mut p, sb::LOGIN_START);
        mcproto::write_string(&mut p, &self.name);
        p.extend_from_slice(&uuid);
        mcproto::write_frame(&mut self.stream, &p)?;

        // login_success -> login_ack -> configuration
        loop {
            let r = mcproto::read_frame(&mut self.stream)?;
            let mut rr = R2::new(&r);
            let pid = rr.vi().unwrap_or(-1);
            match pid {
                cb::LOGIN_SUCCESS => { /* ok */ }
                cb::LOGIN_DISCONNECT => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        "refusé par le serveur",
                    ));
                }
                sb::LOGIN_ACK => unreachable!(),
                _ => {}
            }
            if pid == cb::LOGIN_SUCCESS {
                break;
            }
        }
        let mut a = Vec::new();
        mcproto::write_varint(&mut a, sb::LOGIN_ACK);
        mcproto::write_frame(&mut self.stream, &a)?;
        // configuration : client_information + brand, registres, finish
        {
            let ci = W2::new()
                .s("fr_fr")
                .i8v(8)
                .vi(0)
                .b(false)
                .u8v(0x7F)
                .vi(1)
                .b(false)
                .b(true)
                .done();
            let mut body = Vec::new();
            mcproto::write_varint(&mut body, sb::CFG_SETTINGS);
            body.extend_from_slice(&ci);
            mcproto::write_frame(&mut self.stream, &body)?;
            let brand = W2::new().s("minecraft:brand").s("rustvoxel").done();
            let mut body = Vec::new();
            mcproto::write_varint(&mut body, sb::CFG_PLUGIN_MESSAGE);
            body.extend_from_slice(&brand);
            mcproto::write_frame(&mut self.stream, &body)?;
        }
        loop {
            let r = mcproto::read_frame(&mut self.stream)?;
            let mut rr = R2::new(&r);
            let pid = rr.vi().unwrap_or(-1);
            match pid {
                cb::CFG_REGISTRY_DATA => { /* registres acceptés tels quels */ }
                cb::CFG_KEEP_ALIVE => {
                    let t = rr.i64v().unwrap_or(0);
                    let mut body = Vec::new();
                    mcproto::write_varint(&mut body, sb::CFG_KEEP_ALIVE);
                    body.extend_from_slice(&t.to_be_bytes());
                    mcproto::write_frame(&mut self.stream, &body)?;
                }
                cb::CFG_DISCONNECT => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        "refusé (configuration)",
                    ));
                }
                cb::CFG_FINISH => break,
                _ => {}
            }
        }
        let mut a = Vec::new();
        mcproto::write_varint(&mut a, sb::CFG_FINISH);
        mcproto::write_frame(&mut self.stream, &a)?;
        Ok(())
    }

    // ------------------------------------------------------------- envois

    pub fn say(&mut self, msg: &str) {
        if let Some(rest) = msg.strip_prefix('/') {
            self.send_packet(
                sb::CHAT_COMMAND,
                &W2::new().s(rest).done(),
            );
        } else {
            self.send_packet(
                sb::CHAT_MESSAGE,
                &W2::new()
                    .s(msg)
                    .i64v(0)
                    .i64v(0)
                    .b(false)
                    .vi(0)
                    .bytes(&[0; 3])
                    .done(),
            );
        }
    }

    pub fn send_dig(&mut self, x: i32, y: i32, z: i32) {
        self.send_packet(
            sb::BLOCK_DIG,
            &W2::new().vi(0).pos(x, y, z).i8v(0).vi(0).done(),
        );
    }

    /// Pose : synchronise l'item créatif du slot courant puis place.
    pub fn send_place(&mut self, x: i32, y: i32, z: i32, b: u16) {
        if let Some(item) = vd::item_id_for_block(b) {
            self.send_packet(
                sb::SET_CREATIVE_SLOT,
                &W2::new()
                    .i16v(36 + self.slot as i16)
                    .i8v(1)
                    .vi(item)
                    .vi(0)
                    .vi(0)
                    .done(),
            );
        }
        self.send_packet(
            sb::BLOCK_PLACE,
            &W2::new()
                .vi(0)
                .pos(x, y, z)
                .vi(1)
                .f32v(0.5)
                .f32v(0.5)
                .f32v(0.5)
                .b(false)
                .vi(0)
                .done(),
        );
    }

    /// Slot de hotbar sélectionné (molette / touches).
    pub fn set_held(&mut self, slot: usize) {
        self.slot = slot.min(8);
        self.send_packet(sb::HELD_ITEM_SLOT, &W2::new().i16v(self.slot as i16).done());
    }

    pub fn send_attack(&mut self, uid: u32) {
        self.send_packet(
            sb::USE_ENTITY,
            &W2::new().vi(uid as i32).vi(1).b(false).done(),
        );
    }

    pub fn send_eat(&mut self) {
        self.send_packet(sb::USE_ITEM, &W2::new().vi(0).vi(0).done());
    }

    pub fn send_respawn(&mut self) {
        self.send_packet(sb::CLIENT_COMMAND, &W2::new().vi(0).done());
    }

    pub fn send_packet(&mut self, pid: i32, payload: &[u8]) {
        // CORRECTION v0.7.2 : le paquet doit être ENCADRÉ (préfixe de longueur
        // varint) comme le fait write_frame pour le login. Sans ce préfixe, le
        // serveur lit le premier octet (l'id) comme une longueur de trame et
        // ferme la connexion au premier envoi du client (ex. teleport_confirm
        // dès l'entrée dans le monde) -> « échec de la connexion » en solo.
        let mut body = Vec::with_capacity(payload.len() + 10);
        mcproto::write_varint(&mut body, pid);
        body.extend_from_slice(payload);
        let mut framed = Vec::with_capacity(body.len() + 5);
        mcproto::write_varint(&mut framed, body.len() as i32);
        framed.extend_from_slice(&body);
        if let Err(e) = write_retry(&mut self.stream, &framed) {
            if self.kick.is_none() {
                self.kick = Some(format!("Connexion perdue ({e})"));
            }
        }
    }

    /// Mouvement 20 Hz (client autoritaire comme vanilla).
    pub fn tick(&mut self) {
        if self.last_pos.elapsed() >= Duration::from_millis(50) && self.phase == Phase::Playing {
            self.last_pos = Instant::now();
            let p = &self.me;
            let pid = if p.moving_look_changed() {
                sb::POSITION_LOOK
            } else {
                sb::POSITION
            };
            let mut w = W2::new()
                .f64v(p.pos.x as f64)
                .f64v(p.pos.y as f64)
                .f64v(p.pos.z as f64);
            if pid == sb::POSITION_LOOK {
                w = w
                    .f32v(mcproto::van_yaw_deg(p.yaw))
                    .f32v(mcproto::van_pitch(p.pitch));
            }
            w = w.b(p.on_ground);
            self.send_packet(pid, &w.done());
        }
    }

    /// Drain la socket (non-bloquant) et traite les paquets.
    pub fn pump(&mut self) {
        let mut tmp = [0u8; 65536];
        loop {
            match self.stream.read(&mut tmp) {
                Ok(0) => {
                    if self.kick.is_none() {
                        self.kick = Some("Connexion fermée par le serveur".into());
                    }
                    break;
                }
                Ok(n) => self.buf.extend_from_slice(&tmp[..n]),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    if self.kick.is_none() {
                        self.kick = Some(format!("Connexion perdue ({e})"));
                    }
                    break;
                }
            }
        }
        // extraire les paquets (longueur varint)
        loop {
            if self.buf.is_empty() {
                break;
            }
            // décode la longueur varint sur place
            let mut len: u32 = 0;
            let mut shift = 0;
            let mut hdr = 0usize;
            let mut ok = false;
            while hdr < self.buf.len() && hdr < 5 {
                let byte = self.buf[hdr];
                hdr += 1;
                len |= ((byte & 0x7F) as u32) << shift;
                if byte & 0x80 == 0 {
                    ok = true;
                    break;
                }
                shift += 7;
            }
            if !ok {
                break; // en-tête incomplet
            }
            if len == 0 || len as usize > mcproto::MAX_PACKET {
                self.kick = Some("Protocole invalide".into());
                break;
            }
            if self.buf.len() < hdr + len as usize {
                break;
            }
            let frame: Vec<u8> = self.buf[hdr..hdr + len as usize].to_vec();
            self.buf.drain(..hdr + len as usize);
            let mut rr = R2::new(&frame);
            let pid = rr.vi().unwrap_or(-1);
            let payload = frame[mcproto::varint_len(pid as u32)..].to_vec();
            self.handle(pid, &payload);
            if self.kick.is_some() {
                break;
            }
        }
        while let Some((t, _)) = self.chat.front() {
            if t.elapsed().as_secs_f64() > 60.0 {
                self.chat.pop_front();
            } else {
                break;
            }
        }
    }
}

impl Player {
    /// Le client vanilla envoie position_look quand la vue change.
    fn moving_look_changed(&self) -> bool {
        true // simplification : toujours avec rotation (coût négligeable)
    }
}

impl Client {
    fn handle(&mut self, pid: i32, payload: &[u8]) {
        // pendant la phase Connecting, bufferise jusqu'au paquet login
        if self.phase == Phase::Connecting && pid != cb::LOGIN && pid != cb::KICK_DISCONNECT {
            self.backlog.push((pid, payload.to_vec()));
            return;
        }
        let mut r = R2::new(payload);
        match pid {
            cb::LOGIN => {
                self.id = r.i32v().unwrap_or(0) as u32;
                let _hardcore = r.b().unwrap_or(false);
                let _worlds = r.vi().unwrap_or(0);
                for _ in 0.._worlds {
                    let _ = r.s(64);
                }
                let _max = r.vi();
                let _view = r.vi();
                let _sim = r.vi();
                let _hardcore2 = r.b();
                let _respawn_screen = r.b();
                let _limited = r.b();
                let _dim_type = r.vi();
                let _dim = r.s(64);
                let _seed = r.i64v();
                let gm = r.i8v().unwrap_or(1);
                self.creative = gm == 1;
                self.phase = Phase::Downloading;
                self.chunks_have = 0;
                // rejoue le backlog (envoyé avant le login, normalement vide)
                let backlog = std::mem::take(&mut self.backlog);
                for (p2, pl2) in backlog {
                    self.handle(p2, &pl2);
                }
            }
            cb::POSITION => {
                let x = r.f64v().unwrap_or(0.0) as f32;
                let y = r.f64v().unwrap_or(80.0) as f32;
                let z = r.f64v().unwrap_or(0.0) as f32;
                let yaw = r.f32v().unwrap_or(0.0);
                let pitch = r.f32v().unwrap_or(0.0);
                let _flags = r.u8v().unwrap_or(0);
                let tele_id = r.vi().unwrap_or(0);
                self.send_packet(sb::TELEPORT_CONFIRM, &W2::new().vi(tele_id).done());
                self.spawn = Vec3::new(x, y, z);
                self.me.pos = self.spawn;
                self.me.vel = Vec3::new(0.0, 0.0, 0.0);
                self.me.peak_y = y;
                self.me.yaw = mcproto::our_yaw(yaw);
                self.me.pitch = mcproto::our_pitch(pitch);
                if self.phase == Phase::Connecting {
                    self.phase = Phase::Downloading;
                }
            }
            cb::MAP_CHUNK => {
                let cx = r.i32v().unwrap_or(0);
                let cz = r.i32v().unwrap_or(0);
                // heightmaps NBT (skip)
                {
                    let mut nr = crate::nbt::NbtRdr::new(r.rest());
                    if nr.read_network().is_some() {
                        r.p += nr.pos();
                    }
                }
                let clen = r.vi().unwrap_or(0) as usize;
                // buffer exact (block entities + lumière suivent, ignorés ici)
                let end = (r.p + clen).min(r.b.len());
                let data = r.b[r.p..end].to_vec();
                self.decode_chunk(cx, cz, &data);
                self.chunks_have += 1;
                if self.phase == Phase::Downloading && self.chunks_have >= self.chunks_needed {
                    self.phase = Phase::Playing;
                    self.me.pos = self.spawn;
                    self.me.vel = Vec3::new(0.0, 0.0, 0.0);
                    self.me.peak_y = self.spawn.y;
                }
            }
            cb::UNLOAD_CHUNK => {
                let cz = r.i32v().unwrap_or(0);
                let cx = r.i32v().unwrap_or(0);
                self.world.chunks.remove(&(cx, cz));
            }
            cb::BLOCK_CHANGE => {
                let (x, y, z) = r.pos().unwrap_or((0, 0, 0));
                let state = r.vi().unwrap_or(0) as u32;
                let old = self.world.get_block(x, y, z);
                let nb = self.state_to_block.get(&state).copied().unwrap_or(crate::world::AIR);
                self.world.set_block(x, y, z, nb);
                if nb == crate::world::AIR && old != crate::world::AIR {
                    self.events.push(NetEvent::Break(
                        Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5),
                        old,
                    ));
                }
            }
            cb::SPAWN_ENTITY => {
                let eid = r.vi().unwrap_or(0) as u32;
                let uuid = r.uuid().unwrap_or([0; 16]);
                let etype = r.vi().unwrap_or(0);
                let x = r.f64v().unwrap_or(0.0) as f32;
                let y = r.f64v().unwrap_or(0.0) as f32;
                let z = r.f64v().unwrap_or(0.0) as f32;
                let _pitch = r.i8v().unwrap_or(0);
                let yaw = mcproto::i8_angle(r.u8v().unwrap_or(0));
                if eid == 0 {
                    return;
                }
                let kind = vd::mob_kind_from_entity_type(etype)
                    .and_then(MobKind::from_id)
                    .unwrap_or(MobKind::Zombie);
                if kind == MobKind::Player {
                    self.spawned_players.insert(eid);
                    if let Some(nm) = self.uuid_names.get(&uuid).cloned() {
                        if !self.players.iter().any(|(i, _)| *i == eid) {
                            self.players.push((eid, nm));
                        }
                    }
                }
                let pos = Vec3::new(x, y, z);
                let g = self.ghosts.entry(eid).or_insert_with(|| Ghost {
                    kind,
                    prev: pos,
                    cur: pos,
                    yaw_prev: mcproto::our_yaw(yaw),
                    yaw: mcproto::our_yaw(yaw),
                    flags: 0,
                    extra: 0,
                    anim: 0.0,
                    moving: false,
                    last_update: Instant::now(),
                });
                g.kind = kind;
                g.cur = pos;
                g.prev = pos;
                g.last_update = Instant::now();
            }
            cb::ENTITY_MOVE_LOOK | cb::REL_ENTITY_MOVE | cb::ENTITY_TELEPORT => {
                let eid = r.vi().unwrap_or(0) as u32;
                let (nx, ny, nz) = if pid == cb::ENTITY_TELEPORT {
                    let x = r.f64v().unwrap_or(0.0) as f32;
                    let y = r.f64v().unwrap_or(0.0) as f32;
                    let z = r.f64v().unwrap_or(0.0) as f32;
                    (x, y, z)
                } else {
                    let Some(g) = self.ghosts.get_mut(&eid) else { return };
                    let dx = r.i16v().unwrap_or(0) as f32 / 4096.0;
                    let dy = r.i16v().unwrap_or(0) as f32 / 4096.0;
                    let dz = r.i16v().unwrap_or(0) as f32 / 4096.0;
                    (g.cur.x + dx, g.cur.y + dy, g.cur.z + dz)
                };
                let yaw8 = r.u8v().unwrap_or(0);
                let _pitch8 = r.i8v().unwrap_or(0);
                let _ground = r.b().unwrap_or(true);
                self.update_ghost(eid, Vec3::new(nx, ny, nz), mcproto::our_yaw(mcproto::i8_angle(yaw8)));
            }
            cb::ENTITY_HEAD_ROTATION => {
                let _eid = r.vi().unwrap_or(0);
                let _head = r.i8v().unwrap_or(0);
            }
            cb::ENTITY_DESTROY => {
                let n = r.vi().unwrap_or(0);
                for _ in 0..n {
                    let eid = r.vi().unwrap_or(0) as u32;
                    self.ghosts.remove(&eid);
                    if self.spawned_players.remove(&eid) {
                        self.players.retain(|(i, _)| *i != eid);
                    }
                }
            }
            cb::KEEP_ALIVE => {
                let t = r.i64v().unwrap_or(0);
                self.send_packet(sb::KEEP_ALIVE, &t.to_be_bytes());
            }
            cb::UPDATE_TIME => {
                let _age = r.i64v().unwrap_or(0);
                let t = r.i64v().unwrap_or(6000);
                self.day_t = (t.rem_euclid(24000) as f32) / 24000.0;
            }
            cb::SYSTEM_CHAT => {
                let mut nr = crate::nbt::NbtRdr::new(r.rest());
                if let Some(nbt) = nr.read_network() {
                    let text = nbt.get("text").and_then(|t| t.as_str()).unwrap_or("");
                    if !text.is_empty() {
                        self.chat.push_back((Instant::now(), text.to_string()));
                        if self.chat.len() > 100 {
                            self.chat.pop_front();
                        }
                    }
                }
            }
            cb::PLAYER_INFO => {
                let actions = r.u8v().unwrap_or(0);
                let n = r.vi().unwrap_or(0);
                for _ in 0..n {
                    let uuid = r.uuid().unwrap_or([0; 16]);
                    if actions & 0x01 != 0 {
                        let name = r.s(16).unwrap_or_default();
                        let props = r.vi().unwrap_or(0);
                        for _ in 0..props {
                            let _ = r.s(64);
                            let _ = r.s(1024);
                            if r.b().unwrap_or(false) {
                                let _ = r.s(1024);
                            }
                        }
                        let _gm = r.vi();
                        let _ping = r.vi();
                        self.uuid_names.insert(uuid, name);
                    } else if actions & 0x04 != 0 {
                        let _gm = r.vi();
                    } else if actions & 0x08 != 0 {
                        let _listed = r.vi();
                    } else if actions & 0x10 != 0 {
                        let _lat = r.vi();
                    } else if actions & 0x20 != 0 {
                        // display name : option NBT
                        if r.b().unwrap_or(false) {
                            let mut nr = crate::nbt::NbtRdr::new(r.rest());
                            if let Some(_) = nr.read_network() {
                                r.p += nr.pos();
                            }
                        }
                    } else {
                        break; // inconnu : on ne peut pas continuer sainement
                    }
                }
            }
            cb::PLAYER_REMOVE => {
                let n = r.vi().unwrap_or(0);
                for _ in 0..n {
                    let uuid = r.uuid().unwrap_or([0; 16]);
                    self.uuid_names.remove(&uuid);
                }
            }
            cb::UPDATE_HEALTH => {
                let hp = r.f32v().unwrap_or(20.0);
                let food = r.vi().unwrap_or(20);
                let new_dead = hp <= 0.0;
                if hp < self.hp as f32 {
                    self.me.hurt_flash = self.me.hurt_flash.max(0.35);
                }
                if new_dead && !self.dead {
                    self.me.hurt_flash = 0.4;
                }
                self.hp = hp.round() as i32;
                self.hunger = food;
                self.me.hp = self.hp.max(0);
                self.dead = new_dead;
                self.me.dead = new_dead;
            }
            cb::EXPERIENCE => {
                let _bar = r.f32v().unwrap_or(0.0);
                self.level = r.vi().unwrap_or(0) as u32;
                self.xp = r.vi().unwrap_or(0) as u32;
            }
            cb::RESPAWN => {
                self.dead = false;
                self.me.dead = false;
                self.hp = 20;
                self.ghosts.clear();
                self.players.clear();
                self.spawned_players.clear();
                self.chunks_have = self.chunks_needed; // le monde reste chargé
            }
            cb::GAME_STATE_CHANGE => {
                let reason = r.u8v().unwrap_or(0);
                let val = r.f32v().unwrap_or(0.0);
                if reason == 3 {
                    self.creative = val as i32 == 1;
                }
            }
            cb::KICK_DISCONNECT => {
                let mut nr = crate::nbt::NbtRdr::new(r.rest());
                let reason = nr
                    .read_network()
                    .and_then(|n| n.get("text").and_then(|t| t.as_str()).map(String::from))
                    .unwrap_or_else(|| "expulsé".into());
                self.kick = Some(reason);
            }
            _ => {}
        }
    }

    fn update_ghost(&mut self, eid: u32, pos: Vec3, yaw: f32) {
        let Some(g) = self.ghosts.get_mut(&eid) else { return };
        if (g.cur - pos).length() > 8.0 {
            g.prev = pos; // téléport : snap
        } else {
            g.prev = g.cur;
            g.moving = (g.cur - pos).length() > 0.001;
        }
        g.cur = pos;
        g.yaw_prev = g.yaw;
        g.yaw = yaw;
        g.last_update = Instant::now();
    }

    /// Décode le blob chunkData (sections palettées) vers nos blocs.
    fn decode_chunk(&mut self, cx: i32, cz: i32, data: &[u8]) {
        let mut ch = crate::world::Chunk::new();
        let mut r = R2::new(data);
        for sy in 0..24usize {
            let y0 = sy as i32 * 16 - 64;
            let _count = r.i16v().unwrap_or(0);
            let in_world = y0 >= 0 && y0 + 15 < crate::world::CY as i32;
            if !self.decode_container(&mut r, &mut ch, y0, in_world) {
                return;
            }
            // biomes (ignorés : recalculés côté client)
            if !self.skip_biome_container(&mut r) {
                return;
            }
        }
        self.world.chunks.insert((cx, cz), ch);
    }

    fn decode_container(
        &mut self,
        r: &mut R2,
        ch: &mut crate::world::Chunk,
        y0: i32,
        in_world: bool,
    ) -> bool {
        let bits = r.vi().unwrap_or(-1);
        let mut entries: Vec<u32> = Vec::new();
        match bits {
            0 => {
                let v = r.vi().unwrap_or(0) as u32;
                let n = r.vi().unwrap_or(0);
                if n != 0 {
                    return false;
                }
                entries = vec![v; 4096];
            }
            4..=8 => {
                let plen = r.vi().unwrap_or(0) as usize;
                let mut pal = Vec::with_capacity(plen.min(256));
                for _ in 0..plen {
                    pal.push(r.vi().unwrap_or(0) as u32);
                }
                let nlongs = r.vi().unwrap_or(0) as usize;
                let per = 64 / bits as usize;
                let expected = (4096 + per - 1) / per;
                if nlongs != expected {
                    return false;
                }
                for i in 0..4096usize {
                    let li = i / per;
                    let off = i % per;
                    let base = r.p + li * 8;
                    let mut buf = [0u8; 8];
                    if base + 8 > r.b.len() {
                        return false;
                    }
                    buf.copy_from_slice(&r.b[base..base + 8]);
                    let l = i64::from_be_bytes(buf) as u64;
                    let v = ((l >> (off * bits as usize)) & ((1u64 << bits) - 1)) as u32;
                    let state = pal.get(v as usize).copied().unwrap_or(0);
                    entries.push(state);
                }
                r.p += nlongs * 8;
            }
            _ => {
                // direct : on ne décode pas les ids globaux (fallback air)
                return false;
            }
        }
        if in_world {
            for (i, st) in entries.iter().enumerate() {
                let b = self.state_to_block.get(st).copied().unwrap_or(crate::world::AIR);
                let lx = i & 15;
                let lz = (i >> 4) & 15;
                let ly = i >> 8;
                let wy = y0 + ly as i32;
                if wy >= 0 && wy < crate::world::CY as i32 {
                    ch.blocks[lx + 16 * (lz + 16 * wy as usize)] = b;
                }
            }
        }
        true
    }

    fn skip_biome_container(&mut self, r: &mut R2) -> bool {
        let bits = r.vi().unwrap_or(-1);
        match bits {
            0 => {
                let _v = r.vi();
                let n = r.vi().unwrap_or(0);
                n == 0
            }
            1..=8 => {
                let plen = r.vi().unwrap_or(0) as usize;
                for _ in 0..plen {
                    let _ = r.vi();
                }
                let nlongs = r.vi().unwrap_or(0) as usize;
                r.skip(nlongs * 8).is_some()
            }
            _ => false,
        }
    }

    /// Build the interpolated mob list for rendering (ghost rigs).
    pub fn ghost_mobs(&self) -> Vec<Mob> {
        let mut out = Vec::with_capacity(self.ghosts.len());
        for g in self.ghosts.values() {
            let (pos, yaw) = g.lerped();
            let mut m = Mob::new(g.kind, pos);
            m.yaw = yaw;
            m.moving = g.moving;
            m.anim = g.anim;
            m.hurt_t = if g.flags & 4 != 0 { 0.3 } else { 0.0 };
            m.burn_t = if g.flags & 16 != 0 { 0.5 } else { 0.0 };
            m.skin = g.extra;
            out.push(m);
        }
        out
    }

    /// Advance ghost animation phases (client-side clock).
    pub fn animate_ghosts(&mut self, dt: f32) {
        for g in self.ghosts.values_mut() {
            g.anim += dt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Client hors-ligne pour tester le décodage.
    fn new_offline() -> Client {
        Client {
            stream: TcpStream::connect("127.0.0.1:1").unwrap_or_else(|_| unsafe {
                // socket inutilisée : les tests ne l'emploient jamais
                std::mem::zeroed::<TcpStream>()
            }),
            buf: Vec::new(),
            backlog: Vec::new(),
            id: 0,
            world: World::new(7),
            me: Player::new(),
            name: "t".into(),
            creative: true,
            players: Vec::new(),
            ghosts: HashMap::new(),
            chat: VecDeque::new(),
            hp: 20,
            hunger: 20,
            air: 10,
            xp: 0,
            level: 0,
            dead: false,
            ping_ms: 0,
            kick: None,
            phase: Phase::Playing,
            spawn: Vec3::new(0.0, 80.0, 0.0),
            day_t: 0.25,
            events: Vec::new(),
            chunks_needed: 1,
            chunks_have: 0,
            last_pos: Instant::now(),
            slot: 0,
            uuid_names: HashMap::new(),
            state_to_block: build_state_map(),
            spawned_players: HashSet::new(),
        }
    }

    /// Le serveur encode un chunk -> le client le décode -> mêmes blocs.
    #[test]
    fn chunk_encode_decode_roundtrip() {
        let mut w = World::new(7);
        w.gen_chunk(0, 0);
        w.set_block(3, 60, 3, crate::world::TORCH);
        w.set_block(4, 60, 3, crate::world::CHERRY_PLANKS);
        let biomes = [0u8; 256];
        let (cdata, _light) = crate::server::encode_chunk_packet_data(&w, 0, 0, &biomes);
        let mut c = new_offline();
        c.decode_chunk(0, 0, &cdata);
        let back = c.world.chunks.get(&(0, 0)).unwrap();
        for (x, y, z) in [
            (3usize, 42usize, 3usize),
            (3, 60, 3),
            (4, 60, 3),
            (0, 41, 0),
            (15, 30, 15),
            (8, 20, 9),
        ] {
            let want = w.chunks.get(&(0, 0)).unwrap().blocks[x + 16 * (z + 16 * y)];
            let got = back.blocks[x + 16 * (z + 16 * y)];
            assert_eq!(got, want, "bloc ({x},{y},{z})");
        }
        assert_eq!(back.blocks[3 + 16 * (3 + 16 * 60)], crate::world::TORCH);
        assert_eq!(back.blocks[4 + 16 * (3 + 16 * 60)], crate::world::CHERRY_PLANKS);
    }
}
