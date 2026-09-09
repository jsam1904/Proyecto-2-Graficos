use crate::material::Material;
use crate::noise::fbm;
use crate::render::{Light, Scene};
use crate::sky::Sky;
use crate::texture::{self, Texture};
use crate::vec3::{v3, Vec3};
use crate::world::World;

pub const M_GRASS: u8 = 0;
pub const M_DIRT: u8 = 1;
pub const M_STONE: u8 = 2;
pub const M_WOOD: u8 = 3;
pub const M_WATER: u8 = 4;
pub const M_LAVA: u8 = 5;
pub const M_OBSIDIAN: u8 = 6;
pub const M_GLASS: u8 = 7;

pub const SIZE: i32 = 16;
pub const SEA_LEVEL: i32 = 4;

pub fn build(seed: u32) -> Scene {
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

    let n_stone = texture::normal_from_height(&texture::tex_stone(32), 2.5);
    let n_stone = push(n_stone, &mut textures);
    let n_wood = texture::normal_from_height(&texture::tex_wood(32), 1.6);
    let n_wood = push(n_wood, &mut textures);
    let n_water = texture::normal_from_height(&texture::height_waves(32), 1.2);
    let n_water = push(n_water, &mut textures);

    let materials = vec![
        Material::opaque("pasto", t_grass).with_phong(0.95, 0.05, 8.0),
        Material::opaque("tierra", t_dirt).with_phong(0.95, 0.03, 4.0),
        Material::opaque("piedra", t_stone)
            .with_phong(0.80, 0.20, 32.0)
            .with_normal_map(n_stone)
            .with_reflectivity(0.03),
        Material::opaque("madera", t_wood)
            .with_phong(0.85, 0.15, 24.0)
            .with_normal_map(n_wood),
        Material::opaque("agua", t_water)
            .with_phong(0.15, 0.70, 180.0)
            .with_normal_map(n_water)
            .with_refraction(0.88, 1.33)
            .with_reflectivity(0.10)
            .with_albedo(v3(0.75, 0.92, 1.0)),
        Material::opaque("lava", t_lava)
            .with_phong(0.25, 0.05, 8.0)
            .with_emission(v3(1.0, 0.42, 0.10), 3.2),
        Material::opaque("obsidiana", t_obs)
            .with_phong(0.30, 0.55, 220.0)
            .with_reflectivity(0.65),
        Material::opaque("vidrio", t_glass)
            .with_phong(0.05, 0.60, 200.0)
            .with_refraction(0.92, 1.52)
            .with_reflectivity(0.08)
            .with_albedo(v3(0.88, 0.96, 0.92)),
    ];

    let mut world = World::new();

    for x in 0..SIZE {
        for z in 0..SIZE {
            let n = fbm(x as f32 * 0.13, z as f32 * 0.13, 4, seed);
            let h = 1 + (n * 6.0) as i32;

            for y in 0..=h {
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

            for y in (h + 1)..=SEA_LEVEL {
                world.set(x, y, z, M_WATER);
            }
        }
    }

    let top_of = |w: &World, x: i32, z: i32| -> i32 {
        let mut y = 12;
        while y >= 0 {
            if let Some(m) = w.get(x, y, z) {
                if m != M_WATER {
                    return y;
                }
            }
            y -= 1;
        }
        0
    };

    let (hx, hz) = (10, 3);
    let base = top_of(&world, hx, hz).max(SEA_LEVEL) + 1;
    for x in hx..hx + 5 {
        for z in hz..hz + 4 {
            for y in 0..base {
                if world.get(x, y, z).map_or(true, |m| m == M_WATER) {
                    world.set(x, y, z, M_STONE);
                }
            }
            world.set(x, base, z, M_WOOD);
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
            world.set(x, base + 4, z, M_WOOD);
        }
    }

    let (lx, lz) = (3, 12);
    let mut lava_center = v3(lx as f32 + 1.5, 6.0, lz as f32 + 1.5);
    for x in lx..lx + 3 {
        for z in lz..lz + 3 {
            let t = top_of(&world, x, z);
            world.set(x, t, z, M_LAVA);
            if x == lx + 1 && z == lz + 1 {
                lava_center = v3(x as f32 + 0.5, t as f32 + 1.2, z as f32 + 0.5);
            }
        }
    }

    let mut torches: Vec<Vec3> = Vec::new();
    for &(px, pz) in &[(6, 6), (13, 11), (2, 5)] {
        let t = top_of(&world, px, pz);
        for y in t + 1..t + 4 {
            world.set(px, y, pz, M_OBSIDIAN);
        }
        world.set(px, t + 4, pz, M_LAVA);
        torches.push(v3(px as f32 + 0.5, (t + 5) as f32, pz as f32 + 0.5));
    }

    for &(tx, tz) in &[(7, 13), (12, 14), (5, 9)] {
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

    let sun_dir = v3(0.55, 0.72, 0.42).normalize();
    let mut lights = vec![
        Light {
            pos: sun_dir * 400.0,
            color: v3(1.0, 0.96, 0.88),
            intensity: 1.45,
            attenuate: false,
        },
        Light {
            pos: lava_center,
            color: v3(1.0, 0.45, 0.12),
            intensity: 9.0,
            attenuate: true,
        },
    ];
    for t in torches {
        lights.push(Light {
            pos: t,
            color: v3(1.0, 0.55, 0.18),
            intensity: 5.0,
            attenuate: true,
        });
    }

    let sky = Sky::Procedural {
        zenith: v3(0.16, 0.33, 0.72),
        horizon: v3(0.70, 0.80, 0.94),
        ground: v3(0.22, 0.20, 0.18),
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
    }
}