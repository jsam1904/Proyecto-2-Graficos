# Proyecto 2 — Diorama con Raytracing

Raytracer escrito **desde cero en Rust, sin dependencias externas** (`[dependencies]` vacío).
Renderiza un diorama estilo Minecraft construido con cubos texturizados: terreno
generado con ruido, un lago, una casa de madera con ventanas de vidrio, un pozo de
lava, antorchas de obsidiana y árboles.

Incluye tres modos de uso: imagen fija, visor interactivo en el navegador y
secuencia de frames para armar un video.

---

## Requisitos

- Rust estable (edición 2024) — `cargo`
- Opcional: `ffmpeg`, solo para convertir los frames del modo `animate` en video

## Uso

```bash
# Visor interactivo (por defecto) -> http://127.0.0.1:8080
cargo run --release -- view [puerto]

# Imagen fija -> out/diorama.bmp y out/diorama.ppm
cargo run --release -- still [ancho] [alto] [supersampling]

# Secuencia de frames -> out/frame_0000.ppm ...
cargo run --release -- animate [ancho] [alto] [frames] [supersampling]
```

Valores por defecto:

| Modo      | Parámetros                                            |
|-----------|-------------------------------------------------------|
| `view`    | puerto `8080`                                         |
| `still`   | `1280x720`, supersampling `2` (4 rayos por píxel)     |
| `animate` | `800x450`, `120` frames, supersampling `1`            |

Armar el video con los frames generados:

```bash
ffmpeg -framerate 30 -i out/frame_%04d.ppm -c:v libx264 -pix_fmt yuv420p diorama.mp4
```

### Controles del visor

El visor levanta un servidor HTTP con `std::net` y sirve una página que pide un
frame nuevo (`GET /frame?yaw=…&pitch=…&dist=…`) codificado como BMP en cada
movimiento de cámara. Mientras la cámara se mueve baja la resolución y el
supersampling, y al soltar vuelve a renderizar en calidad completa.

| Acción                | Control                       |
|-----------------------|-------------------------------|
| Rotar (órbita)        | arrastrar con el mouse / `A` `D` |
| Cambiar altura        | `W` `S`                       |
| Zoom                  | rueda del mouse / `Q` `E`     |
| Reiniciar la cámara   | `R`                           |

## Características implementadas

- **Raytracing recursivo** con profundidad máxima 3 (`render::MAX_DEPTH`).
- **DDA voxel traversal** (algoritmo de Amanatides–Woo) sobre un `HashMap` de
  voxeles, con AABB previo para descartar rayos que no tocan el diorama.
- **Iluminación Phong** (ambiente + difuso + especular) con múltiples luces:
  sol direccional, luz naranja del pozo de lava y una luz por antorcha, con
  atenuación cuadrática para las luces puntuales.
- **Sombras** que respetan la transparencia: al cruzar agua o vidrio la sombra se
  tiñe y atenúa en lugar de bloquearse por completo.
- **Reflexión y refracción** con **Fresnel (aproximación de Schlick)**, índice de
  refracción por material y reflexión total interna como caso de respaldo.
- **Materiales emisivos** (lava) que iluminan la escena.
- **Texturas procedurales** de 32×32 generadas en código (pasto, tierra, piedra,
  madera, agua, lava, obsidiana) — sin archivos de imagen.
- **Normal mapping** derivado por Sobel a partir de mapas de altura, para piedra,
  madera y las olas del agua.
- **Skybox procedural**: degradado cenit/horizonte/suelo, nubes con fBm y disco
  solar. También soporta panorama equirectangular (`Sky::Panorama`).
- **Ruido propio**: hash entero, value noise y fBm, usados tanto para el terreno
  como para las texturas.
- **Render paralelo** con `std::thread::scope`, repartiendo bandas de filas entre
  los hilos disponibles (`available_parallelism`).
- **Antialiasing** por supersampling de rejilla (`ss × ss` rayos por píxel).
- **Escritura de imágenes propia**: BMP de 24 bits y PPM binario (P6), sin
  librerías de imagen.

## Materiales

| Material   | Propiedades destacadas                                  |
|------------|---------------------------------------------------------|
| Pasto      | difuso                                                  |
| Tierra     | difuso                                                  |
| Piedra     | normal map, reflectividad leve                           |
| Madera     | normal map, especular medio                              |
| Agua       | transparencia 0.88, IOR 1.33, normal map de olas, tinte  |
| Lava       | emisiva (×3.2)                                           |
| Obsidiana  | reflectividad 0.65, especular alto                       |
| Vidrio     | transparencia 0.92, IOR 1.52                             |

## Estructura del código

```
src/
├── main.rs      # CLI: view / still / animate, escritura de PPM
├── server.rs    # visor interactivo (servidor HTTP + página con JS embebido)
├── render.rs    # Scene, Light, trace(), sombras, render paralelo
├── world.rs     # grid de voxeles, AABB y recorrido DDA, UV por cara
├── scene.rs     # construcción del diorama: terreno, casa, lava, antorchas, árboles
├── material.rs  # Material (builder), Fresnel-Schlick, marco tangente
├── texture.rs   # texturas procedurales, normal maps, carga de PPM
├── sky.rs       # cielo procedural y panorama equirectangular
├── noise.rs     # hash, value noise, fBm
├── camera.rs    # cámara orbital (yaw / pitch / dist / fov)
├── bmp.rs       # codificación y escritura de BMP 24 bits
└── vec3.rs      # álgebra vectorial y conversión a RGB8
```

La escena se genera de forma determinista a partir de una semilla
(`scene::build(2026)`), sobre un terreno de 16×16 con nivel de mar en `y = 4`.

## Salida

Los archivos generados quedan en `out/`:

- `out/diorama.bmp`, `out/diorama.ppm` — modo `still`
- `out/frame_0000.ppm`, … — modo `animate`
