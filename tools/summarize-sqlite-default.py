"""Aggregate fixed-slot native comparison logs. Never opens a database."""
import argparse
import json
import math
from pathlib import Path
from statistics import median

parser = argparse.ArgumentParser()
parser.add_argument("prefix", nargs="?", default="docs/benchmarks/sqlite-default-10k-20261016")
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
prefix = root / args.prefix


def read(suffix):
    return json.loads(Path(f"{prefix}-{suffix}.json").read_text(encoding="utf-8-sig"))


def phases(suffix):
    rows = Path(f"{prefix}-{suffix}.jsonl").read_text(encoding="utf-8-sig").splitlines()
    return {row["phase"]: row for row in map(json.loads, rows)}


def delta(candidate, control):
    return (candidate / control - 1) * 100


def aggregate(timings):
    samples = sorted(v for t in timings for v in t["samples"])
    assert all(t["n"] == len(t["samples"]) for t in timings)
    return {"n": len(samples), **{f"p{p}_ms": samples[math.ceil(len(samples) * p / 100) - 1] for p in (50, 95, 99)},
            "process_p95_ms": [t["p95_ms"] for t in timings]}


result = {"prefix": args.prefix, "writes": {}, "queries": [], "gets": {}, "opening": {}, "memory": {}}
for api, pairs in (("streamed", 4), ("owned", 2)):
    rows = {engine: [] for engine in ("fdb", "sqlite")}
    hashes = []
    for pair in range(pairs):
        for engine in rows:
            stem = f"{api}-{pair}-{engine}"
            log = phases(f"{stem}-ingest-10000-0")
            memory = read(f"{stem}-ingest-10000-0.memory")
            verified = phases(f"{api}-{pair}-{engine}-verify")["verified"]
            assert verified["full_document_checks"] == 10000 and verified["input_crc_verified"]
            assert log["result"]["text_bytes"] == 264842858
            assert memory["exit_code"] == 0 and not memory["low_disk_stop"]
            if engine == "fdb":
                assert verified["first50_full_pages_verified"]
                hashes.append(read(f"{api}-{pair}-{engine}-files"))
            rows[engine].append({
                "ingest_s": log["result"]["ingest_ms"] / 1000,
                "maintenance_s": log["result"]["maintenance_ms"] / 1000,
                "total_s": (log["result"]["ingest_ms"] + log["result"]["maintenance_ms"]) / 1000,
                "cpu_s": memory["cpu_ms"] / 1000,
                "private_bytes": memory["os_lifetime_peaks"]["peak_private_commit_bytes"],
                "working_set_bytes": memory["os_lifetime_peaks"]["peak_working_set_bytes"],
                "disk_bytes": memory["retained_disk_bytes"],
            })
    assert all(h == hashes[0] for h in hashes)
    medians = {e: {key: median(row[key] for row in series) for key in series[0]} for e, series in rows.items()}
    paired = {key: [delta(f[key], s[key]) for f, s in zip(rows["fdb"], rows["sqlite"])] for key in medians["fdb"]}
    result["writes"][api] = {"pairs": pairs, "rows": rows, "medians": medians,
                             "fdb_change_percent": {key: delta(medians["fdb"][key], medians["sqlite"][key]) for key in medians["fdb"]},
                             "paired_fdb_change_percent": paired, "fdb_core_hashes_equal_across_runs": True}

matrix = {e: [phases(f"matrix-{e}-{n}") for n in range(4)] for e in ("fdb", "sqlite")}
for engine in matrix:
    for run in matrix[engine]:
        assert run["matrix_reads"]["full_document_checks"]
        assert run["matrix_oracle"]["source_crc_verified"]
        assert run["matrix_oracle"]["records"] == 10000
        assert run["matrix_oracle"]["text_bytes"] == 264842858
        assert run["matrix_oracle"]["get_keys"] == matrix["fdb"][0]["matrix_oracle"]["get_keys"]
for n, first in enumerate(matrix["fdb"][0]["matrix_reads"]["queries"]):
    fields = ("text", "kind", "offset", "limit", "total", "late_matches")
    entry = {key: first[key] for key in fields}
    for engine in matrix:
        series = [run["matrix_reads"]["queries"][n] for run in matrix[engine]]
        assert all(all(row[key] == entry[key] for key in fields) for row in series)
        assert all(row["timing"]["n"] == (5 if entry["kind"] == "page" else 20) for row in series)
        entry[engine] = aggregate([row["timing"] for row in series])
    entry["fdb_over_sqlite_p95"] = entry["fdb"]["p95_ms"] / entry["sqlite"]["p95_ms"]
    entry["fdb_faster_process_pairs"] = sum(f < s for f, s in zip(entry["fdb"]["process_p95_ms"], entry["sqlite"]["process_p95_ms"]))
    result["queries"].append(entry)
assert len(result["queries"]) == 38
for engine in matrix:
    result["gets"][engine] = [aggregate([run["matrix_reads"]["gets"][n]["timing"] for run in matrix[engine]]) for n in range(5)]
assert [row["n"] for row in result["gets"]["fdb"]] == [row["n"] for row in result["gets"]["sqlite"]]
result["query_counts"] = {
    "fdb_faster_pooled_p95": sum(q["fdb_over_sqlite_p95"] < 1 for q in result["queries"]),
    "primary_fdb_faster": sum(q["kind"] != "page" and q["fdb_over_sqlite_p95"] < 1 for q in result["queries"]),
    "page_fdb_faster": sum(q["kind"] == "page" and q["fdb_over_sqlite_p95"] < 1 for q in result["queries"]),
}
for variant in ("fdb", "sqlite", "record-fdb"):
    series = [phases(f"open-{variant}-{n}") for n in range(4)]
    use_phase = "first_use" if variant == "sqlite" else "deferred_first_use"
    opening = [run["opened"]["open_ms"] for run in series]
    usage = [{k: run[use_phase][k] for k in ("first_get_ms", "first_search_ms", "warm_search_ms")} for run in series]
    result["opening"][variant] = {
        "process_open_ms": opening, "median_open_ms": median(opening), "process_first_use": usage,
        **{f"median_{key}": median(row[key] for row in usage) for key in usage[0]},
        "median_open_plus_first_get_search_ms": median(o + u["first_get_ms"] + u["first_search_ms"] for o, u in zip(opening, usage)),
    }
for engine in ("fdb", "sqlite"):
    query_memory = [read(f"query-memory-{engine}-{n}.memory") for n in range(2)]
    for n in range(2):
        verified = phases(f"query-memory-{engine}-{n}")["query_memory_verified"]
        assert not verified["oracle_loaded"] and verified["searches"] == 24 and verified["gets"] == 771
        assert query_memory[n]["exit_code"] == 0
    idle = [read(f"{engine}-idle-10000-{n}.memory")["idle"] for n in range(2)]
    assert all(row["search_calls"] == 0 for row in idle)
    result["memory"][engine] = {
        "query_lifetime_private_bytes": median(row["os_lifetime_peaks"]["peak_private_commit_bytes"] for row in query_memory),
        "query_lifetime_working_set_bytes": median(row["os_lifetime_peaks"]["peak_working_set_bytes"] for row in query_memory),
        "query_process_peaks": [row["os_lifetime_peaks"] for row in query_memory],
        "idle_private_bytes": median(row["private_bytes"] for row in idle),
        "idle_working_set_bytes": median(row["working_set_bytes"] for row in idle),
    }
for phase in ("read", "open-memory"):
    assert read(f"{phase}-before") == read(f"{phase}-after")
result["read_only_hashes_unchanged"] = True
result["limits"] = [
    "Warm OS cache, no eviction; fresh process is not cold I/O. No concurrent load.",
    "FDB strict open verifies all sources; SQLite normal open does not perform full integrity_check.",
    "Private commit is not physical RAM; working set includes shared image pages; neither includes total OS file cache.",
    "Write maintenance differs: FDB flush/drain merge versus SQLite FTS optimize/checkpoint. No update/delete churn measurement.",
    "38 fixed substring/AND/page cases; not a proof of full dynamic-field/link/guard workload equivalence.",
    "Pages have20 pooled samples, primary80; low page p95 sample count is explicit.",
]
Path(f"{prefix}-summary.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
lines = ["| Query | Offset/limit | Total | FDB p95 ms | SQLite p95 ms | FDB/SQLite | Faster pairs |",
         "|---|---:|---:|---:|---:|---:|---:|"]
for q in result["queries"]:
    text = q["text"] if len(q["text"]) < 80 else "zx x150"
    lines.append(f"| `{text}` | {q['offset']}/{q['limit']} | {q['total']} | {q['fdb']['p95_ms']:.4f} | {q['sqlite']['p95_ms']:.4f} | {q['fdb_over_sqlite_p95']:.6f} | {q['fdb_faster_process_pairs']}/4 |")
Path(f"{prefix}-table.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
print(json.dumps({key: result[key] for key in ("query_counts", "opening", "memory", "read_only_hashes_unchanged")}, indent=2))
for api, write in result["writes"].items():
    print(json.dumps({"api": api, "medians": write["medians"], "changes_percent": write["fdb_change_percent"]}))
