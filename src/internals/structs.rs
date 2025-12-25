#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldType {
    Int16,
    Int,
    Ptr,
    SizeT,
}

impl FieldType {
    fn size(self) -> usize {
        match self {
            FieldType::Int16 => 2,
            FieldType::Int => 4,
            FieldType::Ptr | FieldType::SizeT => 8,
        }
    }
}

#[derive(Debug, Clone)]
struct FieldDef {
    typ: FieldType,
    name: &'static str,
    count: usize,
}

#[derive(Debug, Clone)]
pub struct CStructDef {
    fields: Vec<FieldDef>,
}

impl CStructDef {
    fn new(fields: Vec<FieldDef>) -> Self {
        Self { fields }
    }

    pub fn size(&self) -> usize {
        self.fields
            .iter()
            .map(|f| f.typ.size() * f.count)
            .sum()
    }

    fn field_def_offset(&self, name: &str) -> Option<(usize, &FieldDef)> {
        let mut offset = 0usize;
        for f in &self.fields {
            let size = f.typ.size();
            if f.name == name {
                return Some((offset, f));
            }
            offset += size * f.count;
        }
        None
    }

    fn offset_of(&self, name: &str, index: usize) -> Option<usize> {
        let (offset, f) = self.field_def_offset(name)?;
        if index >= f.count {
            return None;
        }
        Some(offset + index * f.typ.size())
    }

    pub fn new_instance(&self, mut memdump: Vec<u8>, addr: u64) -> CStructInstance {
        let needed = self.size();
        if memdump.len() < needed {
            memdump.resize(needed, 0u8);
        }
        CStructInstance {
            def: self.clone(),
            mem: memdump,
            addr,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CStructInstance {
    def: CStructDef,
    mem: Vec<u8>,
    addr: u64,
}

impl CStructInstance {
    pub fn size(&self) -> usize {
        self.def.size()
    }

    pub fn addr(&self) -> u64 {
        self.addr
    }

    pub fn addrof(&self, var: &str) -> Option<u64> {
        let (name, idx) = parse_name_index(var)?;
        let index = idx.unwrap_or(0);
        let offset = self.def.offset_of(name, index)?;
        Some(self.addr + offset as u64)
    }

    pub fn get_ptr(&self, var: &str) -> Option<u64> {
        let (name, idx) = parse_name_index(var)?;
        let index = idx.unwrap_or(0);
        let (offset, f) = self.def.field_def_offset(name)?;
        if !matches!(f.typ, FieldType::Ptr | FieldType::SizeT) {
            return None;
        }
        if index >= f.count {
            return None;
        }
        let off = offset + index * f.typ.size();
        let slice = self.mem.get(off..off + 8)?;
        Some(read_u64_le(slice))
    }

    pub fn get_ptr_array(&self, name: &str) -> Option<Vec<u64>> {
        let (offset, f) = self.def.field_def_offset(name)?;
        if !matches!(f.typ, FieldType::Ptr | FieldType::SizeT) {
            return None;
        }
        let size = f.typ.size();
        let mut out = Vec::with_capacity(f.count);
        for i in 0..f.count {
            let off = offset + i * size;
            let slice = self.mem.get(off..off + size)?;
            out.push(read_u64_le(slice));
        }
        Some(out)
    }

    pub fn get_int(&self, var: &str) -> Option<u32> {
        let (name, idx) = parse_name_index(var)?;
        let index = idx.unwrap_or(0);
        let (offset, f) = self.def.field_def_offset(name)?;
        if f.typ != FieldType::Int {
            return None;
        }
        if index >= f.count {
            return None;
        }
        let off = offset + index * f.typ.size();
        let slice = self.mem.get(off..off + 4)?;
        Some(read_u32_le(slice))
    }

    pub fn get_u16(&self, var: &str) -> Option<u16> {
        let (name, idx) = parse_name_index(var)?;
        let index = idx.unwrap_or(0);
        let (offset, f) = self.def.field_def_offset(name)?;
        if f.typ != FieldType::Int16 {
            return None;
        }
        if index >= f.count {
            return None;
        }
        let off = offset + index * f.typ.size();
        let slice = self.mem.get(off..off + 2)?;
        Some(read_u16_le(slice))
    }
}

fn parse_name_index(s: &str) -> Option<(&str, Option<usize>)> {
    if let Some(open) = s.find('[') {
        if !s.ends_with(']') {
            return None;
        }
        let name = &s[..open];
        let idx_str = &s[open + 1..s.len() - 1];
        if idx_str.is_empty() {
            return None;
        }
        let idx = idx_str.parse::<usize>().ok()?;
        Some((name, Some(idx)))
    } else {
        Some((s, None))
    }
}

fn read_u64_le(slice: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(slice);
    u64::from_le_bytes(buf)
}

fn read_u32_le(slice: &[u8]) -> u32 {
    let mut buf = [0u8; 4];
    buf.copy_from_slice(slice);
    u32::from_le_bytes(buf)
}

fn read_u16_le(slice: &[u8]) -> u16 {
    let mut buf = [0u8; 2];
    buf.copy_from_slice(slice);
    u16::from_le_bytes(buf)
}

fn malloc_state_struct_64() -> CStructDef {
    use FieldType::*;
    let fields = vec![
        FieldDef { typ: Int, name: "mutex", count: 1 },
        FieldDef { typ: Int, name: "flags", count: 1 },
        FieldDef { typ: Int, name: "have_fastchunks", count: 1 },
        FieldDef { typ: Int, name: "align", count: 1 },
        FieldDef { typ: Ptr, name: "fastbinsY", count: 10 },
        FieldDef { typ: Ptr, name: "top", count: 1 },
        FieldDef { typ: Ptr, name: "last_remainder", count: 1 },
        FieldDef { typ: Ptr, name: "bins", count: 254 },
        FieldDef { typ: Int, name: "binmap", count: 4 },
        FieldDef { typ: Ptr, name: "next", count: 1 },
        FieldDef { typ: Ptr, name: "next_free", count: 1 },
        FieldDef { typ: SizeT, name: "attached_threads", count: 1 },
        FieldDef { typ: SizeT, name: "system_mem", count: 1 },
        FieldDef { typ: SizeT, name: "max_system_mem", count: 1 },
    ];
    CStructDef::new(fields)
}


fn malloc_chunk_struct_64() -> CStructDef {
    use FieldType::*;
    let fields = vec![
        FieldDef { typ: SizeT, name: "prev_size", count: 1 },
        FieldDef { typ: SizeT, name: "size", count: 1 },
        FieldDef { typ: Ptr, name: "fd", count: 1 },
        FieldDef { typ: Ptr, name: "bk", count: 1 },
        FieldDef { typ: Ptr, name: "fd_nextsize", count: 1 },
        FieldDef { typ: Ptr, name: "bk_nextsize", count: 1 },
    ];
    CStructDef::new(fields)
}

fn tcache_perthread_struct_64() -> CStructDef {
    use FieldType::*;
    let fields = vec![
        FieldDef { typ: Int16, name: "counts", count: 64 },
        FieldDef { typ: Ptr, name: "entries", count: 64 },
    ];
    CStructDef::new(fields)
}

pub fn malloc_state_generator(_version: &str) -> CStructDef {
    malloc_state_struct_64()
}

pub fn malloc_chunk_generator(_version: &str) -> CStructDef {
    malloc_chunk_struct_64()
}

pub fn tcache_struct_generator(_version: &str) -> CStructDef {
    tcache_perthread_struct_64()
}
