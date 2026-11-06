//! The kit's icon files (`gui/icons/*.svg`, ADR 0073): Lucide's and ours in its style, a 24-unit grid stroked 2 wide
//! with round ends and no fills. Read here (paths, circles, ellipses, rects, lines; the path commands Lucide uses) and
//! stroked with the painter in the caller's colour, so an icon takes its key's or button's tint like the text.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

type Line = (Vec<[f32; 2]>, bool);

/// Strokes the icon `svg` into `rect` (its 24 units over the shorter side) in `c`.
pub fn paint(p: &Painter, rect: Rect, svg: &str, c: Color32) {
    let s = rect.width().min(rect.height()) / 24.0;
    let o = rect.center() - Vec2::splat(12.0 * s);
    let at = |q: [f32; 2]| o + Vec2::new(q[0], q[1]) * s;
    let st = Stroke::new((2.0 * s).max(1.0), c);
    for (points, closed) in lines(svg) {
        let pts: Vec<Pos2> = points.iter().map(|&q| at(q)).collect();
        // Round ends and joins: a dot at each corner (a zero-length line is Lucide's dot).
        for q in &pts {
            p.circle_filled(*q, st.width / 2.0, c);
        }
        if pts.len() > 1 {
            p.add(if closed { Shape::closed_line(pts, st) } else { Shape::line(pts, st) });
        }
    }
}

/// The icon's outlines in its 24 units.
fn lines(svg: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for (tag, a) in elements(svg) {
        let f = |n: &str| attr(a, n).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
        match tag {
            "path" => out.extend(path(attr(a, "d").unwrap_or(""))),
            "circle" => out.push((ellipse(f("cx"), f("cy"), f("r"), f("r")), true)),
            "ellipse" => out.push((ellipse(f("cx"), f("cy"), f("rx"), f("ry")), true)),
            "line" => out.push((vec![[f("x1"), f("y1")], [f("x2"), f("y2")]], false)),
            "rect" => {
                let (x, y, w, h, r) = (f("x"), f("y"), f("width"), f("height"), f("rx"));
                let mut v = Vec::new();
                for (cx, cy, a0) in [(x + w - r, y + r, -90.0f32), (x + w - r, y + h - r, 0.0), (x + r, y + h - r, 90.0), (x + r, y + r, 180.0)] {
                    v.extend((0..=6).map(|j| (a0 + 15.0 * j as f32).to_radians()).map(|t| [cx + r * t.cos(), cy + r * t.sin()]));
                }
                out.push((v, true));
            }
            _ => {}
        }
    }
    out
}

/// The elements `<tag …/>` and their attribute text.
fn elements(svg: &str) -> impl Iterator<Item = (&str, &str)> {
    svg.split('<').skip(1).filter_map(|e| {
        let e = e.split('>').next()?;
        let (tag, rest) = e.split_once(' ').unwrap_or((e, ""));
        (!tag.starts_with(['/', '!', '?'])).then_some((tag, rest))
    })
}

/// An attribute's value.
fn attr<'a>(a: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let i = format!(" {a}").find(&key)?;
    let v = &a[i + key.len() - 1..];
    v.split('"').next()
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<[f32; 2]> {
    (0..32).map(|j| j as f32 / 32.0 * std::f32::consts::TAU).map(|t| [cx + rx * t.cos(), cy + ry * t.sin()]).collect()
}

/// A path's subpaths (M, L, H, V, C, S, A, Z and their relative forms) as polylines.
fn path(d: &str) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    let mut cur: Vec<[f32; 2]> = Vec::new();
    let (mut pos, mut start, mut ctrl) = ([0.0f32; 2], [0.0f32; 2], None::<[f32; 2]>);
    let mut tokens = Tokens { s: d.as_bytes(), i: 0 };
    let mut cmd = b'M';
    while let Some(c) = tokens.command_or(cmd) {
        cmd = c;
        let rel = c.is_ascii_lowercase();
        let base = if rel { pos } else { [0.0; 2] };
        let pt = |x: f32, y: f32| [base[0] + x, base[1] + y];
        match c.to_ascii_uppercase() {
            b'M' => {
                let (Some(x), Some(y)) = (tokens.number(), tokens.number()) else { break };
                if cur.len() > 1 {
                    out.push((std::mem::take(&mut cur), false));
                }
                pos = pt(x, y);
                start = pos;
                cur = vec![pos];
                // Further pairs are lines.
                cmd = if rel { b'l' } else { b'L' };
                ctrl = None;
            }
            b'L' => {
                let (Some(x), Some(y)) = (tokens.number(), tokens.number()) else { break };
                pos = pt(x, y);
                cur.push(pos);
                ctrl = None;
            }
            b'H' => {
                let Some(x) = tokens.number() else { break };
                pos = [if rel { pos[0] + x } else { x }, pos[1]];
                cur.push(pos);
                ctrl = None;
            }
            b'V' => {
                let Some(y) = tokens.number() else { break };
                pos = [pos[0], if rel { pos[1] + y } else { y }];
                cur.push(pos);
                ctrl = None;
            }
            b'C' | b'S' => {
                let c1 = if c.eq_ignore_ascii_case(&b'C') {
                    let (Some(x), Some(y)) = (tokens.number(), tokens.number()) else { break };
                    pt(x, y)
                } else {
                    ctrl.map_or(pos, |k| [2.0 * pos[0] - k[0], 2.0 * pos[1] - k[1]])
                };
                let (Some(x2), Some(y2), Some(x), Some(y)) = (tokens.number(), tokens.number(), tokens.number(), tokens.number()) else { break };
                let (c2, end) = (pt(x2, y2), pt(x, y));
                cur.extend((1..=12).map(|j| cubic(pos, c1, c2, end, j as f32 / 12.0)));
                pos = end;
                ctrl = Some(c2);
            }
            b'A' => {
                let (Some(rx), Some(ry), Some(phi), Some(large), Some(sweep), Some(x), Some(y)) = (tokens.number(), tokens.number(), tokens.number(), tokens.flag(), tokens.flag(), tokens.number(), tokens.number()) else { break };
                let end = pt(x, y);
                cur.extend(arc(pos, rx, ry, phi, large, sweep, end));
                pos = end;
                ctrl = None;
            }
            b'Z' => {
                if cur.len() > 1 {
                    out.push((std::mem::take(&mut cur), true));
                }
                pos = start;
                cur = vec![pos];
                ctrl = None;
            }
            _ => break,
        }
    }
    if !cur.is_empty() {
        // A lone point (`M12 8h.01`) stays: it is drawn as Lucide's dot.
        out.push((cur, false));
    }
    out
}

fn cubic(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], t: f32) -> [f32; 2] {
    let u = 1.0 - t;
    let f = |i: usize| u * u * u * p0[i] + 3.0 * u * u * t * p1[i] + 3.0 * u * t * t * p2[i] + t * t * t * p3[i];
    [f(0), f(1)]
}

/// An SVG arc from `p0` to `p1` (the endpoint form, SVG 1.1 F.6.5) as points after `p0`.
fn arc(p0: [f32; 2], rx: f32, ry: f32, phi: f32, large: bool, sweep: bool, p1: [f32; 2]) -> Vec<[f32; 2]> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 || p0 == p1 {
        return vec![p1];
    }
    let (sn, cs) = phi.to_radians().sin_cos();
    let (dx, dy) = ((p0[0] - p1[0]) / 2.0, (p0[1] - p1[1]) / 2.0);
    let (x1, y1) = (cs * dx + sn * dy, -sn * dx + cs * dy);
    let lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lam > 1.0 {
        rx *= lam.sqrt();
        ry *= lam.sqrt();
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let k = (num / den).max(0.0).sqrt() * if large == sweep { -1.0 } else { 1.0 };
    let (cxp, cyp) = (k * rx * y1 / ry, -k * ry * x1 / rx);
    let (cx, cy) = (cs * cxp - sn * cyp + (p0[0] + p1[0]) / 2.0, sn * cxp + cs * cyp + (p0[1] + p1[1]) / 2.0);
    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let (ux, uy, vx, vy) = ((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry);
    let t1 = angle(1.0, 0.0, ux, uy);
    let mut dt = angle(ux, uy, vx, vy);
    if !sweep && dt > 0.0 {
        dt -= std::f32::consts::TAU;
    } else if sweep && dt < 0.0 {
        dt += std::f32::consts::TAU;
    }
    (1..=16)
        .map(|j| t1 + dt * j as f32 / 16.0)
        .map(|t| [cs * rx * t.cos() - sn * ry * t.sin() + cx, sn * rx * t.cos() + cs * ry * t.sin() + cy])
        .collect()
}

/// A path's command letters and numbers.
struct Tokens<'a> {
    s: &'a [u8],
    i: usize,
}

impl Tokens<'_> {
    fn skip(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] == b' ' || self.s[self.i] == b',' || self.s[self.i].is_ascii_whitespace()) {
            self.i += 1;
        }
    }

    /// The next command letter, or `last` again when numbers follow (an implicit repeat); none at the end.
    fn command_or(&mut self, last: u8) -> Option<u8> {
        self.skip();
        let c = *self.s.get(self.i)?;
        if c.is_ascii_alphabetic() {
            self.i += 1;
            Some(c)
        } else {
            Some(last)
        }
    }

    fn number(&mut self) -> Option<f32> {
        self.skip();
        let st = self.i;
        let mut seen_dot = false;
        if matches!(self.s.get(self.i), Some(b'-' | b'+')) {
            self.i += 1;
        }
        while let Some(&c) = self.s.get(self.i) {
            match c {
                b'0'..=b'9' => {}
                b'.' if !seen_dot => seen_dot = true,
                b'e' | b'E' => {
                    self.i += 1;
                    if matches!(self.s.get(self.i), Some(b'-' | b'+')) {
                        self.i += 1;
                    }
                    continue;
                }
                _ => break,
            }
            self.i += 1;
        }
        std::str::from_utf8(&self.s[st..self.i]).ok()?.parse().ok()
    }

    /// An arc flag: one digit, with or without a separator after it.
    fn flag(&mut self) -> Option<bool> {
        self.skip();
        let c = *self.s.get(self.i)?;
        self.i += 1;
        match c {
            b'0' => Some(false),
            b'1' => Some(true),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lucide's info: a closed circle, a line and the dot; the arc and relative forms land where the path says.
    #[test]
    fn reads_lucide_paths() {
        let l = lines(r#"<svg><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>"#);
        assert_eq!(l.len(), 3);
        assert!(l[0].1 && (l[0].0[0][0] - 22.0).abs() < 1e-3);
        assert_eq!(l[1].0, vec![[12.0, 16.0], [12.0, 12.0]]);
        let a = path("M3 6h18M8 6V4c0-1 1-2 2-2h4a2 2 0 0 1 2 2v2");
        assert_eq!(a.len(), 2);
        let end = *a[1].0.last().unwrap();
        assert!((end[0] - 16.0).abs() < 1e-3 && (end[1] - 6.0).abs() < 1e-3, "{end:?}");
        // The arc from (14, 2) to (16, 4) bulges up and right: its middle near (15.41, 2.59).
        let mid = a[1].0[a[1].0.len() - 10];
        assert!(mid[0] > 15.0 && mid[1] < 3.2, "{mid:?}");
        // Every icon file reads.
        for svg in super::super::widgets::ICON_FILES {
            assert!(!lines(svg).is_empty());
        }
    }
}
