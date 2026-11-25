//! The mission's 3D kept between the HUD's bars.
//!
//! The game cuts its 3D to the view (rows 26 to 396 in a mission, the rectangle 0x4e9c44
//! points to) itself, in software: polygons go through the view's clipper (0x57b178) in
//! 0x477bb0. Particles do not: they are points tested at their centre only (against the
//! clipper's rectangle, 0x57b19c..) and drawn as small squares (through [0x56fb28]), so one
//! near the view's edge reaches over a bar. The bars are drawn only once (static HUD sprites)
//! and what lands on them stays there. So while the views are drawn, Glide cuts the triangles
//! to the view's rows too (`glide::set_rows`).
//!
//! The target zoom draws each polygon a second time into its box, below the view: after the
//! zoom's clipper (0x57b1b8) passed it. Those are cut by the game already; the cut is off for
//! them and on again for the next mesh.

use recomp::rt::{Cpu, ECX, Hook, call_original};
use recomp::ver::Us;
use std::sync::Mutex;
use win32::platform::glide;
use win32::platform::mem;

/// thiscall: draws a mesh's polygons (each a second time into the zoom's box when it is on).
const DRAW_POLYS: Us = Us(0x477bb0);
/// thiscall: a clipper's first test of a polygon; with the zoom's clipper, a zoom polygon
/// follows when it passes.
const CLIP_TEST: Us = Us(0x480430);
const ZOOM_CLIP: Us = Us(0x57b1b8);
/// Points to a pointer to the 3D view's rectangle (left top right bottom).
const VIEW_RECT: Us = Us(0x4e9c44);

/// The view's rows while the views are drawn.
static ROWS: Mutex<Option<(i32, i32)>> = Mutex::new(None);

pub fn hooks() -> Vec<(u32, Hook)> {
    vec![(DRAW_POLYS.at(), draw_polys), (CLIP_TEST.at(), clip_test)]
}

/// The views begin: the triangles cut to the view's rows.
pub fn views_begin() {
    let m = mem();
    let at = m.rd32(VIEW_RECT.at());
    let rect = if at == 0 { 0 } else { m.rd32(at) };
    if rect == 0 {
        return;
    }
    let rows = (m.rd32(rect + 4) as i32, m.rd32(rect + 12) as i32);
    *ROWS.lock().unwrap() = Some(rows);
    glide::set_rows(Some(rows));
}

/// The views are drawn: the glow over them (before the HUD, which it leaves).
pub fn bloom() {
    if let Some(rows) = *ROWS.lock().unwrap() {
        glide::bloom(rows);
    }
}

/// The views are drawn: the HUD is not cut.
pub fn views_end() {
    *ROWS.lock().unwrap() = None;
    glide::set_rows(None);
}

fn draw_polys(c: &mut Cpu) {
    call_original(c, DRAW_POLYS.at());
    // the cut on again after the mesh's zoom polygons
    glide::set_rows(*ROWS.lock().unwrap());
}

fn clip_test(c: &mut Cpu) {
    if c.r[ECX] == ZOOM_CLIP.at() {
        glide::set_rows(None);
    }
    call_original(c, CLIP_TEST.at());
}
