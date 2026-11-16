//! The `xosl-newsweb` binary. Everything it does is in the library, starting
//! at `cli::main`.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    xosl_newsweb::cli::main().await
}
