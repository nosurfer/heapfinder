use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::ops::Range;
use std::path::PathBuf;
use std::path::Path;
use super::common::{get_arch, Arch};

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


}

