<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/brand/siftwake-logo-dark.svg">
  <img src="assets/brand/siftwake-logo.svg" alt="Siftwake" width="540">
</picture>

# Siftwake

**Document storage. Content search.**

Siftwake is an embedded document database and content search engine written in
Rust. It stores versioned documents, indexes extracted text and metadata, and
publishes consistent snapshots while a single writer processes updates.

**Current version — 0.1.5-preview.** APIs are under development. The Cargo package,
Rust crate and executable currently use the historical name `fdb`. Reports
written before the rename also use FDB. [Version history](docs/VERSIONING.md)

## What you can build

- Local content indexes for extracted text, with Unicode substring search and
  Turkish character normalization.
- Document stores with scoped keys, JSON metadata, same-scope links and
  version-checked updates.
- Search interfaces with first-page results, cancellation and snapshot readers
  that keep a consistent committed view.

The storage engine, WAL, recovery, immutable segments and indexes are implemented
in this repository. The runtime uses general serialization and checksum
libraries. Separate tools under `tools/` contain benchmark comparators.

## Start locally

The manifest declares Rust 1.89 or newer. Recorded development and validation
use Windows/MSVC with Rust 1.99. Other toolchains need their own validation.

```powershell
cargo build --release --bin fdb
.\target\release\fdb.exe data/mydb --stdio
```

Send one UTF-8 JSON request per line. For example, insert a document and search
its content.

```json
{"op":"commit","mutations":[{"op":"put","guard":{"kind":"missing"},"document":{"key":{"scope":"shared","collection":"documents","id":"readme"},"title":"Project notes","text":"An embedded document search engine","fields":{},"links":[]}}]}
{"op":"search","query":{"scope":"shared","collection":"documents","text":"document engine","limit":50}}
```

For embedded applications, use the Rust API and streamed writes. The CLI exposes
the owned batch API. [Getting started](docs/GETTING-STARTED.md) ·
[Rust API](docs/API.md) · [CLI](docs/CLI.md)

## Design

| Area | Contract |
| --- | --- |
| Identity | A unique `(scope, collection, id)` key |
| Writes | Atomic batches with optional existence or version guards |
| Search | AND substring matching across title and text, results in key order |
| Totals | Exact search or an explicit first-page lower bound |
| Readers | Consistent snapshots with pinned storage |
| Recovery | Checksummed source records and WAL replay |
| Maintenance | Background compaction and revision reclamation with host polling |

Applications provide extracted UTF-8 text, file watching and user authorization.
Siftwake's scope field is an application-supplied data partition.

## Measurements and current work

On the recorded 50k Turkish corpus, Siftwake returned lower p95 in 39 of 41
first-page cases and 40 of 41 exact cases than the configured Tantivy comparator.
Tantivy's successful runs built indexes faster, used less disk and produced a
warm first result sooner. The report includes failed comparator runs, settings,
sample counts and validation differences.
[Measurements](docs/PERFORMANCE.md) · [Full comparison](docs/TANTIVY-COMPARISON-50K-20261116.md)

Current priorities include write latency, index build cost, disk footprint and
startup cost. The Rust API is available. C ABI and application integration have
separate requirements. [Open work](docs/OPEN-WORK.md)

## Documentation

[Documentation index](docs/README.md) · [Engine contracts](docs/ENGINE.md) ·
[Brand guide](docs/BRAND.md) · [Historical engineering notes](docs/ARCHITECTURE.md) ·
[Archived development README](docs/README-FDB-HISTORY.md)

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --tests
```

Read [AGENTS.md](AGENTS.md) before running corpus or performance tooling.

The Cargo manifest declares the MIT license. Logo typography and its font
license are documented in the [brand assets](assets/brand/README.md).
