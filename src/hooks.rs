//! Game functions replaced by Rust functions. A hook gets the guest's registers (`Cpu`); its
//! arguments are on the guest stack above the return address (`win32::platform::arg(c, k)`).
//! `call_original(c, address)` runs the game's own function (it pops the return address
//! itself); a hook that does not call it must pop the return address (and, for stdcall, the
//! arguments) and set eax itself.
//!
//! Addresses are the reference version's (`Us(0x...)`), `.at()` gives their place in the
//! version that runs. Write what you learn about each function into data/symbols.txt.

use recomp::rt::{Cpu, Hook, call_original};
use recomp::ver::Us;
use win32::platform::user32;

/// Every hook: (address in the version that runs, function).
pub fn all() -> Vec<(u32, Hook)> {
    let table: &[(Us, Hook)] = &[(APP_RUN, app_run)];
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
