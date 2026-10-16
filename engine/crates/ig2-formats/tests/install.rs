//! Golden tests against the user's own install. Set `IG2_INSTALL_DIR` to the GOG folder.
//! Without it, these tests skip (pass vacuously) so CI without game data stays green.

use std::io::{Read, Seek, SeekFrom};

use ig2_formats::container::{self, ContainerKind};
use ig2_formats::pvr::Pvr;
use ig2_formats::scn::Scenario;
use ig2_formats::text::Lang;
use ig2_formats::Install;

fn install() -> Option<Install> {
    match Install::from_env(Lang::En) {
        Some(r) => Some(r.expect("IG2_INSTALL_DIR is set but isn't a valid install")),
        None => {
            eprintln!("IG2_INSTALL_DIR not set; skipping install tests");
            None
        }
    }
}

/// Keywords `LoadScenario` (ELF 0x62b880) recognises, in check order. Extracted from the 225
/// `strncasecmp` call sites (re/formats/scn.md).
const SCN_KEYWORDS: &str = include_str!("../../../../re/formats/scn-keywords.txt");

#[test]
fn every_container_decodes() {
    let Some(inst) = install() else { return };
    let mut counts = [0usize; 4];
    for rel in inst.all_files() {
        let raw = inst.read_raw(rel).unwrap();
        let kind = container::kind(&raw);
        counts[kind as usize] += 1;
        if matches!(rel.rsplit('.').next(), Some("mp4" | "ogg")) {
            assert_eq!(kind, ContainerKind::Encrypted, "{rel}");
            continue; // Large media is covered by the streaming test.
        }
        let dec = container::decode(rel, &raw).unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert!(!dec.is_empty() || raw.is_empty(), "{rel}");
    }
    eprintln!("plain/encrypted/compressed/enc+comp: {counts:?}");
    // Survey 2026-10-15: 17,575 files, of which 7,843 are neGs media and the rest mostly ecGs.
    assert!(counts[ContainerKind::EncryptedCompressed as usize] > 9000);
}

#[test]
fn streams_decrypt_with_random_access() {
    let Some(inst) = install() else { return };
    for rel in ["mfx/menu.ogg", "anim/mpeg4/".to_owned().as_str()] {
        let rel = if rel.ends_with('/') { inst.list(rel)[0] } else { rel };
        let whole = inst.read(rel).unwrap();
        let mut s = inst.open_stream(rel).unwrap();
        let mut head = [0u8; 8];
        s.read_exact(&mut head).unwrap();
        assert!(head.starts_with(b"OggS") || &head[4..8] == b"ftyp", "{rel}");
        for off in [1u64, 15, 16, 4097, whole.len() as u64 / 2] {
            let mut buf = [0u8; 37];
            s.seek(SeekFrom::Start(off)).unwrap();
            s.read_exact(&mut buf).unwrap();
            assert_eq!(&buf[..], &whole[off as usize..off as usize + 37], "{rel} @{off}");
        }
    }
}

#[test]
fn every_pvr_decodes() {
    let Some(inst) = install() else { return };
    let mut n = 0;
    for rel in inst.all_files().into_iter().filter(|p| p.ends_with(".pvr")) {
        let bytes = inst.read(rel).unwrap();
        let pvr = Pvr::parse(&bytes).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let img = pvr.decode_rgba8().unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(img.pixels.len(), (img.width * img.height * 4) as usize, "{rel}");
        n += 1;
    }
    assert!(n >= 2000, "only {n} PVRs");
}

#[test]
fn every_scn_parses_with_known_keywords() {
    let Some(inst) = install() else { return };
    // `ProgramBattleMode` is in 21 shipped files but never checked by LoadScenario. The string
    // doesn't appear in either binary, so it's dead data (re/formats/scn.md).
    let known: Vec<&str> = SCN_KEYWORDS.split_whitespace().chain(["ProgramBattleMode"]).collect();
    let is_known = |k: &str| known.iter().any(|kw| kw.eq_ignore_ascii_case(k));
    let mut n = 0;
    for rel in inst.list("").into_iter().filter(|p| p.ends_with(".scn")) {
        let scn = Scenario::parse_bytes(&inst.read(rel).unwrap(), Lang::En);
        assert!(scn.global("Title").is_some(), "{rel}: no Title");
        let all_keys = scn.globals.iter().chain(scn.records.iter().flat_map(|r| {
            r.fields.iter().chain(r.children.iter().flat_map(|c| c.fields.iter()))
        }));
        for e in all_keys {
            assert!(is_known(&e.key), "{rel}:{} unknown key {:?}", e.line, e.key);
        }
        n += 1;
    }
    assert!(n >= 24);
}

#[test]
fn every_colony_map_parses() {
    let Some(inst) = install() else { return };
    let maps = inst.list("dataraw/colony/gen_maps/");
    assert_eq!(maps.len(), 75);
    let (mut lo, mut hi, mut padded, mut negative) = (f32::MAX, f32::MIN, 0, 0);
    for rel in maps {
        // Layout check: parse() verifies the total size against the trailing list count.
        let m = ig2_formats::colony_map::ColonyMap::parse(&inst.read(rel).unwrap()).unwrap_or_else(|e| panic!("{rel}: {e}"));
        negative += m.heights.iter().filter(|&&h| h < 0).count();
        for y in 0..50 {
            for x in 0..50 {
                let h = m.height_world(x, y);
                (lo, hi) = (lo.min(h), hi.max(h));
            }
        }
        // Mask columns 50–51 (padding the game never reads) are set in some files.
        padded += (0..50).filter(|&y| (m.mask_a[y * 13 + 12] | m.mask_b[y * 13 + 12]) & 0xF0 != 0).count();
    }
    eprintln!("colony heights {lo}..{hi} world units ({negative} negative samples); rows with padding bits set: {padded}");
    assert!(lo > -8192.0 && hi < 8192.0); // within the default terrain extent (`TEREPXSIZE`)
}

#[test]
fn terrain_description_has_one_surface_per_map() {
    let Some(inst) = install() else { return };
    let raw = inst.read("dataraw/colony/terepleiro.txt").unwrap();
    let surfaces = ig2_formats::colony_map::parse_surfaces(&ig2_formats::text::decode(&raw, inst.lang())).unwrap();
    assert_eq!(surfaces.len(), inst.list("dataraw/colony/gen_maps/").len());
    assert!(surfaces.iter().all(|s| (0..=8).contains(&s.kind)));
    // The automatic placement start must pass LocatePos's bounds (colony-placement.md §5).
    assert!(surfaces.iter().all(|s| (10..=189).contains(&s.ai_build_start.0) && (10..=189).contains(&s.ai_build_start.1)));
}

#[test]
fn every_xv_archive_parses_exactly() {
    use ig2_formats::xv::{Archive, ChildContent, Material, Mesh, Node, NodeHeader};
    let Some(inst) = install() else { return };
    let mut counts = [0usize; 4]; // archives, nodes, meshes, materials
    let (mut counts_scenes, mut without_root) = (0usize, Vec::new());
    let mut anim = [0usize; 4]; // animated meshes, ..with frame 0 = base; animated-node children, ..with key₀ = offset
    for rel in inst.all_files().into_iter().filter(|p| p.ends_with(".xv")) {
        let bytes = inst.read(rel).unwrap();
        let a = Archive::parse(&bytes).unwrap_or_else(|e| panic!("{rel}: {e}"));
        counts[0] += 1;
        for m in &a.members {
            let lower = m.name.to_ascii_lowercase();
            let ctx = || format!("{rel}:{}", m.name);
            if lower.ends_with(".xvb") {
                NodeHeader::parse(m.data).unwrap_or_else(|e| panic!("{}: {e}", ctx()));
                counts[1] += 1;
            } else if lower.ends_with(".xvo") {
                Mesh::parse(m.data).unwrap_or_else(|e| panic!("{}: {e}", ctx()));
                counts[2] += 1;
            } else if lower.ends_with(".xvm") {
                Material::parse(m.data).unwrap_or_else(|e| panic!("{}: {e}", ctx()));
                counts[3] += 1;
            }
        }
        // Every archive yields a scene, and face indices stay in range.
        fn check(n: &Node, rel: &str, anim: &mut [usize; 4]) {
            if n.header.frames() >= 3 {
                // Animated node: frame 0's translation key is the child's own offset.
                for (i, (c, _)) in n.children.iter().enumerate() {
                    let k = n.header.key(0, i).unwrap();
                    anim[2] += 1;
                    anim[3] += (0..3).all(|a| (k.translation[a] - c.translation()[a]).abs() < 1e-3) as usize;
                }
            }
            for (c, content) in &n.children {
                match content {
                    ChildContent::Node(sub) => check(sub, rel, anim),
                    ChildContent::Mesh(m) => {
                        for f in &m.faces {
                            assert!(f.idx.iter().all(|&i| (i as usize) < m.vertices.len()), "{rel}:{}", c.name);
                        }
                        if m.is_vertex_animated() && !m.vertices.is_empty() {
                            // Vertex frames hold position + normal; frame 0 is the base mesh.
                            anim[0] += 1;
                            anim[1] += m.vertices.iter().enumerate().all(|(i, v)| m.frame_vertex(0, i).unwrap().0 == v.pos) as usize;
                        }
                    }
                    ChildContent::Missing => {}
                }
            }
        }
        let stem = rel.rsplit('/').next().unwrap().trim_end_matches(".xv");
        match a.scene(Some(stem)).unwrap_or_else(|e| panic!("{rel}: {e}")) {
            Some(scene) => {
                check(&scene, rel, &mut anim);
                counts_scenes += 1;
            }
            None => without_root.push(rel),
        }
    }
    eprintln!("archives/nodes/meshes/materials: {counts:?}; scenes {counts_scenes}; no root: {without_root:?}; anim {anim:?}");
    assert!(anim[0] > 0 && anim[1] == anim[0], "{anim:?}");
    assert!(anim[2] > 0 && anim[3] * 10 >= anim[2] * 9, "{anim:?}");
    assert_eq!(counts, [937, 1816, 14594, 16074]);
    assert!(without_root.len() <= 3, "{without_root:?}");
}

#[test]
fn model_textures_resolve() {
    use ig2_formats::xv::{Archive, Material};
    let Some(inst) = install() else { return };
    let (mut total, mut found, mut retried) = (0usize, 0usize, 0usize);
    for rel in inst.list("dataraw/vector/").into_iter().filter(|p| p.ends_with(".xv")) {
        let screen = rel["dataraw/vector/".len()..].split('/').next().unwrap();
        let bytes = inst.read(rel).unwrap();
        for m in Archive::parse(&bytes).unwrap().members.iter().filter(|m| m.name.to_ascii_lowercase().ends_with(".xvm")) {
            let mat = Material::parse(m.data).unwrap();
            if mat.texture.is_empty() {
                continue;
            }
            total += 1;
            if let Some(p) = inst.resolve_model_texture(screen, &mat.texture) {
                found += 1;
                retried += (p.ends_with("_bmt.pvr") || p.ends_with("_bmf.pvr")) as usize;
            }
        }
    }
    eprintln!("model textures: {found}/{total} resolve, {retried} through the .bmt/.bmf retry");
    assert!(found * 1000 >= total * 997, "{found}/{total}");
    // `D3DTexture::Load`'s retry: the game's own lookup finds `_bmt` textures.
    let t = inst.resolve_texture("textures/colony/", "shadowmap.bmp");
    assert_eq!(t.as_deref(), Some("dataraw/textures/colony/shadowmap_bmf.pvr"));
}

#[test]
fn localized_override_wins() {
    let Some(inst) = install() else { return };
    let hu = Install::open(inst.root(), Lang::Hu).unwrap();
    let en_path = inst.resolve("dataraw/text/txt01.txt").unwrap().to_owned();
    let hu_path = hu.resolve("DATARAW/TEXT/TXT01.TXT").unwrap();
    assert_ne!(en_path, hu_path);
    assert!(hu_path.to_string_lossy().replace('\\', "/").contains("localization/hu/"));
}

#[test]
fn every_ani_decodes_all_frames() {
    let Some(inst) = install() else { return };
    let files: Vec<&str> = inst.all_files().into_iter().filter(|p| p.ends_with(".ani")).collect();
    assert_eq!(files.len(), 14);
    for rel in files {
        let bytes = inst.read(rel).unwrap();
        let ani = ig2_formats::ani::Ani::parse(&bytes).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let frames = ani.frames().unwrap_or_else(|e| panic!("{rel}: {e}"));
        assert_eq!(frames.len(), ani.frame_count as usize, "{rel}");
    }
}

#[test]
fn string_tables_parse_in_every_language() {
    use ig2_formats::strings::{KeyedStrings, UiStrings, KEYED_TABLES};
    let Some(base) = install() else { return };
    for lang in Lang::ALL {
        let inst = Install::open(base.root(), lang).unwrap();
        let mut ui = UiStrings::default();
        for n in (1..=19).filter(|&n| n != 12) {
            let path = format!("dataraw/text/txt{n:02}.txt");
            ui.add_file(&ig2_formats::text::decode(&inst.read(&path).unwrap(), lang));
        }
        assert!(ui.rejected.is_empty(), "{lang:?}: {:#?}", &ui.rejected[..ui.rejected.len().min(5)]);
        assert!(ui.entries.len() > 1000, "{lang:?}: only {} UI strings", ui.entries.len());
        // Race names live at txt17 section 1 from index 6 (marked "do not translate", but the
        // French table translates them anyway: "Solarien").
        if lang == Lang::En {
            assert_eq!(ui.get(17, 1, 6), "Solarian");
        }
        assert!(!ui.get(17, 1, 6).is_empty(), "{lang:?}");
        let mut keyed = KeyedStrings::default();
        for (file, table) in KEYED_TABLES {
            keyed.add_file(table, &ig2_formats::text::decode(&inst.read(&format!("dataraw/text/{file}")).unwrap(), lang));
        }
        assert!(keyed.rejected.is_empty(), "{lang:?}: {:#?}", &keyed.rejected[..keyed.rejected.len().min(5)]);
        eprintln!("{lang:?}: {} UI strings ({} dup keys), {} keyed, race6={:?}", ui.entries.len(), ui.duplicates.len(), keyed.entries.len(), ui.get(17, 1, 6));
    }
}
