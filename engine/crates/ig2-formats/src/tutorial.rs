//! The interactive tutorial: `tutorial/main/script/main.tut` (command script) and
//! `tutorial/main/*.mou` (recorded mouse and keyboard input).
//!
//! Spec: `re/formats/text.md` §6. Evidence (ELF `ig2`, `tutorial.cpp`): `TutorialStart` @0x7b3d50
//! (loads `tutorial/main/script/<name>.tut` whole), `TutorialGetNextCommand` @0x7b4bc0 (the
//! interpreter), `TutorialPlayMouse` @0x7b3310 / `TutorialStopMouseSave` @0x7b30b0 (`.mou` I/O),
//! `Mouse_Tutorial_Kezel` @0x5b7230 and `ReadNext` @0x57f9b0 (record fields).

use crate::{format_err, Result};

/// The interpreter's keywords in the order it tests them. Each is a case-insensitive *prefix* match
/// against the text right after `<`, so the first hit wins (`<WAITFORMOUSEMOVE>` runs
/// `WAITFORMOUSE`). A tag that matches none stops the tutorial (`TutorialStop`).
pub const COMMANDS: [&str; 74] = [
    "AT", "END", "RETURNTOMAINMENU", "SEEKCHAPTER", "QUIT", "KURVAANYAD", "TYPE", "TIME", "PLAYWAV",
    "COUNT", "GIVE MONEY", "DISABLE HAJOELADAS", "DISABLE OVERRUN", "DISABLE STARMAP", "ENABLE STARMAP",
    "DOUBLEGOMB ON", "DOUBLEGOMB OFF", "DISABLE COLONY", "ENABLE COLONY", "DISABLE EXITGOMB",
    "ENABLE EXITGOMB", "DISABLE AI", "ENABLE AI", "DISABLE ROMBOLAS", "ENABLE ROMBOLAS",
    "DISABLE SPLITFLEET", "ENABLE SPLITFLEET", "DISABLE PRODUCTION", "ENABLE PRODUCTION",
    "DISABLE SCREEN", "ENABLE SCREEN", "DISABLE ALL SCREENS", "CENTERFLEET", "FIND FLEET",
    "FIND PLANET", "SHOW PLANET", "CENTERPLANET", "CENTERUNCOLONISEDPLANET", "ENABLE INVENTION",
    "DISABLE INVENTION", "ENABLE BUILDING", "DISABLE BUILDING", "DISABLE ALL BUILDINGS",
    "ENABLE ALL BUILDINGS", "DISABLE AUTOBUILD", "DISABLE TRADERS", "ENABLE TRADERS", "PLAYMOUSE",
    "SET MAIN BOLYGONEV", "WAITFORMOUSE", "WAITFORSCREEN", "WAITFORBATTLE", "WAITFORTEXT",
    "WAITFOREVENT", "LOADSCENARIO", "LOADSAVEGAME", "GAMESPEED", "CHAPTER", "WAITFORATLEPES",
    "DIPLOMACY EXPLORE EMPIRE", "EXPLORE EMPIRE", "GIVE KILLER FLEET", "GIVE KOLONIZALO FLEET", "PAUSE",
    "UNPAUSE", "IRRITATEME", "SPY MISSION", "ADD DESTROYERS TO TRADE", "DISABLE NEW SPIES",
    "DISABLE TEXTWINDOW", "GOTO SCREEN", "SEND MESSAGE", "RESTART", "TEXTURE",
];

/// One `<…>` tag of a tutorial script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    /// Byte offset of the `<`.
    pub offset: usize,
    /// The matched entry of [`COMMANDS`]; `None` = the game would stop the tutorial here.
    pub keyword: Option<&'static str>,
    /// Everything between `<` and the closing `>`.
    pub text: String,
    /// The quoted argument, if any (`TYPE`, `PLAYWAV`, `PLAYMOUSE`, `WAITFOREVENT`, `CHAPTER`, …).
    pub quoted: Option<String>,
}

impl Command {
    /// The decimal numbers after the keyword (`GetInt(args, n)`): `<AT 395,220>` → `[395, 220]`.
    pub fn ints(&self) -> Vec<i32> {
        let args = &self.text[self.keyword.map_or(0, str::len)..];
        let mut out = Vec::new();
        let mut cur: Option<(bool, i64)> = None;
        let mut prev = ' ';
        for c in args.chars().chain([' ']) {
            match (c.to_digit(10), &mut cur) {
                (Some(d), Some((_, v))) => *v = *v * 10 + d as i64,
                (Some(d), None) => cur = Some((prev == '-', d as i64)),
                (None, Some((neg, v))) => {
                    out.push(if *neg { -*v } else { *v } as i32);
                    cur = None;
                }
                (None, None) => {}
            }
            prev = c;
        }
        out
    }
}

/// Split a script into its tags the way `TutorialGetNextCommand` walks it: text outside `<…>` is
/// commentary (`;` lines, `###` notes, translator banners), `/* … */` blocks are skipped. A quoted
/// argument may contain `>`; the tag ends at the first `>` after it.
pub fn parse_script(text: &str) -> Vec<Command> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"/*") {
            i = text[i + 2..].find("*/").map_or(b.len(), |e| i + 2 + e + 2);
            continue;
        }
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let close = text[start..].find('>').map_or(b.len(), |e| start + e);
        let quote = text[start..close].find('"').map(|q| start + q);
        let (end, quoted) = match quote {
            Some(q) => {
                let qe = text[q + 1..].find('"').map_or(b.len(), |e| q + 1 + e);
                let end = text[qe.min(b.len())..].find('>').map_or(b.len(), |e| qe + e);
                (end, Some(text[q + 1..qe.min(b.len())].to_owned()))
            }
            None => (close, None),
        };
        let body = &text[start..end];
        let keyword = COMMANDS.iter().copied().find(|k| body.as_bytes().get(..k.len()).is_some_and(|p| p.eq_ignore_ascii_case(k.as_bytes())));
        out.push(Command { offset: i, keyword, text: body.to_owned(), quoted });
        i = end + 1;
    }
    out
}

/// One input sample of a `.mou` recording (one per `TutorialCounter` step).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseFrame {
    /// `Mouse_Akt_x/y`, in 640×480 screen coordinates.
    pub x: i32,
    pub y: i32,
    /// `Mouse_Akt_Gomb`, `Mouse_Akt_Bal`, `Mouse_Akt_Jobb` (bit 0 of each byte).
    pub button: bool,
    pub left: bool,
    pub right: bool,
    /// 1 = Alt, 2 = Shift, 4 = Ctrl.
    pub modifiers: u8,
    /// Up to 4 key presses `(virtual code, key)` delivered in this step (`ReadNext`); zero keys are
    /// empty slots.
    pub keys: Vec<(u8, u32)>,
}

/// `.mou`: `u32 count`, then per record 12 bytes (`i32 x, i32 y, u8 button, left, right,
/// modifiers`), a `u8` flag and, when the flag is 1, 20 key bytes (`u8 virt[4]`, `u32 key[4]`).
/// Record 0 is never written by the recorder (it increments before storing), so shipped files
/// start with a block of `0xCD` fill.
pub fn parse_mou(b: &[u8]) -> Result<Vec<MouseFrame>> {
    let err = |m: String| format_err("mou", m);
    let rd = |o: usize, n: usize| b.get(o..o + n).ok_or_else(|| err(format!("truncated at {o}")));
    let count = u32::from_le_bytes(rd(0, 4)?.try_into().unwrap()) as usize;
    let mut o = 4;
    let mut out = Vec::with_capacity(count.min(1 << 20));
    for _ in 0..count {
        let r = rd(o, 13)?;
        let i32_at = |k: usize| i32::from_le_bytes(r[k..k + 4].try_into().unwrap());
        let mut f = MouseFrame {
            x: i32_at(0),
            y: i32_at(4),
            button: r[8] & 1 != 0,
            left: r[9] & 1 != 0,
            right: r[10] & 1 != 0,
            modifiers: r[11],
            keys: Vec::new(),
        };
        o += 13;
        if r[12] == 1 {
            let k = rd(o, 20)?;
            f.keys = (0..4)
                .map(|j| (k[j], u32::from_le_bytes(k[4 + 4 * j..8 + 4 * j].try_into().unwrap())))
                .filter(|&(_, key)| key != 0)
                .collect();
            o += 20;
        }
        out.push(f);
    }
    if o != b.len() {
        return Err(err(format!("{} trailing bytes", b.len() - o)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_tags() {
        let s = "; c\r\n<SEEKCHAPTER>\r\n/* <END> */<AT 395,220>\r\n!! x\r\n<TYPE \"a > b\r\nc\">\r\n<WAITFORMOUSEMOVE><BOGUS>";
        let c = parse_script(s);
        let k: Vec<_> = c.iter().map(|c| c.keyword).collect();
        assert_eq!(k, [Some("SEEKCHAPTER"), Some("AT"), Some("TYPE"), Some("WAITFORMOUSE"), None]);
        assert_eq!(c[1].ints(), [395, 220]);
        assert_eq!(c[2].quoted.as_deref(), Some("a > b\r\nc"));
    }

    #[test]
    fn mouse_records() {
        let mut b = 2u32.to_le_bytes().to_vec();
        b.extend_from_slice(&[0xCD; 13]);
        b.extend_from_slice(&135i32.to_le_bytes());
        b.extend_from_slice(&441i32.to_le_bytes());
        b.extend_from_slice(&[0, 1, 0, 2, 1]);
        b.extend_from_slice(&[0x0d, 0, 0, 0]);
        b.extend_from_slice(&13u32.to_le_bytes());
        b.extend_from_slice(&[0; 12]);
        let f = parse_mou(&b).unwrap();
        assert_eq!(f.len(), 2);
        assert_eq!((f[1].x, f[1].y, f[1].left, f[1].modifiers), (135, 441, true, 2));
        assert_eq!(f[1].keys, [(0x0d, 13)]);
    }
}
