# Versioning

openIG2 versions are **MAJOR.MINOR.N**:

- **MAJOR.MINOR** is in the `VERSION` file at the repository root.
- **N** counts the commits on `main` since `VERSION` last changed (0 for that commit). Every merged pull request
  raises it by one; nobody edits it by hand. `tools/release/version.py` computes it, and CI passes it to the build as
  `OPENIG2_VERSION`. A local build without it shows `VERSION` with `-dev` (for example `0.7-dev`).
- **MINOR** goes up once per milestone or big update; the commit that starts it changes `VERSION` and adds a new
  section to `CHANGELOG.md`.
- **1.0.0** comes only when the game is complete and plays like the original from start to end, multiplayer works,
  and no important bugs are known.

| Version | Milestone |
|---|---|
| 0.1 | Foundations: formats, rules from the player's `ig2.exe`, the simulation core, the campaign |
| 0.2 | Playable from start to finish |
| 0.3 | Every screen of the original |
| 0.4 | Input and feel |
| 0.5 | Multiplayer playable |
| 0.6 | Fixes against the original |
| 0.7 | Playable 1.0: the last features, install detection, release builds, internet play |

## Releases
The release workflow (`.github/workflows/release.yml`) builds the Windows zip and publishes it on GitHub:

- **Beta**: started by hand from the Actions tab (Release → Run workflow), from the current `main`. Named
  "openIG2 MAJOR.MINOR.N (beta)".
- **Stable**: a pushed tag `vMAJOR.MINOR.N` that matches the computed version.

Both are normal GitHub releases marked **Latest**, so the repository page, the README's download link and the
website always point at the newest build. Before 1.0 every release is a beta in practice.

The release notes come from the version's `CHANGELOG.md` section.
