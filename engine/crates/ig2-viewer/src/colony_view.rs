use eframe::egui;
use ig2_formats::colony_map::{ColonyMap, MASK_STRIDE, SIDE};
use ig2_formats::Install;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    Heights,
    Tiles,
}

pub struct ColonyView {
    current: crate::Cached<(ColonyMap, Images)>,
    layer: Layer,
    show_a: bool,
    show_b: bool,
    show_pad: bool,
    cell: f32,
}

impl Default for ColonyView {
    fn default() -> Self {
        Self { current: None, layer: Layer::Heights, show_a: true, show_b: false, show_pad: false, cell: 12.0 }
    }
}

struct Images {
    heights: egui::TextureHandle,
    tiles: egui::TextureHandle,
    h_range: (i16, i16),
}

fn tile_color(t: u8) -> [u8; 3] {
    // Stable pseudo-random colour per tile ID.
    let h = (t as u32).wrapping_mul(0x9E37_79B1);
    [(h >> 24) as u8 | 0x30, (h >> 16) as u8 | 0x30, (h >> 8) as u8 | 0x30]
}

fn build(ctx: &egui::Context, path: &str, m: &ColonyMap) -> Images {
    let (lo, hi) = m.heights.iter().fold((i16::MAX, i16::MIN), |(lo, hi), &h| (lo.min(h), hi.max(h)));
    let span = (hi as i32 - lo as i32).max(1) as f32;
    let mut hp = Vec::with_capacity(SIDE * SIDE * 4);
    let mut tp = Vec::with_capacity(SIDE * SIDE * 4);
    for y in 0..SIDE {
        for x in 0..SIDE {
            let v = ((m.height(x, y) as i32 - lo as i32) as f32 / span * 255.0) as u8;
            hp.extend_from_slice(&[v, v, v, 255]);
            let c = tile_color(m.tile(x, y));
            tp.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    let img = |p: &[u8]| egui::ColorImage::from_rgba_unmultiplied([SIDE, SIDE], p);
    Images {
        heights: ctx.load_texture(format!("{path}#h"), img(&hp), egui::TextureOptions::NEAREST),
        tiles: ctx.load_texture(format!("{path}#t"), img(&tp), egui::TextureOptions::NEAREST),
        h_range: (lo, hi),
    }
}

impl ColonyView {
    pub fn ui(&mut self, ui: &mut egui::Ui, install: &Install, path: &str) {
        if self.current.as_ref().map(|(p, _)| p.as_str()) != Some(path) {
            let res = install
                .read(path)
                .map_err(|e| e.to_string())
                .and_then(|b| ColonyMap::parse(&b).map_err(|e| e.to_string()))
                .map(|m| {
                    let imgs = build(ui.ctx(), path, &m);
                    (m, imgs)
                });
            self.current = Some((path.to_owned(), res));
        }
        let Some((_, res)) = &self.current else { return };
        let (map, imgs) = match res {
            Ok(v) => v,
            Err(e) => {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
                return;
            }
        };
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.layer, Layer::Heights, "Heights");
            ui.selectable_value(&mut self.layer, Layer::Tiles, "Tile IDs");
            ui.separator();
            ui.checkbox(&mut self.show_a, "mask A (+0xae)");
            ui.checkbox(&mut self.show_b, "mask B (+0x338)");
            ui.checkbox(&mut self.show_pad, "mask cols 50–51");
            ui.separator();
            ui.add(egui::Slider::new(&mut self.cell, 4.0..=24.0).text("cell px"));
        });
        ui.label(format!(
            "heights {}–{}   base {:.3}   extra floats {}   distinct tiles {}",
            imgs.h_range.0,
            imgs.h_range.1,
            map.base,
            map.extra.len(),
            {
                let mut t = map.tiles.clone();
                t.sort_unstable();
                t.dedup();
                t.len()
            }
        ));
        ui.separator();
        let cols = if self.show_pad { MASK_STRIDE * 4 } else { SIDE };
        let size = egui::vec2(cols as f32, SIDE as f32) * self.cell;
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
            let base_rect = egui::Rect::from_min_size(rect.min, egui::vec2(SIDE as f32, SIDE as f32) * self.cell);
            let tex = if self.layer == Layer::Heights { &imgs.heights } else { &imgs.tiles };
            egui::Image::new((tex.id(), base_rect.size())).paint_at(ui, base_rect);
            let p = ui.painter();
            for y in 0..SIDE {
                for x in 0..cols {
                    let r = egui::Rect::from_min_size(
                        rect.min + egui::vec2(x as f32, y as f32) * self.cell,
                        egui::vec2(self.cell, self.cell),
                    );
                    if self.show_a && map.mask_a(x, y) != 0 {
                        p.rect_filled(r.shrink(1.0), 0.0, egui::Color32::from_rgba_unmultiplied(255, 60, 60, 90));
                    }
                    if self.show_b && map.mask_b(x, y) != 0 {
                        p.rect_stroke(r.shrink(2.0), 0.0, (1.5, egui::Color32::from_rgb(80, 160, 255)), egui::StrokeKind::Inside);
                    }
                }
            }
            if let Some(pos) = resp.hover_pos() {
                let c = ((pos - rect.min) / self.cell).floor();
                let (x, y) = (c.x as usize, c.y as usize);
                if x < cols && y < SIDE {
                    let text = if x < SIDE {
                        format!("({x},{y}) h={} tile={} A={} B={}", map.height(x, y), map.tile(x, y), map.mask_a(x, y), map.mask_b(x, y))
                    } else {
                        format!("({x},{y}) A={} B={}", map.mask_a(x, y), map.mask_b(x, y))
                    };
                    resp.on_hover_text(text);
                }
            }
        });
    }
}
