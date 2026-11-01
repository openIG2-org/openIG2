//! Mod text patches (ADR 0044 §10–§13): a mod's `texts.<lang>.toml` sets lines of the game's text files by key,
//! `texts.en.toml` standing in for a language the mod has no file for.
//!
//! A patch is applied to the text file itself, in its own grammar (`re/formats/text.md`), so every reader of that
//! file sees the patched lines: the string tables (§1, §2), the diplomacy phrases (§4) and the message text parts (§3).

use std::collections::BTreeMap;

use toml_edit::{DocumentMut, Item, Value};

use crate::dialogue::{atoi, is_header};
use crate::strings::{skip_markers, take_number, MAX_INDEX, MAX_SECTION};
use crate::text::{lines, Lang};

/// A text patch file's name for `lang` (at a mod folder's top; not a game path).
pub fn file_name(lang: Lang) -> String {
    format!("texts.{}.toml", lang.dir().unwrap_or("en"))
}

/// The language of a text patch file name (`texts.hu.toml` → `Hu`).
pub fn file_lang(name: &str) -> Option<Lang> {
    Lang::ALL.into_iter().find(|&l| file_name(l) == name)
}

/// The patchable text files, by their keys' grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `dataraw/text/txtNN.txt`: key `"file section index"`.
    Ui,
    /// `dataraw/text/s_*.txt`: key = the line number `n`.
    Keyed,
    /// `dataraw/diplomac/*.eng` but `special.eng`: key = the phrase code (`001/000/003`, `A001/000`).
    Phrases,
    /// `dataraw/uzenetek/txtNNNN.txt`: key = the part number.
    Parts,
}

fn digits(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_digit())
}

pub fn kind(path: &str) -> Option<Kind> {
    let (dir, name) = path.rsplit_once('/')?;
    let stem = name.strip_suffix(".txt");
    match dir {
        "dataraw/text" if stem.and_then(|s| s.strip_prefix("txt")).is_some_and(|n| digits(n, 2)) => Some(Kind::Ui),
        "dataraw/text" if crate::strings::KEYED_TABLES.iter().any(|(f, _)| *f == name) => Some(Kind::Keyed),
        "dataraw/diplomac" if name.ends_with(".eng") && name != "special.eng" => Some(Kind::Phrases),
        "dataraw/uzenetek" if stem.and_then(|s| s.strip_prefix("txt")).is_some_and(|n| digits(n, 4)) => Some(Kind::Parts),
        _ => None,
    }
}

/// One patched key: its text (one entry per diplomacy variant, `\n` between a phrase's or a part's lines) and the
/// file that set it, for error messages.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    texts: Vec<String>,
    from: String,
}

/// Every patched key of the enabled mods, merged: game path → key → text.
#[derive(Debug, Clone, Default)]
pub struct TextPatch {
    files: BTreeMap<String, BTreeMap<String, Entry>>,
}

impl TextPatch {
    /// Merge one patch file (`from` names it in errors) over what is there; the game's code page is `lang`'s.
    /// Returns how many keys it sets. Nothing is merged when it fails.
    pub fn add(&mut self, toml: &str, from: &str, lang: Lang) -> Result<usize, String> {
        let doc: DocumentMut = toml.parse().map_err(|e| format!("{from}: {e}"))?;
        let mut add: Vec<(String, String, Entry)> = Vec::new();
        for (path, item) in doc.iter() {
            let err = |key: &str, e: String| format!("{from}: [\"{path}\"] {key}: {e}");
            let p = path.to_ascii_lowercase().replace('\\', "/");
            let k = kind(&p).ok_or_else(|| {
                format!("{from}: [\"{path}\"]: not a patchable text file (dataraw/text/txtNN.txt or s_*.txt, dataraw/diplomac/*.eng, dataraw/uzenetek/txtNNNN.txt)")
            })?;
            let table = item.as_table_like().ok_or_else(|| format!("{from}: \"{path}\" must be a table"))?;
            for (key, v) in table.iter() {
                let key_norm = normal_key(k, key).map_err(|e| err(key, e))?;
                let texts = values(k, v).map_err(|e| err(key, e))?;
                if let Some(t) = texts.iter().find(|t| lang.encoding().encode(t).2) {
                    return Err(err(key, format!("{t:?} has a character the game's {} code page lacks", lang.encoding().name())));
                }
                add.push((p.clone(), key_norm, Entry { texts, from: from.to_owned() }));
            }
        }
        let n = add.len();
        for (p, k, e) in add {
            self.files.entry(p).or_default().insert(k, e);
        }
        Ok(n)
    }

    /// The patched game paths.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// `text` (the decoded file at `path`) with the patch's lines for it.
    pub fn apply(&self, path: &str, text: &str) -> Result<String, String> {
        let Some(keys) = self.files.get(path) else { return Ok(text.to_owned()) };
        let mut out: Vec<String> = lines(text).map(str::to_owned).collect();
        if out.last().is_some_and(String::is_empty) {
            out.pop();
        }
        match kind(path) {
            // The later line of a key wins (`StringProcess`), so the patch's lines go last.
            Some(Kind::Ui) => out.extend(keys.iter().map(|(k, e)| format!("{k}|\"{}\"", e.texts[0]))),
            Some(Kind::Keyed) => keyed(&mut out, keys),
            Some(Kind::Phrases) => phrases(&mut out, keys),
            Some(Kind::Parts) => parts(&mut out, keys).map_err(|e| format!("{path}: {e}"))?,
            None => unreachable!("checked in add"),
        }
        Ok(out.join("\r\n") + "\r\n")
    }
}

fn normal_key(k: Kind, key: &str) -> Result<String, String> {
    let num = |s: &str, max: u32| s.parse::<u32>().ok().filter(|n| (1..=max).contains(n)).ok_or(());
    match k {
        Kind::Ui => {
            let n: Vec<&str> = key.split_whitespace().collect();
            match n[..] {
                [f, s, i] => match (num(f, u16::MAX as u32), num(s, MAX_SECTION as u32), num(i, MAX_INDEX as u32)) {
                    (Ok(f), Ok(s), Ok(i)) => Ok(format!("{f} {s} {i}")),
                    _ => Err(format!("file 1..={}, section 1..={MAX_SECTION}, index 1..={MAX_INDEX}", u16::MAX)),
                },
                _ => Err("the key is \"file section index\", as in the file".into()),
            }
        }
        Kind::Keyed => num(key, u16::MAX as u32).map(|n| n.to_string()).map_err(|_| format!("the key is a line number 1..={}", u16::MAX)),
        Kind::Phrases => {
            let body = key.strip_prefix('A').unwrap_or(key);
            if !body.is_empty() && body.split('/').all(|p| digits(p, 3)) { Ok(key.to_owned()) } else { Err("the key is a phrase code: 001/000/003, or A001/000".into()) }
        }
        Kind::Parts => num(key, 99).map(|n| n.to_string()).map_err(|_| "the key is a part number 1..=99".into()),
    }
}

fn values(k: Kind, v: &Item) -> Result<Vec<String>, String> {
    let text = |v: &Value| v.as_str().map(str::to_owned).ok_or_else(|| format!("expected a string, got {}", v.type_name()));
    let texts = match (k, v.as_value()) {
        (Kind::Phrases, Some(Value::Array(a))) if !a.is_empty() => a.iter().map(text).collect::<Result<Vec<_>, _>>()?,
        (_, Some(v)) => vec![text(v)?],
        _ => return Err("expected a string".into()),
    };
    let multiline = matches!(k, Kind::Phrases | Kind::Parts);
    for t in &texts {
        if t.contains('\r') || (!multiline && t.contains('\n')) {
            return Err(format!("{t:?}: a line break is only allowed in diplomacy phrases and message parts"));
        }
    }
    Ok(texts)
}

/// The number of a `s_*.txt` data line, by `LoadStrTable`'s grammar.
fn keyed_number(line: &str) -> Option<u32> {
    let lead = line.trim_start_matches(' ');
    (!lead.starts_with(';')).then(|| take_number(skip_markers(lead).0).0).flatten()
}

/// Replace the lines of the patched numbers; insert the new ones before the first larger number, so the table stays
/// in the increasing order `LoadStrTable` asserts.
fn keyed(out: &mut Vec<String>, keys: &BTreeMap<String, Entry>) {
    let line = |n: u32, e: &Entry| format!("{n}|\"{}\"", e.texts[0]);
    let keys: BTreeMap<u32, &Entry> = keys.iter().map(|(k, e)| (k.parse().expect("normalized"), e)).collect();
    let nums: Vec<Option<u32>> = out.iter().map(|l| keyed_number(l)).collect();
    for (l, n) in out.iter_mut().zip(&nums) {
        if let Some(e) = n.and_then(|n| keys.get(&n)) {
            *l = line(n.unwrap(), e);
        }
    }
    for (&n, e) in keys.iter().filter(|(n, _)| !nums.contains(&Some(**n))) {
        let at = out.iter().position(|l| keyed_number(l).is_some_and(|m| m > n)).unwrap_or(out.len());
        out.insert(at, line(n, e));
    }
}

/// Drop every phrase whose code is a patched one (its header to the next header), then append the patch's phrases,
/// one per variant.
fn phrases(out: &mut Vec<String>, keys: &BTreeMap<String, Entry>) {
    let mut dropping = false;
    out.retain(|l| {
        if matches!(l.as_bytes().first(), Some(b'#' | b'A')) {
            let body = l.trim_start_matches('#');
            dropping = keys.contains_key(body.split_once(';').map_or(body, |(h, _)| h).trim());
        }
        !dropping
    });
    for (code, e) in keys {
        for t in &e.texts {
            out.push(if code.starts_with('A') { code.clone() } else { format!("#{code}") });
            out.extend(t.split('\n').map(|l| format!("- {l}")));
        }
    }
}

/// Keep each patched part's header and replace its lines with the patch's, one `+` line each.
fn parts(out: &mut Vec<String>, keys: &BTreeMap<String, Entry>) -> Result<(), String> {
    let mut found: Vec<&str> = Vec::new();
    let mut result = Vec::with_capacity(out.len());
    let mut dropping = false;
    for l in out.drain(..) {
        if is_header(&l) {
            let part = atoi(&l[..2]).to_string();
            dropping = false;
            if let Some((k, e)) = keys.get_key_value(&part) {
                result.push(l);
                result.extend(e.texts[0].split('\n').map(|t| format!("+ {t}")));
                found.push(k);
                dropping = true;
                continue;
            }
        }
        if !dropping {
            result.push(l);
        }
    }
    *out = result;
    match keys.iter().find(|(k, _)| !found.contains(&k.as_str())) {
        Some((k, e)) => Err(format!("part {k} ({}): no such part in the file; a new message is a new file", e.from)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{diplomacy::PhraseFile, dialogue::parse_txt_script, strings::{KeyedStrings, UiStrings}};

    fn patch(toml: &str) -> TextPatch {
        let mut p = TextPatch::default();
        p.add(toml, "texts.en.toml of mod t", Lang::En).unwrap();
        p
    }

    #[test]
    fn names_and_kinds() {
        assert_eq!(file_name(Lang::Hu), "texts.hu.toml");
        assert_eq!(file_lang("texts.en.toml"), Some(Lang::En));
        assert_eq!(file_lang("texts.eng.toml"), None);
        assert_eq!(kind("dataraw/text/txt03.txt"), Some(Kind::Ui));
        assert_eq!(kind("dataraw/text/s_races.txt"), Some(Kind::Keyed));
        assert_eq!(kind("dataraw/diplomac/ajanlat.eng"), Some(Kind::Phrases));
        assert_eq!(kind("dataraw/diplomac/special.eng"), None);
        assert_eq!(kind("dataraw/uzenetek/txt0042.txt"), Some(Kind::Parts));
        assert_eq!(kind("dataraw/uzenetek/msg0042.txt"), None);
    }

    #[test]
    fn ui_lines_win() {
        let p = patch("[\"dataraw/text/txt03.txt\"]\n\"3 1 1\" = \"Mend; now\"\n\"3  2 5\" = \"Glorp\"\n");
        let mut t = UiStrings::default();
        t.add_file(&p.apply("dataraw/text/txt03.txt", "3 1 1 \"Fix\" ;10\r\n3 1 2 \"Keep\"\r\n").unwrap());
        assert_eq!((t.get(3, 1, 1), t.get(3, 1, 2), t.get(3, 2, 5)), ("Mend; now", "Keep", "Glorp"));
        assert!(t.rejected.is_empty());
    }

    #[test]
    fn keyed_replaces_and_inserts_in_order() {
        let p = patch("[\"dataraw/text/s_races.txt\"]\n2 = \"Blorbs\"\n4 = \"Quux\"\n9 = \"Zorgons\"\n");
        let text = p.apply("dataraw/text/s_races.txt", "; races\r\n1 Alpha\r\n2 Beta\r\n5 Gamma\r\n").unwrap();
        let mut t = KeyedStrings::default();
        t.add_file(10, &text);
        let all: Vec<(u32, &str)> = t.table(10).collect();
        assert_eq!(all, [(1, "Alpha"), (2, "Blorbs"), (4, "Quux"), (5, "Gamma"), (9, "Zorgons")]);
        assert!(t.out_of_order.is_empty() && t.rejected.is_empty());
    }

    #[test]
    fn phrases_replace_by_code() {
        let p = patch("[\"dataraw/diplomac/ajanlat.eng\"]\n\"001/000/003\" = [\"Hi #1#.\\nSecond line\", \"Yo.\"]\n\"009/001\" = \"New\"\n");
        let src = "#001/000/003 ; note\r\n- Old one\r\n#001/000/003\r\n- Old two\r\n#002/000 ; kept\r\n- Kept\r\nA001/000\r\n- Fallback\r\n";
        let f = PhraseFile::parse(&p.apply("dataraw/diplomac/ajanlat.eng", src).unwrap());
        let v: Vec<&Vec<String>> = f.variants("001/000/003").iter().map(|p| &p.lines).collect();
        assert_eq!(v, [&vec!["Hi #1#.".to_owned(), "Second line".into()], &vec!["Yo.".to_owned()]]);
        assert_eq!(f.variants("002/000")[0].lines, ["Kept"]);
        assert_eq!(f.variants("009/001")[0].lines, ["New"]);
        assert_eq!(f.phrases.iter().filter(|p| p.code == "A001/000").count(), 1);
    }

    #[test]
    fn parts_keep_their_header() {
        let p = patch("[\"dataraw/uzenetek/txt0042.txt\"]\n2 = \"Greetings.\\nWe come in peace.\"\n");
        let src = "01 0000 03 EN\r\n+ Hello\r\n02 0010 03 TE #4\r\n+ Old\r\n- flowing\r\n03 0020 03 EN\r\n+ Bye\r\n";
        let parts = parse_txt_script(&p.apply("dataraw/uzenetek/txt0042.txt", src).unwrap());
        assert_eq!(parts.len(), 3);
        assert_eq!((parts[1].anim_start, parts[1].map_anim), (10, Some(4)));
        assert_eq!(parts[1].lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["Greetings.", "We come in peace."]);
        assert_eq!(parts[2].lines[0].text, "Bye");
        let missing = patch("[\"dataraw/uzenetek/txt0042.txt\"]\n7 = \"x\"\n").apply("dataraw/uzenetek/txt0042.txt", src).unwrap_err();
        assert!(missing.contains("part 7"), "{missing}");
    }

    #[test]
    fn later_files_win_and_errors_name_the_place() {
        let mut p = patch("[\"dataraw/text/s_races.txt\"]\n2 = \"Blorbs\"\n3 = \"Kept\"\n");
        p.add("[\"dataraw/text/s_races.txt\"]\n2 = \"Blörbök\"\n", "texts.hu.toml of mod t", Lang::Hu).unwrap();
        let text = p.apply("dataraw/text/s_races.txt", "").unwrap();
        assert_eq!(text, "2|\"Blörbök\"\r\n3|\"Kept\"\r\n");
        for (bad, says) in [
            ("[\"dataraw/text/fleet.txt\"]\n1 = \"x\"\n", "not a patchable"),
            ("[\"dataraw/text/txt03.txt\"]\n\"3 1\" = \"x\"\n", "file section index"),
            ("[\"dataraw/text/txt03.txt\"]\n\"3 12 1\" = \"x\"\n", "section 1..=11"),
            ("[\"dataraw/text/s_races.txt\"]\n0 = \"x\"\n", "line number"),
            ("[\"dataraw/text/s_races.txt\"]\n2 = 5\n", "expected a string"),
            ("[\"dataraw/text/s_races.txt\"]\n2 = \"a\\nb\"\n", "line break"),
            ("[\"dataraw/diplomac/ajanlat.eng\"]\n\"1/0\" = \"x\"\n", "phrase code"),
            ("[\"dataraw/text/s_races.txt\"]\n2 = \"\u{4e2d}\"\n", "code page"),
            ("[\"dataraw/text/s_races.txt\"\n", "TOML"),
        ] {
            let e = TextPatch::default().add(bad, "texts.en.toml of mod t", Lang::En).unwrap_err();
            assert!(e.starts_with("texts.en.toml of mod t") && e.contains(says), "{bad}: {e}");
        }
    }
}
