use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn v3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

impl Vec3 {
    pub const ZERO: Vec3 = v3(0.0, 0.0, 0.0);
    pub const ONE: Vec3 = v3(1.0, 1.0, 1.0);

    pub fn splat(s: f32) -> Vec3 {
        v3(s, s, s)
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        v3(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn len_sq(self) -> f32 {
        self.dot(self)
    }

    pub fn len(self) -> f32 {
        self.len_sq().sqrt()
    }

    pub fn normalize(self) -> Vec3 {
        let l = self.len();
        if l > 1e-8 {
            self / l
        } else {
            self
        }
    }

    pub fn reflect(self, n: Vec3) -> Vec3 {
        self - n * (2.0 * self.dot(n))
    }

    pub fn refract(self, n: Vec3, eta: f32) -> Option<Vec3> {
        let cos_i = (-self.dot(n)).clamp(-1.0, 1.0);
        let sin2_t = eta * eta * (1.0 - cos_i * cos_i);
        if sin2_t > 1.0 {
            return None;
        }
        let cos_t = (1.0 - sin2_t).sqrt();
        Some(self * eta + n * (eta * cos_i - cos_t))
    }

    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self * (1.0 - t) + o * t
    }

    pub fn max_comp(self) -> f32 {
        self.x.max(self.y).max(self.z)
    }

    pub fn to_rgb8(self) -> [u8; 3] {
        let f = |c: f32| {
            let c = if c.is_finite() { c.max(0.0) } else { 0.0 };
            let mapped = c / (1.0 + c); // Reinhard
            let g = mapped.powf(1.0 / 2.2); // gamma
            (g * 255.0 + 0.5).clamp(0.0, 255.0) as u8
        };
        [f(self.x), f(self.y), f(self.z)]
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        v3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        self.x += o.x;
        self.y += o.y;
        self.z += o.z;
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        v3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        v3(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<Vec3> for Vec3 {
    type Output = Vec3;
    fn mul(self, o: Vec3) -> Vec3 {
        v3(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}

impl Div<f32> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f32) -> Vec3 {
        v3(self.x / s, self.y / s, self.z / s)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        v3(-self.x, -self.y, -self.z)
    }
}