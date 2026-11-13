"""Verify the production default with one 30-round run in the owned FDB slot."""
from datetime import datetime, timezone
import importlib
from pathlib import Path
import shutil
import sys
import time

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
EXE = ROOT / "target/release/examples/measure_churn.exe"
sys.path.insert(0, str(ROOT / "tools"))
bench = importlib.import_module("measure-live-disk50")
bench.OUT = OUT
bench.ORDER = ("default-fdb",)
bench.PROGRAMS = {"fdb": EXE}
bench.VERIFY_PROGRAMS = bench.PROGRAMS
bench.MAX_SECONDS = 20 * 60


def source_hashes():
    return {str(p.relative_to(ROOT)): bench.sha(p) for p in (ROOT / "src").rglob("*.rs")}


def main():
    bench.START = time.monotonic()
    assert not (OUT / "status.json").exists()
    registry = bench.read(bench.REGISTRY)
    slot = registry["slots"]["fdb"]
    assert slot["status"] == registry["slots"]["sqlite"]["status"] == "ready"
    assert slot["sequence"] == 3101
    previous = Path(slot["state"])
    assert bench.sha(previous) == slot["model_sha256"]
    assert (bench.SLOTS / ".fdb-benchmark-owned").read_text(encoding="utf-8-sig").strip() == (
        "FDB benchmark workspace v1"
    )
    assert shutil.disk_usage(bench.SLOTS).free > bench.MIN_FREE_BYTES + bench.MAX_SLOT_BYTES
    audit = bench.read(bench.AUDIT)
    assert bench.sha(bench.CORPUS) == registry["sha256"] == audit["payload_sha256"]
    assert bench.sha(bench.CATALOG) == audit["catalog_sha256"]
    assert bench.sha(bench.FROZEN) == "B7845966E330624407D66416225F9395ED20272DD0065BAA996E4CC26D93D939"
    fixtures = bench.BASE.protected()
    source = source_hashes()
    bench.save(OUT / "initial-slot-registry.json", registry)
    run = bench.Experiment(registry)
    model = OUT / "disk-model.json"
    run.ledger.update(
        binary_sha=bench.sha(EXE),
        corpus_sha=audit["payload_sha256"],
        deadline_utc=datetime.fromtimestamp(
            time.time() + bench.MAX_SECONDS - (time.monotonic() - bench.START), timezone.utc
        ).isoformat(),
    )
    bench.save(OUT / "status.json", run.ledger)
    try:
        run.active = ("fdb", "default-fdb", model)
        run.slot("fdb", "default-fdb", "measuring_background_reclaim_default", model)
        bench.BASE.reset("fdb")
        seed = bench.environment()
        run.run("fdb", "disk-seed", "scale-seed", model, seed)
        env = {**seed, "FDB_LIVE_DISK_FIRST": "0", "FDB_LIVE_DISK_END": "30"}
        env.pop("FDB_RECLAMATION_CONTROL")
        log = run.run("fdb", "disk-30", "live-disk50-default-churn", model, env, monitor=True)
        baseline, = bench.rows(log, "live_disk50_baseline")
        rounds = list(bench.rows(log, "live_disk50_round"))
        assert [r["round"] for r in rounds] == list(range(1, 31))
        assert all(len(r["commits"]) == 100 for r in rounds)
        initial = baseline["disk"]["retained_bytes"]
        ratios = [row["after"]["retained_bytes"] / initial for row in rounds]
        result = dict(
            label="default-fdb",
            engine="fdb",
            initial_bytes=initial,
            ratios=ratios,
            max_ratio=max(ratios),
            final_ratio=ratios[-1],
            sampled_peak_bytes=bench.read(log.with_suffix(".process.json"))["observed_peak_bytes"],
            disk_gate=all(row["after"]["retained_bytes"] * 10 <= initial * 13 for row in rounds),
        )
        bench.save(OUT / "disk-performance.json", result)
        run.run("fdb", "default-fdb-kill", "live-disk50-kill", model, env, kill=True)
        assert bench.read(model)["sequence"] == 3101
        run.verify_arm("fdb", "default-fdb", result, model, env)
        run.ledger.update(status="complete", disk_gate=result["disk_gate"])
        print("30-round gate", result["disk_gate"], "peak/final", result["max_ratio"], result["final_ratio"])
    except BaseException as error:
        run.ledger.update(status="stopped", error=repr(error))
        if run.active:
            run.slot("fdb", "default-fdb", "blocked_partial_background_reclaim_default", model)
        raise
    finally:
        run.ledger.update(
            ended_utc=bench.utc(),
            source_unchanged=source_hashes() == source,
            fixtures_unchanged=bench.BASE.protected() == fixtures,
            corpus_unchanged=bench.sha(bench.CORPUS) == audit["payload_sha256"],
            prior_model_unchanged=bench.sha(previous) == slot["model_sha256"],
        )
        bench.save(OUT / "status.json", run.ledger)


if __name__ == "__main__":
    main()
