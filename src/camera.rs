use crate::vec3::{v3, Vec3};

pub struct Camera {
    pub center: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub fov: f32,
}

impl Camera {
    pub fn new(center: Vec3, dist: f32) -> Camera {
        Camera {
            center,
            yaw: 0.7,
            pitch: 0.45,
            dist,
            fov: 60f32.to_radians(),
        }
    }

    pub fn eye(&self) -> Vec3 {
        let cp = self.pitch.cos();
        v3(
            self.center.x + self.dist * cp * self.yaw.sin(),
            self.center.y + self.dist * self.pitch.sin(),
            self.center.z + self.dist * cp * self.yaw.cos(),
        )
    }

    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let forward = (self.center - self.eye()).normalize();
        let world_up = v3(0.0, 1.0, 0.0);
        let right = forward.cross(world_up).normalize();
        let up = right.cross(forward).normalize();
        (forward, right, up)
    }

    pub fn ray(&self, sx: f32, sy: f32, basis: &(Vec3, Vec3, Vec3)) -> Vec3 {
        let scale = (self.fov * 0.5).tan();
        (basis.0 + basis.1 * (sx * scale) + basis.2 * (sy * scale)).normalize()
    }

    pub fn orbit(&mut self, d_yaw: f32, d_pitch: f32) {
        self.yaw += d_yaw;
        self.pitch = (self.pitch + d_pitch).clamp(-1.35, 1.45);
    }

    pub fn zoom(&mut self, d: f32) {
        self.dist = (self.dist + d).clamp(6.0, 80.0);
    }
}