use std::fs::*;
use std::io;
use std::path::*;


/// Iterator that walks over a directory recursively.
///
/// Entries are yielded in a depth-first order.
pub struct RecursiveDirIterator {
    readers: Vec<ReadDir>,
}

impl RecursiveDirIterator {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<RecursiveDirIterator> {
        let reader = read_dir(path)?;

        Ok(RecursiveDirIterator {
            readers: vec![reader],
        })
    }
}

impl Iterator for RecursiveDirIterator {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        loop {
            if let Some(mut reader) = self.readers.pop() {
                if let Some(item) = reader.next() {
                    self.readers.push(reader);

                    if let Ok(ref entry) = item {
                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_dir() {
                                self.readers.push(read_dir(&entry.path()).unwrap());
                            }
                        }
                    }

                    return Some(item);
                }
            } else {
                return None;
            }
        }
    }
}
