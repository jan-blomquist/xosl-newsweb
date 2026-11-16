//! The `xosl-newsweb` binary: the mirror and its command-line client.
//!
//! For now it prints its version.

fn main() {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
}
