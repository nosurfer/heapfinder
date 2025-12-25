use hex;
use regex::Regex;
use std::fs::File;
use std::ops::Range;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::io::{Read, Seek, SeekFrom};
use super::common::{get_arch, Arch};

const LIBC_REGEX: &str = r"^[^\x00]*libc(?:-[\d\.]+)?\.so(?:\.6)?$";
const LD_REGEX: &str = r"^[^\x00]*ld(?:-[\d\.]+)?\.so(?:\.2)?$";

#[derive(Debug)]
pub struct Map {
    range: Range<u64>,
    perm: String,
    mapname: String,
}

#[derive(Debug)]
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

    pub fn arch(&self) -> Arch {
        self.arch
    }

    pub fn exe_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/{}/exe", self.pid))
    }

    pub fn libc_path(&self) -> Option<String> {
        self.libc()
    }

    pub fn ld_path(&self) -> Option<String> {
        self.ld()
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
                let mut name = caps.name("n").unwrap().as_str();
                if name.is_empty() {
                    name = "mapped";
                }
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

            bases
                .entry(key)
                .and_modify(|v| v.push(m.range.start))
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

    pub fn read(&self, addr: u64, size: usize) -> Option<Vec<u8>> {
        let path = format!("/proc/{}/mem", self.pid);
        let mut buf = vec![0u8; size];

        if let Ok(mut f) = File::open(&path) {
            if f.seek(SeekFrom::Start(addr)).is_ok() {
                if f.read_exact(&mut buf).is_ok() {
                    return Some(buf);
                }
            }
        }
        None
    }

    fn searchmem(&self, range: &Range<u64>, pattern: &str) -> Vec<(u64, String)> {
        let mut result = Vec::new();

        if range.start >= range.end {
            return result;
        }

        let size_u64 = range.end.saturating_sub(range.start);
        if size_u64 == 0 {
            return result;
        }
        let size = match usize::try_from(size_u64) {
            Ok(n) => n,
            Err(_) => return result,
        };

        let mem = match self.read(range.start, size) {
            Some(m) => m,
            None => return result,
        };

        let needle: Vec<u8> = if pattern.starts_with("0x") {
            let mut hexstr = &pattern[2..];
            if hexstr.len() % 2 != 0 {
                let mut s = String::with_capacity(hexstr.len() + 1);
                s.push('0');
                s.push_str(hexstr);
                hexstr = Box::leak(s.into_boxed_str());
            }
            let mut b = match hex::decode(hexstr) {
                Ok(v) => v,
                Err(_) => return result,
            };
            b.reverse();
            b
        } else if pattern.chars().all(|c| c.is_ascii_digit()) {
            // decimal integer -> minimal little-endian bytes
            if let Ok(mut n) = pattern.parse::<u128>() {
                if n == 0 {
                    vec![0u8]
                } else {
                    let mut bytes = Vec::new();
                    while n != 0 {
                        bytes.push((n & 0xff) as u8);
                        n >>= 8;
                    }
                    bytes
                }
            } else {
                pattern.as_bytes().to_vec()
            }
        } else {
            pattern.as_bytes().to_vec()
        };

        if needle.is_empty() || needle.len() > mem.len() {
            return result;
        }

        let plen = needle.len();
        for i in 0..=(mem.len() - plen) {
            if &mem[i..i + plen] == needle.as_slice() {
                let addr = range.start + i as u64;
                result.push((addr, hex::encode(&mem[i..i + plen])));
            }
        }

        result
    }

    pub fn searchmem_by_mapname(&self, mapname: &str, search: &str) -> Vec<(u64, String)> {
        let mut result= Vec::new();

        for m in self.vmmap().iter() {
            if m.mapname == mapname && m.perm.contains('r') {
                let overlap = self.searchmem(&m.range, search);
                result.extend(overlap);
            }
        };
        result
    }

    pub fn search_in_libc(&self, search: &str) -> Vec<(u64, String)> {
        if let Some(libc) = self.libc() {
            self.searchmem_by_mapname(&libc, search)
        } else {
            Vec::new()
        }
    }

    pub fn search_in_stack(&self, search: &str) -> Vec<(u64, String)> {
        self.searchmem_by_mapname("[stack]", search)
    }

    pub fn search_in_heap(&self, search: &str) -> Vec<(u64, String)> {
        self.searchmem_by_mapname("[heap]", search)
    }

    fn libc(&self) -> Option<String> {
        let re = Regex::new(LIBC_REGEX).ok()?;
        for m in self.vmmap() {
            if re.is_match(&m.mapname) {
                return Some(m.mapname.clone());
            }
        };
        None
    }

    fn ld(&self) -> Option<String> {
        let re = Regex::new(LD_REGEX).ok()?;
        for m in self.vmmap() {
            if re.is_match(&m.mapname) {
                return Some(m.mapname.clone());
            }
        };
        None
    }

}
