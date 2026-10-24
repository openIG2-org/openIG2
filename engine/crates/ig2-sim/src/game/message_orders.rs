//! The human's inbox as the sim writes it (re/rules/info-messages.md §6–§7): the stories' message calls
//! (`NewMessage` exe 0x508fe0, `SetParamStr`/`SetParamInt` 0x509570/0x509500, `SetMessageFajOverride` 0x5094c0,
//! `KiszedUzenet` 0x506200, `KiszedUtolsoUzenet` 0x506140, `LetiltReszlet` 0x5063d0) and the messages screen's
//! writes to saved state with their story hand-offs (`KiertekelUjuzenet` 0x505d60, `KeresBlokk` 0x505930,
//! `KiertekelBlokk` 0x505c20, `ValaszKivalasztva` 0x509d10, the typing end of `MessagesGfx2D_2` 0x507b43,
//! `Messages_Belep` 0x507f90, `FRISSIT_Messages2`'s automatic messages 0x505180). The original sends no lockstep
//! message for the screen's writes: they are [`MessageOrder`]s here so every peer and the save see them.
//!
//! The dialogue's unsaved bits the next order reads (the open block's choices, `AktValasztasok`, `ParbeszedVege`,
//! `ParbeszedVegeJon`, `KiirtaMar`, the rows) are kept in the state's memory at the exe's addresses.

use super::story_chain;
use super::*;
use crate::messages::{block_choices, find_block, Inbox, MESSAGE, PLAYBACK};
use crate::save::va;
use crate::story::{flag, MessageText};

/// The messages screen's globals (exe VAs; `save.rs`'s chunk table names the saved ones).
mod at {
    pub const FOLYAMATBAN_U: u32 = 0xDABDA0;
    pub const FOLYAMATBAN_UH: u32 = 0xDABDA4;
    pub const FOLYAMATBAN_U3: u32 = 0xDABDA8;
    pub const FOLYAMATBAN_UP: u32 = 0xDABDAC;
    pub const FOLYAMATBAN_PA: u32 = 0xDA65E7;
    pub const OLD_BLOCK: u32 = 0xDABDB0;
    pub const NEW_ANSWER: u32 = 0xDABDB4;
    pub const NEW_MESSAGE: u32 = 0xDABDB8;
    pub const CURRENT: u32 = 0xDABDC0;
    pub const BLOCK: u32 = 0xDABDC4;
    pub const ARRIVED: u32 = 0xDA65FF;
    pub const LAST_NEW: u32 = 0xDABDC8;
    pub const LAST_EXPIRY: u32 = 0xDABDCC;
    pub const POINTER: u32 = 0xDABDD4;
    pub const AUTOMATIC: u32 = 0xDABDD8;
    pub const AUTOMATIC_AT: u32 = 0xDABDDC;
    pub const TEXT_END: u32 = 0xDABDE0;
    pub const DISABLED_COUNT: u32 = 0xDABDE4;
    pub const DISABLED: u32 = 0xDABDE8;
    // Unsaved.
    pub const ROW: u32 = 0xD90DC8;
    pub const ANSWER: u32 = 0xD90DCC;
    pub const REPLAY_AT: u32 = 0xD90DD4;
    pub const FIRST_UNREAD: u32 = 0xD90DC0;
    pub const FIRST_UNREAD_ID: u32 = 0x91494C;
    pub const CHOICES: u32 = 0xD8E160;
    pub const CHOICE: u32 = 0x5c;
    pub const CHOICE_COUNT: u32 = 0xDABE3C;
    pub const OVER: u32 = 0xD909BA;
    pub const ENDS: u32 = 0xD909BB;
    pub const TYPED: u32 = 0xDA6607;
}

/// Whether choice `k` of the open block is disabled (`Valasztasok[k] + 0x58`, written by the last order).
pub fn choice_disabled(state: &crate::save::SaveState, k: usize) -> bool {
    k < 4 && state.mem.u8(at::CHOICES + at::CHOICE * k as u32 + 8) != 0
}

/// The entry of the dialogue in progress (`FolyamatbanU ≠ −1`: `UAktUzenet`), for the screen to show.
pub fn dialogue_in_progress(state: &crate::save::SaveState) -> Option<usize> {
    (state.mem.i32(at::FOLYAMATBAN_U) != -1).then(|| state.mem.i32(at::CURRENT)).filter(|&i| i >= 0).map(|i| i as usize)
}

/// One write of the messages screen (the human's input or the screen's own step).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageOrder {
    /// `Messages_Belep`'s saved part: no dialogue in progress → the first unread message's place in the list (after
    /// a finished one) and `KeresAutomataUzenet`; else the dialogue in progress is taken up again.
    Enter,
    /// `Messages_Main`'s arrows: `AktUzenetPointer` and the selected row (`AktUzenet0to3`, −1 = none).
    Scroll { pointer: i32, row: i32 },
    /// A click on list row `row` (0 = the bottom one): `KiertekelUjuzenet` when the row holds a message.
    Open { row: i32 },
    /// `FRISSIT_Messages2` (every 11th UI frame on the screen): `KeresAutomataUzenet` once the last text is out,
    /// and the automatic message found is opened.
    Auto,
    /// Answer `k` taken (a click, or `Melyikvalaszfolottvanazeger`'s own pick): `ValaszKivalasztva`.
    Answer { k: i32 },
    /// `MessagesGfx2D_2`'s typing end (`KiirtaMar` 0 → 1).
    TypedOut,
}

impl MessageOrder {
    /// The order as a type and values (the wire form of `Command::Message`).
    pub fn to_message(self) -> (i32, Vec<i32>) {
        match self {
            Self::Enter => (0, vec![]),
            Self::Scroll { pointer, row } => (1, vec![pointer, row]),
            Self::Open { row } => (2, vec![row]),
            Self::Auto => (3, vec![]),
            Self::Answer { k } => (4, vec![k]),
            Self::TypedOut => (5, vec![]),
        }
    }

    pub fn from_message(kind: i32, v: &[i32]) -> Option<Self> {
        Some(match kind {
            0 => Self::Enter,
            1 => Self::Scroll { pointer: *v.first()?, row: *v.get(1)? },
            2 => Self::Open { row: *v.first()? },
            3 => Self::Auto,
            4 => Self::Answer { k: *v.first()? },
            5 => Self::TypedOut,
            _ => return None,
        })
    }
}

/// An entry's byte `+o` (bounds-checked: the exe reads past the count freely, the arrays hold 100).
fn byte(b: &Inbox, i: i32, o: usize) -> u8 {
    b.messages.get((i.max(0) as usize) * MESSAGE + o).copied().unwrap_or(0)
}

fn id_of(b: &Inbox, i: i32) -> i32 {
    let o = (i.max(0) as usize) * MESSAGE;
    b.messages.get(o..o + 4).map_or(0, |r| i32::from_le_bytes(r.try_into().unwrap()))
}

fn set_byte(b: &mut Inbox, i: i32, o: usize, v: u8) {
    if let Some(x) = b.messages.get_mut((i.max(0) as usize) * MESSAGE + o) {
        *x = v;
    }
}

/// Close the gap at entry `i` (the three arrays; the last entry stays duplicated) and drop the count.
fn take_out(b: &mut Inbox, i: usize) {
    let n = b.count.max(0) as usize;
    if i + 1 < n {
        b.random_from.copy_within((i + 1) * 4..n * 4, i * 4);
        b.playbacks.copy_within((i + 1) * PLAYBACK..n * PLAYBACK, i * PLAYBACK);
        b.messages.copy_within((i + 1) * MESSAGE..n * MESSAGE, i * MESSAGE);
    }
    b.count -= 1;
}

impl Game<'_> {
    fn g(&self, a: u32) -> i32 {
        self.state.mem.i32(a)
    }

    fn set(&mut self, a: u32, v: i32) {
        self.state.mem.set_i32(a, v);
    }

    /// The stories' message calls, in event order after the check that made them.
    pub(super) fn inbox_events(&mut self, events: &[StoryEvent]) {
        for e in events {
            match *e {
                StoryEvent::NewMessage { id, arg } => self.new_message(id, arg),
                StoryEvent::Message { id, empire } => {
                    self.new_message(id, 0);
                    let name = self.empire_name(empire.clamp(0, 255) as u8);
                    self.set_param(1, &name);
                    let race = self.state.races().get(empire.max(0) as usize).copied().unwrap_or(0) as i32;
                    self.message_race(race);
                }
                StoryEvent::MessageRace(r) => self.message_race(r),
                StoryEvent::MessageParam { slot, text } => {
                    if let Some(t) = self.message_text(text) {
                        self.set_param(slot, &t);
                    }
                }
                StoryEvent::MessageParamInt { slot, value } => self.set_param(slot, value.to_string().as_bytes()),
                StoryEvent::RemoveMessage(id) => self.remove_message(id),
                StoryEvent::WithdrawMessage { id, .. } => self.withdraw_message(id),
                // Undo type 10 (`campaign.md` §2.3): `KiszedUtolsoUzenet(a, 1)`.
                StoryEvent::Undo { kind: 10, value } => self.withdraw_message(value),
                StoryEvent::DisablePart(part) => {
                    let n = self.g(at::DISABLED_COUNT);
                    self.set(at::DISABLED + 4 * n as u32, part);
                    self.set(at::DISABLED_COUNT, n + 1);
                }
                _ => {}
            }
        }
    }

    /// A message call made outside a story check (the replay's staged calls): as the stories' events.
    pub fn inbox_call(&mut self, ev: StoryEvent) {
        self.inbox_events(&[ev]);
    }

    /// `NewMessage(id, days)` (exe 0x508fe0): first the slow-down `SetIdoLelassit` (even when the inbox is full; a
    /// story check's own already ran as the script made the call, `GameStory::event`), then a new entry at the
    /// end, parameters 1..=5 `"#=#"`, read and seen cleared, the arrival date. The kind's automatic flag switches
    /// to the messages screen (`AktKepKell` 10: the client's, from the event).
    pub(super) fn new_message(&mut self, id: i32, days: i32) {
        self.slow_down();
        let mut b = self.state.inbox();
        if b.count >= 100 {
            return;
        }
        let n = b.count;
        let ido = self.g(va::IDO);
        let expiry = if days == 0 { 0 } else { days * 50 + ido };
        self.state.mem.set_u8(at::ARRIVED, 1);
        self.set(at::LAST_NEW, n);
        self.set(at::LAST_EXPIRY, expiry);
        let o = n as usize * MESSAGE;
        b.random_from[n as usize * 4..n as usize * 4 + 4].fill(0);
        for k in 1..6 {
            b.messages[o + 0x10 + 0x14 * k..o + 0x14 + 0x14 * k].copy_from_slice(&0x233d23u32.to_le_bytes());
        }
        b.messages[o..o + 4].copy_from_slice(&id.to_le_bytes());
        b.messages[o + 8..o + 12].copy_from_slice(&expiry.to_le_bytes());
        b.messages[o + 4] = 0;
        b.messages[o + 5] = 0;
        b.messages[o + 0xc..o + 0x10].copy_from_slice(&ido.to_le_bytes());
        b.count += 1;
        self.state.store_inbox(&b);
    }

    /// `SetParamStr(k, s)` / `SetParamInt` (exe 0x509570 / 0x509500): the last new message's parameter `k`
    /// (`k < 6`), copied with its NUL and no length check.
    fn set_param(&mut self, k: i32, s: &[u8]) {
        if k >= 6 {
            return;
        }
        let mut b = self.state.inbox();
        let at = self.g(at::LAST_NEW) as isize * MESSAGE as isize + 0x10 + 0x14 * k as isize;
        for (i, &c) in s.iter().chain(std::iter::once(&0)).enumerate() {
            if let Some(x) = usize::try_from(at + i as isize).ok().and_then(|o| b.messages.get_mut(o)) {
                *x = c;
            }
        }
        self.state.store_inbox(&b);
    }

    /// `SetMessageFajOverride(r)` (exe 0x5094c0): the last new message's portrait race (1..=8, 13, 14; else 1).
    fn message_race(&mut self, r: i32) {
        let v = if (1..=8).contains(&r) || r == 13 || r == 14 { r } else { 1 };
        let mut b = self.state.inbox();
        let i = self.g(at::LAST_NEW).max(0) as usize * 4;
        if let Some(x) = b.random_from.get_mut(i..i + 4) {
            x.copy_from_slice(&v.to_le_bytes());
        }
        self.state.store_inbox(&b);
    }

    /// The text a story names as a message parameter; `None` for the kinds the sim has no names for yet.
    fn message_text(&self, t: MessageText) -> Option<Vec<u8>> {
        use messages::LineNames;
        Some(match t {
            MessageText::Empire(e) => self.empire(e),
            MessageText::Planet(p) => self.planet(p),
            MessageText::Tech(t) => self.tech(t),
            MessageText::Player(e) => cstr(&self.state.mem.read(0xBAE588 + 11 * e.clamp(0, 18) as u32, 11)).to_vec(),
            MessageText::Text(n) => cstr(self.rules.messages.story_text(n)).to_vec(),
            MessageText::Fleet(f) => cstr(&self.state.fleets.get(f.max(0) as usize)?[..0x20]).to_vec(),
            MessageText::Literal(s) => s.as_bytes().to_vec(),
            MessageText::Number(x) => format!("{x:.2}").into_bytes(),
            _ => return None,
        })
    }

    /// `KiszedUzenet(id)` (exe 0x506200): the first entry with block `id` leaves the inbox, read or not (the loop
    /// runs past the end once it has moved the rest down).
    fn remove_message(&mut self, id: i32) {
        let mut b = self.state.inbox();
        if let Some(i) = (0..b.count.max(0)).find(|&i| id_of(&b, i) == id) {
            take_out(&mut b, i as usize);
            self.state.store_inbox(&b);
        }
    }

    /// `KiszedUtolsoUzenet(id, _)` (exe 0x506140): from the slot one past the last entry down, the first entry with
    /// block `id`; it leaves the inbox only when unread (`+4 == 0`).
    fn withdraw_message(&mut self, id: i32) {
        let mut b = self.state.inbox();
        let Some(i) = (0..=b.count.max(0)).rev().find(|&i| id_of(&b, i) == id) else { return };
        // The slot past the end counts too: the count drops with nothing to move.
        if byte(&b, i, 4) == 0 {
            take_out(&mut b, i as usize);
            self.state.store_inbox(&b);
        }
    }

    /// The messages screen's order from empire `issuer`. The inbox is the human's: other issuers' orders change
    /// nothing (each player's inbox in multiplayer is open: re/rules/info-messages.md §7.6).
    pub fn message_order(&mut self, issuer: u8, o: MessageOrder) {
        if issuer as i32 != self.g(va::EN) {
            return;
        }
        match o {
            MessageOrder::Enter => self.enter_messages(),
            MessageOrder::Scroll { pointer, row } => {
                self.set(at::POINTER, pointer);
                self.set(at::ROW, row);
            }
            MessageOrder::Open { row } => {
                // `Messages_Main` takes a row only with no dialogue running and no automatic message waiting.
                if self.g(at::FOLYAMATBAN_U) != -1 || self.state.mem.u8(at::OVER) == 0 || self.g(at::AUTOMATIC) != -1 {
                    return;
                }
                let n = self.state.inbox().count;
                self.set(at::NEW_MESSAGE, -1);
                if (0..4).contains(&row) && n > row {
                    self.set(at::ROW, row);
                    self.set(at::NEW_MESSAGE, 1);
                }
                if n < self.g(at::ROW) {
                    self.set(at::ROW, -1);
                }
                self.automatic_place();
                if self.g(at::NEW_MESSAGE) == 1 {
                    self.open_message();
                }
            }
            MessageOrder::Auto => {
                let over = self.state.mem.u8(at::OVER) != 0;
                if self.g(at::NEW_MESSAGE) == -1 || (self.g(at::TEXT_END) == 1 && over) {
                    self.find_automatic();
                    if self.g(at::AUTOMATIC) != -1 {
                        self.set(at::NEW_MESSAGE, 1);
                        self.automatic_place();
                        self.open_message();
                    }
                }
            }
            MessageOrder::Answer { k } => {
                // `Messages_Main` / `Melyikvalaszfolottvanazeger` pick only among the block's answers.
                if !(0..self.g(at::CHOICE_COUNT)).contains(&k) {
                    return;
                }
                self.set(at::ANSWER, k);
                self.set(at::NEW_ANSWER, 1);
                self.take_answer();
            }
            MessageOrder::TypedOut => self.typed_out(),
        }
    }

    /// The kind (`UzenetFajtak[KerUz(id)]`): conversation, automatic. An id without a kind reads the slot past the
    /// table (zeros in the exe).
    fn kind_of(&self, id: i32) -> (bool, bool) {
        self.rules.inbox.kind(id)
    }

    fn story_hand_off(&mut self, flag: usize, block: i32, part: i32) {
        self.with_story(|st, w| {
            st.message = (block, part);
            st.flags[flag] = true;
            st.check(true, &story_chain(), w);
        });
    }

    /// `Story_MessageMegnezes(id)` (exe 0x5c90f0): once per block (`OlvasasVodor`), flag 9.
    fn message_seen(&mut self, id: i32) {
        if self.state.story().read.contains(id) {
            return;
        }
        self.with_story(|st, w| {
            st.message.0 = id;
            st.flags[flag::READ] = true;
            st.read.add(id, 0);
            st.check(true, &story_chain(), w);
        });
    }

    /// `KeresBlokk(n)` (exe 0x505930): `FolyamatbanOldBlokk` := n; `AktBlokk` := n when the block exists.
    fn seek_block(&mut self, n: i32) -> i32 {
        self.set(at::OLD_BLOCK, n);
        let id = self.g(va::INBOX_SHOWING);
        let off = self.rules.inbox.scripts.get(&id).map_or(-1, |m| find_block(m, n));
        if off >= 0 {
            self.set(at::BLOCK, n);
        }
        off
    }

    /// `KiertekelBlokk(off)` (exe 0x505c20): the disabled list cleared, `Story_LetiltasCheck(UAktUzenetFajta,
    /// AktBlokk)` (flag 8: the stories disable answers through `LetiltReszlet`), then the block's choices; a block
    /// that doesn't start with `-` leaves the old choices and gives none.
    fn eval_block(&mut self, off: i32) {
        self.set(at::DISABLED_COUNT, 0);
        let (id, block) = (self.g(va::INBOX_SHOWING), self.g(at::BLOCK));
        self.story_hand_off(flag::DISABLED, id, block);
        let script = self.rules.inbox.scripts.get(&id).cloned().unwrap_or_default();
        let choices = block_choices(&script, off);
        let n = self.g(at::DISABLED_COUNT).max(0);
        let disabled: Vec<i32> = (0..n).map(|i| self.g(at::DISABLED + 4 * i as u32)).collect();
        for (k, c) in choices.iter().enumerate() {
            let a = at::CHOICES + at::CHOICE * k as u32;
            self.state.mem.set_u8(a + 8, disabled.contains(&c.part) as u8);
            self.set(a, c.part);
            self.set(a + 4, c.next);
            self.copy_label(&script, c.at + 8, a - 0x50);
        }
        self.set(at::CHOICE_COUNT, choices.len() as i32);
    }

    /// `mesAtteszSorvegeig` (exe 0x509f20): the label from `src` to its `\n` (kept, no NUL) into `dest`, `#k#`
    /// replaced by the open entry's parameter `k` with its NUL (the next byte overwrites it); no bound, so a long
    /// label runs over its choice's own fields and into the next choice.
    fn copy_label(&mut self, msg: &[u8], src: i32, dest: u32) {
        let at = |i: i32| msg.get(i.max(0) as usize).copied().unwrap_or(b'\n');
        let base = va::INBOX as i64 + self.g(at::CURRENT) as i64 * MESSAGE as i64 + 0x10;
        let (mut i, mut o) = (src, 0u32);
        let mut c = at(i);
        while c != b'\n' {
            if at(i) == b'#' && at(i + 2) == b'#' {
                let p = (base + (at(i + 1) as i64 - 0x30) * 0x14) as u32;
                i += 3;
                let mut j = 0;
                loop {
                    let b = self.state.mem.u8(p + j);
                    self.state.mem.set_u8(dest + o + j, b);
                    if b == 0 {
                        break;
                    }
                    j += 1;
                }
                o += j;
            }
            self.state.mem.set_u8(dest + o, at(i));
            i += 1;
            o += 1;
            c = at(i);
        }
        self.state.mem.set_u8(dest + o, at(i));
    }

    fn choice(&self, k: i32) -> (i32, i32, bool) {
        let a = (at::CHOICES as i64 + at::CHOICE as i64 * k as i64) as u32;
        (self.g(a), self.g(a + 4), self.state.mem.u8(a + 8) != 0)
    }

    /// `UzenetPlaybackek[UAktUzenet]`: one more part played (no bound: past 20 it runs into the next record).
    fn record_part(&mut self, part: i32) {
        let mut b = self.state.inbox();
        let e = self.g(at::CURRENT).max(0) as usize * PLAYBACK;
        let Some(n) = b.playbacks.get(e..e + 4).map(|r| i32::from_le_bytes(r.try_into().unwrap())) else { return };
        let o = e + 4 + 4 * n.max(0) as usize;
        if let Some(x) = b.playbacks.get_mut(o..o + 4) {
            x.copy_from_slice(&part.to_le_bytes());
        }
        b.playbacks[e..e + 4].copy_from_slice(&(n + 1).to_le_bytes());
        self.state.store_inbox(&b);
    }

    fn set_folyamatban(&mut self) {
        let b = self.state.inbox();
        let cur = self.g(at::CURRENT);
        self.set(at::FOLYAMATBAN_U, id_of(&b, cur));
        self.set(at::FOLYAMATBAN_UH, cur);
        self.set(at::FOLYAMATBAN_U3, self.g(at::ROW));
        self.set(at::FOLYAMATBAN_UP, self.g(at::POINTER));
    }

    /// `KiertekelUjuzenet` (exe 0x505d60): entry `AktUzenetPointer + AktUzenet0to3` opened.
    fn open_message(&mut self) {
        let cur = self.g(at::POINTER) + self.g(at::ROW);
        self.set(at::CURRENT, cur);
        self.set(at::TEXT_END, 0);
        self.state.mem.set_u8(at::TYPED, 0);
        // `BetoltFajlok`.
        let id = id_of(&self.state.inbox(), cur);
        self.state.mem.set_i32(va::INBOX_SHOWING, id);
        if !self.kind_of(id).0 {
            self.state.mem.set_u8(at::OVER, 1);
            self.set(at::CHOICE_COUNT, 0);
            self.set_folyamatban();
            let mut b = self.state.inbox();
            set_byte(&mut b, cur, 4, 1);
            let seen = byte(&b, cur, 5) != 0;
            if !seen {
                set_byte(&mut b, cur, 5, 1);
            }
            self.state.store_inbox(&b);
            if !seen {
                self.message_seen(id);
            }
            return;
        }
        self.set_folyamatban();
        let mut b = self.state.inbox();
        let read = byte(&b, cur, 4) != 0;
        if read {
            self.set(at::REPLAY_AT, 0);
        } else if let Some(x) = b.playbacks.get_mut(cur.max(0) as usize * PLAYBACK..cur.max(0) as usize * PLAYBACK + 4) {
            x.fill(0);
            self.state.store_inbox(&b);
        }
        let off = self.seek_block(0);
        self.state.mem.set_u8(at::OVER, 0);
        self.state.mem.set_u8(at::ENDS, 0);
        self.eval_block(off);
        if self.g(at::CHOICE_COUNT) > 0 {
            let (part, next, _) = self.choice(0);
            if part != -1 && byte(&self.state.inbox(), cur, 4) == 0 {
                self.record_part(part);
            }
            let off = self.seek_block(next);
            self.eval_block(off);
        }
        let mut b = self.state.inbox();
        if byte(&b, cur, 5) == 0 {
            // The exe marks the slot one past the last entry, not the message (exe 0x505ff9, the ELF too).
            let n = b.count;
            set_byte(&mut b, n, 5, 1);
            self.state.store_inbox(&b);
            self.message_seen(id);
        }
    }

    /// `ValaszKivalasztva` (exe 0x509d10): the selected answer, unless disabled; an unread message tells the stories
    /// (`Story_MessageSelected`, flag 6) and records it; its next block, or the dialogue ends with this text.
    fn take_answer(&mut self) {
        let k = self.g(at::ANSWER);
        let (part, _, disabled) = self.choice(k);
        if self.g(at::NEW_ANSWER) != 1 || disabled {
            return;
        }
        self.set(at::NEW_ANSWER, 0);
        let cur = self.g(at::CURRENT);
        if byte(&self.state.inbox(), cur, 4) == 0 {
            self.story_hand_off(flag::MESSAGE, self.g(va::INBOX_SHOWING), part);
            let (part, ..) = self.choice(k);
            self.record_part(part);
        }
        self.state.mem.set_u8(at::ENDS, 0);
        let (_, next, _) = self.choice(k);
        if next != -1 {
            let off = self.seek_block(next);
            self.eval_block(off);
        } else {
            self.set(at::CHOICE_COUNT, 0);
            self.state.mem.set_u8(at::ENDS, 1);
            self.set(at::OLD_BLOCK, -1);
        }
        self.state.mem.set_u8(at::TYPED, 0);
    }

    /// `MessagesGfx2D_2`'s typing end (exe 0x507b43..0x507c1b).
    fn typed_out(&mut self) {
        if self.state.mem.u8(at::TYPED) == 1 {
            return;
        }
        self.state.mem.set_u8(at::TYPED, 1);
        self.set(at::TEXT_END, 1);
        let cur = self.g(at::CURRENT);
        let mut b = self.state.inbox();
        if !self.kind_of(id_of(&b, cur)).0 {
            self.set(at::FOLYAMATBAN_U, -1);
            self.set(at::FOLYAMATBAN_UH, -1);
        }
        if self.state.mem.u8(at::ENDS) != 0 {
            self.state.mem.set_u8(at::OVER, 1);
            self.set(at::FOLYAMATBAN_U, -1);
            self.set(at::FOLYAMATBAN_UH, -1);
            self.set(at::OLD_BLOCK, -1);
            set_byte(&mut b, cur, 4, 1);
            self.state.store_inbox(&b);
            self.set(at::NEW_MESSAGE, -1);
        }
        let over = self.state.mem.u8(at::OVER);
        self.state.mem.set_u8(at::FOLYAMATBAN_PA, over);
    }

    /// `KeresAutomataUzenet` (exe 0x5058b0): the first unread entry of an automatic kind and its index.
    fn find_automatic(&mut self) {
        let b = self.state.inbox();
        self.set(at::AUTOMATIC, -1);
        if let Some(i) = (0..b.count.max(0)).find(|&i| self.kind_of(id_of(&b, i)).1 && byte(&b, i, 4) == 0) {
            self.set(at::AUTOMATIC, id_of(&b, i));
            self.set(at::AUTOMATIC_AT, i);
        }
    }

    /// `AutomataUzenetKiertekelo` (exe 0x504a20): the list scrolled to show the automatic message.
    fn automatic_place(&mut self) {
        if self.g(at::AUTOMATIC) == -1 {
            return;
        }
        let (n, i) = (self.state.inbox().count, self.g(at::AUTOMATIC_AT));
        if ((n - 1) as u32) < 4 {
            self.set(at::POINTER, 0);
            self.set(at::ROW, i);
        }
        if n > 4 {
            if i + 3 < n {
                self.set(at::POINTER, i);
                self.set(at::ROW, 0);
            }
            if n < i + 4 {
                self.set(at::POINTER, n - 4);
                self.set(at::ROW, i - n + 4);
            }
        }
    }

    /// `Messages_Belep`'s saved part (exe 0x507f90).
    fn enter_messages(&mut self) {
        if self.g(at::CURRENT) != -1 {
            let id = id_of(&self.state.inbox(), self.g(at::CURRENT));
            self.state.mem.set_i32(va::INBOX_SHOWING, id);
        }
        self.set(at::FIRST_UNREAD_ID, -1);
        if self.g(at::FOLYAMATBAN_U) == -1 {
            if self.state.mem.u8(at::FOLYAMATBAN_PA) != 0 {
                self.first_unread();
                self.first_unread_place();
            }
            self.find_automatic();
            let none = self.g(at::AUTOMATIC) == -1;
            self.state.mem.set_u8(at::TYPED, none as u8);
            return;
        }
        let cur = self.g(at::CURRENT);
        let b = self.state.inbox();
        if !self.kind_of(id_of(&b, cur)).0 {
            // The exe takes `FolyamatbanU` (the block id) for the entry, not `FolyamatbanUH` (the ELF too).
            self.set(at::ROW, self.g(at::FOLYAMATBAN_U3));
            self.set(at::CURRENT, self.g(at::FOLYAMATBAN_U));
            self.set(at::POINTER, self.g(at::FOLYAMATBAN_UP));
            self.set(at::NEW_MESSAGE, 1);
            self.open_message();
            self.state.mem.set_u8(at::TYPED, 0);
            return;
        }
        self.set(at::CHOICE_COUNT, 0);
        self.set(at::TEXT_END, 0);
        if byte(&b, cur, 4) == 0 {
            self.state.mem.set_u8(at::OVER, 0);
            if self.g(at::OLD_BLOCK) != -1 {
                let off = self.seek_block(self.g(at::OLD_BLOCK));
                self.state.mem.set_u8(at::OVER, 0);
                self.state.mem.set_u8(at::ENDS, 0);
                self.eval_block(off);
                if self.g(at::CHOICE_COUNT) > 0 && self.choice(0).0 == -1 {
                    let off = self.seek_block(self.choice(0).1);
                    self.eval_block(off);
                }
            }
        } else {
            self.state.mem.set_u8(at::OVER, 1);
        }
        self.state.mem.set_u8(at::TYPED, 0);
    }

    /// `FUN_00505bd0`: the first unread entry (`AutomataUzenet == −1` only).
    fn first_unread(&mut self) {
        self.set(at::FIRST_UNREAD_ID, -1);
        if self.g(at::AUTOMATIC) != -1 {
            return;
        }
        let b = self.state.inbox();
        if let Some(i) = (0..b.count.max(0)).find(|&i| byte(&b, i, 4) == 0) {
            self.set(at::FIRST_UNREAD_ID, id_of(&b, i));
            self.set(at::FIRST_UNREAD, i);
        }
    }

    /// `FUN_00509cb0`: the list scrolled to the first unread entry.
    fn first_unread_place(&mut self) {
        if self.g(at::FIRST_UNREAD_ID) == -1 {
            return;
        }
        let n = self.state.inbox().count;
        if (n - 1) as u32 > 3 {
            // `AktUzenetPointer &= −(3 < n − 1)`: kept for more than four messages.
        } else {
            self.set(at::POINTER, 0);
        }
        if n > 4 {
            let i = self.g(at::FIRST_UNREAD);
            if i + 3 < n {
                self.set(at::POINTER, i);
            }
            if n < i + 4 {
                self.set(at::POINTER, n - 4);
            }
        }
    }
}
