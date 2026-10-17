//! The language the GOG `ig2.exe` runs in (`re/formats/locale.md`).
//!
//! The remaster's `TextServer::loadTexts` (ELF 0x8f4c90) takes `TextServer::forcelanguage` (the `language=` line
//! of the install's `ig2.settings`, read at exe 0x73a24f) or else `PlatformUtils::GetSGLocaleInfo` (exe 0x6e8790:
//! `locale.txt` in `%LOCALAPPDATA%/GyroscopeGames/ig2/`, written by GOG's `language_setup.exe`, then `locale.txt`
//! next to the game, then exe 0x6f0b50 = `GetLocaleInfoA(LOCALE_SYSTEM_DEFAULT, LOCALE_SISO639LANGNAME)` +
//! `_` + the country), and keeps the part before the first `_` as `TextServer::language`. That language picks the
//! `localization/<lang>/` tree and the font's glyph order (exe 0x4a9f0f: Hungarian when it starts with `h`). The
//! 1999 code's own `Lang` global (exe 0x912724, from `init/config` or `data\XXX\text.dat`) stays 1 (ENG) on GOG
//! [obs: Frida on this host, no `locale.txt`, a Hungarian system locale: `localization/hu/`, `Lang` = 1].

use std::path::Path;

use crate::text::Lang;

/// Where the exe's choice came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// `ig2.settings`: `language=xx`.
    Settings,
    /// A `locale.txt` (the path).
    LocaleFile(std::path::PathBuf),
    /// The system's locale.
    System,
    /// Nothing usable: the base (English) data.
    Default,
}

/// The `language=` value of an `ig2.settings` text (`#` and `//` lines are comments).
pub fn settings_language(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#') && !l.starts_with("//"))
        .find_map(|l| l.split_once('=').filter(|(k, _)| k.trim() == "language").map(|(_, v)| v.trim().to_string()))
        .filter(|v| !v.is_empty())
}

/// `TextServer::language`: a locale id's part before the first `_` (`hu_HU` → `hu`).
pub fn language_of(locale_id: &str) -> &str {
    locale_id.split('_').next().unwrap_or("").trim()
}

/// The locale id the exe would read, and from where.
pub fn exe_locale_id(install_root: &Path) -> (Option<String>, Source) {
    let read = |p: &Path| std::fs::read_to_string(p).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if let Some(v) = read(&install_root.join("ig2.settings")).as_deref().and_then(settings_language) {
        return (Some(v), Source::Settings);
    }
    let local = std::env::var_os("LOCALAPPDATA").map(|d| Path::new(&d).join("GyroscopeGames/ig2/locale.txt"));
    for p in local.into_iter().chain([install_root.join("locale.txt")]) {
        if let Some(v) = read(&p) {
            return (Some(v), Source::LocaleFile(p));
        }
    }
    match system_language() {
        Some(v) => (Some(v), Source::System),
        None => (None, Source::Default),
    }
}

/// The language `ig2.exe` would run in with this install on this machine; English when it has no localization for
/// the locale (the asset rules then fall back to the base files).
pub fn exe_language(install_root: &Path) -> (Lang, Source) {
    match exe_locale_id(install_root) {
        (Some(id), src) => match Lang::from_code(language_of(&id)) {
            Some(l) => (l, src),
            None => (Lang::En, src),
        },
        (None, src) => (Lang::En, src),
    }
}

#[cfg(windows)]
fn system_language() -> Option<String> {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLocaleInfoA(locale: u32, lctype: u32, data: *mut u8, len: i32) -> i32;
    }
    const LOCALE_SYSTEM_DEFAULT: u32 = 0x800;
    const LOCALE_SISO639LANGNAME: u32 = 0x59;
    let mut buf = [0u8; 9];
    // SAFETY: the buffer and its length match; the call writes at most `len` bytes, NUL included.
    let n = unsafe { GetLocaleInfoA(LOCALE_SYSTEM_DEFAULT, LOCALE_SISO639LANGNAME, buf.as_mut_ptr(), buf.len() as i32) };
    (n > 1).then(|| String::from_utf8_lossy(&buf[..n as usize - 1]).into_owned())
}

/// The ELF build's locale: the POSIX locale variables (`hu_HU.UTF-8` → `hu_HU`).
#[cfg(not(windows))]
fn system_language() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty())
        .map(|v| v.split('.').next().unwrap_or("").to_string())
        .filter(|v| !v.is_empty() && v != "C" && v != "POSIX")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_override() {
        let gog = "#override language (en, it, fr, de, es, hu)\n//language=en\n\n#activate windowed mode\nwindow=1\n";
        assert_eq!(settings_language(gog), None);
        assert_eq!(settings_language("window=1\r\nlanguage = de\r\n").as_deref(), Some("de"));
    }

    #[test]
    fn locale_ids() {
        assert_eq!(language_of("hu_HU"), "hu");
        assert_eq!(language_of("hu"), "hu");
        assert_eq!(Lang::from_code(language_of("pl_PL")), None);
    }
}
