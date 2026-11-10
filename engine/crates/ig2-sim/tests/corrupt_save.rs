//! A damaged save that still parses (values overwritten inside its chunks) goes through what the client runs before
//! it loads one (`SaveState::from_save`, then the damage and mod checks of `ig2-client::world::check_kinds`) without
//! panicking. Saves: `IG2_SAVE_DIR`, else `_local/saves`; the tech table comes from the user's exe. Skips without them.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

use ig2_formats::save::SaveGame;
use ig2_formats::text::Lang;
use ig2_formats::Install;
use ig2_sim::save::SaveState;

#[test]
fn damaged_saves_dont_panic_the_state() {
    let Some(inst) = Install::from_env(Lang::En) else { return };
    let tech_defaults = ig2_rules::Rules::from_install(&inst.unwrap()).expect("catalogued exe").tech_defaults;
    let dir = std::env::var_os("IG2_SAVE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../_local/saves"));
    let mut fails = Vec::new();
    let mut checked = 0;
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "sav") {
            continue;
        }
        let Ok(save) = SaveGame::parse(&std::fs::read(&p).unwrap()) else { continue };
        let mut x = save.body.len() as u64 | 1;
        for k in 0..64 {
            let mut body = save.body.clone();
            for _ in 0..16 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                let at = x as usize % body.len();
                body[at] = (x >> 32) as u8;
            }
            let Ok(s) = SaveGame::parse_stream(body, false) else { continue };
            checked += 1;
            std::panic::set_hook(Box::new(|_| {}));
            let r = catch_unwind(AssertUnwindSafe(|| {
                let state = SaveState::from_save(&s, &tech_defaults);
                let _ = state.bad_count();
                let _ = state.unknown_kind(usize::MAX);
                let _ = state.unknown_hull(usize::MAX);
                let _ = state.unknown_race(usize::MAX);
                let _ = state.unknown_surface(usize::MAX);
            }));
            let _ = std::panic::take_hook();
            if let Err(e) = r {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
                fails.push(format!("{} damage {k}: {}", p.display(), msg.unwrap_or_default()));
            }
        }
    }
    eprintln!("{checked} damaged saves parsed");
    assert!(fails.is_empty(), "{}", fails.join("\n"));
}
