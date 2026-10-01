//! Construccion del diorama: tres dimensiones de Minecraft una al lado de otra.
//!   - al centro, la isla del overworld (terreno procedural 16x16, lago,
//!     cabana, portal, antorchas y arboles)
//!   - debajo de la isla, una cueva del Deep Dark (pizarra profunda, sculk,
//!     sensores y un fragmento de ciudad antigua)
//!   - a la derecha (+x), un trozo del Nether (lago de lava, cascada de lava,
//!     glowstone colgando, bosque carmesi y portal)
//!   - a la izquierda (-x), el End (isla flotante de piedra del End, pilares de
//!     obsidiana con cristales, plantas de chorus y una torre de purpur)
//!
//! "Derecha" e "izquierda" son vistas desde el frente (+z), que es hacia donde
//! mira la camara por defecto: en esa vista +x queda a la derecha de pantalla.
//! Cada dimension tiene su propia luz y su propia luz ambiente (ver `Zone`).

use crate::material::Material;
use crate::noise::fbm;
use crate::render::{Fog, Light, Scene, Zone};
use crate::sky::Sky;
use crate::texture::{self, Texture};
use crate::vec3::{v3, Vec3};
use crate::world::World;

// Identificadores de material = valor guardado en cada voxel.
pub const M_GRASS: u8 = 0;
pub const M_DIRT: u8 = 1;
pub const M_STONE: u8 = 2;
pub const M_WOOD: u8 = 3;
pub const M_WATER: u8 = 4;
pub const M_LAVA: u8 = 5;
pub const M_OBSIDIAN: u8 = 6;
pub const M_GLASS: u8 = 7;
pub const M_NETHERRACK: u8 = 8;
pub const M_GLOWSTONE: u8 = 9;
pub const M_PORTAL: u8 = 10;
pub const M_TORCH: u8 = 11;
pub const M_LEAVES: u8 = 12;
pub const M_DEEPSLATE: u8 = 13;
pub const M_SCULK: u8 = 14;
pub const M_SCULK_SENSOR: u8 = 15;
pub const M_REINFORCED: u8 = 16;
pub const M_DEEPSLATE_BRICKS: u8 = 17;
pub const M_SOUL_LANTERN: u8 = 18;
pub const M_END_STONE: u8 = 19;
pub const M_PURPUR: u8 = 20;
pub const M_CHORUS: u8 = 21;
pub const M_END_CRYSTAL: u8 = 22;
pub const M_END_ROD: u8 = 23;
pub const M_NYLIUM: u8 = 24;
pub const M_CRIMSON_STEM: u8 = 25;
pub const M_WART: u8 = 26;
pub const M_SHROOMLIGHT: u8 = 27;

pub const SIZE: i32 = 16; // cada bioma ocupa 16x16 cubos de planta
pub const CAVE_ROOF: i32 = 7; // capa de piedra entre la cueva y la isla
pub const GROUND_BASE: i32 = 8; // primera capa del overworld
/// Nivel del mar. Con el ruido reescalado (ver abajo) las alturas van de
/// GROUND_BASE+1 a GROUND_BASE+8, asi que este nivel inunda solo el ~18% mas
/// bajo del terreno: un lago, no un oceano que tape todo el relieve.
pub const SEA_LEVEL: i32 = GROUND_BASE + 3;

/// Separacion (en cubos) entre la isla y los biomas de los lados.
pub const GAP: i32 = 3;
/// Primera columna x del Nether (a la derecha de la isla).
pub const NETHER_X: i32 = SIZE + GAP;
/// Primera columna x del End (a la izquierda de la isla).
pub const END_X: i32 = -SIZE - GAP;

/// Nivel del lago de lava del Nether.
const LAVA_LEVEL: i32 = 2;

/// Punto al que mira la camara. Los tres biomas van de x = -19 a x = 35, asi
/// que el centro en x sigue siendo el de la isla; en altura el conjunto va de
/// y = 0 (piso de la cueva) a y ~ 21 (copa de los arboles y pilares del End).
pub fn center() -> Vec3 {
    v3(SIZE as f32 * 0.5, 9.0, SIZE as f32 * 0.5)
}

pub fn build(seed: u32) -> Scene {
    // ---------------- Texturas ----------------
    let mut textures: Vec<Texture> = Vec::new();
    let push = |t: Texture, textures: &mut Vec<Texture>| -> usize {
        textures.push(t);
        textures.len() - 1
    };

    let t_grass = push(texture::tex_grass(32), &mut textures);
    let t_dirt = push(texture::tex_dirt(32), &mut textures);
    let t_stone = push(texture::tex_stone(32), &mut textures);
    let t_wood = push(texture::tex_wood(32), &mut textures);
    let t_water = push(texture::tex_water(32), &mut textures);
    let t_lava = push(texture::tex_lava(32), &mut textures);
    let t_obs = push(texture::tex_obsidian(32), &mut textures);
    let t_glass = push(texture::tex_glass(16), &mut textures);
    let t_nether = push(texture::tex_netherrack(32), &mut textures);
    let t_glow = push(texture::tex_glowstone(32), &mut textures);
    let t_portal = push(texture::tex_portal(32), &mut textures);
    let t_torch = push(texture::tex_torch(16), &mut textures);
    let t_leaves = push(texture::tex_leaves(32), &mut textures);
    let t_deep = push(texture::tex_deepslate(32), &mut textures);
    let t_sculk = push(texture::tex_sculk(32), &mut textures);
    let t_sensor = push(texture::tex_sculk_sensor(32), &mut textures);
    let t_reinf = push(texture::tex_reinforced_deepslate(32), &mut textures);
    let t_dbricks = push(texture::tex_deepslate_bricks(32), &mut textures);
    let t_soul = push(texture::tex_soul_lantern(16), &mut textures);
    let t_end = push(texture::tex_end_stone(32), &mut textures);
    let t_purpur = push(texture::tex_purpur(32), &mut textures);
    let t_chorus = push(texture::tex_chorus(32), &mut textures);
    let t_crystal = push(texture::tex_end_crystal(32), &mut textures);
    let t_rod = push(texture::tex_end_rod(16), &mut textures);
    let t_nylium = push(texture::tex_crimson_nylium(32), &mut textures);
    let t_stem = push(texture::tex_crimson_stem(32), &mut textures);
    let t_wart = push(texture::tex_wart_block(32), &mut textures);
    let t_shroom = push(texture::tex_shroomlight(32), &mut textures);

    // Mapas normales derivados de mapas de altura SUAVES (filtro Sobel propio).
    let n_stone = texture::normal_from_height(&texture::height_blobs(32, 0.30, 77), 0.9);
    let n_stone = push(n_stone, &mut textures);
    let n_wood = texture::normal_from_height(&texture::height_planks(32), 0.7);
    let n_wood = push(n_wood, &mut textures);
    let n_water = texture::normal_from_height(&texture::height_waves(32), 0.45);
    let n_water = push(n_water, &mut textures);
    let n_nether = texture::normal_from_height(&texture::height_blobs(32, 0.30, 131), 1.1);
    let n_nether = push(n_nether, &mut textures);
    let n_obs = texture::normal_from_height(&texture::height_blobs(32, 0.55, 67), 0.6);
    let n_obs = push(n_obs, &mut textures);
    let n_deep = texture::normal_from_height(&texture::height_blobs(32, 0.25, 607), 1.0);
    let n_deep = push(n_deep, &mut textures);
    let n_bricks = texture::normal_from_height(&texture::height_bricks(32), 0.8);
    let n_bricks = push(n_bricks, &mut textures);
    let n_end = texture::normal_from_height(&texture::height_blobs(32, 0.28, 801), 0.8);
    let n_end = push(n_end, &mut textures);

    // ---------------- Materiales ----------------
    // El indice DEBE coincidir con las constantes M_*.
    // Cada material tiene su textura y sus propios albedo, especular (ks y
    // shininess), transparencia y reflectividad. Los que no se indican quedan en
    // 0 (opaco y mate), que tambien es un valor elegido: la tabla completa esta
    // en el README.
    let materials = vec![
        // 0 - pasto
        Material::opaque("pasto", t_grass)
            .with_albedo(v3(0.96, 1.0, 0.92))
            .with_phong(0.95, 0.05, 8.0),
        // 1 - tierra
        Material::opaque("tierra", t_dirt)
            .with_albedo(v3(1.0, 0.96, 0.93))
            .with_phong(0.95, 0.03, 4.0),
        // 2 - piedra (mapa normal + reflexion leve)
        Material::opaque("piedra", t_stone)
            .with_albedo(v3(0.96, 0.97, 1.0))
            .with_phong(0.80, 0.20, 32.0)
            .with_normal_map(n_stone)
            .with_reflectivity(0.04),
        // 3 - madera (mapa normal)
        Material::opaque("madera", t_wood)
            .with_albedo(v3(1.0, 0.95, 0.88))
            .with_phong(0.85, 0.15, 24.0)
            .with_normal_map(n_wood),
        // 4 - agua (refraccion + reflexion + mapa normal de olas)
        Material::opaque("agua", t_water)
            .with_phong(0.22, 0.45, 90.0)
            .with_normal_map(n_water)
            .with_refraction(0.88, 1.33)
            .with_reflectivity(0.10)
            .with_albedo(v3(0.75, 0.92, 1.0)),
        // 5 - lava (emisivo)
        // La emision se multiplica por la textura en el shader, asi que el
        // color final va de rojo oscuro (grietas) a naranja brillante.
        Material::opaque("lava", t_lava)
            .with_albedo(v3(1.0, 0.85, 0.75))
            .with_phong(0.30, 0.05, 8.0)
            .with_emission(v3(1.0, 0.55, 0.25), 2.2),
        // 6 - obsidiana: roca volcanica negra. Especular alto y estrecho para
        // que se vea vidriada, pero con reflexion moderada: con 0.65 actuaba
        // como espejo y la textura desaparecia.
        Material::opaque("obsidiana", t_obs)
            .with_albedo(v3(0.95, 0.90, 1.0))
            .with_phong(0.75, 0.35, 150.0)
            .with_normal_map(n_obs)
            .with_reflectivity(0.18),
        // 7 - vidrio (refraccion fuerte)
        Material::opaque("vidrio", t_glass)
            .with_phong(0.05, 0.60, 200.0)
            .with_refraction(0.92, 1.52)
            .with_reflectivity(0.08)
            .with_albedo(v3(0.88, 0.96, 0.92)),
        // 8 - netherrack (mapa normal, superficie rugosa y mate)
        Material::opaque("netherrack", t_nether)
            .with_albedo(v3(1.0, 0.92, 0.90))
            .with_phong(0.70, 0.05, 10.0)
            .with_normal_map(n_nether),
        // 9 - glowstone (emisivo)
        Material::opaque("glowstone", t_glow)
            .with_albedo(v3(1.0, 0.95, 0.85))
            .with_phong(0.40, 0.10, 16.0)
            .with_emission(v3(1.0, 0.85, 0.55), 1.6),
        // 10 - portal: emisivo morado y semitransparente a la vez
        Material::opaque("portal", t_portal)
            .with_albedo(v3(0.92, 0.85, 1.0))
            .with_phong(0.20, 0.30, 60.0)
            .with_refraction(0.45, 1.10)
            .with_reflectivity(0.05)
            .with_emission(v3(0.55, 0.18, 0.95), 1.6),
        // 11 - llama de antorcha: emision baja para que no se queme a blanco
        Material::opaque("llama", t_torch)
            .with_albedo(v3(1.0, 0.95, 0.85))
            .with_phong(0.30, 0.10, 12.0)
            .with_emission(v3(1.0, 0.80, 0.55), 1.8),
        // 12 - hojas: material propio, no el pasto del suelo
        Material::opaque("hojas", t_leaves)
            .with_albedo(v3(0.92, 1.0, 0.90))
            .with_phong(0.92, 0.08, 12.0),
        // 13 - pizarra profunda (mapa normal)
        Material::opaque("pizarra profunda", t_deep)
            .with_albedo(v3(0.95, 0.97, 1.0))
            .with_phong(0.80, 0.15, 24.0)
            .with_normal_map(n_deep),
        // 14 - sculk: casi negro y algo humedo (especular alto y estrecho)
        Material::opaque("sculk", t_sculk)
            .with_albedo(v3(0.95, 1.0, 1.0))
            .with_phong(0.60, 0.35, 48.0)
            .with_reflectivity(0.03),
        // 15 - sensor de sculk: los tentaculos cian brillan (emisivo)
        Material::opaque("sensor de sculk", t_sensor)
            .with_albedo(v3(0.95, 1.0, 1.0))
            .with_phong(0.50, 0.25, 32.0)
            .with_emission(v3(0.30, 0.90, 1.0), 1.2),
        // 16 - pizarra reforzada: el marco de la ciudad antigua
        Material::opaque("pizarra reforzada", t_reinf)
            .with_albedo(v3(1.0, 1.0, 1.0))
            .with_phong(0.80, 0.25, 40.0)
            .with_normal_map(n_obs),
        // 17 - ladrillos de pizarra profunda (mapa normal de juntas)
        Material::opaque("ladrillo de pizarra", t_dbricks)
            .with_albedo(v3(0.95, 0.97, 1.0))
            .with_phong(0.80, 0.15, 24.0)
            .with_normal_map(n_bricks),
        // 18 - linterna de almas: llama cian (emisivo)
        Material::opaque("linterna de almas", t_soul)
            .with_albedo(v3(1.0, 1.0, 1.0))
            .with_phong(0.30, 0.20, 32.0)
            .with_emission(v3(0.40, 0.95, 1.0), 1.8),
        // 19 - piedra del End (mapa normal)
        Material::opaque("piedra del End", t_end)
            .with_albedo(v3(1.0, 1.0, 0.95))
            .with_phong(0.90, 0.05, 10.0)
            .with_normal_map(n_end),
        // 20 - purpur (mapa normal de juntas)
        Material::opaque("purpur", t_purpur)
            .with_albedo(v3(1.0, 0.96, 1.0))
            .with_phong(0.85, 0.15, 30.0)
            .with_normal_map(n_bricks),
        // 21 - planta de chorus
        Material::opaque("chorus", t_chorus)
            .with_albedo(v3(1.0, 0.95, 1.0))
            .with_phong(0.90, 0.08, 12.0),
        // 22 - cristal del End: emisivo rosa y algo reflejante
        Material::opaque("cristal del End", t_crystal)
            .with_albedo(v3(1.0, 1.0, 1.0))
            .with_phong(0.30, 0.50, 120.0)
            .with_reflectivity(0.10)
            .with_emission(v3(1.0, 0.60, 1.0), 1.5),
        // 23 - vara del End: luz blanca (emisivo)
        Material::opaque("vara del End", t_rod)
            .with_albedo(v3(1.0, 1.0, 1.0))
            .with_phong(0.30, 0.20, 32.0)
            .with_emission(v3(1.0, 0.97, 1.0), 1.6),
        // 24 - nylium carmesi
        Material::opaque("nylium carmesi", t_nylium)
            .with_albedo(v3(1.0, 0.95, 0.95))
            .with_phong(0.90, 0.05, 10.0)
            .with_normal_map(n_nether),
        // 25 - tallo carmesi (mapa normal de la madera)
        Material::opaque("tallo carmesi", t_stem)
            .with_albedo(v3(1.0, 0.95, 0.95))
            .with_phong(0.85, 0.12, 20.0)
            .with_normal_map(n_wood),
        // 26 - bloque de verruga
        Material::opaque("verruga del Nether", t_wart)
            .with_albedo(v3(1.0, 0.95, 0.95))
            .with_phong(0.90, 0.05, 10.0),
        // 27 - shroomlight (emisivo naranja)
        Material::opaque("shroomlight", t_shroom)
            .with_albedo(v3(1.0, 0.95, 0.90))
            .with_phong(0.40, 0.10, 16.0)
            .with_emission(v3(1.0, 0.65, 0.35), 1.6),
    ];

    // Rejilla para los tres biomas, con margen para copas y estructuras.
    let mut world = World::new((END_X - 2, 0, -2), (NETHER_X + SIZE + 2, 26, SIZE + 3));

    let mut lights: Vec<Light> = Vec::new();

    build_cave(&mut world, seed, &mut lights);
    build_overworld(&mut world, seed, &mut lights);
    build_nether(&mut world, seed, &mut lights);
    build_end(&mut world, seed, &mut lights);

    let sky = Sky::Procedural {
        zenith: v3(0.10, 0.28, 0.78),
        horizon: v3(0.55, 0.72, 0.95),
        ground: v3(0.09, 0.11, 0.15),
        sun_dir: SUN_DIR.normalize(),
        sun_color: v3(1.0, 0.90, 0.72),
    };

    Scene {
        world,
        materials,
        textures,
        lights,
        sky,
        ambient: Vec3::splat(0.12) * v3(0.9, 1.0, 1.2),
        ambient_zones: vec![
            // Cueva: casi negra con un dejo azul verdoso del sculk.
            (cave_zone(), v3(0.030, 0.045, 0.055)),
            (nether_zone(), v3(0.16, 0.07, 0.05)),
            (end_zone(), v3(0.11, 0.09, 0.14)),
        ],
        // Cajas de niebla: los rayos que atraviesan una dimension cerrada (la
        // cueva, el Nether o el vacio del End) sin chocar nada se ven de su
        // color en vez de cielo azul.
        //
        // Los limites van EXACTAMENTE sobre la huella de cada bioma. Con un
        // bloque de margen, un rayo rasante recorria esa franja a lo largo,
        // se tenia del todo y aparecia un halo oscuro alrededor de la silueta.
        fogs: vec![
            Fog {
                min: v3(0.0, 0.0, 0.0),
                max: v3(SIZE as f32, CAVE_ROOF as f32, SIZE as f32),
                density: 0.30,
                color: v3(0.005, 0.012, 0.018),
                stars: 0.0,
            },
            // El Nether: solo bruma rojo oscuro, sin estrellas.
            Fog {
                min: v3(NETHER_X as f32, 0.0, 0.0),
                max: v3((NETHER_X + SIZE) as f32, 22.0, SIZE as f32),
                density: 0.40,
                color: v3(0.08, 0.016, 0.010),
                stars: 0.0,
            },
            // El End: el vacio negro-morado con estrellas.
            Fog {
                min: v3(END_X as f32, 0.0, 0.0),
                max: v3((END_X + SIZE) as f32, 23.0, SIZE as f32),
                density: 0.45,
                color: v3(0.025, 0.010, 0.045),
                stars: 0.010,
            },
        ],
    }
}

/// Sol a ~36 grados sobre el horizonte. Mas bajo = sombras mas largas, que es
/// lo que le da lectura al relieve del terreno procedural.
const SUN_DIR: Vec3 = Vec3 {
    x: 0.55,
    y: 0.50,
    z: 0.42,
};

fn overworld_zone() -> Zone {
    Zone::new(
        v3(0.0, CAVE_ROOF as f32, 0.0),
        v3(SIZE as f32, 40.0, SIZE as f32),
    )
}

fn cave_zone() -> Zone {
    Zone::new(v3(0.0, 0.0, 0.0), v3(SIZE as f32, CAVE_ROOF as f32, SIZE as f32))
}

fn nether_zone() -> Zone {
    Zone::new(
        v3(NETHER_X as f32, 0.0, 0.0),
        v3((NETHER_X + SIZE) as f32, 24.0, SIZE as f32),
    )
}

fn end_zone() -> Zone {
    Zone::new(
        v3(END_X as f32, 0.0, 0.0),
        v3((END_X + SIZE) as f32, 26.0, SIZE as f32),
    )
}

/// Luz puntual con caida por distancia, limitada a una zona.
fn point(pos: Vec3, color: Vec3, intensity: f32, zone: Zone) -> Light {
    Light {
        pos,
        color,
        intensity,
        attenuate: true,
        zone: Some(zone),
    }
}

/// Centro de una celda, desplazado `dy` en altura.
fn cell(x: i32, y: i32, z: i32, dy: f32) -> Vec3 {
    v3(x as f32 + 0.5, y as f32 + dy, z as f32 + 0.5)
}

// =====================================================================
// DEBAJO DE LA ISLA: CUEVA DEL DEEP DARK
// =====================================================================
fn build_cave(world: &mut World, seed: u32, lights: &mut Vec<Light>) {
    let roof = CAVE_ROOF - 1; // ultima capa del techo de la cueva
    // Altura del piso en cada columna, para apoyar cosas encima.
    let mut piso = [[0i32; SIZE as usize]; SIZE as usize];

    for x in 0..SIZE {
        for z in 0..SIZE {
            let n = fbm(x as f32 * 0.21, z as f32 * 0.21, 3, seed ^ 0xDA4C);
            // Por donde se extiende el sculk.
            let s = fbm(x as f32 * 0.17 + 40.0, z as f32 * 0.17, 3, seed ^ 0x5C01);
            let sculk = s > 0.47;

            // Piso con relieve suave (0, 1 o 2 bloques). En las dos filas del
            // frente se queda bajo, igual que el techo, para que el borde del
            // corte no tape el interior.
            let frente = z >= SIZE - 2;
            let h = if frente {
                0
            } else if n > 0.66 {
                2
            } else if n > 0.54 {
                1
            } else {
                0
            };
            for y in 0..=h {
                world.set(x, y, z, M_DEEPSLATE);
            }
            if sculk {
                world.set(x, h, z, M_SCULK);
            }
            piso[x as usize][z as usize] = h;

            // Techo con estalactitas donde el ruido es bajo.
            world.set(x, roof, z, if s > 0.62 { M_SCULK } else { M_DEEPSLATE });
            let c = fbm(x as f32 * 0.33, z as f32 * 0.33, 2, seed ^ 0xC0FE);
            if c < 0.40 && !frente {
                world.set(x, roof - 1, z, M_DEEPSLATE);
                if c < 0.30 {
                    world.set(x, roof - 2, z, M_DEEPSLATE);
                }
            }

            world.set(x, CAVE_ROOF, z, M_STONE); // capa que separa de la isla
        }
    }

    // Paredes cerradas atras (z = 0) y a los lados (x = 0 y x = SIZE-1): la
    // cueva solo se abre hacia el frente, donde mira la camara.
    for y in 1..roof {
        for i in 0..SIZE {
            for &(x, z) in &[(i, 0), (0, i), (SIZE - 1, i)] {
                let s = fbm(x as f32 * 0.4 + z as f32 * 0.4, y as f32 * 0.4, 2, seed ^ 0x5C02);
                world.set(x, y, z, if s > 0.58 { M_SCULK } else { M_DEEPSLATE });
            }
        }
    }

    // Fragmento de ciudad antigua: plataforma de ladrillo de pizarra contra la
    // pared del fondo, con el gran marco de pizarra reforzada.
    let (cx0, cx1) = (4, 11);
    for x in cx0..=cx1 {
        for z in 1..=4 {
            for y in 0..=1 {
                world.set(x, y, z, M_DEEPSLATE_BRICKS);
            }
            for y in 2..roof {
                world.remove(x, y, z);
            }
            piso[x as usize][z as usize] = 1;
        }
    }
    let (fx0, fx1) = (5, 10);
    for y in 2..roof {
        world.set(fx0, y, 1, M_REINFORCED);
        world.set(fx1, y, 1, M_REINFORCED);
    }
    for x in fx0..=fx1 {
        world.set(x, roof - 1, 1, M_REINFORCED);
    }
    // Detras del marco, la pared queda cubierta de sculk.
    for x in fx0 + 1..fx1 {
        for y in 2..roof - 1 {
            world.set(x, y, 0, M_SCULK);
        }
    }

    // Postes de ladrillo con linterna de almas a los lados del marco.
    let soul = v3(0.40, 0.90, 1.0);
    for &x in &[cx0, cx1] {
        world.set(x, 2, 4, M_DEEPSLATE_BRICKS);
        world.set(x, 3, 4, M_SOUL_LANTERN);
        lights.push(point(cell(x, 3, 4, 0.5), soul, 0.7, cave_zone()));
    }

    // Columnas de pizarra del piso al techo.
    for &(x, z) in &[(3, 12), (13, 10)] {
        for y in 1..roof {
            world.set(x, y, z, M_DEEPSLATE);
        }
    }

    // Sensores de sculk: brillan cian y alumbran un poco a su alrededor.
    for &(x, z) in &[(2, 7), (8, 9), (12, 13), (6, 14), (13, 6)] {
        let y = piso[x as usize][z as usize] + 1;
        world.set(x, y - 1, z, M_SCULK);
        world.set(x, y, z, M_SCULK_SENSOR);
        lights.push(point(cell(x, y, z, 0.9), v3(0.25, 0.85, 1.0), 0.3, cave_zone()));
    }
}

// =====================================================================
// AL CENTRO: LA ISLA DEL OVERWORLD
// =====================================================================
fn build_overworld(world: &mut World, seed: u32, lights: &mut Vec<Light>) {
    for x in 0..SIZE {
        for z in 0..SIZE {
            // El fBm de N octavas promedia hacia 0.5 y en la practica solo
            // recorre [0.25, 0.75]: usado en crudo daba apenas 3 alturas
            // distintas y el terreno salia plano. Se reescala ese rango util a
            // [0, 1] para aprovechar toda la amplitud.
            let raw = fbm(x as f32 * 0.13, z as f32 * 0.13, 4, seed);
            let n = ((raw - 0.25) / 0.5).clamp(0.0, 1.0);
            let h = GROUND_BASE + 1 + (n * 7.0) as i32;

            for y in GROUND_BASE..=h {
                let mat = if y == h {
                    if h >= SEA_LEVEL {
                        M_GRASS
                    } else {
                        M_DIRT
                    }
                } else if y >= h - 1 {
                    M_DIRT
                } else {
                    M_STONE
                };
                world.set(x, y, z, mat);
            }

            // Lago: se rellena con agua todo lo que quede bajo el nivel del mar.
            for y in (h + 1)..=SEA_LEVEL {
                world.set(x, y, z, M_WATER);
            }
        }
    }

    // Devuelve la altura del bloque solido mas alto (ignorando el agua).
    let top_of = |w: &World, x: i32, z: i32| -> i32 {
        let mut y = 22;
        while y >= GROUND_BASE {
            if let Some(m) = w.get(x, y, z) {
                if m != M_WATER {
                    return y;
                }
            }
            y -= 1;
        }
        GROUND_BASE
    };

    // Cabana de madera con ventanas de vidrio.
    let (hx, hz) = (10, 3);
    let base = flatten(world, hx, hz, 5, 4, &top_of);
    for x in hx..hx + 5 {
        for z in hz..hz + 4 {
            world.set(x, base, z, M_WOOD); // piso
        }
    }
    for y in base + 1..base + 4 {
        for x in hx..hx + 5 {
            for z in hz..hz + 4 {
                let borde = x == hx || x == hx + 4 || z == hz || z == hz + 3;
                if !borde {
                    continue;
                }
                let ventana = y == base + 2 && ((x + z) % 2 == 0);
                world.set(x, y, z, if ventana { M_GLASS } else { M_WOOD });
            }
        }
    }
    for x in hx..hx + 5 {
        for z in hz..hz + 4 {
            world.set(x, base + 4, z, M_WOOD); // techo
        }
    }

    // Portal de obsidiana: el enlace con el portal del Nether de al lado.
    let (px, pz) = (4, 9);
    let pbase = flatten(world, px, pz, 4, 1, &top_of);
    nether_portal(world, px, pbase, pz);

    // Antorchas: poste de madera rematado con una llama.
    let mut torches: Vec<Vec3> = Vec::new();
    for &(tx, tz) in &[(13, 12), (7, 6)] {
        let t = top_of(world, tx, tz);
        for y in t + 1..t + 3 {
            world.set(tx, y, tz, M_WOOD);
        }
        world.set(tx, t + 3, tz, M_TORCH);
        torches.push(v3(tx as f32 + 0.5, (t + 3) as f32 + 0.6, tz as f32 + 0.5));
    }

    // Arboles (tronco de madera + copa de hojas).
    for &(tx, tz) in &[(7, 13), (12, 14), (5, 12)] {
        let t = top_of(world, tx, tz);
        if world.get(tx, t, tz) != Some(M_GRASS) {
            continue;
        }
        for y in t + 1..t + 4 {
            world.set(tx, y, tz, M_WOOD);
        }
        for dx in -1..=1 {
            for dz in -1..=1 {
                world.set(tx + dx, t + 4, tz + dz, M_LEAVES);
            }
        }
        world.set(tx, t + 5, tz, M_LEAVES);
    }

    // ---------------- Luces del overworld ----------------
    // El sol solo alumbra la isla: no entra a la cueva ni a las otras
    // dimensiones, que no tienen sol.
    lights.push(Light {
        pos: SUN_DIR.normalize() * 400.0,
        color: v3(1.0, 0.94, 0.82),
        intensity: 1.70,
        attenuate: false,
        zone: Some(overworld_zone()),
    });

    lights.push(point(
        v3(px as f32 + 2.0, pbase as f32 + 2.5, pz as f32 + 0.8),
        v3(0.62, 0.25, 1.0),
        1.4,
        overworld_zone(),
    ));

    for t in torches {
        lights.push(point(t, v3(1.0, 0.55, 0.18), 1.6, overworld_zone()));
    }
}

/// Portal del Nether de 4x5 (marco de obsidiana) en el plano z = `z`, con la
/// esquina inferior izquierda en (x, y).
fn nether_portal(world: &mut World, x: i32, y: i32, z: i32) {
    for yy in y..y + 5 {
        world.set(x, yy, z, M_OBSIDIAN);
        world.set(x + 3, yy, z, M_OBSIDIAN);
    }
    for xx in x..x + 4 {
        world.set(xx, y, z, M_OBSIDIAN);
        world.set(xx, y + 4, z, M_OBSIDIAN);
    }
    for yy in y + 1..y + 4 {
        for xx in x + 1..x + 3 {
            world.set(xx, yy, z, M_PORTAL);
        }
    }
}

// =====================================================================
// A LA DERECHA: EL NETHER
// =====================================================================
fn build_nether(world: &mut World, seed: u32, lights: &mut Vec<Light>) {
    let x0 = NETHER_X;
    let mut top = [[0i32; SIZE as usize]; SIZE as usize];

    for lx in 0..SIZE {
        for z in 0..SIZE {
            let x = x0 + lx;
            let raw = fbm(lx as f32 * 0.15, z as f32 * 0.15, 3, seed ^ 0xB00B);
            let n = ((raw - 0.25) / 0.5).clamp(0.0, 1.0);
            let mut h = (n * 6.0) as i32;

            // Acantilados al fondo (z = 0) y a la derecha (lx = SIZE-1): el
            // terreno sube en rampa hacia ellos y el borde es una pared alta.
            let d = z.min(SIZE - 1 - lx);
            if d == 0 {
                let w = fbm(lx as f32 * 0.3 + z as f32 * 0.3, 7.0, 2, seed ^ 0xB0B0);
                h = 12 + (w * 5.0) as i32;
            } else if d < 4 {
                h += (4 - d) * 2;
            }

            for y in 0..=h {
                world.set(x, y, z, M_NETHERRACK);
            }
            for y in h + 1..=LAVA_LEVEL {
                world.set(x, y, z, M_LAVA);
            }

            // Nylium carmesi sobre las partes altas que no son pared.
            let s = fbm(lx as f32 * 0.25, z as f32 * 0.25, 2, seed ^ 0xC12);
            if d > 0 && h > LAVA_LEVEL && s > 0.45 {
                world.set(x, h, z, M_NYLIUM);
            }
            top[lx as usize][z as usize] = h;
        }
    }

    // Repisa que sale de la pared del fondo, con glowstone colgando debajo.
    let mut glow: Vec<Vec3> = Vec::new();
    for lx in 2..8 {
        for z in 1..3 {
            world.set(x0 + lx, 11, z, M_NETHERRACK);
        }
    }
    for &(lx, z, largo) in &[(3, 1, 1), (5, 2, 2), (6, 1, 1)] {
        for k in 0..largo {
            world.set(x0 + lx, 10 - k, z, M_GLOWSTONE);
        }
        glow.push(cell(x0 + lx, 10 - largo, z, 0.3));
    }
    // Otra repisa en la pared derecha.
    for lx in 13..15 {
        for z in 6..11 {
            world.set(x0 + lx, 12, z, M_NETHERRACK);
        }
    }
    for &(lx, z) in &[(13, 7), (14, 9)] {
        world.set(x0 + lx, 11, z, M_GLOWSTONE);
        glow.push(cell(x0 + lx, 10, z, 0.3));
    }

    // Cascada de lava que cae de la pared del fondo al lago.
    let (cx, cz) = (10, 1);
    let tope = 13;
    for y in top[cx as usize][cz as usize] + 1..=tope {
        world.set(x0 + cx, y, cz, M_LAVA);
    }
    world.set(x0 + cx, tope, 0, M_LAVA);

    // Hongos carmesi gigantes: tallo + copa de verruga con shroomlight.
    let mut shrooms: Vec<Vec3> = Vec::new();
    for &(lx, z, alto) in &[(5, 10, 5), (11, 8, 4)] {
        let x = x0 + lx;
        let t = top[lx as usize][z as usize].max(LAVA_LEVEL);
        world.set(x, t, z, M_NYLIUM);
        for y in t + 1..=t + alto {
            world.set(x, y, z, M_CRIMSON_STEM);
        }
        let c = t + alto;
        for dx in -2i32..=2 {
            for dz in -2i32..=2 {
                let borde = dx.abs() == 2 || dz.abs() == 2;
                let esquina = dx.abs() == 2 && dz.abs() == 2;
                if esquina {
                    continue;
                }
                // Copa de 5x5 con faldon colgando por los bordes.
                world.set(x + dx, c + 1, z + dz, M_WART);
                if borde {
                    world.set(x + dx, c, z + dz, M_WART);
                }
                if dx.abs() <= 1 && dz.abs() <= 1 {
                    world.set(x + dx, c + 2, z + dz, M_WART);
                }
            }
        }
        // Shroomlight bajo la copa, al lado del tallo.
        world.set(x + 1, c, z, M_SHROOMLIGHT);
        world.set(x - 1, c, z + 1, M_SHROOMLIGHT);
        shrooms.push(cell(x, c, z + 1, 0.0));
    }

    // Portal del Nether: el otro extremo del de la isla.
    let (plx, pz) = (2, 5);
    let base = (plx..plx + 4)
        .map(|lx| top[lx as usize][pz as usize])
        .max()
        .unwrap_or(0)
        .max(LAVA_LEVEL);
    for lx in plx - 1..plx + 5 {
        for z in pz - 1..=pz + 1 {
            for y in 0..base {
                world.set(x0 + lx, y, z, M_NETHERRACK);
            }
            for y in base..base + 6 {
                world.remove(x0 + lx, y, z);
            }
        }
    }
    nether_portal(world, x0 + plx, base, pz);

    // ---------------- Luces del Nether ----------------
    let zone = nether_zone();
    // Relleno rojizo sin caida: el Nether no tiene sol, pero sin una luz
    // general el relieve se pierde. Viene del frente y de arriba.
    lights.push(Light {
        pos: v3(0.25, 0.85, 0.55).normalize() * 400.0,
        color: v3(1.0, 0.45, 0.30),
        intensity: 0.40,
        attenuate: false,
        zone: Some(zone),
    });
    // Resplandor del lago y de la cascada de lava.
    for &(lx, y, z) in &[(8.5, 3.6, 6.5), (6.5, 3.6, 13.5), (10.5, 8.0, 2.4)] {
        lights.push(point(v3(x0 as f32 + lx, y, z), v3(1.0, 0.40, 0.10), 2.2, zone));
    }
    for g in glow {
        lights.push(point(g, v3(1.0, 0.80, 0.40), 2.2, zone));
    }
    for s in shrooms {
        lights.push(point(s, v3(1.0, 0.60, 0.30), 1.4, zone));
    }
    lights.push(point(
        v3((x0 + plx) as f32 + 2.0, base as f32 + 2.5, pz as f32 + 0.8),
        v3(0.62, 0.25, 1.0),
        1.4,
        zone,
    ));
}

// =====================================================================
// A LA IZQUIERDA: EL END
// =====================================================================
fn build_end(world: &mut World, seed: u32, lights: &mut Vec<Light>) {
    let x0 = END_X;
    // Superficie de la isla flotante; -1 donde no hay isla.
    let mut top = [[-1i32; SIZE as usize]; SIZE as usize];
    let base_y = 9;

    for lx in 0..SIZE {
        for z in 0..SIZE {
            let (dx, dz) = (lx as f32 - 7.5, z as f32 - 7.5);
            let borde = fbm(lx as f32 * 0.25, z as f32 * 0.25, 2, seed ^ 0xE4D);
            let r = (dx * dx + dz * dz).sqrt() / 7.8 + (borde - 0.5) * 0.35;
            if r >= 1.0 {
                continue;
            }
            let bump = fbm(lx as f32 * 0.2, z as f32 * 0.2, 2, seed ^ 0xE4E);
            let t = base_y + (bump * 2.0) as i32;
            // La isla se angosta hacia abajo como una gota.
            let fondo = t - ((1.0 - r).sqrt() * 8.0) as i32;
            for y in fondo..=t {
                world.set(x0 + lx, y, z, M_END_STONE);
            }
            top[lx as usize][z as usize] = t;
        }
    }
    let suelo = |lx: i32, z: i32| top[lx as usize][z as usize].max(base_y);

    // Pilares de obsidiana (en cruz) con un cristal del End encima.
    let mut crystals: Vec<Vec3> = Vec::new();
    for &(lx, z, alto) in &[(3, 5, 18), (12, 11, 15)] {
        let t = suelo(lx, z);
        for &(dx, dz) in &[(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let b = top[(lx + dx) as usize][(z + dz) as usize];
            let desde = if b >= 0 { b + 1 } else { t - 3 };
            for y in desde..=alto {
                world.set(x0 + lx + dx, y, z + dz, M_OBSIDIAN);
            }
        }
        world.set(x0 + lx, alto + 1, z, M_END_CRYSTAL);
        crystals.push(cell(x0 + lx, alto + 1, z, 0.5));
    }

    // Torre de purpur (fragmento de ciudad del End) con varas en el techo.
    let (tx, tz) = (7, 2);
    let t = (tx..tx + 3)
        .flat_map(|lx| (tz..tz + 3).map(move |z| (lx, z)))
        .map(|(lx, z)| suelo(lx, z))
        .max()
        .unwrap_or(base_y);
    for lx in tx..tx + 3 {
        for z in tz..tz + 3 {
            for y in t - 2..=t + 5 {
                world.set(x0 + lx, y, z, M_PURPUR);
            }
        }
    }
    // Techo volado de 5x5 y una vara del End en cada esquina.
    for lx in tx - 1..tx + 4 {
        for z in tz - 1..tz + 4 {
            world.set(x0 + lx, t + 6, z, M_PURPUR);
        }
    }
    for &(lx, z) in &[(tx - 1, tz - 1), (tx + 3, tz - 1), (tx - 1, tz + 3), (tx + 3, tz + 3)] {
        world.set(x0 + lx, t + 7, z, M_END_ROD);
    }

    // Plantas de chorus: tallo con ramas que suben.
    let chorus: &[(i32, i32, i32)] = &[
        (0, 1, 0),
        (0, 2, 0),
        (0, 3, 0),
        (1, 3, 0),
        (1, 4, 0),
        (1, 5, 0),
        (0, 4, 0),
        (0, 4, 1),
        (0, 5, 1),
        (0, 6, 1),
        (-1, 4, 0),
        (-1, 5, 0),
        (0, 5, 0),
        (0, 6, 0),
    ];
    for &(lx, z) in &[(5, 11), (11, 5), (8, 8)] {
        let t = suelo(lx, z);
        for &(dx, dy, dz) in chorus {
            world.set(x0 + lx + dx, t + dy, z + dz, M_CHORUS);
        }
    }

    // ---------------- Luces del End ----------------
    let zone = end_zone();
    // Luz general fria y tenue: sin ella la isla se pierde contra el vacio.
    lights.push(Light {
        pos: v3(0.30, 0.90, 0.45).normalize() * 400.0,
        color: v3(0.90, 0.86, 1.0),
        intensity: 0.85,
        attenuate: false,
        zone: Some(zone),
    });
    for c in crystals {
        lights.push(point(c, v3(1.0, 0.55, 0.95), 2.0, zone));
    }
    lights.push(point(
        v3((x0 + tx) as f32 + 1.5, (t + 7) as f32 + 0.8, tz as f32 + 1.5),
        v3(0.95, 0.90, 1.0),
        1.6,
        zone,
    ));
}

/// Prepara un terreno plano para construir sobre la huella `w x d` que empieza
/// en (x0, z0). Devuelve la altura `y` donde va la primera capa del edificio.
///
/// Sin esto, la casa y el portal se apoyaban en la altura de UNA esquina y el
/// resto del terreno los atravesaba. Aqui:
///   - la altura es el promedio de la huella (nunca bajo el nivel del mar),
///   - dentro de la huella se rellena por debajo y se vacia por encima,
///   - en un anillo de un bloque alrededor se recorta lo que sobresalga y se
///     deja pasto, para que las paredes no queden enterradas.
fn flatten(
    world: &mut World,
    x0: i32,
    z0: i32,
    w: i32,
    d: i32,
    top_of: &dyn Fn(&World, i32, i32) -> i32,
) -> i32 {
    let mut suma = 0;
    for x in x0..x0 + w {
        for z in z0..z0 + d {
            suma += top_of(world, x, z);
        }
    }
    let promedio = (suma as f32 / (w * d) as f32).round() as i32;
    let level = promedio.max(SEA_LEVEL) + 1;

    for x in (x0 - 1).max(0)..(x0 + w + 1).min(SIZE) {
        for z in (z0 - 1).max(0)..(z0 + d + 1).min(SIZE) {
            let dentro = x >= x0 && x < x0 + w && z >= z0 && z < z0 + d;
            let mut recortado = false;
            for y in level..24 {
                if world.get(x, y, z).is_some() {
                    world.remove(x, y, z);
                    recortado = true;
                }
            }
            if dentro {
                // Cimiento solido (tambien reemplaza el agua).
                for y in GROUND_BASE..level {
                    let hueco = world.get(x, y, z).map_or(true, |m| m == M_WATER);
                    if hueco {
                        world.set(x, y, z, if y == level - 1 { M_DIRT } else { M_STONE });
                    }
                }
            } else if recortado {
                world.set(x, level - 1, z, M_GRASS);
            }
        }
    }
    level
}
