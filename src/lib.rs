//! `xosl-newsweb`: a self-hosted mirror of Oslo Børs NewsWeb.
//!
//! The library holds everything; `main.rs` is the binary's entry point. The
//! split exists so the integration tests under `tests/` can call the public
//! API.

pub mod exchange;
pub mod pb;
pub mod store;
