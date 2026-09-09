# Proyecto 2 — Diorama con Raytracing

Raytracer escrito **desde cero en Rust, sin dependencias externas** (`[dependencies]` vacío).
Renderiza un diorama estilo Minecraft construido con cubos texturizados: terreno
generado con ruido, un lago, una cabaña de madera con ventanas de vidrio, un pozo
de lava, antorchas de obsidiana y árboles.

Incluye tres modos de uso: imagen fija, visor interactivo en el navegador y
secuencia de frames para armar un video.

---

## Requisitos

- Rust estable (edición 2024) — `cargo`
- Opcional: `ffmpeg`, solo para convertir los frames del modo `animate` en video

El perfil `release` está configurado para rendimiento (`opt-level = 3`,
`lto = "fat"`, `codegen-units = 1`), así que **siempre conviene correr con
`--release`**: en modo debug el render es órdenes de magnitud más lento.

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

En `animate` la cámara da una vuelta completa alrededor del diorama mientras se
acerca y se aleja. Armar el video con los frames generados:

```bash
ffmpeg -framerate 30 -i out/frame_%04d.ppm -c:v libx264 -pix_fmt yuv420p diorama.mp4
```

### Controles del visor

El visor levanta un servidor HTTP con `std::net` y sirve una página que pide un
frame nuevo en cada movimiento de cámara
(`GET /frame?yaw=…&pitch=…&dist=…&w=…&h=…&ss=…&depth=…`), codificado como BMP.
Mientras la cámara se mueve pide un **borrador** (40 % de la resolución, sin
antialiasing y con un solo rebote); 250 ms después de soltar vuelve a pedir el
frame en calidad final (960×540, `ss = 2`, 3 rebotes).

| Acción                | Control                          |
|-----------------------|----------------------------------|
| Rotar (órbita)        | arrastrar con el mouse / `A` `D` |
| Cambiar altura        | `W` `S`                          |
| Zoom                  | rueda del mouse / `Q` `E`        |
| Reiniciar la cámara   | `R`                              |

## Características implementadas

- **Raytracing recursivo** con profundidad configurable por render
  (`RenderOpts::max_depth`, por defecto `render::MAX_DEPTH = 3`).
- **DDA voxel traversal** (algoritmo de Amanatides–Woo) sobre una **rejilla densa
  de voxeles** (`Vec<u8>`), con slab test contra el AABB de las celdas ocupadas
  para entrar y salir rápido.
- **Iluminación Blinn-Phong** (ambiente + difuso + especular con vector medio)
  con múltiples luces: sol lejano sin atenuación, luz naranja del pozo de lava y
  una luz por antorcha, con atenuación cuadrática para las luces puntuales.
- **Sombras** que respetan la transparencia: al cruzar agua o vidrio la sombra se
  tiñe y atenúa en lugar de bloquearse por completo.
- **Reflexión y refracción** con **Fresnel (aproximación de Schlick)**, índice de
  refracción por material y reflexión total interna como caso de respaldo.
- **Materiales emisivos** (lava) que iluminan la escena.
- **Texturas procedurales** de 32×32 generadas en código (pasto, tierra, piedra,
  madera, agua, lava, obsidiana) — sin archivos de imagen.
- **Normal mapping** derivado por filtro Sobel a partir de **mapas de altura
  suaves** (no de la textura de color), para piedra, madera y las olas del agua.
- **Skybox procedural**: degradado cenit/horizonte/suelo, nubes con fBm y disco
  solar. También soporta panorama equirectangular (`Sky::Panorama`).
- **Ruido propio**: hash entero, value noise y fBm, usados tanto para el terreno
  como para las texturas.
- **Render paralelo** con `std::thread::scope`, repartiendo bandas contiguas de
  filas entre los hilos disponibles (`available_parallelism`), sin locks.
- **Antialiasing** por supersampling de rejilla (`ss × ss` rayos por píxel).
- **Lectura y escritura de imágenes propias**: BMP de 24 bits, PPM binario (P6) y
  un decodificador de PPM para texturas externas, sin librerías de imagen.

## Optimizaciones

El recorrido de rayos es el bucle más caliente del programa, así que el trabajo
está puesto ahí:

- **Rejilla densa en vez de `HashMap`**: consultar una celda es una multiplicación
  y un indexado, sin hashing.
- **AABB previo (slab test)**: los rayos que no tocan el diorama se descartan
  antes de empezar el DDA, y el recorrido se corta al salir de la caja.
- **Tope de pasos** (`world::MAX_STEPS = 192`) como red de seguridad para rayos
  rasantes.
- **Descarte de luces débiles**: si el aporte de una luz cae bajo un umbral, ni
  siquiera se lanza el rayo de sombra (las antorchas lejanas salen gratis).
- **Corte temprano de sombras**: la atenuación acumulada se aborta al bajar del
  2 %, con un máximo de 3 medios transparentes atravesados.
- **Material ignorado en refracción**: el rayo que viaja *dentro* del agua o del
  vidrio salta ese material para no refractar en cada celda vecina del bloque.
- **Calidad adaptativa en el visor**: resolución, supersampling y rebotes bajan
  mientras la cámara se mueve y vuelven a subir al soltar.

## Materiales

| Material   | Propiedades destacadas                                   |
|------------|----------------------------------------------------------|
| Pasto      | difuso                                                   |
| Tierra     | difuso                                                   |
| Piedra     | normal map, reflectividad leve (0.03)                    |
| Madera     | normal map, especular medio                              |
| Agua       | transparencia 0.88, IOR 1.33, normal map de olas, tinte  |
| Lava       | emisiva (×3.2)                                           |
| Obsidiana  | reflectividad 0.65, especular alto                       |
| Vidrio     | transparencia 0.92, IOR 1.52, reflectividad 0.08         |

El índice de cada material dentro de `Scene::materials` **debe** coincidir con
las constantes `M_*` de `scene.rs`: ese índice es el byte que se guarda en cada
voxel de la rejilla.

## Estructura del código

```
src/
├── main.rs      # CLI: view / still / animate, escritura de PPM
├── server.rs    # visor interactivo (servidor HTTP + página con JS embebido)
├── render.rs    # Scene, Light, RenderOpts, trace(), sombras, render paralelo
├── world.rs     # rejilla densa de voxeles, AABB y recorrido DDA, UV por cara
├── scene.rs     # construcción del diorama: terreno, casa, lava, antorchas, árboles
├── material.rs  # Material (builder), Fresnel-Schlick, marco tangente
├── texture.rs   # texturas procedurales, mapas de altura, normal maps, carga de PPM
├── sky.rs       # cielo procedural y panorama equirectangular
├── noise.rs     # hash, value noise, fBm
├── camera.rs    # cámara orbital (yaw / pitch / dist / fov)
├── bmp.rs       # codificación y escritura de BMP 24 bits
└── vec3.rs      # álgebra vectorial y conversión a RGB8
```

La escena se genera de forma determinista a partir de una semilla
(`scene::build(2026)`), sobre un terreno de 16×16 con nivel de mar en `y = 4`.
La rejilla se reserva con margen (de `(-2, 0, -2)` a `(19, 24, 19)`) para que
entren las copas de los árboles y las estructuras.

## Salida

Los archivos generados quedan en `out/` (ignorado por git):

- `out/diorama.bmp`, `out/diorama.ppm` — modo `still`
- `out/frame_0000.ppm`, … — modo `animate`
