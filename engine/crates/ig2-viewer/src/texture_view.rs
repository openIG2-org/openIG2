use eframe::egui;
use ig2_formats::pvr::{PixelFormat, Pvr};
use ig2_formats::Install;

/// Shared helper: decode a PVR from the install into an egui texture.
pub fn load_pvr(ctx: &egui::Context, install: &Install, path: &str) -> Result<(egui::TextureHandle, String), String> {
    let bytes = install.read(path).map_err(|e| e.to_string())?;
    let pvr = Pvr::parse(&bytes).map_err(|e| e.to_string())?;
    let img = pvr.decode_rgba8().map_err(|e| e.to_string())?;
    let fmt = match pvr.format {
        PixelFormat::Generic { channels, bits } => {
            let n = channels.iter().take_while(|&&c| c != 0).count();
            let ch: String = channels[..n].iter().map(|&c| c as char).collect();
            let b: Vec<String> = bits[..n].iter().map(u8::to_string).collect();
            format!("{ch}{}", b.join(""))
        }
        PixelFormat::Compressed(7) => "BC1/DXT1".to_owned(),
        PixelFormat::Compressed(id) => format!("compressed #{id}"),
    };
    let info = format!("{}×{}  {fmt}  mips {}  premultiplied {}", img.width, img.height, pvr.mip_count, pvr.premultiplied);
    let color = egui::ColorImage::from_rgba_unmultiplied([img.width as usize, img.height as usize], &img.pixels);
    Ok((ctx.load_texture(path, color, egui::TextureOptions::NEAREST), info))
}

#[derive(Default)]
pub struct TextureView {
    current: crate::Cached<(egui::TextureHandle, String)>,
    zoom: f32,
    checker: bool,
}

impl TextureView {
    pub fn ui(&mut self, ui: &mut egui::Ui, install: &Install, path: &str) {
        if self.current.as_ref().map(|(p, _)| p.as_str()) != Some(path) {
            self.current = Some((path.to_owned(), load_pvr(ui.ctx(), install, path)));
            if self.zoom == 0.0 {
                self.zoom = 2.0;
                self.checker = true;
            }
        }
        let Some((_, res)) = &self.current else { return };
        match res {
            Err(e) => {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
            Ok((tex, info)) => {
                ui.horizontal(|ui| {
                    ui.label(info);
                    ui.separator();
                    ui.add(egui::Slider::new(&mut self.zoom, 0.25..=8.0).logarithmic(true).text("zoom"));
                    for z in [1.0, 2.0, 4.0] {
                        if ui.button(format!("{z}×")).clicked() {
                            self.zoom = z;
                        }
                    }
                    ui.checkbox(&mut self.checker, "checkerboard");
                });
                ui.separator();
                egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                    let size = tex.size_vec2() * self.zoom;
                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                    if self.checker {
                        paint_checker(ui.painter(), rect);
                    }
                    egui::Image::new((tex.id(), size)).paint_at(ui, rect);
                });
            }
        }
    }
}

pub fn paint_checker(p: &egui::Painter, rect: egui::Rect) {
    let s = 8.0;
    p.rect_filled(rect, 0.0, egui::Color32::from_gray(90));
    let (nx, ny) = ((rect.width() / s).ceil() as i32, (rect.height() / s).ceil() as i32);
    for y in 0..ny {
        for x in (y % 2..nx).step_by(2) {
            let min = rect.min + egui::vec2(x as f32 * s, y as f32 * s);
            let r = egui::Rect::from_min_size(min, egui::vec2(s, s)).intersect(rect);
            p.rect_filled(r, 0.0, egui::Color32::from_gray(140));
        }
    }
}
