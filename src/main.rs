//! Recoil recompiled to native code with recomp_kit.
//!
//! ```text
//! rustcoil [path to the EXE]     the game (default: game_files next to the program)
//! ```

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hooks;

use recomp::smc::SmcSpec;
use recomp::ver::Version;
use win32::{Game, Identity};

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
    cd: None,
    computer_name: "PC",
    // the key Westwood's installer made (the EXE reads SOFTWARE\Westwood\Recoil)
    registry_key: r"HKEY_LOCAL_MACHINE\Software\Westwood\Recoil",
    registry_seed: &[],
    // files and folders next to the EXE without which the game cannot start
    required: &["zbd", "maps"],
    // other files kept in the game folder (saves, the game's own settings)
    game_files: &[],
    // a file in the CD's video folder, if the game has videos there
    video_marker: None,
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
        recomp::discover::find_functions(&mut code, &seeds)
    }

    fn dlls(&self) -> &'static [(&'static str, u32)] {
        // the MFC and C runtime from the CD (group "Msvc DLLs"), at their own bases
        &[("MSVCRT.DLL", 0x7800_0000), ("MSVCP50.DLL", 0x780c_0000), ("mfc42.dll", 0x5f40_0000)]
    }

    fn smc(&self) -> SmcSpec {
        // self-modifying code beyond `mov [code], x` (see `rtools smc`)
        SmcSpec::default()
    }

    fn hooks(&self) -> Vec<(u32, recomp::rt::Hook)> {
        hooks::all()
    }

    fn on_start(&self) {
        recomp::image::set_symbols(include_str!("../data/symbols.txt"));
    }
}

static GAME: MyGame = MyGame;

fn main() {
    win32::run(&GAME)
}
