//! The starmap's info lines (`GameInfo`, exe 0xE3E3A0; `re/rules/campaign.md` §2.3): what `st_AddGameInfo` and
//! `RemoveGameInfo` do to the saved list.

use super::Game;
use crate::messages::{cstr, LineNames};
use crate::save::GameInfo;
use crate::story::InfoLine;

/// `GameInfoMut` (exe 0xE3E3A4), kept beside the list.
const COUNT: u32 = 0xE3E3A4;

/// `CString(int)` (ELF 0x4f3e00): `"%d"`.
fn number(n: i32) -> Vec<u8> {
    n.to_string().into_bytes()
}

/// `"%d.%2d.%2d"` (ELF 0xd5f732) of a date the script already moved a year on.
fn date([y, m, d]: [i32; 3]) -> Vec<u8> {
    format!("{y}.{m:2}.{d:2}").into_bytes()
}

/// `stringhelper::format(fmt, a1, a2, a3)` over gameinfo.txt's line: each `%s` takes the next text (a missing one
/// prints as the CRT's `(null)`); the lines hold no other conversion.
fn format(fmt: &[u8], args: &[Option<Vec<u8>>; 3]) -> Vec<u8> {
    let mut out = Vec::with_capacity(fmt.len());
    let mut next = args.iter();
    let mut i = 0;
    while i < fmt.len() {
        if fmt[i] == b'%' && fmt.get(i + 1) == Some(&b's') {
            out.extend_from_slice(next.next().and_then(|a| a.as_deref()).unwrap_or(b"(null)"));
            i += 2;
        } else {
            out.push(fmt[i]);
            i += 1;
        }
    }
    out
}

impl Game<'_> {
    /// `st_AddGameInfo`'s texts `a1..a3` and colour for a script's line (the scripts' calls, ELF `st_*_include`).
    fn info_args(&self, line: InfoLine) -> ([Option<Vec<u8>>; 3], i32) {
        let planet = |p: i32| Some(self.planet(p));
        let player = |e: i32| Some(cstr(&self.state.mem.read(0xBAE588 + 11 * e.clamp(0, 18) as u32, 11)).to_vec());
        let args = match line {
            InfoLine::Blank => [None, None, None],
            InfoLine::Planet { planet: p, count } => [planet(p), count.map(number), None],
            InfoLine::Countdown { count, planet: p } => [Some(number(count)), planet(p), None],
            InfoLine::Empire(e) => [Some(self.empire(e)), None, None],
            InfoLine::Tech(t) => [Some(self.tech(t)), None, None],
            InfoLine::EmpirePlanet { empire, planet: p } => [Some(self.empire(empire)), planet(p), None],
            InfoLine::PlanetDate { planet: p, date: d } => [planet(p), Some(date(d)), None],
            InfoLine::Date(d) => [Some(date(d)), None, None],
            InfoLine::Planets(ps) => {
                let mut s = Vec::new();
                for p in ps.into_iter().filter(|&p| p != 0) {
                    if !s.is_empty() {
                        s.extend_from_slice(b", ");
                    }
                    s.extend_from_slice(&self.planet(p));
                }
                [Some(s), None, None]
            }
            InfoLine::TwoPlanets([a, b]) => [planet(a), planet(b), None],
            // `st_Multi` 10032 + k: colour 6 / 3 / 7 for RED / YELLOW / GREEN.
            InfoLine::Threat { empire, level, planet: p } => {
                let (word, colour) = match level {
                    3 => ("RED", 6),
                    2 => ("YELLOW", 3),
                    _ => ("GREEN", 7),
                };
                return ([player(empire), Some(word.as_bytes().to_vec()), planet(p)], colour);
            }
            // `st_Multi` 10051, colour 3.
            InfoLine::Score { score, touchdowns, flag } => {
                let f = |b: bool| if b { 'F' } else { ' ' };
                // `"%s: %4d/%d %c   %s: %4d/%d %c"` with `PlayerNev[1]`, `PlayerNev[2]`.
                let mut s = player(1).unwrap_or_default();
                s.extend_from_slice(format!(": {:4}/{} {}   ", score[0], touchdowns[0], f(flag[0])).as_bytes());
                s.extend_from_slice(&player(2).unwrap_or_default());
                s.extend_from_slice(format!(": {:4}/{} {}", score[1], touchdowns[1], f(flag[1])).as_bytes());
                return ([Some(s), None, None], 3);
            }
        };
        (args, 1)
    }

    /// `AddGameInfo(id, a1, a2, a3, colour)` (exe 0x53ad10): the gameinfo.txt line `id` (none: nothing) unless the
    /// list already shows that line (the old entry stays), appended with its text formatted from `a1..a3` (`a1`
    /// NULL: the line as it is). `a2az` runs only for `Lang == 4`, which the GOG build never sets (`locale.md` §2).
    fn add_game_info(&mut self, id: i32, args: [Option<Vec<u8>>; 3], colour: i32) {
        let Some(index) = self.rules.story.game_info.iter().position(|(k, _)| *k == id) else { return };
        let list = &mut self.state.game_info;
        if list.iter().any(|e| e.index == index as i32) {
            return;
        }
        let fmt = &self.rules.story.game_info[index].1;
        let text = if args[0].is_none() { fmt.clone() } else { format(fmt, &args) };
        list.push(GameInfo { index: index as i32, text, colour: if colour == 0 { 1 } else { colour } });
        self.state.mem.set_i32(COUNT, list.len() as i32);
    }

    /// `st_AddGameInfo` with a script's number (`CString(value)`).
    pub(super) fn add_game_info_number(&mut self, id: i32, value: i32) {
        self.add_game_info(id, [Some(number(value)), None, None], 1);
    }

    /// `st_AddGameInfo` with a script's line.
    pub(super) fn add_game_info_line(&mut self, id: i32, line: InfoLine) {
        let (args, colour) = self.info_args(line);
        self.add_game_info(id, args, colour);
    }

    /// `RemoveGameInfo(id)` (exe 0x540e50): the first entry showing line `id` leaves, the rest move down.
    pub(super) fn remove_game_info(&mut self, id: i32) {
        let lines = &self.rules.story.game_info;
        let list = &mut self.state.game_info;
        if let Some(i) = list.iter().position(|e| lines.get(e.index as usize).is_some_and(|(k, _)| *k == id)) {
            list.remove(i);
            self.state.mem.set_i32(COUNT, list.len() as i32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{GameRules, StoryRules};
    use crate::save::{Memory, SaveState};

    #[test]
    fn lines_append_once_and_close_up_on_removal() {
        let lines = vec![(3711, b"You have %s years left".to_vec()), (8021, b"Find the spy".to_vec()), (1311, b"From %s to %s".to_vec())];
        let rules = GameRules { story: StoryRules { game_info: lines, ..Default::default() }, ..Default::default() };
        let mut g = Game::new(SaveState::from_parts(Memory::default(), Vec::new()), &rules);
        g.add_game_info_number(3711, 6);
        g.add_game_info_number(3711, 5);
        g.add_game_info_line(8021, InfoLine::Blank);
        g.add_game_info_number(9999, 1);
        let texts = |g: &Game| g.state.game_info.iter().map(|e| (e.index, e.text.clone(), e.colour)).collect::<Vec<_>>();
        assert_eq!(texts(&g), [(0, b"You have 6 years left".to_vec(), 1), (1, b"Find the spy".to_vec(), 1)]);
        g.remove_game_info(3711);
        g.remove_game_info(3711);
        assert_eq!(texts(&g), [(1, b"Find the spy".to_vec(), 1)]);
        assert_eq!(g.state.mem.i32(COUNT), 1);
        // A missing text prints as `(null)`.
        assert_eq!(format(b"From %s to %s", &[Some(b"A".to_vec()), None, None]), b"From A to (null)");
        assert_eq!(date([2346, 3, 7]), b"2346. 3. 7");
    }
}
