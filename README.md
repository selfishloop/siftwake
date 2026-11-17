<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/brand/siftwake-logo-dark.svg">
  <img src="assets/brand/siftwake-logo.svg" alt="Siftwake" width="540">
</picture>

# Siftwake

**Document storage. Content search.**

Siftwake is an embedded document database and content search engine written in
Rust. It stores versioned documents, indexes extracted text and metadata, and
publishes consistent snapshots while a single writer processes updates.

**Current version — 0.1.5-preview.** APIs are under development.
[Version history](docs/VERSIONING.md)

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
cargo build --release --bin siftwake
.\target\release\siftwake.exe data/mydb --stdio
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

## Benchmarks

50,000 documents from the same audited Turkish cap200k corpus. Streamed ingest,
500 documents per commit. Siftwake and Tantivy were measured together on
16 November 2026. The SQLite FTS5 trigram baseline was measured on
29 October 2026 with the same document prefix in a separate session.

| Measurement | Siftwake | SQLite FTS5 trigram | Tantivy 0.27.0 |
| --- | ---: | ---: | ---: |
| Ingest + maintenance, two runs | 39.61–46.43 s | 49.55–50.11 s | 16.44–16.79 s |
| Retained disk after maintenance | 300.84 MB | 421.74 MB | 137.82 MB |
| Ingest peak private commit | 17.85–17.86 MiB | 17.95–18.01 MiB | 37.42–38.40 MiB |
| Ingest peak working set | 21.93–27.84 MiB | 19.90–19.94 MiB | 68.89–74.56 MiB |
| Idle private commit | 2.37–2.38 MiB | 3.02 MiB | 1.10–1.11 MiB |
| Warm open to first result p95, `olduğu` | 217.93 ms | 252.42 ms | 12.43 ms |

Private commit and working set are OS process measurements. Siftwake uses
strict open. Query results contain full documents in key order.
[Settings and evidence](docs/PERFORMANCE.md)

<details>
<summary>All 41 warm queries — first 50 results, p95 in milliseconds</summary>

| Query | Siftwake | SQLite FTS5 trigram | Tantivy 0.27.0 |
| --- | ---: | ---: | ---: |
| olduğu | 1.850 | 360.281 | 4.004 |
| büyük | 1.582 | 345.915 | 4.025 |
| ayrıca | 1.834 | 422.913 | 3.391 |
| arasında | 2.192 | 553.408 | 4.023 |
| birlikte | 2.166 | 382.420 | 4.543 |
| sanat | 2.734 | 371.881 | 5.298 |
| görenler | 5.065 | 368.515 | 7.950 |
| millî | 1.753 | 110.818 | 2.947 |
| ikinci | 2.764 | 434.531 | 4.877 |
| toplam | 2.131 | 312.824 | 5.026 |
| isimli | 5.311 | 442.418 | 10.721 |
| öldürür | 19.680 | 198.867 | 32.385 |
| ölümü | 2.404 | 333.069 | 5.125 |
| akarsu | 11.449 | 143.657 | 18.354 |
| demokratik | 4.053 | 70.846 | 5.326 |
| çalışmalarıyla | 58.994 | 280.753 | 68.135 |
| üreticisi | 23.877 | 216.947 | 39.057 |
| joseph | 1.662 | 45.236 | 3.661 |
| leyla | 37.446 | 301.401 | 61.639 |
| protest | 5.322 | 116.706 | 10.652 |
| furlong | 5.713 | 9.698 | 10.583 |
| korkaklığa | 22.323 | 37.477 | 40.347 |
| ormanların | 82.899 | 361.930 | 133.160 |
| yanıklarda | 86.421 | 269.177 | 382.200 |
| külliyatı | 76.970 | 233.332 | 303.440 |
| timmy | 11.535 | 21.975 | 22.297 |
| yüklük | 15.473 | 201.590 | 26.307 |
| ilçesine köyleri | 3.823 | 98.630 | 4.959 |
| geldiği buldu | 26.043 | 245.433 | 40.127 |
| chicago studio | 5.292 | 13.236 | 9.533 |
| kısım dişli | 92.164 | 294.378 | 187.835 |
| güzeli zafer | 25.668 | 41.669 | 45.454 |
| İnternetten hazinem | 58.688 | 128.193 | 126.172 |
| oyuncudur atlanta | 42.154 | 69.294 | 89.867 |
| kendisine çıkacağını | 72.279 | 219.625 | 291.662 |
| ve | 0.699 | 546.692 | 2.337 |
| if | 1.541 | 682.061 | 3.925 |
| id | 1.098 | 625.513 | 2.700 |
| zx | 97.019 | 621.388 | 1028.157 |
| qzxkvwnever | 0.416 | 0.423 | 0.108 |
| transubstantiationz | 1.485 | 3.290 | 0.475 |

</details>

[Siftwake/Tantivy report](docs/TANTIVY-COMPARISON-50K-20261116.md) ·
[SQLite baseline](docs/TRWIKI-CAP200K-SCALE-20261029.md)

The separate cold-cache run on 15 November used the existing 50k churn layout
and measured open-start to the first 50 `olduğu` results.

| Cold-cache measurement | Siftwake | SQLite FTS5 trigram | Tantivy |
| --- | ---: | ---: | ---: |
| Open to first result, two trials | 885–909 ms | 6,663–6,696 ms | — |

The Tantivy cold-cache result is unmeasured.
[Cold-cache evidence](docs/COLD-OPEN-PAIRED-50K-20261115.md)

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
