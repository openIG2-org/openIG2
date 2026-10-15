//! `.scn` scenario/campaign state files: line-based keyword text.
//!
//! Spec: `re/formats/scn.md`. Evidence: `LoadScenario(const char*, int)` (ELF 0x62b880) checks
//! 225 keywords, each with `strncasecmp(line, kw, strlen(kw))` followed by an `isspace` check.
//! That makes keys whole words, case-insensitive, with leading indentation skipped.
//! A record keyword (`planet`, `BUILDING`, `Fleet`, …) starts a new record. Subsequent field
//! keywords apply to the most recent record of the matching kind.
//!
//! This module is a lossless syntactic reader. Mapping fields to game state is Phase 2 work.

use crate::text::{self, Lang};

/// Keywords that open a record. Nesting: `Building` belongs to the preceding `Planet`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordKind {
    Spy,
    Talalmany,
    Terv,
    Planet,
    Building,
    Fleet,
    DummyFleet,
    Viszony,
}

impl RecordKind {
    pub fn from_keyword(kw: &str) -> Option<Self> {
        const TABLE: [(&str, RecordKind); 8] = [
            ("SPY", RecordKind::Spy),
            ("TALALMANY", RecordKind::Talalmany),
            ("TERV", RecordKind::Terv),
            ("PLANET", RecordKind::Planet),
            ("BUILDING", RecordKind::Building),
            ("FLEET", RecordKind::Fleet),
            ("DUMMYFLEET", RecordKind::DummyFleet),
            ("VISZONY", RecordKind::Viszony),
        ];
        TABLE.iter().find(|(k, _)| k.eq_ignore_ascii_case(kw)).map(|&(_, r)| r)
    }

    /// Field-name prefix the 1999 code uses for this record's fields
    /// (`sNev` for spies, `pNev` for planets …).
    pub fn field_prefix(self) -> char {
        match self {
            RecordKind::Spy => 's',
            RecordKind::Talalmany => 'r',
            RecordKind::Terv => 't',
            RecordKind::Planet => 'p',
            RecordKind::Building => 'e',
            RecordKind::Fleet | RecordKind::DummyFleet => 'f',
            RecordKind::Viszony => 'v',
        }
    }
}

/// One `key value…` line.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub line: usize,
    pub key: String,
    /// The rest of the line after the key, trimmed. Kept raw for free-text values like names.
    pub raw: String,
}

impl Entry {
    /// Values split on whitespace and commas (`FLEET -9.81, -82.29 1`).
    pub fn tokens(&self) -> impl Iterator<Item = &str> {
        self.raw.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty())
    }

    pub fn ints(&self) -> Vec<i64> {
        // The 1999 parser uses atoi semantics: "50645.00" → 50645, garbage → 0.
        self.tokens().map(atoi).collect()
    }

    pub fn floats(&self) -> Vec<f64> {
        self.tokens().map(|t| t.parse().unwrap_or(0.0)).collect()
    }

    pub fn int(&self) -> Option<i64> {
        self.tokens().next().map(atoi)
    }

    pub fn float(&self) -> Option<f64> {
        self.tokens().next().map(|t| t.parse().unwrap_or(0.0))
    }
}

fn atoi(t: &str) -> i64 {
    let t = t.trim_start();
    let (sign, digits) = match t.as_bytes().first() {
        Some(b'-') => (-1, &t[1..]),
        Some(b'+') => (1, &t[1..]),
        _ => (1, t),
    };
    let end = digits.find(|c: char| !c.is_ascii_digit()).unwrap_or(digits.len());
    sign * digits[..end].parse::<i64>().unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub kind: RecordKind,
    pub header: Entry,
    pub fields: Vec<Entry>,
    /// Buildings of a planet.
    pub children: Vec<Record>,
}

impl Record {
    pub fn get(&self, key: &str) -> Option<&Entry> {
        self.fields.iter().find(|e| e.key.eq_ignore_ascii_case(key))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scenario {
    /// Keys outside any record (`Title`, `Money`, `Ev`, `RandSeed`, `BirodalomIsmeri` …),
    /// in file order. Some repeat (`BirodalomIsmeri` once per empire).
    pub globals: Vec<Entry>,
    pub records: Vec<Record>,
    pub comments: Vec<(usize, String)>,
}

impl Scenario {
    pub fn parse_bytes(bytes: &[u8], lang: Lang) -> Self {
        Self::parse(&text::decode(bytes, lang))
    }

    pub fn parse(src: &str) -> Self {
        let mut scn = Scenario::default();
        for (i, line) in text::lines(src).enumerate() {
            let t = line.trim_start();
            if t.is_empty() {
                continue;
            }
            if t.starts_with(';') {
                scn.comments.push((i + 1, t.to_owned()));
                continue;
            }
            let (key, raw) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
            let entry = Entry { line: i + 1, key: key.to_owned(), raw: raw.trim().to_owned() };
            if let Some(kind) = RecordKind::from_keyword(key) {
                let rec = Record { kind, header: entry, fields: vec![], children: vec![] };
                match (kind, scn.records.last_mut()) {
                    (RecordKind::Building, Some(p)) if p.kind == RecordKind::Planet => p.children.push(rec),
                    _ => scn.records.push(rec),
                }
                continue;
            }
            match scn.current_record_for(key) {
                Some(rec) => rec.fields.push(entry),
                None => scn.globals.push(entry),
            }
        }
        scn
    }

    /// The record a field line belongs to. Field keys carry their record's lowercase prefix
    /// (`pNev`, `eTipus`). A few planet display fields have no prefix (`szin`, `Dist` …).
    fn current_record_for(&mut self, key: &str) -> Option<&mut Record> {
        const PLANET_EXTRAS: [&str; 7] = ["szin", "bolygomeret", "csillagmeret", "SpinSpeed", "Yaw", "Dist", "Speed"];
        let last = self.records.last_mut()?;
        if last.kind == RecordKind::Planet {
            if is_field_of(key, 'e') {
                return last.children.last_mut();
            }
            if PLANET_EXTRAS.iter().any(|k| k.eq_ignore_ascii_case(key)) {
                return Some(last);
            }
        }
        is_field_of(key, last.kind.field_prefix()).then_some(last)
    }

    pub fn global(&self, key: &str) -> Option<&Entry> {
        self.globals.iter().find(|e| e.key.eq_ignore_ascii_case(key))
    }

    pub fn records_of(&self, kind: RecordKind) -> impl Iterator<Item = &Record> {
        self.records.iter().filter(move |r| r.kind == kind)
    }
}

/// `pNev`, `eTipus`: prefix then an uppercase letter. `px`, `ex`: prefix plus one coordinate letter.
fn is_field_of(key: &str, prefix: char) -> bool {
    let mut c = key.chars();
    c.next() == Some(prefix)
        && match (c.next(), c.next()) {
            (Some(second), _) if second.is_ascii_uppercase() => true,
            (Some('x' | 'y' | 'z'), None) => true,
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "TITLE 530\r\nMoney 50645.00 50630.00\r\nPLANET\r\n pNev Central 7\r\n px -228.00\r\n szin 9888486\r\nBUILDING\r\n   eTipus 1\r\n   ex 86.00\r\nFLEET -9.81, -82.29 1\r\n fNev Conqueror\r\n; comment\r\nVISZONY 1 2\r\n vViszony 5000\r\n";

    #[test]
    fn structure() {
        let s = Scenario::parse(SAMPLE);
        assert_eq!(s.global("title").unwrap().raw, "530");
        assert_eq!(s.global("Money").unwrap().ints(), [50645, 50630]);
        let p = s.records_of(RecordKind::Planet).next().unwrap();
        assert_eq!(p.get("pNev").unwrap().raw, "Central 7");
        assert_eq!(p.get("szin").unwrap().int(), Some(9888486));
        assert_eq!(p.children[0].get("ex").unwrap().float(), Some(86.0));
        let f = s.records_of(RecordKind::Fleet).next().unwrap();
        assert_eq!(f.header.floats(), [-9.81, -82.29, 1.0]);
        assert_eq!(s.records_of(RecordKind::Viszony).next().unwrap().get("vViszony").unwrap().int(), Some(5000));
        assert_eq!(s.comments.len(), 1);
    }
}
