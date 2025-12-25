use std::collections::{HashMap, HashSet};
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};

use super::common::{uk64, Arch};
use super::proc::Proc;
use super::structs::{
    malloc_chunk_generator, malloc_state_generator, tcache_struct_generator, CStructDef,
    CStructInstance,
};

#[derive(Debug, Clone)]
pub struct MallocState {
    inst: CStructInstance,
}

impl MallocState {
    fn new(def: &CStructDef, mem: Vec<u8>, addr: u64) -> Self {
        Self {
            inst: def.new_instance(mem, addr),
        }
    }

    pub fn addrof(&self, name: &str) -> Option<u64> {
        self.inst.addrof(name)
    }

    pub fn fastbins(&self) -> Option<Vec<u64>> {
        self.inst.get_ptr_array("fastbinsY")
    }
}

#[derive(Debug, Clone)]
pub struct MallocChunk {
    inst: CStructInstance,
}

impl MallocChunk {
    fn new(def: &CStructDef, mem: Vec<u8>, addr: u64) -> Self {
        Self {
            inst: def.new_instance(mem, addr),
        }
    }

    pub fn addr(&self) -> u64 {
        self.inst.addr()
    }

    pub fn fd(&self) -> u64 {
        self.inst.get_ptr("fd").unwrap_or(0)
    }

    pub fn bk(&self) -> u64 {
        self.inst.get_ptr("bk").unwrap_or(0)
    }

    pub fn size(&self) -> u64 {
        self.inst.get_ptr("size").unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct Tcache {
    inst: CStructInstance,
}

impl Tcache {
    fn new(def: &CStructDef, mem: Vec<u8>, addr: u64) -> Self {
        Self {
            inst: def.new_instance(mem, addr),
        }
    }

    pub fn entries(&self) -> Option<Vec<u64>> {
        self.inst.get_ptr_array("entries")
    }
}

#[derive(Debug, Clone)]
pub struct HeapInspectorConfig {
    pub main_arena_offset: u64,
    pub tcache_enable: bool,
    pub libc_version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Chain {
    pub addrs: Vec<u64>,
    pub cycle: bool,
}

impl Default for HeapInspectorConfig {
    fn default() -> Self {
        Self {
            main_arena_offset: 0,
            tcache_enable: false,
            libc_version: None,
        }
    }
}

#[derive(Debug)]
pub struct HeapInspector {
    pid: u64,
    proc: Proc,
    arch: Arch,
    size_t: usize,
    libc_version: String,
    tcache_enable: bool,
    main_arena_offset: u64,
    libc_base: u64,
    heap_base: u64,
    libc_path: Option<String>,
    ld_path: Option<String>,
    exe_path: PathBuf,
    malloc_state: CStructDef,
    malloc_chunk: CStructDef,
    tcache_struct: CStructDef,
}

impl HeapInspector {
    pub fn new(pid: u64, config: HeapInspectorConfig) -> io::Result<Self> {
        let proc = Proc::new(pid);
        let arch = proc.arch();
        if arch != Arch::X64 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Only x86_64 is supported in this port",
            ));
        }

        let bases = proc.bases();
        let libc_base = first_base(&bases, "libc");
        let heap_base = first_base(&bases, "heap");
        let (default_libc, default_ld) = default_lib_paths();
        let libc_path = proc.libc_path().or(default_libc);
        let ld_path = proc.ld_path().or(default_ld);
        let exe_path = proc.exe_path();
        let libc_version = config
            .libc_version
            .unwrap_or_else(|| "unknown".to_string());

        Ok(Self {
            pid,
            proc,
            arch,
            size_t: 8,
            libc_version,
            tcache_enable: config.tcache_enable,
            main_arena_offset: config.main_arena_offset,
            libc_base,
            heap_base,
            libc_path,
            ld_path,
            exe_path,
            malloc_state: malloc_state_generator("unknown"),
            malloc_chunk: malloc_chunk_generator("unknown"),
            tcache_struct: tcache_struct_generator("unknown"),
        })
    }

    pub fn pid(&self) -> u64 {
        self.pid
    }

    pub fn arch(&self) -> Arch {
        self.arch
    }

    pub fn libc_version(&self) -> &str {
        &self.libc_version
    }

    pub fn libc_path(&self) -> Option<&str> {
        self.libc_path.as_deref()
    }

    pub fn ld_path(&self) -> Option<&str> {
        self.ld_path.as_deref()
    }

    pub fn exe_path(&self) -> &PathBuf {
        &self.exe_path
    }

    pub fn ranges(&self) -> HashMap<String, Vec<Range<u64>>> {
        self.proc.ranges()
    }

    pub fn bases(&self) -> HashMap<String, Vec<u64>> {
        self.proc.bases()
    }

    pub fn heapmem(&self) -> Option<Vec<u8>> {
        let ranges = self.proc.ranges();
        let heap_ranges = ranges.get("heap")?;
        let first = heap_ranges.first()?;
        let size_u64 = first.end.saturating_sub(first.start);
        let size = usize::try_from(size_u64).ok()?;
        self.proc.read(first.start, size)
    }

    pub fn arenamem(&self) -> Option<Vec<u8>> {
        let arena_size = self.malloc_state.size();
        let arena_addr = self.libc_base() + self.main_arena_offset;
        self.proc.read(arena_addr, arena_size)
    }

    pub fn main_arena(&self) -> Option<MallocState> {
        let arena_addr = self.libc_base() + self.main_arena_offset;
        let mem = self.arenamem()?;
        Some(MallocState::new(&self.malloc_state, mem, arena_addr))
    }

    pub fn tcache(&self) -> Option<Tcache> {
        if !self.tcache_enable {
            return None;
        }

        let heap_base = self.heap_base();
        let testmem = self.proc.read(heap_base + self.size_t as u64, self.size_t)?;
        let testval = uk64(testmem);
        let base_addr = if testval == 0 {
            heap_base + 4 * self.size_t as u64
        } else {
            heap_base + 2 * self.size_t as u64
        };

        let mem = self.proc.read(base_addr, self.tcache_struct.size())?;
        Some(Tcache::new(&self.tcache_struct, mem, base_addr))
    }

    pub fn heap_chunks(&self) -> Vec<MallocChunk> {
        let heap_mem = match self.heapmem() {
            Some(m) => m,
            None => return Vec::new(),
        };
        let mut cur_pos = 0usize;
        let size_t = self.size_t;

        if heap_mem.len() < size_t * 2 {
            return Vec::new();
        }

        let first_chunk_size = uk64(&heap_mem[size_t..size_t * 2]) & !0b111;
        if first_chunk_size == 0 {
            cur_pos += 2 * size_t;
        }

        let mut result = Vec::new();
        while cur_pos + size_t * 2 <= heap_mem.len() {
            let size_slice = &heap_mem[cur_pos + size_t..cur_pos + size_t * 2];
            let cur_block_size = uk64(size_slice) & !0b111;
            if cur_block_size == 0 {
                break;
            }
            let end = match usize::try_from(cur_block_size) {
                Ok(sz) => cur_pos + sz,
                Err(_) => break,
            };
            if end > heap_mem.len() {
                break;
            }

            let memblock = heap_mem[cur_pos..end].to_vec();
            let addr = self.heap_base() + cur_pos as u64;
            result.push(MallocChunk::new(&self.malloc_chunk, memblock, addr));

            let align_mask = 0b1111usize;
            let next = (cur_pos + cur_block_size as usize) & !align_mask;
            if cur_block_size < (2 * size_t) as u64 || next <= cur_pos {
                break;
            }
            cur_pos = next;
        }
        result
    }

    pub fn tcache_chunks(&self) -> HashMap<usize, Vec<MallocChunk>> {
        if !self.tcache_enable {
            return HashMap::new();
        }

        let mut result: HashMap<usize, Vec<MallocChunk>> = HashMap::new();
        let tcache = match self.tcache() {
            Some(t) => t,
            None => return result,
        };
        let entries = match tcache.entries() {
            Some(e) => e,
            None => return result,
        };

        for (index, entry_ptr) in entries.into_iter().enumerate() {
            let mut ptr = entry_ptr;
            let mut lst = Vec::new();
            let mut traversed = Vec::new();
            while ptr != 0 {
                let addr = ptr.saturating_sub(2 * self.size_t as u64);
                let mem = match self.proc.read(addr, 4 * self.size_t) {
                    Some(m) => m,
                    None => break,
                };
                let chunk = MallocChunk::new(&self.malloc_chunk, mem, addr);
                let next = chunk.fd();
                lst.push(chunk);
                if traversed.contains(&ptr) {
                    break;
                }
                traversed.push(ptr);
                ptr = next;
            }
            if !lst.is_empty() {
                result.insert(index, lst);
            }
        }
        result
    }

    pub fn tcache_chains(&self) -> HashMap<usize, Chain> {
        if !self.tcache_enable {
            return HashMap::new();
        }

        let mut result: HashMap<usize, Chain> = HashMap::new();
        let tcache = match self.tcache() {
            Some(t) => t,
            None => return result,
        };
        let entries = match tcache.entries() {
            Some(e) => e,
            None => return result,
        };

        for (index, entry_ptr) in entries.into_iter().enumerate() {
            let mut addrs = Vec::new();
            let mut seen = HashSet::new();
            let mut ptr = entry_ptr;
            let mut cycle = false;

            while ptr != 0 {
                if !seen.insert(ptr) {
                    cycle = true;
                    break;
                }
                let addr = ptr.saturating_sub(2 * self.size_t as u64);
                let mem = match self.proc.read(addr, 4 * self.size_t) {
                    Some(m) => m,
                    None => break,
                };
                let chunk = MallocChunk::new(&self.malloc_chunk, mem, addr);
                addrs.push(chunk.addr());
                ptr = chunk.fd();
            }

            if !addrs.is_empty() {
                result.insert(index, Chain { addrs, cycle });
            }
        }

        result
    }

    pub fn fastbins(&self) -> HashMap<usize, Vec<MallocChunk>> {
        let mut result = HashMap::new();
        let arena = match self.main_arena() {
            Some(a) => a,
            None => return result,
        };
        let fastbins = match arena.fastbins() {
            Some(f) => f,
            None => return result,
        };

        for (index, fastbin_head) in fastbins.into_iter().enumerate() {
            let mut fastbin_ptr = fastbin_head;
            let mut lst = Vec::new();
            let mut traversed = Vec::new();
            while fastbin_ptr != 0 {
                let mem = match self.proc.read(fastbin_ptr, 4 * self.size_t) {
                    Some(m) => m,
                    None => break,
                };
                let chunk =
                    MallocChunk::new(&self.malloc_chunk, mem, fastbin_ptr);
                let fd = chunk.fd();
                lst.push(chunk);
                if traversed.contains(&fastbin_ptr) {
                    break;
                }
                traversed.push(fastbin_ptr);
                fastbin_ptr = fd;
            }
            if !lst.is_empty() {
                result.insert(index, lst);
            }
        }
        result
    }

    pub fn fastbin_chains(&self) -> HashMap<usize, Chain> {
        let mut result = HashMap::new();
        let arena = match self.main_arena() {
            Some(a) => a,
            None => return result,
        };
        let fastbins = match arena.fastbins() {
            Some(f) => f,
            None => return result,
        };

        for (index, fastbin_head) in fastbins.into_iter().enumerate() {
            let mut addrs = Vec::new();
            let mut seen = HashSet::new();
            let mut ptr = fastbin_head;
            let mut cycle = false;

            while ptr != 0 {
                if !seen.insert(ptr) {
                    cycle = true;
                    break;
                }
                let mem = match self.proc.read(ptr, 4 * self.size_t) {
                    Some(m) => m,
                    None => break,
                };
                let chunk = MallocChunk::new(&self.malloc_chunk, mem, ptr);
                addrs.push(chunk.addr());
                ptr = chunk.fd();
            }

            if !addrs.is_empty() {
                result.insert(index, Chain { addrs, cycle });
            }
        }
        result
    }

    pub fn bins(
        &self,
        start: usize,
        end: usize,
        chunk_size: usize,
    ) -> HashMap<usize, Vec<MallocChunk>> {
        let mut result = HashMap::new();
        let arena = match self.main_arena() {
            Some(a) => a,
            None => return result,
        };

        for index in start..end {
            let head_addr = match arena.addrof(&format!("bins[{}]", index * 2)) {
                Some(a) => a.saturating_sub(2 * self.size_t as u64),
                None => continue,
            };
            let mut chunk_ptr = head_addr;
            let mut lst = Vec::new();
            let mut traversed = Vec::new();

            let mem = match self.proc.read(chunk_ptr, chunk_size) {
                Some(m) => m,
                None => continue,
            };
            let mut chunk = MallocChunk::new(&self.malloc_chunk, mem, chunk_ptr);

            while chunk.bk() != head_addr {
                chunk_ptr = chunk.bk();
                let mem = match self.proc.read(chunk_ptr, chunk_size) {
                    Some(m) => m,
                    None => break,
                };
                chunk = MallocChunk::new(&self.malloc_chunk, mem, chunk_ptr);
                let bk = chunk.bk();
                lst.push(chunk.clone());
                if traversed.contains(&bk) {
                    break;
                }
                traversed.push(bk);
            }
            if !lst.is_empty() {
                result.insert(index, lst);
            }
        }
        result
    }

    pub fn bin_chains(
        &self,
        start: usize,
        end: usize,
        chunk_size: usize,
    ) -> HashMap<usize, Chain> {
        let mut result = HashMap::new();
        let arena = match self.main_arena() {
            Some(a) => a,
            None => return result,
        };

        for index in start..end {
            let head_addr = match arena.addrof(&format!("bins[{}]", index * 2)) {
                Some(a) => a.saturating_sub(2 * self.size_t as u64),
                None => continue,
            };
            let mut addrs = Vec::new();
            let mut seen = HashSet::new();
            let mut cycle = false;

            let mem = match self.proc.read(head_addr, chunk_size) {
                Some(m) => m,
                None => continue,
            };
            let mut chunk = MallocChunk::new(&self.malloc_chunk, mem, head_addr);

            while chunk.bk() != head_addr {
                let next = chunk.bk();
                if !seen.insert(next) {
                    cycle = true;
                    break;
                }
                let mem = match self.proc.read(next, chunk_size) {
                    Some(m) => m,
                    None => break,
                };
                chunk = MallocChunk::new(&self.malloc_chunk, mem, next);
                addrs.push(chunk.addr());
            }

            if !addrs.is_empty() {
                result.insert(index, Chain { addrs, cycle });
            }
        }
        result
    }

    pub fn unsortedbins(&self) -> Vec<MallocChunk> {
        let bins = self.bins(0, 1, 0x20);
        bins.get(&0).cloned().unwrap_or_default()
    }

    pub fn unsortedbin_chain(&self) -> Option<Chain> {
        let bins = self.bin_chains(0, 1, 0x20);
        bins.get(&0).cloned()
    }

    pub fn smallbins(&self) -> HashMap<usize, Vec<MallocChunk>> {
        self.bins(2, 64, 0x20)
    }

    pub fn smallbin_chains(&self) -> HashMap<usize, Chain> {
        self.bin_chains(2, 64, 0x20)
    }

    pub fn largebins(&self) -> HashMap<usize, Vec<MallocChunk>> {
        self.bins(64, 127, 0x30)
    }

    pub fn largebin_chains(&self) -> HashMap<usize, Chain> {
        self.bin_chains(64, 127, 0x30)
    }

    pub fn record(&self) -> HeapRecord {
        HeapRecord::new(self)
    }

    fn libc_base(&self) -> u64 {
        if self.libc_base != 0 {
            self.libc_base
        } else {
            first_base(&self.proc.bases(), "libc")
        }
    }

    fn heap_base(&self) -> u64 {
        if self.heap_base != 0 {
            self.heap_base
        } else {
            first_base(&self.proc.bases(), "heap")
        }
    }
}

#[derive(Debug, Clone)]
pub struct HeapRecord {
    pub pid: u64,
    pub arch: Arch,
    pub libc_version: String,
    pub tcache_enable: bool,
    pub libc_path: Option<String>,
    pub exe_path: PathBuf,
    pub size_t: usize,
    pub main_arena: Option<MallocState>,
    pub tcache: Option<Tcache>,
    pub heap_chunks: Vec<MallocChunk>,
    pub fastbins: HashMap<usize, Vec<MallocChunk>>,
    pub unsortedbins: Vec<MallocChunk>,
    pub smallbins: HashMap<usize, Vec<MallocChunk>>,
    pub largebins: HashMap<usize, Vec<MallocChunk>>,
    pub tcache_chunks: HashMap<usize, Vec<MallocChunk>>,
    pub libc_base: u64,
    pub heap_base: u64,
    pub bases: HashMap<String, Vec<u64>>,
    pub ranges: HashMap<String, Vec<Range<u64>>>,
}

impl HeapRecord {
    pub fn new(hi: &HeapInspector) -> Self {
        Self {
            pid: hi.pid(),
            arch: hi.arch(),
            libc_version: hi.libc_version().to_string(),
            tcache_enable: hi.tcache_enable,
            libc_path: hi.libc_path().map(|s| s.to_string()),
            exe_path: hi.exe_path.clone(),
            size_t: hi.size_t,
            main_arena: hi.main_arena(),
            tcache: hi.tcache(),
            heap_chunks: hi.heap_chunks(),
            fastbins: hi.fastbins(),
            unsortedbins: hi.unsortedbins(),
            smallbins: hi.smallbins(),
            largebins: hi.largebins(),
            tcache_chunks: hi.tcache_chunks(),
            libc_base: hi.libc_base(),
            heap_base: hi.heap_base(),
            bases: hi.bases(),
            ranges: hi.ranges(),
        }
    }
}

fn first_base(bases: &HashMap<String, Vec<u64>>, key: &str) -> u64 {
    bases
        .get(key)
        .and_then(|v| v.first().copied())
        .unwrap_or(0)
}

fn default_lib_paths() -> (Option<String>, Option<String>) {
    let libc_candidates = [
        "/usr/lib/libc.so.6",
        "/lib/x86_64-linux-gnu/libc.so.6",
        "/lib64/libc.so.6",
    ];
    let ld_candidates = [
        "/usr/lib64/ld-linux-x86-64.so.2",
        "/lib64/ld-linux-x86-64.so.2",
        "/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2",
    ];

    let libc_path = libc_candidates
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| (*p).to_string());
    let ld_path = ld_candidates
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| (*p).to_string());
    (libc_path, ld_path)
}
