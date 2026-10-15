//! Diplomacy text: `diplomac/ajanlat.eng` (offers), `valaszok.eng` (answers),
//! `viszontv.eng` (counter-answers, not shipped in the GOG install) and `special.eng`.
//!
//! Spec: `re/formats/text.md` § Diplomacy. Evidence (ELF `ig2`): `GetSzoveg` (code lookup and
//! variant pick), `Diplomacy_ReadSpecials` (special.eng state machine).

use crate::dialogue::atoi;
use crate::text::lines;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrase {
    /// Header code as written, without `#`: `001/000/003`, or `A001/000` for fallback answers.
    pub code: String,
    /// Comment after `;` on the header line (author's note on when the phrase applies).
    pub note: String,
    /// `- ` lines. Placeholders: `#1#` (empire), `#2#`/`#3#` (offered/requested item).
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PhraseFile {
    pub phrases: Vec<Phrase>,
}

impl PhraseFile {
    pub fn parse(text: &str) -> Self {
        let mut phrases: Vec<Phrase> = Vec::new();
        for line in lines(text) {
            let first = line.as_bytes().first().copied();
            if first == Some(b'#') || first == Some(b'A') {
                let body = line.trim_start_matches('#');
                let (head, note) = body.split_once(';').unwrap_or((body, ""));
                phrases.push(Phrase { code: head.trim().to_owned(), note: note.trim().to_owned(), lines: Vec::new() });
            } else if let Some(t) = line.strip_prefix('-') {
                if let Some(p) = phrases.last_mut() {
                    p.lines.push(t.strip_prefix(' ').unwrap_or(t).trim_end().to_owned());
                }
            }
        }
        // GetSzoveg skips from a matching header to the next `-` line, so a header with no text of
        // its own (ES/FR files repeat headers with a second comment) uses the following block.
        for i in (0..phrases.len()).rev() {
            if phrases[i].lines.is_empty() {
                if let Some(next) = phrases.get(i + 1).map(|p| p.lines.clone()) {
                    phrases[i].lines = next;
                }
            }
        }
        Self { phrases }
    }

    /// Build the lookup key exactly as `GetSzoveg` does: `#AAA/BBB[/CCC[/DDD]]`, each `%03d`.
    pub fn key(a: u32, b: u32, c: Option<u32>, d: Option<u32>) -> String {
        let mut k = format!("{a:03}/{b:03}");
        for x in [c, d].into_iter().flatten() {
            k.push_str(&format!("/{x:03}"));
        }
        k
    }

    /// All variants whose code starts with `key` (the game does a raw prefix scan and then picks
    /// `random(n)`, or `n * seed / 10001` when a seed is passed). Fallback `A…` codes are not included.
    pub fn variants(&self, key: &str) -> Vec<&Phrase> {
        self.phrases.iter().filter(|p| !p.code.starts_with('A') && p.code.starts_with(key)).collect()
    }

    /// Variant selection with an explicit seed in 0..=10000, as `GetSzoveg` does when one is given.
    pub fn pick_seeded(&self, key: &str, seed: u32) -> Option<&Phrase> {
        let v = self.variants(key);
        (!v.is_empty()).then(|| v[((v.len() as f32 * seed as f32) / 10001.0) as usize])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Special {
    pub id: i32,
    pub title: String,
    pub text_a: String,
    pub text_b: String,
    /// (option, result) pairs; 1..=5 of them.
    pub options: Vec<(String, String)>,
}

/// `special.eng`: `#id/n` then `- ` lines: title, text A, text B, then n × (option, result).
pub fn parse_specials(text: &str) -> Vec<Special> {
    let mut out = Vec::new();
    let mut cur: Option<(Special, usize)> = None;
    let mut payload: Vec<String> = Vec::new();
    for line in lines(text) {
        let line = line.trim_end_matches(|c: char| c < ' ');
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        if cur.is_none() {
            if let Some(h) = line.strip_prefix('#') {
                if let Some((id, n)) = h.split_once('/') {
                    let n = atoi(n);
                    if (1..=5).contains(&n) {
                        let s = Special { id: atoi(id), title: String::new(), text_a: String::new(), text_b: String::new(), options: vec![] };
                        cur = Some((s, n as usize));
                        payload.clear();
                    }
                }
            }
            continue;
        }
        if line.len() > 1 && line.starts_with('-') {
            payload.push(line.get(2..).unwrap_or("").to_owned());
            let (s, n) = cur.as_mut().unwrap();
            if payload.len() == 3 + 2 * *n {
                s.title = payload[0].clone();
                s.text_a = payload[1].clone();
                s.text_b = payload[2].clone();
                s.options = payload[3..].chunks(2).map(|c| (c[0].clone(), c[1].clone())).collect();
                out.push(cur.take().unwrap().0);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrases_and_lookup() {
        let f = PhraseFile::parse(
            ";c\r\n#001/000/003 ; to a stronger, neutral empire\r\n- We would like #2#\r\n- Deal?\r\n\r\n#001/000/003\r\n- Other\r\nA001/000\r\n- Fallback\r\n",
        );
        assert_eq!(f.phrases.len(), 3);
        assert_eq!(f.variants(&PhraseFile::key(1, 0, Some(3), None)).len(), 2);
        assert_eq!(f.variants("001/000").len(), 2, "prefix match, A-codes excluded");
        assert_eq!(f.pick_seeded("001/000/003", 10000).unwrap().lines, ["Other"]);
        assert_eq!(f.phrases[0].note, "to a stronger, neutral empire");
    }

    #[test]
    fn specials() {
        let s = parse_specials("#7/1\r\n- Title\r\n- A\r\n- B\r\n- Opt\r\n- Res\r\n");
        assert_eq!(s[0].id, 7);
        assert_eq!(s[0].options, [("Opt".to_owned(), "Res".to_owned())]);
    }
}
