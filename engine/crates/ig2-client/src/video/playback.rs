//! A video on screen (re/screens/video.md §2–§4): `OpenBink` + `PlayBink` + `CloseBink` as one object. Fullscreen
//! videos cover the window (aspect kept, black around) and skip on a press and release of any key or button;
//! in-screen clips sit at a point of the 640×480 frame and don't skip. The title adds the remaster's publisher logo.

use crate::assets::Image;
use crate::kit::layout::Virtual;
use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};

use super::Player;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Place {
    /// `OpenBink(name, 1, 0, 0, _)`.
    Full,
    /// `OpenBink(name, 0, x, y, _)`: the clip's top left in the 640×480 frame.
    At([f32; 2]),
}

/// `KellenekCutScenek` (`DAT_0091605c`, the options' `Cutscenes`, default 1): `OpenBink` without `force` plays
/// nothing when it is 0.
pub fn cutscenes_enabled() -> bool {
    ig2_formats::options::Options::load().int("Cutscenes").unwrap_or(1) != 0
}

/// The title's overlay (`ig2_title`, `ig2screen::render` exe 0x7370f0): visible for 5649 ms of the clip.
const TITLE_LOGO: &str = "gfx/thqnordic.pvr";

pub struct Playback {
    pub player: Player,
    pub place: Place,
    tex: Option<TexId>,
    /// A frame came out and was written to `tex`.
    shown: bool,
    /// `DAT_00fe6438`: a press seen while the video plays; the next release stops a fullscreen one.
    armed: bool,
    stopped: bool,
    /// The title's logo texture (`title.bik` only).
    logo: Option<Option<TexId>>,
    /// The credits line (`credits.bik` only): the window height it was rendered for and its texture.
    credits: Option<Option<(u32, TexId, [f32; 2])>>,
    /// The clock of the last update and of the first (the overlay's time base).
    now: u64,
    start: Option<u64>,
}

impl Playback {
    /// `OpenBink(bik, place, force)` + `PlayBink(_, looping)`: `None` when the cut-scenes are off (and not
    /// forced) or the file doesn't open (`ig2_binkres` 0).
    pub fn open(install: &ig2_formats::Install, bik: &str, place: Place, looping: bool, force: bool) -> Option<Self> {
        if !force && !cutscenes_enabled() {
            return None;
        }
        let path = super::mp4_path(bik);
        let player = match Player::open(install, &path, looping) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("ig2-client: video {bik}: {e}");
                return None;
            }
        };
        let lower = bik.to_ascii_lowercase();
        let (title, credits) = (lower.contains("title.bik"), lower.contains("credits.bik"));
        Some(Self {
            player, place, tex: None, shown: false, armed: false, stopped: false, logo: title.then_some(None),
            credits: credits.then_some(None), now: 0, start: None,
        })
    }

    /// One drawn frame at `now_ms` on a window `win_h` pixels high: the frame that came due goes to the texture.
    pub fn frame(&mut self, r: &mut Renderer, install: &ig2_formats::Install, now_ms: u64, win_h: u32) {
        self.now = now_ms;
        self.start.get_or_insert(now_ms);
        if let Some(None) = self.logo {
            self.logo = Some(Image::load(install, TITLE_LOGO).ok().map(|i| r.add_texture(&i)));
        }
        if let Some(c) = &mut self.credits {
            if c.is_none_or(|(h, _, _)| h != win_h) {
                let img = super::credits::text(install).and_then(|t| super::credits::render(install, &t, win_h));
                *c = img.map(|i| (win_h, r.add_texture(&i), [i.width as f32, i.height as f32]));
            }
        }
        if let Some(img) = self.player.update(now_ms) {
            self.upload(r, &img);
        }
    }

    /// [`Self::frame`] waiting for the decoder: a capture on the script's clock shows the frame due, not the one
    /// decoded by then.
    pub fn frame_blocking(&mut self, r: &mut Renderer, now_ms: u64) {
        self.now = now_ms;
        self.start.get_or_insert(now_ms);
        if let Some(img) = self.player.update_blocking(now_ms) {
            self.upload(r, &img);
        }
    }

    /// Shows `img` as the current frame (also the snapshot's still).
    pub fn upload(&mut self, r: &mut Renderer, img: &Image) {
        let tex = *self.tex.get_or_insert_with(|| r.video_texture(img.width, img.height));
        r.write_video(tex, img);
        self.shown = true;
    }

    /// The milliseconds since the first frame.
    pub fn elapsed(&self) -> u64 {
        self.now - self.start.unwrap_or(self.now)
    }

    /// The quad on a window of `win` pixels (`ig2screen::openAnim` exe 0x736960): fullscreen in a centred space
    /// 1024 units wide; in the screen through the 640×480 frame `v`.
    pub fn rect(&self, win: [f32; 2], v: &Virtual) -> ([f32; 2], [f32; 2]) {
        let (vw, vh) = self.player.size();
        let (vw, vh) = (vw as f32, vh as f32);
        match self.place {
            Place::Full => {
                let (mut w, mut h) = (0.0, 1024.0 * win[1] / win[0]);
                w = if vh > 0.0 { vw * h / vh } else { w };
                if w > 1024.0 {
                    w = 1024.0;
                    h = vh * 1024.0 / vw;
                }
                let k = win[0] / 1024.0;
                ([win[0] / 2.0, win[1] / 2.0], [w * k / 2.0, h * k / 2.0])
            }
            Place::At([x, y]) => {
                let a = v.px(x, y);
                let half = [vw * v.k / 2.0, vh * v.k / 2.0];
                ([a[0] + half[0], a[1] + half[1]], half)
            }
        }
    }

    /// The current frame's texture and rectangle (`None` before the first frame).
    pub fn quad(&self, win: [f32; 2], v: &Virtual) -> Option<(TexId, [f32; 2], [f32; 2])> {
        let tex = self.tex.filter(|_| self.shown)?;
        let (c, half) = self.rect(win, v);
        Some((tex, c, half))
    }

    /// Draws the frame (and the title's logo). Fullscreen frames go on a frame of their own over black.
    pub fn draw(&self, f: &mut Frame, win: [f32; 2], v: &Virtual) {
        let Some(tex) = self.tex.filter(|_| self.shown) else { return };
        let (c, half) = self.rect(win, v);
        f.push(tex, Blend::GammaAlpha, Instance::new(kind::SPRITE, c, half, [1.0; 4]));
        if let (Some(Some(logo)), Place::Full) = (self.logo, self.place) {
            let t = self.elapsed();
            if t < 0x1611 {
                // Alpha `t >> 3` up to 0x800, 255 to 0xe12, then down by `(t − 0xe12) >> 3`; `0xFFaaaaaa` tints RGB.
                let a = if t < 0x800 { (t >> 3) as f32 } else if t <= 0xe12 { 255.0 } else { 255.0 - ((t - 0xe12) >> 3) as f32 };
                let a = a.clamp(0.0, 255.0) / 255.0;
                // `(w/2, w/8)` units at `(0, w·240/1024)` below the centre, `w` the video quad's width in units.
                let k = win[0] / 1024.0;
                let w = half[0] * 2.0 / k;
                let size = [w / 2.0 * k, w / 8.0 * k];
                let at = [win[0] / 2.0, win[1] / 2.0 + w * 240.0 / 1024.0 * k];
                f.push(logo, Blend::GammaAlpha, Instance::new(kind::SPRITE, at, [size[0] / 2.0, size[1] / 2.0], [a, a, a, 1.0]));
            }
        }
        if let (Some(Some((_, text, size))), Some(a)) = (self.credits, super::credits::alpha(self.elapsed())) {
            // The shadow first, black at four fifths of the alpha, 2 px right and down (`(W>>1)+2`, `0.96·H + 2`).
            let at = [(win[0] as u32 >> 1) as f32, win[1] * 0.96];
            let half = [size[0] / 2.0, size[1] / 2.0];
            let sa = super::credits::shadow_alpha(a) as f32 / 255.0;
            f.push(text, Blend::GammaAlpha, Instance::new(kind::SPRITE, [at[0] + 2.0, at[1] + 2.0], half, [0.0, 0.0, 0.0, sa]));
            f.push(text, Blend::GammaAlpha, Instance::new(kind::SPRITE, at, half, [1.0, 1.0, 1.0, a as f32 / 255.0]));
        }
    }

    /// A key, mouse button or finger went down (`anyEvent` 0x300/0x401/0x700).
    pub fn press(&mut self) {
        self.armed = true;
    }

    /// One went up (0x301/0x402/0x701): after a press, a fullscreen video stops. Returns whether it did.
    pub fn release(&mut self) -> bool {
        if self.armed && self.place == Place::Full {
            self.stopped = true;
        }
        self.stopped
    }

    /// The video is over: played out (not looping) or skipped.
    pub fn done(&self) -> bool {
        self.stopped || self.player.done()
    }
}

/// `--snapshot --screen video:NAME --video-ms T`: the window at the clip's time `T` (frame `⌊T / 40⌋`, the overlay at
/// `T`), for the comparison with `run_video.py`'s grabs.
pub fn snapshot(o: &crate::Options, name: &str, out: &std::path::Path) -> Result<(), String> {
    let root = crate::install_root().map_err(|_| "no Imperium Galactica II install found; set IG2_INSTALL_DIR")?;
    let install = ig2_formats::Install::open(&root, ig2_formats::text::Lang::En).map_err(|e| e.to_string())?;
    let mut r = Renderer::headless(wgpu::TextureFormat::Rgba8UnormSrgb)?;
    let place = match o.video_at {
        Some(at) => Place::At(at),
        None => Place::Full,
    };
    let mut p = Playback::open(&install, name, place, o.video_loop, true).ok_or("the video doesn't open")?;
    let want = (o.video_ms as f32 / (1000.0 / p.player.fps)) as usize;
    let mut img = None;
    for _ in 0..=want {
        img = p.player.next_blocking();
    }
    let img = img.ok_or("the clip is shorter")?;
    p.frame(&mut r, &install, 0, o.window.1);
    p.now = o.video_ms;
    p.upload(&mut r, &img);
    let win = [o.window.0 as f32, o.window.1 as f32];
    let mut f = Frame::new([0.0, 0.0, 0.0, 1.0]);
    let v = Virtual::new(win);
    p.draw(&mut f, win, &v);
    let rgba = r.draw_to_image(&f, o.window, 0.0);
    crate::assets::write_png(out, o.window.0, o.window.1, &rgba)
}
