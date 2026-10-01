//! Motor de raytracing: sombreado, sombras, reflexion, refraccion y paralelismo.

use crate::camera::Camera;
use crate::material::{fresnel_schlick, tangent_frame, Material};
use crate::noise::hash31;
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
    /// Zona (bioma) que ilumina. `None` = toda la escena. Cada dimension tiene
    /// su propia luz: el sol no entra a la cueva, ni la lava del Nether pinta
    /// la isla. De paso, los puntos fuera de la zona no lanzan rayo de sombra.
    pub zone: Option<Zone>,
}

/// Caja alineada a los ejes que delimita un bioma del diorama.
#[derive(Clone, Copy)]
pub struct Zone {
    pub min: Vec3,
    pub max: Vec3,
}

impl Zone {
    pub fn new(min: Vec3, max: Vec3) -> Zone {
        Zone { min, max }
    }

    /// Con un margen chico: los puntos de impacto caen justo sobre las caras
    /// de los cubos, que coinciden con los bordes de la zona.
    #[inline]
    pub fn contains(&self, p: Vec3) -> bool {
        const M: f32 = 0.01;
        p.x >= self.min.x - M
            && p.x <= self.max.x + M
            && p.y >= self.min.y - M
            && p.y <= self.max.y + M
            && p.z >= self.min.z - M
            && p.z <= self.max.z + M
    }
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
/// se tinen los rayos que de verdad atraviesan una dimension cerrada (la cueva,
/// el Nether o el End), asi que su interior se ve oscuro y el cielo del
/// overworld queda intacto.
pub struct Fog {
    pub min: Vec3,
    pub max: Vec3,
    /// Cuanto tine cada unidad recorrida dentro de la caja.
    pub density: f32,
    pub color: Vec3,
    /// Densidad de estrellas sobre el color de la niebla (0 = sin estrellas).
    /// La usa el End para simular su cielo negro.
    pub stars: f32,
}

pub struct Scene {
    pub world: World,
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    pub lights: Vec<Light>,
    pub sky: Sky,
    /// Luz ambiente por defecto (la del overworld).
    pub ambient: Vec3,
    /// Luz ambiente propia de cada bioma; gana la primera zona que contenga
    /// el punto. Fuera de todas se usa `ambient`.
    pub ambient_zones: Vec<(Zone, Vec3)>,
    /// Una caja de niebla por dimension cerrada (cueva, Nether, End).
    pub fogs: Vec<Fog>,
}

impl Scene {
    /// Color de fondo para un rayo que se escapa de la escena.
    fn background(&self, ro: Vec3, rd: Vec3) -> Vec3 {
        let mut c = self.sky.sample(rd);
        for f in &self.fogs {
            let recorrido = box_path(f, ro, rd);
            if recorrido <= 0.0 {
                continue;
            }
            let k = (recorrido * f.density).clamp(0.0, 1.0);
            let mut tint = f.color;
            if f.stars > 0.0 {
                tint += Vec3::splat(star(rd, f.stars));
            }
            c = c.lerp(tint, k);
        }
        c
    }

    fn ambient_at(&self, p: Vec3) -> Vec3 {
        self.ambient_zones
            .iter()
            .find(|(z, _)| z.contains(p))
            .map_or(self.ambient, |(_, a)| *a)
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
    ///
    /// Los bloques emisivos no hacen sombra: son la fuente de luz. Las luces
    /// puntuales de antorchas y portales viven DENTRO de su bloque, y si ese
    /// bloque bloqueara el rayo nunca iluminarian nada.
    fn shadow_factor(&self, origin: Vec3, dir: Vec3, dist: f32) -> Vec3 {
        let mut atten = Vec3::ONE;
        let mut o = origin;
        let mut remaining = dist;
        // Material que se esta atravesando. Sin esto, el siguiente recorrido
        // arranca dentro de la misma celda y la vuelve a chocar en t = 0.
        let mut medium = None;

        for _ in 0..4 {
            let h = match self.world.traverse(o, dir, remaining, medium) {
                Some(h) => h,
                None => return atten,
            };
            let m = self.material(h.mat);
            if !m.is_emissive() {
                if m.transparency <= 0.0 {
                    return Vec3::ZERO;
                }
                let tint = self.tex_color(m, h.u, h.v);
                atten = atten * tint * m.transparency;
                if atten.max_comp() < 0.02 {
                    return Vec3::ZERO;
                }
            }
            remaining -= h.t + EPS;
            if remaining <= 0.0 {
                return atten;
            }
            o = h.point + dir * EPS;
            medium = Some(h.mat);
        }
        // Demasiados medios seguidos: se asume oclusion.
        Vec3::ZERO
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
        let mut local = base * self.ambient_at(hit.point);

        // --- Iluminacion directa (Blinn-Phong + sombras) --------------------
        for l in &self.lights {
            if let Some(z) = &l.zone {
                if !z.contains(hit.point) {
                    continue;
                }
            }
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
        // Modulada por la textura: si se sumara un color plano, el tone mapping
        // lo lavaba a un beige uniforme y la lava perdia todo su detalle.
        local += m.emission * base;

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

/// Distancia que recorre el rayo dentro de la caja de niebla (slab test).
fn box_path(f: &Fog, ro: Vec3, rd: Vec3) -> f32 {
    let o = [ro.x, ro.y, ro.z];
    let d = [rd.x, rd.y, rd.z];
    let lo = [f.min.x, f.min.y, f.min.z];
    let hi = [f.max.x, f.max.y, f.max.z];
    let mut t0 = 0.0f32;
    let mut t1 = f32::INFINITY;

    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if o[i] < lo[i] || o[i] > hi[i] {
                return 0.0;
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
            return 0.0;
        }
    }
    (t1 - t0).max(0.0)
}

/// Estrellas: la direccion se cuantiza en celdas y unas pocas se encienden.
fn star(rd: Vec3, density: f32) -> f32 {
    let q = 700.0;
    let h = hash31(
        (rd.x * q).floor() as i32,
        (rd.y * q).floor() as i32,
        (rd.z * q).floor() as i32,
        4242,
    );
    if h > 1.0 - density {
        0.35 + 0.65 * (h - (1.0 - density)) / density
    } else {
        0.0
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