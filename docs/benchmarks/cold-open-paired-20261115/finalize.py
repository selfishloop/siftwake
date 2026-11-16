"""Merge measured timing, loss accounting, and the independent FileIO audit."""
from datetime import datetime, timezone
import json
import math
from pathlib import Path


ROOT = Path(__file__).resolve().parent
ARCHIVED_ORACLE = ROOT.parent / "wal-prefix-readmission-20261115/DB/DB-legacy.jsonl"


def load(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def timing(row):
    queries = row["query_ms"]
    assert row["oracle"] and len(queries) == 30 and row["crc32"] == 1149311291
    return dict(open_ms=row["open_ms"], q1_ms=queries[0], first_result_ms=row["open_start_to_first_ms"],
                q30_ms=queries[29], q2_30_p95=sorted(queries[1:])[math.ceil(.95*29)-1])


def oracle(path):
    decoded = [json.loads(line) for line in path.read_text(encoding="utf-8-sig").splitlines()]
    assert decoded[0]["documents"] == 50000 and decoded[0]["oracle"] and decoded[0]["sequence"] == 3101
    cases = [item["actual_crc32"] for item in decoded if item["phase"] == "case_digest" and item["oracle"]]
    assert len(cases) == 41
    return cases


def main():
    controller = load(ROOT / "status.json")
    assert controller["status"] == "complete" and controller["source_unchanged"]
    for engine in ("fdb", "sqlite"):
        for label in ("before", "kill", "after"):
            record = load(ROOT / f"{engine}-{label}.jsonl")
            assert record["oracle"] and record["gets"] == 50000 and record["cases"] == 41
    assert oracle(ROOT / "frozen-fdb.jsonl") == oracle(ARCHIVED_ORACLE)
    rows = []
    for trial in controller["trials"]:
        directory = ROOT / f"trial-{trial['trial']:02}"
        admin = load(directory / "admin-result.json")
        decoded = directory / "decoded"
        proof = load(decoded / "identity.json")
        audit_path = decoded / "window-reconciliation.json"
        audit = load(audit_path) if audit_path.exists() else {}
        decoded_valid = proof["technical_pass"] or (
            proof["errors"] == ["merged loss or full event count"]
            and proof["merged_lost"] == 0 and audit.get("exact") is True
            and audit["rows"] == proof["protected_fileio_rows"]
            and audit["sha256"].lower() == proof["protected_fileio_sha"].lower()
        )
        valid = bool(decoded_valid and not admin["failure"] and admin["stop_status_idle"]
                     and all(admin[k] == 0 for k in ("start_exit","es_exit","et_exit","stop_exit"))
                     and proof["merged_lost"] == 0
                     and proof["matched_files"] == proof["expected_files"]
                     and proof["total_attributed_disk_bytes"] > 0)
        rows.append(dict(trial=trial["trial"],engine=trial["engine"],trace_valid=valid,
            events_lost=proof["merged_lost"],matched_files=proof["matched_files"],
            expected_files=proof["expected_files"],storage_bytes=proof["total_attributed_disk_bytes"],
            sqlite_main_storage_bytes=proof["disk_bytes_by_name"].get("sqlite.db"),
            independent_rows=audit.get("rows"),trace_bytes=admin["merged_bytes"],
            cold=timing(trial["cold"]),warm=timing(trial["warm"])))
    etl_bytes = sum(p.stat().st_size for p in ROOT.rglob("*.etl"))
    evidence_bytes = sum(p.stat().st_size for p in ROOT.rglob("*") if p.is_file())
    result = dict(rows=rows,source_unchanged=True,fdb_runs=controller["fdb_runs"],
                  start_utc=controller["start_utc"],timing_controller_end_utc=controller["ended_utc"],
                  evidence_finalized_utc=datetime.now(timezone.utc).isoformat(),
                  etl_bytes=etl_bytes,evidence_bytes=evidence_bytes,all_timings_completed=len(rows)==4,
                  all_traces_verified=len(rows)==4 and all(r["trace_valid"] for r in rows))
    (ROOT / "results.json").write_text(json.dumps(result,indent=2)+"\n",encoding="utf-8")
    print(json.dumps(result,indent=2))


if __name__ == "__main__":
    main()
