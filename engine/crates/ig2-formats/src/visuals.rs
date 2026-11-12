//! A mod's `visuals.toml` (ADR 0088): the look a mod switches on. Display only: never part of the game's state, the
//! save or the multiplayer check.
//!
//! ```toml
//! [lighting]
//! anti_aliasing = true
//! per_pixel = true
//! soft_shadows = true
//! bloom = true
//! hdr = true                # the scene in a floating-point picture: glow over all of it, tone mapped
//! bloom_threshold = 0.8     # where the glow and the tone curve start, 0..1
//! bloom_strength = 0.6      # how much glow is added, 0..2
//! battle_shadows = true    # the battle's ships shadow one another and the planet
//! blast_light = true       # blasts and shots light the hulls near them
//!
//! [effects]
//! battle = true
//! smooth_glows = true
//!
//! [starmap]
//! atmosphere = 0.5            # the planets' halo: strength 0..1
//! atmosphere_size = 1.3       # its radius in planet radii, 1..3
//! atmosphere_color = "#7fb2ff"
//! star_layers = 2             # far-star layers from the original's generator, 1..4
//! cloud_layers = 2            # galaxy-cloud layers, 1..4 (sharing the clouds' light)
//! star_rays = 0.5             # the systems' stars glow with rays: strength 0..1
//! night_side = 0.6            # the planets' unlit side darkened: 0..1
//! cloud_shadows = 0.5         # the clouds' shadows on the long info's planet: 0..1
//! engine_trails = 0.6         # a trail behind moving fleets: 0..1
//! ```

/// The file's name at a mod's top; not a game path.
pub const VISUALS_FILE: &str = "visuals.toml";

/// What the enabled mods' `visuals.toml` files switch on. A switch a file leaves out stays the player's setting.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Visuals {
    pub anti_aliasing: bool,
    pub per_pixel: bool,
    pub soft_shadows: bool,
    pub bloom: bool,
    pub hdr: bool,
    /// Where the whole-scene glow and the tone curve start (a stored colour value), 0..=1.
    pub bloom_threshold: Option<f32>,
    /// How much of the glow is added, 0..=2.
    pub bloom_strength: Option<f32>,
    /// The space battle's ship shadows (ADR 0088 §11).
    pub battle_shadows: bool,
    /// The space battle's light from blasts and shots on the hulls (ADR 0088 §11).
    pub blast_light: bool,
    pub battle: bool,
    pub smooth_glows: bool,
    /// The starmap planets' halo strength, 0..=1.
    pub atmosphere: Option<f32>,
    /// Its radius in planet radii, 1..=3.
    pub atmosphere_size: Option<f32>,
    pub atmosphere_color: Option<[u8; 3]>,
    /// Far-star layers (the original's generator from further seeds), 1..=4.
    pub star_layers: Option<u8>,
    /// Galaxy-cloud layers, 1..=4.
    pub cloud_layers: Option<u8>,
    /// The systems' star rays, 0..=1.
    pub star_rays: Option<f32>,
    /// The planets' night side, 0..=1.
    pub night_side: Option<f32>,
    /// The long info planet's cloud shadows, 0..=1.
    pub cloud_shadows: Option<f32>,
    /// The moving fleets' engine trails, 0..=1.
    pub engine_trails: Option<f32>,
}

impl Visuals {
    /// One file; `from` names it in the errors. An unknown table or key is an error, so a typo doesn't pass silently.
    pub fn parse(text: &str, from: &str) -> Result<Self, String> {
        let doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| format!("{from}: {}", e.message()))?;
        let mut v = Self::default();
        for (table, item) in doc.iter() {
            let t = item.as_table_like().ok_or_else(|| format!("{from}: {table} must be a table"))?;
            for (key, value) in t.iter() {
                let at = format!("{from}: {table}.{key}");
                let flag = || value.as_bool().ok_or_else(|| format!("{at} must be true or false"));
                let number = |lo: f64, hi: f64| {
                    let n = value.as_float().or_else(|| value.as_integer().map(|i| i as f64));
                    n.filter(|n| (lo..=hi).contains(n)).map(|n| n as f32).ok_or_else(|| format!("{at} must be a number from {lo} to {hi}"))
                };
                let count = |lo: i64, hi: i64| value.as_integer().filter(|n| (lo..=hi).contains(n)).map(|n| n as u8).ok_or_else(|| format!("{at} must be a whole number from {lo} to {hi}"));
                match (table, key) {
                    ("lighting", "anti_aliasing") => v.anti_aliasing = flag()?,
                    ("lighting", "per_pixel") => v.per_pixel = flag()?,
                    ("lighting", "soft_shadows") => v.soft_shadows = flag()?,
                    ("lighting", "bloom") => v.bloom = flag()?,
                    ("lighting", "hdr") => v.hdr = flag()?,
                    ("lighting", "battle_shadows") => v.battle_shadows = flag()?,
                    ("lighting", "blast_light") => v.blast_light = flag()?,
                    ("lighting", "bloom_threshold") => v.bloom_threshold = Some(number(0.0, 1.0)?),
                    ("lighting", "bloom_strength") => v.bloom_strength = Some(number(0.0, 2.0)?),
                    ("effects", "battle") => v.battle = flag()?,
                    ("effects", "smooth_glows") => v.smooth_glows = flag()?,
                    ("starmap", "atmosphere") => v.atmosphere = Some(number(0.0, 1.0)?),
                    ("starmap", "atmosphere_size") => v.atmosphere_size = Some(number(1.0, 3.0)?),
                    ("starmap", "atmosphere_color") => {
                        let hex = value.as_str().and_then(|s| s.strip_prefix('#')).filter(|s| s.len() == 6 && s.is_ascii());
                        let rgb = hex.and_then(|s| u32::from_str_radix(s, 16).ok()).ok_or_else(|| format!("{at} must be a colour like \"#7fb2ff\""))?;
                        v.atmosphere_color = Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]);
                    }
                    ("starmap", "star_layers") => v.star_layers = Some(count(1, 4)?),
                    ("starmap", "cloud_layers") => v.cloud_layers = Some(count(1, 4)?),
                    ("starmap", "star_rays") => v.star_rays = Some(number(0.0, 1.0)?),
                    ("starmap", "night_side") => v.night_side = Some(number(0.0, 1.0)?),
                    ("starmap", "cloud_shadows") => v.cloud_shadows = Some(number(0.0, 1.0)?),
                    ("starmap", "engine_trails") => v.engine_trails = Some(number(0.0, 1.0)?),
                    _ => return Err(format!("{at}: no such setting")),
                }
            }
        }
        Ok(v)
    }

    /// Lay a higher-priority mod's file over this one: a switch on in either is on, its values win.
    pub fn merge(&mut self, higher: &Self) {
        self.anti_aliasing |= higher.anti_aliasing;
        self.per_pixel |= higher.per_pixel;
        self.soft_shadows |= higher.soft_shadows;
        self.bloom |= higher.bloom;
        self.hdr |= higher.hdr;
        self.battle_shadows |= higher.battle_shadows;
        self.blast_light |= higher.blast_light;
        self.bloom_threshold = higher.bloom_threshold.or(self.bloom_threshold);
        self.bloom_strength = higher.bloom_strength.or(self.bloom_strength);
        self.battle |= higher.battle;
        self.smooth_glows |= higher.smooth_glows;
        self.atmosphere = higher.atmosphere.or(self.atmosphere);
        self.atmosphere_size = higher.atmosphere_size.or(self.atmosphere_size);
        self.atmosphere_color = higher.atmosphere_color.or(self.atmosphere_color);
        self.star_layers = higher.star_layers.or(self.star_layers);
        self.cloud_layers = higher.cloud_layers.or(self.cloud_layers);
        self.star_rays = higher.star_rays.or(self.star_rays);
        self.night_side = higher.night_side.or(self.night_side);
        self.cloud_shadows = higher.cloud_shadows.or(self.cloud_shadows);
        self.engine_trails = higher.engine_trails.or(self.engine_trails);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_merges_and_refuses_typos() {
        let low = Visuals::parse("[lighting]\nbloom = true\nhdr = true\nbloom_threshold = 0.7\n[starmap]\natmosphere = 1\natmosphere_color = \"#7fb2ff\"\n", "a").unwrap();
        assert_eq!((low.bloom, low.anti_aliasing, low.atmosphere, low.atmosphere_color), (true, false, Some(1.0), Some([0x7f, 0xb2, 0xff])));
        let high = Visuals::parse("[lighting]\nbloom_strength = 1.5\n[effects]\nbattle = true\n[starmap]\natmosphere = 0.25\n", "b").unwrap();
        let mut all = low;
        all.merge(&high);
        assert_eq!((all.bloom, all.battle, all.atmosphere, all.atmosphere_color), (true, true, Some(0.25), Some([0x7f, 0xb2, 0xff])));
        assert_eq!((all.hdr, all.bloom_threshold, all.bloom_strength), (true, Some(0.7), Some(1.5)));
        assert_eq!(Visuals::parse("", "c").unwrap(), Visuals::default());
        let a = Visuals::parse("[starmap]\nstar_layers = 2\ncloud_layers = 3\nstar_rays = 0.5\nnight_side = 1\ncloud_shadows = 0.25\nengine_trails = 0.75\n", "e").unwrap();
        let mut b = Visuals::parse("[starmap]\nstar_layers = 4\n", "f").unwrap();
        b.merge(&a);
        assert_eq!((b.star_layers, b.cloud_layers, b.star_rays, b.night_side, b.cloud_shadows, b.engine_trails), (Some(2), Some(3), Some(0.5), Some(1.0), Some(0.25), Some(0.75)));
        for bad in ["[starmap]\nstar_layers = 5", "[starmap]\nstar_layers = 0", "[starmap]\ncloud_layers = 1.5", "[starmap]\nstar_rays = 2", "[starmap]\nnight_side = true", "[starmap]\nengine_trails = -1"] {
            assert!(Visuals::parse(bad, "g").is_err(), "{bad}");
        }
        let mut lit = Visuals::parse("[lighting]\nbattle_shadows = true\n", "e").unwrap();
        assert_eq!((lit.battle_shadows, lit.blast_light), (true, false));
        lit.merge(&Visuals::parse("[lighting]\nblast_light = true\nbattle_shadows = false\n", "f").unwrap());
        assert_eq!((lit.battle_shadows, lit.blast_light), (true, true));
        for bad in ["[lighting]\nblom = true", "[starmap]\natmosphere = 2", "[starmap]\natmosphere_color = \"blue\"", "[lighting]\nbloom = 1", "[lighting]\nhdr = 1", "[lighting]\nbattle_shadows = 1", "[lighting]\nblast_lights = true", "[lighting]\nbloom_strength = 3","lighting = 1"] {
            assert!(Visuals::parse(bad, "d").is_err(), "{bad}");
        }
    }
}
