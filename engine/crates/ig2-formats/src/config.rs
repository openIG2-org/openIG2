//! The 1999 code's config file `init/config` (`ConfigFileClass::Read` ELF 0x6202b0, opened by `IG2_gameInit` relative
//! to the working directory, the install; the GOG install ships none). Lines of `key=value`: the trailing bytes below
//! `' '` (signed `char`, so bytes ≥ 0x80 too, never the first) are cut, the first `=` splits, lines without one are
//! skipped, at most 30 entries (the 30th asserts). `GetParam` (ELF 0x620580, exe 0x53e4c0): the first key equal
//! ignoring ASCII case, else the default; the callers `atoi` it (`storyline`, `storymap`, `mpstory`, …).

use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config(Vec<(Vec<u8>, Vec<u8>)>);

impl Config {
    pub fn parse(bytes: &[u8]) -> Self {
        let mut entries = Vec::new();
        for line in bytes.split_inclusive(|&b| b == b'\n') {
            let mut end = line.len();
            while end > 1 && (line[end - 1] as i8) < 0x20 {
                end -= 1;
            }
            let line = &line[..end];
            let Some(eq) = line.iter().position(|&b| b == b'=') else { continue };
            entries.push((line[..eq].to_vec(), line[eq + 1..].to_vec()));
            if entries.len() == 30 {
                break;
            }
        }
        Self(entries)
    }

    /// `<install>/init/config`; empty when absent (`Read` returns 0 and every `GetParam` its default).
    pub fn read(install_root: &Path) -> Self {
        std::fs::read(install_root.join("init/config")).map(|b| Self::parse(&b)).unwrap_or_default()
    }

    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.0.iter().find(|(k, _)| k.eq_ignore_ascii_case(key.as_bytes())).map(|(_, v)| v.as_slice())
    }

    /// `atoi(GetParam(key, "0"))`.
    pub fn int(&self, key: &str) -> i32 {
        self.get(key).map_or(0, |v| crate::dialogue::atoi(&String::from_utf8_lossy(v)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_split_at_the_first_equals_and_keys_ignore_case() {
        let c = Config::parse(b"# no equals sign\r\nStoryLine=2\r\nmpstory=1003 x\nstoryline=1\nlang==ENG\n\n");
        assert_eq!((c.int("storyline"), c.int("MPSTORY"), c.get("lang")), (2, 1003, Some(&b"=ENG"[..])));
        assert_eq!((c.int("storymap"), c.get("# no equals sign")), (0, None));
    }

    #[test]
    fn thirty_entries_at_most() {
        let text: Vec<u8> = (0..40).flat_map(|i| format!("k{i}={i}\n").into_bytes()).collect();
        let c = Config::parse(&text);
        assert_eq!((c.int("k29"), c.get("k30")), (29, None));
    }
}
