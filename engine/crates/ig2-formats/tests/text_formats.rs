//! Golden tests for the message, diplomacy, info, help, scenario-description and formation files.
//! They read the user's install from `IG2_INSTALL_DIR` and pass vacuously when it isn't set.

use std::collections::{BTreeMap, BTreeSet};

use ig2_formats::text::{self, Lang};
use ig2_formats::{dialogue, diplomacy, formation, misc_text, Install};

fn installs() -> Vec<Install> {
    let Some(base) = Install::from_env(Lang::En) else {
        eprintln!("IG2_INSTALL_DIR not set; skipping");
        return vec![];
    };
    let root = base.expect("valid install").root().to_owned();
    Lang::ALL.into_iter().map(|l| Install::open(&root, l).unwrap()).collect()
}

fn read_text(inst: &Install, path: &str) -> String {
    text::decode(&inst.read(path).unwrap_or_else(|e| panic!("{path}: {e}")), inst.lang())
}

#[test]
fn dialogue_scripts_are_consistent() {
    for inst in installs() {
        let lang = inst.lang();
        let index = dialogue::parse_message_index(&read_text(&inst, "dataraw/uzenetek/uzenetek.txt"));
        assert!(index.len() > 200, "{lang:?}: index has {}", index.len());
        let by_id: BTreeMap<u32, &dialogue::MessageKind> = index.iter().map(|k| (k.id, k)).collect();
        let (mut msgs, mut txts, mut choices) = (0, 0, 0);
        let mut broken: Vec<String> = Vec::new();
        let mut orphans: Vec<u32> = Vec::new();
        let mut empty_parts = 0usize;
        for rel in inst.list("dataraw/uzenetek/") {
            let name = rel.rsplit('/').next().unwrap();
            let Some(num) = name.strip_prefix("msg").and_then(|n| n.strip_suffix(".txt")) else { continue };
            let id: u32 = num.parse().unwrap();
            let blocks = dialogue::parse_msg_script(&read_text(&inst, rel));
            let txt_path = format!("dataraw/uzenetek/txt{num}.txt");
            if !by_id.contains_key(&id) {
                // Script for a message that isn't in the index (cut content): unreachable in game.
                orphans.push(id);
                continue;
            }
            if inst.resolve(&txt_path).is_none() {
                broken.push(format!("{num}: no txt file"));
                continue;
            }
            let parts = dialogue::parse_txt_script(&read_text(&inst, &txt_path));
            let part_ids: BTreeSet<u32> = parts.iter().map(|p| p.part).collect();
            let block_ids: BTreeSet<u32> = blocks.iter().map(|b| b.id).collect();
            assert!(block_ids.contains(&0), "{lang:?} {rel}: no entry block 00");
            for b in &blocks {
                for c in &b.choices {
                    choices += 1;
                    if c.part != -1 && !part_ids.contains(&(c.part as u32)) {
                        broken.push(format!("{num}: part {}", c.part));
                    }
                    if c.next != -1 && !block_ids.contains(&(c.next as u32)) {
                        broken.push(format!("{num}: block {}", c.next));
                    }
                }
            }
            if !by_id.get(&id).is_some_and(|k| k.conversation) {
                // KiertekelUjuzenet only opens msgNNNN for "II" conversations: this script is unreachable.
                broken.push(format!("{num}: has msg script but index flag is NN"));
            }
            msgs += 1;
        }
        for rel in inst.list("dataraw/uzenetek/").into_iter().filter(|r| r.rsplit('/').next().unwrap().starts_with("txt")) {
            let parts = dialogue::parse_txt_script(&read_text(&inst, rel));
            assert!(!parts.is_empty(), "{lang:?} {rel}: no parts");
            // Translators occasionally left a header with no text (FR txt1844); the game prints nothing.
            empty_parts += parts.iter().filter(|p| p.lines.is_empty()).count();
            txts += 1;
        }
        eprintln!("{lang:?}: {} index entries, {msgs} dialogue scripts ({choices} choices), {txts} text scripts; broken refs {broken:?}", index.len());
        eprintln!("{lang:?}: {} orphan msg scripts (not in index): {orphans:?}; empty text parts {empty_parts}", orphans.len());
        assert!(empty_parts <= 10, "{lang:?}: {empty_parts} empty parts");
        // Shipped data bugs: references the game can't resolve either (it prints nothing for them).
        // Pinned for EN so a parser change that adds or hides one is noticed.
        if lang == Lang::En {
            let known = [
                "0193: part 2", "1561: part 3", "1561: has msg script but index flag is NN", "1685: part 2",
                "1685: part 3", "1685: part 4", "1685: part 5", "2402: part 4", "2412: part 4", "5532: part 2",
                "5532: part 3", "5532: has msg script but index flag is NN", "5540: part 2", "5540: part 3",
                "8130: part 2", "8130: part 3", "8130: has msg script but index flag is NN", "8315: part 2",
                "8443: has msg script but index flag is NN",
            ];
            assert_eq!(broken, known);
        }
        assert!(broken.len() <= 30, "{lang:?}: {broken:?}");
        assert!(msgs > 200 && txts > 700);
    }
}

#[test]
fn clip_lists_resolve_to_videos() {
    let Some(inst) = installs().into_iter().next() else { return };
    for n in [1, 3, 5] {
        let groups = dialogue::parse_clip_lists(&read_text(&inst, &format!("dataraw/text/uzenetanim_{n}.txt")));
        let mut missing = BTreeSet::new();
        let mut clips = 0;
        for g in &groups {
            for p in &g.parts {
                for c in std::iter::once(&p.clip).chain(&p.loop_clip) {
                    clips += 1;
                    if inst.resolve(&dialogue::ClipPart::video_path(c)).is_none() {
                        missing.insert(c.clone());
                    }
                }
            }
        }
        eprintln!("uzenetanim_{n}: {} groups, {clips} clip refs, missing {missing:?}", groups.len());
        assert!(groups.len() > 50);
        assert!(missing.len() * 100 <= clips, "uzenetanim_{n}: too many missing clips: {missing:?}");
    }
}

#[test]
fn diplomacy_files_parse() {
    for inst in installs() {
        let lang = inst.lang();
        for file in ["ajanlat.eng", "valaszok.eng"] {
            let f = diplomacy::PhraseFile::parse(&read_text(&inst, &format!("dataraw/diplomac/{file}")));
            assert!(f.phrases.len() > 100, "{lang:?} {file}: {}", f.phrases.len());
            for p in &f.phrases {
                assert!(!p.lines.is_empty(), "{lang:?} {file}: {} has no text", p.code);
                // Codes are compared by raw prefix, so typos like ES `001/000/000/0025` still match `.../002`.
                let ok = p.code.trim_start_matches('A').split('/').all(|x| x.len() >= 3 && x.bytes().all(|c| c.is_ascii_digit()));
                assert!(ok, "{lang:?} {file}: odd code {:?}", p.code);
            }
        }
        let specials = diplomacy::parse_specials(&read_text(&inst, "dataraw/diplomac/special.eng"));
        // EN ships 6 specials; some translations ship only 3 (data gap, see text.md).
        assert!(specials.len() == 6 || specials.len() == 3, "{lang:?}: {}", specials.len());
        eprintln!("{lang:?}: special.eng ids {:?}", specials.iter().map(|s| s.id).collect::<Vec<_>>());
    }
}

#[test]
fn info_and_tech_texts_parse() {
    for inst in installs() {
        let lang = inst.lang();
        let (info, stop) = misc_text::parse_infostr(&read_text(&inst, "dataraw/text/infostr.txt"));
        assert_eq!(stop, None, "{lang:?}: infostr loading would stop early");
        assert!(info.len() > 50);
        let techs = misc_text::parse_res_info(&read_text(&inst, "dataraw/text/res_info.txt"));
        assert!(techs.len() > 150, "{lang:?}: {} techs", techs.len());
        if lang == Lang::En {
            assert_eq!(techs[0].name, "Fighter");
        }
        // Unused slots have an empty name (`;  ""`) and blank or dotted lines (techs 32, 48, …).
        let unused: Vec<u32> = techs.iter().filter(|t| t.name.is_empty()).map(|t| t.index).collect();
        for t in techs.iter().filter(|t| !t.name.is_empty()) {
            assert!(t.lines.iter().any(|l| !l.trim().is_empty()), "{lang:?}: tech {} {:?} has no text", t.index, t.name);
        }
        eprintln!("{lang:?}: {} tech slots, unused {unused:?}", techs.len());
        assert!(unused.len() < 60, "{lang:?}: {} unused", unused.len());
    }
}

#[test]
fn scenario_descriptions_parse() {
    for inst in installs() {
        let dscs: Vec<&str> = inst.list("dataraw/scenario/").into_iter().filter(|p| p.ends_with(".dsc")).collect();
        assert_eq!(dscs.len(), 14);
        for rel in dscs {
            let d = misc_text::parse_dsc(&read_text(&inst, rel));
            assert!(!d.title.is_empty() && !d.desc.is_empty() && (1..=14).contains(&d.race), "{:?} {rel}: {d:?}", inst.lang());
        }
    }
}

#[test]
fn help_pages_resolve() {
    for inst in installs() {
        let lang = inst.lang();
        let topics = misc_text::parse_help_index(&read_text(&inst, "dataraw/help/helpmain.txt"));
        assert!(topics.len() >= 10, "{lang:?}");
        let mut missing_pics = BTreeSet::new();
        for t in &topics {
            let page = misc_text::parse_help_page(&read_text(&inst, &format!("dataraw/help/{}", t.file.to_ascii_lowercase())));
            assert!(!page.is_empty(), "{lang:?}: {} empty", t.file);
            for l in &page {
                for s in &l.spans {
                    if let misc_text::HelpSpan::Picture(p) = s {
                        let pvr = format!("dataraw/help/{}.pvr", p.to_ascii_lowercase().replace('.', "_"));
                        if inst.resolve(&pvr).is_none() {
                            missing_pics.insert(p.clone());
                        }
                    }
                }
            }
        }
        assert!(missing_pics.is_empty(), "{lang:?}: help pictures missing: {missing_pics:?}");
    }
}

#[test]
fn formations_parse() {
    let Some(inst) = installs().into_iter().next() else { return };
    for n in 0..10 {
        let slots = formation::parse(&inst.read(&format!("dataraw/spacewar/felallas{n}.bin")).unwrap()).unwrap();
        assert!(!slots.is_empty());
        eprintln!("felallas{n}: {} slots, first {:?} kind {} extra {:?}", slots.len(), slots[0].cell, slots[0].kind, slots[0].extra_point());
    }
}

#[test]
fn story_data_tables_parse() {
    use ig2_formats::story_data::{self, MAX_STORY_BUILDINGS, MAX_STORY_VALUES};
    let Some(inst) = installs().into_iter().next() else { return };
    let b = story_data::parse_buildings(&read_text(&inst, "dataraw/text/building.txt"));
    let d = story_data::parse_designs(&read_text(&inst, "dataraw/text/designs.txt"));
    let v = story_data::parse_values(&read_text(&inst, "dataraw/text/values.txt"));
    eprintln!(
        "buildings {} (types {:?}), designs {} ({} auto-id, colours {}), values {}",
        b.len(),
        b.iter().map(|x| x.building_type).collect::<BTreeSet<_>>(),
        d.designs.len(),
        d.designs.iter().filter(|x| x.id.is_none()).count(),
        d.colors.len(),
        v.len()
    );
    assert!(!b.is_empty() && b.len() < MAX_STORY_BUILDINGS);
    assert!(d.rejected.is_empty(), "{:#?}", d.rejected);
    assert!(d.designs.len() > 50);
    assert!(d.designs.iter().all(|x| (1..=23).contains(&x.hull)));
    assert!(!v.is_empty() && v.len() < MAX_STORY_VALUES);
}
