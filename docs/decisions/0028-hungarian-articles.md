# ADR 0028: Hungarian articles are filled in

Date: 2026-10-31. Status: accepted. A deliberate deviation from the classic client (ADR 0009), with the original's
look kept as an option. Spec: `re/formats/locale.md` §2. Implemented by `ig2-client::diplomacy::articles` and its
callers (the message line, the diplomacy screen's menus and texts, the space battle's opening banner).

## Context
Hungarian texts mark the article before a name with a byte: 0x95 (`a`/`az`) or 0xB0 (`A`/`AZ`), because "az" goes
before a vowel and "a" before a consonant, and the name is filled in only when the text is shown. The game's
`Magyar_a2az_konverter` (ELF 0x6240b0) turns the marker into `z`/`Z` or drops it; the 1999 code calls it only when
`Lang == 4` (HUN). The GOG build never sets `Lang` from the text language (it stays 1, `re/formats/locale.md` §2), so a
Hungarian game shows the raw byte, as in "Felfedezted a<0x95> Antari Birodalom-t!" on the message line. Players read
this as a broken character.

## Decision
1. **Option "Hungarian articles", on by default.** In a Hungarian game every text that goes through `a2az` in the
   1999 code (the message line, the diplomacy screen's menus and dialogue lines, the space battle's "A vs. B" banner)
   is converted as the 1999 code does with `Lang == 4`.
2. **Classic mode** (`HungarianArticles` 0 in the remake's settings file, or the Remake row on the Options screen's
   game page): the markers stay, as the GOG exe shows them.
3. Other languages' texts have no markers; nothing changes for them.

## Consequences
- Capture comparisons and `--snapshot` draw classic (the option is read by the window only).
