Put an image of your Recoil CD next to rustcoil.exe (Linux: rustcoil),
or put your copy of the game (PC version) into this folder. Any of these:

  - a CD image: .cue with its .bin files, .iso, .img (CloneCD) or .mdf (Alcohol)
  - the contents of the CD, copied into a folder here
  - your installed game folder (C:\Westwood\Recoil)

At its next start rustcoil finds the game, checks that it is a version
this port knows, and sets it up here:

  Recoil\       the game's EXE and data (and your settings and saved games)
  Video\        the CD's videos

The CD holds the game in its installer's archive (data1.cab): rustcoil
unpacks it itself, as the CD's setup would. Your installed game folder
works too; files its installer put elsewhere (the MFC DLLs) are taken from
the CD image or the CD's contents when they are here as well.

Music: the game plays its music from the CD's audio tracks. Keep the .cue
image (with its .bin files) here or next to rustcoil.exe; an .iso has no
audio tracks, the game then runs without music. Without videos it runs
without them.

Nothing in this folder is ever deleted: remove other copies yourself once
the game runs (keep the .cue image for the music).
