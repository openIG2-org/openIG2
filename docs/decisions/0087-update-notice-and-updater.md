# ADR 0087: the update notice and the updater

Date: 2026-11-10. Status: accepted. Builds on ADR 0042 (the remake's texts), ADR 0048 (the remake's options), ADR 0078
(the modern main menu) and ADR 0086 §1.2 (HTTPS downloads). Implemented by `ig2-client::update` (the ask, the
install, the notice) and `ig2-client::window::front` (the notice on the main menu, the restart).

## Context
A new beta follows every finished bigger piece of work (docs/VERSIONING.md), several a week. A player learns of one
only by looking at the project's page, then downloads the archive and unpacks it over the old files by hand. The
original has nothing of the kind; it changes nothing in the game itself.

## Decision
1. **One option, on by default.** "Check for updates" (Options, Game; `CheckUpdates` in our settings file). Off
   sends nothing. On, the game asks once per start, in the background.
2. **GitHub's releases are the source.** One request to
   `https://api.github.com/repos/openIG2-org/openIG2/releases/latest` (HTTPS only, the `openIG2/<version>` user
   agent, a reply of at most 1 MiB; nothing about the player or the install is sent). The release's tag `vMAJOR.MINOR.N`
   is compared with the build's version as three numbers; only a higher one is an update. A build without a release
   version (`-dev`), the browser build and a scripted run never ask. A failed ask writes one line to the log and
   shows nothing.
3. **The notice.** On the modern main menu's main panel, at the bottom right over the version: "New version: N" and
   one button. The classic main menu shows nothing; the log has the line with the download page's address.
4. **The player's press installs it (Windows).** "Update and restart" downloads the release's
   `openig2-<version>-windows-x64.zip`, taken only from
   `https://github.com/openIG2-org/openIG2/releases/download/v<version>/` and only when the release lists its
   SHA-256 (the asset's `digest`). In memory: the size and SHA-256 are checked against the release's, the zip's own
   checksums, every name is a plain file name (no folders), and the running exe's name is among them (a build under
   another name is not a release's and is refused). Then each file is written beside its place and swapped in; the
   replaced files stay as `<name>.update-old` until the next start removes them (Windows lets a running exe be
   renamed, not overwritten), and a failed swap puts them back. The game then starts again with the same arguments.
   Nothing but the release's own files is touched: settings, saves and mods stay.
5. **Elsewhere, and after a failure: the download page.** On Linux and macOS, when the release has no archive the
   updater accepts, and after a failed install (a game folder that can't be written, a damaged download) the button
   is "Download page", which opens `https://github.com/openIG2-org/openIG2/releases/latest` in the browser.

## Consequences
- The updater trusts what a player downloading by hand trusts: GitHub's TLS and the project's release page. The
  SHA-256 comes from the same API, so it guards against a damaged download, not against a changed release. Signing
  the archives (as the mod index is, ADR 0086 §1.3) would close that; it needs a signing key in the release
  workflow.
- The `.tar.gz` bundles are not unpacked: Linux and macOS players update by hand from the notice.
- No new dependency: `ureq`, `serde_json`, `sha2` and the zip reader are the mod repository's.
