"""Summarize existing read-only JSONL runs; never touches corpus/databases."""
import json
import argparse
from pathlib import Path
from statistics import median

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument("--prefix", default="content-reads-10k-20261012")
parser.add_argument("--runs", type=int, default=3)
parser.add_argument("--engines", nargs="+", default=["fdb", "sqlite"])
args = parser.parse_args()
PREFIX = ROOT / "docs/benchmarks" / args.prefix


def read(engine, iteration):
    path = Path(f"{PREFIX}-{engine}-{iteration}.jsonl")
    rows = [json.loads(line) for line in path.read_text(encoding="utf-8-sig").splitlines()]
    return {row["phase"]: row for row in rows}


runs = {engine: [read(engine, i) for i in range(args.runs)] for engine in args.engines}
result = {"processes_per_engine": args.runs, "aggregation": "median of per-process nearest-rank percentiles; not pooled samples", "engines": {}}
for engine, samples in runs.items():
    queries = []
    for n, query in enumerate(samples[0]["reads"]["queries"]):
        assert all(run["reads"]["queries"][n]["total"] == query["total"] for run in samples)
        queries.append({"text": query["text"], "total": query["total"], **{
            metric: median(run["reads"]["queries"][n]["timing"][metric] for run in samples)
            for metric in ("p50_ms", "p95_ms")}, "runs": [run["reads"]["queries"][n]["timing"]["p95_ms"] for run in samples]})
    result["engines"][engine] = {
        "open_ms": median(run["opened"]["open_ms"] for run in samples),
        "get": {metric: median(run["reads"]["get"][metric] for run in samples) for metric in ("p50_ms", "p95_ms")},
        "queries": queries,
    }
for n in range(4):
    a, b = (result["engines"][engine]["queries"][n] for engine in ("fdb", "sqlite"))
    assert (a["text"], a["total"]) == (b["text"], b["total"])
    a["fdb_over_sqlite_p95"] = a["p95_ms"] / b["p95_ms"]
Path(f"{PREFIX}-summary.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps(result, indent=2))
