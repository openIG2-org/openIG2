//! The game's options file `%LOCALAPPDATA%/GyroscopeGames/ig2/options.dat`: key/value string pairs, each string a
//! u32 little-endian length and its bytes (`Cutscenes` `1`, `OptionsFogResolution` `2`, …), ended by an empty
//! pair. Keys the file lacks take the exe's defaults (re/screens/starmap.md §6.1 lists the ones the port reads).

use std::path::PathBuf;

/// The parsed pairs in file order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Options {
    pub pairs: Vec<(String, String)>,
}

impl Options {
    /// Parse the pairs; a truncated tail ends the list.
    pub fn parse(b: &[u8]) -> Self {
        let mut o = 0;
        let mut next = || {
            let n = u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?) as usize;
            let s = b.get(o + 4..o + 4 + n)?;
            o += 4 + n;
            Some(String::from_utf8_lossy(s).into_owned())
        };
        let mut pairs = Vec::new();
        while let (Some(k), Some(v)) = (next(), next()) {
            if k.is_empty() {
                break;
            }
            pairs.push((k, v));
        }
        Self { pairs }
    }

    /// The user's file (empty when there is none), with the remake's own settings ([`Options::own_path`]) over it.
    pub fn load() -> Self {
        let read = |p: Option<PathBuf>| p.and_then(|p| std::fs::read(p).ok()).map(|b| Self::parse(&b)).unwrap_or_default();
        let mut o = read(Self::path());
        for (k, v) in read(Self::own_path()).pairs {
            o.set(&k, &v);
        }
        o
    }

    /// The original's file in its save folder; `IG2_OPTIONS_DIR` puts the folder elsewhere (captures pin it there,
    /// `tools/trace/run_screendiff.py`), the remake's own file and the save slots with it.
    pub fn path() -> Option<PathBuf> {
        if let Some(d) = std::env::var_os("IG2_OPTIONS_DIR") {
            return Some(PathBuf::from(d).join("options.dat"));
        }
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("GyroscopeGames/ig2/options.dat"))
    }

    /// Where the remake keeps the settings its Options screen changes: beside the original's file, in the same
    /// format, so the original's own `options.dat` is never rewritten (re/deviations.md).
    pub fn own_path() -> Option<PathBuf> {
        Self::path().map(|p| p.with_file_name("ig2-rework-options.dat"))
    }

    /// Set `key` (replacing its pair, else appending one).
    pub fn set(&mut self, key: &str, value: &str) {
        match self.pairs.iter_mut().find(|(k, _)| k == key) {
            Some(p) => p.1 = value.to_string(),
            None => self.pairs.push((key.to_string(), value.to_string())),
        }
    }

    /// The file's bytes (`save_options_new` exe 0x51c4d0): every pair, then the empty one.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        for s in self.pairs.iter().flat_map(|(k, v)| [k, v]).map(String::as_str).chain(["", ""]) {
            b.extend_from_slice(&(s.len() as u32).to_le_bytes());
            b.extend_from_slice(s.as_bytes());
        }
        b
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn int(&self, key: &str) -> Option<i32> {
        self.get(key)?.trim().parse().ok()
    }

    /// Set `pairs` in the remake's own file ([`Options::own_path`]), its folder made when the original never ran.
    pub fn store_own(pairs: &[(&str, &str)]) -> std::io::Result<()> {
        let Some(p) = Self::own_path() else { return Ok(()) };
        let mut own = std::fs::read(&p).map(|b| Self::parse(&b)).unwrap_or_default();
        for (k, v) in pairs {
            own.set(k, v);
        }
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&p, own.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Vec<u8> {
        let mut b = (v.len() as u32).to_le_bytes().to_vec();
        b.extend_from_slice(v.as_bytes());
        b
    }

    #[test]
    fn pairs_until_the_empty_one() {
        let b = [s("Cutscenes"), s("1"), s("OptionsFogResolution"), s("2"), s(""), s("")].concat();
        let o = Options::parse(&b);
        assert_eq!(o.pairs.len(), 2);
        assert_eq!(o.int("OptionsFogResolution"), Some(2));
        assert_eq!(o.get("Missing"), None);
        assert_eq!(Options::parse(&b[..b.len() - 8]).pairs.len(), 2, "the file may end without the empty pair");
        assert_eq!(Options::parse(&b[..b.len() - 9]).pairs.len(), 1, "a truncated value drops its pair");
    }

    #[test]
    fn set_and_write_round_trip() {
        let b = [s("Cutscenes"), s("1"), s("OptionsFogResolution"), s("2"), s(""), s("")].concat();
        let mut o = Options::parse(&b);
        assert_eq!(o.to_bytes(), b);
        o.set("Cutscenes", "0");
        o.set("Music Volume", "40");
        let back = Options::parse(&o.to_bytes());
        assert_eq!(back, o);
        assert_eq!((back.int("Cutscenes"), back.int("Music Volume"), back.pairs.len()), (Some(0), Some(40), 3));
    }
}
