use std::env;
use zip;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.is_empty() {
        println!("din pacate trebuie sa dai numele fisierului ):");
        panic!("Invalid arguments");
    }

    let path = &args[1];

    let file = std::fs::File::open(path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();

    for i in 0..archive.len() {
        let file = archive.by_index(i).unwrap();
        println!("filename: {}", file.name());
    }
}
