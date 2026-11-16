"""Compare deferred fallback-merge scheduling with the original policy in the owned slot."""
from collections import Counter
import importlib.util
import json
from pathlib import Path
import sys

OUT = Path(__file__).resolve().parent
sys.dont_write_bytecode = True
previous = OUT.parent / "wal-prefix-remeasure-20261114" / "run.py"
spec = importlib.util.spec_from_file_location("replay_prefix", previous)
replay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(replay)
replay.OUT = OUT
replay.BUILD = replay.ROOT / "target/normal-merge-defer-20261114"
replay.EXAMPLES = replay.BUILD / "build/release/examples"
bench = replay.bench
ram = replay.load("04-run")
confirm = replay.ORIGINAL / "02-profile-abba/A1-model.json"
FLAG = "FDB_BENCH_DEFER_FLUSH_MERGE"
all_file_bytes = replay.total


def release_measurement_bytes(path):
    # Run's build guard covers the release executables used by the measurement.
    # Keep and report the debug test outputs alongside the measurement build.
    if path == replay.BUILD:
        return all_file_bytes(path / "source") + all_file_bytes(path / "build/release")
    return all_file_bytes(path)


replay.total = release_measurement_bytes


def env_for(defer):
    seed, work = replay.environments()
    seed.pop(FLAG, None)  # Same clean initial layout in all four arms.
    work.pop(FLAG, None)
    if defer:
        work[FLAG] = "1"
    return seed, work


def joins(log, summary):
    entry, = bench.rows(log, "followup_profile")
    frames = entry["trace"]["commits"]["frames"]
    aux = entry["trace"]["workers"]["aux"]
    longs = {row["sequence"] for row in summary["over_1s"]}
    by_seq = {}
    phase_count = Counter()
    phase_ms = Counter()
    for frame in frames:
        seq = frame["operation_sequence"]
        for phase in frame["phases"]:
            if phase["name"] != "merge.join":
                continue
            parents = ("stream.worker_drain", "stream.post_flush_drain")
            parent = next(
                (p["name"] for p in frame["phases"] if p["name"] in parents
                 and p["start_ns"] <= phase["start_ns"] <= phase["end_ns"] <= p["end_ns"]),
                "other",
            )
            phase_count[parent] += 1
            phase_ms[parent] += phase["elapsed_ns"] / 1e6
            by_seq.setdefault(seq, []).append({
                "location": parent, "ms": phase["elapsed_ns"] / 1e6,
                "output_id": phase["active_wal_id"], "over_1s": seq in longs,
            })
    rows = [{"sequence": f["operation_sequence"],
             **replay.analyze.describe(f, aux)} for f in frames
            if f["operation_sequence"] in longs or f["operation_sequence"] == 2511]
    return dict(counts=dict(phase_count), ms=dict(phase_ms), joins=by_seq, outliers=rows)


def performance(run):
    results = []
    # Profile state is paired across variants. All arms use the same binary.
    for label, defer, profiled in (
        ("A1-control", False, False), ("B1-defer", True, False),
        ("B2-defer", True, True), ("A2-control", False, True),
    ):
        seed, work = env_for(defer)
        model = bench.OUT / f"{label}-model.json"
        run.seed(label, model, seed)
        mode = "live-disk50-profile-churn" if profiled else "live-disk50-default-churn"
        log = run.run("fdb", label + "-30", mode, model, work, monitor=True)
        result = replay.profile.summarize(log, profiled)
        result.update(label=label, defer=defer)
        if profiled:
            data = joins(log, result)
            bench.save(bench.OUT / f"{label}-joins.json", data)
            result["join_counts"] = data["counts"]
            result["join_ms"] = data["ms"]
        bench.save(bench.OUT / f"{label}-timing.json", result)
        replay.finish_50k(run, label, model, work, result, 3101, confirm)
        results.append(result)
        bench.save(bench.OUT / "summary.json", results)
        print(label, "p99", round(result["p99"], 2), "max", round(result["max"], 2),
              ">1s", len(result["over_1s"]), flush=True)
    return True, None


def disk(run):
    seed, work = env_for(True)
    label = "continuous"
    model = bench.OUT / "continuous-model.json"
    run.seed(label, model, seed)
    replay.continuous.OUT = bench.OUT
    replay.continuous.EXE = run.exe
    result = replay.continuous.summarize(replay.continuous.timed(run, model, work))
    bench.save(bench.OUT / "disk-summary.json", result)
    replay.finish_50k(run, label, model, work, result, 3101, confirm)
    print("continuous", result["gate"], result["max_round_ratio"], flush=True)
    return result["gate"], model


def visibility(run, prior):
    from shutil import copyfile

    _, work = env_for(True)
    replay.arrival.OUT = bench.OUT
    model = bench.OUT / "01-arrival-model.json"
    copyfile(prior, model)
    run.begin("arrival", model)
    result = replay.arrival.run_timed(run, model, work)
    opened, = bench.rows(bench.OUT / "arrival.jsonl", "opened")
    result["initial_stats"] = opened["stats"]
    bench.save(bench.OUT / "01-arrival-summary.json", result)
    replay.finish_50k(run, "arrival", model, work, result, 3750,
                      replay.ORIGINAL / "01-arrival-model.json")
    print("visibility", result["gate"], result["stats"]["background"]["arrival_to_visible_ms"], flush=True)
    return result["gate"], model


def memory(run):
    ram.OUT = bench.OUT
    ram.EXE = replay.EXAMPLES / "measure_engine_only.exe"
    _, work = env_for(True)
    label = "defer"
    model_path = bench.OUT / "ram-model.json"
    run.begin(label, model_path)
    bench.BASE.reset("fdb")
    catalog = bench.read(bench.CATALOG)["entries"]
    initial_model = ram.fresh_model()
    ingest = ram.child_run(run, label, "ingest", work, ((i, i, 0) for i in range(50_000)), catalog)
    idle = ram.child_run(run, label, "idle", work, None, catalog)
    ingest_mib = ingest["ingest"]["memory"]["peak_private_commit_bytes"] / 2**20
    idle_mib = idle["idle"]["memory"]["private_bytes"] / 2**20
    result = dict(label=label, engine="fdb", ingest=ingest, idle=idle,
                  ingest_mib=ingest_mib, idle_mib=idle_mib, gate=ingest_mib<=60 and idle_mib<=15)
    bench.save(bench.OUT / "ram-summary.json", result)
    bench.save(model_path, initial_model)
    replay.finish_50k(run, label, model_path, work, result, 101)
    print("motor-only ingest / idle", ingest_mib, idle_mib, "gate", result["gate"], flush=True)
    return result["gate"], model_path


def main():
    registry = bench.read(bench.REGISTRY)
    assert all(s["status"] == "ready" for s in registry["slots"].values())
    assert (bench.SLOTS / ".fdb-benchmark-owned").read_text(encoding="utf-8-sig").strip() == "FDB benchmark workspace v1"
    assert replay.marker("fdb").is_file()
    previous_model = Path(registry["slots"]["fdb"]["state"])
    assert bench.sha(previous_model) == registry["slots"]["fdb"]["model_sha256"]
    sqlite_entry = registry["slots"]["sqlite"].copy()
    sqlite_inventory = replay.inventory("sqlite")
    audit = bench.read(bench.AUDIT)
    assert bench.sha(bench.CORPUS) == audit["payload_sha256"] == registry["sha256"]
    assert bench.sha(bench.CATALOG) == audit["catalog_sha256"]
    assert bench.sha(bench.FROZEN) == replay.FROZEN_SHA
    fixtures = bench.BASE.protected()
    old_sha = bench.sha(previous_model)
    bench.save(OUT / "prior-slot-registry.json", registry)
    status = dict(start_utc=bench.utc(), completed=[])
    prior = None
    try:
        for name, minutes, evidence, execute in (
            ("01-commit", 45, 192, performance),
            ("02-continuous", 20, 64, disk),
            ("03-visibility", 15, 64, lambda run: visibility(run, prior)),
            ("04-ram", 25, 32, memory),
        ):
            run = replay.Run(registry, name, minutes*60, evidence, 50_000)
            error = None
            try:
                gate, prior = execute(run)
                run.ledger["gate"] = gate
            except BaseException as exc:
                error = exc
                raise
            finally:
                run.close(error)
            status["completed"].append(name)
            if not gate:
                status.update(status="stopped_gate", step=name)
                return
        status["status"] = "complete"
    except BaseException as exc:
        status.update(status="stopped", error=repr(exc))
        raise
    finally:
        status.update(end_utc=bench.utc(), sqlite_unchanged=sqlite_entry == bench.read(bench.REGISTRY)["slots"]["sqlite"]
                      and sqlite_inventory == replay.inventory("sqlite"),
                      corpus_unchanged=bench.sha(bench.CORPUS) == audit["payload_sha256"],
                      fixtures_unchanged=bench.BASE.protected() == fixtures,
                      prior_model_unchanged=bench.sha(previous_model) == old_sha)
        bench.save(OUT / "status.json", status)


if __name__ == "__main__":
    main()
