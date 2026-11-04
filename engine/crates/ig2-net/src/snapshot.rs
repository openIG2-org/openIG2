//! Snapshots, the state hash and our save format (ADR 0007 §3–§4).
//!
//! A snapshot is the synced state at a turn boundary: the whole [`SaveState`], the `Game` fields that persist and
//! the pending sim-originated commands. Its encoding ([`crate::wire`]) is canonical: the state hash is SHA-256 of
//! exactly those bytes, so equal hashes mean equal snapshots.

use std::collections::BTreeMap;

use ig2_sim::game::{Game, GameRules, Libc, MpFormation, RealTime, ViewMode};
use ig2_sim::colony_side::EMPIRES;
use ig2_sim::ground::Field;
use ig2_sim::stats::Stats;
use ig2_sim::story_mods::ModStoryState;
use ig2_sim::save::{HumanSide, Memory, SaveState, StoryQueues, BUILDING_SIZE, DESIGN_SIZE, FLEET_SIZE};
use ig2_sim::story::Queue;
use serde::{Deserialize, Serialize};

use crate::command::Command;
use crate::wire::{self, ByteBuf, Bytes};

/// The save file's magic and format version (bumped whenever an encoded type changes).
pub const MAGIC: &[u8; 8] = b"IG2RSAVE";
pub const VERSION: u16 = 15;
pub const HEADER: usize = 64;
/// The largest state a save, a start or a replay's chunk may inflate to: the header's length is the sender's word.
pub const MAX_STATE: usize = 64 << 20;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not one of our saves (bad magic)")]
    Magic,
    #[error("save format version {0}; this build reads {VERSION}")]
    Version(u16),
    #[error("truncated save")]
    Truncated,
    #[error("corrupt save: {0}")]
    Corrupt(String),
    #[error("the state doesn't match its hash (stored {stored:016x}, computed {computed:016x})")]
    Hash { stored: u64, computed: u64 },
    #[error(transparent)]
    Wire(#[from] wire::Error),
}

/// Fixed-size records encoded as one byte string.
struct Records<'a, const N: usize>(&'a [[u8; N]]);

impl<const N: usize> Serialize for Records<'_, N> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(self.0.as_flattened())
    }
}

fn records<const N: usize>(b: &ByteBuf) -> Result<Vec<[u8; N]>, Error> {
    if b.0.len() % N != 0 {
        return Err(Error::Corrupt(format!("{} bytes of {N}-byte records", b.0.len())));
    }
    Ok(b.0.chunks_exact(N).map(|c| c.try_into().unwrap()).collect())
}

type Entries = Vec<(i32, i32)>;

/// What a snapshot encodes, borrowed from the game (the encoder's side). [`Owned`] mirrors it field by field.
#[derive(Serialize)]
struct View<'a> {
    mem: Vec<(u32, Bytes<'a>)>,
    template: Vec<(&'a str, Bytes<'a>)>,
    fleets: Records<'a, FLEET_SIZE>,
    ships: Records<'a, 6>,
    waypoints: Records<'a, 0xc>,
    designs: Vec<Records<'a, DESIGN_SIZE>>,
    buildings: Vec<Records<'a, BUILDING_SIZE>>,
    story: [&'a Entries; 5],
    buckets: Vec<&'a Entries>,
    multiplayer_blob: bool,
    fresh_buildings: &'a [(usize, usize)],
    libc: u32,
    fleet_name: Option<Bytes<'a>>,
    empire_names: Vec<Bytes<'a>>,
    family_names: Vec<Bytes<'a>>,
    ground_fields: &'a BTreeMap<usize, Field>,
    /// `Game::view`: `None` = classic, `Some(en)` = canonical with that `EN` (since version 2).
    view: Option<u8>,
    /// `SaveState::humans`: `(empire, contacts, ghost, shown, reports)` (since version 3).
    humans: Vec<(u8, Bytes<'a>, Bytes<'a>, i32, Bytes<'a>)>,
    realtime: &'a RealTime,
    pending: &'a [Command],
    /// The other humans' own inboxes (`HumanSide::inbox`, multiplayer.md §5.3; since version 5).
    inboxes: Vec<(u8, Bytes<'a>)>,
    /// The humans' open transfer panels (`SaveState::transfer_panels`; since version 6).
    transfers: &'a BTreeMap<u8, (i32, i32)>,
    /// The MP formation phase (`RealTime::formation`, ADR 0023; since version 7).
    formation: &'a MpFormation,
    /// `Game::spectators` (ADR 0026; since version 8).
    spectators: bool,
    /// `SaveState::ai_difficulty` (ADR 0033; since version 9).
    ai_difficulty: &'a [i32; EMPIRES],
    /// `Game::auto_research` (ADR 0037; since version 10).
    auto_research: &'a [u8; EMPIRES],
    /// `Game::stats` (ADR 0034; since version 11).
    stats: &'a Stats,
    /// `Game::enhanced_ai` (ADR 0045; since version 12).
    enhanced_ai: bool,
    /// `Game::story_fixes` (ADR 0052; since version 13).
    story_fixes: bool,
    /// `SaveState::mod_stories` (ADR 0061; since version 14, repeats and timers since version 15).
    mod_stories: &'a ModStoryState,
}

/// The version 14 story mods' state (ADR 0061 part 1: no repeating stories, no timers).
#[derive(Deserialize)]
struct ModStoryState14 {
    fired: std::collections::BTreeSet<String>,
    flags: std::collections::BTreeSet<String>,
    counters: BTreeMap<String, i32>,
}

impl From<ModStoryState14> for ModStoryState {
    fn from(o: ModStoryState14) -> Self {
        Self { fired: o.fired, flags: o.flags, counters: o.counters, ..Self::default() }
    }
}

/// A state as saved; `I` = the version 5 tail (`()` reads a version 4 state), `T` the version 6 one, `F` the
/// version 7 one, `S` the version 8 one, `D` the version 9 one, `A` the version 10 one, `G` the version 11 one, `H` the
/// version 12 one, `K` the version 13 one, `M` the version 15 one (`ModStoryState14` reads a version 14 one).
#[derive(Deserialize)]
struct Owned<I = Vec<(u8, ByteBuf)>, T = BTreeMap<u8, (i32, i32)>, F = MpFormation, S = bool, D = [i32; EMPIRES], A = [u8; EMPIRES], G = Stats, H = bool, K = bool, M = ModStoryState> {
    mem: Vec<(u32, ByteBuf)>,
    template: Vec<(String, ByteBuf)>,
    fleets: ByteBuf,
    ships: ByteBuf,
    waypoints: ByteBuf,
    designs: Vec<ByteBuf>,
    buildings: Vec<ByteBuf>,
    story: [Entries; 5],
    buckets: Vec<Entries>,
    multiplayer_blob: bool,
    fresh_buildings: Vec<(usize, usize)>,
    libc: u32,
    fleet_name: Option<ByteBuf>,
    empire_names: Vec<ByteBuf>,
    family_names: Vec<ByteBuf>,
    ground_fields: BTreeMap<usize, Field>,
    view: Option<u8>,
    humans: Vec<(u8, ByteBuf, ByteBuf, i32, ByteBuf)>,
    realtime: RealTime,
    pending: Vec<Command>,
    inboxes: I,
    transfers: T,
    formation: F,
    spectators: S,
    ai_difficulty: D,
    auto_research: A,
    stats: G,
    enhanced_ai: H,
    story_fixes: K,
    mod_stories: M,
}

impl<I, T, F, S, D, A, G, H, K, M> Owned<I, T, F, S, D, A, G, H, K, M> {
    /// An older state: a version 4..=13 one (`M = ()`) no story mods' state; a version 4..=12 one (`K = ()`) the original's story; a version 4..=11 one (`H = ()`) the classic computer empires; a version 4..=10 one (`G = ()`)
    /// no statistics; a version 4..=9 one (`A = ()`) no auto research (off); a version 4..=8 one (`D = ()`) no own computer levels (the game's); a
    /// version 4..=7 one (`S = ()`) no spectators (off); a version 4..=6 one (`F = ()`) no formation phase; a version 4
    /// or 5 one (`T = ()`) no transfer panels, a version 4 one (`I = ()`) no inboxes either.
    #[allow(clippy::type_complexity, clippy::too_many_arguments)]
    fn upgrade(self, inboxes: impl FnOnce(I) -> Vec<(u8, ByteBuf)>, transfers: impl FnOnce(T) -> BTreeMap<u8, (i32, i32)>, formation: impl FnOnce(F) -> MpFormation, spectators: impl FnOnce(S) -> bool, difficulty: impl FnOnce(D) -> [i32; EMPIRES], research: impl FnOnce(A) -> [u8; EMPIRES], stats: impl FnOnce(G) -> Stats, ai: impl FnOnce(H) -> bool, fixes: impl FnOnce(K) -> bool, mods: impl FnOnce(M) -> ModStoryState) -> Owned {
        let Owned { mem, template, fleets, ships, waypoints, designs, buildings, story, buckets, multiplayer_blob, fresh_buildings, libc, fleet_name, empire_names, family_names, ground_fields, view, humans, realtime, pending, inboxes: i, transfers: t, formation: f, spectators: s, ai_difficulty: d, auto_research: a, stats: g, enhanced_ai: h, story_fixes: k, mod_stories: m } = self;
        Owned { mem, template, fleets, ships, waypoints, designs, buildings, story, buckets, multiplayer_blob, fresh_buildings, libc, fleet_name, empire_names, family_names, ground_fields, view, humans, realtime, pending, inboxes: inboxes(i), transfers: transfers(t), formation: formation(f), spectators: spectators(s), ai_difficulty: difficulty(d), auto_research: research(a), stats: stats(g), enhanced_ai: ai(h), story_fixes: fixes(k), mod_stories: mods(m) }
    }
}

fn view<'a>(g: &'a Game, pending: &'a [Command]) -> View<'a> {
    let s = &g.state;
    let q = &s.story;
    View {
        mem: s.mem.runs().iter().map(|(&at, b)| (at, Bytes(b))).collect(),
        template: s.template().iter().map(|(n, b)| (n.as_str(), Bytes(b))).collect(),
        fleets: Records(&s.fleets),
        ships: Records(&s.ships),
        waypoints: Records(&s.waypoints),
        designs: s.designs.iter().map(|d| Records(d)).collect(),
        buildings: s.buildings.iter().map(|b| Records(b)).collect(),
        story: [&q.history.entries, &q.starting.entries, &q.read.entries, &q.finished.entries, &q.specials.entries],
        buckets: q.buckets.iter().map(|b| &b.entries).collect(),
        multiplayer_blob: s.multiplayer_blob,
        fresh_buildings: &s.fresh_buildings,
        libc: g.libc.0,
        fleet_name: g.fleet_name.as_deref().map(Bytes),
        empire_names: g.empire_names.iter().map(|n| Bytes(n)).collect(),
        family_names: g.family_names.iter().map(|n| Bytes(n)).collect(),
        ground_fields: g.state.ground_fields(),
        view: match g.view {
            ViewMode::Classic => None,
            ViewMode::Canonical { en } => Some(en),
        },
        humans: s.humans.iter().map(|(&e, h)| (e, Bytes(&h.contacts), Bytes(&h.ghost), h.shown, Bytes(&h.reports))).collect(),
        realtime: &g.realtime,
        pending,
        inboxes: s.humans.iter().filter(|(_, h)| !h.inbox.is_empty()).map(|(&e, h)| (e, Bytes(&h.inbox))).collect(),
        transfers: &s.transfer_panels,
        formation: &g.realtime.formation,
        spectators: g.spectators,
        ai_difficulty: &s.ai_difficulty,
        auto_research: &g.auto_research,
        stats: &g.stats,
        enhanced_ai: g.enhanced_ai,
        story_fixes: g.story_fixes,
        mod_stories: &s.mod_stories,
    }
}

/// [`state_hash`]'s parts, each hashed on its own: which part of two states differs (a desync's diagnosis).
pub fn part_hashes(game: &Game, pending: &[Command]) -> Vec<(&'static str, u64)> {
    let v = view(game, pending);
    vec![
        ("mem", encoded_hash(&v.mem)),
        ("template", encoded_hash(&v.template)),
        ("fleets", encoded_hash(&v.fleets)),
        ("ships", encoded_hash(&v.ships)),
        ("waypoints", encoded_hash(&v.waypoints)),
        ("designs", encoded_hash(&v.designs)),
        ("buildings", encoded_hash(&v.buildings)),
        ("story", encoded_hash(&v.story)),
        ("buckets", encoded_hash(&v.buckets)),
        ("multiplayer_blob", encoded_hash(&v.multiplayer_blob)),
        ("fresh_buildings", encoded_hash(&v.fresh_buildings)),
        ("libc", encoded_hash(&v.libc)),
        ("names", encoded_hash(&(&v.fleet_name, &v.empire_names, &v.family_names))),
        ("ground_fields", encoded_hash(&v.ground_fields)),
        ("view", encoded_hash(&v.view)),
        ("humans", encoded_hash(&v.humans)),
        ("realtime", encoded_hash(&v.realtime)),
        ("pending", encoded_hash(&v.pending)),
        ("inboxes", encoded_hash(&v.inboxes)),
        ("transfers", encoded_hash(&v.transfers)),
        ("formation", encoded_hash(&v.formation)),
        ("spectators", encoded_hash(&v.spectators)),
        ("ai_difficulty", encoded_hash(&v.ai_difficulty)),
        ("auto_research", encoded_hash(&v.auto_research)),
        ("stats", encoded_hash(&v.stats)),
        ("enhanced_ai", encoded_hash(&v.enhanced_ai)),
        ("story_fixes", encoded_hash(&v.story_fixes)),
        ("mod_stories", encoded_hash(&v.mod_stories)),
    ]
}

/// The state hash (ADR 0007 §3): the first 8 bytes of SHA-256 over the snapshot encoding of `game` and `pending`.
pub fn state_hash(game: &Game, pending: &[Command]) -> u64 {
    encoded_hash(&view(game, pending))
}

/// The first 8 bytes of SHA-256 over `x`'s wire encoding.
fn encoded_hash<T: Serialize + ?Sized>(x: &T) -> u64 {
    let mut d = <sha2::Sha256 as sha2::Digest>::new();
    wire::to_sink(x, &mut d).expect("the state encodes");
    digest_u64(&sha2::Digest::finalize(d))
}

/// A SHA-256 digest's first 8 bytes as the `u64` the hashes compare.
fn digest_u64(d: &[u8]) -> u64 {
    u64::from_le_bytes(d[..8].try_into().unwrap())
}

/// Where two snapshots differ (a desync's diagnosis): the parts whose hashes differ, the first `max` differing memory
/// bytes (`address: a vs b`), and the real-time battle or pending commands when those parts differ. Empty: equal.
pub fn diff(a: &Snapshot, b: &Snapshot, rules: &GameRules, max: usize) -> Result<Vec<String>, Error> {
    let ((ga, pa), (gb, pb)) = (a.restore(rules)?, b.restore(rules)?);
    let (ha, hb) = (part_hashes(&ga, &pa), part_hashes(&gb, &pb));
    let parts: Vec<&str> = ha.iter().zip(&hb).filter(|(x, y)| x.1 != y.1).map(|(x, _)| x.0).collect();
    if parts.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = vec![format!("parts differ: {parts:?}")];
    let (ma, mb) = (ga.state.mem.runs(), gb.state.mem.runs());
    let mut n = 0;
    for (at, x) in ma {
        let Some(y) = mb.get(at) else {
            out.push(format!("run {at:#x} only on the first"));
            continue;
        };
        for i in 0..x.len().max(y.len()) {
            if x.get(i) != y.get(i) && n < max {
                out.push(format!("{:#x}: {:?} vs {:?}", at + i as u32, x.get(i), y.get(i)));
                n += 1;
            }
        }
    }
    if parts.contains(&"realtime") {
        out.push(format!("realtime: {:?} vs {:?}", ga.realtime.current, gb.realtime.current));
    }
    if parts.contains(&"pending") {
        out.push(format!("pending: {pa:?} vs {pb:?}"));
    }
    Ok(out)
}

fn hash_bytes(b: &[u8]) -> u64 {
    digest_u64(&<sha2::Sha256 as sha2::Digest>::digest(b))
}

/// The synced state at a turn boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// The next turn to run and the sim ticks run so far.
    pub turn: u64,
    pub tick: u64,
    /// [`state_hash`] of the state.
    pub hash: u64,
    /// The encoded state.
    pub state: Vec<u8>,
}

/// The fixed part of a save file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Header {
    pub version: u16,
    /// Bit 0: a multiplayer save.
    pub flags: u16,
    pub turn: u64,
    pub tick: u64,
    pub hash: u64,
    /// The rules the state was made with (a hash of the user's rule tables, ADR 0004/0005); the loader decides
    /// what a mismatch means.
    pub rules: u64,
}

impl Snapshot {
    pub fn capture(game: &Game, pending: &[Command], turn: u64, tick: u64) -> Self {
        let state = wire::to_vec(&view(game, pending)).expect("the state encodes");
        Self { turn, tick, hash: hash_bytes(&state), state }
    }

    /// The game and its pending sim-originated commands. `uninit` stays `None`: appended records start zeroed, the
    /// same on every peer. The per-peer parts (`Game::local`, `Game::cosmetic`) are the joiner's to set.
    pub fn restore<'r>(&self, rules: &'r GameRules) -> Result<(Game<'r>, Vec<Command>), Error> {
        let o: Owned = match wire::from_slice(&self.state) {
            Ok(o) => o,
            Err(e) => wire::from_slice::<Owned<Vec<(u8, ByteBuf)>, BTreeMap<u8, (i32, i32)>, MpFormation, bool, [i32; EMPIRES], [u8; EMPIRES], Stats, bool, bool, ModStoryState14>>(&self.state)
                .map(|o| o.upgrade(|i| i, |t| t, |f| f, |s| s, |d| d, |a| a, |g| g, |h| h, |k| k, ModStoryState::from))
                .or_else(|_| wire::from_slice::<Owned<Vec<(u8, ByteBuf)>, BTreeMap<u8, (i32, i32)>, MpFormation, bool, [i32; EMPIRES], [u8; EMPIRES], Stats, bool, bool, ()>>(&self.state).map(|o| o.upgrade(|i| i, |t| t, |f| f, |s| s, |d| d, |a| a, |g| g, |h| h, |k| k, |()| ModStoryState::default())))
                .or_else(|_| wire::from_slice::<Owned<Vec<(u8, ByteBuf)>, BTreeMap<u8, (i32, i32)>, MpFormation, bool, [i32; EMPIRES], [u8; EMPIRES], Stats, bool, (), ()>>(&self.state).map(|o| o.upgrade(|i| i, |t| t, |f| f, |s| s, |d| d, |a| a, |g| g, |h| h, |()| false, |()| ModStoryState::default())))
                .or_else(|_| wire::from_slice::<Owned<Vec<(u8, ByteBuf)>, BTreeMap<u8, (i32, i32)>, MpFormation, bool, [i32; EMPIRES], [u8; EMPIRES], Stats, (), (), ()>>(&self.state).map(|o| o.upgrade(|i| i, |t| t, |f| f, |s| s, |d| d, |a| a, |g| g, |()| false, |()| false, |()| ModStoryState::default())))
                .or_else(|_| wire::from_slice::<Owned<Vec<(u8, ByteBuf)>, BTreeMap<u8, (i32, i32)>, MpFormation, bool, [i32; EMPIRES], [u8; EMPIRES], (), (), (), ()>>(&self.state).map(|o| o.upgrade(|i| i, |t| t, |f| f, |s| s, |d| d, |a| a, |()| Stats::default(), |()| false, |()| false, |()| ModStoryState::default())))
                .or_else(|_| wire::from_slice::<Owned<Vec<(u8, ByteBuf)>, BTreeMap<u8, (i32, i32)>, MpFormation, bool, [i32; EMPIRES], (), (), (), (), ()>>(&self.state).map(|o| o.upgrade(|i| i, |t| t, |f| f, |s| s, |d| d, |()| [0; EMPIRES], |()| Stats::default(), |()| false, |()| false, |()| ModStoryState::default())))
                .or_else(|_| wire::from_slice::<Owned<Vec<(u8, ByteBuf)>, BTreeMap<u8, (i32, i32)>, MpFormation, bool, (), (), (), (), (), ()>>(&self.state).map(|o| o.upgrade(|i| i, |t| t, |f| f, |s| s, |()| [0; EMPIRES], |()| [0; EMPIRES], |()| Stats::default(), |()| false, |()| false, |()| ModStoryState::default())))
                .or_else(|_| wire::from_slice::<Owned<Vec<(u8, ByteBuf)>, BTreeMap<u8, (i32, i32)>, MpFormation, (), (), (), (), (), (), ()>>(&self.state).map(|o| o.upgrade(|i| i, |t| t, |f| f, |()| false, |()| [0; EMPIRES], |()| [0; EMPIRES], |()| Stats::default(), |()| false, |()| false, |()| ModStoryState::default())))
                .or_else(|_| wire::from_slice::<Owned<Vec<(u8, ByteBuf)>, BTreeMap<u8, (i32, i32)>, (), (), (), (), (), (), (), ()>>(&self.state).map(|o| o.upgrade(|i| i, |t| t, |()| MpFormation::default(), |()| false, |()| [0; EMPIRES], |()| [0; EMPIRES], |()| Stats::default(), |()| false, |()| false, |()| ModStoryState::default())))
                .or_else(|_| wire::from_slice::<Owned<Vec<(u8, ByteBuf)>, (), (), (), (), (), (), (), (), ()>>(&self.state).map(|o| o.upgrade(|i| i, |()| BTreeMap::new(), |()| MpFormation::default(), |()| false, |()| [0; EMPIRES], |()| [0; EMPIRES], |()| Stats::default(), |()| false, |()| false, |()| ModStoryState::default())))
                .or_else(|_| wire::from_slice::<Owned<(), (), (), (), (), (), (), (), (), ()>>(&self.state).map(|o| o.upgrade(|()| Vec::new(), |()| BTreeMap::new(), |()| MpFormation::default(), |()| false, |()| [0; EMPIRES], |()| [0; EMPIRES], |()| Stats::default(), |()| false, |()| false, |()| ModStoryState::default())))
                .map_err(|_| e)?,
        };
        let mut s = SaveState::from_parts(Memory::from_runs(o.mem.into_iter().map(|(at, b)| (at, b.0)).collect()), o.template.into_iter().map(|(n, b)| (n, b.0)).collect());
        s.fleets = records(&o.fleets)?;
        s.ships = records(&o.ships)?;
        s.waypoints = records(&o.waypoints)?;
        s.designs = o.designs.iter().map(records).collect::<Result<_, _>>()?;
        s.buildings = o.buildings.iter().map(records).collect::<Result<_, _>>()?;
        let [history, starting, read, finished, specials] = o.story.map(|entries| Queue { entries });
        s.story = StoryQueues {
            history,
            starting,
            read,
            finished,
            specials,
            buckets: o.buckets.into_iter().map(|entries| Queue { entries }).collect(),
        };
        s.multiplayer_blob = o.multiplayer_blob;
        s.fresh_buildings = o.fresh_buildings;
        s.set_ground_fields(o.ground_fields);
        s.humans = o
            .humans
            .into_iter()
            .map(|(e, contacts, ghost, shown, reports)| (e, HumanSide { contacts: contacts.0, ghost: ghost.0, shown, reports: reports.0, inbox: Vec::new() }))
            .collect();
        for (e, inbox) in o.inboxes {
            s.humans.entry(e).or_default().inbox = inbox.0;
        }
        s.transfer_panels = o.transfers;
        s.ai_difficulty = o.ai_difficulty;
        let mut g = Game::new(s, rules);
        g.libc = Libc(o.libc);
        g.fleet_name = o.fleet_name.map(|b| b.0);
        g.empire_names = o.empire_names.into_iter().map(|b| b.0).collect();
        g.family_names = o.family_names.into_iter().map(|b| b.0).collect();
        if let Some(en) = o.view {
            g.canonical_view(en);
        }
        g.realtime = o.realtime;
        g.realtime.formation = o.formation;
        g.spectators = o.spectators;
        g.auto_research = o.auto_research;
        g.stats = o.stats;
        g.enhanced_ai = o.enhanced_ai;
        g.story_fixes = o.story_fixes;
        g.state.mod_stories = o.mod_stories;
        Ok((g, o.pending))
    }

    /// The save file: [`HEADER`] bytes, then the zlib-compressed state.
    pub fn to_file(&self, rules: u64, multiplayer: bool) -> Vec<u8> {
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&self.state, 6);
        let mut out = Vec::with_capacity(HEADER + z.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(multiplayer as u16).to_le_bytes());
        for v in [self.turn, self.tick, self.hash, rules] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&(z.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.state.len() as u32).to_le_bytes());
        out.resize(HEADER, 0);
        out.extend_from_slice(&z);
        out
    }

    /// A save file back; the state must match the stored hash. Bytes after the state are left alone (an MP save's
    /// session, `session::SavedSession`).
    pub fn from_file(b: &[u8]) -> Result<(Self, Header), Error> {
        let h = Header::read(b)?;
        let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize;
        let (zlen, raw) = (u32_at(44), u32_at(48));
        if raw > MAX_STATE {
            return Err(Error::Corrupt(format!("{raw} state bytes, at most {MAX_STATE}")));
        }
        let z = b.get(HEADER..HEADER.saturating_add(zlen)).ok_or(Error::Truncated)?;
        let state = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(z, raw).map_err(|e| Error::Corrupt(format!("{e:?}")))?;
        if state.len() != raw {
            return Err(Error::Corrupt(format!("{} state bytes, header says {raw}", state.len())));
        }
        let computed = hash_bytes(&state);
        if computed != h.hash {
            return Err(Error::Hash { stored: h.hash, computed });
        }
        Ok((Self { turn: h.turn, tick: h.tick, hash: h.hash, state }, h))
    }

    /// The length of the save file [`Snapshot::from_file`] reads from the front of `b`.
    pub fn file_len(b: &[u8]) -> Result<usize, Error> {
        Header::read(b)?;
        Ok(HEADER.saturating_add(u32::from_le_bytes(b[44..48].try_into().unwrap()) as usize))
    }
}

impl Header {
    /// A save file's header (magic and version checked; the state isn't read).
    pub fn read(b: &[u8]) -> Result<Self, Error> {
        if b.len() < HEADER {
            return Err(if b.starts_with(&MAGIC[..b.len().min(8)]) { Error::Truncated } else { Error::Magic });
        }
        if &b[..8] != MAGIC {
            return Err(Error::Magic);
        }
        let u16_at = |o: usize| u16::from_le_bytes(b[o..o + 2].try_into().unwrap());
        let u64_at = |o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
        let h = Header { version: u16_at(8), flags: u16_at(10), turn: u64_at(12), tick: u64_at(20), hash: u64_at(28), rules: u64_at(36) };
        // Version 4 states lack the inboxes, 4 and 5 the transfer panels, 4..=6 the formation phase (read as none),
        // 4..=7 the spectators option (off), 4..=8 the computers' own levels (the game's), 4..=9 auto research (off), 4..=10 the statistics (none), 4..=11 the computer empires' ruleset (classic), 4..=12 the story fixes (off), 4..=13 the story mods' state (none).
        if !(4..=VERSION).contains(&h.version) {
            return Err(Error::Version(h.version));
        }
        Ok(h)
    }
}
