//! The aiming reticle, drawn by us instead of the game's sprite: sharp at any resolution,
//! replaceable, and visible beside the 4:3 columns in a wide picture (the game's sprite is
//! pixels in its 640 x 480 frame buffer and is not drawn beyond it).
//!
//! The game's reticle is a HUD sprite (object 0x4ea0f4) drawn by the sprites' draw 0x4b7790
//! (it blits the sprite's picture into the locked frame buffer). 0x411300 places it: its
//! centre (screen pixels) at 0x4ea0dc / 0x4ea0e0, its half size at 0x4ea0c4 / 0x4ea0c8.
//! Whenever the game would draw it, ours is drawn at that centre and size instead.

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
    // cut to the view's rows as the game cuts it (not over the HUD's bars); its columns reach
    // beside the 4:3 screen in a wide picture
    let rect = m.rd32(m.rd32(VIEW_RECT.at()));
    let rows = (m.rd32(rect + 4) as i32, m.rd32(rect + 12) as i32);
    draw(cx, cy, r, rows);
    // return without the game's blit (thiscall, no arguments)
    c.r[ESP] += 4;
}

/// The reticle's picture for a size in frame pixels (made once per size).
static PICTURE: Mutex<Option<(u32, Vec<u8>)>> = Mutex::new(None);

/// Our reticle centred at (cx, cy) with radius r (screen pixels), cut to `rows`.
fn draw(cx: f32, cy: f32, r: f32, rows: (i32, i32)) {
    let size = (2.0 * r * glide::frame_scale()).ceil().max(8.0) as u32;
    let mut pic = PICTURE.lock().unwrap();
    if pic.as_ref().is_none_or(|p| p.0 != size) {
        *pic = Some((size, picture(size, r)));
    }
    let (_, rgba) = pic.as_ref().unwrap();
    glide::draw_image([cx - r, cy - r, 2.0 * r, 2.0 * r], (size, size), rgba, rows);
}

/// The reticle as RGBA, `size` pixels square, for a radius of `r` screen pixels (its strokes
/// keep their width in screen pixels): a red ring, four red corner wedges pointing inwards
/// and a cyan cross with a dot, after the game's own.
fn picture(size: u32, r: f32) -> Vec<u8> {
    const RED: [u8; 3] = [214, 0, 40];
    const CYAN: [u8; 3] = [0, 200, 200];
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
            let red = ring.min(wedge);
            // the cross: arms from 0.2 to 0.46 out, and a dot
            let arm = |a: f32| (a - 0.33).abs() - 0.13;
            let h = arm(ax).max(ay - line / 2.0);
            let v = arm(ay).max(ax - line / 2.0);
            let dot = ax.max(ay) - line / 2.0;
            let cyan = h.min(v).min(dot);
            let cover = |d: f32| (0.5 - d / px).clamp(0.0, 1.0);
            let (cr, cc) = (cover(red), cover(cyan));
            let (rgb, a) = if cr >= cc { (RED, cr) } else { (CYAN, cc) };
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
