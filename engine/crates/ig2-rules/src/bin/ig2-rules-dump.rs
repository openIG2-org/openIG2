//! Dump the rules tables read from your own ig2.exe to RON, for inspection and modding.
//! Usage: ig2-rules-dump <install-dir> <out.ron>
//! The output contains values from the game: keep it local, never commit or distribute it.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [install, out] = &args[..] else {
        eprintln!("usage: ig2-rules-dump <install-dir> <out.ron>");
        std::process::exit(2);
    };
    let inst = ig2_formats::Install::open(install, ig2_formats::text::Lang::En).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let rules = ig2_rules::Rules::from_install(&inst).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    std::fs::write(out, rules.to_ron()).expect("write output");
    println!("wrote {out}: {} techs, {} buildings, {} pod types", rules.techs.len(), rules.buildings.len(), rules.pod_types.len());
}
