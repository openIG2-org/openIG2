// The browser build's loader (ADR 0047): the player's own install, picked here, copied into the Origin Private File
// System and handed to the client file by file. No game file leaves the machine. A server that hosts the game's files
// itself (ADR 0081, `game/index.json`) is offered instead, downloaded into the same copy once the player agrees.
import init, { add_file, add_user_file, start } from "./pkg/ig2_client.js";

const $ = (id) => document.getElementById(id);
const STORE = "install";
// The player's own files (settings, saves, replays): the client's `/user` folder (ADR 0047 part 3).
const USER = "user";
// Written last: a copy without it was interrupted.
const DONE = ".openig2-complete";

function status(text, error = false) {
  $("status").textContent = text;
  $("status").classList.toggle("error", error);
  if (error) progress();
}

// The bar over the status line: `done` of `total`, or hidden without them.
function progress(done, total) {
  $("bar").hidden = total === undefined;
  if (total) $("bar").value = done / total;
}

// Nothing can be pressed while files are copied or loaded.
function busy(on) {
  for (const e of $("setup").querySelectorAll("button, input")) e.disabled = on;
}

const LANGS = ["en", "de", "fr", "es", "it", "hu"];
// The game's language: the page's choice. It decides which files are downloaded and loaded, so the client always
// gets it (`--lang`).
const language = () => document.querySelector("input[name=lang]:checked").value;

// The choice a visit starts with: the last one, else `?lang=`, else the browser's language, else English.
function firstLanguage() {
  let kept;
  try {
    kept = localStorage.getItem("lang");
  } catch {}
  const wish = [kept, new URLSearchParams(location.search).get("lang"), ...navigator.languages.map((l) => l.slice(0, 2))];
  return wish.find((l) => LANGS.includes(l)) ?? "en";
}

// The language whose copy of a file this is (`localization/<lang>/…`); undefined for the files every language reads.
const langOf = (path) => /^localization\/([^/]+)\//i.exec(path)?.[1].toLowerCase();
// The client reads `localization/<lang>/` only in that language (ig2-formats' vfs): the other languages' copies,
// most of the install's files, are neither downloaded nor loaded.
const wanted = (path, lang) => [undefined, lang].includes(langOf(path));
const mb = (bytes) => Math.ceil(bytes / 1e6);

// The client's report when its start fails after the page handed over (no WebGPU, say).
window.ig2_failed = (message) => {
  document.querySelector("canvas")?.remove();
  $("setup").hidden = false;
  busy(false);
  status(`The client stopped: ${message}`, true);
};

// The client's changes to the player's files, stored in order: `data` the bytes, `undefined` a removed file.
let writes = Promise.resolve();
window.ig2_user_file = (path, data) => {
  writes = writes.then(async () => {
    const parts = path.split("/");
    let dir = await (await navigator.storage.getDirectory()).getDirectoryHandle(USER, { create: true });
    for (const p of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(p, { create: true });
    if (data === undefined) {
      await dir.removeEntry(parts.at(-1)).catch(() => {});
      return;
    }
    const w = await (await dir.getFileHandle(parts.at(-1), { create: true })).createWritable();
    await w.write(data);
    await w.close();
  }).catch((e) => console.error(`storing ${path}:`, e));
};

// The videos (ADR 0047 part 4): WebCodecs decodes the H.264 access units the client feeds; each frame goes back as
// RGBA through a canvas (`null`: the decoder failed).
window.ig2_video_open = (codec, width, height, onFrame) => {
  const ctx = new OffscreenCanvas(width, height).getContext("2d", { willReadFrequently: true });
  const dec = new VideoDecoder({
    output: (f) => {
      ctx.drawImage(f, 0, 0, width, height);
      f.close();
      onFrame(new Uint8Array(ctx.getImageData(0, 0, width, height).data.buffer));
    },
    error: (e) => {
      console.error("video:", e);
      onFrame(null);
    },
  });
  dec.configure({ codec, codedWidth: width, codedHeight: height, optimizeForLatency: true });
  return dec;
};
window.ig2_video_chunk = (dec, key, timestamp, data) => {
  if (dec.state === "configured") dec.decode(new EncodedVideoChunk({ type: key ? "key" : "delta", timestamp, data }));
};
window.ig2_video_queue = (dec) => dec.decodeQueueSize;
window.ig2_video_flush = (dec) => (dec.state === "configured" ? dec.flush() : Promise.resolve());
window.ig2_video_close = (dec) => {
  if (dec.state !== "closed") dec.close();
};

// The game quit (the menu's Exit): the page starts over once the last writes are stored.
window.ig2_exited = () => writes.then(() => location.reload());

async function storeDir(create = false) {
  const root = await navigator.storage.getDirectory();
  return root.getDirectoryHandle(STORE, { create });
}

async function stored() {
  try {
    await (await storeDir()).getFileHandle(DONE);
    return true;
  } catch {
    return false;
  }
}

// Every file under a directory handle: [path, handle].
async function* walk(dir, prefix = "") {
  for await (const [name, h] of dir.entries()) {
    const path = prefix + name;
    if (h.kind === "directory") yield* walk(h, path + "/");
    else yield [path, h];
  }
}

// The picked folder as [path, File] pairs.
async function pickFiles() {
  if (window.showDirectoryPicker) {
    const dir = await window.showDirectoryPicker({ id: "ig2-install", mode: "read" });
    const files = [];
    for await (const [path, h] of walk(dir)) files.push([path, await h.getFile()]);
    return files;
  }
  // Every browser: a folder input; its paths start with the picked folder's name.
  return new Promise((resolve) => {
    const input = $("folder");
    input.onchange = () => resolve([...input.files].map((f) => [f.webkitRelativePath.replace(/^[^/]*\//, ""), f]));
    input.click();
  });
}

function looksLikeInstall(files) {
  const names = new Set(files.map(([p]) => p.toLowerCase()));
  return names.has("ig2.exe") && names.has("dataraw/text/scenario.txt");
}

// Copies [path, File or URL, size] entries into the stored copy; `fresh` replaces the copy, else they are added to it.
async function copy(files, fresh = true) {
  await navigator.storage.persist?.();
  const root = await navigator.storage.getDirectory();
  if (fresh) await root.removeEntry(STORE, { recursive: true }).catch(() => {});
  const store = await storeDir(true);
  const total = files.reduce((n, [, , size]) => n + size, 0);
  const verb = typeof files[0]?.[1] === "string" ? "Downloading" : "Copying";
  let done = 0;
  let bytes = 0;
  let next = 0;
  // A few files at a time: one by one, a distant server's round trips would take most of the download.
  const worker = async () => {
    while (next < files.length) await copyOne(...files[next++]);
  };
  const copyOne = async (path, file, size) => {
    const parts = path.split("/");
    let dir = store;
    for (const p of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(p, { create: true });
    const w = await (await dir.getFileHandle(parts.at(-1), { create: true })).createWritable();
    if (typeof file === "string") {
      // A URL of this server's game files (or the test server's): streamed in; pipeTo closes the file.
      const r = await fetch(file);
      if (!r.ok) throw new Error(`${file}: ${r.status}`);
      await r.body.pipeTo(w);
    } else {
      await w.write(file);
      await w.close();
    }
    bytes += size;
    if (++done % 20 === 0 || done === files.length) {
      progress(bytes, total);
      status(`${verb} the game files: ${mb(bytes)} / ${mb(total)} MB, ${done} / ${files.length} files`);
    }
  };
  await Promise.all(Array.from({ length: 6 }, worker));
  await (await (await store.getFileHandle(DONE, { create: true })).createWritable()).close();
}

// The game files this server hosts (ADR 0081): [path, size] pairs, or null where it hosts none.
let hosted = null;

const hostedFor = (lang) => hosted.filter(([path]) => wanted(path, lang));
const asDownloads = (files) => files.map(([path, size]) => [path, gameUrl(path), size]);

// The stored copy's files as [path, handle].
async function storedFiles() {
  const files = [];
  for await (const f of walk(await storeDir())) if (f[0] !== DONE) files.push(f);
  return files;
}

async function play() {
  busy(true);
  try {
    const lang = language();
    let files = await storedFiles();
    if (hosted) {
      // A language picked after the download: its files come now, beside the stored ones.
      const have = new Set(files.map(([path]) => path));
      const missing = hosted.filter(([path]) => langOf(path) === lang && !have.has(path));
      if (missing.length) {
        await copy(asDownloads(missing), false);
        files = await storedFiles();
      }
    }
    files = files.filter(([path]) => wanted(path, lang));
    let n = 0;
    for (const [path, h] of files) {
      add_file(path, new Uint8Array(await (await h.getFile()).arrayBuffer()));
      if (++n % 100 === 0) {
        progress(n, files.length);
        status(`Loading the game files: ${n} / ${files.length}`);
      }
    }
    const user = await (await navigator.storage.getDirectory()).getDirectoryHandle(USER, { create: true });
    for await (const [path, h] of walk(user)) add_user_file(path, new Uint8Array(await (await h.getFile()).arrayBuffer()));
    progress();
    status("Starting…");
    $("setup").hidden = true;
    const params = new URLSearchParams(location.search);
    // Testing only: `?args=--multiplayer` and the like, the native client's options, on localhost.
    const args = isLocal() ? params.get("args") ?? undefined : undefined;
    start(lang, params.has("webgl"), args);
  } catch (e) {
    busy(false);
    $("setup").hidden = false;
    status(`The client stopped: ${e}`, true);
    console.error(e);
  }
}

const isLocal = () => ["127.0.0.1", "localhost", "[::1]"].includes(location.hostname);

async function hostedFiles() {
  try {
    const r = await fetch("game/index.json", { cache: "no-cache" });
    const files = r.ok ? await r.json() : null;
    return Array.isArray(files) && looksLikeInstall(files) ? files : null;
  } catch {
    return null;
  }
}

const gameUrl = (path) => "game/" + path.split("/").map(encodeURIComponent).join("/");

async function download() {
  await copy(asDownloads(hostedFor(language())));
  await play();
}

// Testing only: `tools/serve_web.py --install` serves the install on this machine; `?dev-install` downloads it without
// asking. Pages not on localhost ignore it.
async function devInstall() {
  if (!isLocal() || !hosted || !new URLSearchParams(location.search).has("dev-install")) return false;
  if (await stored()) await play();
  else await download();
  return true;
}

async function main() {
  document.querySelector(`input[name=lang][value=${firstLanguage()}]`).checked = true;
  await init();
  let have = await stored();
  hosted = await hostedFiles();
  // What the page offers: Play from a stored copy, else this server's files where it hosts them, and the own folder.
  const show = () => {
    $("stored").hidden = $("forget").hidden = !have;
    $("offer").hidden = !hosted || have;
    $("own").hidden = have;
    $("pick").classList.toggle("primary", !have && !hosted);
    $("pick").textContent = have ? "Use another folder…" : hosted ? "Use my own install folder…" : "Choose the install folder…";
    if (hosted) {
      const files = hostedFor(language());
      $("offer-size").textContent = `${files.length.toLocaleString("en")} files · ${mb(files.reduce((n, [, size]) => n + size, 0))} MB`;
    }
  };
  show();
  for (const input of document.querySelectorAll("input[name=lang]")) {
    input.onchange = () => {
      try {
        localStorage.setItem("lang", language());
      } catch {}
      show();
    };
  }
  $("pick").onclick = async () => {
    try {
      const files = await pickFiles();
      if (!looksLikeInstall(files)) {
        status("That folder has no ig2.exe and DataRaw/Text/Scenario.txt: pick the install's own folder.", true);
        return;
      }
      busy(true);
      // Every language of an own install is kept: another one needs no second pick.
      await copy(files.map(([path, file]) => [path, file, file.size]));
      await play();
    } catch (e) {
      busy(false);
      if (e.name !== "AbortError") status(`${e}`, true);
    }
  };
  $("play").onclick = play;
  $("download").onclick = async () => {
    busy(true);
    try {
      await download();
    } catch (e) {
      busy(false);
      status(`The download stopped: ${e}`, true);
    }
  };
  if (await devInstall()) return;
  $("forget").onclick = async () => {
    // The install's copy only: the saves and settings stay.
    const root = await navigator.storage.getDirectory();
    await root.removeEntry(STORE, { recursive: true }).catch(() => {});
    have = false;
    show();
    status("The stored copy is gone.");
  };
}

main().catch((e) => status(`${e}`, true));
