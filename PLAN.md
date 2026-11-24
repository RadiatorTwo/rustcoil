# PLAN — Recoil

The way through: recomp_kit's docs/GUIDE.md. Tick off what is done, note what was learned.
Framework in `crates/`: copied from recomp_kit 7344d58 2026-11-21; its changes went back into recomp_kit 1dbe3cf, the frame rate work (2026-11-22) into e5137f9, the speed work into 05224a2 and fbd499f, the missing imports (2026-11-23) into 74e63c5.

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

- [x] the first level starts and plays (played through by hand 2026-11-23; level 2 loads)
- [x] sound, music, videos
- [x] saving and loading (tried by hand 2026-11-23: a save in level 2 loads)
- [ ] the whole game played through

## 4. Port

- [ ] setup from the CD or a CD image (`required`, `video_marker`)
- [x] high frame rate (GUIDE 8b): logic fixed at 60 a second, pictures at the display's
  refresh rate between logic frames, interpolated (objects and camera)
- [ ] speed (GUIDE 8a): ~400 to ~850 pictures a second unpaced in the first mission
  (2026-11-22); next: 0x487f90, 0x477bb0, 0x4b0370, 0x47f050 (~35 %, spread thin)
- [x] frame rate display: F12 (setting `ShowFps`, saved in the registry; the kit's `stats.rs`)
- [ ] high resolution, widescreen, settings (resolution: drawn at the window's size or
  `RenderSize`; widescreen Hor+ in missions, `src/wide.rs`, 2026-11-23; a settings menu: not yet)
  Widescreen, open (the user's test, 2026-11-24):
  - [x] aiming: the aim no longer matches where the projectiles go
  - [x] the mouse stops where the old 4:3 picture's edge was (our own reticle, `reticle.rs`)
  - [x] the target zoom's 3D lands outside the HUD (at the wide picture's lower left)
  - [x] resizing the window blacks out the HUD: it is not drawn again, only the parts that
    change (health bars) (`hud.rs`)
  - [ ] effects (rare, hard to make) draw over the HUD's bars and stay there (the bars are
    drawn once); idea: cut the 3D picture's effects to the view's rows 26..396 (0x4e9c44)
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
  game should not call them: check before lifting MMX. Checked (2026-11-23, `RUSTCOIL_COUNT`
  over `game_files/turn.txt`): they are the software renderer's 15/16-bit span drawers (MMX
  versions picked by bit 4 of [0x637070] in 0x48fdd0 and 0x48d7f0, for 555/565 by
  [0x582d98]). With Glide neither they nor their plain versions (0x49e710, 0x49c3b0, ...)
  are ever called: lifting MMX would gain nothing.
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
- Saving crashed (2026-11-22): "call to unknown address 0x41f6f0", a callback registered with
  `mov edx, 0x41f6f0` (0x41f609). Discovery had a word of data (0x41f886) pointing at the last
  byte of a jne in that function: it decodes as `call far [edi+5Eh]`, was taken as a function,
  and the real one no longer fit. Far calls and jumps through memory now count as "not compiled
  code" (`plausible`, test `far_call_is_no_function`); the only change in the list: 0x41f886 out,
  0x41f6f0 in.
- Music in missions (2026-11-22): Recoil plays one track with MCI_NOTIFY (0x4a2680, mode 5)
  and starts it again on MM_MCINOTIFY successful (0x4a2730, from the window procedure): without
  the notification every piece played once and then stayed silent. The message loop now posts
  it when the track has ended (`cdaudio::poll`); a new play or a stop sends superseded /
  aborted. Menu: track 2; missions: track 2 + n mod 3 (0x42f239). Tracks 3 and 4 begin with
  35-45 s of very quiet music. Checked: track 2 starts again after 213 s (its length).
- Videos (2026-11-22): Recoil plays its AVIs itself through Video for Windows (0x463f70):
  AVIStreamOpenFromFileA / InfoA / ReadFormat / Read for the video and PCM sound streams,
  ICLocate + ICDecompress (cdecl) to its 16-bit BI_BITFIELDS or 24-bit format, frame by frame;
  the sound is read in one piece and played through DirectSound. `platform/avi.rs` reads the
  RIFF file and answers these; codecs: Cinepak (`platform/cinepak.rs`, written here: zipperlogo,
  pumpa, pumpb) and Indeo 5 (`crates/indeo`: FFmpeg's indeo5/ivi ported to Rust, LGPL; equal to
  FFmpeg on all 9077 pictures of the 8 IV50 videos, which are all key frames: motion
  compensation is ported but not exercised). The intro's stripes are in the video itself.
- Skipping videos (2026-11-22): Space skipped only now and then. Recoil reads the keyboard
  device's buffered data in two places in turn: 0x46fa90 (returns a pressed key: the video
  skips) and 0x46f710 (the game's key table, up to 128 events). Our DirectInput reads fetched
  the window's input themselves, so a new key event went to whichever read came first (about
  half the time the wrong one). The input is now fetched by the message loop (PeekMessage comes
  right before 0x46fa90) and by a DirectInput read only when the loop has not run for 20 ms
  (the briefing waits without messages). Each device object also has its own event buffer now
  (Acquire cleared the shared one). Checked: Space at 7 different times skips every time
  (before: 2 of 7).
- Tank turning far too fast (2026-11-22): not the input change (the user's recording played
  the same with the build before it). Recoil ran unbraked, ~550 pictures a second in a mission;
  its turning is not made for that (it times frames with timeGetTime, 0x463140, but something
  inside does not scale). Replayed at 30 fps the tank turns evenly over ~2.5 s. Pictures are
  now paced to at most 60 a second (`Frames` in main.rs, Glide's `frame_shown`). Open: whether
  60 turns as the original did (the user checks; 30 is the fallback).
- Logic at a fixed rate (2026-11-22): the mission's clock is 0x4a5750 (GetTickCount, not
  timeGetTime: 0x463140 is a sequence player's clock), called from the mission frame
  0x42f2c0; it keeps the step in [0x56f2e4]. With an exact fixed step (`frames.rs`) the
  tank turns alike at 30, 60 and 120 logic frames a second (~197 degrees a second, the
  user's recording), faster at 240 (250) and 500 (333): below a step of ~1/120 s some logic no
  longer scales (a least step a frame?). PCGamingWiki agrees: the original Glide renderer was
  capped at 63 fps, and at 120+ fps the game runs very fast. The logic now runs at exactly 60 frames a second in
  a mission; menus and briefings keep the real clock. The tank's matrix (3x3 floats, then
  x, y, z) was at 0x111bd70 in that replay (heap: found by comparing memory snapshots).
  Next: pictures between logic frames (GUIDE 8b steps 2-6); the mission frame mixes logic
  and drawing.
- Pictures between logic frames (2026-11-22): the mission frame 0x42f2c0 is clock, input
  (0x471e60, 0x46fa90, 0x46fa00), logic (0x44eab0: the only call that moves the tank), drawing
  (0x44f670 the world; the HUD into the locked frame buffer 0x4a6820..0x4a6880), swap
  (0x4a69b0). A picture runs the mission frame again without clock, input and logic, the time
  step 0. The engine is Zipper's (as MechWarrior 3): nodes of 0xc4 bytes in lists by type at
  [0x4e1ef8 + 4k] (6: all, 8: the camera); node type at +0x34, data at +0x38. The drawing works
  from an object's (type 5) Euler angles (data+0x18), matrix (+0x30) and place (+0x54), and
  from the camera's (type 1) place and angles (data+0x14/+0x20, again at +0x2c/+0x38): moved
  only while drawing, they are drawn there (`pictures.rs`). The tank turns evenly per picture
  at 120, 144 and 240 Hz (steps within 2%). The kit's LogicClock gave the logic frame's own
  swap when it came less than a period before the frame was due: alpha was cut to 0, steps
  alternated 1.80/1.54 degrees; now pictures go on until one is shown at the due time. At 480
  Hz the game reaches ~400 pictures a second (the logic frame's picture misses a refresh now
  and then): performance work later.
- Objects lagged and jumped between logic frames (2026-11-22, the user: only the tank and
  other vehicles, never the surroundings; not on screenshots): an object's data holds its own
  matrix and place (+0x30, +0x54) and the same in the world (+0x60, +0x84), from which the
  drawing draws. A root object's world values (parent: the world) the logic writes; a
  child's the drawing makes from its parent's (world matrix = own matrix x parent's world
  matrix, world place = own place x parent's world matrix + parent's world place, row
  vectors) only for nodes the logic marked changed (node+0x2c bit 4), so in a logic frame's
  picture alone. Measured from what the drawing used (camera matrix +0x44/+0x68): the tank
  stood still in the pictures between and was a frame ahead in each logic frame's. Now every
  picture interpolates a root's world values too (only where they equal its own: exhaust
  trails, roots whose world values the logic sets apart, were stretched across the picture)
  and makes the children's by the rule (`derived`: children that fit the rule and whose world
  values the logic left, or set exactly by the rule; effects the logic moves are left);
  after the picture the newest values are put back and the children's made from them. Wrong
  turns: putting back children's world values noted at the logic frame (a frame old: objects
  and pictures drifted apart); marking nodes changed in a picture (the drawing then made only
  their bounding spheres again, not the world values). Tank and turret now move evenly in
  camera space (change of speed per picture ~0.00005, was ~0.0015). On the way: pictures are
  paced before they are shown (`GlideExt::before_shown`), kept halfway between the display's
  refreshes (`Pacer::pace_on`, `window::vblank`), at most 240 a second.
- Speed (2026-11-22, GUIDE 8a): measured with `RUSTCOIL_UNPACED=1` (pictures as fast as the
  game draws them, vsync off, at most 16 a logic frame) and `RUSTCOIL_SAMPLE=1` (it now logs
  the last second's frames a second too) over the replay `game_files/turn.txt`; our own code
  resolved by a link map (`cargo rustc --release --bin rustcoil -- -C link-arg=/MAP:<file>`,
  own target directory). From ~400 to ~650 in the mission (runs differ by ~10 %):
  grLfbUnlock converted and uploaded the whole 640x480 locked buffer every picture for a
  HUD of ~5000 pixels (~0.6 ms): now only bands of rows the game wrote, scanned four pixels
  at a time; GetTextExtentPoint32A (~1 a picture) looked for the font file each time (a
  file opened by `Path::exists`): the pick is kept per font; `pictures::noted` kept ~8200
  objects in a hash map (~1.1 ms a logic frame, as long as the game's logic): now a list in
  the node list's order (~0.65 ms, the rest is reading the objects). A logic frame's picture
  takes ~3 ms (logic ~1.1, noted, drawing ~1.4), so `MAX_FPS` stays 240 on a 480 Hz display.
- Speed, part 2 (2026-11-22): the five hot functions' profile was flat (no instruction over
  ~1 %), so first a profile by guest instruction: the lifter notes each instruction's
  address on its Cranelift code (`SourceLoc`, kept in the JIT cache, `jit::guest_at`),
  `RUSTCOIL_SAMPLE=insn` logs the hottest instructions, `RUSTCOIL_DUMP=<hex>` marks them in
  the machine code (`RUSTCOIL_DUMP_CLIF=1` the IR too). Found: (1) 0x4b13b0 sorts the
  see-through polygons by depth with a bubble sort: `src/draw.rs` sorts them (stable, the
  same order; `RUSTCOIL_CHECK_SORT=1` compares with the bubble sort) and goes on in the
  game's code at 0x4b1457 (a function entry of its own, `draw::seeds`, with the frame the
  game's code has there). (2) The x87 arithmetic under the usual control word (Recoil:
  0x37f) branched on it in every operation; the vertex loop at 0x4b0525 (~40 instructions)
  became ~7700 machine instructions with its hot path spread through them. Straight runs of
  x87 code now get a version for the usual control words (`usual87`, ~410 instructions):
  ~743 to ~826 by A/B (`RUSTCOIL_NO_USUAL87=1` turns it off). (3) Flags only where read
  (`flag_liveness`, test `flags_live.rs`; `RUSTCOIL_FLAGS_ALL=1` off): flags were computed
  after every instruction and kept as loop block parameters; leaving dead ones undefined
  made it 10 % slower (old values lived on), defining them 0 is even. Loop heads no longer
  store the registers for `rt_yield`. Unpaced measuring allows 64 pictures a logic frame
  (16 capped it at 960).
- Level 1 played through by hand (2026-11-23): at its end the game crashed on the missing
  KERNEL32!CompareFileTime (0x4346b0 compares two FILETIMEs at +0x14 of found files, the
  last write time: it sorts files by age). Implemented; level 2 then loads.
- The EXE's own imports are all there now (2026-11-23): GetTempPathA (the game folder; for
  gamez.err/out), MessageBoxExA (= MessageBoxA), OpenIcon, GetSystemPaletteEntries (no
  palette device), FindExecutableA / ShellExecuteA (Docs\Index.html, the manual, opened by
  the host's own program), GetOpenFileNameA (no dialog: cancelled; the skipped launcher's),
  WSAStartup / WSACleanup (only the version is read), DirectPlayLobbyCreateA (no lobby:
  DPERR_UNAVAILABLE). The 196 still missing are MFC's and MSVCRT's, unused so far.
- Widescreen (2026-11-23, GUIDE 8c): the projection is 0x474480 (stdcall: left, top, half
  width, half height, x scale, y scale, near, far), called only by the view setup 0x479d60;
  one view a picture (mission: 0, 26, 320, 185; the game clips its 3D to rows 26..396 itself,
  grClipWindow stays 640x480). Narrowing only the projection's x scale was not enough: the
  game's visibility tests use the camera's angle (+0xe8 field of view, +0xf0 half, +0x1d4 =
  1/tan(half); made by 0x44a460 from base and zoom), so the sides showed stale pictures. Now
  the angle is widened during world_draw (every 3D polygon is drawn in it) and put back.
  The camera's frustum is made from the angles only when they change (camera update
  0x44aa80, before the picture): near-plane corners (+0x108.., flag +0xf8) and six planes
  (0x47a140 into +0x13c, flag +0x138; 0x47a250 into world space +0x188). Left narrow, it
  culled objects and large terrain pieces at the sides (2026-11-24): the hook now makes
  them again with the wide angle and sets both flags after the picture.
  The HUD is LFB pixels after the picture, its bars written only once (they stay in the
  frame): clipping the 3D to the whole height painted over them, so beside the bars the
  frame stays black.
  The target zoom (2026-11-24) is no view of its own (one projection call a picture): while
  it is on (0x58298c) 0x477bb0 draws every polygon twice, the second time cut to a source
  rectangle around the aim (clipper 0x57b1b8) and moved in 2D into the zoom's box (x scale
  and offset 0x57b20c/0x57b214, y 0x57b210/0x57b218). 0x47a010 sets them from a fixed-size
  source rectangle (caller 0x41174b). Wide: the hook takes that rectangle in the wide
  picture's columns (320 + k (x - 320)) and maps the box into the frame's 4:3 middle
  (scale k sx, offset k ox + 320 (1 - k)). Found by tracing the polygons below row 396:
  one zoom polygon after each main one.
  Aiming (2026-11-24): the projection's x values (0x56a718, 0x56a6f8, 0x56a710, 0x56a720)
  stayed wide after the picture; 0x474c40 turns screen points into directions with 0x56a720
  (the aim 0x4113eb, the hit tests 0x49aa80), so shots went off the more to the side. They
  are put back to the game's own after the picture (tested by hand: shots go to the mark).
  Mouse range (2026-11-24): in a mission too [0x565b6c] is 0, so our hook on 0x470570
  (`hooks.rs`) sets the game's mouse from `user32::mouse_pos`; `window.rs` frame_pos clamped
  it to the 640 columns, now to the picture's width (-side..640+side). The aim follows
  (0x565b48 beyond +-1), the tank turns. The game's reticle is a HUD sprite (0x4ea0f4, drawn
  by the sprites' draw 0x4b7790 as frame buffer pixels, cut to the 640 x 480 screen); 0x411300
  places it (centre 0x4ea0dc/e0, half size 0x4ea0c4/c8). `reticle.rs` draws ours there
  instead (made in Rust, sharp at any resolution, replaceable) with `glide::draw_image`, which
  draws over the whole target (the view's viewport cut what lay beside it). Found by skipping
  functions for a second each in a scripted run and looking at the screenshots.
  HUD after a resize (2026-11-25): the HUD's sprites go through 0x4b79c0 each picture; flag 2
  (+0xc) marks a static one, drawn only when flag 4 or 8 asks (one per buffer, cleared by the
  draw). The frame made anew starts black, so `hud.rs` asks every static sprite once more
  (4 | 8) after each new frame (`glide::targets_made`). Tested by hand: the HUD comes back.
  A scripted resize (`size@` tried) did not change the render target here: its resolution is
  fixed (Preferences), only an aspect change makes it anew.
