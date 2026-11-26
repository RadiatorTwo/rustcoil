# Recoil in Rust

Recoil (YEAR) recompiled to native code: your own copy of the game's EXE is translated to
the machine code of your computer when the program starts, and Windows and DirectX are
replaced by Rust code (OpenGL, winit, cpal, gilrs). Runs on 64-bit Windows and Linux.

Built with [recomp_kit](https://github.com/RadiatorTwo/recomp_kit).

## Playing

You need the PC version of the game. Download the release, unpack it, and put your CD image
(.cue/.bin, .iso, .img or .mdf) next to `rustcoil.exe` or into `game_files`, or copy the CD or
your installed game into `game_files` (see `game_files/README.txt`). Start `rustcoil.exe`: at
the first start it unpacks the game from the CD's installer archive itself. The music comes
from the CD's audio tracks: keep the .cue image for it.

## Building

```text
cargo build --release
```

## License

MIT (the code here), except `crates/indeo` (the Indeo 5 video decoder, a port of FFmpeg's):
LGPL-2.1-or-later, see `crates/indeo/COPYING`. The game and its files belong to their owners and are not part of this
project.
