// Star field: slow twinkle, still frame when reduced motion is on.
(() => {
  const c = document.getElementById("stars");
  const g = c.getContext("2d");
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const tint = ["255,255,255", "190,210,255", "255,226,170"];
  let stars = [];
  function resize() {
    const d = devicePixelRatio || 1;
    c.width = innerWidth * d; c.height = innerHeight * d;
    const n = Math.round((innerWidth * innerHeight) / 6500);
    stars = Array.from({ length: n }, () => ({
      x: Math.random() * c.width, y: Math.random() * c.height,
      r: (Math.random() ** 2 * 1.3 + 0.25) * d, p: Math.random() * 6.28, s: 0.4 + Math.random(),
      t: tint[Math.random() < 0.8 ? 0 : Math.random() < 0.6 ? 1 : 2],
    }));
    if (reduce) draw(0);
  }
  function draw(t) {
    g.clearRect(0, 0, c.width, c.height);
    for (const s of stars) {
      const a = 0.25 + 0.6 * Math.abs(Math.sin(s.p + t * 0.0005 * s.s));
      g.fillStyle = `rgba(${s.t},${a})`;
      g.beginPath(); g.arc(s.x, s.y, s.r, 0, 6.2832); g.fill();
    }
    if (!reduce) requestAnimationFrame(draw);
  }
  addEventListener("resize", resize);
  resize();
  if (!reduce) requestAnimationFrame(draw);
})();

// Tick diagram: 250 ms, 20 Hz ticks against 144 Hz frames, drawn to one scale.
(() => {
  const svg = document.getElementById("tick-svg");
  const ns = "http://www.w3.org/2000/svg";
  const x0 = 24, x1 = 616, ms = 250, X = (t) => x0 + (t / ms) * (x1 - x0);
  const css = getComputedStyle(document.documentElement);
  const col = (n) => css.getPropertyValue(n).trim();
  const el = (tag, a) => { const e = document.createElementNS(ns, tag); for (const k in a) e.setAttribute(k, a[k]); svg.append(e); return e; };
  el("line", { x1: x0, x2: x1, y1: 52, y2: 52, stroke: col("--line"), "stroke-width": 1 });
  for (let t = 0; t <= ms + 0.01; t += 1000 / 144) el("rect", { x: X(t) - 1, y: 36, width: 2, height: 16, fill: col("--ion"), opacity: .75 });
  for (let t = 0; t <= ms; t += 50) {
    el("rect", { x: X(t) - 2, y: 18, width: 4, height: 34, fill: col("--readout") });
    const lbl = el("text", { x: X(t), y: 74, "text-anchor": "middle", fill: col("--dim"),
      "font-family": "Share Tech Mono, monospace", "font-size": 12 });
    lbl.textContent = `${t} ms`;
  }
})();

// Screenshot switcher.
(() => {
  const shots = [
    ["main-menu", "Main menu", "The original's galaxy menu, at full resolution."],
    ["starmap", "Starmap", "Territories, fleets and the date and credit readouts of the top bar."],
    ["colony", "Colony", "Buildings, scaffolding, weather and smoke, as in the original."],
    ["research", "Research", "The tech tree, with the original's rules for cost and time."],
  ];
  const list = document.getElementById("thumbs");
  const big = document.getElementById("shot"), name = document.getElementById("shot-name"), text = document.getElementById("shot-text");
  const buttons = shots.map(([file, title, desc], i) => {
    const li = document.createElement("li");
    const b = document.createElement("button");
    b.type = "button"; b.id = `shot-${file}`;
    b.setAttribute("aria-pressed", i === 0 ? "true" : "false");
    b.innerHTML = `<img src="assets/screens/${file}.jpg" alt="" loading="lazy"><span class="t-text"><b>${title}</b><small>${file}.jpg</small></span>`;
    b.setAttribute("aria-label", title);
    b.onclick = () => show(i);
    li.append(b); list.append(li);
    return b;
  });
  function show(i) {
    const [file, title, desc] = shots[i];
    big.src = `assets/screens/${file}.jpg`; big.alt = title;
    name.textContent = title; text.textContent = desc;
    buttons.forEach((b, j) => b.setAttribute("aria-pressed", String(i === j)));
  }
  show(0);
})();

// Copy the build commands.
(() => {
  const btn = document.getElementById("copy-build");
  const pre = document.getElementById("build-cmd");
  btn.addEventListener("click", () => {
    const cmd = [...pre.childNodes].filter((n) => !(n.classList && (n.classList.contains("p") || n.classList.contains("c"))))
      .map((n) => n.textContent).join("").trim();
    const done = (msg) => { btn.textContent = msg; setTimeout(() => (btn.textContent = "Copy"), 1600); };
    if (navigator.clipboard) {
      navigator.clipboard.writeText(cmd).then(() => done("Copied"), () => select());
    } else select();
    function select() {
      const r = document.createRange(); r.selectNodeContents(pre);
      const s = getSelection(); s.removeAllRanges(); s.addRange(r); done("Selected");
    }
  });
})();

// Latest release on the download button and the HUD; the releases page link stays when this fails.
(async () => {
  try {
    const r = await fetch("https://api.github.com/repos/openIG2-org/openIG2/releases?per_page=1");
    if (!r.ok) return;
    const [rel] = await r.json();
    if (!rel) return;
    const zip = rel.assets.find((a) => a.name.endsWith("-windows-x64.zip"));
    if (zip) document.getElementById("download").href = zip.browser_download_url;
    document.getElementById("dl-version").textContent =
      `${rel.tag_name}${rel.prerelease ? " beta" : ""} · Windows 10/11, 64-bit`;
    document.getElementById("hud-version").textContent = `${rel.tag_name}${rel.prerelease ? " beta" : ""}`;
  } catch (_) { /* offline or blocked */ }
})();
