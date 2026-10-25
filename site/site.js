// Star field, screenshot gallery, and the latest release's version on the download button.
(() => {
  const c = document.getElementById("stars");
  const g = c.getContext("2d");
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  let stars = [];
  function resize() {
    c.width = innerWidth * devicePixelRatio;
    c.height = innerHeight * devicePixelRatio;
    const n = Math.round((innerWidth * innerHeight) / 5000);
    stars = Array.from({ length: n }, () => ({
      x: Math.random() * c.width, y: Math.random() * c.height,
      r: (Math.random() * 1.2 + 0.2) * devicePixelRatio, p: Math.random() * Math.PI * 2, s: 0.5 + Math.random(),
    }));
  }
  function draw(t) {
    g.clearRect(0, 0, c.width, c.height);
    for (const s of stars) {
      g.globalAlpha = 0.35 + 0.65 * Math.abs(Math.sin(s.p + t * 0.0006 * s.s));
      g.fillStyle = "#fff";
      g.beginPath();
      g.arc(s.x, s.y, s.r, 0, Math.PI * 2);
      g.fill();
    }
    if (!reduce) requestAnimationFrame(draw);
  }
  addEventListener("resize", resize);
  resize();
  requestAnimationFrame(draw);
})();

(() => {
  // Screenshots: site/assets/screens/<name>.jpg, listed here with their captions.
  const shots = [
    ["main-menu", "Main menu"],
    ["starmap", "Starmap"],
    ["colony", "Colony"],
    ["research", "Research"],
  ];
  const gallery = document.getElementById("gallery");
  const box = document.getElementById("lightbox");
  for (const [name, caption] of shots) {
    const f = document.createElement("figure");
    const img = new Image();
    img.src = `assets/screens/${name}.jpg`;
    img.alt = caption;
    img.loading = "lazy";
    img.onerror = () => f.remove();
    const cap = document.createElement("figcaption");
    cap.textContent = caption;
    f.append(img, cap);
    f.onclick = () => {
      box.querySelector("img").src = img.src;
      box.querySelector("p").textContent = caption;
      box.hidden = false;
    };
    gallery.append(f);
  }
  box.onclick = () => (box.hidden = true);
  addEventListener("keydown", (e) => e.key === "Escape" && (box.hidden = true));
})();

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
  } catch (_) { /* offline or rate-limited: the link to the releases page stays */ }
})();
