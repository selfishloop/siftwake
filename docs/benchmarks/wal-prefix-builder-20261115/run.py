"""Use the single FDB slot for one detail arm, then four balanced arms."""
import argparse
from datetime import datetime, timedelta, timezone
import importlib.util
from pathlib import Path
import shutil
import sys


OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
BUILD = ROOT / "target/wal-prefix-builder-20261115"
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
MAX_EVIDENCE = 48 * 2**20
MINUTES = 25


class Run(replay.Run):
    def event(self, kind, **fields):
        self.check_limits()
        super().event(kind, **fields)

    def check_limits(self):
        assert shutil.disk_usage(bench.SLOTS).free >= bench.MIN_FREE_BYTES
        assert replay.total(BUILD) < MAX_BUILD
        assert replay.total(OUT) < MAX_EVIDENCE


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("detail", "ab"))
    mode = parser.parse_args().mode
    start = datetime.fromisoformat((OUT / "start.utc").read_text().strip())
    deadline = start + timedelta(minutes=MINUTES)
    assert datetime.now(timezone.utc) < deadline
    order = ("D",) if mode == "detail" else ("A1", "B1", "B2", "A2")
    registry = bench.read(bench.REGISTRY)
    assert all(s["status"] == "ready" for s in registry["slots"].values())
    assert (bench.SLOTS / ".fdb-benchmark-owned").read_text(encoding="utf-8-sig").strip() == "FDB benchmark workspace v1"
    assert replay.marker("fdb").is_file()
    if mode == "ab":
        assert bench.read(OUT / "status-detail.json")["status"] == "complete"
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
            run = Run(registry, label, MINUTES*60, 16, 50_000, origin=start)
            model = bench.OUT / f"{label}-model.json"
            seed, env = replay.environments()
            seed["FDB_TIMING_DISABLED"] = "1"
            env.update(FDB_LIVE_DISK_END="3", FDB_TIMING_DISABLED="1")
            if label == "D":
                env["FDB_PREFIX_DETAILED"] = "1"
            if label.startswith("B"):
                env["FDB_PREFIX_POSTING_CANDIDATE"] = "1"
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
                assert trace["workers"]["lost"] == trace["workers"]["prefix_lost"] == 0
                assert trace["commits"]["dropped_frames"] == 0
                assert len(trace["commits"]["frames"]) == 300
                assert trace["workers"]["prefix"]
                assert bench.read(model)["sequence"] == 400
                result = dict(label=label, commits=300, rounds=3, final_sequence=400)
                reference = OUT / "D/D-model.json" if label != "D" else None
                replay.finish_50k(run, label, model, env, result, 401, reference)
                assert result["reopen"]["oracle"] and result["frozen_reader"]
                status["arms"].append(result)
                print(f"{label}: 300 commits, kill/reopen and frozen reader passed", flush=True)
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
