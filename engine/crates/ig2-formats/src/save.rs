//! Original save games (`save<n>.sav`) and the multiplayer state blob. Spec: `re/formats/save.md`.
//!
//! A file is an 80-byte raw header (`VERS0002`, the save's name, a few calendar globals) followed by one zlib
//! stream. Inside it every saved variable is a chunk `[i32 length][bytes]` written by `LoadSaveVariable` (exe
//! 0x5440d0) in the fixed order of `LoadSaveVariables` (exe 0x544220); counts saved earlier in the stream size
//! the later variable-length chunks. The multiplayer blob (`GetSavedGame` exe 0x543670) is the same stream
//! without the file header. Win32 and x64 builds write the same stream (§7 of the spec).
//!
//! [`SaveGame::parse`] walks the whole stream against that schema (checking every chunk's length), keeps a
//! named list of the chunks and lifts the sections the simulation already models (planets, fleets, ships,
//! vehicles, designs, buildings, techs, spies, ground battles) into raw records with small typed views.

use std::ops::Range;

use crate::{format_err, Result};

const FMT: &str = "save";

/// The version `LoadSave` writes (and the only one observed); older gates are kept as in the exe.
pub const VERSION: i32 = 0x6a;
/// `"VERS0002"` + NUL.
pub const MAGIC: &[u8; 9] = b"VERS0002\0";
/// The raw file header before the zlib stream.
pub const HEADER_LEN: usize = 80;

pub const EMPIRES: usize = 19;
pub const PLANET_SLOTS: usize = 201;
pub const TECHS: usize = 193;
pub const BATTLEFIELDS: usize = 200;

pub const PLANET_SIZE: usize = 0x188;
pub const FLEET_SIZE: usize = 0x80;
pub const SHIP_SIZE: usize = 6;
pub const VEHICLE_SIZE: usize = 8;
pub const WAYPOINT_SIZE: usize = 0xc;
pub const DESIGN_SIZE: usize = 0x84;
pub const BUILDING_SIZE: usize = 0x20;
pub const SPY_SIZE: usize = 0x4c;

/// The raw header in front of the zlib stream (`LoadSave` exe 0x543c80), all little-endian i32 after the name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// 31 bytes: up to 30 name characters, NUL-padded by `strncpy`; byte 30 is uninitialised stack.
    pub name: [u8; 31],
    pub version: i32,
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub ido: i32,
    pub old_day: i32,
    /// `EN` (the human's empire) and `CampaignType`: present from version 101.
    pub en: Option<i32>,
    pub campaign_type: Option<i32>,
    /// zlib stream length, then the *buffer capacity* the game compressed (≥ the used stream, zero padded).
    pub compressed_len: u32,
    pub decompressed_len: u32,
}

impl Header {
    /// The header alone (`LoadTitle` exe 0x553590: what the Options screen's slot list reads).
    pub fn read(file: &[u8]) -> Result<Self> {
        if file.len() < HEADER_LEN || &file[..9] != MAGIC {
            return Err(format_err(FMT, "missing VERS0002 header"));
        }
        let at = |o: usize| rd_i32(file, o);
        let version = at(40);
        // From version 101 the header carries EN and CampaignType; without them the lengths move up by 8.
        let (en, campaign_type, lens) = if version > 100 { (Some(at(64)), Some(at(68)), 72) } else { (None, None, 64) };
        // Only VERS0002 compresses (`atoi("0002") == 2`); other magics would be an uncompressed stream.
        Ok(Self {
            name: file[9..40].try_into().unwrap(),
            version,
            year: at(44),
            month: at(48),
            day: at(52),
            ido: at(56),
            old_day: at(60),
            en,
            campaign_type,
            compressed_len: at(lens) as u32,
            decompressed_len: at(lens + 4) as u32,
        })
    }

    pub fn name_str(&self) -> String {
        let n = self.name[..30].iter().position(|&b| b == 0).unwrap_or(30);
        String::from_utf8_lossy(&self.name[..n]).into_owned()
    }
}

/// One `LoadSaveVariable` chunk: its schema name and where its payload sits in [`SaveGame::body`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub name: String,
    pub range: Range<usize>,
}

/// The per-empire part of one tech record (`Talalmanyok[t]`, 0x114 B) that the save carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechState {
    /// `+0x45 + e` status per empire.
    pub status: [u8; EMPIRES],
    /// `+0x58 + 4e` research time spent per empire.
    pub time: [i32; EMPIRES],
    /// `+0xa4 + e` given/traded flags.
    pub given: [u8; EMPIRES],
    /// `+0x2c..+0x3c`, four i32.
    pub fields: [i32; 4],
    /// `+0xc0 + 9i + o` seen matrix (81 bytes).
    pub seen: [u8; 81],
}

/// One ground-war vehicle group (`WF_Tank`, `WF_Tank::LoadSave` exe 0x4859a0): the framing fields and arrays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundUnit {
    /// `+0x188` group flags; bit 0x4000000 = a short record (most per-unit state is not saved).
    pub flags: u32,
    /// `+0x190` unit count, `+0x194` owner, `+0x198` side.
    pub count: i32,
    pub owner: i32,
    pub side: i32,
    /// `+0x1d0` weapon mask: one ammunition word per set bit 0..13 (twice, two arrays).
    pub weapons: u32,
    /// `+0x154` hit points and `+0x19c` origins, `count` i32 each.
    pub hp: Vec<i32>,
    pub origin: Vec<i32>,
    /// Chunk index range of this group in [`SaveGame::chunks`].
    pub chunks: Range<usize>,
}

/// One battlefield (`WF`, exe 0x9C3B30 + i·0x8EC; `WF::LoadSave` exe 0x485420).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Battlefield {
    /// `+0x180` unit slots, `+0xe0/+0xe4/+0xe8` rocket/paralyser/laser slots.
    pub unit_slots: i32,
    pub rockets: i32,
    pub paralysers: i32,
    pub lasers: i32,
    /// `+0x188` alive byte per unit slot.
    pub alive: Vec<u8>,
    pub units: Vec<GroundUnit>,
    pub chunks: Range<usize>,
}

/// A parsed save stream.
#[derive(Debug, Clone)]
pub struct SaveGame {
    /// The file header (`None` for a multiplayer blob).
    pub header: Option<Header>,
    /// Written by `GetSavedGame`: carries `Multi_ido`/`Multiplayer_speed` and the receiving player in place of `EN`.
    pub multiplayer: bool,
    /// The decompressed stream (the game's whole buffer, padding included).
    pub body: Vec<u8>,
    /// Bytes of `body` the chunks use; the rest is the buffer's padding.
    pub used: usize,
    pub chunks: Vec<Chunk>,

    pub version: i32,
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub ido: i32,
    pub ido2: i32,
    pub old_day: i32,
    pub difficulty: i32,
    /// `EN`, or in a multiplayer blob `Multiplayer_save_player`.
    pub en: i32,
    pub campaign_type: i32,
    /// `Multi_ido`, `Multiplayer_speed` (multiplayer blob only).
    pub mp_time: Option<(i32, i32)>,
    /// `Money` (19 × f64).
    pub money: [f64; EMPIRES],
    /// `RandSeed` (21 stream seeds).
    pub rand_seed: [u32; 21],
    pub player_ki: [u8; EMPIRES],
    pub player_faj: [u8; EMPIRES],
    pub empire_alive: [u8; EMPIRES],
    /// `Kemek[e][0..=OsszHanyKem[e]]` for e = 1..=8 (index 0 empty).
    pub spies: Vec<Vec<[u8; SPY_SIZE]>>,
    /// `Flottak[0..=OsszHanyFlotta]`, `Hajok[0..=OsszHanyHajo]`, `Tankok[0..=OsszHanyTank]`.
    pub fleets: Vec<[u8; FLEET_SIZE]>,
    pub ships: Vec<[u8; SHIP_SIZE]>,
    pub vehicles: Vec<[u8; VEHICLE_SIZE]>,
    pub waypoints: Vec<[u8; WAYPOINT_SIZE]>,
    /// `Bolygok[0..=HanyBolygo]` (index 0 is the unused slot 0).
    pub planets: Vec<[u8; PLANET_SIZE]>,
    /// Index 1..=192 (index 0 empty).
    pub techs: Vec<TechState>,
    /// `Tervek[e][0..=OsszHanyTerv[e]]` for e = 1..=18 (index 0 empty).
    pub designs: Vec<Vec<[u8; DESIGN_SIZE]>>,
    /// `HanyEpulet`, `HanyEpuletLefoglalva`, `HanyEpuletBlokk` (201 i32 each).
    pub building_counts: Vec<i32>,
    pub building_slots: Vec<i32>,
    pub building_blocks: Vec<i32>,
    /// `Epuletek[p][0..=HanyEpuletBlokk[p]]` for p = 1..=200 (index 0 empty).
    pub buildings: Vec<Vec<[u8; BUILDING_SIZE]>>,
    pub battlefields: Vec<Battlefield>,
}

// ------------------------------------------------------------------------------------------------
// Stream walker

/// The chunk source of [`SaveGame::write_stream`]: `(name, len)` → the chunk's `len` bytes.
pub type ChunkSource<'s> = dyn FnMut(&str, usize) -> Vec<u8> + 's;

/// Walks the stream with the schema: reads `body` (parse), or with a `source` writes each chunk into `body` as
/// `LoadSaveVariable` saves it (write).
struct Walker<'s> {
    body: Vec<u8>,
    pos: usize,
    chunks: Vec<Chunk>,
    source: Option<&'s mut ChunkSource<'s>>,
}

impl Walker<'_> {
    /// The next chunk, whose length must be `len` (`LoadSaveVariable` writes nothing for `len <= 0`).
    fn take(&mut self, name: impl Into<String>, len: i64) -> Result<Vec<u8>> {
        if len <= 0 {
            return Ok(Vec::new());
        }
        let name = name.into();
        let at = self.pos;
        if let Some(src) = self.source.as_mut() {
            let data = src(&name, len as usize);
            if data.len() != len as usize {
                return Err(format_err(FMT, format!("{name}: the source gave {} bytes, expected {len}", data.len())));
            }
            self.body.extend_from_slice(&(len as i32).to_le_bytes());
            self.body.extend_from_slice(&data);
            self.pos = self.body.len();
            self.chunks.push(Chunk { name, range: at + 4..self.pos });
            return Ok(data);
        }
        let Some(head) = self.body.get(at..at + 4) else {
            return Err(format_err(FMT, format!("{name}: stream ends at {at:#x}")));
        };
        let got = i32::from_le_bytes(head.try_into().unwrap()) as i64;
        if got != len {
            return Err(format_err(FMT, format!("{name}: chunk at {at:#x} holds {got} bytes, expected {len}")));
        }
        let range = at + 4..at + 4 + len as usize;
        let Some(data) = self.body.get(range.clone()) else {
            return Err(format_err(FMT, format!("{name}: chunk at {at:#x} runs past the stream")));
        };
        let data = data.to_vec();
        self.pos = range.end;
        self.chunks.push(Chunk { name, range });
        Ok(data)
    }
    fn i32(&mut self, name: impl Into<String>) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(name, 4)?.try_into().unwrap()))
    }
    fn i32s(&mut self, name: impl Into<String>, n: usize) -> Result<Vec<i32>> {
        Ok(i32s(self.take(name, 4 * n as i64)?))
    }
    /// A chunk whose length is the previous chunk's value (strings).
    fn sized(&mut self, name: &str) -> Result<Vec<u8>> {
        let n = self.i32(format!("{name}.len"))?;
        self.take(name, n as i64)
    }
    fn records<const N: usize>(&mut self, name: impl Into<String>, count: i64) -> Result<Vec<[u8; N]>> {
        let data = self.take(name, count * N as i64)?;
        Ok(data.chunks_exact(N).map(|c| c.try_into().unwrap()).collect())
    }
    fn mark(&self) -> usize {
        self.chunks.len()
    }
}

fn i32s(b: impl AsRef<[u8]>) -> Vec<i32> {
    b.as_ref().chunks_exact(4).map(|c| i32::from_le_bytes(c.try_into().unwrap())).collect()
}
fn rd_i32(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn rd_i16(b: &[u8], o: usize) -> i16 {
    i16::from_le_bytes([b[o], b[o + 1]])
}
fn rd_f32(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl SaveGame {
    /// Parse a `save<n>.sav` file.
    pub fn parse(file: &[u8]) -> Result<Self> {
        let header = Header::read(file)?;
        let start = if header.en.is_some() { 80 } else { 72 };
        let end = start + header.compressed_len as usize;
        let Some(z) = file.get(start..end) else {
            return Err(format_err(FMT, "zlib stream runs past the file"));
        };
        let body = inflate(z)?;
        if body.len() != header.decompressed_len as usize {
            return Err(format_err(FMT, format!("stream inflates to {} bytes, header says {}", body.len(), header.decompressed_len)));
        }
        let mut save = Self::parse_stream(body, false)?;
        if save.version != header.version {
            return Err(format_err(FMT, format!("stream version {} != header version {}", save.version, header.version)));
        }
        save.header = Some(header);
        Ok(save)
    }

    /// Parse a multiplayer state blob (`GetSavedGame`: `[u32 zlib len][u32 buffer len][zlib]`).
    pub fn parse_multiplayer(blob: &[u8]) -> Result<Self> {
        if blob.len() < 8 {
            return Err(format_err(FMT, "multiplayer blob too short"));
        }
        let clen = rd_i32(blob, 0) as usize;
        let Some(z) = blob.get(8..8 + clen) else {
            return Err(format_err(FMT, "zlib stream runs past the blob"));
        };
        Self::parse_stream(inflate(z)?, true)
    }

    /// Walk a decompressed stream (`LoadSaveVariables`, exe 0x544220).
    pub fn parse_stream(body: Vec<u8>, multiplayer: bool) -> Result<Self> {
        let mut w = Walker { body, pos: 0, chunks: Vec::new(), source: None };
        let fields = walk(&mut w, multiplayer)?;
        let Walker { body, pos: used, chunks, .. } = w;
        if body[used..].iter().any(|&b| b != 0) {
            // The padding is the game's buffer tail (fresh allocation); nonzero bytes mean a schema mismatch.
            return Err(format_err(FMT, format!("{} unparsed nonzero bytes after {used:#x}", body[used..].iter().filter(|&&b| b != 0).count())));
        }
        Ok(Self { header: None, multiplayer, body, used, chunks, ..fields })
    }

    /// Write a stream (no padding) as `LoadSaveVariables` saves it: every chunk in schema order comes from
    /// `source(name, len)`; the counts it returns size the later chunks, as in a parse.
    pub fn write_stream(multiplayer: bool, source: &mut ChunkSource<'_>) -> Result<Self> {
        let mut w = Walker { body: Vec::new(), pos: 0, chunks: Vec::new(), source: Some(source) };
        let fields = walk(&mut w, multiplayer)?;
        let Walker { body, pos: used, chunks, .. } = w;
        Ok(Self { header: None, multiplayer, body, used, chunks, ..fields })
    }

    /// The first chunk with this schema name.
    pub fn chunk(&self, name: &str) -> Option<&[u8]> {
        self.chunks.iter().find(|c| c.name == name).map(|c| &self.body[c.range.clone()])
    }

    /// Rebuild a `save<n>.sav` file from the (possibly edited) header and stream. The zlib bytes differ from
    /// the game's encoder; the decompressed stream is the same.
    pub fn to_file(&self) -> Option<Vec<u8>> {
        let h = self.header.as_ref()?;
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&self.body, 6);
        let mut out = Vec::with_capacity(HEADER_LEN + z.len());
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&h.name);
        for v in [h.version, h.year, h.month, h.day, h.ido, h.old_day] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        if let (Some(en), Some(ct)) = (h.en, h.campaign_type) {
            out.extend_from_slice(&en.to_le_bytes());
            out.extend_from_slice(&ct.to_le_bytes());
        }
        out.extend_from_slice(&(z.len() as u32).to_le_bytes());
        out.extend_from_slice(&(self.body.len() as u32).to_le_bytes());
        out.extend_from_slice(&z);
        Some(out)
    }

    pub fn planet(&self, p: usize) -> PlanetView<'_> {
        PlanetView(&self.planets[p])
    }
    pub fn fleet(&self, f: usize) -> FleetView<'_> {
        FleetView(&self.fleets[f])
    }
    pub fn ship(&self, i: usize) -> ShipView {
        ShipView::from_bytes(&self.ships[i])
    }
    pub fn vehicle(&self, i: usize) -> VehicleView {
        VehicleView::from_bytes(&self.vehicles[i])
    }
    pub fn design(&self, e: usize, d: usize) -> DesignView<'_> {
        DesignView(&self.designs[e][d])
    }
    pub fn building(&self, p: usize, k: usize) -> BuildingView<'_> {
        BuildingView(&self.buildings[p][k])
    }
}

fn inflate(z: &[u8]) -> Result<Vec<u8>> {
    miniz_oxide::inflate::decompress_to_vec_zlib(z).map_err(|e| format_err(FMT, format!("zlib: {e:?}")))
}

/// The schema. Each step names the exe function and global it mirrors (spec §4–§6).
fn walk(w: &mut Walker, mp: bool) -> Result<SaveGame> {
    let version = w.i32("version")?;
    let v = version;
    let year = w.i32("Ev")?;
    let month = w.i32("Honap")?;
    let day = w.i32("Nap")?;
    let ido = w.i32("Ido")?;
    if v > 0x68 {
        w.i32("IdoMultiPlayerBattle")?;
    }
    let old_day = w.i32("OldNap")?;
    let difficulty = w.i32("Nehezseg")?;
    let ido2 = w.i32("Ido2")?;
    if v > 0x5b {
        w.i32("scenario_kezdesi_ido")?;
        w.take("scenario_registry_name", 0x1e)?;
    }
    let mp_time = if mp { Some((w.i32("Multi_ido")?, w.i32("Multiplayer_speed")?)) } else { None };

    // Diplomacy_Save (exe 0x4c0080).
    for (n, len) in [("AktPlayerek", 0xf50), ("BirodalmiMacskak", 0x4c), ("BirodalmiPapagajok", 0x4c),
        ("BirodalmiEvfordulok", 0x4c), ("BirodalmiGyengesegek", 0x4c), ("HanyevesAzUralkodo", 0x4c),
        ("VoltmarUdvozloUzenetEbbenAzEvben", 0x13), ("AktDipUzenet", 0x34), ("AktDipValasz", 0x30),
        ("SpecialListaMut", 4), ("SpecialListaMut", 4)] {
        w.take(n, len)?;
    }
    if v < 0x60 {
        w.i32("Diplomacy.unused")?;
    }
    for (n, len) in [("PlayerSelected", 4), ("PlayerSelectedOld", 4), ("MostHanyPlayerVan", 4), ("SpecialLista", 0x960),
        ("MaxPlayerek", 4), ("DiplomaciaEngedelyezve", 1), ("TradeEngedelyezve", 1), ("SpyEngedelyezve", 1)] {
        w.take(n, len)?;
    }
    // Info_Save (exe 0x4ea780).
    for (n, len) in [("Info_SelectedChart", 4), ("Info_ChartFelbontasSelected", 4), ("InfoTriX", 4), ("InfoTriY", 4),
        ("InfoMaxTervekPtr", 4), ("InfoMaxTervek", 4), ("Info_ChartMode", 1), ("Info_StarLogMode", 1),
        ("Info_NagyInfoMode", 1), ("StarlogHanysorfeltoltve", 4), ("StarlogHanyszoveg", 4), ("InfoSelected", 4),
        ("InfoBolygoFlottaSelected", 4), ("BolygoFlottaPointer", 4), ("HanyBolygoFlotta", 4), ("StarLogPointer", 4),
        ("InfoCharts", 0x44c0), ("InfoChartsSmoothed", 0x44c0), ("StarLog", 0x2260)] {
        w.take(n, len)?;
    }
    if v >= 0x62 {
        w.take("BattlesWon", 0x48)?;
    }
    // Research_Save (exe 0x524590).
    w.take("ResearchOldForgato", 4)?;
    w.take("Terulet_Akt", 0x34)?;
    // Messages.
    w.take("Uzenetek", 0x3520)?;
    if (0x5b..99).contains(&v) {
        w.take("UzenetekRandomKitol", 4)?;
    } else if v >= 99 {
        w.take("UzenetekRandomKitol", 400)?;
    }
    w.take("UzenetPlaybackek", 0x20d0)?;
    for (n, len) in [("FolyamatbanU", 4), ("FolyamatbanUH", 4), ("FolyamatbanU3", 4), ("FolyamatbanUP", 4),
        ("FolyamatbanPA", 1), ("FolyamatbanOldBlokk", 4), ("VoltUjValasz", 4), ("VoltUjUzenet", 4),
        ("UAktUzenetFajta", 4), ("UAktUzenet", 4), ("AktBlokk", 4), ("UVoltUjUzenet", 1), ("LastNewMessage", 4),
        ("UUjUzenetLejarasa", 4), ("UzenetekSzama", 4), ("AktUzenetPointer", 4), ("AutomataUzenet", 4),
        ("AutomataUzenetHanyadik", 4), ("Szovegvegen", 4), ("Tiltolista", 0x50), ("MaxTiltasok", 4), ("HanyUzenetFajta", 4)] {
        w.take(n, len)?;
    }
    let db = w.i32("UzenetekDb")?;
    let dbh = w.i32("UzenetekDbHistory")?;
    w.take("ScrollUzenetek", (db as i64 + 1) * 0x40)?;
    w.take("ScrollUzenetekHistory", (dbh as i64 + 1) * 0x40)?;
    // Story event queues: StoryVodor (all 256 bins, one total), HistoryVodor, StartVodor (v > 0x5c),
    // OlvasasVodor (v > 0x67); each `[n]` then n × (`[key]`, `[value]`).
    for (q, on) in [("StoryVodor", true), ("HistoryVodor", true), ("StartVodor", v > 0x5c), ("OlvasasVodor", v > 0x67)] {
        if !on {
            continue;
        }
        let n = w.i32(format!("{q}.n"))?;
        for _ in 0..n {
            w.i32(format!("{q}.key"))?;
            w.i32(format!("{q}.value"))?;
        }
    }
    let n = w.i32("st_vOzonvizMut")?;
    w.take("st_vOzonviz", n as i64 * 0xc)?;
    w.i32("StoryInterregnumIdo")?;
    w.take("StoryAdat", 0x128)?;
    let campaign_type = w.i32("CampaignType")?;
    w.i32("Story_Minden_Flotta_Latszik")?;
    for l in ["sBolygok", "sSzamlalok", "sLogikaiak", "sValtozok"] {
        let n = w.i32(format!("{l}.n"))?;
        w.take(l, (n as i64 + 1) * 8)?;
    }
    w.i32("StoryTerroristaCselValoszinusege")?;
    for _ in 0..8 {
        w.sized("Szovegek")?;
        w.i32("SzovegSzin")?;
    }
    w.i32("UtolsoSzovegIdo")?;
    w.i32("SzovegEltolas")?;
    let n = w.i32("GameInfoMut")?;
    for _ in 0..n {
        w.sized("GameInfo.text")?;
        w.i32("GameInfo.0")?;
        w.i32("GameInfo.8")?;
    }
    let en = w.i32(if mp { "Multiplayer_save_player" } else { "EN" })?;
    let money_b = w.take("Money", 0x98)?;
    let money = std::array::from_fn(|e| f64::from_le_bytes(money_b[8 * e..8 * e + 8].try_into().unwrap()));
    for n in ["MoneyTeruletenkent", "MoneyHatar", "MoneyTeruletCsuszka", "MoneyTeruletSzorzo"] {
        w.take(n, 0x390)?;
    }
    for n in ["ProgramMode", "ProgramDifficulty", "ProgramSebesseg", "ProgramPlayers", "ProgramPlayers2", "ProgramMachines",
        "ProgramGalaxyShape"] {
        w.i32(n)?;
    }
    if v > 0x69 {
        w.i32("MultiplayerBattleStatusz")?;
        w.i32("OnlyServerCanModifyTime")?;
    }
    let player_ki = w.take("PlayerKi", 0x13)?.try_into().unwrap();
    let player_faj = w.take("PlayerFaj", 0x13)?.try_into().unwrap();
    w.take("PlayerNev", 0xd1)?;
    w.take("BirodalomEro", 0x4c)?;
    w.take("BirodalomUrEro", 0x4c)?;
    w.take("BirodalomHanyBolygo", 0x4c)?;
    let empire_alive = w.take("BirodalomEl", 0x13)?.try_into().unwrap();
    for (n, len) in [("BirodalomVisszaSzamlalas", 0x4c), ("BirodalomIsmeri", 0x169), ("BirodalomMiotaIsmeri", 0x5a4),
        ("MiVeszelyAllapot", 0x4c), ("GlobalisMoralCSokkentoBarkacs", 4), ("GlobalisMoralCSokkentoBarkacsSebesseg", 4)] {
        w.take(n, len)?;
    }
    let seeds = w.take("RandSeed", 0x54)?;
    let rand_seed = std::array::from_fn(|k| u32::from_le_bytes(seeds[4 * k..4 * k + 4].try_into().unwrap()));
    for n in ["FullGDP", "FullTax", "TradeIncomeLastYear", "TradeIncomeThisYear", "TaxIncomeLastYear", "TaxIncomeThisYear"] {
        w.take(n, 0x4c)?;
    }
    let n = w.i32("KemHanyBeszolas")?;
    w.take("KemBeszolasok", (n as i64 + 1) * 0x30)?;
    let spy_counts = w.i32s("OsszHanyKem", 9)?;
    let mut spies = vec![Vec::new()];
    for (e, &c) in spy_counts.iter().enumerate().skip(1) {
        spies.push(w.records::<SPY_SIZE>(format!("Kemek[{e}]"), c as i64 + 1)?);
    }
    for (n, len) in [("HanyKemLehet", 0x24), ("HanyKemVan", 0x24), ("HanyKemLehetOsszesen", 0x24),
        ("HanyKemKozpontBolygonkent", 0x1c44), ("CsaladtagElrabolvaKi", 0x24), ("CsaladtagElrabolvaKialtal", 0x24),
        ("KemKuldetesek", 0x360)] {
        w.take(n, len)?;
    }
    let nf = w.i32("OsszHanyFlotta")?;
    let fleets = w.records::<FLEET_SIZE>("Flottak", nf as i64 + 1)?;
    w.take("OsszHanySajatFlotta", 0x4c)?;
    let ns = w.i32("OsszHanyHajo")?;
    let ships = w.records::<SHIP_SIZE>("Hajok", ns as i64 + 1)?;
    let nt = w.i32("OsszHanyTank")?;
    let vehicles = w.records::<VEHICLE_SIZE>("Tankok", nt as i64 + 1)?;
    w.take("OsszHanyKolonizalo", 0x4c)?;
    w.take("OsszHanyTerraformalo", 0x4c)?;
    w.i32("OsszHanyWormhole")?;
    w.take("Wormhole", 0xa8)?;
    let nw = w.i32("OsszHanyWaypoint")?;
    let waypoints = if nw != 0 { w.records::<WAYPOINT_SIZE>("Waypointek", nw as i64 + 1)? } else { Vec::new() };
    let np = w.i32("HanyBolygo")?;
    w.i32("HanyBolygo2")?;
    let planets = w.records::<PLANET_SIZE>("Bolygok", np as i64 + 1)?;
    for (n, on) in [("BolygoUtolsoKozeliFlotta", true), ("BolygoMennyiIdejeLazad", true), ("BolygoMennyiFoldiEro", true),
        ("BolygoMennyiErodEro", true), ("BolygoMennyiIonGunEro", v > 0x58), ("BolygoMennyiTuzoltosag", true),
        ("BolygoNeededMoral", v > 0x5d)] {
        if on {
            w.take(n, 0x324)?;
        }
    }
    for n in ["AktPlanet", "AktFleet", "StarmapMode", "StarmapXStart", "StarmapYStart", "StarmapRotate", "StarmapPitch",
        "StarmapZoom", "StarmapPlanetKering"] {
        w.i32(n)?;
    }
    let mut techs = vec![TechState { status: [0; EMPIRES], time: [0; EMPIRES], given: [0; EMPIRES], fields: [0; 4], seen: [0; 81] }];
    for t in 1..TECHS {
        let status = w.take(format!("Talalmanyok[{t}].status"), 0x13)?.try_into().unwrap();
        let time = w.i32s(format!("Talalmanyok[{t}].time"), EMPIRES)?.try_into().unwrap();
        let given = w.take(format!("Talalmanyok[{t}].given"), 0x13)?.try_into().unwrap();
        let mut fields = [0; 4];
        for (k, f) in fields.iter_mut().enumerate() {
            *f = w.i32(format!("Talalmanyok[{t}]+{:#x}", 0x2c + 4 * k))?;
        }
        let seen = w.take(format!("Talalmanyok[{t}].seen"), 0x51)?.try_into().unwrap();
        techs.push(TechState { status, time, given, fields, seen });
    }
    for (n, len) in [("KutatasSurgos", 0x13), ("ResearchAktFejlesztett", 0x4c), ("ResearchTerulet", 4), ("ResearchMezo", 4),
        ("ResearchTalalmany", 4), ("LaborSzint", 0x130), ("LaborSzintOsszes", 0x130), ("DesignTervSorszam", 4),
        ("AktTervMutato", 4)] {
        w.take(n, len)?;
    }
    let design_counts = w.i32s("OsszHanyTerv", EMPIRES)?;
    let mut designs = vec![Vec::new()];
    for (e, &c) in design_counts.iter().enumerate().skip(1) {
        designs.push(w.records::<DESIGN_SIZE>(format!("Tervek[{e}]"), c as i64 + 1)?);
    }
    let n = w.i32("TradeHanyTradeCuccVan")?;
    w.take("TradeTervek", n as i64 * 0x84)?;
    let trade = w.i32s("TradeHanySzallitas", EMPIRES)?;
    for (e, &c) in trade.iter().enumerate().skip(1) {
        w.take(format!("TradeSzallitas[{e}]"), c as i64 * 0x98)?;
    }
    w.i32("TradeOldNap")?;
    w.i32("AktEpuletTipus")?;
    let building_counts = w.i32s("HanyEpulet", PLANET_SLOTS)?;
    let building_slots = w.i32s("HanyEpuletLefoglalva", PLANET_SLOTS)?;
    let building_blocks = w.i32s("HanyEpuletBlokk", PLANET_SLOTS)?;
    let mut buildings = vec![Vec::new()];
    for (p, &c) in building_blocks.iter().enumerate().skip(1) {
        buildings.push(w.records::<BUILDING_SIZE>(format!("Epuletek[{p}]"), c as i64 + 1)?);
    }
    let mut battlefields = Vec::with_capacity(BATTLEFIELDS);
    for i in 0..BATTLEFIELDS {
        battlefields.push(battlefield(w, v, i)?);
    }
    for n in ["ColonyXStart", "ColonyYStart", "ColonyRotate", "ColonyPitch", "ColonyZoom"] {
        w.i32(n)?;
    }
    w.take("Viszony", 0x3596)?;
    if v > 0x57 {
        w.take("DiplomaciaMikorHivtaUtoljara", 0x144)?;
    }
    for (n, len) in [("MiTervMutato", 0x3b6), ("MiTervMutatoSpec", 0x3b6), ("MiTervHanyVan", 0x76c), ("MiFoBolygo", 0x4c),
        ("MiFoFlotta", 0x4c), ("MiFelderitesRacs", 0x20bb), ("VanUresBolygo[0]", 1), ("VanUresBolygo", 0x13),
        ("MiUtolsoSzarKolonizalas", 0x4c), ("MiMennyiIdeigNemPakolhatFelTankot", 0x4c),
        ("MiMennyiIdeigTartATankPakoloFazis", 0x4c)] {
        w.take(n, len)?;
    }
    if v > 0x66 {
        w.take("MiMennyiIdeigNemLehetTervUpgrade", 0x4c)?;
    }
    if v > 0x60 {
        w.take("MiGyarthatoCuccok", 0x1d64)?;
        w.take("MiBirodalomKozeppontX", 0x4c)?;
        w.take("MiBirodalomKozeppontY", 0x4c)?;
    }
    Ok(SaveGame {
        header: None, multiplayer: mp, body: Vec::new(), used: 0, chunks: Vec::new(),
        version, year, month, day, ido, ido2, old_day, difficulty, en, campaign_type, mp_time, money, rand_seed,
        player_ki, player_faj, empire_alive, spies, fleets, ships, vehicles, waypoints, planets, techs, designs,
        building_counts, building_slots, building_blocks, buildings, battlefields,
    })
}

/// `WF::LoadSave` (exe 0x485420) for battlefield `i`.
fn battlefield(w: &mut Walker, v: i32, i: usize) -> Result<Battlefield> {
    let start = w.mark();
    let p = format!("WF[{i}]");
    if v > 0x52 {
        w.i32(format!("{p}+0x8c8"))?;
        w.i32(format!("{p}+0x8cc"))?;
    }
    w.i32(format!("{p}+0x18"))?;
    if v >= 100 {
        w.i32(format!("{p}.countdown"))?; // +0x148
    }
    let unit_slots = w.i32(format!("{p}.unit_slots"))?;
    let rockets = w.i32(format!("{p}.rockets"))?;
    let paralysers = w.i32(format!("{p}.paralysers"))?;
    let lasers = w.i32(format!("{p}.lasers"))?;
    for (o, len) in [(0x190, 1), (0x1ec, 0x140), (0x32c, 1), (0x184, 1), (0x18c, 4), (0x1a8, 4), (0x1ac, 4), (0x1b0, 4),
        (0x1b4, 4), (0x1b8, 4), (0x334, 4), (0x338, 4), (0x33c, 0x4c0), (0x7fc, 0x90), (0x88c, 1), (0x88d, 0x12),
        (0x89f, 0x12)] {
        w.take(format!("{p}+{o:#x}"), len)?;
    }
    // Pointer arrays travel as u32 presence flags (`loadsave_ptr`, exe 0x4552f0), then the present records.
    let present = i32s(w.take(format!("{p}.rocket_present"), 4 * rockets as i64)?);
    for _ in present.iter().filter(|&&x| x != 0) {
        for k in 0..11 {
            w.i32(format!("{p}.rocket+{:#x}", 8 + 4 * k))?;
        }
        if v >= 0x57 {
            w.take(format!("{p}.rocket+0x34"), 1)?;
        }
    }
    let present = i32s(w.take(format!("{p}.paralyser_present"), 4 * paralysers as i64)?);
    for _ in present.iter().filter(|&&x| x != 0) {
        w.take(format!("{p}.paralyser"), 0x38)?;
    }
    let present = i32s(w.take(format!("{p}.laser_present"), 4 * lasers as i64)?);
    for _ in present.iter().filter(|&&x| x != 0) {
        w.take(format!("{p}.laser"), 0x44)?;
    }
    let alive = w.take(format!("{p}.alive"), unit_slots as i64)?.to_vec();
    w.take(format!("{p}.unit_present"), 4 * unit_slots as i64)?;
    let mut units = Vec::new();
    for (k, &a) in alive.iter().enumerate() {
        if a != 0 {
            units.push(ground_unit(w, v, &format!("{p}.unit[{k}]"))?);
        }
    }
    if v > 0x4e {
        w.i32(format!("{p}+0x8d0"))?;
        w.i32(format!("{p}+0x8d4"))?;
    }
    Ok(Battlefield { unit_slots, rockets, paralysers, lasers, alive, units, chunks: start..w.mark() })
}

/// `WF_Tank::LoadSave` (exe 0x4859a0). `short` = flag 0x4000000: most per-unit state is skipped.
fn ground_unit(w: &mut Walker, v: i32, p: &str) -> Result<GroundUnit> {
    let start = w.mark();
    let f = |o: u32| format!("{p}+{o:#x}");
    let mut flags = if v >= 0x55 { w.i32(f(0x188))? as u32 } else { 0 };
    let full_a = v < 0x55 || flags & 0x400_0000 == 0;
    if v >= 0x4c {
        w.i32(f(0x10c))?;
        if full_a {
            w.take(f(0x104), 1)?;
        }
    }
    for o in [0x1b0, 0x1b4, 0x23c, 0x14, 0xe8, 0x158, 0x15c] {
        w.i32(f(o))?;
    }
    if v < 0x55 {
        flags = w.i32(f(0x188))? as u32;
    }
    let full = flags & 0x400_0000 == 0 || v < 0x54;
    let count = w.i32(f(0x190))?;
    let owner = w.i32(f(0x194))?;
    let side = w.i32(f(0x198))?;
    let opt = |w: &mut Walker, on: bool, offs: &[u32]| -> Result<()> {
        if on {
            for &o in offs {
                w.i32(f(o))?;
            }
        }
        Ok(())
    };
    opt(w, full, &[0x19c])?;
    opt(w, full && v >= 0x57, &[0x108, 0x110, 0x114, 0x118])?;
    opt(w, true, &[0x1a0, 0x1a4])?;
    opt(w, full, &[0x1a8, 0x1ac, 0x1b8, 0x1cc])?;
    let weapons = w.i32(f(0x1d0))? as u32;
    opt(w, full, &[0x1e0, 0x1e4, 0x1e8, 0x1ec])?;
    opt(w, true, &[0xc, 0x10])?;
    opt(w, full, &[0x12c])?;
    opt(w, true, &[0x18])?;
    opt(w, full, &[0x150, 0x98, 0x9c])?;
    opt(w, true, &[0x148])?;
    opt(w, v >= 0x66, &[0x11c])?;
    opt(w, full && v >= 0x5f, &[0x288])?;
    opt(w, full && v >= 0x56, &[0x120])?;
    // Ammunition: one word (v < 0x51), 14 (v < 0x55), else one per weapon bit; same again for +0x1f0.
    let ammo = |w: &mut Walker, base: &str| -> Result<()> {
        let n = if v < 0x51 { 1 } else if v < 0x55 { 14 } else { (weapons & 0x3fff).count_ones() };
        for k in 0..n {
            w.i32(format!("{p}.{base}[{k}]"))?;
        }
        Ok(())
    };
    ammo(w, "ammo")?;
    opt(w, full, &[0xe0])?;
    opt(w, true, &[0xe4])?;
    opt(w, full, &[0x1c0])?;
    opt(w, true, &[0x1c4, 0x228, 0x1c8, 0x1d4, 0x1d8, 0x1dc])?;
    ammo(w, "ammo2")?;
    w.take(f(0x1c), 0x24)?;
    opt(w, true, &[0x244, 0x248])?;
    let hp = i32s(w.take(f(0x154), 4 * count as i64)?);
    let origin = i32s(w.take(f(0x19c), 4 * count as i64)?);
    opt(w, true, &[0x18c])?;
    opt(w, full, &[0x138])?;
    w.take(f(0x146), 1)?;
    w.take(f(0x13c), 10)?;
    opt(w, full, &[0x240])?;
    opt(w, true, &[0x24c])?;
    if full {
        w.take(f(0x250), 1)?;
        w.i32(f(0x254))?;
        w.i32(f(0x258))?;
    }
    opt(w, true, &[0x25c, 0x260, 0x238])?;
    if full {
        w.take(f(0x264), 0x18)?;
    }
    w.take(f(0x130), 1)?;
    w.i32(f(0x134))?;
    Ok(GroundUnit { flags, count, owner, side, weapons, hp, origin, chunks: start..w.mark() })
}

// ------------------------------------------------------------------------------------------------
// Typed views over the raw records (field meanings from the rules specs; the sim decodes the rest).

/// `BolygoTipus` (0x188 B, `re/rules/colony-economy.md` §2.1).
#[derive(Debug, Clone, Copy)]
pub struct PlanetView<'a>(pub &'a [u8; PLANET_SIZE]);

impl PlanetView<'_> {
    /// `+0x18 / +0x1c` starmap position.
    pub fn x(&self) -> f32 {
        rd_f32(self.0, 0x18)
    }
    pub fn y(&self) -> f32 {
        rd_f32(self.0, 0x1c)
    }
    /// `+0x20` owner empire, `+0x21` race, `+0x22` surface, `+0x23` colony map.
    pub fn owner(&self) -> u8 {
        self.0[0x20]
    }
    pub fn race(&self) -> u8 {
        self.0[0x21]
    }
    pub fn surface(&self) -> u8 {
        self.0[0x22]
    }
    pub fn colony_map(&self) -> u8 {
        self.0[0x23]
    }
    /// `+0x28` population, `+0x30` morale.
    pub fn population(&self) -> i32 {
        rd_i32(self.0, 0x28)
    }
    pub fn morale(&self) -> i32 {
        rd_i32(self.0, 0x30)
    }
    /// `+0xd8 + 2e` visibility per empire.
    pub fn visibility(&self, e: usize) -> i16 {
        rd_i16(self.0, 0xd8 + 2 * e)
    }
}

/// `FlottaTipus` (0x80 B, `re/rules/fleet-movement.md` §1.1).
#[derive(Debug, Clone, Copy)]
pub struct FleetView<'a>(pub &'a [u8; FLEET_SIZE]);

impl FleetView<'_> {
    /// `+0x00` NUL-terminated name (21 bytes).
    pub fn name(&self) -> String {
        let n = self.0[..21].iter().position(|&b| b == 0).unwrap_or(21);
        String::from_utf8_lossy(&self.0[..n]).into_owned()
    }
    /// `+0x15` owner (0xFF = deleted slot).
    pub fn owner(&self) -> u8 {
        self.0[0x15]
    }
    pub fn x(&self) -> f32 {
        rd_f32(self.0, 0x30)
    }
    pub fn y(&self) -> f32 {
        rd_f32(self.0, 0x34)
    }
    /// `+0x40` order, `+0x42` kind.
    pub fn order(&self) -> i16 {
        rd_i16(self.0, 0x40)
    }
    pub fn kind(&self) -> i16 {
        rd_i16(self.0, 0x42)
    }
    /// `+0x52` flags, `+0x54` ship count, `+0x60` strength, `+0x68` ground strength.
    pub fn flags(&self) -> u16 {
        rd_i16(self.0, 0x52) as u16
    }
    pub fn ship_count(&self) -> i32 {
        rd_i32(self.0, 0x54)
    }
    pub fn strength(&self) -> i32 {
        rd_i32(self.0, 0x60)
    }
    pub fn ground_strength(&self) -> i32 {
        rd_i32(self.0, 0x68)
    }
}

/// `Hajok` entry (6 B): fleet, design slot, `+4` (i16, not decoded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipView {
    pub fleet: i16,
    pub design: i16,
    pub field_4: i16,
}

impl ShipView {
    pub fn from_bytes(b: &[u8; SHIP_SIZE]) -> Self {
        Self { fleet: rd_i16(b, 0), design: rd_i16(b, 2), field_4: rd_i16(b, 4) }
    }
}

/// `Tankok` entry (8 B): `count` vehicles of one design; fleet ≤ 0 = held by planet −fleet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VehicleView {
    pub fleet: i16,
    pub design: i16,
    pub count: i32,
}

impl VehicleView {
    pub fn from_bytes(b: &[u8; VEHICLE_SIZE]) -> Self {
        Self { fleet: rd_i16(b, 0), design: rd_i16(b, 2), count: rd_i32(b, 4) }
    }
}

/// `TervTipus` (0x84 B, `re/rules/space-combat.md` §2).
#[derive(Debug, Clone, Copy)]
pub struct DesignView<'a>(pub &'a [u8; DESIGN_SIZE]);

impl DesignView<'_> {
    /// `+0x16` class (1 space, 2 ground, < 0 deleted).
    pub fn class(&self) -> i16 {
        rd_i16(self.0, 0x16)
    }
    /// `+0x24` price, `+0x2c` attack, `+0x30` hit points.
    pub fn price(&self) -> i32 {
        rd_i32(self.0, 0x24)
    }
    pub fn attack(&self) -> i32 {
        rd_i32(self.0, 0x2c)
    }
    pub fn hp(&self) -> i32 {
        rd_i32(self.0, 0x30)
    }
    /// `+0x44` hull, `+0x48` hull tech, `+0x4c` slot count.
    pub fn hull(&self) -> i32 {
        rd_i32(self.0, 0x44)
    }
    pub fn hull_tech(&self) -> i32 {
        rd_i32(self.0, 0x48)
    }
    pub fn slots(&self) -> i32 {
        rd_i32(self.0, 0x4c)
    }
}

/// Building instance (0x20 B, `re/rules/colony-economy.md` §2.2). Empty slots have `kind == 0`.
#[derive(Debug, Clone, Copy)]
pub struct BuildingView<'a>(pub &'a [u8; BUILDING_SIZE]);

impl BuildingView<'_> {
    pub fn kind(&self) -> u8 {
        self.0[0]
    }
    pub fn owner(&self) -> u8 {
        self.0[1]
    }
    pub fn damage(&self) -> i32 {
        rd_i32(self.0, 4)
    }
    pub fn work_left(&self) -> u16 {
        rd_i16(self.0, 8) as u16
    }
    pub fn planet(&self) -> u8 {
        self.0[0xa]
    }
    pub fn x(&self) -> f32 {
        rd_f32(self.0, 0xc)
    }
    pub fn y(&self) -> f32 {
        rd_f32(self.0, 0x10)
    }
    pub fn flags(&self) -> u8 {
        self.0[0x19]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal stream: every count zero, version 0x6a, no ground units.
    fn minimal_stream() -> Vec<u8> {
        struct S(Vec<u8>);
        impl S {
            fn put(&mut self, b: &[u8]) {
                if !b.is_empty() {
                    self.0.extend_from_slice(&(b.len() as i32).to_le_bytes());
                    self.0.extend_from_slice(b);
                }
            }
            fn i32(&mut self, v: i32) {
                self.put(&v.to_le_bytes());
            }
        }
        // Reuse the walker's own schema by feeding it a recorder: simplest is to build by hand the fixed parts.
        let mut s = S(Vec::new());
        let z = |n: usize| vec![0u8; n];
        s.i32(VERSION);
        for v in [2330, 3, 4, 1000, 0, 4, 2, 20] {
            s.i32(v);
        }
        s.i32(0);
        s.put(&z(0x1e));
        for n in [0xf50, 0x4c, 0x4c, 0x4c, 0x4c, 0x4c, 0x13, 0x34, 0x30, 4, 4, 4, 4, 4, 0x960, 4, 1, 1, 1] {
            s.put(&z(n));
        }
        for n in [4, 4, 4, 4, 4, 4, 1, 1, 1, 4, 4, 4, 4, 4, 4, 4, 0x44c0, 0x44c0, 0x2260, 0x48, 4, 0x34, 0x3520, 400, 0x20d0] {
            s.put(&z(n));
        }
        for n in [4, 4, 4, 4, 1, 4, 4, 4, 4, 4, 4, 1, 4, 4, 4, 4, 4, 4, 4, 0x50, 4, 4] {
            s.put(&z(n));
        }
        s.i32(0);
        s.i32(0);
        s.put(&z(0x40));
        s.put(&z(0x40));
        for _ in 0..4 {
            s.i32(0);
        }
        s.i32(0); // st_vOzonvizMut
        s.i32(0);
        s.put(&z(0x128));
        s.i32(0);
        s.i32(0);
        for _ in 0..4 {
            s.i32(0);
            s.put(&z(8));
        }
        s.i32(0);
        for _ in 0..8 {
            s.i32(0);
            s.i32(0);
        }
        s.i32(0);
        s.i32(0);
        s.i32(0); // GameInfoMut
        s.i32(6); // EN
        s.put(&z(0x98));
        for _ in 0..4 {
            s.put(&z(0x390));
        }
        for _ in 0..9 {
            s.i32(0);
        }
        for n in [0x13, 0x13, 0xd1, 0x4c, 0x4c, 0x4c, 0x13, 0x4c, 0x169, 0x5a4, 0x4c, 4, 4, 0x54, 0x4c, 0x4c, 0x4c, 0x4c, 0x4c, 0x4c] {
            s.put(&z(n));
        }
        s.i32(0);
        s.put(&z(0x30));
        s.put(&z(0x24));
        for _ in 1..9 {
            s.put(&z(SPY_SIZE));
        }
        for n in [0x24, 0x24, 0x24, 0x1c44, 0x24, 0x24, 0x360] {
            s.put(&z(n));
        }
        s.i32(0);
        s.put(&z(FLEET_SIZE));
        s.put(&z(0x4c));
        s.i32(0);
        s.put(&z(SHIP_SIZE));
        s.i32(0);
        s.put(&z(VEHICLE_SIZE));
        s.put(&z(0x4c));
        s.put(&z(0x4c));
        s.i32(0);
        s.put(&z(0xa8));
        s.i32(0); // no waypoints
        s.i32(1);
        s.i32(1);
        s.put(&z(2 * PLANET_SIZE));
        for _ in 0..7 {
            s.put(&z(0x324));
        }
        for _ in 0..9 {
            s.i32(0);
        }
        for _ in 1..TECHS {
            for n in [0x13, 0x4c, 0x13, 4, 4, 4, 4, 0x51] {
                s.put(&z(n));
            }
        }
        for n in [0x13, 0x4c, 4, 4, 4, 0x130, 0x130, 4, 4, 0x4c] {
            s.put(&z(n));
        }
        for _ in 1..EMPIRES {
            s.put(&z(DESIGN_SIZE));
        }
        s.i32(0);
        s.put(&z(0x4c));
        s.i32(0);
        s.i32(0);
        for _ in 0..3 {
            s.put(&z(0x324));
        }
        for _ in 1..PLANET_SLOTS {
            s.put(&z(BUILDING_SIZE));
        }
        for _ in 0..BATTLEFIELDS {
            for _ in 0..3 {
                s.i32(0);
            }
            s.i32(-1);
            for _ in 0..4 {
                s.i32(0);
            }
            for n in [1, 0x140, 1, 1, 4, 4, 4, 4, 4, 4, 4, 4, 0x4c0, 0x90, 1, 0x12, 0x12, 4, 4] {
                s.put(&z(n));
            }
        }
        for _ in 0..5 {
            s.i32(0);
        }
        for n in [0x3596, 0x144, 0x3b6, 0x3b6, 0x76c, 0x4c, 0x4c, 0x20bb, 1, 0x13, 0x4c, 0x4c, 0x4c, 0x4c, 0x1d64, 0x4c, 0x4c] {
            s.put(&z(n));
        }
        s.0
    }

    #[test]
    fn minimal_stream_round_trips() {
        let mut body = minimal_stream();
        let used = body.len();
        body.resize(used + 100, 0);
        let save = SaveGame::parse_stream(body.clone(), false).unwrap();
        assert_eq!(save.used, used);
        assert_eq!((save.year, save.month, save.day, save.en), (2330, 3, 4, 6));
        assert_eq!(save.planets.len(), 2);
        assert_eq!(save.buildings.len(), PLANET_SLOTS);
        assert_eq!(save.battlefields.len(), BATTLEFIELDS);
        assert_eq!(save.chunk("EN"), Some(&6i32.to_le_bytes()[..]));

        let mut file = Vec::new();
        file.extend_from_slice(MAGIC);
        file.extend_from_slice(&[0; 31]);
        for v in [VERSION, 2330, 3, 4, 1000, 0, 6, 0] {
            file.extend_from_slice(&v.to_le_bytes());
        }
        let z = miniz_oxide::deflate::compress_to_vec_zlib(&body, 6);
        file.extend_from_slice(&(z.len() as u32).to_le_bytes());
        file.extend_from_slice(&(body.len() as u32).to_le_bytes());
        file.extend_from_slice(&z);
        let parsed = SaveGame::parse(&file).unwrap();
        assert_eq!(parsed.header.as_ref().unwrap().en, Some(6));
        let again = SaveGame::parse(&parsed.to_file().unwrap()).unwrap();
        assert_eq!(again.body, parsed.body);
    }

    #[test]
    fn wrong_chunk_length_is_reported() {
        let mut body = minimal_stream();
        body[4 + 4] = 5; // "Ev" length prefix (second chunk) 4 -> 5
        let e = SaveGame::parse_stream(body, false).unwrap_err().to_string();
        assert!(e.contains("Ev"), "{e}");
    }
}
