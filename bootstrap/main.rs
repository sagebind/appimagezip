#![feature(alloc_system)]
extern crate alloc_system;
extern crate fuse;
extern crate libc;
extern crate tempdir;
extern crate time;
extern crate zip;

mod event;
mod fs;

use fs::AppImageFileSystem;
use std::env;
use std::fs::read_link;
use std::process::{exit, Command};
use tempdir::TempDir;


macro_rules! printerr {
    ($fmt:expr) => {
        use std::io::{stderr, Write};
        let _ = writeln!(stderr(), $fmt);
    };
    ($fmt:expr, $($arg:tt)*) => {
        use std::io::{stderr, Write};
        let _ = writeln!(stderr(), $fmt, $($arg)*);
    };
}

fn run() -> i32 {
    // Open this binary as an AppImage file system.
    let file_system = match AppImageFileSystem::open_self() {
        Some(v) => v,
        None => {
            printerr!("Cannot read AppImage filesystem, binary could be corrupt.");
            return 70;
        },
    };

    // Prepare a temporary directory to mount the file system image to.
    let mount_dir = match TempDir::new("appimage") {
        Ok(v) => v,
        Err(_) => {
            printerr!("Failed to create mount directory.");
            return 75;
        },
    };

    let mount_path = mount_dir.path().to_path_buf();

    // Mount the filesystem image in a background thread.
    let ready = file_system.ready();
    let session = unsafe {
        match fuse::spawn_mount(file_system, &mount_path, &[]) {
            Ok(s) => s,
            Err(e) => {
                printerr!("Failed to mount FUSE file system: {}", e);
                return 71;
            },
        }
    };

    // Some useful variable for the client application.
    env::set_var("APPIMAGE", read_link("/proc/self/exe").unwrap());
    env::set_var("APPDIR", mount_dir.path());

    let mut app_run_path = mount_dir.path().to_path_buf();
    app_run_path.push("AppRun");

    // Wait for the file system to be initialized.
    ready.wait();

    // Run the client application.
    if let Err(e) = Command::new(&app_run_path).args(env::args()).status() {
        printerr!("Failed to execute {:?}: {}", app_run_path, e);
        return 70;
    }

    drop(session);
    drop(mount_dir);

    0
}

fn main() {
    exit(run());
}
