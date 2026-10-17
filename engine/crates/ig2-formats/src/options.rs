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

    /// The user's file (empty when there is none).
    pub fn load() -> Self {
        Self::path().and_then(|p| std::fs::read(p).ok()).map(|b| Self::parse(&b)).unwrap_or_default()
    }

    pub fn path() -> Option<PathBuf> {
        std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("GyroscopeGames/ig2/options.dat"))
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn int(&self, key: &str) -> Option<i32> {
        self.get(key)?.trim().parse().ok()
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
}
