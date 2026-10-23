//! The two string-table dialects of the 1999 code.
//!
//! Spec: `re/formats/text.md`.
//! - **UI table** (`dataraw/text/txt01..txt19.txt`): `<file> <section> <index> "text" ;maxlen comment`,
//!   parsed by `StringProcess` (ELF, `../sources_base/file.cpp`) into `Stringek[file][section][index]`
//!   and read back with `GetString(file, section, index)` @0x533650.
//! - **Keyed table** (`dataraw/text/s_*.txt`): `<n>[|] "text"`, parsed by
//!   `LoadStrTable(table, path, reset)` (ELF 0x623760, `routines.cpp`); key = `table << 16 | n`.
//!
//! Both use the same line conventions: `;` comments, optional `!`/`*`/`#` markers before the numbers,
//! optional surrounding quotes.

use std::collections::BTreeMap;

/// Dimensions of the `Stringek` array: index stride 8, section stride 0x1A0 (52 entries),
/// file stride 0x11E0 (11 sections).
pub const MAX_INDEX: u16 = 52;
pub const MAX_SECTION: u16 = 11;

/// `LoadStrTable` table IDs, from the call sites in `IG2_gameInit`.
pub const KEYED_TABLES: [(&str, u16); 5] =
    [("s_story.txt", 5), ("s_anims.txt", 6), ("s_races.txt", 10), ("s_stname.txt", 29), ("s_design.txt", 59)];

/// Lines that look like data but don't parse. The game asserts on these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    pub line: usize,
    pub text: String,
    pub reason: &'static str,
}

/// Skip the `!`, `*`, `#` markers (and the whitespace after them, if any were present).
fn skip_markers(s: &str) -> (&str, usize) {
    let t = s.trim_start_matches(' ');
    let rest = t.trim_start_matches(['!', '*', '#']);
    let n = t.len() - rest.len();
    (if n > 0 { rest.trim_start() } else { rest }, n)
}

fn take_number(s: &str) -> (Option<u32>, &str) {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    (s[..end].parse().ok(), &s[end..])
}

/// Remove a pair of surrounding quotes, as both loaders do: only if the text starts with `"`,
/// is longer than one character, and the *line* ends with `"`.
fn unquote(s: &str) -> &str {
    if s.len() > 1 && s.starts_with('"') && s.ends_with('"') { &s[1..s.len() - 1] } else { s }
}

#[derive(Debug, Clone, Default)]
pub struct UiStrings {
    pub entries: BTreeMap<(u16, u16, u16), String>,
    pub rejected: Vec<Rejected>,
    /// Keys that appeared more than once (the later line wins, as in the game).
    pub duplicates: Vec<(u16, u16, u16)>,
}

impl UiStrings {
    /// Parse one `txtNN.txt` file and merge it into this table.
    pub fn add_file(&mut self, text: &str) {
        for (i, raw) in crate::text::lines(text).enumerate() {
            let line = i + 1;
            // StringProcess: scan from the end for ';' or '"'; a ';' found first starts a comment.
            let mut s = raw;
            if let Some(p) = s.rfind([';', '"']) {
                if s.as_bytes()[p] == b';' {
                    s = &s[..p];
                }
            }
            let s = s.trim_end_matches(|c: char| c <= ' ');
            let lead = s.trim_start_matches(' ');
            if lead.is_empty() || raw.trim_start_matches(' ').starts_with(';') {
                continue;
            }
            let (rest, _) = skip_markers(lead);
            if rest.is_empty() {
                continue;
            }
            let (f, rest) = take_number(rest);
            let (sec, rest) = take_number(rest.trim_start_matches(|c: char| c <= ' '));
            let (idx, rest) = take_number(rest.trim_start_matches(|c: char| c <= ' '));
            let reject = |reason| Rejected { line, text: raw.to_owned(), reason };
            let (Some(f), Some(sec), Some(idx)) = (f, sec, idx) else {
                self.rejected.push(reject("missing file/section/index number"));
                continue;
            };
            if f == 0 || sec == 0 || idx == 0 || sec > MAX_SECTION as u32 || idx > MAX_INDEX as u32 || f > u16::MAX as u32 {
                self.rejected.push(reject("number out of range"));
                continue;
            }
            let body = match rest.strip_prefix('|') {
                Some(b) => b,
                None => rest.trim_start_matches(|c: char| c <= ' '),
            };
            let key = (f as u16, sec as u16, idx as u16);
            if self.entries.insert(key, unquote(body).to_owned()).is_some() {
                self.duplicates.push(key);
            }
        }
    }

    /// `GetString(file, section, index)`. Missing entries are empty, like an unset CString.
    pub fn get(&self, file: u16, section: u16, index: u16) -> &str {
        self.entries.get(&(file, section, index)).map_or("", String::as_str)
    }
}

#[derive(Debug, Clone, Default)]
pub struct KeyedStrings {
    /// Key = `table << 16 | n`, in file order (the game asserts it's strictly increasing).
    pub entries: Vec<(u32, String)>,
    pub rejected: Vec<Rejected>,
    pub out_of_order: Vec<u32>,
}

impl KeyedStrings {
    pub fn add_file(&mut self, table: u16, text: &str) {
        for (i, raw) in crate::text::lines(text).enumerate() {
            let line = i + 1;
            let s = raw.trim_end_matches(|c: char| c < ' ');
            let lead = s.trim_start_matches(' ');
            if lead.starts_with(';') {
                continue;
            }
            let (rest, _) = skip_markers(lead);
            if rest.is_empty() {
                continue;
            }
            let (n, rest) = take_number(rest);
            let body = if let Some(b) = rest.strip_prefix('|') {
                Some(b)
            } else if rest.starts_with(' ') {
                Some(rest.trim_start_matches(' '))
            } else {
                None
            };
            let (Some(n), Some(body)) = (n.filter(|&n| n != 0), body) else {
                self.rejected.push(Rejected { line, text: raw.to_owned(), reason: "expected `<n> text` or `<n>|text`" });
                continue;
            };
            let key = (table as u32) << 16 | n;
            if self.entries.last().is_some_and(|(k, _)| *k >= key) {
                self.out_of_order.push(key);
            }
            self.entries.push((key, unquote(body).to_owned()));
        }
    }

    pub fn get(&self, table: u16, n: u32) -> Option<&str> {
        let key = (table as u32) << 16 | n;
        self.entries.iter().find(|(k, _)| *k == key).map(|(_, s)| s.as_str())
    }

    /// Every line of `table` as `(n, text)`.
    pub fn table(&self, table: u16) -> impl Iterator<Item = (u32, &str)> {
        self.entries.iter().filter(move |(k, _)| k >> 16 == table as u32).map(|(k, s)| (k & 0xffff, s.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_line_grammar() {
        let mut t = UiStrings::default();
        t.add_file(concat!(
            "; header comment\r\n",
            "3 1  1 \"Repair\"                ;70 - repairing\r\n",
            "! 3 1 2 \"Has; semicolon\"\r\n",
            "3 2 3|  leading spaces kept\r\n",
            "3 2 4 no quotes ;12\r\n",
            "x y z\r\n",
        ));
        assert_eq!(t.get(3, 1, 1), "Repair");
        assert_eq!(t.get(3, 1, 2), "Has; semicolon");
        assert_eq!(t.get(3, 2, 3), "  leading spaces kept");
        assert_eq!(t.get(3, 2, 4), "no quotes");
        assert_eq!(t.rejected.len(), 1);
    }

    #[test]
    fn keyed_line_grammar() {
        let mut t = KeyedStrings::default();
        t.add_file(29, "1 Central\r\n2|Orion\r\n; c\r\n3 \"Exterior\"\r\n");
        assert_eq!(t.get(29, 2), Some("Orion"));
        assert_eq!(t.get(29, 3), Some("Exterior"));
        assert!(t.out_of_order.is_empty() && t.rejected.is_empty());
    }
}
