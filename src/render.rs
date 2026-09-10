//! Motor de raytracing: sombreado, sombras, reflexion, refraccion y paralelismo.

use crate::camera::Camera;
use crate::material::{fresnel_schlick, tangent_frame, Material};
use crate::sky::Sky;
use crate::texture::Texture;
use crate::vec3::Vec3;
use crate::world::World;

/// Profundidad de rebotes para el render final.
pub const MAX_DEPTH: u32 = 3;
const T_MAX: f32 = 1.0e4;
const EPS: f32 = 1.0e-3;

pub struct Light {
    pub pos: Vec3,
    pub color: Vec3,
    pub intensity: f32,
    /// false = luz tipo sol (sin caida por distancia).
    pub attenuate: bool,
}

/// Parametros de calidad del render. Bajarlos da frames rapidos para
/// la vista interactiva; subirlos da la calidad final.
#[derive(Clone, Copy)]
pub struct RenderOpts {
    /// Supersampling NxN por pixel (antialiasing).
    pub samples: usize,
    /// Rebotes maximos de reflexion/refraccion.
    pub max_depth: u32,
    pub threads: usize,
}

impl Default for RenderOpts {
    fn default() -> RenderOpts {
        RenderOpts {
            samples: 2,
            max_depth: MAX_DEPTH,
            threads: 4,
        }
    }
}

/// Niebla encerrada en una caja: un rayo que escapa sin chocar nada se tine de
/// `color` en proporcion a cuanto recorrio DENTRO de la caja.
///
/// Antes esto se hacia por altura evaluada a distancia fija, lo que pintaba un
/// manchon circular detras del diorama en las tomas aereas. Con la caja, solo
/// se tinen los rayos que de verdad atraviesan el hueco del Nether, asi que el
/// interior se ve oscuro y el cielo del overworld queda intacto.
pub struct Fog {
    pub min: Vec3,
    pub max: Vec3,
    /// Cuanto tine cada unidad recorrida dentro de la caja.
    pub density: f32,
    pub color: Vec3,
}

pub struct Scene {
    pub world: World,
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    pub lights: Vec<Light>,
    pub sky: Sky,
    pub ambient: Vec3,
    pub fog: Option<Fog>,
}

impl Scene {
    /// Color de fondo para un rayo que se escapa de la escena.
    fn background(&self, ro: Vec3, rd: Vec3) -> Vec3 {
        let sky = self.sky.sample(rd);
        let f = match &self.fog {
            Some(f) => f,
            None => return sky,
        };

        // Slab test contra la caja de niebla.
        let o = [ro.x, ro.y, ro.z];
        let d = [rd.x, rd.y, rd.z];
        let lo = [f.min.x, f.min.y, f.min.z];
        let hi = [f.max.x, f.max.y, f.max.z];
        let mut t0 = 0.0f32;
        let mut t1 = f32::INFINITY;

        for i in 0..3 {
            if d[i].abs() < 1e-9 {
                if o[i] < lo[i] || o[i] > hi[i] {
                    return sky;
                }
                continue;
            }
            let inv = 1.0 / d[i];
            let mut ta = (lo[i] - o[i]) * inv;
            let mut tb = (hi[i] - o[i]) * inv;
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            t0 = t0.max(ta);
            t1 = t1.min(tb);
            if t0 >= t1 {
                return sky;
            }
        }

        // Cuanto recorrio el rayo dentro de la caja.
        let recorrido = (t1 - t0).max(0.0);
        let k = (recorrido * f.density).clamp(0.0, 1.0);
        sky.lerp(f.color, k)
    }

    fn material(&self, id: u8) -> &Material {
        &self.materials[id as usize]
    }

    fn tex_color(&self, m: &Material, u: f32, v: f32) -> Vec3 {
        match m.texture {
            Some(i) => self.textures[i].sample(u, v) * m.albedo,
            None => m.albedo,
        }
    }

    /// Factor de sombra. Devuelve Vec3::ONE si no hay oclusion; los materiales
    /// transparentes dejan pasar luz tintada en vez de bloquearla del todo.
    fn shadow_factor(&self, origin: Vec3, dir: Vec3, dist: f32) -> Vec3 {
        let mut atten = Vec3::ONE;
        let mut o = origin;
        let mut remaining = dist;

        for _ in 0..3 {
            match self.world.traverse(o, dir, remaining, None) {
                None => break,
                Some(h) => {
                    let m = self.material(h.mat);
                    if m.transparency <= 0.0 {
                        return Vec3::ZERO;
                    }
                    let tint = self.tex_color(m, h.u, h.v);
                    atten = atten * tint * m.transparency;
                    if atten.max_comp() < 0.02 {
                        return Vec3::ZERO;
                    }
                    remaining -= h.t + EPS;
                    if remaining <= 0.0 {
                        break;
                    }
                    o = h.point + dir * EPS;
                }
            }
        }
        atten
    }

    /// Traza un rayo y devuelve el color radiante.
    pub fn trace(
        &self,
        ro: Vec3,
        rd: Vec3,
        depth: u32,
        inside: Option<u8>,
        max_depth: u32,
    ) -> Vec3 {
        let hit = match self.world.traverse(ro, rd, T_MAX, inside) {
            Some(h) => h,
            None => return self.background(ro, rd),
        };

        let m = self.material(hit.mat);
        let base = self.tex_color(m, hit.u, hit.v);

        // --- Mapa normal en espacio tangente -------------------------------
        let mut n = hit.normal;
        if let Some(nm) = m.normal_map {
            let (tan, bitan) = tangent_frame(hit.normal);
            let tn = self.textures[nm].sample_normal(hit.u, hit.v);
            n = (tan * tn.x + bitan * tn.y + hit.normal * tn.z).normalize();
        }

        let view = -rd;
        let mut local = base * self.ambient;

        // --- Iluminacion directa (Blinn-Phong + sombras) --------------------
        for l in &self.lights {
            let to_l = l.pos - hit.point;
            let dist = to_l.len();
            if dist < 1e-4 {
                continue;
            }
            let ldir = to_l / dist;
            if ldir.dot(hit.normal) <= 0.0 {
                continue;
            }
            let falloff = if l.attenuate {
                1.0 / (1.0 + 0.010 * dist * dist)
            } else {
                1.0
            };
            // Si la luz aporta menos que el umbral, ni siquiera se lanza el
            // rayo de sombra (las antorchas lejanas se descartan gratis).
            if l.intensity * falloff * l.color.max_comp() < 0.012 {
                continue;
            }
            let s = self.shadow_factor(hit.point + hit.normal * EPS, ldir, dist - EPS);
            if s.max_comp() <= 0.0 {
                continue;
            }
            let radiance = l.color * (l.intensity * falloff) * s;

            let diff = n.dot(ldir).max(0.0);
            local += base * radiance * (diff * m.kd);

            let half = (ldir + view).normalize();
            let spec = n.dot(half).max(0.0).powf(m.shininess);
            local += radiance * (spec * m.ks);
        }

        // --- Emision (material emisivo) ------------------------------------
        local += m.emission;

        if depth >= max_depth {
            return local;
        }

        // --- Reflexion y refraccion con Fresnel ----------------------------
        let mut kr = m.reflectivity;
        let mut kt = 0.0;

        if m.transparency > 0.0 {
            let cos_i = view.dot(n).abs();
            let f = fresnel_schlick(cos_i, m.ior);
            kt = m.transparency * (1.0 - f);
            kr = (m.reflectivity + m.transparency * f).min(1.0);
        }

        let mut color = local * (1.0 - kr - kt).max(0.0);

        if kr > 0.01 {
            let rdir = rd.reflect(n).normalize();
            color += self.trace(hit.point + n * EPS, rdir, depth + 1, None, max_depth) * kr;
        }

        if kt > 0.01 {
            // El rayo entra al medio: eta = 1 / ior. Al viajar dentro se ignora
            // ese material para no refractar en cada celda vecina del mismo bloque.
            match rd.refract(n, 1.0 / m.ior) {
                Some(tdir) => {
                    let tdir = tdir.normalize();
                    color += self.trace(
                        hit.point + tdir * EPS,
                        tdir,
                        depth + 1,
                        Some(hit.mat),
                        max_depth,
                    ) * kt;
                }
                None => {
                    // Reflexion interna total.
                    let rdir = rd.reflect(n).normalize();
                    color +=
                        self.trace(hit.point + n * EPS, rdir, depth + 1, None, max_depth) * kt;
                }
            }
        }

        color
    }
}

/// Render paralelo con hilos de la biblioteca estandar (sin rayon).
///
/// El framebuffer se parte en franjas chicas y los hilos las van tomando de una
/// cola conforme terminan. Con franjas fijas por hilo, el que recibia puro cielo
/// terminaba enseguida y el que recibia el interior del Nether cargaba con todo;
/// asi el reparto se equilibra solo.
pub fn render_parallel(
    scene: &Scene,
    cam: &Camera,
    w: usize,
    h: usize,
    opts: RenderOpts,
) -> Vec<Vec3> {
    let mut buf = vec![Vec3::ZERO; w * h];
    let threads = opts.threads.max(1);
    // Muchas mas franjas que hilos: eso es lo que permite balancear.
    let rows_per_chunk = 4usize;
    let aspect = w as f32 / h as f32;
    let eye = cam.eye();
    let basis = cam.basis();
    let ss = opts.samples.max(1);
    let inv_ss = 1.0 / (ss * ss) as f32;
    let max_depth = opts.max_depth;

    // Bloque propio: la cola debe soltarse antes de devolver `buf`.
    {
    let tiles: Vec<(usize, &mut [Vec3])> =
        buf.chunks_mut(rows_per_chunk * w).enumerate().collect();
    let queue = std::sync::Mutex::new(tiles.into_iter());

    std::thread::scope(|s| {
        for _ in 0..threads {
            let queue = &queue;
            s.spawn(move || loop {
                // Tomar la siguiente franja libre.
                let next = queue.lock().map(|mut q| q.next()).unwrap_or(None);
                let (chunk_idx, chunk) = match next {
                    Some(t) => t,
                    None => break,
                };
                let y0 = chunk_idx * rows_per_chunk;
                for (i, px) in chunk.iter_mut().enumerate() {
                    let x = i % w;
                    let y = y0 + i / w;
                    let mut acc = Vec3::ZERO;
                    // Supersampling en rejilla ss x ss (antialiasing).
                    for sy in 0..ss {
                        for sx in 0..ss {
                            let ox = (sx as f32 + 0.5) / ss as f32;
                            let oy = (sy as f32 + 0.5) / ss as f32;
                            let ndc_x = (2.0 * (x as f32 + ox) / w as f32 - 1.0) * aspect;
                            let ndc_y = 1.0 - 2.0 * (y as f32 + oy) / h as f32;
                            let dir = cam.ray(ndc_x, ndc_y, &basis);
                            acc += scene.trace(eye, dir, 0, None, max_depth);
                        }
                    }
                    *px = acc * inv_ss;
                }
            });
        }
    });
    }

    buf
}