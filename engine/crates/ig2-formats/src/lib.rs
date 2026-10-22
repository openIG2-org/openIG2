//! Readers for Imperium Galactica II data files as shipped in the GOG remaster.
//!
//! This crate contains no game data. Everything is read from the user's own install at runtime.
//! Format specs live in `re/formats/` and cite the recovered functions in the shipped binaries.

pub mod ani;
pub mod colony_map;
pub mod container;
pub mod dialogue;
pub mod diplomacy;
pub mod formation;
pub mod locale;
pub mod mp4;
pub mod misc_text;
pub mod options;
pub mod pvr;
pub mod save;
pub mod scn;
pub mod story_data;
pub mod strings;
pub mod text;
pub mod tutorial;
pub mod vfs;
pub mod xv;

pub use container::{decode as decode_container, ContainerKind};
pub use vfs::Install;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o error on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("file not found in install: {0}")]
    NotFound(String),
    #[error("{format}: {msg}")]
    Format { format: &'static str, msg: String },
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn format_err(format: &'static str, msg: impl Into<String>) -> Error {
    Error::Format { format, msg: msg.into() }
}
