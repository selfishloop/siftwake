# Rust API

The Cargo package and crate are `siftwake`. Examples use the local dependency
from [Getting started](GETTING-STARTED.md). Public types are exported from
[lib.rs](../src/lib.rs).

## Documents and keys

A `Document` contains `key`, `title`, extracted UTF-8 `text`, JSON `fields`
and same-scope `links`. A `StoredDocument` adds `version`.

`Key` contains `scope`, `collection` and `id`. Each component is 1–1024 UTF-8
bytes, with control characters rejected. The key is unique within this tuple.
Scope is supplied by the host, which also handles authentication and access.

## Updates and atomic batches

`commit_streamed` accepts an iterator of `Result<Mutation>`. A `Put` replaces
the complete document at its key. A `Delete` removes its visible value.
All mutations in one successful batch share the returned sequence.
Each new stored document receives that sequence as its version.

| Guard | Condition |
| --- | --- |
| `Any` | Apply to the current state |
| `Missing` | The key has no visible document |
| `Version { version }` | The visible document has this version |

The engine checks batch guards against the previous state. A guard conflict
rejects the batch. Duplicate mutations for the same key in one atomic batch
are rejected.

```rust
use siftwake::{Database, Document, Guard, Mutation};

fn replace(db: &mut Database, document: Document, version: u64) -> siftwake::Result<u64> {
    db.commit_streamed(std::iter::once(Ok(Mutation::Put {
        document,
        guard: Guard::Version { version },
    })))
}
```

For a changed ID, submit `Delete { key: old_key, guard: old_guard }` and
`Put { document: new_document, guard: new_guard }` in the same batch.

The streamed path allows 1–10,000 mutations, subject to encoded entry, frame
and memory admission limits. Each encoded input entry is at most 256 KiB.
The WAL frame limit is 64 MiB. Key and builder allowances can reject a batch
before that frame limit. These constraints should be handled as input errors.

`commit_streamed_chunks` is an explicitly non-atomic helper. It targets at
most 32 mutations and 512 KiB of encoded entries per chunk. Each entry still
passes the streamed entry limit. The success report includes acknowledged
sequences and chunk sizes. On a later error, earlier chunks remain committed
and the error contains their report.

`commit(Vec<Mutation>)` is the owned API. Caller batches and resulting delta
state contribute to its memory usage. The recommended content-ingest path is
streamed input.

## Search

`Query` selects a required scope, optional collection, text, exact metadata,
inclusive integer ranges and reverse links. Terms separated by whitespace
are combined with AND. Each term can occur in either title or text.
Results follow lexicographic key order.

Text matching uses Unicode lowercase and Turkish normalization, including
`I/İ/ı → i`, `ş → s`, `ğ → g`, `ç → c`, `ö → o`, `ü → u`.
Exact metadata equality has its own typed matching rules.

| Method | Result |
| --- | --- |
| `search(&query)` | `SearchPage`, exact total, serial verification |
| `search_first(&query)` | `FirstPage`, capped first-page search |
| `search_first_hybrid(&query, 200)` | Serial first 200 candidates, then shared worker admission |
| `search_first_hybrid_cancellable(&query, 200, &token)` | Hybrid first page with cooperative cancellation |

The first-page methods require offset 0 and a limit from 1 to 200.
`MatchTotal::Exact(n)` is an exhausted count.
`MatchTotal::AtLeast(n)` is a proved lower bound. It should be displayed
as a lower bound in the application.

After the hybrid serial budget, worker admission uses one non-blocking try.
If admission is unavailable, the caller continues serially. The normal
`search` method retains exact serial behavior.

## Snapshots

```rust
use siftwake::{Database, Query};

fn read_after_commit(db: &mut Database, query: &Query) -> siftwake::Result<()> {
    let reader = db.snapshot_reader()?;
    let snapshot = reader.acquire()?;
    let sequence = snapshot.sequence();
    let page = snapshot.search_first_hybrid(query, 200)?;
    assert_eq!(page.sequence, sequence);
    Ok(())
}
```

Move or clone a `SnapshotReader` into reader threads. Each acquired snapshot
pins its source files and keeps one sequence for its lifetime. Acquire a fresh
snapshot when a query should include newly committed changes. Release old
snapshots promptly so obsolete source files can be collected.

## Cancellation

Create one `CancelToken` per search request. Give a clone to the controller
and pass the token to the cancellable method. Calling `cancel()` is sticky
and causes cooperative checks to return `Error::Cancelled`. A new request
uses a new token. Cancellation timing depends on the current work unit.

## Maintenance and errors

`poll_maintenance()` performs a foreground maintenance tick and returns a
boolean. Call it during host idle opportunities.
`maintain_idle()` flushes, waits for eligible compaction, publishes and checks
the resulting state, and retries obsolete-file collection. Its duration can
be substantially longer than a poll.

`set_revision_reclamation(false)` disables automatic revision reclamation
for a writer session. `set_streamed_wal_layer(true)` enables the opt-in small
WAL layer. These are runtime choices.

| Error | Application response |
| --- | --- |
| `Conflict(key)` | Fetch the current document and resolve the update |
| `Invalid(message)` | Correct input or batch sizing |
| `Cancelled` | Drop the superseded search request |
| `Locked` | Route access through the existing database owner |
| `Corrupt(message)` | Surface the failure and follow the application's rebuild/recovery policy |
| `Poisoned` | Close and reopen, then inspect versions before retrying |
| `Io(error)` | Inspect the I/O outcome and reopen as required |

A failed commit after an I/O operation can have an uncertain durable outcome.
On reopen, inspect the document version before retrying the operation.
