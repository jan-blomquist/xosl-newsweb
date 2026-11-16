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

gRPC is the only interface. The database schema and the file layout are
internal and can change between versions, so no other program should read
them directly.

Out of scope: search, text extraction, classification and anything else
derived from the disclosures. Those belong in the programs that consume the
mirror.

## The API

The contract is the set of `.proto` files in
[`contracts/`](contracts/newsweb/proto/com/vizroy/xosl/newsweb/v1), package
`com.vizroy.xosl.newsweb.v1`.

One service, `NewswebService`:

| Call | What it does |
|---|---|
| `ListDisclosures` | Streams the disclosures in a time range, for chosen tickers, newest first. |
| `GetDisclosure` | Returns one disclosure by its NewsWeb message id, the number in its address. |
| `ReadAttachment` | Streams an attachment's bytes. |
| `WatchEvents` | Replays what changed since a point the caller names, then follows new events live. |

Any gRPC client can be generated from those files. The Rust code generated
from them is committed under `src/gen`.

## Load on the exchange

The mirror reads a public website that publishes no rate limit, so its
defaults keep the load low:

- Ingest is off until you enable it; until then it sends NewsWeb no request.
- Requests are rate-limited, one per second by default.
- A new mirror fetches the last few days. The full history is hundreds of
  thousands of disclosures and several hundred gigabytes of attachments, and
  has to be asked for.
- Every request carries a `User-Agent` that names `xosl-newsweb` and its
  version.
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
   just check              # everything a pull request must pass
   just test               # the unit tests only
   just test-integration   # the integration tests only
   just test-live          # the tests that call the real exchange
   just gen                # regenerate the Rust code after changing a .proto file
   just build              # the release binary, in target/release/
   ```

   From the host, `vz -- just check` runs one command in the shell.

Without viz-shell, the same recipes work with Rust at the version in
`rust-toolchain.toml` and [just](https://github.com/casey/just) installed.

The tests are in two places, by what they touch:

- **Unit tests** are inline in `src/`, beside the code. They run in memory:
  no file, no database, no network, no real clock.
- **Integration tests** are under `tests/`. `tests/store.rs` runs the schema
  and the queries against SQLite. `tests/live.rs` calls the real exchange and
  is ignored unless run with `just test-live`; the gate never runs it.

Changes arrive as pull requests, squash-merged once the `gate` workflow is
green. The gate runs `just check`: format, clippy, buf lint, the unit tests
and the integration tests.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
