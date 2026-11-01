//! Where the output goes on Windows. Release builds use the windows subsystem (no console window beside the game,
//! docs/USER_GUIDE.md → Running), so they start with no stdout or stderr unless the parent handed some over (a pipe,
//! a file: scripts and tests read them as before). A missing one goes to the console of the terminal that started
//! the client, else to `ig2-client.log` in the settings folder, emptied each start; a panic's message goes with
//! stderr.

use std::fs::{File, OpenOptions};
use std::os::windows::io::IntoRawHandle;

type Handle = *mut core::ffi::c_void;

const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
const STD_ERROR_HANDLE: u32 = -12i32 as u32;
const ATTACH_PARENT_PROCESS: u32 = u32::MAX;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(which: u32) -> Handle;
    fn SetStdHandle(which: u32, h: Handle) -> i32;
    fn AttachConsole(pid: u32) -> i32;
}

/// The log file's name, beside the settings files.
pub const LOG_FILE: &str = "ig2-client.log";

pub fn route() {
    let missing = |which| {
        let h = unsafe { GetStdHandle(which) };
        h.is_null() || h as isize == -1
    };
    let to_set: Vec<u32> = [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE].into_iter().filter(|&w| missing(w)).collect();
    if to_set.is_empty() {
        return;
    }
    // Read access too: std asks the console's mode before writing to it (UTF-8 then comes out right).
    let out = if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } != 0 {
        OpenOptions::new().read(true).write(true).open("CONOUT$").ok()
    } else {
        log_file()
    };
    // The handle stays open for the whole process.
    let Some(h) = out.map(|f| f.into_raw_handle()) else { return };
    for w in to_set {
        unsafe { SetStdHandle(w, h as Handle) };
    }
}

fn log_file() -> Option<File> {
    let dir = ig2_formats::options::Options::path()?.parent()?.to_path_buf();
    std::fs::create_dir_all(&dir).ok()?;
    File::create(dir.join(LOG_FILE)).ok()
}
