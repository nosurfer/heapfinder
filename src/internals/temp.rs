use std::collections::HashMap;
use std::convert::TryInto;

/// Типы полей, которые мы поддерживаем (как в оригинале).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldType {
    Bool,   // 1
    Byte,   // 1
    Char,   // 1
    Int,    // 4
    Ptr,    // 8 (только 64-бита)
    SizeT,  // 8 (только 64-бита)
}

impl FieldType {
    /// Размер в байтах (для 64-битной архитектуры).
    fn size(self) -> usize {
        match self {
            FieldType::Bool | FieldType::Byte | FieldType::Char => 1,
            FieldType::Int => 4,
            FieldType::Ptr | FieldType::SizeT => 8,
        }
    }
}

/// Описание поля в структуре
#[derive(Debug, Clone)]
struct FieldDef {
    typ: FieldType,
    name: String,
    num: usize, // количество элементов (1 = скаляр, >1 = массив)
}

/// Описание структуры (аналог "код" в вашем Python, но явно)
#[derive(Debug, Clone)]
struct StructDef {
    name: String,
    fields: Vec<FieldDef>,
}

impl StructDef {
    /// Создать описание структуры (например malloc_state, malloc_chunk и т.д.)
    fn new(name: &str, fields: Vec<FieldDef>) -> Self {
        Self {
            name: name.to_string(),
            fields,
        }
    }

    /// Суммарный размер структуры
    fn size(&self) -> usize {
        self.fields
            .iter()
            .map(|f| f.typ.size() * f.num)
            .sum()
    }

    /// Смещение поля (поддерживается "name" и "name[index]").
    fn offset_of(&self, var: &str) -> Option<usize> {
        let (name, index) = parse_name_index(var)?;
        let mut offset = 0usize;
        for f in &self.fields {
            if f.name == name {
                if index >= f.num {
                    return None; // выход за границы
                }
                offset += index * f.typ.size();
                return Some(offset);
            }
            offset += f.typ.size() * f.num;
        }
        None
    }

    /// Полный размер поля (если указано без индекса — весь массив).
    fn sizeof(&self, var: &str) -> Option<usize> {
        let (name, index_opt) = parse_name_index(var)?;
        for f in &self.fields {
            if f.name == name {
                return if let Some(_) = index_opt {
                    Some(f.typ.size())
                } else {
                    Some(f.typ.size() * f.num)
                };
            }
        }
        None
    }

    /// Создаёт экземпляр структуры из дампа памяти и адреса (это аналог `_new`).
    fn new_instance(&self, mut memdump: Vec<u8>, addr: u64) -> CStructInstance {
        // дополняем до размера структуры нулями
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

/// Экземпляр структуры (с конкретным дампом и адресом)
#[derive(Debug, Clone)]
struct CStructInstance {
    def: StructDef,
    mem: Vec<u8>,
    addr: u64,
}

/// Значение поля, которое может вернуть get_field
#[derive(Debug, Clone)]
enum FieldValue {
    Bytes(Vec<u8>),          // для char/byte/bool и т.д.
    U32(u32),                // int (4 байта)
    U64(u64),                // ptr / size_t (8 байт)
    Array(Vec<FieldValue>),  // массив значений
}

impl CStructInstance {
    /// Возвращает общий размер структуры
    fn size(&self) -> usize {
        self.def.size()
    }

    /// Адрес поля (addr + offset)
    fn addrof(&self, var: &str) -> Option<u64> {
        self.def.offset_of(var).map(|off| self.addr + off as u64)
    }

    /// Смещение поля внутри структуры
    fn offset(&self, var: &str) -> Option<usize> {
        self.def.offset_of(var)
    }

    /// Размер поля
    fn sizeof(&self, var: &str) -> Option<usize> {
        self.def.sizeof(var)
    }

    /// Получить значение поля (поддерживает name и name[index])
    fn get_field(&self, var: &str) -> Option<FieldValue> {
        let (name, index_opt) = parse_name_index(var)?;
        // найти описание поля
        let f = self.def.fields.iter().find(|f| f.name == name)?;
        let a_size = f.typ.size();

        if f.num > 1 && index_opt.is_none() {
            // вернуть весь массив как Vec<FieldValue>
            let mut res = Vec::with_capacity(f.num);
            for i in 0..f.num {
                let off = self
                    .def
                    .offset_of(&format!("{}[{}]", name, i))
                    .unwrap_or(0);
                let slice = &self.mem[off..off + a_size];
                res.push(parse_value_from_slice(f.typ, slice));
            }
            Some(FieldValue::Array(res))
        } else {
            // скаляр или конкретный индекс
            let idx = index_opt.unwrap_or(0);
            if idx >= f.num {
                return None;
            }
            let off = self
                .def
                .offset_of(&format!("{}[{}]", name, idx))
                .unwrap_or(0);
            let slice = &self.mem[off..off + a_size];
            Some(parse_value_from_slice(f.typ, slice))
        }
    }

    /// Аналог `_copy` — клонирует экземпляр структуры
    fn copy(&self) -> Self {
        self.clone()
    }

    /// Установить внутренние данные поля (замена `_init` поведения в Python).
    /// Перезаписывает весь дамп и адрес.
    fn init(&mut self, mut memdump: Vec<u8>, addr: u64) {
        let needed = self.size();
        if memdump.len() < needed {
            memdump.resize(needed, 0u8);
        }
        self.mem = memdump;
        self.addr = addr;
    }

    /// Удобный метод — собрать словарь (map) всех полей к их "сырым" вариантам.
    /// Полезно для отладки/печати.
    fn to_map(&self) -> HashMap<String, FieldValue> {
        let mut map = HashMap::new();
        for f in &self.def.fields {
            if f.num > 1 {
                if let Some(FieldValue::Array(arr)) = self.get_field(&f.name) {
                    map.insert(f.name.clone(), FieldValue::Array(arr));
                }
            } else {
                if let Some(v) = self.get_field(&f.name) {
                    map.insert(f.name.clone(), v);
                }
            }
        }
        map
    }
}

/// Распарсить "name" или "name[index]" -> (name, Option<index>)
fn parse_name_index(s: &str) -> Option<(&str, Option<usize>)> {
    if let Some(open) = s.find('[') {
        if s.ends_with(']') {
            let name = &s[..open];
            let idx_str = &s[open + 1..s.len() - 1];
            if idx_str.is_empty() {
                return None;
            }
            if let Ok(idx) = idx_str.parse::<usize>() {
                return Some((name, Some(idx)));
            } else {
                return None;
            }
        } else {
            return None;
        }
    } else {
        Some((s, None))
    }
}

/// Преобразовать срез байт в FieldValue в зависимости от типа
fn parse_value_from_slice(typ: FieldType, slice: &[u8]) -> FieldValue {
    match typ {
        FieldType::Int => {
            let bytes: [u8; 4] = slice.try_into().unwrap_or([0u8; 4]);
            FieldValue::U32(u32::from_le_bytes(bytes))
        }
        FieldType::Ptr | FieldType::SizeT => {
            let bytes: [u8; 8] = slice.try_into().unwrap_or([0u8; 8]);
            FieldValue::U64(u64::from_le_bytes(bytes))
        }
        FieldType::Bool | FieldType::Byte | FieldType::Char => {
            FieldValue::Bytes(slice.to_vec())
        }
    }
}

/// Примеры генераторов описаний структур (только 64-битные версии)
fn malloc_state_struct_64() -> StructDef {
    use FieldType::*;
    let f = vec![
        FieldDef { typ: Int, name: "mutex".into(), num: 1 },
        FieldDef { typ: Int, name: "flags".into(), num: 1 },
        FieldDef { typ: Int, name: "have_fastchunks".into(), num: 1 },
        FieldDef { typ: Int, name: "align".into(), num: 1 },
        FieldDef { typ: Ptr, name: "fastbinsY".into(), num: 10 },
        FieldDef { typ: Ptr, name: "top".into(), num: 1 },
        FieldDef { typ: Ptr, name: "last_remainder".into(), num: 1 },
        FieldDef { typ: Ptr, name: "bins".into(), num: 254 },
        FieldDef { typ: Int, name: "binmap".into(), num: 4 },
        FieldDef { typ: Ptr, name: "next".into(), num: 1 },
        FieldDef { typ: Ptr, name: "next_free".into(), num: 1 },
        FieldDef { typ: SizeT, name: "attached_threads".into(), num: 1 },
        FieldDef { typ: SizeT, name: "system_mem".into(), num: 1 },
        FieldDef { typ: SizeT, name: "max_system_mem".into(), num: 1 },
    ];
    StructDef::new("malloc_state", f)
}

fn malloc_chunk_struct_64() -> StructDef {
    use FieldType::*;
    let f = vec![
        FieldDef { typ: SizeT, name: "prev_size".into(), num: 1 },
        FieldDef { typ: SizeT, name: "size".into(), num: 1 },
        FieldDef { typ: Ptr, name: "fd".into(), num: 1 },
        FieldDef { typ: Ptr, name: "bk".into(), num: 1 },
        FieldDef { typ: Ptr, name: "fd_nextsize".into(), num: 1 },
        FieldDef { typ: Ptr, name: "bk_nextsize".into(), num: 1 },
    ];
    StructDef::new("malloc_chunk", f)
}

fn tcache_perthread_struct_64() -> StructDef {
    use FieldType::*;
    let f = vec![
        FieldDef { typ: Char, name: "counts".into(), num: 64 },
        FieldDef { typ: Ptr, name: "entries".into(), num: 64 },
    ];
    StructDef::new("tcache_perthread", f)
}

/// Пример использования
fn main() {
    // Создадим описание malloc_state (64-bit)
    let def = malloc_state_struct_64();

    // Создадим "дамп памяти" (в реальности вы бы прочитали его из процесса).
    // Для демонстрации заполняем нулями и установим несколько полей вручную.
    let mut mem = vec![0u8; def.size()];

    // Пример: положим значение u64 (ptr) в fastbinsY[2]
    let some_ptr: u64 = 0x4141414142424242;
    // находим оффсет нужного элемента
    if let Some(off) = def.offset_of("fastbinsY[2]") {
        mem[off..off + 8].copy_from_slice(&some_ptr.to_le_bytes());
    }

    // создаём экземпляр структуры (аналог _new)
    let instance = def.new_instance(mem, 0x7fff_0000_0000u64);

    // читаем fastbinsY[2]
    if let Some(FieldValue::U64(v)) = instance.get_field("fastbinsY[2]") {
        println!("fastbinsY[2] = 0x{:x}", v);
    } else {
        println!("Не удалось прочитать fastbinsY[2]");
    }

    // получим адрес поля
    if let Some(a) = instance.addrof("fastbinsY[2]") {
        println!("Адрес fastbinsY[2] = 0x{:x}", a);
    }

    // Пример получения массива (первые 3 элементов)
    if let Some(FieldValue::Array(arr)) = instance.get_field("fastbinsY") {
        for (i, v) in arr.iter().enumerate().take(3) {
            match v {
                FieldValue::U64(x) => println!("fastbinsY[{}] = 0x{:x}", i, x),
                FieldValue::Bytes(b) => println!("fastbinsY[{}] raw = {:?}", i, b),
                _ => println!("fastbinsY[{}] other", i),
            }
        }
    }
}
