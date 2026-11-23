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
//! x scale is k = (4/3) / aspect of the game's, and the picture is drawn over the whole frame
//! (`View::Full`): the game still draws into its 640 columns, stretched by 1/k, so the
//! proportions are right and the sides show more of the world.
//!
//! The HUD is pixels written to the frame buffer after the picture (grLfbLock), the static
//! parts (its bars) only once: they stay in the frame. It is drawn unstretched in the frame's
//! 4:3 middle (`View::Hud(0.5)`); beside its bars the frame stays black. Its marks are at the
//! same angles, since the wide picture keeps the centre and the vertical scale. Open: the
//! target zoom's 3D (drawn with the picture, clipped to the zoom's box at the lower left)
//! lands in the wide picture's lower left, not in its box.
//!
//! On by default; REG_DWORD `Widescreen` 0 in the game's key turns it off.

use recomp::rt::{Cpu, ESP, Hook, call_original};
use recomp::ver::Us;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use win32::platform::glide::{self, View};
use win32::platform::mem;
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
    ]
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

/// The camera's fields as the game set them, while the picture uses wider ones.
static SAVED: Mutex<Option<(u32, [u32; 3])>> = Mutex::new(None);

/// The main view in a wide mission picture: the camera's horizontal field of view widened so
/// that its x scale is (4/3) / aspect of the game's. The game's own visibility tests use the
/// angle, the projection the scale: both see the wider picture. Put back after the picture.
fn projection(c: &mut Cpu) {
    let m = mem();
    let at = |k: u32| c.r[ESP] + 4 + 4 * k;
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
        }
        let x_scale = f32::from_bits(m.rd32(at(4)));
        m.wr32(at(4), (x_scale * k).to_bits());
    }
    call_original(c, PROJECTION.at());
}

/// The camera's own field of view back (after the picture).
fn put_back() {
    if let Some((cam, old)) = SAVED.lock().unwrap().take() {
        let m = mem();
        for (o, v) in WIDENED.into_iter().zip(old) {
            m.wr32(cam + o, v);
        }
    }
}
