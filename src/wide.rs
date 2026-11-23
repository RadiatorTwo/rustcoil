//! Widescreen (Hor+, recomp_kit GUIDE 8c): in a mission the frame takes the window's aspect
//! and shows more at the left and right; menus, briefings and videos stay 4:3.
//!
//! The camera's projection is set up by 0x474480 (stdcall: left, top, half width, half height,
//! x and y scale, near, far; called by the view setup 0x479d60). The mission's main view is
//! (0, 26, 320, 185): the 640 columns, centred on row 211 between the HUD's bars.
//! Everything else follows from the x scale: the screen x scale ([0x57b140]), its inverse
//! for the visibility tests ([0x56a6f8]). In the mission's picture (`views_begin` to
//! `views_end`) the main view gets its x scale times k = (4/3) / aspect and is drawn over the
//! whole wide frame (`View::Full`): the game still draws into its 640 columns, stretched by
//! 1/k, so the proportions are right. The game draws its 3D over the whole screen (the HUD
//! covers the rest), so beside the HUD the wide picture shows the world too. The HUD is
//! drawn after the picture: unstretched in the frame's 4:3 middle
//! (`View::Hud(0.5)`). It lands where it should: its marks are at the same angles, since the
//! wide picture keeps the centre and the vertical scale.
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

/// The game's screen width: a view or clip window this wide is the main view's.
const SCREEN_W: f32 = 640.0;

/// How long after the last mission picture the frame stays wide.
const LINGER: Duration = Duration::from_millis(250);

/// When the last mission picture was drawn.
static LAST_MISSION: Mutex<Option<Instant>> = Mutex::new(None);

/// The mission's picture is being drawn (world_draw runs).
static IN_VIEWS: AtomicBool = AtomicBool::new(false);

pub fn hooks() -> Vec<(u32, Hook)> {
    vec![(PROJECTION.at(), projection)]
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
    if wide_now() {
        glide::set_view(View::Hud(0.5));
    }
}

/// The main view's x scale times (4/3) / aspect in a wide mission picture.
fn projection(c: &mut Cpu) {
    let m = mem();
    let at = |k: u32| c.r[ESP] + 4 + 4 * k;
    let half_w = f32::from_bits(m.rd32(at(2)));
    if IN_VIEWS.load(Ordering::Relaxed) && wide_now() && half_w * 2.0 >= SCREEN_W {
        let k = (4.0 / 3.0) / glide::frame_aspect();
        let x_scale = f32::from_bits(m.rd32(at(4)));
        m.wr32(at(4), (x_scale * k).to_bits());
    }
    call_original(c, PROJECTION.at());
}
