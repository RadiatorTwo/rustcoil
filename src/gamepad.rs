//! The gamepad in a mission, and the settings menu's page Gamepad.
//!
//! The game's own joystick support (DirectInput, `Joystick` in its key, off) knew one stick and
//! buttons; here every pad button can be bound to any command of the game (`PadMap`, a dword
//! per command: bit n our button n, `dinput::CONTROLS`). A command bound is pressed the way
//! the keyboard or mouse would: its first key from the game's bindings (`controls`), with its
//! modifiers, else its mouse button. The left stick drives (the keys of forward, backward and
//! the turns), the right stick moves the reticle: the game's mouse is the window's pointer
//! (`hooks::mouse_move`), the stick's motion goes on top of it (`aim`). Done once a frame before the
//! game reads its input (0x471e60, `frames::input`); Start is Escape (the kit's), the game's menu.

use std::sync::Mutex;
use std::time::Instant;
use win32::platform::options::{Bound, Row};
use win32::platform::{advapi32, dinput};

/// The game's commands for the left stick: forward, backward, left, right.
const DRIVE: [u32; 4] = [4, 1, 2, 3];
/// How far a stick must go to drive.
const DRIVE_AT: f32 = 0.5;
/// The right stick's dead zone, and its speed at full tilt (mouse counts a second).
const AIM_DEAD: f32 = 0.15;
const AIM_SPEED: f32 = 700.0;

/// The right stick's motion so far (screen pixels), added to the pointer (`aim`).
static AIM: Mutex<(f32, f32)> = Mutex::new((0.0, 0.0));

/// The game's mouse: the window's pointer `(x, y)` (screen pixels) and the right stick's
/// motion, kept on the picture (a wide one reaches beside the 640 columns).
pub fn aim((x, y): (i32, i32)) -> (i32, i32) {
    let side = ((480.0 * win32::platform::glide::frame_aspect() - 640.0) / 2.0).max(0.0);
    let mut a = AIM.lock().unwrap();
    let ax = (x as f32 + a.0).clamp(-side, 639.0 + side);
    let ay = (y as f32 + a.1).clamp(0.0, 479.0);
    // what lies beyond the edge is dropped: the stick turns back at once
    *a = (ax - x as f32, ay - y as f32);
    (ax.round() as i32, ay.round() as i32)
}

/// Commands with their default buttons (our numbers: 0 RT, 1 LT, 2 B, 3 A, 4 X, 5 Y, 6 LB,
/// 7 RB, 8 Back, 9 R3, 10 L3, 11-14 the D-pad up, down, left, right).
const DEFAULTS: &[(u32, u32)] = &[
    (11, 0),  // fire pulse gun: RT
    (12, 1),  // fire advanced weapon: LT
    (13, 3),  // detonate: A
    (7, 2),   // center turret: B
    (9, 4),   // turret lock: X
    (30, 5),  // view: Y
    (15, 6),  // weapon 1: LB
    (22, 7),  // missiles: RB
    (27, 8),  // map: Back
    (8, 9),   // center chassis: R3
    (16, 11), // mortars: up
    (18, 12), // proximity mines: down
    (19, 13), // remote mines: left
    (20, 14), // lasers: right
];

/// The commands' count in the map (as the game's table).
const COMMANDS: usize = 47;

/// The bindings, read once.
static MAP: Mutex<Option<[u32; COMMANDS]>> = Mutex::new(None);

fn with_map<R>(f: impl FnOnce(&mut [u32; COMMANDS]) -> R) -> R {
    let mut m = MAP.lock().unwrap();
    let map = m.get_or_insert_with(|| {
        let mut map = [0; COMMANDS];
        match advapi32::value("", "PadMap") {
            Some((_, d)) => {
                for (k, c) in d.chunks_exact(4).take(COMMANDS).enumerate() {
                    map[k] = u32::from_le_bytes(c.try_into().unwrap());
                }
            }
            None => {
                for &(cmd, b) in DEFAULTS {
                    map[cmd as usize] |= 1 << b;
                }
            }
        }
        map
    });
    f(map)
}

fn save(map: &[u32; COMMANDS]) {
    let data: Vec<u8> = map.iter().flat_map(|v| v.to_le_bytes()).collect();
    advapi32::set_value("", "PadMap", 3, &data);
}

/// What the pad held at the last frame: the commands, and when.
static LAST: Mutex<(Vec<u32>, Option<Instant>)> = Mutex::new((Vec::new(), None));

/// Once a frame, before the game reads its input (`frames.rs`, 0x471e60).
pub fn step() {
    let pad = dinput::current_pad(0);
    let buttons = pad.buttons();
    let [lx, ly, rx, ry] = pad.stick;
    let mut held: Vec<u32> = with_map(|m| {
        (0..COMMANDS as u32)
            .filter(|&c| m[c as usize] & buttons != 0)
            .collect()
    });
    for (cmd, on) in
        DRIVE
            .into_iter()
            .zip([ly < -DRIVE_AT, ly > DRIVE_AT, lx < -DRIVE_AT, lx > DRIVE_AT])
    {
        if on && !held.contains(&cmd) {
            held.push(cmd);
        }
    }
    let mut last = LAST.lock().unwrap();
    for &cmd in held.iter().filter(|c| !last.0.contains(c)) {
        press(cmd, true);
    }
    for &cmd in last.0.iter().filter(|c| !held.contains(c)) {
        press(cmd, false);
    }
    last.0 = held;
    // the reticle: faster the further the stick goes
    let now = Instant::now();
    let dt = last.1.map_or(0.0, |t| (now - t).as_secs_f32().min(0.05));
    last.1 = Some(now);
    let speed = |v: f32| {
        let a = ((v.abs() - AIM_DEAD) / (1.0 - AIM_DEAD)).max(0.0);
        v.signum() * a * a * AIM_SPEED * dt
    };
    let mut a = AIM.lock().unwrap();
    a.0 += speed(rx);
    a.1 += speed(ry);
}

/// The modifier keys' DirectInput codes for the binding's bits.
const MODS: [(u32, u8); 3] = [(0x100, 0x2a), (0x200, 0x1d), (0x400, 0x38)];

/// Press or let go a command as its first key (with its modifiers) or its mouse button.
fn press(cmd: u32, down: bool) {
    let v = crate::controls::binding(cmd);
    let key = v & 0x7ff;
    if key != 0 {
        let mods = MODS.iter().filter(|m| key & m.0 != 0);
        if down {
            mods.for_each(|m| dinput::key(m.1, true));
            dinput::key(key as u8, true);
        } else {
            dinput::key(key as u8, false);
            mods.for_each(|m| dinput::key(m.1, false));
        }
    } else if let b @ 1..=3 = v >> 26 & 3 {
        dinput::button(b as u8 - 1, down);
    }
}

fn text(cmd: u32) -> String {
    let mask = with_map(|m| m[cmd as usize]);
    let names: Vec<&str> = (0..15)
        .filter(|b| mask >> b & 1 != 0)
        .map(|b| dinput::CONTROLS[b])
        .collect();
    if names.is_empty() {
        "-".into()
    } else {
        names.join(", ")
    }
}

/// A button pressed for `cmd`: added to it and taken off the others; pressed again, it comes
/// off.
fn bind(cmd: u32, b: Bound) {
    let Bound::Pad(button @ 0..15) = b else {
        return;
    };
    with_map(|m| {
        let bit = 1 << button;
        if m[cmd as usize] & bit != 0 {
            m[cmd as usize] &= !bit;
        } else {
            m.iter_mut().for_each(|v| *v &= !bit);
            m[cmd as usize] |= bit;
        }
        save(m);
    });
}

fn clear(cmd: u32) {
    with_map(|m| {
        m[cmd as usize] = 0;
        save(m);
    });
}

/// The page's rows: the game's commands (the left stick drives, the right aims).
pub fn rows() -> Vec<Row> {
    crate::controls::COMMANDS
        .iter()
        .filter(|(cmd, _)| !DRIVE.contains(cmd))
        .map(|&(cmd, name)| {
            Row::bind(
                name,
                "A / Enter: bind a button   X / Delete: none   Sticks: drive and aim",
                move || text(cmd),
                move |b| bind(cmd, b),
                move || clear(cmd),
            )
        })
        .collect()
}
