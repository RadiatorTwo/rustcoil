//! Recoil's logic at a fixed rate (recomp_kit GUIDE.md 8b), the pictures paced on their own.
//!
//! The game's clock (`game_clock`, 0x4a5750) reads GetTickCount once a frame and keeps the
//! time step in `[0x56f2e4]`. Unbraked (~550 frames a second in a mission) the tank turned far
//! too fast. With an exact fixed step the tank turns alike at 30, 60 and 120 logic frames a
//! second (~197 degrees a second), faster at 240 (250) and 500 (333): below a step of about
//! 1/120 s some of the logic no longer scales. In a mission the clock now goes on by exactly
//! one logic frame (`LOGIC_FPS`) each pass of the loop, and the loop waits for that frame's
//! time. Outside a mission (menus, briefings) it reads the real time.
//!
//! Pictures come at the display's refresh rate: while the next logic frame is not yet due, the
//! mission's frame runs again as a picture (`picture`: no clock, input or logic), and every
//! picture shows what moved between the last two logic frames (`crate::pictures`).

use recomp::rt::{Cpu, EAX, ECX, ESP, Hook, call_original};
use recomp::ver::Us;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use win32::GlideExt;
use win32::platform::tween::{LogicClock, Pacer};
use win32::platform::{guest_call, hold_turn, mem, without_turn};

/// The rate of the game's logic in a mission.
pub const LOGIC_FPS: u32 = 60;

/// Pictures a second when the display's refresh rate is unknown.
const DEFAULT_FPS: f64 = 60.0;

/// The most pictures a second: what the game reaches with room to spare (~400 at most in a
/// mission on a fast computer, 2026). A faster display gets a picture every second (third...)
/// refresh.
const MAX_FPS: f64 = 240.0;

/// The game's clock: real time since the last call into `NOW`/`LAST`, the time step (times
/// `SCALE`, which it sets back to 1, at most `MAX_STEP` while `CLAMP`) into `STEP` and
/// `GAME_TIME`, the unscaled step into `RAW_STEP` and `TOTAL`.
pub const GAME_CLOCK: Us = Us(0x4a5750);
const STEP: Us = Us(0x56f2e4);
const GAME_TIME: Us = Us(0x56f2e8);
const RAW_STEP: Us = Us(0x56f2ec);
const TOTAL: Us = Us(0x56f2f0);
const SCALE: Us = Us(0x4e6fb8);
const CLAMP: Us = Us(0x4e6fac);
const MAX_STEP: Us = Us(0x4e6fa8);

/// Where the mission's frame calls the clock: its return address.
const MISSION_CALL: Us = Us(0x42f2cd);

/// One frame of a mission (thiscall, one argument): clock, input (`INPUT`, `KEY_READ`,
/// `KEY_STATE`), logic (`LOGIC`: the only call that moves the tank), drawing (the world at
/// 0x44f670, the HUD into the locked frame buffer), buffer swap.
const MISSION_FRAME: Us = Us(0x42f2c0);
const LOGIC: Us = Us(0x44eab0);
/// Draws the world (`pictures`: what moved is put between logic frames meanwhile).
const RENDER: Us = Us(0x44f670);
/// Polls mouse, joystick and keyboard (DirectInput) into the game's tables.
const INPUT: Us = Us(0x471e60);
/// The next key pressed, from the keyboard's buffered data.
const KEY_READ: Us = Us(0x46fa90);
/// fastcall(slot): a key slot's state ([0x565b94 + 8 * slot]), moved on (pressed to held).
const KEY_STATE: Us = Us(0x46fa00);
const KEY_SLOTS: Us = Us(0x565b94);

/// The hooks of this module.
pub fn hooks() -> Vec<(u32, Hook)> {
    let table: &[(Us, Hook)] = &[
        (GAME_CLOCK, game_clock),
        (LOGIC, logic),
        (INPUT, input),
        (KEY_READ, key_read),
        (KEY_STATE, key_state),
        (RENDER, render),
    ];
    table.iter().map(|&(a, h)| (a.at(), h)).collect()
}

/// A picture between logic frames is being drawn: the mission's frame runs without its clock,
/// input and logic.
static PICTURE: AtomicBool = AtomicBool::new(false);

fn picture_now() -> bool {
    PICTURE.load(Ordering::Relaxed)
}

/// Return from a hooked function without running it.
fn skip(c: &mut Cpu, eax: u32) {
    c.r[ESP] += 4;
    c.r[EAX] = eax;
}

fn logic(c: &mut Cpu) {
    if picture_now() {
        return skip(c, 0);
    }
    crate::pictures::before_logic();
    call_original(c, LOGIC.at());
    crate::pictures::noted(*LAST.lock().unwrap());
}

/// Every picture (the logic frame's own too) shows the game a frame late, where it is when the
/// picture is shown.
fn render(c: &mut Cpu) {
    let alpha = CLOCK.alpha(PACER.next_shown());
    crate::wide::views_begin();
    crate::cut::views_begin();
    crate::pictures::draw(alpha, || call_original(c, RENDER.at()));
    crate::cut::views_end();
    crate::wide::views_end();
}

fn input(c: &mut Cpu) {
    if picture_now() {
        return skip(c, 0);
    }
    crate::gamepad::step();
    call_original(c, INPUT.at());
}

fn key_read(c: &mut Cpu) {
    if picture_now() {
        return skip(c, 0);
    }
    call_original(c, KEY_READ.at());
}

/// In a picture: the state as it is, not moved on.
fn key_state(c: &mut Cpu) {
    if picture_now() {
        let state = mem().rd32(KEY_SLOTS.at() + 8 * c.r[ECX]);
        return skip(c, state);
    }
    call_original(c, KEY_STATE.at());
}

/// A picture: the mission's frame again, with the time step 0 (the HUD's animations go on
/// with the logic).
fn picture(c: &mut Cpu, this: u32, arg: u32) {
    let saved = win32::platform::tween::save(c);
    let step = mem().rd32(STEP.at());
    mem().wr32(STEP.at(), 0);
    PICTURE.store(true, Ordering::Relaxed);
    hold_turn(true);
    c.r[ECX] = this;
    guest_call(c, MISSION_FRAME.at(), &[arg]);
    hold_turn(false);
    PICTURE.store(false, Ordering::Relaxed);
    mem().wr32(STEP.at(), step);
    win32::platform::tween::restore(c, &saved);
}

static CLOCK: LogicClock = LogicClock::new(LOGIC_FPS);
static PACER: Pacer = Pacer::new();
/// The number of the last logic frame.
static LAST: Mutex<u64> = Mutex::new(0);

/// The pictures' pace: a picture each refresh of the display (each second, third... above
/// `MAX_FPS`), kept to the display's refreshes (`refresh`, `Pacer::pace_on`), else
/// `DEFAULT_FPS`. Vsync on (no tearing in fullscreen).
///
/// Not by the swap alone: in a window it returns at once (no wait for the refresh). Not by our
/// own clock alone: 240 pictures a second on 480 Hz, meant for two refreshes each, landed a
/// refresh early or late now and then (the clocks differ a little): the moving tank lagged
/// behind and jumped ahead; ~400 unpaced ones flickered. Screenshots never showed it.
struct Pace {
    /// the time between pictures
    period: Duration,
    /// the display's last refresh and the time between two
    refresh: Option<(std::time::Instant, Duration)>,
}

/// `RUSTCOIL_UNPACED=1` (to measure): pictures as fast as the game draws them, vsync off.
fn unpaced() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| recomp::config::env_on("UNPACED"))
}

fn pace() -> Pace {
    if unpaced() {
        return Pace {
            period: Duration::ZERO,
            refresh: None,
        };
    }
    let refresh = win32::platform::window::vblank();
    let hz = match refresh {
        Some((_, p)) => Some(1.0 / p.as_secs_f64()),
        None => win32::platform::window::refresh_hz(),
    };
    let period = match hz {
        // (480.001 Hz is two refreshes a picture at 240, not three)
        Some(hz) => (hz / MAX_FPS - 0.01).ceil().max(1.0) / hz,
        None => 1.0 / DEFAULT_FPS,
    };
    Pace {
        period: Duration::from_secs_f64(period),
        refresh,
    }
}

fn rdf(a: Us) -> f32 {
    f32::from_bits(mem().rd32(a.at()))
}

fn wrf(a: Us, v: f32) {
    mem().wr32(a.at(), v.to_bits())
}

/// The game's clock: in a mission, the step is a whole number of logic frames.
pub fn game_clock(c: &mut Cpu) {
    let ret = mem().rd32(c.r[ESP]);
    if picture_now() {
        return skip(c, c.r[EAX]);
    }
    if ret != MISSION_CALL.at() {
        CLOCK.reset();
        crate::pictures::forget();
        call_original(c, GAME_CLOCK.at());
        return;
    }
    let (scale, total, game_time) = (rdf(SCALE), rdf(TOTAL), rdf(GAME_TIME));
    // the mission frame's own `this` and argument (above its 0x18 bytes of locals and saved
    // registers and its return address)
    let (this, arg) = (c.r[ECX], mem().rd32(c.r[ESP] + 0x20));
    let (n, _) = CLOCK.frame(|| picture(c, this, arg), &PACER, pace().period);
    let frames = std::mem::replace(&mut *LAST.lock().unwrap(), n);
    call_original(c, GAME_CLOCK.at());
    if n == 0 {
        // the first frame of a mission: the real step since the last call, as the game had it
        return;
    }
    let raw = (n - frames) as f32 / LOGIC_FPS as f32;
    let mut step = raw * scale;
    if mem().rd32(CLAMP.at()) != 0 && step > rdf(MAX_STEP) {
        step = rdf(MAX_STEP);
    }
    wrf(RAW_STEP, raw);
    wrf(TOTAL, total + raw);
    wrf(STEP, step);
    wrf(GAME_TIME, game_time + step);
}

/// Glide's buffer swap: the pictures' pace (`pace`).
pub struct Frames;

pub static FRAMES: Frames = Frames;

impl GlideExt for Frames {
    fn aspect(&self, window: Option<f32>) -> f32 {
        crate::wide::aspect(window)
    }

    fn before_shown(&self) {
        let p = pace();
        win32::platform::gl::set_vsync(!unpaced());
        without_turn(|| PACER.pace_on(p.period, p.refresh));
    }
}
