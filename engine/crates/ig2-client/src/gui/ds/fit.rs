//! Text that fits a fixed box (ADR 0073 §3): one line on the box's baseline (the capitals centred, so rows of one
//! style share a baseline), never wider than the box, cut with "…" or shrunk down to a floor, the whole text as a
//! tooltip where it is cut; or wrapped body text with a cap on its lines. A long translation never resizes a panel:
//! the box is the caller's, the text adapts to it.

use super::{tokens, Font};
use egui::{Align, Color32, Pos2, Rect, Response, Sense, Ui, Vec2};
use std::sync::Arc;

/// What a line too long for its box does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Overflow {
    /// Cut with "…".
    Elide,
    /// Smaller, down to this size in points, then cut.
    Shrink(f32),
}

/// A text laid out for its box.
#[derive(Debug, Clone)]
pub struct Fitted {
    pub galley: Arc<egui::Galley>,
    /// The style it ended at (smaller where it shrank).
    pub font: Font,
    /// Not all of the text shows.
    pub cut: bool,
    /// The drawn width, without the last letter's spacing.
    pub width: f32,
}

impl Fitted {
    /// The first line's baseline below the galley's top.
    pub fn baseline(&self) -> f32 {
        self.galley.rows.first().map_or(self.font.size * 0.8, |r| r.pos.y + r.row.glyphs.first().map_or(self.font.size * 0.8, |g| g.pos.y))
    }
}

/// A text in a style, for a box: `Text::new("Felbontás", Font::BODY).right().show(ui, size)`.
#[derive(Debug, Clone)]
pub struct Text<'a> {
    text: &'a str,
    font: Font,
    color: Option<Color32>,
    align: Align,
    overflow: Overflow,
    lines: usize,
    baseline: Option<f32>,
    upright: bool,
}

impl<'a> Text<'a> {
    pub fn new(text: &'a str, font: Font) -> Self {
        Self { text, font, color: None, align: Align::Min, overflow: Overflow::Elide, lines: 1, baseline: None, upright: false }
    }
    pub fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
    pub fn center(self) -> Self {
        self.align(Align::Center)
    }
    pub fn right(self) -> Self {
        self.align(Align::Max)
    }
    /// Shrink a long line down to `min` points before cutting it.
    pub fn shrink(mut self, min: f32) -> Self {
        self.overflow = Overflow::Shrink(min);
        self
    }
    /// Wrap into at most `n` lines (the last cut with "…"), from the box's top; left aligned.
    pub fn lines(mut self, n: usize) -> Self {
        self.lines = n.max(1);
        self
    }
    /// Put a single line's baseline at `y` instead of centring it (a row of mixed styles on one baseline).
    pub fn baseline(mut self, y: f32) -> Self {
        self.baseline = Some(y);
        self
    }

    /// The LCD face without its slant (the boards' table numbers, `.num`).
    pub fn upright(mut self) -> Self {
        self.upright = true;
        self
    }

    fn job(&self, font: Font, width: Option<f32>) -> egui::text::LayoutJob {
        let mut job = font.job(self.text, self.color.unwrap_or_else(|| tokens().ink));
        if let Some(w) = width {
            // The last letter's spacing hangs past the box.
            job.wrap = egui::text::TextWrapping { max_width: w + font.spacing(), max_rows: self.lines, break_anywhere: self.lines == 1, overflow_character: Some('…') };
        }
        job
    }

    /// The text laid out for a box `width` points wide.
    pub fn layout(&self, ctx: &egui::Context, width: f32) -> Fitted {
        let lay = |font: Font, wrap: Option<f32>| {
            let galley = ctx.fonts(|f| f.layout_job(self.job(font, wrap)));
            let width = (galley.size().x - if galley.is_empty() { 0.0 } else { font.spacing() }).max(0.0);
            Fitted { cut: galley.elided, galley, font, width }
        };
        if self.lines > 1 {
            return lay(self.font, Some(width));
        }
        let mut font = self.font;
        loop {
            let whole = lay(font, None);
            if whole.width <= width + 0.01 {
                return whole;
            }
            match self.overflow {
                Overflow::Shrink(min) if font.size - 0.5 >= min - 1e-3 => font = font.sized(font.size - 0.5),
                _ => return lay(font, Some(width)),
            }
        }
    }

    /// Draw the text in `rect` (nothing allocated); the whole text as a tooltip over it where it is cut.
    pub fn paint(&self, ui: &Ui, rect: Rect) -> Fitted {
        let f = self.paint_only(ui, rect);
        if f.cut {
            let id = ui.id().with(("ig2-fit", rect.min.x.to_bits(), rect.min.y.to_bits()));
            ui.interact(rect, id, Sense::hover()).on_hover_text(self.text);
        }
        f
    }

    /// [`Self::paint`] without the tooltip (a widget showing its own over its whole area).
    pub fn paint_only(&self, ui: &Ui, rect: Rect) -> Fitted {
        let f = self.layout(ui.ctx(), rect.width());
        let pos = self.place(&f, rect);
        let ppp = ui.ctx().pixels_per_point();
        let pos = Pos2::new((pos.x * ppp).round() / ppp, (pos.y * ppp).round() / ppp);
        let painter = ui.painter().with_clip_rect(rect.expand2(Vec2::new(f.font.size * 0.3, f.font.line)).intersect(ui.clip_rect()));
        if f.font.lcd && !self.upright {
            super::sheared_galley(&painter, pos, &f.galley, 0.2);
        } else {
            painter.galley(pos, f.galley.clone(), Color32::WHITE);
        }
        f
    }

    /// Where the galley's top left goes in `rect`.
    fn place(&self, f: &Fitted, rect: Rect) -> Pos2 {
        let x = match self.align {
            Align::Min => rect.left(),
            Align::Center => rect.center().x - f.width / 2.0,
            Align::Max => rect.right() - f.width,
        };
        if self.lines > 1 {
            return Pos2::new(rect.left(), rect.top());
        }
        let base = self.baseline.unwrap_or_else(|| baseline(rect, f.font));
        Pos2::new(x, base - f.baseline())
    }

    /// Allocate exactly `size` (never more, whatever the text) and draw the text in it.
    pub fn show(self, ui: &mut Ui, size: Vec2) -> Response {
        let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
        let f = self.paint_only(ui, rect);
        if f.cut { resp.on_hover_text(self.text) } else { resp }
    }
}

/// The baseline that centres `font`'s capitals in `rect` (every row of one height and style shares it).
pub fn baseline(rect: Rect, font: Font) -> f32 {
    rect.center().y + font.cap() / 2.0
}

/// `text` in `font` in `rect`, cut to fit: the short form of [`Text`].
pub fn text(ui: &Ui, rect: Rect, s: &str, font: Font, color: Color32) -> Fitted {
    Text::new(s, font).color(color).paint(ui, rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The last letter shown.
    fn last(f: &Fitted) -> Option<char> {
        f.galley.rows.last().and_then(|r| r.row.glyphs.last()).map(|g| g.chr)
    }

    fn ctx() -> egui::Context {
        let ctx = crate::gui::Gui::new(wgpu::TextureFormat::Bgra8UnormSrgb).ctx;
        let _ = ctx.run(Default::default(), |_| {});
        ctx
    }

    /// A short line keeps its size and shows whole; a long one is cut with "…" inside the box, or shrinks to its floor
    /// first; a box never takes the text's size.
    #[test]
    fn lines_fit_their_box() {
        let ctx = ctx();
        let short = Text::new("Gamma", Font::BODY).layout(&ctx, 200.0);
        assert!(!short.cut && short.width < 200.0 && short.width > 20.0);
        let long = "Pixelenkénti megvilágítás és lágy árnyékok a kolónián";
        let cut = Text::new(long, Font::BODY).layout(&ctx, 160.0);
        assert!(cut.cut, "{}", cut.galley.text());
        assert!(cut.width <= 160.0 + 0.01, "{}", cut.width);
        assert_eq!(last(&cut), Some('…'));
        assert_eq!(cut.galley.rows.len(), 1);
        let shrunk = Text::new("Textúra-felskálázás", Font::BODY).shrink(13.0).layout(&ctx, Text::new("Textúra-felskálázás", Font::BODY).layout(&ctx, 1e4).width * 0.85);
        assert!(!shrunk.cut && shrunk.font.size < 17.0 && shrunk.font.size >= 13.0, "{:?}", shrunk.font);
        let floor = Text::new(long, Font::BODY).shrink(13.0).layout(&ctx, 120.0);
        assert!(floor.cut && floor.font.size == 13.0 && floor.width <= 120.01);
        // The same text in an allocated box: the box keeps its size.
        let mut size = Vec2::ZERO;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| size = Text::new(long, Font::HEADING).show(ui, Vec2::new(90.0, 30.0)).rect.size());
        });
        assert_eq!(size, Vec2::new(90.0, 30.0));
    }

    /// Wrapped text stops at its line cap with "…".
    #[test]
    fn wrapped_text_caps_its_lines() {
        let ctx = ctx();
        let body = "Keret nélküli ablak az asztal felbontásán: gyors váltás más programokra, a játék mégis kitölti a képernyőt, és a kép éles marad minden felbontáson.";
        let f = Text::new(body, Font::BODY).lines(2).layout(&ctx, 300.0);
        assert!(f.cut && f.galley.rows.len() == 2 && last(&f) == Some('…'), "{} {:?}", f.galley.rows.len(), last(&f));
        assert!(f.galley.rows.iter().all(|r| r.rect().width() <= 300.0 + Font::BODY.spacing() + 0.01));
        let all = Text::new(body, Font::BODY).lines(9).layout(&ctx, 300.0);
        assert!(!all.cut && all.galley.rows.len() > 2);
    }

    /// Rows of one height and style share a baseline whatever their text (accents, descenders, a cut line), and a
    /// label and a value of different sizes meet on one when given it.
    #[test]
    fn rows_share_a_baseline() {
        let ctx = ctx();
        let mut bases = Vec::new();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for (i, s) in ["Gamma", "Függőleges szinkron", "ÁÉŐŰ jpqy", "Pixelenkénti megvilágítás nagyon hosszú szöveggel"].iter().enumerate() {
                    let rect = Rect::from_min_size(Pos2::new(10.0, 10.0 + 50.0 * i as f32), Vec2::new(160.0, 50.0));
                    let t = Text::new(s, Font::BODY);
                    let f = t.paint(ui, rect);
                    bases.push(t.place(&f, rect).y + f.baseline() - rect.top());
                }
                let rect = Rect::from_min_size(Pos2::new(10.0, 300.0), Vec2::new(300.0, 50.0));
                let y = baseline(rect, Font::BODY);
                let (a, b) = (Text::new("Típus", Font::LABEL).baseline(y), Text::new("Romboló", Font::DATA).baseline(y));
                let (fa, fb) = (a.layout(ctx, 100.0), b.layout(ctx, 100.0));
                assert!((a.place(&fa, rect).y + fa.baseline() - y).abs() < 0.01);
                assert!((b.place(&fb, rect).y + fb.baseline() - y).abs() < 0.01);
            });
        });
        assert!(bases.iter().all(|b| (b - bases[0]).abs() < 0.01), "{bases:?}");
        // The capitals sit in the middle of the row.
        let cap = Font::BODY.cap();
        assert!((bases[0] - (25.0 + cap / 2.0)).abs() < 0.01);
    }
}
