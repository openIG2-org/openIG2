//! The look the enabled mods switch on (ADR 0088): their `visuals.toml` files laid over the player's graphics
//! settings. Without such a mod nothing here changes a pixel.

use std::sync::RwLock;

use ig2_formats::visuals::{Visuals, VISUALS_FILE};
use ig2_formats::Install;

use crate::render3d::Lighting;

static MODS: RwLock<Option<Visuals>> = RwLock::new(None);

/// The enabled mods' files merged, lowest priority first; a file that doesn't read is named and skipped.
pub fn of(install: &Install) -> Visuals {
    let mut all = Visuals::default();
    for (m, bytes) in install.visual_files().unwrap_or_default() {
        match Visuals::parse(&String::from_utf8_lossy(&bytes), &format!("mod {m}: {VISUALS_FILE}")) {
            Ok(v) => all.merge(&v),
            Err(e) => eprintln!("ig2-client: {e} (skipped)"),
        }
    }
    all
}

/// Take the install's mods' look and switch it on over the settings in effect.
pub fn load(install: &Install) {
    crate::battle_fx::load_mods(install);
    *MODS.write().unwrap() = Some(of(install));
    apply(crate::render3d::lighting(), crate::battle_fx::enhanced(), crate::render::smooth_glows());
}

fn mods() -> Visuals {
    MODS.read().unwrap().unwrap_or_default()
}

/// Set the player's graphics switches; what a mod switches on stays on.
pub fn apply(l: Lighting, battle: bool, smooth_glows: bool) {
    let m = mods();
    crate::render3d::set_lighting(Lighting { msaa: l.msaa || m.anti_aliasing, per_pixel: l.per_pixel || m.per_pixel, soft_shadows: l.soft_shadows || m.soft_shadows, bloom: l.bloom || m.bloom });
    // A mod's effects.toml (ADR 0088 §10) needs the enhanced layers.
    crate::battle_fx::set_enhanced(battle || m.battle || crate::battle_fx::recipes().is_some());
    crate::render::set_smooth_glows(smooth_glows || m.smooth_glows);
}

/// The whole-scene glow and tone curve (bright threshold, glow strength), when a mod switches HDR on.
pub fn hdr() -> Option<(f32, f32)> {
    let m = mods();
    // shortcut: no float scene in the browser build (WebGL2 may lack float render targets), upgrade with a format check.
    (m.hdr && !cfg!(target_arch = "wasm32")).then(|| (m.bloom_threshold.unwrap_or(0.8), m.bloom_strength.unwrap_or(0.6)))
}

/// The starmap planets' halo: (radius in planet radii, colour × strength), when a mod asks for one.
pub fn atmosphere() -> Option<(f32, [f32; 3])> {
    let m = mods();
    let strength = m.atmosphere.filter(|&s| s > 0.0)?;
    let c = m.atmosphere_color.unwrap_or([0x7f, 0xb2, 0xff]);
    Some((m.atmosphere_size.unwrap_or(1.3), c.map(|x| x as f32 / 255.0 * strength)))
}
