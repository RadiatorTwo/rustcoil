//! The tank's exhaust trails (the light streaks behind it) under its tracks.
//!
//! The effect `bft_exhaust_trail` is a script (a node of type 5) attached to the tank. Its
//! place command (0x45b510, thiscall on the effect instance, the command as argument) puts
//! the node `exhaust_trail` at a reference node with an offset in that node's space: flag 2
//! takes the instance's node [+0x74] (here `left_morphs` / `right_morphs`), flag 8 the offset
//! at command+0x18 (0, 0.2, 2.0); the command's target is the instance's node table [+0x114]
//! (40 bytes each, the node at +0x24) at the index command+0x10. The trail's pieces
//! (`exhaust_trail_fade`) are left where it was. The morph nodes are the tracks' joints, x
//! +-1.55, while the tracks' middle lies about 0.40 further out (their bounding box in the
//! joint's space, node+0x74..+0x88): the streaks lay about a fifth too close together (the
//! original's too). Now the offset's x is the middle of the reference node's box. A fix of the
//! original's: the settings menu's page Fixes turns it off (`option`, setting `FixTrails`); the
//! commands' own offsets are kept to be put back.

use recomp::rt::{Cpu, ECX, ESP, Hook, call_original};
use recomp::ver::Us;
use std::collections::HashMap;
use std::sync::Mutex;
use win32::platform::mem;
use win32::platform::options::Row;
use win32::platform::setting::Setting;

static FIX: Setting = Setting::new("FixTrails", "FIX_TRAILS", true);

/// The place commands changed: their own offset's x (bits).
static OWN_X: Mutex<Option<HashMap<u32, u32>>> = Mutex::new(None);

/// The settings menu's row.
pub fn option() -> Row {
    Row::on_off(
        "Exhaust trail fix",
        "The streaks behind the tank run behind its tracks (the original's too close)",
        || FIX.get(),
        |on| FIX.set(on),
    )
}

/// thiscall(command): an effect script's place command.
const PLACE: Us = Us(0x45b510);

/// Command flags: the reference is the instance's node +0x74; the offset is at +0x18.
const REF_INSTANCE_NODE: u32 = 2;
const OFFSET_GIVEN: u32 = 8;

pub fn hooks() -> Vec<(u32, Hook)> {
    vec![(PLACE.at(), place)]
}

fn place(c: &mut Cpu) {
    let (instance, command) = (c.r[ECX], mem().rd32(c.r[ESP] + 4));
    if instance != 0 && command != 0 {
        under_tracks(instance, command);
    }
    call_original(c, PLACE.at());
}

/// A trail placed at a track's joint goes to the track's middle.
fn under_tracks(instance: u32, command: u32) {
    let m = mem();
    let flags = m.rd32(command + 0xc);
    if flags & (REF_INSTANCE_NODE | OFFSET_GIVEN) != REF_INSTANCE_NODE | OFFSET_GIVEN {
        return;
    }
    let index = m.rd32(command + 0x10) as u16 as i16;
    let table = m.rd32(instance + 0x114);
    let reference = m.rd32(instance + 0x74);
    if index < 0 || table == 0 || reference == 0 {
        return;
    }
    let target = m.rd32(table + 40 * index as u32 + 0x24);
    if target == 0 || name(target) != "exhaust_trail" || !name(reference).ends_with("_morphs") {
        return;
    }
    let mut own = OWN_X.lock().unwrap();
    let own_x = *own
        .get_or_insert_with(HashMap::new)
        .entry(command)
        .or_insert_with(|| m.rd32(command + 0x18));
    let x = if FIX.get() {
        let x = |o: u32| f32::from_bits(m.rd32(reference + o));
        ((x(0x74) + x(0x80)) / 2.0).to_bits()
    } else {
        own_x
    };
    m.wr32(command + 0x18, x);
}

/// A node's name (at its start).
fn name(node: u32) -> String {
    let m = mem();
    (0..32)
        .map(|k| m.rd8(node + k))
        .take_while(|&b| b != 0)
        .map(char::from)
        .collect()
}
