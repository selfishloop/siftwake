"""Replay the four approved tests using the original workloads and one owned FDB slot."""
import importlib.util
from datetime import datetime, timedelta, timezone
from pathlib import Path
import shutil
import sys
import time

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
ORIGINAL = OUT.parent / "reclaim-followup-20261114"
BUILD = ROOT / "target/wal-prefix-remeasure-20261114"
EXAMPLES = BUILD / "build/release/examples"
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "tools"))


def load(name):
    spec = importlib.util.spec_from_file_location("replay_" + name, ORIGINAL / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


arrival = load("01-run")
profile = load("02-run")
continuous = load("03-run")
pin = load("05-run")
analyze = load("02-analyze")
bench = arrival.bench
FROZEN_SHA = "B7845966E330624407D66416225F9395ED20272DD0065BAA996E4CC26D93D939"


def total(path):
    return sum(p.stat().st_size for p in path.rglob("*") if p.is_file())


def marker(which):
    return next(p for p in (bench.SLOTS / which).iterdir() if p.name == "LOCK")


def inventory(which):
    return {p.relative_to(bench.SLOTS / which).as_posix(): p.stat().st_size
            for p in (bench.SLOTS / which).rglob("*") if p.is_file()}


class Run(bench.Experiment):
    def __init__(self, registry, name, seconds, mib, n, origin=None):
        assert shutil.disk_usage(bench.SLOTS).free >= 23 * 2**30
        assert total(BUILD) < 2 * 2**30
        bench.OUT = OUT / name
        bench.OUT.mkdir()
        bench.ORDER = (name,)
        bench.START = time.monotonic()
        now = datetime.now(timezone.utc)
        origin = origin or now
        deadline = origin + timedelta(seconds=seconds)
        bench.MAX_SECONDS = (deadline - now).total_seconds()
        assert bench.MAX_SECONDS > 0
        super().__init__(registry)
        self.n = n
        self.cap = mib * 2**20
        suffix = "-10k" if n == 10_000 else ""
        self.exe = EXAMPLES / f"measure_churn{suffix}.exe"
        bench.PROGRAMS = bench.VERIFY_PROGRAMS = {"fdb": self.exe}
        bench.WRAPPER = EXAMPLES / f"repair_block3a{suffix}.exe"
        self.ledger.update(start_utc=origin.isoformat(), deadline_utc=deadline.isoformat(),
                           binary_sha=bench.sha(self.exe), evidence_limit_bytes=self.cap)
        bench.save(bench.OUT / "status.json", self.ledger)

    def event(self, kind, **fields):
        if kind == "process_start":
            assert total(bench.OUT) < self.cap - (8 if self.n == 50_000 else 6) * 2**20
        super().event(kind, **fields)

    def slot(self, engine, label, status, model):
        assert engine == "fdb"
        info = dict(documents=self.n, status=status, label=label, state=str(model))
        if model.is_file():
            info.update(sequence=bench.read(model)["sequence"], model_sha256=bench.sha(model))
        self.registry["slots"][engine] = info
        bench.save(bench.REGISTRY, self.registry)

    def begin(self, label, model):
        self.active = ("fdb", label, model)
        self.slot("fdb", label, "measuring_prefix_reset", model)

    def seed(self, label, model, env):
        self.begin(label, model)
        bench.BASE.reset("fdb")
        # Seed expects an absent slot, so it cannot use an inventory sampler yet.
        self.run("fdb", label + "-seed", "scale-seed", model, env)
        proof, = bench.rows(bench.OUT / f"{label}-seed.jsonl", "scale_verified")
        assert proof["oracle"] and proof["documents"] == self.n
        assert bench.read(model)["sequence"] == (20 if self.n == 10_000 else 100)

    def cap_check(self):
        assert total(bench.OUT) <= self.cap

    def close(self, error):
        if error is not None:
            self.ledger.update(status="stopped", error=repr(error))
            if self.active:
                engine, label, model = self.active
                self.slot(engine, label, "blocked_partial_prefix_remeasure", model)
        else:
            self.ledger["status"] = "complete"
        self.ledger.update(ended_utc=bench.utc(), evidence_bytes=total(bench.OUT))
        bench.save(bench.OUT / "status.json", self.ledger)


def environments(n=50_000):
    seed = {**bench.environment(), "FDB_SCALE_DOCUMENTS": str(n)}
    env = {**seed, "FDB_LIVE_DISK_FIRST": "0", "FDB_LIVE_DISK_END": "30"}
    env.pop("FDB_RECLAMATION_CONTROL")
    return seed, env


def finish_50k(run, label, model, env, result, sequence, expected_model=None):
    run.run("fdb", label + "-kill", "live-disk50-kill", model, env, kill=True, monitor=True)
    assert bench.read(model)["sequence"] == sequence
    if expected_model:
        assert bench.sha(model) == bench.sha(expected_model)
    run.verify_arm("fdb", label, result, model, env)
    run.cap_check()


def rebuild_arrival_start(run):
    seed, env = environments()
    model = bench.OUT / "recreated-model.json"
    run.seed("recreate-3101", model, seed)
    log = run.run("fdb", "recreate-3101", "live-disk50-default-churn", model, env, monitor=True)
    rounds = list(bench.rows(log, "live_disk50_round"))
    assert [row["round"] for row in rounds] == list(range(1, 31))
    assert all(len(row["commits"]) == 100 for row in rounds)
    previous = OUT.parent / "background-reclaim-default-50k-20261112" / "disk-model.json"
    finish_50k(run, "recreate-3101", model, env, dict(label="recreate-3101"), 3101, previous)
    print("Arrival start replayed and model matches historical sequence 3101.", flush=True)
    return model


def run_arrival(run, prior):
    arrival.OUT = bench.OUT
    _, env = environments()
    model = bench.OUT / "01-arrival-model.json"
    shutil.copyfile(prior, model)
    run.begin("arrival", model)
    summary = arrival.run_timed(run, model, env)
    opened, = bench.rows(bench.OUT / "arrival.jsonl", "opened")
    old, = bench.rows(ORIGINAL / "arrival.jsonl", "opened")
    summary["initial_stats"] = dict(historical=old["stats"], replay=opened["stats"])
    summary["startup_model_matches"] = True
    bench.save(bench.OUT / "01-arrival-summary.json", summary)
    assert bench.read(model)["sequence"] == 3749
    finish_50k(run, "arrival", model, env, summary, 3750, ORIGINAL / "01-arrival-model.json")
    run.ledger["gate"] = summary["gate"]
    print("Arrival visibility gate", summary["gate"], "rewrite overlap", len(summary["overlap"]["rewrite"]), flush=True)
    return summary["gate"]


def run_profile(run, prior):
    del prior
    seed, env = environments()
    order = ("A1", "B1", "B2", "A2")
    run.ledger["order"] = order
    for arm in order:
        model = bench.OUT / f"{arm}-model.json"
        run.seed(arm, model, seed)
        profiled = arm.startswith("B")
        mode = "live-disk50-profile-churn" if profiled else "live-disk50-default-churn"
        log = run.run("fdb", arm + "-30", mode, model, env, monitor=True)
        result = profile.summarize(log, profiled)
        result["label"] = arm
        bench.save(bench.OUT / f"{arm}-timing.json", result)
        finish_50k(run, arm, model, env, result, 3101, ORIGINAL / "02-profile-abba" / f"{arm}-model.json")
        print(arm, ">1 s", len(result["over_1s"]), "seq2511", result["seq2511"]["ms"], flush=True)
    analyze.OUT = bench.OUT
    bench.save(bench.OUT / "outlier-detail.json", [analyze.analyze(arm) for arm in ("B1", "B2")])
    run.cap_check()
    return True


def run_continuous(run, prior):
    del prior
    seed, env = environments()
    label = "continuous"
    model = bench.OUT / "continuous-model.json"
    run.seed(label, model, seed)
    continuous.OUT = bench.OUT
    continuous.EXE = run.exe
    result = continuous.summarize(continuous.timed(run, model, env))
    bench.save(bench.OUT / "disk-summary.json", result)
    finish_50k(run, label, model, env, result, 3101, ORIGINAL / "03-continuous/continuous-model.json")
    run.ledger["gate"] = result["gate"]
    print("Continuous disk gate", result["gate"], "over rounds", result["over_rounds"], flush=True)
    return result["gate"]


def run_pin(run, prior):
    pin.OUT = bench.OUT
    _, env = environments(10_000)
    model = bench.OUT / "10k-model.json"
    # The first 10k executable stopped at the corpus-size assertion, before churn.
    assert bench.read(prior)["sequence"] == 20
    shutil.copyfile(prior, model)
    run.begin("long-pin", model)
    log = run.run("fdb", "long-pin-120s", "live-disk50-long-pin", model, env, monitor=True)
    samples = list(bench.rows(log, "pin_sample"))
    assert [row["target_s"] for row in samples] == [5, 30, 120]
    assert all(row["manifest_stable"] for row in samples)
    release, = bench.rows(log, "release")
    assert release["held_s"] >= 120
    held, = bench.rows(log, "held_oracle")
    before, = bench.rows(log, "before_release_total")
    after, = bench.rows(log, "after_release")
    events = list(bench.rows(log, "inventory_event"))
    result = dict(
        samples=samples, release=release, held=held, before=before, after=after,
        publications=list(bench.rows(log, "revision_published")),
        live_oracle=list(bench.rows(log, "live_disk50_verified")),
        disk_gate=all(row["retained_to_live"] <= 2 for row in events + samples + [before]),
        pinned_cleanup_verified=bool(release["pending_files_at_120s"] and release["all_pinned_files_gone_ms"] is not None),
    )
    bench.save(bench.OUT / "summary.json", result)
    assert bench.read(model)["sequence"] == 40
    assert len(result["live_oracle"]) == 1 and result["live_oracle"][0]["oracle"]
    result["oracle"] = pin.oracle(run, model, env)
    bench.save(bench.OUT / "summary.json", result)
    run.slot("fdb", "long-pin", "ready", model)
    run.active = None
    run.cap_check()
    run.ledger.update(disk_gate=result["disk_gate"], cleanup=result["pinned_cleanup_verified"], frozen_reader=True)
    print("Pin disk gate", result["disk_gate"], "cleanup", result["pinned_cleanup_verified"], flush=True)
    return result["disk_gate"] and result["pinned_cleanup_verified"]


def main():
    ledger = dict(start_utc=bench.utc(), completed=[])
    registry = bench.read(bench.REGISTRY)
    assert all(slot["status"] == "ready" for slot in registry["slots"].values())
    assert (bench.SLOTS / ".fdb-benchmark-owned").read_text(encoding="utf-8-sig").strip() == "FDB benchmark workspace v1"
    assert marker("fdb").is_file()
    sqlite_inventory = inventory("sqlite")
    sqlite_entry = registry["slots"]["sqlite"].copy()
    audit = bench.read(bench.AUDIT)
    assert bench.sha(bench.CORPUS) == registry["sha256"] == audit["payload_sha256"]
    assert bench.sha(bench.CATALOG) == audit["catalog_sha256"]
    assert bench.sha(bench.FROZEN) == FROZEN_SHA
    fixtures = bench.BASE.protected()
    old_model = Path(registry["slots"]["fdb"]["state"])
    old_sha = bench.sha(old_model)
    assert old_sha == registry["slots"]["fdb"]["model_sha256"]
    bench.save(OUT / "prior-slot-registry.json", registry)
    origin = datetime.fromisoformat(sys.argv[1]) if len(sys.argv) > 1 else datetime.now(timezone.utc)
    try:
        # The historical arrival layout was overwritten by later experiments.
        # Recreate it with the original logical workload and check its model hash.
        run = Run(registry, "00-recreate", 20 * 60, 64, 50_000, origin)
        error = None
        try:
            model = rebuild_arrival_start(run)
        except BaseException as exc:
            error = exc
            raise
        finally:
            run.close(error)
        ledger["completed"].append("00-recreate")
        for name, seconds, mib, n, run_step in (
            ("01-arrival", 15 * 60, 64, 50_000, run_arrival),
            ("02-profile-abba", 45 * 60, 192, 50_000, run_profile),
            ("03-continuous", 20 * 60, 64, 50_000, run_continuous),
        ):
            run = Run(registry, name, seconds, mib, n)
            error = None
            try:
                gate = run_step(run, model)
            except BaseException as exc:
                error = exc
                raise
            finally:
                run.close(error)
            ledger["completed"].append(name)
            if not gate:
                ledger.update(status="stopped_gate", step=name)
                return
        ledger["status"] = "ready_for_pin_build"
    except BaseException as error:
        ledger.update(status="stopped", error=repr(error))
        raise
    finally:
        ledger.update(end_utc=bench.utc(), sqlite_unchanged=sqlite_inventory == inventory("sqlite")
                      and sqlite_entry == bench.read(bench.REGISTRY)["slots"]["sqlite"],
                      fixtures_unchanged=bench.BASE.protected() == fixtures,
                      corpus_unchanged=bench.sha(bench.CORPUS) == audit["payload_sha256"],
                      prior_model_unchanged=bench.sha(old_model) == old_sha)
        bench.save(OUT / "status.json", ledger)


def pin_only(origin):
    overall = bench.read(OUT / "status.json")
    failed = OUT / "05-long-pin"
    assert overall["status"] == "stopped"
    failed_status = bench.read(failed / "status.json")
    assert failed_status["start_utc"] == origin.isoformat()
    opened, = [r for r in bench.rows(failed / "long-pin-120s.jsonl", "opened")]
    assert opened["stats"]["sequence"] == 20 and opened["stats"]["documents"] == 10_000
    assert len((failed / "long-pin-120s.jsonl").read_bytes().splitlines()) == 1
    assert "right: 50000" in (failed / "long-pin-120s.stderr").read_text()
    proof, = bench.rows(failed / "long-pin-seed.jsonl", "scale_verified")
    assert proof["documents"] == 10_000 and proof["oracle"]
    registry = bench.read(bench.REGISTRY)
    prior = Path(registry["slots"]["fdb"]["state"])
    assert registry["slots"]["fdb"]["status"] == "blocked_partial_prefix_remeasure"
    assert prior == failed / "10k-model.json" and bench.read(prior)["sequence"] == 20
    before = bench.sha(prior)
    assert before == registry["slots"]["fdb"]["model_sha256"]
    audit = bench.read(bench.AUDIT)
    assert bench.sha(bench.CORPUS) == audit["payload_sha256"] == registry["sha256"]
    assert bench.sha(bench.CATALOG) == audit["catalog_sha256"] and bench.sha(bench.FROZEN) == FROZEN_SHA
    fixtures = bench.BASE.protected()
    sqlite_inventory = inventory("sqlite")
    sqlite_entry = registry["slots"]["sqlite"].copy()
    run = Run(registry, "05-long-pin-resume", 8 * 60, 31, 10_000, origin)
    error = None
    try:
        gate = run_pin(run, prior)
        overall["completed"].append("05-long-pin")
        overall["status"] = "complete" if gate else "stopped_gate"
    except BaseException as exc:
        error = exc
        overall.update(status="stopped", error=repr(exc))
        raise
    finally:
        run.close(error)
        overall.update(end_utc=bench.utc(), sqlite_unchanged=overall["sqlite_unchanged"]
                       and sqlite_inventory == inventory("sqlite")
                       and sqlite_entry == bench.read(bench.REGISTRY)["slots"]["sqlite"],
                       fixtures_unchanged=overall["fixtures_unchanged"] and bench.BASE.protected() == fixtures,
                       corpus_unchanged=overall["corpus_unchanged"] and bench.sha(bench.CORPUS) == audit["payload_sha256"],
                       prior_model_unchanged=overall["prior_model_unchanged"] and bench.sha(prior) == before)
        bench.save(OUT / "status.json", overall)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--pin"]:
        pin_only(datetime.fromisoformat(sys.argv[2]))
    else:
        main()
