"""Summarize the three-round timing trace without opening the database."""
import collections
import csv
import json
import math
from pathlib import Path


OUT = Path(__file__).resolve().parent
MIB = 2**20
MS = 1_000_000


def read_phase(path, phase):
    with path.open(encoding="utf-8-sig") as stream:
        return [item for line in stream if (item := json.loads(line))["phase"] == phase]


def quantile(values, q):
    return sorted(values)[math.ceil(q * len(values)) - 1] if values else None


def describe(values):
    return dict(count=len(values), sum=sum(values), p50=quantile(values, .50),
                p95=quantile(values, .95), max=max(values, default=None))


def rejected(admission):
    a = admission
    checks = {
        "layer_off": not a["streamed_wal"],
        "owned_delta": a["owned_delta"],
        "stage_keys": a["stage_keys"] > 128,
        "stage_bytes": a["stage_bytes"] > 8 * MIB,
        "shadow_limit": a["shadow_keys"] + a["stage_keys"] > 2048,
        "wal_limit": a["wal_bytes"] + a["stage_bytes"] > 64 * MIB,
        "live_keys": a["shadow_keys"] >= a["max_delta_keys"],
        "live_bytes": a["live_bytes"] >= a["max_delta_bytes"],
    }
    return [name for name, failed in checks.items() if failed]


def overlap(phase, start, end):
    return max(0, min(phase["end_ns"], end) - max(phase["start_ns"], start))


def write_table(name, rows):
    with (OUT / name).open("w", newline="", encoding="utf-8") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def commit_baseline(path):
    rounds = read_phase(path, "live_disk50_round")[:3]
    values = [commit["ms"] for row in rounds for commit in row["commits"]]
    assert len(values) == 300
    return {**describe(values), "p99": quantile(values, .99),
            "over_1s": sum(value > 1000 for value in values)}


def main():
    raw = OUT / "01-profile/first-pass.jsonl"
    profile, = read_phase(raw, "followup_profile")
    trace = profile["trace"]
    workers = trace["workers"]
    raw_frames = trace["commits"]
    assert workers["lost"] == workers["prefix_lost"] == raw_frames["dropped_frames"] == 0
    samples = {sample["output"]: sample for sample in workers["prefix"]}
    spans = collections.defaultdict(dict)
    for item in workers["aux"]:
        if item["kind"] == "prefix" and item["phase"] in ("worker.wall", "source.prepare", "index.prepare", "budget.source_workspace", "budget.builder_workspace"):
            assert item["phase"] not in spans[item["id"]]
            spans[item["id"]][item["phase"]] = item
    worker_rows = {}
    for wid, sample in samples.items():
        worker = spans[wid]["worker.wall"]
        first_pass = sample["first_pass_end_ns"] - sample["first_pass_start_ns"]
        assert first_pass >= sample["wal_rows_ns"] + sample["add_builder_ns"]
        worker_rows[wid] = dict(
            output=wid, capture_sequence=sample["capture_sequence"],
            captured_keys=sample["captured_keys"], captured_payload_bytes=sample["captured_payload_bytes"],
            captured_wal_bytes=sample["captured_wal_bytes"],
            worker_ms=(worker["end_ns"]-worker["start_ns"]) / MS, first_pass_ms=first_pass / MS,
            wal_rows_ms=sample["wal_rows_ns"] / MS, add_builder_ms=sample["add_builder_ns"] / MS,
            first_pass_cpu_ms=None if sample["first_pass_cpu_ns"] is None else sample["first_pass_cpu_ns"] / MS,
            worker_cpu_ms=None if sample["worker_cpu_ns"] is None else sample["worker_cpu_ns"] / MS,
            source_ms=(spans[wid]["source.prepare"]["end_ns"] - spans[wid]["source.prepare"]["start_ns"]) / MS,
            index_ms=(spans[wid]["index.prepare"]["end_ns"] - spans[wid]["index.prepare"]["start_ns"]) / MS,
        )
    joins = []
    frames = []
    for frame in raw_frames["frames"]:
        seq = frame["operation_sequence"]
        a = frame["admission"]
        failures = rejected(a)
        phases = frame["phases"]
        fall = any(p["name"] == "stream.worker_drain" for p in phases)
        assert bool(failures) == fall, (seq, failures, fall)
        def total(*names):
            return sum(p["elapsed_ns"] for p in phases if p["name"] in names) / MS
        frames.append(dict(
            sequence=seq, commit_ms=frame["elapsed_ns"] / MS, fallback=fall,
            failed_checks="|".join(failures), **a,
            prefix_join_ms=total("prefix.worker_join"), merge_join_ms=total("merge.join"),
            fallback_drain_ms=total("stream.worker_drain"),
            fallback_delta_flush_ms=total("stream.delta_flush"),
            stage_ms=total("stream.stage_frame"),
        ))
        for j in (p for p in phases if p["name"] == "prefix.worker_join"):
            wid = j["active_wal_id"]
            sample = samples[wid]
            begin, end = j["start_ns"], j["end_ns"]
            first_overlap_ms = overlap(j, sample["first_pass_start_ns"], sample["first_pass_end_ns"]) / MS
            first_pass_ns = sample["first_pass_end_ns"] - sample["first_pass_start_ns"]
            unobserved_or_outside_join_ns = first_pass_ns - overlap(
                j, sample["first_pass_start_ns"], sample["first_pass_end_ns"])
            builder_overlap_min_ms = max(0, sample["add_builder_ns"] - unobserved_or_outside_join_ns) / MS
            builder_overlap_max_ms = min(sample["add_builder_ns"], overlap(
                j, sample["first_pass_start_ns"], sample["first_pass_end_ns"])) / MS
            joins.append(dict(
                sequence=seq, commit_ms=frame["elapsed_ns"]/ MS,
                failed_checks="|".join(failures), join_ms=j["elapsed_ns"] / MS,
                drained=fall and any(p["name"] == "stream.worker_drain" and p["start_ns"] <= begin <= end <= p["end_ns"] for p in phases),
                age_ms=(begin - sample["worker_start_ns"]) / MS,
                first_pass_overlap_ms=first_overlap_ms,
                builder_overlap_min_ms=builder_overlap_min_ms,
                builder_overlap_max_ms=builder_overlap_max_ms,
                source_overlap_ms=overlap(spans[wid]["source.prepare"], begin, end) / MS,
                index_overlap_ms=overlap(spans[wid]["index.prepare"], begin, end) / MS,
                source_workspace_overlap_ms=overlap(spans[wid]["budget.source_workspace"], begin, end) / MS,
                builder_workspace_overlap_ms=overlap(spans[wid]["budget.builder_workspace"], begin, end) / MS,
                **worker_rows[wid],
            ))
    slow = [j for j in joins if j["join_ms"] >= 50]
    waited_workers = [worker_rows[wid] for wid in dict.fromkeys(j["output"] for j in slow)]
    assert len(frames) == 300 and slow
    write_table("commits.csv", frames)
    write_table("workers.csv", list(worker_rows.values()))
    write_table("joins.csv", joins)
    groups = {}
    for label, rows in (("all_workers", list(worker_rows.values())), ("waited_workers", waited_workers)):
        groups[label] = {field: describe([r[field] for r in rows if r[field] is not None])
                         for field in ("worker_ms", "first_pass_ms", "wal_rows_ms", "add_builder_ms",
                                       "first_pass_cpu_ms", "worker_cpu_ms", "source_ms", "index_ms", "captured_keys", "captured_payload_bytes")}
    round_rows = read_phase(raw, "live_disk50_round")
    summary = dict(
        frames=describe([f["commit_ms"] for f in frames]),
        wall_commit=commit_baseline(raw),
        archived_first_three={
            label: commit_baseline(OUT.parent / "normal-merge-defer-20261114" / "01-commit" / f"{label}-30.jsonl")
            for label in ("A1-control", "A2-control")
        },
        observed_throughput=30_000 / (sum(row["write_ms"] + row["maintenance_ms"] for row in round_rows) / 1000),
        fallbacks=dict(collections.Counter(f["failed_checks"] for f in frames if f["fallback"])),
        waits=describe([j["join_ms"] for j in slow]),
        compile_and_run_profile_scope="mechanism only, one profiled arm",
        timing_tails_over_1s=sum(f["commit_ms"] > 1000 for f in frames),
        slow_wait_failed_checks=dict(collections.Counter(j["failed_checks"] for j in slow)),
        slow_wait_overlap={field: sum(j[field] for j in slow) for field in (
            "first_pass_overlap_ms", "builder_overlap_min_ms", "builder_overlap_max_ms",
            "source_overlap_ms", "index_overlap_ms",
            "source_workspace_overlap_ms", "builder_workspace_overlap_ms")},
        **groups,
        examples=sorted(slow, key=lambda row: row["join_ms"], reverse=True)[:6],
    )
    (OUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({name: v for name, v in summary.items() if name not in ("all_workers", "examples")}, indent=2))


if __name__ == "__main__":
    main()
