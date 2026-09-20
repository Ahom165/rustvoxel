// Mobs: zombies, siffleurs (exploding green stalker), pigs, sheep, rabbits,
// chickens, cows, foxes, slimes, ombres (night teleporters), bees, parrots,
// turtles, dolphins, goats, frogs and axolotls - the Legacy Update fauna.
// Pure game logic + CPU-side vertex building; rendering goes through renderer.
use crate::math::Vec3;
use crate::mesher::SHADE;
use crate::noise::{hash01, hash2i};
use crate::player::{collide_move, Player};
use crate::world::{
    is_cross_id, is_solid_id, CY, WATER_LEVEL, T_AXOLOTL_FACE, T_AXOLOTL_SKIN, T_BEE_FACE,
    T_BEE_SKIN, T_COW_FACE, T_COW_SKIN, T_CHICKEN_FACE, T_CHICKEN_SKIN, T_DOLPHIN_FACE,
    T_DOLPHIN_SKIN, T_FOX_FACE, T_FOX_SKIN, T_FROG_FACE, T_FROG_SKIN, T_GLOW_SQUID_FACE,
    T_GLOW_SQUID_SKIN, T_GOAT_FACE, T_GOAT_SKIN, T_PLAYER_FACE, T_PLAYER_PANTS,
    T_PLAYER_SHIRT, T_PLAYER_SHIRTS, T_PLAYER_SKIN,
    T_MEAT, T_PARROT_FACE, T_PARROT_SKIN, T_PIG_FACE, T_PIG_SKIN, T_RABBIT_FACE, T_RABBIT_SKIN,
    T_SHADOW_FACE, T_SHADOW_SKIN, T_SHEEP_FACE, T_SHEEP_WOOL, T_SIFFLEUR_FACE, T_SIFFLEUR_SKIN,
    T_SLIME, T_TURTLE_FACE, T_TURTLE_SKIN, T_ZOMBIE_BODY, T_ZOMBIE_FACE, T_ZOMBIE_LIMB,
    T_ZOMBIE_SKIN, B_BADLANDS, B_CHERRY, B_DAPPLED, B_FOREST, B_JUNGLE, B_PEAKS, B_PLAINS,
    B_SAVANNA, B_SNOW, B_SWAMP, World,
};

const GRAVITY: f32 = 32.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MobKind {
    Zombie,
    Siffleur,
    Pig,
    Sheep,
    Rabbit,
    Chicken,
    Cow,
    Fox,
    Slime,
    Ombre,
    Bee,
    Parrot,
    Turtle,
    Dolphin,
    Goat,
    Frog,
    Axolotl,
    /// 1.18: glowing squid drifting in dark and underground water.
    GlowSquid,
    /// v0.6 (multiplayer): remote players rendered with the same rig.
    Player,
}

impl MobKind {
    /// Wire id used by the entity snapshot packet.
    pub fn id(self) -> u8 {
        match self {
            MobKind::Player => 0,
            MobKind::Zombie => 1,
            MobKind::Siffleur => 2,
            MobKind::Pig => 3,
            MobKind::Sheep => 4,
            MobKind::Rabbit => 5,
            MobKind::Chicken => 6,
            MobKind::Cow => 7,
            MobKind::Fox => 8,
            MobKind::Slime => 9,
            MobKind::Ombre => 10,
            MobKind::Bee => 11,
            MobKind::Parrot => 12,
            MobKind::Turtle => 13,
            MobKind::Dolphin => 14,
            MobKind::Goat => 15,
            MobKind::Frog => 16,
            MobKind::Axolotl => 17,
            MobKind::GlowSquid => 18,
        }
    }
    pub fn from_id(v: u8) -> Option<MobKind> {
        Some(match v {
            0 => MobKind::Player,
            1 => MobKind::Zombie,
            2 => MobKind::Siffleur,
            3 => MobKind::Pig,
            4 => MobKind::Sheep,
            5 => MobKind::Rabbit,
            6 => MobKind::Chicken,
            7 => MobKind::Cow,
            8 => MobKind::Fox,
            9 => MobKind::Slime,
            10 => MobKind::Ombre,
            11 => MobKind::Bee,
            12 => MobKind::Parrot,
            13 => MobKind::Turtle,
            14 => MobKind::Dolphin,
            15 => MobKind::Goat,
            16 => MobKind::Frog,
            17 => MobKind::Axolotl,
            18 => MobKind::GlowSquid,
            _ => return None,
        })
    }
}

impl MobKind {
    pub fn half(self) -> f32 {
        match self {
            MobKind::Zombie => 0.3,
            MobKind::Siffleur => 0.28,
            MobKind::Pig => 0.35,
            MobKind::Sheep => 0.38,
            MobKind::Rabbit => 0.22,
            MobKind::Chicken => 0.18,
            MobKind::Cow => 0.45,
            MobKind::Fox => 0.26,
            MobKind::Slime => 0.42,
            MobKind::Ombre => 0.35,
            MobKind::Bee => 0.18,
            MobKind::Parrot => 0.16,
            MobKind::Turtle => 0.32,
            MobKind::Dolphin => 0.35,
            MobKind::Goat => 0.34,
            MobKind::Frog => 0.18,
            MobKind::Axolotl => 0.22,
            MobKind::GlowSquid => 0.32,
            MobKind::Player => 0.3,
        }
    }
    pub fn height(self) -> f32 {
        match self {
            MobKind::Zombie => 1.9,
            MobKind::Siffleur => 1.65,
            MobKind::Pig => 0.9,
            MobKind::Sheep => 1.15,
            MobKind::Rabbit => 0.5,
            MobKind::Chicken => 0.65,
            MobKind::Cow => 1.35,
            MobKind::Fox => 0.75,
            MobKind::Slime => 0.85,
            MobKind::Ombre => 2.8,
            MobKind::Bee => 0.35,
            MobKind::Parrot => 0.6,
            MobKind::Turtle => 0.35,
            MobKind::Dolphin => 0.5,
            MobKind::Goat => 1.2,
            MobKind::Frog => 0.3,
            MobKind::Axolotl => 0.35,
            MobKind::GlowSquid => 0.8,
            MobKind::Player => 1.8,
        }
    }
    pub fn speed(self) -> f32 {
        match self {
            MobKind::Zombie => 2.2,
            MobKind::Siffleur => 2.7,
            MobKind::Pig => 1.3,
            MobKind::Sheep => 1.15,
            MobKind::Rabbit => 1.8,
            MobKind::Chicken => 1.1,
            MobKind::Cow => 1.0,
            MobKind::Fox => 2.4,
            MobKind::Slime => 1.9,
            MobKind::Ombre => 2.9,
            MobKind::Bee => 1.7,
            MobKind::Parrot => 1.9,
            MobKind::Turtle => 0.5,
            MobKind::Dolphin => 2.4,
            MobKind::Goat => 1.3,
            MobKind::Frog => 1.5,
            MobKind::Axolotl => 1.5,
            MobKind::GlowSquid => 1.1,
            MobKind::Player => 0.0,
        }
    }
    pub fn hp_max(self) -> i32 {
        match self {
            MobKind::Zombie | MobKind::Siffleur => 20,
            MobKind::Ombre => 26,
            MobKind::Pig | MobKind::Cow => 10,
            MobKind::Sheep | MobKind::Fox => 8,
            MobKind::Slime => 8,
            MobKind::Goat | MobKind::Turtle | MobKind::Dolphin => 12,
            MobKind::Frog | MobKind::Axolotl => 8,
            MobKind::GlowSquid => 10,
            MobKind::Chicken | MobKind::Bee | MobKind::Parrot => 4,
            MobKind::Rabbit => 3,
            MobKind::Player => 20,
        }
    }
    pub fn hostile(self) -> bool {
        matches!(self, MobKind::Zombie | MobKind::Siffleur | MobKind::Slime | MobKind::Ombre)
    }
    /// Skin/shirt index for the player rig (0..8).
    pub fn shirt_tile(skin: u8) -> u32 {
        T_PLAYER_SHIRT + (skin % T_PLAYER_SHIRTS as u8) as u32
    }
    /// Flyers hover above the ground instead of walking.
    pub fn flies(self) -> bool {
        matches!(self, MobKind::Bee | MobKind::Parrot)
    }
    /// Aquatic mobs swim; on land they just flop around.
    pub fn aquatic(self) -> bool {
        matches!(self, MobKind::Dolphin | MobKind::Axolotl | MobKind::GlowSquid)
    }
    pub fn eye_offset(self) -> f32 {
        self.height() * 0.85
    }
}

#[derive(Clone)]
pub struct Mob {
    pub kind: MobKind,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub hp: i32,
    pub on_ground: bool,
    pub in_water: bool,
    pub anim: f32,
    pub wander_t: f32,
    pub moving: bool,
    pub attack_cd: f32,
    pub fuse: f32,
    pub hurt_t: f32,
    pub burn_t: f32,
    pub flee_t: f32,
    pub jump_cd: f32,
    pub tp_cd: f32,
    pub dead_flag: bool,
    /// Player rig only: shirt color index (0..8).
    pub skin: u8,
    /// Server-assigned stable entity id (0 until first server tick).
    pub uid: u32,
}

impl Mob {
    pub fn new(kind: MobKind, pos: Vec3) -> Mob {
        Mob {
            kind,
            pos,
            vel: Vec3::new(0.0, 0.0, 0.0),
            yaw: 0.0,
            hp: kind.hp_max(),
            on_ground: false,
            in_water: false,
            anim: 0.0,
            wander_t: 0.0,
            moving: false,
            attack_cd: 0.0,
            fuse: 0.0,
            hurt_t: 0.0,
            burn_t: 0.0,
            flee_t: 0.0,
            jump_cd: 0.0,
            tp_cd: 3.0,
            dead_flag: false,
            skin: 0,
            uid: 0,
        }
    }

    pub fn center(&self) -> Vec3 {
        self.pos + Vec3::new(0.0, self.kind.height() * 0.5, 0.0)
    }

    pub fn hurt(&mut self, dmg: i32, knock: Vec3) {
        self.hp -= dmg;
        self.hurt_t = 0.4;
        self.vel += knock;
        if !self.kind.hostile() {
            self.flee_t = 3.5;
        }
        if self.hp <= 0 {
            self.hp = 0;
            self.dead_flag = true;
        }
    }
}

// ---------------------------------------------------------------- targets
/// Who the mobs can see and hurt. Solo/tests use the local `Player`;
/// the multiplayer server implements it over its table of connected players.
pub trait MobTargets {
    /// Feet position of the nearest LIVING target within `max` (None: all
    /// dead or too far). Mobs wander when there is no target.
    fn nearest_pos(&self, pos: Vec3, max: f32) -> Option<Vec3>;
    /// Damage the nearest living target within `max` of `pos`.
    fn hurt_nearest(&mut self, pos: Vec3, max: f32, dmg: i32, knock: Vec3);
    /// Explosive blast centered at `pos`: damage falls off with distance.
    fn hurt_radius(&mut self, pos: Vec3, radius: f32, base: i32, knock: f32);
}

impl MobTargets for Player {
    fn nearest_pos(&self, pos: Vec3, max: f32) -> Option<Vec3> {
        if self.dead {
            return None;
        }
        let d = (self.pos - pos).length();
        if d <= max {
            Some(self.pos)
        } else {
            None
        }
    }
    fn hurt_nearest(&mut self, pos: Vec3, max: f32, dmg: i32, knock: Vec3) {
        if !self.dead && (self.pos - pos).length() <= max {
            self.hurt(dmg, knock);
        }
    }
    fn hurt_radius(&mut self, pos: Vec3, radius: f32, base: i32, knock: f32) {
        let pc = self.pos + Vec3::new(0.0, 0.9, 0.0);
        let d = (pc - pos).length();
        if d < radius {
            let f = 1.0 - d / radius;
            let dir = (pc - pos).normalize();
            self.invuln = 0.0;
            self.hurt(
                (f * base as f32).round() as i32,
                dir * (f * knock) + Vec3::new(0.0, f * knock * 0.42, 0.0),
            );
        }
    }
}

// ---------------------------------------------------------------- particles
pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub col: [f32; 4],
    pub gravity: bool,
    pub phase: f32,
    /// Constant vertical drift for drifting particles (falling leaves).
    pub sink: f32,
}

impl Particle {
    pub fn new(pos: Vec3, vel: Vec3, life: f32, col: [f32; 4], gravity: bool) -> Particle {
        Particle {
            pos,
            vel,
            life,
            max_life: life,
            col,
            gravity,
            phase: hash01(hash2i(
                (pos.x * 31.0) as i64,
                (pos.z * 17.0) as i64,
                0xF1A9,
            )) * 6.28,
            sink: 0.0,
        }
    }

    pub fn pack(&self, out: &mut Vec<f32>) {
        let f = (self.life / self.max_life).clamp(0.0, 1.0);
        let a = if f > 0.7 {
            self.col[3]
        } else {
            self.col[3] * (f / 0.7)
        };
        out.extend_from_slice(&[
            self.pos.x, self.pos.y, self.pos.z, self.col[0], self.col[1], self.col[2], a,
        ]);
    }
}

pub fn update_particles(ps: &mut Vec<Particle>, world: &World, time: f32, dt: f32) {
    ps.retain(|p| p.life > 0.0);
    for p in ps.iter_mut() {
        p.life -= dt;
        if p.gravity {
            p.vel.y -= 20.0 * dt;
            p.pos += p.vel * dt;
            // crude floor collision
            let b = world.get_block(
                p.pos.x.floor() as i32,
                (p.pos.y - 0.06).floor() as i32,
                p.pos.z.floor() as i32,
            );
            if is_solid_id(b) && p.vel.y < 0.0 {
                p.pos.y = p.pos.y.floor() + 1.06;
                p.vel.y = -p.vel.y * 0.25;
                p.vel.x *= 0.6;
                p.vel.z *= 0.6;
            }
        } else {
            // fireflies + falling leaves drift on lissajous paths
            let a = p.phase;
            p.vel = Vec3::new(
                0.4 * (time * 0.9 + a).sin(),
                0.25 * (time * 1.4 + a * 2.0).sin() + p.sink,
                0.4 * (time * 0.8 + a * 3.0).cos(),
            );
            p.pos += p.vel * dt;
        }
    }
}

// ---------------------------------------------------------------- explosions
pub fn explode(
    world: &mut World,
    pos: Vec3,
    radius: f32,
    targets: &mut dyn MobTargets,
    mobs: &mut [Mob],
    particles: &mut Vec<Particle>,
    on_block: &mut dyn FnMut(i32, i32, i32),
) {
    let r = radius.ceil() as i32;
    let ir = r as f32;
    for dy in -r..=r {
        for dz in -r..=r {
            for dx in -r..=r {
                let d = ((dx * dx + dy * dy + dz * dz) as f32).sqrt();
                if d > ir {
                    continue;
                }
                let bx = pos.x as i32 + dx;
                let by = pos.y as i32 + dy;
                let bz = pos.z as i32 + dz;
                let b = world.get_block(bx, by, bz);
                if b == crate::world::AIR
                    || b == crate::world::WATER
                    || b == crate::world::BEDROCK
                {
                    continue;
                }
                // keep outer shell ragged
                if d > ir - 0.8
                    && hash01(hash2i(bx as i64, (by * 7 + bz) as i64, 0xE71D1)) < 0.4
                {
                    continue;
                }
                world.set_block(bx, by, bz, crate::world::AIR);
                on_block(bx, by, bz);
                if hash01(hash2i(bx as i64, by as i64, 0xB10B)) < 0.35 {
                    particles.push(Particle::new(
                        Vec3::new(bx as f32 + 0.5, by as f32 + 0.5, bz as f32 + 0.5),
                        Vec3::new(
                            (hash01(hash2i(bx as i64, by as i64, 1)) - 0.5) * 8.0,
                            3.0 + hash01(hash2i(bx as i64, by as i64, 2)) * 6.0,
                            (hash01(hash2i(bx as i64, bz as i64, 3)) - 0.5) * 8.0,
                        ),
                        0.9 + hash01(hash2i(by as i64, bz as i64, 4)) * 0.5,
                        [0.55, 0.5, 0.45, 1.0],
                        true,
                    ));
                }
            }
        }
    }
    // burst cloud
    for i in 0..36 {
        let a = hash01(hash2i(i, 11, 0x9E)) * 6.283;
        let b = hash01(hash2i(i, 12, 0x9F)) * 3.14;
        let sp = 4.0 + hash01(hash2i(i, 13, 0xA0)) * 7.0;
        let dir = Vec3::new(a.cos() * b.sin(), b.cos(), a.sin() * b.sin());
        let fire = i % 3 == 0;
        particles.push(Particle::new(
            pos,
            dir * sp,
            0.6 + hash01(hash2i(i, 14, 0xA1)) * 0.6,
            if fire {
                [1.0, 0.6, 0.2, 0.95]
            } else {
                [0.42, 0.40, 0.38, 0.9]
            },
            fire,
        ));
    }
    // player damage + knockback (nearest players inside the blast radius)
    targets.hurt_radius(pos, radius * 2.0, 22, 17.0);
    // mob damage (chain: dead siffleurs will pop next tick)
    let rr = radius * 2.0;
    for m in mobs.iter_mut() {
        let c = m.center();
        let d = (c - pos).length();
        if d < rr {
            let f = 1.0 - d / rr;
            let dir = (c - pos).normalize();
            m.hurt(
                (f * 24.0).round() as i32,
                dir * (f * 12.0) + Vec3::new(0.0, f * 5.0, 0.0),
            );
        }
    }
}

// ---------------------------------------------------------------- picking
/// Ray vs AABB slab test; returns entry distance if hit.
pub fn ray_aabb(o: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut tmin = 0.0f32;
    let mut tmax = f32::INFINITY;
    for ax in 0..3 {
        let (o, d, mn, mx) = match ax {
            0 => (o.x, dir.x, min.x, max.x),
            1 => (o.y, dir.y, min.y, max.y),
            _ => (o.z, dir.z, min.z, max.z),
        };
        let d = if d.abs() < 1e-7 { 1e-7 } else { d };
        let inv = 1.0 / d;
        let mut t1 = (mn - o) * inv;
        let mut t2 = (mx - o) * inv;
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
        }
        tmin = tmin.max(t1);
        tmax = tmax.min(t2);
        if tmin > tmax {
            return None;
        }
    }
    Some(tmin)
}

/// Nearest mob under the crosshair within max_t.
pub fn pick_mob(mobs: &[Mob], origin: Vec3, dir: Vec3, max_t: f32) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;
    for (i, m) in mobs.iter().enumerate() {
        let h = m.kind.half();
        let min = Vec3::new(m.pos.x - h, m.pos.y, m.pos.z - h);
        let max = Vec3::new(m.pos.x + h, m.pos.y + m.kind.height(), m.pos.z + h);
        if let Some(t) = ray_aabb(origin, dir, min, max) {
            if t <= max_t && best.map(|(_, bt)| t < bt).unwrap_or(true) {
                best = Some((i, t));
            }
        }
    }
    best
}

// ---------------------------------------------------------------- spawning
pub struct Spawner {
    pub t: f32,
    pub rng: u64,
}

impl Spawner {
    pub fn new() -> Spawner {
        Spawner {
            t: 0.0,
            rng: 0x1234_5678_9ABC_DEF0,
        }
    }
    fn next01(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0xD1B5_4A32_D192_ED03);
        ((self.rng >> 33) as f32) / 8589934592.0
    }
}

fn ground_ok(world: &World, x: i32, y: i32, z: i32) -> bool {
    if y < 1 || y + 2 >= CY as i32 {
        return false;
    }
    let below = world.get_block(x, y - 1, z);
    is_solid_id(below)
        && !is_cross_id(below)
        && world.get_block(x, y, z) == crate::world::AIR
        && world.get_block(x, y + 1, z) == crate::world::AIR
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_tick(
    mobs: &mut Vec<Mob>,
    world: &World,
    centers: &[Vec3],
    light: f32,
    dt: f32,
    sp: &mut Spawner,
) {
    if centers.is_empty() {
        return;
    }
    sp.t -= dt;
    if sp.t > 0.0 {
        return;
    }
    sp.t = 1.7;
    let mut hostiles = 0;
    let mut passives = 0;
    for m in mobs.iter() {
        if m.kind.hostile() {
            hostiles += 1;
        } else {
            passives += 1;
        }
    }
    let want_hostile = light < 0.35 && hostiles < 8;
    let want_passive = light > 0.6 && passives < 10;
    if !want_hostile && !want_passive {
        return;
    }

    // v0.5 (1.18): glow squids drift in dark water - flooded caves below
    // y=24 and the deep ocean at night.
    if want_passive && sp.next01() < 0.22 {
        for _ in 0..8 {
            let c = centers[(sp.next01() * centers.len() as f32) as usize % centers.len()];
            let a = sp.next01() * 6.283;
            let d = 12.0 + sp.next01() * 20.0;
            let x = (c.x + a.cos() * d).floor() as i32;
            let z = (c.z + a.sin() * d).floor() as i32;
            if !world.has_chunk((x.div_euclid(16), z.div_euclid(16))) {
                continue;
            }
            let y = 8 + (sp.next01() * 16.0) as i32;
            if world.get_block(x, y, z) == crate::world::WATER
                && world.get_block(x, y + 1, z) == crate::world::WATER
                && world.get_block(x, y - 1, z) != crate::world::WATER
                && world.get_block(x, y - 1, z) != crate::world::AIR
            {
                mobs.push(Mob::new(
                    MobKind::GlowSquid,
                    Vec3::new(x as f32 + 0.5, y as f32 + 0.3, z as f32 + 0.5),
                ));
                return;
            }
        }
    }
    // aquatic passives: dolphins in open water, axolotls in swamp pools
    if want_passive && sp.next01() < 0.35 {
        for _ in 0..6 {
            let c = centers[(sp.next01() * centers.len() as f32) as usize % centers.len()];
            let a = sp.next01() * 6.283;
            let d = 14.0 + sp.next01() * 22.0;
            let x = (c.x + a.cos() * d).floor() as i32;
            let z = (c.z + a.sin() * d).floor() as i32;
            if !world.has_chunk((x.div_euclid(16), z.div_euclid(16))) {
                continue;
            }
            let y = WATER_LEVEL;
            if world.get_block(x, y, z) == crate::world::WATER
                && world.get_block(x, y + 1, z) == crate::world::WATER
                && world.get_block(x, y - 1, z) != crate::world::WATER
            {
                let kind = if world.biome_at(x, z) == B_SWAMP && sp.next01() < 0.45 {
                    MobKind::Axolotl
                } else {
                    MobKind::Dolphin
                };
                mobs.push(Mob::new(
                    kind,
                    Vec3::new(x as f32 + 0.5, y as f32 + 0.4, z as f32 + 0.5),
                ));
                return;
            }
        }
    }
    for _ in 0..6 {
        let c = centers[(sp.next01() * centers.len() as f32) as usize % centers.len()];
        let a = sp.next01() * 6.283;
        let d = 13.0 + sp.next01() * 26.0;
        let x = (c.x + a.cos() * d).floor() as i32;
        let z = (c.z + a.sin() * d).floor() as i32;
        // chunk must be loaded
        if !world.has_chunk((x.div_euclid(16), z.div_euclid(16))) {
            continue;
        }
        if let Some(y) = world.surface_y(x, z) {
            let yy = y + 1;
            if !ground_ok(world, x, yy, z) {
                continue;
            }
            let pos = Vec3::new(x as f32 + 0.5, yy as f32, z as f32 + 0.5);
            if want_hostile {
                let r = sp.next01();
                let kind = if r < 0.58 {
                    MobKind::Zombie
                } else if r < 0.78 {
                    MobKind::Siffleur
                } else if r < 0.93 {
                    MobKind::Slime
                } else {
                    MobKind::Ombre
                };
                mobs.push(Mob::new(kind, pos));
                return;
            }
            // passives only on grassy / pale / snowy ground
            let below = world.get_block(x, y, z);
            if below != crate::world::GRASS
                && below != crate::world::GRASS_PALE
                && below != crate::world::SNOW
            {
                continue;
            }
            // Wilderness Bound fauna: the dappled forest hosts foxes, rabbits
            // and (cold) sheep; other biomes get the classic barnyard.
            let biome = world.biome_at(x, z);
            let r = sp.next01();
            let kind = match biome {
                B_DAPPLED => {
                    if r < 0.4 {
                        MobKind::Fox
                    } else if r < 0.75 {
                        MobKind::Rabbit
                    } else {
                        MobKind::Sheep
                    }
                }
                B_SNOW => {
                    if r < 0.5 {
                        MobKind::Rabbit
                    } else {
                        MobKind::Fox
                    }
                }
                B_JUNGLE => {
                    if r < 0.45 {
                        MobKind::Parrot
                    } else if r < 0.75 {
                        MobKind::Bee
                    } else {
                        MobKind::Chicken
                    }
                }
                B_CHERRY => {
                    if r < 0.4 {
                        MobKind::Bee
                    } else if r < 0.75 {
                        MobKind::Sheep
                    } else {
                        MobKind::Rabbit
                    }
                }
                B_SAVANNA => {
                    if r < 0.4 {
                        MobKind::Cow
                    } else if r < 0.8 {
                        MobKind::Sheep
                    } else {
                        MobKind::Pig
                    }
                }
                B_PEAKS => MobKind::Goat,
                B_SWAMP => {
                    if r < 0.55 {
                        MobKind::Frog
                    } else if r < 0.8 {
                        MobKind::Rabbit
                    } else {
                        MobKind::Chicken
                    }
                }
                B_BADLANDS => {
                    if r < 0.7 {
                        MobKind::Rabbit
                    } else {
                        MobKind::Chicken
                    }
                }
                B_FOREST | B_PLAINS => {
                    if r < 0.25 {
                        MobKind::Pig
                    } else if r < 0.5 {
                        MobKind::Sheep
                    } else if r < 0.7 {
                        MobKind::Chicken
                    } else if r < 0.88 {
                        MobKind::Cow
                    } else {
                        MobKind::Rabbit
                    }
                }
                _ => {
                    if r < 0.5 {
                        MobKind::Sheep
                    } else {
                        MobKind::Chicken
                    }
                }
            };
            let group = 1 + (sp.next01() * 2.9) as i32;
            for g in 0..group {
                let gx = x + (g % 2) as i32;
                let gz = z + (g / 2) as i32;
                if let Some(gy) = world.surface_y(gx, gz) {
                    if ground_ok(world, gx, gy + 1, gz) {
                        mobs.push(Mob::new(
                            kind,
                            Vec3::new(gx as f32 + 0.5, (gy + 1) as f32, gz as f32 + 0.5),
                        ));
                    }
                }
            }
            return;
        }
    }
}

// ---------------------------------------------------------------- AI update
pub fn update_mobs(
    mobs: &mut Vec<Mob>,
    world: &mut World,
    targets: &mut dyn MobTargets,
    particles: &mut Vec<Particle>,
    light: f32,
    _time: f32,
    dt: f32,
    on_block: &mut dyn FnMut(i32, i32, i32),
) {
    let mut explode_at: Vec<Vec3> = Vec::new();
    for i in 0..mobs.len() {
        let m = &mut mobs[i];
        m.hurt_t = (m.hurt_t - dt).max(0.0);
        m.attack_cd = (m.attack_cd - dt).max(0.0);
        m.jump_cd = (m.jump_cd - dt).max(0.0);
        m.flee_t = (m.flee_t - dt).max(0.0);
        m.anim += dt;

        // nearest living target; no target (all dead / offline) -> despawn
        let tpos = targets.nearest_pos(m.pos, 64.0);
        let to_p = match tpos {
            Some(t) => t - m.pos,
            None => {
                m.dead_flag = true;
                continue;
            }
        };
        let dist_p = to_p.length();
        if dist_p > 56.0 {
            m.dead_flag = true;
            continue;
        }

        let mut speed = 0.0f32;
        match m.kind {
            MobKind::Zombie => {
                if dist_p < 20.0 {
                    m.yaw = (to_p.x).atan2(-to_p.z); // matches fwd=(sin,-cos)
                    m.moving = dist_p > 1.15;
                    speed = m.kind.speed();
                    if dist_p < 1.6 && m.attack_cd <= 0.0 {
                        let dir = Vec3::new(to_p.x, 0.0, to_p.z).normalize();
                        targets.hurt_nearest(m.pos, 2.2, 3, dir * 7.0 + Vec3::new(0.0, 3.0, 0.0));
                        m.attack_cd = 1.1;
                    }
                } else {
                    wander(m, dt, 0.5);
                    speed = if m.moving { m.kind.speed() * 0.55 } else { 0.0 };
                }
                // burn in daylight
                if light > 0.72 && sky_visible(world, m) {
                    m.burn_t += dt;
                    if m.burn_t > 1.0 {
                        m.burn_t = 0.0;
                        m.hurt(2, Vec3::new(0.0, 0.0, 0.0));
                        particles.push(Particle::new(
                            m.center(),
                            Vec3::new(0.0, 2.0, 0.0),
                            0.6,
                            [0.95, 0.55, 0.15, 0.9],
                            false,
                        ));
                    }
                } else {
                    m.burn_t = 0.0;
                }
            }
            MobKind::Siffleur => {
                if dist_p < 18.0 {
                    m.yaw = (to_p.x).atan2(-to_p.z);
                    if dist_p < 2.6 {
                        m.moving = false;
                        m.fuse += dt;
                    } else {
                        m.moving = true;
                        speed = m.kind.speed();
                        m.fuse = (m.fuse - dt * 0.9).max(0.0);
                    }
                } else {
                    m.fuse = (m.fuse - dt * 0.9).max(0.0);
                    wander(m, dt, 0.4);
                    speed = if m.moving { m.kind.speed() * 0.5 } else { 0.0 };
                }
                if m.fuse >= 1.5 {
                    explode_at.push(m.center());
                    m.dead_flag = true;
                }
            }
            MobKind::Pig | MobKind::Sheep | MobKind::Rabbit | MobKind::Chicken | MobKind::Cow
            | MobKind::Fox | MobKind::Turtle | MobKind::Goat | MobKind::Frog => {
                if m.flee_t > 0.0 && dist_p < 12.0 {
                    m.yaw = (-to_p.x).atan2(to_p.z); // run away
                    m.moving = true;
                    speed = m.kind.speed() * 1.8;
                } else {
                    wander(m, dt, 0.8);
                    speed = if m.moving { m.kind.speed() } else { 0.0 };
                }
                if (m.kind == MobKind::Rabbit || m.kind == MobKind::Frog)
                    && m.moving
                    && m.on_ground
                    && m.jump_cd <= 0.0
                {
                    m.vel.y = 6.6;
                    m.jump_cd = 0.7;
                }
                if m.kind == MobKind::Goat && m.moving && m.on_ground && m.jump_cd <= 0.0 {
                    // goats hop around their mountains
                    let r = hash01(hash2i((m.pos.x * 3.0) as i64, (m.anim * 9.0) as i64, 0x60A7));
                    if r < 0.06 {
                        m.vel.y = 8.8;
                        m.jump_cd = 1.4;
                    }
                }
            }
            MobKind::Slime => {
                if dist_p < 14.0 {
                    m.yaw = (to_p.x).atan2(-to_p.z);
                    m.moving = dist_p > 1.0;
                    speed = m.kind.speed();
                    if dist_p < 1.5 && m.attack_cd <= 0.0 {
                        let dir = Vec3::new(to_p.x, 0.0, to_p.z).normalize();
                        targets.hurt_nearest(m.pos, 2.0, 2, dir * 6.0 + Vec3::new(0.0, 3.0, 0.0));
                        m.attack_cd = 1.2;
                    }
                } else {
                    wander(m, dt, 0.4);
                    speed = if m.moving { m.kind.speed() } else { 0.0 };
                }
                // slimes bounce in arcs toward their target
                if m.on_ground && m.moving && m.jump_cd <= 0.0 {
                    m.vel.y = 6.8;
                    m.jump_cd = 0.9;
                }
            }
            MobKind::Ombre => {
                m.tp_cd -= dt;
                if light < 0.5 && dist_p < 24.0 {
                    m.yaw = (to_p.x).atan2(-to_p.z);
                    m.moving = dist_p > 1.4;
                    speed = m.kind.speed();
                    if dist_p < 1.8 && m.attack_cd <= 0.0 {
                        let dir = Vec3::new(to_p.x, 0.0, to_p.z).normalize();
                        targets.hurt_nearest(m.pos, 2.4, 4, dir * 7.0 + Vec3::new(0.0, 3.0, 0.0));
                        m.attack_cd = 1.4;
                    }
                    // blink around their prey
                    if m.tp_cd <= 0.0 && dist_p < 12.0 && hash01(hash2i((m.anim * 7.0) as i64, 3, 0x0BB0)) < 0.4 {
                        teleport(m, world, particles);
                        m.tp_cd = 5.0 + hash01(hash2i((m.anim * 13.0) as i64, 5, 0x0BB1)) * 4.0;
                    }
                } else {
                    // shy in daylight: vanish when approached
                    if light > 0.7 && dist_p < 7.0 && m.tp_cd <= 0.0 {
                        teleport(m, world, particles);
                        m.tp_cd = 4.0;
                    }
                    wander(m, dt, 0.3);
                    speed = if m.moving { m.kind.speed() * 0.5 } else { 0.0 };
                }
            }
            MobKind::Bee | MobKind::Parrot => {
                if m.flee_t > 0.0 && dist_p < 10.0 {
                    m.yaw = (-to_p.x).atan2(to_p.z);
                    m.moving = true;
                    speed = m.kind.speed() * 1.7;
                } else {
                    wander(m, dt, 0.9);
                    speed = if m.moving { m.kind.speed() } else { 0.0 };
                }
            }
            MobKind::Dolphin | MobKind::Axolotl | MobKind::GlowSquid => {
                if m.flee_t > 0.0 && dist_p < 10.0 {
                    m.yaw = (-to_p.x).atan2(to_p.z);
                    m.moving = true;
                    speed = m.kind.speed() * 1.6;
                } else {
                    wander(m, dt, 0.55);
                    speed = if m.moving { m.kind.speed() } else { 0.0 };
                }
            }
            MobKind::Player => {
                // players are not AI-driven: the server streams their positions
            }
        }

        // physics
        let feet_b = world.get_block(
            m.pos.x.floor() as i32,
            (m.pos.y + 0.3).floor() as i32,
            m.pos.z.floor() as i32,
        );
        m.in_water = feet_b == crate::world::WATER;

        let fwd = Vec3::new(m.yaw.sin(), 0.0, -m.yaw.cos());
        let accel = if m.on_ground { 10.0 } else { 3.0 };
        let k = 1.0 - (-accel * dt).exp();
        m.vel.x += (fwd.x * speed - m.vel.x) * k;
        m.vel.z += (fwd.z * speed - m.vel.z) * k;

        if m.kind.flies() {
            // hover a bit above the surface with a gentle bob
            let bx = m.pos.x.floor() as i32;
            let bz = m.pos.z.floor() as i32;
            let want = if let Some(sy) = world.surface_y(bx, bz) {
                sy as f32 + 2.1 + (m.anim * 1.7).sin() * 0.4
            } else {
                m.pos.y + (m.anim * 1.7).sin() * 0.4
            };
            // do not fight ceilings
            let above = world.get_block(bx, (m.pos.y + 1.2).floor() as i32, bz);
            let want = if is_solid_id(above) { m.pos.y - 0.4 } else { want };
            m.vel.y += ((want - m.pos.y) * 2.2 - m.vel.y) * (1.0 - (-6.0 * dt).exp());
        } else if m.kind.aquatic() {
            if m.in_water {
                let head_b = world.get_block(
                    m.pos.x.floor() as i32,
                    (m.pos.y + m.kind.height() + 0.2).floor() as i32,
                    m.pos.z.floor() as i32,
                );
                // stay submerged: bob, but sink if about to breach
                m.vel.y = if head_b == crate::world::WATER {
                    (m.anim * 1.4).sin() * 0.7
                } else {
                    -1.8
                };
            } else {
                m.vel.y -= GRAVITY * dt;
                // beached: desperate flops
                if m.on_ground && m.moving && m.jump_cd <= 0.0 {
                    m.vel.y = 5.0;
                    m.jump_cd = 0.9;
                }
            }
        } else if m.in_water {
            m.vel.y += (if m.moving { 2.4 } else { -1.0 } - m.vel.y)
                * (1.0 - (-4.0 * dt).exp());
        } else {
            m.vel.y -= GRAVITY * dt;
            if m.vel.y < -60.0 {
                m.vel.y = -60.0;
            }
        }

        let (og, hw) = collide_move(world, &mut m.pos, &mut m.vel, m.kind.half(), m.kind.height(), dt);
        m.on_ground = og;
        if hw && og && m.moving && m.jump_cd <= 0.0 && !m.in_water {
            m.vel.y = 8.4; // hop over 1-block obstacles
            m.jump_cd = 0.7;
        }

        // cactus hurts mobs too
        let h = m.kind.half();
        let bb = crate::player::aabb_of(&m.pos, h, m.kind.height());
        'outer: for by in (bb.2 + 0.1).floor() as i32..=(bb.3 - 0.1).floor() as i32 {
            for bz in (bb.4 - 0.1).floor() as i32..=(bb.5 + 0.1).floor() as i32 {
                for bx in (bb.0 - 0.1).floor() as i32..=(bb.1 + 0.1).floor() as i32 {
                    if world.get_block(bx, by, bz) == crate::world::CACTUS {
                        m.hurt(1, Vec3::new(0.0, 1.0, 0.0));
                        break 'outer;
                    }
                }
            }
        }
    }

    // chain explosions (dead siffleurs pushed by the first blast)
    for pos in explode_at {
        explode(world, pos, 3.2, targets, mobs, particles, on_block);
    }

    mobs.retain(|m| !m.dead_flag);
}

fn wander(m: &mut Mob, dt: f32, chance: f32) {
    m.wander_t -= dt;
    if m.wander_t <= 0.0 {
        let r = hash01(hash2i(
            (m.pos.x * 13.0) as i64 + (m.anim * 10.0) as i64,
            (m.pos.z * 7.0) as i64,
            0x3EBD,
        ));
        if r < chance * 0.55 {
            m.moving = true;
            m.yaw = r * 47.0;
        } else {
            m.moving = false;
        }
        m.wander_t = 1.0 + r * 3.0;
    }
}

/// Ombre teleport: purple poof, random solid landing spot 5-12 blocks away.
fn teleport(m: &mut Mob, world: &World, particles: &mut Vec<Particle>) {
    particles.push(Particle::new(
        m.center(),
        Vec3::new(0.0, 0.5, 0.0),
        0.5,
        [0.78, 0.36, 0.92, 0.9],
        false,
    ));
    for i in 0..12 {
        let a = hash01(hash2i(i, (m.anim * 31.0) as i64, 0x7E1E)) * 6.283;
        let d = 5.0 + hash01(hash2i(i, (m.anim * 17.0) as i64 + 1, 0x7E1F)) * 7.0;
        let x = (m.pos.x + a.cos() * d).floor() as i32;
        let z = (m.pos.z + a.sin() * d).floor() as i32;
        if let Some(y) = world.surface_y(x, z) {
            if ground_ok(world, x, y + 1, z) {
                m.pos = Vec3::new(x as f32 + 0.5, (y + 1) as f32, z as f32 + 0.5);
                m.vel = Vec3::new(0.0, 0.0, 0.0);
                break;
            }
        }
    }
    particles.push(Particle::new(
        m.center(),
        Vec3::new(0.0, 0.5, 0.0),
        0.5,
        [0.78, 0.36, 0.92, 0.9],
        false,
    ));
}

fn sky_visible(world: &World, m: &Mob) -> bool {
    let x = m.pos.x.floor() as i32;
    let z = m.pos.z.floor() as i32;
    let mut y = (m.pos.y + m.kind.height()) as i32;
    while y < CY as i32 {
        if crate::world::is_opaque_id(world.get_block(x, y, z)) {
            return false;
        }
        y += 1;
    }
    true
}

// ---------------------------------------------------------------- rendering
struct Part {
    half: [f32; 3],
    pivot: [f32; 3],
    tiles: [u32; 6], // +X, -X, +Y, -Y, +Z, -Z
    swing: f32,
    phase: f32,
    base_rot: f32,
}

fn rot_y(p: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.sin_cos();
    Vec3::new(c * p.x - s * p.z, p.y, s * p.x + c * p.z)
}

fn rot_x(p: Vec3, a: f32) -> Vec3 {
    let (s, c) = a.sin_cos();
    Vec3::new(p.x, c * p.y - s * p.z, s * p.y + c * p.z)
}

/// Build world-space textured vertices for all mobs (8 floats per vertex).
pub fn build_verts(mobs: &[Mob], out: &mut Vec<f32>) {
    build_verts_filtered(mobs, out, &|_| true);
}

/// Idem, mais ne rend que les kinds acceptés par `keep` (client : fallback
/// procédural pour les mobs sans modèle pack).
pub fn build_verts_filtered(mobs: &[Mob], out: &mut Vec<f32>, keep: &dyn Fn(MobKind) -> bool) {
    for m in mobs {
        if !keep(m.kind) {
            continue;
        }
        let flash = if m.hurt_t > 0.0 {
            2.2
        } else if m.kind == MobKind::Siffleur
            && m.fuse > 0.0
            && ((m.fuse * 7.0) as i32) % 2 == 0
        {
            2.6
        } else {
            1.0
        };
        let swing_on = if m.moving { 1.0 } else { 0.15 };
        for part in parts_of(m) {
            let swing_a = part.base_rot + (m.anim * 7.0).sin() * part.swing * swing_on;
            for d in 0..6 {
                for &ci in &[0usize, 1, 2, 0, 2, 3] {
                    let co = crate::mesher::CORNERS[d][ci];
                    let local = Vec3::new(
                        (co[0] as f32 - 0.5) * 2.0 * part.half[0],
                        (co[1] as f32 - 0.5) * 2.0 * part.half[1],
                        (co[2] as f32 - 0.5) * 2.0 * part.half[2],
                    );
                    let piv = Vec3::new(part.pivot[0], part.pivot[1], part.pivot[2]);
                    let p = rot_x(local, swing_a) + piv;
                    let p = rot_y(p, m.yaw) + m.pos;
                    let uv = crate::mesher::face_corner_uv(part.tiles[d], d, ci);
                    out.extend_from_slice(&[
                        p.x,
                        p.y,
                        p.z,
                        uv[0],
                        uv[1],
                        SHADE[d] * flash,
                        SHADE[d] * flash,
                        SHADE[d] * flash,
                    ]);
                }
            }
        }
    }
}

fn parts_of(m: &Mob) -> Vec<Part> {
    let all = |t: u32| [t, t, t, t, t, t];
    match m.kind {
        MobKind::Zombie => vec![
            Part { half: [0.12, 0.35, 0.12], pivot: [-0.13, 0.70, 0.0], tiles: all(T_ZOMBIE_LIMB), swing: 0.65, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.12, 0.35, 0.12], pivot: [0.13, 0.70, 0.0], tiles: all(T_ZOMBIE_LIMB), swing: 0.65, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.25, 0.36, 0.14], pivot: [0.0, 1.06, 0.0], tiles: all(T_ZOMBIE_BODY), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.11, 0.36, 0.11], pivot: [-0.36, 1.36, 0.0], tiles: all(T_ZOMBIE_SKIN), swing: 0.12, phase: 0.0, base_rot: -1.35 },
            Part { half: [0.11, 0.36, 0.11], pivot: [0.36, 1.36, 0.0], tiles: all(T_ZOMBIE_SKIN), swing: 0.12, phase: 3.14159, base_rot: -1.35 },
            Part { half: [0.24, 0.24, 0.24], pivot: [0.0, 1.44, 0.0], tiles: [T_ZOMBIE_SKIN, T_ZOMBIE_SKIN, T_ZOMBIE_SKIN, T_ZOMBIE_SKIN, T_ZOMBIE_SKIN, T_ZOMBIE_FACE], swing: 0.05, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Siffleur => vec![
            Part { half: [0.09, 0.18, 0.09], pivot: [-0.14, 0.35, -0.20], tiles: all(T_SIFFLEUR_SKIN), swing: 0.55, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.09, 0.18, 0.09], pivot: [0.14, 0.35, -0.20], tiles: all(T_SIFFLEUR_SKIN), swing: 0.55, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.09, 0.18, 0.09], pivot: [-0.14, 0.35, 0.20], tiles: all(T_SIFFLEUR_SKIN), swing: 0.55, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.09, 0.18, 0.09], pivot: [0.14, 0.35, 0.20], tiles: all(T_SIFFLEUR_SKIN), swing: 0.55, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.25, 0.35, 0.16], pivot: [0.0, 0.70, 0.0], tiles: all(T_SIFFLEUR_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.24, 0.24, 0.24], pivot: [0.0, 1.30, -0.04], tiles: [T_SIFFLEUR_SKIN, T_SIFFLEUR_SKIN, T_SIFFLEUR_SKIN, T_SIFFLEUR_SKIN, T_SIFFLEUR_SKIN, T_SIFFLEUR_FACE], swing: 0.06, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Pig => vec![
            Part { half: [0.10, 0.18, 0.10], pivot: [-0.18, 0.36, -0.30], tiles: all(T_PIG_SKIN), swing: 0.5, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.10, 0.18, 0.10], pivot: [0.18, 0.36, -0.30], tiles: all(T_PIG_SKIN), swing: 0.5, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.10, 0.18, 0.10], pivot: [-0.18, 0.36, 0.30], tiles: all(T_PIG_SKIN), swing: 0.5, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.10, 0.18, 0.10], pivot: [0.18, 0.36, 0.30], tiles: all(T_PIG_SKIN), swing: 0.5, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.30, 0.24, 0.46], pivot: [0.0, 0.58, 0.05], tiles: all(T_PIG_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.22, 0.21, 0.20], pivot: [0.0, 0.66, -0.52], tiles: [T_PIG_SKIN, T_PIG_SKIN, T_PIG_SKIN, T_PIG_SKIN, T_PIG_SKIN, T_PIG_FACE], swing: 0.07, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Sheep => vec![
            Part { half: [0.09, 0.24, 0.09], pivot: [-0.16, 0.48, -0.28], tiles: all(T_SHEEP_WOOL), swing: 0.45, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.09, 0.24, 0.09], pivot: [0.16, 0.48, -0.28], tiles: all(T_SHEEP_WOOL), swing: 0.45, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.09, 0.24, 0.09], pivot: [-0.16, 0.48, 0.28], tiles: all(T_SHEEP_WOOL), swing: 0.45, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.09, 0.24, 0.09], pivot: [0.16, 0.48, 0.28], tiles: all(T_SHEEP_WOOL), swing: 0.45, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.34, 0.30, 0.46], pivot: [0.0, 0.78, 0.05], tiles: all(T_SHEEP_WOOL), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.18, 0.18, 0.16], pivot: [0.0, 1.00, -0.52], tiles: [T_SHEEP_WOOL, T_SHEEP_WOOL, T_SHEEP_WOOL, T_SHEEP_WOOL, T_SHEEP_WOOL, T_SHEEP_FACE], swing: 0.08, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Rabbit => vec![
            Part { half: [0.16, 0.13, 0.22], pivot: [0.0, 0.20, 0.03], tiles: all(T_RABBIT_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.13, 0.13, 0.12], pivot: [0.0, 0.34, -0.20], tiles: [T_RABBIT_SKIN, T_RABBIT_SKIN, T_RABBIT_SKIN, T_RABBIT_SKIN, T_RABBIT_SKIN, T_RABBIT_FACE], swing: 0.1, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.035, 0.12, 0.02], pivot: [-0.05, 0.48, -0.20], tiles: all(T_RABBIT_SKIN), swing: 0.1, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.035, 0.12, 0.02], pivot: [0.05, 0.48, -0.20], tiles: all(T_RABBIT_SKIN), swing: 0.1, phase: 3.14159, base_rot: 0.0 },
        ],
        MobKind::Chicken => vec![
            Part { half: [0.035, 0.16, 0.035], pivot: [-0.08, 0.18, 0.0], tiles: all(T_CHICKEN_SKIN), swing: 0.6, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.035, 0.16, 0.035], pivot: [0.08, 0.18, 0.0], tiles: all(T_CHICKEN_SKIN), swing: 0.6, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.14, 0.14, 0.19], pivot: [0.0, 0.42, 0.0], tiles: all(T_CHICKEN_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.10, 0.10, 0.10], pivot: [0.0, 0.55, -0.14], tiles: [T_CHICKEN_SKIN, T_CHICKEN_SKIN, T_CHICKEN_SKIN, T_CHICKEN_SKIN, T_CHICKEN_SKIN, T_CHICKEN_FACE], swing: 0.08, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Cow => vec![
            Part { half: [0.12, 0.26, 0.12], pivot: [-0.22, 0.52, -0.32], tiles: all(T_COW_SKIN), swing: 0.4, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.12, 0.26, 0.12], pivot: [0.22, 0.52, -0.32], tiles: all(T_COW_SKIN), swing: 0.4, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.12, 0.26, 0.12], pivot: [-0.22, 0.52, 0.32], tiles: all(T_COW_SKIN), swing: 0.4, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.12, 0.26, 0.12], pivot: [0.22, 0.52, 0.32], tiles: all(T_COW_SKIN), swing: 0.4, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.36, 0.32, 0.52], pivot: [0.0, 0.86, 0.05], tiles: all(T_COW_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.20, 0.20, 0.18], pivot: [0.0, 1.06, -0.60], tiles: [T_COW_SKIN, T_COW_SKIN, T_COW_SKIN, T_COW_SKIN, T_COW_SKIN, T_COW_FACE], swing: 0.06, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Fox => vec![
            Part { half: [0.06, 0.16, 0.06], pivot: [-0.11, 0.30, -0.18], tiles: all(T_FOX_SKIN), swing: 0.55, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.06, 0.16, 0.06], pivot: [0.11, 0.30, -0.18], tiles: all(T_FOX_SKIN), swing: 0.55, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.06, 0.16, 0.06], pivot: [-0.11, 0.30, 0.18], tiles: all(T_FOX_SKIN), swing: 0.55, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.06, 0.16, 0.06], pivot: [0.11, 0.30, 0.18], tiles: all(T_FOX_SKIN), swing: 0.55, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.16, 0.16, 0.30], pivot: [0.0, 0.44, 0.06], tiles: all(T_FOX_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.13, 0.12, 0.12], pivot: [0.0, 0.52, -0.32], tiles: [T_FOX_SKIN, T_FOX_SKIN, T_FOX_SKIN, T_FOX_SKIN, T_FOX_SKIN, T_FOX_FACE], swing: 0.08, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.045, 0.10, 0.02], pivot: [-0.07, 0.64, -0.32], tiles: all(T_FOX_SKIN), swing: 0.05, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.045, 0.10, 0.02], pivot: [0.07, 0.64, -0.32], tiles: all(T_FOX_SKIN), swing: 0.05, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.07, 0.07, 0.16], pivot: [0.0, 0.50, 0.38], tiles: all(T_FOX_SKIN), swing: 0.25, phase: 0.0, base_rot: 0.5 },
        ],
        MobKind::Slime => vec![
            Part { half: [0.42, 0.42, 0.42], pivot: [0.0, 0.42, 0.0], tiles: all(T_SLIME), swing: 0.0, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Ombre => vec![
            Part { half: [0.10, 0.55, 0.10], pivot: [-0.12, 1.10, 0.0], tiles: all(T_SHADOW_SKIN), swing: 0.5, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.10, 0.55, 0.10], pivot: [0.12, 1.10, 0.0], tiles: all(T_SHADOW_SKIN), swing: 0.5, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.22, 0.62, 0.13], pivot: [0.0, 1.72, 0.0], tiles: all(T_SHADOW_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.08, 0.66, 0.08], pivot: [-0.34, 1.98, 0.0], tiles: all(T_SHADOW_SKIN), swing: 0.10, phase: 0.0, base_rot: -1.45 },
            Part { half: [0.08, 0.66, 0.08], pivot: [0.34, 1.98, 0.0], tiles: all(T_SHADOW_SKIN), swing: 0.10, phase: 3.14159, base_rot: -1.45 },
            Part { half: [0.22, 0.22, 0.22], pivot: [0.0, 2.58, 0.0], tiles: [T_SHADOW_SKIN, T_SHADOW_SKIN, T_SHADOW_SKIN, T_SHADOW_SKIN, T_SHADOW_SKIN, T_SHADOW_FACE], swing: 0.04, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Bee => vec![
            Part { half: [0.16, 0.13, 0.22], pivot: [0.0, 0.35, 0.02], tiles: all(T_BEE_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.11, 0.11, 0.08], pivot: [0.0, 0.40, -0.26], tiles: [T_BEE_SKIN, T_BEE_SKIN, T_BEE_SKIN, T_BEE_SKIN, T_BEE_SKIN, T_BEE_FACE], swing: 0.05, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.04, 0.02, 0.14], pivot: [-0.16, 0.52, -0.04], tiles: all(T_BEE_SKIN), swing: 0.9, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.04, 0.02, 0.14], pivot: [0.16, 0.52, -0.04], tiles: all(T_BEE_SKIN), swing: 0.9, phase: 3.14159, base_rot: 0.0 },
        ],
        MobKind::Parrot => vec![
            Part { half: [0.10, 0.13, 0.16], pivot: [0.0, 0.30, 0.02], tiles: all(T_PARROT_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.09, 0.09, 0.09], pivot: [0.0, 0.46, -0.14], tiles: [T_PARROT_SKIN, T_PARROT_SKIN, T_PARROT_SKIN, T_PARROT_SKIN, T_PARROT_SKIN, T_PARROT_FACE], swing: 0.06, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.03, 0.10, 0.14], pivot: [-0.11, 0.36, 0.0], tiles: all(T_PARROT_SKIN), swing: 0.8, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.03, 0.10, 0.14], pivot: [0.11, 0.36, 0.0], tiles: all(T_PARROT_SKIN), swing: 0.8, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.05, 0.03, 0.12], pivot: [0.0, 0.34, 0.18], tiles: all(T_PARROT_SKIN), swing: 0.2, phase: 0.0, base_rot: 0.6 },
        ],
        MobKind::Turtle => vec![
            Part { half: [0.28, 0.12, 0.34], pivot: [0.0, 0.16, 0.02], tiles: all(T_TURTLE_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.10, 0.07, 0.09], pivot: [0.0, 0.24, -0.40], tiles: [T_TURTLE_SKIN, T_TURTLE_SKIN, T_TURTLE_SKIN, T_TURTLE_SKIN, T_TURTLE_SKIN, T_TURTLE_FACE], swing: 0.06, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.06, 0.06, 0.08], pivot: [-0.22, 0.08, -0.24], tiles: all(T_TURTLE_SKIN), swing: 0.35, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.06, 0.06, 0.08], pivot: [0.22, 0.08, -0.24], tiles: all(T_TURTLE_SKIN), swing: 0.35, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.06, 0.06, 0.08], pivot: [-0.22, 0.08, 0.26], tiles: all(T_TURTLE_SKIN), swing: 0.35, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.06, 0.06, 0.08], pivot: [0.22, 0.08, 0.26], tiles: all(T_TURTLE_SKIN), swing: 0.35, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Dolphin => vec![
            Part { half: [0.14, 0.16, 0.52], pivot: [0.0, 0.30, 0.04], tiles: all(T_DOLPHIN_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.10, 0.10, 0.16], pivot: [0.0, 0.32, -0.56], tiles: [T_DOLPHIN_SKIN, T_DOLPHIN_SKIN, T_DOLPHIN_SKIN, T_DOLPHIN_SKIN, T_DOLPHIN_SKIN, T_DOLPHIN_FACE], swing: 0.05, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.04, 0.14, 0.16], pivot: [0.0, 0.34, 0.52], tiles: all(T_DOLPHIN_SKIN), swing: 0.5, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.03, 0.10, 0.12], pivot: [0.0, 0.48, 0.0], tiles: all(T_DOLPHIN_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Goat => vec![
            Part { half: [0.09, 0.26, 0.09], pivot: [-0.16, 0.52, -0.26], tiles: all(T_GOAT_SKIN), swing: 0.45, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.09, 0.26, 0.09], pivot: [0.16, 0.52, -0.26], tiles: all(T_GOAT_SKIN), swing: 0.45, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.09, 0.26, 0.09], pivot: [-0.16, 0.52, 0.28], tiles: all(T_GOAT_SKIN), swing: 0.45, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.09, 0.26, 0.09], pivot: [0.16, 0.52, 0.28], tiles: all(T_GOAT_SKIN), swing: 0.45, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.30, 0.28, 0.44], pivot: [0.0, 0.80, 0.05], tiles: all(T_GOAT_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.17, 0.16, 0.14], pivot: [0.0, 1.02, -0.50], tiles: [T_GOAT_SKIN, T_GOAT_SKIN, T_GOAT_SKIN, T_GOAT_SKIN, T_GOAT_SKIN, T_GOAT_FACE], swing: 0.06, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Frog => vec![
            Part { half: [0.16, 0.09, 0.18], pivot: [0.0, 0.12, 0.02], tiles: all(T_FROG_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.05, 0.05, 0.14], pivot: [-0.14, 0.10, 0.18], tiles: all(T_FROG_SKIN), swing: 0.7, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.05, 0.05, 0.14], pivot: [0.14, 0.10, 0.18], tiles: all(T_FROG_SKIN), swing: 0.7, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.04, 0.04, 0.10], pivot: [-0.10, 0.10, -0.16], tiles: all(T_FROG_SKIN), swing: 0.5, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.04, 0.04, 0.10], pivot: [0.10, 0.10, -0.16], tiles: all(T_FROG_SKIN), swing: 0.5, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.11, 0.06, 0.10], pivot: [0.0, 0.20, -0.20], tiles: [T_FROG_SKIN, T_FROG_SKIN, T_FROG_SKIN, T_FROG_SKIN, T_FROG_SKIN, T_FROG_FACE], swing: 0.06, phase: 0.0, base_rot: 0.0 },
        ],
        MobKind::Axolotl => vec![
            Part { half: [0.10, 0.09, 0.28], pivot: [0.0, 0.16, 0.03], tiles: all(T_AXOLOTL_SKIN), swing: 0.0, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.08, 0.07, 0.09], pivot: [0.0, 0.20, -0.32], tiles: [T_AXOLOTL_SKIN, T_AXOLOTL_SKIN, T_AXOLOTL_SKIN, T_AXOLOTL_SKIN, T_AXOLOTL_SKIN, T_AXOLOTL_FACE], swing: 0.06, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.02, 0.07, 0.05], pivot: [-0.10, 0.26, -0.30], tiles: all(T_AXOLOTL_SKIN), swing: 0.4, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.02, 0.07, 0.05], pivot: [0.10, 0.26, -0.30], tiles: all(T_AXOLOTL_SKIN), swing: 0.4, phase: 3.14159, base_rot: 0.0 },
            Part { half: [0.03, 0.05, 0.12], pivot: [0.0, 0.18, 0.34], tiles: all(T_AXOLOTL_SKIN), swing: 0.6, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.02, 0.04, 0.06], pivot: [-0.08, 0.06, -0.16], tiles: all(T_AXOLOTL_SKIN), swing: 0.3, phase: 0.0, base_rot: 0.0 },
            Part { half: [0.02, 0.04, 0.06], pivot: [0.08, 0.06, -0.16], tiles: all(T_AXOLOTL_SKIN), swing: 0.3, phase: 3.14159, base_rot: 0.0 },
        ],
        MobKind::GlowSquid => {
            // 1.18 glow squid: round mantle + eyes + drifting tentacles
            vec![
                Part { half: [0.28, 0.24, 0.28], pivot: [0.0, 0.42, 0.0], tiles: all(T_GLOW_SQUID_SKIN), swing: 0.04, phase: 0.0, base_rot: 0.0 },
                Part { half: [0.16, 0.10, 0.16], pivot: [0.0, 0.24, 0.0], tiles: [T_GLOW_SQUID_SKIN, T_GLOW_SQUID_SKIN, T_GLOW_SQUID_SKIN, T_GLOW_SQUID_SKIN, T_GLOW_SQUID_SKIN, T_GLOW_SQUID_FACE], swing: 0.05, phase: 0.0, base_rot: 0.0 },
                Part { half: [0.03, 0.14, 0.03], pivot: [-0.16, 0.10, -0.14], tiles: all(T_GLOW_SQUID_SKIN), swing: 0.35, phase: 0.0, base_rot: 3.0 },
                Part { half: [0.03, 0.14, 0.03], pivot: [0.16, 0.10, -0.14], tiles: all(T_GLOW_SQUID_SKIN), swing: 0.35, phase: 1.2, base_rot: 3.0 },
                Part { half: [0.03, 0.14, 0.03], pivot: [-0.16, 0.10, 0.14], tiles: all(T_GLOW_SQUID_SKIN), swing: 0.35, phase: 2.1, base_rot: 3.0 },
                Part { half: [0.03, 0.14, 0.03], pivot: [0.16, 0.10, 0.14], tiles: all(T_GLOW_SQUID_SKIN), swing: 0.35, phase: 3.3, base_rot: 3.0 },
                Part { half: [0.03, 0.12, 0.03], pivot: [0.0, 0.08, -0.18], tiles: all(T_GLOW_SQUID_SKIN), swing: 0.3, phase: 0.7, base_rot: 3.0 },
                Part { half: [0.03, 0.12, 0.03], pivot: [0.0, 0.08, 0.18], tiles: all(T_GLOW_SQUID_SKIN), swing: 0.3, phase: 2.7, base_rot: 3.0 },
            ]
        }
        MobKind::Player => {
            // remote players: classic humanoid - pants, colored shirt, bare
            // arms, blocky head with a face. Arms hang down (unlike zombies).
            let shirt = MobKind::shirt_tile(m.skin);
            vec![
                Part { half: [0.115, 0.34, 0.115], pivot: [-0.12, 0.72, 0.0], tiles: all(T_PLAYER_PANTS), swing: 0.62, phase: 0.0, base_rot: 0.0 },
                Part { half: [0.115, 0.34, 0.115], pivot: [0.12, 0.72, 0.0], tiles: all(T_PLAYER_PANTS), swing: 0.62, phase: 3.14159, base_rot: 0.0 },
                Part { half: [0.24, 0.36, 0.13], pivot: [0.0, 1.09, 0.0], tiles: all(shirt), swing: 0.0, phase: 0.0, base_rot: 0.0 },
                Part { half: [0.10, 0.33, 0.10], pivot: [-0.35, 1.40, 0.0], tiles: all(T_PLAYER_SKIN), swing: 0.55, phase: 0.0, base_rot: 0.0 },
                Part { half: [0.10, 0.33, 0.10], pivot: [0.35, 1.40, 0.0], tiles: all(T_PLAYER_SKIN), swing: 0.55, phase: 3.14159, base_rot: 0.0 },
                Part { half: [0.24, 0.24, 0.24], pivot: [0.0, 1.48, 0.0], tiles: [T_PLAYER_SKIN, T_PLAYER_SKIN, T_PLAYER_SKIN, T_PLAYER_SKIN, T_PLAYER_SKIN, T_PLAYER_FACE], swing: 0.05, phase: 0.0, base_rot: 0.0 },
            ]
        }
    }
}

/// Item icon tile for drops / food (used by HUD).
pub fn drop_tile(kind: MobKind) -> u32 {
    match kind {
        MobKind::Pig | MobKind::Cow | MobKind::Chicken => T_MEAT,
        _ => T_MEAT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::*;

    fn flat_world() -> World {
        let mut w = World::new(7);
        for cx in 0..=1 {
            for cz in 0..=1 {
                w.gen_chunk(cx, cz);
            }
        }
        for lx in 0..32 {
            for lz in 0..32 {
                for y in 1..CY {
                    w.set_block(lx, y as i32, lz, AIR);
                }
                w.set_block(lx, 0, lz, STONE);
                w.set_block(lx, 1, lz, GRASS);
            }
        }
        w
    }

    #[test]
    fn zombie_chases_the_player() {
        let mut w = flat_world();
        let mut p = Player::new();
        p.pos = Vec3::new(8.5, 2.02, 20.5);
        let mut m = Mob::new(MobKind::Zombie, Vec3::new(8.5, 2.02, 8.5));
        let mut mobs = vec![m.clone()];
        let mut parts = Vec::new();
        let mut onb = |_: i32, _: i32, _: i32| {};
        let start = (mobs[0].pos - p.pos).length();
        for _ in 0..240 {
            update_mobs(&mut mobs, &mut w, &mut p, &mut parts, 0.1, 0.0, 1.0 / 60.0, &mut onb);
        }
        m = mobs[0].clone();
        let end = (m.pos - p.pos).length();
        assert!(end < start - 2.0, "zombie should approach: {start} -> {end}");
    }

    #[test]
    fn siffleur_explodes_and_destroys_blocks() {
        let mut w = flat_world();
        let mut p = Player::new();
        p.pos = Vec3::new(8.5, 2.02, 10.6); // ~2 blocks away -> fuse triggers
        let mut mobs = vec![Mob::new(MobKind::Siffleur, Vec3::new(8.5, 2.02, 8.5))];
        let mut parts = Vec::new();
        let destroyed = std::cell::Cell::new(0i32);
        let mut onb = |_: i32, _: i32, _: i32| destroyed.set(destroyed.get() + 1);
        let mut exploded = false;
        for _ in 0..300 {
            update_mobs(&mut mobs, &mut w, &mut p, &mut parts, 0.1, 0.0, 1.0 / 60.0, &mut onb);
            if destroyed.get() > 0 {
                exploded = true;
                break;
            }
        }
        assert!(exploded, "siffleur must explode near the player");
        assert!(p.hp < 20, "explosion must damage the player");
        assert_eq!(w.get_block(8, 1, 8), AIR, "ground below blast must be gone");
    }

    #[test]
    fn pick_mob_hits_the_nearest() {
        let mobs = vec![
            Mob::new(MobKind::Zombie, Vec3::new(5.0, 0.0, 0.0)),
            Mob::new(MobKind::Pig, Vec3::new(3.0, 0.0, 0.0)),
        ];
        let o = Vec3::new(0.0, 0.5, 0.0);
        let d = Vec3::new(1.0, 0.0, 0.0);
        let (i, t) = pick_mob(&mobs, o, d, 10.0).unwrap();
        assert_eq!(i, 1);
        assert!((t - 2.65).abs() < 0.2);
    }

    #[test]
    fn mobs_do_not_fall_through_the_ground() {
        let mut w = flat_world();
        let mut p = Player::new();
        p.pos = Vec3::new(30.5, 2.02, 30.5);
        let mut mobs = vec![
            Mob::new(MobKind::Pig, Vec3::new(4.5, 2.02, 4.5)),
            Mob::new(MobKind::Sheep, Vec3::new(10.5, 2.02, 4.5)),
            Mob::new(MobKind::Rabbit, Vec3::new(12.5, 2.02, 12.5)),
        ];
        let mut parts = Vec::new();
        let mut onb = |_: i32, _: i32, _: i32| {};
        for _ in 0..600 {
            update_mobs(&mut mobs, &mut w, &mut p, &mut parts, 0.9, 0.0, 1.0 / 60.0, &mut onb);
        }
        for m in &mobs {
            assert!(m.pos.y >= 1.9, "{:?} fell through: y={}", m.kind, m.pos.y);
            assert!(m.pos.y < 3.0, "{:?} flew away: y={}", m.kind, m.pos.y);
        }
    }

    #[test]
    fn zombies_burn_in_daylight() {
        let mut w = flat_world();
        let mut p = Player::new();
        p.pos = Vec3::new(30.5, 2.02, 30.5); // far away: no aggro
        let mut mobs = vec![Mob::new(MobKind::Zombie, Vec3::new(8.5, 2.02, 8.5))];
        let mut parts = Vec::new();
        let mut onb = |_: i32, _: i32, _: i32| {};
        for _ in 0..600 {
            update_mobs(&mut mobs, &mut w, &mut p, &mut parts, 1.0, 0.0, 1.0 / 60.0, &mut onb);
        }
        assert!(
            mobs.iter().all(|m| m.hp < MobKind::Zombie.hp_max()),
            "zombie must burn in daylight"
        );
    }

    #[test]
    fn spawner_produces_mobs_at_night() {
        let w = flat_world();
        let mut p = Player::new();
        p.pos = Vec3::new(8.5, 2.02, 8.5);
        let mut mobs = Vec::new();
        let mut sp = Spawner::new();
        for _ in 0..600 {
            spawn_tick(&mut mobs, &w, &[p.pos], 0.1, 1.0 / 60.0, &mut sp);
        }
        assert!(!mobs.is_empty(), "hostiles must spawn at night");
        assert!(mobs.iter().all(|m| m.kind.hostile()));
    }

    #[test]
    fn mob_verts_are_built() {
        let mobs = vec![Mob::new(MobKind::Zombie, Vec3::new(0.0, 0.0, 0.0))];
        let mut out = Vec::new();
        build_verts(&mobs, &mut out);
        // 6 parts x 6 faces x 6 verts x 8 floats
        assert_eq!(out.len(), 6 * 6 * 6 * 8);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn particles_pack_7_floats() {
        let mut ps = vec![Particle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            1.0,
            [1.0, 0.0, 0.0, 1.0],
            true,
        )];
        let w = flat_world();
        let mut out = Vec::new();
        update_particles(&mut ps, &w, 0.0, 0.016);
        ps[0].pack(&mut out);
        assert_eq!(out.len(), 7);
    }
}
