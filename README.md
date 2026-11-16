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
| `--listen <ADDRESS>` | `XOSL_NEWSWEB_LISTEN` | `127.0.0.1:50051` | The address of the gRPC server. The container image sets `0.0.0.0:50051`. |
| `--log-format <FORMAT>` | `XOSL_NEWSWEB_LOG_FORMAT` | `text` | How log lines are written: `text` or `json`. |
| `--webhook-url <URL>` | `XOSL_NEWSWEB_WEBHOOK_URL` | none | Sends each event to this URL. See "Webhook". |
| `--webhook-secret <SECRET>` | `XOSL_NEWSWEB_WEBHOOK_SECRET` | none | Signs each webhook request. |

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

### In a container

Each version is published as `ghcr.io/jan-blomquist/xosl-newsweb:<version>`,
for `linux/amd64`. The image holds the binary on a base without a shell or a
package manager, and runs it as user 65532. In the image the data folder is
`/data` and the server listens on `0.0.0.0:50051`.

[`compose.yaml`](compose.yaml) is a sample: ingest on, the data in `./data`,
the API published to the host only, a read-only file system and no
capabilities.

```
mkdir data
docker compose up -d
docker compose ps                                   # shows the health
docker compose exec xosl-newsweb /xosl-newsweb status
```

`./data` must belong to the user the service runs as: the sample sets
`user: "1000:1000"`.

Each image carries a signed record of the workflow run and the commit it was
built from. To verify it with the [GitHub CLI](https://cli.github.com):

```
gh attestation verify oci://ghcr.io/jan-blomquist/xosl-newsweb:<version> \
  --repo jan-blomquist/xosl-newsweb
```

To back the mirror up, copy the data folder while the service is stopped. A
copy taken while it runs must include `mirror.sqlite-wal` and can still be
inconsistent; `sqlite3 mirror.sqlite ".backup <file>"` takes a consistent copy
of the database from a running mirror.

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
(exit code 1) when the call fails within 5 seconds. `--address <URL>`
(`XOSL_NEWSWEB_ADDRESS`, default `http://127.0.0.1:50051`) names the server.

The container image has no shell and no curl, so its health check runs the
binary itself, every 30 seconds:

```dockerfile
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD ["/xosl-newsweb", "health"]
```

Docker and Docker Compose show the result as the container's health, with no
setting of their own.

### Webhook

With `--webhook-url` set, the mirror sends each event of its log to that URL
as an HTTP POST, so another program can react to a new disclosure without
holding a connection open.

- **The body** is the event as JSON: its `seq`, its kind, and the disclosure
  as it stood when the event was written. It is the JSON that `watch --json`
  prints. 64-bit numbers, such as `seq` and `messageId`, are JSON strings.
- **Headers:** `Content-Type: application/json` and `X-Xosl-Newsweb-Seq`
  with the event's `seq`.
- **Order:** one event at a time, in `seq` order. The next event is sent
  only after the receiver answered the current one with a 2xx status.
- **Retries:** a delivery that fails is tried again after 1 second, then
  with the delay doubled each time, up to 5 minutes, until it succeeds. No
  event is skipped, so a receiver that is down delays every later event. A
  retry can deliver an event twice; a receiver can tell by the `seq`.
- **Start:** the first time a webhook is set, delivery starts with the next
  event, not with the events already in the log. The position is stored, so
  a restart continues where it stopped.
- **Signature:** with `--webhook-secret` set, each request carries
  `X-Xosl-Newsweb-Signature: sha256=<hex>`, the HMAC-SHA256 of the body under
  the secret. A receiver that computes the same value knows the request came
  from the mirror.

## Commands

The same binary calls a running mirror and prints the answer.

| Command | What it does |
|---|---|
| `list [--from <TIME>] [--before <TIME>] [--sign <SIGN>]... [--limit <N>]` | Prints the disclosures in a time range, newest first: time, message id, issuer sign, title. `--limit` stops after N. |
| `get <MESSAGE_ID>` | Prints one disclosure: its fields, its attachments and its body. |
| `attachment <MESSAGE_ID> <ATTACHMENT_ID> [--output <FILE>]` | Writes an attachment's bytes to the file, or to stdout. |
| `watch [--since <SEQ>]` | Prints events as they are written, until interrupted. Without `--since` it starts after the latest event; `--since 0` replays the whole log. |
| `status` | Prints the version, the counts, the days known to be complete and each job's last success and failure. |
| `failures [--message-id <ID>] [--limit <N>]` | Prints the failure log, newest first. |
| `health` | Prints `healthy` or `unhealthy`; see Health. |

Every command takes `--address <URL>` (`XOSL_NEWSWEB_ADDRESS`, default
`http://127.0.0.1:50051`), the address of the mirror's gRPC server. Every
command except `attachment` and `health` takes `--json`: one proto-JSON
object per line on stdout (a disclosure, an event, a status or a failure),
and nothing else. Without it the output is text, with times as RFC 3339 in
UTC.

`<TIME>` is an RFC 3339 timestamp (`2026-11-16T08:00:00Z`) or a date
(`2026-11-16`), which means the start of that day in Oslo. `--from` is
inclusive and `--before` is exclusive.

A failed call prints one line on stderr with the status code and the
server's message, for example `not found: no disclosure 12`, and exits with
1. A mirror that cannot be reached prints `unreachable: <cause>` and exits
with 1.

```
xosl-newsweb list --sign EQNR --from 2026-11-01
xosl-newsweb attachment 654321 7 --output report.pdf
xosl-newsweb watch --json
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
   with every tool at a pinned version, described by `vz.yml` and `dev.Dockerfile`:

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

A release is a tag: `v<version>`, with the version in `Cargo.toml`, on a
commit of `master`. The `image` workflow builds the image from `Dockerfile`
and publishes it. A pull request that changes what the image is built from
builds it too, starts it and waits for its health check to pass.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
