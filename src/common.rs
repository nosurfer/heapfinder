use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

const EI_CLASS_OFFSET: u64 = 4;
pub enum Arch {
    X86, 
    X64,
}

pub fn get_arch(pid: usize) -> io::Result<Arch> {
    let fmt = format!("/proc/{}/exe", pid);
    let mut fd = File::open(&fmt).map_err(|e| {
        io::Error::new(
            e.kind(), 
            format!("Failed to open file {}: {}", fmt, e))
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
                format!("Bad EI_CLASS field value: {:#08x}", other)
            )
        ),
    }
}