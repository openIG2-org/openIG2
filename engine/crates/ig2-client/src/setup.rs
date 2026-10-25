//! The window when no install is found (docs/USER_GUIDE.md → Installing): what the remake needs, the places it
//! looked in and why each failed; a folder (or its `ig2.exe`) dropped on the window is checked, remembered in the
//! settings file (`InstallDir`) and the remake starts again with the same arguments. Its text is drawn with the
//! system's Arial (the game's own fonts are in the install it can't find).

use std::path::Path;
use std::sync::Arc;

use ig2_rules::install::{self, Candidate, Verdict};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

use crate::render::{kind, Blend, Frame, Instance, Renderer, TexId};

/// The settings file's key for the install folder.
pub const INSTALL_KEY: &str = "InstallDir";

/// The message's lines and their colours (linear RGB).
pub fn lines(tried: &[(Candidate, Verdict)], status: Option<&str>) -> Vec<(String, [f32; 3])> {
    const TITLE: [f32; 3] = [1.0, 0.8, 0.3];
    const TEXT: [f32; 3] = [0.85, 0.85, 0.85];
    const DIM: [f32; 3] = [0.5, 0.5, 0.5];
    let mut out = vec![
        ("Imperium Galactica II was not found".to_string(), TITLE),
        (String::new(), TEXT),
        ("The remake runs on your own copy of the game (GOG) and ships none of its files.".into(), TEXT),
        ("Drop the game's folder, or its ig2.exe, on this window.".into(), TEXT),
        (String::new(), TEXT),
        ("Looked in:".into(), TEXT),
    ];
    let shown = tried.iter().filter(|(_, v)| *v != Verdict::Missing).take(10).collect::<Vec<_>>();
    for (c, v) in &shown {
        out.push((c.path.display().to_string(), TEXT));
        out.push((format!("      {} ({})", v.describe(), c.source.label()), DIM));
    }
    if shown.is_empty() {
        out.push(("the GOG registry entries and the default GOG folders: nothing there".into(), DIM));
    }
    if let Some(s) = status {
        out.push((String::new(), TEXT));
        out.push((s.to_string(), [1.0, 0.35, 0.3]));
    }
    out.push((String::new(), TEXT));
    out.push(("Esc quits.".into(), DIM));
    out
}

/// What a dropped path makes of the install: `Ok(folder)` when it is usable, else the line to show.
pub fn dropped(p: &Path) -> Result<std::path::PathBuf, String> {
    let dir = install::folder_of(p);
    match install::check(&dir) {
        Verdict::Ok => Ok(dir),
        v => Err(format!("{}: {}", dir.display(), v.describe())),
    }
}

/// Show the window until the player quits or drops a usable install (then the remake starts again).
pub fn run(tried: Vec<(Candidate, Verdict)>) -> Result<(), String> {
    for (l, _) in lines(&tried, None) {
        eprintln!("ig2-client: {l}");
    }
    if !cfg!(windows) {
        return Err("set IG2_INSTALL_DIR to your Imperium Galactica II folder".into());
    }
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let mut app = Setup { tried, status: None, gpu: None, restart: false, dirty: true };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    if app.restart {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        std::process::Command::new(exe).args(std::env::args_os().skip(1)).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// A drawn line: its texture and size.
type Line = (TexId, u32, u32);

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    /// Per line its texture and size (none for an empty line) and colour.
    text: Vec<(Option<Line>, [f32; 3])>,
}

struct Setup {
    tried: Vec<(Candidate, Verdict)>,
    status: Option<String>,
    gpu: Option<Gpu>,
    restart: bool,
    /// The lines' textures are out of date.
    dirty: bool,
}

const LINE: u32 = 20;

impl Setup {
    fn init(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let attrs = Window::default_attributes().with_title("Imperium Galactica II").with_inner_size(winit::dpi::LogicalSize::new(1000.0, 520.0));
        let window = Arc::new(el.create_window(attrs).map_err(|e| e.to_string())?);
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { compatible_surface: Some(&surface), ..Default::default() })).map_err(|e| e.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(wgpu::TextureFormat::is_srgb).unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: Renderer::view_formats(format),
        };
        surface.configure(&device, &config);
        let renderer = Renderer::new(device, queue, format);
        self.gpu = Some(Gpu { window, surface, config, renderer, text: Vec::new() });
        Ok(())
    }

    fn draw(&mut self) {
        let Some(g) = &mut self.gpu else { return };
        let scale = g.window.scale_factor() as f32;
        let px = ((LINE as f32 * scale).round() as u32).max(8);
        if self.dirty {
            g.text.clear();
            for (l, c) in lines(&self.tried, self.status.as_deref()) {
                let img = if l.is_empty() { None } else { text_line(&l, px) };
                g.text.push((img.map(|i| (g.renderer.add_texture(&i), i.width, i.height)), c));
            }
            self.dirty = false;
        }
        let mut frame = Frame::new([0.02, 0.02, 0.04, 1.0]);
        let (mut y, x) = (24.0 * scale, 24.0 * scale);
        for &(line, c) in &g.text {
            if let Some((tex, w, h)) = line {
                let half = [w as f32 / 2.0, h as f32 / 2.0];
                frame.push(tex, Blend::Alpha, Instance::new(kind::SPRITE, [x + half[0], y + half[1]], half, [c[0], c[1], c[2], 1.0]));
            }
            y += px as f32 * 1.35;
        }
        let Ok(out) = g.surface.get_current_texture() else { return };
        g.renderer.draw(&frame, &out.texture, (g.config.width, g.config.height), 0.0);
        g.window.pre_present_notify();
        out.present();
    }
}

#[cfg(windows)]
fn text_line(l: &str, px: u32) -> Option<crate::assets::Image> {
    crate::video::credits::gdi::render_left(l, px)
}

#[cfg(not(windows))]
fn text_line(_: &str, _: u32) -> Option<crate::assets::Image> {
    None
}

impl ApplicationHandler for Setup {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_none() {
            if let Err(e) = self.init(el) {
                eprintln!("ig2-client: {e}");
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() && event.logical_key == Key::Named(NamedKey::Escape) => el.exit(),
            WindowEvent::DroppedFile(p) => match dropped(&p) {
                Ok(dir) => {
                    if let Err(e) = ig2_formats::options::Options::store_own(&[(INSTALL_KEY, &dir.to_string_lossy())]) {
                        eprintln!("ig2-client: remembering the install: {e}");
                    }
                    // The restart finds it remembered (or, if the file can't be written, through the variable).
                    // SAFETY: single-threaded here; the restarted process inherits it.
                    unsafe { std::env::set_var(ig2_formats::vfs::INSTALL_ENV, &dir) };
                    self.restart = true;
                    el.exit();
                }
                Err(line) => {
                    self.status = Some(line);
                    self.dirty = true;
                }
            },
            WindowEvent::Resized(s) => {
                if let Some(g) = &mut self.gpu {
                    g.config.width = s.width.max(1);
                    g.config.height = s.height.max(1);
                    g.surface.configure(&g.renderer.device, &g.config);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => self.dirty = true,
            WindowEvent::RedrawRequested => self.draw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(g) = &self.gpu {
            g.window.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ig2_rules::install::Source;

    #[test]
    fn the_message_lists_the_places_tried() {
        let tried = vec![
            (Candidate { path: "C:\\GOG Games\\Imperium Galactica II".into(), source: Source::DefaultFolder }, Verdict::WrongBuild("abcdef0123456789".into())),
            (Candidate { path: "D:\\nowhere".into(), source: Source::Remembered }, Verdict::Missing),
        ];
        let l = lines(&tried, Some("X: no ig2.exe"));
        let text: Vec<&str> = l.iter().map(|(s, _)| s.as_str()).collect();
        let at = text.iter().position(|s| s.contains("GOG Games")).unwrap();
        assert!(text[at + 1].contains("another build (sha256 abcdef012345") && text[at + 1].contains("default folder"));
        assert!(!text.iter().any(|s| s.contains("nowhere")), "missing folders are left out");
        assert!(text.contains(&"X: no ig2.exe"));
    }

    /// A dropped `ig2.exe` stands for its folder (IG2_INSTALL_DIR; skipped without it).
    #[test]
    fn a_dropped_exe_is_its_folder() {
        let Some(root) = std::env::var_os(ig2_formats::vfs::INSTALL_ENV) else { return };
        let root = std::path::PathBuf::from(root);
        assert_eq!(dropped(&root.join("ig2.exe")), Ok(root.clone()));
        assert!(dropped(&root.join("dataraw")).is_err());
    }
}
