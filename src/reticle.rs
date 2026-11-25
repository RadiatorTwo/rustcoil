//! The aiming reticle, drawn by us instead of the game's sprite: sharp at any resolution,
//! replaceable, and visible beside the 4:3 columns in a wide picture (the game's sprite is
//! pixels in its 640 x 480 frame buffer and is not drawn beyond it).
//!
//! The game's reticle is a HUD sprite (object 0x4ea0f4) drawn by the sprites' draw 0x4b7790
//! (it blits the sprite's picture into the locked frame buffer). 0x411300 places it: its
//! centre (screen pixels) at 0x4ea0dc / 0x4ea0e0, its half size at 0x4ea0c4 / 0x4ea0c8.
//! Whenever the game would draw it, ours is drawn at that centre and size instead.
//!
//! The aim (0x4113eb) gives the sprite one of three pictures each picture (0x4b7650 sets
//! +0x3c), by what the ray through the reticle meets (0x444e30): 0x4ea0e4 an object (red
//! corner wedges, cyan cross), 0x4ea0ec one that can be targeted (its +0xbc set: green
//! wedges), 0x4ea0e8 nothing (no wedges, a red cross). Ours follows (`Look`).

use recomp::rt::{Cpu, ECX, ESP, Hook, call_original};
use recomp::ver::Us;
use std::sync::Mutex;
use win32::platform::glide;
use win32::platform::mem;

/// thiscall: draws a HUD sprite (blits its picture at its place).
const SPRITE_DRAW: Us = Us(0x4b7790);
/// The reticle's sprite.
const RETICLE: Us = Us(0x4ea0f4);
/// Its centre and half size in screen pixels (ints).
const CENTRE: [Us; 2] = [Us(0x4ea0dc), Us(0x4ea0e0)];
const HALF: [Us; 2] = [Us(0x4ea0c4), Us(0x4ea0c8)];
/// Points to a pointer to the 3D view's rectangle (left top right bottom, 0 26 640 396 in a
/// mission), what the game cuts the reticle to (0x408630).
const VIEW_RECT: Us = Us(0x4e9c44);
/// The sprite's picture, and two of the game's three for the reticle (see the head).
const SPRITE_PICTURE: u32 = 0x3c;
const PICTURE_NOTHING: Us = Us(0x4ea0e8);
const PICTURE_TARGET: Us = Us(0x4ea0ec);

/// What the aim meets, as the game's picture shows it.
#[derive(Clone, Copy, PartialEq)]
enum Look {
    /// an object: red wedges, cyan cross
    Hit,
    /// one that can be targeted: green wedges, cyan cross
    Target,
    /// nothing: no wedges, a red cross
    Nothing,
}

pub fn hooks() -> Vec<(u32, Hook)> {
    vec![(SPRITE_DRAW.at(), sprite_draw)]
}

fn sprite_draw(c: &mut Cpu) {
    if c.r[ECX] != RETICLE.at() {
        return call_original(c, SPRITE_DRAW.at());
    }
    let m = mem();
    let [cx, cy] = CENTRE.map(|a| m.rd32(a.at()) as i32 as f32);
    let [hx, hy] = HALF.map(|a| m.rd32(a.at()) as i32 as f32);
    let r = hx.max(hy).max(4.0);
    let pic = m.rd32(RETICLE.at() + SPRITE_PICTURE);
    let look = if pic == m.rd32(PICTURE_TARGET.at()) {
        Look::Target
    } else if pic == m.rd32(PICTURE_NOTHING.at()) {
        Look::Nothing
    } else {
        Look::Hit
    };
    // cut to the view's rows as the game cuts it (not over the HUD's bars); its columns reach
    // beside the 4:3 screen in a wide picture
    let rect = m.rd32(m.rd32(VIEW_RECT.at()));
    let rows = (m.rd32(rect + 4) as i32, m.rd32(rect + 12) as i32);
    draw(cx, cy, r, look, rows);
    // return without the game's blit (thiscall, no arguments)
    c.r[ESP] += 4;
}

/// The reticle's pictures by size in frame pixels and look (each made once per size).
static PICTURES: Mutex<Vec<(u32, Look, Vec<u8>)>> = Mutex::new(Vec::new());

/// Our reticle centred at (cx, cy) with radius r (screen pixels), cut to `rows`.
fn draw(cx: f32, cy: f32, r: f32, look: Look, rows: (i32, i32)) {
    let size = (2.0 * r * glide::frame_scale()).ceil().max(8.0) as u32;
    let mut pics = PICTURES.lock().unwrap();
    pics.retain(|p| p.0 == size);
    let i = match pics.iter().position(|p| p.1 == look) {
        Some(i) => i,
        None => {
            pics.push((size, look, picture(size, r, look)));
            pics.len() - 1
        }
    };
    let rgba = &pics[i].2;
    glide::draw_image([cx - r, cy - r, 2.0 * r, 2.0 * r], (size, size), rgba, rows);
}

/// The reticle as RGBA, `size` pixels square, for a radius of `r` screen pixels (its strokes
/// keep their width in screen pixels), after the game's own: a red ring, four corner wedges
/// pointing inwards (red, green on a target, none on nothing) and a cross with a dot (cyan,
/// red on nothing).
fn picture(size: u32, r: f32, look: Look) -> Vec<u8> {
    const RED: [u8; 3] = [214, 0, 40];
    const GREEN: [u8; 3] = [0, 230, 0];
    const CYAN: [u8; 3] = [0, 200, 200];
    let (wedges, cross) = match look {
        Look::Hit => (Some(RED), CYAN),
        Look::Target => (Some(GREEN), CYAN),
        Look::Nothing => (None, RED),
    };
    // units: 1 = the radius; one screen pixel and one picture pixel in units
    let line = 1.3 / r;
    let px = 2.0 / size as f32;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for j in 0..size {
        for i in 0..size {
            let x = (i as f32 + 0.5) * px - 1.0;
            let y = (j as f32 + 0.5) * px - 1.0;
            let (ax, ay) = (x.abs(), y.abs());
            // signed distances (negative inside)
            let ring = ((x * x + y * y).sqrt() - (1.0 - line / 2.0)).abs() - line / 2.0;
            // a wedge per corner: right angle inside at (0.42, 0.39), out to 0.72 / 0.69
            let wedge = triangle((ax, ay), (0.42, 0.39), (0.72, 0.39), (0.42, 0.69));
            // the cross: arms from 0.2 to 0.46 out, and a dot
            let arm = |a: f32| (a - 0.33).abs() - 0.13;
            let h = arm(ax).max(ay - line / 2.0);
            let v = arm(ay).max(ax - line / 2.0);
            let dot = ax.max(ay) - line / 2.0;
            let cross_d = h.min(v).min(dot);
            let cover = |d: f32| (0.5 - d / px).clamp(0.0, 1.0);
            // each pixel takes the colour of the shape covering it most
            let mut best = (RED, cover(ring));
            for (rgb, d) in [(cross, cross_d)].into_iter().chain(wedges.map(|w| (w, wedge))) {
                if cover(d) > best.1 {
                    best = (rgb, cover(d));
                }
            }
            let (rgb, a) = best;
            let p = ((j * size + i) * 4) as usize;
            out[p..p + 3].copy_from_slice(&rgb);
            out[p + 3] = (a * 255.0).round() as u8;
        }
    }
    out
}

/// Signed distance (roughly: the largest of the edges' distances) from `p` to the triangle
/// with a right angle at `a` and legs to `b` (along x) and `c` (along y).
fn triangle(p: (f32, f32), a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    let leg_x = a.0 - p.0;
    let leg_y = a.1 - p.1;
    // the hypotenuse from b to c, its outward normal away from a
    let (dx, dy) = (c.0 - b.0, c.1 - b.1);
    let len = (dx * dx + dy * dy).sqrt();
    let (nx, ny) = (dy / len, -dx / len);
    let hyp = (p.0 - b.0) * nx + (p.1 - b.1) * ny;
    leg_x.max(leg_y).max(hyp)
}
