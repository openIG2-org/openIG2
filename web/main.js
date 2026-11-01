// The browser build's loader (ADR 0047): the player's own install, picked here, copied into the Origin Private File
// System and handed to the client file by file. No game file leaves the machine.
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
}

// The client's report when its start fails after the page handed over (no WebGPU, say).
window.ig2_failed = (message) => {
  document.querySelector("canvas")?.remove();
  $("setup").hidden = false;
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

async function copy(files) {
  await navigator.storage.persist?.();
  const root = await navigator.storage.getDirectory();
  await root.removeEntry(STORE, { recursive: true }).catch(() => {});
  const store = await storeDir(true);
  let done = 0;
  for (const [path, file] of files) {
    const parts = path.split("/");
    let dir = store;
    for (const p of parts.slice(0, -1)) dir = await dir.getDirectoryHandle(p, { create: true });
    const w = await (await dir.getFileHandle(parts.at(-1), { create: true })).createWritable();
    if (typeof file === "string") {
      // A URL of the test server (?dev-install): streamed in; pipeTo closes the file.
      const r = await fetch(file);
      if (!r.ok) throw new Error(`${file}: ${r.status}`);
      await r.body.pipeTo(w);
    } else {
      await w.write(file);
      await w.close();
    }
    if (++done % 50 === 0) status(`Copying the install into this browser: ${done} / ${files.length} files`);
  }
  await (await (await store.getFileHandle(DONE, { create: true })).createWritable()).close();
}

async function play() {
  $("pick").disabled = $("play").disabled = true;
  try {
    let n = 0;
    for await (const [path, h] of walk(await storeDir())) {
      if (path === DONE) continue;
      add_file(path, new Uint8Array(await (await h.getFile()).arrayBuffer()));
      if (++n % 200 === 0) status(`Loading the game files: ${n}`);
    }
    const user = await (await navigator.storage.getDirectory()).getDirectoryHandle(USER, { create: true });
    for await (const [path, h] of walk(user)) add_user_file(path, new Uint8Array(await (await h.getFile()).arrayBuffer()));
    status("Starting…");
    $("setup").hidden = true;
    const params = new URLSearchParams(location.search);
    // Testing only: `?args=--multiplayer` and the like, the native client's options, on localhost.
    const args = isLocal() ? params.get("args") ?? undefined : undefined;
    start($("lang").value || params.get("lang") || undefined, params.has("webgl"), args);
  } catch (e) {
    $("setup").hidden = false;
    status(`The client stopped: ${e}`, true);
    console.error(e);
  }
}

// Testing only: `tools/serve_web.py --install` serves the install on this machine; `?dev-install` copies it from there
// instead of asking for the folder. Pages not on localhost ignore it.
const isLocal = () => ["127.0.0.1", "localhost", "[::1]"].includes(location.hostname);

async function devInstall() {
  if (!isLocal() || !new URLSearchParams(location.search).has("dev-install")) return false;
  if (!(await stored())) {
    const names = await (await fetch("/dev-install/index.json")).json();
    await copy(names.map((n) => [n, "/dev-install/" + n.split("/").map(encodeURIComponent).join("/")]));
  }
  await play();
  return true;
}

async function main() {
  await init();
  const have = await stored();
  $("play").hidden = $("forget").hidden = !have;
  status(have ? "The install is stored in this browser." : "");
  $("pick").onclick = async () => {
    try {
      const files = await pickFiles();
      if (!looksLikeInstall(files)) {
        status("That folder has no ig2.exe and DataRaw/Text/Scenario.txt: pick the install's own folder.", true);
        return;
      }
      await copy(files);
      await play();
    } catch (e) {
      if (e.name !== "AbortError") status(`${e}`, true);
    }
  };
  $("play").onclick = play;
  if (await devInstall()) return;
  $("forget").onclick = async () => {
    // The install's copy only: the saves and settings stay.
    const root = await navigator.storage.getDirectory();
    await root.removeEntry(STORE, { recursive: true }).catch(() => {});
    $("play").hidden = $("forget").hidden = true;
    status("The stored copy is gone.");
  };
}

main().catch((e) => status(`${e}`, true));
