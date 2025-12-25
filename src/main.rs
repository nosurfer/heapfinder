mod internals;
use internals::core::{Chain, HeapInspector, HeapInspectorConfig};
use libc::geteuid;
use std::env;

fn parse_u64(input: &str) -> Option<u64> {
    if let Some(rest) = input.strip_prefix("0x") {
        u64::from_str_radix(rest, 16).ok()
    } else {
        input.parse::<u64>().ok()
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: {} <pid>",
            args.get(0).map(String::as_str).unwrap_or("heapfinder")
        );
        return;
    }

    let pid = match parse_u64(&args[1]) {
        Some(v) => v,
        None => {
            eprintln!("Invalid pid: {}", args[1]);
            return;
        }
    };
    if unsafe { geteuid() } != 0 {
        eprintln!("Root privileges required. Please run with sudo.");
        return;
    }
    let main_arena_offset = 0x210ac0;
    let tcache_enable = true;

    let config = HeapInspectorConfig {
        main_arena_offset,
        tcache_enable,
        libc_version: None,
    };

    let hi = match HeapInspector::new(pid, config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to initialize HeapInspector: {}", e);
            return;
        }
    };

    let record = hi.record();

    println!("pid: {}", record.pid);
    println!("arch: {:?}", record.arch);
    println!("libc_base: 0x{:x}", record.libc_base);
    println!("heap_base: 0x{:x}", record.heap_base);
    println!("heap_chunks: {}", record.heap_chunks.len());
    println!("fastbins: {}", record.fastbins.len());
    for (i, chunk) in record.heap_chunks.iter().enumerate() {
        println!("heap_chunk[{}]: 0x{:x}", i, chunk.addr());
    }

    print_chain_map("tcache", &hi.tcache_chains());
    if let Some(chain) = hi.unsortedbin_chain() {
        print_chain("unsortedbin", &chain);
    }
    print_chain_map("fastbin", &hi.fastbin_chains());
    print_chain_map("smallbin", &hi.smallbin_chains());
    print_chain_map("largebin", &hi.largebin_chains());
}

fn print_chain_map(label: &str, chains: &std::collections::HashMap<usize, Chain>) {
    let mut keys: Vec<usize> = chains.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        if let Some(chain) = chains.get(&key) {
            print_chain(&format!("{}[{}]", label, key), chain);
        }
    }
}

fn print_chain(label: &str, chain: &Chain) {
    if chain.addrs.is_empty() {
        return;
    }
    let mut out = String::new();
    for (i, addr) in chain.addrs.iter().enumerate() {
        if i > 0 {
            out.push_str(" -> ");
        }
        out.push_str(&format!("0x{:x}", addr));
    }
    if chain.cycle {
        out.push_str(" -> (cycle)");
    }
    println!("{}: {}", label, out);
}
