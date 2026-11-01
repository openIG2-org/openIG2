//! The browser build (ADR 0047): the page (web/main.js) hands over the install's files from the Origin Private File
//! System, then starts the window on the main menu.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use ig2_formats::vfs::{Files, ReadSeek};
use wasm_bindgen::prelude::*;

/// The install's folder in [`Store`]'s paths.
pub const ROOT: &str = "/install";

/// The install's files the page read, in memory.
/// shortcut: the page reads every file but the videos before the start (about 400 MB); a worker's synchronous OPFS
/// access handles read them on demand once the videos come in (ADR 0047 §2).
pub struct Store(HashMap<PathBuf, Arc<[u8]>>);

impl Files for Store {
    fn scan(&self, root: &Path) -> io::Result<Vec<PathBuf>> {
        Ok(self.0.keys().filter(|p| p.starts_with(root)).cloned().collect())
    }

    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.0.get(path).map(|b| b.to_vec()).ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn open(&self, path: &Path) -> io::Result<Box<dyn ReadSeek>> {
        let b = self.0.get(path).ok_or(io::ErrorKind::NotFound)?;
        Ok(Box::new(io::Cursor::new(b.clone())))
    }
}

/// The files handed over so far; [`start`] moves them into the store.
static ADDED: Mutex<Vec<(PathBuf, Arc<[u8]>)>> = Mutex::new(Vec::new());
static STORE: OnceLock<Arc<Store>> = OnceLock::new();

/// The store the install opens from ([`crate::open_install`]).
pub fn files() -> Arc<dyn Files> {
    STORE.get().expect("web::start sets the store").clone()
}

/// One of the install's files, its path relative to the install's folder (`/`-separated).
#[wasm_bindgen]
pub fn add_file(path: &str, data: Box<[u8]>) {
    ADDED.lock().unwrap().push((Path::new(ROOT).join(path), data.into()));
}

/// The page's folder of the player's own files (settings, saves, replays) in [`ig2_formats::userfs`]'s paths.
pub const USER: &str = "/user";

/// One of the player's own files the page had stored, its path relative to [`USER`].
#[wasm_bindgen]
pub fn add_user_file(path: &str, data: Box<[u8]>) {
    ig2_formats::userfs::insert(Path::new(USER).join(path), data.into());
}

/// Hands a changed or removed file of the player's back to the page, which stores it.
fn user_changed(path: &Path, bytes: Option<&[u8]>) {
    if let Ok(rel) = path.strip_prefix(USER) {
        user_file(&rel.to_string_lossy(), bytes.map(Box::from));
    }
}

static WEBGL_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the page asked for WebGL2 even where WebGPU works (`?webgl`).
pub fn webgl_only() -> bool {
    WEBGL_ONLY.load(std::sync::atomic::Ordering::Relaxed)
}

/// Starts the client on the main menu with the files added; `lang` a language code ("en", "de", …) or the default;
/// `webgl` draws through WebGL2 even where WebGPU works.
#[wasm_bindgen]
pub fn start(lang: Option<String>, webgl: bool) -> Result<(), JsValue> {
    WEBGL_ONLY.store(webgl, std::sync::atomic::Ordering::Relaxed);
    console_error_panic_hook::set_once();
    ig2_formats::userfs::set_sink(user_changed);
    let files = std::mem::take(&mut *ADDED.lock().unwrap());
    STORE.set(Arc::new(Store(files.into_iter().collect()))).map_err(|_| JsValue::from_str("already started"))?;
    let args: Vec<String> = lang.into_iter().flat_map(|l| ["--lang".to_string(), l]).collect();
    crate::parse(&args).and_then(crate::window::run).map_err(|e| JsValue::from_str(&e))
}

#[wasm_bindgen]
extern "C" {
    /// The page's report of a start that failed (web/main.js).
    #[wasm_bindgen(js_name = ig2_failed)]
    pub fn failed(message: &str);

    /// The page's end of a game that quit (the menu's Exit): back to its start (web/main.js).
    #[wasm_bindgen(js_name = ig2_exited)]
    pub fn exited();

    /// The page's store of one of the player's files (web/main.js): its bytes, or `None` when it was removed.
    #[wasm_bindgen(js_name = ig2_user_file)]
    fn user_file(path: &str, data: Option<Box<[u8]>>);
}

/// No clipboard in the browser build yet: `navigator.clipboard` comes with ADR 0047 part 3.
pub mod arboard {
    pub struct Clipboard;

    impl Clipboard {
        pub fn new() -> Result<Self, &'static str> {
            Err("no clipboard in the browser build yet")
        }

        pub fn set_text(&mut self, _: String) -> Result<(), &'static str> {
            Ok(())
        }

        pub fn get_text(&mut self) -> Result<String, &'static str> {
            Ok(String::new())
        }
    }
}
