//! A mod's `visuals.toml` (ADR 0088): the look a mod switches on. Display only: never part of the game's state, the
//! save or the multiplayer check.
//!
//! ```toml
//! [lighting]
//! anti_aliasing = true
//! per_pixel = true
//! soft_shadows = true
//! bloom = true
//!
//! [effects]
//! battle = true
//! smooth_glows = true
//!
//! [starmap]
//! atmosphere = 0.5            # the planets' halo: strength 0..1
//! atmosphere_size = 1.3       # its radius in planet radii, 1..3
//! atmosphere_color = "#7fb2ff"
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
    pub battle: bool,
    pub smooth_glows: bool,
    /// The starmap planets' halo strength, 0..=1.
    pub atmosphere: Option<f32>,
    /// Its radius in planet radii, 1..=3.
    pub atmosphere_size: Option<f32>,
    pub atmosphere_color: Option<[u8; 3]>,
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
                match (table, key) {
                    ("lighting", "anti_aliasing") => v.anti_aliasing = flag()?,
                    ("lighting", "per_pixel") => v.per_pixel = flag()?,
                    ("lighting", "soft_shadows") => v.soft_shadows = flag()?,
                    ("lighting", "bloom") => v.bloom = flag()?,
                    ("effects", "battle") => v.battle = flag()?,
                    ("effects", "smooth_glows") => v.smooth_glows = flag()?,
                    ("starmap", "atmosphere") => v.atmosphere = Some(number(0.0, 1.0)?),
                    ("starmap", "atmosphere_size") => v.atmosphere_size = Some(number(1.0, 3.0)?),
                    ("starmap", "atmosphere_color") => {
                        let hex = value.as_str().and_then(|s| s.strip_prefix('#')).filter(|s| s.len() == 6 && s.is_ascii());
                        let rgb = hex.and_then(|s| u32::from_str_radix(s, 16).ok()).ok_or_else(|| format!("{at} must be a colour like \"#7fb2ff\""))?;
                        v.atmosphere_color = Some([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]);
                    }
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
        self.battle |= higher.battle;
        self.smooth_glows |= higher.smooth_glows;
        self.atmosphere = higher.atmosphere.or(self.atmosphere);
        self.atmosphere_size = higher.atmosphere_size.or(self.atmosphere_size);
        self.atmosphere_color = higher.atmosphere_color.or(self.atmosphere_color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_merges_and_refuses_typos() {
        let low = Visuals::parse("[lighting]\nbloom = true\n[starmap]\natmosphere = 1\natmosphere_color = \"#7fb2ff\"\n", "a").unwrap();
        assert_eq!((low.bloom, low.anti_aliasing, low.atmosphere, low.atmosphere_color), (true, false, Some(1.0), Some([0x7f, 0xb2, 0xff])));
        let high = Visuals::parse("[effects]\nbattle = true\n[starmap]\natmosphere = 0.25\n", "b").unwrap();
        let mut all = low;
        all.merge(&high);
        assert_eq!((all.bloom, all.battle, all.atmosphere, all.atmosphere_color), (true, true, Some(0.25), Some([0x7f, 0xb2, 0xff])));
        assert_eq!(Visuals::parse("", "c").unwrap(), Visuals::default());
        for bad in ["[lighting]\nblom = true", "[starmap]\natmosphere = 2", "[starmap]\natmosphere_color = \"blue\"", "[lighting]\nbloom = 1", "lighting = 1"] {
            assert!(Visuals::parse(bad, "d").is_err(), "{bad}");
        }
    }
}
