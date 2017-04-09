//! Zip-based AppImage implementation of a FUSE file system.
use fuse::*;
use libc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{File, Metadata};
use std::io::Read;
use std::os::unix::fs::*;
use std::path::*;
use time::Timespec;
use zip::ZipArchive;


/// Time-to-live for responses. As this is a read-only file system, we can have long TTL values.
const TTL: Timespec = Timespec {
    sec: 10,
    nsec: 0,
};

/// Inode data type.
type Inode = u64;

/// Cached data about an inode.
#[derive(Clone)]
struct NodeData {
    path: PathBuf,
    is_dir: bool,
    attr: FileAttr,
}

impl NodeData {
    pub fn inode(&self) -> u64 {
        self.attr.ino
    }

    pub fn name(&self) -> &str {
        self.path.file_name().unwrap().to_str().unwrap()
    }
}

pub struct AppImageFileSystem {
    /// Metadata about the AppImage file.
    metadata: Metadata,

    /// An open handle to the zipped AppImage filesystem.
    archive: RefCell<ZipArchive<File>>,

    /// Cache of inode data.
    inode_cache: RefCell<HashMap<Inode, NodeData>>,

    /// Cache mapping paths to inodes.
    path_cache: RefCell<HashMap<PathBuf, NodeData>>,
}

impl AppImageFileSystem {
    /// Open an AppImage filesystem from a file.
    pub fn open<P: AsRef<Path>>(path: P) -> Option<Self> {
        let file = match File::open(path) {
            Ok(v) => v,
            Err(_) => return None,
        };

        let metadata = file.metadata()
            .expect("failed to fetch file metadata");

        let archive = match ZipArchive::new(file) {
            Ok(v) => v,
            Err(_) => return None,
        };

        Some(Self {
            metadata: metadata,
            archive: RefCell::new(archive),
            inode_cache: RefCell::new(HashMap::new()),
            path_cache: RefCell::new(HashMap::new()),
        })
    }

    /// Open the current executable as an AppImage filesystem.
    pub fn open_self() -> Option<Self> {
        Self::open("/proc/self/exe")
    }

    fn get_max_inode(&self) -> u64 {
        self.archive.borrow().len() as u64 + 1
    }

    fn get_node_by_inode(&self, inode: Inode) -> Option<NodeData> {
        if inode < FUSE_ROOT_ID || inode > self.get_max_inode() {
            return None;
        }

        Some(self.inode_cache.borrow_mut().entry(inode).or_insert_with(|| {
            if inode == FUSE_ROOT_ID {
                NodeData {
                    path: PathBuf::new(),
                    is_dir: true,
                    attr: FileAttr {
                        ino: 1,
                        size: 0,
                        blocks: 0,
                        atime: Timespec::new(self.metadata.atime(), self.metadata.atime_nsec() as i32),
                        mtime: Timespec::new(self.metadata.mtime(), self.metadata.mtime_nsec() as i32),
                        ctime: Timespec::new(self.metadata.ctime(), self.metadata.ctime_nsec() as i32),
                        crtime: Timespec::new(self.metadata.ctime(), self.metadata.ctime_nsec() as i32),
                        kind: FileType::Directory,
                        perm: self.metadata.permissions().mode() as u16,
                        nlink: 2,
                        uid: self.metadata.uid(),
                        gid: self.metadata.gid(),
                        rdev: 0,
                        flags: 0,
                    },
                }
            } else {
                let mut archive = self.archive.borrow_mut();
                let entry = archive.by_index(inode as usize - 2).unwrap();
                let time = entry.last_modified().to_timespec();
                let is_dir = entry.name().ends_with("/");

                NodeData {
                    path: PathBuf::from(entry.name()),
                    is_dir: is_dir,
                    attr: FileAttr {
                        ino: inode,
                        size: if is_dir {0} else {entry.size()},
                        blocks: if is_dir {0} else {1},
                        atime: time,
                        mtime: time,
                        ctime: time,
                        crtime: time,
                        kind: if is_dir {
                            FileType::Directory
                        } else {
                            FileType::RegularFile
                        },
                        perm: entry.unix_mode().unwrap_or(0o777) as u16,
                        nlink: 2,
                        uid: self.metadata.uid(),
                        gid: self.metadata.gid(),
                        rdev: 0,
                        flags: 0,
                    },
                }
            }
        }).clone())
    }

    fn get_node_by_path(&self, path: PathBuf) -> Option<NodeData> {
        let mut cache = self.path_cache.borrow_mut();

        if cache.contains_key(&path) {
            return cache.get(&path).cloned();
        }

        for i in 2..self.get_max_inode()+1 {
            let data = self.get_node_by_inode(i).unwrap();

            if data.path == path {
                cache.insert(path, data.clone());
                return Some(data);
            }
        }

        None
    }
}

impl Filesystem for AppImageFileSystem {
    fn lookup(&mut self, _req: &Request, parent_inode: u64, child_name: &OsStr, reply: ReplyEntry) {
        if let Some(parent) = self.get_node_by_inode(parent_inode) {
            let mut child_path = parent.path.clone();
            child_path.push(child_name);

            if let Some(child) = self.get_node_by_path(child_path) {
                reply.entry(&TTL, &child.attr, 0);
                return;
            }
        }

        reply.error(libc::ENOENT);
    }

    fn getattr(&mut self, _req: &Request, inode: u64, reply: ReplyAttr) {
        if let Some(data) = self.get_node_by_inode(inode) {
            reply.attr(&TTL, &data.attr);
        } else {
            reply.error(libc::ENOENT);
        }
    }

    fn readdir(&mut self, _req: &Request, inode: u64, _fh: u64, offset: u64, mut reply: ReplyDirectory) {
        if let Some(data) = self.get_node_by_inode(inode) {
            if offset > 0 {
                reply.ok();
                return;
            }

            // Add the current directory.
            reply.add(inode, 0, FileType::Directory, ".");

            // Find the parent directory.
            println!("inode {}", inode);
            if inode == FUSE_ROOT_ID {
                reply.add(1, 1, FileType::Directory, "..");
            } else {
                for i in 1..self.get_max_inode()+1 {
                    let parent_data = self.get_node_by_inode(i).unwrap();

                    if let Some(parent_path) = data.path.parent() {
                        if parent_data.path == parent_path {
                            reply.add(parent_data.inode(), 1, FileType::Directory, "..");
                            return;
                        }
                    }
                }
            }

            // Find child nodes.
            for i in 2..self.get_max_inode()+1 {
                let child_data = self.get_node_by_inode(i).unwrap();

                if let Some(parent) = child_data.path.parent() {
                    if parent == data.path {
                        reply.add(child_data.inode(), i, child_data.attr.kind, child_data.name());
                    }
                }
            }

            reply.ok();
        } else {
            reply.error(libc::ENOENT);
        }
    }

    fn read(&mut self, _req: &Request, inode: u64, _fh: u64, offset: u64, size: u32, reply: ReplyData) {
        if let Some(data) = self.get_node_by_inode(inode) {
            if !data.is_dir {
                let mut archive = self.archive.borrow_mut();
                let mut entry =  archive.by_index(inode as usize - 2).unwrap();

                let mut read = offset as usize + size as usize;
                if read > data.attr.size as usize {
                    read = data.attr.size as usize;
                }

                let mut buffer = Vec::with_capacity(read);
                buffer.resize(read, 0);

                if let Err(e) = entry.read_exact(&mut buffer) {
                    reply.error(e.raw_os_error().unwrap_or(libc::EIO));
                    return;
                }

                reply.data(&buffer[offset as usize..]);
                return;
            }
        }

        reply.error(libc::ENOENT);
    }
}
