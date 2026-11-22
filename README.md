# Recoil in Rust

Recoil (YEAR) recompiled to native code: your own copy of the game's EXE is translated to
the machine code of your computer when the program starts, and Windows and DirectX are
replaced by Rust code (OpenGL, winit, cpal, gilrs). Runs on 64-bit Windows and Linux.

Built with [recomp_kit](https://github.com/RadiatorTwo/recomp_kit).

## Playing

You need the PC version of the game. Download the release, unpack it, and put your CD image
(.iso or .bin) next to `rustcoil.exe` or copy the CD into `game_files` (see
`game_files/README.txt`). Start `rustcoil.exe`.

## Building

```text
cargo build --release
```

## License

MIT (the code here), except `crates/indeo` (the Indeo 5 video decoder, a port of FFmpeg's):
LGPL-2.1-or-later, see `crates/indeo/COPYING`. The game and its files belong to their owners and are not part of this
project.
