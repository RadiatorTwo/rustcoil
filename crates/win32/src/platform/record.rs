//! `<PREFIX>_RECORD=file.txt`: the player's input is written there as `<PREFIX>_INPUT` items, one a
//! line, to be played again with `<PREFIX>_INPUT=@file.txt` (as rustschweine's HOGS_RECORD and
//! rust_metal's GUNMETAL_RECORD).
//!
//! Items are at ms of the input clock (`input_ms`), as <PREFIX>_INPUT counts them: real time
//! since the game's start, but in a game's fixed logic frames (`logic_frames`) exactly the
//! frames' time, and every entry into them is a `sync` item: a replay waits there for the game
//! to get there too and goes on from the item's time. So the logic gets each input in the same
//! frame as when it was recorded, however long loading takes. (Before, at real ms, a replay
//! went off course within seconds of a mission.)
//!
//! Recorded: keys with how
//! long they were held (`Up*1500`), mouse buttons with where and how long (`click@x,y*n`,
//! `rclick`, `mclick`, `x1click`, `x2click`), the mouse's moves (`move@x,y`, the last of each
//! ms) and its raw motion, what DirectInput's mouse gets (`motion@dx,dy`, summed over each ms:
//! the cursor's moves are not it, at the window's edges or while the cursor is caught), the wheel (`wheel@n`), typed characters (`text@c`, `char@hex`), the window's
//! focus and its close button, and the first real gamepad (`pad@A*n`, `pad@LX=0.5*n`: its
//! buttons and its sticks in steps of 0.05; a replay plays it on the scripted pad, which player 1
//! uses). Repeats of a held key are not written (the hold is).
//!
//! F11 marks a moment: a comment line with the time, and the picture shown is written to
//! `file.txt.<ms>.png`. F11 does not reach the game while recording.

use super::log;
use super::window::Input;
use std::io::Write;
use std::sync::Mutex;

/// The pad's controls (`dinput::CONTROLS`).
const PAD_CONTROLS: usize = 20;

/// The items for the input events, in order (pure: what the file gets).
#[derive(Default)]
pub struct Recorder {
    /// keys and mouse buttons held: their item (without its hold) and since when
    held: Vec<(String, u64)>,
    /// the mouse in frame pixels, and a move not written yet (when, where): the last of its ms
    mouse: (i32, i32),
    moved: Option<(u64, i32, i32)>,
    /// raw motion not written yet: its ms and the sum
    motion: Option<(u64, i32, i32)>,
    /// the pad's controls as last seen, and since when (a value of 0 is not held)
    pad: [(f32, u64); PAD_CONTROLS],
}

impl Recorder {
    /// The lines for one input event at `t` (ms since start).
    pub fn event(&mut self, i: &Input, t: u64) -> Vec<String> {
        let mut out = Vec::new();
        if let Input::Move(x, y) = *i {
            if self.moved.is_some_and(|(m, ..)| m != t) {
                self.flush_move(&mut out);
            }
            self.mouse = (x, y);
            self.moved = Some((t, x, y));
            return out;
        }
        self.flush_move(&mut out);
        match *i {
            Input::Key {
                vk,
                scan,
                down,
                repeat: false,
            } => {
                if let Some(name) = super::window::key_name(vk, scan) {
                    self.press(name, down, t, &mut out);
                }
            }
            Input::Button(b, down) => {
                let Some(kind) =
                    ["click", "rclick", "mclick", "x1click", "x2click"].get(b as usize)
                else {
                    return out;
                };
                let (x, y) = self.mouse;
                // the item's own position: a scripted click moves there 30 ms before it presses
                let name = format!("{kind}@{x},{y}");
                if down {
                    self.release(kind, t, &mut out);
                    self.held.push((name, t.saturating_sub(30)));
                } else {
                    self.release(kind, t, &mut out);
                }
            }
            Input::Char(ch) => out.push(match ch {
                b'!'..=b'~' if ch != b'#' && ch != b'_' => format!("{t}:text@{}", ch as char),
                b' ' => format!("{t}:text@_"),
                _ => format!("{t}:char@{ch:02x}"),
            }),
            Input::Wheel(n) => out.push(format!("{t}:wheel@{n}")),
            Input::Focus(f) => out.push(format!("{t}:{}", if f { "focus" } else { "blur" })),
            Input::Close => {
                // what is still held ends here
                for (name, since) in std::mem::take(&mut self.held) {
                    out.push(format!("{since}:{name}*{}", t.saturating_sub(since).max(1)));
                }
                out.extend(self.pad(&[0.0; PAD_CONTROLS], t));
                out.push(format!("{t}:close"));
            }
            _ => {}
        }
        out
    }

    /// The move and motion not written yet.
    fn flush_move(&mut self, out: &mut Vec<String>) {
        if let Some((t, x, y)) = self.moved.take() {
            out.push(format!("{t}:move@{x},{y}"));
        }
        if let Some((t, dx, dy)) = self.motion.take() {
            out.push(format!("{t}:motion@{dx},{dy}"));
        }
    }

    /// The mouse's raw motion at `t` (DirectInput's).
    pub fn motion(&mut self, dx: i32, dy: i32, t: u64) -> Vec<String> {
        let mut out = Vec::new();
        match self.motion.as_mut() {
            Some((m, x, y)) if *m == t => {
                *x += dx;
                *y += dy;
            }
            _ => {
                self.flush_move(&mut out);
                self.motion = Some((t, dx, dy));
            }
        }
        out
    }

    /// The game enters its fixed logic frames at `t`.
    pub fn sync(&mut self, t: u64) -> Vec<String> {
        let mut out = Vec::new();
        self.flush_move(&mut out);
        out.push(format!("{t}:sync"));
        out
    }

    fn press(&mut self, name: String, down: bool, t: u64, out: &mut Vec<String>) {
        let k = self.held.iter().position(|h| h.0 == name);
        match (down, k) {
            (true, None) => self.held.push((name, t)),
            (false, Some(k)) => {
                let (name, since) = self.held.remove(k);
                out.push(format!("{since}:{name}*{}", t.saturating_sub(since).max(1)));
            }
            _ => {}
        }
    }

    /// A mouse button of this kind lets go.
    fn release(&mut self, kind: &str, t: u64, out: &mut Vec<String>) {
        let at = format!("{kind}@");
        if let Some(k) = self.held.iter().position(|h| h.0.starts_with(&at)) {
            let (name, since) = self.held.remove(k);
            // held from 30 ms after the item's time
            out.push(format!(
                "{since}:{name}*{}",
                t.saturating_sub(since + 30).max(1)
            ));
        }
    }

    /// The pad's controls now (`dinput::CONTROLS` order; buttons 0 or 1, axes -1...1, y up).
    pub fn pad(&mut self, v: &[f32; PAD_CONTROLS], t: u64) -> Vec<String> {
        let mut out = Vec::new();
        for (k, (&new, (old, since))) in v.iter().zip(self.pad.iter_mut()).enumerate() {
            // steps of 0.05, so a stick held still is one item; a resting stick's noise is 0
            let new = if new.abs() < 0.1 {
                0.0
            } else {
                (new.clamp(-1.0, 1.0) * 20.0).round() / 20.0
            };
            if new == *old {
                continue;
            }
            if *old != 0.0 {
                let name = super::dinput::CONTROLS[k];
                let hold = t.saturating_sub(*since).max(1);
                out.push(if k < 16 {
                    format!("{since}:pad@{name}*{hold}")
                } else {
                    format!("{since}:pad@{name}={old}*{hold}")
                });
            }
            (*old, *since) = (new, t);
        }
        out
    }
}

struct File {
    file: std::fs::File,
    path: String,
    rec: Recorder,
}

static RECORD: std::sync::LazyLock<Mutex<Option<File>>> = std::sync::LazyLock::new(|| {
    Mutex::new((|| {
        let path = recomp::config::env("RECORD")
            .ok_or(())
            .ok()
            .filter(|p| !p.is_empty())?;
        let file = std::fs::File::create(&path)
            .map_err(|e| log!("RECORD: cannot write {path}: {e}"))
            .ok()?;
        log!(
            "RECORD: the input goes to {path} (F11: a mark with a picture; play it with INPUT=@{path})"
        );
        Some(File {
            file,
            path,
            rec: Recorder::default(),
        })
    })())
});

/// Recording is on.
pub fn on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| RECORD.lock().unwrap().is_some())
}

/// The input clock: real time plus `offset`, or within fixed logic frames their own time.
struct Clock {
    offset: i64,
    fixed: Option<Fixed>,
}

#[derive(Clone, Copy)]
struct Fixed {
    base: u64,
    frames: u64,
    fps: u32,
}

impl Fixed {
    fn ms(&self) -> u64 {
        self.base + self.frames * 1000 / self.fps as u64
    }
}

static CLOCK: Mutex<Clock> = Mutex::new(Clock {
    offset: 0,
    fixed: None,
});

fn real_ms() -> i64 {
    super::kernel32::start_time().elapsed().as_millis() as i64
}

/// The time for input items (recorded and replayed), ms.
pub fn input_ms() -> u64 {
    let c = CLOCK.lock().unwrap();
    match c.fixed {
        Some(f) => f.ms(),
        None => (real_ms() + c.offset).max(0) as u64,
    }
}

/// A game's fixed logic frames (`fps` a second): `frames` more have begun (0 at the first of a
/// run of them). Their input time is the frames' own, so a replay gives the logic each input in
/// the frame it was recorded in. The first of a run is a `sync` item (see the head).
pub fn logic_frames(frames: u64, fps: u32) {
    let mut c = CLOCK.lock().unwrap();
    if let Some(f) = c.fixed.as_mut() {
        f.frames += frames;
        return;
    }
    let now = (real_ms() + c.offset).max(0) as u64;
    // a replay goes on from the recording's time here
    let base = super::window::script_sync().unwrap_or(now);
    c.fixed = Some(Fixed { base, frames, fps });
    drop(c);
    if on() {
        let mut r = RECORD.lock().unwrap();
        if let Some(f) = r.as_mut() {
            let lines = f.rec.sync(base);
            write(f, lines);
        }
    }
}

/// Out of fixed logic frames: real time again, going on from the input time reached.
pub fn real_time() {
    let mut c = CLOCK.lock().unwrap();
    if let Some(f) = c.fixed.take() {
        c.offset = f.ms() as i64 - real_ms();
    }
}

fn now() -> u64 {
    input_ms()
}

fn write(f: &mut File, lines: Vec<String>) {
    for l in lines {
        let _ = writeln!(f.file, "{l}");
    }
}

/// A real input event (before it reaches the game).
pub fn input(i: &Input) {
    if !on() {
        return;
    }
    let mut r = RECORD.lock().unwrap();
    let Some(f) = r.as_mut() else { return };
    let lines = f.rec.event(i, now());
    write(f, lines);
    if matches!(i, Input::Close) {
        let _ = f.file.flush();
    }
}

/// The mouse's raw motion (what DirectInput's mouse gets).
pub fn motion(dx: i32, dy: i32) {
    if !on() {
        return;
    }
    let mut r = RECORD.lock().unwrap();
    let Some(f) = r.as_mut() else { return };
    let lines = f.rec.motion(dx, dy, now());
    write(f, lines);
}

/// The first real pad's controls, at every read of the pads.
pub fn pad(v: &[f32; PAD_CONTROLS]) {
    if !on() {
        return;
    }
    let mut r = RECORD.lock().unwrap();
    let Some(f) = r.as_mut() else { return };
    let lines = f.rec.pad(v, now());
    write(f, lines);
}

/// F11: a mark (a comment) and the path for a picture of what is shown.
pub fn mark() -> Option<String> {
    let mut r = RECORD.lock().unwrap();
    let f = r.as_mut()?;
    let ms = now();
    let _ = writeln!(f.file, "# mark {ms} ms");
    let _ = f.file.flush();
    let shot = format!("{}.{ms:06}.png", f.path);
    log!("RECORD: mark at {ms} ms: {shot}");
    Some(shot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_clicks() {
        let mut r = Recorder::default();
        let up = Input::Key {
            vk: 0x26,
            scan: 0x48,
            down: true,
            repeat: false,
        };
        assert!(r.event(&up, 1000).is_empty());
        assert!(
            r.event(
                &Input::Key {
                    vk: 0x26,
                    scan: 0x48,
                    down: true,
                    repeat: true
                },
                1100
            )
            .is_empty()
        );
        assert_eq!(
            r.event(
                &Input::Key {
                    vk: 0x26,
                    scan: 0x48,
                    down: false,
                    repeat: false
                },
                2500
            ),
            ["1000:Up*1500"]
        );
        assert!(r.event(&Input::Move(480, 300), 3000).is_empty());
        assert!(r.event(&Input::Move(484, 303), 3000).is_empty());
        assert_eq!(r.event(&Input::Button(0, true), 3100), ["3000:move@484,303"]);
        assert_eq!(
            r.event(&Input::Button(0, false), 3200),
            ["3070:click@484,303*100"]
        );
        assert_eq!(r.event(&Input::Char(b'A'), 4000), ["4000:text@A"]);
        assert_eq!(r.event(&Input::Char(b' '), 4050), ["4050:text@_"]);
    }

    #[test]
    fn pad() {
        let mut r = Recorder::default();
        let mut v = [0.0; PAD_CONTROLS];
        v[3] = 1.0;
        v[16] = 0.51;
        assert!(r.pad(&v, 100).is_empty());
        v[16] = 0.49;
        assert!(r.pad(&v, 150).is_empty());
        v[3] = 0.0;
        v[16] = 0.0;
        assert_eq!(r.pad(&v, 600), ["100:pad@A*500", "100:pad@LX=0.5*500"]);
    }
}
