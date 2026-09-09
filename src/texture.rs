use crate::noise::{fbm, hash21, value_noise};
use crate::vec3::{v3, Vec3};
use std::fs;
use std::io;

pub struct Texture {
    pub w: usize,
    pub h: usize,
    pub data: Vec<Vec3>,
}

impl Texture {
    pub fn new(w: usize, h: usize) -> Texture {
        Texture {
            w,
            h,
            data: vec![Vec3::ZERO; w * h],
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> Vec3 {
        self.data[y * self.w + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, c: Vec3) {
        self.data[y * self.w + x] = c;
    }

    #[inline]
    pub fn sample(&self, u: f32, v: f32) -> Vec3 {
        let uu = u - u.floor();
        let vv = v - v.floor();
        let x = ((uu * self.w as f32) as usize).min(self.w - 1);
        let y = ((vv * self.h as f32) as usize).min(self.h - 1);
        self.get(x, y)
    }

    #[inline]
    pub fn sample_normal(&self, u: f32, v: f32) -> Vec3 {
        let c = self.sample(u, v);
        v3(c.x * 2.0 - 1.0, c.y * 2.0 - 1.0, c.z * 2.0 - 1.0).normalize()
    }

    pub fn from_ppm(path: &str) -> io::Result<Texture> {
        let bytes = fs::read(path)?;
        let mut pos = 0usize;
        let mut token = || -> Option<String> {
            while pos < bytes.len() {
                if bytes[pos].is_ascii_whitespace() {
                    pos += 1;
                } else if bytes[pos] == b'#' {
                    while pos < bytes.len() && bytes[pos] != b'\n' {
                        pos += 1;
                    }
                } else {
                    break;
                }
            }
            let start = pos;
            while pos < bytes.len() && !bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if start == pos {
                None
            } else {
                Some(String::from_utf8_lossy(&bytes[start..pos]).to_string())
            }
        };

        let magic = token().unwrap_or_default();
        let w: usize = token().unwrap_or_default().parse().unwrap_or(0);
        let h: usize = token().unwrap_or_default().parse().unwrap_or(0);
        let maxv: f32 = token().unwrap_or_default().parse().unwrap_or(255.0);
        drop(token);

        if magic != "P6" || w == 0 || h == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Solo se soporta PPM binario P6",
            ));
        }

        let start = pos + 1;
        let mut tex = Texture::new(w, h);
        for i in 0..w * h {
            let o = start + i * 3;
            if o + 2 >= bytes.len() {
                break;
            }
            tex.data[i] = v3(
                bytes[o] as f32 / maxv,
                bytes[o + 1] as f32 / maxv,
                bytes[o + 2] as f32 / maxv,
            );
        }
        Ok(tex)
    }
}

pub fn normal_from_height(height: &Texture, strength: f32) -> Texture {
    let mut out = Texture::new(height.w, height.h);
    let at = |x: i32, y: i32| -> f32 {
        let xx = x.rem_euclid(height.w as i32) as usize;
        let yy = y.rem_euclid(height.h as i32) as usize;
        height.get(xx, yy).x
    };
    for y in 0..height.h as i32 {
        for x in 0..height.w as i32 {
            let dx = (at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1))
                - (at(x - 1, y - 1) + 2.0 * at(x - 1, y) + at(x - 1, y + 1));
            let dy = (at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1))
                - (at(x - 1, y - 1) + 2.0 * at(x, y - 1) + at(x + 1, y - 1));
            let n = v3(-dx * strength, -dy * strength, 1.0).normalize();
            out.set(
                x as usize,
                y as usize,
                v3(n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5),
            );
        }
    }
    out
}

pub fn tex_grass(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let r = hash21(x as i32, y as i32, 11);
            let f = fbm(x as f32 * 0.25, y as f32 * 0.25, 3, 5);
            let base = v3(0.18, 0.45, 0.12).lerp(v3(0.34, 0.70, 0.22), f);
            let c = base * (0.85 + 0.30 * r);
            t.set(x, y, c);
        }
    }
    t
}

pub fn tex_dirt(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let r = hash21(x as i32, y as i32, 23);
            let c = v3(0.42, 0.28, 0.16) * (0.80 + 0.40 * r);
            t.set(x, y, c);
        }
    }
    t
}

pub fn tex_stone(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let blobs = value_noise(x as f32 * 0.18, y as f32 * 0.18, 77);
            let grain = hash21(x as i32, y as i32, 91);
            let mut g = 0.35 + 0.35 * blobs + 0.15 * grain;
            if blobs > 0.46 && blobs < 0.52 {
                g *= 0.45;
            }
            t.set(x, y, v3(g, g, g * 1.03));
        }
    }
    t
}

pub fn tex_wood(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let plank = (y * 4 / n) as f32; // 4 tablas por cara
            let veta = value_noise(x as f32 * 0.6, y as f32 * 0.15 + plank * 3.0, 41);
            let mut c = v3(0.55, 0.36, 0.19).lerp(v3(0.38, 0.23, 0.11), veta);
            // linea de separacion entre tablas
            if (y * 4) % n < 2 {
                c = c * 0.55;
            }
            t.set(x, y, c);
        }
    }
    t
}

pub fn tex_water(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let f = fbm(x as f32 * 0.2, y as f32 * 0.2, 3, 17);
            t.set(x, y, v3(0.62, 0.84, 0.95).lerp(v3(0.78, 0.93, 1.0), f));
        }
    }
    t
}

pub fn tex_lava(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let f = fbm(x as f32 * 0.22, y as f32 * 0.22, 4, 3);
            let hot = (f * 1.6).min(1.0);
            let c = v3(0.55, 0.06, 0.0).lerp(v3(1.0, 0.75, 0.15), hot);
            t.set(x, y, c);
        }
    }
    t
}

pub fn tex_obsidian(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let f = fbm(x as f32 * 0.3, y as f32 * 0.3, 3, 61);
            let c = v3(0.05, 0.03, 0.09).lerp(v3(0.16, 0.10, 0.26), f);
            t.set(x, y, c);
        }
    }
    t
}

pub fn height_waves(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let u = x as f32 / n as f32;
            let v = y as f32 / n as f32;
            let h = 0.5
                + 0.25 * (u * std::f32::consts::TAU * 2.0).sin()
                + 0.25 * (v * std::f32::consts::TAU * 3.0).sin();
            t.set(x, y, v3(h, h, h));
        }
    }
    t
}