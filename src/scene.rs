//! Construccion del diorama de dos niveles:
//!   - abajo el Nether (netherrack, lago de lava, glowstone en el techo)
//!   - arriba el overworld (terreno procedural 16x16, lago, cabana, arboles)
//! Los dos mundos estan separados por una capa de piedra, como en un diorama
//! de corte: dos paredes traseras cerradas y dos lados abiertos para ver dentro.

use crate::material::Material;
use crate::noise::fbm;
use crate::render::{Fog, Light, Scene};
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

pub const SIZE: i32 = 16; // area procedural: 16x16 cubos por nivel
pub const NETHER_ROOF: i32 = 7; // capa de piedra que separa los mundos
pub const GROUND_BASE: i32 = 8; // primera capa del overworld
pub const SEA_LEVEL: i32 = GROUND_BASE + 3;

/// Punto al que mira la camara (entre los dos niveles).
pub fn center() -> Vec3 {
    v3(SIZE as f32 * 0.5, 6.5, SIZE as f32 * 0.5)
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
    let t_glass = push(texture::tex_water(16), &mut textures);
    let t_nether = push(texture::tex_netherrack(32), &mut textures);
    let t_glow = push(texture::tex_glowstone(32), &mut textures);

    // Mapas normales derivados de mapas de altura SUAVES (filtro Sobel propio).
    let n_stone = texture::normal_from_height(&texture::height_blobs(32, 0.30, 77), 0.9);
    let n_stone = push(n_stone, &mut textures);
    let n_wood = texture::normal_from_height(&texture::height_planks(32), 0.7);
    let n_wood = push(n_wood, &mut textures);
    let n_water = texture::normal_from_height(&texture::height_waves(32), 0.45);
    let n_water = push(n_water, &mut textures);
    let n_nether = texture::normal_from_height(&texture::height_blobs(32, 0.30, 131), 1.1);
    let n_nether = push(n_nether, &mut textures);

    // ---------------- Materiales ----------------
    // El indice DEBE coincidir con las constantes M_*.
    let materials = vec![
        // 0 - pasto
        Material::opaque("pasto", t_grass).with_phong(0.95, 0.05, 8.0),
        // 1 - tierra
        Material::opaque("tierra", t_dirt).with_phong(0.95, 0.03, 4.0),
        // 2 - piedra (mapa normal)
        Material::opaque("piedra", t_stone)
            .with_phong(0.80, 0.20, 32.0)
            .with_normal_map(n_stone)
            .with_reflectivity(0.03),
        // 3 - madera (mapa normal)
        Material::opaque("madera", t_wood)
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
        Material::opaque("lava", t_lava)
            .with_phong(0.25, 0.05, 8.0)
            .with_emission(v3(1.0, 0.42, 0.10), 3.2),
        // 6 - obsidiana pulida (reflexion)
        Material::opaque("obsidiana", t_obs)
            .with_phong(0.30, 0.55, 220.0)
            .with_reflectivity(0.65),
        // 7 - vidrio (refraccion fuerte)
        Material::opaque("vidrio", t_glass)
            .with_phong(0.05, 0.60, 200.0)
            .with_refraction(0.92, 1.52)
            .with_reflectivity(0.08)
            .with_albedo(v3(0.88, 0.96, 0.92)),
        // 8 - netherrack (mapa normal, superficie rugosa y mate)
        Material::opaque("netherrack", t_nether)
            .with_phong(0.92, 0.06, 10.0)
            .with_normal_map(n_nether),
        // 9 - glowstone (emisivo)
        Material::opaque("glowstone", t_glow)
            .with_phong(0.35, 0.10, 16.0)
            .with_emission(v3(1.0, 0.82, 0.42), 2.6),
    ];

    let mut world = World::new((-2, 0, -2), (SIZE + 3, 24, SIZE + 3));

    // ================= NIVEL INFERIOR: EL NETHER =================
    // Invertido: el techo es un mar de lava pegado a la capa de piedra que
    // separa los mundos, y el netherrack queda como piso.
    for x in 0..SIZE {
        for z in 0..SIZE {
            world.set(x, 0, z, M_NETHERRACK); // base del diorama

            let n = fbm(x as f32 * 0.17, z as f32 * 0.17, 3, seed ^ 0xB00B);

            // Relieve del piso.
            if n > 0.56 {
                world.set(x, 1, z, M_NETHERRACK);
            }

            // Techo: lava donde el ruido es bajo, netherrack donde es alto.
            if n < 0.46 {
                world.set(x, NETHER_ROOF - 1, z, M_LAVA);
                // Goterones colgando de las zonas mas profundas del mar de lava.
                if n < 0.22 {
                    world.set(x, NETHER_ROOF - 2, z, M_LAVA);
                }
            } else {
                world.set(x, NETHER_ROOF - 1, z, M_NETHERRACK);
            }

            world.set(x, NETHER_ROOF, z, M_STONE); // capa que separa los mundos
        }
    }

    // Dos paredes traseras cerradas: el diorama se ve como casa de munecas.
    for y in 1..NETHER_ROOF - 1 {
        for i in 0..SIZE {
            world.set(SIZE - 1, y, i, M_NETHERRACK);
            world.set(i, y, SIZE - 1, M_NETHERRACK);
        }
    }

    // Columnas de netherrack que suben del piso hasta el mar de lava.
    for &(cx, cz) in &[(4, 4), (11, 6), (6, 11)] {
        for y in 1..NETHER_ROOF - 1 {
            world.set(cx, y, cz, M_NETHERRACK);
        }
    }

    // Portal de obsidiana en la pared del fondo del Nether (plano z = SIZE-1).
    let np_x = 6;
    let np_z = SIZE - 1;
    for y in 1..6 {
        world.set(np_x, y, np_z, M_OBSIDIAN);
        world.set(np_x + 3, y, np_z, M_OBSIDIAN);
    }
    for x in np_x..np_x + 4 {
        world.set(x, 1, np_z, M_OBSIDIAN);
        world.set(x, 5, np_z, M_OBSIDIAN);
    }
    for y in 2..5 {
        for x in np_x + 1..np_x + 3 {
            world.set(x, y, np_z, M_GLASS); // interior refractante
        }
    }

    // Glowstone incrustado en el piso: ilumina desde abajo, ahora que el
    // techo lo ocupa la lava.
    let glow_cells = [(3, 8), (12, 11), (8, 3)];
    for &(gx, gz) in &glow_cells {
        world.set(gx, 1, gz, M_GLOWSTONE);
    }

    // ================= NIVEL SUPERIOR: OVERWORLD =================
    for x in 0..SIZE {
        for z in 0..SIZE {
            let n = fbm(x as f32 * 0.13, z as f32 * 0.13, 4, seed);
            let h = GROUND_BASE + 1 + (n * 5.0) as i32;

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
    let base = top_of(&world, hx, hz).max(SEA_LEVEL) + 1;
    for x in hx..hx + 5 {
        for z in hz..hz + 4 {
            for y in GROUND_BASE..base {
                if world.get(x, y, z).map_or(true, |m| m == M_WATER) {
                    world.set(x, y, z, M_STONE);
                }
            }
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

    // Portal de obsidiana: el enlace visual entre los dos mundos.
    let (px, pz) = (4, 9);
    let pbase = top_of(&world, px, pz) + 1;
    for y in pbase..pbase + 4 {
        world.set(px, y, pz, M_OBSIDIAN);
        world.set(px + 3, y, pz, M_OBSIDIAN);
    }
    for x in px..px + 4 {
        world.set(x, pbase, pz, M_OBSIDIAN);
        world.set(x, pbase + 4, pz, M_OBSIDIAN);
    }
    // Interior del portal: vidrio tenido de morado (refraccion).
    for y in pbase + 1..pbase + 4 {
        for x in px + 1..px + 3 {
            world.set(x, y, pz, M_GLASS);
        }
    }

    // Antorchas: pilares de obsidiana rematados con lava.
    let mut torches: Vec<Vec3> = Vec::new();
    for &(tx, tz) in &[(13, 12), (7, 6)] {
        let t = top_of(&world, tx, tz);
        for y in t + 1..t + 3 {
            world.set(tx, y, tz, M_OBSIDIAN);
        }
        world.set(tx, t + 3, tz, M_LAVA);
        torches.push(v3(tx as f32 + 0.5, (t + 4) as f32, tz as f32 + 0.5));
    }

    // Arboles (tronco de madera + copa de hojas).
    for &(tx, tz) in &[(7, 13), (12, 14), (5, 12)] {
        let t = top_of(&world, tx, tz);
        if world.get(tx, t, tz) != Some(M_GRASS) {
            continue;
        }
        for y in t + 1..t + 4 {
            world.set(tx, y, tz, M_WOOD);
        }
        for dx in -1..=1 {
            for dz in -1..=1 {
                world.set(tx + dx, t + 4, tz + dz, M_GRASS);
            }
        }
        world.set(tx, t + 5, tz, M_GRASS);
    }

    // ---------------- Luces y cielo ----------------
    let sun_dir = v3(0.55, 0.72, 0.42).normalize();
    let mut lights = vec![Light {
        pos: sun_dir * 400.0,
        color: v3(1.0, 0.96, 0.88),
        intensity: 1.45,
        attenuate: false,
    }];

    // Resplandor del mar de lava del techo del Nether.
    for &(lx, lz) in &[(5, 6), (11, 10)] {
        lights.push(Light {
            pos: v3(lx as f32 + 0.5, NETHER_ROOF as f32 - 2.2, lz as f32 + 0.5),
            color: v3(1.0, 0.40, 0.10),
            intensity: 9.0,
            attenuate: true,
        });
    }

    // Luz de cada glowstone del piso.
    for &(gx, gz) in &glow_cells {
        lights.push(Light {
            pos: v3(gx as f32 + 0.5, 2.1, gz as f32 + 0.5),
            color: v3(1.0, 0.80, 0.40),
            intensity: 7.0,
            attenuate: true,
        });
    }

    for t in torches {
        lights.push(Light {
            pos: t,
            color: v3(1.0, 0.55, 0.18),
            intensity: 5.0,
            attenuate: true,
        });
    }

    let sky = Sky::Procedural {
        zenith: v3(0.10, 0.28, 0.78),
        horizon: v3(0.55, 0.72, 0.95),
        ground: v3(0.09, 0.11, 0.15),
        sun_dir,
        sun_color: v3(1.0, 0.90, 0.72),
    };

    Scene {
        world,
        materials,
        textures,
        lights,
        sky,
        ambient: Vec3::splat(0.12) * v3(0.9, 1.0, 1.2),
        // Sin cielo debajo de la capa de piedra: el Nether queda contra un
        // fondo rojo muy oscuro en vez del azul del overworld.
        fog: Some(Fog {
            center: center(),
            level: NETHER_ROOF as f32 + 0.7,
            thickness: 1.8,
            color: v3(0.05, 0.015, 0.015),
        }),
    }
}