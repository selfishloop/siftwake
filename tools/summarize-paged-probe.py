"""Five-run bulk metadata lookup results; not end-to-end query timings."""
import json
import pathlib
import statistics
import sys

prefix = pathlib.Path(sys.argv[1])
runs = []
for number in range(1, 6):
    rows = [json.loads(line) for line in pathlib.Path(f"{prefix}-{number}.jsonl").read_text(encoding="utf-8-sig").splitlines()]
    assert len(rows) == 3 and rows[0]["phase"] == "layout"
    if runs:
        assert rows[0] == runs[0][0]
    runs.append(rows)
result = {"layout": runs[0][0], "independent_processes": 5, "bulk_lookup_only": True, "workloads": []}
for index in (1, 2):
    resident = [run[index]["resident_ms"] for run in runs]
    paged = [run[index]["paged_ms"] for run in runs]
    assert all(run[index]["lookups"] == 20000 for run in runs)
    result["workloads"].append({"workload": runs[0][index]["workload"],
        "resident_median_ms": statistics.median(resident), "paged_median_ms": statistics.median(paged),
        "resident_min_max_ms": [min(resident), max(resident)], "paged_min_max_ms": [min(paged), max(paged)],
        "median_slowdown": statistics.median(paged) / statistics.median(resident),
        "page_reads": [run[index]["page_reads"] for run in runs]})
print(json.dumps(result, indent=2))
