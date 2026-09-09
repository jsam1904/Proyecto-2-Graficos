//! Skybox. Dos modos: procedural (gradiente + sol + nubes) o panorama
//! equirectangular cargado desde un PPM.

use crate::noise::fbm;
use crate::texture::Texture;
use crate::vec3::Vec3;

pub enum Sky {
    Procedural {
        zenith: Vec3,
        horizon: Vec3,
        ground: Vec3,
        sun_dir: Vec3,
        sun_color: Vec3,
    },
    Panorama(Texture),
}

impl Sky {
    /// Color del cielo en la direccion `d` (normalizada).
    pub fn sample(&self, d: Vec3) -> Vec3 {
        match self {
            Sky::Procedural {
                zenith,
                horizon,
                ground,
                sun_dir,
                sun_color,
            } => {
                let t = d.y;
                let mut c = if t >= 0.0 {
                    horizon.lerp(*zenith, t.powf(0.6))
                } else {
                    horizon.lerp(*ground, (-t).powf(0.75))
                };

                // Nubes: fBm proyectado sobre el hemisferio superior.
                if t > 0.02 {
                    let s = 1.0 / t.max(0.05);
                    let n = fbm(d.x * s * 0.45 + 12.0, d.z * s * 0.45 + 4.0, 4, 909);
                    let cloud = (((n - 0.52) * 4.0).clamp(0.0, 1.0)) * (t * 2.2).min(1.0);
                    c = c.lerp(Vec3::splat(1.05), cloud * 0.75);
                }

                // Disco solar + halo.
                let cos_sun = d.dot(*sun_dir).max(0.0);
                c += *sun_color * cos_sun.powf(900.0) * 12.0;
                c += *sun_color * cos_sun.powf(24.0) * 0.25;
                c
            }
            Sky::Panorama(tex) => {
                let u = 0.5 + d.z.atan2(d.x) / std::f32::consts::TAU;
                let v = (d.y.clamp(-1.0, 1.0)).acos() / std::f32::consts::PI;
                tex.sample(u, v)
            }
        }
    }
}