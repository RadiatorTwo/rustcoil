# PLAN — Recoil

The way through: recomp_kit's docs/GUIDE.md. Tick off what is done, note what was learned.
Framework in `crates/`: copied from recomp_kit 7344d58 2026-11-21; its changes went back into recomp_kit 1dbe3cf.

## 1. Start

- [x] EXE's SHA-256 in `VERSIONS`, `Identity` filled in (title, exe, folder, registry key)
- [x] functions found (`rtools missing <exe>` runs; or a Ghidra export in data/functions.json): 5131 found
- [ ] every instruction supported (`rtools missing` lists none; new ones go into recomp with a test)
- [x] imports listed (`rtools imports <exe>`); the missing ones noted at start

## 2. Window and menu

- [x] the game opens its window (missing imports implemented in win32, or in this crate)
- [x] the main menu shows (`RUSTCOIL_SHOT`)
- [x] the menu answers to the mouse (`RUSTCOIL_INPUT`); keys in the menu: not tried

## 3. Playing

- [ ] the first level starts and plays (it starts, textured; driving and fighting not tried)
- [ ] sound, music, videos (sound effects and CD music work; videos not)
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
- Solved: the "settings dialog" of the original is the main window itself: GAMEBMP and a menu
  (Game: Start / Multi Player Start / Westwood Online / Exit; Options: Software - GameZ, Card 1-3,
  320x200 .. 1024x768, DirectSound / A3D, Fullscreen, Full HUD, CD Audio, Joystick; Help). Only
  Campaigns (position 1) is removed. Game > Start is 0x68 (0x430790 -> 0x42e4c0). The hook on
  CRecoilApp::Run (0x442d50, `src/hooks.rs`) posts Card 1, 640 x 480 and Start: the launcher is
  skipped. A settings screen of our own comes later (registry or menu, no environment variables).
- After Start: MCI (CD audio, MCI_OPEN) answers "no device" for now (the music is the CD's audio
  tracks 2-5: later, from the .bin files). Glide opens 640x480. Missing in the kit's Glide:
  grSstOrigin, grCullMode, grDepthBufferFunction, grDepthMask, grDepthBiasLevel, grFogMode,
  grFogColorValue, grColorMask, grRenderBuffer, grGlideGetState / SetState,
  grDrawPolygonVertexList, grLfbLock / Unlock, grLfbReadRegion / WriteRegion (nuclear_rust_strike
  has the region functions). The picture is black until they are there.
- Glide for a real 3D game (2026-11-21, this project's copy): perspective-correct textures
  (sow, tow, oow interpolated linearly on the screen and divided per pixel; affine games send
  oow 1 and look the same), a depth buffer (DEPTH32F renderbuffers beside the colour and the
  multisampled one; `gl_FragDepth`: z-buffer ooz / 65535, w-buffer 1 / (w + 1) with the depth
  functions turned round; bias, mask, clear values), grCullMode (Glide's signed area, on the
  CPU), grSstOrigin (lower left: y flipped, clip windows too), fog with iterated alpha (mult2 /
  add2), grColorMask, Get/SetState (kept by the GrState's address), grDrawPolygonVertexList (a
  fan), the linear frame buffer (grLfbLock hands out a guest buffer at a 2048-byte stride: read
  locks get the picture so far, write locks start as 0x0821 and the changed pixels are drawn
  over the picture at grLfbUnlock; Read/WriteRegion).
- GDI text (2026-11-21): the game makes its bitmap fonts with GDI (zimg_fonts.cpp: a 16-bit
  top-down DIB section, TextOutA per character, GetTextExtentPoint32A). gdi32 now keeps DC
  state (selected objects, text and background colour, background mode, alignment), DIB
  sections, PatBlt, and draws text with TrueType fonts (`platform/text.rs`, ab_glyph: the game
  folder's .ttf by family name, Eurostile here; else Windows' fonts, Arial). Smooth edges.
- Videos are AVI with Indeo Video 5 (IV50) and sound: no decoder yet. AVIStreamOpenFromFileA
  fails (AVIERR_FILEOPEN) and the game goes on without them (zipperlogo, intro_01). Options
  later: an Indeo 5 decoder in Rust (FFmpeg's is LGPL C: a port would be a large job), or
  converting the videos once at setup.
- **The main menu shows** (Glide, 640x480): New Game, Load Game, Options, Controls, Credits,
  Quit. Next: input in the menu, starting a mission, sound, CD music.
- Menu input (2026-11-21): the menu's PumpA / PumpB are videos drawn as textures; unopened (no
  Indeo 5) they have no frames and object_advance_time (0x4c35b0) divided by zero: the hook leaves
  such a texture out. The mouse is DirectInput (GetDeviceState, DIMOUSESTATE): the game moves its
  own cursor ([0x565b40], [0x565b44]) by the motion times a sensitivity (about 1.2), so in a
  window it drifted from the system pointer. The hook on mouse_move (0x470570) puts it where the
  pointer is while the cursor is kept on the screen ([0x565b6c] = 0: menus); the motion stays for
  the game. Glide draws at the window's size, so window positions were in its pixels, not the
  game's 640x480: `window::set_screen` (from grSstWinOpen) maps them to the game's screen.
  DrawTextA (word wrap, alignment, DT_CALCRECT) and GetObjectA of a DIB section (bmBits, the
  DIBSECTION header) were missing. Main menu > New Game (name, difficulty) > Start reaches the
  campaign briefing (Campaign 1, Mission 3 "Island Storm" shown, map). Seen: pieces of the New
  Game screen ("MED", "HA") stay in the briefing's empty places: to check (LFB, or a clear).
- First mission (2026-11-22): the briefing waits for a key with Sleep and DirectInput's
  GetDeviceData (0x404120), without reading messages: DirectInput reads now fetch the window's
  input themselves (`user32::poll_input`). Then the mission loads and runs. Glide fixes in the
  kit's copy: (1) the game downloads 621 textures at once; 64 layers dropped most: the arrays
  grow (64 .. 2048 layers). (2) glGenerateMipmap over the whole array after every download took
  the NVIDIA driver down ("Unable to recover from a kernel exception", exit 0xC0000409): mipmaps
  are made on the CPU per texture (`half`). (3) Glide's s/t go 0..256 over the longer side for
  every texture size: scaled by size / 256 (the earlier games had 256x256 only). (4) Recoil
  leaves tmu 0's oow at 0: the texture rectangle used 1 instead of the vertex's oow and clamped
  everything to a texel or two. (5) Repeated textures (ground) read outside the texture in the
  layer with mipmaps on: wrapped inside the texture in the shader. A scripted run
  (`RUSTCOIL_INPUT`) no longer catches the user's mouse.
  Black vehicles (2026-11-22): the shader read the texture layer from 8 bits (`layer << 8`, the
  replacement's layer above it): with more than 256 layers the late textures (the vehicles) read
  a wrong layer. 12 bits now (up to MAX_LAYERS). Clamped texture rectangles stop at the
  texture's edge (Glide's clamp). A scripted run ignores the user's mouse and keys over the
  window (the cursor followed the real pointer and the clicks missed).
  Seen while testing: with `RUSTCOIL_SHOT` and the present thread, pictures sometimes stop
  (the game goes on): use `RUSTCOIL_PRESENT=0` for screenshot runs until that is found.
  Tested by hand (2026-11-22): the mission looks and plays right; the picture at the lower left
  is a target zoom (not a radar), the mission screen shows its pictures. grTexCombine: Recoil sets
  rgb LOCAL, alpha ZERO for every texture; the texture's alpha now reads 0 (it warned on every
  call before).
- Lost game files (2026-11-22): a start without the EXE's path runs the kit's setup, and its
  cleanup ("what the game does not need") put everything in game_files into the recycle bin that
  `Identity` did not list: the CD image and all of Recoil's files but the EXE, maps and zbd.
  The cleanup is gone (setup removes nothing now); DeleteFileA only deletes inside the game
  folder, and nothing is written outside it. Restored from the CD image (rtools iso / iscab, the
  groups as above). ShowOwnedPopups and WinHelpA (closing) added. Closing the window ended
  nothing: DestroyWindow sent WM_DESTROY but not WM_NCDESTROY, where MFC posts WM_QUIT for its
  main window. It sends both now; the game ends with ExitProcess(0).
- Real mouse in the menus (2026-11-22): the game hides the system cursor and draws its own, and
  the kit caught a hidden cursor; winit then keeps it in the window's centre on Windows, so
  only raw motion arrived and the menu cursor stood at (320, 240). The hook on mouse_move says
  whether the game draws its cursor at the mouse (`window::cursor_drawn`, menus): then the
  cursor is hidden but not caught. In missions it is caught as before.
- CD music (2026-11-22): `platform/cdaudio.rs` (from Hogs of War's port) answers MCI cdaudio
  from the CD image: the .cue in the game folder or the one above it, its .bin tracks read when
  played (2352-byte sectors, 16-bit stereo 44.1 kHz) and mixed by dsound.rs. Nothing extracted.
  Recoil opens the device, plays track 2 in the menu and sets the volume with aux*
  (auxGetNumDevs, auxGetDevCapsA, auxGetVolume, auxSetVolume: one CD device).
