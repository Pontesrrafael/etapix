//! Prints the Project XML the native reader makes of an .mpp: `cargo run --example mpp_dump -- file.mpp`.

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let x = pjsched::mpp::mpp_to_mspdi(&std::fs::read(&a[1]).unwrap()).unwrap();
    println!("{x}");
}
