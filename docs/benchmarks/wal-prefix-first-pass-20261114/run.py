"""One bounded profiling arm in the owned FDB slot."""
from datetime import datetime, timedelta, timezone
import importlib.util
from pathlib import Path
import shutil
import sys


OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
BUILD = ROOT / "target/wal-prefix-first-pass-20261114"
EXAMPLES = BUILD / "build/release/examples"
sys.dont_write_bytecode = True
previous = OUT.parent / "wal-prefix-remeasure-20261114" / "run.py"
spec = importlib.util.spec_from_file_location("prefix_replay", previous)
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)
replay.OUT = OUT
replay.BUILD = BUILD
replay.EXAMPLES = EXAMPLES
bench = replay.bench
MAX_BUILD = 2 * 2**30
MAX_EVIDENCE = 32 * 2**20


class Run(replay.Run):
    def event(self, kind, **fields):
        self.check_limits()
        super().event(kind, **fields)

    def check_limits(self):
        assert shutil.disk_usage(bench.SLOTS).free >= bench.MIN_FREE_BYTES
        assert replay.total(BUILD) < MAX_BUILD
        assert replay.total(OUT) < MAX_EVIDENCE


def main():
    deadline = datetime.fromisoformat((OUT / "deadline.utc").read_text().strip().replace("Z", "+00:00"))
    start = deadline - timedelta(minutes=20)
    assert datetime.now(timezone.utc) < deadline
    registry = bench.read(bench.REGISTRY)
    assert all(s["status"] == "ready" for s in registry["slots"].values())
    assert (bench.SLOTS / ".fdb-benchmark-owned").read_text(encoding="utf-8-sig").strip() == "FDB benchmark workspace v1"
    assert replay.marker("fdb").is_file()
    prior = registry["slots"]["fdb"]
    prior_model = Path(prior["state"])
    sqlite = registry["slots"]["sqlite"].copy()
    sqlite_inventory = replay.inventory("sqlite")
    audit = bench.read(bench.AUDIT)
    protected = bench.BASE.protected()
    assert bench.sha(prior_model) == prior["model_sha256"]
    assert bench.sha(bench.CORPUS) == audit["payload_sha256"] == registry["sha256"]
    assert bench.sha(bench.CATALOG) == audit["catalog_sha256"]
    assert bench.sha(bench.FROZEN) == replay.FROZEN_SHA
    assert (EXAMPLES / "measure_churn.exe").is_file()
    assert (EXAMPLES / "repair_block3a.exe").is_file()

    run = Run(registry, "01-profile", 20*60, 20, 50_000, origin=start)
    label = "first-pass"
    model = bench.OUT / f"{label}-model.json"
    seed, env = replay.environments()
    env.update(FDB_LIVE_DISK_END="3")
    assert "FDB_BENCH_DEFER_FLUSH_MERGE" not in env
    status = dict(start_utc=start.isoformat(), deadline_utc=deadline.isoformat())
    error = None
    try:
        run.check_limits()
        run.seed(label, model, seed)
        raw = run.run("fdb", label, "live-disk50-profile-churn", model, env, monitor=True)
        rounds = list(bench.rows(raw, "live_disk50_round"))
        assert [row["round"] for row in rounds] == [1, 2, 3]
        assert all(len(row["commits"]) == 100 for row in rounds)
        payload, = bench.rows(raw, "followup_profile")
        trace = payload["trace"]
        assert trace["commits"]["dropped_frames"] == 0
        assert len(trace["commits"]["frames"]) == 300
        assert trace["workers"]["lost"] == trace["workers"]["prefix_lost"] == 0
        assert len(trace["workers"]["prefix"]) > 0
        assert all(frame["admission"] for frame in trace["commits"]["frames"])
        assert bench.read(model)["sequence"] == 400
        result = dict(label=label, commits=300, rounds=3, final_sequence=400)
        replay.finish_50k(run, label, env=env, result=result, model=model, sequence=401)
        assert result["reopen"]["oracle"] and result["frozen_reader"]
        status["result"] = result
        status["status"] = "complete"
        print("300 profiled commits + kill/reopen + frozen reader", flush=True)
    except BaseException as failure:
        error = failure
        status.update(status="stopped", error=repr(failure))
        raise
    finally:
        run.close(error)
        status.update(
            ended_utc=bench.utc(),
            temporary_bytes=replay.total(BUILD),
            evidence_bytes=replay.total(OUT),
            sqlite_unchanged=sqlite == bench.read(bench.REGISTRY)["slots"]["sqlite"]
            and sqlite_inventory == replay.inventory("sqlite"),
            corpus_unchanged=bench.sha(bench.CORPUS) == audit["payload_sha256"],
            fixtures_unchanged=bench.BASE.protected() == protected,
            prior_model_unchanged=bench.sha(prior_model) == prior["model_sha256"],
        )
        bench.save(OUT / "status.json", status)


if __name__ == "__main__":
    main()
