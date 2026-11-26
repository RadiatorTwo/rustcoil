//! Recoil recompiled to native code with recomp_kit.
//!
//! ```text
//! rustcoil [path to the EXE]     the game (default: game_files next to the program)
//! ```

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod controls;
mod cut;
mod draw;
mod frames;
mod gamepad;
mod hooks;
mod hud;
mod pictures;
mod reticle;
mod wide;

use recomp::smc::SmcSpec;
use recomp::ver::{Us, Version};
use win32::platform::options;
use win32::{Game, GlideExt, Identity};

/// The versions of the game's EXE this port knows; the first is the reference version: every
/// address in this crate (hooks, symbols.txt) is one of its addresses (`Us(0x...)`).
static VERSIONS: [Version; 1] = [Version {
    sha256: "eba7dab7cc0514ac7998912c53542b5b707c9b907fd754a9a5db694789243658",
    name: "US",
    map: None,
}];

static ID: Identity = Identity {
    title: "Recoil",
    // the prefix of the debug switches (RUSTCOIL_INPUT, RUSTCOIL_SHOT, ...)
    env_prefix: "RUSTCOIL",
    exe: "recoil3dfx.exe",
    versions: &VERSIONS,
    // where the original installer put the game (GetModuleFileName answers with it)
    install_dir: r"C:\Westwood\Recoil",
    // the game folder's name in game_files (and on the CD, if the CD has one)
    folder: "Recoil",
    cd_folder: None,
    // the CD's volume label and serial, if the game checks for its CD (GetVolumeInformationA)
    cd: Some(("RECOIL", 0x1999_0101)),
    computer_name: "PC",
    // the key Westwood's installer made (the EXE reads SOFTWARE\Westwood\Recoil)
    registry_key: r"HKEY_LOCAL_MACHINE\Software\Westwood\Recoil",
    registry_seed: &[],
    // files and folders next to the EXE without which the game cannot start (the MFC and C
    // runtime the installer put beside the game: `dlls`)
    required: &["zbd", "maps", "messages.dll", "MSVCRT.DLL", "MSVCP50.DLL", "mfc42.dll"],
    // other files kept in the game folder: sounds by fidelity (an installed game has one),
    // fonts, saves, the game's logs
    game_files: &[
        "soundsh.zbd",
        "soundsm.zbd",
        "soundsl.zbd",
        "eurosti.ttf",
        "eurostib.ttf",
        "SavedGames",
        "recoil.err",
        "recoil.out",
    ],
    // a file in the CD's video folder (the CD check looks for D:\video\intro_01.avi)
    video_marker: Some("intro_01.avi"),
    // the CD holds the game only in data1.cab: every group but the installer's, the A3D and
    // Indeo drivers and the videos (the CD's Video folder has them)
    cab_left_out: Some(&["Uninstal Files", "A3D DLLs", "Indeo 5 codec", "AVI Files"]),
    // network discovery (DirectPlay over UDP): 8 bytes of the game's own, question and answer
    net_magic: (b"GAMENET?", b"GAMENET!"),
    log_file: "rustcoil.log",
};

struct MyGame;

impl Game for MyGame {
    fn identity(&self) -> &'static Identity {
        &ID
    }

    fn functions(&self, img: &recomp::image::Image) -> Vec<(u32, u32, String)> {
        // a Ghidra export, when there is one (only function boundaries: fine to commit):
        // recomp::image::load_functions(include_str!("../data/functions.json"))
        let mut code = recomp::discover::Code::new(img);
        let mut seeds = vec![(img.entry, "entry".into())];
        seeds.extend(win32::platform::module::seeds());
        seeds.extend(draw::seeds());
        recomp::discover::find_functions(&mut code, &seeds)
    }

    fn dlls(&self) -> &'static [(&'static str, u32)] {
        // the MFC and C runtime from the CD (group "Msvc DLLs"), at their own bases
        &[
            ("MSVCRT.DLL", 0x7800_0000),
            ("MSVCP50.DLL", 0x780c_0000),
            ("mfc42.dll", 0x5f40_0000),
            // the game's messages (it wants 0x10000000, inside our heap)
            ("messages.dll", 0x5f30_0000),
        ]
    }

    fn code_patches(&self) -> Vec<(u32, &'static [u8], &'static [u8])> {
        vec![
            // the CD check at the start (0x42e4c0): 0x4a5aa0 looks for video\intro_01.avi on
            // every CD drive, and without it the game asks for the CD again and again. The
            // CD's files come from game_files (the videos are optional there): the check
            // always passes (jne -> jmp; its answer is not used further)
            (Us(0x42e78a).at(), &[0x75, 0x2d], &[0xeb, 0x2d]),
        ]
    }

    fn smc(&self) -> SmcSpec {
        // self-modifying code beyond `mov [code], x` (see `rtools smc`)
        SmcSpec::default()
    }

    fn hooks(&self) -> Vec<(u32, recomp::rt::Hook)> {
        hooks::all()
    }

    fn glide(&self) -> Option<&'static dyn GlideExt> {
        Some(&frames::FRAMES)
    }

    fn on_start(&self) {
        recomp::image::set_symbols(include_str!("../data/symbols.txt"));
    }

    fn options(&self) -> Vec<options::Page> {
        vec![
            options::Page {
                title: "Display",
                rows: vec![
                    options::fullscreen(),
                    options::window_size(),
                    wide::option(),
                    options::anti_aliasing(),
                    options::textures(),
                    options::frame_rate_display(),
                    frames::option(),
                ],
            },
            options::Page {
                title: "Effects",
                rows: vec![options::bloom()],
            },
            options::Page {
                title: "Keys",
                rows: controls::rows(),
            },
            options::Page {
                title: "Gamepad",
                rows: gamepad::rows(),
            },
        ]
    }
}

static GAME: MyGame = MyGame;

fn main() {
    win32::run(&GAME)
}
