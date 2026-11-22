//! Parts of Recoil's drawing done in Rust, for speed (recomp_kit GUIDE.md 8a, point 4): the
//! same results as the game's code, faster.
//!
//! `see_through` (0x4b13b0) draws the see-through polygons of a picture, farthest first. It
//! sorted them by a bubble sort (O(n²), ~4.5 % of a mission's time unbraked): numbers
//! 0..n of the polygon records into the list `ORDER` (backwards: n-1 first), then passes that
//! swap two neighbours while the second's depth is below the first's, until a pass swaps
//! none. A bubble sort that swaps only on "below" is stable: the same order as a stable sort
//! by depth of the backward list. The hook sorts so and goes on with the game's code where
//! its sort ends (`DRAW_SORTED`, a function entry of its own: `seeds`), with the registers
//! and stack as the game's code has them there. A depth that is not a number (the x87
//! compare finds no order and swaps) keeps the game's bubble sort, done here the same way.

use recomp::rt::{Cpu, EBP, EBX, EDI, ESI, ESP, Hook, call_original};
use recomp::ver::Us;
use win32::platform::mem;

const SEE_THROUGH: Us = Us(0x4b13b0);
/// Where `SEE_THROUGH`'s sort ends: it draws them in `ORDER`'s order.
const DRAW_SORTED: Us = Us(0x4b1457);
/// The number of polygons and the order (u32 record numbers).
const COUNT: Us = Us(0x637f48);
const ORDER: Us = Us(0x637f4c);
/// A record's depth (f32): at `DEPTH + 4116 * k`.
const DEPTH: Us = Us(0x77eaac);
const RECORD: u32 = 4116;

pub fn hooks() -> Vec<(u32, Hook)> {
    vec![(SEE_THROUGH.at(), see_through)]
}

/// Function entries the hooks go on at.
pub fn seeds() -> Vec<(u32, String)> {
    vec![(DRAW_SORTED.at(), "see_through_draw".into())]
}

/// `<PREFIX>_CHECK_SORT=1`: each sort compared with the game's bubble sort (done in Rust).
fn check() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| recomp::config::env_on("CHECK_SORT"))
}

fn depth(k: u32) -> f32 {
    f32::from_bits(mem().rd32(DEPTH.at() + RECORD * k))
}

/// The game's sort: passes over neighbours, swapped when the second is below the first or
/// either is not a number (x87: C0 set when unordered).
fn bubble(order: &mut [u32]) {
    let n = order.len();
    loop {
        let mut swapped = false;
        for i in 0..n.saturating_sub(1) {
            let (a, b) = (depth(order[i]), depth(order[i + 1]));
            if b < a || a.is_nan() || b.is_nan() {
                order.swap(i, i + 1);
                swapped = true;
            }
        }
        if !swapped {
            return;
        }
    }
}

fn see_through(c: &mut Cpu) {
    let m = mem();
    let n = m.rd32(COUNT.at());
    if n == 0 {
        return call_original(c, SEE_THROUGH.at());
    }
    let mut order: Vec<u32> = (0..n).rev().collect();
    let keys: Vec<f32> = order.iter().map(|&k| depth(k)).collect();
    if keys.iter().any(|d| d.is_nan()) {
        bubble(&mut order);
    } else {
        // (stable: equal depths keep their order)
        let mut by: Vec<(f32, u32)> = keys.into_iter().zip(order.iter().copied()).collect();
        by.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        if check() {
            let mut want = order.clone();
            bubble(&mut want);
            let got: Vec<u32> = by.iter().map(|x| x.1).collect();
            assert_eq!(got, want, "see-through sort differs from the game's");
        }
        for (o, x) in order.iter_mut().zip(by) {
            *o = x.1;
        }
    }
    for (k, &o) in order.iter().enumerate() {
        m.wr32(ORDER.at() + 4 * k as u32, o);
    }
    // the game's frame there: return address, 8 bytes of locals (the first: "swapped", 0),
    // ebx, edi, esi, ebp pushed; ebx 0, ebp 1, edi the count
    let mut sp = c.r[ESP] - 8;
    m.wr32(sp, 0);
    m.wr32(sp + 4, 0);
    for r in [EBX, EDI, ESI, EBP] {
        sp -= 4;
        m.wr32(sp, c.r[r]);
    }
    c.r[ESP] = sp;
    (c.r[EBX], c.r[EBP], c.r[EDI]) = (0, 1, n);
    recomp::rt::run(c, DRAW_SORTED.at());
}
