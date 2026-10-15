use eframe::egui;
use ig2_formats::ani::{Ani, Frame};
use ig2_formats::Install;

pub struct Decoded {
    pub width: usize,
    pub height: usize,
    pub frames: Vec<Frame>,
    pub info: String,
}

pub fn decode(install: &Install, path: &str) -> Result<Decoded, String> {
    let bytes = install.read(path).map_err(|e| e.to_string())?;
    let ani = Ani::parse(&bytes).map_err(|e| e.to_string())?;
    let frames = ani.frames().map_err(|e| e.to_string())?;
    Ok(Decoded {
        width: ani.width as usize,
        height: ani.height as usize,
        info: format!(
            "{}×{}  {} frames  flags {:#x} ({}, {} colours)",
            ani.width,
            ani.height,
            frames.len(),
            ani.flags,
            if ani.packed() { "LZSS" } else { "stored" },
            if ani.flags & 4 != 0 { 256 } else { 128 }
        ),
        frames,
    })
}

/// Headless: tile every `step`-th frame into a 5-wide contact sheet.
pub fn snapshot(install: &Install, path: &str, out: &std::path::Path) -> Result<String, String> {
    let d = decode(install, path)?;
    let picks: Vec<&Frame> = d.frames.iter().step_by((d.frames.len() / 10).max(1)).take(10).collect();
    let cols = 5.min(picks.len());
    let rows = picks.len().div_ceil(cols);
    let (sw, sh) = (cols * d.width, rows * d.height);
    let mut sheet = vec![0u8; sw * sh * 4];
    for (k, f) in picks.iter().enumerate() {
        let (ox, oy) = ((k % cols) * d.width, (k / cols) * d.height);
        for y in 0..d.height {
            let src = &f.rgba[y * d.width * 4..(y + 1) * d.width * 4];
            let dst = ((oy + y) * sw + ox) * 4;
            sheet[dst..dst + d.width * 4].copy_from_slice(src);
        }
    }
    crate::write_png(out, sw as u32, sh as u32, &sheet)?;
    Ok(format!("{path}: {}", d.info))
}

pub struct AniView {
    current: crate::Cached<(Decoded, Vec<egui::TextureHandle>)>,
    frame: usize,
    playing: bool,
    fps: f32,
    last: f64,
    zoom: f32,
}

impl Default for AniView {
    fn default() -> Self {
        Self { current: None, frame: 0, playing: true, fps: 12.0, last: 0.0, zoom: 3.0 }
    }
}

impl AniView {
    pub fn ui(&mut self, ui: &mut egui::Ui, install: &Install, path: &str) {
        if self.current.as_ref().map(|(p, _)| p.as_str()) != Some(path) {
            let res = decode(install, path).map(|d| {
                let texs = d
                    .frames
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let img = egui::ColorImage::from_rgba_unmultiplied([d.width, d.height], &f.rgba);
                        ui.ctx().load_texture(format!("{path}#{i}"), img, egui::TextureOptions::NEAREST)
                    })
                    .collect();
                (d, texs)
            });
            self.current = Some((path.to_owned(), res));
            self.frame = 0;
        }
        let Some((_, res)) = &self.current else { return };
        let (d, texs) = match res {
            Ok(v) => v,
            Err(e) => {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
                return;
            }
        };
        if texs.is_empty() {
            ui.label("no frames");
            return;
        }
        ui.horizontal(|ui| {
            ui.label(&d.info);
            ui.separator();
            if ui.button(if self.playing { "⏸" } else { "▶" }).clicked() {
                self.playing = !self.playing;
            }
            ui.add(egui::Slider::new(&mut self.frame, 0..=texs.len() - 1).text("frame"));
            ui.add(egui::Slider::new(&mut self.fps, 1.0..=30.0).text("fps"));
            ui.add(egui::Slider::new(&mut self.zoom, 1.0..=6.0).text("zoom"));
        });
        let now = ui.input(|i| i.time);
        if self.playing {
            if now - self.last >= 1.0 / self.fps as f64 {
                self.frame = (self.frame + 1) % texs.len();
                self.last = now;
            }
            ui.ctx().request_repaint();
        }
        let tex = &texs[self.frame.min(texs.len() - 1)];
        let size = tex.size_vec2() * self.zoom;
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        crate::texture_view::paint_checker(ui.painter(), rect);
        egui::Image::new((tex.id(), size)).paint_at(ui, rect);
    }
}
