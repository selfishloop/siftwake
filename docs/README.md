# Siftwake documentation

Siftwake `0.1.5-preview` is the public identity of the Rust engine developed as FDB.
The Cargo package, crate and executable currently retain the `fdb` name.

| Start here | Purpose |
| --- | --- |
| [Getting started](GETTING-STARTED.md) | Build, embed, insert and query |
| [Rust API](API.md) | Updates, snapshots, cancellation and maintenance |
| [CLI](CLI.md) | JSON Lines operations and process behavior |
| [Engine contracts](ENGINE.md) | Storage, ordering, recovery and limits |
| [Performance](PERFORMANCE.md) | Current measured results and their limits |
| [Open work](OPEN-WORK.md) | Outstanding measurements and improvements |

| Project identity | Purpose |
| --- | --- |
| [Brand guide](BRAND.md) | Positioning, logo, colors and typography |
| [Brand files](../assets/brand/README.md) | SVG, PNG, ICO and size-specific icons |
| [Versioning](VERSIONING.md) | Public version and historical local tag mapping |
| [Preview release notes](RELEASE-0.1.5-preview.md) | Identity, interfaces and current limits |
| [Preview validation](VALIDATION-0.1.5-preview.md) | Build, tests, examples and artifact checks |

## Engineering evidence

[Architecture history](ARCHITECTURE.md) records earlier implementation stages.
[The archived FDB README](README-FDB-HISTORY.md) preserves the development
timeline. Historical defaults and version numbers describe the recorded stage.
The contracts in [Engine contracts](ENGINE.md) describe the current preview.

- [Background reclamation default](BACKGROUND-RECLAIM-DEFAULT-50K-20261112.md)
- [WAL prefix publication oracle](WAL-PREFIX-PUBLICATION-ORACLE-20261114.md)
- [Motor-only RAM](RECLAIM-MOTOR-ONLY-RAM-50K-20261114.md)
- [Cold open against SQLite](COLD-OPEN-PAIRED-50K-20261115.md)
- [Same-session Tantivy comparison](TANTIVY-COMPARISON-50K-20261116.md)

The Rust model in [model.rs](../src/model.rs), the public exports in
[lib.rs](../src/lib.rs) and the CLI request enum in [main.rs](../src/main.rs)
define the implemented interfaces.
