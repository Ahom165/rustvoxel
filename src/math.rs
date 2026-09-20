// Minimal f32 linear algebra, column-major mat4 (OpenGL convention).

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length_sq(self) -> f32 {
        self.dot(self)
    }
    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }
    pub fn normalize(self) -> Vec3 {
        let l = self.length();
        if l > 1e-8 {
            self * (1.0 / l)
        } else {
            Vec3::new(0.0, 0.0, 0.0)
        }
    }
    pub fn floor(self) -> Vec3 {
        Vec3::new(self.x.floor(), self.y.floor(), self.z.floor())
    }
}

impl std::ops::Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl std::ops::Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl std::ops::Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl std::ops::AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}

pub type Mat4 = [f32; 16];

/// out = a * b (apply b first, then a). Column-major.
pub fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut o = [0f32; 16];
    for c in 0..4 {
        for r in 0..4 {
            let mut s = 0.0;
            for k in 0..4 {
                s += a[k * 4 + r] * b[c * 4 + k];
            }
            o[c * 4 + r] = s;
        }
    }
    o
}

pub fn perspective(fovy_rad: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fovy_rad * 0.5).tan();
    let nf = 1.0 / (near - far);
    [
        f / aspect,
        0.0,
        0.0,
        0.0,
        0.0,
        f,
        0.0,
        0.0,
        0.0,
        0.0,
        (far + near) * nf,
        -1.0,
        0.0,
        0.0,
        2.0 * far * near * nf,
        0.0,
    ]
}

fn rot_x(a: f32) -> Mat4 {
    let (s, c) = a.sin_cos();
    [1.0, 0.0, 0.0, 0.0, 0.0, c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0]
}

fn rot_y(a: f32) -> Mat4 {
    let (s, c) = a.sin_cos();
    [c, 0.0, -s, 0.0, 0.0, 1.0, 0.0, 0.0, s, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0]
}

fn translate(t: Vec3) -> Mat4 {
    [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, t.x, t.y, t.z, 1.0,
    ]
}

/// World-to-camera view matrix. yaw=0 looks towards -Z, yaw>0 turns right.
pub fn view_matrix(eye: Vec3, yaw: f32, pitch: f32) -> Mat4 {
    mat_mul(&rot_x(-pitch), &mat_mul(&rot_y(yaw), &translate(eye * -1.0)))
}

/// Maps pixel space (0,0 = top-left, y down) to clip space.
pub fn ortho_pixels(w: f32, h: f32) -> Mat4 {
    [
        2.0 / w,
        0.0,
        0.0,
        0.0,
        0.0,
        -2.0 / h,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        -1.0,
        1.0,
        0.0,
        1.0,
    ]
}

/// Full look direction (pitch positive = looking up).
pub fn forward(yaw: f32, pitch: f32) -> Vec3 {
    let cp = pitch.cos();
    Vec3::new(cp * yaw.sin(), pitch.sin(), -cp * yaw.cos())
}

pub fn fwd_flat(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, -yaw.cos())
}

pub fn right(yaw: f32) -> Vec3 {
    Vec3::new(yaw.cos(), 0.0, yaw.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_maps_forward_to_minus_z() {
        // yaw = 0 : point straight ahead should land on -Z in camera space.
        let v = view_matrix(Vec3::new(0.0, 0.0, 0.0), 0.0, 0.0);
        let p = mul_point(&v, Vec3::new(0.0, 0.0, -5.0));
        assert!((p.x).abs() < 1e-4 && (p.y).abs() < 1e-4);
        assert!((p.z + 5.0).abs() < 1e-4);

        // yaw = +pi/2 : facing +X, so world +X must map to camera -Z.
        let v = view_matrix(Vec3::new(0.0, 0.0, 0.0), std::f32::consts::FRAC_PI_2, 0.0);
        let p = mul_point(&v, Vec3::new(5.0, 0.0, 0.0));
        assert!((p.x).abs() < 1e-4 && (p.y).abs() < 1e-4);
        assert!((p.z + 5.0).abs() < 1e-4);

        // pitch up: world direction forward() must map to camera -Z.
        let pitch = 0.4;
        let yaw = 1.1;
        let eye = Vec3::new(3.0, 10.0, -2.0);
        let v = view_matrix(eye, yaw, pitch);
        let d = forward(yaw, pitch);
        let p = mul_point(&v, eye + d * 7.0);
        assert!((p.x).abs() < 1e-3 && (p.y).abs() < 1e-3);
        assert!((p.z + 7.0).abs() < 1e-3);
    }

    fn mul_point(m: &Mat4, p: Vec3) -> Vec3 {
        let x = m[0] * p.x + m[4] * p.y + m[8] * p.z + m[12];
        let y = m[1] * p.x + m[5] * p.y + m[9] * p.z + m[13];
        let z = m[2] * p.x + m[6] * p.y + m[10] * p.z + m[14];
        Vec3::new(x, y, z)
    }

    #[test]
    fn perspective_is_finite() {
        let m = perspective(75f32.to_radians(), 16.0 / 9.0, 0.1, 400.0);
        assert!(m.iter().all(|v| v.is_finite()));
    }
}
