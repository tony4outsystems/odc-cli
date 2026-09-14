use std::process;

fn main() {
    match odc::run(&std::env::args().collect::<Vec<_>>()[1..]) {
        Ok(_) => process::exit(0),
        Err(e) => {
            eprintln!("error: {}", e);
            process::exit(1);
        }
    }
}
