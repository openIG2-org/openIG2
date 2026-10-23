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
    if st.total != 0 && st.sel_ships != 0 {
        text(&format(&s(5), &[(st.sel_ships + st.sel_guns) as i64], &[]), 0.0, 3.0, 2, 3);
        let owner = match st.last_kind {
            2 => b.planet_owner,
            1 => b.units.get(st.last.max(0) as usize).map_or(1, |u| u.owner),
            _ => 1,
        };
        let name = hud.empire_names.get(owner.max(0) as usize).cloned().unwrap_or_default();
        text(&name, 0.0, 23.0, 1, 0x21);
        if st.ships != 0 && st.max_hp != 0 {
            let r = if st.own == 0 { Vec::new() } else { rank(hud, st.rank_sum / st.ships) };
            text(&format(&s(0xb), &[st.value as i64], &[&r]), 0.0, 32.0, 1, 0x21);
            text(&format(&s(0xc), &[0], &[]), 0.0, 41.0, 1, 0x21);
            crate::colony_ui::hp_stripe(f, hud.white, v, &hud.palette, st.max_hp - st.damage, st.max_hp, x + 44.0, y + 41.0, 66.0);
        }
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
    let cs = [corner(0.0, 0.0), corner(640.0, 0.0), corner(0.0, 480.0), corner(640.0, 480.0)];
    let lc = hud.palette.get(2).copied().unwrap_or([1.0; 3]);
    for (a, b) in [(0, 1), (0, 2), (2, 3), (1, 3)] {
        if let (Some(a), Some(b)) = (cs[a], cs[b]) {
            if let Some((a, b)) = crate::hud::clip_line(a, b, [274.0, 388.0, 367.0, 454.0]) {
                f.push(white, Blend::GammaAlpha, Instance::line(v.px(a[0], a[1]), v.px(b[0], b[1]), 2.0 * v.k, [lc[0], lc[1], lc[2], 1.0]));
            }
        }
    }
    // The window: colour word 0x40000020 (transparency 0x40; its 0x20 blue [obs: navy, not red]), over the starmap's own
    // dark window (`StarmapGfx2D_1`'s `FadeScreen(271, 385, 98, 71, black, 0x40)`, still drawn on this screen [obs: the
    // exe's window is near black]).
    f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px(271.0 + 49.0, 385.0 + 35.5), [49.0 * v.k, 35.5 * v.k], crate::kit::fader::fade_color([0.0; 3], 1.0 - 0x40 as f32 / 255.0)));
    let [x0, y0, x1, y1] = RADAR_RECT;
    f.push(white, Blend::GammaAlpha, Instance::new(kind::SPRITE, v.px((x0 + x1) / 2.0, (y0 + y1) / 2.0), [(x1 - x0) * v.k / 2.0, (y1 - y0) * v.k / 2.0], [0.0, 0.0, 0x20 as f32 / 255.0, 1.0 - 0x40 as f32 / 255.0]));
    let b: &Battle = d.battle;
    for u in b.units.iter().filter(|u| u.alive && (u.owner == b.human || !u.cloaked)) {
        let (y, z) = (u.pos.y, u.pos.z);
        let q = [(-c * y + s * z) * 94.0 * kd / ARENA + RADAR_CENTRE[0], (c * z + s * y) * 67.0 * kd / ARENA + RADAR_CENTRE[1]];
        if !inside(q) {
            continue;
        }
        let rgb = if u.owner == b.human { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let p = [q[0].trunc(), q[1].trunc()];
        f.push(white, Blend::GammaAdditive, Instance::new(kind::SPRITE, v.px(p[0] + 0.5, p[1] + 0.5), [v.k / 2.0; 2], [rgb[0], rgb[1], rgb[2], 1.0]));
    }
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
