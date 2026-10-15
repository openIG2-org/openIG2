//! In-game messages: index, dialogue choice scripts, text parts and talking-head clip lists.
//!
//! Spec: `re/formats/text.md` § Messages. Evidence (ELF `ig2`, `../sources_base/messages.cpp`):
//! `LoadMessagesFile` @0x5a6f50 (`uzenetek/uzenetek.txt`), `BetoltFajlok` @0x5a83d0 (`msg%04d`/`txt%04d`),
//! `KeresBlokk`/`KiertekelBlokk` (msg blocks and choices), `KeresReszlet`/`KiirReszlet`/
//! `GetTxtFileCommands` (txt parts), `ReadUzenetAnimLista`/`GetUzenetAnimLista` (`uzenetanim_%d`).
//!
//! All positions are byte columns, as in the original (the 1999 code indexes the raw buffer).

use crate::text::lines;

fn col(line: &str, from: usize, len: usize) -> &str {
    line.get(from..(from + len).min(line.len())).unwrap_or("")
}

/// C `atoi`: optional sign, digits, stop at the first non-digit; 0 if none.
pub fn atoi(s: &str) -> i32 {
    let s = s.trim_start();
    let (neg, rest) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    let v: i64 = rest[..end].parse().unwrap_or(0);
    (if neg { -v } else { v }) as i32
}

/// A block/part header: two digits, then a space or end of line. Title lines such as `1013` at the
/// top of some txt files would also match the game's `%02d` prefix scan, but only if that part
/// number were ever requested; they aren't headers.
fn is_header(line: &str) -> bool {
    let b = line.as_bytes();
    b.len() >= 2 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b.get(2).is_none_or(|&c| c == b' ' || c == 9)
}

fn starts_with_digit(s: &str) -> bool {
    s.as_bytes().first().is_some_and(u8::is_ascii_digit)
}

// ---------------------------------------------------------------------------------------------
// uzenetek/uzenetek.txt

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageKind {
    pub id: u32,
    /// "BESZELGETES": has an interactive dialogue (`msgNNNN.txt` exists).
    pub conversation: bool,
    /// "AUTOMATIKUS": shown automatically.
    pub automatic: bool,
    /// "LEVALTAS" [inf: replaces an older message].
    pub replaces: bool,
    /// Sender name (20-column field, trailing spaces trimmed).
    pub sender: String,
    /// Subject, two 20-column lines.
    pub subject: [String; 2],
}

/// Parse `uzenetek/uzenetek.txt` (fixed columns; non-digit lines are comments).
pub fn parse_message_index(text: &str) -> Vec<MessageKind> {
    let mut out = Vec::new();
    for line in lines(text) {
        if !starts_with_digit(line) {
            continue;
        }
        let id = atoi(col(line, 0, 5)).max(0) as u32;
        let sp = line.bytes().take(6).position(|b| b == b' ').unwrap_or(6);
        let p = sp + 1;
        let ii = |o: usize| col(line, p + o, 2) == "II";
        out.push(MessageKind {
            id,
            conversation: ii(0),
            automatic: ii(3),
            replaces: ii(6),
            sender: col(line, p + 9, 20).trim_end().to_owned(),
            subject: [col(line, p + 30, 20).trim_end().to_owned(), line.get(p + 51..).unwrap_or("").trim_end().to_owned()],
        });
    }
    out
}

// ---------------------------------------------------------------------------------------------
// uzenetek/msgNNNN.txt: dialogue blocks

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    /// Text part (in `txtNNNN.txt`) printed when chosen; −1 = auto-continue without printing.
    pub part: i32,
    /// Next block; −1 = end of dialogue.
    pub next: i32,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: u32,
    pub choices: Vec<Choice>,
}

/// Parse a `msgNNNN.txt` dialogue script. Block 0 is the entry point (`KeresBlokk(0)`).
pub fn parse_msg_script(text: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    for line in lines(text) {
        if is_header(line) {
            blocks.push(Block { id: atoi(col(line, 0, 2)) as u32, choices: Vec::new() });
        } else if line.starts_with('-') {
            if let Some(b) = blocks.last_mut() {
                b.choices.push(Choice {
                    part: atoi(col(line, 2, 2)),
                    next: atoi(col(line, 5, 2)),
                    label: line.get(8..).unwrap_or("").to_owned(),
                });
            }
        }
    }
    blocks
}

// ---------------------------------------------------------------------------------------------
// uzenetek/txtNNNN.txt: text parts

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    /// `EN` ("me"): the player.
    Player,
    /// `TE` ("you"): the other party; triggers the talking-head clip for this part.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLine {
    /// `+` lines start a new line; `-` lines flow into the previous one.
    pub new_line: bool,
    /// Text as stored, including inline markup (`~z`, `~s`, `#1#` placeholders).
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextPart {
    pub part: u32,
    pub anim_start: i32,
    /// Typing speed (`Sebesseg`; per-character advance = speed / OSZTO).
    pub speed: i32,
    pub speaker: Speaker,
    /// `#n` at column 14: story-map animation (`CreateStoryMap`).
    pub map_anim: Option<i32>,
    pub lines: Vec<TextLine>,
}

pub fn parse_txt_script(text: &str) -> Vec<TextPart> {
    let mut parts: Vec<TextPart> = Vec::new();
    for line in lines(text) {
        if is_header(line) {
            parts.push(TextPart {
                part: atoi(col(line, 0, 2)) as u32,
                anim_start: atoi(col(line, 3, 4)),
                speed: atoi(col(line, 8, 2)),
                speaker: if col(line, 11, 2) == "EN" { Speaker::Player } else { Speaker::Other },
                map_anim: (line.as_bytes().get(14) == Some(&b'#')).then(|| atoi(line.get(15..).unwrap_or(""))),
                lines: Vec::new(),
            });
        } else if let Some(m @ (b'+' | b'-')) = line.as_bytes().first().copied() {
            if let Some(p) = parts.last_mut() {
                // KiirReszlet: skip the marker, and one more character unless it's a space/EOL.
                let skip = match line.as_bytes().get(1) {
                    None | Some(b' ') => 1,
                    _ => 2,
                };
                let t = line.get(skip..).unwrap_or("");
                p.lines.push(TextLine { new_line: m == b'+', text: t.strip_prefix(' ').unwrap_or(t).to_owned() });
            }
        }
    }
    parts
}

// ---------------------------------------------------------------------------------------------
// text/uzenetanim_{1,3,5}.txt: talking-head clips per message part

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipPart {
    pub part: u32,
    /// Clip path without extension (`trader/k13_0-750`). The game prefixes `messages/` unless the
    /// name contains `misc/`, and appends `.bik`; the remaster ships `anim/mpeg4/<path>.mp4`.
    pub clip: String,
    /// Optional second (waiting/loop) clip.
    pub loop_clip: Option<String>,
    /// `#n`: face/race override, clamped to 1..=19 by the game.
    pub face: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipGroup {
    /// Message IDs this group applies to (a header line without `!` holding one or more numbers).
    pub message_ids: Vec<u32>,
    /// The most recent label line without numbers (e.g. "Kereskedok" = traders).
    pub label: Option<String>,
    pub parts: Vec<ClipPart>,
}

impl ClipPart {
    /// Path of the remaster video for this clip, relative to the install.
    pub fn video_path(clip: &str) -> String {
        if clip.contains("misc/") { format!("anim/mpeg4/{clip}.mp4") } else { format!("anim/mpeg4/messages/{clip}.mp4") }
    }
}

pub fn parse_clip_lists(text: &str) -> Vec<ClipGroup> {
    let mut groups: Vec<ClipGroup> = Vec::new();
    let mut label: Option<String> = None;
    for line in lines(text) {
        let t = line.trim();
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        if !t.contains('!') {
            let ids: Vec<u32> = t
                .split(|c: char| !c.is_ascii_digit())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect();
            if ids.is_empty() {
                label = Some(t.to_owned());
            } else {
                groups.push(ClipGroup { message_ids: ids, label: label.clone(), parts: Vec::new() });
            }
            continue;
        }
        if starts_with_digit(t) {
            let Some(g) = groups.last_mut() else { continue };
            let after = &t[t.find('!').unwrap() + 1..];
            let clip_end = after.find([' ', '\t']).unwrap_or(after.len());
            let clip = after[..clip_end].to_owned();
            let rest = &after[clip_end..];
            let (loop_clip, rest) = match rest.strip_prefix(' ').and_then(|r| r.strip_prefix('!')) {
                Some(r) => {
                    let e = r.find([' ', '\t']).unwrap_or(r.len());
                    (Some(r[..e].to_owned()), &r[e..])
                }
                None => (None, rest),
            };
            let face = rest.trim_start().strip_prefix('#').map(|f| atoi(f).clamp(1, 19));
            g.parts.push(ClipPart { part: atoi(t) as u32, clip, loop_clip, face });
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_columns() {
        let k = parse_message_index("; c\r\n0021 II NN NN Governor Mander      Rebellious faction   on our colony\r\n");
        assert_eq!(k[0].id, 21);
        assert!(k[0].conversation && !k[0].automatic);
        assert_eq!(k[0].sender, "Governor Mander");
        assert_eq!(k[0].subject, ["Rebellious faction".to_owned(), "on our colony".to_owned()]);
    }

    #[test]
    fn msg_and_txt() {
        let b = parse_msg_script("00\r\n- 01 02 -----------\r\n02\r\n- 02 -1 I'll take all the necessary steps.\r\n");
        assert_eq!(b[0].choices[0], Choice { part: 1, next: 2, label: "-----------".into() });
        assert_eq!(b[1].choices[0].next, -1);
        let p = parse_txt_script("; PICTURE\r\n01 0001 12 TE #3\r\n+ ~zHello #1#~s\r\n- world\r\n");
        assert_eq!(p[0].speaker, Speaker::Other);
        assert_eq!(p[0].map_anim, Some(3));
        assert_eq!(p[0].lines[0], TextLine { new_line: true, text: "~zHello #1#~s".into() });
        assert!(!p[0].lines[1].new_line);
    }

    #[test]
    fn clip_lists() {
        let g = parse_clip_lists("Kereskedok\r\n\r\n5540, 5541\r\n01 !trader/k15-0-200loop !trader/k15-w #3\r\n02 !misc/x\r\n");
        assert_eq!(g[0].message_ids, [5540, 5541]);
        assert_eq!(g[0].label.as_deref(), Some("Kereskedok"));
        assert_eq!(g[0].parts[0].loop_clip.as_deref(), Some("trader/k15-w"));
        assert_eq!(g[0].parts[0].face, Some(3));
        assert_eq!(ClipPart::video_path(&g[0].parts[1].clip), "anim/mpeg4/misc/x.mp4");
    }
}
