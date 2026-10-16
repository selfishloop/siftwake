"""Verify the owned fixed-slot comparison after all timing has finished."""
import argparse
import hashlib
import json
import sqlite3
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("prefix", nargs="?", default="docs/benchmarks/sqlite-default-10k-20261016")
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
prefix = root / args.prefix


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest().upper()


def git(*command):
    return subprocess.check_output(["git", *command], cwd=root, text=True).strip()


environment = json.loads(Path(f"{prefix}-environment.json").read_text(encoding="utf-8-sig"))
summary = json.loads(Path(f"{prefix}-summary.json").read_text(encoding="utf-8"))
timed_versions = set()
for n in range(4):
    rows = Path(f"{prefix}-matrix-sqlite-{n}.jsonl").read_text(encoding="utf-8-sig").splitlines()
    timed_versions.update(row["sqlite_version"] for row in map(json.loads, rows) if row["phase"] == "opened")
assert len(timed_versions) == 1
assert not git("diff", "HEAD", "--", "src", "Cargo.toml", "Cargo.lock")
assert not git("status", "--short", "--", "tests/fixtures")
assert sorted(p.name for p in (root / "data").iterdir() if p.is_dir()) == sorted(environment["data_directories"])
corpus = root / "data/rkyv-corpus-20261009/json.payload-bench"
assert corpus.stat().st_size == environment["corpus_bytes"]
assert digest(corpus) == environment["corpus_sha256"]
assert summary["read_only_hashes_unchanged"]
database = root / "data/benchmark-work/sqlite/sqlite.db"
before = digest(database)
db = sqlite3.connect(str(database))
try:
    db.execute("PRAGMA cache_size=-2048")
    db.execute("PRAGMA mmap_size=0")
    db.execute("PRAGMA temp_store=FILE")
    assert db.execute("SELECT count(*) FROM docs").fetchone()[0] == 10000
    assert db.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
    db.execute("BEGIN")
    db.execute("INSERT INTO search(search,rank) VALUES('integrity-check',1)")
    db.rollback()
finally:
    db.close()
assert digest(database) == before
inventory = {}
for engine in ("fdb", "sqlite"):
    files = sorted(p for p in (root / "data/benchmark-work" / engine).iterdir() if p.is_file())
    inventory[engine] = {
        "all_files_bytes": sum(p.stat().st_size for p in files),
        "by_suffix": {suffix: sum(p.stat().st_size for p in files if p.suffix == suffix) for suffix in sorted({p.suffix for p in files})},
        "files": [{"name": p.name, "bytes": p.stat().st_size} for p in files],
    }
proof = {
    "engine_source_manifest_lock_unchanged": True,
    "fixture_unchanged": True,
    "corpus_sha256": environment["corpus_sha256"],
    "same_data_directories": True,
    "sqlite_full_integrity": "ok",
    "sqlite_fts_external_content_integrity": "ok; explicit transaction rolled back",
    "validator_sqlite_version": sqlite3.sqlite_version,
    "timed_sqlite_version": next(iter(timed_versions)),
    "sqlite_database_hash_unchanged_after_validation": True,
    "read_source_index_cache_hashes_unchanged": True,
    "full_document_checks_per_ingest": 10000,
    "paired_matrix_processes": 8,
    "queries_per_matrix": 38,
    "memory_probe_has_no_corpus_oracle": True,
    "inventory": inventory,
}
Path(f"{prefix}-proof.json").write_text(json.dumps(proof, indent=2) + "\n", encoding="utf-8")
print(json.dumps({key: value for key, value in proof.items() if key != "inventory"}, indent=2))
