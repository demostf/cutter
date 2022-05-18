use clap::Parser;
use democutter::bookmarks;
use std::fs;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the demo
    path: String,
}

fn main() {
    let args = Args::parse();
    let file = fs::read(&args.path).unwrap();
    let output = bookmarks(&file);
    println!("{:?}", output);
}
