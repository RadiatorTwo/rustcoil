//! Recoil's logic at a fixed rate (recomp_kit GUIDE.md 8b), the pictures paced on their own.
//!
//! The game's clock (`game_clock`, 0x4a5750) reads GetTickCount once a frame and keeps the
//! time step in `[0x56f2e4]`. Unbraked (~550 frames a second in a mission) the tank turned far
//! too fast. With an exact fixed step the tank turns alike at 30, 60 and 120 logic frames a
//! second (~197 degrees a second), faster at 240 (250) and 500 (333): below a step of about
//! 1/120 s some of the logic no longer scales. In a mission the clock now goes on by exactly
//! one logic frame (`LOGIC_FPS`) each pass of the loop, and the loop waits for that frame's
//! time. Outside a mission (menus, briefings) it reads the real time. Pictures above the
//! logic's rate come later: between logic frames, interpolated (GUIDE.md 8b).

use recomp::rt::{Cpu, ESP, call_original};
use recomp::ver::Us;
use std::sync::Mutex;
use std::time::Duration;
use win32::GlideExt;
use win32::platform::tween::{LogicClock, Pacer};
use win32::platform::{mem, without_turn};

/// The rate of the game's logic in a mission.
pub const LOGIC_FPS: u32 = 60;

/// The most pictures a second.
const MAX_FPS: u32 = 60;

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

/// Where the mission's frame (0x42f2c0) calls the clock: its return address.
const MISSION_CALL: Us = Us(0x42f2cd);

static CLOCK: LogicClock = LogicClock::new(LOGIC_FPS);
static PACER: Pacer = Pacer::new();
/// The number of the last logic frame.
static LAST: Mutex<u64> = Mutex::new(0);

fn period() -> Duration {
    Duration::from_nanos(1_000_000_000 / MAX_FPS as u64)
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
    if ret != MISSION_CALL.at() {
        CLOCK.reset();
        call_original(c, GAME_CLOCK.at());
        return;
    }
    let (scale, total, game_time) = (rdf(SCALE), rdf(TOTAL), rdf(GAME_TIME));
    let (n, _) = CLOCK.frame(|| {}, &PACER, period());
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

/// Glide's buffer swap: the pictures at most `MAX_FPS` a second.
pub struct Frames;

pub static FRAMES: Frames = Frames;

impl GlideExt for Frames {
    fn frame_shown(&self) {
        without_turn(|| PACER.pace(period()));
    }
}
