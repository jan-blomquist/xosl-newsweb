//! The `xosl-newsweb` binary: the mirror and its command-line client.
//!
//! For now it prints its version.

mod exchange;
mod pb;

fn main() {
    println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
}
