# PLAN — Recoil

The way through: recomp_kit's docs/GUIDE.md. Tick off what is done, note what was learned.
Framework in `crates/`: copied from recomp_kit 7344d58 2026-11-21.

## 1. Start

- [x] EXE's SHA-256 in `VERSIONS`, `Identity` filled in (title, exe, folder, registry key)
- [x] functions found (`rtools missing <exe>` runs; or a Ghidra export in data/functions.json): 5131 found
- [ ] every instruction supported (`rtools missing` lists none; new ones go into recomp with a test)
- [ ] imports listed (`rtools imports <exe>`); the missing ones noted at start

## 2. Window and menu

- [ ] the game opens its window (missing imports implemented in win32, or in this crate)
- [ ] the main menu shows (`RUSTCOIL_SHOT`)
- [ ] the menu answers to mouse and keys (`RUSTCOIL_INPUT`)

## 3. Playing

- [ ] the first level starts and plays
- [ ] sound, music, videos
- [ ] saving and loading
- [ ] the whole game played through

## 4. Port

- [ ] setup from the CD or a CD image (`required`, `video_marker`)
- [ ] high resolution, widescreen, settings
- [ ] gamepad
- [ ] release (tag `v*`)

## Learned

(what was found out and why, dead ends included)

- The CD (data track of the .cue/.bin) holds the game only in an InstallShield 5 cabinet
  (`data1.cab`, header at its start, no data1.hdr); its SETUP.EXE is 16-bit and does not run
  on 64-bit Windows. `rtools iso <bin> --extract <dir>` copies the CD out,
  `rtools iscab data1.cab --extract <dir>` unpacks the cab by file group
  (`crates/win32/src/iscab.rs`, IS5 and IS6). IS5 descriptors carry no MD5 (the bytes after
  the data offset are something else): no check.
- Groups: Game Files (EXEs, maps, zbd), Hardware / Software Textures, Sound Low/Medium/High
  Fidelity, Font Files, Msvc DLLs (MFC42, MSVCRT, MSVCP50, ...), A3D, Indeo 5, AVI Files
  (the videos, also in the CD's Video folder). game_files\Recoil = every group but the
  installer's, A3D, Indeo and AVI.
- Two EXEs: `Recoil.exe` (DirectDraw, software / Direct3D) and `recoil3dfx.exe` (Glide 2,
  `glide2x.dll`, 40 imports). The port uses recoil3dfx.exe: the kit has Glide.
- Registry: `HKLM\SOFTWARE\Westwood\Recoil` (value names: see `RUSTCOIL_TRACE_REG`).
  Westwood Online (WOLAPI) is also looked up.
- Unsupported instructions: MMX only, in 10 functions (0x48d560..0x49ec70), plus noise
  (fbstp/iretd decoded from data in sub_414450). The kit reports no MMX in CPUID, so the
  game should not call them: check before lifting MMX.
- Imports: 193 by ordinal from MFC42.DLL, 88 MSVCRT, 2 MSVCP50, AVIFIL32 / MSVFW32 (the AVI
  videos), the usual Windows and DirectX ones. Plan: load the CD's own MFC42 / MSVCRT /
  MSVCP50 as guest DLLs and recompile them with the EXE (`Image::combine`; black_white's
  `Module` / DllMain loading) instead of writing MFC in Rust.
