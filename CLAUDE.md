# CLAUDE.md — Recoil in Rust

Hand-over for future sessions. What is done and what is open: **PLAN.md** (keep it current).

## Goal

A native port of Recoil (YEAR) with recomp_kit: the player's own EXE recompiled to native
code (Cranelift), Windows and DirectX in Rust (`win32` crate), playable on Windows and Linux.
Only code is shared; players supply their own game files. **Never commit anything derived
from the EXE** (translated code, extracted data). `game_files/` is git-ignored. Function
boundaries (`data/functions.json`) and our notes by address (`data/symbols.txt`) are fine.

## The user

- New to reverse engineering; explain in plain words, give a recommendation, not a survey.
- Speaks German; works in short sessions and watches usage limits.
- Confirm before anything outward-facing or destructive (pushing, deleting).
- Clean Rust first, performance later.
- The GPU is never the limit with this old game: a low frame rate is CPU side (game code,
  our code, draw calls, driver). Measure (recomp_kit GUIDE 8a).
- Better than the original: glitches are fixed whether or not the original has them.
- No environment variables for what the user tests by hand (cheat keys: Cargo feature
  `cheats`, never in a release; settings: menu or registry). Replay the user's recordings with
  exactly their settings.

## How it works

- The framework, copied from recomp_kit into `crates/` (`recomp`: discovery, lifter, JIT,
  runtime; `win32`: the Windows layer; `tools`: `rtools`). This project's own copy: change it
  freely, with a test for the lifter (`crates/recomp/tests/`). recomp_kit's docs/GUIDE.md is
  the way through a new port, docs/LESSONS.md the known traps. A fix that other games need
  too: also into recomp_kit (ask the user).
- `src/main.rs`: `Identity` (names, paths, registry key, CD) and `impl Game` (functions,
  self-modifying code, hooks, extensions).
- `src/hooks.rs`: game functions replaced by Rust (`call_original` runs the game's own).
- `data/symbols.txt`: what we know about the game's functions and data, by address.
- Switches (prefix `RUSTCOIL_`): `TRACE_API=1` every import call, `TRACE_MSG=1` window
  messages, `WATCH=1` where the game is every 2 s, `SHOT=file.png` screenshots,
  `INPUT="5000:click@320,240 8000:Enter"` scripted input, `RECORD=file.txt` records input,
  `NO_VIDEO=1`, `WINDOW=1280x960`, `COUNT=file` calls per function.

## Known facts about the game

Details and the history: PLAN.md, section "Learned".

- Game files: the CD image (.cue + 5 .bin) is in `game_files/`; the game only in an
  InstallShield 5 cab (`rtools iso <bin> --extract <dir>`, `rtools iscab data1.cab --extract
  <dir>`). `game_files/Recoil` = the cab's groups; `game_files/Video` = the CD's Video folder.
- EXE: `recoil3dfx.exe` (Glide 2), an MFC 4.2 application. `Game::dlls` loads MSVCRT.DLL,
  MSVCP50.DLL, mfc42.dll (own bases) and messages.dll (at 0x5f300000; it wants 0x10000000,
  inside our heap). They are compiled with the EXE (`platform/module.rs`).
- CD check: drive D:, label `RECOIL`, `D:\video\intro_01.avi`. DirectX 6 check at 0x40c38d.
- Launcher: the main window (GAMEBMP + menu) is skipped. The hook on CRecoilApp::Run (0x442d50,
  `src/hooks.rs`) posts WM_COMMAND Card 1 (0x9c72), 640x480 (0x209), Start (0x68).
- Fonts: built by the game with GDI text into a DIB (`platform/text.rs`, Eurostile .ttf).
- Videos: AVI Indeo 5, not played yet (AVI open fails, the game goes on). CD music: MCI
  cdaudio reads the audio tracks straight from the .cue/.bin in game_files (`platform/cdaudio.rs`).
- The game writes `recoil.err` / `recoil.out` in its folder.
- Test run: `target\release\rustcoil.exe game_files\Recoil\recoil3dfx.exe` with
  `RUSTCOIL_SHOT=...{t}.png`; it reaches the main menu in about 50 s (no video).

## Next steps

See PLAN.md. Now: playing further missions, saving and loading; later an Indeo 5 decoder or
converting the videos at setup.
