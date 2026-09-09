//! Escritura de BMP de 24 bits. Windows y los navegadores lo abren directamente,
//! asi que sirve tanto para revisar el render como para el visor interactivo.

use crate::vec3::Vec3;
use std::fs::File;
use std::io::{self, BufWriter, Write};

/// Codifica el framebuffer como BMP en memoria.
pub fn encode_bmp(w: usize, h: usize, buf: &[Vec3]) -> Vec<u8> {
    let row_bytes = w * 3;
    let padding = (4 - row_bytes % 4) % 4; // cada fila se alinea a 4 bytes
    let data_size = (row_bytes + padding) * h;
    let file_size = 54 + data_size;

    let mut out: Vec<u8> = Vec::with_capacity(file_size);

    // BITMAPFILEHEADER (14 bytes)
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());

    // BITMAPINFOHEADER (40 bytes)
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // planos
    out.extend_from_slice(&24u16.to_le_bytes()); // bits por pixel
    out.extend_from_slice(&0u32.to_le_bytes()); // sin compresion
    out.extend_from_slice(&(data_size as u32).to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&2835i32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    // BMP guarda las filas de abajo hacia arriba y en orden BGR.
    for y in (0..h).rev() {
        for x in 0..w {
            let [r, g, b] = buf[y * w + x].to_rgb8();
            out.extend_from_slice(&[b, g, r]);
        }
        for _ in 0..padding {
            out.push(0);
        }
    }

    out
}

pub fn save_bmp(path: &str, w: usize, h: usize, buf: &[Vec3]) -> io::Result<()> {
    let bytes = encode_bmp(w, h, buf);
    let mut out = BufWriter::new(File::create(path)?);
    out.write_all(&bytes)?;
    out.flush()
}