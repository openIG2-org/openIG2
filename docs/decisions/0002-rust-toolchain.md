# ADR 0002: Rust toolchain and workspace layout

Date: 2026-10-15. Status: accepted (revisit at Phase 3).

## Context
The dev machine had no Rust. Visual Studio 18 Community is installed without the C++ build tools, so
the MSVC linker isn't available. WinLibs MinGW-w64 (GCC 16) is present.

## Decision
- Install rustup (winget `Rustlang.Rustup`) and use `stable-x86_64-pc-windows-gnu` as the default
  toolchain. Rust 1.99, edition 2024.
- Cargo workspace in `engine/`, crates under `engine/crates/`. `publish = false` until a license is
  chosen (open question for the maintainers).
- Dependencies stay minimal and pure Rust where possible: `lz4_flex` (legacy LZ4 blocks),
  `encoding_rs` (8-bit codepages), `thiserror`.
- Golden tests read the install from `IG2_INSTALL_DIR` and pass vacuously when it's unset, so the
  repo builds and tests anywhere without game data.

## Consequences
- wgpu, winit, kira and quinn all support windows-gnu. If a later dependency needs MSVC (some
  video codecs), we either install the VS C++ workload or isolate that dependency.
