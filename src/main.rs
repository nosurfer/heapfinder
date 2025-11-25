mod internals;
use internals::proc::Proc;
use internals::proc::Map;

fn main() {
    let proc = Proc::new(4372);
    let maps: Vec<Map> = proc.vmmap();
    let ranges = proc.ranges();
    let mem = proc.read(proc.bases()["a.out"][0], 16);
    println!("{:#?}", maps);
    println!("{:#?}", ranges);
    println!("{:?}", mem);
    println!("{:?}", proc.whereis(140631755595776));
    println!("{:?}", proc.search_in_libc("/bin/sh"));
}

