//! The tutorial interpreter: `TutorialFrissit` (ELF 0x7bd460 / exe 0x60c020) running the script
//! `tutorial/main/script/main.tut` through `TutorialGetNextCommand` (ELF 0x7b4bc0 / exe 0x60c570), its waits,
//! events, text window and gate flags.
//!
//! Spec: `re/rules/tutorial.md` (§1 state, §2 driver, §3 the 74 commands, §4 events, §5 gate flags, §6 the
//! `.mou` replay). The script's lexing and the `.mou` records are `ig2_formats::tutorial`.
//!
//! The interpreter runs on the UI tick (100 Hz), beside the game: it reads the screen, the 20 Hz game counter
//! and a few counts through [`TutorialHost`], and everything it does outside its own state goes out as an
//! [`Effect`]. The effects marked *sim* in the spec change the game and belong in the lockstep queue (the
//! tutorial is single player); `Game::tutorial_effect` carries out the ones the sim owns. The client's overlay
//! reads [`Tutorial::window`], [`Tutorial::gates`] and [`Tutorial::mouse_sample`].

/// The text window's grid: 8 rows of 40 (character, colour) cells (`TKTomb`).
pub const ROWS: usize = 8;
pub const COLS: usize = 40;
/// `TKQueue`'s size (unchecked in the exe; the port grows it).
const QUEUE: usize = 3000;
/// `AktKep` of the options (ESC) screen, where the tutorial stands still.
pub const SCREEN_OPTIONS: i32 = 0xd;
/// `AktKepKell` after `RETURNTOMAINMENU`: the main screen.
pub const SCREEN_MAIN: i32 = 0xf;
/// `TutorialVisszaSzamlalo` for `15masodperc` (UI ticks).
const COUNTDOWN: i32 = 0x640;

const CR: u8 = b'\r';
const HASH: u8 = b'#';
const SP: u8 = b' ';

/// `WAITFOREVENT`'s names (`strcasecmp`, first hit wins) and the ids `TutorialCheckEvent` takes (spec §4).
pub const EVENTS: [(&str, i32); 43] = [
    ("colonizeaplanet", 0x02), ("exploreaplanet", 0x01), ("renameplanet", 0x03), ("renamefleet", 0x04),
    ("splitfleet", 0x05), ("producedestroyer", 0x06), ("gotocolony", 0x07), ("click on tank turret 1.0", 0x08),
    ("research laser 2.0", 0x09), ("research heavy destroyer", 0x0a), ("equip laser 2.0", 0x0b),
    ("designdestroyer", 0x0c), ("make friends in diplomacy", 0x0d), ("disable empire 6 in diplomacy", 0x0e),
    ("send a spy for information", 0x0f), ("send a spy for counter intell", 0x10), ("attack darkon empire", 0x11),
    ("press i in spacewar", 0x12), ("build a small house", 0x13), ("build a spaceship factory", 0x17),
    ("build a tank factory", 0x18), ("build a fusion plant", 0x14), ("movefleet", 0x15),
    ("exitfromplanetinfo", 0x16), ("build 3 tanks", 0x19), ("gotostarmap", 0x1a), ("press pageupordown", 0x1b),
    ("gotonomaincolony", 0x1c), ("build a police", 0x1d), ("transfer tanks", 0x1e), ("attack any empire", 0x1f),
    ("win in spacebattle", 0x20), ("win in groundbattle", 0x21), ("15masodperc", 0x22),
    ("build a ship research", 0x23), ("changetax", 0x24), ("changeautobuild", 0x25), ("buildtradeport", 0x26),
    ("buydestroyer", 0x27), ("changetodiplomacy", 0x28), ("buildfoodfactory", 0x29), ("exchangedipl", 0x2a),
    ("sendsattoplanet", 0x2b),
];
/// The id `TutorialFrissit`'s countdown fires.
pub const EVENT_COUNTDOWN: i32 = 0x22;

/// What the interpreter reads from the game and the UI.
pub trait TutorialHost {
    /// `AktKep`.
    fn screen(&self) -> i32;
    /// `Counter`, the 20 Hz game counter (`COUNT` and `TutorialStartCounter`).
    fn counter(&self) -> i32;
    /// `Atlepes || Belep`: a screen transition is running (`WAITFORATLEPES`).
    fn transition(&self) -> bool {
        false
    }
    /// `SelfPlaying` (demo mode: the tutorial also runs on the options screen).
    fn self_playing(&self) -> bool {
        false
    }
    /// `ExistAnyTutorialMp2()`: a tutorial speech stream still plays (`PLAYWAV` waits for it).
    fn speech_playing(&mut self) -> bool {
        false
    }
    /// `TutorialStartMp2_2(name)`: start `mfx/tutorial/<name>` and return its length in bytes (0 = missing).
    fn start_speech(&mut self, _name: &[u8]) -> i32 {
        0
    }
    /// `TutorialPlayMouse(name)`: the sample count of `tutorial/main/<name>` (`ig2_formats::tutorial`).
    fn mouse_samples(&mut self, _name: &[u8]) -> i32 {
        0
    }
    /// Planets with `explored[EN] == 1`, planets owned by EN, `CountMyTanks()` (`TutorialInitEvents`).
    fn explored_planets(&self) -> i32 {
        0
    }
    fn owned_planets(&self) -> i32 {
        0
    }
    fn my_tanks(&self) -> i32 {
        0
    }
    /// The vehicles of EN's fleets named "Assault fleet" whose design byte +0x36 is 0 (`TutorialCheckEvents`).
    fn assault_tanks(&self) -> i32 {
        0
    }
    /// Everything the interpreter does outside its own state, in order.
    fn effect(&mut self, e: Effect);
}

/// A command's work outside the interpreter (spec §3; *sim* = the game's state, the rest is UI).
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// `END`: the message line "Tutorial END." (`AddSzoveg(.., 7)`).
    End,
    /// `RETURNTOMAINMENU`: `AktKepKell = 0xf`.
    ReturnToMainMenu,
    /// `QUIT`: the game exits.
    Quit,
    /// `TutorialStop`: restore the battle debug flags (if `DISABLE OVERRUN` changed them), the debug AI flag
    /// saved at the start, and (`restore_music`) the music volume.
    Stopped { restore_overrun: bool, restore_music: bool },
    /// `KURVAANYAD`: the colony's planet-info view.
    PlanetInfo,
    /// *sim* `GIVE MONEY n`: `MoneyHozzaadLog(EN, 0, n)`.
    GiveMoney(i32),
    /// *sim* `DISABLE OVERRUN`: battle debug flags 0xb77230 = 0, 0xb77234 = 0, 0xb77238 = 1, 0xb7723c = 1.
    DisableOverrun,
    /// `DOUBLEGOMB ON`: `ResearchDoubleGomb()` (toggles the research screen's double button, message 0x2b).
    DoubleButton,
    /// *sim* `DISABLE AI` / `ENABLE AI`: the debug AI-off flag (exe 0xb77244).
    AiDisabled(bool),
    /// `CENTERFLEET`: select and centre EN's first fleet.
    CenterFleet,
    /// `FIND FLEET "name"`: EN's first fleet of that name.
    FindFleet(Vec<u8>),
    /// `FIND PLANET "name"`: EN's first planet of that name.
    FindPlanet(Vec<u8>),
    /// *sim* `SHOW PLANET "name"`: the first planet of that name gets `explored[EN] = 1`.
    ShowPlanet(Vec<u8>),
    /// `CENTERPLANET`: EN's first planet.
    CenterPlanet,
    /// `CENTERUNCOLONISEDPLANET`: the first planet explored by EN and owned by nobody.
    CenterUncolonisedPlanet,
    /// *sim* the tech state byte of EN (`ENABLE INVENTION` 5, `DISABLE INVENTION` 1, `research laser 2.0`
    /// sets tech 0x12 to 3).
    Invention { tech: i32, state: u8 },
    /// *sim* `DISABLE AUTOBUILD`: planet +0x6c = 0 on every planet.
    DisableAutobuild,
    /// *sim* `DISABLE TRADERS` / `ENABLE TRADERS`: the merchants' (owner 9) hold bit.
    Traders { enabled: bool },
    /// *sim* `WAITFORBATTLE`: `TankNew(f, 9, 2)` for each fleet of EN without vehicles.
    BattleVehicles,
    /// *sim* `WAITFOREVENT`: planet +0x186 = 1 on EN's planets.
    MarkOwnedPlanets,
    /// *sim* `LOADSAVEGAME n`.
    LoadSaveGame(i32),
    /// *sim* `GAMESPEED n` (0 = pause, else speed n and unpause), `PAUSE`, `UNPAUSE`.
    GameSpeed(i32),
    Pause(bool),
    /// *sim* `DIPLOMACY EXPLORE EMPIRE e` (`FelfedezBirodalom(e, true)` first) / `EXPLORE EMPIRE e`:
    /// `BirodalomIsmeri[EN][e] = 1`.
    ExploreEmpire { empire: i32, diplomacy: bool },
    /// *sim* `GIVE KILLER FLEET e` (e == EN: "Terminator" beside EN's last planet; else "Unbeatable").
    KillerFleet(i32),
    /// *sim* `GIVE KOLONIZALO FLEET`: "Colonization" beside EN's last planet.
    ColonizationFleet,
    /// *sim* `IRRITATEME`: `DiplomaciaTalkAbout(6, EN, 4, -1)`.
    Irritate,
    /// *sim* `ADD DESTROYERS TO TRADE`: `DestoryerATradereTutorialhoz()` four times.
    DestroyersToTrade,
    /// `GOTO SCREEN n`: `AktKepKell = n`.
    GotoScreen(i32),
    /// *sim* `SEND MESSAGE n`: `NewMessage(n, 0)`.
    SendMessage(i32),
    /// `TEXTURE "name"`: `DemoMode_TexturaNev`.
    Texture(Vec<u8>),
    /// The script read a quoted name across a line break where the exe loops forever (`FIND*`, `SHOW PLANET`,
    /// `WAITFOREVENT`, `TEXTURE`; `re/deviations.md`): the port stops the tutorial instead.
    Hang,
}

/// The flags other screens read (spec §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gates {
    /// `KepernyokLetiltva[20]`.
    pub screens: [bool; 20],
    /// `TutorialEpuletekLetiltva[0x3c]`.
    pub buildings: [bool; 0x3c],
    pub split: bool,
    pub production: bool,
    /// `TutorialDisableHajokEladasa` (on from the start).
    pub sell_ships: bool,
    /// `TutorialExitGombDisabled`.
    pub exit_button: bool,
    /// `TutorialStarmapMindenLetiltvaKiveveAKurzorMozgatoBillentyuket`.
    pub starmap: bool,
    /// `TutorialColonyMindenLetiltvaKiveveAPageupot`.
    pub colony: bool,
    /// `TutorialDisableEpuletLerombolas`.
    pub demolish: bool,
    /// `TutorialUjKemekLetiltva` (read by `FRISSIT_Spy`).
    pub new_spies: bool,
    /// `TutorialSpyMelyikLehet` (-1 = any mission).
    pub spy_mission: i32,
    /// `TutorialDisableTextWindow`.
    pub text_window: bool,
    /// `TutorialTextShown` (the `t` key toggles it).
    pub text_shown: bool,
    /// `Tutorial_MainBolygoNeve` (event 0x1c).
    pub main_planet: Vec<u8>,
}

impl Default for Gates {
    fn default() -> Self {
        Gates {
            screens: [false; 20], buildings: [false; 0x3c], split: false, production: false, sell_ships: false,
            exit_button: false, starmap: false, colony: false, demolish: false, new_spies: false, spy_mission: -1,
            text_window: false, text_shown: true, main_planet: Vec::new(),
        }
    }
}

/// The text window (`TK*`): its position, the typing queue and the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    /// `TKActX/Y`: the top-left corner (640x480 frame).
    pub x: f32,
    pub y: f32,
    /// `TKTargetX/Y` (`AT`).
    pub target: (i32, i32),
    /// `TKXAdder/TKYAdder`.
    pub step: (f32, f32),
    /// `aktct2`: the window moves every 6th tick.
    pub move_div: i32,
    /// `TKQueue` (a fixed buffer: the word wrap reads past its length) and `TKQueueLen`.
    pub queue: Vec<u8>,
    pub queue_len: i32,
    /// `TKTomb`: `[row][col] = (char, colour)`; colour >= 0x40 blinks with colour - 0x40.
    pub grid: [[(u8, u8); COLS]; ROWS],
    /// `TKTombSorPtr`: the typing column on row 7.
    pub column: i8,
    /// `TKTCol`, `TKTVil`.
    pub colour: i32,
    pub blink: bool,
    /// `Tutorial_Szoveg_Counter`, `_Max`, `_Novelo`: one character each time the counter passes the max.
    pub typing: f32,
    pub typing_max: f32,
    pub typing_step: f32,
}

impl Default for Window {
    fn default() -> Self {
        Window {
            x: 0.0, y: 0.0, target: (0, 0), step: (0.0, 0.0), move_div: 0, queue: vec![0; QUEUE], queue_len: 0,
            grid: [[(SP, 1); COLS]; ROWS], column: 0, colour: 0, blink: false, typing: 0.0, typing_max: 0.0,
            typing_step: 0.0,
        }
    }
}

impl Window {
    fn q(&self, i: i32) -> u8 {
        self.queue.get(i.max(0) as usize).copied().unwrap_or(0)
    }
    fn push(&mut self, c: u8) {
        let i = self.queue_len.max(0) as usize;
        if i >= self.queue.len() {
            self.queue.resize(i + 1, 0);
        }
        self.queue[i] = c;
        self.queue_len += 1;
    }
    /// `SlideQueue` (exe 0x60bd70).
    fn slide(&mut self) {
        for i in 0..(self.queue_len - 1).max(0) as usize {
            self.queue[i] = self.queue[i + 1];
        }
        self.queue_len -= 1;
    }
    /// `TKTombSoremeles` (exe 0x60bda0): rows scroll up, row 7 blank.
    fn newline(&mut self) {
        self.column = 0;
        self.grid.copy_within(1..ROWS, 0);
        self.grid[ROWS - 1] = [(SP, 1); COLS];
    }
    /// `AT x,y`: the steps are `|target - act| / 40.0` in f64 from an f32 difference (exe 0x60c7d2..0x60c856).
    fn at(&mut self, x: i32, y: i32) {
        self.target = (x, y);
        self.step = (((x as f32 - self.x).abs() as f64 / 40.0) as f32, ((y as f32 - self.y).abs() as f64 / 40.0) as f32);
    }
    /// One move step of `TutorialFrissit` (f32, exe 0x60c0d2..0x60c186).
    fn glide(&mut self) {
        fn axis(act: &mut f32, target: i32, step: f32) {
            let t = target as f32;
            if *act < t {
                *act += step;
                if t < *act {
                    *act = t;
                }
            }
            if t < *act {
                *act -= step;
                if *act < t {
                    *act = t;
                }
            }
        }
        axis(&mut self.x, self.target.0, self.step.0);
        axis(&mut self.y, self.target.1, self.step.1);
    }
    /// The typing step after the command loop (spec §2 step 6).
    fn type_tick(&mut self) {
        self.typing += self.typing_step;
        if self.typing <= self.typing_max {
            return;
        }
        self.typing -= self.typing_max;
        while self.q(0) == CR && self.queue_len > 0 {
            self.slide();
            self.newline();
        }
        if self.queue_len > 0 {
            // The printable length of the next word: markup `#C3#` / `#V#` doesn't count.
            let (mut w, mut i) = (0, 0);
            while self.q(i) != SP && self.q(i) != CR && i < self.queue_len {
                let j = i + 1;
                if self.q(j) != HASH && (self.q(i) != HASH || self.q(i + 2) != HASH) {
                    w += 1;
                }
                i = j;
            }
            if COLS as i32 - (self.column as i32) < w {
                self.newline();
            }
            while self.q(0) == SP && self.column == 0 && self.queue_len > 0 {
                self.slide();
            }
            while self.q(0) == HASH && self.queue_len > 0 {
                let before = self.queue_len;
                if self.q(1) == b'C' {
                    if self.queue_len > 0 {
                        self.slide();
                    }
                    self.colour = get_int(&self.queue, 0, 0);
                    self.skip_markup();
                }
                if self.q(0) == HASH && self.q(1) == b'V' {
                    if self.queue_len > 0 {
                        self.slide();
                    }
                    self.skip_markup();
                    self.blink = true;
                }
                // `#` before another letter loops forever in the exe (no shipped text has one).
                if self.queue_len == before {
                    break;
                }
            }
            if self.queue_len > 0 {
                let c = self.column as usize;
                if c < COLS {
                    self.grid[ROWS - 1][c] = (self.q(0), (self.colour as u8).wrapping_add(if self.blink { 0x40 } else { 0 }));
                }
                self.slide();
                self.column = self.column.wrapping_add(1);
            }
        }
        if self.column == COLS as i8 {
            self.newline();
        }
    }
    fn skip_markup(&mut self) {
        while self.q(0) != HASH && self.queue_len > 0 {
            self.slide();
        }
        if self.q(0) == HASH && self.queue_len > 0 {
            self.slide();
        }
    }
    /// The characters still to type.
    pub fn pending(&self) -> &[u8] {
        &self.queue[..self.queue_len.max(0) as usize]
    }
}

/// `GetInt(p, n)` (exe 0x60bcc0): skip to the first run of `0-9 . -`, skip `n` runs, `atoi` the next one. The
/// exe reads on unbounded; the port stops at the end of `buf`.
pub fn get_int(buf: &[u8], from: usize, n: i32) -> i32 {
    let num = |c: u8| c.is_ascii_digit() || c == b'.' || c == b'-';
    let at = |i: usize| buf.get(i).copied().unwrap_or(0);
    let mut i = from;
    while i < buf.len() && !num(at(i)) {
        i += 1;
    }
    for _ in 0..n {
        while num(at(i)) {
            i += 1;
        }
        while i < buf.len() && !num(at(i)) {
            i += 1;
        }
    }
    let start = i;
    while num(at(i)) {
        i += 1;
    }
    atoi(&buf[start.min(buf.len())..i.min(buf.len())])
}

/// C `atoi` on a run of `0-9 . -`.
fn atoi(s: &[u8]) -> i32 {
    let (neg, digits) = match s.first() {
        Some(b'-') => (true, &s[1..]),
        _ => (false, s),
    };
    let v = digits.iter().take_while(|c| c.is_ascii_digit()).fold(0i32, |v, &c| v.wrapping_mul(10).wrapping_add((c - b'0') as i32));
    if neg { v.wrapping_neg() } else { v }
}

/// The interpreter's state (spec §1).
#[derive(Debug, Clone, PartialEq)]
pub struct Tutorial {
    /// `TutorialPuffer`.
    script: Vec<u8>,
    /// `TutorialActive`: the script runs and the recorded input drives the game.
    pub active: bool,
    /// `TutorialPaused`: waiting for an event; live input.
    pub paused: bool,
    /// `TutorialPufferPtr`.
    pub ptr: i32,
    /// `TutorialTargetTime` (`TIME`; never read).
    pub target_time: i32,
    /// `TutorialTargetCounter` (`COUNT`, against `counter - start_counter`).
    pub target_counter: i32,
    /// `TutorialStartCounter`.
    pub start_counter: i32,
    pub restarting: bool,
    /// `TutorialDefaultChapter` (`SEEKCHAPTER`).
    pub chapter: i32,
    /// `TutorialWaitForMouseMoveToEnd`, `…TextToEnd`, `…Atlepes`, `…Screen` + `TutorialMelyikreWait`,
    /// `…Battle`.
    pub wait_mouse: bool,
    pub wait_text: bool,
    pub wait_transition: bool,
    pub wait_screen: Option<i32>,
    pub wait_battle: bool,
    /// `TutorialEvent` (0 = none).
    pub event: i32,
    /// `TutorialVisszaSzamlalo` (UI ticks to event 0x22; not reset by start or stop).
    pub countdown: i32,
    /// `SpeechMiattKilepve`.
    pub speech_blocked: bool,
    /// `TutorialSzinVillogtato` (30..100) and its direction: the blinking cells' brightness.
    pub blink: i32,
    pub blink_up: bool,
    /// `Tutorial_LastMp2_Length`, `TutorialLastSzovegHossz`.
    pub speech_len: i32,
    pub text_len: i32,
    pub window: Window,
    pub gates: Gates,
    /// `TutorialActMouseAllapot` (-1 = no recording) and `…Max`.
    pub mouse: i32,
    pub mouse_max: i32,
    /// `TutorialCounter` (UI ticks, stops while paused) and `TutorialOldCounter`.
    pub ui_counter: i32,
    pub old_counter: i32,
    /// `TutorialKellOverrunVisszaAllitas`.
    pub overrun: bool,
    /// `TutorialInitEvents`' baselines (explored planets is never read).
    pub explored: i32,
    pub owned: i32,
    pub tanks: i32,
    /// `DemoMode_TexturaNev`.
    pub texture: Vec<u8>,
}

impl Default for Tutorial {
    fn default() -> Self {
        Tutorial {
            script: Vec::new(), active: false, paused: false, ptr: 0, target_time: 0, target_counter: 0,
            start_counter: 0, restarting: false, chapter: 0, wait_mouse: false, wait_text: false,
            wait_transition: false, wait_screen: None, wait_battle: false, event: 0, countdown: 0,
            speech_blocked: false, blink: 0, blink_up: true, speech_len: 0, text_len: 0, window: Window::default(),
            gates: Gates::default(), mouse: -1, mouse_max: 0, ui_counter: 0, old_counter: -1, overrun: false,
            explored: 0, owned: 0, tanks: 0, texture: Vec::new(),
        }
    }
}

/// How a quoted name treats a line break (spec §2).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Quoted {
    /// `PLAYMOUSE`, `SET MAIN BOLYGONEV`: CR LF is stepped over and the name starts again.
    Restart,
    /// `FIND*`, `SHOW PLANET`, `WAITFOREVENT`, `TEXTURE`: the exe loops forever.
    Hang,
    /// `PLAYWAV`: the bytes up to the quote, line breaks included.
    Raw,
}

impl Tutorial {
    /// `TutorialStart(name, chapter)` (exe 0x611f80) on the script's bytes, after `TutorialStop(true)`.
    pub fn start(&mut self, script: Vec<u8>, chapter: i32, host: &mut dyn TutorialHost) {
        self.stop(true, host);
        self.chapter = chapter;
        self.script = script;
        self.active = true;
        self.gates.text_window = false;
        self.ptr = 0;
        self.target_time = 0;
        self.target_counter = 0;
        self.restarting = false;
        self.blink = 0;
        self.blink_up = true;
        let w = &mut self.window;
        w.x = 398.0;
        w.y = 220.0;
        w.target = (398, 220);
        w.queue_len = 0;
        w.column = 0;
        w.step = (0.0, 0.0);
        w.typing = 0.0;
        w.typing_max = 100.0;
        w.typing_step = 15.0;
        w.grid = [[(SP, 1); COLS]; ROWS];
        self.old_counter = -1;
        self.mouse = -1;
        self.start_counter = host.counter();
        self.gates.text_shown = true;
        self.wait_transition = false;
        self.wait_screen = None;
        self.paused = false;
        self.gates.split = false;
        self.gates.production = false;
        self.gates.starmap = false;
        self.gates.colony = false;
        self.event = 0;
        self.gates.sell_ships = true;
        self.gates.exit_button = false;
        self.gates.screens = [false; 20];
        self.gates.buildings[1..].fill(false);
        self.wait_mouse = false;
        self.wait_text = false;
        self.gates.new_spies = false;
        self.wait_battle = false;
        self.overrun = false;
        self.speech_len = 0;
    }

    /// `TutorialStop(restore_music)` (exe 0x6124e0): only while active or paused.
    pub fn stop(&mut self, restore_music: bool, host: &mut dyn TutorialHost) {
        if !(self.active || self.paused) {
            return;
        }
        self.script.clear();
        self.active = false;
        let g = &mut self.gates;
        g.screens = [false; 20];
        g.buildings = [false; 0x3c];
        g.new_spies = false;
        g.spy_mission = -1;
        self.wait_battle = false;
        g.split = false;
        g.production = false;
        g.demolish = false;
        self.paused = false;
        g.exit_button = false;
        g.starmap = false;
        g.colony = false;
        g.sell_ships = false;
        g.main_planet.clear();
        host.effect(Effect::Stopped { restore_overrun: self.overrun, restore_music });
        self.overrun = false;
    }

    /// The `.mou` sample the recorded input is at, if one plays (`Mouse_Tutorial_Kezel`); `None` parks the
    /// pointer at (320, 200).
    pub fn mouse_sample(&self) -> Option<usize> {
        (self.active && self.mouse != -1 && self.mouse != self.mouse_max).then_some(self.mouse as usize)
    }

    /// `Mouse_Tutorial_Kezel`'s replay step (exe 0x510930), before `TutorialFrissit` each UI tick.
    pub fn mouse_tick(&mut self, screen: i32) {
        if self.active && screen != SCREEN_OPTIONS && self.old_counter != self.ui_counter {
            if self.mouse < self.mouse_max && self.mouse != -1 {
                self.mouse += 1;
            }
            self.old_counter = self.ui_counter;
        }
    }

    /// One UI tick of `Fake_FRISSITES`' tutorial part: the replay step, `TutorialFrissit`, `TutorialCounter++`
    /// unless paused. (The screens' own updates, which fire events, run between the first two.)
    pub fn ui_tick(&mut self, host: &mut dyn TutorialHost) {
        self.mouse_tick(host.screen());
        self.frissit(host);
        self.count_tick();
    }

    /// `if (!TutorialPaused) TutorialCounter++`.
    pub fn count_tick(&mut self) {
        if !self.paused {
            self.ui_counter = self.ui_counter.wrapping_add(1);
        }
    }

    /// `TutorialFrissit` (exe 0x60c020).
    pub fn frissit(&mut self, host: &mut dyn TutorialHost) {
        if host.screen() == SCREEN_OPTIONS && !host.self_playing() {
            return;
        }
        if self.blink_up {
            self.blink += 1;
            if 99 < self.blink {
                self.blink_up = false;
                self.blink = 100;
            }
        } else {
            self.blink -= 1;
            if self.blink < 0x1f {
                self.blink_up = true;
                self.blink = 0x1e;
            }
        }
        if 0 < self.countdown {
            self.countdown -= 1;
            if self.countdown == 0 {
                self.check_event(EVENT_COUNTDOWN);
            }
        }
        if self.paused || self.active {
            self.window.move_div += 1;
            if 5 < self.window.move_div {
                self.window.move_div = 0;
                self.window.glide();
            }
        }
        if !self.active {
            if self.paused {
                self.check_events(host);
            }
            return;
        }
        self.speech_blocked = false;
        while self.may_run(host) {
            let n = self.next_command(self.ptr, host);
            self.ptr = self.ptr.wrapping_add(n);
            if self.restarting {
                self.restarting = false;
                self.ptr = 0;
            }
        }
        self.window.type_tick();
    }

    /// The command loop's condition (exe 0x60c1b5..0x60c2ab): each wait check clears its flag when it passes.
    fn may_run(&mut self, host: &dyn TutorialHost) -> bool {
        if !(self.ptr < self.script.len() as i32
            && self.target_counter <= host.counter().wrapping_sub(self.start_counter)
            && self.active)
        {
            return false;
        }
        if self.wait_mouse && self.mouse < self.mouse_max && self.mouse != -1 {
            return false;
        }
        self.wait_mouse = false;
        if self.wait_transition && host.transition() {
            return false;
        }
        self.wait_transition = false;
        if self.wait_screen.is_some_and(|s| s != host.screen()) {
            return false;
        }
        self.wait_screen = None;
        if self.wait_text && self.window.queue_len >= 1 {
            return false;
        }
        self.wait_text = false;
        !self.wait_battle && !self.speech_blocked
    }

    /// `TutorialCheckEvent(id)` (exe 0x60bdf0): the awaited event resumes the script (`TutorialUnPause`; the
    /// host clears the keyboard and the starmap/colony input state).
    pub fn check_event(&mut self, id: i32) {
        if self.event == id {
            self.event = 0;
            if self.paused {
                self.paused = false;
                self.active = true;
            }
        }
    }

    /// `TutorialCheckEvents` (exe 0x60be40), every tick while paused.
    pub fn check_events(&mut self, host: &dyn TutorialHost) {
        if !self.paused {
            return;
        }
        if self.event == 2 && self.owned < host.owned_planets() {
            self.check_event(2);
        }
        let assault = host.assault_tanks();
        if self.event == 0x19 && self.tanks + 3 <= host.my_tanks() {
            self.check_event(0x19);
        }
        if 3 < assault {
            self.check_event(0x1e);
        }
    }

    /// `StartGrWar` / `SpaceWar_Belep`: a battle began (`WAITFORBATTLE`).
    pub fn battle_started(&mut self) {
        self.wait_battle = false;
    }

    /// The script byte at `i` (0 past the end, like the exe's malloc slack).
    fn b(&self, i: i32) -> u8 {
        self.script.get(i.max(0) as usize).copied().unwrap_or(0)
    }
    fn len(&self) -> i32 {
        self.script.len() as i32
    }
    fn matches(&self, at: i32, kw: &str) -> bool {
        let at = at.max(0) as usize;
        self.script.get(at..at + kw.len()).is_some_and(|s| s.eq_ignore_ascii_case(kw.as_bytes()))
    }
    /// Skip from `base + i` to the next `c` (or the end); returns the new `i`.
    fn skip_to(&self, base: i32, mut i: i32, c: u8) -> i32 {
        while base + i < self.len() && self.b(base + i) != c {
            i += 1;
        }
        i
    }
    /// The quoted name after `base + i` and the new `i` (on the closing quote); `None` = the exe hangs.
    fn quoted(&self, base: i32, i: i32, mode: Quoted) -> (Option<Vec<u8>>, i32) {
        let mut i = self.skip_to(base, i, b'"');
        let mut name = Vec::new();
        while base + i + 1 < self.len() && self.b(base + i + 1) != b'"' {
            if mode == Quoted::Raw {
                name.push(self.b(base + i + 1));
                i += 1;
                continue;
            }
            name.clear();
            while base + i + 1 < self.len() && self.b(base + i + 1) != b'"' && self.b(base + i + 1) != CR {
                name.push(self.b(base + i + 1));
                i += 1;
            }
            if self.b(base + i + 1) == CR {
                if mode == Quoted::Hang {
                    return (None, i);
                }
                i += 2;
            }
        }
        (Some(name), i)
    }
    fn int(&self, at: i32, n: i32) -> i32 {
        get_int(&self.script, at.max(0) as usize, n)
    }

    /// `TutorialGetNextCommand(p)` (exe 0x60c570): run the next command after `p`; returns the offset of its
    /// closing `>` from `p` (0 = nothing consumed).
    pub fn next_command(&mut self, p: i32, host: &mut dyn TutorialHost) -> i32 {
        let comment = |t: &Self, i: i32| t.b(p + i) == b'/' && t.b(p + i + 1) == b'*';
        let mut i = 0;
        while p + i < self.len() && self.b(p + i) != b'<' && !comment(self, i) {
            i += 1;
        }
        if p + i == self.len() {
            self.stop(true, host);
            return 0;
        }
        loop {
            if !comment(self, i) {
                i += 1;
                return self.command(p, i, host);
            }
            while p + i < self.len() && !(self.b(p + i) == b'*' && self.b(p + i + 1) == b'/') {
                i += 1;
            }
            if p + i == self.len() {
                self.stop(true, host);
                return 0;
            }
            while p + i < self.len() && self.b(p + i) != b'<' && !comment(self, i) {
                i += 1;
            }
            if p + i == self.len() {
                self.stop(true, host);
                return 0;
            }
        }
    }

    /// The 74 keyword tests from `p + i` (the byte after `<`), in `ig2_formats::tutorial::COMMANDS` order.
    fn command(&mut self, p: i32, i: i32, host: &mut dyn TutorialHost) -> i32 {
        let at = p + i;
        let kw = ig2_formats::tutorial::COMMANDS.iter().copied().find(|k| self.matches(at, k));
        let Some(kw) = kw else {
            self.stop(true, host);
            return i - 1;
        };
        let end = |t: &Self, i: i32| t.skip_to(p, i, b'>');
        let int = |t: &Self, n: i32| t.int(at, n);
        match kw {
            "AT" => {
                let (x, y) = (int(self, 0), int(self, 1));
                let i = end(self, i);
                self.window.at(x, y);
                return i;
            }
            "END" => {
                self.stop(true, host);
                host.effect(Effect::End);
                return i;
            }
            "RETURNTOMAINMENU" => {
                self.stop(true, host);
                host.effect(Effect::ReturnToMainMenu);
                return i;
            }
            "SEEKCHAPTER" => {
                let label = format!("Chapter {}", self.chapter);
                let mut i = i;
                while p + i < self.len() && !self.script[(p + i) as usize..].starts_with(label.as_bytes()) {
                    i += 1;
                }
                return end(self, i);
            }
            "QUIT" => {
                self.stop(true, host);
                host.effect(Effect::Quit);
                return i;
            }
            "KURVAANYAD" => host.effect(Effect::PlanetInfo),
            "TYPE" => return self.type_text(p, i),
            "TIME" => {
                let (a, b) = (int(self, 0), int(self, 1));
                let i = end(self, i);
                self.target_time = a.wrapping_mul(60).wrapping_add(b);
                return i;
            }
            "PLAYWAV" => {
                if host.speech_playing() {
                    self.speech_blocked = true;
                    return 0;
                }
                let (name, i) = self.quoted(p, i, Quoted::Raw);
                self.speech_len = host.start_speech(&name.unwrap_or_default());
                return end(self, i);
            }
            "COUNT" => self.target_counter = int(self, 0),
            "GIVE MONEY" => host.effect(Effect::GiveMoney(int(self, 0))),
            "DISABLE HAJOELADAS" => self.gates.sell_ships = true,
            "DISABLE OVERRUN" => {
                self.overrun = true;
                host.effect(Effect::DisableOverrun);
            }
            "DISABLE STARMAP" => self.gates.starmap = true,
            "ENABLE STARMAP" => self.gates.starmap = false,
            "DOUBLEGOMB ON" => host.effect(Effect::DoubleButton),
            "DOUBLEGOMB OFF" => {}
            "DISABLE COLONY" => self.gates.colony = true,
            "ENABLE COLONY" => self.gates.colony = false,
            "DISABLE EXITGOMB" => self.gates.exit_button = true,
            "ENABLE EXITGOMB" => self.gates.exit_button = false,
            "DISABLE AI" => host.effect(Effect::AiDisabled(true)),
            "ENABLE AI" => host.effect(Effect::AiDisabled(false)),
            "DISABLE ROMBOLAS" => self.gates.demolish = true,
            "ENABLE ROMBOLAS" => self.gates.demolish = false,
            "DISABLE SPLITFLEET" => self.gates.split = true,
            "ENABLE SPLITFLEET" => self.gates.split = false,
            "DISABLE PRODUCTION" => self.gates.production = true,
            "ENABLE PRODUCTION" => self.gates.production = false,
            "DISABLE SCREEN" | "ENABLE SCREEN" => {
                let n = int(self, 0).max(0) as usize;
                if let Some(s) = self.gates.screens.get_mut(n) {
                    *s = kw == "DISABLE SCREEN";
                }
            }
            "DISABLE ALL SCREENS" => {
                for (k, s) in self.gates.screens.iter_mut().enumerate() {
                    if k != 13 {
                        *s = true;
                    }
                }
            }
            "CENTERFLEET" => host.effect(Effect::CenterFleet),
            "FIND FLEET" | "FIND PLANET" | "SHOW PLANET" | "TEXTURE" | "WAITFOREVENT" => {
                let (name, i) = self.quoted(p, i, Quoted::Hang);
                let Some(name) = name else {
                    self.stop(true, host);
                    host.effect(Effect::Hang);
                    return i;
                };
                match kw {
                    "FIND FLEET" => host.effect(Effect::FindFleet(name)),
                    "FIND PLANET" => host.effect(Effect::FindPlanet(name)),
                    "SHOW PLANET" => host.effect(Effect::ShowPlanet(name)),
                    "TEXTURE" => self.texture = name,
                    _ => self.wait_for_event(&name, host),
                }
                return end(self, i);
            }
            "CENTERPLANET" => host.effect(Effect::CenterPlanet),
            "CENTERUNCOLONISEDPLANET" => host.effect(Effect::CenterUncolonisedPlanet),
            "ENABLE INVENTION" => host.effect(Effect::Invention { tech: int(self, 0), state: 5 }),
            "DISABLE INVENTION" => host.effect(Effect::Invention { tech: int(self, 0), state: 1 }),
            "ENABLE BUILDING" | "DISABLE BUILDING" => {
                let n = int(self, 0).max(0) as usize;
                if let Some(b) = self.gates.buildings.get_mut(n) {
                    *b = kw == "DISABLE BUILDING";
                }
            }
            "DISABLE ALL BUILDINGS" => self.gates.buildings = [true; 0x3c],
            "ENABLE ALL BUILDINGS" => self.gates.buildings = [false; 0x3c],
            "DISABLE AUTOBUILD" => host.effect(Effect::DisableAutobuild),
            "DISABLE TRADERS" => host.effect(Effect::Traders { enabled: false }),
            "ENABLE TRADERS" => host.effect(Effect::Traders { enabled: true }),
            "PLAYMOUSE" => {
                let (name, i) = self.quoted(p, i, Quoted::Restart);
                // `TutorialPlayMouse`: a new recording replaces the running one.
                self.mouse_max = host.mouse_samples(&name.unwrap_or_default());
                self.mouse = 0;
                return end(self, i);
            }
            "SET MAIN BOLYGONEV" => {
                let (name, i) = self.quoted(p, i, Quoted::Restart);
                self.gates.main_planet = name.unwrap_or_default();
                return end(self, i);
            }
            "WAITFORMOUSE" => self.wait_mouse = true,
            "WAITFORSCREEN" => self.wait_screen = Some(int(self, 0)),
            "WAITFORBATTLE" => {
                self.wait_battle = true;
                host.effect(Effect::BattleVehicles);
            }
            "WAITFORTEXT" => self.wait_text = true,
            "LOADSCENARIO" | "CHAPTER" => {}
            "LOADSAVEGAME" => host.effect(Effect::LoadSaveGame(int(self, 0))),
            "GAMESPEED" => host.effect(Effect::GameSpeed(int(self, 0))),
            "WAITFORATLEPES" => self.wait_transition = true,
            "DIPLOMACY EXPLORE EMPIRE" => host.effect(Effect::ExploreEmpire { empire: int(self, 0), diplomacy: true }),
            "EXPLORE EMPIRE" => host.effect(Effect::ExploreEmpire { empire: int(self, 0), diplomacy: false }),
            "GIVE KILLER FLEET" => host.effect(Effect::KillerFleet(int(self, 0))),
            "GIVE KOLONIZALO FLEET" => host.effect(Effect::ColonizationFleet),
            "PAUSE" => host.effect(Effect::Pause(true)),
            "UNPAUSE" => host.effect(Effect::Pause(false)),
            "IRRITATEME" => host.effect(Effect::Irritate),
            "SPY MISSION" => self.gates.spy_mission = int(self, 0),
            "ADD DESTROYERS TO TRADE" => host.effect(Effect::DestroyersToTrade),
            "DISABLE NEW SPIES" => self.gates.new_spies = true,
            "DISABLE TEXTWINDOW" => self.gates.text_window = true,
            "GOTO SCREEN" => host.effect(Effect::GotoScreen(int(self, 0))),
            "SEND MESSAGE" => host.effect(Effect::SendMessage(int(self, 0))),
            "RESTART" => self.restarting = true,
            _ => unreachable!("{kw}"),
        }
        end(self, i)
    }

    /// `TYPE "…"`: the text goes to the typing queue after two line breaks; CR LF and LF become spaces.
    fn type_text(&mut self, p: i32, i: i32) -> i32 {
        let mut i = self.skip_to(p, i, b'"');
        self.window.push(CR);
        self.window.push(CR);
        self.text_len = 0;
        while p + i + 1 < self.len() && self.b(p + i + 1) != b'"' {
            while self.b(p + i + 1) == CR {
                self.window.push(SP);
                self.text_len += 1;
                i += 2;
            }
            let c = self.b(p + i + 1);
            self.window.push(if c == b'\n' { SP } else { c });
            self.text_len += 1;
            i += 1;
        }
        // f32 throughout (exe 0x60cdb2..0x60cdd4): divide, then 50, then 160.
        self.window.typing_step = if self.speech_len == 0 { 15.0 } else { self.text_len as f32 / self.speech_len as f32 * 50.0 * 160.0 };
        let i = self.skip_to(p, i, b'>');
        self.window.colour = 1;
        self.window.blink = false;
        self.window.typing = 0.0;
        i
    }

    /// `WAITFOREVENT "name"`: the event, the planets' mark, `TutorialPause`, `TutorialInitEvents`.
    fn wait_for_event(&mut self, name: &[u8], host: &mut dyn TutorialHost) {
        if let Some(&(_, id)) = EVENTS.iter().find(|(n, _)| n.as_bytes().eq_ignore_ascii_case(name)) {
            self.event = id;
            match id {
                9 => host.effect(Effect::Invention { tech: 0x12, state: 3 }),
                EVENT_COUNTDOWN => self.countdown = COUNTDOWN,
                _ => {}
            }
        }
        host.effect(Effect::MarkOwnedPlanets);
        self.gates.text_shown = true;
        if self.active {
            self.paused = true;
            self.active = false;
        }
        self.explored = host.explored_planets();
        self.owned = host.owned_planets();
        self.tanks = host.my_tanks();
    }

    /// The script (empty once stopped).
    pub fn script(&self) -> &[u8] {
        &self.script
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Host {
        screen: i32,
        counter: i32,
        effects: Vec<Effect>,
        samples: i32,
    }
    impl TutorialHost for Host {
        fn screen(&self) -> i32 {
            self.screen
        }
        fn counter(&self) -> i32 {
            self.counter
        }
        fn mouse_samples(&mut self, _name: &[u8]) -> i32 {
            self.samples
        }
        fn effect(&mut self, e: Effect) {
            self.effects.push(e);
        }
    }

    fn run(script: &str, chapter: i32) -> (Tutorial, Host) {
        let mut h = Host { screen: 2, samples: 3, ..Host::default() };
        let mut t = Tutorial::default();
        t.start(script.as_bytes().to_vec(), chapter, &mut h);
        (t, h)
    }

    #[test]
    fn get_int_reads_runs_of_digits_dots_and_minus() {
        assert_eq!(get_int(b"AT 395,220>", 0, 0), 395);
        assert_eq!(get_int(b"AT 395,220>", 0, 1), 220);
        assert_eq!(get_int(b"X -5 1.5", 0, 0), -5);
        assert_eq!(get_int(b"X -5 1.5", 0, 1), 1);
        assert_eq!(get_int(b"nothing", 0, 0), 0);
    }

    #[test]
    fn seekchapter_runs_the_chapter_and_waits() {
        let s = "; note\r\n<SEEKCHAPTER>\r\n<CHAPTER \"Chapter 1\">\r\n<GIVE MONEY 300000>\r\n<AT 5,80>\r\n\
                 <TYPE \"Hello\r\nworld\">\r\n<WAITFORTEXT>\r\n<RETURNTOMAINMENU>\r\n<CHAPTER \"Chapter 2\">\r\n<GIVE MONEY 7>\r\n";
        let (mut t, mut h) = run(s, 1);
        t.frissit(&mut h);
        assert_eq!(h.effects, vec![Effect::GiveMoney(300000)]);
        assert!(t.wait_text && t.active);
        assert_eq!(t.window.target, (5, 80));
        assert_eq!(t.window.step, ((393.0f32 / 40.0f64 as f32), 140.0 / 40.0));
        assert_eq!(t.window.pending(), b"\r\rHello world");
        // 15/100 of a character per tick: the 13 queued bytes take 2 rows' newlines + 11 letters.
        let mut ticks = 1;
        while t.wait_text || t.active {
            t.frissit(&mut h);
            ticks += 1;
            assert!(ticks < 2000);
        }
        assert_eq!(&t.window.grid[ROWS - 1][..11].iter().map(|c| c.0).collect::<Vec<_>>(), b"Hello world");
        let n = h.effects.len();
        assert_eq!(h.effects[n - 2..], [Effect::Stopped { restore_overrun: false, restore_music: true }, Effect::ReturnToMainMenu]);
    }

    #[test]
    fn waitforevent_pauses_until_the_event() {
        let (mut t, mut h) = run("<WAITFOREVENT \"movefleet\"><GIVE MONEY 1>", 0);
        t.frissit(&mut h);
        assert!(t.paused && !t.active && t.event == 0x15);
        t.check_event(0x14);
        assert!(t.paused);
        t.check_event(0x15);
        t.frissit(&mut h);
        assert!(h.effects.contains(&Effect::GiveMoney(1)));
    }

    #[test]
    fn countdown_fires_event_0x22() {
        let (mut t, mut h) = run("<WAITFOREVENT \"15masodperc\"><GIVE MONEY 1>", 0);
        t.frissit(&mut h);
        assert_eq!(t.countdown, COUNTDOWN);
        for _ in 0..COUNTDOWN - 1 {
            t.frissit(&mut h);
        }
        assert!(t.paused && !h.effects.contains(&Effect::GiveMoney(1)));
        // The tick the countdown runs out resumes the script at once (which then ends).
        t.frissit(&mut h);
        assert!(!t.paused && h.effects.contains(&Effect::GiveMoney(1)));
    }

    #[test]
    fn playmouse_and_waitformouse() {
        let (mut t, mut h) = run("<PLAYMOUSE \"a.mou\"><WAITFORMOUSE><GIVE MONEY 1>", 0);
        t.ui_tick(&mut h);
        assert_eq!((t.mouse, t.mouse_max), (0, 3));
        for _ in 0..3 {
            t.ui_tick(&mut h);
        }
        assert_eq!(t.mouse, 3);
        assert!(h.effects.contains(&Effect::GiveMoney(1)));
    }

    #[test]
    fn unknown_tag_and_prefix_matching() {
        let (mut t, mut h) = run("<WAITFORMOUSEMOVE><BOGUS>", 0);
        t.frissit(&mut h);
        assert!(!t.active);
        assert!(matches!(h.effects[0], Effect::Stopped { .. }));
    }

    #[test]
    fn comments_are_skipped() {
        let (mut t, mut h) = run("/* <GIVE MONEY 5> */ <GIVE MONEY 6>", 0);
        t.frissit(&mut h);
        assert_eq!(h.effects[0], Effect::GiveMoney(6));
    }
}
