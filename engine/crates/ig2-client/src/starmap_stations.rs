//! The long info's space stations round the player's planet (re/screens/starmap.md §15.2):
//! `StarmapLoadUrallomasok`'s three models per slot and `StarmapForgatEsMozgatUrallomasok`'s ring, drawn camera-fixed
//! over the starmap the way the battle screen draws its column.

use crate::spacewar_screen::{Fixed, Mesh};
use ig2_sim::bytes::{cstr, i16_at, i32_at};
use ig2_sim::save::SaveState;

/// The models by the design's hull tech 0x4d..=0x4f (`vector/talalmanyok/`).
const MODELS: [&str; 3] = ["ur_kurbazis.xvb", "ur_urbazis.xvb", "ur_nehezur.xvb"];
/// The whole screen (virtual): the ring sits round the pinned planet.
const RECT: [f32; 4] = [0.0, 0.0, 640.0, 480.0];

/// One station on the ring: its model (0..=2), camera-space point and transparency (0 = opaque).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub model: usize,
    pub at: [f32; 3],
    pub clear: f32,
}

/// The ring's radius factor `r` and the transparency `t` at camera zoom `zoom`: twice the radius and gone up to 580,
/// flying in to 1 by 600.
pub fn fly(zoom: f32) -> (f32, f32) {
    let r = if zoom > 580.0 { ((600.0 - zoom) as f64 / 20.0 + 1.0) as f32 } else { 2.0 };
    (r, (r - 1.0).clamp(0.0, 1.0))
}

/// Station `i` of `n` with model `model` at zoom `zoom` and `smkeringeto` = `kering` (f32 sums, f64 trigonometry as
/// the ELF does).
pub fn place(i: usize, n: usize, model: usize, zoom: f32, kering: f32) -> Placed {
    let (r, t) = fly(zoom);
    let a = ((i as f32 / n as f32 + kering / 4.0) as f64 + t as f64 / 10.0) * std::f64::consts::PI * 2.0;
    let x = (a.sin() * 150.0 * r as f64 - 180.0) as f32;
    let y = (a.cos() * 150.0 * r as f64 - 75.0) as f32;
    Placed { model, at: [x, y, -700.0], clear: t }
}

/// Station `i`'s design record on planet record `r`.
fn design<'a>(state: &'a SaveState, r: &[u8], i: usize) -> Option<&'a [u8]> {
    let d = i16_at(r, 0x100 + 2 * i).max(0) as usize;
    state.designs.get(r[0x20] as usize)?.get(d).map(|d| &d[..])
}

/// The player's planet `p`'s stations at zoom `zoom` (none on another empire's planet).
pub fn stations(state: &SaveState, p: usize, zoom: f32, kering: f32) -> Vec<Placed> {
    let r = crate::starmap_menus::planet(state, p);
    if r[0x20] as i32 != state.mem.i32(ig2_sim::save::va::EN) {
        return Vec::new();
    }
    let n = i32_at(&r, 0x114).clamp(0, 10) as usize;
    (0..n)
        .map(|i| {
            let hull = design(state, &r, i).map_or(0x4d, |d| i32_at(d, 0x48));
            place(i, n, (hull - 0x4d).clamp(0, 2) as usize, zoom, kering)
        })
        .collect()
}

/// Station `i`'s tooltip: its design's name (record `+0`).
pub fn name(state: &SaveState, p: usize, i: usize) -> Vec<u8> {
    let r = crate::starmap_menus::planet(state, p);
    design(state, &r, i).map(|d| cstr(&d[..0x14]).to_vec()).unwrap_or_default()
}

/// This drawn frame's stations of the long info: sets their hot-spots in `menus` and returns the models to draw.
pub fn update(menus: &mut crate::starmap_menus::Menus, state: &SaveState, zoom: f32, kering: f32) -> Vec<Placed> {
    let placed = menus.info.map_or_else(Vec::new, |i| stations(state, i.planet, zoom, kering));
    menus.stations = placed.iter().map(|s| spot(s.at, zoom)).collect();
    placed
}

/// The hot-spot of a station at camera-space `at`: the 80 × 80 box round its screen point (`TransformPoint`, fov 600,
/// centre (320, 250)), only while the zoom is past 580.
pub fn spot(at: [f32; 3], zoom: f32) -> Option<[i32; 4]> {
    if zoom <= 580.0 {
        return None;
    }
    let (sx, sy) = (600.0 * at[0] / -at[2] + 320.0, 600.0 * at[1] / -at[2] + 250.0);
    Some([(sx - 40.0) as i32, (sy - 40.0) as i32, 80, 80])
}

/// The models and the render target, loaded on first use.
#[derive(Default)]
pub struct Gfx {
    models: Option<[Option<Mesh>; 3]>,
    target: Option<crate::render::Target>,
}

impl Gfx {
    /// Draw `placed` over the scene; each model spins `spin` about its own y (`RotateObjr(0, 0.01, 0)` a drawn frame).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, f: &mut crate::render::Frame, r: &mut crate::render::Renderer, install: &ig2_formats::Install, placed: &[Placed], kering: f32, spin: f32, size: [f32; 2]) {
        if placed.iter().all(|s| s.clear >= 1.0) {
            return;
        }
        let models = self.models.get_or_insert_with(|| MODELS.map(|m| Mesh::load(r, install, "vector/talalmanyok/", m, "textures/talalmanyok/")));
        let items: Vec<Fixed> = placed
            .iter()
            .filter(|s| s.clear < 1.0)
            .filter_map(|s| {
                let mesh = models[s.model].as_ref()?;
                Some(Fixed { mesh, at: s.at, size: 0.04, rot: (std::f32::consts::FRAC_PI_2, spin, 0.0), opacity: 1.0 - s.clear, tex0: None, additive: false })
            })
            .collect();
        let tex = crate::spacewar_screen::fixed_texture(r, &mut self.target, &items, RECT, kering, true, 320.0);
        crate::spacewar_screen::put_fixed(f, &crate::font::Virtual::new(size), tex, RECT, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_flies_in_past_580() {
        assert_eq!(fly(400.0), (2.0, 1.0));
        assert_eq!(fly(580.0), (2.0, 1.0));
        let (r, t) = fly(599.0);
        assert!((r - 1.05).abs() < 1e-6 && (t - 0.05).abs() < 1e-6);
        assert_eq!(fly(600.0), (1.0, 0.0));
    }

    #[test]
    fn first_station_sits_below_the_ring_centre() {
        // a = (0 + 0 + 0.005)·2π at the pin: just past straight down (cos ≈ 1).
        let s = place(0, 1, 2, 599.0, 0.0);
        assert_eq!(s.model, 2);
        assert!((s.at[0] - (-180.0 + (0.01 * std::f64::consts::PI).sin() * 157.5) as f32).abs() < 1e-3);
        assert!((s.at[1] - (-75.0 + (0.01 * std::f64::consts::PI).cos() * 157.5) as f32).abs() < 1e-3);
        assert_eq!(s.at[2], -700.0);
        // Two stations stand opposite.
        let (a, b) = (place(0, 2, 0, 600.0, 0.0), place(1, 2, 0, 600.0, 0.0));
        assert!((a.at[0] + b.at[0] + 360.0).abs() < 1e-3 && (a.at[1] + b.at[1] + 150.0).abs() < 1e-3);
    }

    #[test]
    fn spot_is_the_box_round_the_screen_point() {
        assert_eq!(spot([-180.0, -75.0, -700.0], 580.0), None);
        // (−180, −75, −700) → (165.7, 185.7).
        assert_eq!(spot([-180.0, -75.0, -700.0], 599.0), Some([125, 145, 80, 80]));
    }
}
