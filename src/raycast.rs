// Voxel ray traversal (Amanatides & Woo DDA).
use crate::math::Vec3;
use crate::world::World;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Face normal, pointing towards the ray origin.
    pub nx: i32,
    pub ny: i32,
    pub nz: i32,
    /// Distance along the ray at which the hit occurred.
    pub t: f32,
}

#[allow(unused_assignments)]
pub fn raycast(w: &World, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RayHit> {
    let d = dir.normalize();
    let mut x = origin.x.floor() as i32;
    let mut y = origin.y.floor() as i32;
    let mut z = origin.z.floor() as i32;

    let step_x = if d.x > 0.0 { 1 } else { -1 };
    let step_y = if d.y > 0.0 { 1 } else { -1 };
    let step_z = if d.z > 0.0 { 1 } else { -1 };

    let inf = f32::INFINITY;
    let tdx = if d.x != 0.0 { (1.0 / d.x).abs() } else { inf };
    let tdy = if d.y != 0.0 { (1.0 / d.y).abs() } else { inf };
    let tdz = if d.z != 0.0 { (1.0 / d.z).abs() } else { inf };

    let mut tmx = if d.x != 0.0 {
        let f = if d.x > 0.0 {
            (x + 1) as f32 - origin.x
        } else {
            origin.x - x as f32
        };
        f * tdx
    } else {
        inf
    };
    let mut tmy = if d.y != 0.0 {
        let f = if d.y > 0.0 {
            (y + 1) as f32 - origin.y
        } else {
            origin.y - y as f32
        };
        f * tdy
    } else {
        inf
    };
    let mut tmz = if d.z != 0.0 {
        let f = if d.z > 0.0 {
            (z + 1) as f32 - origin.z
        } else {
            origin.z - z as f32
        };
        f * tdz
    } else {
        inf
    };

    let mut nx = 0i32;
    let mut ny = 0i32;
    let mut nz = 0i32;
    let mut t = 0.0f32;

    // Check the starting cell too (head inside a block edge-case).
    let b0 = w.get_block(x, y, z);
    if crate::world::is_targetable(b0) {
        return Some(RayHit { x, y, z, nx: 0, ny: 1, nz: 0, t: 0.0 });
    }

    loop {
        if tmx < tmy && tmx < tmz {
            x += step_x;
            t = tmx;
            tmx += tdx;
            nx = -step_x;
            ny = 0;
            nz = 0;
        } else if tmy < tmz {
            y += step_y;
            t = tmy;
            tmy += tdy;
            nx = 0;
            ny = -step_y;
            nz = 0;
        } else {
            z += step_z;
            t = tmz;
            tmz += tdz;
            nx = 0;
            ny = 0;
            nz = -step_z;
        }
        if t > max_dist {
            return None;
        }
        let b = w.get_block(x, y, z);
        // water is not targetable; cross-plants are
        if crate::world::is_targetable(b) {
            return Some(RayHit { x, y, z, nx, ny, nz, t });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn straight_down_hits_top_face() {
        let mut w = World::new(5);
        w.gen_chunk(0, 0);
        // clear any tree canopy above the test column
        for y in (w.height_at(8, 8) + 1)..crate::world::CY as i32 {
            w.set_block(8, y, 8, crate::world::AIR);
        }
        let h = w.height_at(8, 8);
        let o = Vec3::new(8.5, 63.5, 8.5);
        let hit = raycast(&w, o, Vec3::new(0.0, -1.0, 0.0), 200.0)
            .expect("must hit the ground");
        assert_eq!(hit.x, 8);
        assert_eq!(hit.z, 8);
        assert_eq!(hit.y, h);
        assert_eq!(hit.ny, 1);
        assert!(hit.t > 0.0 && hit.t < 200.0);
    }

    #[test]
    fn upward_ray_misses() {
        let mut w = World::new(5);
        w.gen_chunk(0, 0);
        for y in (w.height_at(8, 8) + 1)..crate::world::CY as i32 {
            w.set_block(8, y, 8, crate::world::AIR);
        }
        let h = w.height_at(8, 8);
        let o = Vec3::new(8.5, h as f32 + 1.5, 8.5);
        assert!(raycast(&w, o, Vec3::new(0.0, 1.0, 0.0), 200.0).is_none());
    }

    #[test]
    fn horizontal_ray_hits_side_face() {
        let mut w = World::new(5);
        w.gen_chunk(0, 0);
        // y=58 is above the tallest mountain peak (clamped at CY-10 = 54)
        w.set_block(10, 58, 8, crate::world::STONE);
        let o = Vec3::new(4.5, 58.5, 8.5);
        let hit = raycast(&w, o, Vec3::new(1.0, 0.0, 0.0), 50.0).expect("hit wall");
        assert_eq!(hit.x, 10);
        assert_eq!(hit.nx, -1);
    }
}
