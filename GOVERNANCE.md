# Governance

openIG2 is a small volunteer project.

## Roles
- **Maintainers** review and merge pull requests, cut releases and decide on deviations from the original. The
  current maintainer is [@justdawee](https://github.com/justdawee).
- **Contributors** are everyone who reports bugs, writes docs, specs or code.

A contributor with a record of good pull requests can be invited as a maintainer by the existing maintainers.

## Decisions
- `main` is always shippable: it builds, its tests pass, and a player can start and play the game.
- A pull request is merged by a maintainer after review and green CI. A maintainer's own change waits for a second
  maintainer's review when there is one.
- Fidelity questions are settled by evidence from the original (code, runtime traces, captures), not by taste.
- A deliberate difference from the original needs a decision record (`docs/decisions/`) and, where reasonable, an
  opt-in *enhanced* option, so the classic game stays available.
- Disagreements are discussed in the issue or pull request; if they can't be settled, the maintainers decide.

## Conduct
Be kind and constructive, assume good faith, and keep discussions about the work. Harassment is not tolerated;
maintainers may remove comments and block accounts that break this.
