//! What moves, put between the last two logic frames while a picture is drawn (recomp_kit
//! GUIDE.md 8b, steps 3 and 4).
//!
//! Recoil's engine (Zipper's, as MechWarrior 3) keeps its scene as nodes (0xc4 bytes), every
//! node in one list (`ALL_NODES`), the camera alone in another (`CAMERAS`). A node's type is at
//! +0x34, its data at +0x38. The drawing (0x44f670) works from the nodes' own values each time:
//!
//! - an object (type 5): Euler angles at data+0x18, a 3x3 matrix at +0x30, its place at +0x54,
//!   and the same in the world at +0x60 and +0x84, from which the drawing draws. A root
//!   object's (its parent the world: the tank, other vehicles) the logic writes. A child's
//!   (the tank's parts) the drawing makes from its parent's, only while the logic marked the
//!   node changed (node+0x2c bit 4), so in a logic frame's picture alone: world matrix = own
//!   matrix x parent's world matrix, world place = own place x parent's world matrix +
//!   parent's world place (row vectors; exactly the drawing's). Some children's world values
//!   the logic sets itself (effects: exhaust puffs); they do not fit that rule and are left.
//!
//!   A picture puts the own values of what moved between the logic frames, a root's world
//!   values too, and makes the children's world values from them itself (`derived`); after,
//!   it puts back the newest values and makes the children's from those (what the logic
//!   frame's own drawing would have made). Interpolating only the own values, the tank stood
//!   still in the pictures between and was a frame ahead in each logic frame's: it lagged
//!   behind and jumped ahead, the surroundings did not. Marking nodes changed in a picture did
//!   not make the drawing make children again (only their bounding spheres), and putting back
//!   world values noted at the logic frame put back a child's from the frame before: objects
//!   and pictures drifted apart;
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

/// A node's children and parents: count, array of node pointers.
const CHILDREN: u32 = 0x5c;
const PARENTS: u32 = 0x54;
/// An object's world matrix (9 floats) and place (at +0x84).
const WORLD: u32 = 0x60;
/// A child's world values made by the rule differ from the drawing's last ones at most by a
/// logic frame's move. Not enough to tell an effect's (set by the logic each frame) apart:
/// with that test alone the exhaust trails were stretched across the picture. A child whose
/// world values the logic left as they were is the drawing's (`watched`); one the logic set
/// exactly by the rule follows its parent too.
const DERIVED_MATRIX: f32 = 0.25;
const DERIVED_PLACE: f32 = 3.0;
/// A child whose world values the logic makes by the rule itself (rounding apart).
const EXACT_MATRIX: f32 = 0.001;
const EXACT_PLACE: f32 = 0.01;

/// A move larger than this in one logic frame (1/60 s) is a jump.
const JUMP: f32 = 200.0;

/// An object's values: Euler angles, matrix, place.
#[derive(Clone, Copy, PartialEq)]
struct Object {
    euler: [f32; 3],
    matrix: [f32; 9],
    place: [f32; 3],
    /// the world's matrix and place (`WORLD`): the logic's for a root object
    world: [f32; 12],
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
    /// the objects that moved in the newest logic frame: (node, data, root, before, now)
    moving: Vec<(u32, u32, bool, Object, Object)>,
    /// children whose world values are made from their parent's, below what moved: (data,
    /// parent's data), parents before children
    derived: Vec<(u32, u32)>,
    /// the children below what moved, and their world values right before the logic frame
    watched: Vec<u32>,
    before_logic: HashMap<u32, [f32; 12], std::hash::BuildHasherDefault<AddrHash>>,
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
            world: vec(d + WORLD),
        }
    }

    /// The object's own values (the world's: `write_world`).
    fn write(&self, d: u32) {
        put(d + 0x18, &self.euler);
        put(d + 0x30, &self.matrix);
        put(d + 0x54, &self.place);
    }

    fn write_world(&self, d: u32) {
        put(d + WORLD, &self.world);
    }

    fn mix(&self, b: &Object, t: f32) -> Object {
        let (wa, wb) = (&self.world, &b.world);
        let m = mix_matrix(wa[..9].try_into().unwrap(), wb[..9].try_into().unwrap(), t);
        let p = mix3(wa[9..].try_into().unwrap(), wb[9..].try_into().unwrap(), t);
        let mut world = [0.0; 12];
        world[..9].copy_from_slice(&m);
        world[9..].copy_from_slice(&p);
        Object {
            euler: mix_angles(&self.euler, &b.euler, t),
            matrix: mix_matrix(&self.matrix, &b.matrix, t),
            place: mix3(&self.place, &b.place, t),
            world,
        }
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
        derived: Vec::new(),
        watched: Vec::new(),
        before_logic: HashMap::default(),
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
            n.moving.push((
                node,
                d,
                is_root(node) && own_world(&before) && own_world(&now),
                before,
                now,
            ));
        }
    }
    // objects gone (their data may be another's now)
    n.objects.retain(|_, h| h.frame == frame);
    let mut derived: Vec<(u32, u32, u32)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let before = std::mem::take(&mut n.before_logic);
    for &(node, ..) in &n.moving {
        derive_below(node, depth(node), &before, &mut derived, &mut seen);
    }
    derived.sort_by_key(|x| x.2);
    n.derived = derived.into_iter().map(|(d, p, _)| (d, p)).collect();
    n.watched = seen.into_iter().collect();
    let old_camera = n.camera.take();
    n.camera = nodes(CAMERAS)
        .find(|&c| m.rd32(c + 0x34) == CAMERA && m.rd32(c + 0x38) != 0)
        .map(|c| {
            let d = m.rd32(c + 0x38);
            let h = old_camera.filter(|&(od, _)| od == d).map(|(_, h)| h);
            (d, Hist::note(h.as_ref(), frame, Camera::read(d)))
        });
}

/// Right before a logic frame: the watched children's world values (`DERIVED_MATRIX`).
pub fn before_logic() {
    let mut g = NOTED.lock().unwrap();
    if let Some(n) = g.as_mut() {
        let watched = std::mem::take(&mut n.watched);
        n.before_logic.clear();
        for &d in &watched {
            n.before_logic.insert(d, vec(d + WORLD));
        }
        n.watched = watched;
    }
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

/// Whether an object's world values are the logic's: its parent is no object (the world). A
/// child's the drawing makes from its parent's.
fn is_root(node: u32) -> bool {
    let m = mem();
    let (count, parents) = (m.rd32(node + PARENTS), m.rd32(node + PARENTS + 4));
    count == 0 || parents == 0 || {
        let parent = m.rd32(parents);
        parent == 0 || m.rd32(parent + 0x34) != OBJECT
    }
}

/// Whether a root object's world values are its own (the tank's): an effect's (exhaust trails)
/// the logic sets apart from them, and interpolated they were stretched across the picture.
fn own_world(o: &Object) -> bool {
    (0..9).all(|i| (o.world[i] - o.matrix[i]).abs() < EXACT_MATRIX)
        && (0..3).all(|i| (o.world[9 + i] - o.place[i]).abs() < EXACT_PLACE)
}

/// How many object parents a node has above it.
fn depth(mut node: u32) -> u32 {
    let m = mem();
    for k in 0..16 {
        let (count, parents) = (m.rd32(node + PARENTS), m.rd32(node + PARENTS + 4));
        let parent = if count == 0 || parents == 0 {
            0
        } else {
            m.rd32(parents)
        };
        if parent == 0 || m.rd32(parent + 0x34) != OBJECT {
            return k;
        }
        node = parent;
    }
    16
}

/// A child's world values by the drawing's rule from its parent's (data addresses).
fn world_of(d: u32, parent: u32) -> [f32; 12] {
    rule(&vec(parent + WORLD), &vec(d + 0x30), &vec(d + 0x54))
}

/// The drawing's rule: a child's world values from its parent's (`pw`) and its own matrix and
/// place.
fn rule(pw: &[f32; 12], cm: &[f32; 9], cp: &[f32; 3]) -> [f32; 12] {
    let (pm, pp) = (&pw[..9], &pw[9..]);
    let mut w = [0.0f32; 12];
    for i in 0..3 {
        for j in 0..3 {
            w[3 * i + j] = (0..3).map(|k| cm[3 * i + k] * pm[3 * k + j]).sum();
        }
    }
    for j in 0..3 {
        w[9 + j] = (0..3).map(|k| cp[k] * pm[3 * k + j]).sum::<f32>() + pp[j];
    }
    w
}

/// The children below `node` (at `depth`) whose world values follow the rule, noted with
/// their parent's data and their depth.
fn derive_below(
    node: u32,
    depth: u32,
    before: &HashMap<u32, [f32; 12], std::hash::BuildHasherDefault<AddrHash>>,
    out: &mut Vec<(u32, u32, u32)>,
    seen: &mut std::collections::HashSet<u32>,
) {
    let m = mem();
    let (count, array) = (m.rd32(node + CHILDREN), m.rd32(node + CHILDREN + 4));
    let pd = m.rd32(node + 0x38);
    if array == 0 || count > 256 || depth >= 16 || pd == 0 {
        return;
    }
    for k in 0..count {
        let child = m.rd32(array + 4 * k);
        if child == 0 || m.rd32(child + 0x34) != OBJECT {
            continue;
        }
        let d = m.rd32(child + 0x38);
        if d == 0 || !seen.insert(d) {
            continue;
        }
        let (want, now): ([f32; 12], [f32; 12]) = (world_of(d, pd), vec(d + WORLD));
        let fits = (0..9).all(|i| (want[i] - now[i]).abs() < DERIVED_MATRIX)
            && (9..12).all(|i| (want[i] - now[i]).abs() < DERIVED_PLACE);
        // left by the logic: the drawing's (an effect's the logic moves); or set by the logic
        // by the rule itself (the tank's hull below it)
        let left = before.get(&d) == Some(&now);
        let exact = (0..9).all(|i| (want[i] - now[i]).abs() < EXACT_MATRIX)
            && (9..12).all(|i| (want[i] - now[i]).abs() < EXACT_PLACE);
        if fits && (left || exact) {
            out.push((d, pd, depth + 1));
        }
        if fits {
            derive_below(child, depth + 1, before, out, seen);
        }
    }
}

/// Draw (`f`) with what moved put `alpha` (of 256) of the way from the last logic frame to the
/// newest, then the newest back; the children's world values made from either.
pub fn draw(alpha: Option<u32>, f: impl FnOnce()) {
    let g = NOTED.lock().unwrap();
    let Some((n, alpha)) = g.as_ref().zip(alpha) else {
        drop(g);
        return f();
    };
    let t = alpha as f32 / 256.0;
    let mut put_back = Vec::with_capacity(n.moving.len());
    for &(_, d, root, before, now) in &n.moving {
        let shown = before.mix(&now, t);
        shown.write(d);
        if root {
            shown.write_world(d);
        }
        put_back.push((d, root, now));
    }
    for &(d, parent) in &n.derived {
        put(d + WORLD, &world_of(d, parent));
    }
    let derived = n.derived.clone();
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
    for (d, root, o) in put_back {
        o.write(d);
        if root {
            o.write_world(d);
        }
    }
    for (d, parent) in derived {
        put(d + WORLD, &world_of(d, parent));
    }
    if let Some((d, c)) = camera {
        c.write(d);
    }
}
