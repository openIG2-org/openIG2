//! The HUD drawn over the screens (re/screens/hud.md): brush records on the game's 2D pictures, the top bar
//! every screen shows (money, date, speed buttons, the message line's frame), and the colony screen's panels and
//! radar.

use std::collections::HashMap;

use ig2_formats::Install;
use ig2_rules::Brush;

use crate::assets::Image;
use crate::font::{Align, Font, Virtual};
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};

/// The pictures (`Kepek`) the HUD draws from, loaded once: kep → texture and its size in pixels.
#[derive(Default)]
pub struct Pictures {
    loaded: HashMap<i32, (TexId, [f32; 2])>,
}

/// Kep 14 (`BRUSHOK.BMP`): `HACK_SetBrushok` swaps in the original bar's sheet when `IG2_TOOLBAR` is clear (§1).
const BAR_SHEET: (i32, &str) = (14, "BRUSHOK_ORIG.BMP");

impl Pictures {
    /// Load pictures `keps` by their `Kepek` names: `dataraw/screen/<name, lowercase, '.' → '_'>.pvr`.
    pub fn load(r: &mut Renderer, install: &Install, names: &[String], keps: &[i32]) -> Self {
        let mut loaded = HashMap::new();
        for &k in keps {
            let name = if k == BAR_SHEET.0 { BAR_SHEET.1 } else { names.get(k as usize).map_or("", String::as_str) };
            if name.is_empty() {
                continue;
            }
            let path = format!("dataraw/screen/{}.pvr", name.to_ascii_lowercase().replace('.', "_"));
            match Image::load(install, &path) {
                Ok(img) => {
                    let size = [img.width as f32, img.height as f32];
                    loaded.insert(k, (r.add_texture(&img), size));
                }
                Err(e) => eprintln!("ig2-client: {e}"),
            }
        }
        Self { loaded }
    }

    /// `BrushToScreenMelyikXY(b, n, at)` → `DDBrushToScreen3d` (§1): frame `n` (1-based) of the brush at `at` (else
    /// its own place), `|w| × |h|` virtual pixels, alpha-blended; a negative size reads the source at 2×.
    pub fn brush(&self, f: &mut Frame, v: &Virtual, b: &Brush, n: i32, at: Option<[f32; 2]>, tint: [f32; 4]) {
        let Some(&(tex, [tw, th])) = self.loaded.get(&b.kep) else { return };
        let per_row = b.per_row.max(1);
        let n = n.max(1) - 1;
        let src = [b.src[0] + b.step[0] * (n % per_row), b.src[1] + b.step[1] * (n / per_row)];
        let k = |s: i32| if s < 0 { 2.0 } else { 1.0 };
        let (kx, ky) = (k(b.size[0]), k(b.size[1]));
        let (w, h) = (b.size[0].abs() as f32, b.size[1].abs() as f32);
        let [x, y] = at.unwrap_or([b.dst[0] as f32, b.dst[1] as f32]);
        let mut i = Instance::new(kind::SPRITE, v.px(x + w / 2.0, y + h / 2.0), [w * v.k / 2.0, h * v.k / 2.0], tint);
        i.uv = [src[0] as f32 * kx / tw, src[1] as f32 * ky / th, w * kx / tw, h * ky / th];
        f.push(tex, Blend::GammaAlpha, i);
    }
}

const fn rec(src: [i32; 2], dst: [i32; 2], size: [i32; 2], step: [i32; 2], per_row: i32) -> Brush {
    Brush { src, dst, size, step, per_row, kep: BAR_SHEET.0 }
}

/// The GOG build's top-bar records (`HACK_SetBrushok` ELF 0x569860 / exe 0x4ef690 writes them as code immediates
/// over `Brushok`): 1003 the frame of the three boxes, 1005 the research button (3 frames).
const BAR: Brush = rec([0, 0], [75, 0], [-525, -20], [0, 0], 1);
const RESEARCH: Brush = rec([0, 40], [600, 0], [-40, -20], [43, 18], 3);
/// The speed buttons 1008..=1011 (selected) and 1012..=1015: pause, ×1, ×2, ×3.
const SPEED: [Brush; 8] = [
    rec([1, 20], [0, 0], [-15, -20], [15, 20], 1),
    rec([16, 20], [15, 0], [-14, -20], [14, 20], 1),
    rec([30, 20], [29, 0], [-21, -20], [21, 20], 1),
    rec([51, 20], [50, 0], [-23, -20], [23, 20], 1),
    rec([77, 20], [0, 0], [-15, -20], [15, 20], 1),
    rec([92, 20], [15, 0], [-14, -20], [14, 20], 1),
    rec([106, 20], [29, 0], [-21, -20], [21, 20], 1),
    rec([127, 20], [50, 0], [-23, -20], [23, 20], 1),
];

/// What the top bar shows.
#[derive(Debug, Clone, Copy, Default)]
pub struct BarState {
    /// `MoneyMennyi(EN, 0)`.
    pub money: f64,
    /// `Ev`, `Honap`, `Nap`.
    pub date: (i32, i32, i32),
    /// `ResearchAktFejlesztett[EN] > 0`; some tech at status 3 or 4 for the human.
    pub researching: bool,
    pub researchable: bool,
    /// `IdoSpeed` 0..=2 and `IdoPause`.
    pub speed: usize,
    pub paused: bool,
}

/// `IdoSpeed` (0..=2) for an `IdoSpeedSzorzo` of 1, 4 or 16 (single player).
pub fn speed_index(ido_per_tick: i32) -> usize {
    match ido_per_tick {
        ..=1 => 0,
        2..=4 => 1,
        _ => 2,
    }
}

/// `GFX_AllandoCallBack1` + `GFX_IdospeedPenzDatum` (ELF 0x66c750, 0x66a590; §2): the bar's frame, the research
/// button, the speed buttons, the money and the date. The message line is drawn into the bar's third box after.
pub fn draw_bar(f: &mut Frame, pics: &Pictures, font: &Font, v: &Virtual, palette: &[[f32; 3]], s: &BarState) {
    let white = [1.0; 4];
    pics.brush(f, v, &BAR, 1, None, white);
    let research = if s.researching { 3 } else if s.researchable { 1 } else { 2 };
    pics.brush(f, v, &RESEARCH, research, None, white);
    let selected = if s.paused { 1 } else { s.speed + 2 };
    for i in 1..=4 {
        pics.brush(f, v, &SPEED[if i == selected { i - 1 } else { i + 3 }], 1, None, white);
    }
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    let money = format!("${}", (s.money + 0.5).floor() as i64);
    font.text(f, v, money.as_bytes(), 78.0, 4.0, 10, 3, Align::Field, col(0x21), None);
    let (y, m, d) = s.date;
    font.int(f, v, y as i64, 171.0, 4.0, 4, 3, Align::Right, col(0x21));
    font.text(f, v, b":", 205.0, 4.0, 1, 3, Align::Right, col(2), None);
    font.int(f, v, m as i64, 215.0, 4.0, 2, 3, Align::Right, col(0x21));
    font.text(f, v, b":", 233.0, 4.0, 1, 3, Align::Right, col(2), None);
    font.int(f, v, d as i64, 243.0, 4.0, 2, 3, Align::Right, col(0x21));
}

/// What the colony screen's HUD shows of the planet (re/screens/hud.md §3).
#[derive(Debug, Clone, Default)]
pub struct ColonyHud {
    /// The human's planet and not at war (`+0x20 == EN`, `+0x64` bit 0 clear).
    pub own: bool,
    /// The planet panel is drawn (not in a ground war once EN's units have landed).
    pub panel: bool,
    /// Workers and energy surplus: `+0x28 − +0x50`, `+0x48 − +0x4c`.
    pub workers: i32,
    pub energy: i32,
    /// The build menu's building (`AktEpuletTipus`): name, price, energy and worker demand.
    pub build: Option<(Vec<u8>, i32, i32, i32)>,
    /// The selected building (`AktEpulet`): its damage and `+0x19` flags.
    pub selected: Option<(i32, u8)>,
    /// Radar dots: buildings `(gx, gy, colour)`, units `(gx, gy, own)`, and the view's four corners on the ground
    /// (radar pixels).
    pub buildings: Vec<(f32, f32, usize)>,
    pub units: Vec<(f32, f32, bool)>,
    pub view: Option<[[f32; 2]; 4]>,
    /// The planet panel's turning building, rendered over the whole virtual screen (`crate::colony_ui::PanelBuilding`).
    pub panel_model: Option<TexId>,
}

/// The radar square (`DrawRadar` ELF 0x4cb220: `HatarX1..Y2` = 522..621 × 362..461).
pub const RADAR: [f32; 4] = [522.0, 362.0, 621.0, 461.0];

/// The colony screen's 2D passes (`GFX_Colony` faders, `ColonyGfx2D_1..3`): the radar (frame 13, the picture,
/// `DrawRadar`), the bottom-left surplus line, the planet panel (12) with the build menu's building.
#[allow(clippy::too_many_arguments)]
pub fn draw_colony(f: &mut Frame, pics: &Pictures, font: &Font, white: TexId, v: &Virtual, palette: &[[f32; 3]], brushes: &[Brush], radar: Option<TexId>, c: &ColonyHud) {
    let col = |i: usize| palette.get(i).copied().unwrap_or([1.0; 3]);
    let full = [1.0; 4];
    // `KirakFader`s behind the bottom-left info; the two over the surplus line only with it (EN's planet, not at war).
    for r in [[6.0, 403.0, 77.0, 17.0], [89.0, 403.0, 77.0, 17.0], [5.0, 423.0, 162.0, 52.0]] {
        if c.own || r[1] > 410.0 {
            crate::starmap_hud::fader(f, white, v, r);
        }
    }
    // `ColonyGfx2D_1`: the radar frame, then the picture (`CopyToScreen(28)`), then `DrawRadar`.
    if let Some(b) = brushes.get(13) {
        pics.brush(f, v, b, 1, None, full);
    }
    if let Some(t) = radar {
        let (x0, y0) = (RADAR[0], RADAR[1]);
        f.push(t, Blend::Alpha, Instance::new(kind::SPRITE, v.px(x0 + 50.0, y0 + 50.0), [50.0 * v.k; 2], full));
    }
    let dot = |f: &mut Frame, x: f32, y: f32, s: f32, c: [f32; 3]| {
        if x >= RADAR[0] && y >= RADAR[1] && x <= RADAR[2] && y <= RADAR[3] {
            f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(x + s / 2.0, y + s / 2.0), [s * v.k / 2.0; 2], [c[0], c[1], c[2], 1.0]));
        }
    };
    for &(gx, gy, c) in &c.buildings {
        dot(f, (gx / 2.0 + 522.0).trunc(), (gy / 2.0 + 362.0).trunc(), 2.0, col(c));
    }
    for &(gx, gy, own) in &c.units {
        let rgb = if own { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        dot(f, (gx as i32 / 2 + 522) as f32, (gy as i32 / 2 + 362) as f32, 1.0, rgb);
    }
    if let Some(q) = c.view {
        for (a, b) in [(q[0], q[1]), (q[0], q[2]), (q[3], q[2]), (q[3], q[1])] {
            if let Some((a, b)) = clip_line(a, b, RADAR) {
                f.push(white, Blend::GammaAlpha, Instance::line(v.px(a[0], a[1]), v.px(b[0], b[1]), v.k.max(1.0), [1.0; 4]));
            }
        }
    }
    // `ColonyGfx2D_2`: workers and energy surplus (bytes 1 and 2 are the font's icons).
    if c.own {
        let line = |n: i32, icon: u8| {
            let mut s = vec![icon, b':'];
            s.extend(format!("{n:+}").bytes());
            s
        };
        font.text(f, v, &line(c.workers, 1), 4.0, 406.0, 10, 3, Align::Field, col(if c.workers < 0 { 6 } else { 1 }), None);
        font.text(f, v, &line(c.energy, 2), 87.0, 406.0, 10, 3, Align::Field, col(if c.energy < 0 { 6 } else { 1 }), None);
    }
    // The selected building's buttons: repair (1) when damaged, demolish (2) and on/off (29, frame 2 when `+0x19`
    // bit 5 is set); none for a flagged (`+0x19` bit 4) one.
    if let (true, Some((damage, flags))) = (c.own && brushes.len() > 29, c.selected) {
        if flags & 0x10 == 0 {
            if damage > 0 {
                pics.brush(f, v, &brushes[1], 1, None, full);
            }
            pics.brush(f, v, &brushes[2], 1, None, full);
            pics.brush(f, v, &brushes[29], if flags & 0x20 != 0 { 2 } else { 1 }, None, full);
        }
    }
    // `ColonyGfx2D_3`: the planet panel over the second 3D view of the build menu's building (a dark viewport,
    // 18..173 × 36..167 [obs], with `buildshown` turning in it), and its name and costs.
    let own_panel = c.own && brushes.len() > 12;
    if own_panel {
        f.push(white, Blend::Alpha, Instance::new(kind::SPRITE, v.px(95.5, 101.5), [77.5 * v.k, 65.5 * v.k], [0.02, 0.03, 0.06, 1.0]));
        if let Some(t) = c.panel_model {
            crate::colony_ui::PanelBuilding::draw(f, v, t);
        }
    }
    if let Some(b) = brushes.get(12).filter(|_| c.panel) {
        pics.brush(f, v, b, 1, None, full);
    }
    if let (true, Some((name, price, energy, workers))) = (c.own, &c.build) {
        font.text(f, v, name, 54.0, 141.0, 16, 2, Align::Field, col(0x25), None);
        for (y, head, n) in [(61.0, b"$:", *price), (92.0, b"\x02:", *energy), (123.0, b"\x01:", *workers)] {
            let mut s = head.to_vec();
            s.extend(n.to_string().bytes());
            font.text(f, v, &s, 160.0, y, 7, 2, Align::Left, col(0x25), None);
        }
    }
}

/// A segment clipped to the box `[x0, y0, x1, y1]` (`LineClip`), `None` when it stays outside.
pub fn clip_line(a: [f32; 2], b: [f32; 2], r: [f32; 4]) -> Option<([f32; 2], [f32; 2])> {
    let (d0, d1) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [(-d0, a[0] - r[0]), (d0, r[2] - a[0]), (-d1, a[1] - r[1]), (d1, r[3] - a[1])] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then(|| ([a[0] + t0 * d0, a[1] + t0 * d1], [a[0] + t1 * d0, a[1] + t1 * d1]))
}

/// `AktIdo300Szazalek`/`AktIdo100Szazalek`-style blinking: 1 or 0.35 every `ms` milliseconds [inf].
pub fn blink(time: f32, ms: f32) -> f32 {
    if (time * 1000.0 / ms) as i64 % 2 == 0 {
        1.0
    } else {
        0.35
    }
}

/// `txt17.txt`'s tech names (the localised `Talalmanyok[t]` names, §4.1): section `8 + (t − 1)/48`, index
/// `(t − 1) % 48 + 1`.
pub fn tech_name(texts: &crate::ui::UiTexts, t: usize) -> Vec<u8> {
    if t == 0 {
        return Vec::new();
    }
    texts.get(17, (8 + (t - 1) / 48) as u16, ((t - 1) % 48 + 1) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_clip_to_the_radar() {
        assert_eq!(clip_line([500.0, 400.0], [700.0, 400.0], RADAR), Some(([522.0, 400.0], [621.0, 400.0])));
        assert_eq!(clip_line([500.0, 300.0], [700.0, 300.0], RADAR), None);
        let inside = ([530.0, 370.0], [600.0, 450.0]);
        assert_eq!(clip_line(inside.0, inside.1, RADAR), Some(inside));
    }

    #[test]
    fn bar_records_are_the_hack_layout() {
        // The speed buttons sit side by side from x 0, each selected/other pair at the same place.
        let mut x = 0;
        for i in 0..4 {
            assert_eq!(SPEED[i].dst, [x, 0]);
            assert_eq!(SPEED[i + 4].dst, SPEED[i].dst);
            assert_eq!(SPEED[i + 4].size, SPEED[i].size);
            x += -SPEED[i].size[0];
        }
        assert_eq!(x, 73);
        assert_eq!(BAR.dst[0] + -BAR.size[0], 600);
    }
}
