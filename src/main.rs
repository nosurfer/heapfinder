mod internals;
use internals::proc::Proc;
use internals::proc::Map;

fn main() {
    let proc = Proc::new(38699);
    let maps: Vec<Map> = proc.vmmap();
    let ranges = proc.ranges();
    println!("{:#?}", maps);
    println!("{:#?}", ranges);
}

