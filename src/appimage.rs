use bootstrap;
use std::fs::{self, File};
use std::io;
use std::io::prelude::*;
use std::os::unix::fs::*;
use std::path::{Path, PathBuf};
use time::{self, Timespec};
use util::RecursiveDirIterator;
use zip::write::*;


/// Update metadata information.
#[derive(Clone)]
pub enum UpdateEndpoint {
    Zsync {
        url: String,
    },

    BintrayZsync {
        username: String,
        repository: String,
        package: String,
        path: String,
    },
}

impl ToString for UpdateEndpoint {
    fn to_string(&self) -> String {
        match self {
            &UpdateEndpoint::Zsync {
                ref url,
            } => format!("zsync|{}", url),
            &UpdateEndpoint::BintrayZsync {
                ref username,
                ref repository,
                ref package,
                ref path,
            } => format!("bintray-zsync|{}|{}|{}|{}", username, repository, package, path),
        }
    }
}

/// Creates an AppImage.
pub struct Creator {
    app_dir: PathBuf,
    update_endpoint: Option<UpdateEndpoint>,
}

impl Creator {
    pub fn new<P: Into<PathBuf>>(app_dir: P) -> Creator {
        Creator {
            app_dir: app_dir.into(),
            update_endpoint: None,
        }
    }

    pub fn write_to<W: Write + Seek>(&self, mut writer: W) -> io::Result<()> {
        // First start the file with the bootstrap binary.
        bootstrap::write(&mut writer);

        // Now create a zip archive by copying all files in the app dir.
        let mut zip = ZipWriter::new(&mut writer);

        for entry in RecursiveDirIterator::new(&self.app_dir)?.filter_map(|r| r.ok()) {
            println!("copy: {:?}", entry.path());
            let path = entry.path();
            let relative_path = entry.path().strip_prefix(&self.app_dir).unwrap().to_path_buf();

            if path.exists() {
                let metadata = fs::metadata(entry.path())?;
                let mtime = Timespec::new(metadata.mtime(), metadata.mtime_nsec() as i32);
                let options = FileOptions::default()
                    .last_modified_time(time::at(mtime))
                    .unix_permissions(metadata.mode());

                if entry.file_type()?.is_dir() {
                    let name_with_slash = format!("{}/", relative_path.to_string_lossy());
                    zip.add_directory(name_with_slash, options)?;
                } else {
                    zip.start_file(relative_path.to_string_lossy(), options)?;

                    let mut file = File::open(entry.path())?;
                    io::copy(&mut file, &mut zip)?;
                    zip.flush()?;
                }
            }
        }

        zip.finish()?;

        Ok(())
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, path: P)-> io::Result<()> {
        let mut file = File::create(path)?;
        self.write_to(&mut file)?;

        // Mark the file as executable.
        let mut permissions = file.metadata()?.permissions();
        let mode = permissions.mode() | 0o111;
        permissions.set_mode(mode);
        file.set_permissions(permissions)?;

        Ok(())
    }
}
