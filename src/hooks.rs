//! Game functions replaced by Rust functions. A hook gets the guest's registers (`Cpu`); its
//! arguments are on the guest stack above the return address (`win32::platform::arg(c, k)`).
//! `call_original(c, address)` runs the game's own function (it pops the return address
//! itself); a hook that does not call it must pop the return address (and, for stdcall, the
//! arguments) and set eax itself.
//!
//! Addresses are the reference version's (`Us(0x...)`), `.at()` gives their place in the
//! version that runs. Write what you learn about each function into data/symbols.txt.

use recomp::rt::{Cpu, ECX, Hook, call_original};
use recomp::ver::Us;
use win32::platform::{mem, user32};

/// Every hook: (address in the version that runs, function).
pub fn all() -> Vec<(u32, Hook)> {
    let table: &[(Us, Hook)] = &[
        (APP_RUN, app_run),
        (ANIM_ADVANCE, anim_advance),
        (MOUSE_MOVE, mouse_move),
        (crate::frames::GAME_CLOCK, crate::frames::game_clock),
    ];
    table.iter().map(|&(a, h)| (a.at(), h)).collect()
}

/// CRecoilApp::Run: the main loop (PeekMessage, else a frame). Until the menu command
/// Game > Start the game only waits for messages.
const APP_RUN: Us = Us(0x442d50);

/// WM_COMMAND ids of the main window's menu (its launcher): Options > Card 1 (Glide),
/// Options > 640 x 480, Game > Start.
const CMD_CARD1: u32 = 0x9c72;
const CMD_640X480: u32 = 0x209;
const CMD_START: u32 = 0x68;

/// The launcher window (a picture and a menu: card, resolution, sound) is skipped: its
/// choices are made here and the game is started. Settings of our own come later.
fn app_run(c: &mut Cpu) {
    let hwnd = user32::main_window();
    for id in [CMD_CARD1, CMD_640X480, CMD_START] {
        user32::post(hwnd, 0x111, id, 0);
    }
    call_original(c, APP_RUN.at());
}

/// Advances an object's time (thiscall, one float: the seconds passed). When it has a video
/// texture (`[this+0x34]`), it first picks that texture's frame: time * rate modulo the frame
/// count (`[tex+0x4c]`). A video that could not be opened (no Indeo 5 decoder yet: the main
/// menu's PumpA / PumpB) has no frames, and the modulo divides by zero.
const ANIM_ADVANCE: Us = Us(0x4c35b0);

/// A video texture without frames is left out for this call: the rest (the object's own time)
/// goes on as before.
fn anim_advance(c: &mut Cpu) {
    let m = mem();
    let this = c.r[ECX];
    let tex = m.rd32(this + 0x34);
    let empty = tex != 0 && m.rd32(tex + 0x4c) == 0;
    if empty {
        m.wr32(this + 0x34, 0);
    }
    call_original(c, ANIM_ADVANCE.at());
    if empty {
        m.wr32(this + 0x34, tex);
    }
}

/// Moves the game's own mouse cursor by DirectInput's motion (`[MOUSE_DX]`, `[MOUSE_DY]`) times
/// a sensitivity, kept on the screen while `[MOUSE_FREE]` is 0 (the menus); then the cursor's
/// place from the screen's centre, scaled (`[MOUSE_REL_X]`, `[MOUSE_REL_Y]`).
const MOUSE_MOVE: Us = Us(0x470570);
const MOUSE_X: Us = Us(0x565b40);
const MOUSE_Y: Us = Us(0x565b44);
const MOUSE_FREE: Us = Us(0x565b6c);
const MOUSE_CENTRE_X: Us = Us(0x565b30);
const MOUSE_CENTRE_Y: Us = Us(0x565b10);
const MOUSE_SCALE_X: Us = Us(0x565b28);
const MOUSE_SCALE_Y: Us = Us(0x565b2c);
const MOUSE_REL_X: Us = Us(0x565b48);
const MOUSE_REL_Y: Us = Us(0x565b4c);

/// In the menus the cursor is where the player's mouse is in the window (the original moves it
/// by the motion only: in a window it would drift away from the system's pointer). The motion
/// itself stays as it was for the game.
fn mouse_move(c: &mut Cpu) {
    call_original(c, MOUSE_MOVE.at());
    let m = mem();
    let menu = m.rd32(MOUSE_FREE.at()) == 0;
    win32::platform::window::cursor_drawn(menu);
    if !menu {
        return;
    }
    let (x, y) = user32::mouse_pos();
    m.wr32(MOUSE_X.at(), x as u32);
    m.wr32(MOUSE_Y.at(), y as u32);
    let rel = |p: i32, centre: Us, scale: Us| {
        ((p - m.rd32(centre.at()) as i32) as f32 * f32::from_bits(m.rd32(scale.at()))).to_bits()
    };
    m.wr32(MOUSE_REL_X.at(), rel(x, MOUSE_CENTRE_X, MOUSE_SCALE_X));
    m.wr32(MOUSE_REL_Y.at(), rel(y, MOUSE_CENTRE_Y, MOUSE_SCALE_Y));
}
