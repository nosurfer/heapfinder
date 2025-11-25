use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use nix::{sys::ptrace, unistd::Pid, sys::wait::{waitpid, WaitStatus}};
use std::io::Seek;
use std::io::SeekFrom;
use std::ops::Range;
use std::path::PathBuf;
use std::path::Path;
use super::common::{get_arch, Arch};
use std::ffi::{c_void, c_long};
use std::error::Error;

const LIBC_REGEX: &str = r"^[^\x00]*libc(?:-[\d\.]+)?\.so(?:\.6)?$";
const LD_REGEX: &str = r"^[^\x00]*ld(?:-[\d\.]+)?\.so(?:\.2)?$";

#[derive(Debug)]
pub struct Map {
    range: Range<u64>,
    perm: String,
    mapname: String,
}

pub struct Proc {
    pid: u64,
    arch: Arch,
}

impl Map {
    fn new<P, M>(range: Range<u64>, perm: P, mapname: M) -> Self 
    where 
        P: Into<String>,
        M: Into<String>, 
    {
        Map {
            range,
            perm: perm.into(),
            mapname: mapname.into(),
        }
    }
}

impl Proc {
    pub fn new(pid: u64) -> Self {
        let path = format!("/proc/{}/exe", pid);
        Proc { pid, arch: get_arch(&path).expect("Can't determine arch") }
    }

    fn path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/{}/exe", self.pid))
    }

    pub fn vmmap(&self) -> Vec<Map> {
        let mpath = format!("/proc/{}/maps", self.pid);
        let re = Regex::new(
            r"(?<s>[0-9a-f]*)-(?<e>[0-9a-f]*) (?<p>[rwxps-]*)(?: [^ ]*){3} *(?<n>.*)"
        ).unwrap();
        let mut contents = String::new();

        File::open(&mpath)
            .expect(&format!("Failed to open file: {}", mpath))
            .read_to_string(&mut contents)
            .unwrap();

        re.captures_iter(&contents)
            .map(|caps| {
                let s = u64::from_str_radix(caps.name("s").unwrap().as_str(), 16).unwrap();
                let e = u64::from_str_radix(caps.name("e").unwrap().as_str(), 16).unwrap();
                let range = s..e;
                let perm = caps.name("p").unwrap().as_str();
                let name = caps.name("n").unwrap().as_str();
                Map::new(range, perm, name)
        })
        .collect()
    }

    fn range_merge(vec: &mut Vec<Range<u64>>, mut new: Range<u64>) {
        let mut i = 0;

        while i < vec.len() {
            let r = &vec[i];

            if new.start <= r.end && new.end >= r.start {
                new.start = new.start.min(r.start);
                new.end = new.end.max(r.end);
                vec.remove(i);
            } else {
                i += 1;
            }
        }
        vec.push(new);
    }

    pub fn ranges(&self) -> HashMap<String, Vec<Range<u64>>> {
        let libc_re = Regex::new(LIBC_REGEX).unwrap();
        let mut ranges: HashMap<String, Vec<Range<u64>>> = 
            ["mapped", "libc", "heap", "stack"]
                .map(|k| (k.to_string(), Vec::new())).into();

        for m in self.vmmap() {
            let key = match &*m.mapname {
                "mapped" => "mapped".to_string(),
                "[stack]" => "stack".to_string(),
                "[heap]" => "heap".to_string(),
                x if libc_re.is_match(x) => "libc".to_string(),
                other => Path::new(other)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            };

            ranges
                .entry(key)
                .and_modify(|v| Self::range_merge(v, m.range.clone()))
                .or_insert(vec![m.range]);   
        }
        ranges
    }

    pub fn bases(&self) -> HashMap<String, Vec<u64>> {
        let libc_re = Regex::new(LIBC_REGEX).unwrap();
        let mut bases: HashMap<String, Vec<u64>> = 
            ["mapped", "libc", "heap", "stack"]
                .map(|k| (k.to_string(), Vec::new())).into();

        for m in self.vmmap().into_iter().rev() {
            let key = match &*m.mapname {
                "mapped" => "mapped".to_string(),
                "[stack]" => "stack".to_string(),
                "[heap]" => "heap".to_string(),
                x if libc_re.is_match(x) => "libc".to_string(),
                other => Path::new(other)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            };

            bases
                .entry(key)
                .or_insert(vec![m.range.start]);   
        }
        bases
    }

    pub fn whereis(&self, addr: u64) -> Option<String> {
        let libc_re = Regex::new(LIBC_REGEX).unwrap();

        for m in self.vmmap() {
            if !m.range.contains(&addr) {
                continue;
            }

            let name = m.mapname.as_str();

            return Some(match name {
                "mapped" => "mapped".to_string(),
                "[stack]" => "stack".to_string(),
                "[heap]" => "heap".to_string(),
                x if libc_re.is_match(x) => "libc".to_string(),
                other => Path::new(other)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            });
        }
        None
    }

    // pub fn read(&self, addr: u64, size: usize) ->  Vec<u8> {
    //     let path = format!("/proc/{}/mem", self.pid);
    //     let mut file = File::open(&path)
    //         .unwrap_or_else(|e| panic!("Failed to read file {}: {}", path, e));

    //     let mut buf = vec![0u8; size];

    //     file.seek(SeekFrom::Start(addr)).unwrap();
    //     file.read_exact(&mut buf).unwrap();

    //     buf
    // }
    // pub fn read(&self, addr: u64, size: usize) ->  Vec<u8> {
    //     let pid = Pid::from_raw(self.pid as i32);
    //     let path = format!("/proc/{}/mem", self.pid);

    //     ptrace::attach(pid).expect("cant't attach to process");
    //     waitpid(pid, None).expect("Waitpid failed");

    //     let mut file = File::open(&path)
    //         .unwrap_or_else(|e| panic!("Failed to open {}: {}", path, e));

    //     file.seek(SeekFrom::Start(addr)).expect("Seek failed");
    //     let mut buf = vec![0u8; size];
    //     ptrace::read(pid, addr);

    //     ptrace::detach(pid, None).expect("Can't detach");
    //     buf
    // }

    pub fn read_mem(&self, addr: u64, size: usize) -> Vec<u8> {
        let path = format!("/proc/{}/mem", self.pid);
        let mut buf = vec![0u8; size];

        if let Ok(mut f) = File::open(&path) {
            if f.seek(SeekFrom::Start(addr)).is_ok() {
                if f.read_exact(&mut buf).is_err() {
                    // on error, return empty
                    buf.clear();
                }
            } else {
                buf.clear();
            }
        } else {
            buf.clear();
        }
        buf
    }

    pub fn read_gpt(&self, addr: u64, size: usize) -> Result<Vec<u8>, Box<dyn Error>> {
        let pid = Pid::from_raw(self.pid as i32);

        ptrace::attach(pid)?;
        waitpid(pid, None).expect("Waitpid failed");

        let result = (|| -> Result<Vec<u8>, Box<dyn Error>> {
            let mut buf = Vec::with_capacity(size);
            let word_size = std::mem::size_of::<c_long>();
            let mut offset = 0usize;

            while offset < size {
                let cur_addr = addr + offset as u64;
                let c_addr: *mut c_void = cur_addr as *mut c_void;
                let word = ptrace::read(pid, c_addr)?;
                let bytes = word.to_ne_bytes();
                let take = std::cmp::min(word_size, size - offset);
                buf.extend_from_slice(&bytes[..take]);
                offset += take;
            }

            buf.truncate(size);
            Ok(buf)
        })();

        let _ = ptrace::detach(pid, None);
        result
    }

}

