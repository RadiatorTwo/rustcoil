//! What moves, put between the last two logic frames while a picture is drawn (recomp_kit
//! GUIDE.md 8b, steps 3 and 4).
//!
//! Recoil's engine (Zipper's, as MechWarrior 3) keeps its scene as nodes (0xc4 bytes), every
//! node in one list (`ALL_NODES`), the camera alone in another (`CAMERAS`). A node's type is at
//! +0x34, its data at +0x38. The drawing (0x44f670) works from the nodes' own values each time:
//!
//! - an object (type 5): Euler angles at data+0x18, a 3x3 matrix at +0x30, its place at +0x54
//!   (an object moved while drawing is drawn there);
//! - the camera (type 1): place at data+0x14, Euler angles at +0x20, the same again at +0x2c and
//!   +0x38 (its matrices at +0x44 and +0x188 are made from them while drawing).
//!
//! After each logic frame (`noted`) these values are noted; a picture shows them a frame late,
//! at `alpha` of the way from the last frame's to the newest (`draw`), and puts the newest
//! back after. Something that jumps (respawn, a camera cut) is not interpolated.

use std::collections::HashMap;
use std::sync::Mutex;
use win32::platform::mem;
use win32::platform::tween::{Hist, from_quat, is_rotation, quat};

/// The node lists by type (`[LISTS + 4 * k]` points at a list's head pointer); a list's links
/// are { node, ?, next }.
const LISTS: u32 = 0x4e1ef8;
const ALL_NODES: u32 = 6;
const CAMERAS: u32 = 8;
const OBJECT: u32 = 5;
const CAMERA: u32 = 1;

/// A move larger than this in one logic frame (1/60 s) is a jump.
const JUMP: f32 = 200.0;

/// An object's values: Euler angles, matrix, place.
#[derive(Clone, Copy, PartialEq)]
struct Object {
    euler: [f32; 3],
    matrix: [f32; 9],
    place: [f32; 3],
}

/// The camera's values: place and Euler angles, twice.
#[derive(Clone, Copy, PartialEq)]
struct Camera {
    v: [[f32; 3]; 4],
}

/// Hashes a data address (a u32) by one multiplication: SipHash over ~8000 objects a logic
/// frame took ~1.6 ms with the rest of `noted`, the logic frame's picture came late.
#[derive(Default)]
struct AddrHash(u64);

impl std::hash::Hasher for AddrHash {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn write_u32(&mut self, v: u32) {
        self.0 = (v as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

type Objects = HashMap<u32, Hist<Object>, std::hash::BuildHasherDefault<AddrHash>>;

struct Noted {
    /// every object's values at the last two logic frames, by data address
    objects: Objects,
    /// the objects that moved in the newest logic frame: (data, before, now)
    moving: Vec<(u32, Object, Object)>,
    camera: Option<(u32, Hist<Camera>)>,
}

static NOTED: Mutex<Option<Noted>> = Mutex::new(None);

fn rdf(a: u32) -> f32 {
    f32::from_bits(mem().rd32(a))
}

fn wrf(a: u32, v: f32) {
    mem().wr32(a, v.to_bits())
}

fn vec<const N: usize>(a: u32) -> [f32; N] {
    std::array::from_fn(|k| rdf(a + 4 * k as u32))
}

fn put<const N: usize>(a: u32, v: &[f32; N]) {
    for (k, &x) in v.iter().enumerate() {
        wrf(a + 4 * k as u32, x);
    }
}

/// The nodes of list `k`.
fn nodes(k: u32) -> impl Iterator<Item = u32> {
    let m = mem();
    let head = m.rd32(LISTS + 4 * k);
    let mut link = if head == 0 { 0 } else { m.rd32(head) };
    std::iter::from_fn(move || {
        while link != 0 {
            let node = m.rd32(link);
            link = m.rd32(link + 8);
            if node != 0 {
                return Some(node);
            }
        }
        None
    })
}

impl Object {
    fn read(d: u32) -> Self {
        Object {
            euler: vec(d + 0x18),
            matrix: vec(d + 0x30),
            place: vec(d + 0x54),
        }
    }

    fn write(&self, d: u32) {
        put(d + 0x18, &self.euler);
        put(d + 0x30, &self.matrix);
        put(d + 0x54, &self.place);
    }
}

impl Camera {
    fn read(d: u32) -> Self {
        Camera {
            v: [vec(d + 0x14), vec(d + 0x20), vec(d + 0x2c), vec(d + 0x38)],
        }
    }

    fn write(&self, d: u32) {
        for (k, o) in [0x14, 0x20, 0x2c, 0x38].into_iter().enumerate() {
            put(d + o, &self.v[k]);
        }
    }
}

/// After logic frame `frame`: the objects' and the camera's values.
pub fn noted(frame: u64) {
    let m = mem();
    let mut g = NOTED.lock().unwrap();
    let n = g.get_or_insert_with(|| Noted {
        objects: Objects::default(),
        moving: Vec::new(),
        camera: None,
    });
    n.moving.clear();
    for node in nodes(ALL_NODES) {
        if m.rd32(node + 0x34) != OBJECT {
            continue;
        }
        let d = m.rd32(node + 0x38);
        if d == 0 {
            continue;
        }
        let now = Object::read(d);
        let h = n.objects.entry(d).or_insert(Hist {
            frame,
            cur: now,
            prev: None,
        });
        *h = Hist::note(Some(&*h), frame, now);
        if let Some(before) = h.prev.filter(|p| *p != now && !far(&p.place, &now.place)) {
            n.moving.push((d, before, now));
        }
    }
    // objects gone (their data may be another's now)
    n.objects.retain(|_, h| h.frame == frame);
    let old_camera = n.camera.take();
    n.camera = nodes(CAMERAS)
        .find(|&c| m.rd32(c + 0x34) == CAMERA && m.rd32(c + 0x38) != 0)
        .map(|c| {
            let d = m.rd32(c + 0x38);
            let h = old_camera.filter(|&(od, _)| od == d).map(|(_, h)| h);
            (d, Hist::note(h.as_ref(), frame, Camera::read(d)))
        });
}

/// Out of a mission: nothing noted.
pub fn forget() {
    *NOTED.lock().unwrap() = None;
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn mix3(a: &[f32; 3], b: &[f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|k| mix(a[k], b[k], t))
}

/// Angles in radians, the short way round.
fn mix_angles(a: &[f32; 3], b: &[f32; 3], t: f32) -> [f32; 3] {
    use std::f32::consts::{PI, TAU};
    std::array::from_fn(|k| {
        let d = (b[k] - a[k] + PI).rem_euclid(TAU) - PI;
        a[k] + d * t
    })
}

fn far(a: &[f32; 3], b: &[f32; 3]) -> bool {
    (0..3).map(|k| (b[k] - a[k]).powi(2)).sum::<f32>() > JUMP * JUMP
}

/// A matrix between two: by quaternions when both are rotations (mixed element by element a
/// turning one would shrink), else element by element.
fn mix_matrix(a: &[f32; 9], b: &[f32; 9], t: f32) -> [f32; 9] {
    let (ma, mb) = (a.map(f64::from), b.map(f64::from));
    if !(is_rotation(&ma, 0.02) && is_rotation(&mb, 0.02)) {
        return std::array::from_fn(|k| mix(a[k], b[k], t));
    }
    let (qa, mut qb) = (quat(&ma), quat(&mb));
    if (0..4).map(|i| qa[i] * qb[i]).sum::<f64>() < 0.0 {
        qb = qb.map(|v| -v);
    }
    let t = t as f64;
    let q: [f64; 4] = std::array::from_fn(|i| qa[i] + (qb[i] - qa[i]) * t);
    let n = q.iter().map(|v| v * v).sum::<f64>().sqrt();
    from_quat(q.map(|v| v / n)).map(|v| v as f32)
}

/// Draw (`f`) with what moved put `alpha` (of 256) of the way from the last logic frame to the
/// newest, then the newest back.
pub fn draw(alpha: Option<u32>, f: impl FnOnce()) {
    let g = NOTED.lock().unwrap();
    let Some((n, alpha)) = g.as_ref().zip(alpha) else {
        drop(g);
        return f();
    };
    let t = alpha as f32 / 256.0;
    let mut put_back = Vec::with_capacity(n.moving.len());
    for &(d, before, now) in &n.moving {
        Object {
            euler: mix_angles(&before.euler, &now.euler, t),
            matrix: mix_matrix(&before.matrix, &now.matrix, t),
            place: mix3(&before.place, &now.place, t),
        }
        .write(d);
        put_back.push((d, now));
    }
    let camera = n.camera.and_then(|(d, h)| {
        let prev = h
            .prev
            .filter(|p| *p != h.cur && !far(&p.v[0], &h.cur.v[0]))?;
        Camera {
            v: [
                mix3(&prev.v[0], &h.cur.v[0], t),
                mix_angles(&prev.v[1], &h.cur.v[1], t),
                mix3(&prev.v[2], &h.cur.v[2], t),
                mix_angles(&prev.v[3], &h.cur.v[3], t),
            ],
        }
        .write(d);
        Some((d, h.cur))
    });
    drop(g);
    f();
    for (d, o) in put_back {
        o.write(d);
    }
    if let Some((d, c)) = camera {
        c.write(d);
    }
}
