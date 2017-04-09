use std::io::Write;


/// Get the raw bootstrap bytes.
#[inline]
pub fn bytes() -> &'static [u8] {
    static BOOTSTRAP_BINARY: &'static [u8] = include_bytes!("../bin/bootstrap");

    BOOTSTRAP_BINARY
}

/// Write the bootstrap contents.
pub fn write<W: Write>(mut writer: W) {
    writer.write_all(bytes()).unwrap();
}
