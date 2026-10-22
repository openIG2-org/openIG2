//! Text decoding for the original 8-bit game text.
//!
//! The 1999 code renders text from bitmap charsets (`GetCharsetKepAddr`), so bytes are glyph
//! indices. Survey of high bytes per language (2026-10-15) [inf]:
//! - `hu` uses 0xF5 (ő) and 0xFB (ű) → Windows-1250.
//! - de/fr/es/it use Windows-1252 letters (FR 0x9C = œ), as does the English base.
//!
//! This still needs checking against the charset glyph images (re/STATUS.md U8).

use encoding_rs::{Encoding, WINDOWS_1250, WINDOWS_1252};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    De,
    Fr,
    Es,
    It,
    Hu,
}

impl Lang {
    pub const ALL: [Lang; 6] = [Lang::En, Lang::De, Lang::Fr, Lang::Es, Lang::It, Lang::Hu];

    /// Directory under `localization/`. English is the base data and has none.
    pub fn dir(self) -> Option<&'static str> {
        match self {
            Lang::En => None,
            Lang::De => Some("de"),
            Lang::Fr => Some("fr"),
            Lang::Es => Some("es"),
            Lang::It => Some("it"),
            Lang::Hu => Some("hu"),
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.dir().unwrap_or("en").eq_ignore_ascii_case(code))
    }

    pub fn encoding(self) -> &'static Encoding {
        match self {
            Lang::Hu => WINDOWS_1250,
            _ => WINDOWS_1252,
        }
    }
}

/// Decode game text bytes to a Unicode string. A UTF-8 BOM, if present, is honoured.
pub fn decode(bytes: &[u8], lang: Lang) -> String {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return String::from_utf8_lossy(rest).into_owned();
    }
    lang.encoding().decode_without_bom_handling(bytes).0.into_owned()
}

/// Encode a decoded game text back to the game's bytes (the inverse of [`decode`] for its code page).
pub fn encode(text: &str, lang: Lang) -> Vec<u8> {
    lang.encoding().encode(text).0.into_owned()
}

/// Split into lines, accepting CRLF and LF.
pub fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hungarian_double_acute() {
        assert_eq!(decode(b"\xF5\xFB", Lang::Hu), "őű");
        assert_eq!(decode(b"\xF5\xFB", Lang::De), "õû");
    }

    #[test]
    fn crlf_lines() {
        assert_eq!(lines("a\r\nb\nc").collect::<Vec<_>>(), ["a", "b", "c"]);
    }
}
