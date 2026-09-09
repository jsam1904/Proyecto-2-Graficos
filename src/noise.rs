fn hash_u32(mut a: u32) -> u32 {
    a ^= a >> 16;
    a = a.wrapping_mul(0x7feb_352d);
    a ^= a >> 15;
    a = a.wrapping_mul(0x846c_a68b);
    a ^= a >> 16;
    a
}

pub fn hash21(x: i32, y: i32, seed: u32) -> f32 {
    let h = hash_u32(
        (x as u32)
            .wrapping_mul(0x9e37_79b9)
            .wrapping_add((y as u32).wrapping_mul(0x85eb_ca6b))
            .wrapping_add(seed),
    );
    (h >> 8) as f32 / 16_777_216.0
}

pub fn hash31(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let h = hash_u32(
        (x as u32)
            .wrapping_mul(0x9e37_79b9)
            .wrapping_add((y as u32).wrapping_mul(0x85eb_ca6b))
            .wrapping_add((z as u32).wrapping_mul(0xc2b2_ae35))
            .wrapping_add(seed),
    );
    (h >> 8) as f32 / 16_777_216.0
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

pub fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let fx = smooth(x - xi);
    let fy = smooth(y - yi);
    let (ix, iy) = (xi as i32, yi as i32);

    let a = hash21(ix, iy, seed);
    let b = hash21(ix + 1, iy, seed);
    let c = hash21(ix, iy + 1, seed);
    let d = hash21(ix + 1, iy + 1, seed);

    let top = a + (b - a) * fx;
    let bottom = c + (d - c) * fx;
    top + (bottom - top) * fy
}

pub fn fbm(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += value_noise(x * freq, y * freq, seed.wrapping_add(o * 7919)) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    if norm > 0.0 {
        sum / norm
    } else {
        0.0
    }
}