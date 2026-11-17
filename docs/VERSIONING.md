# Versioning

The first Siftwake public preview is **0.1.5-preview**. The user selected
this version on 17 November 2026 and authorized renumbering local historical
tags before creating the GitHub repository.

## Historical mapping

The repository had one release tag, `v0.4.0`, pointing to `a2d9e08`.
It becomes `v0.1.4`. Earlier tags in the table are retrospective names
for selected development checkpoints.

| New tag | Commit | Development checkpoint |
| --- | --- | --- |
| `v0.1.0` | `b63a113` | Initial engine snapshot |
| `v0.1.1` | `42116f7` | Measured low-memory checkpoint |
| `v0.1.2` | `8b90c11` | Bounded engine profile promoted to defaults |
| `v0.1.3` | `51f96e2` | Snapshot readers and bounded commits |
| `v0.1.4` | `a2d9e08` | Background reclaim release, formerly `v0.4.0` |
| `v0.1.5-preview` | Branding and documentation release commit | Siftwake identity and public preview |

Historical commit contents keep their original package versions. Reports,
fixture names, source hashes and format markers retain their recorded identity.
The new tags establish the Siftwake sequence retrospectively.

## Current names

| Surface | Name |
| --- | --- |
| Product and repository identity | Siftwake |
| Public preview version | `0.1.5-preview` |
| Cargo package, crate and CLI | `siftwake` |
| Local application dependency | `siftwake = { path = "../siftwake" }` |
| Source and cache format identifiers | Existing FDB/MAN/SEG/MET identifiers |

The package, Rust import, CLI, importer and Python client use the Siftwake name.
Registry publication is a separate distribution step.

## Preview policy

Preview releases may change the public API. A patch number records the
selected project milestone, while storage format versions are independent.
Compatibility and migration tests accompany changes to persistent formats.

This renumbering applies to local tags. The repository had no configured
remote when the mapping was prepared.
