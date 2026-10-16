"""Aggregate the fixed-slot read matrix; JSONL reports only, no DB writes."""
import json
import math
from pathlib import Path
from statistics import median

root = Path(__file__).resolve().parent.parent
prefix = root / "docs/benchmarks/read-matrix-10k-20261012"


def read(engine, run):
    rows = [json.loads(line) for line in Path(f"{prefix}-{engine}-{run}.jsonl").read_text(encoding="utf-8-sig").splitlines()]
    return {row["phase"]: row for row in rows}


def aggregate(timings):
    samples = sorted(value for timing in timings for value in timing["samples"])
    rank = lambda percent: samples[math.ceil(len(samples) * percent / 100) - 1]
    return {"n": len(samples), "p50_ms": rank(50), "p95_ms": rank(95), "p99_ms": rank(99),
            "run_p95": [t["p95_ms"] for t in timings], "min_run_p95_ms": min(t["p95_ms"] for t in timings),
            "max_run_p95_ms": max(t["p95_ms"] for t in timings)}


runs = {engine: [read(engine, n) for n in range(5 if engine != "control" else 2)] for engine in ("fdb", "sqlite", "control")}
result = {"aggregation": "pooled nearest-rank percentiles across five processes; control two processes", "queries": [], "gets": {}}
for n, query in enumerate(runs["fdb"][0]["matrix_reads"]["queries"]):
    fields = ("text", "kind", "offset", "limit", "total", "late_matches")
    entry = {key: query[key] for key in fields}
    for engine in ("fdb", "sqlite"):
        series = [run["matrix_reads"]["queries"][n] for run in runs[engine]]
        assert all(all(row[key] == entry[key] for key in fields) for row in series)
        entry[engine] = aggregate([row["timing"] for row in series])
    if entry["kind"] in ("short-negative", "negative-and", "tail-token", "long-negative"):
        series = [next(row for row in run["matrix_reads"]["queries"] if row["text"] == entry["text"]) for run in runs["control"]]
        entry["control"] = aggregate([row["timing"] for row in series])
        entry["fdb_over_control_p95"] = entry["fdb"]["p95_ms"] / entry["control"]["p95_ms"]
    entry["fdb_over_sqlite_p95"] = entry["fdb"]["p95_ms"] / entry["sqlite"]["p95_ms"]
    result["queries"].append(entry)
for engine in runs:
    result["gets"][engine] = [aggregate([run["matrix_reads"]["gets"][n]["timing"] for run in runs[engine]]) for n in range(5)]
result["open_ms"] = {engine: [run["opened"]["open_ms"] for run in series] for engine, series in runs.items()}
primary = [q for q in result["queries"] if q["kind"] != "page"]
result["primary_fdb_p95_wins"] = sum(q["fdb_over_sqlite_p95"] < 1 for q in primary)
result["primary_cases"] = len(primary)
result["page_fdb_p95_wins"] = sum(q["fdb_over_sqlite_p95"] < 1 for q in result["queries"] if q["kind"] == "page")
Path(f"{prefix}-summary.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
lines = ["| Sorgu | Tür | Toplam | Offset/limit | FDB p50/p95/p99 ms | SQLite p50/p95/p99 ms | FDB/SQLite p95 |",
         "|---|---|---:|---|---:|---:|---:|"]
for q in result["queries"]:
    label = q["text"] if len(q["text"]) < 80 else "zx × 150 (300 bayt)"
    fmt = lambda engine: "/".join(f"{q[engine][key]:.3f}" for key in ("p50_ms", "p95_ms", "p99_ms"))
    lines.append(f"| `{label}` | {q['kind']} | {q['total']} | {q['offset']}/{q['limit']} | {fmt('fdb')} | {fmt('sqlite')} | {q['fdb_over_sqlite_p95']:.2f} |")
Path(f"{prefix}-table.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
print(json.dumps({key: result[key] for key in ("primary_fdb_p95_wins", "primary_cases", "page_fdb_p95_wins")}, indent=2))
for q in result["queries"]:
    print(json.dumps({"text": q["text"][:80], "kind": q["kind"], "total": q["total"], "fdb95":q["fdb"]["p95_ms"], "sqlite95":q["sqlite"]["p95_ms"], "ratio":q["fdb_over_sqlite_p95"], "control95":q.get("control",{}).get("p95_ms")}, ensure_ascii=False))
