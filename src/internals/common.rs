use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

const EI_CLASS_OFFSET: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    X86, 
    X64,
}

pub fn get_arch(path: &str) -> io::Result<Arch> {
    let mut fd = File::open(&path).map_err(|e| {
        io::Error::new(
            e.kind(), 
            format!("Failed to open file {}: {}", path, e))
    })?;
    fd.seek(SeekFrom::Start(EI_CLASS_OFFSET))?;

    let mut byte = 0u8;
    fd.read_exact(std::slice::from_mut(&mut byte))?;

    match byte {
        1 => Ok(Arch::X86),
        2 => Ok(Arch::X64),
        other => Err(
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Bad EI_CLASS field value: {:#x}", other)
            )
        ),
    }
}

pub fn uk64<T: AsRef<[u8]>>(bytes: T) -> u64 {
    let bytes = bytes.as_ref();
    let mut buf = [0; 8];
    let len = bytes.len().min(8);
    buf[..len].copy_from_slice(&bytes[..len]);
    u64::from_le_bytes(buf)
}

pub fn pk64(num: u64) -> [u8; 8] {
    num.to_le_bytes()
} 

// https://doc.rust-lang.org/stable/std/?search=from_le_bytes
