"""Summarize existing opening logs; never open or modify databases."""
import argparse
import json
import re
from pathlib import Path
from statistics import median

parser = argparse.ArgumentParser()
parser.add_argument("prefix")
parser.add_argument("--test-prefix", default="docs/benchmarks/open-strict-tests")
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
prefix = root / args.prefix


def read(suffix):
    return json.loads(Path(f"{prefix}-{suffix}.json").read_text(encoding="utf-8-sig"))


def phases(suffix):
    return {r["phase"]: r for r in map(json.loads, Path(f"{prefix}-{suffix}.jsonl").read_text(encoding="utf-8-sig").splitlines())}


result = {"design": read("design"), "modes": {}, "memory": {}, "tests": {}}
for mode in result["design"]["modes"]:
    rows = {}
    for engine in ("control", "fdb"):
        rows[engine] = []
        for pair in range(result["design"]["pairs"]):
            log = phases(f"{mode}-{engine}-{pair}")
            opened = log["opened"]
            assert opened["stats"]["documents"] == 10000 and opened["stats"]["sequence"] == 20
            assert opened["stats"]["segments"] == 5 and opened["stats"]["delta_keys"] == 0
            assert opened["source_validation_on_open"] == (mode != "record-first")
            assert opened["pending_source_checks"] == (5 if mode == "record-first" else 0)
            row = {"open_ms": opened["open_ms"]}
            if mode != "strict":
                use = log["deferred_first_use"]
                assert use["pending_on_open"] == opened["pending_source_checks"]
                assert use["pending_after_get"] == opened["pending_source_checks"]
                row.update({k: use[k] for k in ("first_get_ms", "first_search_ms", "warm_search_ms")})
                row["open_get_search_ms"] = row["open_ms"] + row["first_get_ms"] + row["first_search_ms"]
            rows[engine].append(row)
    medians = {e: {k: median(row[k] for row in series) for k in series[0]} for e, series in rows.items()}
    changes = {k: (medians["fdb"][k] / medians["control"][k] - 1) * 100 for k in medians["control"]}
    paired = {k: [(b[k] / a[k] - 1) * 100 for a, b in zip(rows["control"], rows["fdb"])] for k in changes}
    result["modes"][mode] = {"rows": rows, "medians": medians, "changes_percent": changes, "paired_changes_percent": paired,
                             "lower_pairs": {k: sum(v < 0 for v in series) for k, series in paired.items()}}
if result["design"]["query_memory_pairs"]:
    for engine in ("control", "fdb"):
        series = []
        for pair in range(result["design"]["query_memory_pairs"]):
            log = phases(f"memory-{engine}-{pair}")
            verified = log["query_memory_verified"]
            assert not verified["oracle_loaded"] and verified["searches"] == 24 and verified["gets"] == 771
            memory = read(f"memory-{engine}-{pair}.memory")
            assert memory["exit_code"] == 0
            series.append(memory["os_lifetime_peaks"])
        result["memory"][engine] = {k: median(row[k] for row in series) for k in ("peak_private_commit_bytes", "peak_working_set_bytes")}
assert read("before") == read("after")
result["source_cache_hashes_unchanged"] = True
for label in ("default-final", "reference", "all"):
    name = root / f"{args.test_prefix}-{label}-20261017.log"
    text = name.read_text(encoding="utf-8-sig")
    assert not re.search(r"test result: FAILED|error: test failed", text)
    counts = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", text)
    assert counts and all(failed == "0" for _, failed, _ in counts)
    result["tests"][label] = {"log": str(name.relative_to(root)), "passed": sum(int(p) for p, _, _ in counts), "ignored_helpers": sum(int(i) for _, _, i in counts)}
result["scope"] = "same first10k existing source;strict full validation;warm OS cache;no eviction;no mutations or new corpus;query-memory lifetime peaks exclude OS file cache"
Path(f"{prefix}-summary.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps(result, indent=2))
