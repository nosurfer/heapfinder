enum Type {
    Bool,   // 1
    Char,   // 1
    Int,    // 4
    Ptr,    // 8 (только 64-бита)
    SizeT,  // 8 (только 64-бита)
}

impl Type {
    fn size(&self) -> usize {
        match self {
            Type::Bool | Type::Char => 1,
            Type::Int => 4,
            Type::Ptr | Type::SizeT => 8,
        }
    }
}

enum FieldValue {
    Bytes(Vec<u8>),
    U32(u32),
    U64(u64),
    Array(Vec<Field>),
}

struct Field {
    f_type: Type,
    name: String,
    count: usize,
}

struct C_Struct {
    name: String,
    fields: Vec<Field>,
}

struct C_Struct_Instance {
    c_struct: C_Struct,
    mem: Vec<u8>,
    addr: u64,
}