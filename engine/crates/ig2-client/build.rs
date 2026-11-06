//! The program file's icon on Windows: `src/icon.rs`'s drawing through `win_icon.rs`. Without `windres` the window
//! still sets its icon at run time.

#[path = "src/icon.rs"]
#[allow(dead_code)]
mod icon;
#[path = "win_icon.rs"]
mod win_icon;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=win_icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    win_icon::link("openig2", &icon::rgba);
}
