# Engine contracts

Siftwake `0.1.5-preview` provides an embedded Rust engine with a single database
owner, versioned documents, consistent snapshots and derived search indexes.

## Data and ordering

`(scope, collection, id)` identifies one visible document. Updates replace
the complete value. Deletes shadow previous revisions. Search results are
ordered lexicographically by the full key. Numeric or chronological ID order
can be encoded with fixed-width keys at the application layer.

The text index produces candidates from character trigrams. Candidate title
and text are checked for the normalized substring predicate. Queries containing
only terms shorter than three characters can scan a large portion of the
document set.

Scalar metadata equality, inclusive `i64` ranges and reverse same-scope links
are supported. Applications choose a consistent numeric representation.
The signed-zero behavior difference between owned and light paths remains
an open issue. [Open work](OPEN-WORK.md)

## Source and derived data

The WAL and immutable `.fdb` sources, selected by the `.ready` manifest, define
the committed state. Search index files such as `.idx` are derived caches.
Strict open checks authoritative sources and index relationships before
returning. An intact cache cannot establish the integrity of a damaged source.

The current source, manifest and cache formats retain their existing identifiers,
including FDBLOG04 and SEG05. Product version changes are independent of storage
format identifiers.

## Publication and readers

Commits are staged, validated and durably written before successful return.
The writer publishes the new snapshot view before acknowledging success.
Readers acquire a committed sequence and pin the associated storage. Background
work builds immutable outputs, and the owner publishes ready work.

One operating-system owner lock protects a database directory. Use snapshot
readers inside that owner process. The local JSON Lines process provides another
application access path.

## Current defaults

| Choice | Current preview |
| --- | --- |
| Build feature profile | `bounded-engine` |
| Normal search | Exact total, serial verification |
| First-page hybrid search | Explicit API call, budget supplied by caller |
| Hybrid worker admission | Try once, then caller-thread serial continuation |
| WAL sync | Enabled in ordinary builds |
| Small streamed WAL layer | Opt-in per writer session |
| Revision reclamation | Enabled with background planning and rewrite |
| Maintenance publication | Owner commits or explicit maintenance polling |
| Normal open | Strict |

## Bounds and lifecycle

Streamed intake limits each encoded entry to 256 KiB. Atomic batches are subject
to 10,000 mutations, 64 MiB frame capacity, key allowance and builder admission.
Total process RAM also includes caller data, readers, returned documents,
allocator capacity and background work.

Snapshots retain old source files until their pins are released. Retained disk
therefore depends on update history, maintenance progress and reader lifetimes.
Host event loops should provide maintenance polling opportunities.

The process-kill and reopen tests establish recovery for their recorded
interruption points. Power-loss behavior and filesystem durability require
separate evidence.

Siftwake accepts extracted UTF-8 text. Applications supply extraction, filesystem
events, authorization, and recovery or rebuild policy. The current published
interfaces are Rust and the local CLI. A C ABI remains a separate work item.

[Rust API](API.md) · [Historical architecture record](ARCHITECTURE.md) ·
[Current measurements](PERFORMANCE.md)
