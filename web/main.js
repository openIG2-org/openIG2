// The browser build's loader (ADR 0047): the player's own install, picked here, copied into the Origin Private File
// System and handed to the client file by file. No game file leaves the machine.
import init, { add_file, start } from "./pkg/ig2_client.js";

const $ = (id) => document.getElementById(id);
const STORE = "install";
// Written last: a copy without it was interrupted.
const DONE = ".openig2-complete";
// The videos wait for the browser's decoder (ADR 0047 part 3).
const SKIP = /\.mp4$/i;

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
    await w.write(file);
    await w.close();
    if (++done % 50 === 0) status(`Copying the install into this browser: ${done} / ${files.length} files`);
  }
  await (await (await store.getFileHandle(DONE, { create: true })).createWritable()).close();
}

async function play() {
  $("pick").disabled = $("play").disabled = true;
  try {
    let n = 0;
    for await (const [path, h] of walk(await storeDir())) {
      if (path === DONE || SKIP.test(path)) continue;
      add_file(path, new Uint8Array(await (await h.getFile()).arrayBuffer()));
      if (++n % 200 === 0) status(`Loading the game files: ${n}`);
    }
    status("Starting…");
    $("setup").hidden = true;
    start($("lang").value || new URLSearchParams(location.search).get("lang") || undefined);
  } catch (e) {
    $("setup").hidden = false;
    status(`The client stopped: ${e}`, true);
    console.error(e);
  }
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
  $("forget").onclick = async () => {
    const root = await navigator.storage.getDirectory();
    await root.removeEntry(STORE, { recursive: true }).catch(() => {});
    $("play").hidden = $("forget").hidden = true;
    status("The stored copy is gone.");
  };
}

main().catch((e) => status(`${e}`, true));
