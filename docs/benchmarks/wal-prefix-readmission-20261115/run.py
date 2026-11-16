"""Fixed-slot 50k readmission ABBA and paired uninterrupted disk check."""
import argparse
from datetime import datetime, timedelta, timezone
import importlib.util
from pathlib import Path
import shutil
import sys


OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
BUILD = ROOT / "target/wal-prefix-readmission-20261115"
EXAMPLES = BUILD / "build/release/examples"
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location(
    "prefix_replay", OUT.parent / "wal-prefix-remeasure-20261114/run.py"
)
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)
replay.OUT, replay.BUILD, replay.EXAMPLES = OUT, BUILD, EXAMPLES
bench = replay.bench
MAX_BUILD = 2 * 2**30
MAX_EVIDENCE = 64 * 2**20
MINUTES = 30


class Run(replay.Run):
    def event(self, kind, **fields):
        self.check_limits()
        super().event(kind, **fields)

    def check_limits(self):
        assert shutil.disk_usage(bench.SLOTS).free >= bench.MIN_FREE_BYTES
        assert replay.total(BUILD) < MAX_BUILD
        assert replay.total(OUT) < MAX_EVIDENCE


def short(run, label, model, env, candidate):
    env["FDB_LIVE_DISK_END"] = "3"
    path = run.run("fdb", label, "live-disk50-profile-churn", model, env, monitor=True)
    rounds = list(bench.rows(path, "live_disk50_round"))
    assert [row["round"] for row in rounds] == [1, 2, 3]
    assert all(len(row["commits"]) == 100 for row in rounds)
    payload, = bench.rows(path, "followup_profile")
    trace = payload["trace"]
    assert trace["workers"]["lost"] == trace["workers"]["prefix_lost"] == 0
    assert trace["commits"]["dropped_frames"] == 0
    frames = trace["commits"]["frames"]
    assert len(frames) == 300
    retried = sum(any(p["name"] == "stream.prefix_readmission" for p in f["phases"]) for f in frames)
    readmitted = sum(any(p["name"] == "stream.prefix_readmitted" for p in f["phases"]) for f in frames)
    assert (retried > 0) == candidate
    assert readmitted <= retried
    return dict(label=label, commits=300, rounds=3, final_sequence=400,
                retried=retried, readmitted=readmitted)


def continuous(run, model, env):
    replay.continuous.OUT = bench.OUT
    replay.continuous.EXE = EXAMPLES / "measure_churn.exe"
    path = replay.continuous.timed(run, model, env)
    summary = replay.continuous.summarize(path)
    bench.save(bench.OUT / "disk-summary.json", summary)
    return dict(label=bench.OUT.name, commits=3000, rounds=30, final_sequence=3100,
                disk_gate=summary["gate"], max_round_ratio=summary["max_round_ratio"])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("ab", "disk"))
    mode = parser.parse_args().mode
    start = datetime.fromisoformat((OUT / "start.utc").read_text().strip())
    deadline = start + timedelta(minutes=MINUTES)
    assert datetime.now(timezone.utc) < deadline
    order = ("A1", "B1", "B2", "A2") if mode == "ab" else ("DA", "DB")
    registry = bench.read(bench.REGISTRY)
    assert all(s["status"] == "ready" for s in registry["slots"].values())
    assert (bench.SLOTS / ".fdb-benchmark-owned").read_text(encoding="utf-8-sig").strip() == "FDB benchmark workspace v1"
    assert replay.marker("fdb").is_file()
    if mode == "disk":
        prior = bench.read(OUT / "status-ab.json")
        assert prior["status"] == "complete"
        assert all(x["readmitted"] > 0 for x in prior["arms"] if x["label"].startswith("B"))
    audit = bench.read(bench.AUDIT)
    protected = bench.BASE.protected()
    sqlite = registry["slots"]["sqlite"].copy()
    sqlite_inventory = replay.inventory("sqlite")
    prior = registry["slots"]["fdb"]
    assert bench.sha(Path(prior["state"])) == prior["model_sha256"]
    assert bench.sha(bench.CORPUS) == audit["payload_sha256"] == registry["sha256"]
    assert bench.sha(bench.CATALOG) == audit["catalog_sha256"]
    assert bench.sha(bench.FROZEN) == replay.FROZEN_SHA
    assert all((EXAMPLES / name).is_file() for name in ("measure_churn.exe", "repair_block3a.exe"))

    status = dict(start_utc=start.isoformat(), deadline_utc=deadline.isoformat(),
                  mode=mode, order=order, arms=[])
    try:
        for label in order:
            run = Run(registry, label, MINUTES*60, 22, 50_000, origin=start)
            model = bench.OUT / f"{label}-model.json"
            seed, env = replay.environments()
            candidate = label.startswith("B") or label == "DB"
            if candidate:
                env["FDB_BENCH_PREFIX_READMISSION"] = "1"
            error = None
            try:
                run.check_limits()
                run.seed(label, model, seed)
                result = (short(run, label, model, env, candidate) if mode == "ab"
                          else continuous(run, model, env))
                sequence = result["final_sequence"]
                assert bench.read(model)["sequence"] == sequence
                first = order[0]
                reference = OUT / first / f"{first}-model.json" if label != first else None
                replay.finish_50k(run, label, model, env, result, sequence+1, reference)
                assert result["reopen"]["oracle"] and result["frozen_reader"]
                status["arms"].append(result)
                print(f"{label}: {result['commits']} commits, kill/reopen and frozen reader passed", flush=True)
            except BaseException as failure:
                error = failure
                raise
            finally:
                run.close(error)
        status["status"] = "complete"
    except BaseException as failure:
        status.update(status="stopped", error=repr(failure))
        raise
    finally:
        status.update(
            ended_utc=bench.utc(), temporary_bytes=replay.total(BUILD),
            evidence_bytes=replay.total(OUT),
            sqlite_unchanged=sqlite == bench.read(bench.REGISTRY)["slots"]["sqlite"]
            and sqlite_inventory == replay.inventory("sqlite"),
            corpus_unchanged=bench.sha(bench.CORPUS) == audit["payload_sha256"],
            fixtures_unchanged=bench.BASE.protected() == protected,
        )
        bench.save(OUT / f"status-{mode}.json", status)


if __name__ == "__main__":
    main()
