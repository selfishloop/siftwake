"""Paired cold/warm timings in the existing benchmark slots."""
from datetime import datetime, timedelta, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
import uuid
import zlib


OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
BUILD = ROOT / "target/cold-open-paired-20261115/build/release"
WORK = ROOT / "data/benchmark-work"
SLOT = {engine: WORK / engine for engine in ("fdb", "sqlite")}
PROBE = {"fdb": BUILD / "examples/measure_cold_case.exe", "sqlite": BUILD / "measure_cold_case.exe"}
ORACLE = ROOT / "docs/benchmarks/wal-prefix-readmission-20261115/DB/DB-legacy.jsonl"
REGISTRY = ROOT / "docs/benchmarks/active-benchmark-slots.json"
CORPUS = Path("D:/frunner-corpus/trwiki-100k-cap200k/corpus.payload-bench")
CATALOG = OUT.parent / "trwiki-cap200k-20261029-v3-catalog.json"
ORDER = ("sqlite", "fdb", "fdb", "sqlite")


def now():
    return datetime.now(timezone.utc)


def save(path, value):
    pending = path.with_suffix(".pending")
    with pending.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2)
    pending.replace(path)


def load(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest().upper()


def inventory(engine):
    assert SLOT[engine].resolve() == SLOT[engine]
    entries = []
    for path in sorted(SLOT[engine].iterdir()):
        assert path.is_file() and not path.is_symlink()
        if engine == "sqlite":
            assert path.name in ("sqlite.db", "sqlite.db-wal", "sqlite.db-shm")
            if path.name == "sqlite.db-wal":
                assert path.stat().st_size == 0
        entries.append(dict(name=path.name, path=str(path), length=path.stat().st_size, sha256=sha(path)))
    return entries


def base_manifest():
    active = max(SLOT["fdb"].glob("generation-*.ready"))
    source = active.read_bytes()
    assert source[:8] == b"FDBMAN03" and zlib.crc32(source[:-4]) == int.from_bytes(source[-4:], "little")
    manifest = json.loads(source[20:-4])
    assert all(r["kind"] == "Run" for r in manifest["runs"])
    mandatory = [active.name, f"generation-{manifest['active']['id']:016x}.fdb"]
    mandatory += [f"run-{r['id']:016x}.fdb" for r in manifest["runs"]]
    return manifest, mandatory


def immutable(entries):
    return {x["name"]: (x["length"], x["sha256"]) for x in entries
            if x["name"].endswith((".fdb", ".ready", ".db"))}


def launch_probe(engine, mode, output, model, env, deadline, hold=False):
    with output.open("x", encoding="utf-8") as stdout, output.with_suffix(".stderr").open("x", encoding="utf-8") as stderr:
        child = subprocess.Popen([PROBE[engine], mode, model, ORACLE], cwd=ROOT, env=env,
                                 stdout=stdout, stderr=stderr, creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            if hold:
                while child.poll() is None and now() < deadline:
                    try:
                        ready = load(output)
                        if ready["kill_ready"]:
                            return ready
                    except (json.JSONDecodeError, KeyError):
                        pass
                    time.sleep(.1)
                raise RuntimeError(f"{engine} kill witness unavailable")
            child.wait(timeout=max(1, (deadline-now()).total_seconds()))
            if child.returncode:
                raise RuntimeError(f"{engine}/{mode} exit {child.returncode}: {output.with_suffix('.stderr').name}")
        finally:
            if child.poll() is None:
                child.kill()
                child.wait(timeout=10)
    result = load(output)
    assert result["oracle"] and result["engine"] == engine and result["sequence"] == 3101
    if mode in ("cold", "warm"):
        assert len(result["query_ms"]) == 30 and result["hits"] == 50 and result["crc32"] == 1149311291
    else:
        assert result["gets"] == 50000 and result["cases"] == 41
    return result


def warm_files(engine):
    buffer = bytearray(1024 * 1024)
    for path in SLOT[engine].iterdir():
        if path.is_file() and path.name != "LOCK":
            with path.open("rb", buffering=0) as source:
                while source.readinto(buffer):
                    pass


def check_trace(directory, env):
    if not (directory / "preflight.etl").is_file():
        return dict(technical_pass=False, errors=["merged ETL missing"])
    decoded = directory / "decoded"
    decoded.mkdir()
    script = ["pwsh.exe", "-NoProfile", "-File", OUT / "decode.ps1", "-Trial", directory]
    with (directory / "decode.log").open("x", encoding="utf-8") as log:
        child = subprocess.run(script, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                               timeout=120, creationflags=subprocess.CREATE_NO_WINDOW)
    proof = decoded / "identity.json"
    if child.returncode or not proof.exists():
        return dict(technical_pass=False, errors=["ETL decode failed"], decode_exit=child.returncode)
    data = load(proof)
    reconciliation = decoded / "window-reconciliation.json"
    if reconciliation.is_file() and load(reconciliation)["exact"]:
        assert data["errors"] == ["merged loss or full event count"] and data["merged_lost"] == 0
        data.update(technical_pass=True, independent_fileio_reconciliation=True)
    return data


def main():
    assert all(exe.is_file() for exe in PROBE.values())
    assert shutil.disk_usage(WORK).free >= 15 * 2**30
    registry = load(REGISTRY)
    slots = registry["slots"]
    assert {s["sequence"] for s in slots.values()} == {3101}
    assert {s["model_sha256"] for s in slots.values()} == {sha(Path(slots["fdb"]["state"]))}
    assert sha(Path(slots["sqlite"]["state"])) == slots["sqlite"]["model_sha256"]
    assert all(s["status"] == "ready" for s in slots.values())
    assert sha(CORPUS) == registry["sha256"]
    start = now()
    deadline = start + timedelta(minutes=30)
    env = {k: v for k, v in os.environ.items() if not k.startswith("FDB_")}
    env.update(FDB_BENCH_CORPUS_PATH=str(CORPUS), FDB_BENCH_CORPUS_INDEX=str(CATALOG),
               FDB_BENCH_ORIGINAL_BODIES="1", FDB_BENCH_50K_WRITE="1",
               FDB_SCALE_DOCUMENTS="50000", FDB_LIVE_DISK50="1",
               FDB_OPEN_COLD_DEADLINE=deadline.isoformat())
    previous = {engine: immutable(inventory(engine)) for engine in SLOT}
    status = dict(start_utc=start.isoformat(), deadline_utc=deadline.isoformat(), order=ORDER, trials=[])
    nonce = uuid.uuid4().hex
    plan = dict(controller_pid=os.getpid(), nonce=nonce, order=ORDER, deadline_utc=deadline.isoformat(),
                profile_sha=sha(OUT / "cold-identity.wprp"), binary_sha={k: sha(v) for k,v in PROBE.items()})
    try:
        for engine in SLOT:
            launch_probe(engine, "verify", OUT / f"{engine}-before.jsonl", slots[engine]["state"], env, deadline)
        manifest, required_fdb = base_manifest()
        status["fdb_runs"] = len(manifest["runs"])
        status["fdb_layout_source"] = slots["fdb"]["label"]
        baseline = {engine: inventory(engine) for engine in SLOT}
        assert previous == {x: immutable(baseline[x]) for x in SLOT}
        required = dict(fdb=required_fdb, sqlite=["sqlite.db"])
        known_sqlite = {entry["name"] for entry in baseline["sqlite"]}
        baseline["sqlite"] += [
            dict(name=name, path=str(SLOT["sqlite"] / name))
            for name in ("sqlite.db-shm", "sqlite.db-wal") if name not in known_sqlite
        ]
        save(OUT / "cold-inventory.json", dict(files={
            e: dict(entries=baseline[e], required=required[e]) for e in SLOT
        }))
        save(OUT / "plan.json", plan)
        print(f"Ready. UTC window ends {deadline.isoformat()}.", flush=True)
        for i,engine in enumerate(ORDER,1):
            directory = OUT / f"trial-{i:02}"
            directory.mkdir()
            file_plan = baseline[engine]
            save(directory / "ready.json", dict(trial=i, engine=engine, nonce=nonce,
                 expected_files=len(required[engine]), files=len(file_plan)))
            while not (directory / "go").exists():
                if now() >= deadline:
                    raise TimeoutError("Administrator purge handoff not received")
                time.sleep(.05)
            model = slots[engine]["state"]
            cold_env = {**env, "FDB_COLD_TRIAL_DIR":str(directory)}
            cold = launch_probe(engine, "cold", directory / "cold.jsonl", model, cold_env, deadline)
            warm_files(engine)
            warm = launch_probe(engine, "warm", directory / "warm.jsonl", model, env, deadline)
            unchanged = immutable(inventory(engine)) == previous[engine]
            save(directory / "reader-result.json", dict(oracle=True, source_and_model_unchanged=unchanged))
            assert unchanged, f"{engine} authoritative files changed"
            trial = dict(trial=i, engine=engine, cold=cold, warm=warm, trace_status="pending")
            status["trials"].append(trial)
            save(OUT / "progress.json", status)
            print(f"{i}/4 {engine}: {cold['open_start_to_first_ms']:.2f} ms cold timing collected", flush=True)

        for trial in status["trials"]:
            directory = OUT / f"trial-{trial['trial']:02}"
            try:
                proof = check_trace(directory, env)
                trial["trace_status"] = "storage_attributed" if proof["technical_pass"] else "unverified"
                trial["trace"] = {k: proof.get(k) for k in ("errors","merged_lost","matched_files",
                    "expected_files","total_attributed_disk_bytes","foreign_count","ambiguous_count")}
            except Exception as issue:
                trial["trace_status"] = "unverified"
                trial["trace"] = dict(errors=[repr(issue)])
        for engine in SLOT:
            model = slots[engine]["state"]
            witness = launch_probe(engine, "hold", OUT / f"{engine}-kill.jsonl", model, env, deadline, hold=True)
            assert witness["oracle"] and witness["sequence"] == 3101
            launch_probe(engine, "verify", OUT / f"{engine}-after.jsonl", model, env, deadline)
        status["status"] = "complete"
    except BaseException as issue:
        status.update(status="partial", error=repr(issue))
        raise
    finally:
        status.update(ended_utc=now().isoformat(), source_unchanged=all(
            immutable(inventory(x)) == previous[x] for x in SLOT))
        save(OUT / "status.json", status)
        (OUT / "done").touch()


if __name__ == "__main__":
    main()
