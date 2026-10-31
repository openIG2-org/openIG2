//! The battle screen's 2D HUD (re/screens/spacewar.md §5): the starmap's control panel `kezel.xvb` and its faders, the
//! battle radar (`RadarFrissit` ELF 0x685290, exe 0x56c890) in the panel's window, and the small info box
//! (`SpwC::kisinfokiir` ELF 0x6b4220, exe 0x577c00) that `StarmapGfx2D_3` draws while `AktKep` is 11.

use crate::font::{Align, Virtual};
use crate::render::{kind, Blend, Frame, Instance};
use crate::spacewar_screen::Shown;
use ig2_sim::spacebattle::Battle;
use ig2_sim::spacebattle_raw as raw;

/// The radar window (spec §5.3): centre (321, 421), 94 × 67 pixels for the arena's 384 × 384 units.
const RADAR_CENTRE: [f32; 2] = [321.0, 421.0];
const RADAR_RECT: [f32; 4] = [274.0, 388.0, 368.0, 455.0];
const ARENA: f32 = 384.0;

/// What `infoszamol` (ELF 0x6a4080, exe 0x574040) left for the info box: the snapshot of the selection's counters
/// and statistics (exe slot offsets in brackets, spec §5.2).
#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    /// Selected ships `[+0x1fc8]`, their damage `[+0x1fd4]` and hit points `[+0x1fd8]`, value `[+0x1fdc]`, rank
    /// sum `[+0x1fe0]`.
    pub ships: i32,
    pub damage: i32,
    pub max_hp: i32,
    pub value: i32,
    pub rank_sum: i32,
    /// Selected units `[+0x1fe8]`, of them ships `[+0x1fec]` and guns `[+0x1ff0]`, the last one `[+0x1ff4]` and its
    /// kind `[+0x1ff8]` (1 ship, 2 gun), own `[+0x1ffc]`.
    pub total: i32,
    pub sel_ships: i32,
    pub sel_guns: i32,
    pub last: i32,
    pub last_kind: i32,
    pub own: i32,
    /// The selected ships' shield used and shield (`+0x2558`, `+0x255c`).
    pub shield_used: i32,
    pub shield: i32,
    /// `infoszamol`'s battle overview (exe 0xE72D94..0xE72DB4, spec §7.4): ships, fighters and firepower of the
    /// viewer's side, its enemies and the others.
    pub sides: [[i32; 3]; 3],
}

impl Stats {
    pub fn from_slot(obj: &[u8]) -> Self {
        let i = |o: usize| raw::i32_at(obj, o);
        Self {
            ships: i(0x1fc8),
            damage: i(0x1fd4),
            max_hp: i(0x1fd8),
            value: i(0x1fdc),
            rank_sum: i(0x1fe0),
            total: i(0x1fe8),
            sel_ships: i(0x1fec),
            sel_guns: i(0x1ff0),
            last: i(0x1ff4),
            last_kind: i(0x1ff8),
            own: i(0x1ffc),
            shield_used: i(0x1fcc),
            shield: i(0x1fd0),
            sides: [[0; 3]; 3],
        }
    }
}

/// `sprintf` with `%d` and `%s`, each with an optional width (`%4d`: right-aligned in 4 cells, `%-4d` left).
fn format(fmt: &[u8], nums: &[i64], strs: &[&[u8]]) -> Vec<u8> {
    let (mut out, mut i, mut n, mut s) = (Vec::new(), 0, nums.iter(), strs.iter());
    while i < fmt.len() {
        if fmt[i] != b'%' {
            out.push(fmt[i]);
            i += 1;
            continue;
        }
        let mut j = i + 1;
        let left = fmt.get(j) == Some(&b'-');
        if left {
            j += 1;
        }
        let w0 = j;
        while fmt.get(j).is_some_and(|c| c.is_ascii_digit()) {
            j += 1;
        }
        let width: usize = std::str::from_utf8(&fmt[w0..j]).ok().and_then(|w| w.parse().ok()).unwrap_or(0);
        let field: Vec<u8> = match fmt.get(j) {
            Some(b'd') => n.next().copied().unwrap_or(0).to_string().into_bytes(),
            Some(b's') => s.next().copied().unwrap_or(b"").to_vec(),
            Some(b'%') => b"%".to_vec(),
            _ => {
                out.push(b'%');
                i += 1;
                continue;
            }
        };
        let pad = width.saturating_sub(field.len());
        if !left {
            out.extend(std::iter::repeat_n(b' ', pad));
        }
        out.extend(&field);
        if left {
            out.extend(std::iter::repeat_n(b' ', pad));
        }
        i = j + 1;
    }
    out
}

/// The rank text (`GetString(0xb, 1, 6..10)` by the average rank: below 4, 0x10, 0x1e, 0x32, else; "-" below 0).
fn rank(hud: &crate::ui::Hud, avg: i32) -> Vec<u8> {
    let k = match avg {
        a if a < 0 => return b"-".to_vec(),
        a if a < 4 => 6,
        a if a < 0x10 => 7,
        a if a < 0x1e => 8,
        a if a < 0x32 => 9,
        _ => 10,
    };
    hud.texts.get(0xb, 1, k)
}

/// The panel, the radar and the small info box.
#[allow(clippy::too_many_arguments)]
pub fn draw(f: &mut Frame, hud: &crate::ui::Hud, console: Option<&crate::starmap_hud::Console>, v: &Virtual, d: &Shown) {
    let fade = d.fade;
    let Some(font) = &hud.font else { return };
    let col = |i: usize| hud.palette.get(i).copied().unwrap_or([1.0; 3]);
    // The box select (`SpaceWarMouse`, spec §7.1.2): four `Line(.., 7, 0xff)`, one screen pixel of colour 7 [obs: s4
    // t655, (50, 255, 50) a pixel wide at 800 × 600].
    if let Some([x0, y0, x1, y1]) = d.boxed {
        let c = col(7);
        let ([ax, ay], [bx, by]) = (v.px(x0, y0).map(f32::floor), v.px(x1, y1).map(f32::floor));
        let (lx, hx, ly, hy) = (ax.min(bx), ax.max(bx) + 1.0, ay.min(by), ay.max(by) + 1.0);
        for [x, y, w, h] in [[lx, ly, hx - lx, 1.0], [lx, hy - 1.0, hx - lx, 1.0], [lx, ly, 1.0, hy - ly], [hx - 1.0, ly, 1.0, hy - ly]] {
            f.push(hud.white, Blend::GammaAlpha, Instance::new(kind::SPRITE, [x + w / 2.0, y + h / 2.0], [w / 2.0, h / 2.0], [c[0], c[1], c[2], 1.0]));
        }
    }
    if let Some(c) = console {
        c.draw(f, v);
    }
    crate::starmap_hud::fader(f, hud.white, v, [5.0, 423.0, 162.0, 52.0]);
    radar(f, hud, v, d);

    // `kisinfokiir(50, 424, 0)`, case A (spec §5.2): ships selected.
    let (x, y) = (50.0, 424.0);
    let st = d.stats;
    let b = d.battle;
    let s = |k: u16| hud.texts.get(0xb, 1, k);
    let mut text = |t: &[u8], dx: f32, dy: f32, size: usize, c: usize| {
        let rgb = col(c).map(|x| x * fade);
        font.text(f, v, t, x + dx, y + dy, 0x13, size, Align::Left, rgb, None);
    };
    let alpha = (fade * 255.0) as i32 as f32 / 255.0;
    if st.total != 0 && st.sel_ships != 0 {
        // One unit: its plan's name and hull tech (`Tervek[owner][design]`, `Talalmanyok[+0x10]`); more: the count.
        if st.total == 1 {
            text(&d.title, 0.0, 3.0, 2, 3);
            if d.title_tech >= 0 && st.last >= 0 {
                text(&hud.tech_name(d.title_tech as usize), 0.0, 12.0, 2, 0x21);
            }
        } else {
            text(&format(&s(5), &[(st.sel_ships + st.sel_guns) as i64], &[]), 0.0, 3.0, 2, 3);
        }
        let owner = match st.last_kind {
            2 => b.planet_owner,
            1 => b.units.get(st.last.max(0) as usize).map_or(1, |u| u.owner),
            _ => 1,
        };
        let name = hud.empire_names.get(owner.max(0) as usize).cloned().unwrap_or_default();
        text(&name, 0.0, 23.0, 1, 0x21);
        if st.ships != 0 && st.max_hp != 0 {
            // `kisinfokiir` tests `0 <= (f32)sum / (f32)n` first: a sum still −1 (no ranked ship) shows "-".
            let r = if st.own == 0 { Vec::new() } else if (st.rank_sum as f32 / st.ships as f32) < 0.0 { b"-".to_vec() } else { rank(hud, st.rank_sum / st.ships) };
            text(&format(&s(0xb), &[st.value as i64], &[&r]), 0.0, 32.0, 1, 0x21);
            text(&format(&s(0xc), &[0], &[]), 0.0, 41.0, 1, 0x21);
            crate::colony_ui::hp_stripe_alpha(f, hud.white, v, &hud.palette, st.max_hp - st.damage, st.max_hp, x + 44.0, y + 41.0, 66.0, (fade * 255.0) as i32);
        }
    } else if st.total != 0 && st.sel_guns != 0 && st.last_kind == 2 && st.last != -1 {
        // Case B, planet guns only: the gun building's tech, or the count.
        if st.total == 1 && d.title_tech >= 0 {
            text(&hud.tech_name(d.title_tech as usize), 0.0, 3.0, 2, 3);
        } else {
            text(&format(&s(0xd), &[st.total as i64], &[]), 0.0, 3.0, 2, 3);
        }
        let name = hud.empire_names.get(b.planet_owner.max(0) as usize).cloned().unwrap_or_default();
        text(&name, 0.0, 23.0, 1, 0x21);
        if st.ships != 0 && st.max_hp != 0 {
            text(&format(&s(0xe), &[st.value as i64], &[]), 0.0, 32.0, 1, 0x21);
            text(&format(&s(0xf), &[0], &[]), 0.0, 41.0, 1, 0x21);
            crate::colony_ui::hp_stripe_alpha(f, hud.white, v, &hud.palette, st.max_hp - st.damage, st.max_hp, x + 44.0, y + 41.0, 66.0, (fade * 255.0) as i32);
        }
    } else if st.total == 0 {
        // Case C, the battle overview: firepower (with K or M above 9999 and 999000), ships and fighters of the
        // viewer's side, its enemies and (when they have any) the others; then the fighters' stance slider.
        let [ships, fighters, fire] = st.sides;
        let others = fire[2] != 0 || ships[2] != 0 || fighters[2] != 0;
        let top = fire.iter().copied().max().unwrap_or(0);
        let (div, unit): (i32, &[u8]) = if top > 0xf3e58 { (1_000_000, b"M") } else if top > 9999 { (1000, b"K") } else { (1, b"") };
        text(&s(0x10), 0.0, 3.0, 2, 3);
        let cols = [(0x2a, 0x29), (0x42, 0x2a), (0x5a, 0x21)];
        let row = |f: &mut Frame, label: Vec<u8>, dy: f32, vals: [i32; 3], with_unit: bool| {
            font.text(f, v, &label, x, y + dy, 0x13, 1, Align::Left, col(0x21).map(|z| z * fade), None);
            for (k, &(dx, c)) in cols.iter().enumerate() {
                if k == 2 && !others {
                    continue;
                }
                let t = if with_unit { format(b"%d%s", &[(vals[k] / div) as i64], &[unit]) } else { vals[k].to_string().into_bytes() };
                let rgb = col(c).map(|z| z * fade);
                font.text(f, v, &t, x + dx as f32, y + dy, 4, 1, Align::Right, rgb, None);
            }
        };
        row(f, format(&s(0x11), &[0], &[]), 14.0, fire, true);
        row(f, s(0x12), 23.0, ships, false);
        row(f, s(0x13), 32.0, fighters, false);
        // `DDFillToScreen3dtRGB(x, y + 0x2a, 0x71, 3, ..)`: red rises and green falls with the stance; the marker a
        // 3 × 7 fill of colour 2 with a 1 × 5 of colour 0 in it.
        let st_v = d.stance.clamp(0, 100);
        let r = (st_v * 0x7f / 100) as f32 / 255.0;
        let g = (0x7f - st_v * 0x7f / 100) as f32 / 255.0;
        let fill = |f: &mut Frame, x0: f32, y0: f32, w: f32, h: f32, c: [f32; 3]| {
            let p = v.px(x0 + w / 2.0, y0 + h / 2.0);
            f.push(hud.white, Blend::GammaAlpha, Instance::new(kind::SPRITE, p, [w * v.k / 2.0, h * v.k / 2.0], [c[0], c[1], c[2], alpha]));
        };
        fill(f, x, y + 42.0, 113.0, 3.0, [r, g, 0.0]);
        let mx = x + (st_v * 0x71 / 100) as f32;
        fill(f, mx - 1.0, y + 40.0, 3.0, 7.0, col(2));
        fill(f, mx, y + 41.0, 1.0, 5.0, col(0));
        // The label last, over the bar (exe 0x577c00) [obs spacewar_info t500: the exe's label is over the bar].
        font.text(f, v, &s(0x14), x, y + 41.0, 0x13, 1, Align::Left, col(0x21).map(|z| z * fade), None);
    }
}

/// `RadarFrissit` (spec §5.3): the dark window, the view's outline on the grid plane, a pixel per unit (green for the
/// viewer's, red for the rest), turned by the snapped view yaw `+0xc7c`; fully open (`spwradarcounter` 100).
fn radar(f: &mut Frame, hud: &crate::ui::Hud, v: &Virtual, d: &Shown) {
    let (eye, target) = (d.eye, d.target);
    let th = d.snap_yaw;
    // `spwradarcounter`'s opening: the outline flies in from 7× and the dots from 10× (about the screen's origin).
    let g = crate::spacewar_screen::gorbe(d.radar as f32 / 100.0);
    let (kv, kd) = ((1.0 - g) * 6.0 + 1.0, (1.0 - g) * 9.0 + 1.0);
    let (s, c) = ((th as f64).sin() as f32, (th as f64).cos() as f32);
    let white = hud.white;
    let inside = |q: [f32; 2]| q[0] > RADAR_RECT[0] && q[0] < RADAR_RECT[2] && q[1] > RADAR_RECT[1] && q[1] < RADAR_RECT[3];
    // The view's outline: the screen corners' rays meet the grid plane (h = 18).
    let m = crate::spacewar_screen::look_matrix(eye, target);
    let corner = |sx: f32, sy: f32| -> Option<[f32; 2]> {
        let e = [(sx - 320.0) / 600.0, (sy - 250.0) / 600.0, -1.0];
        let dir: [f32; 3] = std::array::from_fn(|j| m[0][j] * e[0] + m[1][j] * e[1] + m[2][j] * e[2]);
        if dir[0].abs() < 1e-9 {
            return None;
        }
        let t = (18.0 - eye[0]) / dir[0];
        (t > 0.0).then(|| {
            let (py, pz) = (eye[1] + t * dir[1], eye[2] + t * dir[2]);
            let (a, b) = (-py, pz);
            [((c * a + s * b) * 94.0 / ARENA + RADAR_CENTRE[0]) * kv, ((c * b - s * a) * 94.0 / ARENA + RADAR_CENTRE[1]) * kv]
        })
    };
    // The window: colour word 0x40000020 (its 0x20 blue at 0x40/255 opacity [obs: grabs 3 and 4, the exe's window is
    // (4, 9, 12), which 25 % fits and 75 % doesn't]), over the starmap's own
    // dark window (`StarmapGfx2D_1`'s `FadeScreen(271, 385, 98, 71, black, 0x40)`, still drawn on this screen [obs: the
    // exe's window is near black]).
    f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(271.0 + 49.0, 385.0 + 35.5), [49.0 * v.k, 35.5 * v.k], crate::kit::fader::fade_color([0.0; 3], 1.0 - 0x40 as f32 / 255.0)));
    let [x0, y0, x1, y1] = RADAR_RECT;
    f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px((x0 + x1) / 2.0, (y0 + y1) / 2.0), [(x1 - x0) * v.k / 2.0, (y1 - y0) * v.k / 2.0], [0.0, 0.0, 0x20 as f32 / 255.0, 0x40 as f32 / 255.0]));
    // The outline: `LineClip`'s 1 px `line2d` goes over the queued window (`line3`) [obs grab4 t800: a full white
    // line, not dimmed].
    let cs = [corner(0.0, 0.0), corner(640.0, 0.0), corner(0.0, 480.0), corner(640.0, 480.0)];
    // `DDColor[2]` at alpha 0xff: white [obs fx8b: (255, 255, 255), one device pixel wide, stepped per pixel].
    for (a, b) in [(0, 1), (0, 2), (2, 3), (1, 3)] {
        if let (Some(a), Some(b)) = (cs[a], cs[b]) {
            if let Some((a, b)) = crate::hud::clip_line(a, b, [274.0, 388.0, 367.0, 454.0]) {
                let (p, q) = (v.px(a[0], a[1]), v.px(b[0], b[1]));
                let n = (q[0] - p[0]).abs().max((q[1] - p[1]).abs()).ceil().max(1.0) as usize;
                for i in 0..=n {
                    let t = i as f32 / n as f32;
                    let c = [(p[0] + (q[0] - p[0]) * t).floor() + 0.5, (p[1] + (q[1] - p[1]) * t).floor() + 0.5];
                    f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, c, [0.5; 2], [1.0; 4]));
                }
            }
        }
    }
    let b: &Battle = d.battle;
    for u in b.units.iter().filter(|u| u.alive && (u.owner == b.human || !u.cloaked)) {
        let (y, z) = (u.pos.y, u.pos.z);
        let q = [(-c * y + s * z) * 94.0 * kd / ARENA + RADAR_CENTRE[0], (c * z + s * y) * 67.0 * kd / ARENA + RADAR_CENTRE[1]];
        if !inside(q) {
            continue;
        }
        let rgb = if u.owner == b.human { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let p = [q[0].trunc(), q[1].trunc()];
        // A 2 × 2 device-pixel point at 800 × 600 [obs fx8b].
        let c = v.px(p[0], p[1]).map(|z| z.floor() + 1.0);
        f.push(white, Blend::GammaAdditive, Instance::new(kind::SPRITE, c, [v.k.max(1.0) * 0.8; 2], [rgb[0], rgb[1], rgb[2], 1.0]));
    }
}

/// The hot-spots' tooltips (`SpaceWar_MouseInit`, spec §4.2, §7.4), by id: the radar 1, the slider 0x19 (on while
/// nothing is selected), the small box 0x24, the camera cluster 0x28..0x2d (`GetString(0xb, 2, k)`), the column's
/// shown buttons 3..0xc: the weapon groups' names `GetString(0xb, 3, i + 1)`, in the formation phase the formations'
/// `GetString(0xb, 3, i + 11)`.
pub fn tooltips(hud: &crate::ui::Hud, d: &Shown, formation: bool) -> Vec<Vec<u8>> {
    let mut t = vec![Vec::new(); 0x2f];
    let s = |k: u16| hud.texts.get(0xb, 2, k);
    t[1] = s(1);
    if d.stats.total == 0 {
        t[0x19] = s(4);
    }
    t[0x24] = s(5);
    for (i, k) in (0x28..=0x2d).zip(6..=11) {
        t[i] = s(k);
    }
    for i in 0..10 {
        if d.slots[i].counter != 0 {
            t[3 + i] = hud.texts.get(0xb, 3, if formation { i as u16 + 11 } else { i as u16 + 1 });
        }
    }
    t
}

/// The cursor (`UpdateFrame` exe 0x4e3e90, spec §7.4): `MOUSE.BMP`'s 20 × 20 cell of column `kind`, row `(MouseFazis/5) %
/// 40` (`MouseFazis` +1 a UI tick), at the pointer less the hot-spot offset (types 0 and 1: 0; the rest 10).
pub fn cursor(f: &mut Frame, v: &Virtual, sheet: Option<(crate::render::TexId, [f32; 2])>, kind: i32, ticks: u64, at: [f32; 2]) {
    let Some((tex, [tw, th])) = sheet else { return };
    if kind < 0 {
        return;
    }
    let hot = if kind < 2 { 0.0 } else { 10.0 };
    let row = ((ticks / 5) % 40) as f32;
    let (x, y) = (at[0] as i32 as f32 - hot, at[1] as i32 as f32 - hot);
    let mut i = Instance::new(kind::SPRITE, v.px(x + 10.0, y + 10.0), [10.0 * v.k, 10.0 * v.k], [1.0; 4]);
    i.uv = [kind as f32 * 20.0 / tw, row * 20.0 / th, 20.0 / tw, 20.0 / th];
    f.push(tex, Blend::GammaAlpha, i);
}

/// The big info box (`nagyinfokiir` ELF 0x6b5e50 / exe 0x57c9c0, spec §5.2, §7.4) while the info view is on and
/// something is selected: its box (`nagyinfofader`) sliding in from the right with `f`, one live ship's plan, hull,
/// owner and fleet, then the hull, shield, value and rank rows, and a single own ship's weapons.
pub fn big_box(f: &mut Frame, hud: &crate::ui::Hud, v: &Virtual, d: &Shown, big: &BigInfo) {
    let st = d.stats;
    if st.total == 0 {
        return;
    }
    let Some(font) = &hud.font else { return };
    let fr = big.f.clamp(0.0, 1.0);
    let x0 = (320.0 + (1.0 - fr) * 350.0) as i32 as f32;
    crate::kit::fader::fader(f, hud.white, v, [((1.0 - fr) * 350.0 + 319.0) as i32 as f32, 33.0, 302.0, 326.0], [0.0, 15.0, 20.0], 60.0, [0.0, 10.0, 15.0], 30.0);
    let col = |i: usize| hud.palette.get(i).copied().unwrap_or([1.0; 3]);
    let s = |k: u16| hud.texts.get(0xb, 1, k);
    let label = |f: &mut Frame, k: u16, y: f32, len: usize| font.text(f, v, &s(k), x0 + 10.0, y, len, 3, Align::Left, col(1), None);
    let value = |f: &mut Frame, t: &[u8], y: f32| font.text(f, v, t, x0 + 98.0, y, 0x1a, 3, Align::Left, col(0x21), None);
    let y0 = 34.0;
    let mut y = y0 + 10.0;
    if let Some(ship) = &big.ship {
        label(f, 0x15, y0 + 10.0, 8);
        value(f, &ship.plan, y0 + 10.0);
        label(f, 0x16, y0 + 26.0, 0xb);
        value(f, &hud.tech_name(ship.tech.max(0) as usize), y0 + 26.0);
        label(f, 0x17, y0 + 42.0, 0xb);
        value(f, &hud.empire_names.get(ship.owner.max(0) as usize).cloned().unwrap_or_default(), y0 + 42.0);
        label(f, 0x18, y0 + 58.0, 0xb);
        if let Some(n) = &ship.fleet {
            value(f, n, y0 + 58.0);
        }
        y = y0 + 74.0;
    }
    label(f, 0x19, y, 0xb);
    if st.max_hp != 0 {
        value(f, &format(b"%d%%, %dhp", &[(st.damage * 100 / st.max_hp) as i64, ((st.max_hp - st.damage) / 15) as i64], &[]), y);
    }
    y += 16.0;
    label(f, 0x1a, y, 0xb);
    if st.ships != 0 {
        let t = if st.shield == 0 { b"-".to_vec() } else { format(b"%d%%", &[(100 - st.shield_used * 100 / st.shield) as i64], &[]) };
        value(f, &t, y);
    }
    y += 16.0;
    label(f, 0x1b, y, 0xb);
    value(f, &format(b"%d", &[st.value as i64], &[]), y);
    y += 16.0;
    label(f, 0x1c, y, 0xb);
    if st.own != 0 && st.ships != 0 {
        value(f, &rank(hud, st.rank_sum / st.ships), y);
    }
    y += 32.0;
    for (n, (tech, count)) in big.weapons.iter().take(10).enumerate() {
        let yy = y + n as f32 * 16.0;
        font.text(f, v, &hud.tech_name(*tech as usize), x0 + 10.0, yy, 0x1e, 3, Align::Left, col(1), None);
        font.int(f, v, *count as i64, x0 + 260.0, yy, 3, 3, Align::Right, col(0x21));
    }
}

/// What the big box shows beyond the statistics, from `infoszamol`'s snapshot: its slide factor and, for one live ship,
/// the plan's name, hull tech, owner (the fleet's, else the planet's), the fleet's name and the plan's weapons.
#[derive(Debug, Clone, Default)]
pub struct BigInfo {
    pub f: f32,
    pub ship: Option<BigShip>,
    pub weapons: Vec<(i32, i32)>,
}

#[derive(Debug, Clone, Default)]
pub struct BigShip {
    pub plan: Vec<u8>,
    pub tech: i32,
    pub owner: i32,
    pub fleet: Option<Vec<u8>>,
}

/// `screenfrissites`' notices (exe 0x57f800, spec §3 step 12): at (200, y) in 0x46 cells, size 2, right-aligned in
/// them (flags 6), colour 2, `KiirStrSzinSzorzo = (int)(spacewarelfade·100)`: own ships retreating (`+0x2d90`,
/// `GetString(0xb, 1, 2)`) at y 30, an AI's (`+0x2d91`, `(0xb, 1, 3)`) on the next line (+10), and in the formation
/// phase (`+0x2d85`) `(0xb, 1, 4)` on the next, its multiplier scaled by `(slot-0 counter·100/50)/100` (ints), so it
/// slides in and out with the weapon column [obs spacewar_info t500: the exe's caption right-aligned at x 620].
pub fn notices(f: &mut Frame, hud: &crate::ui::Hud, v: &Virtual, [own, ai, formation]: [bool; 3], slot0: i32, fade: f32) {
    let Some(font) = &hud.font else { return };
    let base = (fade * 100.0) as i32;
    let colour = |k: i32| hud.palette.get(2).copied().unwrap_or([1.0; 3]).map(|x| x * k as f32 / 100.0);
    let line = |f: &mut Frame, line: u16, y: f32, k: i32| {
        let t = hud.texts.get(0xb, 1, line);
        let t = t.split(|&c| c == 0).next().unwrap_or(&[]);
        font.text(f, v, t, 200.0, y, 0x46, 2, Align::Right, colour(k), None);
    };
    let mut y = 30.0;
    if own {
        line(f, 2, y, base);
        y = 40.0;
    }
    if ai {
        line(f, 3, y, base);
        y += 10.0;
    }
    if formation {
        line(f, 4, y, base * (slot0 * 100 / 0x32) / 100);
    }
}

/// Ours (ADR 0023): an MP formation phase's side that has started waits for the others; below the notices, in their
/// style.
pub fn waiting(f: &mut Frame, hud: &crate::ui::Hud, v: &Virtual, fade: f32) {
    let Some(font) = &hud.font else { return };
    let c = hud.palette.get(2).copied().unwrap_or([1.0; 3]).map(|x| x * fade);
    font.text(f, v, b"Waiting for the other players", 200.0, 60.0, 0x46, 2, Align::Right, c, None);
}

/// The MP versus banner (`screenfrissites`, spec §7.4): `"%s (%s) vs. %s (%s)"` at (200, 30), 0x46 cells, size 2,
/// centred, colour 2, for 9 s from the opening zoom's start (`spwckezdotimer`), fading over its last 0.5 s.
pub fn banner(f: &mut Frame, hud: &crate::ui::Hud, v: &Virtual, text: &[u8], ms: u32, fade: f32) {
    let Some(font) = &hud.font else { return };
    if ms >= 9000 {
        return;
    }
    // `KiirStrSzinSzorzo = (int)(spacewarelfade·100)`, then `· min(100, (9000 − ms)/5) / 100` (ints).
    let k = ((fade * 100.0) as i32 * ((9000 - ms) / 5).min(100) as i32 / 100) as f32 / 100.0;
    let c = hud.palette.get(2).copied().unwrap_or([1.0; 3]).map(|x| x * k);
    font.text(f, v, text, 200.0, 30.0, 0x46, 2, Align::Centre, c, None);
}

/// `RetreatWindowXSize` (exe 0x926B40): the prompt's length + 2 cells.
fn retreat_cells(hud: &crate::ui::Hud) -> usize {
    let s = hud.texts.get(0xb, 1, 1);
    s.split(|&c| c == 0).next().map_or(0, |t| t.len()) + 2
}

/// `WriteRetreatWindow`'s hover (spec §5.4): 1 on the "Yes" row, 2 on "No", else 0.
pub fn retreat_hover(hud: &crate::ui::Hud, m: [f32; 2]) -> u8 {
    let w = retreat_cells(hud) as f32;
    let (x, y) = (m[0] as i32 as f32, m[1] as i32 as f32);
    if !(401.0..6.0 * w + 400.0).contains(&x) {
        0
    } else if (394.0..=406.0).contains(&y) {
        1
    } else if (408.0..=420.0).contains(&y) {
        2
    } else {
        0
    }
}

/// The retreat window (spec §5.4): `ShowRetreatWindow`'s box at (400, 380), `6·w` × 44, and `WriteRetreatWindow`'s
/// prompt, "Yes" and "No" (colour 2 hovered, 1 otherwise).
pub fn retreat_window(f: &mut Frame, hud: &crate::ui::Hud, v: &Virtual, m: [f32; 2]) {
    let Some(font) = &hud.font else { return };
    let w = retreat_cells(hud);
    crate::kit::fader::fader(f, hud.white, v, [400.0, 380.0, w as f32 * 6.0, 44.0], [0.0, 20.0, 30.0], 60.0, [0.0, 15.0, 25.0], 30.0);
    let col = |i: usize| hud.palette.get(i).copied().unwrap_or([1.0; 3]);
    let h = retreat_hover(hud, m);
    font.text(f, v, &hud.texts.get(0xb, 1, 1), 400.0, 383.0, w, 2, Align::Centre, col(2), None);
    font.text(f, v, &hud.texts.get(3, 4, 2), 400.0, 395.0, w, 3, Align::Centre, col(if h == 1 { 2 } else { 1 }), None);
    font.text(f, v, &hud.texts.get(3, 4, 3), 400.0, 409.0, w, 3, Align::Centre, col(if h == 2 { 2 } else { 1 }), None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_takes_widths() {
        assert_eq!(format(b"TE: %4d %s", &[37], &[b"X"]), b"TE:   37 X");
        assert_eq!(format(b"%d%%, %dhp", &[50, 7], &[]), b"50%, 7hp");
        assert_eq!(format(b"[%-3d]", &[5], &[]), b"[5  ]");
    }
}
