# Your first mod

This walk-through makes a small mod from nothing, checks it, switches it on and sees it work in a game: two days into
any game a line runs across the ticker and your treasury gets 5000 credits. It takes about ten minutes and needs no
tools beyond a text editor and the game itself. The reference for everything a mod can do is the Mods section of the
[user guide](../USER_GUIDE.md#mods); this page is the gentle way in.

The game ships no original files, and neither should your mod: everything below is text you write yourself.

## 1. Make the mod's folder

Open the game's mods folder: main menu → Mods → Open mods folder (it is the `mods` folder beside `openIG2.exe`; the
button makes it when it's missing). Inside it, make a folder named `first-mod`. The folder's name is the mod's name
in the enabled list, so keep it short, lower case, without spaces.

## 2. Describe it: `mod.toml`

In `first-mod`, make a text file named `mod.toml` (check that your editor doesn't save it as `mod.toml.txt`):

```toml
name = "My first mod"
author = "Your name"
version = "1.0"
description = "A welcome line and a gift of 5000 credits on day 2."
requires = ">=0.7"
```

Every line is optional; the Mods page shows them. `requires` says which openIG2 versions the mod works with (`">=0.7"`
means 0.7 or newer); the page won't load it on a version outside that range and tells you why. Older mods use `game =
"0.7"` instead, which is still read but only shown as a hint.

## 3. Add a story: `stories.toml`

A story is a rule of the form "when this happens, do that". Make `stories.toml` beside `mod.toml`:

```toml
# Day 2: a line on the ticker and 5000 credits for the human player.
[[story]]
name = "welcome-gift"
when = [{ day = 2 }]
do = [{ ticker = 9100 }, { money = { empire = "human", amount = 5000 } }]
```

- `[[story]]` starts a story; a file can hold as many as you like.
- `when` lists the triggers, all of which must hold: here, the game has run for at least 2 days.
- `do` lists the actions, in order: show ticker line 9100, then give the human 5000 credits.
- A story fires once unless it says `repeat = true`.

The user guide's story section lists every trigger and action (battles, techs, relations, fleets, planets, timers,
flags and more) and a longer worked example.

## 4. Add its text: `texts.en.toml`

Line 9100 doesn't exist yet, so the mod adds it. Make `texts.en.toml` beside the other two:

```toml
# The ticker line the story shows: a new line of the story texts.
["dataraw/text/s_story.txt"]
9100 = "Welcome, commander: the council sends 5000 credits."
```

The heading names one of the game's text files; the lines under it set entries by their key. The English file is used
in every language that has no file of its own; add `texts.de.toml`, `texts.fr.toml`, `texts.es.toml`,
`texts.it.toml` or `texts.hu.toml` with the same keys to translate it.

Your folder now holds:

```
mods/
  first-mod/
    mod.toml
    stories.toml
    texts.en.toml
```

## 5. Check it

Open a terminal in the game's folder and run:

```bash
openIG2-check --quick --mod first-mod
```

Among the lines it prints you should see:

```
    mod first-mod: texts.en.toml sets 1 lines
    mod first-mod: stories.toml holds 1 stories
...
all ... checks passed
```

A typo shows up here with the file, the story and the word it didn't understand, for example
`story welcome-gift: when 1: dayy: unknown trigger (known: day, …)`. Fix it and run the check again.

## 6. Switch it on and play

1. Main menu → Mods: `My first mod` is in the list. Switch it on and press Apply: Apply loads it at once.
2. Start any new game and let two days pass. The ticker at the top shows your line and the money display jumps by
   5000.

If the game starts without your mod, the Mods page says why in red, naming the file and the line.

## 7. Share it

Zip the `first-mod` folder (Windows: right click → Send to → Compressed (zipped) folder) and share
`first-mod.zip`. Players drop the zip into their `mods` folder as it is, without unpacking it, and switch it on in the
Mods page.

Two things to know before you share:

- In multiplayer every player needs the same mods. The room refuses a player whose mods differ, and lists the host's
  mods.
- Saves made with a mod may need it to load. Tell your players when an update changes what a save holds.

## Where next

- Change rule values (prices, research times, a race's production): `rules.toml`, in the user guide's Mods section.
- Add techs, buildings, ship hulls, weapons, tanks, races and planet types: the same section, with examples.
- Replace textures and models, or add sharper versions of them: the user guide's "Texture and model packs".
- Longer stories with timers, flags, battles and diplomacy: the `zorg-ultimatum` example in the user guide.
