//! `xosl-newsweb`: a self-hosted mirror of Oslo Børs NewsWeb.
//!
//! The library holds everything; `main.rs` is the binary's entry point. The
//! split exists so the integration tests under `tests/` can call the public
//! API.

pub mod cli;
pub mod clock;
pub mod exchange;
pub mod files;
pub mod health;
pub mod ingest;
pub mod logging;
pub mod pb;
pub mod server;
pub mod store;
