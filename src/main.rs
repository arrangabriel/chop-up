use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::exit;

use wasm_manipulation::parse_wast_string;

fn main() {
    let config = parse_config(env::args().collect()).unwrap_or_else(|| {
        println!("Usage: [wasm-manipulator] file-path");
        exit(1)
    });

    let file_path = Path::new(config.file_path.as_str());
    let mut wat_string = String::new();

    if !file_path.is_file() {
        println!("No such file: {:?}", config.file_path.as_str());
        exit(1);
    }

    File::open(file_path)
        .and_then(|mut file| file.read_to_string(&mut wat_string))
        .unwrap_or_else(|err| {
            println!("Failed to read file: {:?}", err);
            exit(1);
        });

    parse_wast_string(wat_string.as_str(), config.print, config.skip_safe).unwrap_or_else(|err| {
        println!("Failed to parse: {:?}", err);
        exit(1);
    });
}

struct Config {
    file_path: String,
    print: bool,
    skip_safe: bool,
}

fn parse_config(mut args: Vec<String>) -> Option<Config> {
    args.remove(0);
    let file_path = args
        .iter()
        .position(|arg| !arg.starts_with("-"))
        .map(|pos| args.remove(pos))?;

    let print = check_flag(&mut args, "-p");

    let skip_safe = check_flag(&mut args, "--skip-safe");

    Some(Config {
        file_path,
        print,
        skip_safe,
    })
}

fn check_flag(args: &mut Vec<String>, flag: &str) -> bool {
    args.iter()
        .position(|arg| arg.as_str().eq(flag))
        .map(|pos| args.swap_remove(pos))
        .is_some()
}
