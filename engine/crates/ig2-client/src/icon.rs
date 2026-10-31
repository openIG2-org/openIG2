//! The remake's own icon (window, taskbar, Alt+Tab and, through `build.rs`, the program file): a ringed planet on a
//! disc of deep space, drawn here rather than taken from the original's files. `build.rs` includes this file, so it
//! uses std alone.

/// The icon as `size` × `size` RGBA, rows top-down, straight alpha (4 × 4 samples a pixel).
pub fn rgba(size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0.0f32; 4];
            for s in 0..16 {
                let u = (x as f32 + (s % 4) as f32 / 4.0 + 0.125) / size as f32;
                let v = (y as f32 + (s / 4) as f32 / 4.0 + 0.125) / size as f32;
                let c = sample(u, v);
                for i in 0..3 {
                    acc[i] += c[i] * c[3];
                }
                acc[3] += c[3];
            }
            let a = acc[3] / 16.0;
            for i in 0..3 {
                let c = if acc[3] > 0.0 { acc[i] / acc[3] } else { 0.0 };
                out.push((c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8);
            }
            out.push((a * 255.0 + 0.5) as u8);
        }
    }
    out
}

/// The tilted ring's ellipse: (x, y) relative to the planet's centre turned into the ring's frame.
fn ring_frame(x: f32, y: f32) -> (f32, f32) {
    let (s, c) = (-0.38f32).sin_cos();
    (x * c - y * s, x * s + y * c)
}

/// One sample at (u, v) in 0..1, RGBA.
fn sample(u: f32, v: f32) -> [f32; 4] {
    let (dx, dy) = (u - 0.5, v - 0.5);
    let r = (dx * dx + dy * dy).sqrt();
    // The ring's band: between two ellipses (a 0.44 / b 0.13 and 0.33 / 0.085) around the planet.
    let (px, py) = (u - 0.5, v - 0.53);
    let (rx, ry) = ring_frame(px, py);
    let outer = (rx / 0.45).powi(2) + (ry / 0.13).powi(2);
    let inner = (rx / 0.34).powi(2) + (ry / 0.09).powi(2);
    let ring = outer <= 1.0 && inner >= 1.0;
    let ring_col = {
        let t = ((outer.sqrt() - 0.76) / 0.24).clamp(0.0, 1.0);
        [0.55 + 0.35 * t, 0.85 + 0.1 * t, 1.0, 1.0]
    };
    let planet_r = 0.24;
    let pr = (px * px + py * py).sqrt();
    // The ring's front half (below the planet's centre in its frame) is over the planet.
    if ring && ry > 0.0 {
        return ring_col;
    }
    if pr <= planet_r {
        // Lit from the upper left: a sphere's normal against the light, with bands of the gas giant's colour.
        let (nx, ny) = (px / planet_r, py / planet_r);
        let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
        let l = (-0.55 * nx - 0.6 * ny + 0.58 * nz).max(0.0);
        let band = 0.5 + 0.5 * ((ny * 1.0 + 0.15 * nx) * 13.0).sin();
        let base = [0.95 - 0.12 * band, 0.55 - 0.15 * band, 0.18 + 0.04 * band];
        let k = 0.12 + 0.95 * l;
        return [base[0] * k, base[1] * k, base[2] * k, 1.0];
    }
    if ring {
        return ring_col;
    }
    if r <= 0.48 {
        // Deep space, lighter towards the centre, and a few stars.
        let t = r / 0.48;
        let mut c = [0.08 + 0.10 * (1.0 - t), 0.10 + 0.16 * (1.0 - t), 0.25 + 0.30 * (1.0 - t)];
        for (sx, sy, sr) in [(0.24, 0.24, 0.018), (0.74, 0.2, 0.014), (0.8, 0.72, 0.016), (0.28, 0.8, 0.012), (0.6, 0.12, 0.01)] {
            if ((u - sx).powi(2) + (v - sy).powi(2)).sqrt() <= sr {
                c = [1.0, 1.0, 0.92];
            }
        }
        // The rim: a thin lighter edge.
        if r > 0.455 {
            c = [0.45, 0.6, 0.9];
        }
        return [c[0], c[1], c[2], 1.0];
    }
    [0.0; 4]
}

#[cfg(test)]
mod tests {
    #[test]
    fn opaque_inside_clear_outside() {
        let px = super::rgba(32);
        assert_eq!(px.len(), 32 * 32 * 4);
        // The corner is outside the disc; the middle is the planet.
        assert_eq!(px[3], 0);
        let mid = (16 * 32 + 16) * 4;
        assert_eq!(px[mid + 3], 255);
    }
}
