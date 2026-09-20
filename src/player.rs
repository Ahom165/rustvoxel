// Player physics: AABB collision per axis, gravity, water, fly mode.
use crate::math::{fwd_flat, right, Vec3};
use crate::world::{is_solid_id, World, CY, WATER};

pub const HALF_W: f32 = 0.3;
pub const HEIGHT: f32 = 1.8;
pub const EYE_H: f32 = 1.62;
pub const EPS: f32 = 1e-3;

pub fn aabb_of(pos: &Vec3, half: f32, height: f32) -> (f32, f32, f32, f32, f32, f32) {
    (
        pos.x - half,
        pos.x + half,
        pos.y,
        pos.y + height,
        pos.z - half,
        pos.z + half,
    )
}

/// Integrate + collide a generic AABB entity (used by mobs).
/// Returns (on_ground, hit_wall). Moves pos by vel*dt with axis separation.
pub fn collide_move(
    w: &World,
    pos: &mut Vec3,
    vel: &mut Vec3,
    half: f32,
    height: f32,
    dt: f32,
) -> (bool, bool) {
    let sub = ((dt / (1.0 / 120.0)).ceil() as i32).clamp(1, 8);
    let sdt = dt / sub as f32;
    let mut on_ground = false;
    let mut hit_wall = false;
    for _ in 0..sub {
        // Y
        pos.y += vel.y * sdt;
        let (x0, x1, y0, y1, z0, z1) = aabb_of(pos, half, height);
        'youter: for by in (y0 + EPS).floor() as i32..=(y1 - EPS).floor() as i32 {
            for bz in (z0 + EPS).floor() as i32..=(z1 - EPS).floor() as i32 {
                for bx in (x0 + EPS).floor() as i32..=(x1 - EPS).floor() as i32 {
                    if !is_solid_id(w.get_block(bx, by, bz)) {
                        continue;
                    }
                    if vel.y <= 0.0 {
                        pos.y = by as f32 + 1.0 + EPS;
                        on_ground = true;
                    } else {
                        pos.y = by as f32 - height - EPS;
                    }
                    vel.y = 0.0;
                    break 'youter;
                }
            }
        }
        // X
        if vel.x != 0.0 {
            pos.x += vel.x * sdt;
            let (x0, x1, y0, y1, z0, z1) = aabb_of(pos, half, height);
            'xouter: for by in (y0 + EPS).floor() as i32..=(y1 - EPS).floor() as i32 {
                for bz in (z0 + EPS).floor() as i32..=(z1 - EPS).floor() as i32 {
                    for bx in (x0 + EPS).floor() as i32..=(x1 - EPS).floor() as i32 {
                        if !is_solid_id(w.get_block(bx, by, bz)) {
                            continue;
                        }
                        if vel.x > 0.0 {
                            pos.x = bx as f32 - half - EPS;
                        } else {
                            pos.x = bx as f32 + 1.0 + half + EPS;
                        }
                        vel.x = 0.0;
                        hit_wall = true;
                        break 'xouter;
                    }
                }
            }
        }
        // Z
        if vel.z != 0.0 {
            pos.z += vel.z * sdt;
            let (x0, x1, y0, y1, z0, z1) = aabb_of(pos, half, height);
            'zouter: for by in (y0 + EPS).floor() as i32..=(y1 - EPS).floor() as i32 {
                for bz in (z0 + EPS).floor() as i32..=(z1 - EPS).floor() as i32 {
                    for bx in (x0 + EPS).floor() as i32..=(x1 - EPS).floor() as i32 {
                        if !is_solid_id(w.get_block(bx, by, bz)) {
                            continue;
                        }
                        if vel.z > 0.0 {
                            pos.z = bz as f32 - half - EPS;
                        } else {
                            pos.z = bz as f32 + 1.0 + half + EPS;
                        }
                        vel.z = 0.0;
                        hit_wall = true;
                        break 'zouter;
                    }
                }
            }
        }
    }
    (on_ground, hit_wall)
}

const GRAVITY: f32 = 32.0;
const JUMP_V: f32 = 9.0;
const WALK: f32 = 4.3;
const SPRINT: f32 = 5.8;
const FLY: f32 = 10.5;
const SWIM: f32 = 2.4;

#[derive(Clone, Copy, Default)]
pub struct Input {
    pub fwd: f32,   // -1..1
    pub side: f32,  // -1..1
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
}

pub struct Player {
    pub pos: Vec3, // feet center
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub flying: bool,
    pub in_water: bool,
    // --- survival state (Wilderness Bound) ---
    pub hp: i32,
    pub air: f32,
    pub invuln: f32,
    pub hurt_flash: f32,
    pub regen_cd: f32,
    pub peak_y: f32,
    pub drown_tick: f32,
    pub dead: bool,
}

impl Player {
    pub fn new() -> Player {
        Player {
            pos: Vec3::new(8.5, 40.0, 8.5),
            vel: Vec3::new(0.0, 0.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            flying: false,
            in_water: false,
            hp: 20,
            air: 10.0,
            invuln: 0.0,
            hurt_flash: 0.0,
            regen_cd: 0.0,
            peak_y: 40.0,
            drown_tick: 0.0,
            dead: false,
        }
    }

    /// Apply damage with a short invulnerability window and a knockback impulse.
    /// Returns true if the hit landed.
    pub fn hurt(&mut self, dmg: i32, knock: Vec3) -> bool {
        if self.dead || self.invuln > 0.0 || dmg <= 0 {
            return false;
        }
        self.hp -= dmg;
        self.invuln = 0.6;
        self.hurt_flash = 0.45;
        self.regen_cd = 6.0;
        self.vel += knock;
        if self.hp <= 0 {
            self.hp = 0;
            self.dead = true;
        }
        true
    }

    pub fn heal(&mut self, amount: i32) {
        if !self.dead {
            self.hp = (self.hp + amount).min(20);
        }
    }

    pub fn eye(&self) -> Vec3 {
        Vec3::new(self.pos.x, self.pos.y + EYE_H, self.pos.z)
    }

    fn aabb(&self) -> (f32, f32, f32, f32, f32, f32) {
        aabb_of(&self.pos, HALF_W, HEIGHT)
    }

    /// Does the player AABB overlap the given block cell?
    pub fn aabb_overlaps(&self, bx: i32, by: i32, bz: i32) -> bool {
        let (x0, x1, y0, y1, z0, z1) = self.aabb();
        (bx as f32) < x1 && (bx as f32 + 1.0) > x0 && (by as f32) < y1
            && (by as f32 + 1.0) > y0 && (bz as f32) < z1 && (bz as f32 + 1.0) > z0
    }

    pub fn respawn(&mut self, w: &World) {
        let mut y = CY as i32 - 1;
        while y > 0 && !w.is_solid(8, y, 8) {
            y -= 1;
        }
        self.pos = Vec3::new(8.5, (y + 1) as f32 + 0.01, 8.5);
        self.vel = Vec3::new(0.0, 0.0, 0.0);
        self.flying = false;
        self.hp = 20;
        self.air = 10.0;
        self.dead = false;
        self.invuln = 1.5;
        self.hurt_flash = 0.0;
        self.peak_y = self.pos.y;
        self.drown_tick = 0.0;
    }

    pub fn update(&mut self, dt: f32, input: &Input, w: &World) {
        // chunk under the player must exist (streaming safety net)
        let pcx = (self.pos.x / 16.0).floor() as i32;
        let pcz = (self.pos.z / 16.0).floor() as i32;
        if !w.has_chunk((pcx, pcz)) {
            return;
        }

        let feet = w.get_block(self.pos.x.floor() as i32, (self.pos.y + 0.4).floor() as i32, self.pos.z.floor() as i32);
        let head = w.get_block(self.pos.x.floor() as i32, self.eye().y.floor() as i32, self.pos.z.floor() as i32);
        self.in_water = feet == WATER || head == WATER;

        // horizontal wish direction
        let f = fwd_flat(self.yaw);
        let r = right(self.yaw);
        let mut wish = f * input.fwd + r * input.side;
        let wl = wish.length();
        if wl > 1.0 {
            wish = wish * (1.0 / wl);
        }

        let speed = if self.flying {
            FLY * if input.sprint { 2.0 } else { 1.0 }
        } else if self.in_water {
            SWIM
        } else if input.sprint {
            SPRINT
        } else {
            WALK
        };

        let accel = if self.flying {
            10.0
        } else if self.on_ground {
            12.0
        } else if self.in_water {
            4.0
        } else {
            3.0
        };
        let k = 1.0 - (-accel * dt).exp();
        self.vel.x += (wish.x * speed - self.vel.x) * k;
        self.vel.z += (wish.z * speed - self.vel.z) * k;

        // vertical control
        if self.flying {
            let target = if input.jump {
                9.0
            } else if input.sneak {
                -9.0
            } else {
                0.0
            };
            self.vel.y += (target - self.vel.y) * (1.0 - (-10.0 * dt).exp());
        } else if self.in_water {
            let target = if input.jump {
                3.6
            } else if input.sneak {
                -3.4
            } else {
                -1.4
            };
            self.vel.y += (target - self.vel.y) * (1.0 - (-4.5 * dt).exp());
        } else {
            self.vel.y -= GRAVITY * dt;
            if self.vel.y < -78.0 {
                self.vel.y = -78.0;
            }
            if input.jump && self.on_ground {
                self.vel.y = JUMP_V;
                self.on_ground = false;
            }
        }

        // integrate in substeps to avoid tunneling
        let sub = ((dt / (1.0 / 120.0)).ceil() as i32).clamp(1, 8);
        let sdt = dt / sub as f32;
        self.on_ground = false;
        for _ in 0..sub {
            self.move_axis(1, self.vel.y * sdt, w);
            self.move_axis(0, self.vel.x * sdt, w);
            self.move_axis(2, self.vel.z * sdt, w);
        }

        // ---------------- survival (Wilderness Bound) ----------------
        self.invuln = (self.invuln - dt).max(0.0);
        self.hurt_flash = (self.hurt_flash - dt).max(0.0);

        // fall damage: track the highest point while airborne
        if self.flying || self.in_water {
            self.peak_y = self.pos.y;
        } else if self.on_ground {
            let fall = self.peak_y - self.pos.y;
            self.peak_y = self.pos.y;
            if fall > 3.5 {
                let dmg = ((fall - 3.2) * 1.15).round() as i32;
                if dmg > 0 {
                    self.hurt(dmg, Vec3::new(0.0, 2.0, 0.0));
                }
            }
        } else if self.pos.y > self.peak_y {
            self.peak_y = self.pos.y;
        }

        // drowning: air runs out when the eye is inside water
        let eye_in = w.get_block(
            self.eye().x.floor() as i32,
            self.eye().y.floor() as i32,
            self.eye().z.floor() as i32,
        ) == crate::world::WATER;
        if eye_in {
            self.air -= dt;
            if self.air <= 0.0 {
                self.drown_tick += dt;
                if self.drown_tick >= 1.2 {
                    self.drown_tick = 0.0;
                    self.invuln = 0.0;
                    self.hurt(2, Vec3::new(0.0, 0.0, 0.0));
                }
            }
        } else {
            self.air = (self.air + dt * 3.0).min(10.0);
            self.drown_tick = 0.0;
        }

        // slow natural regeneration
        if !self.dead && self.hp < 20 && self.invuln <= 0.0 {
            self.regen_cd -= dt;
            if self.regen_cd <= 0.0 {
                self.regen_cd = 2.5;
                self.hp += 1;
            }
        }

        // cactus contact damage
        let (cx0, cx1, cy0, cy1, cz0, cz1) = self.aabb();
        'cact: for by in (cy0 + 0.1).floor() as i32..=(cy1 - 0.1).floor() as i32 {
            for bz in (cz0 - 0.05).floor() as i32..=(cz1 + 0.05).floor() as i32 {
                for bx in (cx0 - 0.05).floor() as i32..=(cx1 + 0.05).floor() as i32 {
                    if w.get_block(bx, by, bz) == crate::world::CACTUS
                        || w.get_block(bx, by, bz) == crate::world::MAGMA
                    {
                        self.hurt(1, Vec3::new(0.0, 1.5, 0.0));
                        break 'cact;
                    }
                }
            }
        }
    }

    fn move_axis(&mut self, axis: usize, d: f32, w: &World) {
        if d == 0.0 {
            return;
        }
        match axis {
            0 => self.pos.x += d,
            1 => self.pos.y += d,
            _ => self.pos.z += d,
        }
        let (x0, x1, y0, y1, z0, z1) = self.aabb();
        let bx0 = (x0 + EPS).floor() as i32;
        let bx1 = (x1 - EPS).floor() as i32;
        let by0 = (y0 + EPS).floor() as i32;
        let by1 = (y1 - EPS).floor() as i32;
        let bz0 = (z0 + EPS).floor() as i32;
        let bz1 = (z1 - EPS).floor() as i32;

        for by in by0..=by1 {
            for bz in bz0..=bz1 {
                for bx in bx0..=bx1 {
                    if !is_solid_id(w.get_block(bx, by, bz)) {
                        continue;
                    }
                    match axis {
                        0 => {
                            if d > 0.0 {
                                self.pos.x = bx as f32 - HALF_W - EPS;
                            } else {
                                self.pos.x = bx as f32 + 1.0 + HALF_W + EPS;
                            }
                            self.vel.x = 0.0;
                        }
                        1 => {
                            if d > 0.0 {
                                self.pos.y = by as f32 - HEIGHT - EPS;
                            } else {
                                self.pos.y = by as f32 + 1.0 + EPS;
                                self.on_ground = true;
                            }
                            self.vel.y = 0.0;
                        }
                        _ => {
                            if d > 0.0 {
                                self.pos.z = bz as f32 - HALF_W - EPS;
                            } else {
                                self.pos.z = bz as f32 + 1.0 + HALF_W + EPS;
                            }
                            self.vel.z = 0.0;
                        }
                    }
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_falls_and_lands() {
        let mut w = World::new(5);
        w.gen_chunk(0, 0);
        // clear any tree/plant above the spawn column
        for y in (w.height_at(8, 8) + 1)..CY as i32 {
            w.set_block(8, y, 8, crate::world::AIR);
        }
        let mut p = Player::new();
        p.respawn(&w);
        let input = Input::default();
        for _ in 0..300 {
            p.update(1.0 / 60.0, &input, &w);
        }
        assert!(p.on_ground, "player must land");
        assert!(p.vel.y.abs() < 1e-4);
        // must not sink into the ground
        let h = w.height_at(8, 8);
        assert!(p.pos.y >= h as f32, "feet below terrain: {}", p.pos.y);
        assert!(p.pos.y < h as f32 + 1.5);
    }

    #[test]
    fn walls_stop_the_player() {
        let mut w = World::new(5);
        w.gen_chunk(0, 0);
        // full-height wall at z=4 so it cannot be walked under or jumped over
        for y in 0..CY as i32 {
            for dx in 4..=12 {
                w.set_block(dx, y, 4, crate::world::STONE);
            }
        }
        let mut p = Player::new();
        p.respawn(&w);
        let input = Input { fwd: 1.0, ..Default::default() }; // walks -Z
        for _ in 0..600 {
            p.update(1.0 / 60.0, &input, &w);
        }
        assert!(
            p.pos.z > 4.5,
            "player passed through a wall: z={}",
            p.pos.z
        );
        assert!(p.pos.z < 6.0, "wall should stop the player: z={}", p.pos.z);
    }

    #[test]
    fn jump_reaches_about_one_block() {
        let mut w = World::new(5);
        w.gen_chunk(0, 0);
        // solid platform high above any terrain (gen caps at CY-8=120)
        // so the test controls the ground
        for dx in 6..=10 {
            for dz in 6..=10 {
                w.set_block(dx, 121, dz, crate::world::STONE);
            }
        }
        let mut p = Player::new();
        p.pos = Vec3::new(8.5, 122.01, 8.5);
        p.vel = Vec3::new(0.0, 0.0, 0.0);
        let idle = Input::default();
        for _ in 0..60 {
            p.update(1.0 / 60.0, &idle, &w);
        }
        assert!(p.on_ground, "player must settle on the platform");
        let start = p.pos.y;
        let mut input = Input::default();
        input.jump = true;
        let mut apex = start;
        for _ in 0..90 {
            p.update(1.0 / 60.0, &input, &w);
            apex = apex.max(p.pos.y);
        }
        let gain = apex - start;
        assert!(gain > 0.9 && gain < 1.6, "jump height {gain}");
    }
}
