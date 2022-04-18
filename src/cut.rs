use clap::Parser;
use democutter::cut;
use std::fs;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the source demo
    path: String,
    /// Start tick
    start: u32,
    /// End tick
    end: Option<u32>,
}

fn main() {
    let args = Args::parse();
    let file = fs::read(&args.path).unwrap();
    let output = cut(&file, args.start, args.end.unwrap_or(u32::MAX));
    fs::write("out.dem", output).unwrap();
}
