//! Dump the rules tables read from your own ig2.exe to RON, for inspection and modding (as `openIG2-check
//! --dump-rules`). Usage: ig2-rules-dump <install-dir> <out.ron>
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
    match ig2_rules::Rules::dump(&inst, out.as_ref()) {
        Ok(said) => println!("{said}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
}
