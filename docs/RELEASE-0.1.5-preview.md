# Siftwake 0.1.5-preview

Siftwake is the new public name for the embedded document database and content
search engine developed as FDB.

This preview introduces the Siftwake identity, a nine-document puzzle symbol,
size-specific icons, a GitHub avatar and a social preview. Public-facing
documentation covers streamed writes, exact and capped search, snapshot
readers, cancellation, maintenance, recovery contracts and measured limits.

The package version is `0.1.5-preview`. The Cargo package, library import and
executable are named `siftwake`. The importer is `siftwake-import-sdb`, and
the Python client is `siftwake_client.SiftwakeClient`.

Local historical tags were mapped to `v0.1.0` through `v0.1.4`. Their commit
contents retain the earlier package versions. Source, manifest and cache format
identifiers keep their existing definitions.
[Version mapping](VERSIONING.md)

## Current capabilities

- Versioned documents with scoped identity and guarded atomic updates.
- Streamed Rust intake and a separate owned batch API.
- AND substring text search, metadata filters and document links.
- Exact totals and opt-in capped first-page search.
- Consistent, pinned snapshot readers and cooperative query cancellation.
- Strict source validation, WAL replay and background revision reclamation.

## Known limits

Current measurements identify ingest, retained disk, strict startup and
single-update latency as improvement areas. The small WAL layer remains
opt-in. Broad short-term scans, long-lived snapshot retention and commit tails
have workload-dependent costs. C ABI, application integration and registry
packaging remain separate work items.

This release collects the existing engine state with its new identity and
documentation. [Performance evidence](PERFORMANCE.md) · [Open work](OPEN-WORK.md)

Validation passed with 239 tests and 16 ignored cases, format and Clippy checks,
and three compiled documentation snippets. [Validation record](VALIDATION-0.1.5-preview.md)
