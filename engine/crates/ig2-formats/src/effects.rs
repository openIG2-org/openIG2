//! A mod's `effects.toml` (ADR 0088 §10): the enhanced battle effects (ADR 0057) as data. Each moment the classic
//! effects start at (a blast, a hit, a shield hit, a smoke column's puff, a thruster, a projectile in flight) gets a
//! list of emitters; a moment a file leaves out keeps the engine's own recipe. Display only: never part of the
//! game's state, the save or the multiplayer check.
//!
//! ```toml
//! [pictures]
//! fire = { file = "effects/fire.png", blend = "add" }   # the mod's own picture; blend "add" or "darken"
//!
//! [[blast]]                  # one emitter; a moment may have several
//! sprite = "fire"            # glow, spark, ring, smoke or a [pictures] name
//! count = [10, 16]           # particles per start: a number or [least, most]
//! life = [0.4, 1.0]          # seconds
//! size = [0.3, 1.5]          # half size at birth and at death, in the moment's radius
//! color = "#ffcc66"
//! speed = [0.5, 3.0]         # radii a second, in a random direction
//! drag = 0.05                # the share of the speed kept after a second, 0..1
//! fall = 0                   # radii a second², down (the ground war only)
//! rise = 0                   # radii a second, up (the ground war only)
//! offset = 0.2               # born this far out (radii), along its direction of flight
//! ```

/// The file's name at a mod's top; not a game path.
pub const EFFECTS_FILE: &str = "effects.toml";

/// The moments an effect starts at, in table order.
pub const MOMENTS: [&str; 6] = ["blast", "hit", "shield", "smoke", "engine", "shot"];

/// How a picture is drawn: added (light) or darkening what lies below (smoke).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Add,
    Darken,
}

/// A picture of the mod's own: its game path (a PNG in the mod) and its blend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub file: String,
    pub blend: Blend,
}

/// What an emitter's particles look like: one of the engine's four sprites or a [`Picture`] by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sprite {
    Glow,
    Spark,
    Ring,
    Smoke,
    Picture(String),
}

/// One emitter: what it starts each time its moment comes. Ranges are drawn uniformly per particle.
#[derive(Debug, Clone, PartialEq)]
pub struct Emitter {
    pub sprite: Sprite,
    pub count: [u16; 2],
    /// Seconds.
    pub life: [f32; 2],
    /// Half size at birth and at death, in the moment's radius.
    pub size: [f32; 2],
    pub color: [f32; 3],
    /// Radii a second.
    pub speed: [f32; 2],
    /// The share of the speed kept after a second.
    pub drag: f32,
    pub fall: f32,
    pub rise: f32,
    pub offset: f32,
}

impl Default for Emitter {
    fn default() -> Self {
        Self { sprite: Sprite::Glow, count: [1; 2], life: [0.5; 2], size: [1.0; 2], color: [1.0; 3], speed: [0.0; 2], drag: 1.0, fall: 0.0, rise: 0.0, offset: 0.0 }
    }
}

/// The enabled mods' `effects.toml` files merged: the pictures by name, the moments a file sets.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Effects {
    pub pictures: Vec<(String, Picture)>,
    /// Per [`MOMENTS`] entry: the emitters, or none to keep the engine's recipe.
    pub moments: [Option<Vec<Emitter>>; 6],
}

impl Effects {
    /// One file; `from` names it in the errors. An unknown table or key, a value out of range or a sprite that
    /// names no picture is an error.
    pub fn parse(text: &str, from: &str) -> Result<Self, String> {
        let doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| format!("{from}: {}", e.message()))?;
        let mut fx = Self::default();
        if let Some(item) = doc.get("pictures") {
            let t = item.as_table_like().ok_or_else(|| format!("{from}: pictures must be a table"))?;
            for (name, p) in t.iter() {
                let at = format!("{from}: pictures.{name}");
                let p = p.as_table_like().ok_or_else(|| format!("{at} must be a table like {{ file = \"effects/fire.png\" }}"))?;
                let (mut file, mut blend) = (None, Blend::Add);
                for (key, v) in p.iter() {
                    match key {
                        "file" => file = Some(v.as_str().filter(|s| s.to_ascii_lowercase().ends_with(".png")).ok_or_else(|| format!("{at}.file must be a .png path in the mod"))?),
                        "blend" => {
                            blend = match v.as_str() {
                                Some("add") => Blend::Add,
                                Some("darken") => Blend::Darken,
                                _ => return Err(format!("{at}.blend must be \"add\" or \"darken\"")),
                            }
                        }
                        _ => return Err(format!("{at}.{key}: no such setting")),
                    }
                }
                let file = file.ok_or_else(|| format!("{at}: no file"))?.replace('\\', "/").to_ascii_lowercase();
                fx.pictures.push((name.to_owned(), Picture { file, blend }));
            }
        }
        for (table, item) in doc.iter().filter(|(t, _)| *t != "pictures") {
            let m = MOMENTS.iter().position(|&n| n == table).ok_or_else(|| format!("{from}: {table}: no such moment (one of {})", MOMENTS.join(", ")))?;
            let list = item.as_array_of_tables().ok_or_else(|| format!("{from}: {table} must be [[{table}]] emitters"))?;
            let mut emitters = Vec::new();
            for (i, t) in list.iter().enumerate() {
                emitters.push(fx.emitter(t, &format!("{from}: {table}[{}]", i + 1))?);
            }
            fx.moments[m] = Some(emitters);
        }
        Ok(fx)
    }

    fn emitter(&self, t: &toml_edit::Table, at: &str) -> Result<Emitter, String> {
        let mut e = Emitter::default();
        for (key, value) in t.iter() {
            let at = format!("{at}.{key}");
            let number = |v: &toml_edit::Value, lo: f64, hi: f64| {
                let n = v.as_float().or_else(|| v.as_integer().map(|i| i as f64));
                n.filter(|n| (lo..=hi).contains(n)).map(|n| n as f32).ok_or_else(|| format!("{at} must be a number from {lo} to {hi}"))
            };
            // A number, or [least, most].
            let range = |lo: f64, hi: f64| -> Result<[f32; 2], String> {
                match value.as_value() {
                    Some(toml_edit::Value::Array(a)) if a.len() == 2 => {
                        let (a, b) = (number(a.get(0).unwrap(), lo, hi)?, number(a.get(1).unwrap(), lo, hi)?);
                        Ok([a, b])
                    }
                    Some(v) => number(v, lo, hi).map(|n| [n; 2]),
                    None => Err(format!("{at} must be a number")),
                }
            };
            let one = |lo, hi| range(lo, hi).and_then(|[a, b]| if a == b { Ok(a) } else { Err(format!("{at} must be one number")) });
            match key {
                "sprite" => {
                    e.sprite = match value.as_str() {
                        Some("glow") => Sprite::Glow,
                        Some("spark") => Sprite::Spark,
                        Some("ring") => Sprite::Ring,
                        Some("smoke") => Sprite::Smoke,
                        Some(n) if self.pictures.iter().any(|(p, _)| p == n) => Sprite::Picture(n.to_owned()),
                        _ => return Err(format!("{at} must be glow, spark, ring, smoke or a name from [pictures]")),
                    }
                }
                "count" => {
                    let [a, b] = range(0.0, 256.0)?;
                    if a.fract() != 0.0 || b.fract() != 0.0 || a > b {
                        return Err(format!("{at} must be whole numbers, the least first"));
                    }
                    e.count = [a as u16, b as u16];
                }
                "life" => e.life = range(0.05, 30.0)?,
                "size" => e.size = range(0.0, 20.0)?,
                "speed" => e.speed = range(0.0, 50.0)?,
                "drag" => e.drag = one(0.0, 1.0)?,
                "fall" => e.fall = one(0.0, 50.0)?,
                "rise" => e.rise = one(0.0, 50.0)?,
                "offset" => e.offset = one(0.0, 10.0)?,
                "color" => {
                    let hex = value.as_str().and_then(|s| s.strip_prefix('#')).filter(|s| s.len() == 6 && s.is_ascii());
                    let rgb = hex.and_then(|s| u32::from_str_radix(s, 16).ok()).ok_or_else(|| format!("{at} must be a colour like \"#ffcc66\""))?;
                    e.color = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8].map(|c| c as f32 / 255.0);
                }
                _ => return Err(format!("{at}: no such setting")),
            }
        }
        Ok(e)
    }

    /// Lay a higher-priority mod's file over this one: its pictures replace those of the same name, each moment it
    /// sets replaces the whole moment.
    pub fn merge(&mut self, higher: &Self) {
        for (name, p) in &higher.pictures {
            match self.pictures.iter_mut().find(|(n, _)| n == name) {
                Some(slot) => slot.1 = p.clone(),
                None => self.pictures.push((name.clone(), p.clone())),
            }
        }
        for (mine, theirs) in self.moments.iter_mut().zip(&higher.moments) {
            if theirs.is_some() {
                mine.clone_from(theirs);
            }
        }
    }

    /// Whether any moment is set.
    pub fn any(&self) -> bool {
        self.moments.iter().any(Option::is_some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "[pictures]\nfire = { file = \"Effects\\\\Fire.PNG\", blend = \"darken\" }\n\n[[blast]]\nsprite = \"fire\"\ncount = [10, 16]\nlife = [0.4, 1.0]\nsize = [0.3, 1.5]\ncolor = \"#ff8000\"\nspeed = [0.5, 3]\ndrag = 0.05\noffset = 0.2\n\n[[blast]]\nsprite = \"ring\"\n\n[[engine]]\nsprite = \"glow\"\nlife = 0.2\n";

    #[test]
    fn parses_an_effects_file() {
        let fx = Effects::parse(FILE, "a").unwrap();
        assert_eq!(fx.pictures, vec![("fire".to_owned(), Picture { file: "effects/fire.png".into(), blend: Blend::Darken })]);
        let blast = fx.moments[0].as_ref().unwrap();
        assert_eq!(blast.len(), 2);
        let e = &blast[0];
        assert_eq!((e.sprite.clone(), e.count, e.life, e.size, e.speed, e.drag, e.offset), (Sprite::Picture("fire".into()), [10, 16], [0.4, 1.0], [0.3, 1.5], [0.5, 3.0], 0.05, 0.2));
        assert_eq!(e.color, [1.0, 128.0 / 255.0, 0.0]);
        assert_eq!(blast[1], Emitter { sprite: Sprite::Ring, ..Emitter::default() });
        assert_eq!(fx.moments[4].as_ref().unwrap()[0].life, [0.2; 2]);
        assert!(fx.moments[1].is_none() && fx.any());
        assert!(!Effects::parse("", "b").unwrap().any());
    }

    #[test]
    fn merges_by_moment_and_picture() {
        let mut low = Effects::parse(FILE, "a").unwrap();
        let high = Effects::parse("[pictures]\nfire = { file = \"x/hot.png\" }\n[[hit]]\nsprite = \"spark\"\n[[engine]]\nsprite = \"fire\"\ncount = 2\n", "b").unwrap();
        low.merge(&high);
        assert_eq!(low.pictures[0].1, Picture { file: "x/hot.png".into(), blend: Blend::Add });
        assert_eq!(low.moments[0].as_ref().unwrap().len(), 2);
        assert_eq!(low.moments[1].as_ref().unwrap()[0].sprite, Sprite::Spark);
        assert_eq!(low.moments[4].as_ref().unwrap()[0].count, [2, 2]);
    }

    #[test]
    fn refuses_typos_and_out_of_range_values() {
        for bad in [
            "[[blats]]\nsprite = \"glow\"",
            "[[blast]]\nsprit = \"glow\"",
            "[[blast]]\nsprite = \"fire\"",
            "[[blast]]\ncount = 300",
            "[[blast]]\ncount = [5, 2]",
            "[[blast]]\ncount = 1.5",
            "[[blast]]\nlife = 0",
            "[[blast]]\ndrag = [0.1, 0.2]",
            "[[blast]]\ncolor = \"red\"",
            "[blast]\nsprite = \"glow\"",
            "[pictures]\nfire = { file = \"fire.bmp\" }",
            "[pictures]\nfire = { file = \"fire.png\", blend = \"multiply\" }",
            "[pictures]\nfire = \"fire.png\"",
        ] {
            assert!(Effects::parse(bad, "c").is_err(), "{bad}");
        }
    }
}
