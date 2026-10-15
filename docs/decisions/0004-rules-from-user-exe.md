# ADR 0004: Read the compiled-in rules tables from the user's ig2.exe

Date: 2026-10-15. Status: accepted.

## Context
The balance rules (technologies, prices, research, buildings, pods and slots, race and difficulty
multipliers) are compiled into `ig2.exe` as static data. Runtime verification shows they're
authoritative (scenario files don't override them). The project ships no original assets, and these
tables are a bulk copy of the original program's data.

## Decision
- `engine/crates/ig2-rules` reads the tables **at runtime from the user's own `ig2.exe`**, exactly
  like assets. The repo contains only the layout catalogue (addresses, strides, field offsets) and code.
- The exe is identified by SHA-256 (GOG 2017-02-06 build). Unknown builds are rejected with a clear
  error; supporting more builds means adding a catalogue entry, not changing code.
- `ig2-rules-dump` exports the tables to RON for local inspection and modding. Mods override values
  through RON files layered on top of the extracted defaults (Phase 3). Dumped files are never committed.
- Tests check shapes, cross-references and a few spot values against the user's install
  (`IG2_INSTALL_DIR`), and pass vacuously without it.

## Consequences
- Rules stay faithful by construction: no transcription errors.
- The engine requires the original exe to be present (it already requires the original data).
- Enhanced-ruleset changes are expressed as overrides, keeping the classic values intact.
