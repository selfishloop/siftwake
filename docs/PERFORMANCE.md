# Performance evidence

Results describe the named corpus, implementation, settings, physical layout
and session. The reports preserve failed runs and known limitations.

## Siftwake, SQLite and Tantivy, 50k

The README summary uses the paired 16 November Siftwake/Tantivy measurements
and the 29 October SQLite baseline on the same cap200k 50k prefix.
The SQLite column comes from a separate session with its recorded settings.
The complete 41-case first-page table is included in the README.
[SQLite baseline](TRWIKI-CAP200K-SCALE-20261029.md)

On 16 November 2026, Siftwake and Tantivy 0.27.0 processed
the same first 50,000 documents of the audited cap200k Turkish corpus.
Successful arm order was Siftwake → Tantivy → Tantivy → Siftwake. Each query path has
24 warm samples across two blocks.

| Measurement | Siftwake | Tantivy comparator |
| --- | ---: | ---: |
| First-page cases with lower p95 | 39 / 41 | 2 / 41 |
| Exact cases with lower p95 | 40 / 41 | 1 / 41 |
| Ingest and maintenance, two successful runs | 39.61–46.43 s | 16.44–16.79 s |
| Post-maintenance retained disk | 300.84 MB | 137.82 MB |
| Ingest peak private commit | 17.85–17.86 MiB | 37.42–38.40 MiB |
| Idle private commit | 2.37–2.38 MiB | 1.10–1.11 MiB |
| Warm open to first result p95 | 217.93 ms | 12.43 ms |
| Single update to visible result p95 | 143.23 ms | 81.17 ms |

Siftwake used strict open. Tantivy's full checksum verification ran in a separate
oracle phase. Its stored documents use LZ4, and the comparator builds a text
candidate index. Siftwake also indexes metadata and links. Its small streamed
WAL option was disabled, as in its current default configuration.

Tantivy had four interrupted seed attempts and one interrupted update series
in the session. Two seed attempts completed. Successful timings exclude failed
attempts and manual restart intervals. Windows access failures and unknown
worker causes are recorded in the full report.

Both successful blocks passed full document and 41-case search validation.
Open has 12 samples per arm, so nearest-rank p95 equals maximum.
These process memory values include the specified harness state.

[Full report](TANTIVY-COMPARISON-50K-20261116.md) ·
[All query timings](benchmarks/tantivy-comparison-50k-20261116/query-matrix.md) ·
[Raw samples](benchmarks/tantivy-comparison-50k-20261116/summary.json)

## Cold open against SQLite

The recorded 15 November run used the existing 50k churn layout with 39 Siftwake
runs. Four ETW-supported cold trials produced Siftwake open-to-first-result
885–909 ms and SQLite 6,663–6,696 ms. Corresponding warm results were
627–640 ms and 297–339 ms.

This is a different layout and protocol from the clean Tantivy experiment.
Each arm has two cold samples. Collector-specific lost-event counts were
unavailable. The combined ETL lost-event count was zero, and target file
reads were reconciled.
[Cold report and evidence](COLD-OPEN-PAIRED-50K-20261115.md)

## Memory and remaining work

The separate 50k motor-only run reported default ingest peak private commit
15.49–16.36 MiB and clean idle 2.20–2.26 MiB. Reclamation planning overlapped
the workload. Full revision rewrite peak was not established.
[Motor-only memory evidence](RECLAIM-MOTOR-ONLY-RAM-50K-20261114.md)

Current work targets single-update latency, ingest cost, disk footprint,
startup and idle allocations. Concurrent reads, commit tails, continuous
churn and snapshot-retained disk have their own recorded limits.
[Prioritized tasks](OPEN-WORK.md)
