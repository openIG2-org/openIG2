//! IG2 asset viewer: browse and inspect the user's own install.
//!
//! GUI:      `ig2-viewer [<install-dir>]`  (or set `IG2_INSTALL_DIR`)
//! Headless: `ig2-viewer --snapshot <game-path.xv|.ani> <out.png> [--transpose] [--flip-v]`
//!           renders a model from four angles (or an .ani contact sheet), using the install from `IG2_INSTALL_DIR`.
//!           Output PNGs are derived from game assets: keep them out of the repo.

/// A per-file view cache: the path it was built for, and the decode result.
pub type Cached<T> = Option<(String, Result<T, String>)>;

mod ani_view;
mod app;
mod colony_view;
mod message_view;
mod model_view;
mod scn_view;
mod texture_view;

use ig2_formats::text::Lang;
use ig2_formats::Install;

pub fn write_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(rgba).map_err(|e| e.to_string())
}

fn snapshot_cli(args: &[String]) -> Result<(), String> {
    let [model, out, ..] = args else {
        return Err("usage: --snapshot <game-path.xv> <out.png> [--transpose] [--flip-v]".into());
    };
    let install = Install::from_env(Lang::En).ok_or("set IG2_INSTALL_DIR")?.map_err(|e| e.to_string())?;
    let flag = |f: &str| args.iter().any(|a| a == f);
    let out = std::path::Path::new(out);
    let summary = if model.ends_with(".ani") {
        ani_view::snapshot(&install, model, out)?
    } else {
        model_view::snapshot(&install, model, out, flag("--transpose"), flag("--flip-v"))?
    };
    println!("{summary}");
    Ok(())
}

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--snapshot") {
        if let Err(e) = snapshot_cli(&args[1..]) {
            eprintln!("snapshot failed: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let install = args.first().cloned().or_else(|| std::env::var(ig2_formats::vfs::INSTALL_ENV).ok());
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("IG2 asset viewer")
            .with_inner_size([1400.0, 900.0]),
        ..Default::default()
    };
    eframe::run_native("ig2-viewer", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, install)))))
}
