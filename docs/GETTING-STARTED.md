# Getting started

Siftwake `0.1.5-preview` builds from this checkout. The package and executable
are named `siftwake`. The following commands run from the repository root.

## Build

```powershell
cargo build --release --bin siftwake
```

The manifest declares Rust 1.89. Recorded validation used Rust 1.99 on
Windows/MSVC. The engine uses the default `bounded-engine` feature profile.
The API documentation can be generated with `cargo doc --no-deps`.

## Embed in Rust

For a local checkout stored next to your application, add a path dependency.
Application code imports the `siftwake` crate directly.

```toml
[dependencies]
siftwake = { path = "../siftwake" }
```

The path points to the directory containing the engine's `Cargo.toml`.

```rust
use siftwake::{Database, Document, Guard, Key, Mutation, Query};
use std::collections::BTreeMap;

fn main() -> siftwake::Result<()> {
    let mut db = Database::open("data/mydb")?;
    let document = Document {
        key: Key::new("shared", "documents", "project-notes"),
        title: "Project notes".into(),
        text: "An embedded document search engine".into(),
        fields: BTreeMap::new(),
        links: Vec::new(),
    };

    db.commit_streamed(std::iter::once(Ok(Mutation::Put {
        document,
        guard: Guard::Any,
    })))?;

    let mut query = Query::new("shared");
    query.collection = Some("documents".into());
    query.text = "document engine".into();
    query.limit = 50;

    let page = db.search(&query)?;
    println!("{} matches", page.total);
    for hit in page.hits {
        println!("{} {}", hit.document.key.id, hit.version);
    }
    Ok(())
}
```

`Guard::Any` allows the example to replace its existing key when rerun.
For create-only writes use `Guard::Missing`. For updates based on a fetched
document use `Guard::Version { version: stored.version }`.

## Get the first page

`search` returns an exact total. For an interactive first page with offset 0,
use `search_first_hybrid(&query, 200)`. The result's `MatchTotal` distinguishes
an exact count from a proved lower bound.

## Read while writing

Obtain a `SnapshotReader` from the writer, share it with reader threads, and call
`acquire()` for each new query. A snapshot has one committed sequence. Existing
snapshots remain readable after subsequent commits.

Successful commits publish the new view before returning. New acquisitions
after the return observe that commit or a later committed sequence.
[Snapshots and updates](API.md)

## Keep maintenance progressing

When an application event loop is idle, call `poll_maintenance()`. It publishes
ready work and advances background maintenance. Use `maintain_idle()` when a
synchronous drain is appropriate, such as an explicitly idle indexing phase.

Background revision reclamation is enabled by default. The optional small
streamed WAL layer is enabled with `set_streamed_wal_layer(true)` for an
individual writer session. Review its write and maintenance tradeoffs before
selecting it for an application. [Engine contracts](ENGINE.md)

## CLI

```powershell
.\target\release\siftwake.exe data/mydb --stdio
```

Send one JSON request per line and read one response per line. For a request
file, use `siftwake.exe data/mydb request.json`. [CLI reference](CLI.md)

## Validate a build

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --tests
```

Read [the benchmark workspace rules](../AGENTS.md) before using corpus tools.
Existing benchmark slots and fixtures have ownership and compatibility
requirements.
