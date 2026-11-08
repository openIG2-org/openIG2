//! `ig2-check` (shipped as `openIG2-check`): loads every file of the user's Imperium Galactica II install through the
//! engine's loaders and reports what fails (like openOMSI's `omsi-check`). Ships and writes no game data.
//!
//! `openIG2-check [INSTALL_DIR] [--quick] [--mod NAME]...`: the install from the argument, else found as the client finds
//! it (`ig2_rules::install`). `--quick` skips decoding the music, sounds and videos (their containers are still read).
//! The enabled mods (`--mod`, else `mods/enabled.txt`, ADR 0044) are loaded over the install, so their files are
//! checked too, and listed with the install file each one replaces or the file it adds; each `rules.toml` and
//! `texts.<lang>.toml` is checked on its own.
//! Exit code 0 when everything loads, 1 when something fails, 2 when there is no install.
//!
//! Modder tools (ADR 0086 §5): `--dump-rules FILE` writes the install's rule tables (RON, the ids and values a
//! `rules.toml` names) and `--dump-texts FILE` every patchable text file's keys and lines (a `texts.<lang>.toml`),
//! instead of the checks. Both hold the game's data: for the player's own use, never to be shared.
//! `--package FOLDER` checks a mod folder and writes its zip, the zip's SHA-256 and its mod index entry (ADR 0086
//! §1.2, [`package`]); without an install (none given or found) it runs the format checks only.

use std::io::Read;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::time::Instant;

use ig2_formats::text::{self, Lang};
use ig2_formats::Install;

mod package;

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

impl Group {
    /// A check whose success leaves a note: the note and a pass, or the failure under `what`.
    fn record(&mut self, what: String, checked: Result<String, String>) {
        match checked {
            Ok(note) => {
                self.notes.push(note);
                self.ok += 1;
            }
            Err(e) => self.failed.push((what, e)),
        }
    }
}

/// The enabled mods (ADR 0044): the ones that didn't load, each mod's files, and each rules, text and story patch
/// on its own.
fn check_mods(inst: &Install, missing: Vec<(String, String)>, lang: Lang) -> Group {
    let mut g = Group::new("mods");
    for (n, why) in missing {
        g.check(&format!("mod {n}"), || Err(why));
    }
    let overrides = inst.overrides();
    for m in inst.mods() {
        let files: Vec<_> = overrides.iter().filter(|o| o.mod_name == m.name).collect();
        g.notes.push(format!("mod {} ({}): {} files", m.name, m.dir.display(), files.len()));
        g.notes.extend(files.iter().map(|o| format!("  {} {}", if o.replaces { "replaces" } else { "adds    " }, o.path)));
        g.ok += 1;
    }
    rule_patches(inst, &mut g);
    // Each text patch file on its own (ADR 0044 §12); together they were applied when the install opened.
    for (name, file, bytes) in inst.text_patches().unwrap_or_default() {
        let lang = ig2_formats::textpatch::file_lang(&file).unwrap_or(lang);
        let from = format!("mod {name}: {file}");
        let checked = utf8(bytes).and_then(|t| ig2_formats::textpatch::TextPatch::default().add(&t, &from, lang));
        g.record(from.clone(), checked.map(|n| format!("{from} sets {n} lines")));
    }
    story_files(inst, &mut g);
    g
}

fn utf8(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes).map_err(|_| "not UTF-8 text".to_string())
}

/// Each mod's `rules.toml` on its own against the exe's tables (ADR 0044 §8), with where its added entries landed,
/// then all of them together in load order (a mod's entries may name an earlier mod's keys, ADR 0069).
fn rule_patches(inst: &Install, g: &mut Group) {
    let exe = ig2_rules::Rules::from_exe(&inst.read_raw("ig2.exe").unwrap_or_default());
    let patches = inst.rule_patches().unwrap_or_default();
    let several = patches.len() > 1;
    for (name, bytes) in patches {
        let applied = || -> Result<(usize, ig2_rules::Rules), String> {
            let mut r = exe.as_ref().map_err(|e| e.to_string())?.clone();
            let n = r.apply_patch(&utf8(bytes)?)?;
            Ok((n, r))
        };
        let checked = applied().map(|(n, r)| {
            let mut lines = vec![format!("mod {name}: rules.toml sets {n} fields")];
            lines.extend(r.added_techs.iter().map(|a| format!("  adds tech {} \"{}\" (category {}, field {})", a.index, a.key, (a.index - 1) / ig2_rules::TECH_FIELDS + 1, (a.index - 1) % ig2_rules::TECH_FIELDS + 1)));
            lines.extend(r.added_buildings.iter().map(|a| format!("  adds building kind {} \"{}\" (like {})", a.kind, a.key, r.buildings[a.kind].like)));
            lines.extend(r.added_hulls.iter().map(|a| format!("  adds hull {} \"{}\" (like {}, tech {})", a.index, a.key, r.hulls[a.index].like, a.tech)));
            lines.extend(r.added_weapons.iter().map(|a| format!("  adds space weapon {} \"{}\" (tech {})", a.index, a.key, a.tech)));
            lines.extend(r.added_races.iter().map(|a| format!("  adds race {} \"{}\" (like {}, AI like {})", a.race, a.key, a.like, a.ai)));
            lines.extend(r.added_planet_types.iter().map(|a| format!("  adds planet type {} \"{}\" (like {}, {}% of a new game's)", a.surface, a.key, a.like, a.share)));
            lines.extend(r.added_techs.iter().filter(|a| r.techs[a.index].like != 0).map(|a| format!("  tech {} \"{}\" is like tech {}", a.index, a.key, r.techs[a.index].like)));
            lines.join("\n")
        });
        match checked {
            Ok(note) => {
                g.notes.extend(note.lines().map(str::to_string));
                g.ok += 1;
            }
            Err(e) => g.failed.push((format!("mod {name}: rules.toml"), e)),
        }
    }
    if several {
        g.check("mods: every rules.toml together", || ig2_rules::Rules::from_install(inst).map(|_| ()).map_err(|e| e.to_string()));
    }
}

/// Each story file on its own (ADR 0061 §7), and the messages its stories send: in `uzenetek.txt`'s index, with
/// their text file.
fn story_files(inst: &Install, g: &mut Group) {
    let index = inst.read("dataraw/uzenetek/uzenetek.txt").map(|b| ig2_formats::dialogue::parse_message_index(&String::from_utf8_lossy(&b))).unwrap_or_default();
    for (name, bytes) in inst.story_files().unwrap_or_default() {
        let from = format!("mod {name}: {}", ig2_formats::vfs::STORY_FILE);
        let checked = utf8(bytes).and_then(|t| ig2_sim::story_mods::parse(&t, name)).and_then(|stories| {
            let missing: Vec<String> = stories
                .iter()
                .flat_map(|s| s.actions.iter())
                .filter_map(|a| match a {
                    &ig2_sim::story_mods::Action::Message(id) => Some(id),
                    _ => None,
                })
                .filter(|&id| !index.iter().any(|k| k.id as i32 == id) || inst.resolve(&format!("dataraw/uzenetek/txt{id:04}.txt")).is_none())
                .map(|id| id.to_string())
                .collect();
            if missing.is_empty() { Ok(stories.len()) } else { Err(format!("messages not in uzenetek.txt or without their txtNNNN.txt: {}", missing.join(", "))) }
        });
        g.record(from.clone(), checked.map(|n| format!("{from} holds {n} stories")));
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
        let li = match inst.with_lang(lang) {
            Ok(li) => li,
            Err(e) => {
                g.failed.push((format!("string tables ({dir})"), e.to_string()));
                continue;
            }
        };
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

const HELP: &str = "\
openIG2-check [INSTALL_DIR] [--quick] [--mod NAME]...
    loads every file of your Imperium Galactica II install and its mods and reports what fails
openIG2-check [INSTALL_DIR] --package FOLDER
    checks the mod in FOLDER and writes <id>-<version>.zip, its .sha256 and <id>-<version>.toml (the mod index
    entry; fill in its download URL) here; without an install, only the checks that need none
openIG2-check [INSTALL_DIR] [--dump-rules FILE] [--dump-texts FILE]
    modder tools: writes your own install's rule ids and values (RON) or every text file's keys and lines (in the
    form of a mod's texts.<lang>.toml) to FILE. The files hold the game's data: they are for your own use and must
    never be shared or put in a mod.";

/// Every patchable text file of `inst` (its language), keys and lines, to `out` as a `texts.<lang>.toml`.
fn write_texts(inst: &Install, out: &std::path::Path) -> Result<String, String> {
    use ig2_formats::textpatch;
    let files: Vec<(&str, String)> = inst
        .all_files()
        .into_iter()
        .filter(|p| textpatch::kind(p).is_some())
        .map(|p| decoded(inst, p).map(|t| (p, t)).map_err(|e| format!("{p}: {e}")))
        .collect::<Result<_, _>>()?;
    let name = textpatch::file_name(inst.lang());
    let head = format!(
        "# {name}: the keys and lines of your own install's texts (openIG2-check --dump-texts).\n\
         # They are the game's data: for your own use, never share this file or put it in a mod; copy only the keys you\n\
         # change into your mod's {name} and write your own lines.\n\n"
    );
    let body = textpatch::dump(files.iter().map(|(p, t)| (*p, t.as_str())));
    std::fs::write(out, head + &body).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(format!("wrote {}: {} text files ({name})", out.display(), files.len()))
}

/// The install as the client finds it (`ig2_rules::install`), or where it looked.
fn found_install() -> Result<PathBuf, Vec<(ig2_rules::install::Candidate, ig2_rules::install::Verdict)>> {
    let file = ig2_formats::options::Options::load();
    let remembered = file.get(ig2_rules::install::SETTINGS_KEY).filter(|s| !s.is_empty()).map(PathBuf::from);
    ig2_rules::install::find(remembered.as_deref()).map(|c| c.path)
}

/// `OPENIG2_VERSION` at build time (the release's MAJOR.MINOR.N), else the repository's `VERSION` with `-dev`.
fn version() -> String {
    option_env!("OPENIG2_VERSION").map_or_else(|| format!("{}-dev", include_str!("../../../../VERSION").trim()), str::to_owned)
}

fn main() {
    let mut root = None;
    let mut quick = false;
    let mut given: Option<Vec<String>> = None;
    let (mut dump_rules, mut dump_texts, mut pack) = (None, None, None);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--quick" => quick = true,
            "--mod" => given.get_or_insert_with(Vec::new).extend(args.next()),
            "--dump-rules" | "--dump-texts" | "--package" => {
                let Some(file) = args.next() else {
                    eprintln!("openIG2-check: {a} needs a file or folder name");
                    std::process::exit(2);
                };
                *match a.as_str() {
                    "--dump-rules" => &mut dump_rules,
                    "--dump-texts" => &mut dump_texts,
                    _ => &mut pack,
                } = Some(PathBuf::from(file));
            }
            "--help" | "-h" => {
                println!("{HELP}");
                return;
            }
            _ => root = Some(PathBuf::from(a)),
        }
    }
    if let Some(folder) = pack {
        let install = root.or_else(|| found_install().ok()).filter(|r| matches!(ig2_rules::install::check(r), ig2_rules::install::Verdict::Ok | ig2_rules::install::Verdict::WrongBuild(_)));
        println!("openIG2-check {}: packaging {}", version(), folder.display());
        std::panic::set_hook(Box::new(|_| {}));
        match catch_unwind(AssertUnwindSafe(|| package::package(&folder, install.as_deref(), Path::new(".")))) {
            Ok(Ok(p)) => {
                p.notes.iter().for_each(|n| println!("    {n}"));
                println!("wrote {} ({} bytes, SHA-256 {}), {}.sha256 and {}", p.zip.display(), p.size, p.sha256, p.zip.display(), p.entry.display());
            }
            Ok(Err(e)) => {
                eprintln!("openIG2-check: {} can't be packaged:", folder.display());
                e.lines().for_each(|l| eprintln!("    {l}"));
                std::process::exit(1);
            }
            Err(_) => {
                eprintln!("openIG2-check: a loader panicked while checking {}", folder.display());
                std::process::exit(1);
            }
        }
        return;
    }
    let root = match root {
        Some(r) => r,
        None => match found_install() {
            Ok(r) => r,
            Err(tried) => {
                eprintln!("openIG2-check: no Imperium Galactica II install found; give its folder as the argument. Looked in:");
                for (c, v) in tried.iter().filter(|(_, v)| *v != ig2_rules::install::Verdict::Missing) {
                    eprintln!("  {} ({}): {}", c.path.display(), c.source.label(), v.describe());
                }
                std::process::exit(2);
            }
        },
    };
    println!("openIG2-check {}: {}", version(), root.display());
    let verdict = ig2_rules::install::check(&root);
    if !matches!(verdict, ig2_rules::install::Verdict::Ok | ig2_rules::install::Verdict::WrongBuild(_)) {
        eprintln!("openIG2-check: {}: {}", root.display(), verdict.describe());
        std::process::exit(2);
    }
    let lang = ig2_formats::locale::exe_language(&root).0;
    if dump_rules.is_some() || dump_texts.is_some() {
        let done = Install::open(&root, lang).map_err(|e| e.to_string()).and_then(|inst| {
            let mut said = Vec::new();
            if let Some(f) = &dump_rules {
                said.push(ig2_rules::Rules::dump(&inst, f)?);
            }
            if let Some(f) = &dump_texts {
                said.push(write_texts(&inst, f)?);
            }
            Ok(said)
        });
        match done {
            Ok(said) => {
                said.iter().for_each(|s| println!("{s}"));
                println!("these files hold your game's data: keep them for your own use, never share them");
            }
            Err(e) => {
                eprintln!("openIG2-check: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let (mods, missing) = ig2_formats::vfs::Mod::enabled(given);
    let inst = match Install::open_with_mods(&root, lang, mods) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("openIG2-check: {e}");
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
    if !inst.mods().is_empty() || !missing.is_empty() {
        let g = check_mods(&inst, missing, lang);
        report(&g, Instant::now());
        groups.push(g);
    }
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

    /// The dumps of the install in `IG2_INSTALL_DIR` read back: the texts as a patch, the rules as RON.
    #[test]
    fn the_dumps_read_back() {
        let Some(root) = std::env::var_os("IG2_INSTALL_DIR").map(PathBuf::from).filter(|r| r.join("ig2.exe").is_file()) else {
            eprintln!("IG2_INSTALL_DIR not set: skipped");
            return;
        };
        let inst = Install::open(&root, Lang::En).unwrap();
        let dir = std::env::temp_dir().join(format!("openig2-check-dump-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let said = write_texts(&inst, &dir.join("texts.toml")).unwrap();
        let toml = std::fs::read_to_string(dir.join("texts.toml")).unwrap();
        let n = ig2_formats::textpatch::TextPatch::default().add(&toml, "dump", Lang::En).unwrap();
        assert!(n > 1000 && toml.contains("[\"dataraw/text/s_races.txt\"]"), "{said}: {n} keys");
        ig2_rules::Rules::dump(&inst, &dir.join("rules.ron")).unwrap();
        assert!(std::fs::read_to_string(dir.join("rules.ron")).unwrap().contains("techs"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
