//! The main menu (`AktKep` 15, re/screens/mainscr.md): the ring menus' state, their 10 ms UI tick
//! (`FRISSIT_MainScr2` exe 0x4f65f0) and their input (`Mainscreen_Update3D` exe 0x500270's ring hit-tests,
//! `MainScr_Main` exe 0x4fefc0). The scene itself is `mainscr_scene`.
//!
//! Every button is a ring on a circle that turns with `MainScrMozgato`; a menu change flies the rings out
//! (`MainKorokTavolsagCel` 5) and back in as the next menu's set. The title video (`title.bik`) and the credits
//! (`misc/credits.bik`) wait for video playback (U30); the race, scenario, tutorial and multiplayer screens behind the
//! rings aren't ported yet (spec §14): their rings report [`Act`]s the front end answers.

use crate::starmap::MsvcRand;
use crate::scene::V3;

/// The ring menus (`Mode_Main`, `Mode_SinglePlayerSelected`, `Mode_MultiPlayerSelected`, `Mode_Tutorial_Valasztas`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    Main,
    Single,
    Multi,
    Tutorial,
}

/// The label sets by menu (`Main3DInit` step 11, exe pointer tables 0x913d90, 0x913da8, 0x913db4, 0x913dcc): the
/// file under `textures/main/eng/{unselected,selected}/`.
pub const LABELS_MAIN: [&str; 6] = ["0_credits.bmp", "0_exitus!.bmp", "0_loadgam.bmp", "0_multipl.bmp", "0_singlep.bmp", "0_tutoria.bmp"];
pub const LABELS_SINGLE: [&str; 3] = ["0__camp.bmp", "0__scenar.bmp", "0__back.bmp"];
pub const LABELS_MULTI: [&str; 3] = ["0___local.bmp", "0___inet.bmp", "0__back.bmp"];
pub const LABELS_TUTORIAL: [&str; 6] = ["tutorial/part1.bmp", "tutorial/part2.bmp", "tutorial/part3.bmp", "tutorial/part4.bmp", "tutorial/part5.bmp", "0__back.bmp"];

impl Menu {
    pub fn labels(self) -> &'static [&'static str] {
        match self {
            Menu::Main => &LABELS_MAIN,
            Menu::Single => &LABELS_SINGLE,
            Menu::Multi => &LABELS_MULTI,
            Menu::Tutorial => &LABELS_TUTORIAL,
        }
    }
}

/// What a ring press asks of the front end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Act {
    /// `misc/credits.bik` (U30: no video yet).
    Credits,
    /// `AktIkon` 0x26: `ig2_running = 0`, at once.
    Quit,
    /// `AktIkon` 0x25: the Options screen in load-only mode (`OptionsCsakLoadLehet`, `AktKep` 0xd).
    LoadGame,
    /// `AktIkon` 0x2b: the race choice (`Mode_Fajvalasztas`).
    Campaign,
    /// `AktIkon` 0x20: the scenario / skirmish list (`ScenarioGombraRanyomtak("")`).
    Scenario,
    /// A tutorial part 1..=5 (`TutorialStartMelyik`): the screen is left for the starmap.
    Tutorial(u8),
    /// `AktIkon` 0x23 (ring 0, "local"): the server path; 0x22 (ring 1, "inet"): the remaster's matchmaking.
    MultiServer,
    MultiMatchmaking,
}

/// A meteor (`Meteorok`, exe position 0xbbabe4, spin 0xbbac14, speed 0xbbac44, sound byte 0xbbac50).
#[derive(Debug, Clone, Copy, Default)]
pub struct Meteor {
    pub pos: V3,
    /// `RotateObjr`'s per-frame spin and the angles it has added up to.
    pub spin: V3,
    pub angles: V3,
    pub scale: f32,
    pub speed: f32,
    pub sounded: bool,
}

/// A ship (`Urhajok`; exe pos 0xbbac60, start 0xbbace0, target 0xbbad60, speed 0xbbade0, alpha 0xbbae70, roll
/// 0xbbae90).
#[derive(Debug, Clone, Copy, Default)]
pub struct Ship {
    pub shown: bool,
    pub pos: V3,
    pub start: V3,
    pub target: V3,
    pub speed: V3,
    /// `UrhajoAtlatszosagok`: the transparency, 1 → 0 over 50 ticks after a respawn.
    pub alpha: f32,
    /// `UrhajoElforgatasok`.
    pub roll: f32,
}

/// `METMAX` (exe 0x9140e4).
pub const METMAX: f32 = -50000.0;
/// `METKOZ` (ELF 0x1184638): where a meteor has faded in.
pub const METKOZ: f32 = -48000.0;

/// The main menu's state.
#[derive(Debug, Clone)]
pub struct MainScr {
    pub menu: Menu,
    /// `MainScrMozgato` (exe 0xbbabe0): the clock, +0.1 a tick (in f64, stored f32).
    pub m: f32,
    /// `MainKorokTavolsag` (0xbbb050) and its target `…Cel` (0xbbb0bc): 1.0 = the rings in place.
    pub tav: f32,
    pub cel: f32,
    /// `MaxMainKorok` (0xbbb0c0): the rings shown.
    pub rings: usize,
    /// `GlobalTransparency` (0xbbb0cc): the scene's fade-in 0..1.
    pub gt: f32,
    /// `MainUrhajoTimer` (0xbbb0c8), `MainAszteroidaTimer` (0xbbb0d0).
    pub ship_timer: i32,
    pub meteor_timer: i32,
    /// `AktKijelolt` (0xbbb3dc): the ring under the pointer this frame.
    pub hovered: Option<usize>,
    pub meteors: [Meteor; 3],
    pub ships: [Ship; 8],
    /// The UI ticks run.
    pub ticks: u64,
    /// `kamerapont`, `elforgatas` as the last drawn frame set them (the ticks read them).
    pub cam: (V3, f32),
}

/// `kamerapont` and `elforgatas` from `MainScrMozgato` with the y and z amplitudes and offsets of `Update3D` (100, −100,
/// 100, 2000) or `Main3DInit` (200, 0, 1000, 1500).
fn camera(m: f32, ya: f32, yo: f32, za: f64, zo: f64) -> (V3, f32) {
    let x = ((m / 50.0) as f64).cos() as f32 * 100.0 + 100.0;
    let y = ((m / 82.0) as f64).sin() as f32 * ya + yo;
    let z = (((m / 165.0) as f64 + std::f64::consts::FRAC_PI_2).sin() * za + zo) as f32;
    let a = (((m / 150.0) as f64 + std::f64::consts::PI).cos() / 3.0 + 0.2) as f32;
    ([x, y, z], a)
}

/// `random(n)` with the exe's `rand() % n`.
fn rnd(r: &mut MsvcRand, n: i32) -> i32 {
    r.random(n)
}

/// The libc stream as a 10 ms tick finds it: every drawn frame ends with `DeGenerateKod`'s `srand(400)` and
/// 3 × 1750 draws (spec §3.2).
fn tick_stream() -> MsvcRand {
    let mut r = MsvcRand(400);
    for _ in 0..5250 {
        r.rand();
    }
    r
}

impl MainScr {
    /// `MainScr_Belep` on a fresh entry (`Main3DInit`): the rings far out (`Tav` 10, `Cel` 1), the clocks at 0, the
    /// meteors and ships placed by `Main3DInit` steps 8–9 from the stream `GenerateKod2` left (`srand(400)` + 7000).
    pub fn enter() -> Self {
        let mut r = MsvcRand(400);
        for _ in 0..7000 {
            r.rand();
        }
        let mut meteors = [Meteor::default(); 3];
        for (i, m) in meteors.iter_mut().enumerate() {
            let s = (3.0 / (i as f32 + 0.5)) / 3.0 * 1.5;
            let s = if s <= 0.0 { 0.1 } else { s };
            let scale = s * ((rnd(&mut r, 3) as f32) / 10.0 + 0.5);
            let x = (rnd(&mut r, 5000) - 5000) as f32;
            let y = (rnd(&mut r, 5000) - 5000) as f32;
            let z = -(rnd(&mut r, (-METMAX) as i32) as f32);
            let spin = [0; 3].map(|_| (rnd(&mut r, 10) as f32) / 55.0 / scale);
            *m = Meteor { pos: [x, y, z], spin, angles: [0.0; 3], scale, speed: 25.0, sounded: false };
        }
        let mut s = Self {
            menu: Menu::Main,
            m: 0.0,
            tav: 10.0,
            cel: 1.0,
            rings: 6,
            gt: 0.0,
            ship_timer: 0,
            meteor_timer: 0,
            hovered: None,
            meteors,
            ships: [Ship::default(); 8],
            ticks: 0,
            cam: camera(0.0, 200.0, 0.0, 1000.0, 1500.0),
        };
        for i in 0..8 {
            s.respawn_ship(i, &mut r);
        }
        s
    }

    /// The camera (`Mainscreen_Update3D` step 1 [asm 0x5002b0..]): `kamerapont` and `elforgatas`. `M/k` is an f32
    /// division, the sines f64; x and y go on in f32, z and the angle in f64.
    pub fn camera(&self) -> (V3, f32) {
        camera(self.m, 100.0, -100.0, 100.0, 2000.0)
    }

    /// Per drawn frame: the camera of the frame (what the next ticks read as `kamerapont`).
    pub fn frame(&mut self) {
        self.cam = self.camera();
        for m in &mut self.meteors {
            for c in 0..3 {
                m.angles[c] += m.spin[c];
            }
        }
    }

    /// Ring `i`'s position (step 8): on a circle of 800·`Tav`, pushed back by `(Tav − 1)·400`.
    pub fn ring_pos(&self, i: usize) -> V3 {
        let t = self.tav as f64;
        let th = self.m as f64 / 50.0 + (std::f64::consts::TAU / self.rings as f64) * i as f64;
        [(th.sin() * 800.0 * t) as f32, (th.cos() * 800.0 * t) as f32, (-3000.0 - (t - 1.0) * 400.0) as f32]
    }

    /// Ring `i`'s rotation (`RotateObj(A, ((Tav − 1)/5)·π, 0, (2π/N)·i + M/(2i + 18))`).
    pub fn ring_rot(&self, i: usize) -> (f32, f32, f32) {
        let a = (self.tav - 1.0) / 5.0 * std::f32::consts::PI;
        let c = (std::f32::consts::TAU / self.rings as f32) * i as f32 + self.m / (i as f32 * 2.0 + 18.0);
        (a, 0.0, c)
    }

    /// The ring's hit centre on the 640 × 480 screen (`GetObjPos` × 1024 → `TransformPoint`: the rings are camera-fixed,
    /// fov 600, centre (320, 240) [obs]).
    pub fn ring_centre(&self, i: usize) -> [f32; 2] {
        let p = self.ring_pos(i);
        let k = 600.0 / -p[2];
        [p[0] * k + 320.0, p[1] * k + 240.0]
    }

    /// The labels this frame (step 10): the menu's set; `None` while the rings are flying out (`Cel` 5).
    pub fn labels(&self) -> Option<&'static [&'static str]> {
        (self.cel != 5.0).then(|| self.menu.labels())
    }

    /// The ring under the virtual point `at`, only while the rings rest (`Tav == 1.0` exactly): a box of ±40 around
    /// each centre (f32 compares, left/top inclusive).
    pub fn ring_at(&self, at: [f32; 2]) -> Option<usize> {
        if self.tav != 1.0 {
            return None;
        }
        let n = if self.menu == Menu::Main { 6 } else { self.rings };
        let (x, y) = (at[0].floor(), at[1].floor());
        (0..n).find(|&i| {
            let c = self.ring_centre(i);
            c[0] - 40.0 <= x && x < c[0] + 40.0 && c[1] - 40.0 <= y && y < c[1] + 40.0
        })
    }

    /// The pointer this frame (`AktKijelolt`; the hand pointer over a ring).
    pub fn hover(&mut self, at: [f32; 2]) {
        self.hovered = self.ring_at(at);
    }

    /// A left press at `at` (the rings that act on `Mouse_Lenyom && Mouse_Bal` in `Update3D`); the box rings act on
    /// the release ([`MainScr::release`]).
    pub fn press(&mut self, at: [f32; 2]) -> Option<Act> {
        let i = self.ring_at(at)?;
        match (self.menu, i) {
            (Menu::Main, 0) => Some(Act::Credits),
            (Menu::Main, 3) => {
                // The multiplayer menu (the name entry `EnterNameMode` over it is not ported).
                self.fly_to(Menu::Multi);
                None
            }
            (Menu::Main, 4) => {
                self.fly_to(Menu::Single);
                None
            }
            (Menu::Single, 2) | (Menu::Multi, 2) | (Menu::Tutorial, 5) => {
                self.fly_to(Menu::Main);
                None
            }
            // The exe flies the rings away (`Cel` 6) and leaves for the tutorial; until it is ported they stay.
            (Menu::Tutorial, k) => Some(Act::Tutorial(k as u8 + 1)),
            _ => None,
        }
    }

    /// A left release at `at` on a box ring (`Mouse_Szoveg_Set(.., 1, 1)`: `MainScr_Main` sees `AktIkon` once).
    pub fn release(&mut self, at: [f32; 2]) -> Option<Act> {
        let i = self.ring_at(at)?;
        match (self.menu, i) {
            (Menu::Main, 1) => Some(Act::Quit),
            (Menu::Main, 2) => {
                self.cel = 6.0;
                Some(Act::LoadGame)
            }
            (Menu::Main, 5) => {
                self.fly_to(Menu::Tutorial);
                None
            }
            // `Cel` 6 and the race choice in the exe; the rings stay until it is ported.
            (Menu::Single, 0) => Some(Act::Campaign),
            (Menu::Single, 1) => Some(Act::Scenario),
            (Menu::Multi, 0) => Some(Act::MultiServer),
            (Menu::Multi, 1) => Some(Act::MultiMatchmaking),
            _ => None,
        }
    }

    /// Esc (`MainScr_ESCPressedWhenMultiplayerselect` exe 0x4fe780, the ring menus' case): with the rings at rest,
    /// back to the main set through a fly-out, also from the main set itself; never quits.
    pub fn escape(&mut self) {
        if self.tav == 1.0 {
            self.fly_to(Menu::Main);
        }
    }

    /// A menu change: the rings fly out (`Cel` 5) and come back as `menu`'s set.
    fn fly_to(&mut self, menu: Menu) {
        self.menu = menu;
        self.cel = 5.0;
    }

    /// One 10 ms UI tick (`FRISSIT_MainScr2`, exe 0x4f65f0, spec §5; the gate `MainElindult` is open once a frame was
    /// drawn).
    pub fn tick(&mut self) {
        self.ticks += 1;
        self.ship_timer += 1;
        self.meteor_timer += 1;
        self.m = (self.m as f64 + 0.1) as f32;
        let mut r = tick_stream();
        self.tick_meteors(&mut r);
        self.tick_ships(&mut r);
        // The rings toward `Cel` by 0.05 (f64), clamped; reaching 5.0 going out brings them back as the next set.
        if self.tav != self.cel {
            if self.tav > self.cel {
                self.tav = (self.tav as f64 - 0.05) as f32;
                if self.cel >= self.tav {
                    self.tav = self.cel;
                }
            }
            if self.cel > self.tav {
                self.tav = (self.tav as f64 + 0.05) as f32;
                if self.tav >= self.cel {
                    self.tav = self.cel;
                    if self.cel == 5.0 {
                        self.rings = if self.menu == Menu::Single || self.menu == Menu::Multi { 3 } else { 6 };
                        self.cel = 1.0;
                    }
                }
            }
        }
        // §5.2: the fade-in (f32 steps of `GlobalTransparencyNovelo` 0.003).
        if self.gt < 1.0 {
            self.gt += 0.003;
            if self.gt > 1.0 {
                self.gt = 1.0;
            }
        }
    }

    /// §5.3 meteors (k = 1: the exit animation isn't ported).
    fn tick_meteors(&mut self, r: &mut MsvcRand) {
        let camz = self.cam.0[2];
        let mut timer = self.meteor_timer;
        for m in &mut self.meteors {
            let p = &mut m.pos;
            if p[2] < camz + 1000.0 {
                p[2] += m.speed;
            }
            for c in [0, 1] {
                if 0.0 <= p[c] && p[c] < 500.0 {
                    p[c] += 10.0;
                }
                if -500.0 < p[c] && p[c] < 0.0 {
                    p[c] -= 10.0;
                }
            }
            if -3000.0 < p[2] && p[2] < 3000.0 && -150.0 < p[1] && p[1] < 1500.0 {
                if 0.0 < p[0] && p[0] < 500.0 {
                    p[0] += 1.0;
                }
                if -500.0 < p[0] && p[0] < 0.0 {
                    p[0] -= 1.0;
                }
            }
            if p[2] > camz + 1000.0 && timer > 50 {
                p[2] = METMAX - (r.rand() % 2000) as f32;
                p[0] = (r.rand() % 3500 - 2500) as f32;
                p[1] = (r.rand() % 4500 - 2500) as f32;
                self.meteor_timer = 0;
                timer = 0;
                m.sounded = false;
            }
        }
    }

    /// §5.3 ships.
    fn tick_ships(&mut self, r: &mut MsvcRand) {
        let mut timer = self.ship_timer;
        for i in 0..8 {
            let s = &mut self.ships[i];
            let p = &mut s.pos;
            if -3000.0 < p[2] && p[2] < 3000.0 && -150.0 < p[1] && p[1] < 1500.0 {
                if 0.0 < p[0] && p[0] < 500.0 {
                    p[0] += 1.0;
                    s.roll = (s.roll as f64 - 0.005) as f32;
                }
                if -500.0 < p[0] && p[0] < 0.0 {
                    p[0] -= 1.0;
                    s.roll = (s.roll as f64 + 0.005) as f32;
                }
            }
            if s.roll != 0.0 {
                s.roll = (s.roll as f64 * 0.99) as f32;
            }
            if (s.roll as f64).abs() < 0.001 {
                s.roll = 0.0;
            }
            s.roll = s.roll.clamp(-2.0, 2.0);
            if (s.pos[2] - s.target[2]).abs() >= 50.0 {
                for c in 0..3 {
                    s.pos[c] += s.speed[c];
                }
            } else if timer > 200 {
                self.respawn_ship(i, r);
                self.ship_timer = 0;
                timer = 0;
            }
            let s = &mut self.ships[i];
            if s.alpha != 0.0 {
                s.alpha = (s.alpha as f64 - 0.02) as f32;
                if s.alpha < 0.0 {
                    s.alpha = 0.0;
                }
            }
        }
    }

    /// `GenerateUrhajoPosok(i)` (exe 0x4f8380, spec §5.5; all f32).
    fn respawn_ship(&mut self, i: usize, r: &mut MsvcRand) {
        let shown = r.rand() % 6 == 0;
        let (cam, a) = self.cam;
        // The eye in the world: the inverse of `CreateMatrix(elforgatas, 0, 0)` (a rotation: its transpose).
        let m = crate::scene::create_matrix(a, 0.0, 0.0);
        let p = [
            m[0] * cam[0] + m[3] * cam[1] + m[6] * cam[2],
            m[1] * cam[0] + m[4] * cam[1] + m[7] * cam[2],
            m[2] * cam[0] + m[5] * cam[1] + m[8] * cam[2],
        ];
        let start = [(r.rand() % 3000 - 1000) as f32, (r.rand() % 3000 - 1500) as f32, (r.rand() % 2000 - 9000) as f32];
        let target = [p[0], p[1], p[2] + 3000.0];
        let k = (r.rand() % 50) as f32 / 10.0 + 2.0;
        let speed = std::array::from_fn(|c| (target[c] - start[c]) * k / 8000.0);
        self.ships[i] = Ship { shown, pos: start, start, target, speed, alpha: 1.0, roll: 0.0 };
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ring hit centres the exe's `TransformPoint` gave at `MainScrMozgato` 25.1 (`tools/trace/probes/mainscr.js`,
    /// quick-1.sav, spec §13).
    #[test]
    fn ring_centres_match_the_exe() {
        let mut s = MainScr::enter();
        s.m = 25.100_06;
        s.tav = 1.0;
        let exe = [(396.98892, 380.25943), (479.9627, 243.45535), (402.97375, 103.19592), (243.01108, 99.74057), (160.0373, 236.54465), (237.02625, 376.80408)];
        for (i, (x, y)) in exe.into_iter().enumerate() {
            let c = s.ring_centre(i);
            assert!((c[0] - x).abs() < 0.01 && (c[1] - y).abs() < 0.01, "ring {i}: {c:?} vs ({x}, {y})");
        }
    }

    /// `Main3DInit`'s meteors and ships as the exe placed them (probe of `Update3D`'s first call, spec §13): the MSVC
    /// stream from `srand(400)` + 7000 draws.
    #[test]
    fn meteors_and_ships_match_the_exe() {
        let s = MainScr::enter();
        let met = [[-4339.0, -956.0, -10050.0], [-1254.0, -3312.0, -18238.0], [-2493.0, -474.0, -8061.0]];
        for (m, e) in s.meteors.iter().zip(met) {
            assert_eq!(m.pos, e);
        }
        let ships = [[977.0, -843.0, -8837.0], [887.0, 813.0, -7774.0], [-975.0, -588.0, -8766.0], [-100.0, -473.0, -7274.0], [1928.0, 1132.0, -7926.0], [1760.0, -249.0, -8105.0], [263.0, 399.0, -8919.0], [1623.0, -506.0, -7206.0]];
        for (sh, e) in s.ships.iter().zip(ships) {
            assert_eq!(sh.pos, e);
        }
        // Ship 0's speed (−0.6, 0.4, 11.8) [obs, rounded].
        let v = s.ships[0].speed;
        assert!((v[0] + 0.6).abs() < 0.06 && (v[1] - 0.4).abs() < 0.06 && (v[2] - 11.8).abs() < 0.06, "{v:?}");
    }

    /// `Tav` flies 10 → 1 in 180 ticks (0.05 a tick in f64) and lands on 1.0 exactly; M is 18.0 after 180 ticks.
    #[test]
    fn rings_fly_in() {
        let mut s = MainScr::enter();
        let mut n = 0;
        while s.tav != 1.0 {
            s.tick();
            n += 1;
            assert!(n < 1000);
        }
        assert_eq!(n, 180);
    }

    /// A menu change flies out to 5 and back as the next set (3 rings for single player).
    #[test]
    fn menu_change() {
        let mut s = MainScr::enter();
        s.tav = 1.0;
        let c = s.ring_centre(4);
        assert_eq!(s.press(c), None);
        assert_eq!(s.menu, Menu::Single);
        while s.cel != 1.0 || s.tav != 1.0 {
            s.tick();
        }
        assert_eq!(s.rings, 3);
        assert_eq!(s.labels(), Some(&LABELS_SINGLE[..]));
        // Esc at rest flies back to the main set (0x4fe780); not while the rings move.
        s.escape();
        assert_eq!((s.menu, s.cel), (Menu::Main, 5.0));
        s.tick();
        s.menu = Menu::Single;
        s.escape();
        assert_eq!(s.menu, Menu::Single);
    }
}
