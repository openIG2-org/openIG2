# The game's language (GOG remaster)

Which language `ig2.exe` (GOG, sha256 2baa530b…0442) runs in, so the client can pick the same (ADR 0009: a capture
comparison needs the same texts on both sides). Port: `ig2_formats::locale` (`exe_language`), used by
`ig2-client` unless `--lang CODE` or `IG2_LANG` says otherwise.

## 1. The remaster's text language
`TextServer::loadTexts` (ELF 0x8f4c90, exe: inlined into its caller) sets `TextServer::localeid` to
`TextServer::forcelanguage` when that is non-empty, else to `PlatformUtils::GetSGLocaleInfo()`, then
`TextServer::language` = the first part of `stringhelper::explode(localeid, "_")` (ELF 0x8f5849, separator at ELF
0xd982b5).

- `forcelanguage` comes from the install's `ig2.settings` (plain text; exe 0x73a24f reads the file `ig2.settings`
  0x88fe5c and looks up the key `language` 0x88fe6c). GOG ships it commented out (`//language=en`); `#` and `//`
  lines are comments.
- `GetSGLocaleInfo` on Windows (exe 0x6e8790): reads `locale.txt` (0x87ca3c) first from the user folder
  (`%LOCALAPPDATA%/GyroscopeGames/ig2/locale.txt`, flag 1 of exe 0x6e3440), then with flag 0 [inf: next to the
  game]; with neither it calls exe 0x6f0b50: `GetLocaleInfoA(0x800 = LOCALE_SYSTEM_DEFAULT, 0x59 =
  LOCALE_SISO639LANGNAME)` + `_` + `GetLocaleInfoA(0x800, 0x5a = LOCALE_SISO3166CTRYNAME)`.
- `locale.txt` is written by GOG's `language_setup.exe` (`language_setup.ini`: it copies `_locale/locale.<xx>`, two
  bytes such as `hu`, to `{localappdata}/GyroscopeGames/ig2/locale.txt`).

`TextServer::language` picks the asset tree `localization/<lang>/` (the `AssetHelper` rule, `vfs.rs`; files missing
there fall back to the base = English files) and the bitmap font's glyph order: exe 0x4a9efa calls exe 0x739b80
(the language string) and copies `GLYPHS_HU` 0x8527F0 when its first byte is `h`, else `GLYPHS` 0x852748.

## 2. The 1999 code's `Lang`
`IG2_gameInit` (ELF 0x586960, exe 0x4dfbaf..0x4dfcfa) sets `Lang` (exe 0x912724, ELF 0x117b050) to the index of the
`init/config` value `lang` (default `none`) in `{none, ENG, FRA, GER, HUN, ITA, SPA}` (exe 0x854a54..0x854a70);
for `none` it tries `data\<XXX>\text.dat` for 1..=6 and keeps the first that opens, else 1. The GOG install has
neither, so `Lang` = 1 (ENG) whatever the text language: the remaster's code paths that test `Lang == 4` (HUN) don't
run on GOG.

## 3. Observed
Frida on this host (`tools/trace/probe_lang.py`: hooks on `CreateFileA/W` and `GetLocaleInfoA`, no `locale.txt`, a
Hungarian system locale): the exe opens `ig2.settings`, `%LOCALAPPDATA%/GyroscopeGames/ig2/locale.txt` (absent),
`init/config`, `data\{ENG,FRA,GER,HUN,ITA,SPA}\text.dat` (all absent), gets `hu` / `HU` from `GetLocaleInfoA`, then
reads `localization/hu/…`; `Lang` reads 1.

## 4. Port
`ig2_formats::locale::exe_language(install_root)` follows §1 (on other systems the system locale is `LC_ALL` /
`LC_MESSAGES` / `LANG`, as for the ELF build [inf]); a language without a localization (`pl`, …) gives English.
The client's texts keep their files' code page (Windows-1250 for Hungarian: ő/ű are 0xF5/0xFB, which the Hungarian
glyph order maps), never Latin-1.
