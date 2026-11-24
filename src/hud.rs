//! The HUD drawn again whole when the frame is made anew (a new window size or aspect: it
//! starts black).
//!
//! The HUD is sprites (widgets) drawn into the frame buffer by 0x4b79c0 each picture. One with
//! flag 2 (`+0xc`) is static: drawn only when flag 4 or 8 asks for it (one each, for the two
//! buffers; the draw clears it), so the game draws its bars once and trusts the card's buffers
//! to keep them. Each time the frame is made anew, every such sprite is asked once more
//! (flags 4 and 8), as the game does after a mode change (recomp_kit LESSONS: "Drawing once").

use recomp::rt::{Cpu, ECX, Hook, call_original};
use recomp::ver::Us;
use std::collections::HashMap;
use std::sync::Mutex;
use win32::platform::glide;
use win32::platform::mem;

/// thiscall(time step): a HUD sprite's picture step (draws it when visible and asked).
const SPRITE_STEP: Us = Us(0x4b79c0);
/// The sprite's flags: 2 static (drawn only when asked), 4 and 8 asked, 0x10 hidden.
const FLAGS: u32 = 0xc;
const STATIC: u32 = 2;
const ASK: u32 = 4 | 8;

/// Per sprite, the count of frames made (`glide::targets_made`) when it was last seen.
static SEEN: Mutex<Option<HashMap<u32, u32>>> = Mutex::new(None);

pub fn hooks() -> Vec<(u32, Hook)> {
    vec![(SPRITE_STEP.at(), sprite_step)]
}

fn sprite_step(c: &mut Cpu) {
    let sprite = c.r[ECX];
    let made = glide::targets_made();
    let before = SEEN
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(sprite, made);
    if before.is_some_and(|b| b != made) {
        let m = mem();
        let flags = m.rd32(sprite + FLAGS);
        if flags & STATIC != 0 {
            m.wr32(sprite + FLAGS, flags | ASK);
        }
    }
    call_original(c, SPRITE_STEP.at());
}
