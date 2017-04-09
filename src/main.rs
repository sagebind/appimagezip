#![allow(dead_code)]
extern crate getopts;
extern crate time;
extern crate zip;

mod appimage;
mod bootstrap;
mod util;

use getopts::Options;
use std::env;
use std::io::stdout;


fn main() {
    let mut options = Options::new();

    options.optflag("h", "help", "Show this help message");
    options.optflag("v", "version", "Show version info");
    options.optflag("D", "dump-bootstrap", "Dump the runtime bootstrap binary");
    options.optopt("", "target", "Build for the target triple", "TRIPLE");

    let args = options.parse(env::args()).unwrap();

    if args.opt_present("h") {
        let short = options.short_usage(env!("CARGO_PKG_NAME"));
        let long = options.usage(&short);
        println!("{}", long);
        return;
    }

    if args.opt_present("v") {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.opt_present("D") {
        bootstrap::write(stdout());
        return;
    }

    let app_dir = args.free.get(1);
    if let Some(app_dir) = app_dir {
        let creator = appimage::Creator::new(app_dir);

        if let Err(e) = creator.write_to_file("Out.AppImage") {
            println!("error: {}", e);
        }
    }
}
