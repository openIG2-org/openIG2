//! Rules tables read from the user's own `ig2.exe` at runtime (no rule values ship with the engine).
//!
//! Catalogue and field evidence: `re/rules/tables.md`. Runtime verification (Frida): the static
//! tables equal the exe file bytes at the main menu and after a scenario load, except for
//! localised name fields; the live tech table's balance fields equal `Talalmanyok0` after load
//! (`re/rules/traces/2026-10-15-tables.md`).

mod pe;
pub mod sounds;

use serde::Serialize;
use sha2::{Digest, Sha256};

pub use pe::PeImage;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported ig2.exe build (sha256 {0}); only the GOG 2017-02-06 build is catalogued")]
    UnknownBuild(String),
    #[error("malformed PE: {0}")]
    Pe(&'static str),
    #[error("address {0:#x} is outside the image's initialised sections")]
    Unmapped(u32),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Fails unless `exe` is the catalogued build (every table address below is that build's).
pub(crate) fn check_build(exe: &[u8]) -> Result<()> {
    let digest: String = Sha256::digest(exe).iter().map(|b| format!("{b:02x}")).collect();
    if digest != GOG_2017_SHA256 {
        return Err(Error::UnknownBuild(digest));
    }
    Ok(())
}

/// SHA-256 of the catalogued GOG `ig2.exe` (PE32, MSVC 2013, linked 2017-02-06).
pub const GOG_2017_SHA256: &str = "2baa530be6bc86000b9d63e7c47a6131f0d0f4a6cc8d3f1897f5db0427950442";

/// Table locations in the catalogued build (image VA, element stride, element count).
pub mod layout {
    pub const TECHS: (u32, usize, usize) = (0x8DE048, 276, 193); // Talalmanyok0
    pub const TECH_FIREPOWER: (u32, usize, usize) = (0x8EB730, 4, 193); // TalalmanyokrEro
    pub const SKIRMISH_TECH_START: (u32, usize, usize) = (0x8EC218, 4, 193); // ScirmishTalalmanyFejlesztes
    pub const BUILDINGS: (u32, usize, usize) = (0x8EBA38, 36, 56); // EpuletekLeiro
    pub const POD_TYPES: (u32, usize, usize) = (0x8F8918, 84, 55); // PodTipusok
    pub const POD_SLOTS: (u32, usize, usize) = (0x8F9B28, 8, 14 * 27 * 12); // PodFajok
    pub const RACE_POD_TABLE: (u32, usize, usize) = (0x902F10, 4, 19); // HanyasFajPodTablazatatHasznalja
    pub const HULLS: (u32, usize, usize) = (0x90BBF8, 36, 25); // Alapok
    pub const DESIGNS: (u32, usize, usize) = (0x90BF80, 132, 201); // Tervek0
    pub const SPACE_WEAPONS: (u32, usize, usize) = (0x91D858, 92, 40); // fegyverek
    pub const SHOT_FORMS: (u32, usize, usize) = (0x9256F8, 0x78, 40); // fegyveralakok
    pub const RACE_SURFACE_AFFINITY: (u32, usize, usize) = (0x90B9D8, 4, 135); // FajBolygoViszony (15 × 9)
    pub const RESEARCH_SIZE_MULT: (u32, usize, usize) = (0x9185D0, 4, 192); // Research_MeretSzorzok
    /// The research screen (`re/screens/research.md`): `TTeruletek` (the techs' model names, `char*`), the models'
    /// orbit offsets `TPozX/Y/Z[1..=16]`, the middle wheel's `forgastablai`/`forgastabla2i`/`forgastabla3i` (mask
    /// colour, category, button id per slot) and its line ends `forgastabla4x/4y`, the lab numbers' places
    /// `Tudomany_X`/`Tudomany_X2`/`Tudomany_Y[1..=3]`.
    pub const RESEARCH_MODELS: (u32, usize, usize) = (0x919298, 4, 192);
    /// The design screen's hull models (`LoadAlapVektor`, re/screens/design.md §5.1): `AlapNevek[m]` (file in
    /// `vector/design/alapok/`) and `AlapAranyok[m]` (the `AddToScene` scale before its ×0.5), m = `Alapok[hull]+8`.
    pub const DESIGN_HULL_MODELS: (u32, usize, usize) = (0x9028E8, 4, 25);
    pub const DESIGN_HULL_SCALE: (u32, usize, usize) = (0x8F88B0, 4, 25);
    /// `PodFajok[table][hull][slot]` (8 tables × 27 hulls × 12 slots of (class, i32)) and
    /// `HanyasFajPodTablazatatHasznalja[race]` (the race's table, 1-based).
    pub const DESIGN_POD_SLOTS: (u32, usize, usize) = (0x8F9B28, 4, 8 * 0xA20 / 4);
    pub const DESIGN_POD_TABLE_OF_RACE: (u32, usize, usize) = (0x902F10, 4, 19);
    /// `DesignBrushok[1..=4]` (`AktBrushok` on the screen): the pod rows' minus, plus and their greyed twins.
    pub const DESIGN_BRUSHES: (u32, usize, usize) = (0x902988, 0x38, 4);
    pub const RESEARCH_ORBIT_X: (u32, usize, usize) = (0x9190F8, 4, 17);
    pub const RESEARCH_ORBIT_Y: (u32, usize, usize) = (0x919140, 4, 17);
    pub const RESEARCH_ORBIT_Z: (u32, usize, usize) = (0x919188, 4, 17);
    pub const RESEARCH_WHEEL: (u32, usize, usize) = (0x918BD8, 4, 15);
    pub const RESEARCH_WHEEL_LINES: (u32, usize, usize) = (0x918C50, 4, 10);
    pub const RESEARCH_LABS_XY: (u32, usize, usize) = (0x919268, 4, 12);
    pub const RACE_PRODUCTION_MULT: (u32, usize, usize) = (0x8F8870, 4, 15);
    pub const RACE_RESEARCH_TIME_MULT: (u32, usize, usize) = (0x8F87F8, 4, 15);
    pub const RACE_RESEARCH_MONEY_MULT: (u32, usize, usize) = (0x8F87BC, 4, 15); // FajSpeckoKutatasMoneySzorzo
    pub const DIFFICULTY_PRODUCTION_MULT: (u32, usize, usize) = (0x8F8834, 4, 5);
    pub const DIFFICULTY_MONEY_MULT: (u32, usize, usize) = (0x8F8848, 4, 5);
    pub const DIFFICULTY_DEVELOPMENT_MULT: (u32, usize, usize) = (0x8F885C, 4, 5);
    pub const RACE_PEACE_THRESHOLD: (u32, usize, usize) = (0x90B924, 4, 15);
    pub const RACE_TRADE_DEMAND: (u32, usize, usize) = (0x90B960, 4, 15);
    pub const RACE_FEAR_THRESHOLD: (u32, usize, usize) = (0x90B99C, 4, 15);
    pub const RACE_AI_PRODUCTION_SHARE: (u32, usize, usize) = (0x915F90, 4, 15);
    pub const RACE_AI_MILITARY_AUTOBUILD: (u32, usize, usize) = (0x915FCC, 4, 15);
    pub const PROBLEM_CURVES: (u32, usize, usize) = (0x912858, 24, 20); // ProblemakLeiro
    pub const KIDNAP_MORALE: (u32, usize, usize) = (0x913838, 4, 6); // CsaladtagMoralCsokkenesLeiro
    pub const WORMHOLE_COLOURS: (u32, usize, usize) = (0x913850, 12, 6); // i32 [6][3], AddWormhole's tints
    pub const RADAR_RANGES: (u32, usize, usize) = (0x913898, 4, 6); // RadarHatotav
    pub const PLANET_RADAR_RANGES: (u32, usize, usize) = (0x9138B0, 4, 8); // RadarHatotav2Init
    pub const SATELLITE_PRICES: (u32, usize, usize) = (0x9138D0, 4, 3); // MuholdAr
    pub const TANK_SPEEDS: (u32, usize, usize) = (0x8EC9D4, 4, 5); // TankAlapSpeed (ground hull techs 145..=149)
    pub const AUTOBUILD_PROBLEM: (u32, usize, usize) = (0x8F1D50, 4, 29); // AutoBuildHanyasProblema
    pub const AUTOBUILD_MIN: (u32, usize, usize) = (0x8F1DD0, 4, 29); // AutoBuildMitEpitMin
    pub const AUTOBUILD_MAX: (u32, usize, usize) = (0x8F1E50, 4, 29); // AutoBuildMitEpitMax
    pub const AUTOBUILD_NAMES: (u32, usize, usize) = (0x8F1ED0, 4, 17); // AutoBuildNevek (char*)
    pub const PRODUCTION_DEFAULT: (u32, usize, usize) = (0x90B8A8, 1, 64); // pGyartas0
    pub const BUILDING_MODELS: (u32, usize, usize) = (0x8F1F28, 0x6c, 60); // Building (kind 1..=60), footprint +0x58/+0x5c
    /// The colony screen's unit models (`WF_Tank::LoadMesh`, `re/screens/colony.md` §4): `TankAlap` (hull models, name +
    /// vector directory at +0x34), `TankMeret` (hull scale), `TankPivotAdder`, `TankTorony` (weapon models),
    /// `TankToronySize` (`[hull][9]`), `TankToronyMeret` (defence turrets 0xa1..=0xa8).
    pub const TANK_MODELS: (u32, usize, usize) = (0x8F50D8, 0x38, 5);
    pub const TANK_SCALE: (u32, usize, usize) = (0x8F1BC4, 4, 5);
    pub const TANK_PIVOT: (u32, usize, usize) = (0x8EC524, 4, 5);
    pub const TURRET_MODELS: (u32, usize, usize) = (0x8EC538, 0x38, 19);
    pub const TURRET_SCALE: (u32, usize, usize) = (0x8F1BD8, 4, 45);
    pub const DEFENCE_TURRET_SCALE: (u32, usize, usize) = (0x8F1BA4, 4, 8);
    /// `TransfAlap`: the starmap's model of each hull (design `+0x44`), same records as `TankAlap`.
    pub const TRANSFER_MODELS: (u32, usize, usize) = (0x927410, 0x38, 25);
    /// The bitmap font of `KiirStr` (`re/screens/messages.md` §1): glyph size, step, first row and glyphs per
    /// row for sizes 0..=4 (`CharXMeret`, `CharYMeret`, `CharXTolas`, `CharYTolas`, `CharXKezdet`, `CharYDarab`),
    /// and the glyph order of the atlas (162 bytes; Hungarian and the other languages).
    pub const CHAR_TABLES: (u32, usize, usize) = (0x912A50, 4, 30);
    pub const GLYPHS_HU: (u32, usize, usize) = (0x8527F0, 1, 162);
    pub const GLYPHS: (u32, usize, usize) = (0x852748, 1, 162);
    /// `FajnevX`: the info screen's race-name columns (`KirakFajNeveket`).
    pub const RACE_NAME_X: (u32, usize, usize) = (0x90B6BC, 4, 8);
    /// The HUD (`re/screens/hud.md` §1): `Kepek` (the 2D pictures, file name at +0) and the colony and starmap
    /// screens' brush records (`ColonyBrushok`, `StarmapBrushok`; 14 dwords each).
    pub const PICTURES: (u32, usize, usize) = (0x903558, 0x124, 107);
    pub const COLONY_BRUSHES: (u32, usize, usize) = (0x8ECA08, 0x38, 33);
    pub const STARMAP_BRUSHES: (u32, usize, usize) = (0x927130, 0x38, 13);
    /// `utdarabsize`: the scale of the road pieces `ut1..ut9.xvb` (`AddUtDarab`).
    pub const ROAD_SCALE: (u32, usize, usize) = (0x8EC968, 4, 9);
    pub const SCAFFOLD_HEIGHTS: (u32, usize, usize) = (0x8F3878, 4, 60); // allvanypoziciok (ELF 0x115f740)
    pub const DECLARE_WAR_UNRELIABILITY: (u32, usize, usize) = (0x902FC0, 4, 16); // Megbizhatatlansag_DeclareWar [2][4][2]
    pub const SPY_MISSIONS: (u32, usize, usize) = (0x912F48, 0x20, 27); // KemKuldetesek
    pub const SPY_OUTCOME_STATE: (u32, usize, usize) = (0x9132A8, 4, 5); // KemKuldetesAtmenet
    pub const SPY_RANK_POINTS: (u32, usize, usize) = (0x9132BC, 4, 15); // KemSzintLepesSzetoszthato
    pub const SPY_TEMPLATES: (u32, usize, usize) = (0x9132F8, 0x4c, 15); // KemTartalekok
    /// `KemRangXPHatar[0..=20]` plus the i32 after it, which `Spy_New` reads for rank 20.
    pub const SPY_RANK_XP: (u32, usize, usize) = (0x9137A8, 4, 22);
    pub const SPY_TRAINING_XP: (u32, usize, usize) = (0x92705C, 4, 6); // TrainingKuldetesXP
    pub const SPY_TRAINING_COST: (u32, usize, usize) = (0x927044, 4, 6); // TrainingKuldetesAr
    pub const SPY_TRAINING_DAYS: (u32, usize, usize) = (0x926EE4, 4, 6); // TrainingKuldetesIdo
    pub const SPY_RACE_COST_MULT: (u32, usize, usize) = (0x91376C, 4, 15); // KemKuldetesPenzSzorzoFajonkent
    pub const SPY_HIRE_COST_MULT: (u32, usize, usize) = (0x9137FC, 4, 15); // KemFelvetelPenzSzorzoFajonkent
    /// The AI's unit categories 1..=10 (`re/rules/ai-empire.md` §1.2): how a category is built, and the
    /// range of `MiTervMutato` slots it takes its design from (11 bytes each, padded to 12 in the exe).
    pub const AI_BUILD_MODE: (u32, usize, usize) = (0x915F6C, 1, 11); // MiGyartasMod
    pub const AI_BUILD_MIN: (u32, usize, usize) = (0x915F78, 1, 11); // MiGyartasMitMin
    pub const AI_BUILD_MAX: (u32, usize, usize) = (0x915F84, 1, 11); // MiGyartasMitMax
    /// `Talalmanyok2[tech][race]` (193 × 9 u8): the AI's tech priority for the special variant of a ground hull
    /// (`TervAutoDesign`, `ai-empire.md` §10.3).
    pub const AI_GROUND_PRIORITY: (u32, usize, usize) = (0x8EB060, 1, 193 * 9); // Talalmanyok2
    /// `Spy_New` (code): the recruit names are immediates stored into a local `char*` array.
    pub const SPY_NEW_CODE: (u32, usize, usize) = (0x58F830, 1, 0x4c0);
    pub const SHIP_TYPES: (u32, usize, usize) = (0x91F488, 0x30, 240); // tipusnevek [20 races][12 types]
    /// Ground combat (`re/rules/ground-combat.md` §1.3): `WF_Weapons` {min, max, period, unk} × 14.
    pub const GROUND_WEAPONS: (u32, usize, usize) = (0x8F82C0, 4, 56);
    /// `TankAlapHP` (hulls 0x13..=0x17) as `AddTanksToPlanet` reads it: `0x8F1EC8 + 4·hull` for hulls 0..=24.
    pub const GROUND_HULL_HP: (u32, usize, usize) = (0x8F1EC8, 4, 25);
    /// Colony battlefield size (f32 width, height) for `WF_Tank::SetPosG` (exe 0x49c250).
    pub const GROUND_MAP_SIZE: (u32, usize, usize) = (0x8EC960, 4, 2);
    /// `WF_NonSimulateWeapons` and `WF_FortressWeapons`: 14 × {min, max, reload, range} (real-time ground war).
    pub const GROUND_RT_WEAPONS: (u32, usize, usize) = (0x8F83A0, 4, 56);
    pub const GROUND_FORTRESS_WEAPONS: (u32, usize, usize) = (0x8F8480, 4, 56);
    /// Turning speed by ground hull index 0..5 (`AddTank`: `+0xe0 = t`, `+0xe4 = 1.5 t`).
    pub const GROUND_TURN_SPEEDS: (u32, usize, usize) = (0x8F8560, 4, 5);
    /// Slope limits by passability layer 0..5 (`PlanetAtjarasMap::GenerateMap`: a cell's slope class is
    /// gentle when `|class| <= limit`; layer 0 is the editor's, 1..=4 the vehicles').
    pub const GROUND_SLOPE_LIMITS: (u32, usize, usize) = (0x8F8574, 4, 5);
    /// The diplomacy screen (`re/screens/diplomacy.md` §2): `GombPoziciok` (6 × f32 x, y, z, 0), `DipFejX`,
    /// `MenukKezdX/Y`, `InfonevX/Y`, `InfonevFont`/`InfoSzin1`/`InfoSzin2`, `PnevekFont`/`PnevekSzin2`,
    /// `MenuMaxSorokO`/`MenuOszlopKoz`, `TexturaNevek` (char*), `KemFejNevek` (char*, by race).
    pub const DIPLOMACY_HEADS: (u32, usize, usize) = (0x903418, 4, 24);
    pub const DIPLOMACY_NAME_X: (u32, usize, usize) = (0x90335C, 4, 6);
    pub const DIPLOMACY_MENU_XY: (u32, usize, usize) = (0x902FB8, 4, 2);
    pub const DIPLOMACY_INFO_XY: (u32, usize, usize) = (0x903000, 4, 2);
    pub const DIPLOMACY_INFO_STYLE: (u32, usize, usize) = (0x903374, 4, 3);
    pub const DIPLOMACY_NAME_STYLE: (u32, usize, usize) = (0x902F98, 4, 2);
    pub const DIPLOMACY_MENU_GRID: (u32, usize, usize) = (0x902F80, 4, 2);
    pub const DIPLOMACY_PORTRAITS: (u32, usize, usize) = (0x9033D8, 4, 16);
    pub const DIPLOMACY_HEAD_ANIMS: (u32, usize, usize) = (0x926FFC, 4, 15);
    /// Armour factors by ground hull and hit side (front, side, rear): `SetRealDamage` divides by them.
    pub const GROUND_ARMOUR: (u32, usize, usize) = (0x8F858C, 4, 15);
    /// The campaign tables `StoryEleje`, `StoryElejeSeq`, `StoryLine`, `StoryScenario`, `RndStoryk`,
    /// `RndStoryValasztas`, `StoryHosok`, `RndHosok`, `StoryKuldetes` (contiguous; `re/rules/campaign.md` §8.1).
    pub const CAMPAIGN: (u32, usize, usize) = (0x928340, 4, 423);
    /// The end videos by the human's empire `EN` (`re/screens/video.md` §5, char*): `MEGHALAS`'s loss clip and
    /// `MEGNYERES`'s campaign win clip.
    pub const VIDEO_LOSS: (u32, usize, usize) = (0x919724, 4, 9);
    pub const VIDEO_WIN: (u32, usize, usize) = (0x919748, 4, 9);
    /// `StartAnim`/`VegeAzElsoAnimnak`'s head clips {loop, wait} by sender index (char* pairs, 15 records).
    pub const VIDEO_MESSAGE_CLIPS: (u32, usize, usize) = (0x915e60, 4, 30);
}

fn cstr(b: &[u8]) -> String {
    b.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect()
}

fn i32_at(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

/// The recruit names of `Spy_New`: the code stores each `char*` with `mov dword [ebp + d], imm32`
/// (`C7 45 d8` / `C7 85 d32`) into a local array; sorted by `d` they are the list in order (37 names,
/// `re/rules/espionage.md` §1.2).
fn spy_names(img: &PeImage) -> Result<Vec<Vec<u8>>> {
    let (va, _, len) = layout::SPY_NEW_CODE;
    let code = img.read(va, len)?;
    let mut found = Vec::new();
    let mut i = 0;
    while i + 7 <= code.len() {
        let (d, imm, step) = match (code[i], code[i + 1]) {
            (0xc7, 0x45) => (code[i + 2] as i8 as i32, u32::from_le_bytes(code[i + 3..i + 7].try_into().unwrap()), 7),
            (0xc7, 0x85) if i + 10 <= code.len() => {
                (i32_at(code, i + 2), u32::from_le_bytes(code[i + 6..i + 10].try_into().unwrap()), 10)
            }
            _ => {
                i += 1;
                continue;
            }
        };
        if let Ok(s) = img.read(imm, 16) {
            let name: Vec<u8> = s.iter().take_while(|&&c| c != 0).copied().collect();
            if !name.is_empty() && name.len() < 16 && name.iter().all(|&c| c >= 0x20) {
                found.push((d, name));
            }
        }
        i += step;
    }
    found.sort();
    if found.len() != 37 || found.windows(2).any(|w| w[1].0 - w[0].0 != 4) {
        return Err(Error::Pe("unexpected Spy_New name list"));
    }
    Ok(found.into_iter().map(|(_, n)| n).collect())
}

/// One technology (`Talalmanyok0` record, 276 bytes; per-empire runtime state at 69.. is omitted).
#[derive(Debug, Clone, Serialize)]
pub struct Tech {
    pub index: usize,
    /// Built-in English name (the game overwrites bytes 0..15 with the localised name at start-up).
    pub name: String,
    /// +20 production price (`GetGyartasAr`).
    pub price: i32,
    /// +24 production time (`GetGyartasIdo`).
    pub build_time: i32,
    /// +38 (u8) / +40 (i32): non-zero triggers automatic design on discovery (`TalalmanyKifejlesztve`).
    pub auto_design_flag: u8,
    pub auto_design_kind: i32,
    /// +45..+47: prerequisite tech indices (0 = none).
    pub prerequisites: [u8; 3],
    /// +49..+51: required lab levels [inf: civil, mechanical, computer].
    pub lab_levels: [u8; 3],
    /// +52 research cost (money).
    pub research_cost: i32,
    /// +56 research time; scaled per race by `RACE_RESEARCH_TIME_MULT` on completion bookkeeping.
    pub research_time: i32,
    /// +32: hit points (design and defence-building strength, `TervUjraSzamol`, `SimulateSpBattle`).
    pub hp: i32,
    /// +37 (u8): vehicle capacity added to a design (`TervUjraSzamol`, design `+0x28`).
    pub capacity: u8,
    /// Unidentified static words: +28, +36 (byte 37 is `capacity`) and +60..+68 [unk].
    pub unknown: [i32; 5],
    /// `TalalmanyokrEro[i]`: firepower / strength value [inf from res_info "FIREPOWER"].
    pub firepower: i32,
}

/// Building descriptor (`EpuletekLeiro`, 36 bytes). Fields pinned in `re/rules/colony-economy.md` §2.3.
#[derive(Debug, Clone, Serialize)]
pub struct Building {
    pub index: usize,
    /// +0: tech whose price/time is the building's cost (`EpuletJavitasAr`, `FRISSIT_Planet`).
    pub tech: u8,
    pub b1: u8,
    /// +2: priority class 0..=6; energy and workers are cut from class 6 downwards.
    pub priority: u8,
    pub b3: u8,
    /// +4: construction work in days (×50 when the building is placed, `EpuletNew`).
    pub build_work: i32,
    /// +8: maximum hit points (repair cost = price × damage / max_hp).
    pub max_hp: i32,
    /// +12: energy demand.
    pub energy: i32,
    /// +16: worker demand.
    pub workers: i32,
    /// +20: output; the effect is `output × yield% × efficiency / 256`.
    pub output: i32,
    pub w24: i32,
    pub w28: i32,
    /// +32 bit 0: the effect is the scaled output (otherwise 1 per building);
    /// +33 bit 0: the effect persists while the building isn't working.
    pub flags: [u8; 2],
}

/// How the colony screen shows a building kind (`Building[k − 1]`, 0x6c bytes; `re/screens/colony.md` §3).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BuildingModel {
    /// `+0x00`: the model (`LoadMaxLevelBox` in `vector/colony/`).
    pub model: String,
    /// `+0x28`: the build menu's preview model.
    pub preview: String,
    /// `+0x50`: the model's scale (`AddToSceneGenesis(…, scale / 1024 / 1000)`).
    pub scale: f32,
    /// `+0x54`: the preview's scale.
    pub preview_scale: f32,
    /// `+0x68` (ELF `+0x6c`): the selection marker's size per footprint cell (`DrawBuildingSelection`).
    pub select: f32,
}

/// A model of the colony screen's units: the name (`LoadBox`) and the vector directory it loads from
/// (`+0x34`: 0 `starmap/transfer`, 1 `spacewar`, 2 `talalmanyok`, 3 `colony`).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UnitModel {
    pub name: String,
    pub dir: i32,
}

/// The ground units' models (`WF_Tank::LoadMesh` ELF 0x46efc0, `re/screens/colony.md` §4).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct UnitModels {
    /// `TankAlap[hull]`, scaled `TankMeret[hull] / 1024 / 10 / 5` (`AddToSceneGenesis`).
    pub hulls: Vec<UnitModel>,
    pub hull_scale: Vec<f32>,
    /// `TankPivotAdder[hull]`: how far along the body's third axis the model sits.
    pub pivot: Vec<f32>,
    /// `TankTorony[k]`: weapon pods 0xa1.. (k = pod − 0xa1), the defence turrets (k = pod − 0x98), the
    /// air-strike pods 0x11/0x12.
    pub turrets: Vec<UnitModel>,
    /// `TankToronySize[hull·9 + k]` (scale `/ 1024 / 10`).
    pub turret_scale: Vec<f32>,
    /// `TankToronyMeret[pod − 0xa1]` for the defence turrets.
    pub defence_scale: Vec<f32>,
    /// `TransfAlap[hull]`: the starmap's models of the hulls (the production view's head units, the transfer
    /// panel; `+0x34`: 0 and 2 `talalmanyok`, 1 `spacewar`, 3 `colony` there).
    pub transfer: Vec<UnitModel>,
}

/// The design screen's tables (`re/screens/design.md` §5.1).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct DesignScreen {
    /// `AlapNevek[m]`: hull model `m`'s file (`""` = none; models 5..=11 take the race digit at [4]).
    pub models: Vec<String>,
    /// `AlapAranyok[m]`.
    pub scale: Vec<f32>,
    /// `PodFajok` as dwords (`[table − 1]·0x288 + hull·0x18 + slot·2`: the class, then a second word).
    pub pod_slots: Vec<i32>,
    /// `HanyasFajPodTablazatatHasznalja[race]`.
    pub pod_table: Vec<i32>,
    /// `DesignBrushok[1..=4]`.
    pub brushes: Vec<Brush>,
}

impl DesignScreen {
    /// The pod classes of `hull`'s twelve slots for `race` (`PodFajok[HanyasFaj…[race] − 1][hull][k]`, 0 = none).
    pub fn pod_slots(&self, race: usize, hull: usize) -> Vec<i32> {
        let t = self.pod_table.get(race).copied().unwrap_or(0);
        if t < 1 || hull >= 27 {
            return vec![0; 12];
        }
        let at = (t as usize - 1) * 0x288 + hull * 0x18;
        (0..12).map(|k| self.pod_slots.get(at + 2 * k).copied().unwrap_or(0)).collect()
    }

    /// `GetMaxRarakhato(race, hull, k)` (ELF 0x4fb4c0): how many of a tech slot `k` (1-based) takes, the slot's second
    /// word.
    pub fn pod_max(&self, race: usize, hull: usize, k: usize) -> i32 {
        let t = self.pod_table.get(race).copied().unwrap_or(0);
        if t < 1 || hull >= 27 || !(1..=12).contains(&k) {
            return 0;
        }
        self.pod_slots.get((t as usize - 1) * 0x288 + hull * 0x18 + 2 * (k - 1) + 1).copied().unwrap_or(0)
    }

    /// The text naming pod class `c` (`NEWGAME_Design` ELF 0x508210: `GetString(0x13, 1, c)` below 0x33, else
    /// `GetString(0x13, 2, c − 0x32)`; cut to 14 characters).
    pub fn pod_type_text(&self, c: i32) -> Option<(u16, u16)> {
        match c {
            1..=0x32 => Some((1, c as u16)),
            0x33.. => Some((2, (c - 0x32) as u16)),
            _ => None,
        }
    }
}

/// The research screen's tables (`re/screens/research.md` §2–§4).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ResearchScreen {
    /// `TTeruletek[t − 1]`: tech `t`'s model on the screen (`""` = none; categories 6..=9 use the buildings',
    /// 11's turrets fall back to `TankTorony`).
    pub models: Vec<String>,
    /// `TPozX/Y/Z[k]` (k = 1..=16): where the category's k-th model orbits the ball (×2.2).
    pub orbit: Vec<[f32; 3]>,
    /// The middle wheel's five slots: hot-spot mask colour, category (`ResearchTerulet`) and button id
    /// (`resSetGombFrame`), as `Research_Belep` starts them (`forgastablai`, `…2i`, `…3i`).
    pub wheel_kep: [i32; 5],
    pub wheel_area: [i32; 5],
    pub wheel_button: [i32; 5],
    /// `forgastabla4x/4y`: where `ResDrawVonalak`'s lines start for each slot.
    pub wheel_line: [[i32; 2]; 5],
    /// `Tudomany_X`, `Tudomany_X2`, `Tudomany_Y` for labs 1..=3 (index 0 unused): the lab level and the tech's need.
    pub lab_x: [i32; 4],
    pub lab_x2: [i32; 4],
    pub lab_y: [i32; 4],
}

/// `KiirStr`'s bitmap font (`screen/charset_bmp`, a 256-unit atlas): per size 0..=4 the glyph size, its step in the
/// atlas, the first row's y and the glyphs per row; the glyph order maps a byte to its atlas index (bytes not in it
/// show glyph 76, a space).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Charset {
    pub width: Vec<i32>,
    pub height: Vec<i32>,
    pub step_x: Vec<i32>,
    pub step_y: Vec<i32>,
    pub start_y: Vec<i32>,
    pub per_row: Vec<i32>,
    pub order_hu: Vec<u8>,
    pub order: Vec<u8>,
}

/// A HUD brush record (`re/screens/hud.md` §1): a rectangle of picture `kep` drawn at `dst`. A negative size means
/// the source rectangle is in the original's pixels on a picture twice as large (the remaster's); frame `n` of an
/// animated brush starts `step·((n − 1) % per_row, (n − 1) / per_row)` further on.
#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct Brush {
    pub src: [i32; 2],
    pub dst: [i32; 2],
    pub size: [i32; 2],
    pub step: [i32; 2],
    pub per_row: i32,
    pub kep: i32,
}

impl Brush {
    pub fn from_record(r: &[u8]) -> Self {
        let w = |k: usize| i32_at(r, 4 * k);
        Self { src: [w(0), w(1)], dst: [w(2), w(3)], size: [w(4), w(5)], step: [w(6), w(7)], per_row: w(8), kep: w(9) }
    }
}

/// A spy mission (`KemKuldetesek`, 0x20 bytes; `re/rules/espionage.md` §1.2).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct SpyMission {
    /// +0: enabled (the story toggles story missions; `FRISSIT_Spy` clears mission 11 daily).
    pub enabled: u8,
    /// +4: 1 = the parameter is a planet of the target.
    pub kind: i32,
    pub w8: i32,
    /// +0xc: experience for a success.
    pub xp: i32,
    /// +0x10: length in days; +0x14: cost (`FRISSIT_MI_KEMKEDES`, `re/rules/ai-empire.md` §5).
    pub days: i32,
    pub cost: i32,
    /// +0x18: the rank a spy needs before the AI sends it.
    pub min_rank: u8,
    /// +0x19 / +0x1a / +0x1b: infiltration floor, infiltration for the full chance, full chance %.
    pub skill_floor: u8,
    pub skill_full: u8,
    pub chance: u8,
    /// +0x1c: needs a living target (and can be caught).
    pub needs_target: u8,
    /// +0x1d: story mission (the story script decides).
    pub story: u8,
    /// +0x1e: no countdown.
    pub no_countdown: u8,
    pub b1f: u8,
}

/// One colony problem curve (`ProblemakLeiro`, 6 × i32; entry 0 is the morale curve).
/// See `re/rules/colony-economy.md` §5.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct ProblemCurve {
    /// Value at the threshold.
    pub a: i32,
    /// Steepness.
    pub b: i32,
    /// Maximum value.
    pub c: i32,
    /// Threshold, percent of capacity.
    pub d: i32,
    /// Smoothing rate (multiplies the per-step change limit).
    pub rate: i32,
    /// Weight in the morale sum.
    pub weight: i32,
}

/// Equipment pod type (`PodTipusok`, 84 bytes).
#[derive(Debug, Clone, Serialize)]
pub struct PodType {
    pub index: usize,
    /// +0: pod type ID referenced by the slot table.
    pub id: i32,
    /// +4: built-in name (replaced with the localised one at new game).
    pub name: String,
    /// +20: up to 16 techs that fit this pod (0-terminated).
    pub techs: Vec<i32>,
}

/// One design slot: pod type ID and maximum count (`PodFajok`, `GetMaxRarakhato`).
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct PodSlot {
    pub pod_type: i32,
    pub max_count: i32,
}

/// Hull record (`Alapok`, 36 bytes; `re/rules/space-combat.md` §2.1).
#[derive(Debug, Clone, Serialize)]
pub struct Hull {
    pub index: usize,
    /// +0 (i16): 0 none, 1 space (ships and fighters), 2 ground vehicle.
    pub class: i16,
    /// +2..+7: colony ship, terraformer, fighter, space base, movable base, satellite.
    pub flags: [u8; 6],
    /// +8: hull id (equals the index).
    pub id: i32,
    /// +0xc: the hull's tech (base price, time, firepower and hit points of a design).
    pub tech: i32,
    /// +0x10..+0x23 [unk; read by the design and spy screens].
    pub extra: [i32; 5],
}

/// Space weapon record (`fegyverek`, 92 bytes; `re/rules/space-combat.md` §2.2).
#[derive(Debug, Clone, Serialize)]
pub struct SpaceWeapon {
    pub index: usize,
    /// +0: kind (2 = not counted in a ship's weapon range span; others [unk]).
    pub kind: i32,
    /// +4: reload time in battle ticks (also scales the derived damage, `fegyverszamol`).
    pub reload: i32,
    /// +8: damage per target type 0..=11 (recomputed by `fegyverszamol` from tech firepower; the live table matched the file).
    pub damage: [i32; 12],
    /// +0x38: hit chance in percent.
    pub accuracy: i32,
    /// +0x3c: aim spread (÷1000 per axis), +0x40 [unk], +0x44 projectile speed, +0x48 [unk].
    pub spread: f32,
    pub f40: f32,
    pub speed: f32,
    pub f48: f32,
    /// +0x4c: effect duration (disable/takeover weapons, kinds 9/10 of the projectile).
    pub duration: i32,
    /// +0x50: range.
    pub range: i32,
    /// +0x54: projectile graphics form.
    pub graphic: i32,
    /// +0x58 aimed at the target (else along the ship's heading), +0x59 [unk], +0x5a beam (stays on target).
    pub flags: [u8; 4],
}

/// A projectile form (`fegyveralakok`, 0x78 bytes; `re/rules/space-combat.md` §7.1): its model, sprite size and
/// pool size (the battle's projectile slots of this form).
#[derive(Debug, Clone, Serialize)]
pub struct ShotForm {
    pub index: usize,
    /// +0: the model file; empty = never loaded (the form can't be fired).
    pub model: String,
    /// +0x10: sprite size.
    pub size: f32,
    /// +0x14: pool slots.
    pub slots: i32,
}

/// Default design (`Tervek0`, 0x84 bytes; layout in `re/rules/space-combat.md` §2.3).
#[derive(Debug, Clone, Serialize)]
pub struct DesignTemplate {
    pub index: usize,
    pub name: String,
    pub raw: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Rules {
    pub techs: Vec<Tech>,
    /// `Talalmanyok0` as raw 276-byte records (193): the defaults `InitNewGame` copies to the live table
    /// `Talalmanyok` (exe 0xB9C0C8); a loaded save overwrites the per-empire state of each tech.
    pub tech_defaults: Vec<u8>,
    pub buildings: Vec<Building>,
    pub pod_types: Vec<PodType>,
    /// `[pod_table][hull][slot]`: 14 tables × 27 hulls × 12 slots.
    pub pod_slots: Vec<Vec<Vec<PodSlot>>>,
    /// Race → pod table (1-based index into `pod_slots`).
    pub race_pod_table: Vec<i32>,
    pub hulls: Vec<Hull>,
    pub designs: Vec<DesignTemplate>,
    pub space_weapons: Vec<SpaceWeapon>,
    pub shot_forms: Vec<ShotForm>,
    /// `TankAlapSpeed`: base speed of the ground hulls (techs 145..=149), ÷100 in designs.
    pub tank_speeds: Vec<f32>,
    pub skirmish_tech_start: Vec<i32>,
    /// `[race][surface]` 15 × 9 [inf: planet suitability].
    pub race_surface_affinity: Vec<Vec<i32>>,
    pub research_size_mult: Vec<f32>,
    pub race_production_mult: Vec<f32>,
    pub race_research_time_mult: Vec<f32>,
    /// `FajSpeckoKutatasMoneySzorzo`: research cost per race (`FRISSIT_Research`, exe 0x51ddd0).
    pub race_research_money_mult: Vec<f32>,
    pub difficulty_production_mult: Vec<f32>,
    pub difficulty_money_mult: Vec<f32>,
    pub difficulty_development_mult: Vec<f32>,
    pub race_peace_threshold: Vec<i32>,
    pub race_trade_demand: Vec<i32>,
    pub race_fear_threshold: Vec<i32>,
    pub race_ai_production_share: Vec<i32>,
    pub race_ai_military_autobuild: Vec<i32>,
    /// Colony problem curves, 20 entries.
    pub problem_curves: Vec<ProblemCurve>,
    /// Morale loss per kidnapped-family-member state (`CsaladtagMoralCsokkenesLeiro`).
    pub kidnap_morale: Vec<i32>,
    /// Wormhole `w`'s colour (`i32 [6][3]` at 0x913850, just before `RadarHatotav`; `AddWormhole` tints its ends
    /// with it, fleet-movement.md "Opening a wormhole").
    pub wormhole_colours: [[i32; 3]; 6],
    /// Radar range per radar tech level 0..=5 (`RadarHatotav`; fleet-movement.md §6).
    pub radar_ranges: Vec<f32>,
    /// Planet radar range per planet radar level 0..=7, copied to every empire at new game
    /// (`RadarHatotav2Init`, stored right after `RadarHatotav`).
    pub planet_radar_ranges: Vec<f32>,
    /// Satellite price by kind 0..=2 (`MuholdAr`, right after `RadarHatotav2Init`; `MULTIGET_Starmap` 0x18).
    pub satellite_prices: Vec<i32>,
    /// Autobuild categories 1..=28 (`re/rules/colony-side.md` §1.3): problem slot they answer
    /// (0 = listed category, −1 = none) and the building type range.
    pub autobuild_problem: Vec<i32>,
    pub autobuild_min: Vec<i32>,
    pub autobuild_max: Vec<i32>,
    /// `AutoBuildNevek[0..=16]`: category names matched by `autobldN.txt` (entry 0 is empty).
    pub autobuild_names: Vec<String>,
    /// `pGyartas0`: the empty production queue copied to a planet that changes hands.
    pub production_default: Vec<u8>,
    /// `Building[k − 1]` `+0x58/+0x5c`: footprint (width, height) of building kind `k` on the 200 × 200
    /// colony grid, indexed by kind (entry 0 = (0, 0); `re/rules/colony-placement.md` §1).
    pub building_footprints: Vec<(i32, i32)>,
    /// What the exe reads as kind 0's footprint (`Building[−1]` `+0x58/+0x5c` = 0x8F1F14, the int table before
    /// it): `SetTarget` on an emptied building slot takes its width (re/screens/colony.md §9.1).
    pub building_footprint0: (i32, i32),
    /// `Building[k − 1]`'s models, indexed by kind (entry 0 is empty).
    pub building_models: Vec<BuildingModel>,
    pub unit_models: UnitModels,
    pub research_screen: ResearchScreen,
    pub design_screen: DesignScreen,
    /// `utdarabsize[n − 1]`: the `AddToScene` scale of road piece `ut<n>.xvb`, n = 1..=9.
    pub road_scale: Vec<f32>,
    /// `allvanypoziciok[k − 1]`: building kind `k`'s scaffolding height (the selection marker sits 20 below it).
    pub scaffold_heights: Vec<f32>,
    pub charset: Charset,
    /// `FajnevX[e − 1]`: where the info screen writes race `e`'s name.
    pub race_name_x: Vec<i32>,
    /// `Kepek[k]`'s file names (`COLONY_X.BMP`, …), by picture index.
    pub pictures: Vec<String>,
    /// `ColonyBrushok`, `StarmapBrushok` (index 0 unused).
    pub colony_brushes: Vec<Brush>,
    pub starmap_brushes: Vec<Brush>,
    /// `Megbizhatatlansag_DeclareWar`: `[undeclared][row][victim, others]` unreliability a war adds
    /// (`re/rules/diplomacy.md` §8.1), 16 i32 in memory order.
    pub declare_war_unreliability: Vec<i32>,
    /// Espionage (`re/rules/espionage.md` §1.2): missions 0..=26, the new state per catch outcome 0..=4,
    /// skill points per rank by race, the 15 template spy records (0x4c bytes) by race, rank experience
    /// 0..=21 (21 = the i32 after the table), training experience by level, recruit names.
    pub spy_missions: Vec<SpyMission>,
    pub spy_outcome_state: Vec<i32>,
    pub spy_rank_points: Vec<i32>,
    pub spy_templates: Vec<Vec<u8>>,
    pub spy_rank_xp: Vec<i32>,
    pub spy_training_xp: Vec<i32>,
    pub spy_names: Vec<Vec<u8>>,
    /// `TrainingKuldetesAr[level]`: what a training mission costs (the AI's spy missions, `ai-empire.md` §5).
    pub spy_training_cost: Vec<i32>,
    /// `TrainingKuldetesIdo[level]`: a training mission's length in days (the spy screen, re/screens/spy.md §6).
    pub spy_training_days: Vec<i32>,
    /// `KemKuldetesPenzSzorzoFajonkent[race]`: mission cost multiplier.
    pub spy_race_cost_mult: Vec<f32>,
    /// `KemFelvetelPenzSzorzoFajonkent[race]`: an applicant's price per rank, ÷ 3000 (`espionage.md` §8).
    pub spy_hire_cost_mult: Vec<f32>,
    /// `MiGyartasMod` / `MiGyartasMitMin` / `MiGyartasMitMax`, categories 0..=10 (`ai-empire.md` §1.2).
    pub ai_build_mode: Vec<u8>,
    pub ai_build_min: Vec<u8>,
    pub ai_build_max: Vec<u8>,
    /// `tipusnevek[race·12 + type]`: the battle's ship types (`re/rules/space-combat.md` §7.2, §7.10).
    pub ship_types: Vec<ShipTypeRecord>,
    /// `Talalmanyok2[tech · 9 + race]` (`ai-empire.md` §10.3).
    pub ai_ground_priority: Vec<u8>,
    /// Ground combat (`re/rules/ground-combat.md` §1.3): `WF_Weapons` as 14 × [min, max, period, unk]
    /// (the auto-resolve's damage ranges and periods), member hit points by hull id 0..=24
    /// (`TankAlapHP` for 0x13..=0x17; other ids read the neighbouring memory like the exe), and the
    /// battlefield size used for map positions.
    pub ground_weapons: Vec<[i32; 4]>,
    pub ground_hull_hp: Vec<i32>,
    pub ground_map_size: (f32, f32),
    /// The real-time ground war's weapons (`WF_NonSimulateWeapons` 0x8F83A0) and building turrets'
    /// (`WF_FortressWeapons` 0x8F8480): 14 × [min, max, reload, range].
    pub ground_rt_weapons: Vec<[i32; 4]>,
    pub ground_fortress_weapons: Vec<[i32; 4]>,
    /// Turning speeds by ground hull index (exe 0x8F8560, f32 × 5).
    pub ground_turn_speeds: Vec<f32>,
    /// Armour factors `[hull·3 + side]` (exe 0x8F858C, f32 × 15; side 0 front, 1 flank, 2 rear).
    pub ground_armour: Vec<f32>,
    /// The campaign tables as the exe's words from VA 0x928340 (`re/rules/campaign.md` §8.1); the story
    /// engine indexes them like the exe (`ig2_sim::campaign::CampaignTables`).
    pub campaign_words: Vec<i32>,
    /// Slope limits of the passability layers 0..5 (exe 0x8F8574, f32 × 5; `GenerateMap` uses 1..=4).
    pub ground_slope_limits: Vec<f32>,
    pub diplomacy_screen: DiplomacyScreen,
    pub videos: Videos,
}

/// The video names the exe picks from tables (`re/screens/video.md` §5); `""` where the exe has an empty string.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Videos {
    /// By `EN` 0..=8: `MEGHALAS`'s clip before `misc/end_gameover.bik`.
    pub loss: Vec<String>,
    /// By `EN` 0..=8: `MEGNYERES`'s campaign clip before `misc/end_campaign_win.bik`.
    pub win: Vec<String>,
    /// By the message's head index: (loop, wait) clips.
    pub message_clips: Vec<(String, String)>,
}

/// The diplomacy screen's layout (`re/screens/diplomacy.md` §2).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct DiplomacyScreen {
    /// `GombPoziciok`: the six head slots `(x, y, z)` in camera space.
    pub heads: Vec<[f32; 3]>,
    /// `DipFejX`: the x of each slot's name.
    pub name_x: Vec<i32>,
    /// `MenukKezdX/Y`, `InfonevX/Y`.
    pub menu_xy: [i32; 2],
    pub info_xy: [i32; 2],
    /// `InfonevFont`, `InfoSzin1`, `InfoSzin2`.
    pub info_font: i32,
    pub info_colours: [i32; 2],
    /// `PnevekFont`, `PnevekSzin2`.
    pub name_font: i32,
    pub name_colour: i32,
    /// `MenuMaxSorokO` (rows per column), `MenuOszlopKoz` (column gap).
    pub menu_rows: i32,
    pub menu_gap: i32,
    /// `TexturaNevek`: the portrait textures, two per race (`a`, `b`).
    pub portraits: Vec<String>,
    /// `KemFejNevek[race]`: the leaders' videos in `anim/fejek/` (entry 0 isn't a race).
    pub head_anims: Vec<String>,
}

/// A battle ship type (`tipusnevek`, exe 0x91F488, 0x30 bytes; record `race·12 + type`, spec §7.10).
#[derive(Debug, Clone, Serialize)]
pub struct ShipTypeRecord {
    pub index: usize,
    /// +0: model name.
    pub name: String,
    /// +0x10: model scale (radius in world units / 1024).
    pub scale: f32,
    /// +0x14 top speed, +0x18 [unk], +0x1c acceleration, +0x20 [unk], +0x24 turn rate.
    pub top_speed: f32,
    pub f18: f32,
    pub accel: f32,
    pub f20: f32,
    pub turn: f32,
    /// +0x28 / +0x2c: footprint in grid cells (horizontal, vertical); `AddUjUrhajo` rewrites it for a ship's
    /// type from the scale before placing the ship.
    pub size_xy: i32,
    pub size_z: i32,
}

impl Rules {
    /// `fegyverek` as the exe holds it (40 × 0x5c bytes), for readers that index past a record like the battle does.
    pub fn space_weapon_table(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.space_weapons.len() * 0x5c);
        for w in &self.space_weapons {
            out.extend_from_slice(&w.kind.to_le_bytes());
            out.extend_from_slice(&w.reload.to_le_bytes());
            for d in w.damage {
                out.extend_from_slice(&d.to_le_bytes());
            }
            out.extend_from_slice(&w.accuracy.to_le_bytes());
            for f in [w.spread, w.f40, w.speed, w.f48] {
                out.extend_from_slice(&f.to_le_bytes());
            }
            for v in [w.duration, w.range, w.graphic] {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&w.flags);
        }
        out
    }

    /// Read every catalogued table from the bytes of the user's `ig2.exe`.
    pub fn from_exe(exe: &[u8]) -> Result<Self> {
        check_build(exe)?;
        let img = PeImage::parse(exe)?;
        let tab = |(va, stride, n): (u32, usize, usize)| img.read(va, stride * n).map(|b| (b, stride, n));
        let ints = |t| -> Result<Vec<i32>> { let (b, _, n) = tab(t)?; Ok((0..n).map(|i| i32_at(b, 4 * i)).collect()) };
        let floats = |t| -> Result<Vec<f32>> { Ok(ints(t)?.into_iter().map(|x| f32::from_bits(x as u32)).collect()) };

        let firepower = ints(layout::TECH_FIREPOWER)?;
        let (tb, ts, tn) = tab(layout::TECHS)?;
        let techs = (0..tn)
            .map(|i| {
                let r = &tb[i * ts..(i + 1) * ts];
                Tech {
                    index: i,
                    name: cstr(&r[..20]),
                    price: i32_at(r, 20),
                    build_time: i32_at(r, 24),
                    auto_design_flag: r[38],
                    auto_design_kind: i32_at(r, 40),
                    prerequisites: [r[45], r[46], r[47]],
                    lab_levels: [r[49], r[50], r[51]],
                    research_cost: i32_at(r, 52),
                    research_time: i32_at(r, 56),
                    hp: i32_at(r, 32),
                    capacity: r[37],
                    unknown: [i32_at(r, 28), i32_at(r, 36), i32_at(r, 60), i32_at(r, 64), i32_at(r, 68) & 0xFF],
                    firepower: firepower[i],
                }
            })
            .collect();

        let (bb, bs, bn) = tab(layout::BUILDINGS)?;
        let buildings = (0..bn)
            .map(|i| {
                let r = &bb[i * bs..(i + 1) * bs];
                Building {
                    index: i,
                    tech: r[0],
                    b1: r[1],
                    priority: r[2],
                    b3: r[3],
                    build_work: i32_at(r, 4),
                    max_hp: i32_at(r, 8),
                    energy: i32_at(r, 12),
                    workers: i32_at(r, 16),
                    output: i32_at(r, 20),
                    w24: i32_at(r, 24),
                    w28: i32_at(r, 28),
                    flags: [r[32], r[33]],
                }
            })
            .collect();

        let (pb, ps, pn) = tab(layout::POD_TYPES)?;
        let pod_types = (0..pn)
            .map(|i| {
                let r = &pb[i * ps..(i + 1) * ps];
                PodType {
                    index: i,
                    id: i32_at(r, 0),
                    name: cstr(&r[4..20]).trim_end().to_owned(),
                    techs: (0..16).map(|k| i32_at(r, 20 + 4 * k)).take_while(|&t| t != 0).collect(),
                }
            })
            .collect();

        // Stored as (pod_type, max_count) i32 pairs.
        let (va, stride, n) = layout::POD_SLOTS;
        let sb = img.read(va, stride * n)?;
        let slots: Vec<i32> = (0..2 * n).map(|k| i32_at(sb, 4 * k)).collect();
        let pod_slots = slots
            .chunks(2)
            .map(|c| PodSlot { pod_type: c[0], max_count: c[1] })
            .collect::<Vec<_>>()
            .chunks(27 * 12)
            .map(|t| t.chunks(12).map(<[PodSlot]>::to_vec).collect())
            .collect();

        let rec_ints = |t: (u32, usize, usize)| -> Result<Vec<Vec<i32>>> {
            let (b, s, n) = tab(t)?;
            Ok((0..n).map(|i| (0..s / 4).map(|k| i32_at(b, i * s + 4 * k)).collect()).collect())
        };
        let (db, ds, dn) = tab(layout::DESIGNS)?;
        let designs = (0..dn)
            .map(|i| {
                let r = &db[i * ds..(i + 1) * ds];
                DesignTemplate { index: i, name: cstr(&r[..20]), raw: r.to_vec() }
            })
            .collect();
        let (hb, hs, hn) = tab(layout::HULLS)?;
        let hulls = (0..hn)
            .map(|i| {
                let r = &hb[i * hs..(i + 1) * hs];
                Hull {
                    index: i,
                    class: i16::from_le_bytes([r[0], r[1]]),
                    flags: r[2..8].try_into().unwrap(),
                    id: i32_at(r, 8),
                    tech: i32_at(r, 12),
                    extra: std::array::from_fn(|k| i32_at(r, 16 + 4 * k)),
                }
            })
            .collect();
        let (wb, ws, wn) = tab(layout::SPACE_WEAPONS)?;
        let f32_at = |r: &[u8], o: usize| f32::from_bits(i32_at(r, o) as u32);
        let space_weapons = (0..wn)
            .map(|i| {
                let r = &wb[i * ws..(i + 1) * ws];
                SpaceWeapon {
                    index: i,
                    kind: i32_at(r, 0),
                    reload: i32_at(r, 4),
                    damage: std::array::from_fn(|k| i32_at(r, 8 + 4 * k)),
                    accuracy: i32_at(r, 0x38),
                    spread: f32_at(r, 0x3c),
                    f40: f32_at(r, 0x40),
                    speed: f32_at(r, 0x44),
                    f48: f32_at(r, 0x48),
                    duration: i32_at(r, 0x4c),
                    range: i32_at(r, 0x50),
                    graphic: i32_at(r, 0x54),
                    flags: r[0x58..0x5c].try_into().unwrap(),
                }
            })
            .collect();
        let affinity = ints(layout::RACE_SURFACE_AFFINITY)?;
        let autobuild_names = ints(layout::AUTOBUILD_NAMES)?
            .into_iter()
            .map(|ptr| match ptr as u32 {
                0 => Ok(String::new()),
                va => img.read(va, 32).map(cstr), // names are < 20 chars, inside .rdata
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            tech_defaults: tb.to_vec(),
            techs,
            buildings,
            pod_types,
            pod_slots,
            race_pod_table: ints(layout::RACE_POD_TABLE)?,
            hulls,
            designs,
            space_weapons,
            shot_forms: {
                let (b, s, n) = tab(layout::SHOT_FORMS)?;
                (0..n)
                    .map(|i| {
                        let r = &b[i * s..(i + 1) * s];
                        ShotForm { index: i, model: cstr(&r[..0x10]), size: f32::from_bits(i32_at(r, 0x10) as u32), slots: i32_at(r, 0x14) }
                    })
                    .collect()
            },
            tank_speeds: floats(layout::TANK_SPEEDS)?,
            skirmish_tech_start: ints(layout::SKIRMISH_TECH_START)?,
            race_surface_affinity: affinity.chunks(9).map(<[i32]>::to_vec).collect(),
            research_size_mult: floats(layout::RESEARCH_SIZE_MULT)?,
            race_production_mult: floats(layout::RACE_PRODUCTION_MULT)?,
            race_research_time_mult: floats(layout::RACE_RESEARCH_TIME_MULT)?,
            race_research_money_mult: floats(layout::RACE_RESEARCH_MONEY_MULT)?,
            difficulty_production_mult: floats(layout::DIFFICULTY_PRODUCTION_MULT)?,
            difficulty_money_mult: floats(layout::DIFFICULTY_MONEY_MULT)?,
            difficulty_development_mult: floats(layout::DIFFICULTY_DEVELOPMENT_MULT)?,
            race_peace_threshold: ints(layout::RACE_PEACE_THRESHOLD)?,
            race_trade_demand: ints(layout::RACE_TRADE_DEMAND)?,
            race_fear_threshold: ints(layout::RACE_FEAR_THRESHOLD)?,
            race_ai_production_share: ints(layout::RACE_AI_PRODUCTION_SHARE)?,
            race_ai_military_autobuild: ints(layout::RACE_AI_MILITARY_AUTOBUILD)?,
            problem_curves: rec_ints(layout::PROBLEM_CURVES)?
                .into_iter()
                .map(|v| ProblemCurve { a: v[0], b: v[1], c: v[2], d: v[3], rate: v[4], weight: v[5] })
                .collect(),
            kidnap_morale: ints(layout::KIDNAP_MORALE)?,
            wormhole_colours: {
                let v = rec_ints(layout::WORMHOLE_COLOURS)?;
                std::array::from_fn(|w| std::array::from_fn(|i| v[w][i]))
            },
            radar_ranges: floats(layout::RADAR_RANGES)?,
            planet_radar_ranges: floats(layout::PLANET_RADAR_RANGES)?,
            satellite_prices: ints(layout::SATELLITE_PRICES)?,
            autobuild_problem: ints(layout::AUTOBUILD_PROBLEM)?,
            autobuild_min: ints(layout::AUTOBUILD_MIN)?,
            autobuild_max: ints(layout::AUTOBUILD_MAX)?,
            autobuild_names,
            production_default: tab(layout::PRODUCTION_DEFAULT)?.0.to_vec(),
            building_footprints: {
                let (b, s, n) = tab(layout::BUILDING_MODELS)?;
                std::iter::once((0, 0)).chain((0..n).map(|i| (i32_at(b, i * s + 0x58), i32_at(b, i * s + 0x5c)))).collect()
            },
            building_footprint0: {
                let b = img.read(layout::BUILDING_MODELS.0 - layout::BUILDING_MODELS.1 as u32 + 0x58, 8)?;
                (i32_at(b, 0), i32_at(b, 4))
            },
            road_scale: floats(layout::ROAD_SCALE)?,
            scaffold_heights: floats(layout::SCAFFOLD_HEIGHTS)?,
            charset: {
                let t = ints(layout::CHAR_TABLES)?;
                let bytes = |t| -> Result<Vec<u8>> { Ok(tab(t)?.0.to_vec()) };
                Charset {
                    width: t[0..5].to_vec(),
                    height: t[5..10].to_vec(),
                    step_x: t[10..15].to_vec(),
                    step_y: t[15..20].to_vec(),
                    start_y: t[20..25].to_vec(),
                    per_row: t[25..30].to_vec(),
                    order_hu: bytes(layout::GLYPHS_HU)?,
                    order: bytes(layout::GLYPHS)?,
                }
            },
            race_name_x: ints(layout::RACE_NAME_X)?,
            pictures: {
                let (b, s, n) = tab(layout::PICTURES)?;
                (0..n).map(|i| cstr(&b[i * s..i * s + 0x100])).collect()
            },
            colony_brushes: {
                let (b, s, n) = tab(layout::COLONY_BRUSHES)?;
                (0..n).map(|i| Brush::from_record(&b[i * s..(i + 1) * s])).collect()
            },
            starmap_brushes: {
                let (b, s, n) = tab(layout::STARMAP_BRUSHES)?;
                (0..n).map(|i| Brush::from_record(&b[i * s..(i + 1) * s])).collect()
            },
            unit_models: {
                let models = |t| -> Result<Vec<UnitModel>> {
                    let (b, s, n) = tab(t)?;
                    Ok((0..n).map(|i| UnitModel { name: cstr(&b[i * s..i * s + 0x28]), dir: i32_at(b, i * s + 0x34) }).collect())
                };
                UnitModels {
                    hulls: models(layout::TANK_MODELS)?,
                    hull_scale: floats(layout::TANK_SCALE)?,
                    pivot: floats(layout::TANK_PIVOT)?,
                    turrets: models(layout::TURRET_MODELS)?,
                    turret_scale: floats(layout::TURRET_SCALE)?,
                    defence_scale: floats(layout::DEFENCE_TURRET_SCALE)?,
                    transfer: models(layout::TRANSFER_MODELS)?,
                }
            },
            design_screen: DesignScreen {
                models: ints(layout::DESIGN_HULL_MODELS)?
                    .into_iter()
                    .map(|ptr| match ptr as u32 {
                        0 => Ok(String::new()),
                        va => img.read(va, 32).map(cstr),
                    })
                    .collect::<Result<Vec<_>>>()?,
                scale: floats(layout::DESIGN_HULL_SCALE)?,
                pod_slots: ints(layout::DESIGN_POD_SLOTS)?,
                pod_table: ints(layout::DESIGN_POD_TABLE_OF_RACE)?,
                brushes: {
                    let (b, s, n) = tab(layout::DESIGN_BRUSHES)?;
                    (0..n).map(|i| Brush::from_record(&b[i * s..(i + 1) * s])).collect()
                },
            },
            research_screen: {
                let w = ints(layout::RESEARCH_WHEEL)?;
                let l = ints(layout::RESEARCH_WHEEL_LINES)?;
                let labs = ints(layout::RESEARCH_LABS_XY)?;
                let (x, y, z) = (floats(layout::RESEARCH_ORBIT_X)?, floats(layout::RESEARCH_ORBIT_Y)?, floats(layout::RESEARCH_ORBIT_Z)?);
                ResearchScreen {
                    models: ints(layout::RESEARCH_MODELS)?
                        .into_iter()
                        .map(|ptr| match ptr as u32 {
                            0 => Ok(String::new()),
                            va => img.read(va, 32).map(cstr),
                        })
                        .collect::<Result<Vec<_>>>()?,
                    orbit: (0..x.len()).map(|k| [x[k], y[k], z[k]]).collect(),
                    wheel_kep: std::array::from_fn(|i| w[i]),
                    wheel_area: std::array::from_fn(|i| w[5 + i]),
                    wheel_button: std::array::from_fn(|i| w[10 + i]),
                    wheel_line: std::array::from_fn(|i| [l[i], l[5 + i]]),
                    lab_x: std::array::from_fn(|i| labs[i]),
                    lab_x2: std::array::from_fn(|i| labs[4 + i]),
                    lab_y: std::array::from_fn(|i| labs[8 + i]),
                }
            },
            building_models: {
                let (b, s, n) = tab(layout::BUILDING_MODELS)?;
                let f = |o: usize| f32::from_bits(i32_at(b, o) as u32);
                let empty = BuildingModel { model: String::new(), preview: String::new(), scale: 0.0, preview_scale: 0.0, select: 0.0 };
                std::iter::once(empty)
                    .chain((0..n).map(|i| {
                        let r = i * s;
                        BuildingModel { model: cstr(&b[r..r + 0x28]), preview: cstr(&b[r + 0x28..r + 0x50]), scale: f(r + 0x50), preview_scale: f(r + 0x54), select: f(r + 0x68) }
                    }))
                    .collect()
            },
            declare_war_unreliability: ints(layout::DECLARE_WAR_UNRELIABILITY)?,
            spy_missions: {
                let (b, s, n) = tab(layout::SPY_MISSIONS)?;
                (0..n)
                    .map(|i| {
                        let r = &b[i * s..(i + 1) * s];
                        SpyMission {
                            enabled: r[0],
                            kind: i32_at(r, 4),
                            w8: i32_at(r, 8),
                            xp: i32_at(r, 0xc),
                            days: i32_at(r, 0x10),
                            cost: i32_at(r, 0x14),
                            min_rank: r[0x18],
                            skill_floor: r[0x19],
                            skill_full: r[0x1a],
                            chance: r[0x1b],
                            needs_target: r[0x1c],
                            story: r[0x1d],
                            no_countdown: r[0x1e],
                            b1f: r[0x1f],
                        }
                    })
                    .collect()
            },
            spy_outcome_state: ints(layout::SPY_OUTCOME_STATE)?,
            spy_rank_points: ints(layout::SPY_RANK_POINTS)?,
            spy_templates: {
                let (b, s, n) = tab(layout::SPY_TEMPLATES)?;
                (0..n).map(|i| b[i * s..(i + 1) * s].to_vec()).collect()
            },
            spy_rank_xp: ints(layout::SPY_RANK_XP)?,
            ground_weapons: ints(layout::GROUND_WEAPONS)?.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
            ground_hull_hp: ints(layout::GROUND_HULL_HP)?,
            ground_map_size: { let m = floats(layout::GROUND_MAP_SIZE)?; (m[0], m[1]) },
            ground_rt_weapons: ints(layout::GROUND_RT_WEAPONS)?.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
            ground_fortress_weapons: ints(layout::GROUND_FORTRESS_WEAPONS)?.chunks(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
            ground_turn_speeds: floats(layout::GROUND_TURN_SPEEDS)?,
            ground_armour: floats(layout::GROUND_ARMOUR)?,
            campaign_words: ints(layout::CAMPAIGN)?,
            ground_slope_limits: floats(layout::GROUND_SLOPE_LIMITS)?,
            diplomacy_screen: {
                let names = |t| -> Result<Vec<String>> {
                    ints(t)?.into_iter().map(|ptr| Ok(img.read(ptr as u32, 32).map(cstr).unwrap_or_default())).collect()
                };
                let pair = |t| -> Result<[i32; 2]> { let v = ints(t)?; Ok([v[0], v[1]]) };
                let info = ints(layout::DIPLOMACY_INFO_STYLE)?;
                let style = ints(layout::DIPLOMACY_NAME_STYLE)?;
                let grid = ints(layout::DIPLOMACY_MENU_GRID)?;
                DiplomacyScreen {
                    heads: floats(layout::DIPLOMACY_HEADS)?.chunks(4).map(|c| [c[0], c[1], c[2]]).collect(),
                    name_x: ints(layout::DIPLOMACY_NAME_X)?,
                    menu_xy: pair(layout::DIPLOMACY_MENU_XY)?,
                    info_xy: pair(layout::DIPLOMACY_INFO_XY)?,
                    info_font: info[0],
                    info_colours: [info[1], info[2]],
                    name_font: style[0],
                    name_colour: style[1],
                    menu_rows: grid[0],
                    menu_gap: grid[1],
                    portraits: names(layout::DIPLOMACY_PORTRAITS)?,
                    head_anims: names(layout::DIPLOMACY_HEAD_ANIMS)?,
                }
            },
            spy_training_xp: ints(layout::SPY_TRAINING_XP)?,
            spy_names: spy_names(&img)?,
            spy_training_cost: ints(layout::SPY_TRAINING_COST)?,
            spy_training_days: ints(layout::SPY_TRAINING_DAYS)?,
            spy_race_cost_mult: floats(layout::SPY_RACE_COST_MULT)?,
            spy_hire_cost_mult: floats(layout::SPY_HIRE_COST_MULT)?,
            ai_build_mode: tab(layout::AI_BUILD_MODE)?.0.to_vec(),
            ai_build_min: tab(layout::AI_BUILD_MIN)?.0.to_vec(),
            ai_build_max: tab(layout::AI_BUILD_MAX)?.0.to_vec(),
            ship_types: {
                let (b, s, n) = tab(layout::SHIP_TYPES)?;
                let f = |r: &[u8], o: usize| f32::from_bits(i32_at(r, o) as u32);
                (0..n)
                    .map(|i| {
                        let r = &b[i * s..(i + 1) * s];
                        ShipTypeRecord {
                            index: i,
                            name: cstr(&r[..0x10]),
                            scale: f(r, 0x10),
                            top_speed: f(r, 0x14),
                            f18: f(r, 0x18),
                            accel: f(r, 0x1c),
                            f20: f(r, 0x20),
                            turn: f(r, 0x24),
                            size_xy: i32_at(r, 0x28),
                            size_z: i32_at(r, 0x2c),
                        }
                    })
                    .collect()
            },
            ai_ground_priority: tab(layout::AI_GROUND_PRIORITY)?.0.to_vec(),
            videos: {
                let names = |t| -> Result<Vec<String>> {
                    ints(t)?.into_iter().map(|ptr| Ok(img.read(ptr as u32, 64).map(cstr).unwrap_or_default())).collect()
                };
                let clips = names(layout::VIDEO_MESSAGE_CLIPS)?;
                Videos {
                    loss: names(layout::VIDEO_LOSS)?,
                    win: names(layout::VIDEO_WIN)?,
                    message_clips: clips.chunks(2).map(|c| (c[0].clone(), c[1].clone())).collect(),
                }
            },
        })
    }

    /// Read from an install directory.
    pub fn from_install(install: &ig2_formats::Install) -> std::result::Result<Self, String> {
        let exe = std::fs::read(install.root().join("ig2.exe")).map_err(|e| e.to_string())?;
        Self::from_exe(&exe).map_err(|e| e.to_string())
    }

    /// Serialise to RON (for local inspection and modding; never commit the output).
    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).expect("serialisable")
    }
}
