//! Capture comparison for the 1:1 client (ADR 0009 §4): compares a capture of ig2.exe with an `ig2-client
//! --snapshot` of the same screen and reports the pixel difference, for the whole frame and per region.
//!
//! `ig2-screendiff <exe.png> <client.png> [--region NAME=X,Y,W,H]… [--mask X,Y,W,H]… [--shift N]
//!                 [--threshold T] [--heat out.png] [--json]`
//!
//! Regions and masks are in the original's 640 × 480 virtual pixels, placed in the capture's virtual frame by the
//! remaster's widescreen rule (re/screens/widescreen.md §2: the centred 4:3 frame; wider captures also get the
//! regions `left_strip`/`right_strip` beside it). For
//! every region the tool searches the offset `(dx, dy)` within ±`shift` output pixels (default 2) that makes the
//! client's picture closest to the exe's, and reports the mean difference at offset 0 and at the best offset
//! (per pixel: the mean of |ΔR|, |ΔG|, |ΔB|, 0..255) and the share of pixels whose largest channel differs by more
//! than 32. Verdict: `match` when the best mean is at most `threshold` (default 6) and the offset is within one
//! virtual pixel, `offset` when only the offset is too large, `differs` otherwise. Masked rectangles (animated or
//! random parts) are left out everywhere. `--heat` writes the exe's picture dimmed with the difference at offset 0
//! in red. The images hold game imagery: keep them under `_local/`.

use std::path::Path;

struct Image {
    w: usize,
    h: usize,
    rgb: Vec<u8>,
}

fn load(path: &str) -> Result<Image, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{path}: {e}"))?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().map_err(|e| format!("{path}: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| format!("{path}: {e}"))?;
    let (w, h) = (info.width as usize, info.height as usize);
    let n = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Indexed => return Err(format!("{path}: indexed colour after expansion")),
    };
    let mut rgb = Vec::with_capacity(w * h * 3);
    for px in buf[..w * h * n].chunks_exact(n) {
        match n {
            1 | 2 => rgb.extend_from_slice(&[px[0]; 3]),
            _ => rgb.extend_from_slice(&px[..3]),
        }
    }
    Ok(Image { w, h, rgb })
}

#[derive(Clone)]
struct Rect {
    name: String,
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

/// Where the 640 × 480 virtual frame sits in a `w` × `h` capture (`ig2_resize`, re/screens/widescreen.md §2):
/// `(x, y, k)`, the frame's corner in pixels and the pixels per virtual pixel. Wider than 4:3 the frame is the centred
/// `H·640/480` columns; narrower, the centred `W·480/640` band.
fn frame_of(w: usize, h: usize) -> (f32, f32, f32) {
    if w * 3 >= h * 4 {
        let fw = h * 640 / 480;
        (((w - fw) / 2) as f32, 0.0, h as f32 / 480.0)
    } else {
        let fh = w * 480 / 640;
        (0.0, ((h - fh) / 2) as f32, w as f32 / 640.0)
    }
}

fn rect(spec: &str, f: (f32, f32, f32), w: usize, h: usize) -> Result<Rect, String> {
    let (name, nums) = match spec.split_once('=') {
        Some((n, r)) => (n.to_string(), r),
        None => (String::new(), spec),
    };
    let v: Vec<f32> = nums.split(',').map(|x| x.trim().parse::<f32>().map_err(|e| format!("{spec}: {e}"))).collect::<Result<_, _>>()?;
    let [x, y, rw, rh] = v[..] else { return Err(format!("{spec}: X,Y,W,H")) };
    let (fx, fy, k) = f;
    let px = |v: f32, o: f32, max: usize| ((o + v * k).round().max(0.0) as usize).min(max);
    Ok(Rect { name, x0: px(x, fx, w), y0: px(y, fy, h), x1: px(x + rw, fx, w), y1: px(y + rh, fy, h) })
}

struct Score {
    mean0: f64,
    best: (i32, i32),
    mean: f64,
    over32: f64,
    pixels: usize,
    luma_exe: f64,
    luma_client: f64,
}

fn score(a: &Image, b: &Image, r: &Rect, masked: &[bool], shift: i32) -> Score {
    let px = |img: &Image, x: usize, y: usize| {
        let i = (y * img.w + x) * 3;
        [img.rgb[i] as i32, img.rgb[i + 1] as i32, img.rgb[i + 2] as i32]
    };
    let measure = |dx: i32, dy: i32| {
        let (mut sum, mut over, mut n) = (0u64, 0usize, 0usize);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                if masked[y * a.w + x] {
                    continue;
                }
                let (bx, by) = (x as i32 + dx, y as i32 + dy);
                if bx < 0 || by < 0 || bx >= b.w as i32 || by >= b.h as i32 {
                    continue;
                }
                let (p, q) = (px(a, x, y), px(b, bx as usize, by as usize));
                let d = [(p[0] - q[0]).abs(), (p[1] - q[1]).abs(), (p[2] - q[2]).abs()];
                sum += (d[0] + d[1] + d[2]) as u64;
                over += (d.iter().max().copied().unwrap_or(0) > 32) as usize;
                n += 1;
            }
        }
        let n1 = n.max(1) as f64;
        (sum as f64 / 3.0 / n1, over as f64 / n1, n)
    };
    let (mean0, _, _) = measure(0, 0);
    // The closest offset; on a tie the one nearest to (0, 0).
    let mut best = (f64::INFINITY, i32::MAX, (0, 0), 0.0, 0);
    for dy in -shift..=shift {
        for dx in -shift..=shift {
            let (m, o, n) = measure(dx, dy);
            let dist = dx.abs() + dy.abs();
            if m < best.0 - 1e-9 || (m < best.0 + 1e-9 && dist < best.1) {
                best = (m, dist, (dx, dy), o, n);
            }
        }
    }
    let (bm, _, best, bo, bn) = best;
    let luma = |img: &Image| {
        let (mut s, mut n) = (0u64, 0u64);
        for y in r.y0..r.y1 {
            for x in r.x0..r.x1 {
                if !masked[y * a.w + x] {
                    let p = px(img, x, y);
                    s += (p[0] + p[1] + p[2]) as u64;
                    n += 3;
                }
            }
        }
        s as f64 / n.max(1) as f64
    };
    Score { mean0, best, mean: bm, over32: bo, pixels: bn, luma_exe: luma(a), luma_client: luma(b) }
}

fn write_heat(path: &Path, a: &Image, b: &Image, masked: &[bool]) -> Result<(), String> {
    let mut out = Vec::with_capacity(a.w * a.h * 3);
    for y in 0..a.h {
        for x in 0..a.w {
            let i = (y * a.w + x) * 3;
            let g = (a.rgb[i] as u32 + a.rgb[i + 1] as u32 + a.rgb[i + 2] as u32) / 3 * 35 / 100;
            if masked[y * a.w + x] {
                out.extend_from_slice(&[0, 0, (g + 60).min(255) as u8]);
                continue;
            }
            let d = if x < b.w && y < b.h {
                let j = (y * b.w + x) * 3;
                (0..3).map(|c| (a.rgb[i + c] as i32 - b.rgb[j + c] as i32).unsigned_abs()).sum::<u32>() / 3
            } else {
                255
            };
            out.extend_from_slice(&[(g + d * 4).min(255) as u8, g as u8, g as u8]);
        }
    }
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), a.w as u32, a.h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(&out).map_err(|e| e.to_string())
}

fn run(args: &[String]) -> Result<bool, String> {
    let [exe, client, rest @ ..] = args else {
        return Err("usage: ig2-screendiff <exe.png> <client.png> [--region NAME=X,Y,W,H]… [--mask X,Y,W,H]… [--shift N] [--threshold T] [--heat out.png] [--json]".into());
    };
    let (a, b) = (load(exe)?, load(client)?);
    if (a.w, a.h) != (b.w, b.h) {
        return Err(format!("sizes differ: exe {}x{}, client {}x{}", a.w, a.h, b.w, b.h));
    }
    let f = frame_of(a.w, a.h);
    let k = f.2;
    let (mut regions, mut masks) = (vec![Rect { name: "frame".into(), x0: 0, y0: 0, x1: a.w, y1: a.h }], vec![]);
    // Wider than 4:3: the strips beside the frame (the 3D scenes and stretched pictures, widescreen.md §3).
    let fx = f.0 as usize;
    if fx > 0 {
        regions.push(Rect { name: "left_strip".into(), x0: 0, y0: 0, x1: fx, y1: a.h });
        regions.push(Rect { name: "right_strip".into(), x0: a.w - fx, y0: 0, x1: a.w, y1: a.h });
    }
    let (mut shift, mut threshold, mut heat, mut json) = (2, 6.0, None, false);
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        let mut val = || it.next().cloned().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--region" => regions.push(rect(&val()?, f, a.w, a.h)?),
            "--mask" => masks.push(rect(&val()?, f, a.w, a.h)?),
            "--shift" => shift = val()?.parse().map_err(|e| format!("--shift: {e}"))?,
            "--threshold" => threshold = val()?.parse().map_err(|e| format!("--threshold: {e}"))?,
            "--heat" => heat = Some(val()?),
            "--json" => json = true,
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    let mut masked = vec![false; a.w * a.h];
    for m in &masks {
        for y in m.y0..m.y1 {
            masked[y * a.w + m.x0..y * a.w + m.x1].iter_mut().for_each(|v| *v = true);
        }
    }
    // One virtual pixel of tolerance.
    let tol = k.ceil() as i32;
    let mut all = true;
    let mut rows = vec![];
    for r in &regions {
        let s = score(&a, &b, r, &masked, shift);
        let verdict = if s.mean > threshold {
            "differs"
        } else if s.best.0.abs() > tol || s.best.1.abs() > tol {
            "offset"
        } else {
            "match"
        };
        all &= verdict == "match";
        if json {
            rows.push(format!(
                "{{\"region\":\"{}\",\"rect\":[{},{},{},{}],\"mean0\":{:.2},\"dx\":{},\"dy\":{},\"mean\":{:.2},\"over32\":{:.4},\"pixels\":{},\"luma_exe\":{:.1},\"luma_client\":{:.1},\"verdict\":\"{verdict}\"}}",
                r.name, r.x0, r.y0, r.x1 - r.x0, r.y1 - r.y0, s.mean0, s.best.0, s.best.1, s.mean, s.over32, s.pixels, s.luma_exe, s.luma_client
            ));
        } else {
            println!(
                "{:<14} {:>4},{:<4} {:>4}x{:<4} mean@0 {:6.2}  best ({:+},{:+}) {:6.2}  >32: {:5.1}%  luma exe {:5.1} client {:5.1}  {verdict}",
                r.name, r.x0, r.y0, r.x1 - r.x0, r.y1 - r.y0, s.mean0, s.best.0, s.best.1, s.mean, s.over32 * 100.0, s.luma_exe, s.luma_client
            );
        }
    }
    if json {
        println!("[{}]", rows.join(","));
    }
    if let Some(h) = heat {
        write_heat(Path::new(&h), &a, &b, &masked)?;
    }
    Ok(all)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(all) => std::process::exit(if all { 0 } else { 1 }),
        Err(e) => {
            eprintln!("ig2-screendiff: {e}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: usize, h: usize, f: impl Fn(usize, usize) -> u8) -> Image {
        let mut rgb = vec![];
        for y in 0..h {
            for x in 0..w {
                rgb.extend_from_slice(&[f(x, y); 3]);
            }
        }
        Image { w, h, rgb }
    }

    #[test]
    fn finds_the_offset_of_a_shifted_picture() {
        let a = img(64, 48, |x, y| ((x * 7 + y * 13) % 251) as u8);
        let b = img(64, 48, |x, y| ((x.saturating_sub(1) * 7 + y.saturating_sub(2) * 13) % 251) as u8);
        let r = Rect { name: "r".into(), x0: 8, y0: 8, x1: 56, y1: 40 };
        let s = score(&a, &b, &r, &vec![false; 64 * 48], 2);
        assert_eq!(s.best, (1, 2));
        assert_eq!(s.mean, 0.0);
        assert!(s.mean0 > 10.0);
    }

    #[test]
    fn regions_sit_in_the_virtual_frame() {
        // widescreen.md §2's observed IG2_VIRTUAL_* (x, y, k = width / 640).
        for (w, h, x, y, fw) in [(800, 600, 0, 0, 800), (1920, 1080, 240, 0, 1440), (2520, 1080, 540, 0, 1440),
            (1366, 768, 171, 0, 1024), (1280, 1024, 0, 32, 1280), (1000, 1000, 0, 125, 1000), (3840, 2160, 480, 0, 2880)] {
            let f = frame_of(w, h);
            assert_eq!((f.0, f.1, f.2), (x as f32, y as f32, fw as f32 / 640.0), "{w}x{h}");
        }
        let r = rect("r=0,0,640,480", frame_of(1920, 1080), 1920, 1080).unwrap();
        assert_eq!((r.x0, r.y0, r.x1, r.y1), (240, 0, 1680, 1080));
    }

    #[test]
    fn masks_leave_pixels_out() {
        let a = img(10, 10, |_, _| 0);
        let b = img(10, 10, |x, _| if x < 5 { 255 } else { 0 });
        let r = Rect { name: "r".into(), x0: 0, y0: 0, x1: 10, y1: 10 };
        let mut m = vec![false; 100];
        (0..10).for_each(|y| (0..5).for_each(|x| m[y * 10 + x] = true));
        let s = score(&a, &b, &r, &m, 0);
        assert_eq!((s.mean, s.pixels), (0.0, 50));
    }
}
