//! See ARCHITECTURE.md for this crate's place in the import law.
#![forbid(unsafe_code)]

fn main() {
    println!("pbtool {}", env!("CARGO_PKG_VERSION"));
}
