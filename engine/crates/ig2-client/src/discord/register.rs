//! The launch command Discord runs when a friend presses Join while the game isn't running (ADR 0080 §4): the
//! `discord-<app>` URL scheme, as Discord's own RPC library registers it. Windows: `HKCU\Software\Classes\discord-<app>`
//! (per user, no rights needed); Linux: a `discord-<app>.desktop` handler in `~/.local/share/applications`. The
//! game started this way connects, and Discord hands it the join secret.

use std::path::Path;

/// The `.desktop` file's text for `exe`.
#[cfg_attr(not(unix), allow(dead_code))]
pub fn desktop_entry(app: &str, exe: &Path) -> String {
    let exe = exe.display().to_string().replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "[Desktop Entry]\nName=openIG2\nComment=Imperium Galactica II on the openIG2 engine\nExec=\"{exe}\"\nType=Application\nNoDisplay=true\nCategories=Game;\nMimeType=x-scheme-handler/discord-{app};\n"
    )
}

/// The registry keys and values (`key`, value name or `None` for the default, data) for `exe`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn registry_values(app: &str, exe: &Path) -> Vec<(String, Option<&'static str>, String)> {
    let key = format!(r"HKCU\Software\Classes\discord-{app}");
    let exe = exe.display().to_string();
    vec![
        (key.clone(), None, format!("URL:Run game {app} protocol")),
        (key.clone(), Some("URL Protocol"), String::new()),
        (format!(r"{key}\DefaultIcon"), None, exe.clone()),
        (format!(r"{key}\shell\open\command"), None, format!("\"{exe}\"")),
    ]
}

/// Register this executable for `app` (quietly: a failure only means Discord can't start the game for a Join).
pub fn register(app: &str) {
    let Ok(exe) = std::env::current_exe() else { return };
    if let Err(e) = register_exe(app, &exe) {
        eprintln!("ig2-client: discord: launch command: {e}");
    }
}

#[cfg(windows)]
fn register_exe(app: &str, exe: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    /// `CREATE_NO_WINDOW`: no console flashes up.
    const NO_WINDOW: u32 = 0x0800_0000;
    let command = format!("\"{}\"", exe.display());
    // Already this executable: nothing to write.
    let have = Command::new("reg").args(["query", &format!(r"HKCU\Software\Classes\discord-{app}\shell\open\command"), "/ve"]).creation_flags(NO_WINDOW).stdin(Stdio::null()).stderr(Stdio::null()).output()?;
    if have.status.success() && String::from_utf8_lossy(&have.stdout).contains(&command) {
        return Ok(());
    }
    for (key, name, data) in registry_values(app, exe) {
        let mut c = Command::new("reg");
        c.args(["add", &key]);
        match name {
            Some(n) => c.args(["/v", n]),
            None => c.arg("/ve"),
        };
        let out = c.args(["/d", &data, "/f"]).creation_flags(NO_WINDOW).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status()?;
        if !out.success() {
            return Err(std::io::Error::other(format!("reg add {key} failed")));
        }
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn register_exe(app: &str, exe: &Path) -> std::io::Result<()> {
    let Some(data) = std::env::var_os("XDG_DATA_HOME").map(std::path::PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share"))) else {
        return Ok(());
    };
    let dir = data.join("applications");
    let name = format!("discord-{app}.desktop");
    let file = dir.join(&name);
    let text = desktop_entry(app, exe);
    if std::fs::read_to_string(&file).is_ok_and(|t| t == text) {
        return Ok(());
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::write(&file, text)?;
    // The scheme's default handler; without xdg-utils the MimeType line alone may do.
    let _ = std::process::Command::new("xdg-mime").args(["default", &name, &format!("x-scheme-handler/discord-{app}")]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    Ok(())
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
fn register_exe(_: &str, _: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_entries() {
        let d = desktop_entry("42", Path::new("/games/ig2/ig2-client"));
        assert!(d.contains("Exec=\"/games/ig2/ig2-client\"\n"));
        assert!(d.contains("MimeType=x-scheme-handler/discord-42;"));
        let r = registry_values("42", Path::new(r"C:\GOG\IG2\ig2-client.exe"));
        assert_eq!(r[0].0, r"HKCU\Software\Classes\discord-42");
        assert_eq!(r[1].1, Some("URL Protocol"));
        assert_eq!(r[3], (r"HKCU\Software\Classes\discord-42\shell\open\command".to_string(), None, r#""C:\GOG\IG2\ig2-client.exe""#.to_string()));
    }
}
