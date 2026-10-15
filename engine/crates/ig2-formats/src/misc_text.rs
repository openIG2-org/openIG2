//! Smaller text formats: `text/infostr.txt`, `text/res_info.txt`, `scenario/*.dsc`, `help/*.txt`.
//!
//! Spec: `re/formats/text.md`. Evidence (ELF `ig2`): `InfoString_Read`, `LoadTalalmanyInfok`,
//! `LoadScenarioDescription` (`../sources_base/save.cpp`), `Help_Belep`, `HelpKikeresKepek`.

use crate::dialogue::atoi;
use crate::text::lines;

// ---------------------------------------------------------------------------------------------
// infostr.txt

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoString {
    pub id: i16,
    /// `II` = true, `NN` = false [inf: shown as a warning/flashing problem].
    pub flag: bool,
    pub text: String,
}

/// Returns the entries and, if loading stopped early like the game does, the bad line number.
pub fn parse_infostr(text: &str) -> (Vec<InfoString>, Option<usize>) {
    let mut out = Vec::new();
    for (i, raw) in lines(text).enumerate() {
        let line = raw.trim_end_matches(['\r', '\n']);
        if line.is_empty() || line.starts_with(';') {
            continue;
        }
        let b = line.as_bytes();
        let n = b.iter().take_while(|&&c| c == b'-' || c == b' ' || c == b'\t' || c.is_ascii_digit()).count();
        let ok = n > 0 && matches!(b[n - 1], b' ' | b'\t') && line.len() - n >= 4;
        let flag = match line.get(n..n + 2) {
            Some("II") if ok => true,
            Some("NN") if ok => false,
            _ => return (out, Some(i + 1)),
        };
        if out.len() >= 1000 {
            return (out, Some(i + 1));
        }
        out.push(InfoString { id: atoi(&line[..n - 1]) as i16, flag, text: line[n + 3..].to_owned() });
    }
    (out, None)
}

// ---------------------------------------------------------------------------------------------
// res_info.txt

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechInfo {
    /// Technology index (1-based, < 190), as in `ResTalalmanyInfok`.
    pub index: u32,
    /// Name taken from the `;  "Name"` line. The game ignores this line.
    pub name: String,
    /// Three description lines (each stored in a 180-byte field by the game).
    pub lines: [String; 3],
}

/// `LoadTalalmanyInfok`: strictly 4 lines per technology (comment line + 3 text lines).
pub fn parse_res_info(text: &str) -> Vec<TechInfo> {
    let all: Vec<&str> = lines(text).collect();
    let mut out = Vec::new();
    for (k, chunk) in all.chunks(4).enumerate() {
        let index = k as u32 + 1;
        if index >= 190 || chunk.len() < 4 {
            break;
        }
        // EN and most translations quote the name (`;  "Fighter"  !!! note`); HU doesn't (`;  Vadász`).
        let name = match chunk[0].split('"').nth(1) {
            Some(q) => q.to_owned(),
            None => chunk[0].trim_start_matches(';').trim().to_owned(),
        };
        out.push(TechInfo { index, name, lines: [1, 2, 3].map(|i| chunk[i].trim_end().to_owned()) });
    }
    out
}

// ---------------------------------------------------------------------------------------------
// scenario/*.dsc

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScenarioDesc {
    pub title: String,
    /// Picture path as written (`pic\spwar\L10070.bmp`); the game prefixes `..\scenario\`.
    pub pic: String,
    pub difficulty: i32,
    pub race: i32,
    pub desc: Vec<String>,
}

pub fn parse_dsc(text: &str) -> ScenarioDesc {
    let mut d = ScenarioDesc::default();
    for line in lines(text) {
        let t = line.trim_start();
        let (key, val) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
        let val = val.trim_start();
        match key.to_ascii_lowercase().as_str() {
            "title" => d.title = val.trim_end().to_owned(),
            "pic" => d.pic = val.trim_end().to_owned(),
            "desc" => d.desc.push(val.trim_end().to_owned()),
            "difficulty" => d.difficulty = atoi(val),
            "race" => d.race = atoi(val),
            _ => {}
        }
    }
    d
}

// ---------------------------------------------------------------------------------------------
// help/helpmain.txt and help/<topic>.txt

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpTopic {
    pub name: String,
    pub file: String,
    pub subtopics: Vec<String>,
}

/// `Help_Belep`: `<TOPICname,file>` and `<SUBTOPICname>` tags; everything else is commentary.
pub fn parse_help_index(text: &str) -> Vec<HelpTopic> {
    let mut out: Vec<HelpTopic> = Vec::new();
    for line in lines(text) {
        let t = line.trim();
        if let Some(r) = t.strip_prefix("<TOPIC").and_then(|r| r.strip_suffix('>')) {
            let (name, file) = r.split_once(',').unwrap_or((r, ""));
            out.push(HelpTopic { name: name.to_owned(), file: file.trim().to_owned(), subtopics: vec![] });
        } else if let Some(r) = t.strip_prefix("<SUBTOPIC").and_then(|r| r.strip_suffix('>')) {
            if let Some(tp) = out.last_mut() {
                tp.subtopics.push(r.to_owned());
            }
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpSpan {
    Text(String),
    /// `<Fn>`
    Font(u32),
    /// `<Cn>`
    Color(u32),
    /// `<Pname.bmp>`: picture, loaded from `../help/<name>` (remaster: `help/<name>_bmp.pvr`).
    Picture(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpLine {
    /// `*` heading/anchor, `+` new line, `-` flowing text.
    pub kind: char,
    pub spans: Vec<HelpSpan>,
}

pub fn parse_help_page(text: &str) -> Vec<HelpLine> {
    let mut out = Vec::new();
    for line in lines(text) {
        let Some(kind @ ('*' | '+' | '-')) = line.chars().next() else { continue };
        let mut rest = line.get(1..).unwrap_or("");
        rest = rest.strip_prefix(' ').unwrap_or(rest);
        let mut spans = Vec::new();
        while !rest.is_empty() {
            if let Some(tag_end) = rest.strip_prefix('<').and_then(|r| r.find('>')) {
                let tag = &rest[1..1 + tag_end];
                let span = match tag.as_bytes().first() {
                    Some(b'F') if tag[1..].bytes().all(|c| c.is_ascii_digit()) => Some(HelpSpan::Font(atoi(&tag[1..]) as u32)),
                    Some(b'C') if tag[1..].bytes().all(|c| c.is_ascii_digit()) => Some(HelpSpan::Color(atoi(&tag[1..]) as u32)),
                    Some(b'P') => Some(HelpSpan::Picture(tag[1..].to_owned())),
                    _ => None,
                };
                if let Some(s) = span {
                    spans.push(s);
                    rest = &rest[tag_end + 2..];
                    continue;
                }
            }
            let first_len = rest.chars().next().map_or(1, char::len_utf8);
            let next = rest[first_len..].find('<').map_or(rest.len(), |i| i + first_len);
            match spans.last_mut() {
                Some(HelpSpan::Text(t)) => t.push_str(&rest[..next]),
                _ => spans.push(HelpSpan::Text(rest[..next].to_owned())),
            }
            rest = &rest[next..];
        }
        out.push(HelpLine { kind, spans });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infostr() {
        let (v, err) = parse_infostr("0 NN none\r\n100 II Infected with virus!\r\n");
        assert_eq!(err, None);
        assert_eq!(v[1], InfoString { id: 100, flag: true, text: "Infected with virus!".into() });
        assert_eq!(parse_infostr("1 XX bad line\r\n").1, Some(1));
    }

    #[test]
    fn dsc_and_help() {
        let d = parse_dsc("title Attack\r\n\r\npic pic\\spwar\\L10070.bmp\r\ndifficulty 3\r\nrace 1\r\ndesc A\r\ndesc B\r\n");
        assert_eq!((d.title.as_str(), d.difficulty, d.race, d.desc.len()), ("Attack", 3, 1, 2));
        let t = parse_help_index("x\r\n<TOPICColony,colony1.txt>\r\n<SUBTOPICSMALL INFO>\r\n");
        assert_eq!(t[0].file, "colony1.txt");
        assert_eq!(t[0].subtopics, ["SMALL INFO"]);
        let p = parse_help_page("* <F4><C3>   COLONY\r\n*       <Pkolonia_kis.bmp>\r\n");
        assert_eq!(p[0].spans[..2], [HelpSpan::Font(4), HelpSpan::Color(3)]);
        assert_eq!(p[1].spans.last(), Some(&HelpSpan::Picture("kolonia_kis.bmp".into())));
    }
}

/// `text/autobld1..10.txt` (`INIT_AutoBuild`): one autobuild mode. Lines end at CR (an LF right after
/// it is skipped, `NewTextReadLn`) and are kept verbatim; only lines starting with `:` count: the mode
/// name, the problem threshold (`atoi`), the listed-building priority (`atoi`), then building category
/// names (matched against the exe's `AutoBuildNevek` by the caller) up to `END`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutobuildFile {
    pub name: String,
    pub threshold: i32,
    pub listed_priority: i32,
    pub entries: Vec<String>,
}

pub fn parse_autobuild(text: &str) -> AutobuildFile {
    let mut lines = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let end = rest.find('\r').unwrap_or(rest.len());
        lines.push(&rest[..end]);
        rest = &rest[end..];
        rest = rest.strip_prefix('\r').unwrap_or(rest);
        rest = rest.strip_prefix('\n').unwrap_or(rest);
    }
    let mut fields = lines.into_iter().filter_map(|l| l.strip_prefix(':'));
    let name = fields.next().unwrap_or_default().to_string();
    let threshold = fields.next().map_or(0, atoi);
    let listed_priority = fields.next().map_or(0, atoi);
    let mut entries = Vec::new();
    for f in fields {
        entries.push(f.to_string());
        if f == "END" {
            break;
        }
    }
    AutobuildFile { name, threshold, listed_priority, entries }
}

#[cfg(test)]
mod autobuild_tests {
    use super::*;

    #[test]
    fn sections_and_entries() {
        let t = ":MODE\r\n\r\ncomment\r\n: 12x\r\n\r\n:-3\r\n:GUN\r\n:BANK\r\n:END\r\n:GUN\r\n";
        let f = parse_autobuild(t);
        assert_eq!(f.name, "MODE");
        assert_eq!((f.threshold, f.listed_priority), (12, -3));
        assert_eq!(f.entries, ["GUN", "BANK", "END"]);
    }
}
