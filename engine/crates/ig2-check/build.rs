//! The check tool's program icon on Windows: the game's ringed planet (`ig2-client/src/icon.rs`) with a green badge
//! and a white tick in the lower right, linked in through `ig2-client/win_icon.rs`.

#[path = "../ig2-client/src/icon.rs"]
#[allow(dead_code)]
mod icon;
#[path = "../ig2-client/win_icon.rs"]
mod win_icon;

/// Distance from (px, py) to the segment a–b.
fn seg(px: f32, py: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((px - a.0) * dx + (py - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    ((px - a.0 - t * dx).powi(2) + (py - a.1 - t * dy).powi(2)).sqrt()
}

/// The badge at (u, v) in 0..1: RGBA, alpha 0 outside it.
fn badge(u: f32, v: f32) -> [f32; 4] {
    let r = ((u - 0.72).powi(2) + (v - 0.72).powi(2)).sqrt();
    if r > 0.27 {
        return [0.0; 4];
    }
    if r > 0.23 {
        // A dark rim parts the badge from the planet.
        return [0.04, 0.06, 0.12, 1.0];
    }
    let d = seg(u, v, (0.6, 0.73), (0.69, 0.82)).min(seg(u, v, (0.69, 0.82), (0.85, 0.62)));
    if d < 0.035 {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let k = 1.0 - 0.5 * r / 0.23;
    [0.15 * k, 0.75 * k, 0.3 * k, 1.0]
}

/// The planet with the badge over it, `size` × `size` RGBA (4 × 4 samples a badge pixel).
fn rgba(size: u32) -> Vec<u8> {
    let mut px = icon::rgba(size);
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0.0f32; 4];
            for s in 0..16 {
                let u = (x as f32 + (s % 4) as f32 / 4.0 + 0.125) / size as f32;
                let v = (y as f32 + (s / 4) as f32 / 4.0 + 0.125) / size as f32;
                let c = badge(u, v);
                for i in 0..3 {
                    acc[i] += c[i] * c[3] / 16.0;
                }
                acc[3] += c[3] / 16.0;
            }
            let i = ((y * size + x) * 4) as usize;
            let a = acc[3];
            let under = px[i + 3] as f32 / 255.0;
            let out_a = a + under * (1.0 - a);
            for c in 0..3 {
                let v = acc[c] + px[i + c] as f32 / 255.0 * under * (1.0 - a);
                px[i + c] = if out_a > 0.0 { (v / out_a * 255.0 + 0.5) as u8 } else { 0 };
            }
            px[i + 3] = (out_a * 255.0 + 0.5) as u8;
        }
    }
    px
}

fn main() {
    println!("cargo:rerun-if-changed=../ig2-client/src/icon.rs");
    println!("cargo:rerun-if-changed=../ig2-client/win_icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    win_icon::link("openig2-check", &rgba);
}
