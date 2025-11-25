mod internals;
use internals::proc::Proc;
use internals::proc::Map;

fn main() {
    let proc = Proc::new(8912);
    let maps: Vec<Map> = proc.vmmap();
    let ranges = proc.ranges();
    let mem = proc.read_mem(94374733324288, 16);
    println!("{:#?}", maps);
    println!("{:#?}", ranges);
    println!("{:?}", mem);
}

