//! Campaign data tables read at start-up by `story.cpp`: `text/building.txt`, `text/designs.txt`,
//! `text/values.txt`.
//!
//! These aren't the building/ship *rules* tables (those are compiled into the binary). They're
//! the campaign's pre-placed buildings, predefined designs and story timers.
//! Spec: `re/rules/story-data.md`. Evidence (ELF `ig2`): `ReadBuildingData` @0x72d500,
//! `ReadDesignData` @0x72e1b0 → `ParseDesignData` @0x72ad70 / `ParseDesignColorData` @0x72cc70,
//! `ReadValues` @0x72e3b0; hull names from `DesignStrKonv` (.data 0x119bce0).

use crate::dialogue::atoi;
use crate::text::lines;

/// The game's token separators (`' '`, `'\t'`, `','`).
fn is_sep(c: char) -> bool {
    c == ' ' || c == '\t' || c == ','
}

/// Split into the first token and the rest, skipping separators on both sides.
fn next_token(s: &str) -> (&str, &str) {
    let s = s.trim_start_matches(is_sep);
    let end = s.find(is_sep).unwrap_or(s.len());
    (&s[..end], s[end..].trim_start_matches(is_sep))
}

// ---------------------------------------------------------------------------------------------
// building.txt: `[<story>/]<planet> <building type> [<damage>]`

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoryBuilding {
    /// Story (campaign chapter) this applies to; 0 = all stories.
    pub story: i32,
    pub planet: i32,
    pub building_type: i32,
    /// Initial damage, default 0.
    pub damage: i32,
}

/// Capacity of `StoryBuildingData` (the game asserts below 300).
pub const MAX_STORY_BUILDINGS: usize = 300;

pub fn parse_buildings(text: &str) -> Vec<StoryBuilding> {
    let mut out = Vec::new();
    for line in lines(text) {
        let (first, rest) = next_token(line);
        // `[story/]planet`: with a slash, story = atoi(before); planet = atoi(after) unless story is 0.
        let (story, planet) = match first.find('/') {
            Some(p) => {
                let story = atoi(first);
                (story, if story == 0 { 0 } else { atoi(&first[p + 1..]) })
            }
            None => (0, atoi(first)),
        };
        if planet == 0 || rest.is_empty() {
            continue; // comments, headers, blank lines
        }
        let (ty, rest) = next_token(rest);
        let building_type = atoi(ty);
        if building_type == 0 {
            continue; // the game asserts here (story.cpp:0x1f6)
        }
        out.push(StoryBuilding { story, planet, building_type, damage: atoi(next_token(rest).0) });
    }
    out
}

// ---------------------------------------------------------------------------------------------
// designs.txt

/// `DesignStrKonv`: hull names accepted in designs.txt, with their hull IDs.
pub const HULLS: [(&str, i32); 23] = [
    ("Fighter1", 1), ("Fighter2", 2), ("Fighter3", 3), ("Fighter4", 4), ("Destroyer", 5),
    ("HDestroyer", 6), ("Corvette", 7), ("HCorvette", 8), ("Cruiser", 9), ("HCruiser", 10),
    ("Battleship", 11), ("Transport", 12), ("Colonization", 13), ("Terraformer", 14), ("Base1", 15),
    ("Base2", 16), ("Base3", 17), ("Base4", 18), ("Tank1", 19), ("Tank2", 20), ("Tank3", 21),
    ("Tank4", 22), ("Tank5", 23),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryDesign {
    /// Design ID. `None` = the line started with `"`; the game assigns `StoryDesignElsoTerv++`.
    pub id: Option<i32>,
    /// At most 20 characters (asserted).
    pub name: String,
    /// `II` = visible to the player.
    pub visible: bool,
    pub owner: i32,
    pub hull: i32,
    /// (pod/equipment ID, count), at most 12 pairs.
    pub pods: Vec<(i32, i32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignColor {
    pub id: i32,
    pub values: Vec<i32>,
}

/// First auto-assigned design ID (`StoryDesignElsoTerv = 0x2710`, set at ELF 0x730937).
pub const FIRST_AUTO_DESIGN_ID: i32 = 10_000;

impl Designs {
    /// IDs as the game assigns them: explicit IDs as written; quoted-name lines get
    /// 10000, 10001, … in file order.
    pub fn resolved_ids(&self) -> Vec<i32> {
        let mut next = FIRST_AUTO_DESIGN_ID;
        self.designs
            .iter()
            .map(|d| {
                d.id.unwrap_or_else(|| {
                    next += 1;
                    next - 1
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Designs {
    pub designs: Vec<StoryDesign>,
    /// `C<id> v…` lines (`ParseDesignColorData`). Supported by the game, unused in shipped data.
    pub colors: Vec<DesignColor>,
    pub rejected: Vec<(usize, String)>,
}

pub fn parse_designs(text: &str) -> Designs {
    let mut d = Designs::default();
    for (i, line) in lines(text).enumerate() {
        if let Some(rest) = line.strip_prefix(['C', 'c']) {
            let mut toks = rest.split(is_sep).filter(|t| !t.is_empty()).map(atoi);
            if let Some(id) = toks.next().filter(|&id| id != 0) {
                d.colors.push(DesignColor { id, values: toks.collect() });
            }
            continue;
        }
        let s = line.trim_start_matches(is_sep);
        let (id, s) = if s.starts_with('"') {
            (None, s)
        } else {
            let (tok, rest) = next_token(s);
            match atoi(tok) {
                0 => continue, // comments and headers
                id => (Some(id), rest),
            }
        };
        let mut reject = |why: &str| d.rejected.push((i + 1, format!("{why}: {line}")));
        let Some(body) = s.strip_prefix('"') else {
            reject("missing quoted name");
            continue;
        };
        let Some(q) = body.find('"') else {
            reject("unterminated name");
            continue;
        };
        let name = body[..q].to_owned();
        let mut toks = body[q + 1..].split(is_sep).filter(|t| !t.is_empty());
        let visible = match toks.next() {
            Some(v) if v.eq_ignore_ascii_case("II") => true,
            Some(v) if v.eq_ignore_ascii_case("NN") => false,
            _ => {
                reject("visibility must be II or NN");
                continue;
            }
        };
        let owner = toks.next().map_or(0, atoi);
        let hull_tok = toks.next().unwrap_or("");
        let hull = match atoi(hull_tok) {
            0 => match HULLS.iter().find(|(n, _)| n.eq_ignore_ascii_case(hull_tok)) {
                Some(&(_, id)) => id,
                None => {
                    reject("unknown hull name");
                    continue;
                }
            },
            n => n,
        };
        let pods: Vec<(i32, i32)> = toks
            .map(|t| match t.split_once('/') {
                Some((a, b)) => (atoi(a), atoi(b)),
                None => (atoi(t), 0),
            })
            .collect();
        if name.len() > 20 || pods.len() > 12 {
            reject("name > 20 chars or > 12 pods");
            continue;
        }
        d.designs.push(StoryDesign { id, name, visible, owner, hull, pods });
    }
    d
}

// ---------------------------------------------------------------------------------------------
// values.txt: `<key> <value>`

/// Capacity of `StoryValueData` (the game asserts below 100).
pub const MAX_STORY_VALUES: usize = 100;

/// Story timer values (key = story event/message ID, value = duration [inf: in `Ido` units]).
pub fn parse_values(text: &str) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for line in lines(text) {
        let (k, rest) = next_token(line);
        let key = atoi(k);
        if key == 0 || rest.is_empty() {
            continue;
        }
        out.push((key, atoi(next_token(rest).0)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buildings() {
        let b = parse_buildings("// Foepulet 1\r\n0011  2 // Kis lakoepulet\r\n3/0012 52 40\r\n0/0013 4\r\n");
        assert_eq!(b, [
            StoryBuilding { story: 0, planet: 11, building_type: 2, damage: 0 },
            StoryBuilding { story: 3, planet: 12, building_type: 52, damage: 40 },
        ]);
    }

    #[test]
    fn designs() {
        let d = parse_designs(concat!(
            "//9411 \"destr 4_1\", NN, 4, Destroyer, 17/1\r\n",
            "4020 \"Hercules\", II, 3, TANK1, 161/1, 157/1, 166/1\r\n",
            "\"Toxic\", NN,\t\t1, Fighter2\r\n",
        ));
        assert_eq!(d.designs.len(), 2);
        assert_eq!(d.designs[0], StoryDesign { id: Some(4020), name: "Hercules".into(), visible: true, owner: 3, hull: 19, pods: vec![(161, 1), (157, 1), (166, 1)] });
        assert_eq!(d.designs[1].id, None);
        assert_eq!(d.designs[1].hull, 2);
        assert!(d.rejected.is_empty());
    }

    #[test]
    fn values() {
        assert_eq!(parse_values("//x\r\n1  500\r\n5301 1600 // c\r\n;Emb 2\r\n"), [(1, 500), (5301, 1600)]);
    }
}
