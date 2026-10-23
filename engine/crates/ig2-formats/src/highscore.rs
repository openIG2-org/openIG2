//! A scenario's high-score table (re/rules/game-end.md §4–§5): ten (score, name) slots kept in the options
//! registry (`options.dat`, [`Options`]) under `<key>_point_NN`, `<key>_name_NN` and `<key>_crc`, `<key>` the
//! `.scn` file's base name. `LoadHighscores` (exe 0x543750), `SaveHighscores` (exe 0x5539f0), `AddHighscore`
//! (exe 0x543150). A score is a time in `Ido` units (50 a day), lower is better, 0 an empty slot.

use crate::misc_text::DefaultScore;
use crate::options::Options;

pub const SLOTS: usize = 10;

/// `ReadRegistryString` (exe 0x51b4a0) copies at most this many bytes.
const NAME_MAX: usize = 50;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    pub scores: [i32; SLOTS],
    pub names: [String; SLOTS],
}

impl Table {
    /// `LoadHighscores(key, desc)`: the registry's table when all ten pairs are there and `<key>_crc` matches,
    /// else the `fabscr.txt` lines of `key` (case-insensitive), in file order.
    pub fn load(reg: &Options, key: &str, seed: &[DefaultScore]) -> Self {
        if let Some(t) = Self::read(reg, key) {
            return t;
        }
        let mut t = Self::default();
        // The exe writes on past slot 9 unchecked; the shipped file has at most two lines a scenario.
        for (i, d) in seed.iter().filter(|d| d.scenario.eq_ignore_ascii_case(key)).take(SLOTS).enumerate() {
            t.scores[i] = d.score;
            t.names[i] = d.name.clone();
        }
        t
    }

    fn read(reg: &Options, key: &str) -> Option<Self> {
        let crc = atoi(reg.get(&format!("{key}_crc"))?);
        let mut t = Self::default();
        for i in 0..SLOTS {
            t.scores[i] = atoi(reg.get(&format!("{key}_point_{i:02}"))?);
            let name = reg.get(&format!("{key}_name_{i:02}"))?;
            let mut n = name.len().min(NAME_MAX);
            while !name.is_char_boundary(n) {
                n -= 1;
            }
            t.names[i] = name[..n].to_string();
        }
        (t.crc() == crc).then_some(t)
    }

    /// `SaveHighscores(key, desc)`: the 21 keys into the registry (`options.dat` is written by its next save).
    pub fn save(&self, reg: &mut Options, key: &str) {
        for i in 0..SLOTS {
            reg.set(&format!("{key}_point_{i:02}"), &self.scores[i].to_string());
            reg.set(&format!("{key}_name_{i:02}"), &self.names[i]);
        }
        reg.set(&format!("{key}_crc"), &self.crc().to_string());
    }

    /// The check value (`CountSmartCrc(3, ..)`, ELF 0x629010, inlined in the exe): three byte accumulators and one
    /// running index over every slot's four score bytes (little-endian) and its name's bytes.
    pub fn crc(&self) -> i32 {
        let mut a = [0i32; 3];
        let mut j = 0;
        for (s, n) in self.scores.iter().zip(&self.names) {
            for &b in s.to_le_bytes().iter().chain(n.as_bytes()) {
                a[j] ^= b as i32;
                j = (j + 1) % 3;
            }
        }
        a[0] + a[1] * 0x100 + a[2] * 0x10000
    }

    /// `AddHighscore`'s insert: after every non-empty slot with a score `<= score` (ties go below), the rest move
    /// down and the last falls off; `false` when no slot is left. The exe then always saves the table.
    pub fn add(&mut self, name: &str, score: i32) -> bool {
        let Some(i) = (0..SLOTS).find(|&i| self.scores[i] == 0 || self.scores[i] > score) else { return false };
        self.scores[i..].rotate_right(1);
        self.names[i..].rotate_right(1);
        self.scores[i] = score;
        self.names[i] = name.to_string();
        true
    }
}

/// `atoi`: leading blanks, a sign, the digits up to the first other byte.
fn atoi(s: &str) -> i32 {
    let s = s.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let (neg, d) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let v = d.bytes().take_while(u8::is_ascii_digit).fold(0i32, |v, b| v.wrapping_mul(10).wrapping_add((b - b'0') as i32));
    if neg { v.wrapping_neg() } else { v }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed() -> Vec<DefaultScore> {
        crate::misc_text::parse_fabscr("swarm/Fable:20\r\nSWARM/Bob:30\r\nother/Ann:5\r\n")
    }

    #[test]
    fn seeds_without_a_valid_table() {
        let mut reg = Options::default();
        let t = Table::load(&reg, "swarm", &seed());
        assert_eq!((t.scores[..3].to_vec(), t.names[1].as_str()), (vec![1000, 1500, 0], "Bob"));
        t.save(&mut reg, "swarm");
        assert_eq!(Table::load(&reg, "swarm", &[]), t);
        // A changed score breaks the check value: seeded again.
        reg.set("swarm_point_00", "999");
        assert_eq!(Table::load(&reg, "swarm", &[]), Table::default());
    }

    #[test]
    fn add_keeps_the_order_ties_below() {
        let mut t = Table::load(&Options::default(), "swarm", &seed());
        assert!(t.add("Tie", 1000));
        assert!(t.add("Best", 50));
        assert_eq!(t.names[..4].to_vec(), ["Best", "Fable", "Tie", "Bob"]);
        for k in 0..6 {
            assert!(t.add("x", 2000 + k));
        }
        assert!(!t.add("late", 9000));
        assert!(t.add("Mid", 1200));
        assert_eq!((t.names[3].as_str(), t.scores[9]), ("Mid", 2004));
    }

    #[test]
    fn crc_mixes_scores_and_names() {
        let mut t = Table::default();
        t.scores[0] = 0x0102_0304;
        t.names[0] = "AB".into();
        // bytes 04 03 02 01 'A' 'B' to a0 a1 a2 a0 a1 a2; the empty slots' zero bytes change nothing.
        assert_eq!(t.crc(), (4 ^ 1) + (3 ^ 0x41) * 0x100 + (2 ^ 0x42) * 0x10000);
        assert_eq!(atoi(" -12x"), -12);
    }
}
