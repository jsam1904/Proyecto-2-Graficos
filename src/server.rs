//! Visor interactivo sin librerias externas.
//!
//! Levanta un servidor HTTP con `std::net` que sirve una pagina y, en cada
//! peticion a /frame, renderiza el diorama con los parametros de camara y de
//! calidad que manda el navegador. Mientras la camara se mueve el navegador
//! pide frames chicos y con menos rebotes; al soltar pide el frame completo.

use crate::bmp;
use crate::camera::Camera;
use crate::render::{self, RenderOpts, Scene};
use crate::vec3::Vec3;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

pub fn serve(scene: &Scene, center: Vec3, addr: &str, threads: usize) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    println!("\nVisor listo -> http://{}", addr);
    println!("Abrilo en el navegador de Windows. Ctrl+C para salir.\n");

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                if let Err(e) = handle(s, scene, center, threads) {
                    // Conexiones abortadas por el navegador son normales.
                    if e.kind() != std::io::ErrorKind::BrokenPipe {
                        eprintln!("error atendiendo peticion: {}", e);
                    }
                }
            }
            Err(e) => eprintln!("error de conexion: {}", e),
        }
    }
    Ok(())
}

fn handle(
    mut stream: TcpStream,
    scene: &Scene,
    center: Vec3,
    threads: usize,
) -> std::io::Result<()> {
    stream.set_nodelay(true).ok();
    let mut reader = BufReader::new(stream.try_clone()?);

    // Linea de peticion: "GET /ruta?query HTTP/1.1"
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    // Consumir el resto de encabezados.
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 || line == "\r\n" || line == "\n" {
            break;
        }
    }

    let path = request_line.split_whitespace().nth(1).unwrap_or("/");

    if path.starts_with("/frame") {
        let query = path.splitn(2, '?').nth(1).unwrap_or("");
        let get = |key: &str, default: f32| -> f32 {
            for pair in query.split('&') {
                let mut it = pair.splitn(2, '=');
                if it.next() == Some(key) {
                    if let Some(v) = it.next() {
                        if let Ok(parsed) = v.parse::<f32>() {
                            return parsed;
                        }
                    }
                }
            }
            default
        };

        let w = (get("w", 800.0) as usize).clamp(64, 1920);
        let h = (get("h", 450.0) as usize).clamp(48, 1080);
        let opts = RenderOpts {
            samples: (get("ss", 1.0) as usize).clamp(1, 4),
            max_depth: (get("depth", 3.0) as u32).clamp(0, 5),
            threads,
        };

        let mut cam = Camera::new(center, get("dist", 27.0).clamp(6.0, 80.0));
        cam.yaw = get("yaw", 0.85);
        cam.pitch = get("pitch", 0.48).clamp(-1.35, 1.45);

        let t0 = std::time::Instant::now();
        let buf = render::render_parallel(scene, &cam, w, h, opts);
        let image = bmp::encode_bmp(w, h, &buf);
        println!(
            "frame {}x{} ss={} depth={} en {:.0} ms",
            w,
            h,
            opts.samples,
            opts.max_depth,
            t0.elapsed().as_secs_f32() * 1000.0
        );

        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/bmp\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
            image.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(&image)?;
    } else {
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            PAGE.len()
        );
        stream.write_all(header.as_bytes())?;
        stream.write_all(PAGE.as_bytes())?;
    }

    stream.flush()
}

const PAGE: &str = r#"<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<title>Diorama Raytracer</title>
<style>
  body { margin:0; background:#0e0f12; color:#dcdfe4;
         font-family: system-ui, sans-serif; display:flex;
         flex-direction:column; align-items:center; gap:10px; padding:14px; }
  /* Tamano FIJO: aunque el frame venga a media resolucion, la imagen se
     escala a la misma caja y el visor no cambia de tamano al moverse. */
  #view { width:min(960px, 94vw); aspect-ratio:16/9; object-fit:fill;
          display:block; background:#000; border-radius:6px;
          cursor:grab; touch-action:none; user-select:none; }
  #view.drag { cursor:grabbing; }
  .hud { font-size:13px; opacity:.85; text-align:center; line-height:1.8; }
  kbd { background:#22242a; border:1px solid #3a3d45; border-radius:4px;
        padding:1px 5px; font-size:12px; }
</style>
</head>
<body>
  <img id="view" alt="diorama">
  <div class="hud">
    <kbd>arrastrar</kbd> rotar &middot; <kbd>rueda</kbd> zoom &middot;
    <kbd>A</kbd><kbd>D</kbd> girar &middot; <kbd>W</kbd><kbd>S</kbd> altura &middot;
    <kbd>Q</kbd><kbd>E</kbd> zoom &middot; <kbd>R</kbd> reiniciar<br>
    <span id="info">cargando...</span>
  </div>
<script>
const view = document.getElementById('view');
const info = document.getElementById('info');
const HOME = { yaw: 0.85, pitch: 0.38, dist: 27 };

// Resolucion base del frame final y factor para el modo movimiento.
const FULL_W = 960, FULL_H = 540, DRAFT = 0.40;

let st = Object.assign({}, HOME);
let moving = false, inflight = false, queued = false, lastUrl = '', idleTimer = null;

function frameUrl() {
  const w = moving ? Math.round(FULL_W * DRAFT) : FULL_W;
  const h = moving ? Math.round(FULL_H * DRAFT) : FULL_H;
  const ss = moving ? 1 : 2;          // antialiasing solo en el frame final
  const depth = moving ? 1 : 3;       // menos rebotes mientras se mueve
  return '/frame?yaw=' + st.yaw.toFixed(4) + '&pitch=' + st.pitch.toFixed(4) +
         '&dist=' + st.dist.toFixed(3) + '&w=' + w + '&h=' + h +
         '&ss=' + ss + '&depth=' + depth;
}

function request() {
  if (inflight) { queued = true; return; }
  const url = frameUrl();
  if (url === lastUrl) return;        // nada cambio: no pedir de nuevo
  inflight = true; queued = false; lastUrl = url;

  const t0 = performance.now();
  const next = new Image();
  next.onload = () => {
    view.src = next.src;
    inflight = false;
    info.textContent = 'dist ' + st.dist.toFixed(1) + '  ·  ' +
                       (moving ? 'borrador' : 'calidad final') + '  ·  ' +
                       Math.round(performance.now() - t0) + ' ms';
    if (queued) request();
  };
  next.onerror = () => { inflight = false; if (queued) request(); };
  next.src = url;
}

// Un solo request por cuadro de animacion: el arrastre genera decenas de
// eventos por segundo y no tiene sentido encolarlos todos.
let scheduled = false;
function touch() {
  if (scheduled) return;
  scheduled = true;
  requestAnimationFrame(() => { scheduled = false; request(); });
}

function settle() {
  clearTimeout(idleTimer);
  idleTimer = setTimeout(() => { moving = false; touch(); }, 250);
}

let dragging = false, lx = 0, ly = 0;
view.addEventListener('pointerdown', e => {
  dragging = true; moving = true; lx = e.clientX; ly = e.clientY;
  view.classList.add('drag'); view.setPointerCapture(e.pointerId);
});
view.addEventListener('pointermove', e => {
  if (!dragging) return;
  st.yaw -= (e.clientX - lx) * 0.010;
  st.pitch = Math.max(-1.3, Math.min(1.4, st.pitch + (e.clientY - ly) * 0.006));
  lx = e.clientX; ly = e.clientY;
  touch();
});
window.addEventListener('pointerup', () => {
  if (!dragging) return;
  dragging = false; view.classList.remove('drag'); settle();
});
view.addEventListener('wheel', e => {
  e.preventDefault();
  st.dist = Math.max(8, Math.min(75, st.dist + e.deltaY * 0.02));
  moving = true; touch(); settle();
}, { passive: false });
window.addEventListener('keydown', e => {
  const k = e.key.toLowerCase();
  if (k === 'a') st.yaw -= 0.09;
  else if (k === 'd') st.yaw += 0.09;
  else if (k === 'w') st.pitch = Math.min(1.4, st.pitch + 0.06);
  else if (k === 's') st.pitch = Math.max(-1.3, st.pitch - 0.06);
  else if (k === 'q') st.dist = Math.min(70, st.dist + 1.5);
  else if (k === 'e') st.dist = Math.max(6, st.dist - 1.5);
  else if (k === 'r') st = Object.assign({}, HOME);
  else return;
  e.preventDefault(); moving = true; touch(); settle();
});

request();
</script>
</body>
</html>"#;