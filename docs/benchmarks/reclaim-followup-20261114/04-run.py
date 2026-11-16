"""Isolate the engine child from the 50k corpus, model, and verification."""
from datetime import datetime, timedelta, timezone
import importlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from threading import Thread
import time
import zlib

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[2]
OUT = BASE / "04-ram"
BUILD = ROOT / "target/reclaim-followup-20261114/build/release/examples"
EXE = BUILD / "measure_engine_only.exe"
sys.path.insert(0, str(ROOT / "tools"))
bench = importlib.import_module("measure-live-disk50")
bench.OUT = OUT
bench.ORDER = ("A1-default", "B1-off", "B2-off", "A2-default")
bench.PROGRAMS = bench.VERIFY_PROGRAMS = {"fdb": BUILD / "measure_churn.exe"}
bench.MAX_SECONDS = 25 * 60


def fresh_model():
    return dict(rows={str(i): dict(body=i, revision=0, version=i//500+1) for i in range(50_000)},
                dead={}, next=50_000, sequence=100)


def edits(model):
    for i in range(10_000):
        if i < 5000:
            key, body = i, (i + 997) % 50_000
        elif i < 7500:
            key, body = i, None
        else:
            key = 50_000 + i - 7500
            body = key * 997 % 50_000
        version = 101+i//100
        if body is None:
            model["dead"][str(key)] = model["rows"].pop(str(key))
        else:
            model["rows"][str(key)] = dict(body=body, revision=1, version=version)
        yield key, body, 1
    model.update(next=52_500, sequence=200)


def stream(child, edits_iter, errors, catalog):
    try:
        with child.stdin as pipe, bench.CORPUS.open("rb") as source:
            for id_, body, revision in edits_iter:
                key = dict(scope="shared", collection="texts", id=f"{id_:010}")
                if body is None:
                    pipe.write(("!" + json.dumps(key,separators=(",",":")) + "\n").encode())
                    continue
                record = catalog[body]
                source.seek(record["offset"])
                raw = source.read(record["size"])
                assert zlib.crc32(raw) == record["crc"]
                doc = json.loads(raw)["document"]
                doc["key"] = key
                if revision:
                    marker = f"zqv{id_:010}r{revision:010}zq"
                    limit = 200_000 - len(marker) - 1
                    body_utf8 = doc["text"].encode()
                    doc["text"] = body_utf8[:limit].decode("utf-8", errors="ignore") + " " + marker
                pipe.write((json.dumps(doc,ensure_ascii=False,separators=(",",":")) + "\n").encode())
    except BaseException as error:
        errors.append(error)


def child_run(run, arm, mode, env, entries, catalog):
    log = OUT / f"{arm}-{mode}.jsonl"
    errors = []
    peak = 0
    slot = bench.SLOTS / "fdb"
    started = time.monotonic()
    with log.open("x", encoding="utf-8") as output, log.with_suffix(".stderr").open("x", encoding="utf-8") as error:
        child = subprocess.Popen([EXE, mode], cwd=ROOT, env=env, stdout=output, stderr=error,
                                 stdin=subprocess.PIPE if entries is not None else subprocess.DEVNULL,
                                 creationflags=subprocess.CREATE_NO_WINDOW)
        run.event("process_start", label=f"{arm}-{mode}", pid=child.pid)
        writer = None
        if entries is not None:
            writer = Thread(target=stream, args=(child, entries, errors, catalog))
            writer.start()
        try:
            while child.poll() is None:
                if errors:
                    raise errors[0]
                if time.monotonic() - bench.START >= bench.MAX_SECONDS:
                    raise RuntimeError("RAM time bound")
                if slot.exists():
                    size = stable_size(slot)
                    peak = max(peak, size)
                    if peak >= bench.MAX_SLOT_BYTES:
                        raise RuntimeError("8 GiB slot bound")
                if shutil.disk_usage(bench.SLOTS).free < bench.MIN_FREE_BYTES:
                    raise RuntimeError("15 GiB free-space floor")
                time.sleep(.05)
        finally:
            if child.poll() is None:
                child.kill()
            code = child.wait(timeout=15)
            if writer:
                writer.join(timeout=10)
                assert not writer.is_alive()
            run.event("process_exit", label=f"{arm}-{mode}", exit_code=code,
                      wall_s=time.monotonic()-started, sampled_peak_slot=peak)
    assert not errors and code == 0
    rows = [json.loads(line) for line in log.read_text(encoding="utf-8").splitlines()]
    result = {row["phase"]: row for row in rows}
    assert mode in result and result[mode]["documents"] == 50_000
    assert not result[mode]["snapshot_reader"]
    return result


def stable_size(path):
    size = 0
    for entry in path.iterdir():
        try:
            if entry.is_file():
                size += entry.stat().st_size
        except FileNotFoundError:
            continue
    return size


def main():
    assert bench.read(BASE / "03-continuous/status.json")["gate"] is True
    state = bench.read(bench.REGISTRY)
    slot = state["slots"]["fdb"]
    old_model = Path(slot["state"])
    assert slot["status"] == state["slots"]["sqlite"]["status"] == "ready"
    assert slot["sequence"] == 3101 and bench.sha(old_model) == slot["model_sha256"]
    assert (bench.SLOTS / ".fdb-benchmark-owned").read_text(encoding="utf-8-sig").strip() == "FDB benchmark workspace v1"
    assert shutil.disk_usage(bench.SLOTS).free >= 23 * 2**30
    audit = bench.read(bench.AUDIT)
    assert bench.sha(bench.CORPUS) == state["sha256"] == audit["payload_sha256"]
    assert bench.sha(bench.CATALOG) == audit["catalog_sha256"]
    assert bench.sha(bench.FROZEN) == "B7845966E330624407D66416225F9395ED20272DD0065BAA996E4CC26D93D939"
    catalog = bench.read(bench.CATALOG)["entries"]
    before = bench.BASE.protected()
    OUT.mkdir()
    bench.START = time.monotonic()
    run = bench.Experiment(state)
    start = datetime.now(timezone.utc)
    run.ledger.update(deadline_utc=(start + timedelta(seconds=bench.MAX_SECONDS)).isoformat(),
                      binary_sha=bench.sha(EXE))
    results = []
    base_env = bench.environment()
    try:
        for arm in bench.ORDER:
            default = arm.startswith("A")
            env = dict(base_env)
            if default:
                env.pop("FDB_RECLAMATION_CONTROL")
            else:
                env["FDB_RAM_RECLAIM_OFF"] = "1"
            model_path = OUT / f"{arm}-model.json"
            run.active = ("fdb", arm, model_path)
            run.slot("fdb", arm, "measuring_reclaim_ram", model_path)
            bench.BASE.reset("fdb")
            model = fresh_model()
            ingest = child_run(run, arm, "ingest", env, ((i,i,0) for i in range(50_000)), catalog)
            idle = child_run(run, arm, "idle", env, None, catalog)
            churn = child_run(run, arm, "churn", env, edits(model), catalog)
            bench.save(model_path, model)
            result = dict(label=arm, engine="fdb", default=default, ingest=ingest, idle=idle, churn=churn)
            ingest_mib = ingest["ingest"]["memory"]["peak_private_commit_bytes"]/2**20
            idle_mib = idle["idle"]["memory"]["private_bytes"]/2**20
            result.update(ingest_mib=ingest_mib, idle_mib=idle_mib, gate=ingest_mib<=60 and idle_mib<=15)
            run.run("fdb", f"{arm}-kill", "live-disk50-kill", model_path, env, kill=True, monitor=True)
            assert bench.read(model_path)["sequence"] == 201
            run.verify_arm("fdb", arm, result, model_path, env)
            results.append(result)
            bench.save(OUT / "ram-summary.json", results)
            print(arm, "ingest/idle", ingest_mib, idle_mib, "gate", result["gate"], flush=True)
            if sum(p.stat().st_size for p in OUT.iterdir() if p.is_file()) > 26 * 2**20:
                raise RuntimeError("RAM evidence headroom bound")
        run.ledger.update(status="complete", gate=all(r["gate"] for r in results if r["default"]))
    except BaseException as error:
        run.ledger.update(status="stopped", error=repr(error))
        if run.active:
            engine, arm, model_path = run.active
            run.slot(engine, arm, "blocked_partial_reclaim_ram", model_path)
        raise
    finally:
        run.ledger.update(ended_utc=bench.utc(),
                          corpus_unchanged=bench.sha(bench.CORPUS) == state["sha256"],
                          fixtures_unchanged=bench.BASE.protected() == before,
                          prior_model_unchanged=bench.sha(old_model) == slot["model_sha256"])
        bench.save(OUT / "status.json", run.ledger)


if __name__ == "__main__":
    main()
