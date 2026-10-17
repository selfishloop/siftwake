"""Summarize checked fixed-slot SEG04/SEG05 logs, without opening a database."""
import json
import math
import statistics
import sys
from pathlib import Path

prefix = Path(sys.argv[1])
design = json.loads(Path(f"{prefix}-design.json").read_text(encoding="utf-8-sig"))
layouts = list(dict.fromkeys(x for order in design["orders"] for x in order))

def read(suffix):
    return {r["phase"]: r for r in map(json.loads, Path(f"{prefix}-{suffix}.jsonl").read_text(encoding="utf-8-sig").splitlines())}

def compare(values, baseline):
    return {"median": statistics.median(values), "runs": values,
            "change_percent": 100 * (statistics.median(values) / statistics.median(baseline) - 1),
            "lower_pairs": sum(a < b for a, b in zip(values, baseline)),
            "paired_change_percent": [100 * (a / b - 1) for a, b in zip(values, baseline)]}

runs = {l: [read(f"{l}-{i}") for i in range(4)] for l in layouts}
assert all(r["opened"]["pending_source_checks"] == 0 for rs in runs.values() for r in rs)
result = {"design": design, "layouts": {}, "limits": "Warm OS cache, explicit cache repack excluded; no power loss/cold/concurrent/write throughput measurement."}
if design["phase"] == "profile":
    for layout, rs in runs.items():
        names = set().union(*(r["open_profile"]["counters"] for r in rs))
        result["layouts"][layout] = {
            name: {"median_ms": statistics.median(r["open_profile"]["counters"].get(name, {}).get("elapsed_ns", 0) / 1e6 for r in rs),
                   "run_ms": [r["open_profile"]["counters"].get(name, {}).get("elapsed_ns", 0) / 1e6 for r in rs]}
            for name in sorted(names)}
elif design["phase"] == "open":
    series = {}
    memory = {}
    for layout, rs in runs.items():
        series[layout] = {"open_ms": [r["opened"]["open_ms"] for r in rs]}
        for key in ("first_get_ms", "first_search_ms", "warm_search_ms"):
            series[layout][key] = [r["deferred_first_use"][key] for r in rs]
        series[layout]["first_use_ms"] = [r["opened"]["open_ms"] + r["deferred_first_use"]["first_get_ms"] + r["deferred_first_use"]["first_search_ms"] for r in rs]
        memory[layout] = [json.loads(Path(f"{prefix}-{layout}-{i}-memory.memory.json").read_text(encoding="utf-8-sig"))["os_lifetime_peaks"] for i in range(4)]
    for layout in layouts:
        entry = {key: compare(values, series["json"][key]) for key, values in series[layout].items()}
        entry["memory"] = {key: compare([r[key] for r in memory[layout]], [r[key] for r in memory["json"]]) for key in ("peak_private_commit_bytes", "peak_working_set_bytes")}
        entry["stats"] = runs[layout][0]["opened"]["stats"]
        result["layouts"][layout] = entry
else:
    def pooled(rows):
        for row in rows:
            assert len(row["samples"]) == row["n"]
            assert sorted(row["samples"])[math.ceil(row["n"] * .95) - 1] == row["p95_ms"]
        ordered = sorted(x for row in rows for x in row["samples"])
        return {"n": len(ordered), "p50_ms": ordered[math.ceil(len(ordered) * .5) - 1],
                "p95_ms": ordered[math.ceil(len(ordered) * .95) - 1],
                "p99_ms": ordered[math.ceil(len(ordered) * .99) - 1], "run_p95": [r["p95_ms"] for r in rows]}
    for layout, rs in runs.items():
        assert all(r["matrix_reads"]["full_document_checks"] for r in rs)
        queries = []
        for i, q in enumerate(rs[0]["matrix_reads"]["queries"]):
            identity = {k: q[k] for k in ("text", "kind", "offset", "limit", "total", "late_matches")}
            groups = {}
            for variant in ("json", layout):
                rows = [r["matrix_reads"]["queries"][i] for r in runs[variant]]
                assert all(all(row[k] == v for k, v in identity.items()) for row in rows)
                groups[variant] = pooled([r["timing"] for r in rows])
            base, candidate = groups["json"], groups[layout]
            queries.append({**identity, "json": base, "candidate": candidate,
                            "change_p95_percent": 100 * (candidate["p95_ms"] / base["p95_ms"] - 1),
                            "lower_pairs": sum(a < b for a, b in zip(candidate["run_p95"], base["run_p95"]))})
        result["layouts"][layout] = {"queries": queries,
            "lower_p95": sum(q["change_p95_percent"] < 0 for q in queries),
            "higher_p95": sum(q["change_p95_percent"] > 0 for q in queries),
            "gets": {str(i): pooled([r["matrix_reads"]["gets"][i]["timing"] for r in rs]) for i in range(len(rs[0]["matrix_reads"]["gets"]))}}
Path(f"{prefix}-summary.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
if design["phase"] == "profile":
    for layout, entry in result["layouts"].items():
        print(layout, {k: v["median_ms"] for k, v in entry.items() if k.startswith("open.")})
elif design["phase"] == "open":
    for layout, entry in result["layouts"].items():
        print(layout, json.dumps({k: entry[k] for k in ("open_ms", "first_use_ms", "memory")}, ensure_ascii=False))
else:
    for layout, entry in result["layouts"].items():
        print(layout, entry["lower_p95"], entry["higher_p95"], [(q["text"][:40], q["offset"], q["limit"], round(q["change_p95_percent"], 2), q["lower_pairs"]) for q in entry["queries"] if q["change_p95_percent"] > 0])
