//! The settings menu's page Keys: the game's own key bindings, changed in place.
//!
//! The game keeps a binding per command (`CmdMap`, REG_BINARY in its key, made a registry
//! variable by 0x470970: the table at [[0x569d60] + 8], its length at [[0x569d60] + 4]): a
//! dword each, two keys (11 bits each: the DirectInput code, 0x100 Shift, 0x200 Ctrl, 0x400
//! Alt), a joystick button (4 bits from bit 22) and a mouse button (2 bits from bit 26). A
//! command is held when any of them is (0x470f30, the keys' states by code and modifiers from
//! the keyboard read 0x46f710). Its own editor was in the launcher, which the port skips.
//! Here a key pressed for a command becomes its first key (the old first its second) and is
//! taken off the other commands; the table is used at once and saved.

use win32::platform::options::{Bound, Row};
use win32::platform::{advapi32, dinput, mem};

/// The object of the bindings: +4 the commands' count, +8 the table.
const BINDINGS: u32 = 0x569d60;
const GAME_KEY: &str = r"HKEY_CURRENT_USER\SOFTWARE\Zipper\RECOIL\1.0";

const KEY_BITS: u32 = 0x7ff;
const MODS: [(u32, &str); 3] = [(0x100, "Shift+"), (0x200, "Ctrl+"), (0x400, "Alt+")];

/// The commands offered, by their number in the table, after the manual (the others are
/// empty or the developers').
const COMMANDS: &[(u32, &str)] = &[
    (4, "Forward"),
    (1, "Backward"),
    (2, "Turn left"),
    (3, "Turn right"),
    (11, "Fire pulse gun"),
    (12, "Fire advanced weapon"),
    (13, "Detonate mines"),
    (15, "Weapon 1 (pulse guns)"),
    (16, "Weapon 2 (mortars)"),
    (17, "Weapon 3 (spray)"),
    (18, "Weapon 4 (proximity mines)"),
    (19, "Weapon 5 (remote mines)"),
    (20, "Weapon 6 (lasers)"),
    (21, "Weapon 7 (disturb)"),
    (22, "Weapon 8 (missiles)"),
    (23, "Weapon 9 (nukes)"),
    (7, "Center turret"),
    (8, "Center chassis"),
    (9, "Turret lock"),
    (10, "Reticle steering"),
    (37, "Track mode"),
    (38, "Amphib mode"),
    (39, "Hover mode"),
    (40, "Sub mode"),
    (5, "Sub: surface"),
    (6, "Sub: dive"),
    (30, "View"),
    (32, "HUD on/off"),
    (33, "Cockpit frame"),
    (27, "Map"),
    (28, "Map zoom in"),
    (29, "Map zoom out"),
    (24, "Objective"),
    (25, "Mission review"),
    (42, "Message"),
    (45, "Save"),
    (44, "Load"),
];

/// The table's address and length, once the game made it.
fn table() -> Option<(u32, u32)> {
    let m = mem();
    let obj = m.rd32(BINDINGS);
    if obj == 0 {
        return None;
    }
    let (n, at) = (m.rd32(obj + 4), m.rd32(obj + 8));
    (at != 0 && n > 0 && n < 256).then_some((at, n))
}

fn get(cmd: u32) -> u32 {
    table()
        .filter(|&(_, n)| cmd < n)
        .map_or(0, |(at, _)| mem().rd32(at + 4 * cmd))
}

/// The table changed: saved whole.
fn save() {
    let Some((at, n)) = table() else { return };
    let m = mem();
    let data: Vec<u8> = (0..n)
        .flat_map(|k| m.rd32(at + 4 * k).to_le_bytes())
        .collect();
    advapi32::set_value_at(GAME_KEY, "CmdMap", 3, &data);
}

fn key_name(k: u32) -> String {
    let mods: String = MODS.iter().filter(|m| k & m.0 != 0).map(|m| m.1).collect();
    mods + &dinput::dik_name(k as u8)
}

fn text(cmd: u32) -> String {
    let v = get(cmd);
    let mut parts: Vec<String> = [v & KEY_BITS, v >> 11 & KEY_BITS]
        .into_iter()
        .filter(|&k| k != 0)
        .map(key_name)
        .collect();
    match v >> 26 & 3 {
        0 => {}
        b => parts.push(format!("Mouse {b}")),
    }
    if parts.is_empty() {
        "-".into()
    } else {
        parts.join(", ")
    }
}

/// A key pressed for `cmd`: its first key (the old first its second), off the others; pressed
/// again, it comes off.
fn bind(cmd: u32, b: Bound) {
    let (Bound::Key(dik), Some((at, n))) = (b, table()) else {
        return;
    };
    let m = mem();
    let key = dik as u32;
    let v = get(cmd);
    let (k1, k2) = (v & KEY_BITS, v >> 11 & KEY_BITS);
    let rest = v & !(KEY_BITS | KEY_BITS << 11);
    let (k1, k2) = if k1 == key || k2 == key {
        let other = if k1 == key { k2 } else { k1 };
        (other, 0)
    } else {
        // the key off the other commands
        for c in (0..n).filter(|&c| c != cmd) {
            let w = m.rd32(at + 4 * c);
            let (a, b) = (w & KEY_BITS, w >> 11 & KEY_BITS);
            if a == key || b == key {
                let (a, b) = if a == key { (b, 0) } else { (a, 0) };
                m.wr32(at + 4 * c, (w & !(KEY_BITS | KEY_BITS << 11)) | a | b << 11);
            }
        }
        (key, k1)
    };
    m.wr32(at + 4 * cmd, rest | k1 | k2 << 11);
    save();
}

fn clear(cmd: u32) {
    let Some((at, _)) = table() else { return };
    let v = get(cmd);
    mem().wr32(at + 4 * cmd, v & !(KEY_BITS | KEY_BITS << 11));
    save();
}

/// The page's rows.
pub fn rows() -> Vec<Row> {
    COMMANDS
        .iter()
        .map(|&(cmd, name)| {
            Row::bind(
                name,
                "",
                move || text(cmd),
                move |b| bind(cmd, b),
                move || clear(cmd),
            )
        })
        .collect()
}
