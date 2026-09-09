use crate::vec3::{v3, Vec3};
use std::collections::HashMap;

const EPS: f32 = 1e-4;
const MAX_STEPS: usize = 512;

pub struct Hit {
    pub t: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub u: f32,
    pub v: f32,
    pub mat: u8,
    pub cell: (i32, i32, i32),
}

pub struct World {
    pub voxels: HashMap<(i32, i32, i32), u8>,
    pub min: (i32, i32, i32),
    pub max: (i32, i32, i32),
}

impl World {
    pub fn new() -> World {
        World {
            voxels: HashMap::new(),
            min: (i32::MAX, i32::MAX, i32::MAX),
            max: (i32::MIN, i32::MIN, i32::MIN),
        }
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, mat: u8) {
        self.voxels.insert((x, y, z), mat);
        self.min = (self.min.0.min(x), self.min.1.min(y), self.min.2.min(z));
        self.max = (self.max.0.max(x), self.max.1.max(y), self.max.2.max(z));
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        self.voxels.get(&(x, y, z)).copied()
    }

    pub fn len(&self) -> usize {
        self.voxels.len()
    }

    fn bounds_hit(&self, ro: Vec3, rd: Vec3) -> Option<(f32, f32, usize)> {
        let bmin = v3(self.min.0 as f32, self.min.1 as f32, self.min.2 as f32);
        let bmax = v3(
            (self.max.0 + 1) as f32,
            (self.max.1 + 1) as f32,
            (self.max.2 + 1) as f32,
        );
        let o = [ro.x, ro.y, ro.z];
        let d = [rd.x, rd.y, rd.z];
        let lo = [bmin.x, bmin.y, bmin.z];
        let hi = [bmax.x, bmax.y, bmax.z];

        let mut t0 = f32::NEG_INFINITY;
        let mut t1 = f32::INFINITY;
        let mut axis = 0usize;

        for i in 0..3 {
            if d[i].abs() < 1e-9 {
                if o[i] < lo[i] || o[i] > hi[i] {
                    return None;
                }
                continue;
            }
            let inv = 1.0 / d[i];
            let mut ta = (lo[i] - o[i]) * inv;
            let mut tb = (hi[i] - o[i]) * inv;
            if ta > tb {
                std::mem::swap(&mut ta, &mut tb);
            }
            if ta > t0 {
                t0 = ta;
                axis = i;
            }
            if tb < t1 {
                t1 = tb;
            }
            if t0 > t1 {
                return None;
            }
        }
        if t1 < 0.0 {
            return None;
        }
        Some((t0, t1, axis))
    }

    pub fn traverse(&self, ro: Vec3, rd: Vec3, t_max: f32, ignore: Option<u8>) -> Option<Hit> {
        if self.voxels.is_empty() {
            return None;
        }
        let (tb0, tb1, axis0) = self.bounds_hit(ro, rd)?;
        let t_enter = tb0.max(0.0);
        if t_enter > t_max {
            return None;
        }
        let t_exit = tb1.min(t_max);

        let origin_t = t_enter + EPS;
        let p = ro + rd * origin_t;
        let mut cell = (
            p.x.floor() as i32,
            p.y.floor() as i32,
            p.z.floor() as i32,
        );

        let sx = if rd.x >= 0.0 { 1 } else { -1 };
        let sy = if rd.y >= 0.0 { 1 } else { -1 };
        let sz = if rd.z >= 0.0 { 1 } else { -1 };

        let next_t = |pc: f32, dc: f32, c: i32, s: i32| -> f32 {
            if dc.abs() < 1e-9 {
                f32::INFINITY
            } else {
                let boundary = if s > 0 { (c + 1) as f32 } else { c as f32 };
                origin_t + (boundary - pc) / dc
            }
        };
        let delta = |dc: f32| -> f32 {
            if dc.abs() < 1e-9 {
                f32::INFINITY
            } else {
                (1.0 / dc).abs()
            }
        };

        let mut tnx = next_t(p.x, rd.x, cell.0, sx);
        let mut tny = next_t(p.y, rd.y, cell.1, sy);
        let mut tnz = next_t(p.z, rd.z, cell.2, sz);
        let (tdx, tdy, tdz) = (delta(rd.x), delta(rd.y), delta(rd.z));

        let mut t_cur = t_enter;
        let mut axis = axis0;

        for _ in 0..MAX_STEPS {
            let voxel = self.voxels.get(&cell).copied();
            if let Some(mat) = voxel.filter(|m| Some(*m) != ignore) {
                let normal = match axis {
                    0 => v3(-(sx as f32), 0.0, 0.0),
                    1 => v3(0.0, -(sy as f32), 0.0),
                    _ => v3(0.0, 0.0, -(sz as f32)),
                };
                let point = ro + rd * t_cur;
                let (u, v) = face_uv(point, cell, normal);
                return Some(Hit {
                    t: t_cur,
                    point,
                    normal,
                    u,
                    v,
                    mat,
                    cell,
                });
            }

            if tnx <= tny && tnx <= tnz {
                cell.0 += sx;
                t_cur = tnx;
                tnx += tdx;
                axis = 0;
            } else if tny <= tnz {
                cell.1 += sy;
                t_cur = tny;
                tny += tdy;
                axis = 1;
            } else {
                cell.2 += sz;
                t_cur = tnz;
                tnz += tdz;
                axis = 2;
            }

            if t_cur > t_exit {
                return None;
            }
        }
        None
    }
}

fn face_uv(point: Vec3, cell: (i32, i32, i32), n: Vec3) -> (f32, f32) {
    let lx = (point.x - cell.0 as f32).clamp(0.0, 1.0);
    let ly = (point.y - cell.1 as f32).clamp(0.0, 1.0);
    let lz = (point.z - cell.2 as f32).clamp(0.0, 1.0);

    if n.x != 0.0 {
        let u = if n.x > 0.0 { 1.0 - lz } else { lz };
        (u, 1.0 - ly)
    } else if n.y != 0.0 {
        let v = if n.y > 0.0 { lz } else { 1.0 - lz };
        (lx, v)
    } else {
        let u = if n.z > 0.0 { lx } else { 1.0 - lx };
        (u, 1.0 - ly)
    }
}