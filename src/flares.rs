//! Light flares (the lighthouse's glow) that do not go out while the player shoots.
//!
//! The drawing collects the picture's lights into a list (0x633964, 20 bytes each, count
//! 0x633960, room for 650; added by 0x49a880 from the meshes' draws). After the world the game
//! picks the lights that have a flare (+0x10 points to a flare, its +0xc set) into a list of 64
//! pointers (0x636c30) and draws their glow: 0x49aa10 (the view 0x44d650, a ray tests each
//! flare) or 0x49a970 (the view 0x44d3f0). Both look only at the list's first 64 lights,
//! whatever they are. Every shot and its sparks bring about 30 lights, all before the
//! lighthouse's in the list: from the third shot on its light lay beyond 64 and its glow was
//! left out in that picture, so it flickered while the player fired (the original does too).
//! Now the lights with a flare are moved to the list's front first (their order kept), so the
//! 64 places go to flares. A fix of the original's: the settings menu's page Fixes turns it
//! off (`option`, setting `FixFlares`).

use recomp::rt::{Cpu, Hook, call_original};
use recomp::ver::Us;
use win32::platform::mem;
use win32::platform::options::Row;
use win32::platform::setting::Setting;

static FIX: Setting = Setting::new("FixFlares", "FIX_FLARES", true);

/// The settings menu's row.
pub fn option() -> Row {
    Row::on_off(
        "Light count fix",
        "Past 64 lights glows stay on (the lighthouse's flickered while firing)",
        || FIX.get(),
        |on| FIX.set(on),
    )
}

/// The picture's lights: count, then 20 bytes each.
const LIGHT_COUNT: Us = Us(0x633960);
const LIGHTS: Us = Us(0x633964);
const LIGHT_SIZE: u32 = 0x14;
/// fastcall: the flares to draw, from the lights (view 0x44d650).
const PICK_FLARES: Us = Us(0x49aa10);
/// The same for view 0x44d3f0, which also drops the lights its test finds hidden.
const PICK_FLARES_TESTED: Us = Us(0x49a970);

pub fn hooks() -> Vec<(u32, Hook)> {
    vec![
        (PICK_FLARES.at(), pick_flares),
        (PICK_FLARES_TESTED.at(), pick_flares_tested),
    ]
}

/// The number of lights now (`set_light_count` puts it back).
pub fn light_count() -> u32 {
    mem().rd32(LIGHT_COUNT.at())
}

pub fn set_light_count(n: u32) {
    mem().wr32(LIGHT_COUNT.at(), n);
}

fn pick_flares(c: &mut Cpu) {
    if FIX.get() {
        flares_first();
    }
    call_original(c, PICK_FLARES.at());
}

fn pick_flares_tested(c: &mut Cpu) {
    if FIX.get() {
        flares_first();
    }
    call_original(c, PICK_FLARES_TESTED.at());
}

/// Moves the lights with a flare to the list's front, both groups in their order.
fn flares_first() {
    let m = mem();
    let n = light_count();
    let lights = m.slice(LIGHTS.at(), (n * LIGHT_SIZE) as usize).to_vec();
    let has_flare = |light: &[u8]| {
        let flare = u32::from_le_bytes(light[0x10..0x14].try_into().unwrap());
        flare != 0 && m.rd32(flare + 0xc) != 0
    };
    let (flares, rest): (Vec<&[u8]>, Vec<&[u8]>) = lights
        .chunks(LIGHT_SIZE as usize)
        .partition(|light| has_flare(light));
    if flares.is_empty() {
        return;
    }
    let sorted: Vec<u8> = flares.into_iter().chain(rest).flatten().copied().collect();
    m.slice(LIGHTS.at(), sorted.len()).copy_from_slice(&sorted);
}
