//! `ig2-check`: loads every file of the user's Imperium Galactica II install through the engine's loaders and
//! reports what fails (like openOMSI's `omsi-check`). Ships and writes no game data.
//!
//! `ig2-check [INSTALL_DIR] [--quick]`: the install from the argument, else found as the client finds it
//! (`ig2_rules::install`). `--quick` skips decoding the music, sounds and videos (their containers are still read).
//! Exit code 0 when everything loads, 1 when something fails, 2 when there is no install.

use std::io::Read;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::time::Instant;

use ig2_formats::text::{self, Lang};
use ig2_formats::Install;

/// One kind of file and how its files fared.
struct Group {
    name: &'static str,
    ok: usize,
    failed: Vec<(String, String)>,
    notes: Vec<String>,
}

impl Group {
    fn new(name: &'static str) -> Self {
        Group { name, ok: 0, failed: Vec::new(), notes: Vec::new() }
    }

    /// Run one check; a panic in a loader counts as a failure, not a crash.
    fn check(&mut self, what: &str, f: impl FnOnce() -> Result<(), String>) {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(Ok(())) => self.ok += 1,
            Ok(Err(e)) => self.failed.push((what.to_string(), e)),
            Err(p) => {
                let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                self.failed.push((what.to_string(), format!("loader panicked: {msg}")));
            }
        }
    }
}

/// One group's loader pass.
type Step = dyn Fn(&Install, &mut Group);

fn nonempty<T>(v: &[T], what: &str) -> Result<(), String> {
    if v.is_empty() { Err(format!("no {what}")) } else { Ok(()) }
}

fn files<'a>(inst: &'a Install, ext: &str) -> Vec<&'a str> {
    inst.all_files().into_iter().filter(|p| p.ends_with(ext)).collect()
}

fn decoded(inst: &Install, rel: &str) -> Result<String, String> {
    inst.read(rel).map(|b| text::decode(&b, inst.lang())).map_err(|e| e.to_string())
}

fn exe_tables(inst: &Install, g: &mut Group) {
    g.check("tables (ig2_rules::Rules)", || ig2_rules::Rules::from_install(inst).map(|_| ()));
    let exe = inst.read_raw("ig2.exe").unwrap_or_default();
    g.check("sound tables", || ig2_rules::sounds::SoundTables::from_exe(&exe).map(|_| ()).map_err(|e| e.to_string()));
    g.check("speech tables", || ig2_rules::speech::Speech::from_exe(&exe).map(|_| ()).map_err(|e| e.to_string()));
}

fn containers(inst: &Install, g: &mut Group) {
    for rel in inst.all_files() {
        if matches!(rel.rsplit('.').next(), Some("mp4" | "ogg")) {
            continue; // Streamed: the media groups read them.
        }
        g.check(rel, || {
            let raw = inst.read_raw(rel).map_err(|e| e.to_string())?;
            ig2_formats::container::decode(rel, &raw).map(|_| ()).map_err(|e| e.to_string())
        });
    }
}

fn textures(inst: &Install, g: &mut Group) {
    for rel in files(inst, ".pvr") {
        g.check(rel, || {
            let b = inst.read(rel).map_err(|e| e.to_string())?;
            let pvr = ig2_formats::pvr::Pvr::parse(&b).map_err(|e| e.to_string())?;
            pvr.decode_rgba8().map(|_| ()).map_err(|e| e.to_string())
        });
    }
}

fn models(inst: &Install, g: &mut Group) {
    use ig2_formats::xv::{Archive, Material, Mesh, NodeHeader};
    let (mut textures, mut missing) = (0usize, Vec::new());
    for rel in files(inst, ".xv") {
        g.check(rel, || {
            let b = inst.read(rel).map_err(|e| e.to_string())?;
            let a = Archive::parse(&b).map_err(|e| e.to_string())?;
            let screen = rel.strip_prefix("dataraw/vector/").and_then(|r| r.split('/').next());
            for m in &a.members {
                let lower = m.name.to_ascii_lowercase();
                let err = |e: ig2_formats::Error| format!("{}: {e}", m.name);
                if lower.ends_with(".xvb") {
                    NodeHeader::parse(m.data).map_err(err)?;
                } else if lower.ends_with(".xvo") {
                    Mesh::parse(m.data).map_err(err)?;
                } else if lower.ends_with(".xvm") {
                    let mat = Material::parse(m.data).map_err(err)?;
                    if let (Some(s), false) = (screen, mat.texture.is_empty()) {
                        textures += 1;
                        if inst.resolve_model_texture(s, &mat.texture).is_none() {
                            missing.push(format!("{rel}: {}", mat.texture));
                        }
                    }
                }
            }
            let stem = rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".xv");
            a.scene(Some(stem)).map(|_| ()).map_err(|e| e.to_string())
        });
    }
    // The original ships a few materials naming textures it lacks (it draws them untextured, as ours does).
    g.notes.push(format!("model textures: {}/{textures} found", textures - missing.len()));
    if missing.len() * 100 > textures.max(1) {
        g.failed.push(("model textures".into(), format!("{} missing, e.g. {}", missing.len(), missing[..missing.len().min(3)].join("; "))));
    }
}

fn animations(inst: &Install, g: &mut Group) {
    for rel in files(inst, ".ani") {
        g.check(rel, || {
            let b = inst.read(rel).map_err(|e| e.to_string())?;
            let a = ig2_formats::ani::Ani::parse(&b).map_err(|e| e.to_string())?;
            let f = a.frames().map_err(|e| e.to_string())?;
            if f.len() != a.frame_count as usize { Err(format!("{} of {} frames", f.len(), a.frame_count)) } else { Ok(()) }
        });
    }
}

fn scenarios(inst: &Install, g: &mut Group) {
    for rel in files(inst, ".scn") {
        g.check(rel, || {
            let s = ig2_formats::scn::Scenario::parse_bytes(&inst.read(rel).map_err(|e| e.to_string())?, inst.lang());
            s.global("Title").map(|_| ()).ok_or_else(|| "no Title".to_string())
        });
    }
    for rel in files(inst, ".dsc") {
        g.check(rel, || decoded(inst, rel).map(|t| drop(ig2_formats::misc_text::parse_dsc(&t))));
    }
}

fn colony(inst: &Install, g: &mut Group) {
    let maps = inst.list("dataraw/colony/gen_maps/");
    for rel in &maps {
        g.check(rel, || ig2_formats::colony_map::ColonyMap::parse(&inst.read(rel).map_err(|e| e.to_string())?).map(|_| ()).map_err(|e| e.to_string()));
    }
    g.check("dataraw/colony/terepleiro.txt", || {
        let s = ig2_formats::colony_map::parse_surfaces(&decoded(inst, "dataraw/colony/terepleiro.txt")?).map_err(|e| e.to_string())?;
        if s.len() == maps.len() { Ok(()) } else { Err(format!("{} surfaces for {} maps", s.len(), maps.len())) }
    });
    for rel in inst.list("dataraw/spacewar/").into_iter().filter(|p| p.ends_with(".bin")) {
        g.check(rel, || ig2_formats::formation::parse(&inst.read(rel).map_err(|e| e.to_string())?).map(|_| ()).map_err(|e| e.to_string()));
    }
}

fn texts(inst: &Install, g: &mut Group) {
    use ig2_formats::strings::{KeyedStrings, UiStrings, KEYED_TABLES};
    for lang in Lang::ALL {
        let Some(dir) = lang.dir() else { continue };
        if lang != Lang::En && !inst.root().join("localization").join(dir).is_dir() {
            continue;
        }
        let Ok(li) = Install::open(inst.root(), lang) else { continue };
        g.check(&format!("string tables ({dir})"), || {
            let mut ui = UiStrings::default();
            for n in (1..=19).filter(|&n| n != 12) {
                let p = format!("dataraw/text/txt{n:02}.txt");
                ui.add_file(&text::decode(&li.read(&p).map_err(|e| format!("{p}: {e}"))?, lang));
            }
            let mut keyed = KeyedStrings::default();
            for (file, table) in KEYED_TABLES {
                let p = format!("dataraw/text/{file}");
                keyed.add_file(table, &text::decode(&li.read(&p).map_err(|e| format!("{p}: {e}"))?, lang));
            }
            match (ui.rejected.first(), keyed.rejected.first()) {
                (None, None) => Ok(()),
                (r, k) => Err(format!("{} + {} lines rejected, first {:?}", ui.rejected.len(), keyed.rejected.len(), r.map(|r| format!("{r:?}")).or(k.map(|k| format!("{k:?}"))))),
            }
        });
    }
    use ig2_formats::{misc_text as m, story_data as s};
    let t = |p: &str| decoded(inst, &format!("dataraw/text/{p}"));
    g.check("dataraw/text/building.txt", || nonempty(&s::parse_buildings(&t("building.txt")?), "buildings"));
    g.check("dataraw/text/designs.txt", || t("designs.txt").map(|x| drop(s::parse_designs(&x))));
    g.check("dataraw/text/values.txt", || nonempty(&s::parse_values(&t("values.txt")?), "values"));
    g.check("dataraw/text/fleet.txt", || t("fleet.txt").map(|x| drop(s::parse_fleets(&x))));
    g.check("dataraw/text/infostr.txt", || nonempty(&m::parse_infostr(&t("infostr.txt")?).0, "lines"));
    g.check("dataraw/text/res_info.txt", || nonempty(&m::parse_res_info(&t("res_info.txt")?), "techs"));
    g.check("dataraw/text/fajinfo.txt", || t("fajinfo.txt").map(|x| drop(m::parse_fajinfo(&x))));
    g.check("dataraw/text/gameinfo.txt", || nonempty(&m::parse_gameinfo(&t("gameinfo.txt")?), "lines"));
    for rel in inst.list("dataraw/text/autobld") {
        g.check(rel, || decoded(inst, rel).map(|x| drop(m::parse_autobuild(&x))));
    }
}

fn messages(inst: &Install, g: &mut Group) {
    use ig2_formats::dialogue as d;
    g.check("dataraw/uzenetek/uzenetek.txt", || nonempty(&d::parse_message_index(&decoded(inst, "dataraw/uzenetek/uzenetek.txt")?), "messages"));
    for rel in inst.list("dataraw/uzenetek/") {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        if name.starts_with("msg") {
            g.check(rel, || nonempty(&d::parse_msg_script(&decoded(inst, rel)?), "blocks"));
        } else if name.starts_with("txt") {
            g.check(rel, || nonempty(&d::parse_txt_script(&decoded(inst, rel)?), "text parts"));
        }
    }
    for rel in inst.list("dataraw/text/uzenetanim_") {
        g.check(rel, || nonempty(&d::parse_clip_lists(&decoded(inst, rel)?), "clip groups"));
    }
    for f in ["ajanlat.eng", "valaszok.eng"] {
        let rel = format!("dataraw/diplomac/{f}");
        g.check(&rel, || nonempty(&ig2_formats::diplomacy::PhraseFile::parse(&decoded(inst, &rel)?).phrases, "phrases"));
    }
    g.check("dataraw/diplomac/special.eng", || nonempty(&ig2_formats::diplomacy::parse_specials(&decoded(inst, "dataraw/diplomac/special.eng")?), "specials"));
    for rel in files(inst, ".tut") {
        g.check(rel, || nonempty(&ig2_formats::tutorial::parse_script(&decoded(inst, rel)?), "commands"));
    }
    for rel in files(inst, ".mou") {
        g.check(rel, || ig2_formats::tutorial::parse_mou(&inst.read(rel).map_err(|e| e.to_string())?).map(|_| ()).map_err(|e| e.to_string()));
    }
}

/// Every `.ogg`: its Vorbis headers and first packet (all of it unless `quick`: only that the container opens).
fn audio(inst: &Install, g: &mut Group, quick: bool) {
    for rel in files(inst, ".ogg") {
        g.check(rel, || {
            let s = inst.open_stream(rel).map_err(|e| e.to_string())?;
            if quick {
                let mut head = [0u8; 4];
                return s.take(4).read_exact(&mut head).map_err(|e| e.to_string()).and_then(|_| if &head == b"OggS" { Ok(()) } else { Err("not Ogg".into()) });
            }
            let mut ogg = lewton::inside_ogg::OggStreamReader::new(s).map_err(|e| e.to_string())?;
            ogg.read_dec_packet_itl().map(|_| ()).map_err(|e| e.to_string())
        });
    }
}

/// Every `.mp4`: the ISO-BMFF tables, the H.264 track, and (unless `quick`) its first frame decoded.
fn videos(inst: &Install, g: &mut Group, quick: bool) {
    use ig2_formats::mp4::{self, Codec};
    let mut late = Vec::new();
    for rel in files(inst, ".mp4") {
        g.check(rel, || {
            let data = inst.read(rel).map_err(|e| e.to_string())?;
            let clip = mp4::parse(&data).map_err(|e| e.to_string())?;
            let v = clip.video.as_ref().ok_or("no video track")?;
            let Codec::Avc { sps, pps, nal_length_size, .. } = &v.codec else { return Err("video not H.264".into()) };
            if quick {
                return nonempty(&v.samples, "video samples");
            }
            let cfg = openh264::decoder::DecoderConfig::new().flush_after_decode(openh264::decoder::Flush::NoFlush);
            let mut dec = openh264::decoder::Decoder::with_api_config(openh264::OpenH264API::from_source(), cfg).map_err(|e| e.to_string())?;
            // As the client plays it (`video::decode`): a frame the decoder rejects is skipped; the clip fails when no
            // sample gives a picture, and a clip whose first pictures don't decode is noted (it starts late in the
            // client).
            let mut annexb = mp4::avc_parameter_sets(sps, pps);
            let mut last = String::from("no frame decoded");
            for (n, s) in v.samples.iter().enumerate() {
                mp4::avc_to_annexb(v.data(&data, s).map_err(|e| e.to_string())?, *nal_length_size, &mut annexb).map_err(|e| e.to_string())?;
                match dec.decode(&annexb) {
                    Ok(Some(_)) => {
                        if n > 8 {
                            late.push(format!("{rel}: the first picture decodes at frame {n} ({last})"));
                        }
                        return Ok(());
                    }
                    Ok(None) => {}
                    Err(e) => last = e.to_string(),
                }
                annexb.clear();
            }
            if dec.flush_remaining().is_ok_and(|f| !f.is_empty()) { Ok(()) } else { Err(last) }
        });
    }
    g.notes.extend(late);
}

/// `OPENIG2_VERSION` at build time (the release's MAJOR.MINOR.N), else the repository's `VERSION` with `-dev`.
fn version() -> String {
    option_env!("OPENIG2_VERSION").map_or_else(|| format!("{}-dev", include_str!("../../../../VERSION").trim()), str::to_owned)
}

fn main() {
    let mut root = None;
    let mut quick = false;
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--quick" => quick = true,
            "--help" | "-h" => {
                println!("ig2-check [INSTALL_DIR] [--quick]: loads every file of your Imperium Galactica II install");
                return;
            }
            _ => root = Some(PathBuf::from(a)),
        }
    }
    let root = match root {
        Some(r) => r,
        None => {
            let file = ig2_formats::options::Options::load();
            let remembered = file.get(ig2_rules::install::SETTINGS_KEY).filter(|s| !s.is_empty()).map(PathBuf::from);
            match ig2_rules::install::find(remembered.as_deref()) {
                Ok(c) => c.path,
                Err(tried) => {
                    eprintln!("ig2-check: no Imperium Galactica II install found; give its folder as the argument. Looked in:");
                    for (c, v) in tried.iter().filter(|(_, v)| *v != ig2_rules::install::Verdict::Missing) {
                        eprintln!("  {} ({}): {}", c.path.display(), c.source.label(), v.describe());
                    }
                    std::process::exit(2);
                }
            }
        }
    };
    println!("ig2-check {}: {}", version(), root.display());
    let verdict = ig2_rules::install::check(&root);
    if !matches!(verdict, ig2_rules::install::Verdict::Ok | ig2_rules::install::Verdict::WrongBuild(_)) {
        eprintln!("ig2-check: {}: {}", root.display(), verdict.describe());
        std::process::exit(2);
    }
    let lang = ig2_formats::locale::exe_language(&root).0;
    let inst = match Install::open(&root, lang) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("ig2-check: {e}");
            std::process::exit(2);
        }
    };
    // A loader's panic is reported as a failure; keep the default hook from printing it in the middle.
    std::panic::set_hook(Box::new(|_| {}));
    let mut groups = Vec::new();
    let mut install = Group::new("install");
    install.check("ig2.exe build", || match &verdict {
        ig2_rules::install::Verdict::Ok => Ok(()),
        v => Err(v.describe()),
    });
    report(&install, Instant::now());
    groups.push(install);
    let steps: [(&'static str, &Step); 9] = [
        ("exe tables", &exe_tables),
        ("containers", &containers),
        ("textures", &textures),
        ("models", &models),
        ("animations", &animations),
        ("scenarios", &scenarios),
        ("colony and battle maps", &colony),
        ("texts and story data", &texts),
        ("messages, diplomacy, tutorial", &messages),
    ];
    for (name, f) in steps {
        let t = Instant::now();
        let mut g = Group::new(name);
        f(&inst, &mut g);
        report(&g, t);
        groups.push(g);
    }
    for (name, f) in [("music and sounds", audio as fn(&Install, &mut Group, bool)), ("videos", videos)] {
        let t = Instant::now();
        let mut g = Group::new(name);
        f(&inst, &mut g, quick);
        report(&g, t);
        groups.push(g);
    }
    let failed: usize = groups.iter().map(|g| g.failed.len()).sum();
    let ok: usize = groups.iter().map(|g| g.ok).sum();
    if failed == 0 {
        println!("all {ok} checks passed");
    } else {
        println!("{failed} of {} checks failed", ok + failed);
        std::process::exit(1);
    }
}

fn report(g: &Group, t: Instant) {
    let status = if g.failed.is_empty() { "ok".to_string() } else { format!("{} FAILED", g.failed.len()) };
    println!("{:<32} {:>6} ok  {status}  ({:.1} s)", g.name, g.ok, t.elapsed().as_secs_f32());
    for n in &g.notes {
        println!("    {n}");
    }
    for (what, e) in &g.failed {
        println!("    {what}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loader_panic_is_a_failure() {
        std::panic::set_hook(Box::new(|_| {}));
        let mut g = Group::new("t");
        g.check("a", || Ok(()));
        g.check("b", || Err("bad".into()));
        g.check("c", || panic!("boom"));
        assert_eq!(g.ok, 1);
        assert_eq!(g.failed, [("b".to_string(), "bad".to_string()), ("c".to_string(), "loader panicked: boom".to_string())]);
    }
}
