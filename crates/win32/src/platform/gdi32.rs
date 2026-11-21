//! GDI32: the little GDI the game uses (device capabilities; fonts and text on a DC it only
//! uses for diagnostics). Drawing on a DC does nothing.

use super::user32::{DESKTOP_H, DESKTOP_W};
use super::{Import, arg, mem};
use recomp::rt::Cpu;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

/// The one device context: the screen.
pub const SCREEN_DC: u32 = 0xdc00;

fn get_device_caps(c: &mut Cpu) -> u32 {
    match arg(c, 1) {
        8 => DESKTOP_W,  // HORZRES
        10 => DESKTOP_H, // VERTRES
        12 => 16,        // BITSPIXEL
        14 => 1,         // PLANES
        88 | 90 => 96,   // LOGPIXELSX/Y
        116 => 60,       // VREFRESH
        _ => 0,
    }
}

fn get_text_metrics(c: &mut Cpu) -> u32 {
    let p = arg(c, 1);
    let m = mem();
    m.slice(p, 56).fill(0);
    // height, ascent, descent, internal and external leading, average and maximum width, weight
    for (k, v) in [16u32, 13, 3, 0, 0, 8, 16, 400].into_iter().enumerate() {
        m.wr32(p + 4 * k as u32, v);
    }
    1
}

fn handle(_: &mut Cpu) -> u32 {
    0x6600
}

/// A GDI object behind a handle (what GetObjectType tells; drawing is not done yet).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Obj {
    Pen(u32),
    Brush(u32),
    Dc,
    Palette,
    Font,
    /// its BITMAPINFOHEADER in guest memory (a resource's data), 0 when it has none
    Bitmap(u32),
    Region,
}

impl Obj {
    /// GetObjectType's OBJ_ number.
    fn kind(&self) -> u32 {
        match self {
            Obj::Pen(_) => 1,
            Obj::Brush(_) => 2,
            Obj::Dc => 3,
            Obj::Palette => 5,
            Obj::Font => 6,
            Obj::Bitmap(_) => 7,
            Obj::Region => 8,
        }
    }
}

static OBJECTS: Mutex<BTreeMap<u32, Obj>> = Mutex::new(BTreeMap::new());
static NEXT: AtomicU32 = AtomicU32::new(0x6700);

/// A new GDI object's handle.
pub fn new_obj(o: Obj) -> u32 {
    let h = NEXT.fetch_add(4, Ordering::Relaxed);
    OBJECTS.lock().unwrap_or_else(|e| e.into_inner()).insert(h, o);
    h
}

pub fn obj(h: u32) -> Option<Obj> {
    if h == SCREEN_DC {
        return Some(Obj::Dc);
    }
    OBJECTS.lock().unwrap_or_else(|e| e.into_inner()).get(&h).copied()
}

fn create_solid_brush(c: &mut Cpu) -> u32 {
    new_obj(Obj::Brush(arg(c, 0)))
}

fn create_pen(c: &mut Cpu) -> u32 {
    new_obj(Obj::Pen(arg(c, 2)))
}

fn create_font_indirect(_: &mut Cpu) -> u32 {
    new_obj(Obj::Font)
}

fn create_font(_: &mut Cpu) -> u32 {
    new_obj(Obj::Font)
}

fn create_compatible_dc(_: &mut Cpu) -> u32 {
    new_obj(Obj::Dc)
}

fn delete_object(c: &mut Cpu) -> u32 {
    OBJECTS.lock().unwrap_or_else(|e| e.into_inner()).remove(&arg(c, 0)).is_some() as u32
}

fn get_object_type(c: &mut Cpu) -> u32 {
    obj(arg(c, 0)).map_or(0, |o| o.kind())
}

/// The stock objects (GetStockObject's index): fixed handles below the others.
const STOCK: u32 = 0x6600;

fn get_stock_object(c: &mut Cpu) -> u32 {
    let k = arg(c, 0);
    let o = match k {
        0..=5 => Obj::Brush([0xffffff, 0xc0c0c0, 0x808080, 0x404040, 0, 0][k as usize]),
        6..=8 => Obj::Pen([0xffffff, 0, 0][k as usize - 6]),
        15 => Obj::Palette,
        _ => Obj::Font,
    };
    let h = STOCK + 4 * k;
    OBJECTS.lock().unwrap_or_else(|e| e.into_inner()).entry(h).or_insert(o);
    h
}

/// GetSysColor's colours (Windows 98's standard scheme), by COLOR_ index.
pub const SYS_COLORS: [u32; 31] = [
    0xc0c0c0, 0x808000, 0x800000, 0x808080, 0xc0c0c0, 0xffffff, 0x000000, 0x000000, 0x000000, 0xffffff, 0xc0c0c0,
    0xc0c0c0, 0x808080, 0x800000, 0xffffff, 0xc0c0c0, 0x808080, 0x808080, 0x000000, 0xc0c0c0, 0xffffff, 0x000000,
    0xdfdfdf, 0x000000, 0xe1ffff, 0x000000, 0x800000, 0xd08410, 0x808080, 0x800000, 0xc0c0c0,
];

pub fn get_sys_color(c: &mut Cpu) -> u32 {
    SYS_COLORS.get(arg(c, 0) as usize).copied().unwrap_or(0)
}

/// GetSysColorBrush: one brush per colour index, made once.
pub fn get_sys_color_brush(c: &mut Cpu) -> u32 {
    static MADE: Mutex<BTreeMap<u32, u32>> = Mutex::new(BTreeMap::new());
    let k = arg(c, 0);
    let color = SYS_COLORS.get(k as usize).copied().unwrap_or(0);
    *MADE.lock().unwrap_or_else(|e| e.into_inner()).entry(k).or_insert_with(|| new_obj(Obj::Brush(color)))
}

/// GetObjectA for a pen or brush: LOGPEN / LOGBRUSH; others answer 0.
fn get_object(c: &mut Cpu) -> u32 {
    let (h, n, p) = (arg(c, 0), arg(c, 1), arg(c, 2));
    let m = mem();
    match obj(h) {
        Some(Obj::Brush(color)) if n >= 12 => {
            m.wr32(p, 0); // BS_SOLID
            m.wr32(p + 4, color);
            m.wr32(p + 8, 0);
            12
        }
        Some(Obj::Pen(color)) if n >= 16 => {
            m.slice(p, 16).fill(0);
            m.wr32(p + 12, color);
            16
        }
        Some(Obj::Bitmap(info)) if n >= 24 && info != 0 => {
            // BITMAP: type, width, height, bytes a row (even), planes, bits a pixel, bits
            let (w, h, bpp) = (m.rd32(info + 4), m.rd32(info + 8) as i32, m.rd16(info + 14) as u32);
            let colors = match m.rd32(info + 32) {
                0 if bpp <= 8 => 1 << bpp,
                k => k,
            };
            m.wr32(p, 0);
            m.wr32(p + 4, w);
            m.wr32(p + 8, h.unsigned_abs());
            m.wr32(p + 12, (w * bpp).div_ceil(16) * 2);
            m.wr16(p + 16, 1);
            m.wr16(p + 18, bpp as u16);
            m.wr32(p + 20, info + m.rd32(info) + 4 * colors);
            24
        }
        _ => 0,
    }
}

/// LoadBitmapA(module, name): a bitmap of the module's resources.
pub fn load_bitmap(c: &mut Cpu) -> u32 {
    let (h, name) = (arg(c, 0), arg(c, 1));
    match super::resource_in(h, super::ResId::Id(2), super::ResId::from_guest(name, super::cstr)) {
        Some((at, _)) => new_obj(Obj::Bitmap(at)),
        None => 0,
    }
}fn ret0(_: &mut Cpu) -> u32 {
    0
}
fn ret1(_: &mut Cpu) -> u32 {
    1
}

pub const IMPORTS: &[Import] = &[
    ("GetDeviceCaps", 2, get_device_caps),
    ("GetTextMetricsA", 2, get_text_metrics),
    ("GetStockObject", 1, get_stock_object),
    ("CreateFontA", 14, create_font),
    ("CreateFontIndirectA", 1, create_font_indirect),
    ("CreateSolidBrush", 1, create_solid_brush),
    ("CreatePen", 3, create_pen),
    ("CreateCompatibleDC", 1, create_compatible_dc),
    ("DeleteDC", 1, delete_object),
    ("GetObjectType", 1, get_object_type),
    ("GetObjectA", 3, get_object),
    ("SelectObject", 2, handle),
    ("DeleteObject", 1, delete_object),
    ("TextOutA", 5, ret1),
    // drawing on a window's DC is not shown yet (the game's own screens are DirectDraw and
    // Glide)
    ("BitBlt", 9, ret1),
    ("StretchBlt", 11, ret1),
    ("PatBlt", 6, ret1),
    ("StretchDIBits", 13, ret1),
    ("SetBkColor", 2, ret0),
    ("SetBkMode", 2, ret1),
    ("SetTextColor", 2, ret0),
];
