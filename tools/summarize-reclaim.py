"""Aggregate existing reclaim logs only; never open/reset a database or corpus."""
import json
import statistics
import sys
from pathlib import Path

mode, name = sys.argv[1:]
prefix = Path(name)

def load(suffix):
    return json.loads(Path(f"{prefix}-{suffix}.json").read_text(encoding="utf-8-sig"))

def phases(suffix):
    return {r["phase"]: r for r in map(json.loads, Path(f"{prefix}-{suffix}.jsonl").read_text(encoding="utf-8-sig").splitlines())}

design = load("design")
rows = []
live = []
for pair in range(design["pairs"]):
    for variant in ("control", "fdb"):
        stem = f"{variant}-{pair}"
        if mode == "writes":
            result = phases(f"{stem}-fdb-ingest-10000-0")["result"]
            memory = load(f"{stem}-fdb-ingest-10000-0.memory")
            verified = phases(f"{stem}-verify")["verified"]
            assert verified["full_document_checks"] == 10000 and verified["input_crc_verified"] and verified["first50_full_pages_verified"]
            assert result["text_bytes"] == 264842858 and result["stats"]["documents"] == 10000 and result["stats"]["delta_keys"] == 0 and result["stats"]["sequence"] == 20
            assert memory["exit_code"] == 0 and not memory["low_disk_stop"]
            live.append(load(f"{stem}-live"))
            values = dict(ingest_s=result["ingest_ms"]/1000, maintenance_s=result["maintenance_ms"]/1000,
                          combined_s=(result["ingest_ms"]+result["maintenance_ms"])/1000, cpu_s=memory["cpu_ms"]/1000,
                          private_bytes=memory["os_lifetime_peaks"]["peak_private_commit_bytes"],
                          ws_bytes=memory["os_lifetime_peaks"]["peak_working_set_bytes"], retained_disk_bytes=memory["retained_disk_bytes"])
        elif mode == "open":
            result = phases(stem)
            first = result["deferred_first_use"]
            assert result["opened"]["pending_source_checks"] == first["pending_on_open"] == first["pending_after_get"] == 0
            values = dict(open_ms=result["opened"]["open_ms"], first_use_ms=result["opened"]["open_ms"]+first["first_get_ms"]+first["first_search_ms"])
        else:
            raise SystemExit("usage: summarize-reclaim.py writes|open docs/benchmarks/prefix")
        rows.append(dict(variant=variant, pair=pair, **values))
fields = [k for k in rows[0] if k not in ("variant", "pair")]
medians = {e: {k: statistics.median(r[k] for r in rows if r["variant"] == e) for k in fields} for e in ("control", "fdb")}
comparisons = {}
for field in fields:
    old = [r[field] for r in rows if r["variant"] == "control"]
    new = [r[field] for r in rows if r["variant"] == "fdb"]
    comparisons[field] = dict(change_percent=100*(medians["fdb"][field]/medians["control"][field]-1),
                              lower_pairs=sum(b<a for a,b in zip(old,new)), paired_change_percent=[100*(b/a-1) for a,b in zip(old,new)])
summary = dict(design=design, rows=rows, medians=medians, comparisons=comparisons,
               limits="Warm OS cache, no eviction/hardware isolation. Limited pairs, no general performance/RAM gate. No new SQLite, query p95, concurrent, cold or power-failure measurement. Memory is OS lifetime private commit/WS, not total system RAM.")
if mode == "writes":
    assert all(v == live[0] for v in live) and load("final-live") == live[0]
    assert load("protected-before") == load("protected-after")
    summary.update(all_live_hashes_equal=True, protected_files_unchanged=True, final_files=load("final-files"))
else:
    assert load("before") == load("after")
    summary.update(all_slot_hashes_unchanged=True)
Path(f"{prefix}-summary.json").write_text(json.dumps(summary, indent=2)+"\n", encoding="utf-8")
print(json.dumps(comparisons, indent=2))
