# Local JSON Lines CLI

The executable is named `siftwake`. It opens one database directory,
executes requests in order, and writes one JSON response per request.

```powershell
cargo build --release --bin siftwake
.\target\release\siftwake.exe data/mydb --stdio
```

A request file can contain formatted JSON.

```powershell
.\target\release\siftwake.exe data/mydb request.json
```

## Operations

| Operation | Input | Successful result |
| --- | --- | --- |
| `commit` | `mutations` array | Object containing `sequence` |
| `get` | `key` | Stored document and version, or `null` |
| `search` | `query` | Exact total, sequence, versioned hits |
| `stats` | Operation only | Engine statistics |
| `flush` | Operation only | Statistics after delta flush |
| `compact` | Operation only | Statistics after flush and scheduling |
| `poll_compaction` | Operation only | `{"published":true/false}` |
| `wait_compaction` | Operation only | `{"published":true/false}` |

The CLI uses the owned batch API. Streamed intake, capped search, cancellation,
snapshot handles and the combined maintenance tick are exposed by the Rust API.
The request enum in [main.rs](../src/main.rs) defines accepted operations.

## Request example

```json
{"op":"commit","mutations":[{"op":"put","guard":{"kind":"missing"},"document":{"key":{"scope":"shared","collection":"documents","id":"notes"},"title":"Notes","text":"Searchable project notes","fields":{},"links":[]}}]}
{"op":"get","key":{"scope":"shared","collection":"documents","id":"notes"}}
{"op":"search","query":{"scope":"shared","collection":"documents","text":"project notes","offset":0,"limit":50}}
```

An update can use `{"kind":"version","version":1}` as its guard. A delete
mutation contains `key` and `guard`. In the example, a fresh database assigns
sequence and version 1 to the first commit.

## Responses and failures

Successful responses wrap the result in `{"ok":true,"result":...}`.
In stdio mode, a request error produces `{"ok":false,"error":"..."}`, and
subsequent requests can continue. Open failures and request-file failures
exit with status 1. A stdio request line is limited to 96 MiB.

Use `stats` to inspect logical source and index bytes. Measure retained files
and OS process memory separately when diagnosing disk or RAM.

[Rust API](API.md) · [Engine contracts](ENGINE.md)
