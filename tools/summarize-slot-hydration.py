"""One-process alternating engine calls; rounds are not independent processes."""
import json
import statistics
import sys

with open(sys.argv[1], encoding="utf-8-sig") as stream:
    rows = [json.loads(line) for line in stream if line.strip()]
assert len(rows) == 60, "incomplete alternating comparison"
result = {"independent_processes": 1, "rounds": 5, "includes_stdio": False,
          "includes_json_serialization": False, "warm_os_cache": True, "queries": {}}
for name in sorted({row["query"] for row in rows}):
    selected = {}
    for label in ("baseline", "candidate"):
        selected[label] = sorted((r for r in rows if r["query"] == name and r["engine"] == label), key=lambda r: r["run"])
        assert [r["run"] for r in selected[label]] == list(range(5))
    ratios = [c["timing"]["p50_ms"] / b["timing"]["p50_ms"] for b, c in zip(selected["baseline"], selected["candidate"])]
    summary = {"paired_round_p50_ratios": ratios, "median_paired_ratio": statistics.median(ratios),
               "samples_per_round_per_path": selected["baseline"][0]["samples"]}
    for label, values in selected.items():
        summary[label] = {"median_round_p50_ms": statistics.median(r["timing"]["p50_ms"] for r in values),
                          "median_round_p95_ms": statistics.median(r["timing"]["p95_ms"] for r in values),
                          "worst_round_p95_ms": max(r["timing"]["p95_ms"] for r in values)}
    result["queries"][name] = summary
print(json.dumps(result, indent=2))
