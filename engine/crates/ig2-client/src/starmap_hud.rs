//! The starmap screen's HUD (re/screens/starmap.md §11): the control panel `kezel.xvb` (a camera-fixed model of the
//! 3D pass), `StarmapGfx2D_1`'s fader box and the minimap's dark window, `StarmapGfx2D_3`'s short info of the
//! selected planet or fleet (`StarmapRovidInfoKiir`) and the minimap (`GFX_StarmapRadar`).

use ig2_formats::Install;
use ig2_sim::save::{va, SaveState};

use crate::font::{Align, Font, Virtual};
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};
use crate::render3d::{MeshDraw, MeshVertex, Scene3d, SceneUniforms};
use crate::scene::{self, Model, Placement};
use crate::world::Snapshot;

/// The virtual rectangle the panel is rendered over (it spans about 214..426 × 366..474).
const CONSOLE_RECT: [f32; 4] = [200.0, 360.0, 440.0, 480.0];
/// Texture pixels per virtual pixel for the panel.
const CONSOLE_SCALE: f32 = 4.0;

/// The control panel: `kezel.xvb` (`Init3dStarmap` ELF 0x6db470: `AddToScene(…, 0.17)`, relative to the camera at
/// (0, 170, −600), not rotated), drawn once into a texture over [`CONSOLE_RECT`].
pub struct Console {
    tex: TexId,
}

impl Console {
    /// Lit as a still frame of its lights: the scene's ambient 0x404040 and parallel 0x808080, and the two
    /// coloured lights `smOldalsoFenyekBe` (ELF 0x6d81e0) gives it, 0x7f5d00 and 0x004e7f (`0x00BBGGRR`), which
    /// `smMozgatMainFenyek` circles in front of it [inf: one warm light from the front, the shader has one].
    pub fn load(r: &mut Renderer, install: &Install) -> Result<Self, String> {
        let light = scene::normalise([0.0, -0.35, 1.0]);
        let model = Model::load(install, "vector/starmap/", "kezel.xvb", "textures/starmap/")?;
        let place = Placement::add_to_scene(model.origin, 0.17).world((0.0, 0.0, 0.0), [0.0, 170.0, -600.0]);
        let mut dynamic = Vec::new();
        let mut draws = Vec::new();
        let white = r.add_texture(&crate::assets::Image::white());
        for p in &model.parts {
            let start = dynamic.len() as u32;
            for v in &p.verts {
                dynamic.push(MeshVertex {
                    pos: place.point(v.pos),
                    normal: scene::normalise(scene::apply(&place.matrix, v.normal)),
                    uv: v.uv,
                    color: [1.0, 1.0, 1.0, 0.0],
                });
            }
            // Drawn opaque: the window in the texture's middle is black where the exe draws it (no alpha test).
            let tex = match p.texture.as_deref().map(|t| crate::assets::Image::load(install, t)) {
                Some(Ok(mut img)) => {
                    img.rgba.chunks_exact_mut(4).for_each(|px| px[3] = 255);
                    r.add_texture(&img)
                }
                _ => white,
            };
            draws.push(MeshDraw { mesh: None, range: start..dynamic.len() as u32, tex, glass: false });
        }
        let [x0, y0, x1, y1] = CONSOLE_RECT;
        let (w, h) = (x1 - x0, y1 - y0);
        // Camera space → clip for the virtual window: vx = 600·x/w + 320, vy = 600·y/w + 250 (w = −z); depth
        // 0..1 over w = 500..700.
        let rows = [
            [1200.0 / w, 0.0, -(2.0 * (320.0 - x0) / w - 1.0), 0.0],
            [0.0, -1200.0 / h, -(1.0 - 2.0 * (250.0 - y0) / h), 0.0],
            [0.0, 0.0, -3.5, -1750.0],
            [0.0, 0.0, -1.0, 0.0],
        ];
        let view = std::array::from_fn(|c| std::array::from_fn(|k| rows[k][c]));
        let mut f = Frame::new([0.0, 0.0, 0.0, 0.0]);
        f.scene = Some(Scene3d {
            uniforms: SceneUniforms {
                view,
                eye: [0.0; 4],
                light: [light[0], light[1], light[2], 0.0],
                // `Init3dStarmap`: ambient 0x404040, the parallel light 0x808080.
                ambient: [0.251, 0.251, 0.251, 0.0],
                sun: [0.502 + 0.498, 0.502 + 0.306, 0.502, 0.0],
                fog: [0.0; 4],
                fog_range: [1e9, 2e9, 0.0, 1e9],
            },
            draws,
            dynamic,
        });
        let tex = r.render_texture(&f, ((w * CONSOLE_SCALE) as u32, (h * CONSOLE_SCALE) as u32));
        Ok(Self { tex })
    }

    fn draw(&self, f: &mut Frame, v: &Virtual) {
        let [x0, y0, x1, y1] = CONSOLE_RECT;
        let c = v.px((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        f.push(self.tex, Blend::Alpha, Instance::new(kind::SPRITE, c, [(x1 - x0) * v.k / 2.0, (y1 - y0) * v.k / 2.0], [1.0; 4]));
    }
}

/// The minimap's inside (`GFX_StarmapRadar` ELF 0x70e490: `HatarX1..Y2` = 274..365 × 388..452).
pub const MINIMAP: [f32; 4] = [274.0, 388.0, 365.0, 452.0];

/// A galaxy point on the minimap: `((±x/100/3)·92)/2 + 46 + 274`, `((y/100/3)·65)/2 + 32 + 388`.
pub fn minimap_point(p: [f32; 2]) -> [f32; 2] {
    [(((-p[0] / 100.0) / 3.0) * 92.0) / 2.0 + 46.0 + 274.0, (((p[1] / 100.0) / 3.0) * 65.0) / 2.0 + 32.0 + 388.0]
}

/// What the minimap shows.
#[derive(Debug, Clone, Default)]
pub struct Minimap {
    /// Seen planets: position, `DDColor` index (`owner + 15`, unowned 12), selected.
    pub planets: Vec<([f32; 2], usize, bool)>,
    pub fleets: Vec<([f32; 2], usize, bool)>,
    /// The view's footprint: the galaxy points under the virtual corners (0, 20), (640, 20), (0, 480), (640, 480).
    pub view: [Option<[f32; 2]>; 4],
}

impl Minimap {
    /// From the frame's snapshot (fleets interpolated) and the selection (`StarmapMode`, `AktPlanet`, `AktFleet`).
    pub fn new(s: &Snapshot, fleets: &[crate::world::FleetView], sel: &crate::orders::Selection, cam: &crate::starmap::Camera) -> Self {
        let colour = |o: u8| if o == 0 { 12 } else { o as usize + 15 };
        Self {
            planets: s.planets.iter().filter(|p| p.known).map(|p| (p.pos, colour(p.owner), sel.mode == 1 && sel.planet == p.index)).collect(),
            fleets: fleets.iter().filter(|f| f.visible).map(|f| (f.pos, f.owner as usize + 15, sel.mode == 2 && sel.fleet == f.slot)).collect(),
            view: [[0.0, 20.0], [640.0, 20.0], [0.0, 480.0], [640.0, 480.0]].map(|c| cam.virtual_to_world(c)),
        }
    }
}

/// The short info lines (`StarmapRovidInfoKiir(50, 424, 0)`): text, x, y, size, `DDColor` index; and the brushes
/// `StarmapGfx2D_3` puts beside them: (record, frame, x, y).
#[derive(Debug, Clone, Default)]
pub struct InfoBox {
    pub lines: Vec<(Vec<u8>, f32, f32, usize, usize)>,
    pub brushes: Vec<(usize, i32, f32, f32)>,
}

/// `Starmap_RopIkonKonvert` (exe 0x927088): the icon frames of `StarmapBrushok` 9 by button.
const ICON_FRAME: [i32; 20] = [0, 13, 9, 10, 8, 1, 2, 6, 7, 5, 0, 0, 0, 3, 4, 0, 11, 12, 14, 0];

/// `GetString` texts the box needs.
pub struct InfoTexts<'a> {
    pub texts: &'a crate::ui::UiTexts,
    /// `StringReadAll`'s source (`Moral_Nev`, `Problemak_Nev`).
    pub hu17: &'a ig2_formats::strings::UiStrings,
    pub empire_names: &'a [Vec<u8>],
}

impl InfoTexts<'_> {
    fn s(&self, file: u16, section: u16, index: u16) -> Vec<u8> {
        self.texts.get(file, section, index)
    }
    /// `txt17` §4 from the Hungarian file: `Problemak_Nev[p]` = 21 + p, `Moral_Nev[k]` = 41 + k.
    fn names(&self, index: u16) -> Vec<u8> {
        crate::ui::latin1(self.hu17.get(0x11, 4, index))
    }
    fn empire(&self, e: u8) -> Vec<u8> {
        self.empire_names.get(e as usize).cloned().unwrap_or_default()
    }
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

fn cstr(b: &[u8]) -> &[u8] {
    &b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]
}

impl InfoBox {
    /// The exe's `StarmapRovidInfoKiir` (exe 0x5afc50) for planet mode (`StarmapMode` 1) or fleet mode, at
    /// (x, y) = (50, 424). `step` = how many 30-frame steps of `Counter` the planet has been shown (the morale line,
    /// then each problem in turn).
    pub fn new(state: &SaveState, rules: &ig2_rules::Rules, t: &InfoTexts, sel: &crate::orders::Selection, step: i32, counter: i32) -> Self {
        let (x, y) = (50.0, 424.0);
        let m = &state.mem;
        let en = m.i32(va::EN).clamp(0, 18) as u8;
        let debug = m.u8(va::DEBUGX) != 0;
        let mut lines = Vec::new();
        let mut brushes = Vec::new();
        let mut put = |s: Vec<u8>, dx: f32, dy: f32, size: usize, col: usize| lines.push((s, x + dx, y + dy, size, col));
        if sel.mode == 1 {
            let p = sel.planet;
            if p < 1 || p > state.planet_count() {
                return Self::default();
            }
            let r = m.read(va::PLANETS + (p * ig2_sim::save::PLANET_SIZE) as u32, ig2_sim::save::PLANET_SIZE);
            let i32_at = |o: usize| i32::from_le_bytes(r[o..o + 4].try_into().unwrap());
            let owner = r[0x20];
            // `Starmap_MouseInit2` (ELF 0x7024b0): the tanks button (`StarmapTanksGomb`: the player's fleet near) and
            // the production button (`StarmapProductGomb`) on the player's planet.
            let (tanks, product) = crate::starmap_panels::Panels::buttons(state, sel);
            if tanks {
                brushes.push((9, ICON_FRAME[1], 171.0, 430.0));
            }
            if product || debug {
                brushes.push((9, ICON_FRAME[2], 419.0, 430.0));
            }
            put(cstr(&r[..0x18]).to_vec(), 0.0, 3.0, 2, 0x22);
            // Race 7 sees planets without a `+0xcb` flag; a satellite (`+0xc4 + EN`) shows the details.
            let seen = owner == en || (en == 7 && r[0xcb] == 0) || r[0xc4 + en as usize] != 0 || debug;
            if seen {
                put(b"\x03:".to_vec(), -42.0, 3.0, 2, 1);
                put(cat(&[richness(state, rules, p, en).to_string().as_bytes(), b"%"]), -32.0, 4.0, 1, 1);
                let race = if owner == 0 { en as usize } else { r[0x21] as usize };
                let growth = rules.race_surface_affinity.get(race).and_then(|v| v.get(r[0x22] as usize)).copied().unwrap_or(0);
                put(b"\x01:".to_vec(), -42.0, 42.0, 2, 1);
                put(cat(&[growth.to_string().as_bytes(), b"%"]), -32.0, 43.0, 1, 1);
            }
            if owner != en {
                let level = if en == 7 && r[0xcb] == 0 { 1 } else { r[0xc4 + en as usize] };
                if level != 0 || (en == 7 && r[0xcb] == 0) {
                    // `Talalmanyok[level + 0xe]`: the satellite's tech name.
                    put(crate::hud::tech_name(t.texts, level as usize + 0xe), 0.0, 33.0, 1, 0x23);
                }
            }
            put(t.empire(owner), 0.0, 15.0, 1, 0x23);
            if owner != 0 && seen {
                let line = format!(": {} ({:+})", i32_at(0x28), i32_at(0x2c));
                put(cat(&[&t.s(2, 3, 1), line.as_bytes()]), 0.0, 24.0, 1, 0x23);
            }
            if owner == en || (debug && owner != 0) {
                let order = problem_order(&r, debug);
                let k = step.max(0) as usize % (order.len() + 1);
                let text = if k < 1 {
                    let morale = (i32_at(0x30) / 100) as f32 as i32;
                    cat(&[&t.s(2, 3, 2), b": ", &t.names((41 + morale).clamp(0, 99) as u16)])
                } else {
                    let pr = order[k - 1];
                    let v = i32_at(0x118 + 4 * pr);
                    cat(&[format!("{}% ", v / 10000).as_bytes(), &t.names((21 + pr) as u16)])
                };
                put(text, 0.0, 33.0, 1, 0x23);
                if let Some(s) = production_line(state, &r, owner, counter) {
                    put(s, 0.0, 42.0, 1, 1);
                }
            }
        } else if sel.mode == 2 && sel.fleet > 0 {
            let map = state.starmap();
            let Some(fl) = map.fleets.get(sel.fleet).filter(|f| f.owner != ig2_sim::fleet::DELETED) else { return Self::default() };
            let totals = state.combat().fleets.get(sel.fleet).map(|c| c.totals).unwrap_or_default();
            let (name_col, col) = if fl.flags & 0x400 != 0 { (6, 6) } else if fl.flags & 0x4000 != 0 { (1, 1) } else { (3, 0x21) };
            // A protected fleet's mark (brush 11 at (6, 430)), else a wormhole's (brush 3 at (6, 429)).
            if fl.flags & 0x4000 != 0 {
                brushes.push((11, 1, 6.0, 430.0));
            } else if fl.flags & 0x400 != 0 {
                brushes.push((3, 1, 6.0, 429.0));
            }
            if crate::starmap_panels::Panels::buttons(state, sel).0 {
                brushes.push((9, ICON_FRAME[1], 171.0, 430.0));
            }
            put(cstr(&fl.name).to_vec(), 0.0, 3.0, 2, name_col);
            put(t.empire(fl.owner), 0.0, 15.0, 1, col);
            let mine = fl.owner == en || debug;
            if mine {
                if fl.order == 0 {
                    put(t.s(2, 3, 3), 0.0, 24.0, 1, col);
                } else {
                    let target = match fl.kind {
                        0 => t.s(2, 3, 4),
                        1 => {
                            let i = fl.tx as i32;
                            let r = m.read(va::PLANETS + (i.max(0) as usize * ig2_sim::save::PLANET_SIZE) as u32, 0x18);
                            cstr(&r).to_vec()
                        }
                        _ => map.fleets.get((fl.tx as i32).max(0) as usize).map(|g| cstr(&g.name).to_vec()).unwrap_or_default(),
                    };
                    let head = if fl.order == 1 { t.s(2, 3, 5) } else { t.s(2, 3, 6) };
                    put(cat(&[&head, b":", &target]), 0.0, 24.0, 1, col);
                }
            }
            if mine && fl.flags & 0x4400 == 0 {
                let (ships, fighters, ground, cap) = (totals.ships - totals.fighters, totals.fighters, totals.ground, totals.capacity);
                // "SH:%d FI:%d TA:%d/%d", shortened to "S:… F:… T:…/…", then without the capacity.
                let line = |a: u16, with_cap: bool| {
                    let tail = if with_cap { format!(":{ground}/{cap}") } else { format!(":{ground}") };
                    cat(&[&t.s(2, 3, a), format!(":{ships} ").as_bytes(), &t.s(2, 3, a + 1), format!(":{fighters} ").as_bytes(), &t.s(2, 3, a + 2), tail.as_bytes()])
                };
                let mut s = line(7, true);
                if s.len() > 0x13 {
                    s = line(10, true);
                }
                if s.len() > 0x13 {
                    s = line(10, false);
                }
                put(s, 0.0, 33.0, 1, col);
            }
            if mine && fl.flags & 0x400 == 0 {
                put(cat(&[&t.s(2, 3, 13), format!(": {}", (fl.speed * 100.0 + 0.5) as i32).as_bytes()]), 0.0, 42.0, 1, col);
            }
        }
        Self { lines, brushes }
    }
}

/// `GetBolygoGazdagsag` (ELF 0x4fb180): richness × the owner's race production multiplier, plus the event bonus.
fn richness(state: &SaveState, rules: &ig2_rules::Rules, p: usize, en: u8) -> i32 {
    let c = state.colony();
    let Some(pl) = c.planets.get(p) else { return 0 };
    let mut bonus = 0;
    if state.mem.u8(va::MULTIPLAYER) == 0 && pl.any_event() {
        if pl.has_event(0xc3a) || pl.has_event(0x1f41) {
            bonus = 20;
        }
        if pl.has_event(0x1f42) {
            bonus = 10;
        }
        if pl.has_event(0x91b) {
            bonus = 15;
        }
    }
    let e = if pl.owner == 0 { en } else { pl.owner };
    let race = state.races().get(e as usize).copied().unwrap_or(0) as usize;
    (pl.richness as f32 * rules.race_production_mult.get(race).copied().unwrap_or(1.0) + bonus as f32) as i32
}

/// `StarmapProblemaRendez` (ELF 0x7082c0): the problems 1..=19 at 100000 or more (`+0x118 + 4p`), the four worst
/// first.
fn problem_order(r: &[u8], debug: bool) -> Vec<usize> {
    let v = |p: usize| i32::from_le_bytes(r[0x118 + 4 * p..0x11c + 4 * p].try_into().unwrap());
    let mut order: Vec<usize> = Vec::new();
    for c in 1..0x14 {
        if v(c) > 99999 || debug {
            let mut at = order.len();
            while at > 0 && v(order[at - 1]) < v(c) {
                at -= 1;
            }
            if at < 4 {
                order.insert(at, c);
                order.truncate(4);
            }
        }
    }
    order
}

/// The production line: the head ship (line 0) or vehicle (line 1; every other 30 frames when both) and how much
/// of it is built.
fn production_line(state: &SaveState, r: &[u8], owner: u8, counter: i32) -> Option<Vec<u8>> {
    let head = |l: usize| i16::from_le_bytes([r[0x78 + 8 * l], r[0x79 + 8 * l]]);
    let mut line = if head(0) > 0 { Some(0) } else { None };
    if head(1) > 0 && ((counter / 30) % 2 == 0 || line.is_none()) {
        line = Some(1);
    }
    let l = line?;
    let designs = state.designs.get(owner as usize)?;
    let d = designs.get(head(l) as usize)?;
    let name = cstr(&d[..0x14]).to_vec();
    let time = i32::from_le_bytes(d[0x20..0x24].try_into().unwrap()).max(1);
    let left = i32::from_le_bytes(r[0xa0 + 4 * l..0xa4 + 4 * l].try_into().unwrap());
    let pct = 100 - left * 100 / time;
    let mut s = cat(&[&name, format!(" - {pct}%").as_bytes()]);
    if s.len() > 0x13 {
        s = cat(&[&name, format!("-{pct}%").as_bytes()]);
    }
    if s.len() > 0x13 {
        s = cat(&[&name, format!("{pct}%").as_bytes()]);
    }
    Some(s)
}

/// `KirakFader(i, …, x, y, w, h, 0, 54s, 90s, 155l + 100, 0, 90s, 130s, 185l + 70)` (ELF 0x547920 →
/// `FadeKerettelKerekit` 0x538330: a rounded fill and its frame) with s = 1 and l = 0: the fill (0, 54, 90) at
/// opacity 100/255 and a one-pixel frame (0, 90, 130) at 70/255 [obs: exe pixels inside the box; inf: the rounded
/// corners are left square].
pub fn fader(f: &mut Frame, white: TexId, v: &Virtual, r: [f32; 4]) {
    let [x, y, w, h] = r;
    let quad = |f: &mut Frame, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]| {
        f.push(white, Blend::Alpha, Instance::new(kind::SPRITE, v.px(x + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], c));
    };
    quad(f, x, y, w, h, srgb_blend([0.0, 54.0, 90.0], 100.0 / 255.0));
    let edge = srgb_blend([0.0, 90.0, 130.0], 70.0 / 255.0);
    quad(f, x, y, w, 1.0, edge);
    quad(f, x, y + h - 1.0, w, 1.0, edge);
    quad(f, x, y + 1.0, 1.0, h - 2.0, edge);
    quad(f, x + w - 1.0, y + 1.0, 1.0, h - 2.0, edge);
}

/// The tint and alpha that blend in this sRGB frame (in linear light) as the exe's `c` (0..255) at opacity `a`
/// blends in sRGB: the background keeps `(1 − a)` of its sRGB value (≈ `(1 − a)^2.2` linear), and over black the
/// result is `a·c` exactly.
pub fn srgb_blend(c: [f32; 3], a: f32) -> [f32; 4] {
    let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    let eff = 1.0 - (1.0 - a).powf(2.2);
    let k = |v: f32| if eff > 0.0 { lin(a * v / 255.0) / eff } else { 0.0 };
    [k(c[0]), k(c[1]), k(c[2]), eff]
}

/// The HUD's resources.
pub struct StarmapHud {
    pub console: Option<Console>,
    /// `StarmapRovidInfoKiir`'s statics: the planet shown and the `Counter` its rotation started at.
    pub rotation: std::cell::Cell<(usize, i32)>,
}

impl StarmapHud {
    /// The problem line's step for `planet` at UI frame `counter` (restarting when the planet changes).
    pub fn step(&self, planet: usize, counter: i32) -> i32 {
        let (p, start) = self.rotation.get();
        if p != planet {
            self.rotation.set((planet, counter));
            return 0;
        }
        (counter - start) / 30
    }

    /// Draw the panel, the faders, the short info and the minimap (in the original's pass order).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&self, f: &mut Frame, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], pics: &crate::hud::Pictures, brushes: &[ig2_rules::Brush], info: &InfoBox, mini: &Minimap, time: f32) {
        let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
        if let Some(c) = &self.console {
            c.draw(f, v);
        }
        // `StarmapGfx2D_1`: the fader behind the short info, then the minimap's dark window (`FadeScreen(271, 385,
        // 98, 71, black, 0x40)`; the value is the transparency [obs: the window is dark in the captures]).
        fader(f, white, v, [5.0, 423.0, 162.0, 52.0]);
        f.push(white, Blend::Alpha, Instance::new(kind::SPRITE, v.px(271.0 + 49.0, 385.0 + 35.5), [49.0 * v.k, 35.5 * v.k], srgb_blend([0.0; 3], 1.0 - 0x40 as f32 / 255.0)));
        for &(b, n, x, y) in &info.brushes {
            if let Some(b) = brushes.get(b) {
                pics.brush(f, v, b, n, Some([x, y]), [1.0; 4]);
            }
        }
        for (s, x, y, size, c) in &info.lines {
            font.text(f, v, s, *x, *y, 0x13, *size, Align::Left, col(*c), None);
        }
        // `GFX_StarmapRadar`: fleets as 2 × 1 dots, then planets 2 × 2, the selected one blinking; the view's
        // outline in colour 1, clipped to the window.
        let inside = |q: [f32; 2]| q[0] > 274.0 && q[0] < 366.0 && q[1] > 388.0 && q[1] < 453.0;
        let dot = |f: &mut Frame, q: [f32; 2], w: f32, h: f32, c: [f32; 3], a: f32| {
            let (x, y) = ((q[0] - 1.0).trunc(), (q[1] - 1.0).trunc());
            f.push(white, Blend::Alpha, Instance::new(kind::SPRITE, v.px(x + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], a]));
        };
        let blink = crate::hud::blink(time, 100.0);
        for &(p, c, sel) in &mini.fleets {
            let q = minimap_point(p);
            if inside(q) {
                dot(f, q, 2.0, 1.0, col(c), if sel { blink } else { 1.0 });
            }
        }
        for &(p, c, sel) in &mini.planets {
            let q = minimap_point(p);
            if inside(q) {
                dot(f, q, 2.0, 2.0, col(c), if sel { blink } else { 1.0 });
            }
        }
        let corner = |i: usize| mini.view[i].map(minimap_point);
        for (a, b) in [(0, 1), (0, 2), (2, 3), (1, 3)] {
            if let (Some(a), Some(b)) = (corner(a), corner(b)) {
                if let Some((a, b)) = crate::hud::clip_line(a, b, MINIMAP) {
                    let c = col(1);
                    f.push(white, Blend::Alpha, Instance::line(v.px(a[0], a[1]), v.px(b[0], b[1]), v.k.max(1.0), [c[0], c[1], c[2], 1.0]));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimap_maps_the_galaxy_mirrored_in_x() {
        assert_eq!(minimap_point([0.0, 0.0]), [320.0, 420.0]);
        let q = minimap_point([300.0, -300.0]);
        assert!((q[0] - 274.0).abs() < 1e-4 && (q[1] - 387.5).abs() < 1e-4);
    }

    #[test]
    fn problems_sort_worst_first_and_keep_four() {
        let mut r = vec![0u8; 0x188];
        for (p, v) in [(2, 150000i32), (5, 400000), (7, 200000), (9, 120000), (11, 900000), (3, 5)] {
            r[0x118 + 4 * p..0x11c + 4 * p].copy_from_slice(&v.to_le_bytes());
        }
        assert_eq!(problem_order(&r, false), vec![11, 5, 7, 2]);
    }
}
