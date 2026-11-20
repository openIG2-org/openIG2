# Sounds and music

This page makes a mod that replaces the button click, adds a music track and gives a new power plant its own picture
and building sound. It takes about fifteen minutes and needs a sound editor that saves WAV or FLAC (Audacity, for
example) besides a text editor. The rules behind it are in ADR 0091; the user guide's
[Mods section](../USER_GUIDE.md#sounds-and-music) is the reference. Use sounds you made yourself or may share; never
copy, convert or edit the game's own files into a mod.

## 1. The folder

In the mods folder (main menu → Mods → Open mods folder) make `my-sounds` with a `mod.toml`:

```toml
name = "My sounds"
author = "Your name"
version = "1.0"
description = "A new click, a new track and a plant that hums."
```

A mod with only sounds and music could add `kind = "visual"`: players with and without it then play together. This
one also adds a building, so it stays an ordinary mod.

## 2. Replace a sound

Sounds keep the game's paths; only the ending may change. Save your click as
`my-sounds\sounds\ingame\click1.wav`: it replaces the install's `dataraw\sounds\ingame\click1.ogg`, one of the
in-game clicks. Mono is fine (it plays on both sides), and any rate from 8 to 192 kHz; keep effects short (they stop after
120 seconds). WAV must be plain PCM (16 or 24 bit, or float); FLAC and Ogg Vorbis work too.

## 3. Add a music track

Save a track as `my-sounds\music\my_track.flac`. In a game it plays after your race's three tracks, then the race's
first comes again. Tracks in `music\space_war\` and `music\ground_war\` join the battle music instead.

## 4. A plant with its own picture and sound

Save a short sound as `my-sounds\sounds\my-sounds\hum.wav` and add a `rules.toml`:

```toml
[[techs.new]]
key = "hum_plant"
category = 9                  # power plants
name = "Hum plant"
prerequisites = [135]         # the Mezon plant

[[buildings.new]]
key = "hum_plant"
like = 55                     # the Mezon plant: model, footprint, role
tech = "hum_plant"
icon = "a1full.xvb"           # the build menu's picture: any model of the game's colony models
sound = "my-sounds/hum.wav"   # relative to sounds/: plays when you start building it
```

`icon` names a model the game already has (or one your mod adds in the game's model format). A new space weapon
takes `sound` the same way, as its firing sound.

## 5. Check and play

Run `openIG2-check --mod my-sounds`: each sound is listed with its kind, rate, channels and length, and a file that
doesn't decode or a `sound` that names no file is an error. Switch the mod on in Mods, press Apply and start a game:
that click is yours, the plant is in the colony's build list once researched, and your track comes in the
music's turn. A file the game can't decode plays nothing and is named on stderr; the game goes on.
