//! Campaign data tables read at start-up by `story.cpp`: `text/building.txt`, `text/designs.txt`,
//! `text/values.txt`, `text/fleet.txt`.
//!
//! These aren't the building/ship *rules* tables (those are compiled into the binary). They're
//! the campaign's pre-placed buildings, predefined designs and story timers.
//! Spec: `re/rules/story-data.md`. Evidence (ELF `ig2`): `ReadBuildingData` @0x72d500,
//! `ReadDesignData` @0x72e1b0 → `ParseDesignData` @0x72ad70 / `ParseDesignColorData` @0x72cc70,
//! `ReadValues` @0x72e3b0, `ReadFlottaData` @0x72a390 → `ParseFlottaData` @0x729150; hull names from
//! `DesignStrKonv` (.data 0x119bce0), fleet ship classes from `TervStrKonv` (.data 0x119bc50).

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

// ---------------------------------------------------------------------------------------------
// fleet.txt: `[<story>/]<fleet> <design> <easy>, <normal1>, <normal2>, <hard>`, grouped by `level N`

/// `TervStrKonv`: ship-class words `ParseFlottaData` accepts in place of a design ID. The record
/// stores the class negated.
pub const FLEET_SHIP_CLASSES: [(&str, i32); 8] = [
    ("FIGHTER", 2), ("FIGHTER1", 2), ("FIGHTER2", 11), ("FIGHTER3", 12), ("FIGHTER4", 13),
    ("DESTROYER", 3), ("COLONIZATION", 4), ("SZALLITO", 8),
];
/// Capacity of `StoryFlottaData` (0x8340 bytes / 0x54; the game asserts below 400).
pub const MAX_STORY_FLEETS: usize = 400;
/// Difficulty columns (`Nehezseg` 1..=4: easy, normal 1, normal 2, hard).
pub const FLEET_DIFFICULTIES: usize = 4;
/// `level 1..9` sections (`akt_flotta_level`, initially 1).
pub const FLEET_LEVELS: usize = 9;

type FleetCounts = [[i16; FLEET_LEVELS]; FLEET_DIFFICULTIES];

/// One `StoryFlottaData` record (stride 0x54): the ships of one kind a story fleet gets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoryFleet {
    /// `+0x00`: story it applies to; 0 = all stories.
    pub story: i32,
    /// `+0x04`: story fleet ID (negative IDs such as `-0011` are kept as written).
    pub fleet: i32,
    /// `+0x08`: `> 0` a design ID (`designs.txt` or a `D` line), `< 0` a negated ship class from
    /// [`FLEET_SHIP_CLASSES`]; `HOSHAJO(x)` stores `x − 8`.
    pub design: i32,
    /// `+0x0c`: ship count per difficulty and level, after `ReadFlottaData`'s fill: levels before
    /// the first and after the last given level copy those, the gaps in between are interpolated
    /// linearly in integers (truncating).
    pub counts: FleetCounts,
}

#[derive(Debug, Clone, Default)]
pub struct Fleets {
    pub fleets: Vec<StoryFleet>,
    /// The `D…`/`C…` lines, parsed like `designs.txt` (`ParseDesignData` / `ParseDesignColorData`).
    pub designs: Designs,
    /// Lines the game asserts on (level out of range, a count given twice, fewer than 4 counts),
    /// and ship-class words it doesn't know (the record then keeps a stale design, see the parser).
    pub rejected: Vec<(usize, String)>,
}

/// `ReadFlottaData`. Lines starting with `level` switch the level, `D`/`d` and `C`/`c` lines are
/// designs, everything else goes to `ParseFlottaData` (comments and `#define`s fall out because
/// their first token `atoi`s to 0). Lines with the same story, fleet and design merge into one
/// record.
pub fn parse_fleets(text: &str) -> Fleets {
    const UNSET: FleetCounts = [[-1; FLEET_LEVELS]; FLEET_DIFFICULTIES];
    let mut out = Fleets::default();
    let mut design_lines = String::new();
    let mut level = 1usize;
    let mut given: Vec<FleetCounts> = Vec::new();
    // The `+0x08` field of the next free slot: a line whose design word is unknown leaves it as the
    // last merged line wrote it (a new record moves on to a fresh, zeroed slot).
    let mut scratch_design = 0;
    for (i, line) in lines(text).enumerate() {
        let lineno = i + 1;
        if line.len() >= 5 && line[..5].eq_ignore_ascii_case("level") {
            level = atoi(&line[5..]).clamp(0, 10) as usize;
            if !(1..=FLEET_LEVELS).contains(&level) {
                out.rejected.push((lineno, format!("level out of range: {line}")));
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix(['D', 'd']) {
            design_lines.push_str(rest);
            design_lines.push('\n');
            continue;
        }
        if line.starts_with(['C', 'c']) {
            design_lines.push_str(line);
            design_lines.push('\n');
            continue;
        }
        // First token: `[<story>/]<fleet>`.
        let (tok, rest) = next_token(line);
        let (story, fleet) = match tok.rfind('/') {
            Some(p) if p > 0 => match atoi(tok) {
                0 => (0, 0),
                st => (st, atoi(&tok[p + 1..])),
            },
            _ => (0, atoi(tok)),
        };
        if rest.is_empty() || fleet == 0 {
            continue;
        }
        let (dtok, rest) = next_token(rest);
        let hero = dtok
            .get(..7)
            .filter(|h| h.eq_ignore_ascii_case("HOSHAJO"))
            .map(|_| atoi(&dtok[7..].replace(['(', ')', '+'], " ")))
            .filter(|&x| x > 0);
        let design = match atoi(dtok) {
            0 => match (FLEET_SHIP_CLASSES.iter().find(|(n, _)| n.eq_ignore_ascii_case(dtok)), hero) {
                (Some(&(_, class)), _) => -class,
                (None, Some(x)) => x - 8,
                (None, None) => {
                    out.rejected.push((lineno, format!("unknown ship class {dtok:?}: {line}")));
                    scratch_design
                }
            },
            n => n,
        };
        let idx = match out.fleets.iter().position(|f| f.fleet == fleet && f.design == design && f.story == story) {
            Some(idx) => {
                scratch_design = design;
                idx
            }
            None => {
                out.fleets.push(StoryFleet { story, fleet, design, counts: UNSET });
                given.push(UNSET);
                scratch_design = 0;
                out.fleets.len() - 1
            }
        };
        let mut toks = rest.split(is_sep).filter(|t| !t.is_empty());
        for levels in given[idx].iter_mut() {
            let Some(t) = toks.next() else {
                out.rejected.push((lineno, format!("fewer than {FLEET_DIFFICULTIES} counts: {line}")));
                break;
            };
            if !(1..=FLEET_LEVELS).contains(&level) {
                continue;
            }
            if levels[level - 1] != -1 {
                out.rejected.push((lineno, format!("count given twice for level {level}: {line}")));
            }
            levels[level - 1] = atoi(t) as i16;
        }
    }
    for (f, set) in out.fleets.iter_mut().zip(&given) {
        for (d, levels) in set.iter().enumerate() {
            f.counts[d] = fill_levels(levels);
        }
    }
    out.designs = parse_designs(&design_lines);
    out
}

/// `ReadFlottaData`'s post-pass for one difficulty: unset levels (< 0) before the first set one and
/// after the last copy those; the ones in between are interpolated in integers.
fn fill_levels(given: &[i16; FLEET_LEVELS]) -> [i16; FLEET_LEVELS] {
    let mut v = *given;
    let (Some(first), Some(last)) = (v.iter().position(|&x| x >= 0), v.iter().rposition(|&x| x >= 0)) else {
        return v; // the game asserts here
    };
    v[0] = v[first];
    v[FLEET_LEVELS - 1] = v[last];
    let (mut a, mut b) = (0i32, 0i32);
    for i in 0..FLEET_LEVELS {
        if v[i] < 0 {
            let (va, vb, i) = (v[a as usize] as i32, v[b as usize] as i32, i as i32);
            v[i as usize] = ((va * ((b - a) - (i - a)) + vb * (i - a)) / (b - a)) as i16;
        } else {
            a = i as i32;
            b = (i + 1..FLEET_LEVELS).find(|&j| v[j] >= 0).unwrap_or(FLEET_LEVELS) as i32;
        }
    }
    v
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
    fn fleets() {
        let f = parse_fleets(concat!(
            "//[<story>/]<flottaazonosito> <terv>\r\n#define TANK 1\r\n",
            "0011 0012    0, 1, 1, 1\r\n",
            "level3:\r\n9141 DESTROYER\t1,\t1,\t1,  1\r\n590/9140 HOSHAJO(1)   1, 1, 1, 1\r\n",
            "level7:\r\n9141 DESTROYER\t9,\t9,\t9,  9\r\n",
            "D0011 \"Starlight\", NN, 4, Destroyer, 17/3, 33/1\r\n",
        ));
        assert!(f.rejected.is_empty(), "{:?}", f.rejected);
        assert_eq!(f.fleets.len(), 3);
        assert_eq!((f.fleets[0].fleet, f.fleets[0].design, f.fleets[0].counts[0]), (11, 12, [0; 9]));
        assert_eq!((f.fleets[1].fleet, f.fleets[1].design), (9141, -3));
        // Level 3 = 1, level 7 = 9: levels 1–2 copy 1, 4–6 interpolate, 8–9 copy 9.
        assert_eq!(f.fleets[1].counts[3], [1, 1, 1, 3, 5, 7, 9, 9, 9]);
        assert_eq!((f.fleets[2].story, f.fleets[2].fleet, f.fleets[2].design), (590, 9140, -7));
        assert_eq!(f.designs.designs.len(), 1);
        assert_eq!(f.designs.designs[0].id, Some(11));
    }

    #[test]
    fn values() {
        assert_eq!(parse_values("//x\r\n1  500\r\n5301 1600 // c\r\n;Emb 2\r\n"), [(1, 500), (5301, 1600)]);
    }
}
