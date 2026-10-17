//! Print an `.xv` archive's scene tree: per node its frame count, per child its index (`GetSubMeshid`), name and
//! content (mesh: vertices, faces, frames, materials).
//!
//! `cargo run --release -p ig2-formats --example xvtree -- <install> <dataraw/…/name.xv>`

use ig2_formats::xv::{Archive, ChildContent, Mesh, Node};
use ig2_formats::text::Lang;
use ig2_formats::Install;

fn mesh(m: &Mesh) -> String {
    format!("mesh v{} {} verts {} faces {} frames mats {:?}", m.version, m.vertices.len(), m.faces.len(), m.frames(), m.materials)
}

fn walk(n: &Node, depth: usize) {
    let pad = "  ".repeat(depth);
    println!("{pad}node origin {:?} frames {} children {}", n.header.origin, n.header.frames(), n.children.len());
    for (i, (c, content)) in n.children.iter().enumerate() {
        let t = c.translation();
        match content {
            ChildContent::Node(sub) => {
                println!("{pad}[{i}] {} at {t:?}", c.name);
                walk(sub, depth + 1);
            }
            ChildContent::Mesh(m) => println!("{pad}[{i}] {} at {t:?}: {}", c.name, mesh(m)),
            ChildContent::Missing => println!("{pad}[{i}] {} missing", c.name),
        }
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let install = Install::open(&a[1], Lang::En).expect("install");
    let bytes = install.read(&a[2]).expect("read");
    let archive = Archive::parse(&bytes).expect("parse");
    let stem = a[2].rsplit('/').next().unwrap().trim_end_matches(".xv");
    let scene = archive.scene(Some(stem)).expect("scene").expect("root");
    walk(&scene, 0);
}
