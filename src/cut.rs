use democutter::cut;
use std::env;
use std::fs;

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("1 argument required");
        return;
    }
    let path = args[1].clone();
    let file = fs::read(path).unwrap();
    let output = cut(&file, 30000, 50000);
    fs::write("out.dem", output).unwrap();
}
