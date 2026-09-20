// RustVoxel — serveur autoritaire multiplayers parlant le PROTOCOLE VANILLA
// Minecraft Java 1.20.5 (protocole 766), mode offline, zéro dépendance.
//
// Un client Java officiel 1.20.5/1.20.6 peut se connecter directement
// (0.0.0.0:25565 par défaut pour le binaire dédié). Le client RustVoxel utilise
// exactement le même protocole — le solo passe par un serveur intégré en loopback.
//
// Interopérabilité implémentée d'après la spécification publique du protocole
// (wiki.vg / minecraft.wiki, données machine de PrismarineJS/minecraft-data).
// Aucun code, asset ou contenu propriétaire de Mojang n'est inclus.

use crate::math::Vec3;
use crate::mcproto::{cb, sb, R2, W2};
use crate::mobs::{self, Mob, MobKind, MobTargets, Particle, Spawner};
use crate::nbt::Nbt;
use crate::noise;
use crate::player::{EYE_H, HALF_W};
use crate::vanilla_data as vd;
use crate::world::{is_cross_id, World, CY, WATER};
use std::collections::{HashMap, HashSet};
use std::io::Write as IoWrite;
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DAY_CYCLE: f32 = 480.0; // secondes par cycle jour+nuit
pub const TPS: u64 = 20;
pub const TICK_MS: u64 = 1000 / TPS;
pub const KEEPALIVE_S: u64 = 10;

#[derive(Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub max_players: usize,
    pub view: i32,             // chunks streamés autour de chaque joueur
    pub motd: String,
    pub seed: u64,
    pub world_path: PathBuf,
    pub save_interval_s: u64,
    pub verbose: bool,         // dédié: log stdout
    pub stop_when_empty: bool, // intégré: s'arrête quand le client part
}

impl ServerConfig {
    pub fn local(world_path: PathBuf, seed: Option<u64>) -> ServerConfig {
        ServerConfig {
            port: 0,
            max_players: 8,
            view: 7,
            motd: "Serveur local RustVoxel".into(),
            seed: seed.unwrap_or_else(noise::rand_seed),
            world_path,
            save_interval_s: 120,
            verbose: false,
            stop_when_empty: true,
        }
    }
}

pub struct PlayerS {
    pub id: u32,
    pub uuid: [u8; 16],
    pub name: String,
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub hp: i32,
    pub hunger: f32,
    pub air: f32,
    pub xp: u32,
    pub dead: bool,
    pub announced: bool,
    pub creative: bool,
    pub skin: u8,
    pub sprint: bool,
    pub invuln: f32,
    pub eat_cd: f32,
    pub regen_cd: f32,
    pub starve_cd: f32,
    pub out: Sender<Vec<u8>>,
    pub sent: HashSet<(i32, i32)>,
    pub health_sent: (i32, i32, u32),
    /// entités déjà spawnées chez ce client: uid -> dernière pos envoyée
    pub seen: HashMap<u32, (f32, f32, f32)>,
    /// hotbar côté serveur (items vanilla) posés via set_creative_slot
    pub hotbar: [u16; 9],
    pub held: usize,
    pub on_ground: bool,
    pub peak_y: f32,
    pub keepalive_sent: Option<(i64, Instant)>,
}

impl PlayerS {
    fn hurt(&mut self, dmg: i32, knock: Vec3, invuln: f32) -> bool {
        if self.dead || self.invuln > 0.0 || dmg <= 0 || self.creative {
            return false;
        }
        self.hp -= dmg;
        self.invuln = invuln;
        if self.hp <= 0 {
            self.hp = 0;
            self.dead = true;
        }
        let _ = knock;
        true
    }
    fn center(&self) -> Vec3 {
        self.pos + Vec3::new(0.0, EYE_H * 0.5, 0.0)
    }
}

pub struct Server {
    pub cfg: ServerConfig,
    pub world: World,
    pub players: HashMap<u32, PlayerS>,
    pub mobs: Vec<Mob>,
    pub particles: Vec<Particle>,
    pub spawner: Spawner,
    pub time: f32, // fraction du jour 0..1
    pub next_id: u32,
    pub next_uid: u32,
    pub stop_requested: bool,
    pub tick_n: u64,
    biome_cache: HashMap<(i32, i32), [u8; 256]>,
    spawn: Vec3,
    codec: Vec<Vec<u8>>, // packets registry_data pré-encodés (configuration)
    uuid_rng: u64,
}

// -------------------------------------------------------------- save / load
/// Sauve les chunks modifiés (format RVX2, compatible v0.5+).
pub fn save_world(w: &World, path: &std::path::Path) -> std::io::Result<()> {
    use std::io::BufWriter;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let f = std::fs::File::create(path)?;
    let mut bw = BufWriter::new(f);
    bw.write_all(b"RVX2")?;
    bw.write_all(&w.seed.to_le_bytes())?;
    let modified: Vec<(i32, i32)> = w
        .chunks
        .iter()
        .filter(|(_, c)| c.modified)
        .map(|(k, _)| *k)
        .collect();
    bw.write_all(&(modified.len() as u32).to_le_bytes())?;
    for (cx, cz) in modified {
        let c = &w.chunks[&(cx, cz)];
        bw.write_all(&cx.to_le_bytes())?;
        bw.write_all(&cz.to_le_bytes())?;
        bw.write_all(&crate::net::rle_encode(&c.blocks))?;
    }
    bw.flush()?;
    Ok(())
}

pub fn load_world(path: &std::path::Path) -> Option<World> {
    let data = std::fs::read(path).ok()?;
    let mut r: &[u8] = &data;
    let take = |n: usize, r: &mut &[u8]| -> Option<Vec<u8>> {
        if r.len() < n {
            return None;
        }
        let (a, b) = r.split_at(n);
        *r = b;
        Some(a.to_vec())
    };
    let magic = take(4, &mut r)?;
    let old_cy: usize = if &magic == b"RVX1" {
        64
    } else if &magic == b"RVX2" {
        CY
    } else {
        return None;
    };
    let seed = u64::from_le_bytes(take(8, &mut r)?.try_into().ok()?);
    let count = u32::from_le_bytes(take(4, &mut r)?.try_into().ok()?) as usize;
    let mut w = World::new(seed);
    for _ in 0..count {
        let cx = i32::from_le_bytes(take(4, &mut r)?.try_into().ok()?);
        let cz = i32::from_le_bytes(take(4, &mut r)?.try_into().ok()?);
        let mut ch = crate::world::Chunk::new();
        let mut filled = 0usize;
        while filled < crate::world::CX * old_cy * crate::world::CZ {
            let v = u16::from_le_bytes(take(2, &mut r)?.try_into().ok()?);
            let run = u32::from_le_bytes(take(4, &mut r)?.try_into().ok()?) as usize;
            if run == 0 || filled + run > crate::world::CX * old_cy * crate::world::CZ {
                return None;
            }
            for k in 0..run {
                ch.blocks[filled + k] = v;
            }
            filled += run;
        }
        if old_cy != CY {
            ch.blocks
                .resize(crate::world::CX * CY * crate::world::CZ, crate::world::AIR);
        }
        ch.modified = true;
        w.chunks.insert((cx, cz), ch);
    }
    Some(w)
}

/// Point d'apparition au sec près de l'origine (spirale).
fn find_spawn(w: &World) -> Vec3 {
    for r in 0..24 {
        let n = if r == 0 { 1 } else { r * 8 };
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let x = (a.cos() * r as f32 * 8.0).floor() as i32;
            let z = (a.sin() * r as f32 * 8.0).floor() as i32;
            if !w.has_chunk((x.div_euclid(16), z.div_euclid(16))) {
                continue;
            }
            if let Some(y) = w.surface_y(x, z) {
                if w.get_block(x, y, z) != WATER && w.get_block(x, y + 1, z) == crate::world::AIR {
                    return Vec3::new(x as f32 + 0.5, (y + 1) as f32 + 0.01, z as f32 + 0.5);
                }
            }
        }
    }
    Vec3::new(8.5, 80.0, 8.5)
}

// ------------------------------------------------------------- day/night sky
/// (light, night, couleur brouillard, direction soleil, élévation) pour t 0..1.
pub fn sky_state(day_t: f32) -> (f32, f32, [f32; 3], Vec3, f32) {
    let a = day_t * std::f32::consts::TAU;
    let elev = a.sin();
    let light = (0.5 + elev * 1.45).clamp(0.14, 1.0);
    let night = ((0.42 - light) / 0.32).clamp(0.0, 1.0);
    let k = ((light - 0.14) / 0.86).clamp(0.0, 1.0);
    let day_col = [0.58, 0.75, 0.92];
    let night_col = [0.015, 0.025, 0.065];
    let mut col = [
        night_col[0] + (day_col[0] - night_col[0]) * k,
        night_col[1] + (day_col[1] - night_col[1]) * k,
        night_col[2] + (day_col[2] - night_col[2]) * k,
    ];
    if elev.abs() < 0.18 && elev > -0.08 {
        let s = (1.0 - elev.abs() / 0.18) * 0.55 * k.max(0.15);
        col = [
            col[0] + (0.98 - col[0]) * s,
            col[1] + (0.52 - col[1]) * s,
            col[2] + (0.32 - col[2]) * s,
        ];
    }
    let sun_dir = Vec3::new(a.cos(), a.sin(), 0.18).normalize();
    (light, night, col, sun_dir, elev)
}

pub fn day_to_u16(t: f32) -> u16 {
    (t.rem_euclid(1.0) * 65535.0) as u16
}
pub fn u16_to_day(v: u16) -> f32 {
    v as f32 / 65535.0
}

// ------------------------------------------------------------ codec NBT (configuration)

fn dimension_type_codec() -> Nbt {
    let mut el = Nbt::compound();
    el.set("ambient_light", Nbt::Float(0.0));
    el.set("bed_works", Nbt::Byte(1));
    el.set("coordinate_scale", Nbt::Double(1.0));
    el.set("effects", Nbt::Str("minecraft:overworld".into()));
    el.set("has_ceiling", Nbt::Byte(0));
    el.set("has_raids", Nbt::Byte(1));
    el.set("has_skylight", Nbt::Byte(1));
    el.set("height", Nbt::Int(vd::WORLD_HEIGHT));
    el.set("infiniburn", Nbt::Str("#minecraft:infiniburn_overworld".into()));
    el.set("logical_height", Nbt::Int(vd::WORLD_HEIGHT));
    el.set("min_y", Nbt::Int(vd::WORLD_MIN_Y));
    el.set("monster_spawn_block_light_limit", Nbt::Int(0));
    // IntProvider : un ENTIER SIMPLE est accepté par toutes les versions
    // (1.20.5 : either(int, {type,value:{min,max}}) ; >= 1.21.5/26.x :
    // either(int, {type,min,max}) — le format wrappé casse les clients récents,
    // cf. log « No key max_inclusive in MapLike »).
    el.set("monster_spawn_light_level", Nbt::Int(7));
    el.set("natural", Nbt::Byte(1));
    el.set("piglin_safe", Nbt::Byte(0));
    el.set("respawn_anchor_works", Nbt::Byte(0));
    el.set("ultrawarm", Nbt::Byte(0));
    el
}

fn biome_codec(i: usize) -> Nbt {
    let (name, temp, downfall, precip, sky, fog, water, wfog) = vd::BIOME_ENTRIES[i];
    let mut el = Nbt::compound();
    el.set("has_precipitation", Nbt::Byte(if precip { 1 } else { 0 }));
    el.set("temperature", Nbt::Float(temp));
    el.set("downfall", Nbt::Float(downfall));
    let mut fx = Nbt::compound();
    fx.set("fog_color", Nbt::Int(fog));
    fx.set("sky_color", Nbt::Int(sky));
    fx.set("water_color", Nbt::Int(water));
    fx.set("water_fog_color", Nbt::Int(wfog));
    el.set("effects", fx);
    let mut entry = Nbt::compound();
    entry.set("name", Nbt::Str(format!("minecraft:{name}")));
    entry.set("id", Nbt::Int(i as i32));
    entry.set("element", el);
    entry
}

fn chat_type_codec() -> Nbt {
    // Format >= 1.21.5 (26.x) : la décoration est EN LIGNE dans chat/narration
    // (le wrapper {decoration:{...}} 1.19.3-1.21.4 est rejeté par les clients
    // récents : « No key parameters in MapLike »). Champs requis :
    // translation_key + parameters (parameters peut être omis, on l'envoie).
    let mut chat = Nbt::compound();
    chat.set("translation_key", Nbt::Str("chat.type.text".into()));
    chat.set(
        "parameters",
        Nbt::List(vec![Nbt::Str("sender".into()), Nbt::Str("content".into())]),
    );
    let mut narration = Nbt::compound();
    narration.set("translation_key", Nbt::Str("chat.type.text.narrate".into()));
    narration.set(
        "parameters",
        Nbt::List(vec![Nbt::Str("sender".into()), Nbt::Str("content".into())]),
    );
    let mut el = Nbt::compound();
    el.set("chat", chat);
    el.set("narration", narration);
    let mut entry = Nbt::compound();
    entry.set("name", Nbt::Str("minecraft:chat".into()));
    entry.set("id", Nbt::Int(0));
    entry.set("element", el);
    entry
}

fn damage_type_codec(i: usize) -> Nbt {
    // Registre vanilla COMPLET : un registre partiel fait échouer le gel
    // côté client (« Unbound values in registry ... [minecraft:thorns] »)
    // car les tags par défaut du client référencent tous les types vanilla.
    let (name, msg, scaling, exhaustion, effects, death_msg) = vd::DAMAGE_TYPES[i];
    let mut el = Nbt::compound();
    el.set("message_id", Nbt::Str(msg.into()));
    el.set("scaling", Nbt::Str(scaling.into()));
    el.set("exhaustion", Nbt::Float(exhaustion));
    if !effects.is_empty() {
        el.set("effects", Nbt::Str(effects.into()));
    }
    if !death_msg.is_empty() {
        el.set("death_message_type", Nbt::Str(death_msg.into()));
    }
    let mut entry = Nbt::compound();
    entry.set("name", Nbt::Str(format!("minecraft:{name}")));
    entry.set("id", Nbt::Int(i as i32));
    entry.set("element", el);
    entry
}

/// Payloads `registry_data` de la phase configuration (un par registre).
fn build_codec_packets() -> Vec<Vec<u8>> {
    let reg = |id: &str, entries: Vec<Nbt>| -> Vec<u8> {
        let mut w = W2::new().s(id);
        w = w.vi(entries.len() as i32);
        for e in entries {
            w = w.s(e.get("name").and_then(|n| n.as_str()).unwrap_or("?"));
            if let Some(el) = e.get("element") {
                w = w.b(true).nbt(el);
            } else {
                w = w.b(false);
            }
        }
        frame(cb::CFG_REGISTRY_DATA, &w.done())
    };
    let mut dim = Nbt::compound();
    dim.set("name", Nbt::Str(vd::DIMENSION_TYPE_NAME.into()));
    dim.set("id", Nbt::Int(0));
    dim.set("element", dimension_type_codec());
    let mut biomes = Vec::new();
    for i in 0..vd::BIOME_ENTRIES.len() {
        biomes.push(biome_codec(i));
    }
    vec![
        reg("minecraft:dimension_type", vec![dim]),
        reg("minecraft:worldgen/biome", biomes),
        reg("minecraft:chat_type", vec![chat_type_codec()]),
        reg(
            "minecraft:damage_type",
            (0..vd::DAMAGE_TYPES.len()).map(damage_type_codec).collect(),
        ),
    ]
}

// ---------------------------------------------------------------- framing

/// Paquet complet prêt pour la socket : longueur varint + id + payload.
pub fn frame(id: i32, payload: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(payload.len() + 5);
    crate::mcproto::write_varint(&mut body, id);
    body.extend_from_slice(payload);
    let mut out = Vec::with_capacity(body.len() + 5);
    crate::mcproto::write_varint(&mut out, body.len() as i32);
    out.extend_from_slice(&body);
    out
}

// ------------------------------------------------------------ mob targets
struct Targets<'a>(pub &'a mut HashMap<u32, PlayerS>);

impl MobTargets for Targets<'_> {
    fn nearest_pos(&self, pos: Vec3, max: f32) -> Option<Vec3> {
        let mut best = None;
        let mut bd = max;
        for p in self.0.values() {
            if p.dead {
                continue;
            }
            let d = (p.pos - pos).length();
            if d <= bd {
                bd = d;
                best = Some(p.pos);
            }
        }
        best
    }
    fn hurt_nearest(&mut self, pos: Vec3, max: f32, dmg: i32, knock: Vec3) {
        let mut best_id = None;
        let mut bd = max;
        for p in self.0.values() {
            if p.dead {
                continue;
            }
            let d = (p.pos - pos).length();
            if d <= bd {
                bd = d;
                best_id = Some(p.id);
            }
        }
        if let Some(id) = best_id {
            if let Some(p) = self.0.get_mut(&id) {
                p.hurt(dmg, knock, 0.6);
            }
        }
    }
    fn hurt_radius(&mut self, pos: Vec3, radius: f32, base: i32, knock: f32) {
        for p in self.0.values_mut() {
            if p.dead {
                continue;
            }
            let pc = p.pos + Vec3::new(0.0, 0.9, 0.0);
            let d = (pc - pos).length();
            if d < radius {
                let f = 1.0 - d / radius;
                let dir = (pc - pos).normalize();
                p.invuln = 0.0;
                p.hurt(
                    (f * base as f32).round() as i32,
                    dir * (f * knock) + Vec3::new(0.0, f * knock * 0.42, 0.0),
                    0.0,
                );
            }
        }
    }
}

impl Server {
    pub fn new(cfg: ServerConfig) -> Server {
        let (world, loaded) = match load_world(&cfg.world_path) {
            Some(w) => (w, true),
            None => (World::new(cfg.seed), false),
        };
        if cfg.verbose {
            println!(
                "monde: {} (graine {})",
                if loaded { "chargé" } else { "nouveau" },
                world.seed
            );
        }
        let mut s = Server {
            cfg,
            world,
            players: HashMap::new(),
            mobs: Vec::new(),
            particles: Vec::new(),
            spawner: Spawner::new(),
            time: 0.06,
            next_id: 1,
            next_uid: 0x1000_0000,
            stop_requested: false,
            tick_n: 0,
            biome_cache: HashMap::new(),
            spawn: Vec3::new(8.5, 80.0, 8.5),
            codec: Vec::new(),
            uuid_rng: 0x9E37_79B9_7F4A_7C15,
        };
        for dx in -2..=2 {
            for dz in -2..=2 {
                s.world.gen_chunk(dx, dz);
            }
        }
        s.spawn = find_spawn(&s.world);
        s.codec = build_codec_packets();
        // Ligne de contrôle : permet de vérifier sur la console que le binaire
        // qui tourne contient bien la table damage_type vanilla 1.20.5 exacte
        // (46 = fix « Missing element minecraft:on_fire » ; 40 = ancien build).
        s.log(&format!(
            "registres configuration : biome={}, damage_type={} (vanilla 1.20.5 exact, on_fire inclus)",
            vd::BIOME_ENTRIES.len(),
            vd::DAMAGE_TYPES.len()
        ));
        s
    }

    pub fn log(&self, msg: &str) {
        if !self.cfg.verbose {
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs() % 86400;
        println!(
            "[{:02}:{:02}:{:02}] {}",
            secs / 3600,
            (secs / 60) % 60,
            secs % 60,
            msg
        );
    }

    fn send(&self, p: &PlayerS, id: i32, payload: &[u8]) {
        let _ = p.out.send(frame(id, payload));
    }

    fn bcast(&self, id: i32, payload: &[u8]) {
        let pkt = frame(id, payload);
        for p in self.players.values() {
            let _ = p.out.send(pkt.clone());
        }
    }

    // ------------------------------------------------------------- chat util

    /// Composant texte NBT pour system_chat.
    fn text_nbt(msg: &str) -> Nbt {
        let mut c = Nbt::compound();
        c.set("text", Nbt::Str(msg.to_string()));
        c
    }

    pub fn system_to(&mut self, id: u32, msg: &str) {
        let nbt = Self::text_nbt(msg);
        let w = W2::new().nbt(&nbt).b(false);
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::SYSTEM_CHAT, &w.done());
        }
    }

    fn bcast_chat(&mut self, msg: &str) {
        let nbt = Self::text_nbt(msg);
        let w = W2::new().nbt(&nbt).b(false).done();
        self.bcast(cb::SYSTEM_CHAT, &w);
    }

    // ------------------------------------------------------------- join/leave
    fn join(&mut self, name: String, uuid: [u8; 16], out: Sender<Vec<u8>>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let mut name = if name.trim().is_empty() {
            "Joueur".to_string()
        } else {
            name.trim().chars().take(16).collect()
        };
        let base = name.clone();
        let mut k = 2;
        while self.players.values().any(|p| p.name == name) {
            name = format!("{base}{k}");
            k += 1;
        }
        let skin = (noise::hash2i(
            name.chars().map(|c| c as i64).sum::<i64>(),
            7,
            0x511A,
        )) as u8;
        let p = PlayerS {
            id,
            uuid,
            name: name.clone(),
            pos: self.spawn,
            yaw: 0.0,
            pitch: 0.0,
            hp: 20,
            hunger: 20.0,
            air: 10.0,
            xp: 0,
            dead: false,
            announced: false,
            creative: true, // créatif par défaut : inventaire vanilla jouable sans sync
            skin: skin % crate::world::T_PLAYER_SHIRTS as u8,
            sprint: false,
            invuln: 1.5,
            eat_cd: 0.0,
            regen_cd: 2.0,
            starve_cd: 4.0,
            out,
            sent: HashSet::new(),
            health_sent: (0, 0, 0),
            seen: HashMap::new(),
            hotbar: [
                crate::world::STONE,
                crate::world::DIRT,
                crate::world::GRASS,
                crate::world::PLANKS,
                crate::world::LOG,
                crate::world::TORCH,
                crate::world::GLASS,
                crate::world::COBBLE,
                crate::world::SAND,
            ],
            held: 0,
            on_ground: true,
            peak_y: self.spawn.y,
            keepalive_sent: None,
        };
        self.players.insert(id, p);
        // l'état play DOIT ouvrir avec le paquet login
        self.send_play_login(id);
        self.broadcast_players();
        let msg = format!("§e{} a rejoint la partie", name);
        self.bcast_chat(&msg);
        self.log(&format!("{} (#{}) a rejoint", name, id));
        id
    }

    /// Rafale d'initialisation play (login, abilities, spawn, téléport, heure…).
    fn send_play_login(&mut self, id: u32) {
        let me = match self.players.get(&id) {
            Some(p) => (p.id, p.pos, p.creative, p.name.clone()),
            None => return,
        };
        let (pid, pos, creative, _name) = me;
        // play login (0x2B)
        let gm = if creative { 1 } else { 0 };
        let w = W2::new()
            .i32v(pid as i32)
            .b(false)
            .vi(1)
            .s(vd::DIMENSION_NAME)
            .vi(self.cfg.max_players as i32)
            .vi(self.cfg.view.max(2))
            .vi(self.cfg.view.max(2))
            .b(false)
            .b(true)
            .b(false)
            .vi(0) // dimension_type: index 0 du registre
            .s(vd::DIMENSION_NAME)
            .i64v(self.world.seed as i64)
            .i8v(gm)
            .u8v(0xFF)
            .b(false)
            .b(false)
            .b(false) // death location: absent
            .vi(0)
            .b(false);
        let pkt = w.done();
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::LOGIN, &pkt);
        }
        // abilities
        let flags: i8 = if creative { 0b111 } else { 0b000 };
        let pkt = W2::new().i8v(flags).f32v(0.05).f32v(0.1).done();
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::ABILITIES, &pkt);
        }
        // spawn position
        let sx = self.spawn.x.floor() as i32;
        let sy = self.spawn.y.floor() as i32;
        let sz = self.spawn.z.floor() as i32;
        let pkt = W2::new().pos(sx, sy, sz).f32v(0.0).done();
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::SPAWN_POSITION, &pkt);
        }
        // santé (20/20)
        self.send_health(id);
        // heure
        let pkt = W2::new()
            .i64v(0)
            .i64v((self.time * 24000.0) as i64)
            .done();
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::UPDATE_TIME, &pkt);
        }
        // expérience
        let pkt = W2::new().f32v(0.0).vi(0).vi(0).done();
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::EXPERIENCE, &pkt);
        }
        // téléport (position absolue, confirmée par le client)
        let tele_id = pid as i32;
        let pkt = W2::new()
            .f64v(pos.x as f64)
            .f64v(pos.y as f64)
            .f64v(pos.z as f64)
            .f32v(crate::mcproto::van_yaw_deg(0.0))
            .f32v(0.0)
            .u8v(0)
            .vi(tele_id)
            .done();
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::POSITION, &pkt);
        }
        // chunk central
        let cx = (pos.x / 16.0).floor() as i32;
        let cz = (pos.z / 16.0).floor() as i32;
        let pkt = W2::new().vi(cx).vi(cz).done();
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::UPDATE_VIEW_POSITION, &pkt);
        }
        // hotbar par défaut (slot 0)
        if creative {
            let item = vd::item_id_for_block(crate::world::STONE);
            if let Some(it) = item {
                let pkt = Self::set_slot_packet(0, 36 + 0, it, 64);
                if let Some(p) = self.players.get(&id) {
                    self.send(p, cb::SET_SLOT, &pkt);
                }
            }
        }
        // liste des joueurs déjà en ligne (tab)
        let entries: Vec<([u8; 16], String, bool, i32)> = self
            .players
            .values()
            .map(|p| (p.uuid, p.name.clone(), true, p.creative as i32))
            .collect();
        let pkt = Self::player_info_packet(&entries);
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::PLAYER_INFO, &pkt);
        }
    }

    fn set_slot_packet(_state: i32, slot: i16, item: i32, count: i8) -> Vec<u8> {
        W2::new()
            .i8v(-2) // fenêtre player (inventory) — ContainerID i8
            .vi(1) // stateId
            .i16v(slot)
            .i8v(count) // itemCount (0 = vide)
            .vi(item)
            .vi(0) // composants retirés
            .vi(0) // composants ajoutés
            .done()
    }

    /// player_info : add_player + update_listed + update_game_mode.
    fn player_info_packet(entries: &[([u8; 16], String, bool, i32)]) -> Vec<u8> {
        let mut w = W2::new();
        let actions = 0b1 | 0b100 | 0b1000; // add_player|update_game_mode|update_listed
        w = w.u8v(actions);
        w = w.vi(entries.len() as i32);
        for (uuid, name, listed, gm) in entries {
            w = w.uuid(uuid);
            w = w.s(name); // add_player
            w = w.vi(0); // propriétés
            w = w.vi(*gm); // update_game_mode
            w = w.vi(*listed as i32); // update_listed
        }
        w.done()
    }

    fn broadcast_players(&mut self) {
        let entries: Vec<([u8; 16], String, bool, i32)> = self
            .players
            .values()
            .map(|p| (p.uuid, p.name.clone(), true, p.creative as i32))
            .collect();
        let pkt = Self::player_info_packet(&entries);
        self.bcast(cb::PLAYER_INFO, &pkt);
    }

    fn leave(&mut self, id: u32, reason: &str) {
        if let Some(p) = self.players.remove(&id) {
            let msg = format!("§e{} a quitté la partie ({})", p.name, reason);
            self.bcast_chat(&msg);
            // retire de la tab list
            let pkt = W2::new().vi(1).uuid(&p.uuid).done();
            self.bcast(cb::PLAYER_REMOVE, &pkt);
            // détruit l'entité joueur chez les autres
            let pkt = W2::new().vi(1).vi(p.id as i32).done();
            self.bcast(cb::ENTITY_DESTROY, &pkt);
            for q in self.players.values_mut() {
                q.seen.remove(&p.id);
            }
            self.log(&format!("{} parti: {}", p.name, reason));
        }
    }

    // -------------------------------------------------------------- santé

    fn send_health(&mut self, id: u32) {
        let key = match self.players.get(&id) {
            Some(p) => (
                if p.creative { 20 } else { p.hp.max(0) },
                p.hunger.round().clamp(0.0, 20.0) as i32,
                p.xp.min(65535),
            ),
            None => return,
        };
        if key == self.players[&id].health_sent {
            return;
        }
        let (hp, food, xp) = key;
        let lvl = (xp / 10) as i32;
        let pkt_health = W2::new()
            .f32v(hp as f32)
            .vi(food)
            .f32v(5.0)
            .done();
        let pkt_xp = W2::new().f32v((xp % 10) as f32 / 10.0).vi(lvl).vi(xp as i32).done();
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::UPDATE_HEALTH, &pkt_health);
            self.send(p, cb::EXPERIENCE, &pkt_xp);
        }
        self.players.get_mut(&id).unwrap().health_sent = key;
    }

    /// Synchronise santé/heure si ça a changé (appelé depuis tick).
    fn sync_health(&mut self) {
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            self.send_health(id);
        }
    }
}

impl Server {
    // -------------------------------------------------------------- paquets serveur
    /// Traite un paquet play serverbound (pid déjà lu). Renvoie false si la
    /// connexion doit mourir.
    fn handle(&mut self, id: u32, pid: i32, p: &mut R2) -> bool {
        match pid {
            sb::TELEPORT_CONFIRM => { /* le client confirme la téléportation */ }
            sb::KEEP_ALIVE => {
                let Some(t) = p.i64v() else { return true };
                if let Some(q) = self.players.get_mut(&id) {
                    if let Some((sent, _)) = q.keepalive_sent {
                        if sent == t {
                            q.keepalive_sent = None;
                        }
                    }
                }
            }
            sb::POSITION | sb::POSITION_LOOK | sb::LOOK | sb::FLYING => {
                let (x, y, z, yaw, pitch) = if pid == sb::LOOK || pid == sb::FLYING {
                    let yaw = p.f32v().unwrap_or(0.0);
                    let pitch = p.f32v().unwrap_or(0.0);
                    (None, None, None, yaw, pitch)
                } else {
                    let x = p.f64v().unwrap_or(0.0) as f32;
                    let y = p.f64v().unwrap_or(0.0) as f32;
                    let z = p.f64v().unwrap_or(0.0) as f32;
                    let (yaw, pitch) = if pid == sb::POSITION {
                        (0.0f32, 0.0f32)
                    } else {
                        (p.f32v().unwrap_or(0.0), p.f32v().unwrap_or(0.0))
                    };
                    (Some(x), Some(y), Some(z), yaw, pitch)
                };
                let on_ground = p.b().unwrap_or(true);
                if let Some(q) = self.players.get_mut(&id) {
                    if let Some(x) = x {
                        let np = Vec3::new(x, y.unwrap(), z.unwrap());
                        let d = (np - q.pos).length();
                        if d < 64.0 {
                            // dégâts de chute côté serveur (créatif immuno)
                            if np.y < q.peak_y {
                                if q.on_ground && !on_ground {
                                    q.peak_y = q.pos.y;
                                }
                            } else {
                                q.peak_y = np.y;
                            }
                            if on_ground && !q.creative {
                                let fall = q.peak_y - np.y;
                                if fall > 3.5 {
                                    q.invuln = 0.0;
                                    q.hurt((fall - 3.0).round() as i32, Vec3::new(0.0, 1.0, 0.0), 0.5);
                                }
                                q.peak_y = np.y;
                            }
                            if np.y < -40.0 && !q.creative && !q.dead {
                                q.invuln = 0.0;
                                q.hurt(4, Vec3::new(0.0, 0.0, 0.0), 0.5);
                            }
                            q.pos = np;
                        }
                    }
                    q.yaw = crate::mcproto::our_yaw(yaw);
                    q.pitch = crate::mcproto::our_pitch(pitch);
                    q.on_ground = on_ground;
                }
            }
            sb::CHAT_MESSAGE => {
                let Some(msg) = p.s(256) else { return true };
                self.on_chat(id, &msg);
            }
            sb::CHAT_COMMAND | sb::CHAT_COMMAND_SIGNED => {
                let Some(cmd) = p.s(256) else { return true };
                self.on_chat(id, &format!("/{}", cmd));
            }
            sb::CLIENT_COMMAND => {
                let Some(action) = p.vi() else { return true };
                if action == 0 {
                    self.do_respawn(id);
                }
            }
            sb::BLOCK_DIG => {
                let Some(status) = p.vi() else { return true };
                let Some((x, y, z)) = p.pos() else { return true };
                if status == 0 || status == 2 {
                    // started (créatif = instantané) / finished (survie)
                    self.do_dig(id, x, y, z);
                }
            }
            sb::BLOCK_PLACE => {
                let Some(_hand) = p.vi() else { return true };
                let Some((hx, hy, hz)) = p.pos() else { return true };
                let Some(dir) = p.vi() else { return true };
                let _cx = p.f32v().unwrap_or(0.0);
                let _cy = p.f32v().unwrap_or(0.0);
                let _cz = p.f32v().unwrap_or(0.0);
                let _inside = p.b().unwrap_or(false);
                // le client place contre (hx,hy,hz) côté `dir` (0=-Y,1=+Y,2=-Z,3=+Z,4=-X,5=+X)
                let (dx, dy, dz) = match dir {
                    0 => (0, -1, 0),
                    1 => (0, 1, 0),
                    2 => (0, 0, -1),
                    3 => (0, 0, 1),
                    4 => (-1, 0, 0),
                    5 => (1, 0, 0),
                    _ => (0, 0, 0),
                };
                let (x, y, z) = (hx + dx, hy + dy, hz + dz);
                if dir > 5 {
                    // use_item (pas de pose)
                    return true;
                }
                let b = self
                    .players
                    .get(&id)
                    .map(|q| q.hotbar[q.held.min(8)])
                    .unwrap_or(0);
                self.do_place(id, x, y, z, b);
            }
            sb::USE_ENTITY => {
                let Some(target) = p.vi() else { return true };
                let Some(mouse) = p.vi() else { return true };
                if mouse == 1 {
                    if let Some(_sneak) = p.b() {}
                    self.do_attack(id, target as u32);
                }
            }
            sb::HELD_ITEM_SLOT => {
                let Some(slot) = p.i16v() else { return true };
                if let Some(q) = self.players.get_mut(&id) {
                    q.held = (slot.max(0) as usize).min(8);
                }
            }
            sb::SET_CREATIVE_SLOT => {
                let Some(slot) = p.i16v() else { return true };
                let count = p.i8v().unwrap_or(0);
                if count > 0 {
                    let Some(item) = p.vi() else { return true };
                    let _rm = p.vi().unwrap_or(0);
                    let _add = p.vi().unwrap_or(0);
                    if let Some(b) = vd::block_from_item_id(item) {
                        if let Some(q) = self.players.get_mut(&id) {
                            if (36..=44).contains(&slot) {
                                q.hotbar[(slot - 36) as usize] = b;
                            }
                        }
                    }
                }
            }
            sb::ARM_ANIMATION => {
                // relayé par send_entities (pas d'anim dédiée)
            }
            sb::ENTITY_ACTION => {
                let Some(_eid) = p.vi() else { return true };
                let Some(action) = p.vi() else { return true };
                let _ = p.vi();
                if let Some(q) = self.players.get_mut(&id) {
                    match action {
                        3 => q.sprint = true,
                        4 => q.sprint = false,
                        _ => {}
                    }
                }
            }
            sb::SETTINGS => { /* locale, skin parts... ignoré */ }
            sb::PLUGIN_MESSAGE => {
                let Some(_ch) = p.s(128) else { return true };
                // minecraft:brand etc.
            }
            sb::ABILITIES => { /* le client active/désactive le vol */ }
            sb::CHUNK_BATCH_RECEIVED => { /* feedback vitesse chunks */ }
            _ => {}
        }
        true
    }

    fn on_chat(&mut self, id: u32, msg: &str) {
        let msg = msg.chars().take(120).collect::<String>();
        if msg.starts_with('/') {
            let name = self
                .players
                .get(&id)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            if let Some(reply) = self.command(&name, &msg) {
                self.system_to(id, &reply);
            }
        } else if !msg.trim().is_empty() {
            let name = self
                .players
                .get(&id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "?".to_string());
            let line = format!("§f<{}> §f{}", name, msg);
            self.bcast_chat(&line);
            self.log(&format!("<{}> {}", name, msg));
        }
    }

    fn eye_of(&self, id: u32) -> Option<Vec3> {
        self.players
            .get(&id)
            .map(|p| p.pos + Vec3::new(0.0, EYE_H, 0.0))
    }

    fn in_reach(&self, id: u32, x: i32, y: i32, z: i32, r: f32) -> bool {
        self.eye_of(id)
            .map(|e| (e - Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5)).length() <= r)
            .unwrap_or(false)
    }

    fn do_dig(&mut self, id: u32, x: i32, y: i32, z: i32) {
        if y < 0 || y >= CY as i32 {
            return;
        }
        if !self.in_reach(id, x, y, z, 6.0) {
            return;
        }
        let b = self.world.get_block(x, y, z);
        if b == crate::world::AIR || crate::world::hardness(b) == f32::INFINITY {
            return;
        }
        self.world.set_block(x, y, z, crate::world::AIR);
        let pkt = W2::new().pos(x, y, z).vi(vd::block_state_id(crate::world::AIR) as i32).done();
        self.bcast(cb::BLOCK_CHANGE, &pkt);
    }

    fn do_place(&mut self, id: u32, x: i32, y: i32, z: i32, b: u16) {
        if y < 0 || y >= CY as i32 || b == crate::world::AIR {
            return;
        }
        if !self.in_reach(id, x, y, z, 6.0) {
            return;
        }
        let cur = self.world.get_block(x, y, z);
        if !(cur == crate::world::AIR || cur == WATER || is_cross_id(cur)) {
            return;
        }
        let bb_min = Vec3::new(x as f32 - HALF_W, y as f32, z as f32 - HALF_W);
        let bb_max = Vec3::new(x as f32 + 1.0 + HALF_W, y as f32 + 1.0, z as f32 + 1.0 + HALF_W);
        for q in self.players.values() {
            let c = q.pos;
            if c.x + HALF_W > bb_min.x
                && c.x - HALF_W < bb_max.x
                && c.y + 1.8 > bb_min.y
                && c.y < bb_max.y
                && c.z + HALF_W > bb_min.z
                && c.z - HALF_W < bb_max.z
            {
                return;
            }
        }
        self.world.set_block(x, y, z, b);
        let pkt = W2::new().pos(x, y, z).vi(vd::block_state_id(b) as i32).done();
        self.bcast(cb::BLOCK_CHANGE, &pkt);
    }

    fn do_attack(&mut self, attacker: u32, target: u32) {
        // PvP ?
        if self.players.contains_key(&target) && target != attacker {
            let (tpos, tdead) = {
                let tp = &self.players[&target];
                (tp.pos, tp.dead)
            };
            let in_range = self
                .eye_of(attacker)
                .map(|e| (e - (tpos + Vec3::new(0.0, 0.9, 0.0))).length() <= 4.5)
                .unwrap_or(false);
            if in_range && !tdead {
                let dir = (tpos - self.players[&attacker].pos).normalize();
                if let Some(tp) = self.players.get_mut(&target) {
                    tp.hurt(2, dir * 6.0 + Vec3::new(0.0, 2.5, 0.0), 0.6);
                }
            }
            return;
        }
        // mob ?
        let mi = self.mobs.iter().position(|m| m.uid == target);
        if let Some(mi) = mi {
            let mpos = self.mobs[mi].center();
            let in_range = self
                .eye_of(attacker)
                .map(|e| (e - mpos).length() <= 4.5)
                .unwrap_or(false);
            if !in_range {
                return;
            }
            let dir = (mpos - self.eye_of(attacker).unwrap()).normalize();
            let (kind, was_alive) = (self.mobs[mi].kind, !self.mobs[mi].dead_flag);
            self.mobs[mi].hurt(4, dir * 8.0 + Vec3::new(0.0, 4.0, 0.0));
            if was_alive && self.mobs[mi].dead_flag && kind.hostile() {
                if let Some(q) = self.players.get_mut(&attacker) {
                    q.xp += 3;
                }
            }
        }
    }

    fn do_respawn(&mut self, id: u32) {
        let respawned = {
            let Some(q) = self.players.get_mut(&id) else { return };
            if !q.dead {
                false
            } else {
                q.pos = self.spawn;
                q.hp = 20;
                q.hunger = 20.0;
                q.air = 10.0;
                q.dead = false;
                q.announced = false;
                q.invuln = 2.0;
                q.peak_y = q.pos.y;
                q.seen.clear();
                true
            }
        };
        if respawned {
            let (_pos, gm) = {
                let q = &self.players[&id];
                (q.pos, if q.creative { 1i8 } else { 0i8 })
            };
            // paquet respawn (SpawnInfo complet)
            let w = W2::new()
                .vi(0)
                .s(vd::DIMENSION_NAME)
                .i64v(self.world.seed as i64)
                .i8v(gm)
                .u8v(0xFF)
                .b(false)
                .b(false)
                .b(false)
                .vi(0)
                .b(false); // copyMetadata
            let pkt = w.done();
            if let Some(q) = self.players.get(&id) {
                self.send(q, cb::RESPAWN, &pkt);
            }
            self.send_play_login(id);
            self.log("joueur réapparaît");
        }
    }

    // ------------------------------------------------------------- commandes
    /// Exécute une commande /command. `from` = nom du joueur ou "Console".
    pub fn command(&mut self, from: &str, line: &str) -> Option<String> {
        let mut it = line.trim_start_matches('/').split_whitespace();
        let cmd = it.next()?.to_ascii_lowercase();
        let args: Vec<&str> = it.collect();
        let reply = match cmd.as_str() {
            "help" => Some(
                "/time jour|nuit|<0-1> /tp <joueur|x y z> /gamemode <s|c> [joueur]\n\
                 /list /kick <joueur> /seed /say <msg> /save /stop /help"
                    .to_string(),
            ),
            "list" => {
                let mut names: Vec<String> =
                    self.players.values().map(|p| p.name.clone()).collect();
                names.sort();
                Some(format!("{} joueur(s): {}", names.len(), names.join(", ")))
            }
            "seed" => Some(format!("graine: {}", self.world.seed)),
            "time" => match args.first().copied() {
                Some("jour") | Some("day") => {
                    self.time = 0.06;
                    None
                }
                Some("nuit") | Some("night") => {
                    self.time = 0.55;
                    None
                }
                Some("midi") | Some("noon") => {
                    self.time = 0.25;
                    None
                }
                Some(v) => match v.parse::<f32>() {
                    Ok(t) if (0.0..=1.0).contains(&t) => {
                        self.time = t;
                        None
                    }
                    _ => Some("usage: /time <jour|nuit|0..1>".into()),
                },
                None => Some("usage: /time <jour|nuit|0..1>".into()),
            },
            "tp" => {
                if args.len() == 1 {
                    let want = args[0];
                    let target = self
                        .players
                        .values()
                        .find(|p| p.name == want)
                        .map(|p| p.pos);
                    let from_id = self
                        .players
                        .values()
                        .find(|p| p.name == from)
                        .map(|p| p.id);
                    match (target, from_id) {
                        (Some(t), Some(fid)) => {
                            if let Some(p) = self.players.get_mut(&fid) {
                                p.pos = t;
                                Some(format!("téléporté vers {}", want))
                            } else {
                                None
                            }
                        }
                        _ => Some(format!("joueur \"{}\" introuvable", want)),
                    }
                } else if args.len() == 3 {
                    let ok = args.iter().all(|a| a.parse::<f32>().is_ok());
                    if !ok {
                        Some("usage: /tp x y z".into())
                    } else {
                        let v: Vec<f32> = args.iter().map(|a| a.parse().unwrap()).collect();
                        let t = Vec3::new(v[0], v[1], v[2]);
                        let from_id = self
                            .players
                            .values()
                            .find(|p| p.name == from)
                            .map(|p| p.id);
                        if let Some(fid) = from_id {
                            if let Some(p) = self.players.get_mut(&fid) {
                                p.pos = t;
                                p.seen.clear();
                            }
                            self.teleport_player(fid, t);
                        }
                        None
                    }
                } else {
                    Some("usage: /tp <joueur> | /tp x y z".into())
                }
            }
            "gamemode" | "gm" => {
                let mode = args.first().copied().unwrap_or("");
                let creative = matches!(mode, "c" | "creative" | "creatif" | "créatif");
                let who = args.get(1).copied().unwrap_or(from);
                let changed = match self.players.values_mut().find(|p| p.name == who) {
                    Some(p) => {
                        p.creative = creative;
                        p.seen.clear();
                        Some(p.id)
                    }
                    None => None,
                };
                if let Some(pid) = changed {
                    let gm = if creative { 1i8 } else { 0i8 };
                    // change_game_mode via game_state_change (reason 3)
                    let pkt = W2::new().u8v(3).f32v(gm as f32).done();
                    if let Some(p) = self.players.get(&pid) {
                        self.send(p, cb::GAME_STATE_CHANGE, &pkt);
                    }
                    let flags: i8 = if creative { 0b111 } else { 0b000 };
                    let pkt = W2::new().i8v(flags).f32v(0.05).f32v(0.1).done();
                    if let Some(p) = self.players.get(&pid) {
                        self.send(p, cb::ABILITIES, &pkt);
                    }
                    self.send_health(pid);
                    Some(format!(
                        "{} est maintenant en {}",
                        who,
                        if creative { "créatif" } else { "survie" }
                    ))
                } else {
                    Some(format!("joueur \"{}\" introuvable", who))
                }
            }
            "kick" => {
                let who = args.first().copied().unwrap_or("");
                let target = self
                    .players
                    .values()
                    .find(|p| p.name == who)
                    .map(|p| (p.id, p.name.clone()));
                if let Some((pid, name)) = target {
                    let nbt = Self::text_nbt("expulsé par un opérateur");
                    let pkt = W2::new().nbt(&nbt).done();
                    if let Some(p) = self.players.get(&pid) {
                        self.send(p, cb::KICK_DISCONNECT, &pkt);
                    }
                    self.leave(pid, "expulsé");
                    Some(format!("{} expulsé", name))
                } else {
                    Some(format!("joueur \"{}\" introuvable", who))
                }
            }
            "say" => {
                let msg = args.join(" ");
                if !msg.is_empty() {
                    let line = format!("§6[Serveur] §f{}", msg);
                    self.bcast_chat(&line);
                }
                None
            }
            "save" => {
                self.save();
                Some("monde sauvegardé".into())
            }
            "stop" => {
                self.stop_requested = true;
                Some("arrêt du serveur...".into())
            }
            "kill" => {
                let from_id = self
                    .players
                    .values()
                    .find(|p| p.name == from)
                    .map(|p| p.id);
                if let Some(fid) = from_id {
                    if let Some(p) = self.players.get_mut(&fid) {
                        p.invuln = 0.0;
                        p.hurt(1000, Vec3::new(0.0, 0.0, 0.0), 0.0);
                    }
                }
                None
            }
            _ => Some(format!("commande inconnue: {} (/help)", cmd)),
        };
        reply
    }

    /// Téléporte proprement un joueur (position + vidage des entités vues).
    fn teleport_player(&mut self, id: u32, t: Vec3) {
        let pkt = W2::new()
            .f64v(t.x as f64)
            .f64v(t.y as f64)
            .f64v(t.z as f64)
            .f32v(crate::mcproto::van_yaw_deg(0.0))
            .f32v(0.0)
            .u8v(0)
            .vi(9999)
            .done();
        if let Some(p) = self.players.get(&id) {
            self.send(p, cb::POSITION, &pkt);
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.seen.clear();
            p.peak_y = t.y;
        }
    }

    pub fn save(&mut self) {
        if save_world(&self.world, &self.cfg.world_path).is_ok() {
            self.log("monde sauvegardé");
        }
    }
}

// ------------------------------------------------------------ encodage chunk vanilla

/// Nombre de sections du monde (384/16).
const SECTIONS: usize = 24;

fn pack_longs(values: &[u32], bits: u32) -> Vec<i64> {
    let per = (64 / bits) as usize;
    let mut out = Vec::with_capacity((values.len() + per - 1) / per);
    let mut acc: u64 = 0;
    let mut j = 0usize;
    let mut cur: u64 = 0;
    for &v in values {
        acc |= (v as u64) << (j * bits as usize);
        j += 1;
        if j == per {
            out.push(acc as i64);
            acc = 0;
            j = 0;
        }
        cur = acc;
    }
    let _ = cur;
    if j != 0 {
        out.push(acc as i64);
    }
    out
}

fn ceil_log2(v: usize) -> u32 {
    let mut n = 0u32;
    let mut v = v.saturating_sub(1);
    while v > 0 {
        n += 1;
        v >>= 1;
    }
    n.max(1)
}

/// Encode une section en conteneur paletté (états de blocs ou biomes).
/// Renvoie (payload) prêt à l'emploi.
fn encode_container(entries: &[u32], min_indirect_bits: u32, global_bits: u32) -> Vec<u8> {
    // palette
    let mut palette: Vec<u32> = Vec::new();
    let mut idx = Vec::with_capacity(entries.len());
    for &v in entries {
        if let Some(i) = palette.iter().position(|&p| p == v) {
            idx.push(i as u32);
        } else {
            idx.push(palette.len() as u32);
            palette.push(v);
        }
    }
    let mut out = Vec::new();
    if palette.len() == 1 {
        // single valued: bits=0, valeur, array taille 0
        crate::mcproto::write_varint(&mut out, 0);
        crate::mcproto::write_varint(&mut out, palette[0] as i32);
        crate::mcproto::write_varint(&mut out, 0);
        return out;
    }
    let need = ceil_log2(palette.len());
    if need <= 8 {
        // indirect : bits (borné bas), palette varints, long array
        let bits = need.max(min_indirect_bits);
        crate::mcproto::write_varint(&mut out, bits as i32);
        crate::mcproto::write_varint(&mut out, palette.len() as i32);
        for p in &palette {
            crate::mcproto::write_varint(&mut out, *p as i32);
        }
        let longs = pack_longs(&idx, bits);
        crate::mcproto::write_varint(&mut out, longs.len() as i32);
        for l in longs {
            out.extend_from_slice(&l.to_be_bytes());
        }
    } else {
        // direct (palette globale)
        crate::mcproto::write_varint(&mut out, global_bits as i32);
        let longs = pack_longs(entries, global_bits);
        crate::mcproto::write_varint(&mut out, longs.len() as i32);
        for l in longs {
            out.extend_from_slice(&l.to_be_bytes());
        }
    }
    out
}

/// Payload "chunkData" + light pour un chunk (protocole vanilla).
pub fn encode_chunk_packet_data(
    world: &World,
    cx: i32,
    cz: i32,
    biome_col: &[u8; 256],
) -> (Vec<u8> /*chunkData*/, Vec<u8> /*light*/) {
    let chunk = &world.chunks[&(cx, cz)];
    let mut data = Vec::with_capacity(16 * 1024);
    for sy in 0..SECTIONS {
        let y0 = sy as i32 * 16 - 64; // min_y = -64
        let mut blocks = [0u32; 4096];
        let mut count = 0usize;
        let in_world = y0 >= 0 && y0 + 15 < CY as i32;
        if in_world {
            for ly in 0..16usize {
                for lz in 0..16usize {
                    for lx in 0..16usize {
                        let b = chunk.blocks[lx + 16 * (lz + 16 * (y0 as usize + ly))];
                        let st = vd::block_state_id(b);
                        if b != crate::world::AIR {
                            count += 1;
                        }
                        blocks[(ly << 8) | (lz << 4) | lx] = st;
                    }
                }
            }
        } else {
            blocks = [vd::block_state_id(crate::world::AIR); 4096];
        }
        data.extend_from_slice(&(count.min(0x7FFF) as i16).to_be_bytes());
        data.extend_from_slice(&encode_container(
            &blocks,
            4,
            vd::GLOBAL_BITS,
        ));
        // biomes 4x4x4 (échantillon horizontal, y uniforme dans notre monde)
        let mut biomes = [0u32; 64];
        for by in 0..4usize {
            for bz in 0..4usize {
                for bx in 0..4usize {
                    let lx = (bx * 4).min(15);
                    let lz = (bz * 4).min(15);
                    let b8 = biome_col[lx * 16 + lz] as usize;
                    biomes[(by << 4) | (bz << 2) | bx] = b8 as u32;
                }
            }
        }
        data.extend_from_slice(&encode_container(&biomes, 1, 4));
        let _ = in_world;
    }
    // (les block entities ne font PAS partie de chunkData : le paquet les
    // écrit après le buffer, cf. chunk_packet)

    // lumière : skylight par colonne (0 sous le sol, 15 au-dessus), blocklight vide
    let mut surface = [[0i32; 16]; 16]; // y du premier bloc non-air depuis le haut + 1
    for lx in 0..16usize {
        for lz in 0..16usize {
            let mut top = 0i32;
            for y in (0..CY).rev() {
                if chunk.blocks[lx + 16 * (lz + 16 * y)] != crate::world::AIR {
                    top = y as i32 + 1;
                    break;
                }
            }
            surface[lx][lz] = top;
        }
    }
    let mut sky_mask: Vec<i64> = vec![0; (SECTIONS + 63) / 64];
    let mut empty_mask: Vec<i64> = vec![0; (SECTIONS + 63) / 64];
    let mut arrays: Vec<Vec<u8>> = Vec::new();
    for sy in 0..SECTIONS {
        let y0 = sy as i32 * 16 - 64;
        // section hors du monde: au-dessus -> vide (15 partout => on envoie quand même 15 pour sy >= 12)
        let all_sky = y0 >= CY as i32;
        let all_dark = y0 + 15 < 0;
        if all_dark {
            empty_mask[sy / 64] |= 1i64 << (sy % 64);
            continue;
        }
        let mut arr = vec![0u8; 2048];
        if all_sky {
            for b in arr.iter_mut() {
                *b = 0xFF;
            }
        } else {
            let mut any = false;
            for ly in 0..16usize {
                for lz in 0..16usize {
                    for lx in 0..16usize {
                        let y = y0 as usize + ly;
                        let v = if y as i32 >= surface[lx][lz] { 15u8 } else { 0 };
                        if v != 0 {
                            any = true;
                        }
                        let i = (ly << 8) | (lz << 4) | lx;
                        if i % 2 == 0 {
                            arr[i / 2] |= v;
                        } else {
                            arr[i / 2] |= v << 4;
                        }
                    }
                }
            }
            if !any {
                empty_mask[sy / 64] |= 1i64 << (sy % 64);
                continue;
            }
        }
        sky_mask[sy / 64] |= 1i64 << (sy % 64);
        arrays.push(arr);
    }
    // masques + tableaux de lumière : chaque tableau d'i64 est précédé de son
    // nombre (varint) — sans ce compteur le client lit des masques vides
    // (monde noir) et désynchronise la fin du paquet.
    let mut light = Vec::new();
    crate::mcproto::write_varint(&mut light, sky_mask.len() as i32);
    for m in &sky_mask {
        light.extend_from_slice(&m.to_be_bytes());
    }
    // block light mask : aucun (0 long)
    crate::mcproto::write_varint(&mut light, 0);
    crate::mcproto::write_varint(&mut light, empty_mask.len() as i32);
    for m in &empty_mask {
        light.extend_from_slice(&m.to_be_bytes());
    }
    crate::mcproto::write_varint(&mut light, 0); // empty block light : aucun
    crate::mcproto::write_varint(&mut light, arrays.len() as i32);
    for a in &arrays {
        crate::mcproto::write_varint(&mut light, a.len() as i32);
        light.extend_from_slice(a);
    }
    crate::mcproto::write_varint(&mut light, 0); // block light arrays
    (data, light)
}

fn heightmaps_nbt(chunk: &crate::world::Chunk) -> Nbt {
    let mut motion = [0u32; 256];
    let mut world_surface = [0u32; 256];
    for lx in 0..16usize {
        for lz in 0..16usize {
            let mut top_m = 0i32;
            let mut top_s = 0i32;
            for y in (0..CY).rev() {
                let b = chunk.blocks[lx + 16 * (lz + 16 * y)];
                if b != crate::world::AIR {
                    if top_s == 0 {
                        top_s = y as i32 + 1;
                    }
                    // MOTION_BLOCKING: solide ou fluide (approx: tout sauf plantes/laie)
                    let blocking = crate::world::is_solid_id(b)
                        || b == WATER
                        || crate::world::shape_of(b) == crate::world::Shape::Full;
                    if blocking && top_m == 0 {
                        top_m = y as i32 + 1;
                    }
                }
                if top_m != 0 && top_s != 0 {
                    break;
                }
            }
            let i = lx * 16 + lz;
            motion[i] = (top_m - vd::WORLD_MIN_Y).max(0) as u32;
            world_surface[i] = (top_s - vd::WORLD_MIN_Y).max(0) as u32;
        }
    }
    let mut root = Nbt::compound();
    for (name, vals) in [("MOTION_BLOCKING", &motion), ("WORLD_SURFACE", &world_surface)] {
        let longs = pack_longs(vals, 9);
        root.set(name, Nbt::LongArray(longs.iter().map(|l| *l as i64).collect()));
    }
    root
}

impl Server {
    fn biome_column(&mut self, cx: i32, cz: i32) -> [u8; 256] {
        if let Some(b) = self.biome_cache.get(&(cx, cz)) {
            return *b;
        }
        let mut b = [0u8; 256];
        for lx in 0..16usize {
            for lz in 0..16usize {
                b[lx * 16 + lz] = self
                    .world
                    .biome_at(cx * 16 + lx as i32, cz * 16 + lz as i32);
            }
        }
        self.biome_cache.insert((cx, cz), b);
        b
    }

    /// Paquet map_chunk complet pour (cx, cz).
    fn chunk_packet(&mut self, cx: i32, cz: i32) -> Vec<u8> {
        if !self.world.chunks.contains_key(&(cx, cz)) {
            self.world.gen_chunk(cx, cz);
        }
        let biomes = self.biome_column(cx, cz);
        let (cdata, light) = encode_chunk_packet_data(&self.world, cx, cz, &biomes);
        let hm = heightmaps_nbt(self.world.chunks.get(&(cx, cz)).unwrap());
        let w = W2::new()
            .i32v(cx)
            .i32v(cz)
            .nbt(&hm)
            // longueur chunkData en VARINT (pas un i32 !)
            .vi(cdata.len() as i32)
            .bytes(&cdata)
            // block entities : champ séparé APRÈS le buffer (liste vide)
            .vi(0)
            .bytes(&light);
        w.done()
    }

    fn stream_chunks(&mut self) {
        let mut gen_budget = 8usize;
        let mut unload: Vec<(u32, Vec<(i32, i32)>)> = Vec::new();
        let to_send: Vec<(u32, Vec<Vec<u8>>)> = {
            let mut out = Vec::new();
            for pid in self.players.keys().copied().collect::<Vec<_>>() {
                let (px, pz) = {
                    let p = match self.players.get(&pid) {
                        Some(p) => p,
                        None => continue,
                    };
                    ((p.pos.x / 16.0).floor() as i32, (p.pos.z / 16.0).floor() as i32)
                };
                let view = self.cfg.view;
                // génération (budget partagé)
                let mut need: Vec<(i32, i32, i32)> = Vec::new();
                for dx in -(view + 1)..=(view + 1) {
                    for dz in -(view + 1)..=(view + 1) {
                        let c = (px + dx, pz + dz);
                        if !self.world.chunks.contains_key(&c) {
                            need.push((c.0, c.1, dx * dx + dz * dz));
                        }
                    }
                }
                need.sort_by_key(|&(_, _, d2)| d2);
                for &(cx, cz, _) in need.iter() {
                    if gen_budget == 0 {
                        break;
                    }
                    self.world.gen_chunk(cx, cz);
                    gen_budget -= 1;
                }
                // envoi (plus proche d'abord)
                let mut cands: Vec<(i32, i32, i32)> = Vec::new();
                for dx in -(view)..=(view) {
                    for dz in -(view)..=(view) {
                        let c = (px + dx, pz + dz);
                        let has = self
                            .players
                            .get(&pid)
                            .map(|p| p.sent.contains(&c))
                            .unwrap_or(true);
                        if has || !self.world.chunks.contains_key(&c) {
                            continue;
                        }
                        cands.push((c.0, c.1, dx * dx + dz * dz));
                    }
                }
                cands.sort_by_key(|&(_, _, d2)| d2);
                let mut pkts = Vec::new();
                for (cx, cz, _) in cands {
                    if pkts.len() >= 4 {
                        break;
                    }
                    let pkt = self.chunk_packet(cx, cz);
                    if let Some(p) = self.players.get_mut(&pid) {
                        p.sent.insert((cx, cz));
                    }
                    pkts.push(frame(cb::MAP_CHUNK, &pkt));
                }
                if !pkts.is_empty() {
                    out.push((pid, pkts));
                }
                // chunks à décharger
                let mut gone = Vec::new();
                for c in self.players.get(&pid).map(|p| p.sent.clone()).unwrap_or_default() {
                    let (dx, dz) = (c.0 - px, c.1 - pz);
                    if dx.abs() > view + 2 || dz.abs() > view + 2 {
                        gone.push(c);
                    }
                }
                if !gone.is_empty() {
                    unload.push((pid, gone));
                }
            }
            out
        };
        for (pid, pkts) in to_send {
            if let Some(p) = self.players.get(&pid) {
                for pkt in pkts {
                    let _ = p.out.send(pkt);
                }
            }
        }
        for (pid, gone) in unload {
            if let Some(p) = self.players.get_mut(&pid) {
                for c in &gone {
                    p.sent.remove(c);
                }
                let mut w = W2::new();
                w = w.vi(gone.len() as i32);
                for (cx, cz) in &gone {
                    // unload_chunk: chunkZ d'abord, puis chunkX
                    w = w.i32v(*cz).i32v(*cx);
                }
                let pkt = w.done();
                let _ = p.out.send(frame(cb::UNLOAD_CHUNK, &pkt));
            }
        }
    }

    // ----------------------------------------------------------------- tick
    pub fn tick(&mut self) {
        self.tick_n += 1;
        let dt = 1.0 / TPS as f32;
        self.time = (self.time + dt / DAY_CYCLE) % 1.0;

        // connexions mortes (canal fermé)
        let mut dead_conns = Vec::new();
        for p in self.players.values() {
            if p.out.send(Vec::new()).is_err() {
                dead_conns.push(p.id);
            }
        }
        for id in dead_conns {
            self.leave(id, "déconnecté");
        }

        // keepalive + timeout
        let now = Instant::now();
        let mut kick = Vec::new();
        for p in self.players.values_mut() {
            match p.keepalive_sent {
                None => {
                    if p.out.send(Vec::new()).is_ok() {
                        let id = now.elapsed().as_secs() as i64 ^ (p.id as i64) ^ (self.tick_n as i64);
                        let pkt = frame(cb::KEEP_ALIVE, &W2::new().i64v(id).done());
                        let _ = p.out.send(pkt);
                        p.keepalive_sent = Some((id, now));
                    }
                }
                Some((_, t)) => {
                    if now - t > Duration::from_secs(30) {
                        kick.push(p.id);
                    }
                }
            }
        }
        for id in kick {
            self.leave(id, "timed out");
        }

        // sync heure toutes les 5 s
        if self.tick_n % 100 == 0 {
            let pkt = W2::new()
                .i64v(self.tick_n as i64)
                .i64v((self.time * 24000.0) as i64)
                .done();
            self.bcast(cb::UPDATE_TIME, &pkt);
        }

        let (light, _night, _fog, _sun, _elev) = sky_state(self.time);

        // survie (faim/régén/famine)
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let Some(p) = self.players.get_mut(&id) else { continue };
            p.invuln = (p.invuln - dt).max(0.0);
            p.eat_cd = (p.eat_cd - dt).max(0.0);
            if p.dead || p.creative {
                continue;
            }
            let drain = if p.sprint { 0.06 } else { 0.012 };
            p.hunger = (p.hunger - drain * dt).max(0.0);
            if p.hunger < 0.5 && p.hp > 1 {
                p.starve_cd -= dt;
                if p.starve_cd <= 0.0 {
                    p.starve_cd = 4.0;
                    p.hp -= 1;
                }
            }
            if p.hunger >= 18.0 && p.hp < 20 {
                p.regen_cd -= dt;
                if p.regen_cd <= 0.0 {
                    p.regen_cd = 2.0;
                    p.hp += 1;
                    p.hunger = (p.hunger - 0.4).max(0.0);
                }
            }
        }

        // annonces de mort
        let ids: Vec<u32> = self.players.keys().copied().collect();
        for id in ids {
            let (dead, name, announced) = {
                let p = self.players.get(&id).unwrap();
                (p.dead, p.name.clone(), p.announced)
            };
            if dead && !announced {
                let msg = format!("§c{} est mort", name);
                self.bcast_chat(&msg);
                if let Some(p) = self.players.get_mut(&id) {
                    p.announced = true;
                }
                self.log(&format!("{} est mort", name));
            }
        }
        self.sync_health();

        self.stream_chunks();

        // mobs
        let centers: Vec<Vec3> = self
            .players
            .values()
            .filter(|p| !p.dead)
            .map(|p| p.pos)
            .collect();
        let mut changed_blocks: Vec<(i32, i32, i32)> = Vec::new();
        {
            let mut targets = Targets(&mut self.players);
            mobs::spawn_tick(
                &mut self.mobs,
                &self.world,
                &centers,
                light,
                dt,
                &mut self.spawner,
            );
            let mut on_block = |x: i32, y: i32, z: i32| changed_blocks.push((x, y, z));
            mobs::update_mobs(
                &mut self.mobs,
                &mut self.world,
                &mut targets,
                &mut self.particles,
                light,
                0.0,
                dt,
                &mut on_block,
            );
        }
        for m in self.mobs.iter_mut() {
            if m.uid == 0 {
                m.uid = self.next_uid;
                self.next_uid += 1;
            }
        }
        for (x, y, z) in changed_blocks {
            let b = self.world.get_block(x, y, z);
            let pkt = W2::new()
                .pos(x, y, z)
                .vi(vd::block_state_id(b) as i32)
                .done();
            self.bcast(cb::BLOCK_CHANGE, &pkt);
        }

        // entités 10 Hz
        if self.tick_n % 2 == 0 {
            self.send_entities();
        }
    }

    /// Spawns / mouvements / destructions d'entités incrémentaux par joueur.
    fn send_entities(&mut self) {
        struct Ent {
            id: u32,
            kind: u8,
            pos: Vec3,
            yaw: f32,
            uuid: [u8; 16],
        }
        let mut ents: Vec<Ent> = Vec::with_capacity(self.mobs.len() + self.players.len());
        for m in &self.mobs {
            ents.push(Ent {
                id: m.uid,
                kind: m.kind.id(),
                pos: m.pos,
                yaw: m.yaw,
                uuid: crate::mcproto::random_uuid(&mut self.uuid_rng),
            });
        }
        let player_ents: Vec<Ent> = self
            .players
            .values()
            .map(|p| Ent {
                id: p.id,
                kind: MobKind::Player.id(),
                pos: p.pos,
                yaw: p.yaw,
                uuid: p.uuid,
            })
            .collect();
        ents.extend(player_ents);

        let ids: Vec<u32> = self.players.keys().copied().collect();
        for pid in ids {
            let Some(me) = self.players.get_mut(&pid) else { continue };
            let me_pos = me.pos;
            let me_id = pid;
            let mut txs: Vec<(i32, Vec<u8>)> = Vec::new();
            let mut found: HashSet<u32> = HashSet::new();
            for e in &ents {
                if e.id == 0 {
                    continue;
                }
                let d = (e.pos - me_pos).length();
                let is_me = e.id == me_id && e.kind == MobKind::Player.id();
                if is_me || d > 96.0 {
                    continue;
                }
                found.insert(e.id);
                let prev = me.seen.get(&e.id).copied();
                let yaw8 = crate::mcproto::angle_i8(crate::mcproto::van_yaw_deg(e.yaw));
                match prev {
                    None => {
                        // spawn_entity
                        txs.push((
                            cb::SPAWN_ENTITY,
                            W2::new()
                                .vi(e.id as i32)
                                .uuid(&e.uuid)
                                .vi(vd::entity_type_id(e.kind))
                                .f64v(e.pos.x as f64)
                                .f64v(e.pos.y as f64)
                                .f64v(e.pos.z as f64)
                                .i8v(0) // pitch
                                .u8v(yaw8) // yaw
                                .u8v(yaw8) // head yaw
                                .vi(0) // data
                                .i16v(0)
                                .i16v(0)
                                .i16v(0)
                                .done(),
                        ));
                    }
                    Some(old) => {
                        let np = e.pos;
                        let dx = np.x - old.0;
                        let dy = np.y - old.1;
                        let dz = np.z - old.2;
                        let dist2 = dx * dx + dy * dy + dz * dz;
                        if dist2 > 64.0 * 64.0 {
                            txs.push((
                                cb::ENTITY_TELEPORT,
                                W2::new()
                                    .vi(e.id as i32)
                                    .f64v(np.x as f64)
                                    .f64v(np.y as f64)
                                    .f64v(np.z as f64)
                                    .u8v(yaw8)
                                    .i8v(0)
                                    .b(true)
                                    .done(),
                            ));
                        } else if dist2 > 0.0004 {
                            txs.push((
                                cb::ENTITY_MOVE_LOOK,
                                W2::new()
                                    .vi(e.id as i32)
                                    .i16v((dx * 4096.0) as i16)
                                    .i16v((dy * 4096.0) as i16)
                                    .i16v((dz * 4096.0) as i16)
                                    .u8v(yaw8)
                                    .i8v(0)
                                    .b(true)
                                    .done(),
                            ));
                        }
                    }
                }
                // head rotation
                txs.push((cb::ENTITY_HEAD_ROTATION, W2::new().vi(e.id as i32).u8v(yaw8).done()));
            }
            // destructions
            let gone: Vec<u32> = me
                .seen
                .keys()
                .filter(|k| !found.contains(k))
                .copied()
                .collect();
            if !gone.is_empty() {
                let mut w = W2::new().vi(gone.len() as i32);
                for g in &gone {
                    w = w.vi(*g as i32);
                }
                txs.push((cb::ENTITY_DESTROY, w.done()));
            }
            // maj seen
            for e in &ents {
                if found.contains(&e.id) {
                    me.seen.insert(e.id, (e.pos.x, e.pos.y, e.pos.z));
                }
            }
            for g in &gone {
                me.seen.remove(g);
            }
            for t in &txs {
                let _ = me.out.send(frame(t.0, &t.1));
            }
        }
    }
}

// ---------------------------------------------------------------- running

#[derive(Debug)]
enum CState {
    Handshake,
    Status,
    Login,
    Config,
    Play,
}

#[allow(clippy::needless_return)]
fn conn_threads(srv: Arc<Mutex<Server>>, stream: TcpStream) {
    // trace optionnelle pour le débogage réseau (RV_DEBUG=1)
    let dbg = std::env::var("RV_DEBUG").as_deref() == Ok("1");
    macro_rules! dbglog {
        ($($a:tt)*) => {
            if dbg {
                eprintln!("[net] {}", format_args!($($a)*));
            }
        };
    }
    stream.set_nodelay(true).ok();
    stream.set_nonblocking(false).ok();
    stream.set_write_timeout(Some(Duration::from_secs(30))).ok();
    let Ok(mut writer_stream) = stream.try_clone() else { return };
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let writer = std::thread::spawn(move || {
        for buf in rx {
            if buf.is_empty() {
                continue;
            }
            if writer_stream.write_all(&buf).is_err() {
                break;
            }
        }
    });
    let mut state = CState::Handshake;
    let mut my_id: Option<u32> = None;
    let mut login_name: Option<String> = None;
    let mut s = stream;
    let res = loop {
        let payload = match crate::mcproto::read_frame(&mut s) {
            Ok(p) => p,
            Err(e) => {
                dbglog!("read_frame err: {e}");
                break Ok::<(), ()>(());
            }
        };
        let mut r = R2::new(&payload);
        let pid = match r.vi() {
            Some(v) => {
                dbglog!("state={:?} pid=0x{:02X}", state, v);
                v
            }
            None => {
                dbglog!("trame illisible ({} octets)", payload.len());
                break Ok::<(), ()>(());
            }
        };
        match state {
            CState::Handshake => {
                if pid != 0x00 {
                    break Ok::<(), ()>(());
                }
                let _proto = r.vi().unwrap_or(0);
                let _host = r.s(255).unwrap_or_default();
                let _port = r.u16v().unwrap_or(0);
                let next = r.vi().unwrap_or(0);
                state = match next {
                    1 => CState::Status,
                    2 => CState::Login,
                    _ => break Ok::<(), ()>(()),
                };
            }
            CState::Status => {
                match pid {
                    sb::PING_START => {
                        let g = srv.lock().unwrap();
                        let sample: Vec<crate::json::Json> = g
                            .players
                            .values()
                            .take(5)
                            .map(|p| {
                                crate::json::Json::Obj(vec![
                                    ("name".into(), crate::json::Json::str(&p.name)),
                                    ("id".into(), crate::json::Json::str(&uuid_hex(&p.uuid))),
                                ])
                            })
                            .collect();
                        let desc = crate::json::Json::Obj(vec![(
                            "text".into(),
                            crate::json::Json::str(&g.cfg.motd),
                        )]);
                        let j = crate::json::Json::Obj(vec![
                            (
                                "version".into(),
                                crate::json::Json::Obj(vec![
                                    ("name".into(), crate::json::Json::str(vd::MC_VERSION_NAME)),
                                    (
                                        "protocol".into(),
                                        crate::json::Json::num(vd::PROTOCOL_VERSION as f64),
                                    ),
                                ]),
                            ),
                            (
                                "players".into(),
                                crate::json::Json::Obj(vec![
                                    ("max".into(), crate::json::Json::num(g.cfg.max_players as f64)),
                                    (
                                        "online".into(),
                                        crate::json::Json::num(g.players.len() as f64),
                                    ),
                                    ("sample".into(), crate::json::Json::Arr(sample)),
                                ]),
                            ),
                            ("description".into(), desc),
                        ]);
                        let payload = W2::new().s(&j.to_string()).done();
                        drop(g);
                        let _ = tx.send(frame(cb::STATUS_SERVER_INFO, &payload));
                    }
                    sb::PING => {
                        let t = r.i64v().unwrap_or(0);
                        let _ = tx.send(frame(0x01, &W2::new().i64v(t).done()));
                        break Ok::<(), ()>(()); // le ping ferme la connexion
                    }
                    _ => {}
                }
            }
            CState::Login => {
                match pid {
                    sb::LOGIN_START => {
                        let name = r.s(16).unwrap_or_default();
                        // 1.20.5 : UUID toujours présent
                        let uuid_req = r.uuid().unwrap_or([0; 16]);
                        let uuid = if uuid_req == [0; 16] {
                            crate::mcproto::offline_uuid(&name)
                        } else {
                            uuid_req
                        };
                        // plein ?
                        {
                            let g = srv.lock().unwrap();
                            if g.players.len() >= g.cfg.max_players {
                                let nbt = Server::text_nbt("§cServeur plein");
                                let pkt = frame(
                                    cb::LOGIN_DISCONNECT,
                                    &W2::new().nbt(&nbt).done(),
                                );
                                let _ = tx.send(pkt);
                                break Ok::<(), ()>(());
                            }
                        }
                        // login_success
                        login_name = Some(name.clone());
                        // 1.20.5 : uuid + nom + propriétés + strictErrorHandling
                        // (bool obligatoire — son absence désynchronise le client vanilla
                        //  et les traducteurs de protocole type ViaVersion)
                        let pkt = frame(
                            cb::LOGIN_SUCCESS,
                            &W2::new()
                                .uuid(&uuid)
                                .s(&name)
                                .vi(0) // 0 propriété
                                .b(true) // strictErrorHandling (vanilla = true)
                                .done(),
                        );
                        if tx.send(pkt).is_err() {
                            break Ok::<(), ()>(());
                        }
                    }
                    sb::LOGIN_ACK => {
                        // configuration : registres + finish
                        let pkts: Vec<Vec<u8>> = {
                            let g = srv.lock().unwrap();
                            g.codec.clone()
                        };
                        for p in pkts {
                            if tx.send(p).is_err() {
                                break;
                            }
                        }
                        let _ = tx.send(frame(cb::CFG_FINISH, &[]));
                        state = CState::Config;
                    }
                    _ => {}
                }
            }
            CState::Config => {
                match pid {
                    sb::CFG_FINISH => {
                        // le client a confirmé -> play
                        let mut g = srv.lock().unwrap();
                        let name = login_name
                            .take()
                            .filter(|n| !n.is_empty())
                            .unwrap_or_else(|| format!("Joueur{}", g.next_id));
                        let uuid = crate::mcproto::offline_uuid(&name);
                        let id = g.join(name, uuid, tx.clone());
                        my_id = Some(id);
                        state = CState::Play;
                    }
                    sb::CFG_SETTINGS | sb::CFG_PLUGIN_MESSAGE | sb::CFG_SELECT_KNOWN_PACKS
                    | sb::CFG_KEEP_ALIVE | sb::CFG_PONG => {}
                    _ => {}
                }
            }
            CState::Play => {
                let id = match my_id {
                    Some(i) => i,
                    None => break Ok::<(), ()>(()),
                };
                let mut g = srv.lock().unwrap();
                if !g.handle(id, pid, &mut r) {
                    dbglog!("handle=false, fin de connexion");
                    break Ok::<(), ()>(());
                }
            }
        }
    };
    let _ = res;
    if let Some(id) = my_id {
        srv.lock().unwrap().leave(id, "déconnexion");
    }
    drop(tx);
    let _ = writer.join();
}

fn uuid_hex(u: &[u8; 16]) -> String {
    u.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Dedicated entry point: bind on 0.0.0.0:port, console commands from stdin.
pub fn serve(cfg: ServerConfig, running: Arc<AtomicBool>) -> std::io::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", cfg.port))?;

    let console_q: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    {
        let q = console_q.clone();
        let running2 = running.clone();
        std::thread::spawn(move || {
            let mut line = String::new();
            loop {
                line.clear();
                match std::io::stdin().read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                let is_stop = trimmed.starts_with("/stop") || trimmed == "stop";
                q.lock().unwrap().push(trimmed);
                if is_stop {
                    running2.store(false, Ordering::SeqCst);
                }
            }
        });
    }

    serve_listener(listener, cfg, running, Some(console_q))
}

/// Core server loop over an already-bound listener.
pub fn serve_listener(
    listener: TcpListener,
    cfg: ServerConfig,
    running: Arc<AtomicBool>,
    console_q: Option<Arc<Mutex<Vec<String>>>>,
) -> std::io::Result<()> {
    let addr = listener.local_addr()?;
    let srv = Arc::new(Mutex::new(Server::new(cfg)));
    {
        let g = srv.lock().unwrap();
        g.log(&format!(
            "serveur vanilla 1.20.5 (protocole {}) en écoute sur {}",
            vd::PROTOCOL_VERSION, addr
        ));
    }
    listener.set_nonblocking(true)?;

    let mut next_tick = Instant::now() + Duration::from_millis(TICK_MS);
    let mut last_save = Instant::now();
    while running.load(Ordering::SeqCst) {
        if let Some(q) = &console_q {
            let lines: Vec<String> = std::mem::take(&mut *q.lock().unwrap());
            for line in lines {
                let mut g = srv.lock().unwrap();
                if let Some(reply) = g.command("Console", &line) {
                    g.log(&reply);
                }
            }
        }
        for _ in 0..8 {
            match listener.accept() {
                Ok((s, _addr)) => {
                    let srv2 = srv.clone();
                    std::thread::spawn(move || conn_threads(srv2, s));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        let now = Instant::now();
        if now >= next_tick {
            {
                let mut g = srv.lock().unwrap();
                g.tick();
                if g.stop_requested {
                    drop(g);
                    break;
                }
            }
            next_tick += Duration::from_millis(TICK_MS);
            if next_tick < now - Duration::from_millis(500) {
                next_tick = now + Duration::from_millis(TICK_MS);
            }
        }
        if last_save.elapsed() >= Duration::from_secs(60) {
            last_save = Instant::now();
            srv.lock().unwrap().save();
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    {
        let mut g = srv.lock().unwrap();
        g.save();
    }
    Ok(())
}

/// Integrated server: bind on 127.0.0.1:0 in a background thread and return
/// the bound address. Stops by itself once the last player leaves.
pub fn spawn_local(cfg: ServerConfig) -> std::io::Result<SocketAddr> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let addr = listener.local_addr()?;
    let running = Arc::new(AtomicBool::new(true));
    let running2 = running.clone();
    let cfg2 = cfg;
    let l2 = listener;
    std::thread::spawn(move || {
        let srv = Arc::new(Mutex::new(Server::new(cfg2)));
        l2.set_nonblocking(true).ok();
        let mut next_tick = Instant::now() + Duration::from_millis(TICK_MS);
        let mut empty_since: Option<Instant> = None;
        loop {
            if !running2.load(Ordering::SeqCst) {
                break;
            }
            for _ in 0..8 {
                match l2.accept() {
                    Ok((s, _)) => {
                        let srv2 = srv.clone();
                        std::thread::spawn(move || conn_threads(srv2, s));
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }
            let now = Instant::now();
            if now >= next_tick {
                let mut g = srv.lock().unwrap();
                g.tick();
                let empty = g.players.is_empty();
                let stop = g.stop_requested;
                drop(g);
                if stop {
                    break;
                }
                next_tick += Duration::from_millis(TICK_MS);
                if empty {
                    match empty_since {
                        None => empty_since = Some(now),
                        Some(t) if now - t > Duration::from_secs(8) => break,
                        Some(_) => {}
                    }
                } else {
                    empty_since = None;
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut g = srv.lock().unwrap();
        g.save();
    });
    Ok(SocketAddr::from(addr))
}

pub use std::net::SocketAddr;

// -------------------------------------------------------------------- tests
#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcproto::{read_frame, write_frame, write_string, write_varint};

    fn test_cfg(name: &str) -> ServerConfig {
        ServerConfig {
            port: 0,
            max_players: 8,
            view: 2,
            motd: format!("test {name}"),
            seed: 4242,
            world_path: std::env::temp_dir().join(format!("rvx_srv_{name}_{}.sav", std::process::id())),
            save_interval_s: 9999,
            verbose: false,
            stop_when_empty: true,
        }
    }

    /// Client vanilla minimal pour les tests.
    struct VClient {
        s: TcpStream,
    }

    impl VClient {
        fn handshake(addr: std::net::SocketAddr, next: i32) -> VClient {
            let mut s = TcpStream::connect(addr).unwrap();
            s.set_nonblocking(false).unwrap();
            s.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
            s.set_write_timeout(Some(Duration::from_secs(15))).unwrap();
            let mut h = Vec::new();
            write_varint(&mut h, 0x00);
            write_varint(&mut h, vd::PROTOCOL_VERSION);
            write_string(&mut h, "localhost");
            h.extend_from_slice(&25565u16.to_be_bytes());
            write_varint(&mut h, next);
            write_frame(&mut s, &h).unwrap();
            VClient { s }
        }

        fn login(addr: std::net::SocketAddr, name: &str) -> VClient {
            let mut c = Self::handshake(addr, 2);
            // login_start: nom + uuid
            let mut p = Vec::new();
            write_varint(&mut p, sb::LOGIN_START);
            write_string(&mut p, name);
            p.extend_from_slice(&crate::mcproto::offline_uuid(name));
            write_frame(&mut c.s, &p).unwrap();
            // login_success : uuid + nom + propriétés + strictErrorHandling
            let r = read_frame(&mut c.s).unwrap();
            let mut rr = R2::new(&r);
            assert_eq!(rr.vi(), Some(cb::LOGIN_SUCCESS));
            let _u = rr.uuid().unwrap();
            let _n = rr.s(16).unwrap();
            let props = rr.vi().unwrap();
            assert_eq!(props, 0, "0 propriété attendue");
            assert!(rr.b().unwrap(), "strictErrorHandling obligatoire en 1.20.5");
            // login_acknowledged
            let mut a = Vec::new();
            write_varint(&mut a, sb::LOGIN_ACK);
            write_frame(&mut c.s, &a).unwrap();
            // configuration: registres (skip) + finish
            loop {
                let r = read_frame(&mut c.s).unwrap();
                let mut rr = R2::new(&r);
                let id = rr.vi().unwrap();
                if id == cb::CFG_REGISTRY_DATA {
                    let _rid = rr.s(64).unwrap();
                    let n = rr.vi().unwrap();
                    for _ in 0..n {
                        let _k = rr.s(64).unwrap();
                        let has = rr.b().unwrap();
                        if has {
                            let mut nr = crate::nbt::NbtRdr::new(rr.rest());
                            nr.read_network().unwrap();
                            rr.p += nr.pos();
                        }
                    }
                } else if id == cb::CFG_FINISH {
                    break;
                }
            }
            let mut a = Vec::new();
            write_varint(&mut a, sb::CFG_FINISH);
            write_frame(&mut c.s, &a).unwrap();
            // play: le premier paquet doit être LOGIN (0x2B)
            let r = read_frame(&mut c.s).unwrap();
            let mut rr = R2::new(&r);
            assert_eq!(rr.vi(), Some(cb::LOGIN), "le play doit ouvrir avec login");
            c
        }

        fn send(&mut self, id: i32, payload: &[u8]) {
            let mut body = Vec::new();
            write_varint(&mut body, id);
            body.extend_from_slice(payload);
            write_frame(&mut self.s, &body).unwrap();
        }

        fn recv(&mut self) -> (i32, Vec<u8>) {
            let r = read_frame(&mut self.s).unwrap();
            let mut rr = R2::new(&r);
            let id = rr.vi().unwrap();
            (id, r[crate::mcproto::varint_len(id as u32)..].to_vec())
        }

        fn recv_until(&mut self, id: i32) -> (i32, Vec<u8>) {
            loop {
                let (o, p) = self.recv();
                if o == id {
                    return (o, p);
                }
            }
        }
    }

    #[test]
    fn status_reply_reports_motd_vanilla() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let cfg = test_cfg("status");
        let running = Arc::new(AtomicBool::new(true));
        let r2 = running.clone();
        let jh = std::thread::spawn(move || serve_listener(listener, cfg, r2, None).unwrap());

        let mut c = VClient::handshake(addr, 1);
        let mut p = Vec::new();
        write_varint(&mut p, sb::PING_START);
        write_frame(&mut c.s, &p).unwrap();
        let (_, payload) = c.recv();
        let mut rr = R2::new(&payload);
        let json = crate::json::parse(&rr.s(32768).unwrap()).unwrap();
        assert_eq!(json.get("description").unwrap().get("text").unwrap().as_str(), Some("test status"));
        assert_eq!(
            json.get("version").unwrap().get("protocol").unwrap().as_num(),
            Some(vd::PROTOCOL_VERSION as f64)
        );
        // ping echo
        let mut p = Vec::new();
        write_varint(&mut p, sb::PING);
        p.extend_from_slice(&1234567i64.to_be_bytes());
        write_frame(&mut c.s, &p).unwrap();
        let r = read_frame(&mut c.s).unwrap();
        let mut rr = R2::new(&r);
        assert_eq!(rr.vi(), Some(0x01));
        assert_eq!(rr.i64v(), Some(1234567));

        running.store(false, Ordering::SeqCst);
        jh.join().unwrap();
    }

    #[test]
    fn two_players_share_blocks_and_chat_vanilla() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let cfg = test_cfg("multi");
        let running = Arc::new(AtomicBool::new(true));
        let r2 = running.clone();
        let jh = std::thread::spawn(move || serve_listener(listener, cfg, r2, None).unwrap());

        let mut c1 = VClient::login(addr, "Alice");
        let _ = c1.recv_until(cb::POSITION);
        let _ = c1.recv_until(cb::MAP_CHUNK);

        let mut c2 = VClient::login(addr, "Bob");
        let _ = c2.recv_until(cb::MAP_CHUNK);let _ = c1.recv_until(cb::MAP_CHUNK);

        // Alice monte au-dessus du sol et pose une pierre contre un bloc
        let spawn = {
            let mut g = srv_lock(&jh, &running);
            g.spawn
        };
        let (bx, by, bz) = (spawn.x as i32, spawn.y as i32 + 6, spawn.z as i32);
        c1.send(
            sb::POSITION,
            &W2::new()
                .f64v(spawn.x as f64)
                .f64v((spawn.y + 10.0) as f64)
                .f64v(spawn.z as f64)
                .b(true)
                .done(),
        );
        std::thread::sleep(Duration::from_millis(120));
        // slot 0 = stone par défaut -> place au-dessus du bloc visé (face +Y)
        c1.send(
            sb::BLOCK_PLACE,
            &W2::new()
                .vi(0)
                .pos(bx, by - 1, bz)
                .vi(1)
                .f32v(0.5)
                .f32v(0.0)
                .f32v(0.5)
                .b(false)
                .vi(0)
                .done(),
        );
        let (_, p) = c2.recv_until(cb::BLOCK_CHANGE);
        let mut rr = R2::new(&p);
        assert_eq!(rr.pos(), Some((bx, by, bz)));
        assert_eq!(
            rr.vi(),
            Some(vd::block_state_id(crate::world::STONE) as i32)
        );


        // chat relayé
        c2.send(
            sb::CHAT_MESSAGE,
            &W2::new()
                .s("salut Alice")
                .i64v(0)
                .i64v(0)
                .b(false)
                .vi(0)
                .bytes(&[0; 3])
                .done(),
        );
        loop {
            let (_, p) = c1.recv_until(cb::SYSTEM_CHAT);
            let mut rr = R2::new(&p);
            let mut nr = crate::nbt::NbtRdr::new(rr.rest());
            let nbt = nr.read_network().unwrap();
            let _bar = rr.b().unwrap();
            let text = nbt.get("text").and_then(|t| t.as_str()).unwrap_or("");
            if text.contains("salut Alice") {
                assert!(text.contains("Bob"), "{text}");
                break;
            }
        }

        // commande /seed
        c1.send(
            sb::CHAT_MESSAGE,
            &W2::new()
                .s("/seed")
                .i64v(0)
                .i64v(0)
                .b(false)
                .vi(0)
                .bytes(&[0; 3])
                .done(),
        );
        loop {
            let (_, p) = c1.recv_until(cb::SYSTEM_CHAT);
            let mut rr = R2::new(&p);
            let mut nr = crate::nbt::NbtRdr::new(rr.rest());
            let nbt = nr.read_network().unwrap();
            let _ = rr.b();
            let text = nbt.get("text").and_then(|t| t.as_str()).unwrap_or("");
            if text.contains("4242") {
                break;
            }
        }

        // creuser le bloc : les deux voient l'air
        c1.send(
            sb::BLOCK_DIG,
            &W2::new().vi(0).pos(bx, by, bz).i8v(0).vi(0).done(),
        );
        let (_, p) = c2.recv_until(cb::BLOCK_CHANGE);
        let mut rr = R2::new(&p);
        assert_eq!(rr.pos(), Some((bx, by, bz)));
        assert_eq!(rr.vi(), Some(0)); // air

        // Bob part -> Alice reçoit player_remove
        drop(c2);
        let _ = c1.recv_until(cb::PLAYER_REMOVE);

        running.store(false, Ordering::SeqCst);
        jh.join().unwrap();
    }

    // accède au spawn du serveur de test (nécessite un accès direct; simplifié ici:
    // on relit le spawn via une instance séparée)
    fn srv_lock(
        _jh: &std::thread::JoinHandle<()>,
        _running: &Arc<AtomicBool>,
    ) -> Server {
        let cfg = test_cfg("spawn_probe");
        Server::new(cfg)
    }

    #[test]
    fn spawn_local_stops_when_everyone_leaves_vanilla() {
        let cfg = test_cfg("local");
        let addr = spawn_local(cfg).unwrap();
        let mut c = VClient::login(addr, "Solo");
        let _ = c.recv_until(cb::POSITION);
        // keepalive echo
        drop(c);
        std::thread::sleep(Duration::from_secs(10));
        assert!(
            TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_err(),
            "le serveur intégré doit s'arrêter"
        );
    }

    #[test]
    fn placed_blocks_survive_save_via_server() {
        let path = std::env::temp_dir().join(format!("rvx_srv_place_{}.sav", std::process::id()));
        std::fs::remove_file(&path).ok();
        let cfg = test_cfg("place");
        assert_eq!(cfg.world_path, path);
        let mut srv = Server::new(cfg);
        let (tx, _rx) = std::sync::mpsc::channel();
        let id = srv.join("Tester".to_string(), crate::mcproto::offline_uuid("Tester"), tx);
        srv.players.get_mut(&id).unwrap().pos = Vec3::new(5.5, 86.0, 5.5);
        srv.do_place(id, 5, 90, 5, crate::world::STONE);
        assert_eq!(srv.world.get_block(5, 90, 5), crate::world::STONE);
        for _ in 0..30 {
            srv.tick();
        }
        assert_eq!(srv.world.get_block(5, 90, 5), crate::world::STONE);
        srv.save();
        assert!(path.exists());
        let w2 = load_world(&path).expect("save must load");
        assert_eq!(w2.get_block(5, 90, 5), crate::world::STONE);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn world_save_roundtrip_via_server() {
        let path = std::env::temp_dir().join(format!("rvx_srvsave_{}.sav", std::process::id()));
        let mut w = World::new(99);
        w.gen_chunk(0, 0);
        w.set_block(3, 70, 3, crate::world::CHERRY_LOG);
        save_world(&w, &path).unwrap();
        let w2 = load_world(&path).unwrap();
        assert_eq!(w2.seed, 99);
        assert_eq!(w2.get_block(3, 70, 3), crate::world::CHERRY_LOG);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn chunk_encode_decode_roundtrip() {
        // encode un chunk et vérifie que la structure palettée est cohérente
        let mut w = World::new(7);
        w.gen_chunk(0, 0);
        w.set_block(1, 50, 1, crate::world::TORCH);
        let mut biomes = [0u8; 256];
        let (cdata, _light) = encode_chunk_packet_data(&w, 0, 0, &biomes);
        // parse : 24 sections + liste block entities vide
        let mut r = R2::new(&cdata);
        for _ in 0..SECTIONS {
            let count = r.i16v().unwrap();
            let _ = count;
            // états de blocs
            let bits = r.vi().unwrap();
            if bits == 0 {
                // single valued
                let _v = r.vi().unwrap();
                let n = r.vi().unwrap();
                assert_eq!(n, 0);
            } else if bits <= 8 {
                let plen = r.vi().unwrap();
                let _pal: Vec<i32> = (0..plen).map(|_| r.vi().unwrap()).collect();
                let nlongs = r.vi().unwrap();
                assert_eq!(nlongs as usize, (4096 + (64 / bits as usize) - 1) / (64 / bits as usize));
                r.skip(nlongs as usize * 8).unwrap();
            } else {
                let nlongs = r.vi().unwrap();
                r.skip(nlongs as usize * 8).unwrap();
            }
            // biomes
            let bbits = r.vi().unwrap();
            if bbits == 0 {
                let _v = r.vi().unwrap();
                let n = r.vi().unwrap();
                assert_eq!(n, 0);
            } else if bbits <= 8 {
                let plen = r.vi().unwrap();
                let _pal: Vec<i32> = (0..plen).map(|_| r.vi().unwrap()).collect();
                let nlongs = r.vi().unwrap();
                r.skip(nlongs as usize * 8).unwrap();
            } else {
                let nlongs = r.vi().unwrap();
                r.skip(nlongs as usize * 8).unwrap();
            }
        }
        assert_eq!(r.p, cdata.len(), "chunkData consommé à l'octet près");
    }

    /// Repasse le paquet map_chunk COMPLET comme le lit le client vanilla :
    /// x, z, heightmaps NBT, buffer varint-length, block entities, masques
    /// (avec varint de compte) et tableaux de lumière. Valide le framing réel.
    #[test]
    fn map_chunk_packet_framing_matches_vanilla() {
        let mut w = World::new(9);
        w.gen_chunk(1, -1);
        let biomes = [2u8; 256];
        let (cdata, light) = encode_chunk_packet_data(&w, 1, -1, &biomes);
        let hm = heightmaps_nbt(w.chunks.get(&(1, -1)).unwrap());
        let mut p = W2::new()
            .i32v(1)
            .i32v(-1)
            .nbt(&hm)
            .vi(cdata.len() as i32)
            .bytes(&cdata)
            .vi(0)
            .bytes(&light);
        let pkt = p.done();

        let mut r = R2::new(&pkt);
        assert_eq!(r.i32v(), Some(1));
        assert_eq!(r.i32v(), Some(-1));
        {
            let mut nr = crate::nbt::NbtRdr::new(r.rest());
            nr.read_network().unwrap();
            r.p += nr.pos();
        }
        // buffer chunkData (varint)
        let clen = r.vi().unwrap() as usize;
        assert_eq!(clen, cdata.len());
        r.skip(clen).unwrap();
        // block entities juste après le buffer
        assert_eq!(r.vi().unwrap(), 0, "block entities vides hors buffer");
        // skyLightMask
        let n = r.vi().unwrap() as usize;
        assert!(n > 0, "masque sky non vide");
        r.skip(n * 8).unwrap();
        // blockLightMask : vide
        assert_eq!(r.vi().unwrap(), 0);
        // emptySkyLightMask
        let n = r.vi().unwrap() as usize;
        r.skip(n * 8).unwrap();
        // emptyBlockLightMask : vide
        assert_eq!(r.vi().unwrap(), 0);
        // skyLight arrays : chaque tableau a une longueur varint 2048
        let n = r.vi().unwrap() as usize;
        assert!(n > 0);
        for _ in 0..n {
            let l = r.vi().unwrap() as usize;
            assert_eq!(l, 2048);
            r.skip(l).unwrap();
        }
        // blockLight arrays : aucun
        assert_eq!(r.vi().unwrap(), 0);
        assert_eq!(r.p, pkt.len(), "paquet consommé à l'octet près");
    }

    /// login_success : uuid(16) + nom + 0 propriété + strictErrorHandling.
    #[test]
    fn login_success_payload_is_vanilla() {
        let u = crate::mcproto::offline_uuid("Toto");
        let payload = W2::new()
            .uuid(&u)
            .s("Toto")
            .vi(0)
            .b(true)
            .done();
        // 16 uuid + 1 (len nom) + 4 (Toto) + 1 (0 props) + 1 (bool) = 23
        assert_eq!(payload.len(), 23);
        let mut r = R2::new(&payload);
        assert_eq!(r.uuid().unwrap(), u);
        assert_eq!(r.s(16).unwrap(), "Toto");
        assert_eq!(r.vi().unwrap(), 0);
        assert!(r.b().unwrap());
    }

    /// Les registres de la phase configuration doivent être parsables par les
    /// clients vanilla récents (1.20.5 ET >= 1.21.5/26.x via ViaVersion) :
    /// - dimension_type : monster_spawn_light_level = ENTIER SIMPLE ;
    /// - chat_type : décoration EN LIGNE (pas de wrapper {decoration}) ;
    /// - damage_type : registre COMPLET (sinon « Unbound values ... thorns »)
    ///   ET valeurs d'enum STRICTES (effects="burn" au lieu de "burning" a
    ///   déjà fait échouer le gel : « Failed to parse value »).
    #[test]
    fn registry_payloads_are_recent_client_compatible() {
        for pkt in build_codec_packets() {
            // déframe : longueur varint + id + payload
            let mut r = R2::new(&pkt);
            assert_eq!(r.vi().unwrap(), (pkt.len() - r.p) as i32);
            let id = r.vi().unwrap();
            assert_eq!(id, cb::CFG_REGISTRY_DATA);
            let key = r.s(64).unwrap();
            let count = r.vi().unwrap() as usize;
            let mut entries: Vec<(String, Option<crate::nbt::Nbt>)> = Vec::new();
            for _ in 0..count {
                let name = r.s(64).unwrap();
                if r.b().unwrap() {
                    let mut nr = crate::nbt::NbtRdr::new(r.rest());
                    let nbt = nr.read_network().unwrap();
                    r.p += nr.pos();
                    entries.push((name, Some(nbt)));
                } else {
                    entries.push((name, None));
                }
            }
            match key.as_str() {
                "minecraft:dimension_type" => {
                    let el = entries[0].1.as_ref().expect("overworld doit avoir un élément");
                    let light = el.get("monster_spawn_light_level").expect("champ requis");
                    assert!(
                        matches!(light, crate::nbt::Nbt::Int(_)),
                        "monster_spawn_light_level doit être un entier simple, pas {light:?}"
                    );
                }
                "minecraft:chat_type" => {
                    let el = entries[0].1.as_ref().expect("chat doit avoir un élément");
                    let chat = el.get("chat").unwrap();
                    // en ligne : translation_key/parameters directement sous chat
                    assert!(
                        chat.get("translation_key").is_some(),
                        "chat.translation_key requis en ligne (>= 1.21.5)"
                    );
                    assert!(chat.get("parameters").is_some(), "chat.parameters requis");
                    assert!(
                        chat.get("decoration").is_none(),
                        "le wrapper decoration est rejeté par les clients récents"
                    );
                    let narr = el.get("narration").unwrap();
                    assert!(narr.get("translation_key").is_some());
                }
                "minecraft:damage_type" => {
                    // le registre doit contenir TOUS les types vanilla :
                    // les tags par défaut du client les référencent tous.
                    let names: Vec<&str> =
                        entries.iter().map(|(n, _)| n.as_str()).collect();
                    for required in [
                        "minecraft:thorns",
                        "minecraft:fall",
                        "minecraft:in_fire",
                        "minecraft:generic_kill",
                        "minecraft:wind_charge",
                        "minecraft:wither",
                        // résolus DÈS le constructeur DamageSources côté client
                        // (crash « Missing element ... on_fire » si absents) :
                        "minecraft:on_fire",
                        "minecraft:freeze",
                        "minecraft:out_of_world",
                        // autres clés vanilla 1.20.5 qui manquaient :
                        "minecraft:indirect_magic",
                        "minecraft:player_attack",
                        "minecraft:player_explosion",
                        "minecraft:sonic_boom",
                        "minecraft:spit",
                        "minecraft:thrown",
                        "minecraft:wither_skull",
                    ] {
                        assert!(
                            names.contains(&required),
                            "damage_type manquant: {required}"
                        );
                    }
                    assert_eq!(names.len(), vd::DAMAGE_TYPES.len());
                    // champs de base d'une entrée
                    let el = entries[0].1.as_ref().unwrap();
                    assert!(el.get("message_id").is_some());
                    assert!(el.get("scaling").is_some());
                    assert!(el.get("exhaustion").is_some());
                    // enums STRICTES : le codec client rejette toute autre
                    // valeur au gel du registre (« Registry Loading »).
                    const VALID_EFFECTS: &[&str] = &[
                        "hurt",
                        "thorns",
                        "drowning",
                        "burning",
                        "freezing",
                        "poking",
                        "knockback",
                    ];
                    const VALID_DEATH_MSG: &[&str] =
                        &["default", "fall_variants", "intentional_game_design"];
                    const VALID_SCALING: &[&str] = &[
                        "never",
                        "when_caused_by_living_non_player",
                        "always",
                    ];
                    for (name, el) in entries
                        .iter()
                        .filter_map(|(n, e)| e.as_ref().map(|e| (n.as_str(), e)))
                    {
                        let field = |k: &str| el.get(k).and_then(|v| v.as_str());
                        let scaling = field("scaling")
                            .unwrap_or_else(|| panic!("{name}: scaling requis"));
                        assert!(
                            VALID_SCALING.contains(&scaling),
                            "{name}: scaling={scaling:?} invalide"
                        );
                        if let Some(s) = field("effects") {
                            assert!(
                                VALID_EFFECTS.contains(&s),
                                "{name}: effects={s:?} invalide (valides: hurt, thorns, drowning, burning, freezing, knockback)"
                            );
                        }
                        if let Some(s) = field("death_message_type") {
                            assert!(
                                VALID_DEATH_MSG.contains(&s),
                                "{name}: death_message_type={s:?} invalide"
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
