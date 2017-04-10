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


fn print_help(options: &Options) {
    println!("{}", options.usage("Usage: appimagezip [OPTIONS] SOURCE"));
}

fn main() {
    let mut options = Options::new();

    options.optflag("h", "help", "Show this help message");
    options.optopt("o", "output", "Write the AppImage to FILE", "FILE");
    options.optopt("", "target", "Build for the target triple", "TRIPLE");
    options.optflag("D", "dump-bootstrap", "Dump the runtime bootstrap binary");
    options.optflag("v", "version", "Show version info");

    let args = options.parse(env::args()).unwrap();

    if args.opt_present("h") {
        print_help(&options);
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

    let output_file = args.opt_str("o").unwrap_or(String::from("Out.AppImage"));

    let app_dir = args.free.get(1);
    if let Some(app_dir) = app_dir {
        let creator = appimage::Creator::new(app_dir);

        match creator.write_to_file(&output_file) {
            Ok(_) => {
                println!("Created AppImage: {:?}", output_file);
            },
            Err(e) => {
                println!("Error: {}", e);
            },
        }
    }
}
