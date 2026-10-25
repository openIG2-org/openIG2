//! Snapshots, the state hash and our save format (ADR 0007 §3–§4).
//!
//! A snapshot is the synced state at a turn boundary: the whole [`SaveState`], the `Game` fields that persist and
//! the pending sim-originated commands. Its encoding ([`crate::wire`]) is canonical: the state hash is SHA-256 of
//! exactly those bytes, so equal hashes mean equal snapshots.

use std::collections::BTreeMap;

use ig2_sim::game::{Game, GameRules, Libc, RealTime, ViewMode};
use ig2_sim::ground::Field;
use ig2_sim::save::{HumanSide, Memory, SaveState, StoryQueues, BUILDING_SIZE, DESIGN_SIZE, FLEET_SIZE};
use ig2_sim::story::Queue;
use serde::{Deserialize, Serialize};

use crate::command::Command;
use crate::wire::{self, ByteBuf, Bytes};

/// The save file's magic and format version (bumped whenever an encoded type changes).
pub const MAGIC: &[u8; 8] = b"IG2RSAVE";
pub const VERSION: u16 = 5;
pub const HEADER: usize = 64;

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
}

/// A state as saved; `I` = the version 5 tail (`()` reads a version 4 state).
#[derive(Deserialize)]
struct Owned<I = Vec<(u8, ByteBuf)>> {
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
}

impl Owned<()> {
    fn upgrade(self) -> Owned {
        let Owned { mem, template, fleets, ships, waypoints, designs, buildings, story, buckets, multiplayer_blob, fresh_buildings, libc, fleet_name, empire_names, family_names, ground_fields, view, humans, realtime, pending, .. } = self;
        Owned { mem, template, fleets, ships, waypoints, designs, buildings, story, buckets, multiplayer_blob, fresh_buildings, libc, fleet_name, empire_names, family_names, ground_fields, view, humans, realtime, pending, inboxes: Vec::new() }
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
    }
}

/// [`state_hash`]'s parts, each hashed on its own: which part of two states differs (a desync's diagnosis).
pub fn part_hashes(game: &Game, pending: &[Command]) -> Vec<(&'static str, u64)> {
    fn h<T: Serialize + ?Sized>(x: &T) -> u64 {
        let mut d = <sha2::Sha256 as sha2::Digest>::new();
        wire::to_sink(x, &mut d).expect("the state encodes");
        u64::from_le_bytes(sha2::Digest::finalize(d)[..8].try_into().unwrap())
    }
    let v = view(game, pending);
    vec![
        ("mem", h(&v.mem)),
        ("template", h(&v.template)),
        ("fleets", h(&v.fleets)),
        ("ships", h(&v.ships)),
        ("waypoints", h(&v.waypoints)),
        ("designs", h(&v.designs)),
        ("buildings", h(&v.buildings)),
        ("story", h(&v.story)),
        ("buckets", h(&v.buckets)),
        ("multiplayer_blob", h(&v.multiplayer_blob)),
        ("fresh_buildings", h(&v.fresh_buildings)),
        ("libc", h(&v.libc)),
        ("names", h(&(&v.fleet_name, &v.empire_names, &v.family_names))),
        ("ground_fields", h(&v.ground_fields)),
        ("view", h(&v.view)),
        ("humans", h(&v.humans)),
        ("realtime", h(&v.realtime)),
        ("pending", h(&v.pending)),
        ("inboxes", h(&v.inboxes)),
    ]
}

/// The state hash (ADR 0007 §3): the first 8 bytes of SHA-256 over the snapshot encoding of `game` and `pending`.
pub fn state_hash(game: &Game, pending: &[Command]) -> u64 {
    let mut h = <sha2::Sha256 as sha2::Digest>::new();
    wire::to_sink(&view(game, pending), &mut h).expect("the state encodes");
    let out = sha2::Digest::finalize(h);
    u64::from_le_bytes(out[..8].try_into().unwrap())
}

fn hash_bytes(b: &[u8]) -> u64 {
    let out = <sha2::Sha256 as sha2::Digest>::digest(b);
    u64::from_le_bytes(out[..8].try_into().unwrap())
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
            Err(e) => wire::from_slice::<Owned<()>>(&self.state).map(Owned::upgrade).map_err(|_| e)?,
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
        let mut g = Game::new(s, rules);
        g.libc = Libc(o.libc);
        g.fleet_name = o.fleet_name.map(|b| b.0);
        g.empire_names = o.empire_names.into_iter().map(|b| b.0).collect();
        g.family_names = o.family_names.into_iter().map(|b| b.0).collect();
        if let Some(en) = o.view {
            g.canonical_view(en);
        }
        g.realtime = o.realtime;
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
        let z = b.get(HEADER..HEADER + zlen).ok_or(Error::Truncated)?;
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
        Ok(HEADER + u32::from_le_bytes(b[44..48].try_into().unwrap()) as usize)
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
        // Version 4 states lack the inboxes (read as none).
        if h.version != VERSION && h.version != 4 {
            return Err(Error::Version(h.version));
        }
        Ok(h)
    }
}
