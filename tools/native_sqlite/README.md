# Native SQLite comparator

The current native OFF1 FDB comparison covers both synthetic writes/reads
and long-passage searches plus 100 dispersed full-document lookups:
[native general comparison](../../docs/NATIVE-SQLITE-GENERAL-20261011.md).
`tools/measure-native-sqlite-general.ps1` runs three alternating pairs for
each corpus with frozen executables. It sets `SQLITE_BENCH_SKIP_INTEGRITY=1`
only for the synthetic timed process, then integrity is checked separately
with the `passages integrity` command. Standalone synthetic integrity remains
enabled by default. The long query process never includes full integrity.

The separate `passages` binary measures the existing 100k diverse 2–50 KiB
book corpus against the frozen default FDB engine. It verifies all input JSON
payload bytes and every count/full-document page. Run instructions and limits:
[long-passage comparison](../../docs/LONG-SQLITE-COMPARISON-20261011.md).
The older synthetic comparator below is unchanged.

This is a separate benchmark package, not an FDB engine dependency. Its own
Cargo.lock pins the bundled SQLite build. Build and execute from the FDB root:

```powershell
cargo build --release --locked --manifest-path tools/native_sqlite/Cargo.toml
.\tools\measure-native.ps1 -Executable tools/native_sqlite/target/release/fdb-native-sqlite-bench.exe -DatabasePath data/new-sqlite -OutputPrefix output/new-sqlite
.\tools\measure-native.ps1 -Executable target/release/examples/measure_scale.exe -DatabasePath data/new-fdb -OutputPrefix output/new-fdb
```

Run sequentially. Both use the same Rust corpus generator and StoredDocument
JSON serializer: one million records, 256-byte repeated Turkish body, batches
of 500, unique title/ID, source/day, count + 50 decoded payloads. Both verify 50
updates/deletes and reopening. Both now time broad text in the same benchmark
process after the changes, before any process-kill sentinel. FDB's full source
validation remains enabled; SQLite normal open is not a full integrity check.
The comparator also runs separate SQLite/FTS full integrity checks.

SQLite uses WAL + synchronous FULL, 2 MiB requested page cache, mmap disabled,
temp_store FILE, UNIQUE key, source/range indexes and external-content FTS5
trigram detail=none/columnsize=0 with insert/update/delete triggers.

This schema covers the synthetic source/day/text workload. It is not a full
implementation of FDB's dynamic field, link, guard-version or Unicode query
API. LIKE queries in this harness are the literal `rapor-<digits>` and
`teknoloji` corpus queries; arbitrary LIKE wildcard input is not a general FDB
substring comparator. SQLite uniqueness enforces the initial Missing inserts;
the comparator does not independently verify every FDB validation rule.

Batch p50/p95 excludes native SQLite's payload serialization/normalization
prepared before the transaction; FDB does part of that inside commit. Overall
ingest includes generation/serialization on both sides and is the primary write
comparison. Ingest + final maintenance is also reported. SQLite may perform
additional writes during its separate final integrity command; that command
is not counted as ordinary ingest/maintenance.

RSS/private counters cover the native process, including its Rust/C runtime,
and all phases in this harness. They exclude the PowerShell monitor and OS file
cache. Sampled private memory can miss brief peaks. Do not compare this expanded
process peak directly with an older harness excluding broad text/integrity.
Cold I/O, power failure, long/diverse bodies and general workload equivalence
are not established by this benchmark.

The October 8 cost harness additionally verifies every returned page against
the shared `bench_support/verify.rs` corpus oracle (full document, version and
order), outside query timing. Both engines use an untimed query warmup followed
by 30 narrow/exact/range/second-page repetitions and three broad-text/last-page
repetitions. A separate 100-key lookup batch is measured with one warmup and
three repetitions. This establishes equivalence of these specific synthetic
requests, not equivalence of all validation/indexing/recovery work.

Use frozen uninstrumented binaries for timing. FDB's `diagnostic-timing` feature
is for a separate attribution run only; nested spans and concurrent workers
overlap and counters add overhead. Results: `docs/WORKLOAD-COST-VALIDATION.md`.
