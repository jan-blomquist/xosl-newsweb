# xosl-newsweb

A self-hosted mirror of [Oslo Børs NewsWeb](https://newsweb.oslobors.no/):
company disclosures and their attachments, kept on your own disk.

Unofficial. Not affiliated with, or endorsed by, Euronext or Oslo Børs.

## What it is for

NewsWeb is where companies listed in Oslo publish what the market must know:
financial reports, insider trades, large shareholdings, contract awards,
general meetings. It is a website made for reading one disclosure at a time.

Analysis needs something else: every disclosure of one company, everything
published last night, the PDFs themselves, and a way to hear about a new
disclosure the moment it appears. `xosl-newsweb` keeps your own copy, and its
gRPC interface answers those questions from it, so your programs talk to
your mirror and not to the exchange.

## What it does

One binary and one data folder. There is no database server to run and
nothing else to install:

- **Poll** NewsWeb for new disclosures and store each one, with its metadata,
  in an embedded SQLite database.
- **Download** each disclosure's attachments (mostly PDFs) as files beside the
  database.
- **Backfill** earlier days on request, newest first, as far back as you ask.
- **Serve** it all over gRPC: list and fetch disclosures, read attachments,
  and follow a stream of events that tells a consumer what is new and lets it
  resume where it left off.
- **Report** on itself: what it holds, how far behind it is, what failed.

gRPC is the only interface. The database schema and the file layout are
internal and can change between versions, so no other program should read
them directly.

A command-line client for those calls, in the same binary, does not exist
yet.

## Running

```
xosl-newsweb run --ingest
```

`run` opens the data folder and, with `--ingest`, lists today once a poll
interval, lists the past days of the lookback range until each is known
complete, reads every listed disclosure in full and downloads its attachments.
It runs until SIGINT or SIGTERM. Without `--ingest` it opens the data folder,
logs which past days are complete, and sends the exchange no request.

Each setting is a flag and an environment variable; `xosl-newsweb run --help`
lists them.

| Flag | Variable | Default | Meaning |
|---|---|---|---|
| `--data-dir <DIR>` | `XOSL_NEWSWEB_DATA_DIR` | `./data` | The data folder; created when missing. |
| `--ingest` | `XOSL_NEWSWEB_INGEST` | off | Copy NewsWeb into the data folder. The variable takes `true` or `false`. |
| `--request-interval-ms <N>` | `XOSL_NEWSWEB_REQUEST_INTERVAL_MS` | `1000` | The shortest time between two requests to the exchange; at least 1000. |
| `--poll-interval-secs <N>` | `XOSL_NEWSWEB_POLL_INTERVAL_SECS` | `60` | How often today is listed; at least 1. |
| `--lookback-days <N>` | `XOSL_NEWSWEB_LOOKBACK_DAYS` | `7` | How many past days are kept complete; at least 1. |
| `--listen <ADDRESS>` | `XOSL_NEWSWEB_LISTEN` | `127.0.0.1:50051` | The address of the gRPC server. In a container, listen on `0.0.0.0:50051`. |
| `--log-format <FORMAT>` | `XOSL_NEWSWEB_LOG_FORMAT` | `text` | How log lines are written: `text` or `json`. |

Days are Oslo calendar days. A past day is complete once a listing of it,
made at least two hours after the day ended, was not truncated and decoded in
full. A day the exchange truncates is listed again once a day.

The data folder holds:

- `mirror.sqlite`: the database. While `run` runs, SQLite keeps
  `mirror.sqlite-wal` and `mirror.sqlite-shm` beside it.
- `attachments/<message id>/<attachment id>`: the bytes of each attachment.

Out of scope: search, text extraction, classification and anything else
derived from the disclosures. Those belong in the programs that consume the
mirror.

### Health

`run` starts a gRPC server on the `--listen` address, with or without
`--ingest`. Beside the mirror's own service it serves the standard
`grpc.health.v1.Health` service.

The mirror is healthy when ingest is off, or when the poller listed today
successfully within the last 10 minutes or three poll intervals, whichever is
longer. Before the first listing, the time counts from start-up, so a mirror
that has just started is healthy until that limit passes. A failed read or
download, a gap being backfilled and a day without disclosures do not make it
unhealthy. The status is re-evaluated every 10 seconds, and a change is
logged: `info` when it becomes healthy, `error` with the reason when it
becomes unhealthy.

`xosl-newsweb health` asks the server for its status and prints one line:
`healthy` (exit code 0), `unhealthy` (exit code 1), or `unreachable: <cause>`
(exit code 1) when the call fails within 5 seconds. `--address <ADDRESS>`
(`XOSL_NEWSWEB_HEALTH_ADDRESS`, default `http://127.0.0.1:50051`) names the
server. The release image has no shell and no curl, so Docker Compose runs
the binary itself:

```yaml
healthcheck:
  test: ["CMD", "/xosl-newsweb", "health"]
  interval: 30s
  start_period: 60s
```

## Logging

Log lines go to stderr. There are four levels:

| Level | Meaning |
|---|---|
| `error` | The mirror cannot do part of its work and needs a person: the database or the attachment folder cannot be written, start-up failed, a disclosure or an attachment will not be tried again. |
| `warn` | One attempt failed and will be made again: a listing, a read, a download, a retry of a request, a truncated list. |
| `info` | Normal progress: start-up with the settings, coverage, a pass that did work, the periodic status, shutdown. |
| `debug` | Detail of each request to the exchange. |

`RUST_LOG` sets the level; the default is `info`. `RUST_LOG=debug` shows
every request, with its URL, result and duration.

`--log-format text` writes one readable line per event. It is coloured only
when stderr is a terminal, so a container log has no escape sequences.
`--log-format json` writes one JSON object per line, with the timestamp,
level, target and the event's message and fields.

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
| `GetStatus` | Returns what the mirror holds, which days it knows to be complete, and how each ingest job is doing. |
| `ListFailures` | Streams the log of failed attempts, newest first, for everything or for one disclosure. |

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
   just dev                # the mirror against the real exchange, into ./data
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
  and the queries against SQLite; `tests/files.rs` stores attachments in an
  object store in memory and in a temporary folder; `tests/ingest.rs` runs the
  ingest jobs against a fake exchange, with the database and the attachments
  in memory. `tests/live.rs` calls the real exchange and is ignored unless run
  with `just test-live`; the gate never runs it.

`just dev` contacts the real exchange too: roughly 100 to 200 requests, one
per second. It is for manual use, and the gate never runs it.

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
