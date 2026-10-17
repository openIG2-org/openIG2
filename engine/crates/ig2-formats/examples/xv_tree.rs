//! Print a model's node tree (children, translations, frames, meshes and their materials):
//! `cargo run --example xv_tree -- <install> <dataraw path of the .xv>`.

use ig2_formats::xv::{Archive, ChildContent, Node};
use ig2_formats::Install;

fn walk(n: &Node, depth: usize) {
    let pad = "  ".repeat(depth);
    println!("{pad}node frames {} origin {:?}", n.header.frames(), n.header.origin);
    for (i, (c, content)) in n.children.iter().enumerate() {
        let t = c.translation();
        let m = c.matrix;
        match content {
            ChildContent::Node(sub) => {
                println!("{pad}[{i}] {} t {t:?} m {m:?}", c.name);
                walk(sub, depth + 1);
            }
            ChildContent::Mesh(mesh) => {
                println!("{pad}[{i}] {} t {t:?} mesh v {} f {} frames {} mats {:?}", c.name, mesh.vertices.len(), mesh.faces.len(), mesh.frames(), mesh.materials);
            }
            ChildContent::Missing => println!("{pad}[{i}] {} missing", c.name),
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let install = Install::open(&a[1], ig2_formats::text::Lang::En).expect("install");
    let bytes = install.read(&a[2]).expect("read");
    let archive = Archive::parse(&bytes).expect("archive");
    let stem = a[2].rsplit('/').next().unwrap().trim_end_matches(".xv");
    for m in archive.members.iter().filter(|m| m.name.to_ascii_lowercase().ends_with(".xvm")) {
        if let Ok(mat) = ig2_formats::xv::Material::parse(m.data) {
            println!("material {}: texture {:?} texture2 {:?} flags {:?}", m.name, mat.texture, mat.texture2, mat.flags);
        }
    }
    match archive.scene(Some(stem)).expect("scene") {
        Some(n) => walk(&n, 0),
        None => println!("no root"),
    }
}
