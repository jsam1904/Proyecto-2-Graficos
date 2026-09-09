//! Diorama con raytracing sobre cubos texturizados.
//!
//! Uso:
//!   cargo run --release -- view    [puerto]
//!   cargo run --release -- still   [ancho] [alto] [supersampling]
//!   cargo run --release -- animate [ancho] [alto] [frames] [supersampling]
//!
//! `view` abre un visor interactivo en el navegador (mouse + teclado).
//! `still` guarda out/diorama.bmp y out/diorama.ppm.
//! `animate` guarda los frames PPM para armar el video:
//!   ffmpeg -framerate 30 -i out/frame_%04d.ppm -c:v libx264 -pix_fmt yuv420p diorama.mp4

mod bmp;
mod camera;
mod material;
mod noise;
mod render;
mod scene;
mod server;
mod sky;
mod texture;
mod vec3;
mod world;

use camera::Camera;
use render::RenderOpts;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::time::Instant;
use vec3::{v3, Vec3};

fn save_ppm(path: &str, w: usize, h: usize, buf: &[Vec3]) -> io::Result<()> {
    let file = File::create(path)?;
    let mut out = BufWriter::new(file);
    write!(out, "P6\n{} {}\n255\n", w, h)?;
    let mut bytes = Vec::with_capacity(w * h * 3);
    for c in buf {
        let rgb = c.to_rgb8();
        bytes.extend_from_slice(&rgb);
    }
    out.write_all(&bytes)?;
    out.flush()
}

fn arg<T: std::str::FromStr>(args: &[String], i: usize, default: T) -> T {
    args.get(i).and_then(|s| s.parse().ok()).unwrap_or(default)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mode = args.get(1).cloned().unwrap_or_else(|| "view".to_string());

    // Hilos disponibles (sin librerias externas).
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let scene = scene::build(2026);
    println!(
        "Escena: {} cubos, {} materiales, {} luces, {} hilos",
        scene.world.len(),
        scene.materials.len(),
        scene.lights.len(),
        threads
    );

    fs::create_dir_all("out")?;
    let center = v3(scene::SIZE as f32 * 0.5, 4.0, scene::SIZE as f32 * 0.5);

    match mode.as_str() {
        "view" => {
            let port: u16 = arg(&args, 2, 8080u16);
            let addr = format!("127.0.0.1:{}", port);
            server::serve(&scene, center, &addr, threads)?;
        }
        "animate" => {
            let w = arg(&args, 2, 800usize);
            let h = arg(&args, 3, 450usize);
            let frames = arg(&args, 4, 120usize);
            let ss = arg(&args, 5, 1usize);

            let t0 = Instant::now();
            for f in 0..frames {
                let t = f as f32 / frames as f32;
                let mut cam = Camera::new(center, 0.0);
                // Rotacion completa del diorama + acercamiento y alejamiento.
                cam.yaw = t * std::f32::consts::TAU;
                cam.pitch = 0.42 + 0.16 * (t * std::f32::consts::TAU).sin();
                cam.dist = 30.0 - 11.0 * (t * std::f32::consts::TAU).cos();

                let buf = render::render_parallel(
                    &scene,
                    &cam,
                    w,
                    h,
                    RenderOpts {
                        samples: ss,
                        max_depth: render::MAX_DEPTH,
                        threads,
                    },
                );
                save_ppm(&format!("out/frame_{:04}.ppm", f), w, h, &buf)?;
                println!(
                    "frame {}/{}  dist={:.1}  ({:.1}s totales)",
                    f + 1,
                    frames,
                    cam.dist,
                    t0.elapsed().as_secs_f32()
                );
            }
            println!(
                "\nListo. Arma el video con:\n  ffmpeg -framerate 30 -i out/frame_%04d.ppm \
                 -c:v libx264 -pix_fmt yuv420p diorama.mp4"
            );
        }
        _ => {
            let w = arg(&args, 2, 1280usize);
            let h = arg(&args, 3, 720usize);
            let ss = arg(&args, 4, 2usize);

            let mut cam = Camera::new(center, 26.0);
            cam.yaw = 0.85;
            cam.pitch = 0.48;

            let t0 = Instant::now();
            let buf = render::render_parallel(
                &scene,
                &cam,
                w,
                h,
                RenderOpts {
                    samples: ss,
                    max_depth: render::MAX_DEPTH,
                    threads,
                },
            );
            println!("Render en {:.2}s", t0.elapsed().as_secs_f32());
            save_ppm("out/diorama.ppm", w, h, &buf)?;
            bmp::save_bmp("out/diorama.bmp", w, h, &buf)?;
            println!("Guardado en out/diorama.bmp y out/diorama.ppm");
        }
    }

    Ok(())
}