# Siftwake 0.1.5-preview validation

Validated on 17 November 2026, Windows/MSVC, Rust 1.99, with the default
`bounded-engine` profile.

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `cargo test --locked --lib --tests` | 239 passed, 0 failed, 16 ignored |
| Three Rust snippets from the public documentation | Compiled with the `siftwake` crate |
| Public guide links | 78 local links resolved across 12 documents |
| JSON request snippets | Parsed successfully |
| Main, comparator package metadata and lockfiles | Resolved with `--locked --offline` |
| `cargo build --offline --locked --release --bin siftwake` | Passed, release executable produced |
| SQLite, Tantivy and RAM probe package checks | Compiled using the `siftwake` dependency |
| CLI name and Python modules | Usage names `siftwake`, four Python modules parsed |
| Logo assets | SVG structure, PNG alpha and size-specific ICO content checked |

The test set includes process-kill/reopen, source corruption, snapshot
publication, 0.1 fixture compatibility, streamed transactions and model-based
query checks. Ignored cases include subprocess helpers and explicitly selected
diagnostic workloads. Optional feature-only test suites retain their existing
separate build requirements.

The release changes product identity, package version and documentation.
The package, crate and binary now use `siftwake`. Import paths in examples,
tests and comparator tools follow the new name.
The 239-test suite was rerun after the rename. Its process-kill tests
invoke the new executable through Cargo's binary environment variable.
Performance evidence comes from the recorded engineering reports linked in
[Performance](PERFORMANCE.md). The engine source state is the implementation
recorded at `56ee1f0`.

[Release notes](RELEASE-0.1.5-preview.md) · [Version map](VERSIONING.md)
