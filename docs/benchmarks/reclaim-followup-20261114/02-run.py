"""Run the approved bounded ABBA profile. All DB writes use the single owned FDB slot."""
import importlib
import math
from datetime import datetime, timedelta, timezone
from pathlib import Path
import shutil
import statistics
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
BASE = Path(__file__).resolve().parent
OUT = BASE / ("02-profile-abba" if "--resume-seed" in sys.argv else "02-profile")
BUILD = ROOT / "target/reclaim-followup-20261114"
EXE = BUILD / "build/release/examples/measure_churn.exe"
sys.path.insert(0, str(ROOT / "tools"))
bench = importlib.import_module("measure-live-disk50")
bench.OUT = OUT
bench.ORDER = ("A1", "B1", "B2", "A2")
bench.PROGRAMS = bench.VERIFY_PROGRAMS = {"fdb": EXE}
bench.MAX_SECONDS = 45 * 60


def total(path):
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file())


class BoundedRun(bench.Experiment):
    def event(self, kind, **fields):
        if kind == "process_start" and total(OUT) >= 160 * 2**20:
            raise RuntimeError("not enough remaining profile evidence budget")
        super().event(kind, **fields)


def summarize(log, profiled):
    rows = list(bench.rows(log, "live_disk50_round"))
    assert [r["round"] for r in rows] == list(range(1, 31))
    commits = [dict(c, round=r["round"]) for r in rows for c in r["commits"]]
    assert [c["sequence"] for c in commits] == list(range(101, 3101))
    seq2511 = commits[2511 - 101]
    assert seq2511["sequence"] == 2511 and seq2511["round"] == 25
    observed = sorted(c["ms"] for c in commits)
    elapsed = sum(r["write_ms"] + r["maintenance_ms"] for r in rows)
    result = dict(
        engine="fdb", profiled=profiled, seq2511=seq2511,
        n=len(commits), p50=statistics.median(observed),
        p95=observed[math.ceil(.95 * len(observed))-1],
        p99=observed[math.ceil(.99 * len(observed))-1],
        p999=observed[math.ceil(.999 * len(observed))-1],
        max=observed[-1], ops_s=300_000_000 / elapsed,
        over_1s=[c for c in commits if c["ms"] > 1000],
    )
    if profiled:
        emitted, = bench.rows(log, "followup_profile")
        trace = emitted["trace"]
        assert trace["commits"]["dropped_frames"] == trace["workers"]["lost"] == 0
        frames = trace["commits"]["frames"]
        assert [f["operation_sequence"] for f in frames] == [c["sequence"] for c in commits]
        result["trace_coverage"] = dict(frames=len(frames), aux=len(trace["workers"]["aux"]))
    return result


def main():
    assert bench.read(BASE / "status.json")["gate"] is True
    order = bench.ORDER
    initial = bench.read(bench.REGISTRY)
    slot = initial["slots"]["fdb"]
    assert initial["slots"]["sqlite"]["status"] == "ready"
    if "--resume-seed" in sys.argv:
        interrupted = bench.read(BASE / "02-profile/status.json")
        assert interrupted["status"] == "stopped" and not interrupted["completed"]
        assert "FileNotFoundError" in interrupted["error"]
        assert not (BASE / "02-profile/A1-model.json").exists()
        rejected = bench.read(BASE / "02-profile-run/status.json")
        assert rejected["status"] == "stopped" and not rejected["completed"]
        assert (BASE / "02-profile-run/A1-seed.stderr").read_text().strip() == 'Error: "seed requires ownership-checked reset"'
        assert not (BASE / "02-profile-run/A1-model.json").exists()
        partial_slot = bench.SLOTS / "fdb"
        assert partial_slot.is_dir() and not any(partial_slot.iterdir())
        assert slot["status"] == "blocked_partial_reclaim_profile" and slot["label"] == "A1"
        old_model = BASE / "01-arrival-model.json"
        prior_sha = bench.sha(old_model)
        assert bench.read(old_model)["sequence"] == 3750
        original_start = datetime.fromisoformat(interrupted["start_utc"])
        original_deadline = datetime.fromisoformat(interrupted["deadline_utc"])
    else:
        assert slot["status"] == "ready"
        old_model = Path(slot["state"])
        prior_sha = slot["model_sha256"]
        assert slot["sequence"] == 3750 and bench.sha(old_model) == prior_sha
        original_start = datetime.now(timezone.utc)
        original_deadline = original_start + timedelta(seconds=bench.MAX_SECONDS)
    assert (bench.SLOTS / ".fdb-benchmark-owned").read_text(encoding="utf-8-sig").strip() == "FDB benchmark workspace v1"
    assert shutil.disk_usage(bench.SLOTS).free >= 23 * 2**30 and total(BUILD) < 2 * 2**30
    assert bench.sha(bench.CORPUS) == initial["sha256"] == bench.read(bench.AUDIT)["payload_sha256"]
    assert bench.sha(bench.CATALOG) == bench.read(bench.AUDIT)["catalog_sha256"]
    assert bench.sha(bench.FROZEN) == "B7845966E330624407D66416225F9395ED20272DD0065BAA996E4CC26D93D939"
    before = bench.BASE.protected()
    OUT.mkdir(exist_ok=False)
    bench.START = time.monotonic()
    run = BoundedRun(initial)
    bench.MAX_SECONDS = (original_deadline - datetime.now(timezone.utc)).total_seconds()
    assert bench.MAX_SECONDS > 0
    run.ledger.update(start_utc=original_start.isoformat(), deadline_utc=original_deadline.isoformat(),
                      build_bytes=total(BUILD), binary_sha=bench.sha(EXE), model_hashes={})
    bench.save(OUT / "status.json", run.ledger)
    seeds = bench.environment()
    env = {**seeds, "FDB_LIVE_DISK_FIRST": "0", "FDB_LIVE_DISK_END": "30"}
    env.pop("FDB_RECLAMATION_CONTROL")
    try:
        for arm in order:
            profiled = arm.startswith("B")
            model = OUT / f"{arm}-model.json"
            run.active = ("fdb", arm, model)
            run.slot("fdb", arm, "measuring_reclaim_profile", model)
            bench.BASE.reset("fdb")
            run.run("fdb", f"{arm}-seed", "scale-seed", model, seeds)
            mode = "live-disk50-profile-churn" if profiled else "live-disk50-default-churn"
            log = run.run("fdb", f"{arm}-30", mode, model, env, monitor=True)
            result = summarize(log, profiled)
            result["label"] = arm
            bench.save(OUT / f"{arm}-timing.json", result)
            run.run("fdb", f"{arm}-kill", "live-disk50-kill", model, env, kill=True, monitor=True)
            assert bench.read(model)["sequence"] == 3101
            digest = bench.sha(model)
            run.ledger["model_hashes"][arm] = digest
            assert len(set(run.ledger["model_hashes"].values())) == 1
            run.verify_arm("fdb", arm, result, model, env)
            print(arm, ">1s", len(result["over_1s"]), "seq2511", result["seq2511"]["ms"], flush=True)
            if total(OUT) > 192 * 2**20:
                raise RuntimeError("profile evidence budget exceeded")
        run.ledger["status"] = "complete"
    except BaseException as error:
        run.ledger.update(status="stopped", error=repr(error))
        if run.active:
            engine, label, model = run.active
            run.slot(engine, label, "blocked_partial_reclaim_profile", model)
        raise
    finally:
        run.ledger.update(ended_utc=bench.utc(), evidence_bytes=total(OUT),
                          fixtures_unchanged=bench.BASE.protected() == before,
                          corpus_unchanged=bench.sha(bench.CORPUS) == initial["sha256"],
                          prior_model_unchanged=bench.sha(old_model) == prior_sha)
        bench.save(OUT / "status.json", run.ledger)


if __name__ == "__main__":
    main()
