"""Three hydration paths in one process, six rotated/reversed rounds."""
import json
import statistics
import sys

with open(sys.argv[1], encoding="utf-8-sig") as stream:
    rows = [json.loads(line) for line in stream if line.strip()]
names = {row["query"] for row in rows}
assert names in ({"exact", "range", "narrow", "second_page", "last_page", "text"},
                 {"exact", "range", "narrow", "second_page", "last_page"})
assert len(rows) == len(names) * 18, "incomplete three-path comparison"
result = {"independent_processes": 1, "rounds": 6, "includes_stdio": False,
          "includes_json_serialization": False, "warm_os_cache": True, "queries": {}}
for name in sorted({row["query"] for row in rows}):
    selected = {}
    for label in ("baseline", "candidate", "grouped"):
        selected[label] = sorted((r for r in rows if r["query"] == name and r["engine"] == label), key=lambda r: r["run"])
        assert [r["run"] for r in selected[label]] == list(range(6))
    summary = {"samples_per_round_per_path": selected["baseline"][0]["samples"]}
    for numerator, denominator in [("candidate", "baseline"), ("grouped", "baseline"), ("grouped", "candidate")]:
        ratios = [n["timing"]["p50_ms"] / d["timing"]["p50_ms"] for n, d in zip(selected[numerator], selected[denominator])]
        summary[f"{numerator}_to_{denominator}"] = {"round_ratios": ratios, "median_paired_ratio": statistics.median(ratios)}
    for label, values in selected.items():
        summary[label] = {"median_round_p50_ms": statistics.median(r["timing"]["p50_ms"] for r in values),
                          "median_round_p95_ms": statistics.median(r["timing"]["p95_ms"] for r in values),
                          "worst_round_p95_ms": max(r["timing"]["p95_ms"] for r in values)}
    result["queries"][name] = summary
print(json.dumps(result, indent=2))
