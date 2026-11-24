//! Widescreen (Hor+, recomp_kit GUIDE 8c): in a mission the frame takes the window's aspect
//! and shows more at the left and right; menus, briefings and videos stay 4:3.
//!
//! The camera's projection is set up by 0x474480 (stdcall: left, top, half width, half height,
//! x and y scale, near, far; called only by the view setup 0x479d60, once a picture). The
//! mission's main view is (0, 26, 320, 185): the 640 columns, rows 26 to 396 between the
//! HUD's bars; the game clips its 3D to that itself (grClipWindow stays 640x480). The camera
//! keeps its horizontal field of view (+0xe8), its half (+0xf0) and the x scale 1/tan(half)
//! (+0x1d4); its visibility tests use the angle. In a wide mission picture (`views_begin` to
//! `views_end`, world_draw: every 3D polygon is drawn in there) the angle is widened so the
//! x scale is k = (4/3) / aspect of the game's (its visibility frustum made again with it),
//! and the picture is drawn over the whole frame
//! (`View::Full`): the game still draws into its 640 columns, stretched by 1/k, so the
//! proportions are right and the sides show more of the world.
//!
//! The HUD is pixels written to the frame buffer after the picture (grLfbLock), the static
//! parts (its bars) only once: they stay in the frame. It is drawn unstretched in the frame's
//! 4:3 middle (`View::Hud(0.5)`); beside its bars the frame stays black. After the picture
//! the projection's x values are the game's own again (`put_back`): the HUD's marks, the aim
//! and the hit tests turn 4:3 screen points into directions with them, and a point of the
//! 4:3 middle lies over the same direction in the wide picture (it keeps the centre and the
//! angle per pixel).
//!
//! The target zoom is no view of its own: while it is on (0x58298c), every polygon of the
//! picture is drawn a second time, cut to a source rectangle around the aim (0x57b1b8) and
//! moved in 2D into the zoom's box (x * [0x57b20c] + [0x57b214], y * [0x57b210] +
//! [0x57b218]); 0x47a010 sets these from the source rectangle (fixed size, 640 x 480
//! screen). In a wide picture the source rectangle is taken in the wide picture's columns
//! (`x -> 320 + k (x - 320)`: the same piece of the world, narrower by k) and the box's x
//! mapping goes into the frame's 4:3 middle (`X -> 320 (1 - k) + k X`), where the HUD's box is.
//!
//! On by default; REG_DWORD `Widescreen` 0 in the game's key turns it off.

use recomp::rt::{Cpu, ECX, ESP, Hook, call_original};
use recomp::ver::Us;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use win32::platform::glide::{self, View};
use win32::platform::{guest_call, mem};
use win32::platform::setting::Setting;

static WIDE: Setting = Setting::new("Widescreen", "WIDE", true);

/// stdcall(left, top, half width, half height, x scale, y scale, near, far): the projection.
const PROJECTION: Us = Us(0x474480);
/// Clears the frame for a new picture (grBufferClear, grClipWindow, grRenderBuffer).
const FRAME_BEGIN: Us = Us(0x4a6810);

/// The game's screen width: a view or clip window this wide is the main view's.
const SCREEN_W: f32 = 640.0;

/// How long after the last mission picture the frame stays wide.
const LINGER: Duration = Duration::from_millis(250);

/// When the last mission picture was drawn.
static LAST_MISSION: Mutex<Option<Instant>> = Mutex::new(None);

/// The mission's picture is being drawn (world_draw runs).
static IN_VIEWS: AtomicBool = AtomicBool::new(false);

pub fn hooks() -> Vec<(u32, Hook)> {
    vec![
        (PROJECTION.at(), projection),
        (FRAME_BEGIN.at(), frame_begin),
        (ZOOM_SETUP.at(), zoom_setup),
    ]
}

/// fastcall(source rectangle: x0 y0 x1 y1 floats, flag): sets up the target zoom.
const ZOOM_SETUP: Us = Us(0x47a010);
/// The zoom's x mapping into its box: scale and offset.
const ZOOM_SCALE_X: Us = Us(0x57b20c);
const ZOOM_OFFSET_X: Us = Us(0x57b214);

/// The target zoom in a wide picture: its source rectangle in the wide picture's columns,
/// its box in the frame's 4:3 middle (see the head).
fn zoom_setup(c: &mut Cpu) {
    if !wide_now() {
        return call_original(c, ZOOM_SETUP.at());
    }
    let m = mem();
    let k = (4.0 / 3.0) / glide::frame_aspect();
    let mid = SCREEN_W / 2.0;
    let rect = c.r[ECX];
    let xs = [rect, rect + 8];
    let old = xs.map(|a| m.rd32(a));
    for (a, v) in xs.into_iter().zip(old) {
        m.wr32(a, (mid + k * (f32::from_bits(v) - mid)).to_bits());
    }
    call_original(c, ZOOM_SETUP.at());
    for (a, v) in xs.into_iter().zip(old) {
        m.wr32(a, v);
    }
    let scale = f32::from_bits(m.rd32(ZOOM_SCALE_X.at()));
    let offset = f32::from_bits(m.rd32(ZOOM_OFFSET_X.at()));
    m.wr32(ZOOM_SCALE_X.at(), (k * scale).to_bits());
    m.wr32(ZOOM_OFFSET_X.at(), (k * offset + mid * (1.0 - k)).to_bits());
}

/// The frame being drawn is wide.
fn wide_now() -> bool {
    glide::frame_aspect() > 4.0 / 3.0 + 0.01
}

/// The aspect the next frame is drawn with, for a window of aspect `window`.
pub fn aspect(window: Option<f32>) -> f32 {
    let mission = LAST_MISSION
        .lock()
        .unwrap()
        .is_some_and(|t| t.elapsed() < LINGER);
    match window {
        Some(a) if WIDE.get() && mission => a.max(4.0 / 3.0),
        _ => 4.0 / 3.0,
    }
}

/// The mission's picture begins (world_draw).
pub fn views_begin() {
    *LAST_MISSION.lock().unwrap() = Some(Instant::now());
    IN_VIEWS.store(true, Ordering::Relaxed);
    if wide_now() {
        glide::set_view(View::Full);
    }
}

/// The mission's picture is drawn: what follows (the HUD) goes into the 4:3 middle.
pub fn views_end() {
    IN_VIEWS.store(false, Ordering::Relaxed);
    put_back();
    if wide_now() {
        glide::set_view(View::Hud(0.5));
    }
}

/// A new picture: the clear covers the whole frame; what is drawn before the views (the
/// target zoom's background) goes into the 4:3 middle.
fn frame_begin(c: &mut Cpu) {
    let wide = wide_now();
    if wide {
        glide::set_view(View::Full);
    }
    call_original(c, FRAME_BEGIN.at());
    if wide {
        glide::set_view(View::Hud(0.5));
    }
}

/// The camera (thiscall object of the view setup 0x479d60).
const CAMERA: Us = Us(0x57b174);
/// The camera's horizontal field of view (radians), its half and the projection's x scale
/// (1 / tan(half)): 0x44a460 makes them from the base field of view and the zoom.
const FOV_X: u32 = 0xe8;
const HALF_X: u32 = 0xf0;
const SCALE_X: u32 = 0x1d4;
/// The fields put back after the picture.
const WIDENED: [u32; 3] = [FOV_X, HALF_X, SCALE_X];
/// The camera's visibility frustum. The camera update (0x44aa80, before the picture) makes
/// it only when the angles changed: the corners of the near plane (x at these fields,
/// ±tan(fov / 2) * near) when +0xf8 is set, the six planes (0x47a140, from the half angles,
/// into +0x13c) when +0x138 is set; 0x47a250 turns the planes into world space (+0x188).
const NEAR: u32 = 0xb4;
const CORNERS_X: [u32; 4] = [0x108, 0x114, 0x120, 0x12c];
const CORNERS_DIRTY: u32 = 0xf8;
const PLANES_DIRTY: u32 = 0x138;
const PLANES: Us = Us(0x47a140);
const PLANES_WORLD: Us = Us(0x47a250);

/// The camera's fields as the game set them, while the picture uses wider ones.
static SAVED: Mutex<Option<(u32, [u32; 3])>> = Mutex::new(None);

/// The main view in a wide mission picture: the camera's horizontal field of view widened so
/// that its x scale is (4/3) / aspect of the game's. The game's own visibility tests use the
/// angle, the projection the scale: both see the wider picture. Put back after the picture.
fn projection(c: &mut Cpu) {
    let m = mem();
    let esp = c.r[ESP];
    let at = |k: u32| esp + 4 + 4 * k;
    let half_w = f32::from_bits(m.rd32(at(2)));
    if IN_VIEWS.load(Ordering::Relaxed) && wide_now() && half_w * 2.0 >= SCREEN_W {
        let k = (4.0 / 3.0) / glide::frame_aspect();
        let cam = m.rd32(CAMERA.at());
        let mut saved = SAVED.lock().unwrap();
        if saved.is_none() {
            let old = WIDENED.map(|o| m.rd32(cam + o));
            *saved = Some((cam, old));
            let scale = f32::from_bits(old[2]) * k;
            let half = (1.0 / scale).atan();
            m.wr32(cam + FOV_X, (2.0 * half).to_bits());
            m.wr32(cam + HALF_X, half.to_bits());
            m.wr32(cam + SCALE_X, scale.to_bits());
            // the visibility tests' frustum, made from the angles only when they change
            let near = f32::from_bits(m.rd32(cam + NEAR));
            let x = half.tan() * near;
            for (o, v) in CORNERS_X.into_iter().zip([x, -x, x, -x]) {
                m.wr32(cam + o, v.to_bits());
            }
            let regs = c.r;
            c.r[ECX] = cam;
            guest_call(c, PLANES.at(), &[]);
            c.r[ECX] = cam;
            guest_call(c, PLANES_WORLD.at(), &[]);
            c.r = regs;
        }
        let x_scale = f32::from_bits(m.rd32(at(4)));
        m.wr32(at(4), (x_scale * k).to_bits());
        // the projection's x values as the game's own scale makes them, for after the picture
        let one = f32::from_bits(m.rd32(PROJ_ONE.at()));
        *NARROW.lock().unwrap() = Some([
            x_scale,
            one / x_scale,
            x_scale * half_w,
            one / (x_scale * half_w),
        ]);
    }
    call_original(c, PROJECTION.at());
}

/// The projection's values that depend on its x scale (written by 0x474480): the scale, its
/// inverse, scale x half width and its inverse. The last is what screen points are turned
/// back into directions with (0x474c40: the aim, 0x4113eb, and the hit tests, 0x49aa80):
/// left wide after the picture, the shots went the further off the further to the side.
const PROJ_X: [Us; 4] = [Us(0x56a718), Us(0x56a6f8), Us(0x56a710), Us(0x56a720)];
/// The constant 0x474480 divides (1.0).
const PROJ_ONE: Us = Us(0x4d6a24);

/// The projection's x values for the game's own scale, put back after the picture.
static NARROW: Mutex<Option<[f32; 4]>> = Mutex::new(None);

/// The camera's own field of view and the projection's own x values back (after the picture).
fn put_back() {
    if let Some(values) = NARROW.lock().unwrap().take() {
        let m = mem();
        for (a, v) in PROJ_X.into_iter().zip(values) {
            m.wr32(a.at(), v.to_bits());
        }
    }
    if let Some((cam, old)) = SAVED.lock().unwrap().take() {
        let m = mem();
        for (o, v) in WIDENED.into_iter().zip(old) {
            m.wr32(cam + o, v);
        }
        // the game's next camera update makes its frustum again from its own angles
        m.wr32(cam + PLANES_DIRTY, 1);
        m.wr32(cam + CORNERS_DIRTY, 1);
    }
}
