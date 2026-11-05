# ADR 0081: game files hosted by the web server

Date: 2026-11-05. Status: accepted; implemented (`deploy/web`, the page's offer in `web/main.js`).

## Context
ADR 0047 §1 keeps the game data the player's: the page copies the player's own install into the browser. That asks
every visitor to have the install at hand and to pick its folder, a step a plain "open the URL and play" page
doesn't want. Imperium Galactica II is DRM-free; a server operator may hold a copy of the game files they want to offer
their visitors.

## Decision
1. **The repository and the image still ship no game data.** The web image (`deploy/web/Dockerfile`: the wasm build
   and the page behind nginx) serves the game files only from a folder the operator mounts at run time
   (`IG2_GAME_DIR` in `deploy/web/compose.yaml`, read-only at `/srv/game`). Which files they mount, and their right to
   offer them, is the operator's business.
2. **The page asks first.** At start the image lists the mounted folder as `[[path, size], …]` at `game/index.json`
   (`deploy/web/game-index.sh`). Where that list looks like an install (`ig2.exe`, `DataRaw/Text/Scenario.txt`), a
   visitor without a stored copy is told the file count and size and offered "I agree: download the game files and
   play"; only after that click does the page download them, six at a time, into the same OPFS copy as a picked folder
   (ADR 0047 part 2), and start. Picking an own install stays offered beside it; an empty folder hosts nothing and
   the page behaves as before.
3. **HTTPS.** OPFS, WebGPU and audio need a secure context, so a public page runs behind HTTPS: the operator's own
   reverse proxy, or the compose file's `https` profile (Caddy with an automatic certificate for `IG2_DOMAIN`).
4. **Testing.** `tools/serve_web.py --install DIR` serves the same list and paths on 127.0.0.1, so the offer can be
   tried locally; `?dev-install` downloads without asking there.

## Consequences
- A first visit downloads the whole folder (the GOG install: about 17,600 files, 690 MB); later visits start from the
  copy. A new set of files on the server reaches a visitor only after "Forget the stored copy".
- The operator decides what to mount; extra files (the manual, the uninstaller) are downloaded too unless left out.
