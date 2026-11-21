# PLAN — Recoil

The way through: recomp_kit's docs/GUIDE.md. Tick off what is done, note what was learned.
Framework in `crates/`: copied from recomp_kit 7344d58 2026-11-21.

## 1. Start

- [x] EXE's SHA-256 in `VERSIONS`, `Identity` filled in (title, exe, folder, registry key)
- [x] functions found (`rtools missing <exe>` runs; or a Ghidra export in data/functions.json): 5131 found
- [ ] every instruction supported (`rtools missing` lists none; new ones go into recomp with a test)
- [x] imports listed (`rtools imports <exe>`); the missing ones noted at start

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
- Game DLLs (2026-11-21): `Game::dlls` lists MSVCRT.DLL, MSVCP50.DLL and mfc42.dll from the
  game folder, at their own bases (`platform/module.rs`, after black_white's `Module`).
  They are combined with the EXE (`Image::combine`) and compiled together: 24076 functions,
  about 6 s. Imports between modules are bound to exports (by name or `#ordinal`). The
  DllMains run before the EXE's entry, each after the DLLs it imports. GetModuleHandleA,
  GetModuleFileNameA, LoadLibraryA and GetProcAddress know the game DLLs and system DLLs.
  glide2x.dll is imported by decorated names (`_grBufferSwap@4`): bound through
  `glide::proc_address`.
- Start reaches MFC42's DllMain. MSVCRT's init is through (GetCommandLineW, time,
  GetVersionExA, ...). It stops at USER32!GetSysColor. Next is the MFC window layer:
  GetSysColor and GetSysColorBrush, GDI objects (brushes, pens, DCs), window classes
  (GetClassInfoA), the CBT hook MFC sets around CreateWindowEx (`AfxHookWindowCreate`:
  SetWindowsHookExA(WH_CBT) and subclassing by SetWindowLongA(GWL_WNDPROC)), CallWindowProcA,
  window properties. 423 imports are missing at start; most are never called.
- MFC window layer (2026-11-21): user32 classes with their WNDCLASS (GetClassInfoA), the WH_CBT
  hook called in CreateWindowExA before WM_NCCREATE (MFC subclasses there), window extra bytes,
  properties, CallWindowProcA, rectangles, DeferWindowPos; menus as items (`platform/menu.rs`,
  MENU resources parsed); gdi32 objects (pens, brushes, fonts, DCs, bitmaps from resources;
  nothing is drawn yet); resources of any module (`resource_in`, FindResourceA, LoadStringA,
  message tables for FormatMessageA); FindFirstFileA and file information; system DLL ordinals
  (DSOUND #1, WS2_32 #115/#116, DPLAYX #4).
- messages.dll (the game's texts, a message table) wants base 0x10000000, inside our heap: it is
  loaded at 0x5f300000.
- The CD check: GetLogicalDriveStringsA, D: is the CD, label `RECOIL` (`Identity::cd`), and
  `D:\video\intro_01.avi` must exist: the CD's Video folder is copied to `game_files\Video`
  (`host_path` looks in the game folder's parent).
- DirectX 6 check (GetDXVersion at 0x40c38d): QueryInterface of IDirectDrawSurface4 on a surface
  answers with the surface itself (the IID is used nowhere else in the EXE).
- Now: the game runs its main loop (0x442d50: PeekMessage, else a frame), but nothing starts.
  `[app+0xD0]` stays 0, so it calls WaitMessage. 0x442c60 (from 0x42e4c0) and 0x442cc0 set it.
  They are reached from WM_COMMAND handlers: 0x65 (0x4307c0), 0x9c51, 0x9c53 (Multi Player
  Start), 0x9c55.. (campaigns). The menu that holds them (Game / Campaigns / Options / Help, a
  developer menu) is removed at start (RemoveMenu). Open: what sends the start command in the
  release? Candidates: a dialog, the end of the intro video, or the registry
  (HKCU\SOFTWARE\Zipper\RECOIL\1.0, which MFC opens for its settings). The main window draws
  the bitmap GAMEBMP (648x507) with StretchBlt; GDI drawing is not shown yet. The game writes
  recoil.err and recoil.out (empty so far). Glide is not opened yet.