"""Join the stock tracerpt CSV events for one armed case-0 trial."""

import bisect
import collections
import csv
import datetime
import json
import pathlib
import re
import statistics
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/benchmarks/cold-etw-main-20261107"
WINDOWS_EPOCH = 116444736000000000


def load(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def rows(path):
    with path.open(encoding="utf-8-sig", errors="replace", newline="") as raw:
        for n, row in enumerate(csv.reader(raw, skipinitialspace=True), 1):
            if len(row) >= 20 and row[16].strip().isdigit():
                yield n, row[0].strip(), row[1].strip(), int(row[16]), row


def compact(n, kind, opcode, tick, row):
    return dict(csv_row=n, kind=kind, opcode=opcode, filetime=tick,
                pid=row[9].strip(), tid=row[10].strip(), data=row[19:])


def filetime(ns):
    return WINDOWS_EPOCH + int(ns) // 100


def iso_filetime(value):
    stamp = datetime.datetime.fromisoformat(value.replace("Z", "+00:00"))
    return WINDOWS_EPOCH + int(stamp.timestamp() * 10_000_000)


def summarize(samples):
    ordered = sorted(samples)
    return {"p50_ms": statistics.median(ordered),
            "p95_ms": ordered[(95 * len(ordered) + 99) // 100 - 1],
            "max_ms": ordered[-1]}


def main():
    trial = int(sys.argv[1])
    assert 1 <= trial <= 8
    directory = OUT / f"trial-{trial:02}"
    run = load(OUT / "run.json")
    admin = load(directory / "admin-result.json")
    measured = load(directory / "measurement.json")
    engine = run["order"][trial - 1]
    assert admin["run_id"] == run["run_id"] and measured["engine"] == engine
    cold, warm = measured["cold"], measured["warm"]
    start, end = (filetime(cold[key]) for key in
                  ("open_start_unix_ns", "first_result_unix_ns"))
    purge_end = iso_filetime(admin["et_utc"])
    pid = cold["pid"]
    source = directory / "decoded.tmp.csv"
    known = {entry["path"][2:].lower(): entry["name"]
             for entry in run["files"][engine]["entries"]}
    known = {r"\device\harddiskvolume3" + key: name for key, name in known.items()}
    required = set(run["files"][engine]["required"])
    seen = set()
    history = {}
    keys = collections.defaultdict(set)
    init = collections.defaultdict(list)
    disks = []
    foreign = []
    header = None
    evidence = []
    for n, kind, opcode, tick, row in rows(source):
        data = [value.strip() for value in row[19:]]
        event_pid = int(row[9], 16)
        if kind == "EventTrace" and opcode == "Header":
            header = (tick, int(data[4]))
        if kind == "FileIo" and opcode == "Create" and len(data) >= 7:
            name = known.get(re.sub(r"\\+", r"\\", data[6]).lower())
            history[data[1]] = name
            if name:
                evidence.append(compact(n, kind, opcode, tick, row))
                if purge_end <= tick < start and event_pid != pid:
                    foreign.append(n)
        if kind == "FileIo" and opcode == "Close" and len(data) >= 2:
            if history.pop(data[1], None):
                evidence.append(compact(n, kind, opcode, tick, row))
        if kind == "FileIo" and opcode == "Read" and len(data) >= 6:
            name = history.get(data[2])
            if name:
                evidence.append(compact(n, kind, opcode, tick, row))
                if start <= tick <= end and event_pid == pid:
                    seen.add(name)
                    keys[data[3]].add(name)
                if purge_end <= tick < start and event_pid != pid:
                    foreign.append(n)
        if kind == "DiskIo" and opcode == "ReadInit" and start <= tick <= end:
            init[data[0]].append((tick, event_pid, data[1], compact(n, kind, opcode, tick, row)))
        if kind == "DiskIo" and opcode == "Read" and start <= tick <= end:
            disks.append((tick, data, compact(n, kind, opcode, tick, row)))

    storage = collections.Counter()
    ambiguous = []
    for tick, data, event in disks:
        names = keys.get(data[5], set())
        if not names:
            continue
        candidates = sorted(init[data[6]])
        at = bisect.bisect_right([entry[0] for entry in candidates], tick) - 1
        if len(names) != 1 or at < 0:
            ambiguous.append(event["csv_row"])
            continue
        _, issued_pid, issued_tid, issued = candidates[at]
        if issued_tid != data[8]:
            ambiguous.append(event["csv_row"])
        elif issued_pid == pid:
            storage[next(iter(names))] += int(data[2])
            evidence.extend((issued, event))
    summary = (directory / "tracerpt-summary.txt").read_text(encoding="utf-8-sig", errors="replace")
    lost = re.search(r"Total Events\s+Lost\s+(\d+)", summary)
    lost = int(lost.group(1)) if lost else None
    errors = []
    if any(admin[key] != 0 for key in ("es_exit", "et_exit", "wpr_start_exit", "wpr_stop_exit")) or admin["failure"]:
        errors.append("admin lifecycle failed")
    if lost != 0:
        errors.append("merged lost events missing or nonzero")
    if (directory / "decoder-stop.json").exists():
        errors.append("tracerpt CSV export stopped at the decoder reserve")
    if not header or not header[0] <= start <= end <= header[1] or header[1] - header[0] > 600_000_000:
        errors.append("trace coverage or duration")
    if required - seen:
        errors.append("expected reads incomplete")
    if not any(n.endswith((".fdb", ".idx", ".db")) and size > 0 for n, size in storage.items()):
        errors.append("no attributed target data storage read")
    if ambiguous or foreign:
        errors.append("ambiguous attribution or foreign pre-open access")
    with (directory / "target-events.jsonl").open("x", encoding="utf-8") as out:
        for row in evidence:
            out.write(json.dumps(row, ensure_ascii=False) + "\n")
    verdict = dict(run_id=run["run_id"], trial=trial, engine=engine, valid=not errors,
                   errors=errors, merged_events_lost=lost, expected_reads_complete=required <= seen,
                   missing_reads=sorted(required - seen), expected_reads=len(required),
                   attributed_bytes=dict(storage), foreign_rows=foreign, ambiguous_rows=ambiguous,
                   open_start_to_first_ms=cold["open_start_to_first_ms"],
                   first_result_gate=cold["open_start_to_first_ms"] <= 1000,
                   cold_q30_ms=cold["query_ms"][29], warm_q30_ms=warm["query_ms"][29],
                   warm_a=summarize(warm["query_ms"][30:90]),
                   warm_b=summarize(warm["query_ms"][90:150]),
                   trace_filetime=header, retained_evidence_rows=len(evidence),
                   loss_evidence="collector bazı ölçülemedi")
    with (directory / "verdict.json").open("x", encoding="utf-8") as out:
        json.dump(verdict, out, ensure_ascii=False, indent=2)
    print(json.dumps(verdict, ensure_ascii=False))


if __name__ == "__main__":
    main()
