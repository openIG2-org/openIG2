//! Ours (ADR 0080): Discord Rich Presence. The player's Discord profile shows what they do in the game (the menu
//! page, the multiplayer room, the game's mode, race, screen, date and empire numbers), and a multiplayer host's
//! room carries a Join button: a friend's press starts (or brings up) their game and joins the room through its join
//! code. Off, status only, or status and join (the default) on the Options' Game tab (`DiscordPresence`).
//!
//! [`Doing`] is what the window reports ([`crate::window`]'s `discord_doing`), [`activity`] the Discord activity it
//! shows (pure, tested), [`Presence`] the driver: it starts the IPC worker ([`ipc`]), sends the activity when it
//! changed (Discord's rate limit is the worker's) and hands the window the joins and join requests. The browser build
//! has no Discord pipe: [`Presence`] does nothing there.
//!
//! The pictures are the application's art assets on Discord's side (uploaded in the Developer Portal under the
//! keys of [`ASSETS`]); none of the game's own pictures are sent.

use ig2_formats::text::Lang;
use serde_json::{json, Value};

#[cfg(not(target_arch = "wasm32"))]
pub mod ipc;
#[cfg(not(target_arch = "wasm32"))]
pub mod register;

/// The openIG2 application on Discord (its Developer Portal entry: name, art assets).
pub const APP_ID: &str = "1567637511340036228";

/// The option's key in our settings file: 0 off, 1 the status only, 2 the status and the Join button (the default).
pub const KEY: &str = "DiscordPresence";

/// The art asset keys the activity names: the logo, the races' emblems (`race_1` to `race_8`) and the screens' and
/// pages' small icons.
#[cfg_attr(not(test), allow(dead_code))]
pub const ASSETS: &[&str] = &[
    "logo", "race_1", "race_2", "race_3", "race_4", "race_5", "race_6", "race_7", "race_8", "menu", "lobby", "starmap", "colony", "ground_war",
    "space_battle", "research", "design", "diplomacy", "spy", "trade", "info", "messages", "options", "replay", "paused", "video",
];

/// The project's pages, the buttons of an activity without a Join.
const SITE: &str = "https://openig2-org.github.io/openIG2/";
const RELEASES: &str = "https://github.com/openIG2-org/openIG2/releases/latest";

/// Discord's limit for each text (characters).
const TEXT_MAX: usize = 128;

/// The setting's three values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    Off,
    Status,
    Join,
}

impl Setting {
    pub fn read(f: &ig2_formats::options::Options) -> Self {
        match f.int(KEY) {
            Some(0) => Setting::Off,
            Some(1) => Setting::Status,
            _ => Setting::Join,
        }
    }
}

/// A main menu page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Main,
    NewGame,
    Scenarios,
    /// The multiplayer connection page (no room yet).
    Multiplayer,
    History,
    Replays,
    Mods,
    Settings,
    Video,
}

/// A multiplayer room before its start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Room {
    pub name: String,
    pub host: bool,
    /// The humans in the room and the seats humans may take (theirs and the open ones).
    pub humans: u32,
    pub seats: u32,
    /// This member's race, if it holds a seat.
    pub race: Option<u8>,
    pub ready: bool,
    pub private: bool,
    /// A room resuming a saved game.
    pub saved: bool,
    /// The party id every member's activity shares.
    pub party: String,
    /// The host's join code (the Join button's secret).
    pub code: Option<String>,
}

/// A game's mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Campaign,
    Scenario,
    Single,
    Tutorial,
    Multiplayer,
}

/// The game screen up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Starmap,
    Colony,
    GroundWar,
    SpaceBattle,
    Research,
    Design,
    Diplomacy,
    Spy,
    Trade,
    Info,
    Messages,
    Options,
    Help,
    Video,
}

/// The local empire's numbers (`ig2_sim::stats::Row`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Numbers {
    pub planets: i32,
    pub population: i32,
    pub credits: i64,
    pub ships: i32,
    pub techs: i32,
    pub battles: i32,
    pub score: i32,
}

/// A game running (or a replay watched).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Play {
    pub mode: Mode,
    pub replay: bool,
    pub race: u8,
    /// `Nehezseg`: 1 easy, 2 normal, 4 hard (0: none shown).
    pub difficulty: i32,
    /// The game's date (year, month, day).
    pub date: (i32, i32, i32),
    pub screen: Screen,
    pub paused: bool,
    /// The empire is dead and the player watches (ADR 0026).
    pub spectating: bool,
    pub numbers: Numbers,
    /// Multiplayer: the humans in the game and the party id.
    pub players: u32,
    pub party: Option<String>,
}

/// What the player is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doing {
    Menu(Page),
    Room(Room),
    Game(Play),
}

impl Doing {
    /// What restarts the elapsed time: another kind of thing (the menu, a room, a game of a mode, a replay).
    fn phase(&self) -> Phase {
        match self {
            Doing::Menu(_) => (0, None, false),
            Doing::Room(_) => (1, None, false),
            Doing::Game(p) => (2, Some(p.mode), p.replay),
        }
    }
}

/// What restarts the activity's elapsed time ([`Doing::phase`]).
type Phase = (u8, Option<Mode>, bool);

/// A short stable id from `parts` (FNV-1a), for a party every member's game derives alike.
pub fn party_id(parts: &[&str]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.bytes().chain([0]) {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    format!("ig2-{h:016x}")
}

/// `n` with thin thousands separators (`12,345`), and in thousands or millions when large.
fn number(n: i64) -> String {
    let a = n.unsigned_abs();
    let s = if a >= 1_000_000 {
        format!("{:.1}M", a as f64 / 1e6)
    } else if a >= 100_000 {
        format!("{}k", a / 1000)
    } else {
        let d = a.to_string();
        let mut s = String::new();
        for (i, c) in d.chars().enumerate() {
            if i > 0 && (d.len() - i) % 3 == 0 {
                s.push(',');
            }
            s.push(c);
        }
        s
    };
    if n < 0 { format!("-{s}") } else { s }
}

/// `text` cut to Discord's limit, and at least two characters (Discord refuses shorter).
fn fit(text: &str) -> String {
    let mut s: String = text.trim().chars().take(TEXT_MAX).collect();
    while s.chars().count() < 2 {
        s.push(' ');
    }
    s
}

/// The activity's texts and keys, before its JSON.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Shown {
    pub details: String,
    pub state: String,
    pub large: (&'static str, String),
    pub small: Option<(&'static str, String)>,
    /// The party's id and size (now, at most).
    pub party: Option<(String, u32, u32)>,
    pub join: Option<String>,
}

/// What [`activity`] reads besides [`Doing`]: the race names (by race number, the game's), the language, the
/// version, the setting.
pub struct Context<'a> {
    pub races: &'a [String],
    pub lang: Lang,
    pub version: &'a str,
    pub setting: Setting,
}

impl Context<'_> {
    fn t(&self, en: &'static str) -> &str {
        crate::tr::text(self.lang, en)
    }

    /// `en` (a template with `{}`) in the language with `n` in it.
    fn n(&self, en: &'static str, n: impl std::fmt::Display) -> String {
        self.t(en).replacen("{}", &n.to_string(), 1)
    }

    fn race(&self, r: u8) -> String {
        self.races.get(r as usize).filter(|s| !s.trim().is_empty()).cloned().unwrap_or_else(|| self.n("Race {}", r))
    }

    fn logo(&self) -> (&'static str, String) {
        ("logo", format!("openIG2 {}", self.version))
    }
}

/// The emblem key of race `r` (the logo for a mod's race).
fn race_key(r: u8) -> &'static str {
    ["logo", "race_1", "race_2", "race_3", "race_4", "race_5", "race_6", "race_7", "race_8"].get(r as usize).copied().unwrap_or("logo")
}

fn screen_key(s: Screen) -> &'static str {
    match s {
        Screen::Starmap => "starmap",
        Screen::Colony => "colony",
        Screen::GroundWar => "ground_war",
        Screen::SpaceBattle => "space_battle",
        Screen::Research => "research",
        Screen::Design => "design",
        Screen::Diplomacy => "diplomacy",
        Screen::Spy => "spy",
        Screen::Trade => "trade",
        Screen::Info => "info",
        Screen::Messages => "messages",
        Screen::Options | Screen::Help => "options",
        Screen::Video => "video",
    }
}

fn screen_text(s: Screen) -> &'static str {
    match s {
        Screen::Starmap => "On the starmap",
        Screen::Colony => "Building a colony",
        Screen::GroundWar => "Fighting a ground war",
        Screen::SpaceBattle => "In a space battle",
        Screen::Research => "Researching",
        Screen::Design => "Designing ships",
        Screen::Diplomacy => "Negotiating",
        Screen::Spy => "Planning spy missions",
        Screen::Trade => "Trading",
        Screen::Info => "Studying the empires",
        Screen::Messages => "Reading messages",
        Screen::Options => "In the settings",
        Screen::Help => "Reading the help",
        Screen::Video => "Watching a story video",
    }
}

fn page_text(p: Page) -> &'static str {
    match p {
        Page::Main => "In the main menu",
        Page::NewGame => "Choosing an empire",
        Page::Scenarios => "Choosing a scenario",
        Page::Multiplayer => "Looking for a multiplayer game",
        Page::History => "Looking at the match history",
        Page::Replays => "Browsing replays",
        Page::Mods => "Managing mods",
        Page::Settings => "In the settings",
        Page::Video => "Watching a video",
    }
}

fn mode_text(m: Mode) -> &'static str {
    match m {
        Mode::Campaign => "Campaign",
        Mode::Scenario => "Scenario",
        Mode::Single => "Single player",
        Mode::Tutorial => "Tutorial",
        Mode::Multiplayer => "Multiplayer",
    }
}

fn difficulty_text(d: i32) -> Option<&'static str> {
    match d {
        1 => Some("Easy"),
        2 => Some("Normal"),
        4 => Some("Hard"),
        _ => None,
    }
}

/// What the activity shows for `d`.
pub fn shown(d: &Doing, c: &Context) -> Shown {
    match d {
        Doing::Menu(p) => Shown { details: c.t(page_text(*p)).into(), state: c.t("Imperium Galactica II").into(), large: c.logo(), small: Some(("menu", c.t("Main menu").into())), ..Default::default() },
        Doing::Room(r) => {
            let details = if r.saved { c.n("Resuming a saved game: {}", &r.name) } else { c.n("Multiplayer room: {}", &r.name) };
            let state = match (r.host, r.ready) {
                (true, _) if r.humans < r.seats => c.t("Waiting for players").to_string(),
                (true, _) => c.t("Ready to start").into(),
                (false, true) => c.t("Ready").into(),
                (false, false) => c.t("Getting ready").into(),
            };
            let small = r.race.map(|race| (race_key(race), c.n("Playing as {}", c.race(race))));
            let join = (c.setting == Setting::Join && r.host && !r.private && r.humans < r.seats).then(|| r.code.clone()).flatten();
            Shown { details, state, large: c.logo(), small: small.or(Some(("lobby", c.t("Multiplayer room").into()))), party: Some((r.party.clone(), r.humans.max(1), r.seats.max(r.humans).max(1))), join }
        }
        Doing::Game(p) => {
            let mut details = format!("{} · {}", c.t(mode_text(p.mode)), c.race(p.race));
            if let Some(d) = difficulty_text(p.difficulty).filter(|_| p.mode != Mode::Multiplayer && p.mode != Mode::Tutorial) {
                details = format!("{details} · {}", c.t(d));
            }
            if p.replay {
                details = format!("{}: {details}", c.t("Watching a replay"));
            }
            let doing = match () {
                _ if p.spectating => c.t("Watching the galaxy").to_string(),
                _ if p.paused && p.screen == Screen::Starmap => c.t("Paused").into(),
                _ => c.t(screen_text(p.screen)).into(),
            };
            let (y, m, day) = p.date;
            let state = format!("{doing} · {y}.{m:02}.{day:02}");
            let n = &p.numbers;
            let mut numbers = vec![c.n("{} planets", n.planets), c.n("{} ships", n.ships), c.n("{} credits", number(n.credits))];
            if n.population > 0 {
                numbers.insert(1, c.n("{} population", number(n.population as i64)));
            }
            if n.score > 0 {
                numbers.push(c.n("score {}", number(n.score as i64)));
            }
            let large = (race_key(p.race), numbers.join(" · "));
            let small_key = if p.replay { "replay" } else if p.paused && p.screen == Screen::Starmap { "paused" } else { screen_key(p.screen) };
            let mut small_text = vec![c.n("{} techs", n.techs), c.n("{} battles won", n.battles)];
            if p.mode == Mode::Multiplayer && p.players > 0 {
                small_text.insert(0, c.n("{} players", p.players));
            }
            let party = p.party.clone().filter(|_| p.mode == Mode::Multiplayer && !p.replay).map(|id| (id, p.players.max(1), p.players.max(1)));
            Shown { details, state, large, small: Some((small_key, small_text.join(" · "))), party, join: None }
        }
    }
}

/// The Discord activity of `s`, its elapsed time from `since` (Unix seconds).
pub fn activity(s: &Shown, since: u64, c: &Context) -> Value {
    let mut assets = json!({ "large_image": s.large.0, "large_text": fit(&s.large.1) });
    if let Some((k, t)) = &s.small {
        assets["small_image"] = json!(k);
        assets["small_text"] = json!(fit(t));
    }
    let mut a = json!({ "details": fit(&s.details), "state": fit(&s.state), "timestamps": { "start": since }, "assets": assets, "instance": false });
    if let Some((id, now, max)) = &s.party {
        a["party"] = json!({ "id": id, "size": [now, max] });
    }
    // Discord shows either a Join (the secret) or link buttons.
    match &s.join {
        Some(code) => a["secrets"] = json!({ "join": code }),
        None => a["buttons"] = json!([{ "label": fit(c.t("Get openIG2")), "url": RELEASES }, { "label": fit(c.t("Website")), "url": SITE }]),
    }
    a
}

/// What the window does with Discord's events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// Join the room of this join code.
    Join(String),
    /// A friend asks to join: accept (the room's Join is on) or turn down.
    Request { id: String, name: String },
}

/// The driver: the worker while the setting is on, the activity last sent and its start.
#[derive(Default)]
pub struct Presence {
    #[cfg(not(target_arch = "wasm32"))]
    client: Option<ipc::Client>,
    #[cfg(not(target_arch = "wasm32"))]
    registered: bool,
    /// The last look ([`Presence::due`]), the phase and its start, the activity last handed to the worker.
    looked: Option<web_time::Instant>,
    phase: Option<(Phase, u64)>,
    last: Option<Value>,
    /// A join that came while it couldn't be carried out (in a game): kept until the main menu is up.
    pub pending: Option<String>,
}

/// How often the window reports.
const EVERY: std::time::Duration = std::time::Duration::from_secs(1);

fn unix_now() -> u64 {
    web_time::SystemTime::now().duration_since(web_time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Presence {
    /// Time for the window's next report.
    pub fn due(&mut self) -> bool {
        let now = web_time::Instant::now();
        if self.looked.is_some_and(|t| now.duration_since(t) < EVERY) {
            return false;
        }
        self.looked = Some(now);
        true
    }

    /// The window's report: `doing` under `setting` (off ends the worker). Returns Discord's asks since the last
    /// report.
    #[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
    pub fn report(&mut self, doing: &Doing, c: &Context) -> Vec<Ask> {
        #[cfg(target_arch = "wasm32")]
        return Vec::new();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if c.setting == Setting::Off {
                self.client = None;
                self.last = None;
                return Vec::new();
            }
            if self.client.is_none() {
                if !self.registered {
                    self.registered = true;
                    register::register(APP_ID);
                }
                match ipc::Client::start(APP_ID) {
                    Ok(cl) => self.client = Some(cl),
                    Err(e) => {
                        eprintln!("ig2-client: discord: {e}");
                        return Vec::new();
                    }
                }
                self.last = None;
            }
            let Some(client) = &self.client else { return Vec::new() };
            let phase = doing.phase();
            let since = match self.phase {
                Some((p, t)) if p == phase => t,
                _ => {
                    let t = unix_now();
                    self.phase = Some((phase, t));
                    t
                }
            };
            let a = activity(&shown(doing, c), since, c);
            if self.last.as_ref() != Some(&a) {
                client.set(Some(a.clone()));
                self.last = Some(a);
            }
            let mut asks = Vec::new();
            for e in client.events() {
                match e {
                    // The worker sends the activity again on a fresh connection.
                    ipc::Event::Ready(user) => eprintln!("ig2-client: discord: connected as {user}"),
                    ipc::Event::Join(code) => asks.push(Ask::Join(code)),
                    ipc::Event::JoinRequest { id, name } => asks.push(Ask::Request { id, name }),
                    ipc::Event::Lost => {}
                }
            }
            asks
        }
    }

    /// Answer a friend's join request.
    #[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
    pub fn answer(&self, id: String, accept: bool) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(c) = &self.client {
            c.answer(id, accept);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn races() -> Vec<String> {
        ["", "Solarian", "Shinari", "Kra'hen"].map(String::from).to_vec()
    }

    fn ctx(races: &[String], setting: Setting) -> Context<'_> {
        Context { races, lang: Lang::En, version: "0.7", setting }
    }

    fn play() -> Play {
        Play {
            mode: Mode::Campaign,
            replay: false,
            race: 1,
            difficulty: 2,
            date: (3012, 5, 7),
            screen: Screen::Research,
            paused: false,
            spectating: false,
            numbers: Numbers { planets: 12, population: 1_234_567, credits: 52_310, ships: 34, techs: 40, battles: 3, score: 980 },
            players: 0,
            party: None,
        }
    }

    fn room(host: bool, humans: u32, seats: u32, private: bool) -> Room {
        Room { name: "Ada's galaxy".into(), host, humans, seats, race: Some(3), ready: false, private, saved: false, party: party_id(&["Ada's galaxy", "Ada"]), code: Some("ABCDE-12345".into()) }
    }

    #[test]
    fn a_campaign_game() {
        let r = races();
        let c = ctx(&r, Setting::Join);
        let s = shown(&Doing::Game(play()), &c);
        assert_eq!(s.details, "Campaign · Solarian · Normal");
        assert_eq!(s.state, "Researching · 3012.05.07");
        assert_eq!(s.large, ("race_1", "12 planets · 1.2M population · 34 ships · 52,310 credits · score 980".to_string()));
        assert_eq!(s.small, Some(("research", "40 techs · 3 battles won".to_string())));
        assert_eq!((&s.party, &s.join), (&None, &None));
        let a = activity(&s, 1000, &c);
        assert_eq!(a["timestamps"]["start"], 1000);
        assert_eq!(a["assets"]["large_image"], "race_1");
        // No Join: the link buttons.
        assert_eq!(a["buttons"][0]["url"], RELEASES);
        assert!(a.get("secrets").is_none());
    }

    #[test]
    fn paused_spectating_and_replays() {
        let r = races();
        let c = ctx(&r, Setting::Join);
        let mut p = Play { screen: Screen::Starmap, paused: true, ..play() };
        let s = shown(&Doing::Game(p.clone()), &c);
        assert_eq!((s.state.as_str(), s.small.unwrap().0), ("Paused · 3012.05.07", "paused"));
        p.spectating = true;
        assert!(shown(&Doing::Game(p.clone()), &c).state.starts_with("Watching the galaxy"));
        p.replay = true;
        let s = shown(&Doing::Game(p), &c);
        assert_eq!((s.details.as_str(), s.small.unwrap().0), ("Watching a replay: Campaign · Solarian · Normal", "replay"));
    }

    #[test]
    fn a_multiplayer_game_is_a_party_without_join() {
        let r = races();
        let c = ctx(&r, Setting::Join);
        let p = Play { mode: Mode::Multiplayer, race: 3, difficulty: 2, players: 3, party: Some("ig2-1".into()), screen: Screen::SpaceBattle, ..play() };
        let s = shown(&Doing::Game(p), &c);
        assert_eq!(s.details, "Multiplayer · Kra'hen");
        assert_eq!(s.party, Some(("ig2-1".into(), 3, 3)));
        assert_eq!(s.small.unwrap(), ("space_battle", "3 players · 40 techs · 3 battles won".to_string()));
        assert_eq!(s.join, None);
    }

    #[test]
    fn a_hosts_open_room_offers_join() {
        let r = races();
        let s = shown(&Doing::Room(room(true, 2, 4, false)), &ctx(&r, Setting::Join));
        assert_eq!((s.details.as_str(), s.state.as_str()), ("Multiplayer room: Ada's galaxy", "Waiting for players"));
        assert_eq!(s.small, Some(("race_3", "Playing as Kra'hen".to_string())));
        assert_eq!(s.party.as_ref().map(|p| (p.1, p.2)), Some((2, 4)));
        assert_eq!(s.join.as_deref(), Some("ABCDE-12345"));
        let a = activity(&s, 5, &ctx(&r, Setting::Join));
        assert_eq!(a["secrets"]["join"], "ABCDE-12345");
        assert_eq!(a["party"]["size"], json!([2, 4]));
        assert!(a.get("buttons").is_none());
    }

    #[test]
    fn no_join_for_private_full_joined_rooms_or_status_only() {
        let r = races();
        let join = |rm: Room, s: Setting| shown(&Doing::Room(rm), &ctx(&r, s)).join;
        assert_eq!(join(room(true, 2, 4, true), Setting::Join), None);
        assert_eq!(join(room(true, 4, 4, false), Setting::Join), None);
        assert_eq!(join(room(false, 2, 4, false), Setting::Join), None);
        assert_eq!(join(room(true, 2, 4, false), Setting::Status), None);
        // A full room is ready to start.
        assert_eq!(shown(&Doing::Room(room(true, 4, 4, false)), &ctx(&r, Setting::Join)).state, "Ready to start");
    }

    #[test]
    fn menu_pages() {
        let r = races();
        let s = shown(&Doing::Menu(Page::Scenarios), &ctx(&r, Setting::Join));
        assert_eq!((s.details.as_str(), s.large.0), ("Choosing a scenario", "logo"));
        assert_eq!(s.large.1, "openIG2 0.7");
    }

    #[test]
    fn hungarian() {
        let r = races();
        let c = Context { races: &r, lang: Lang::Hu, version: "0.7", setting: Setting::Join };
        let s = shown(&Doing::Game(play()), &c);
        assert_eq!(s.details, "Kampány · Solarian · Normál");
        assert!(s.state.starts_with("Kutat ·"));
    }

    #[test]
    fn texts_fit_discord() {
        assert_eq!(fit("a"), "a ");
        assert_eq!(fit(&"x".repeat(300)).chars().count(), TEXT_MAX);
        assert_eq!(number(999), "999");
        assert_eq!(number(52_310), "52,310");
        assert_eq!(number(-1_234), "-1,234");
        assert_eq!(number(250_000), "250k");
        assert_eq!(number(12_500_000), "12.5M");
        // A mod's race: its number, the logo.
        let r = races();
        assert_eq!(ctx(&r, Setting::Join).race(11), "Race 11");
        assert_eq!(race_key(11), "logo");
    }

    #[test]
    fn parties_and_phases() {
        assert_eq!(party_id(&["a", "b"]), party_id(&["a", "b"]));
        assert_ne!(party_id(&["ab", ""]), party_id(&["a", "b"]));
        assert_eq!(Doing::Menu(Page::Main).phase(), Doing::Menu(Page::Mods).phase());
        assert_ne!(Doing::Game(play()).phase(), Doing::Game(Play { replay: true, ..play() }).phase());
    }

    #[test]
    fn settings() {
        let mut f = ig2_formats::options::Options::default();
        assert_eq!(Setting::read(&f), Setting::Join);
        f.set(KEY, "1");
        assert_eq!(Setting::read(&f), Setting::Status);
        f.set(KEY, "0");
        assert_eq!(Setting::read(&f), Setting::Off);
    }

    #[test]
    fn every_key_is_an_asset() {
        for k in (0..=11).map(race_key).chain([Screen::Starmap, Screen::Colony, Screen::GroundWar, Screen::SpaceBattle, Screen::Research, Screen::Design, Screen::Diplomacy, Screen::Spy, Screen::Trade, Screen::Info, Screen::Messages, Screen::Options, Screen::Help, Screen::Video].map(screen_key)).chain(["menu", "lobby", "replay", "paused"]) {
            assert!(ASSETS.contains(&k), "{k}");
        }
    }
}
