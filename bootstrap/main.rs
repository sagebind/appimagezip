#![feature(alloc_system)]
extern crate alloc_system;
extern crate fuse;
extern crate libc;
#[macro_use]
extern crate log;
extern crate simplelog;
extern crate tempdir;
extern crate time;
extern crate zip;

mod fs;

use fs::AppImageFileSystem;
use std::env;
use std::fs::read_link;
use std::process::exit;
use std::sync::{Arc, Barrier};
use std::thread;
use tempdir::TempDir;


fn main() {
    let _ = simplelog::TermLogger::init(log::LogLevelFilter::Trace, simplelog::Config::default());

    let file_system = match AppImageFileSystem::open_self() {
        Some(v) => v,
        None => {
            println!("Cannot read AppImage filesystem, binary could be corrupt.");
            exit(130);
        },
    };

    let parent = Arc::new(Barrier::new(2));
    let child = parent.clone();

    let mount_dir = TempDir::new("appimage")
        .expect("failed to create mount point");
    let mount_path = mount_dir.path().to_path_buf();

    // Mount the filesystem image in a background thread.
    thread::spawn(move || {
        fuse::mount(file_system, &mount_path, &[])
            .expect("failed to mount fs");

        child.wait();
    });

    env::set_var("APPIMAGE", read_link("/proc/self/exe").unwrap());
    env::set_var("APPDIR", mount_dir.path());

    parent.wait();
}
