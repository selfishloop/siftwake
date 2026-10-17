"""Reclaim the existing fixed slot, preserving live bytes and the existing corpus."""
import hashlib
import json
import struct
import subprocess
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
db = root / "data/benchmark-work/fdb"
prefix = root / "docs/benchmarks/reclaim-existing-10k-20261017"
if Path(f"{prefix}-before.json").exists():
    raise SystemExit("Preserve the existing report")
if not db.is_dir():
    raise SystemExit("Existing fixed slot required")

def digest(path):
    state = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            state.update(chunk)
    return state.hexdigest()

def snapshot(folder):
    return {p.name: {"bytes": p.stat().st_size, "sha256": digest(p)}
            for p in sorted(folder.iterdir()) if p.is_file()}

def protected():
    paths = ["data/rkyv-corpus-20261009/json.payload-bench", "data/benchmark-work/sqlite/sqlite.db",
             "tests/fixtures/v1/checkpoint.fdb", "tests/fixtures/v1/checkpoint.ready"]
    return {p: {"bytes": (root/p).stat().st_size, "sha256": digest(root/p)} for p in paths}

def save(suffix, value):
    Path(f"{prefix}-{suffix}.json").write_text(json.dumps(value, indent=2)+"\n", encoding="utf-8")

def probe(name, program, *args):
    with Path(f"{prefix}-{name}.jsonl").open("w", encoding="utf-8") as log:
        subprocess.run([str(root/program), *args], cwd=root, stdout=log, check=True)

before = snapshot(db)
safe_before = protected()
save("before", before)
save("protected-before", safe_before)
ready = max(db.glob("generation-*.ready"))
raw = ready.read_bytes()
assert raw[:8] == b"FDBMAN03"
manifest = json.loads(raw[20:20+struct.unpack_from("<I",raw,16)[0]])
live = {ready.name, f"generation-{manifest['active']['id']:016x}.fdb"}
for run in manifest["runs"]:
    family = "run" if run["kind"] == "Run" else "generation"
    stem = f"{family}-{run['id']:016x}"
    live.update(p.name for p in db.glob(f"{stem}.*"))
probe("before-oracle", Path("output/seg05-default-reads-20261017.exe"), "--matrix", "17", "--matrix-check")
assert snapshot(db) == before, "Baseline read changed files"
probe("operation", Path("target/release/examples/reclaim_obsolete.exe"))
after = snapshot(db)
save("after", after)
removed = sorted(before.keys() - after.keys())
assert live <= after.keys(), "Reclaim removed a live file"
assert all(after[name] == before[name] for name in after), "Reclaim changed/created retained files"
result = json.loads(Path(f"{prefix}-operation.jsonl").read_text(encoding="utf-8"))
assert result["pending_source_checks"] == 0
assert result["before"]["documents"] == result["after"]["documents"] == 10000
assert result["before"]["sequence"] == result["after"]["sequence"] == 20
assert result["reclaimed"]["removed_files"] == len(removed)
assert result["reclaimed"]["reclaimed_bytes"] == sum(before[name]["bytes"] for name in removed)
probe("after-old-reader", Path("output/seg05-default-reads-20261017.exe"), "--matrix", "17", "--matrix-check")
probe("after-new-reader", Path("target/release/examples/measure_content_reads.exe"), "--matrix", "17", "--matrix-check")
probe("verify-full", Path("target/release/examples/measure_content_memory.exe"), "verify", str(db), "0", "10000")
assert snapshot(db) == after, "Post-cleanup reads changed retained files"
safe_after = protected()
save("protected-after", safe_after)
assert safe_after == safe_before, "Corpus, SQLite or v0.1 fixture changed"
summary = {"before_bytes": sum(p["bytes"] for p in before.values()), "after_bytes": sum(p["bytes"] for p in after.values()),
           "before_files": len(before), "after_files": len(after), "removed_files": removed,
           "reclaimed_bytes": result["reclaimed"]["reclaimed_bytes"], "validation_and_reclaim_ms": result["validation_and_reclaim_ms"],
           "live_files_byte_equal": True, "old_and_new_reader_38_cases_passed": True, "full10000_document_oracle_passed": True,
           "protected_files_unchanged": True, "live_manifest": manifest,
           "scope": "Existing first10000; same source/index bytes; fixed slot; no resets/new corpus/copies. Warm OS cache, no eviction. Explicit full integrity plus unlink time; no isolated delete, query/write timing or RAM measurement; file lengths, not physical block recovery."}
save("summary", summary)
print(json.dumps(summary, indent=2))
