# xosl-newsweb

A self-hosted mirror of [Oslo Børs NewsWeb](https://newsweb.oslobors.no/):
company disclosures and their attachments, kept on your own disk and served
over gRPC.

Unofficial. Not affiliated with, or endorsed by, Euronext or Oslo Børs.

## What it is for

NewsWeb is where companies listed in Oslo publish what the market must know:
financial reports, insider trades, large shareholdings, contract awards,
general meetings. It is a website made for reading one disclosure at a time.

Analysis needs something else: every disclosure of one company, everything
published last night, the PDFs themselves, and a way to hear about a new
disclosure the moment it appears. `xosl-newsweb` keeps your own copy and
answers those questions from it, so your programs talk to your mirror and not
to the exchange.

## What it will do

One binary and one data folder. There is no database server to run and
nothing else to install:

- **Poll** NewsWeb for new disclosures and store each one, with its metadata,
  in an embedded SQLite database.
- **Download** each disclosure's attachments (mostly PDFs) as files beside the
  database, or into an S3-compatible bucket if you prefer one.
- **Backfill** earlier days on request, newest first, as far back as you ask.
- **Serve** it all over gRPC: list and fetch disclosures, read attachments,
  and follow a stream of events that tells a consumer what is new and lets it
  resume where it left off.
- **Report** on itself: what it holds, how far behind it is, what failed.

The same binary carries a command-line client for those calls, for people and
for scripts.

gRPC is the only way in. The database and the files are the mirror's own
business, so nothing else should read them directly.

What it leaves to others: search, text extraction, classification and anything
else derived from the disclosures. Those belong in the programs that consume
the mirror.

## A well-behaved guest

The mirror is a guest of the exchange's website and is built to act like one:

- It contacts NewsWeb only when you switch that on.
- Requests are rate-limited, one per second by default.
- A new mirror fetches the last few days. The full history is hundreds of
  thousands of disclosures and several hundred gigabytes of attachments, and
  has to be asked for.
- It identifies itself honestly in its `User-Agent`.
- The repository ships code and never data. What you mirror is yours to keep
  within NewsWeb's terms of use, which you should read: they govern what you
  may do with the content.

## Development

1. Open the dev shell with
   [viz-shell](https://github.com/jan-blomquist/viz-shell). It is a container
   with every tool at a pinned version, described by `vz.yml` and `Dockerfile`:

   ```
   vz new
   ```

2. Inside it, run the checks or build the binary:

   ```
   just check     # format, clippy, tests: what every pull request must pass
   just build     # the release binary, in target/release/
   ```

   From the host, `vz -- just check` runs one command in the shell.

Without viz-shell, the same recipes work with Rust at the version in
`rust-toolchain.toml` and [just](https://github.com/casey/just) installed.

Changes arrive as pull requests, squash-merged once the `gate` workflow is
green. The gate runs `just check`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
