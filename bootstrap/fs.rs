//! Zip-based AppImage implementation of a FUSE file system.
use event::NotifyFlag;
use fuse::*;
use libc;
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
    sec: 0,
    nsec: 0,
};

/// Inode data type.
type Inode = u64;

/// Cached data about an inode.
#[derive(Clone, Debug)]
struct NodeData {
    path: PathBuf,
    is_dir: bool,
    attr: FileAttr,
}

impl NodeData {
    pub fn inode(&self) -> Inode {
        self.attr.ino
    }

    pub fn name(&self) -> &str {
        self.path.file_name().unwrap().to_str().unwrap()
    }
}

pub struct AppImageFileSystem {
    /// Metadata about the AppImage file.
    metadata: Metadata,

    /// Barrier signaling when the file system is ready.
    ready: NotifyFlag,

    /// An open handle to the zipped AppImage filesystem.
    archive: ZipArchive<File>,

    /// Cache of inode data.
    inode_cache: HashMap<Inode, NodeData>,

    /// Cache mapping paths to inodes.
    path_cache: HashMap<PathBuf, NodeData>,
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
            ready: NotifyFlag::new(),
            archive: archive,
            inode_cache: HashMap::new(),
            path_cache: HashMap::new(),
        })
    }

    /// Open the current executable as an AppImage filesystem.
    pub fn open_self() -> Option<Self> {
        Self::open("/proc/self/exe")
    }

    /// Gets a flag that can be used to wait until the file system is ready.
    pub fn ready(&self) -> NotifyFlag {
        self.ready.clone()
    }

    fn get_inode_count(&self) -> u64 {
        self.archive.len() as u64 + 1
    }

    fn get_node_by_inode(&mut self, inode: Inode) -> Option<NodeData> {
        if inode < FUSE_ROOT_ID || inode > self.get_inode_count() {
            return None;
        }

        if self.inode_cache.contains_key(&inode) {
            return self.inode_cache.get(&inode).cloned();
        }

        let node = if inode == FUSE_ROOT_ID {
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
            let entry = self.archive.by_index(inode as usize - 2).unwrap();
            let time = entry.last_modified().to_timespec();

            // Get the external attributes and derive the permissions from that.
            let external_attributes_high = entry.unix_mode().unwrap_or(0o777);
            let mode = external_attributes_high as u16 & 0o777;

            // Determine if the entry is a directory. If the name ends in /, then it is a directory. If bit 4 is set
            // then it is also a directory.
            let is_dir = external_attributes_high & libc::S_IFDIR == libc::S_IFDIR || entry.name().ends_with("/");

            NodeData {
                path: PathBuf::from(entry.name()),
                is_dir: is_dir,
                attr: FileAttr {
                    ino: inode,
                    size: entry.size(),
                    blocks: 0,
                    atime: time,
                    mtime: time,
                    ctime: time,
                    crtime: time,
                    kind: if is_dir {
                        FileType::Directory
                    } else {
                        FileType::RegularFile
                    },
                    perm: mode,
                    nlink: 3,
                    uid: self.metadata.uid(),
                    gid: self.metadata.gid(),
                    rdev: 0,
                    flags: 0,
                },
            }
        };

        self.inode_cache.insert(inode, node.clone());
        Some(node)
    }

    fn get_node_by_path(&mut self, path: PathBuf) -> Option<NodeData> {
        if self.path_cache.contains_key(&path) {
            return self.path_cache.get(&path).cloned();
        }

        for i in 1..self.get_inode_count()+1 {
            let node = self.get_node_by_inode(i).unwrap();

            if node.path == path {
                self.path_cache.insert(path, node.clone());
                return Some(node);
            }
        }

        None
    }
}

impl Filesystem for AppImageFileSystem {
    fn init(&mut self, _req: &Request) -> Result<(), i32> {
        self.ready.notify_all();

        println!("inode count: {}", self.get_inode_count());
        Ok(())
    }

    fn lookup(&mut self, _req: &Request, parent_inode: u64, child_name: &OsStr, reply: ReplyEntry) {
        if let Some(parent) = self.get_node_by_inode(parent_inode) {
            let mut child_path = parent.path.clone();
            child_path.push(child_name);

            if let Some(child) = self.get_node_by_path(child_path) {
                reply.entry(&TTL, &child.attr, 0);
                return;
            }

        }

        println!("error lookup({:?}, {:?})", parent_inode, child_name);
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
        if let Some(parent_node) = self.get_node_by_inode(inode) {
            // println!("readdir({})", inode);
            if offset > 0 {
                reply.ok();
                return;
            }

            let mut reply_offset = 0;

            // Add the current directory.
            reply.add(inode, reply_offset, FileType::Directory, ".");
            reply_offset += 1;

            // Find the parent directory.
            if inode == FUSE_ROOT_ID {
                reply.add(1, reply_offset, FileType::Directory, "..");
                reply_offset += 1;
            } else if let Some(parent_parent_path) = parent_node.path.parent() {
                for i in 1..self.get_inode_count()+1 {
                    let node = self.get_node_by_inode(i).unwrap();

                    if node.path == parent_parent_path {
                        reply.add(node.inode(), reply_offset, FileType::Directory, "..");
                        reply_offset += 1;
                        break;
                    }
                }
            }

            // Find child nodes.
            for i in 2..self.get_inode_count()+1 {
                let child_node = self.get_node_by_inode(i).unwrap();

                if let Some(child_parent_path) = child_node.path.parent() {
                    // println!("{:?} == {:?}?", child_parent_path, parent_node.path);
                    if child_parent_path == parent_node.path {
                        println!("{:?} > {} - {:?}", parent_node.path, child_node.inode(), child_node.path);
                        reply.add(child_node.inode(), reply_offset, child_node.attr.kind, child_node.name());
                        reply_offset += 1;
                    }
                } else if inode == FUSE_ROOT_ID {
                    reply.add(child_node.inode(), reply_offset, child_node.attr.kind, child_node.name());
                    reply_offset += 1;
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
                let mut entry = self.archive.by_index(inode as usize - 2).unwrap();

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
