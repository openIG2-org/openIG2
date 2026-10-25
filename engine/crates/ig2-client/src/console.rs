//! `ExecuteString`'s multiplayer commands (exe 0x4dc140, re/screens/input.md §5.1): the chat entry's line is the host's
//! `/kick` or `/kill`, else a chat line. `/clearlog` goes out as chat (the packet dumps it truncates are off). The
//! debug cheats (`DEBUGLEHET`/`DEBUGX`) are not ported (ADR 0016).

/// What a line does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line<'a> {
    Chat,
    /// The host's `/kick ` / `/kill ` (`_strnicmp(.., 6)`): the argument, its leading spaces skipped.
    Kick(&'a [u8]),
}

/// Stage A (`MultiPlayerMode`): `host` = `SuperVisor`; a joiner's `/kick` is a chat line.
pub fn line(text: &[u8], host: bool) -> Line<'_> {
    let kick = text.len() >= 6 && (text[..6].eq_ignore_ascii_case(b"/kick ") || text[..6].eq_ignore_ascii_case(b"/kill "));
    if !(kick && host) {
        return Line::Chat;
    }
    let n = text[6..].iter().take_while(|&&c| c == b' ').count();
    Line::Kick(&text[6 + n..])
}

/// A human still playing (`PlayerKi[e] == 1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub empire: u8,
    /// `PlayerNev[e]`.
    pub name: Vec<u8>,
    /// `Faj_Nev[e]`: the race name list indexed by the empire number (`GetString(0x11, 1, e + 5)`).
    pub race: Vec<u8>,
}

/// The local red lines (`AddSzoveg(.., 6)`).
pub const NO_SUCH_PLAYER: &[u8] = b"No such player.";
pub const CANNOT_KICK_YOURSELF: &[u8] = b"Cannot kick yourself.";
/// `AddSzoveg`'s red.
pub const RED: usize = 6;

/// The kick's target (exe 0x4dc1e1..0x4dc333): a first byte `1`..`8` is the empire; else the first player (empire
/// order) whose race name is the argument (`_stricmp`), else the first whose name is. `Err` = the red line.
pub fn kick_target(arg: &[u8], players: &[Player], en: u8) -> Result<u8, &'static [u8]> {
    let mut sorted: Vec<&Player> = players.iter().collect();
    sorted.sort_by_key(|p| p.empire);
    let e = match arg.first() {
        Some(&c @ b'1'..=b'8') => Some(c - b'0'),
        _ => sorted.iter().find(|p| p.race.eq_ignore_ascii_case(arg)).or_else(|| sorted.iter().find(|p| p.name.eq_ignore_ascii_case(arg))).map(|p| p.empire),
    };
    match e {
        Some(e) if players.iter().any(|p| p.empire == e) => if e == en { Err(CANNOT_KICK_YOURSELF) } else { Ok(e) },
        _ => Err(NO_SUCH_PLAYER),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn players() -> Vec<Player> {
        let p = |empire, name: &str, race: &str| Player { empire, name: name.into(), race: race.into() };
        vec![p(6, "Host", "Dribs"), p(3, "Guest", "Morgath"), p(2, "Dribs", "Kroll")]
    }

    #[test]
    fn only_the_host_kicks() {
        assert_eq!(line(b"/kick  3", true), Line::Kick(b"3"));
        assert_eq!(line(b"/KILL guest", true), Line::Kick(b"guest"));
        assert_eq!(line(b"/kick 3", false), Line::Chat);
        assert_eq!(line(b"/kick", true), Line::Chat, "the space is part of the prefix");
        assert_eq!(line(b"/clearlog", true), Line::Chat);
        assert_eq!(line(b"hello", true), Line::Chat);
    }

    #[test]
    fn the_target_by_number_race_then_name() {
        let ps = players();
        assert_eq!(kick_target(b"3", &ps, 6), Ok(3));
        assert_eq!(kick_target(b"37", &ps, 6), Ok(3), "one digit");
        assert_eq!(kick_target(b"6", &ps, 6), Err(CANNOT_KICK_YOURSELF));
        assert_eq!(kick_target(b"5", &ps, 6), Err(NO_SUCH_PLAYER), "not a human still playing");
        assert_eq!(kick_target(b"9", &ps, 6), Err(NO_SUCH_PLAYER), "9 is a name");
        assert_eq!(kick_target(b"0", &ps, 6), Err(NO_SUCH_PLAYER));
        assert_eq!(kick_target(b"morgath", &ps, 6), Ok(3));
        assert_eq!(kick_target(b"GUEST", &ps, 6), Ok(3));
        assert_eq!(kick_target(b"Guest ", &ps, 6), Err(NO_SUCH_PLAYER), "the whole rest");
        assert_eq!(kick_target(b"kroll", &ps, 6), Ok(2));
        // 6's race name wins over 2's player name: the race walk runs first.
        assert_eq!(kick_target(b"dribs", &ps, 6), Err(CANNOT_KICK_YOURSELF));
        assert_eq!(kick_target(b"", &ps, 6), Err(NO_SUCH_PLAYER));
    }
}
