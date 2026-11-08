//! Ours (ADR 0080): what the window tells Discord ([`crate::discord`]) once a second, and what it does with a
//! friend's Join: the multiplayer connection page opened over the main menu and the join code joined there.

use ig2_sim::save::va;

use super::App;
use crate::discord::{Ask, Context, Doing, Mode, Numbers, Page, Play, Screen, Setting};

impl App {
    /// The once-a-second report, and Discord's asks carried out.
    pub(super) fn discord_frame(&mut self) {
        if self.headless.is_some() || !self.discord.due() {
            return;
        }
        let setting = Setting::read(&self.settings);
        if setting != Setting::Off && self.discord_races.is_empty() {
            self.discord_races = self.race_names();
        }
        let doing = self.discord_doing();
        let c = Context { races: &self.discord_races, lang: self.install.lang(), version: crate::version(), setting };
        let asks = self.discord.report(&doing, &c);
        let join_on = matches!(&doing, Doing::Room(r) if crate::discord::shown(&doing, &c).join.is_some() && r.host);
        for a in asks {
            match a {
                Ask::Join(code) if setting == Setting::Join => self.discord.pending = Some(code),
                Ask::Join(_) => {}
                Ask::Request { id, name } => {
                    self.discord.answer(id, join_on);
                    if join_on {
                        if let Some(mp) = self.front.as_mut().and_then(|f| f.mp.as_mut()) {
                            mp.discord_joining(&name);
                        }
                    }
                }
            }
        }
        self.discord_join();
    }

    /// The race names by race number in the game's language (the activity's texts; the hud's when a game is up).
    fn race_names(&self) -> Vec<String> {
        let lang = self.install.lang();
        let names = match &self.hud {
            Some(h) => h.texts.races(),
            None => crate::ui::UiTexts::load(&self.install).races(),
        };
        names.iter().map(|n| ig2_formats::text::decode(n, lang)).collect()
    }

    /// A join that came from Discord, carried out once the main menu is up (a game running keeps it): the connection
    /// page opens (as the Internet ring opens it) and joins the code.
    fn discord_join(&mut self) {
        if (self.discord.pending.is_none() && self.mp_join.is_none()) || self.video.is_some() || self.options.is_some() {
            return;
        }
        let Some(f) = &mut self.front else { return };
        if f.s.start.is_some() || f.s.leaving.is_some() || f.session.is_some() {
            return;
        }
        let (code, discord) = match self.discord.pending.take() {
            Some(c) => (c, true),
            None => match self.mp_join.take() {
                Some(c) => (c, false),
                None => return,
            },
        };
        if f.mp.as_ref().is_some_and(|mp| mp.state().is_some()) {
            // Already in a room: the Join is dropped.
            return;
        }
        if f.mp.is_none() {
            f.stats = None;
            f.replays = None;
            f.mods = None;
            f.custom = None;
            f.s.scenario = None;
            f.s.race = None;
            f.s.cel = 6.0;
            f.s.mp = true;
            self.front_act(crate::mainscr::Act::MultiMatchmaking);
        }
        let Some(f) = &mut self.front else { return };
        if let Some(mp) = &mut f.mp {
            if discord {
                mp.join_from_discord(&code, &mut f.net);
            } else {
                mp.join_typed(&code, &mut f.net);
            }
        }
        if let Some(g) = &self.gpu {
            g.window.focus_window();
        }
    }

    /// What the player is doing, as Discord shows it.
    fn discord_doing(&self) -> Doing {
        if let Some(f) = &self.front {
            let page = match () {
                _ if self.options.is_some() => Page::Settings,
                _ if f.mp.is_some() => return f.mp.as_ref().and_then(|mp| mp.discord_room()).map_or(Doing::Menu(Page::Multiplayer), Doing::Room),
                _ if f.stats.is_some() => Page::History,
                _ if f.replays.is_some() => Page::Replays,
                _ if f.mods.is_some() => Page::Mods,
                _ if f.s.scenario.is_some() => Page::Scenarios,
                _ if f.s.race.is_some() || f.s.new_game_up() || f.custom.is_some() => Page::NewGame,
                _ => Page::Main,
            };
            return Doing::Menu(page);
        }
        let g = &self.world.game;
        let s = &g.state;
        let view = g.local_view();
        let en = view.empire as usize;
        let multiplayer = self.multiplayer();
        let mode = match () {
            _ if multiplayer => Mode::Multiplayer,
            _ if self.tutorial.is_some() || s.mem.u8(va::TUTORIAL_ACTIVE) != 0 => Mode::Tutorial,
            _ if s.mem.i32(va::CAMPAIGN_TYPE) > 0 => Mode::Campaign,
            _ if self.world.scenario => Mode::Scenario,
            _ => Mode::Single,
        };
        let screen = match self.akt_kep_now() {
            1 => Screen::Help,
            3 if self.volt_harc => Screen::GroundWar,
            3 => Screen::Colony,
            4 => Screen::Research,
            5 => Screen::Design,
            6 => Screen::Diplomacy,
            7 => Screen::Spy,
            8 => Screen::Trade,
            9 => Screen::Info,
            10 => Screen::Messages,
            11 => Screen::SpaceBattle,
            0xd => Screen::Options,
            _ => Screen::Starmap,
        };
        let row = en.checked_sub(1).and_then(|i| ig2_sim::stats::sample(s).rows.get(i).copied()).unwrap_or_default();
        let humans: Vec<usize> = s.human_controlled().iter().enumerate().filter(|(_, h)| **h).map(|(e, _)| e).collect();
        let party = self.world.net.as_ref().map(|(session, _)| {
            let mut names: Vec<String> = humans.iter().map(|&e| session.name_of(e as u8)).collect();
            names.sort();
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            crate::discord::party_id(&names)
        });
        Doing::Game(Play {
            mode,
            replay: self.world.viewer.is_some(),
            race: s.races().get(en).copied().unwrap_or(0),
            difficulty: if multiplayer { 0 } else { s.mem.i32(va::NEHEZSEG) },
            date: self.world.curr.date(),
            screen,
            paused: self.world.paused(),
            spectating: view.spectating,
            numbers: Numbers { planets: row.planets, population: row.population, credits: row.credits, ships: row.ships, techs: row.techs, battles: row.battles, score: row.score },
            players: if multiplayer { humans.len() as u32 } else { 0 },
            party,
        })
    }
}
