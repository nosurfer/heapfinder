use crate::common::{self, Arch}; 

struct Map {
    start: usize,
    end: usize,
    perm: [char; 4],
    mapname: String,
}

struct Proc {
    pid: usize,
    arch: Arch,
}

impl Map {
    fn new(start: usize, end: usize, perm: [char; 4], mapname: &str) -> Self {
        Self {
            start,
            end,
            perm,
            mapname: mapname.to_string(),
        }
    }
}

impl Proc {
    fn new(pid: usize) -> Self {
        Self {
            pid, 
            arch: common::get_arch(pid).expect("Failed to detect arch"),
        }
    }
    fn path(&self) -> String {
        format!("/proc/{}/exe", self.pid)
    }
}

fn vmmap(pid: usize) {
    
}