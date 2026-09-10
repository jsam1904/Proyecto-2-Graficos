//! Texturas: generacion procedural, carga de PPM (P6) y mapas normales.
//! No se usan librerias externas: el decodificador de PPM es propio.

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

    /// Muestreo "nearest" con repeticion (look de pixel-art tipo Minecraft).
    #[inline]
    pub fn sample(&self, u: f32, v: f32) -> Vec3 {
        let uu = u - u.floor();
        let vv = v - v.floor();
        let x = ((uu * self.w as f32) as usize).min(self.w - 1);
        let y = ((vv * self.h as f32) as usize).min(self.h - 1);
        self.get(x, y)
    }

    /// Lee un texel de un mapa normal y lo pasa de [0,1] a [-1,1] (espacio tangente).
    #[inline]
    pub fn sample_normal(&self, u: f32, v: f32) -> Vec3 {
        let c = self.sample(u, v);
        v3(c.x * 2.0 - 1.0, c.y * 2.0 - 1.0, c.z * 2.0 - 1.0).normalize()
    }

    /// Carga una imagen PPM binaria (P6, 8 bits). Convierte cualquier PNG/JPG con:
    ///   ffmpeg -i textura.png -pix_fmt rgb24 textura.ppm
    pub fn from_ppm(path: &str) -> io::Result<Texture> {
        let bytes = fs::read(path)?;
        let mut pos = 0usize;

        // Lee un token del encabezado saltando espacios y comentarios (#).
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

        let start = pos + 1; // un unico separador despues del maxval
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

/// Deriva un mapa normal a partir de una textura de altura (usa el canal rojo, filtro Sobel).
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

// ---------------------------------------------------------------------------
// Texturas procedurales (una por material). Todas de 32x32 por defecto.
// ---------------------------------------------------------------------------

pub fn tex_grass(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            // Manchas coherentes (no ruido por pixel) + un grano muy suave.
            let f = fbm(x as f32 * 0.22, y as f32 * 0.22, 3, 5);
            let grano = hash21(x as i32, y as i32, 11) * 0.06;
            let base = v3(0.20, 0.44, 0.15).lerp(v3(0.33, 0.63, 0.22), f);
            t.set(x, y, base * (0.97 + grano));
        }
    }
    t
}

pub fn tex_dirt(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let f = fbm(x as f32 * 0.28, y as f32 * 0.28, 3, 23);
            let grano = hash21(x as i32, y as i32, 29) * 0.08;
            let base = v3(0.36, 0.24, 0.14).lerp(v3(0.48, 0.33, 0.19), f);
            t.set(x, y, base * (0.96 + grano));
        }
    }
    t
}

/// Piedra tipo "cobble": manchas grandes con juntas oscuras.
pub fn tex_stone(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let blobs = value_noise(x as f32 * 0.30, y as f32 * 0.30, 77);
            let grano = hash21(x as i32, y as i32, 91) * 0.05;
            let mut g = 0.42 + 0.28 * blobs + grano;
            // Junta oscura entre piedras (banda estrecha del ruido).
            if (blobs - 0.5).abs() < 0.045 {
                g *= 0.55;
            }
            t.set(x, y, v3(g, g, g * 1.04));
        }
    }
    t
}

pub fn tex_wood(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let plank = (y * 4 / n) as f32; // 4 tablas por cara
            let veta = value_noise(x as f32 * 0.5, y as f32 * 0.12 + plank * 3.0, 41);
            let mut c = v3(0.55, 0.36, 0.19).lerp(v3(0.40, 0.25, 0.12), veta);
            if (y * 4) % n < 2 {
                c = c * 0.6; // linea de separacion entre tablas
            }
            t.set(x, y, c);
        }
    }
    t
}

/// Agua/hielo: casi uniforme, el color real lo dan la refraccion y el reflejo.
pub fn tex_water(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let f = fbm(x as f32 * 0.16, y as f32 * 0.16, 2, 17);
            t.set(x, y, v3(0.68, 0.86, 0.95).lerp(v3(0.80, 0.93, 1.0), f));
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
            // Base casi negra con vetas moradas y algunas facetas mas claras.
            let veta = fbm(x as f32 * 0.35, y as f32 * 0.35, 4, 61);
            let faceta = value_noise(x as f32 * 0.55, y as f32 * 0.55, 67);
            let mut c = v3(0.020, 0.014, 0.040).lerp(v3(0.16, 0.07, 0.30), veta.powf(2.2));
            if faceta > 0.72 {
                c = c + v3(0.05, 0.02, 0.09); // aristas cristalinas
            }
            let grano = hash21(x as i32, y as i32, 73) * 0.05;
            t.set(x, y, c * (0.95 + grano));
        }
    }
    t
}

/// Portal del Nether: remolinos morados brillantes.
pub fn tex_portal(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let u = x as f32 / n as f32 - 0.5;
            let v = y as f32 / n as f32 - 0.5;
            // Coordenadas polares: da un remolino en vez de manchas sueltas.
            let r = (u * u + v * v).sqrt();
            let a = v.atan2(u);
            let f = fbm(a * 2.4 + r * 6.0, r * 9.0, 3, 313);
            let mut c = v3(0.30, 0.05, 0.52).lerp(v3(0.72, 0.42, 1.0), f);
            if f > 0.62 {
                c = c + v3(0.18, 0.10, 0.28); // filamentos brillantes
            }
            t.set(x, y, c);
        }
    }
    t
}

/// Mapa de altura SUAVE. Los mapas normales se derivan de aqui, nunca de una
/// textura con ruido por pixel: si no, cada texel apunta a otro lado y la
/// superficie "hierve" al mover la camara.
pub fn height_blobs(n: usize, freq: f32, seed: u32) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let h = fbm(x as f32 * freq, y as f32 * freq, 3, seed);
            t.set(x, y, v3(h, h, h));
        }
    }
    t
}

/// Mapa de altura de las tablas de madera (solo las juntas marcan relieve).
pub fn height_planks(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let junta = if (y * 4) % n < 2 { 0.0 } else { 1.0 };
            let veta = value_noise(x as f32 * 0.5, y as f32 * 0.12, 41) * 0.15;
            let h = junta * 0.85 + veta;
            t.set(x, y, v3(h, h, h));
        }
    }
    t
}

/// Mapa de altura de olas para el agua.
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

/// Netherrack: rojo oscuro con grumos.
pub fn tex_netherrack(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let f = fbm(x as f32 * 0.30, y as f32 * 0.30, 3, 131);
            let grano = hash21(x as i32, y as i32, 137) * 0.07;
            let mut c = v3(0.32, 0.07, 0.07).lerp(v3(0.55, 0.16, 0.13), f);
            // Vetas mas oscuras.
            if f < 0.32 {
                c = c * 0.65;
            }
            t.set(x, y, c * (0.96 + grano));
        }
    }
    t
}

/// Glowstone: amarillo caliente con nodulos brillantes.
pub fn tex_glowstone(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let f = fbm(x as f32 * 0.35, y as f32 * 0.35, 3, 211);
            let c = v3(0.60, 0.40, 0.12).lerp(v3(1.0, 0.86, 0.42), f);
            t.set(x, y, c);
        }
    }
    t
}

/// Hojas de arbol: verde oscuro con huecos, distinto del pasto del suelo.
pub fn tex_leaves(n: usize) -> Texture {
    let mut t = Texture::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let f = fbm(x as f32 * 0.45, y as f32 * 0.45, 3, 401);
            let mut c = v3(0.06, 0.22, 0.07).lerp(v3(0.16, 0.42, 0.14), f);
            // Huecos entre el follaje.
            if f < 0.34 {
                c = c * 0.45;
            }
            let grano = hash21(x as i32, y as i32, 409) * 0.10;
            t.set(x, y, c * (0.94 + grano));
        }
    }
    t
}